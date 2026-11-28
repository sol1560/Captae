//! 登录窗口在 Windows 上的实现（WebView2），login.rs 用（做法和原因见 docs/DESIGN.md 第 5.1 节）。
//!
//! - 登录存储：网站那块用单独的 WebView2 用户数据目录（`WebviewBuilder::data_directory`）。WebView2 的规矩是同一个数据目录的几个
//!   environment 设置要一样；不同目录就是不同的 environment、不同的浏览器进程，同一个窗口里放两块不同目录的 webview 没问题
//!   （wry 0.55 每块 webview 都自己建 environment，见 `wry::webview2::InnerWebView::create_environment`）。
//! - 读、删 cookie：直接用 webview2-com 调 `ICoreWebView2CookieManager`。不用 Tauri / wry 的 cookie 接口：
//!   它们经过 cookie 包，`.youtube.com` 前面的点会被去掉，分不清「只给这一个主机」和「连子域名」，删也删不掉带点的；
//!   wry 的 `cookies()` 还在主线程里等结果（`wait_with_pump`），从同步命令里调会卡住。这里都是异步的：在主线程发起，结果从完成回调里交出去。
//! - 拦截：wry 给每块 webview 都把 Tauri 的协议（`http://asset.localhost/…`、`ipc.`、`tauri.`）挂成 WebResourceRequested 过滤器，
//!   网站那块的网页也能用 `<img src="http://asset.localhost/…">` 读到 Captae 能读的文件（和 macOS 上 asset: 一样的问题）。
//!   在网站那块把这几个过滤器摘掉，事件就不会再来，wry 的处理函数也就不会给它提供内容（请求会照常发到网络上，
//!   `*.localhost` 指向本机，和普通浏览器里一样）。不另加自己的过滤器去回 403：WebResourceRequested 的处理函数是挨个调的，
//!   wry 的那个先拿到请求、异步回内容，自己的回应会被它盖掉。
//!
//! 这里的函数都只能在主线程调（Tauri 的 `with_webview` 回调就在主线程）。

use captae_core::login::Cookie;
use std::cell::RefCell;
use std::rc::Rc;
use webview2_com::Microsoft::Web::WebView2::Win32::{
    ICoreWebView2, ICoreWebView2Controller, ICoreWebView2Cookie, ICoreWebView2CookieList, ICoreWebView2CookieManager, ICoreWebView2Settings2, ICoreWebView2_2, ICoreWebView2_22, ICoreWebView2_8,
    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL, COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
};
use webview2_com::{take_pwstr, GetCookiesCompletedHandler};
use windows_core::{Interface, BOOL, HSTRING, PCWSTR, PWSTR};

type WinResult<T> = windows_core::Result<T>;

fn webview(c: &ICoreWebView2Controller) -> WinResult<ICoreWebView2> {
    unsafe { c.CoreWebView2() }
}

fn cookie_manager(c: &ICoreWebView2Controller) -> WinResult<ICoreWebView2CookieManager> {
    unsafe { webview(c)?.cast::<ICoreWebView2_2>()?.CookieManager() }
}

fn text(get: impl FnOnce(*mut PWSTR) -> WinResult<()>) -> WinResult<String> {
    let mut p = PWSTR::null();
    get(&mut p)?;
    Ok(take_pwstr(p))
}

fn flag(get: impl FnOnce(*mut BOOL) -> WinResult<()>) -> WinResult<bool> {
    let mut b = BOOL::default();
    get(&mut b)?;
    Ok(b.as_bool())
}

/// WebView2 的 cookie → 核心库的。域名原样带着点：`.youtube.com` 连子域名，`x.com` 只给这一个主机（WebView2 的文档：这三种算不同的域名）
fn convert(c: &ICoreWebView2Cookie) -> WinResult<Cookie> {
    unsafe {
        let session = flag(|b| c.IsSession(b))?;
        let mut exp = 0f64;
        c.Expires(&mut exp)?;
        Ok(Cookie {
            name: text(|p| c.Name(p))?,
            value: text(|p| c.Value(p))?,
            domain: text(|p| c.Domain(p))?,
            path: text(|p| c.Path(p))?,
            secure: flag(|b| c.IsSecure(b))?,
            http_only: flag(|b| c.IsHttpOnly(b))?,
            host_only: false,
            // 会话 cookie 的 Expires 是 -1
            expires: (!session && exp > 0.0).then_some(exp as i64),
        })
    }
}

fn list(l: Option<ICoreWebView2CookieList>) -> WinResult<Vec<(ICoreWebView2Cookie, Cookie)>> {
    let Some(l) = l else { return Ok(vec![]) };
    let mut n = 0u32;
    unsafe { l.Count(&mut n)? };
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        let raw = unsafe { l.GetValueAtIndex(i)? };
        // 个别读不出来的跳过，不让整批失败
        if let Ok(c) = convert(&raw) {
            out.push((raw, c));
        }
    }
    Ok(out)
}

/// 读出登录存储里全部 cookie（连同 WebView2 的原对象，删的时候用原对象，域名带不带点都准）。`done` 只调一次
fn get_all(c: &ICoreWebView2Controller, done: impl FnOnce(Result<Vec<(ICoreWebView2Cookie, Cookie)>, String>) + 'static) {
    // 发起失败时完成回调不会来，自己调；发起成功就交给完成回调
    let done = Rc::new(RefCell::new(Some(done)));
    let d2 = done.clone();
    let handler = GetCookiesCompletedHandler::create(Box::new(move |hr, l| {
        let r = hr.and_then(|_| list(l)).map_err(|e| e.to_string());
        if let Some(f) = d2.borrow_mut().take() {
            f(r);
        }
        Ok(())
    }));
    // uri 为空：这个用户数据目录（profile）下的全部 cookie
    let started = cookie_manager(c).and_then(|m| unsafe { m.GetCookies(PCWSTR::null(), &handler) });
    if let Err(e) = started {
        if let Some(f) = done.borrow_mut().take() {
            f(Err(e.to_string()));
        }
    }
}

/// 读出全部 cookie
pub fn all(c: &ICoreWebView2Controller, done: impl FnOnce(Result<Vec<Cookie>, String>) + 'static) {
    get_all(c, move |r| done(r.map(|v| v.into_iter().map(|(_, c)| c).collect())));
}

/// 删掉 `pick` 选中的 cookie，交出删了几个（`DeleteCookie` 是同步的，删完马上生效）
pub fn delete(c: &ICoreWebView2Controller, pick: impl Fn(&Cookie) -> bool + 'static, done: impl FnOnce(Result<usize, String>) + 'static) {
    let m = match cookie_manager(c) {
        Ok(m) => m,
        Err(e) => return done(Err(e.to_string())),
    };
    get_all(c, move |r| {
        done(r.and_then(|v| {
            let mut n = 0;
            for (raw, ck) in v.iter().filter(|(_, ck)| pick(ck)) {
                unsafe { m.DeleteCookie(raw) }.map_err(|e| format!("{}: {e}", ck.name))?;
                n += 1;
            }
            Ok(n)
        }))
    });
}

/// 测试钩子用：往登录存储里放一个 cookie（域名原样，`.example.com` 就是连子域名的）
#[cfg(debug_assertions)]
pub fn add(c: &ICoreWebView2Controller, ck: &Cookie) -> Result<(), String> {
    (|| unsafe {
        let m = cookie_manager(c)?;
        let raw = m.CreateCookie(&HSTRING::from(&ck.name), &HSTRING::from(&ck.value), &HSTRING::from(&ck.domain), &HSTRING::from(&ck.path))?;
        raw.SetIsSecure(ck.secure)?;
        if let Some(t) = ck.expires {
            raw.SetExpires(t as f64)?;
        }
        m.AddOrUpdateCookie(&raw)
    })()
    .map_err(|e| e.to_string())
}

/// Tauri 挂在每块 webview 上的协议（tauri.conf.json 没开 useHttpsScheme 时是 http，开了是 https，两种都摘）
const PROTOCOLS: [&str; 3] = ["asset", "ipc", "tauri"];

/// 网站那块：摘掉 Tauri 协议的过滤器（见文件开头）。asset 的摘掉了才算成功（这时才许打开网站）
pub fn drop_protocol_filters(c: &ICoreWebView2Controller) -> bool {
    let Ok(wv) = webview(c) else { return false };
    let wv22 = wv.cast::<ICoreWebView2_22>().ok();
    let mut asset_gone = false;
    for name in PROTOCOLS {
        for scheme in ["http", "https"] {
            // 和 wry 加的时候一样的写法：`http://asset.*`（`custom_protocol_workaround::work_around_uri_prefix` + `*`）
            let filter = HSTRING::from(format!("{scheme}://{name}.*"));
            // wry 在 WebView2 够新时用带来源种类的接口加（iframe、Shared Worker 里的请求也拦得到），摘的时候也要用对应的接口
            let a = wv22.as_ref().is_some_and(|w| unsafe { w.RemoveWebResourceRequestedFilterWithRequestSourceKinds(&filter, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL, COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL) }.is_ok());
            let b = unsafe { wv.RemoveWebResourceRequestedFilter(&filter, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL) }.is_ok();
            if name == "asset" && (a || b) {
                asset_gone = true;
            }
        }
    }
    asset_gone
}

/// 这块 webview 实际用的 User-Agent（WebView2 自己的，和 Edge 浏览器的写法一样）
/// 整块 webview 静音（看不见的读取窗口用，见 page_reader.rs）。WebView2 1.0.1072 起才有，没有就返回 false
pub fn mute(c: &ICoreWebView2Controller) -> bool {
    unsafe { webview(c).and_then(|w| w.cast::<ICoreWebView2_8>()).and_then(|w| w.SetIsMuted(true)).is_ok() }
}

pub fn user_agent(c: &ICoreWebView2Controller) -> Option<String> {
    let s = unsafe { webview(c).ok()?.Settings().ok()?.cast::<ICoreWebView2Settings2>().ok()? };
    text(|p| unsafe { s.UserAgent(p) }).ok().filter(|s| !s.is_empty())
}
