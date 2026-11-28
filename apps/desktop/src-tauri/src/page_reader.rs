//! 看不见的网页（核心库 `captae_core::webpage` 的桌面端实现）。
//!
//! 有的网站下载引擎读不了、网页自己却拿得到（抖音，见 `captae_core::douyin`）：在这里静音打开网页，接住网页自己请求到的视频信息。
//! - 单独一个看不见的窗口 `page-reader`（从不显示、不进任务栏、不抢焦点），里面一块 webview `page-reader-view`。
//!   和登录窗口的网站那块同一份存储（登录过、验证过的状态都在）、同样的 User-Agent、同样的拦截规则（挡住 asset: 这些 Captae 自己的协议，
//!   见 `login::attach_blocker`）。什么命令都不能调（`login::allowed` 默认拒绝）。最初也打开 `.invalid` 地址再跳（原因见 login.rs 开头）。
//! - 页面一开始挂上 `webpage::init_script()`：静音；接住要的请求的回应，跳到 `captae-page.invalid` 把回应交出来，这里在导航检查里拦下。
//! - 出不了声：脚本静音之外，再用 `login::mute_media`（macOS 把媒体播放整个停住，Windows / Linux 整块静音），每次开始载入都再来一次。
//! - 后台不限流（`background_throttling` Disabled，macOS 14 起生效）：窗口从不显示，网页默认被当成在后台。
//! - 一次只读一个网页，读完马上换成空白页。窗口建好以后留着，退出 Captae 时才关（macOS 上 wry 销毁 webview 会泄漏一块，见 login.rs 的 destroy_window）。

use crate::login;
use anyhow::{anyhow, Result};
use captae_core::webpage::{self, Capture, Seen, Verdict};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use tauri::utils::config::BackgroundThrottlingPolicy;
use tauri::webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::{AppHandle, LogicalPosition, LogicalSize, Manager, WebviewUrl};

pub const WINDOW: &str = "page-reader";
pub const VIEW: &str = "page-reader-view";
/// 网页按多大的窗口排（看不见，只影响网页自己选的版式）
const SIZE: (f64, f64) = (1280.0, 800.0);
/// 没要的回应留开头多少个字（报错原文用）
const LAST_CHARS: usize = 300;

/// 正在读的这一个网页
struct Current {
    judge: Box<dyn FnMut(&str, &str) -> Verdict + Send>,
    seen: Seen,
    /// 拿到了、或者不用再等了：叫醒 `capture`
    done: Option<tokio::sync::oneshot::Sender<()>>,
}

#[derive(Default)]
struct Inner {
    /// 一次只读一个网页
    busy: tokio::sync::Mutex<()>,
    cur: Mutex<Option<Current>>,
    /// 读完换空白页、等它载入完（`unload`）：on_page_load 看到空白页载入完就用掉它
    blank: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}

impl Inner {
    /// 页面交出来一份回应
    fn got(&self, api: &str, body: &str) {
        let mut cur = self.cur.lock().unwrap();
        let Some(c) = cur.as_mut() else { return };
        if c.done.is_none() {
            return;
        }
        c.seen.count += 1;
        match (c.judge)(api, body) {
            Verdict::Take => c.seen.taken = Some(body.to_string()),
            v => {
                c.seen.last = Some(body.chars().take(LAST_CHARS).collect());
                if v == Verdict::Wait {
                    return;
                }
                c.seen.stopped = true;
            }
        }
        if let Some(tx) = c.done.take() {
            let _ = tx.send(());
        }
    }
}

/// 交给核心库的看不见的网页
pub struct Reader {
    app: AppHandle,
    inner: Arc<Inner>,
}

impl Reader {
    pub fn new(app: AppHandle) -> Reader {
        Reader { app, inner: Arc::default() }
    }
}

impl webpage::Reader for Reader {
    fn capture(&self, c: Capture) -> Pin<Box<dyn Future<Output = Result<Seen>> + Send + 'static>> {
        let (app, inner) = (self.app.clone(), self.inner.clone());
        Box::pin(async move { capture(app, inner, c).await })
    }
}

async fn capture(app: AppHandle, inner: Arc<Inner>, c: Capture) -> Result<Seen> {
    let _g = inner.busy.lock().await;
    let url: tauri::Url = c.url.parse()?;
    let wv = webview(&app, &inner).await?;
    let (tx, rx) = tokio::sync::oneshot::channel();
    *inner.cur.lock().unwrap() = Some(Current { judge: c.judge, seen: Seen::default(), done: Some(tx) });
    let t = std::time::Instant::now();
    let mut guard = Leave { wv: wv.clone(), inner: &inner, armed: true };
    if let Err(e) = wv.navigate(url) {
        return Err(anyhow!("page reader: navigate failed: {e}"));
    }
    let _ = tokio::time::timeout(c.timeout, rx).await;
    let seen = inner.cur.lock().unwrap().take().map(|c| c.seen).unwrap_or_default();
    tracing::info!("看不见的网页：{} 用了 {} 毫秒，接住 {} 份，{}", c.url, t.elapsed().as_millis(), seen.count, if seen.taken.is_some() { "拿到了" } else if seen.stopped { "不用再等了" } else { "没等到" });
    unload(&wv, &inner).await;
    guard.armed = false;
    Ok(seen)
}

/// 读到一半被丢掉（任务取消、读取超时）时：不再收、页面停止上报、换成空白页（来不及等它载入完）
struct Leave<'a> {
    wv: tauri::Webview,
    inner: &'a Inner,
    armed: bool,
}

impl Drop for Leave<'_> {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        self.inner.cur.lock().unwrap().take();
        let _ = self.wv.eval(webpage::STOP_SCRIPT);
        if let Ok(u) = "about:blank".parse() {
            let _ = self.wv.navigate(u);
        }
    }
}

/// 读完马上换成空白页（网页别在后台接着跑），等它载入完再放下一个网页进来。
/// 不等不行：网页排着要交出来的回应也是跳转，会顶掉紧接着发的导航，下一个网页白等到超时（2026-11-28 实测：
/// 一个视频的详情网页交了三份，第一份拿到就去读下一个视频，后两份把下一个视频的导航顶掉了）。所以先让页面停止上报，再换空白页，没换成就再来一次
async fn unload(wv: &tauri::Webview, inner: &Inner) {
    let _ = wv.eval(webpage::STOP_SCRIPT);
    for n in 1..=3 {
        let (tx, rx) = tokio::sync::oneshot::channel();
        *inner.blank.lock().unwrap() = Some(tx);
        let Ok(u) = "about:blank".parse() else { return };
        if wv.navigate(u).is_err() {
            return;
        }
        if matches!(tokio::time::timeout(std::time::Duration::from_millis(1500), rx).await, Ok(Ok(()))) {
            return;
        }
        tracing::info!("看不见的网页：第 {n} 次换空白页没换成");
    }
}

/// 拿那块 webview，没有就建（只建一次，之后一直留着）
async fn webview(app: &AppHandle, inner: &Arc<Inner>) -> Result<tauri::Webview> {
    if let Some(w) = app.get_webview(VIEW) {
        return Ok(w);
    }
    let win = tauri::window::WindowBuilder::new(app, WINDOW).title("Captae").visible(false).focused(false).skip_taskbar(true).inner_size(SIZE.0, SIZE.1).build()?;
    let i2 = inner.clone();
    let i3 = inner.clone();
    #[allow(unused_mut)]
    let mut b = WebviewBuilder::new(VIEW, WebviewUrl::External(login::START.parse()?))
        .initialization_script(webpage::init_script())
        .background_throttling(BackgroundThrottlingPolicy::Disabled)
        .focused(false)
        .on_navigation(move |u| {
            // 页面交出来的回应：记下，拦住（网页不动）
            if webpage::is_report(u) {
                match webpage::parse_report(u.as_str()) {
                    Some((api, body)) => {
                        tracing::debug!("看不见的网页交出来一份（{} 字）：{}", body.len(), api.chars().take(120).collect::<String>());
                        i2.got(&api, &body)
                    }
                    None => tracing::warn!("看不见的网页交出来的内容读不出来"),
                }
                return false;
            }
            login::may_navigate(u)
        })
        .on_page_load(move |wv, p| {
            tracing::debug!("看不见的网页：{:?} {}", p.event(), p.url().as_str().chars().take(120).collect::<String>());
            match p.event() {
                PageLoadEvent::Started => {
                    let _ = wv.with_webview(|pv| {
                        login::mute_media(pv);
                    });
                }
                PageLoadEvent::Finished if p.url().scheme() == "about" => {
                    if let Some(tx) = i3.blank.lock().unwrap().take() {
                        let _ = tx.send(());
                    }
                }
                _ => {}
            }
        })
        // 网页要开新窗口：不开
        .on_new_window(|_, _| NewWindowResponse::Deny);
    // User-Agent、存储和登录窗口的网站那块一样（Windows 不改 User-Agent，用 WebView2 自己的）
    #[cfg(not(windows))]
    {
        b = b.user_agent(&captae_core::login::login_user_agent());
    }
    #[cfg(target_os = "macos")]
    {
        b = b.data_store_identifier(login::store_id());
    }
    #[cfg(any(windows, target_os = "linux"))]
    {
        b = b.data_directory(login::store_dir(app)?);
    }
    let wv = win.add_child(b, LogicalPosition::new(0.0, 0.0), LogicalSize::new(SIZE.0, SIZE.1))?;
    // 挡住 Captae 自己的协议；挡不上就不用这块（attach_blocker 会等在主线程上排队的回调，不能在异步线程里干等）
    #[cfg(any(target_os = "macos", windows, target_os = "linux"))]
    {
        let w2 = wv.clone();
        if !tokio::task::spawn_blocking(move || login::attach_blocker(&w2)).await.unwrap_or(false) {
            let _ = win.destroy();
            return Err(anyhow!("page reader: content blocker not attached"));
        }
    }
    let _ = wv.with_webview(|pv| {
        let ok = login::mute_media(pv);
        tracing::info!("建了看不见的读取网页（整块静音：{ok}）");
    });
    Ok(wv)
}

/// 退出 Captae 时关掉（不然应用退不出去）
pub fn close(app: &AppHandle) {
    if let Some(w) = app.get_window(WINDOW) {
        let _ = w.destroy();
    }
}
