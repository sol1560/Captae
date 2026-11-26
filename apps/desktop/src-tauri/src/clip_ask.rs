//! 剪贴板提示的桌面端这一半：复制了视频链接、1 分钟内把 Captae 拿到前面才问（规则和 AskGate 见 captae-core 的 clipboard.rs）。
//!
//! - 后台线程每半秒看一眼系统剪贴板的变更次数：只看次数、不读内容。macOS 是 NSPasteboard.changeCount，
//!   Windows 是 GetClipboardSequenceNumber，Linux 没有现成的次数，用 X11 的 XFixes 事件自己数（剪贴板换了主人就加一，
//!   Wayland 下经 XWayland 也收得到）；连不上 X11（纯 Wayland、没有 XWayland）时退回按内容算：只在窗口拿到 / 失去焦点时读一次，后台不读；
//! - 窗口拿到 / 失去焦点时记下 Captae 在不在前台。Windows 上不靠窗口的焦点事件：焦点平时在 WebView2 的子窗口里，
//!   用户点回 Captae 时主窗口收不到 WM_SETFOCUS，tao 不发 Focused(true)（2026-11-24 Devin 实测剪贴板提示一次都没出来）；
//!   改成后台线程每半秒看一次前台窗口（GetForegroundWindow）是不是 Captae 的主窗口、登录窗口，变了就当焦点变了；
//! - 前面这些之后：主窗口拿到焦点时问一次：读剪贴板，是视频链接就发剪贴板事件（`Core::emit_clipboard`）
//!   （下载页在输入框上方提示，别的页面弹提示条）；
//! - 刚打开 Captae 时，主窗口拿到焦点那一刻页面多半还没在听事件：页面开始听以后调 `clip_check`，由它来问这一次。
//!
//! 调试版的测试开关（正式版编译不进去）：
//! - `CAPTAE_TEST_CLIPBOARD=<文件>`：用这个文件代替系统剪贴板（内容就是剪贴板里的文字，改一次文件算复制一次），实测不碰用户的剪贴板；
//! - `CAPTAE_TEST_CLIP_SECS=<秒>`：把「1 分钟内」改短，测超时；
//! - `<数据目录>/clip-test.json` 写 `{"focus": true}` 或 `{"focus": false}`：当作主窗口拿到 / 失去焦点（测试版的窗口藏着，拿不到真的焦点）。

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use captae_core::clipboard::{is_media_link, Ask, AskGate, ASK_WITHIN};
use tauri::{AppHandle, Manager};

static GATE: Mutex<AskGate> = Mutex::new(AskGate::new());
/// 现在有焦点的 Captae 窗口（主窗口、登录窗口）：有一个就算 Captae 在前台
static FOCUSED: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// 页面已经在听事件了（调过 clip_check）
static PAGE_READY: AtomicBool = AtomicBool::new(false);

fn front() -> bool {
    !FOCUSED.lock().unwrap().is_empty()
}

fn within() -> Duration {
    #[cfg(debug_assertions)]
    if let Some(s) = std::env::var("CAPTAE_TEST_CLIP_SECS").ok().and_then(|v| v.parse().ok()) {
        return Duration::from_secs(s);
    }
    ASK_WITHIN
}

/// 启动时：读走插件记下的复制（Captae 没开时在网页里复制的），开始看剪贴板的变更次数
pub fn start(app: AppHandle, data: &Path) {
    if let Some((url, age)) = captae_core::clipboard::take_hint(data, within()) {
        if let Some(at) = Instant::now().checked_sub(age) {
            tracing::info!("剪贴板提示：插件记下 {:.0} 秒前在网页里复制过视频链接", age.as_secs_f64());
            GATE.lock().unwrap().hinted(url, at);
        }
    }
    #[cfg(target_os = "linux")]
    x11::start();
    #[cfg(windows)]
    let app2 = app.clone();
    std::thread::spawn(move || loop {
        #[cfg(windows)]
        win_front::poll(&app2);
        // 拿不到变更次数（Linux 退回按内容算）时后台不看，只在焦点变化时看
        if let Some(n) = count() {
            let f = front();
            GATE.lock().unwrap().observe(n, Instant::now(), f);
        }
        std::thread::sleep(Duration::from_millis(500));
    });
    #[cfg(debug_assertions)]
    debug_hook(app, data.to_path_buf());
    #[cfg(not(debug_assertions))]
    let _ = app;
}

/// Windows：前台窗口是 Captae 的哪个窗口（主窗口、登录窗口），变了就照「焦点变了」处理（见文件开头）
#[cfg(windows)]
mod win_front {
    use std::sync::Mutex;
    use tauri::{AppHandle, Manager};

    static LAST: Mutex<Option<String>> = Mutex::new(None);

    fn front_label(app: &AppHandle) -> Option<String> {
        // SAFETY：没有参数，只读系统现在的前台窗口句柄
        let fg = unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetForegroundWindow() } as isize;
        if fg == 0 {
            return None;
        }
        ["main", crate::login::WINDOW]
            .into_iter()
            .find(|l| app.get_window(l).and_then(|w| w.hwnd().ok()).is_some_and(|h| h.0 as isize == fg))
            .map(str::to_string)
    }

    pub fn poll(app: &AppHandle) {
        let now = front_label(app);
        let prev = {
            let mut last = LAST.lock().unwrap();
            if *last == now {
                return;
            }
            std::mem::replace(&mut *last, now.clone())
        };
        if let Some(l) = prev {
            super::focus_changed(app, &l, false);
        }
        if let Some(l) = now {
            super::focus_changed(app, &l, true);
        }
    }
}

/// 窗口焦点变了（main.rs 的 on_window_event 调，Windows 上由 `win_front` 调；调试钩子也调）
pub fn focus_changed(app: &AppHandle, label: &str, focused: bool) {
    if focused {
        // 先把后台还没看到的变化记成「Captae 在后台时复制的」，再标成在前台
        if let Some(n) = count_now() {
            GATE.lock().unwrap().observe(n, Instant::now(), false);
        }
        let mut f = FOCUSED.lock().unwrap();
        if !f.iter().any(|l| l == label) {
            f.push(label.to_string());
        }
    } else {
        // 按内容算的时候后台没人看：失去焦点这一刻看一眼，在 Captae 里复制的（这时还算在前台）记成自己复制的，之后不问
        if count().is_none() {
            if let Some(n) = count_now() {
                GATE.lock().unwrap().observe(n, Instant::now(), true);
            }
        }
        FOCUSED.lock().unwrap().retain(|l| l != label);
    }
    if focused && label == "main" && PAGE_READY.load(Ordering::SeqCst) {
        ask(app);
    }
}

/// 页面开始听事件以后调一次：刚打开 Captae、主窗口这时有焦点的那一下由这里来问
#[tauri::command]
pub fn clip_check(app: AppHandle) {
    PAGE_READY.store(true, Ordering::SeqCst);
    if FOCUSED.lock().unwrap().iter().any(|l| l == "main") {
        ask(&app);
    }
}

/// 问一次：剪贴板 1 分钟内变过（不是在 Captae 里复制的）、现在是视频链接，就发 Event::Clipboard
fn ask(app: &AppHandle) {
    let Some(st) = app.try_state::<crate::App>() else { return };
    if !st.core.settings().clipboard {
        return;
    }
    let Some(n) = count_now() else { return };
    let ask = GATE.lock().unwrap().activate(n, Instant::now(), within());
    if ask == Ask::No {
        return;
    }
    let Some(url) = read_text().as_deref().and_then(is_media_link) else {
        tracing::info!("剪贴板提示：1 分钟内复制过，但剪贴板里不是视频链接，不问");
        return;
    };
    if let Ask::Only(u) = &ask {
        if u.trim() != url {
            tracing::info!("剪贴板提示：插件记下的链接后来被别的复制换掉了，不问");
            return;
        }
    }
    // 日志里只记网站，不记整条链接
    let site = url.split("://").nth(1).and_then(|r| r.split('/').next()).unwrap_or_default();
    tracing::info!("剪贴板提示：问一句要不要下载（{site}）");
    st.core.emit_clipboard(url);
}

/// 剪贴板里的文字：只在要问的时候读一次；下载页的「粘贴」也走这里。调试版设了 CAPTAE_TEST_CLIPBOARD 时读那个文件
pub fn read_text() -> Option<String> {
    #[cfg(debug_assertions)]
    if let Some(f) = fake() {
        return std::fs::read_to_string(f).ok();
    }
    use clipboard_rs::{Clipboard, ClipboardContext};
    ClipboardContext::new().ok()?.get_text().ok()
}

/// 系统剪贴板的变更次数（复制一次加一）。拿不到时是 None（Linux 连不上 X11 时），这时后台不看，见 `count_now`
fn count() -> Option<isize> {
    #[cfg(debug_assertions)]
    if let Some(f) = fake() {
        // 文件改一次算复制一次：用修改时间（纳秒）当变更次数
        return Some(
            std::fs::metadata(f)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_nanos() as isize)
                .unwrap_or(0),
        );
    }
    system_count()
}

/// 焦点变化、要问的时候用：有变更次数就用它；没有就读一次剪贴板、按内容算（读不到也是 None，这一次不看）
fn count_now() -> Option<isize> {
    count().or_else(|| {
        use std::hash::{Hash, Hasher};
        let text = read_text()?;
        let mut h = std::collections::hash_map::DefaultHasher::new();
        text.hash(&mut h);
        Some(h.finish() as isize)
    })
}

#[cfg(target_os = "macos")]
fn system_count() -> Option<isize> {
    Some(objc2::rc::autoreleasepool(|_| objc2_app_kit::NSPasteboard::generalPasteboard().changeCount()))
}

/// Windows：剪贴板的序号，每次有人往剪贴板里放东西加一，不用打开剪贴板
#[cfg(windows)]
fn system_count() -> Option<isize> {
    // SAFETY：没有参数、不碰内存，只读系统的一个计数
    Some(unsafe { windows_sys::Win32::System::DataExchange::GetClipboardSequenceNumber() } as isize)
}

#[cfg(target_os = "linux")]
fn system_count() -> Option<isize> {
    x11::count()
}

/// 其他系统（BSD 之类，不发布）：没有变更次数，退回按内容算
#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn system_count() -> Option<isize> {
    None
}

/// Linux：X11 没有剪贴板的变更次数，用 XFixes 的「剪贴板换了主人」事件自己数。只收事件、不读内容。
/// Wayland 桌面（GNOME、KDE）上 Wayland 程序复制的东西，XWayland 那边也会换主人（给 X11 程序粘贴用），所以也收得到；
/// 读剪贴板的 clipboard-rs 本来就走 X11，连不上 X11 时两样都用不了，这时 `count()` 是 None，退回按内容算
#[cfg(target_os = "linux")]
mod x11 {
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use x11rb::connection::Connection;
    use x11rb::protocol::xfixes::{self, ConnectionExt as _};
    use x11rb::protocol::xproto::ConnectionExt as _;
    use x11rb::protocol::Event;

    static COUNT: AtomicIsize = AtomicIsize::new(0);
    /// 在收事件：COUNT 可信
    static WATCHING: AtomicBool = AtomicBool::new(false);

    pub fn count() -> Option<isize> {
        WATCHING.load(Ordering::SeqCst).then(|| COUNT.load(Ordering::SeqCst))
    }

    /// 连接和订阅在调用的线程里做完（本机套接字，很快），后台线程只收事件：
    /// 这样后台开始看的第一眼就是变更次数，不会先按内容算、再换成次数，被当成一次复制
    pub fn start() {
        match subscribe() {
            Ok(conn) => {
                WATCHING.store(true, Ordering::SeqCst);
                std::thread::spawn(move || {
                    loop {
                        match conn.wait_for_event() {
                            Ok(Event::XfixesSelectionNotify(_)) => {
                                COUNT.fetch_add(1, Ordering::SeqCst);
                            }
                            Ok(_) => {}
                            Err(err) => {
                                tracing::warn!("剪贴板提示：和 X11 的连接断了（{err}），改成拿到焦点时按内容判断");
                                break;
                            }
                        }
                    }
                    WATCHING.store(false, Ordering::SeqCst);
                });
            }
            Err(err) => tracing::info!("剪贴板提示：收不到 X11 的剪贴板事件（{err}），改成拿到焦点时按内容判断"),
        }
    }

    fn subscribe() -> Result<x11rb::rust_connection::RustConnection, Box<dyn std::error::Error>> {
        let (conn, screen) = x11rb::connect(None)?;
        let root = conn.setup().roots.get(screen).ok_or("没有屏幕")?.root;
        conn.xfixes_query_version(5, 0)?.reply()?;
        let clipboard = conn.intern_atom(false, b"CLIPBOARD")?.reply()?.atom;
        let mask = xfixes::SelectionEventMask::SET_SELECTION_OWNER
            | xfixes::SelectionEventMask::SELECTION_WINDOW_DESTROY
            | xfixes::SelectionEventMask::SELECTION_CLIENT_CLOSE;
        conn.xfixes_select_selection_input(root, clipboard, mask)?.check()?;
        Ok(conn)
    }
}

#[cfg(debug_assertions)]
fn fake() -> Option<std::path::PathBuf> {
    std::env::var_os("CAPTAE_TEST_CLIPBOARD").map(Into::into)
}

/// 测试钩子：<数据目录>/clip-test.json 写 {"focus": true|false}，当作主窗口拿到 / 失去焦点
#[cfg(debug_assertions)]
fn debug_hook(app: AppHandle, data: std::path::PathBuf) {
    let f = data.join("clip-test.json");
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(200));
        let Ok(s) = std::fs::read_to_string(&f) else { continue };
        let _ = std::fs::remove_file(&f);
        let v: serde_json::Value = serde_json::from_str(&s).unwrap_or_default();
        if let Some(focus) = v["focus"].as_bool() {
            tracing::info!("测试钩子 clip-test.json：当作主窗口{}焦点", if focus { "拿到" } else { "失去" });
            focus_changed(&app, "main", focus);
        }
    });
}
