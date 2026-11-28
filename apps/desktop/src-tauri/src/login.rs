//! 登录窗口和登录存储（做法和原因见 docs/DESIGN.md 第 5.1 节）。
//!
//! - 登录窗口：单独的系统窗口，同一时间只开一个（再要求登录别的网站时换内容）。窗口里两块 webview：
//!   `login-bar` 是 Captae 自己的界面（index.html#/login-bar，铺满窗口：上面工具条、下面一行说明），
//!   `login-site` 是网站自己的页面，盖在中间（单独的数据存储，所有网站共用这一个；User-Agent 见 `login::login_user_agent`）。
//!   窗口的出现和消失交给系统，页面里不加动画（docs/MOTION.md g）。三个系统都用系统自带的标题栏。
//! - 登录存储：
//!   - macOS：`WKWebsiteDataStore(forIdentifier:)`。读、删 cookie 直接用 objc2 调 WebKit：Tauri 的 cookie 接口会把
//!     `.youtube.com` 前面的点去掉，按这个删不掉真正的 cookie。窗口关了也照样能读。
//!   - Windows、Linux：网站那块用单独的数据目录 `<数据目录>/login-store`（WebView2 的用户数据目录、WebKitGTK 的数据目录），
//!     读、删 cookie 直接调 WebView2 / WebKitGTK 的 CookieManager（`login_win.rs`、`login_linux.rs`）。这两个系统上存储挂在 webview 上，
//!     窗口没开时（下载、设置里列网站）建一块藏起来的 webview（`login-store`，同一个数据目录）来读，闲 2 分钟就关掉。
//! - 安全：网站那块的来源是远程网址，Tauri 2.11 对远程来源一律走权限检查，它没有任何 capability，Captae 的命令都调不了；
//!   main.rs 的 invoke_handler 外面还有一层白名单（`allowed`）。网站那块一开始打开的是一个 `.invalid` 地址：
//!   asset 协议的跨域头取的是 webview 最初的来源，这样永远对不上真实网站，网站的脚本读不到本地文件。
//!   网站那块只许打开 http / https（iframe 里的 about: / data: / blob: 也放行），tauri: / asset: / file: 这些一律不许。
//!   网站那块还要挡住页面里的图片、视频、脚本从 asset: / ipc: / tauri: / file: 加载
//!   （实测 `<img src="asset://localhost/主目录下的图片">` 能显示出来，网站就能试探你下载过哪些视频）：
//!   macOS 用 WebKit 的内容拦截规则（`blocker`），Linux 用 WebKitGTK 的同一种规则，Windows 摘掉 wry 给这块挂的 `http://asset.localhost` 一类过滤器。
//!   挡不上就不打开网站。
//! - 判断登录成功：每秒读一次这个网站的 cookies，登录 cookie 出现（或者换了新的）就关窗、重试等着的任务。
//!   人机验证没有固定的 cookie，等用户点「完成」。「取消」= 关窗，任务留在失败状态。

use crate::App;
use captae_core::login::{self, Cookie, How};
use captae_core::tr;
use serde::Serialize;
use serde_json::{json, Value};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewUrl};

#[cfg(windows)]
#[path = "login_win.rs"]
mod win;
#[cfg(target_os = "linux")]
#[path = "login_linux.rs"]
mod gtk;

pub const WINDOW: &str = "login";
pub const BAR: &str = "login-bar";
pub const SITE: &str = "login-site";
/// Windows、Linux：登录窗口没开时读写登录存储用的那块藏起来的 webview（它自己一个窗口，同样的标签）。什么命令都不能调（`allowed`）
pub const STORE: &str = "login-store";
/// 藏起来的那块闲了多久关掉（秒）
#[cfg(any(windows, target_os = "linux"))]
const STORE_IDLE: i64 = 120;
/// Captae 那一层：上面工具条占多高、下面那行说明占多高（逻辑像素），网站夹在中间。和 theme.css 的 .cp-loginbar / .cp-loginfoot 对应
const TOP: f64 = 58.0;
const BOTTOM: f64 = 52.0;
/// 网站那块最初打开的地址（见文件开头）
pub(crate) const START: &str = "https://captae-login.invalid/";

type R<T> = Result<T, String>;
fn e<E: std::fmt::Display>(x: E) -> String {
    x.to_string()
}
fn now_secs() -> i64 {
    captae_core::util::now_ms() / 1000
}

/// 登录窗口里的 webview 能调哪些命令（main.rs 的 invoke_handler 先过这一层）。网站那块什么都不能调
pub fn allowed(webview: &str, cmd: &str) -> bool {
    match webview {
        "main" => true,
        BAR => matches!(cmd, "login_bar_state" | "login_done" | "login_cancel" | "js_log"),
        // 网站那块，以及以后新加、没在这里登记的 webview：一律不许（默认拒绝，免得新窗口不小心拿到全部命令）
        _ => false,
    }
}

// ------------------------------------------------------------------ 状态

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Login,
    Verify,
}

#[derive(Debug, Clone)]
struct Session {
    /// 第几次打开（轮询据此知道自己是不是过期了）
    gen: u64,
    /// 网站代号：youtube / host:example.com
    key: String,
    kind: Kind,
    /// 等这个网站登录 / 验证的任务
    jobs: Vec<u64>,
    /// 网站那块现在打开的主机名，工具条上显示（让用户看得到自己在哪个网站输密码）
    host: String,
    secure: bool,
    /// 打开时已有的登录 cookie（名字=值）：出现新的才算登录成功（打开前就登录着的，等用户点「完成」）
    before: Vec<String>,
}

/// 登录窗口的状态（`app.manage`）
#[derive(Default)]
pub struct Login {
    cur: Mutex<Option<Session>>,
    next: AtomicU64,
    /// 窗口是 Captae 自己关的（完成、取消），还没收到它的 Destroyed。
    /// 这时来的 Destroyed 不算「用户点了关闭」，免得把紧接着打开的新一次也当成取消（Destroyed 的回调用掉它）
    closing: AtomicBool,
    /// 同上，给下一次 `login_open` 用：先等旧窗口真的从 Tauri 里消失再开新的（`login_open` 用掉它）。
    /// 和 `closing` 分开，因为 Tauri 移除窗口和调 Destroyed 回调不是同一处，先后不一定
    closing_wait: AtomicBool,
    /// Windows、Linux：读写登录存储一次只做一件（建、关藏起来的那块 webview 也在这把锁里，免得建两个、或者用到正在关的）
    #[cfg(any(windows, target_os = "linux"))]
    ops: tokio::sync::Mutex<()>,
    /// Windows、Linux：上次读写登录存储的时间（秒），藏起来的那块据此闲久了关掉
    #[cfg(any(windows, target_os = "linux"))]
    last_use: std::sync::atomic::AtomicI64,
    /// 正在关窗、等网站那块的空白页载入完（见 `destroy_window`）。网站那块的 on_page_load 看到空白页载入完就用掉它
    unloaded: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

/// 关窗时最多等网站那块的空白页载入多久，过了就直接销毁
const UNLOAD_WAIT: Duration = Duration::from_secs(2);

/// 自己关掉登录窗口（先记下是自己关的）：先藏起来、卸掉网页，等网站那块换成空白页再真正销毁。
///
/// 为什么不直接销毁：macOS 上 wry 销毁 webview 时故意多 retain 一次（wry 0.55 的 `InnerWebView::drop`，0.57 也一样），
/// WKWebView 永远不会释放，网页在看不见的地方接着跑：在抖音验证窗口里点「完成」以后视频还在出声，只能退出 Captae
/// （2026-11-28 测试者录屏）。换成空白页再销毁，留下的只是一块空白的 webview。
/// 跳空白页和销毁紧挨着发出去不行：空白页还没载入，窗口就没了，旧网页照样在跑
/// （2026-11-28 实测，本机测试页不静音地播音频、每秒报到一次：改之前关窗以后 10 秒里还报到二十多次；紧挨着发也一样；等载入完再销毁就停了。
/// 测试页静音时看不出来：WebKit 自己会暂停不在屏幕上的静音媒体）
fn destroy_window(app: &AppHandle) {
    let Some(w) = app.get_window(WINDOW) else { return };
    let st = app.state::<Login>();
    st.closing.store(true, Ordering::SeqCst);
    st.closing_wait.store(true, Ordering::SeqCst);
    let _ = w.hide();
    let site = app.get_webview(SITE).is_some();
    let (tx, rx) = tokio::sync::oneshot::channel();
    *st.unloaded.lock().unwrap() = Some(tx);
    unload_pages(app);
    // 销毁的是这一个窗口（拿着它本身，不按名字找），等待期间新开的登录窗口不会被误关
    tauri::async_runtime::spawn(async move {
        let t = std::time::Instant::now();
        let ok = site && matches!(tokio::time::timeout(UNLOAD_WAIT, rx).await, Ok(Ok(())));
        tracing::info!("登录窗口：网页{}卸掉，{} 毫秒后销毁", if ok { "已经" } else { "没等到" }, t.elapsed().as_millis());
        let _ = w.destroy();
    });
}

/// 退出 Captae 时：不等网页卸掉，马上销毁（不然应用退不出去；进程马上就结束了，网页跑不了多久）
fn destroy_window_now(app: &AppHandle) {
    if let Some(w) = app.get_window(WINDOW) {
        let st = app.state::<Login>();
        st.closing.store(true, Ordering::SeqCst);
        st.closing_wait.store(true, Ordering::SeqCst);
        let _ = w.destroy();
    }
}

/// 把两块 webview 里的网页卸掉：先停掉声音和视频（马上安静下来），再换成空白页。
/// 在主线程上调时当场执行；在别的线程上调时按顺序排队
fn unload_pages(app: &AppHandle) {
    for label in [SITE, BAR] {
        if let Some(w) = app.get_webview(label) {
            let _ = w.eval("document.querySelectorAll('video,audio').forEach(m=>{try{m.muted=true;m.pause()}catch(e){}})");
            if let Ok(u) = "about:blank".parse() {
                let _ = w.navigate(u);
            }
        }
    }
}

fn bar_state(s: &Session) -> Value {
    json!({
        "kind": s.kind, "site": s.key, "name": login::key_name(&s.key), "host": s.host, "secure": s.secure,
        "waiting": s.jobs.len(),
        // 工具条用和主界面一样的语言
        "lang": captae_core::i18n::current().code(),
        // 有登录规则的网站：登录好会自己关窗
        "auto": s.kind == Kind::Login && login::site_by_id(&s.key).is_some(),
    })
}

/// 工具条的状态推过去（工具条页面加载好以前推不到，它自己会调 login_bar_state 来取）
fn push_bar(app: &AppHandle, s: &Session) {
    if let Some(w) = app.get_webview(BAR) {
        let _ = w.eval(&format!("window.captaeLoginBar&&window.captaeLoginBar({})", bar_state(s)));
    }
}

/// 告诉主窗口：开始等 / 登录好了 / 关了（界面据此显示和取消「正在等你在弹出的窗口里登录」）。
/// `how`：结束时算登录了还是验证过（done 才有）
fn tell_main(app: &AppHandle, state: &str, s: &Session, retried: usize, how: Option<How>) {
    let _ = app.emit_to("main", "captae://event", json!({"type": "login", "state": state, "site": s.key, "name": login::key_name(&s.key), "kind": s.kind, "jobs": s.jobs, "retried": retried, "how": how}));
}

/// 登录窗口的标题：「登录 YouTube」「登录抖音」「在 X 验证」。站点名是存着的原文（「哔哩哔哩」、主机名），按现在的语言显示；
/// 中文里夹英文名时两边的空格由 tr! 自动加（i18n.rs）
fn title_of(s: &Session) -> String {
    let name = captae_core::i18n::dynamic(&login::key_name(&s.key));
    match s.kind {
        Kind::Login => tr!("登录{site}", site = name),
        Kind::Verify => tr!("在{site}验证", site = name),
    }
}

// ------------------------------------------------------------------ 登录存储

#[cfg(target_os = "macos")]
pub(crate) fn store_id() -> [u8; 16] {
    login::store_uuid(std::env::var_os("CAPTAE_HOME").map(std::path::PathBuf::from).as_deref())
}

/// macOS 14 以下没有按 UUID 分开的存储，登录窗口和 Captae 界面共用默认存储（wry 也是这么退的）。
/// Windows、Linux 按数据目录分开，一直是单独的
pub fn separate_store() -> bool {
    #[cfg(target_os = "macos")]
    return wk::separate();
    #[cfg(not(target_os = "macos"))]
    true
}

/// Windows、Linux：登录存储的目录 `<数据目录>/login-store`（测试版设了 CAPTAE_HOME 时跟着换，碰不到正式版的）。
/// 只有自己能读（Linux 上 WebKitGTK 把 cookie 明文存在这里面的 `cookies` 文件里）
#[cfg(any(windows, target_os = "linux"))]
pub(crate) fn store_dir(app: &AppHandle) -> anyhow::Result<std::path::PathBuf> {
    let a = app.try_state::<App>().ok_or_else(|| anyhow::anyhow!(tr!("Captae 还没启动好，请稍后再试")))?;
    let d = a.core.paths.data.join("login-store");
    std::fs::create_dir_all(&d)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&d, std::fs::Permissions::from_mode(0o700));
    }
    Ok(d)
}

/// Linux：编好的拦截规则放哪（登录存储目录下，和 WebKitGTK 自己的子目录名字错开）
#[cfg(target_os = "linux")]
fn filter_dir(app: &AppHandle) -> anyhow::Result<std::path::PathBuf> {
    Ok(store_dir(app)?.join("captae-filters"))
}

/// Windows、Linux：拿一块能读写登录存储的 webview。登录窗口开着（还有这一次登录）就用网站那块；没开就用（或建）藏起来的 `login-store`。
/// 登录刚结束时网站那块还在、正要关掉：不用它。点「完成」以后紧接着重试的任务马上要读存储，交给正在关的那块的回调不会再跑，
/// 读出来是「登录存储没有回应」，任务就不带登录状态重试（2026-11-25 Devin 的 Windows 上 L3、L4 实测）。
/// `avoid_site`：上一次用网站那块没回应，这次一定用藏起来的那块。调用方要拿着 `Login::ops` 那把锁
#[cfg(any(windows, target_os = "linux"))]
async fn store_webview(app: &AppHandle, avoid_site: bool) -> anyhow::Result<tauri::Webview> {
    let live = app.state::<Login>().cur.lock().unwrap().is_some();
    if live && !avoid_site {
        if let Some(w) = app.get_webview(SITE) {
            return Ok(w);
        }
    }
    if let Some(w) = app.get_webview(STORE) {
        return Ok(w);
    }
    let dir = store_dir(app)?;
    // 和网站那块同一个数据目录、同样的设置（Windows 上同一个用户数据目录的几个 WebView2 environment 设置必须一样：两边都用 Tauri 的默认值）。
    // 不打开任何网页（about:blank 时 Tauri 不加载），哪儿也不许去；不显示、不进任务栏
    let w = tauri::WebviewWindowBuilder::new(app, STORE, WebviewUrl::External("about:blank".parse()?))
        .title("Captae")
        .visible(false)
        .focused(false)
        .skip_taskbar(true)
        .inner_size(320.0, 240.0)
        .data_directory(dir)
        .on_navigation(|u| u.scheme() == "about")
        .build()?;
    tracing::info!("建了读登录存储用的隐藏 webview");
    // Windows：顺便读出 WebView2 实际的 User-Agent，下载时用同一个
    #[cfg(windows)]
    let _ = w.with_webview(|pv| {
        if let Some(ua) = win::user_agent(&pv.controller()) {
            login::set_login_user_agent(ua);
        }
    });
    drop(w);
    spawn_store_closer(app.clone());
    app.get_webview(STORE).ok_or_else(|| anyhow::anyhow!(tr!("读登录存储用的窗口没建好")))
}

/// 藏起来的那块闲了 `STORE_IDLE` 秒就关掉（每建一次起一个）
#[cfg(any(windows, target_os = "linux"))]
fn spawn_store_closer(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            let st = app.state::<Login>();
            let _g = st.ops.lock().await;
            let Some(w) = app.get_window(STORE) else { return };
            if now_secs() - st.last_use.load(Ordering::SeqCst) < STORE_IDLE {
                continue;
            }
            let _ = w.destroy();
            // 在锁里等它真的从 Tauri 里消失，免得下一次读写拿到正要关掉的那块（最多 2 秒）
            for _ in 0..40 {
                if app.get_window(STORE).is_none() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
            tracing::info!("关掉了读登录存储用的隐藏 webview");
            return;
        }
    });
}

/// Windows、Linux：在能读写登录存储的那块 webview 上做一件事（主线程），等它从回调里交出结果（最多 10 秒）
#[cfg(any(windows, target_os = "linux"))]
async fn on_store<T: Send + 'static>(app: &AppHandle, timeout: String, f: impl FnOnce(tauri::webview::PlatformWebview, Box<dyn FnOnce(Result<T, String>) + Send>) + Send + 'static) -> anyhow::Result<T> {
    let st = app.state::<Login>();
    let _g = st.ops.lock().await;
    st.last_use.store(now_secs(), Ordering::SeqCst);
    // 回调可能要再调一次（第一次那块 webview 正好在关、回调被丢掉），所以先放进 Arc 里
    let f = std::sync::Arc::new(Mutex::new(Some(f)));
    for avoid_site in [false, true] {
        let wv = store_webview(app, avoid_site).await?;
        let was_site = wv.label() == SITE;
        let (tx, rx) = tokio::sync::oneshot::channel();
        let f2 = f.clone();
        // 回调真的跑了才取走 f；webview 正在关、回调被丢掉时 f 还留在 Arc 里，下一轮接着用
        wv.with_webview(move |pv| {
            if let Some(f) = f2.lock().unwrap().take() {
                f(
                    pv,
                    Box::new(move |r| {
                        let _ = tx.send(r);
                    }),
                )
            }
        })?;
        match tokio::time::timeout(Duration::from_secs(10), rx).await {
            Err(_) => anyhow::bail!(timeout),
            Ok(Ok(r)) => return r.map_err(|e| anyhow::anyhow!(e)),
            // 回调没跑（webview 正在关）、f 还没被取走：换藏起来的那块再来一次
            Ok(Err(_)) if was_site && f.lock().unwrap().is_some() => {
                tracing::info!("网站那块没回应（正在关），改用藏起来的那块读登录存储");
                continue;
            }
            Ok(Err(_)) => break,
        }
    }
    anyhow::bail!(tr!("登录存储没有回应"))
}

#[cfg(target_os = "macos")]
mod wk {
    //! 只能在主线程调（WebKit 的规矩），结果从完成回调里交出去。
    use block2::RcBlock;
    use captae_core::login::Cookie;
    use objc2::rc::Retained;
    use objc2::MainThreadMarker;
    use objc2_foundation::{NSArray, NSHTTPCookie, NSOperatingSystemVersion, NSProcessInfo, NSUUID};
    use objc2_web_kit::{WKHTTPCookieStore, WKWebsiteDataStore};
    use std::cell::{Cell, RefCell};
    use std::ptr::NonNull;
    use std::rc::Rc;

    thread_local! {
        /// 拿到一次就一直留着（每次重新取再丢掉，存储可能在回调来之前就被释放）
        static STORE: RefCell<Option<Retained<WKWebsiteDataStore>>> = const { RefCell::new(None) };
    }

    pub fn separate() -> bool {
        NSProcessInfo::processInfo().isOperatingSystemAtLeastVersion(NSOperatingSystemVersion { majorVersion: 14, minorVersion: 0, patchVersion: 0 })
    }

    fn cookie_store(id: [u8; 16]) -> Option<Retained<WKHTTPCookieStore>> {
        let mtm = MainThreadMarker::new()?;
        STORE.with(|s| {
            let mut s = s.borrow_mut();
            let store = s.get_or_insert_with(|| unsafe {
                if separate() {
                    WKWebsiteDataStore::dataStoreForIdentifier(&NSUUID::from_bytes(id), mtm)
                } else {
                    WKWebsiteDataStore::defaultDataStore(mtm)
                }
            });
            Some(unsafe { store.httpCookieStore() })
        })
    }

    fn convert(c: &NSHTTPCookie) -> Cookie {
        Cookie {
            name: c.name().to_string(),
            value: c.value().to_string(),
            // NSHTTPCookie 的域名原样带着点：`.youtube.com` 是子域名也带，`x.com` 只给这一个主机
            domain: c.domain().to_string(),
            path: c.path().to_string(),
            secure: c.isSecure(),
            http_only: c.isHTTPOnly(),
            host_only: false,
            expires: c.expiresDate().map(|d| d.timeIntervalSince1970() as i64),
        }
    }

    /// 读出全部 cookie
    pub fn all(id: [u8; 16], done: impl FnOnce(Vec<Cookie>) + 'static) {
        let Some(cs) = cookie_store(id) else { return done(vec![]) };
        let done = RefCell::new(Some(done));
        let block = RcBlock::new(move |arr: NonNull<NSArray<NSHTTPCookie>>| {
            let arr = unsafe { arr.as_ref() };
            let v: Vec<Cookie> = arr.to_vec().iter().map(|c| convert(c)).collect();
            if let Some(f) = done.borrow_mut().take() {
                f(v);
            }
        });
        unsafe { cs.getAllCookies(&block) };
    }

    /// 删掉 `pick` 选中的 cookie（拿 WebKit 给的原对象去删，域名带不带点都准），删完交出删了几个
    pub fn delete(id: [u8; 16], pick: impl Fn(&Cookie) -> bool + 'static, done: impl FnOnce(usize) + 'static) {
        let Some(cs) = cookie_store(id) else { return done(0) };
        let done: Rc<RefCell<Option<Box<dyn FnOnce(usize)>>>> = Rc::new(RefCell::new(Some(Box::new(done))));
        let cs2 = cs.clone();
        let block = RcBlock::new(move |arr: NonNull<NSArray<NSHTTPCookie>>| {
            let arr = unsafe { arr.as_ref() };
            let victims: Vec<Retained<NSHTTPCookie>> = arr.to_vec().into_iter().filter(|c| pick(&convert(c))).collect();
            let n = victims.len();
            if n == 0 {
                if let Some(f) = done.borrow_mut().take() {
                    f(0);
                }
                return;
            }
            let left = Rc::new(Cell::new(n));
            for c in victims {
                let (left, done) = (left.clone(), done.clone());
                let cb = RcBlock::new(move || {
                    left.set(left.get() - 1);
                    if left.get() == 0 {
                        if let Some(f) = done.borrow_mut().take() {
                            f(n);
                        }
                    }
                });
                unsafe { cs2.deleteCookie_completionHandler(&c, Some(&cb)) };
            }
        });
        unsafe { cs.getAllCookies(&block) };
    }
}

/// 登录存储里的全部 cookie（任何线程都能调；值只在内存里）
pub async fn all_cookies(app: &AppHandle) -> anyhow::Result<Vec<Cookie>> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let id = store_id();
        app.run_on_main_thread(move || {
            wk::all(id, move |v| {
                let _ = tx.send(v);
            })
        })?;
        Ok(tokio::time::timeout(Duration::from_secs(10), rx).await.map_err(|_| anyhow::anyhow!(tr!("读登录存储超时")))??)
    }
    #[cfg(windows)]
    return on_store(app, tr!("读登录存储超时"), |pv, done| win::all(&pv.controller(), done)).await;
    #[cfg(target_os = "linux")]
    return on_store(app, tr!("读登录存储超时"), |pv, done| gtk::all(&pv.inner(), done)).await;
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = app;
        Ok(vec![])
    }
}

/// 删掉和这些域名有关的 cookie，返回删了几个
async fn delete_cookies(app: &AppHandle, domains: Vec<String>) -> anyhow::Result<usize> {
    #[cfg(target_os = "macos")]
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let id = store_id();
        app.run_on_main_thread(move || {
            wk::delete(
                id,
                move |c| domains.iter().any(|d| c.related_to(d)),
                move |n| {
                    let _ = tx.send(n);
                },
            )
        })?;
        Ok(tokio::time::timeout(Duration::from_secs(10), rx).await.map_err(|_| anyhow::anyhow!(tr!("清除登录状态超时")))??)
    }
    // 拿存储给的原对象去删（域名带不带点都准，`.youtube.com` 也删得掉）
    #[cfg(windows)]
    return on_store(app, tr!("清除登录状态超时"), move |pv, done| win::delete(&pv.controller(), move |c| domains.iter().any(|d| c.related_to(d)), done)).await;
    #[cfg(target_os = "linux")]
    return on_store(app, tr!("清除登录状态超时"), move |pv, done| gtk::delete(&pv.inner(), move |c| domains.iter().any(|d| c.related_to(d)), done)).await;
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = (app, domains);
        anyhow::bail!(tr!("这个系统还不支持在 Captae 里登录"))
    }
}

/// 给核心库用的登录存储（下载时按网站的域名取 cookies）
pub struct DesktopStore(pub AppHandle);

impl login::Store for DesktopStore {
    fn cookies(&self, domains: Vec<String>) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<Cookie>>> + Send + 'static>> {
        let app = self.0.clone();
        Box::pin(async move {
            let all = all_cookies(&app).await?;
            Ok(all.into_iter().filter(|c| domains.iter().any(|d| c.related_to(d))).collect())
        })
    }
}

/// 这个网站的登录 cookie（名字=值，排好序）：比较打开前后，出现新的就算登录成功
fn login_marks(key: &str, cookies: &[Cookie]) -> Vec<String> {
    let Some(site) = login::site_by_id(key) else { return vec![] };
    let now = now_secs();
    let mut v: Vec<String> = cookies
        .iter()
        .filter(|c| site.login_cookies.contains(&c.name.as_str()) && !c.value.is_empty() && !c.expired(now) && site.domains.iter().any(|d| login::host_under(c.bare_domain(), d)))
        .map(|c| format!("{}={}", c.name, c.value))
        .collect();
    v.sort();
    v.dedup();
    v
}

// ------------------------------------------------------------------ 网站那块的拦截规则

/// 网站那块不许加载 Captae 自己的协议。Tauri 给每块 webview 都挂了 asset: 协议（主窗口显示缩略图、视频要用），
/// 网页的 `<img>` `<video>` 不受跨域限制，能把主目录下的图片、视频显示出来（读不到内容，但能知道在不在、多大，
/// 2026-11-24 实测）。macOS 用 WebKit 的内容拦截规则（WKContentRuleList）挡掉，Linux 用 WebKitGTK 的同一种规则
/// （UserContentFilterStore，见 login_linux.rs），只加在网站那块上。Windows 的做法不一样，见 login_win.rs。
///
/// asset: ipc: tauri: file:，以及 Windows / Android 上 http(s)://asset.localhost 那种写法。
/// WebKit 的 url-filter 不支持 `|`，每个协议单独一条
#[cfg(any(target_os = "macos", target_os = "linux", test))]
const RULES: &str = r#"[
    {"trigger": {"url-filter": "^asset:"}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^ipc:"}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^tauri:"}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^file:"}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://asset\\.localhost"}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://ipc\\.localhost"}, "action": {"type": "block"}},
    {"trigger": {"url-filter": "^https?://tauri\\.localhost"}, "action": {"type": "block"}}
]"#;

#[cfg(target_os = "macos")]
mod blocker {
    use super::RULES;
    use block2::RcBlock;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::NSString;
    use std::cell::{Cell, RefCell};

    const ID: &str = "captae-login-site-v2";

    thread_local! {
        static LIST: RefCell<Option<Retained<AnyObject>>> = const { RefCell::new(None) };
        static ASKED: Cell<bool> = const { Cell::new(false) };
    }

    /// 编译规则（主线程，只编一次；编好以前 `attach` 返回 false）
    pub fn prepare() {
        if ASKED.with(|a| a.replace(true)) {
            return;
        }
        unsafe {
            let store: *mut AnyObject = msg_send![class!(WKContentRuleListStore), defaultStore];
            if store.is_null() {
                tracing::warn!("没有拿到 WebKit 的拦截规则存储");
                return;
            }
            let id = NSString::from_str(ID);
            let rules = NSString::from_str(RULES);
            let done = RcBlock::new(|list: *mut AnyObject, err: *mut AnyObject| match Retained::retain(list) {
                Some(l) => LIST.with(|x| *x.borrow_mut() = Some(l)),
                None => {
                    let why = if err.is_null() {
                        String::new()
                    } else {
                        let d: *mut NSString = msg_send![&*err, localizedDescription];
                        if d.is_null() { String::new() } else { (*d).to_string() }
                    };
                    tracing::warn!("网站那块的拦截规则编译失败：{why}");
                    // 下次再试着编一次
                    ASKED.with(|a| a.set(false));
                }
            });
            let _: () = msg_send![&*store, compileContentRuleListForIdentifier: &*id, encodedContentRuleList: &*rules, completionHandler: &*done];
        }
    }

    /// 加到这块 webview 上（主线程）。规则还没编好时返回 false
    pub fn attach(wk: *mut AnyObject) -> bool {
        if wk.is_null() {
            return false;
        }
        LIST.with(|l| {
            let Some(list) = l.borrow().clone() else { return false };
            unsafe {
                let cfg: *mut AnyObject = msg_send![&*wk, configuration];
                let ucc: *mut AnyObject = msg_send![&*cfg, userContentController];
                let _: () = msg_send![&*ucc, addContentRuleList: &*list];
            }
            true
        })
    }
}

/// 启动时先把拦截规则编好（第一次打开登录窗口时就不用等）
pub fn prepare(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    let _ = app.run_on_main_thread(blocker::prepare);
    // Linux：放到 setup 之后的主线程上（那时数据目录才拿得到）；没编成的话，打开登录窗口时 attach_blocker 会再编一次
    #[cfg(target_os = "linux")]
    {
        let a = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Ok(d) = filter_dir(&a) {
                gtk::prepare(&d, RULES);
            }
        });
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    let _ = app;
}

/// 给网站那块挡住 Captae 自己的协议（macOS、Linux 加拦截规则，Windows 摘掉协议过滤器）。
/// 规则还没编好就等一会儿；一直挡不上返回 false（这时不打开网站）。
/// Windows 上顺便读出 WebView2 实际的 User-Agent（下载时用同一个）
#[cfg(any(target_os = "macos", windows, target_os = "linux"))]
pub(crate) fn attach_blocker(wv: &tauri::Webview) -> bool {
    #[cfg(target_os = "linux")]
    let dir = filter_dir(wv.app_handle()).ok();
    for _ in 0..30 {
        let (tx, rx) = std::sync::mpsc::channel();
        #[cfg(target_os = "macos")]
        let _ = wv.with_webview(move |pv| {
            blocker::prepare();
            let _ = tx.send(blocker::attach(pv.inner() as *mut objc2::runtime::AnyObject));
        });
        #[cfg(windows)]
        let _ = wv.with_webview(move |pv| {
            let c = pv.controller();
            if let Some(ua) = win::user_agent(&c) {
                login::set_login_user_agent(ua);
            }
            let _ = tx.send(win::drop_protocol_filters(&c));
        });
        #[cfg(target_os = "linux")]
        {
            let dir = dir.clone();
            let _ = wv.with_webview(move |pv| {
                if let Some(d) = &dir {
                    gtk::prepare(d, RULES);
                }
                let _ = tx.send(gtk::attach(&pv.inner()));
            });
        }
        if rx.recv_timeout(Duration::from_secs(2)).unwrap_or(false) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    false
}

/// 让这块 webview 出不了声（看不见的读取窗口用，见 page_reader.rs；网页里的脚本静音之外再保险一层）。在主线程上调。
/// - macOS 12 起：WKWebView 的 `setAllMediaPlaybackSuspended:`，媒体播放整个停住，网页自己也恢复不了（公开接口，先问它认不认）
/// - Windows：WebView2 的 `IsMuted`；Linux：WebKitGTK 的 `is-muted`
pub(crate) fn mute_media(pv: tauri::webview::PlatformWebview) -> bool {
    #[cfg(target_os = "macos")]
    unsafe {
        use objc2::runtime::AnyObject;
        let wk = pv.inner() as *mut AnyObject;
        if wk.is_null() {
            return false;
        }
        let wk = &*wk;
        let ok: bool = objc2::msg_send![wk, respondsToSelector: objc2::sel!(setAllMediaPlaybackSuspended:completionHandler:)];
        if ok {
            let none: Option<&block2::Block<dyn Fn()>> = None;
            let _: () = objc2::msg_send![wk, setAllMediaPlaybackSuspended: true, completionHandler: none];
        }
        ok
    }
    #[cfg(windows)]
    return win::mute(&pv.controller());
    #[cfg(target_os = "linux")]
    return gtk::mute(&pv.inner());
    #[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
    {
        let _ = pv;
        false
    }
}

// ------------------------------------------------------------------ 窗口

/// 打开哪个地址：登录 → 常见网站用内置的登录页，其他网站打开链接本身；验证 → 任务本身的链接
fn target_url(key: &str, kind: Kind, page: Option<&str>) -> R<String> {
    let page = page.filter(|p| (p.starts_with("http://") || p.starts_with("https://")) && login::url_in_key(p, key));
    let site = login::site_by_id(key);
    let url = match (kind, site, page) {
        (Kind::Verify, _, Some(p)) => p.to_string(),
        (Kind::Login, Some(s), _) | (Kind::Verify, Some(s), None) => s.login_url.to_string(),
        (_, None, Some(p)) => p.to_string(),
        (_, None, None) => match key.strip_prefix("host:") {
            Some(h) if !h.is_empty() && !h.contains(['/', ' ', '@']) => format!("https://{h}/"),
            _ => return Err(tr!("不知道要打开哪个网址")),
        },
    };
    tauri::Url::parse(&url).map_err(e)?;
    Ok(url)
}

/// 网站那块能去哪：只许 http / https；iframe 里常见的 about: / data: / blob: 也放行。tauri: / asset: / file: / ipc: 这些都不许
pub(crate) fn may_navigate(u: &tauri::Url) -> bool {
    #[cfg(debug_assertions)]
    if u.host_str() == Some("captae-result.invalid") {
        // 开发调试：测试脚本把结果放在这个地址的参数里（见 debug_hooks），只记日志，不真的去
        tracing::info!(target: "login-test", "{}", u.query().unwrap_or(""));
        return false;
    }
    matches!(u.scheme(), "http" | "https" | "about" | "data" | "blob")
}

/// 网站那块开始加载新页面：工具条上换成新的主机名
fn on_site_load(app: &AppHandle, u: &tauri::Url) {
    if !matches!(u.scheme(), "http" | "https") || u.host_str() == Some("captae-login.invalid") {
        return;
    }
    let st = app.state::<Login>();
    let snap = {
        let mut cur = st.cur.lock().unwrap();
        let Some(s) = cur.as_mut() else { return };
        s.host = u.host_str().unwrap_or("").trim_start_matches("www.").to_string();
        s.secure = u.scheme() == "https";
        s.clone()
    };
    push_bar(app, &snap);
}

/// 两块 webview 按窗口大小排：Captae 那一层铺满标题栏以下，网站盖在中间。只在主线程调（要读 NSWindow）
pub fn relayout(app: &AppHandle) {
    let Some(win) = app.get_window(WINDOW) else { return };
    let (Ok(size), Ok(scale)) = (win.inner_size(), win.scale_factor()) else { return };
    let s = size.to_logical::<f64>(scale);
    // macOS 上子 webview 的坐标从窗口最上沿算起（包括标题栏那 28–32pt），要从标题栏底下开始排，
    // 不然网站那块会压住工具条（2026-11-24 实测；两块 webview 的版式见 docs/DESIGN.md 第 5.1 节）
    let tb = titlebar_height(&win).clamp(0.0, s.height);
    let body = s.height - tb;
    if let Some(b) = app.get_webview(BAR) {
        let _ = b.set_bounds(tauri::Rect { position: LogicalPosition::new(0.0, tb).into(), size: LogicalSize::new(s.width, body).into() });
    }
    if let Some(w) = app.get_webview(SITE) {
        let h = (body - TOP - BOTTOM).max(0.0);
        let _ = w.set_bounds(tauri::Rect { position: LogicalPosition::new(0.0, tb + TOP).into(), size: LogicalSize::new(s.width, h).into() });
    }
}

/// 标题栏盖住了内容区上面多高（逻辑像素）：内容视图的高度减去不被标题栏挡住的那块（`contentLayoutRect`）。
/// 内容视图本来就在标题栏底下时是 0
#[cfg(target_os = "macos")]
fn titlebar_height(win: &tauri::Window) -> f64 {
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSRect;
    let Ok(ptr) = win.ns_window() else { return 0.0 };
    if ptr.is_null() {
        return 0.0;
    }
    unsafe {
        let w = &*(ptr as *mut AnyObject);
        let cv: *mut AnyObject = objc2::msg_send![w, contentView];
        if cv.is_null() {
            return 0.0;
        }
        let content: NSRect = objc2::msg_send![&*cv, frame];
        let layout: NSRect = objc2::msg_send![w, contentLayoutRect];
        (content.size.height - layout.size.height).max(0.0)
    }
}

#[cfg(not(target_os = "macos"))]
fn titlebar_height(_: &tauri::Window) -> f64 {
    0.0
}

/// 登录窗口自己的事件（main.rs 的 on_window_event 转过来）
pub fn on_window_event(w: &tauri::Window, ev: &tauri::WindowEvent) {
    let app = w.app_handle().clone();
    match ev {
        tauri::WindowEvent::Resized(_) | tauri::WindowEvent::ScaleFactorChanged { .. } => {
            // 放到下一轮事件循环里排，免得在窗口事件里面再改窗口
            let a = app.clone();
            let _ = app.run_on_main_thread(move || relayout(&a));
        }
        // 用户点了红色的关闭按钮：当成「取消」。先不让系统马上销毁，照 destroy_window 的顺序关（藏起来、卸掉网页、再销毁）
        tauri::WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            if end(&app, End::Closed, None).is_none() {
                destroy_window(&app);
            }
        }
        // 别的原因销毁的（不是 Captae 自己关的）：也当成「取消」
        tauri::WindowEvent::Destroyed => {
            if !app.state::<Login>().closing.swap(false, Ordering::SeqCst) {
                end(&app, End::Closed, None);
            }
        }
        _ => {}
    }
}

/// 打开（或换掉内容）
fn show(app: &AppHandle, s: &Session, target: &str) -> R<()> {
    let target: tauri::Url = target.parse().map_err(e)?;
    if let Some(win) = app.get_window(WINDOW) {
        let _ = win.set_title(&title_of(s));
        if let Some(site) = app.get_webview(SITE) {
            site.navigate(target).map_err(e)?;
        }
        push_bar(app, s);
        if may_show() {
            let _ = win.unminimize();
            let _ = win.show();
            let _ = win.set_focus();
        }
        return Ok(());
    }
    #[allow(unused_mut)]
    let mut wb = tauri::window::WindowBuilder::new(app, WINDOW)
        .title(title_of(s))
        // B 站的登录页要 1000 多宽才放得下（960 宽时右边被截掉，2026-11-24 实测）
        .inner_size(1080.0, 800.0)
        .min_inner_size(560.0, 520.0)
        .center()
        .visible(false)
        .background_color(tauri::window::Color(246, 246, 246, 255));
    // 开发调试：台前调度开着时，从终端启动的开发版拿不到前台，窗口缩在侧边条里截不了图；实测时设 CAPTAE_TEST_ONTOP 让窗口浮在最上面
    #[cfg(debug_assertions)]
    if std::env::var_os("CAPTAE_TEST_ONTOP").is_some() {
        wb = wb.always_on_top(true);
    }
    let win = wb.build().map_err(e)?;
    let size = win.inner_size().map_err(e)?.to_logical::<f64>(win.scale_factor().map_err(e)?);
    win.add_child(
        WebviewBuilder::new(BAR, WebviewUrl::App("index.html#/login-bar".into())).background_color(tauri::window::Color(246, 246, 246, 255)),
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(size.width, size.height),
    )
    .map_err(e)?;
    let a = app.clone();
    #[allow(unused_mut)]
    let mut site = WebviewBuilder::new(SITE, WebviewUrl::External(START.parse().unwrap()))
        .on_navigation(may_navigate)
        .on_page_load(|wv, p| match p.event() {
            PageLoadEvent::Started => on_site_load(wv.app_handle(), p.url()),
            // 关窗时换的空白页载入完了：可以销毁窗口了（见 destroy_window）
            PageLoadEvent::Finished if p.url().scheme() == "about" => {
                if let Some(tx) = wv.app_handle().state::<Login>().unloaded.lock().unwrap().take() {
                    let _ = tx.send(());
                }
            }
            _ => {}
        })
        // 网页要开新窗口（target=_blank、window.open）：在这个窗口里打开
        .on_new_window(move |u, _| {
            if matches!(u.scheme(), "http" | "https") {
                let a2 = a.clone();
                let _ = a.run_on_main_thread(move || {
                    if let Some(w) = a2.get_webview(SITE) {
                        let _ = w.navigate(u);
                    }
                });
            }
            NewWindowResponse::Deny
        });
    // User-Agent：macOS、Linux 装成 Safari；Windows 不改，用 WebView2 自己的（和 Edge 一样的写法，见 login::login_user_agent）
    #[cfg(not(windows))]
    {
        site = site.user_agent(&login::login_user_agent());
    }
    #[cfg(target_os = "macos")]
    {
        site = site.data_store_identifier(store_id());
    }
    // Windows、Linux：单独的数据目录（藏起来的 login-store 用同一个；它要是还开着，两块共用同一份存储）
    #[cfg(any(windows, target_os = "linux"))]
    {
        site = site.data_directory(store_dir(app).map_err(e)?);
    }
    let wv = win.add_child(site, LogicalPosition::new(0.0, TOP), LogicalSize::new(size.width, (size.height - TOP - BOTTOM).max(0.0))).map_err(e)?;
    #[cfg(all(debug_assertions, target_os = "macos"))]
    if std::env::var_os("CAPTAE_TEST_ONTOP").is_some() {
        keep_painting(&wv);
        if let Some(b) = app.get_webview(BAR) {
            keep_painting(&b);
        }
    }
    // 网站那块先挡住 Captae 自己的协议，再去真正的网址
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    if !attach_blocker(&wv) {
        destroy_window(app);
        return Err(tr!("登录窗口没准备好（网页拦截规则没加上），请再试一次"));
    }
    // 按标题栏的实际高度排好再显示（要在主线程读 NSWindow；和下面的 show 按顺序在主线程上执行）
    let a = app.clone();
    let _ = app.run_on_main_thread(move || relayout(&a));
    wv.navigate(target).map_err(e)?;
    if may_show() {
        let _ = win.show();
        let _ = win.set_focus();
    }
    Ok(())
}

/// 开发调试：设了 CAPTAE_TEST_NO_SHOW 时登录窗口照常建好、照常打开网页，只是不显示、不抢焦点
/// （在别人正用着的电脑上实测，见 docs/DESIGN.md 第 11 节）。正式版一直显示
fn may_show() -> bool {
    #[cfg(debug_assertions)]
    if std::env::var_os("CAPTAE_TEST_NO_SHOW").is_some() {
        tracing::info!("测试开关 CAPTAE_TEST_NO_SHOW 开着：登录窗口不显示");
        return false;
    }
    true
}

enum End {
    /// 登录好了（看到登录 cookie）/ 用户点了「完成」
    Done(How),
    /// 取消、关窗
    Closed,
}

/// 结束这一次：记下、重试等着的任务、关窗、告诉主窗口。`gen` 不对（已经换成别的网站了）就什么也不做
fn end(app: &AppHandle, how: End, gen: Option<u64>) -> Option<usize> {
    let st = app.state::<Login>();
    let s = {
        let mut cur = st.cur.lock().unwrap();
        match (&*cur, gen) {
            (None, _) => return None,
            (Some(s), Some(g)) if s.gen != g => return None,
            _ => {}
        }
        cur.take()?
    };
    let core = app.state::<App>().core.clone();
    let retried = match how {
        End::Done(h) => {
            core.login_mark(&s.key, h, Some(captae_core::util::now_ms()));
            // 窗口开着的时候又有任务因为要登录失败了：一起重试
            let mut ids = s.jobs.clone();
            for id in core.login_waiting(&s.key) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
            core.login_retry(&ids)
        }
        End::Closed => 0,
    };
    tracing::info!("登录窗口结束：{} {:?}，重试了 {retried} 个任务", s.key, s.kind);
    // 先结束会话再关窗；关窗记成自己关的，Destroyed 来了不会再算一次「取消」
    destroy_window(app);
    match how {
        End::Done(h) => tell_main(app, "done", &s, retried, Some(h)),
        End::Closed => tell_main(app, "closed", &s, 0, None),
    }
    Some(retried)
}

/// 每秒看一次这个网站的 cookies，出现新的登录 cookie 就算登录好了
fn spawn_poller(app: AppHandle, gen: u64) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let (key, before) = {
                let st = app.state::<Login>();
                let cur = st.cur.lock().unwrap();
                match &*cur {
                    Some(s) if s.gen == gen => (s.key.clone(), s.before.clone()),
                    _ => return,
                }
            };
            let Ok(all) = all_cookies(&app).await else { continue };
            let marks = login_marks(&key, &all);
            if !marks.is_empty() && marks != before {
                end(&app, End::Done(How::Login), Some(gen));
                return;
            }
        }
    });
}

// ------------------------------------------------------------------ 命令（主窗口）

/// 打开登录窗口。从任务打开：`{jobId, kind: "login" | "verify"}`（也可以带 url）；从设置打开：`{site}` 或 `{url}`。
/// 返回 `{site, name, kind, waiting, url}`。登录成功或验证完成以后，后端自己重试等着这个网站的任务。
#[tauri::command]
pub async fn login_open(app: AppHandle, st: State<'_, App>, url: Option<String>, site: Option<String>, job_id: Option<u64>, kind: Option<String>) -> R<Value> {
    let core = st.core.clone();
    let mut kind = if kind.as_deref() == Some("verify") { Kind::Verify } else { Kind::Login };
    let job = job_id.and_then(|id| core.job(id));
    // 任务的网页：插件嗅探来的 M3U8 用来源页面
    let page = job.as_ref().map(|j| j.req.page_url.clone().filter(|p| p.starts_with("http")).unwrap_or_else(|| j.url.clone())).or(url);
    let key = match site.filter(|s| !s.is_empty()) {
        Some(s) if login::site_by_id(&s).is_some() || s.starts_with("host:") => s,
        Some(s) => return Err(tr!("不认识的网站：{site}", site = s)),
        None => page.as_deref().and_then(login::site_key).ok_or_else(|| tr!("不知道要登录哪个网站"))?,
    };
    // YouTube 的「确认不是机器人」实际要登录才能过（yt-dlp 的说明），打开登录页
    if key == "youtube" {
        kind = Kind::Login;
    }
    let target = target_url(&key, kind, page.as_deref())?;
    let mut jobs = core.login_waiting(&key);
    if let Some(id) = job_id.filter(|id| core.job(*id).is_some()) {
        if !jobs.contains(&id) {
            jobs.insert(0, id);
        }
    }
    let before = all_cookies(&app).await.map(|c| login_marks(&key, &c)).unwrap_or_default();
    let st_login = app.state::<Login>();
    // 上一个窗口刚被 Captae 关掉、还没真正关完：等它从 Tauri 里消失再开新的，不然会在快要关掉的窗口里打开网站。
    // 关窗要等网页卸掉（最多 UNLOAD_WAIT），所以最多等 3 秒
    if st_login.closing_wait.swap(false, Ordering::SeqCst) {
        for _ in 0..60 {
            if app.get_window(WINDOW).is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }
    let gen = st_login.next.fetch_add(1, Ordering::SeqCst) + 1;
    let s = Session { gen, key: key.clone(), kind, jobs, host: login::host_of(&target).unwrap_or_default(), secure: target.starts_with("https://"), before };
    // 已经开着别的网站：换内容，原来等着的任务不再等（主窗口里取消「等登录」）
    let old = st_login.cur.lock().unwrap().replace(s.clone());
    if let Some(o) = old {
        tell_main(&app, "closed", &o, 0, None);
    }
    if let Err(err) = show(&app, &s, &target) {
        st_login.cur.lock().unwrap().take();
        return Err(tr!("登录窗口打不开：{err}", err = err));
    }
    tell_main(&app, "open", &s, 0, None);
    if login::site_by_id(&key).is_some() {
        spawn_poller(app.clone(), gen);
    }
    Ok(json!({"site": key, "name": login::key_name(&key), "kind": kind, "waiting": s.jobs.len(), "url": target}))
}

/// 主窗口里点了「取消」：关掉登录窗口（带了 jobId 时，只在这个任务正等着时才关）
#[tauri::command]
pub fn login_close(app: AppHandle, job_id: Option<u64>) -> R<()> {
    let gen = {
        let st = app.state::<Login>();
        let cur = st.cur.lock().unwrap();
        match &*cur {
            Some(s) if job_id.is_none_or(|id| s.jobs.contains(&id)) => s.gen,
            _ => return Ok(()),
        }
    };
    end(&app, End::Closed, Some(gen));
    Ok(())
}

/// 设置里「已登录的网站」：`[{site, name, state: in | verified | out, gain, at?, note}]`。
/// 常见网站没登录也列出来（方便主动登录）；其他网站登录过、验证过的才列。
#[tauri::command]
pub async fn login_sites(app: AppHandle, st: State<'_, App>) -> R<Vec<Value>> {
    let core = st.core.clone();
    let all = all_cookies(&app).await.unwrap_or_else(|err| {
        tracing::warn!("读登录存储失败：{err:#}");
        vec![]
    });
    let logins = core.logins();
    let now = now_secs();
    let has = |domains: &[String]| all.iter().any(|c| !c.expired(now) && domains.iter().any(|d| c.related_to(d)));
    let mut rows = vec![];
    for s in login::SITES {
        let logged = login::logged_in(s.id, &all, now);
        let rec = logins.get(s.id).cloned();
        if !(s.featured || logged || rec.is_some()) {
            continue;
        }
        // 登录着、但 Captae 没记下来（比如在别的网站的登录流程里顺带登录了）：补一条，下载时才会带
        if logged && rec.is_none() {
            core.login_mark(s.id, How::Login, None);
        }
        let domains: Vec<String> = s.domains.iter().map(|d| d.to_string()).collect();
        let state = if logged {
            "in"
        } else if rec.as_ref().is_some_and(|r| r.how == How::Verify) && has(&domains) {
            "verified"
        } else {
            "out"
        };
        // 登录以后有什么用：核心库 SITES 里的原文（tr_key! 标过），按现在的语言显示。name 是存着的原文，界面自己翻译（tAny）
        let gain = captae_core::i18n::dynamic(s.gain);
        let note = match (state, &rec) {
            ("out", Some(r)) if r.how == How::Login => tr!("登录已经失效，重新登录一次"),
            ("verified", _) => tr!("验证过"),
            _ => gain.clone(),
        };
        rows.push(json!({"site": s.id, "name": s.name, "state": state, "gain": gain, "at": rec.and_then(|r| r.at), "note": note}));
    }
    for r in logins.sites.iter().filter(|r| r.site.starts_with("host:")) {
        let domains = login::key_domains(&r.site);
        if !has(&domains) {
            continue;
        }
        let state = if r.how == How::Login { "in" } else { "verified" };
        rows.push(json!({"site": r.site, "name": login::key_name(&r.site), "state": state, "gain": "", "at": r.at, "note": if state == "in" { tr!("登录过") } else { tr!("验证过") }}));
    }
    Ok(rows)
}

/// 退出 / 清除：删掉这个网站在登录存储里的 cookies（YouTube 连 Google 的一起），Captae 也忘掉它
#[tauri::command]
pub async fn login_logout(app: AppHandle, st: State<'_, App>, site: String) -> R<Value> {
    let mut domains = login::key_domains(&site);
    if domains.is_empty() {
        return Err(tr!("不认识的网站：{site}", site = site));
    }
    if let Some(s) = login::site_by_id(&site) {
        domains.extend(s.logout_also.iter().map(|d| d.to_string()));
    }
    let n = delete_cookies(&app, domains).await.map_err(e)?;
    st.core.login_forget(&site);
    tracing::info!("已清除 {site} 的登录状态（{n} 个 cookie）");
    Ok(json!({"site": site, "removed": n}))
}

// ------------------------------------------------------------------ 命令（登录窗口的工具条）

/// 工具条加载好以后来取一次状态（之后由 Rust 推）
#[tauri::command]
pub fn login_bar_state(app: AppHandle) -> Value {
    let st = app.state::<Login>();
    let cur = st.cur.lock().unwrap();
    cur.as_ref().map(bar_state).unwrap_or(Value::Null)
}

/// 「完成」：有登录规则的网站看一眼登录 cookie 在不在（在 = 登录了，不在 = 当成验证过），然后重试等着的任务、关窗
#[tauri::command]
pub async fn login_done(app: AppHandle) -> R<()> {
    let (key, gen) = {
        let st = app.state::<Login>();
        let cur = st.cur.lock().unwrap();
        match &*cur {
            Some(s) => (s.key.clone(), s.gen),
            None => return Ok(()),
        }
    };
    let how = match login::site_by_id(&key) {
        Some(_) if all_cookies(&app).await.is_ok_and(|c| login::logged_in(&key, &c, now_secs())) => How::Login,
        _ => How::Verify,
    };
    // 先把这次调用的结果回给工具条，再关窗
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(60)).await;
        end(&a, End::Done(how), Some(gen));
    });
    Ok(())
}

/// 「取消」：关窗，任务留在失败状态
#[tauri::command]
pub fn login_cancel(app: AppHandle) -> R<()> {
    let gen = app.state::<Login>().cur.lock().unwrap().as_ref().map(|s| s.gen);
    if let Some(g) = gen {
        let a = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(60)).await;
            end(&a, End::Closed, Some(g));
        });
    }
    Ok(())
}

/// 主窗口销毁（退出）时把登录窗口（和读登录存储用的隐藏窗口）也关掉，不然应用退不出去
pub fn close_all(app: &AppHandle) {
    app.state::<Login>().cur.lock().unwrap().take();
    destroy_window_now(app);
    if let Some(w) = app.get_window(STORE) {
        let _ = w.destroy();
    }
}

// ------------------------------------------------------------------ 开发调试（正式版不包含）

/// 测试用的钩子：`<数据目录>/login-test.json`，写进去马上执行一次。
/// - `{"eval": "js"}`：在网站那块执行 JS（结果用 `location.href = "https://captae-result.invalid/?" + 结果` 带回来，写进日志）
/// - `{"evalBar": "js"}`：在工具条那块执行 JS（点「取消」「完成」按钮测试用）
/// - `{"resize": [宽, 高]}`：改登录窗口的大小（逻辑像素），测排版跟着变
/// - `{"bounds": true}`：把登录窗口和两块 webview 的位置、大小写进日志
/// - `{"top": true | false}`：主窗口和登录窗口浮在最上面 / 恢复（台前调度开着时实测截图用）
/// - `{"paint": true}`：几块 webview 不管窗口被不被挡住都照常画（同上）
/// - `{"closeWindow": true}`：像用户点红色按钮那样关掉登录窗口
/// - `{"navigate": "网址"}`：让网站那块跳到这个网址（和 Captae 自己调 navigate 一样，测关窗前卸掉网页）
/// - `{"setCookie": {"name", "value", "domain", "path"?}}`：往登录存储里放一个 cookie（假的登录 cookie，测「判断登录成功」）
/// - `{"cookies": "域名"}`：把和这个域名有关的 cookie 的名字（不写值）写进日志
/// - `{"pageRead": "网址", "secs"?: 秒}`：看不见的读取网页打开这个网址、等满几秒（测它出不出声，见 page_reader.rs）
#[cfg(debug_assertions)]
pub fn debug_hooks(app: AppHandle, data: std::path::PathBuf) {
    tauri::async_runtime::spawn(async move {
        let f = data.join("login-test.json");
        loop {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let Ok(text) = std::fs::read_to_string(&f) else { continue };
            let _ = std::fs::remove_file(&f);
            let Ok(v) = serde_json::from_str::<Value>(&text) else { continue };
            if let Some(js) = v["eval"].as_str() {
                match app.get_webview(SITE) {
                    Some(w) => {
                        let _ = w.eval(js);
                    }
                    None => tracing::info!(target: "login-test", "eval: 登录窗口没开"),
                }
            }
            if let Some([w, h]) = v["resize"].as_array().map(|a| [a.first().and_then(Value::as_f64), a.get(1).and_then(Value::as_f64)]) {
                if let (Some(w), Some(h), Some(win)) = (w, h, app.get_window(WINDOW)) {
                    let _ = win.set_size(LogicalSize::new(w, h));
                }
            }
            if let Some(top) = v["top"].as_bool() {
                for label in ["main", WINDOW] {
                    if let Some(w) = app.get_window(label) {
                        let _ = w.set_always_on_top(top);
                        // 先藏再显示：WebKit 重新判断一次可见（配合 paint）
                        let _ = w.hide();
                        let _ = w.show();
                    }
                }
            }
            #[cfg(target_os = "macos")]
            if v.get("paint").is_some() {
                for label in ["main", BAR, SITE] {
                    if let Some(w) = app.get_webview(label) {
                        keep_painting(&w);
                    }
                }
            }
            if v.get("closeWindow").is_some() {
                if let Some(w) = app.get_window(WINDOW) {
                    let _ = w.close();
                }
            }
            if v.get("bounds").is_some() {
                let win = app.get_window(WINDOW).and_then(|w| Some((w.inner_size().ok()?, w.scale_factor().ok()?)));
                let site = app.get_webview(SITE).and_then(|w| Some((w.position().ok()?, w.size().ok()?)));
                let bar = app.get_webview(BAR).and_then(|w| Some((w.position().ok()?, w.size().ok()?)));
                tracing::info!(target: "login-test", "bounds window={win:?} site={site:?} bar={bar:?}");
            }
            if let Some(u) = v["navigate"].as_str().and_then(|u| u.parse::<tauri::Url>().ok()) {
                let r = app.get_webview(SITE).map(|w| w.navigate(u).is_ok());
                tracing::info!(target: "login-test", "navigate: {r:?}");
            }
            if let Some(js) = v["evalBar"].as_str() {
                if let Some(w) = app.get_webview(BAR) {
                    let _ = w.eval(js);
                }
            }
            #[cfg(not(any(windows, target_os = "linux")))]
            if let Some(c) = v.get("setCookie") {
                let cookie = tauri::webview::cookie::Cookie::build((c["name"].as_str().unwrap_or("x").to_string(), c["value"].as_str().unwrap_or("").to_string()))
                    .domain(c["domain"].as_str().unwrap_or("").to_string())
                    .path(c["path"].as_str().unwrap_or("/").to_string())
                    .secure(true)
                    .build();
                let r = app.get_webview(SITE).map(|w| w.set_cookie(cookie));
                tracing::info!(target: "login-test", "setCookie: {:?}", r.map(|x| x.is_ok()));
            }
            // Windows、Linux：直接放进登录存储（域名原样，`.example.com` 就是连子域名的），登录窗口没开也能放；
            // 可选 `"expires": Unix 秒`（Linux 上会话 cookie 不写进磁盘，测「关窗以后还读得到」时给一个）
            #[cfg(any(windows, target_os = "linux"))]
            if let Some(c) = v.get("setCookie") {
                let ck = Cookie {
                    name: c["name"].as_str().unwrap_or("x").to_string(),
                    value: c["value"].as_str().unwrap_or("").to_string(),
                    domain: c["domain"].as_str().unwrap_or("").to_string(),
                    path: c["path"].as_str().unwrap_or("/").to_string(),
                    secure: c["secure"].as_bool().unwrap_or(true),
                    http_only: false,
                    host_only: false,
                    expires: c["expires"].as_i64(),
                };
                #[cfg(windows)]
                let r = on_store(&app, tr!("读登录存储超时"), move |pv, done| done(win::add(&pv.controller(), &ck))).await;
                #[cfg(target_os = "linux")]
                let r = on_store(&app, tr!("读登录存储超时"), move |pv, done| gtk::add(&pv.inner(), &ck, done)).await;
                tracing::info!(target: "login-test", "setCookie: {:?}", r.map_err(|e| e.to_string()));
            }
            if let Some(u) = v["pageRead"].as_str() {
                // 看不见的读取网页打开这个网址，什么都不要、等满 secs 秒（测它出不出声、读完停没停，见 page_reader.rs）
                let secs = v["secs"].as_u64().unwrap_or(8);
                let c = captae_core::webpage::Capture { url: u.to_string(), timeout: Duration::from_secs(secs), judge: Box::new(|_, _| captae_core::webpage::Verdict::Wait) };
                let r = match captae_core::webpage::reader() {
                    Some(rd) => rd.capture(c).await.map(|s| s.count).map_err(|e| e.to_string()),
                    None => Err("no reader".into()),
                };
                tracing::info!(target: "login-test", "pageRead: {r:?}");
            }
            if let Some(d) = v["cookies"].as_str() {
                let all = all_cookies(&app).await.unwrap_or_default();
                let names: Vec<String> = all.iter().filter(|c| c.related_to(d)).map(|c| format!("{}@{}", c.name, c.domain)).collect();
                tracing::info!(target: "login-test", "cookies {d}: {names:?}");
            }
        }
    });
}

/// 开发调试：让 WebKit 不管窗口被不被挡住都照常画。台前调度开着时，后台程序的窗口被当成看不见，网页不画、截图一片白。
/// 用的是 WebKit 的内部接口 `_setWindowOcclusionDetectionEnabled:`（先问它认不认），只在调试版实测时用，正式版不包含
#[cfg(all(debug_assertions, target_os = "macos"))]
fn keep_painting(wv: &tauri::Webview) {
    let _ = wv.with_webview(keep_painting_pv);
}

/// 同上，直接拿 webview 本体：主窗口是 WebviewWindow，main.rs 用 `w.with_webview(login::keep_painting_pv)`
#[cfg(all(debug_assertions, target_os = "macos"))]
pub fn keep_painting_pv(pv: tauri::webview::PlatformWebview) {
    unsafe {
        use objc2::runtime::AnyObject;
        let wk = pv.inner() as *mut AnyObject;
        if wk.is_null() {
            return;
        }
        let wk = &*wk;
        let sel = objc2::sel!(_setWindowOcclusionDetectionEnabled:);
        let ok: bool = objc2::msg_send![wk, respondsToSelector: sel];
        if ok {
            let _: () = objc2::msg_send![wk, _setWindowOcclusionDetectionEnabled: false];
        }
        tracing::info!(target: "login-test", "keep_painting: {ok}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn who_may_call_what() {
        // 网站那块：什么都不能调
        for c in ["boot", "add_job", "login_done", "login_bar_state", "js_log", "plugin:event|listen"] {
            assert!(!allowed(SITE, c), "{c}");
        }
        // 工具条：只能调自己那几个
        assert!(allowed(BAR, "login_done") && allowed(BAR, "login_cancel") && allowed(BAR, "login_bar_state"));
        for c in ["boot", "add_job", "login_open", "login_logout", "set_settings", "library_query"] {
            assert!(!allowed(BAR, c), "{c}");
        }
        // Windows、Linux 上读登录存储用的隐藏 webview：什么都不能调
        for c in ["boot", "login_done", "js_log", "login_bar_state"] {
            assert!(!allowed(STORE, c), "{c}");
        }
        // 主窗口照旧；没登记的 webview 什么都不能调
        assert!(allowed("main", "boot") && allowed("main", "login_open"));
        assert!(!allowed("other", "boot") && !allowed("", "js_log"));
    }

    #[test]
    fn where_to_open() {
        assert_eq!(target_url("youtube", Kind::Login, Some("https://www.youtube.com/watch?v=x")).unwrap(), login::YOUTUBE_LOGIN);
        assert_eq!(target_url("bilibili", Kind::Login, None).unwrap(), "https://passport.bilibili.com/login");
        // 人机验证：打开任务本身的链接
        assert_eq!(target_url("douyin", Kind::Verify, Some("https://www.douyin.com/video/73")).unwrap(), "https://www.douyin.com/video/73");
        // 其他网站：打开链接本身；没有链接就打开首页
        assert_eq!(target_url("host:example.com", Kind::Login, Some("https://video.example.com/v/1")).unwrap(), "https://video.example.com/v/1");
        assert_eq!(target_url("host:example.com", Kind::Verify, None).unwrap(), "https://example.com/");
        // 链接不是这个网站的：不用它
        assert_eq!(target_url("host:example.com", Kind::Verify, Some("https://evil.net/")).unwrap(), "https://example.com/");
        assert!(target_url("host:", Kind::Login, None).is_err());
    }

    #[test]
    fn navigation_rules() {
        let ok = |s: &str| may_navigate(&s.parse().unwrap());
        assert!(ok("https://accounts.google.com/") && ok("http://example.com/") && ok("about:blank") && ok("blob:https://x.com/1"));
        for bad in ["tauri://localhost/index.html", "asset://localhost/Users/a/.ssh/id_rsa", "file:///etc/passwd", "ipc://localhost/boot", "x-apple.systempreferences:com.apple"] {
            assert!(!ok(bad), "{bad}");
        }
    }

    /// macOS 和 Linux 用同一份规则（WKContentRuleList / WebKitUserContentFilter 是同一种 JSON）
    #[test]
    fn blocker_rules_are_json() {
        let v: serde_json::Value = serde_json::from_str(RULES).unwrap();
        let filters: Vec<&str> = v.as_array().unwrap().iter().map(|r| r["trigger"]["url-filter"].as_str().unwrap()).collect();
        assert!(filters.contains(&"^asset:") && filters.contains(&"^https?://asset\\.localhost"));
        // WebKit 的 url-filter 不支持 `|`（编译会失败，2026-11-24 实测）
        assert!(filters.iter().all(|f| !f.contains('|')));
    }

    #[test]
    fn titles() {
        let s = |key: &str, kind| Session { gen: 1, key: key.into(), kind, jobs: vec![], host: String::new(), secure: true, before: vec![] };
        assert_eq!(title_of(&s("youtube", Kind::Login)), "登录 YouTube");
        assert_eq!(title_of(&s("douyin", Kind::Verify)), "在抖音验证");
        assert_eq!(title_of(&s("host:example.com", Kind::Verify)), "在 example.com 验证");
        // 英文：整句按英文词表，不加中文那种空格
        captae_core::i18n::with_lang(captae_core::i18n::Lang::En, || {
            assert_eq!(title_of(&s("youtube", Kind::Login)), "Sign in to YouTube");
            assert_eq!(title_of(&s("host:example.com", Kind::Verify)), "Verify on example.com");
        });
    }
}
