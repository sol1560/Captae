//! Captae 手机端的 Rust 层：用 UniFFI 把 captae-core 交给 Swift（iOS）和 Kotlin（Android）。
//!
//! 一个 `MobileCore` 对象管一个数据目录，App 启动时建一个、一直用着：
//! - 初始化：`MobileCore::new(InitOptions)`：数据目录、下载目录、平台（iOS 上架版 / iOS 完整版 / Android）、语言、Android 的原生库目录
//! - 任务：`add`、`jobs`、`job`、`job_log`、`action`（`JobAction` 全部动作）、`set_variant`（M3U8 选清晰度）、`choose_entries`（播放列表选条目）、
//!   `pause_all`、`resume_all`
//! - 读取信息：`probe(链接, CancelToken)`（async；UniFFI 的 async 不能从 Swift / Kotlin 那边取消，所以另给一个 `CancelToken`，取消时调它的 `cancel()`）
//! - 事件：`set_listener(CoreListener)`：任务变化、移除、完成、素材库变化、只提示一次的事
//! - 素材库：`library`（类型、搜索、分页）、`asset`、`delete_asset`、`thumbnail_path`
//! - 设置：`settings`、`set_settings`、`set_language`；`info`（版本、语言、证书校验、外部程序）
//! - 失败：`MobileError.Failed(detail)`，`detail` 是和桌面端同一套的错误码、说法、按钮、网站名
//!
//! 外部程序（yt-dlp、ffmpeg……）按平台找：Android 在原生库目录里找 `lib*.so`，iOS 一个都没有（`tools::Runner`）。
//! 没有 aria2c 时直链用 Rust 自己下（engine/direct.rs），没有 ffmpeg 时 M3U8 用 Rust 封装成 MP4（engine/remux.rs）。
//! tokio 的 Runtime 由这里持有。回调在 Rust 的后台线程上调，界面那边自己切回主线程。

uniffi::setup_scaffolding!();

#[cfg(target_os = "android")]
mod android;
mod engine;
mod import;
mod page;
mod remote;
mod types;

pub use engine::*;
pub use import::*;
pub use remote::*;
pub use types::*;

use captae_core::detect::{self, Kind};
use captae_core::errors;
use captae_core::tools::{AndroidRunner, InProcessRunner, Runner};
use captae_core::{i18n, tr, AddRequest, Core, Event, Options, Paths, Settings};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

/// 设置里只有手机端用的几项（核心库的 settings.json 没有这些），存在数据目录的 mobile.json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Prefs {
    default_quality: String,
    default_format: String,
}

impl Default for Prefs {
    fn default() -> Self {
        // 协议值，不翻译（docs/I18N.md 2.4）
        Prefs { default_quality: "最高可用".into(), default_format: "MP4".into() }
    }
}

/// 读取信息这类长调用的取消开关：Swift / Kotlin 建一个传进去，要取消时调 `cancel()`
#[derive(Debug, Default, uniffi::Object)]
pub struct CancelToken {
    token: CancellationToken,
}

#[uniffi::export]
impl CancelToken {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(CancelToken::default())
    }
    pub fn cancel(&self) {
        self.token.cancel();
    }
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }
}

/// 事件回调，Swift / Kotlin 实现它。在 Rust 的后台线程上调，不要在里面做耗时的事
#[uniffi::export(with_foreign)]
pub trait CoreListener: Send + Sync {
    /// 某个任务变了（进度、状态）。进度最多每 250 毫秒一次
    fn on_job(&self, job: MobileJob);
    /// 任务被取消 / 移除了
    fn on_removed(&self, id: u64);
    /// 下完、进了素材库
    fn on_finished(&self, id: u64, title: String, path: String, asset_id: Option<i64>);
    /// 素材库变了（新素材、删了素材），重新查一次
    fn on_library(&self);
    /// 只提示一次的事（比如上次退出时还在录的直播已经保存好了）
    fn on_notice(&self, key: String, title: String, text: String);
    /// 回调太慢、中间的事件被挤掉了：重新调 `jobs()` 对齐
    fn on_resync(&self);
}

#[derive(uniffi::Object)]
pub struct MobileCore {
    // 先丢 core 再丢 rt（字段按声明顺序丢）
    core: Core,
    platform: Platform,
    listener: Mutex<Option<tokio::task::JoinHandle<()>>>,
    rt: Rt,
}

/// 持有 tokio 的 Runtime。遥控电脑的后台任务（remote/）也拿着 `MobileCore`，最后一个引用可能在 Runtime 自己的线程上放掉：
/// 那时不能照常丢 Runtime（tokio 会 panic），改成 `shutdown_background`
struct Rt(Option<tokio::runtime::Runtime>);

impl std::ops::Deref for Rt {
    type Target = tokio::runtime::Runtime;
    fn deref(&self) -> &tokio::runtime::Runtime {
        self.0.as_ref().expect("Runtime 只在丢掉时取走")
    }
}

impl Drop for Rt {
    fn drop(&mut self) {
        if let Some(rt) = self.0.take() {
            if tokio::runtime::Handle::try_current().is_ok() {
                rt.shutdown_background();
            } else {
                drop(rt);
            }
        }
    }
}

/// 被丢掉时结束后台任务（Swift / Kotlin 那边放弃了 await 时，读取信息也停下）
struct AbortOnDrop<T>(tokio::task::JoinHandle<T>);

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        self.0.abort();
    }
}

fn cancelled() -> MobileError {
    MobileError::Failed { detail: MobileFailure::coded("cancelled", tr!("读取信息已取消"), None) }
}

fn fail(e: anyhow::Error, url: &str) -> MobileError {
    types::to_mobile(&e, url)
}

impl MobileCore {
    fn prefs_path(&self) -> PathBuf {
        self.core.paths.data.join("mobile.json")
    }
    fn prefs(&self) -> Prefs {
        std::fs::read(self.prefs_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    /// 上架版的规矩：YouTube 这类要 yt-dlp 的网站在电脑上下；别的网页要先读取信息、找到里面的媒体地址
    fn store_rule(&self, url: &str) -> Result<(), MobileError> {
        if self.platform != Platform::IosStore || detect::detect(url).is_none_or(|d| d.kind != Kind::Page) {
            return Ok(());
        }
        if let Some(name) = detect::known_site(url) {
            let site = i18n::dynamic(name);
            return Err(fail(errors::coded("unsupported_here", tr!("{site}要在电脑上下载", site = site)), url));
        }
        Err(fail(errors::coded("probe_first", tr!("这个网页要先读取信息，找到里面的视频地址再下载")), url))
    }
}

/// 读取信息（在 Runtime 里跑）
async fn probe_inner(core: Core, platform: Platform, url: String) -> Result<MobileProbe, MobileError> {
    let url = detect::canonical(&url);
    let d = detect::detect(&url).ok_or_else(|| fail(anyhow::anyhow!(tr!("链接好像不完整，检查一下是不是少复制了开头的 https://")), &url))?;
    let tools = core.toolbox();
    let http = core.http();
    let headers = Default::default();
    if platform != Platform::IosStore || d.kind != Kind::Page {
        let p = captae_core::probe::probe(&tools, &http, &url, &headers).await.map_err(|e| fail(e, &url))?;
        return Ok(p.into());
    }
    // 上架版读网页：认得的 yt-dlp 网站直接说「在电脑上下」，别的找网页里直接写着的媒体地址
    if let Some(name) = detect::known_site(&url) {
        let site = i18n::dynamic(name);
        return Err(fail(errors::coded("unsupported_here", tr!("{site}要在电脑上下载", site = site)), &url));
    }
    let found = page::scan(&http, &url).await.map_err(|e| fail(e, &url))?;
    let Some(first) = found.media.first().cloned() else {
        return Err(fail(errors::coded("unsupported_here", tr!("这个网页里没有直接写着的视频地址，要在电脑上下载")), &url));
    };
    let mut h = std::collections::BTreeMap::new();
    h.insert("Referer".to_string(), url.clone());
    let p = captae_core::probe::probe(&tools, &http, &first, &h).await.map_err(|e| fail(e, &first))?;
    let mut p: MobileProbe = p.into();
    if found.title.is_some() {
        p.title = found.title;
    }
    p.kind = "page".into();
    p.resolved_url = Some(first);
    p.media_urls = found.media;
    Ok(p)
}

#[uniffi::export]
impl MobileCore {
    /// 打开数据目录（没有就建）。恢复上次没下完的任务（App 被系统结束后再打开，接着下）
    #[uniffi::constructor]
    pub fn new(o: InitOptions) -> Result<Arc<Self>, MobileError> {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).thread_name("captae").enable_all().build().map_err(anyhow::Error::from)?;
        let data = PathBuf::from(&o.data_dir);
        let dl = PathBuf::from(&o.downloads_dir);
        // 手机上视频、声音、图片先放同一个下载目录（存相册 / 「文件」是 App 那边的事，见 PLAN 5.4「保存」）
        let paths = Paths { work: data.join("work"), thumbs: data.join("thumbs"), shares: data.join("shares"), data, movies: dl.clone(), music: dl.clone(), pictures: dl };
        paths.ensure()?;
        // 语言写进设置（核心库打开时、改设置时都按设置里的语言来）；iOS 不许在 App 里更新代码，自动更新恒为关
        let sp = paths.settings();
        let mut s = Settings::load(&sp);
        let fresh = !sp.exists();
        let mut changed = fresh || s.language != o.language;
        s.language = o.language.clone();
        if o.platform.ios() && s.auto_update {
            s.auto_update = false;
            changed = true;
        }
        if fresh {
            // 手机上读不到系统代理（scutil 只在 macOS 上有），默认不用代理；Android 的自动更新 M3 才接上，先关着
            s.proxy_mode = "none".into();
            s.auto_update = false;
        }
        if changed {
            s.save(&sp)?;
        }
        let runner: Arc<dyn Runner> = match o.platform {
            Platform::Android => Arc::new(AndroidRunner::new(o.native_lib_dir.clone().unwrap_or_default())),
            Platform::IosStore | Platform::IosFull => Arc::new(InProcessRunner),
        };
        let core = rt.block_on(Core::open_with(Options { paths, sidecar_dir: None, restore: true }, runner))?;
        Ok(Arc::new(MobileCore { core, platform: o.platform, listener: Mutex::new(None), rt: Rt(Some(rt)) }))
    }

    // ------------------------------------------------------------------ 任务

    /// 加一个下载任务。上架版不收要 yt-dlp 的网页（`unsupported_here`），别的网页要先 `probe` 拿到 `resolved_url` 再加（`probe_first`）
    pub fn add(&self, o: AddOptions) -> Result<MobileJob, MobileError> {
        let _g = self.rt.enter();
        let url = detect::canonical(o.url.trim());
        self.store_rule(&url)?;
        let prefs = self.prefs();
        let page_link = detect::detect(&url).is_some_and(|d| d.kind == Kind::Page);
        let mut headers = std::collections::BTreeMap::new();
        if let Some(p) = o.page_url.as_ref().filter(|p| !p.is_empty()) {
            headers.insert("Referer".to_string(), p.clone());
        }
        let req = AddRequest {
            url: url.clone(),
            quality: o.quality.or_else(|| page_link.then(|| prefs.default_quality.clone())),
            format: o.format.or_else(|| (page_link && !o.audio_only).then(|| prefs.default_format.clone())),
            audio_only: o.audio_only,
            page_url: o.page_url,
            page_title: o.page_title,
            headers,
            // 用户在 App 里亲手点的下载：和桌面端界面上粘贴链接一样，不排队
            origin: Some("ui".into()),
            ..Default::default()
        };
        let j = self.core.add(req).map_err(|e| fail(e, &url))?;
        Ok(MobileJob::from(&j))
    }

    /// 全部任务（按 id 排）
    pub fn jobs(&self) -> Vec<MobileJob> {
        let mut v: Vec<MobileJob> = self.core.jobs().iter().map(MobileJob::from).collect();
        v.sort_by_key(|j| j.id);
        v
    }

    pub fn job(&self, id: u64) -> Option<MobileJob> {
        self.core.job(id).as_ref().map(MobileJob::from)
    }

    /// 任务记录（任务详情里的「下载记录」）
    pub fn job_log(&self, id: u64) -> Vec<MobileLogLine> {
        self.core.job(id).map(|j| j.log.iter().map(|l| MobileLogLine { at: l.0, text: l.1.clone(), level: l.2.to_string() }).collect()).unwrap_or_default()
    }

    pub fn action(&self, id: u64, action: MobileAction) -> Result<(), MobileError> {
        let _g = self.rt.enter();
        self.core.act(id, action.core()).map_err(|e| fail(e, ""))
    }

    /// M3U8 等选清晰度时选第几个（`MobileJob.variants` 的下标）。倒计时照走，要马上开始再调 `action(PickGo)`
    pub fn set_variant(&self, id: u64, index: u32) -> Result<(), MobileError> {
        let _g = self.rt.enter();
        self.core.set_variant(id, index as usize).map_err(|e| fail(e, ""))
    }

    /// 播放列表选好条目（`MobileEntry.id`）：每个条目一个新任务，返回新任务
    pub fn choose_entries(&self, id: u64, ids: Vec<String>) -> Result<Vec<MobileJob>, MobileError> {
        let _g = self.rt.enter();
        Ok(self.core.choose(id, &ids).map_err(|e| fail(e, ""))?.iter().map(MobileJob::from).collect())
    }

    pub fn pause_all(&self) {
        let _g = self.rt.enter();
        self.core.pause_all();
    }

    pub fn resume_all(&self) {
        let _g = self.rt.enter();
        self.core.resume_all();
    }

    /// 读取信息（标题、时长、封面、清晰度和大小）。`cancel` 取消时马上返回 `cancelled`，正在跑的读取一起停下。
    /// 上架版读网页时只找网页里直接写着的媒体地址（结果里的 `resolved_url`）
    pub async fn probe(&self, url: String, cancel: Option<Arc<CancelToken>>) -> Result<MobileProbe, MobileError> {
        let token = cancel.map(|c| c.token.clone()).unwrap_or_default();
        let (core, platform) = (self.core.clone(), self.platform);
        let mut task = AbortOnDrop(self.rt.spawn(async move {
            tokio::select! {
                biased;
                _ = token.cancelled() => Err(cancelled()),
                r = probe_inner(core, platform, url) => r,
            }
        }));
        match (&mut task.0).await {
            Ok(r) => r,
            Err(e) => Err(fail(anyhow::anyhow!("probe task: {e}"), "")),
        }
    }

    // ------------------------------------------------------------------ 事件

    /// 设置回调（再设一次会换掉上一个）
    pub fn set_listener(&self, listener: Arc<dyn CoreListener>) {
        let mut rx = self.core.subscribe();
        let task = self.rt.spawn(async move {
            use tokio::sync::broadcast::error::RecvError;
            loop {
                match rx.recv().await {
                    Ok(Event::Job { job }) => listener.on_job(MobileJob::from(job.as_ref())),
                    Ok(Event::JobRemoved { id }) => listener.on_removed(id),
                    Ok(Event::Finished { id, title, path, asset_id }) => listener.on_finished(id, title, path.display().to_string(), asset_id),
                    Ok(Event::Library) => listener.on_library(),
                    Ok(Event::Notice { key, title, text }) => listener.on_notice(key, title, text),
                    Ok(_) => {}
                    Err(RecvError::Lagged(_)) => listener.on_resync(),
                    Err(RecvError::Closed) => break,
                }
            }
        });
        if let Some(old) = self.listener.lock().unwrap().replace(task) {
            old.abort();
        }
    }

    // ------------------------------------------------------------------ 素材库

    /// 查素材库（按下载时间从新到旧）
    pub fn library(&self, q: LibraryQuery) -> Result<LibraryPage, MobileError> {
        let query = captae_core::library::Query {
            kind: q.kind.filter(|k| !k.is_empty() && k != "all"),
            q: q.search.filter(|s| !s.trim().is_empty()),
            limit: (q.limit > 0).then_some(q.limit),
            offset: (q.offset > 0).then_some(q.offset),
            ..Default::default()
        };
        let (items, total) = self.core.lib.query(&query)?;
        Ok(LibraryPage { items: items.iter().map(MobileAsset::from).collect(), total })
    }

    pub fn asset(&self, id: i64) -> Result<Option<MobileAsset>, MobileError> {
        Ok(self.core.lib.get(id)?.as_ref().map(MobileAsset::from))
    }

    /// 从素材库里去掉。`delete_file`：同时删掉文件和缩略图（已经存进相册的不受影响）
    pub fn delete_asset(&self, id: i64, delete_file: bool) -> Result<(), MobileError> {
        let a = self.core.lib.get(id)?;
        self.core.lib.remove(id)?;
        if let Some(a) = a {
            if let Some(t) = &a.thumb {
                let _ = std::fs::remove_file(t);
            }
            if delete_file {
                if let Err(e) = std::fs::remove_file(&a.path) {
                    if e.kind() != std::io::ErrorKind::NotFound {
                        return Err(fail(e.into(), ""));
                    }
                }
            }
        }
        self.core.emit_library();
        Ok(())
    }

    /// 缩略图文件（还在的话）
    pub fn thumbnail_path(&self, id: i64) -> Option<String> {
        let a = self.core.lib.get(id).ok().flatten()?;
        a.thumb.filter(|t| t.is_file()).map(|t| t.display().to_string())
    }

    // ------------------------------------------------------------------ 设置

    pub fn settings(&self) -> MobileSettings {
        let s = self.core.settings();
        let p = self.prefs();
        MobileSettings {
            concurrency: s.concurrency,
            pick_seconds: s.pick_seconds,
            proxy_mode: s.proxy_mode,
            proxy_url: s.proxy_url,
            auto_update: s.auto_update && !self.platform.ios(),
            default_quality: p.default_quality,
            default_format: p.default_format,
            language: s.language,
        }
    }

    /// 改设置（全部几项一起给，`settings()` 拿到的改完再传回来）。超出范围的值收到范围里，iOS 上 `auto_update` 恒为 false
    pub fn set_settings(&self, s: MobileSettings) -> Result<MobileSettings, MobileError> {
        let _g = self.rt.enter();
        let mode = if matches!(s.proxy_mode.as_str(), "system" | "none" | "manual") { s.proxy_mode.clone() } else { "none".into() };
        let patch = serde_json::json!({
            "concurrency": s.concurrency.clamp(1, 10),
            "pickSeconds": s.pick_seconds.clamp(1, 60),
            "proxyMode": mode,
            "proxyUrl": s.proxy_url.trim(),
            "autoUpdate": s.auto_update && !self.platform.ios(),
            "language": s.language,
        });
        self.core.patch_settings(&patch)?;
        let p = Prefs { default_quality: s.default_quality, default_format: s.default_format };
        std::fs::write(self.prefs_path(), serde_json::to_vec_pretty(&p).map_err(anyhow::Error::from)?).map_err(|e| fail(e.into(), ""))?;
        Ok(self.settings())
    }

    /// 换界面语言（空 = 跟随系统），返回实际用的语言代码。之后生成的说法、任务记录用新语言，已经生成的不变
    pub fn set_language(&self, language: String) -> Result<String, MobileError> {
        let _g = self.rt.enter();
        self.core.patch_settings(&serde_json::json!({ "language": language }))?;
        Ok(i18n::current().code().to_string())
    }

    pub fn info(&self) -> MobileInfo {
        MobileInfo {
            version: env!("CARGO_PKG_VERSION").to_string(),
            platform: self.platform.code().to_string(),
            language: i18n::current().code().to_string(),
            tls: captae_core::net::tls_kind().to_string(),
            runner: self.core.toolbox().runner.kind().to_string(),
        }
    }
}

impl MobileCore {
    /// 数据目录（测试、冒烟程序用）
    pub fn data_dir(&self) -> &Path {
        &self.core.paths.data
    }
}

impl Drop for MobileCore {
    fn drop(&mut self) {
        if let Some(t) = self.listener.lock().unwrap().take() {
            t.abort();
        }
    }
}

#[cfg(test)]
mod tests;
