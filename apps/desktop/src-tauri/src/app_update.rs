//! Captae 自己的更新（docs/DESIGN.md 第 15 节）。下载引擎 yt-dlp 的更新是另一回事，在核心库的 updater.rs。
//!
//! 用 Tauri 官方的更新插件（tauri-plugin-updater）：查 GitHub Release 上的 latest.json、minisign 验签、按安装方式装。
//! 这里管的是什么时候查、下好的更新包放哪、什么时候装、界面上怎么说：
//! - 打开 1 分钟后查一次，开着时每 24 小时查一次；设置里随时能手动查。
//! - 「自动更新 Captae」开着：查到新版就在后台下载（插件下完就验签），存进 `<数据目录>/app-update/`，
//!   连同 latest.json 原文一起记下（重新打开 Captae 后不联网也能装）。下次正常退出时装上；也可以点「重新打开」马上装。
//! - 装之前插件按 latest.json 里的签名再验一次（`Update::install`），签名里记的版本也要和 latest.json 说的一致（`requireSignedVersion`）。
//! - 装不了的情况（从 dmg 里直接打开、被系统转移运行、AppImage 所在的文件夹没有写权限、deb 没有 pkexec、开发时直接运行的程序、
//!   用测试数据目录跑正式地址）不下载，界面说原因，给「去下载页」。
//! - 同一个版本连着失败 3 次就不再自动试，等用户自己点。

use captae_core::tr;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

/// 更新地址是不是 GitHub 上的正式地址（tauri.conf.json 的 plugins.updater.endpoints；测试包换成本机的 http 地址）。
/// 用测试数据目录跑的程序不查正式地址，免得把正式版装到测试版头上。
/// 这里只看开头，不把完整的正式地址写进程序：三个平台的打包检查在程序里搜这串地址，确认编进去的更新地址是正式的；
/// 原来这里有一个写着完整地址的常量，测试包里也搜得到，检查就跟着说「是正式地址」（Devin 的 Windows B1 / B2）
fn official_endpoint(u: &str) -> bool {
    u.starts_with("https://github.com/")
}
const DAY_MS: i64 = 24 * 3600 * 1000;
/// 同一个版本连着失败这么多次，就不再自动下载、退出时也不再自动装
const MAX_FAILS: u32 = 3;

/// 记在 `<数据目录>/app-update/state.json`
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Saved {
    last_check: Option<i64>,
    /// 上一次出错：查、下载、安装哪一步，原因
    error: Option<Problem>,
    /// 每个版本连着失败了几次
    fails: BTreeMap<String, u32>,
    /// 上一次运行的版本：这次比它新，说明刚更新过
    last_run: Option<String>,
    /// 下载好、验过签的更新包
    ready: Option<Ready>,
}

/// 装的时候出的错
#[derive(Debug)]
enum InstallError {
    /// 用户在系统的密码框里点了「取消」：不装，不算这个版本的失败（现在只有 deb 认得出来，见 deb::install）
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Cancelled,
    Failed(String),
}

impl From<String> for InstallError {
    fn from(e: String) -> Self {
        InstallError::Failed(e)
    }
}

impl InstallError {
    fn text(&self) -> String {
        match self {
            InstallError::Cancelled => tr!("取消了输入密码"),
            InstallError::Failed(e) => e.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Ready {
    version: String,
    /// `app-update/` 下的文件名
    file: String,
    /// latest.json 原文：`Updater::restore_update` 用它重新得到 `Update`，不用联网
    raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// check / download / install
    pub step: String,
    pub message: String,
    pub at: i64,
}

/// 给界面的状态（`app_update_status`，变了就发 `{type: "app_update", status}` 事件）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub current: String,
    /// idle / checking / downloading / ready / available / installing
    pub state: &'static str,
    /// 查到的新版本
    pub latest: Option<String>,
    /// 下载进度 0–1（不知道总大小时为空）
    pub progress: Option<f64>,
    pub last_check: Option<i64>,
    pub error: Option<Problem>,
    /// 装不了的原因：dmg / translocated / readonly / no_pkexec / unsupported / test；为空就是能装
    pub blocked: Option<&'static str>,
    /// 安装方式：app / nsis / appimage / deb / unknown
    pub kind: &'static str,
    /// 刚从哪个版本更新过来（更新后第一次打开），界面据此提示一次
    pub updated_from: Option<String>,
    /// 下好以后，正常退出时会不会自动装（deb、要输管理员密码的不会）
    pub install_on_quit: bool,
    /// 「自动更新 Captae」开没开
    pub auto: bool,
}

pub struct AppUpdate {
    dir: PathBuf,
    data: PathBuf,
    busy: AtomicBool,
    inner: Mutex<Inner>,
}

struct Inner {
    saved: Saved,
    state: &'static str,
    latest: Option<String>,
    progress: Option<f64>,
    blocked: Option<&'static str>,
    kind: &'static str,
    updated_from: Option<String>,
    needs_auth: bool,
}

fn now_ms() -> i64 {
    captae_core::util::now_ms()
}

fn current_version(app: &AppHandle) -> String {
    app.package_info().version.to_string()
}

/// a 比 b 新（x.y.z，认不出的按 0）
fn newer(a: &str, b: &str) -> bool {
    captae_core::updater::newer(a, b)
}

// ------------------------------------------------------------------ 安装方式、装不装得了

/// 这个程序是怎么装的（Tauri 打包时写进程序里的标记）
fn kind() -> &'static str {
    use tauri::utils::platform::bundle_type;
    use tauri::utils::config::BundleType;
    match bundle_type() {
        Some(BundleType::App) | Some(BundleType::Dmg) if cfg!(target_os = "macos") => "app",
        Some(BundleType::Nsis) => "nsis",
        Some(BundleType::AppImage) => "appimage",
        Some(BundleType::Deb) => "deb",
        _ => "unknown",
    }
}

/// 更新包存成什么文件名
fn file_name(kind: &str, version: &str) -> String {
    let ext = match kind {
        "app" => "app.tar.gz",
        "nsis" => "exe",
        "appimage" => "AppImage",
        "deb" => "deb",
        _ => "bin",
    };
    format!("Captae_{version}.{ext}")
}

/// 在这个文件夹里能不能直接写（建一个临时文件再删掉）。Err 里是只读的文件系统（dmg）还是没权限
fn writable(dir: &Path) -> Result<(), std::io::ErrorKind> {
    match tempfile::Builder::new().prefix(".captae-update-probe").tempfile_in(dir) {
        Ok(_) => Ok(()),
        Err(e) => Err(e.kind()),
    }
}

/// macOS：.app 本身是不是这个账号的、能不能写。换掉 .app 时要把这个目录挪到别的父目录下（要改它的 ..），只有父目录能写不够。
/// 另一个账号装的（拖进去的是另一个管理员）时插件会改走管理员密码框；退出时主线程已经在收尾，弹不了这个框，所以算「要密码」
#[cfg(target_os = "macos")]
fn bundle_is_mine(bundle: &Path, parent: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    // 在父目录里建的临时文件属于这个进程的有效用户，拿它的 uid 来比（不用另引 libc）
    let Ok(probe) = tempfile::Builder::new().prefix(".captae-update-probe").tempfile_in(parent) else { return false };
    let (Ok(me), Ok(b)) = (probe.as_file().metadata(), std::fs::symlink_metadata(bundle)) else { return false };
    b.uid() == me.uid() && b.mode() & 0o200 != 0
}

/// macOS：程序所在的 .app（`…/Captae.app/Contents/MacOS/captae-desktop` 往上两级）
#[cfg(target_os = "macos")]
fn app_bundle(exe: &Path) -> Option<PathBuf> {
    let b = exe.parent()?.parent()?.parent()?;
    (b.extension().is_some_and(|e| e == "app") && exe.parent()?.ends_with("Contents/MacOS")).then(|| b.to_path_buf())
}

/// 装不了的原因，和装的时候要不要输管理员密码
fn blocked_reason(app: &AppHandle, kind: &'static str) -> (Option<&'static str>, bool) {
    // 测试数据目录 + 正式地址：不查（测试版的标识不同，装上去就把测试版换成了正式版）。测试用的程序在编译配置里换成本机的地址
    if std::env::var_os("CAPTAE_HOME").is_some() && endpoints(app).iter().any(|u| official_endpoint(u)) {
        return (Some("test"), false);
    }
    let Ok(exe) = std::env::current_exe() else { return (Some("unsupported"), false) };
    blocked_for(kind, &exe)
}

fn blocked_for(kind: &'static str, exe: &Path) -> (Option<&'static str>, bool) {
    match kind {
        #[cfg(target_os = "macos")]
        "app" => {
            // 系统把从下载文件夹直接打开的 app 挪到一个随机的只读位置运行（App Translocation），换不了原来那份
            if exe.to_string_lossy().contains("/AppTranslocation/") {
                return (Some("translocated"), false);
            }
            let Some(bundle) = app_bundle(exe) else { return (Some("unsupported"), false) };
            let Some(parent) = bundle.parent() else { return (Some("unsupported"), false) };
            match writable(parent) {
                Ok(()) => (None, !bundle_is_mine(&bundle, parent)),
                // 在 dmg 里直接打开的：磁盘映像是只读的
                Err(std::io::ErrorKind::ReadOnlyFilesystem) => (Some("dmg"), false),
                // 应用程序文件夹没有写权限（不是管理员的账号）：装的时候系统弹管理员密码框
                Err(_) => (None, true),
            }
        }
        "appimage" => {
            // AppImage 的运行时把 .AppImage 文件本身的位置放在 APPIMAGE 里
            let file = std::env::var_os("APPIMAGE").map(PathBuf::from).unwrap_or_else(|| exe.to_path_buf());
            match file.parent().map(writable) {
                Some(Ok(())) => (None, false),
                _ => (Some("readonly"), false),
            }
        }
        "deb" => {
            // 插件用 pkexec 装（系统弹密码框），没有 pkexec 时退回 zenity / kdialog 问密码再交给 sudo
            if ["pkexec", "zenity", "kdialog"].iter().any(|p| captae_core::tools::which(p).is_some()) {
                (None, true)
            } else {
                (Some("no_pkexec"), false)
            }
        }
        "nsis" => (None, false),
        _ => (Some("unsupported"), false),
    }
}

/// tauri.conf.json 里写的更新地址
fn endpoints(app: &AppHandle) -> Vec<String> {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("endpoints"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect())
        .unwrap_or_default()
}

// ------------------------------------------------------------------ 状态

impl AppUpdate {
    fn save(&self, s: &Saved) {
        let _ = std::fs::create_dir_all(&self.dir);
        let tmp = self.dir.join("state.json.tmp");
        if std::fs::write(&tmp, serde_json::to_vec_pretty(s).unwrap_or_default()).is_ok() {
            let _ = std::fs::rename(&tmp, self.dir.join("state.json"));
        }
    }

    /// 这次是刚更新完的第一次打开（上次运行的版本比这次旧）
    pub fn just_updated(&self) -> bool {
        self.inner.lock().unwrap().updated_from.is_some()
    }

    fn auto(&self) -> bool {
        captae_core::Settings::load(&self.data.join("settings.json")).app_auto_update
    }

    pub fn status(&self, app: &AppHandle) -> Status {
        let auto = self.auto();
        let i = self.inner.lock().unwrap();
        Status {
            current: current_version(app),
            state: i.state,
            latest: i.latest.clone(),
            progress: i.progress,
            last_check: i.saved.last_check,
            error: i.saved.error.clone(),
            blocked: i.blocked,
            kind: i.kind,
            updated_from: i.updated_from.clone(),
            install_on_quit: i.kind != "deb" && !i.needs_auth,
            auto,
        }
    }

    fn emit(&self, app: &AppHandle) {
        let _ = app.emit("captae://event", json!({"type": "app_update", "status": self.status(app)}));
    }

    fn set_state(&self, app: &AppHandle, state: &'static str) {
        self.inner.lock().unwrap().state = state;
        self.emit(app);
    }

    /// 记下出错，这一步之前的状态（ready / available / idle）由调用的地方定
    fn fail(&self, app: &AppHandle, step: &str, version: Option<&str>, err: &str, state: &'static str) {
        tracing::warn!("Captae 更新：{step} 失败：{err}");
        let mut i = self.inner.lock().unwrap();
        i.saved.error = Some(Problem { step: step.into(), message: err.into(), at: now_ms() });
        if let Some(v) = version {
            *i.saved.fails.entry(v.to_string()).or_default() += 1;
        }
        i.state = state;
        i.progress = None;
        let s = i.saved.clone();
        drop(i);
        self.save(&s);
        self.emit(app);
    }

    fn fails(&self, version: &str) -> u32 {
        self.inner.lock().unwrap().saved.fails.get(version).copied().unwrap_or(0)
    }

    fn ready_path(&self) -> Option<(Ready, PathBuf)> {
        let r = self.inner.lock().unwrap().saved.ready.clone()?;
        let p = self.dir.join(&r.file);
        p.is_file().then_some((r, p))
    }

    /// 删掉下好的更新包（装好了、过时了、验签不对）
    fn drop_ready(&self) {
        let mut i = self.inner.lock().unwrap();
        if let Some(r) = i.saved.ready.take() {
            let _ = std::fs::remove_file(self.dir.join(&r.file));
        }
        let s = i.saved.clone();
        drop(i);
        self.save(&s);
    }
}

/// 启动时调：读记录、算装不装得了、记下这次运行的版本，开始定时检查
pub fn init(app: &AppHandle, data: &Path) {
    let dir = data.join("app-update");
    let mut saved: Saved = std::fs::read(dir.join("state.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let cur = current_version(app);
    // 上次运行的版本比这次旧：刚更新过（界面提示一次「已更新到 x.y.z」）；失败记录、出错记录都清掉
    let updated_from = saved.last_run.clone().filter(|v| newer(&cur, v));
    if updated_from.is_some() {
        saved.fails.clear();
        saved.error = None;
        tracing::info!("Captae 已从 {} 更新到 {cur}", updated_from.as_deref().unwrap_or(""));
    }
    saved.last_run = Some(cur.clone());
    // 点「重新打开」装更新之前停下来的任务（核心库 hold_for_update 在 queue.json 里做了记号）：真的更新过了才接着下，
    // 没装成、或者版本没变就当普通的重新打开，记号去掉、照旧暂停着
    if let Some(a) = app.try_state::<crate::App>() {
        let resumed = a.core.release_held(updated_from.is_some());
        if !resumed.is_empty() {
            tracing::info!("Captae 更新：更新前在下载的 {} 个任务接着下", resumed.len());
        }
    }
    // 下好的包已经不比现在的新（装好了，或者用户自己装了新版）：删掉
    if saved.ready.as_ref().is_some_and(|r| !newer(&r.version, &cur) || !dir.join(&r.file).is_file()) {
        if let Some(r) = saved.ready.take() {
            let _ = std::fs::remove_file(dir.join(&r.file));
        }
    }
    // 留下的别的文件（下到一半、旧版本的包）都删掉
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            let keep = name == "state.json" || saved.ready.as_ref().is_some_and(|r| r.file == name);
            if !keep {
                let _ = std::fs::remove_file(e.path());
            }
        }
    }
    saved.fails.retain(|v, _| newer(v, &cur));
    let kind = kind();
    let (blocked, needs_auth) = blocked_reason(app, kind);
    let state = if saved.ready.is_some() { "ready" } else { "idle" };
    let latest = saved.ready.as_ref().map(|r| r.version.clone());
    let me = AppUpdate {
        dir,
        data: data.to_path_buf(),
        busy: AtomicBool::new(false),
        inner: Mutex::new(Inner { saved: saved.clone(), state, latest, progress: None, blocked, kind, updated_from, needs_auth }),
    };
    me.save(&saved);
    tracing::info!("Captae 更新：安装方式 {kind}，{}", blocked.map(|b| format!("装不了（{b}）")).unwrap_or_else(|| "能自动更新".into()));
    app.manage(me);
    let h = app.clone();
    tauri::async_runtime::spawn(async move { daily(h).await });
}

/// 打开 1 分钟后查一次，之后每 10 分钟看一眼离上次检查满没满 24 小时（电脑睡眠醒来也能赶上）
async fn daily(app: AppHandle) {
    let mut first = Duration::from_secs(60);
    // 调试版：实测时把第一次检查提前（秒）
    #[cfg(debug_assertions)]
    if let Some(s) = std::env::var("CAPTAE_TEST_UPDATE_DELAY").ok().and_then(|v| v.parse().ok()) {
        first = Duration::from_secs(s);
    }
    tokio::time::sleep(first).await;
    let mut initial = true;
    loop {
        let Some(u) = app.try_state::<AppUpdate>() else { return };
        let due = initial || u.inner.lock().unwrap().saved.last_check.map(|t| now_ms() - t >= DAY_MS).unwrap_or(true);
        let test = u.inner.lock().unwrap().blocked == Some("test");
        if due && !test {
            check(&app).await;
        }
        initial = false;
        tokio::time::sleep(Duration::from_secs(600)).await;
    }
}

/// 建一个更新器：走 Captae 的代理设置
fn updater(app: &AppHandle, data: &Path) -> Result<tauri_plugin_updater::Updater, String> {
    let s = captae_core::Settings::load(&data.join("settings.json"));
    let eff = captae_core::net::effective(&s);
    let mut b = app.updater_builder().timeout(Duration::from_secs(30));
    if !eff.unknown {
        // 代理自己配，带上「不走代理的主机」（插件的 .proxy() 用 Proxy::all，不认例外）：插件先 no_proxy() 清空，再调 configure_client 加上这里的。
        // 系统代理读不出来（eff.unknown）时什么都不设，交给插件自己读系统设置
        b = b.no_proxy();
        if eff.active() {
            b = b.configure_client(move |c| with_proxy(c, &eff));
        }
    }
    #[cfg(windows)]
    {
        // Windows：插件运行安装程序之前先结束进程，和正常退出一样收拾（删 control.json、结束 yt-dlp 这些）
        let h = app.clone();
        b = b.on_before_exit(move || crate::cleanup_on_exit(&h));
    }
    b.build().map_err(|e| e.to_string())
}

/// 按 Captae 的代理设置给插件的 HTTP 客户端加代理（和 net::build_with 一样，插件用的是 reqwest 0.13）
fn with_proxy(mut c: reqwest13::ClientBuilder, eff: &captae_core::net::Effective) -> reqwest13::ClientBuilder {
    let np = || reqwest13::NoProxy::from_string(&eff.no_proxy_list());
    if let Some(Ok(p)) = eff.http.as_deref().map(reqwest13::Proxy::http) {
        c = c.proxy(p.no_proxy(np()));
    }
    if let Some(Ok(p)) = eff.https.as_deref().map(reqwest13::Proxy::https) {
        c = c.proxy(p.no_proxy(np()));
    }
    c
}

/// 查一次（定时检查和「检查更新」都走这里）：有新版时按「自动更新 Captae」的开关决定下不下，关着只说有新版
pub async fn check(app: &AppHandle) -> Status {
    let Some(u) = app.try_state::<AppUpdate>() else { return unavailable(app) };
    if u.inner.lock().unwrap().blocked == Some("test") {
        return u.status(app);
    }
    if u.busy.swap(true, Ordering::SeqCst) {
        return u.status(app);
    }
    let before = u.inner.lock().unwrap().state;
    // 正在装就别动
    if before == "installing" {
        u.busy.store(false, Ordering::SeqCst);
        return u.status(app);
    }
    u.set_state(app, "checking");
    let res = match updater(app, &u.data) {
        Ok(up) => up.check().await.map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    {
        let mut i = u.inner.lock().unwrap();
        i.saved.last_check = Some(now_ms());
        let s = i.saved.clone();
        drop(i);
        u.save(&s);
    }
    let next = match res {
        Err(e) => {
            // 查不到不算这个版本的失败；已经下好的照样等着装
            let back = if u.ready_path().is_some() { "ready" } else if before == "available" { "available" } else { "idle" };
            u.fail(app, "check", None, &e, back);
            u.busy.store(false, Ordering::SeqCst);
            return u.status(app);
        }
        Ok(None) => {
            tracing::info!("Captae 更新：已经是最新的");
            let mut i = u.inner.lock().unwrap();
            i.latest = None;
            // 查成了：上一次检查的错误不再显示（下载、安装的错误留着）
            if i.saved.error.as_ref().is_some_and(|p| p.step == "check") {
                i.saved.error = None;
            }
            let s = i.saved.clone();
            drop(i);
            u.save(&s);
            u.drop_ready();
            "idle"
        }
        Ok(Some(up)) => {
            let v = up.version.clone();
            tracing::info!("Captae 更新：有新版 {v}");
            {
                let mut i = u.inner.lock().unwrap();
                i.latest = Some(v.clone());
                if i.saved.error.as_ref().is_some_and(|p| p.step == "check") {
                    i.saved.error = None;
                }
                let s = i.saved.clone();
                drop(i);
                u.save(&s);
            }
            // 已经下好了同一个版本：验一下还完整就不用再下
            if let Some((r, p)) = u.ready_path() {
                if r.version == v && std::fs::read(&p).ok().is_some_and(|b| up.verify(&b).is_ok()) {
                    u.busy.store(false, Ordering::SeqCst);
                    u.set_state(app, "ready");
                    return u.status(app);
                }
                u.drop_ready();
            }
            let blocked = u.inner.lock().unwrap().blocked.is_some();
            let auto = u.auto() && u.fails(&v) < MAX_FAILS;
            if !blocked && auto {
                u.busy.store(false, Ordering::SeqCst);
                return download_update(app, up).await;
            }
            "available"
        }
    };
    u.busy.store(false, Ordering::SeqCst);
    u.set_state(app, next);
    u.status(app)
}

fn unavailable(app: &AppHandle) -> Status {
    Status {
        current: current_version(app),
        state: "idle",
        latest: None,
        progress: None,
        last_check: None,
        error: None,
        blocked: Some("unsupported"),
        kind: "unknown",
        updated_from: None,
        install_on_quit: false,
        auto: false,
    }
}

/// 用户点「下载并更新」：查一次、下载（不看自动更新的开关和失败次数）
pub async fn download(app: &AppHandle) -> Result<Status, String> {
    let Some(u) = app.try_state::<AppUpdate>() else { return Err(tr!("这个版本的 Captae 不能自动更新")) };
    if let Some(b) = u.inner.lock().unwrap().blocked {
        return Err(blocked_text(b));
    }
    if u.busy.swap(true, Ordering::SeqCst) {
        return Ok(u.status(app));
    }
    u.set_state(app, "checking");
    let res = match updater(app, &u.data) {
        Ok(up) => up.check().await.map_err(|e| e.to_string()),
        Err(e) => Err(e),
    };
    u.busy.store(false, Ordering::SeqCst);
    match res {
        Ok(Some(up)) => {
            if let Some((r, p)) = u.ready_path() {
                if r.version == up.version && std::fs::read(&p).ok().is_some_and(|b| up.verify(&b).is_ok()) {
                    u.set_state(app, "ready");
                    return Ok(u.status(app));
                }
                u.drop_ready();
            }
            Ok(download_update(app, up).await)
        }
        Ok(None) => {
            u.inner.lock().unwrap().latest = None;
            u.set_state(app, "idle");
            Ok(u.status(app))
        }
        Err(e) => {
            u.fail(app, "check", None, &e, "available");
            Err(e)
        }
    }
}

/// 下载、验签（插件下完就验）、存进 app-update/
async fn download_update(app: &AppHandle, up: Update) -> Status {
    let Some(u) = app.try_state::<AppUpdate>() else { return unavailable(app) };
    if u.busy.swap(true, Ordering::SeqCst) {
        return u.status(app);
    }
    let v = up.version.clone();
    {
        let mut i = u.inner.lock().unwrap();
        i.state = "downloading";
        i.progress = Some(0.0);
        i.latest = Some(v.clone());
    }
    u.emit(app);
    let mut got: u64 = 0;
    let mut last_emit = std::time::Instant::now();
    let h = app.clone();
    let res = up
        .download(
            |n, total| {
                got += n as u64;
                let Some(u) = h.try_state::<AppUpdate>() else { return };
                let p = total.filter(|t| *t > 0).map(|t| (got as f64 / t as f64).min(1.0));
                u.inner.lock().unwrap().progress = p;
                // 进度最多每 250 毫秒发一次
                if last_emit.elapsed() >= Duration::from_millis(250) {
                    last_emit = std::time::Instant::now();
                    u.emit(&h);
                }
            },
            || {},
        )
        .await;
    match res {
        Err(e) => u.fail(app, "download", Some(&v), &e.to_string(), "available"),
        Ok(bytes) => {
            let kind = u.inner.lock().unwrap().kind;
            let name = file_name(kind, &v);
            let _ = std::fs::create_dir_all(&u.dir);
            let tmp = u.dir.join(format!("{name}.part"));
            match std::fs::write(&tmp, &bytes).and_then(|_| std::fs::rename(&tmp, u.dir.join(&name))) {
                Err(e) => {
                    let _ = std::fs::remove_file(&tmp);
                    u.fail(app, "download", Some(&v), &e.to_string(), "available");
                }
                Ok(()) => {
                    tracing::info!("Captae 更新：{v} 已下载好（{} MB），验过签", bytes.len() / 1_000_000);
                    let mut i = u.inner.lock().unwrap();
                    i.saved.ready = Some(Ready { version: v.clone(), file: name, raw: up.raw_json.clone() });
                    // 下载成了：之前查、下载出的错不再显示（装的时候出的错留着，下次退出还会再试）
                    if i.saved.error.as_ref().is_some_and(|p| p.step != "install") {
                        i.saved.error = None;
                    }
                    i.state = "ready";
                    i.progress = None;
                    let s = i.saved.clone();
                    drop(i);
                    u.save(&s);
                    u.emit(app);
                }
            }
        }
    }
    u.busy.store(false, Ordering::SeqCst);
    u.status(app)
}

/// 装下好的更新包。`restart`：装好以后重新打开（点了「重新打开」）；false：正常退出时装，装完不打开。
/// Windows 上插件运行安装程序后直接结束进程（之前由 on_before_exit 收拾），这个函数不会返回
fn install(app: &AppHandle, restart: bool) -> Result<String, InstallError> {
    let u = app.try_state::<AppUpdate>().ok_or_else(|| tr!("这个版本的 Captae 不能自动更新"))?;
    let (r, path) = u.ready_path().ok_or_else(|| tr!("还没有下载好的新版本"))?;
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let up = updater(app, &u.data)?.restore_update(r.raw.clone()).map_err(|e| e.to_string())?.ok_or_else(|| tr!("下载好的版本已经不比现在的新"))?;
    #[cfg(windows)]
    let up = up.restart_after_install(restart);
    #[cfg(not(windows))]
    let _ = restart;
    // 验签不对：包坏了或者被换过，删掉，下次重新下
    if let Err(e) = up.verify(&bytes) {
        u.drop_ready();
        return Err(e.to_string().into());
    }
    tracing::info!("Captae 更新：开始装 {}", r.version);
    // deb 自己装（用户在密码框里点了取消就不装，见 deb::install）；其余交给插件（插件装之前会再验一次签）
    #[cfg(target_os = "linux")]
    let own = u.inner.lock().unwrap().kind == "deb";
    #[cfg(not(target_os = "linux"))]
    let own = false;
    if own {
        #[cfg(target_os = "linux")]
        deb::install(&bytes)?;
    } else {
        up.install(&bytes).map_err(|e| e.to_string())?;
    }
    // 装好了：包删掉，状态里不再说「已下载好」（这次运行的还是旧版本，重新打开才是新的）
    u.drop_ready();
    Ok(r.version)
}

/// 点「重新打开」：装上下好的新版，再重新打开 Captae
pub async fn restart(app: &AppHandle) -> Result<(), String> {
    let u = app.try_state::<AppUpdate>().ok_or_else(|| tr!("这个版本的 Captae 不能自动更新"))?;
    if u.busy.swap(true, Ordering::SeqCst) {
        return Err(tr!("正在检查或下载更新，稍等一下"));
    }
    let version = u.ready_path().map(|(r, _)| r.version);
    // 装之前先把正在下载、排队的任务停下来记成「暂停」、在 queue.json 里做上记号，新版打开后接着下（核心库的 hold_for_update，init 里接着下）。
    // Windows 上插件运行安装程序后直接结束进程，没有「装好以后」；不先停下来，插件收拾时结束 aria2c 会被记成下载失败（Devin 的 Windows U4）
    if let Some(a) = app.try_state::<crate::App>() {
        let n = a.core.hold_for_update();
        if n > 0 {
            tracing::info!("Captae 更新：{n} 个任务先停下，新版打开后接着下");
        }
    }
    u.set_state(app, "installing");
    let h = app.clone();
    // macOS 要输管理员密码时，插件把系统的密码框交给主线程弹，这里不能占着主线程
    let res = tauri::async_runtime::spawn_blocking(move || install(&h, true)).await.map_err(|e| InstallError::Failed(e.to_string())).and_then(|r| r);
    match res {
        Ok(v) => {
            tracing::info!("Captae 更新：{v} 装好了，重新打开");
            // 测试数据目录：自己重新打开，带着 CAPTAE_HOME 这些环境变量（见 relaunch_after_exit）
            #[cfg(unix)]
            if std::env::var_os("CAPTAE_HOME").is_some() {
                // Linux 的 AppImage：要重新打开的是 .AppImage 文件本身（current_exe 在每次启动都换地方的临时挂载点里）
                let exe = match std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file()) {
                    Some(p) => p,
                    None => std::env::current_exe().map_err(|e| e.to_string())?,
                };
                let mut cmd = vec![exe.into_os_string()];
                cmd.extend(std::env::args_os().skip(1));
                tracing::info!("测试数据目录：自己重新打开（带着环境变量）");
                return relaunch_after_exit(app, &u.data, cmd);
            }
            // macOS：经 LaunchServices 打开、到前台（见 relaunch_after_exit）
            #[cfg(target_os = "macos")]
            if let Some(bundle) = std::env::current_exe().ok().as_deref().and_then(app_bundle) {
                let mut cmd = vec!["/usr/bin/open".into(), "-n".into(), bundle.into_os_string()];
                let args: Vec<_> = std::env::args_os().skip(1).collect();
                if !args.is_empty() {
                    cmd.push("--args".into());
                    cmd.extend(args);
                }
                return relaunch_after_exit(app, &u.data, cmd);
            }
            app.restart();
        }
        Err(e) => {
            // 没装成：停下来的任务在这个进程里接着下
            if let Some(a) = app.try_state::<crate::App>() {
                a.core.release_held(true);
            }
            let back = if u.ready_path().is_some() { "ready" } else { "available" };
            // 用户在密码框里点了取消：不算这个版本的失败（不影响自动下载），设置里照样说「上次没装成：取消了输入密码」
            let counted = if matches!(e, InstallError::Cancelled) { None } else { version.as_deref() };
            let msg = e.text();
            u.fail(app, "install", counted, &msg, back);
            u.busy.store(false, Ordering::SeqCst);
            Err(msg)
        }
    }
}

/// 装好以后自己重新打开，不用 Tauri 的 `restart`：交给一个 sh 等这个进程号没了再 exec `cmd`（单实例插件的锁还在时，新进程会把自己交给这个进程然后退出），
/// 报错输出写进 `<数据目录>/relaunch.log`。两种情况：
/// - 设了 CAPTAE_HOME（测试数据目录）：直接运行程序，环境变量、参数原样。Tauri 的 restart 在 macOS 上经 LaunchServices 重新打开，
///   新进程拿不到 CAPTAE_HOME，测试版会去读写正式的数据目录、改浏览器里插件宿主的登记（docs/DESIGN.md 第 11 节）。
/// - macOS 正式版：`open -n Captae.app`。Tauri 的 restart 用 `open -n -g`（-g：不到前台），新窗口出来却在访达后面；
///   新进程自己去抢前台也不行（macOS 14 起 `activateIgnoringOtherApps` 不再能从别的程序手里抢，Devin 的 macOS M2）。
///   不带 -g 时由 LaunchServices 打开并放到前台，和在访达里双击一样
#[cfg(unix)]
fn relaunch_after_exit(app: &AppHandle, data: &Path, cmd: Vec<std::ffi::OsString>) -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};
    let log = std::fs::File::create(data.join("relaunch.log")).map(Stdio::from).unwrap_or_else(|_| Stdio::null());
    Command::new("/bin/sh")
        .arg("-c")
        .arg(r#"while kill -0 "$1" 2>/dev/null; do sleep 0.2; done; shift; exec "$@""#)
        .arg("sh")
        .arg(std::process::id().to_string())
        .args(&cmd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(log)
        .process_group(0)
        .spawn()
        .map_err(|e| e.to_string())?;
    tracing::info!("Captae 更新：退出以后重新打开：{}", cmd.iter().map(|s| s.to_string_lossy()).collect::<Vec<_>>().join(" "));
    app.exit(0);
    Ok(())
}

/// Linux 的 deb：自己用 pkexec 装，不交给更新插件。插件在 pkexec 失败（包括用户在系统的认证窗口里点了「取消」）以后，
/// 还会再弹 zenity / kdialog 的密码框，那个也取消了就直接 `sudo dpkg -i`：sudo 不要密码的电脑上，用户取消了照样装上（Devin 的 Linux D4）。
/// 这里用户取消就不装，Captae 不关，设置里说「上次没装成：取消了输入密码」。
/// - pkexec：系统的认证窗口（polkit）。退出码 126 是用户关掉了认证窗口；127 是没能认证（没有在运行的认证代理，常见于精简桌面、远程桌面；
///   或者密码输错的次数用完了），这时退回 zenity / kdialog 问密码、交给 `sudo -S`。
/// - zenity / kdialog 点了取消（退出码 1）也是不装；两个都没有、都弹不出来就算失败。不退回不问密码的 sudo
#[cfg(target_os = "linux")]
mod deb {
    use super::InstallError;
    use captae_core::tr;
    use std::process::{Command, Output, Stdio};

    pub fn install(bytes: &[u8]) -> Result<(), InstallError> {
        let dir = tempfile::Builder::new().prefix("captae-update").tempdir().map_err(|e| e.to_string())?;
        let pkg = dir.path().join("Captae.deb");
        std::fs::write(&pkg, bytes).map_err(|e| e.to_string())?;
        let mut why = None;
        if captae_core::tools::which("pkexec").is_some() {
            match Command::new("pkexec").arg("dpkg").arg("-i").arg(&pkg).stdin(Stdio::null()).output() {
                Ok(o) if o.status.success() => return Ok(()),
                Ok(o) if o.status.code() == Some(126) => return Err(InstallError::Cancelled),
                Ok(o) if o.status.code() == Some(127) => {
                    tracing::info!("Captae 更新：pkexec 没能认证（{}），改用密码框", tail(&o));
                    why = Some(tail(&o));
                }
                // 认证过了、dpkg 自己出错
                Ok(o) => return Err(InstallError::Failed(tail(&o))),
                Err(e) => tracing::warn!("Captae 更新：pkexec 没运行起来：{e}"),
            }
        }
        let password = match ask_password() {
            Ask::Password(p) => p,
            Ask::Cancelled => return Err(InstallError::Cancelled),
            Ask::Unavailable => return Err(InstallError::Failed(why.unwrap_or_else(|| tr!("没能弹出输入密码的窗口")))),
        };
        let mut child = Command::new("sudo")
            .args(["-S", "-p", "", "dpkg", "-i"])
            .arg(&pkg)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| e.to_string())?;
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            let _ = writeln!(stdin, "{password}");
        }
        let o = child.wait_with_output().map_err(|e| e.to_string())?;
        if o.status.success() {
            Ok(())
        } else {
            Err(InstallError::Failed(tail(&o)))
        }
    }

    enum Ask {
        Password(String),
        Cancelled,
        Unavailable,
    }

    /// zenity，没有就 kdialog。点了取消（退出码 1）算取消；没装、弹不出来（没有图形界面等）换下一个
    fn ask_password() -> Ask {
        let text = tr!("输入密码来安装 Captae 的更新");
        let tries: [(&str, Vec<String>); 2] = [
            ("zenity", vec!["--password".into(), format!("--title={text}")]),
            ("kdialog", vec!["--title".into(), "Captae".into(), "--password".into(), text.clone()]),
        ];
        for (prog, args) in tries {
            if captae_core::tools::which(prog).is_none() {
                continue;
            }
            match Command::new(prog).args(&args).stdin(Stdio::null()).stderr(Stdio::null()).output() {
                Ok(o) if o.status.success() => {
                    let p = String::from_utf8_lossy(&o.stdout);
                    return Ask::Password(p.trim_end_matches(['\r', '\n']).to_string());
                }
                Ok(o) if o.status.code() == Some(1) => return Ask::Cancelled,
                _ => continue,
            }
        }
        Ask::Unavailable
    }

    /// 出错时给人看的：stderr（没有就 stdout）最后几行，都没有就是退出码
    fn tail(o: &Output) -> String {
        let pick = if o.stderr.iter().any(|b| !b.is_ascii_whitespace()) { &o.stderr } else { &o.stdout };
        let s = String::from_utf8_lossy(pick);
        let lines: Vec<&str> = s.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        let t = lines[lines.len().saturating_sub(3)..].join(" / ");
        if t.is_empty() {
            format!("exit {}", o.status.code().unwrap_or(-1))
        } else {
            t.chars().take(400).collect()
        }
    }
}

/// 正常退出时（`RunEvent::Exit`，已经收拾完）：有下好的新版、自动更新开着、不用输密码、不是关机，就装上（装完不打开）
pub fn on_exit(app: &AppHandle, quitting_on_signal: bool) {
    let Some(u) = app.try_state::<AppUpdate>() else { return };
    let st = u.status(app);
    if st.state != "ready" || !st.auto || !st.install_on_quit || st.blocked.is_some() || quitting_on_signal || shutting_down() {
        return;
    }
    let Some(v) = st.latest.clone() else { return };
    if u.fails(&v) >= MAX_FAILS || u.busy.swap(true, Ordering::SeqCst) {
        return;
    }
    match install(app, false) {
        Ok(v) => tracing::info!("Captae 更新：退出时装好了 {v}，下次打开就是新版"),
        Err(e) => {
            // 退出流程里不发事件（窗口已经没了），只记下来，下次打开时设置里能看到
            let e = e.text();
            tracing::warn!("Captae 更新：退出时没装成：{e}");
            let mut i = u.inner.lock().unwrap();
            i.saved.error = Some(Problem { step: "install".into(), message: e, at: now_ms() });
            *i.saved.fails.entry(v).or_default() += 1;
            let s = i.saved.clone();
            drop(i);
            u.save(&s);
        }
    }
}

/// Windows 正在关机或注销：这时不运行安装程序
fn shutting_down() -> bool {
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_SHUTTINGDOWN};
        return GetSystemMetrics(SM_SHUTTINGDOWN) != 0;
    }
    #[allow(unreachable_code)]
    false
}

/// 装不了的原因说给人听
pub fn blocked_text(b: &str) -> String {
    match b {
        "dmg" => tr!("在磁盘映像里打开的 Captae 不能自动更新，先把它拖进「应用程序」文件夹"),
        "translocated" => tr!("先把 Captae 拖进「应用程序」文件夹再打开，才能自动更新"),
        "readonly" => tr!("Captae 所在的文件夹不能写入，不能自动更新"),
        "no_pkexec" => tr!("系统里没有输入密码用的 pkexec，不能自动更新"),
        "test" => tr!("测试数据目录下不自动更新"),
        _ => tr!("这个版本的 Captae 不能自动更新"),
    }
}

// ------------------------------------------------------------------ 页面调的命令

#[tauri::command]
pub fn app_update_status(app: AppHandle) -> Result<Status, String> {
    Ok(match app.try_state::<AppUpdate>() {
        Some(u) => u.status(&app),
        None => unavailable(&app),
    })
}

/// 「检查更新」
#[tauri::command]
pub async fn app_update_check(app: AppHandle) -> Result<Status, String> {
    Ok(check(&app).await)
}

/// 「下载并更新」（自动更新关着、或者自动下载失败了 3 次以后）
#[tauri::command]
pub async fn app_update_download(app: AppHandle) -> Result<Status, String> {
    download(&app).await
}

/// 「重新打开」：装上下好的新版并重新打开
#[tauri::command]
pub async fn app_update_restart(app: AppHandle) -> Result<(), String> {
    restart(&app).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names() {
        assert_eq!(file_name("app", "0.3.0"), "Captae_0.3.0.app.tar.gz");
        assert_eq!(file_name("nsis", "0.3.0"), "Captae_0.3.0.exe");
        assert_eq!(file_name("appimage", "0.3.0"), "Captae_0.3.0.AppImage");
        assert_eq!(file_name("deb", "0.3.0"), "Captae_0.3.0.deb");
    }

    #[test]
    fn versions() {
        assert!(newer("0.3.0", "0.2.0"));
        assert!(newer("0.10.0", "0.9.9"));
        assert!(!newer("0.2.0", "0.2.0"));
        assert!(!newer("0.2.0", "0.3.0"));
    }

    #[test]
    fn unknown_install_cannot_update() {
        assert_eq!(blocked_for("unknown", Path::new("/tmp/captae-desktop")), (Some("unsupported"), false));
        assert_eq!(blocked_for("nsis", Path::new("C:/Users/a/AppData/Local/Captae/captae-desktop.exe")), (None, false));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn mac_bundle_rules() {
        // 被系统转移运行的
        let t = Path::new("/private/var/folders/xx/T/AppTranslocation/ABC/d/Captae.app/Contents/MacOS/captae-desktop");
        assert_eq!(blocked_for("app", t), (Some("translocated"), false));
        // 不在 .app 里（开发时直接运行 target/debug/captae-desktop）
        assert_eq!(blocked_for("app", Path::new("/Users/x/Captae/target/debug/captae-desktop")), (Some("unsupported"), false));
        // 能写的文件夹里的 .app
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("Captae.app/Contents/MacOS/captae-desktop");
        assert_eq!(app_bundle(&exe).unwrap(), dir.path().join("Captae.app"));
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        assert_eq!(blocked_for("app", &exe), (None, false));
        // .app 本身没有写权限（另一个账号装的就是这样）：能装，但要管理员密码，退出时不装
        use std::os::unix::fs::PermissionsExt;
        let bundle = dir.path().join("Captae.app");
        std::fs::set_permissions(&bundle, std::fs::Permissions::from_mode(0o555)).unwrap();
        assert_eq!(blocked_for("app", &exe), (None, true));
        std::fs::set_permissions(&bundle, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn saved_state_round_trips() {
        let mut s = Saved::default();
        s.fails.insert("0.3.0".into(), 2);
        s.ready = Some(Ready { version: "0.3.0".into(), file: "Captae_0.3.0.app.tar.gz".into(), raw: json!({"version": "0.3.0"}) });
        let back: Saved = serde_json::from_slice(&serde_json::to_vec(&s).unwrap()).unwrap();
        assert_eq!(back.fails["0.3.0"], 2);
        assert_eq!(back.ready.unwrap().file, "Captae_0.3.0.app.tar.gz");
        // 旧的、不完整的记录也读得出
        let old: Saved = serde_json::from_str(r#"{"lastCheck": 5}"#).unwrap();
        assert_eq!(old.last_check, Some(5));
        assert!(old.ready.is_none());
    }
}
