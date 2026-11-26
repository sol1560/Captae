//! 登录窗口在 Linux 上的实现（WebKitGTK），login.rs 用（做法和原因见 docs/DESIGN.md 第 5.1 节）。
//!
//! - 登录存储：网站那块用单独的数据目录（`WebviewBuilder::data_directory`）。Tauri 按数据目录分 `WebContext`，
//!   wry 给每个有数据目录的 `WebContext` 建自己的 `WebsiteDataManager`（缓存、数据都放这个目录，cookie 存成这个目录下的 `cookies` 文本文件），
//!   和 Captae 界面的存储完全分开（`wry::webkitgtk::web_context::WebContextImpl::new`）。
//! - 读、删 cookie：直接调这块 webview 的 `WebKitCookieManager`。不用 Tauri / wry 的 cookie 接口：它们经过 cookie 包，
//!   `.youtube.com` 前面的点会被去掉，删不掉带点的 cookie；wry 的接口还在主线程里转 GTK 的事件循环等结果。这里都是异步的：
//!   在主线程发起，结果从完成回调里交出去。读全部 cookie 的 `webkit_cookie_manager_get_all_cookies`（WebKitGTK 2.42 起）
//!   webkit2gtk 包里没有，照 wry 的写法自己声明（wry 本来就链接了它，没有新的系统要求）。
//! - 拦截：WebKitGTK 的内容拦截规则（`WebKitUserContentFilterStore`）和 Safari 的 WKContentRuleList 是同一种 JSON，
//!   直接用 login.rs 里 macOS 那份规则。编好的规则存在登录存储目录下，启动时编一次（`prepare`），建网站那块时挂上。
//!
//! 这里的函数都只能在 GTK 主线程调（Tauri 的 `with_webview` 回调就在主线程）。

use captae_core::login::Cookie;
use std::cell::{Cell, RefCell};
use std::ffi::CString;
use std::path::Path;
use std::rc::Rc;
use webkit2gtk::glib::translate::{FromGlibPtrContainer, ToGlibPtr};
use webkit2gtk::{gio, glib, CookieManager, CookieManagerExt, WebView, WebViewExt, WebsiteDataManagerExt};

type Done<T> = Box<dyn FnOnce(Result<T, String>)>;

extern "C" {
    // WebKitGTK 2.42 起有；webkit2gtk 2.0 包里没有绑定（和 wry 的 `cookies()` 同样的声明）
    fn webkit_cookie_manager_get_all_cookies(cm: *mut webkit2gtk::ffi::WebKitCookieManager, cancellable: *mut gio::ffi::GCancellable, callback: gio::ffi::GAsyncReadyCallback, user_data: glib::ffi::gpointer);
    fn webkit_cookie_manager_get_all_cookies_finish(cm: *mut webkit2gtk::ffi::WebKitCookieManager, result: *mut gio::ffi::GAsyncResult, error: *mut *mut glib::ffi::GError) -> *mut glib::ffi::GList;
}

fn cookie_manager(wv: &WebView) -> Result<CookieManager, String> {
    wv.website_data_manager().and_then(|m| m.cookie_manager()).ok_or_else(|| "这块 webview 没有 cookie 管理器".to_string())
}

/// libsoup 的 cookie → 核心库的。域名原样：libsoup 给连子域名的 cookie 存成 `.youtube.com`，只给一个主机的没有点
fn convert(c: &mut soup::Cookie) -> Cookie {
    Cookie {
        name: c.name().map(|s| s.to_string()).unwrap_or_default(),
        value: c.value().map(|s| s.to_string()).unwrap_or_default(),
        domain: c.domain().map(|s| s.to_string()).unwrap_or_default(),
        path: c.path().map(|s| s.to_string()).unwrap_or_else(|| "/".into()),
        secure: c.is_secure(),
        http_only: c.is_http_only(),
        host_only: false,
        expires: c.expires().map(|d| d.to_unix()),
    }
}

/// 读出全部 cookie（libsoup 的原对象，删的时候用原对象，域名带不带点都准）。`done` 只调一次
fn get_all(wv: &WebView, done: Done<Vec<soup::Cookie>>) {
    let cm = match cookie_manager(wv) {
        Ok(cm) => cm,
        Err(e) => return done(Err(e)),
    };
    unsafe extern "C" fn finished(src: *mut glib::gobject_ffi::GObject, res: *mut gio::ffi::GAsyncResult, data: glib::ffi::gpointer) {
        let done: Box<Done<Vec<soup::Cookie>>> = Box::from_raw(data as *mut Done<Vec<soup::Cookie>>);
        let mut err = std::ptr::null_mut();
        let list = webkit_cookie_manager_get_all_cookies_finish(src as *mut _, res, &mut err);
        if err.is_null() {
            let v: Vec<soup::Cookie> = FromGlibPtrContainer::from_glib_full(list);
            done(Ok(v));
        } else {
            let e: glib::Error = glib::translate::from_glib_full(err);
            done(Err(e.to_string()));
        }
    }
    let data = Box::into_raw(Box::new(done)) as glib::ffi::gpointer;
    unsafe {
        let p: *mut webkit2gtk::ffi::WebKitCookieManager = cm.to_glib_none().0;
        webkit_cookie_manager_get_all_cookies(p, std::ptr::null_mut(), Some(finished), data);
    }
}

/// 读出全部 cookie
pub fn all(wv: &WebView, done: impl FnOnce(Result<Vec<Cookie>, String>) + 'static) {
    get_all(wv, Box::new(move |r| done(r.map(|v| v.into_iter().map(|mut c| convert(&mut c)).collect()))));
}

/// 删掉 `pick` 选中的 cookie，全部删完交出删了几个
pub fn delete(wv: &WebView, pick: impl Fn(&Cookie) -> bool + 'static, done: impl FnOnce(Result<usize, String>) + 'static) {
    let cm = match cookie_manager(wv) {
        Ok(cm) => cm,
        Err(e) => return done(Err(e)),
    };
    get_all(
        wv,
        Box::new(move |r| {
            let victims: Vec<soup::Cookie> = match r {
                Ok(v) => v.into_iter().filter(|c| pick(&convert(&mut c.clone()))).collect(),
                Err(e) => return done(Err(e)),
            };
            let n = victims.len();
            if n == 0 {
                return done(Ok(0));
            }
            let done: Rc<RefCell<Option<Done<usize>>>> = Rc::new(RefCell::new(Some(Box::new(done))));
            let left = Rc::new(Cell::new(n));
            let failed: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
            for mut c in victims {
                let (left, done, failed) = (left.clone(), done.clone(), failed.clone());
                cm.delete_cookie(&mut c, None::<&gio::Cancellable>, move |r| {
                    if let Err(e) = r {
                        failed.borrow_mut().get_or_insert(e.to_string());
                    }
                    left.set(left.get() - 1);
                    if left.get() == 0 {
                        if let Some(f) = done.borrow_mut().take() {
                            match failed.borrow_mut().take() {
                                Some(e) => f(Err(e)),
                                None => f(Ok(n)),
                            }
                        }
                    }
                });
            }
        }),
    );
}

/// 测试钩子用：往登录存储里放一个 cookie（域名原样，`.example.com` 就是连子域名的）
#[cfg(debug_assertions)]
pub fn add(wv: &WebView, ck: &Cookie, done: impl FnOnce(Result<(), String>) + 'static) {
    let cm = match cookie_manager(wv) {
        Ok(cm) => cm,
        Err(e) => return done(Err(e)),
    };
    let mut c = soup::Cookie::new(&ck.name, &ck.value, &ck.domain, &ck.path, -1);
    c.set_secure(ck.secure);
    if let Some(t) = ck.expires.and_then(|t| glib::DateTime::from_unix_utc(t).ok()) {
        c.set_expires(&t);
    }
    cm.add_cookie(&mut c, None::<&gio::Cancellable>, move |r| done(r.map_err(|e| e.to_string())));
}

// ------------------------------------------------------------------ 拦截规则

/// 编好的规则的名字（规则改了就换一个，免得用到存储里旧的那份）
const FILTER_ID: &str = "captae-login-site-v2";

thread_local! {
    /// 编好的规则（持有一个引用，程序退出前一直用）
    static FILTER: Cell<*mut webkit2gtk::ffi::WebKitUserContentFilter> = const { Cell::new(std::ptr::null_mut()) };
    static ASKED: Cell<bool> = const { Cell::new(false) };
}

/// 编译规则（主线程，只编一次；编好以前 `attach` 返回 false）。`dir`：存编好的规则的目录
pub fn prepare(dir: &Path, rules: &'static str) {
    if ASKED.with(|a| a.replace(true)) {
        return;
    }
    let _ = std::fs::create_dir_all(dir);
    let (Ok(path), Ok(id)) = (CString::new(dir.to_string_lossy().as_bytes()), CString::new(FILTER_ID)) else {
        ASKED.with(|a| a.set(false));
        return;
    };
    unsafe extern "C" fn saved(src: *mut glib::gobject_ffi::GObject, res: *mut gio::ffi::GAsyncResult, _: glib::ffi::gpointer) {
        let mut err = std::ptr::null_mut();
        let f = webkit2gtk::ffi::webkit_user_content_filter_store_save_finish(src as *mut _, res, &mut err);
        if f.is_null() {
            let why = if err.is_null() {
                String::new()
            } else {
                let e: glib::Error = glib::translate::from_glib_full(err);
                e.to_string()
            };
            tracing::warn!("网站那块的拦截规则编译失败：{why}");
            // 下次再试着编一次
            ASKED.with(|a| a.set(false));
        } else {
            FILTER.with(|x| x.set(f));
        }
        // 存储本身用完了（编好的规则自己持有需要的东西）
        glib::gobject_ffi::g_object_unref(src);
    }
    unsafe {
        let store = webkit2gtk::ffi::webkit_user_content_filter_store_new(path.as_ptr());
        if store.is_null() {
            tracing::warn!("没有拿到 WebKitGTK 的拦截规则存储");
            ASKED.with(|a| a.set(false));
            return;
        }
        let bytes = glib::Bytes::from_static(rules.as_bytes());
        let b: *mut glib::ffi::GBytes = bytes.to_glib_none().0;
        webkit2gtk::ffi::webkit_user_content_filter_store_save(store, id.as_ptr(), b, std::ptr::null_mut(), Some(saved), std::ptr::null_mut());
    }
}

/// 加到这块 webview 上（主线程）。规则还没编好时返回 false
pub fn attach(wv: &WebView) -> bool {
    let f = FILTER.with(|x| x.get());
    if f.is_null() {
        return false;
    }
    let Some(ucm) = wv.user_content_manager() else { return false };
    unsafe {
        let p: *mut webkit2gtk::ffi::WebKitUserContentManager = ucm.to_glib_none().0;
        webkit2gtk::ffi::webkit_user_content_manager_add_filter(p, f);
    }
    true
}
