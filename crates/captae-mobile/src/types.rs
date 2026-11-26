//! 给 Swift / Kotlin 的数据类型（UniFFI 的 Record / Enum / Error）和从核心库类型的转换。
//!
//! 大小都是字节（核心库里是 MB 的小数，这里换算好），时间是毫秒时间戳。
//! 给人看的文字（错误说法、网站名、任务记录）已经按当前语言生成好，App 直接显示。

use captae_core::errors::{self, Failure};
use captae_core::{i18n, Job, JobState};

/// App 是哪个版本，决定能下什么（work/mobile/PLAN.md 第 2 节）
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum Platform {
    /// iOS 上架版：只下直链、M3U8、网页里直接写着的媒体地址；YouTube 这类网站说「在电脑上下」
    IosStore,
    /// iOS 完整版（自己编译安装）：和桌面一样交给 yt-dlp。M6 才接上进程内的 Python，现在 yt-dlp 的网站报 `tool`
    IosFull,
    /// Android 完整版：yt-dlp、ffmpeg 是原生库目录里的 `lib*.so`（M3 打包进去），子进程运行
    Android,
}

impl Platform {
    pub fn code(self) -> &'static str {
        match self {
            Platform::IosStore => "ios-store",
            Platform::IosFull => "ios-full",
            Platform::Android => "android",
        }
    }
    pub fn ios(self) -> bool {
        matches!(self, Platform::IosStore | Platform::IosFull)
    }
}

/// `MobileCore::new` 的参数
#[derive(Debug, Clone, uniffi::Record)]
pub struct InitOptions {
    /// 数据目录（素材库、设置、queue.json、中间文件、缩略图），App 沙盒里能写的位置
    pub data_dir: String,
    /// 下完的文件放哪（视频、声音、图片都放这里；存相册 / 「文件」是 App 的事）
    pub downloads_dir: String,
    pub platform: Platform,
    /// 界面语言：空 = 跟随系统；zh-CN / en / zh-TW / ja / ko / es / fr / de（docs/I18N.md）
    pub language: String,
    /// Android：`ApplicationInfo.nativeLibraryDir`（yt-dlp、ffmpeg 等打包成 `lib*.so` 放在这里）。iOS 传 nil
    pub native_lib_dir: Option<String>,
}

/// 失败详情：和桌面端、插件、AI 助手同一套（docs/DESIGN.md 4.2）
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MobileFailure {
    /// 15 个错误码之一（need_login、network、tool……），或者手机端自己的：
    /// `unsupported_here`（上架版：这个网站要在电脑上下）、`probe_first`（上架版：网页要先读取信息、找到里面的媒体地址再下）、`cancelled`（读取信息被取消了）
    pub code: String,
    /// 给人看的一句话（已经是当前语言）
    pub message: String,
    /// 界面上的下一步按钮：login / verify / retry / use_extension / wait_live / record_live / auto_retry / change_dir / repair / copy_error / none，
    /// 手机端另有 `use_computer`（发到电脑）、`probe`（先读取信息）
    pub action: String,
    /// 原文最后几行（能看、能复制）
    pub raw: String,
    /// 说法里用的网站名字（已经翻译好），「登录 X」按钮用同一个；认不出的网站没有
    pub site: Option<String>,
    /// 同一个错误码下再细分：image_post（图片帖）、radio（电台流）
    pub reason: Option<String>,
    /// 去掉结尾提示的短说法（只有带提示的说法才有）
    pub brief: Option<String>,
    /// 直播还没开始时的开始时间
    pub starts_at: Option<i64>,
}

impl MobileFailure {
    pub fn from_failure(f: &Failure) -> MobileFailure {
        MobileFailure {
            code: f.code.as_str().to_string(),
            message: f.message.clone(),
            action: f.code.action().to_string(),
            raw: f.raw.clone(),
            site: f.site.as_deref().map(i18n::dynamic),
            reason: f.reason.clone(),
            brief: f.brief.clone(),
            starts_at: f.starts_at,
        }
    }

    /// 手机端自己的固定代码（`errors::coded` 生成的）
    pub fn coded(code: &str, message: String, site: Option<String>) -> MobileFailure {
        let action = match code {
            "unsupported_here" => "use_computer",
            "probe_first" => "probe",
            _ => "none",
        };
        MobileFailure { code: code.into(), message, action: action.into(), raw: String::new(), site, reason: None, brief: None, starts_at: None }
    }
}

/// 给 Swift / Kotlin 的错误（Swift 里是 `MobileError.Failed(detail:)`，Kotlin 里是 `MobileException.Failed`）
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum MobileError {
    #[error("{}", .detail.message)]
    Failed { detail: MobileFailure },
}

impl MobileError {
    pub fn detail(&self) -> &MobileFailure {
        match self {
            MobileError::Failed { detail } => detail,
        }
    }
}

/// 核心库的错误 → 失败详情。固定代码（`errors::coded`）原样带出，其余按错误码归类（`errors::from_anyhow`）
pub fn to_mobile(e: &anyhow::Error, url: &str) -> MobileError {
    let detail = match errors::code_in(e) {
        Some(code) => MobileFailure::coded(code, e.to_string(), site_of(url)),
        None => MobileFailure::from_failure(&errors::from_anyhow(e, url)),
    };
    MobileError::Failed { detail }
}

impl From<anyhow::Error> for MobileError {
    fn from(e: anyhow::Error) -> Self {
        to_mobile(&e, "")
    }
}

/// 链接是哪个网站（翻译好的名字），认不出时没有
pub fn site_of(url: &str) -> Option<String> {
    captae_core::detect::known_site(url).or_else(|| errors::site_name(url)).map(i18n::dynamic)
}

/// 任务状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MobileJobState {
    /// 等空位
    Queued,
    Running,
    Paused,
    Done,
    Failed,
}

/// 对任务的操作，和桌面端的 `JobAction` 一一对应
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum MobileAction {
    /// 暂停着就继续，否则暂停
    Toggle,
    Pause,
    Resume,
    /// 马上开始（不排队）
    Now,
    /// 在队列里上移、下移
    Up,
    Down,
    /// 取消并删掉下到一半的文件
    Cancel,
    /// 从列表里去掉（已经完成或失败的）
    Remove,
    /// 失败的任务再试一次
    Retry,
    /// M3U8：只重试失败的分片
    SegmentRetry,
    /// M3U8：用选好的清晰度马上开始（不等倒计时）
    PickGo,
    /// 录直播：停止并保存
    Stop,
}

impl MobileAction {
    pub fn core(self) -> captae_core::JobAction {
        use captae_core::JobAction as A;
        match self {
            MobileAction::Toggle => A::Toggle,
            MobileAction::Pause => A::Pause,
            MobileAction::Resume => A::Resume,
            MobileAction::Now => A::Now,
            MobileAction::Up => A::Up,
            MobileAction::Down => A::Down,
            MobileAction::Cancel => A::Cancel,
            MobileAction::Remove => A::Remove,
            MobileAction::Retry => A::Retry,
            MobileAction::SegmentRetry => A::Segretry,
            MobileAction::PickGo => A::Pickgo,
            MobileAction::Stop => A::Stop,
        }
    }
}

/// `add` 的参数
#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct AddOptions {
    pub url: String,
    /// 「最高可用」/ 2160p / 1080p / 720p……（协议值，不翻译）；不给时用设置里的默认清晰度（只对网页链接）
    pub quality: Option<String>,
    /// MP4 / WEBM / MP3……，「保持原格式」；不给时用设置里的默认格式（只对网页链接）
    pub format: Option<String>,
    pub audio_only: bool,
    /// 媒体是从哪个网页里找到的（`probe` 的 `resolved_url` 来自这个网页时传原来的网页地址）：入库时记成来源，请求带上 Referer
    pub page_url: Option<String>,
    /// 网页标题，用作任务和素材的标题
    pub page_title: Option<String>,
}

/// 播放列表里的一个条目（任务停在「等你选」时列出来）
#[derive(Debug, Clone, uniffi::Record)]
pub struct MobileEntry {
    /// 选条目时传回来
    pub id: String,
    pub title: String,
    /// 秒
    pub duration: Option<f64>,
    pub thumb: Option<String>,
    pub url: String,
    /// 第几个（从 1 开始）
    pub index: u32,
}

impl From<&captae_core::Entry> for MobileEntry {
    fn from(e: &captae_core::Entry) -> Self {
        MobileEntry { id: e.id.clone(), title: e.title.clone(), duration: e.dur, thumb: e.thumb.clone(), url: e.url.clone(), index: e.n }
    }
}

/// YouTube `watch?v=…&list=…`、B 站分 P：这一个属于哪个列表
#[derive(Debug, Clone, uniffi::Record)]
pub struct MobilePlaylist {
    /// 整个列表的地址（「改成下载整个列表」时拿它新建任务）
    pub url: String,
    pub count: Option<u32>,
    pub index: Option<u32>,
}

impl From<&captae_core::PlaylistRef> for MobilePlaylist {
    fn from(p: &captae_core::PlaylistRef) -> Self {
        MobilePlaylist { url: p.url.clone(), count: p.count, index: p.index }
    }
}

/// M3U8 的一种清晰度
#[derive(Debug, Clone, uniffi::Record)]
pub struct MobileVariant {
    /// 1920×1080
    pub resolution: String,
    /// 1080p
    pub label: String,
    pub mbps: f64,
    /// 预估大小（字节）
    pub bytes: u64,
}

/// 一条任务记录
#[derive(Debug, Clone, uniffi::Record)]
pub struct MobileLogLine {
    pub at: i64,
    pub text: String,
    /// 空 / warn / err
    pub level: String,
}

/// 一个任务的快照。进度事件每 250 毫秒最多一次，所以这里不带任务记录（要看记录调 `job_log`）
#[derive(Debug, Clone, uniffi::Record)]
pub struct MobileJob {
    pub id: u64,
    pub title: String,
    /// 标题还是读取前的占位（真标题还没拿到），界面显示「正在读取」
    pub untitled: bool,
    /// 主机 + 路径，展示用
    pub src: String,
    pub url: String,
    /// youtube / bilibili / x / tiktok……（认得的平台）
    pub plat: Option<String>,
    /// yt-dlp / aria2 / M3U8
    pub engine: String,
    /// 「MP4 · 1080p」这类说明
    pub format: String,
    /// 时长「12:34」
    pub duration: String,
    pub thumb: Option<String>,
    pub audio_only: bool,
    pub state: MobileJobState,
    /// probe（读取信息）/ pick（等选清晰度）/ dl（下载中）/ merge（生成文件）/ post（后处理）/ done
    pub phase: String,
    /// 总大小；还不知道时是 0
    pub total_bytes: u64,
    pub got_bytes: u64,
    /// 字节 / 秒
    pub speed: u64,
    /// 生成文件的进度 0–100（phase 是 merge 时）
    pub merge_percent: f64,
    /// 播放列表：等用户选条目（`choose_entries`）
    pub choose: bool,
    pub entries: Vec<MobileEntry>,
    pub playlist: Option<MobilePlaylist>,
    /// M3U8 的清晰度（phase 是 pick 时选，`set_variant`）
    pub variants: Vec<MobileVariant>,
    pub variant_index: u32,
    /// 不选的话什么时候自动用最高清晰度
    pub pick_until: Option<i64>,
    /// M3U8 每个分片的状态：0 等待、1 下载中、2 完成、3 重试中、4 失败
    pub segments: Vec<u8>,
    /// 直播
    pub live: bool,
    /// 正在录（只有这时「停止并保存」有效，不能暂停）
    pub recording: bool,
    pub record_since: Option<i64>,
    pub record_max_secs: Option<u64>,
    /// 这一次读取信息从什么时候开始
    pub probe_at: Option<i64>,
    pub done_at: Option<i64>,
    /// 限流后什么时候自动重试
    pub retry_at: Option<i64>,
    /// 文件在哪（下载中是写到一半的文件）
    pub path: Option<String>,
    pub asset_id: Option<i64>,
    pub failure: Option<MobileFailure>,
}

/// 核心库的大小是 MB（10^6 字节）的小数
pub fn bytes(mb: f64) -> u64 {
    (mb * 1_000_000.0).round().max(0.0) as u64
}

/// serde 序列化成的字符串（Phase、Engine 的协议值）
pub fn code_str<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default()
}

impl From<&Job> for MobileJob {
    fn from(j: &Job) -> Self {
        MobileJob {
            id: j.id,
            title: j.title.clone(),
            untitled: j.untitled,
            src: j.src.clone(),
            url: j.url.clone(),
            plat: j.plat.clone(),
            engine: code_str(&j.eng),
            format: j.fmt.clone(),
            duration: j.dur.clone(),
            thumb: j.thumb.clone(),
            audio_only: j.audio,
            state: match j.state {
                JobState::Queue => MobileJobState::Queued,
                JobState::Run => MobileJobState::Running,
                JobState::Pause => MobileJobState::Paused,
                JobState::Done => MobileJobState::Done,
                JobState::Failed => MobileJobState::Failed,
            },
            phase: code_str(&j.phase),
            total_bytes: bytes(j.size),
            got_bytes: bytes(j.got),
            speed: bytes(j.speed),
            merge_percent: j.merge,
            choose: j.choose,
            entries: j.entries.iter().map(MobileEntry::from).collect(),
            playlist: j.playlist.as_ref().map(MobilePlaylist::from),
            variants: j.vars.iter().map(|v| MobileVariant { resolution: v.r.clone(), label: v.p.clone(), mbps: v.b, bytes: bytes(v.mb) }).collect(),
            variant_index: j.v_idx as u32,
            pick_until: j.pick_until,
            segments: j.segs.iter().map(|s| *s as u8).collect(),
            live: j.live,
            recording: j.recording(),
            record_since: j.rec.as_ref().map(|r| r.since),
            record_max_secs: j.rec.as_ref().and_then(|r| r.max),
            probe_at: j.probe_at,
            done_at: j.done_at,
            retry_at: j.retry_at,
            path: j.path.as_ref().map(|p| p.display().to_string()),
            asset_id: j.asset_id,
            failure: j.fail.as_ref().map(MobileFailure::from_failure).or_else(|| j.err.as_ref().map(|e| MobileFailure::from_failure(&Failure::new(errors::ErrorCode::Unknown, e.clone(), e.clone())))),
        }
    }
}

/// 读取信息（下载前看一眼）的一种清晰度
#[derive(Debug, Clone, uniffi::Record)]
pub struct MobileQuality {
    /// 1080p
    pub label: String,
    pub height: Option<u32>,
    /// 估算的字节数
    pub bytes: Option<u64>,
}

/// 读取信息的结果
#[derive(Debug, Clone, uniffi::Record)]
pub struct MobileProbe {
    /// page / hls / file
    pub kind: String,
    pub title: Option<String>,
    /// 秒
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub site: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// 从高到低
    pub qualities: Vec<MobileQuality>,
    /// 「只要声音」大概多大
    pub audio_bytes: Option<u64>,
    /// 直链文件的大小
    pub size: Option<u64>,
    /// 播放列表：要用户选条目
    pub choose: bool,
    pub entries: Vec<MobileEntry>,
    pub playlist: Option<MobilePlaylist>,
    pub is_live: bool,
    /// 上架版读的是网页：从网页里找到的媒体地址（上面的信息是它的）。下载时 `add` 传它，`page_url` 传原来的网页
    pub resolved_url: Option<String>,
    /// 网页里找到的全部媒体地址（按在网页里出现的顺序）
    pub media_urls: Vec<String>,
}

impl From<captae_core::probe::ProbeInfo> for MobileProbe {
    fn from(p: captae_core::probe::ProbeInfo) -> Self {
        MobileProbe {
            kind: p.kind,
            title: p.title,
            duration: p.duration,
            thumbnail: p.thumbnail,
            site: p.site,
            width: p.width,
            height: p.height,
            qualities: p.qualities.iter().map(|q| MobileQuality { label: q.label.clone(), height: q.height, bytes: q.bytes }).collect(),
            audio_bytes: p.audio,
            size: p.size,
            choose: p.choose,
            entries: p.entries.iter().map(MobileEntry::from).collect(),
            playlist: p.playlist.as_ref().map(MobilePlaylist::from),
            is_live: p.is_live,
            resolved_url: None,
            media_urls: vec![],
        }
    }
}

/// 素材库里的一条
#[derive(Debug, Clone, uniffi::Record)]
pub struct MobileAsset {
    pub id: i64,
    /// video / audio / image
    pub kind: String,
    pub title: String,
    pub description: String,
    /// 下载的链接
    pub url: String,
    /// 来源网页
    pub page_url: String,
    /// 来源（翻译好的网站名）
    pub site: String,
    pub uploader: String,
    pub path: String,
    /// MP4 / MP3……
    pub format: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// 秒
    pub duration: Option<f64>,
    pub bytes: u64,
    /// 缩略图文件（没有 ffmpeg 时没有）
    pub thumb: Option<String>,
    pub tags: Vec<String>,
    pub created_at: i64,
    /// 从哪条素材转换、剪辑来的
    pub parent_id: Option<i64>,
    /// 直播录像
    pub live: bool,
    /// 文件还在不在
    pub exists: bool,
}

impl From<&captae_core::library::Asset> for MobileAsset {
    fn from(a: &captae_core::library::Asset) -> Self {
        MobileAsset {
            id: a.id,
            kind: a.kind.clone(),
            title: a.title.clone(),
            description: a.desc.clone(),
            url: a.url.clone(),
            page_url: a.page.clone(),
            site: i18n::dynamic(&a.src),
            uploader: a.uploader.clone(),
            path: a.path.display().to_string(),
            format: a.fmt.clone(),
            width: a.width,
            height: a.height,
            duration: a.duration,
            bytes: a.bytes,
            thumb: a.thumb.as_ref().map(|p| p.display().to_string()),
            tags: a.tags.clone(),
            created_at: a.created_at,
            parent_id: a.parent_id,
            live: a.live,
            exists: a.exists,
        }
    }
}

/// 素材库查询
#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct LibraryQuery {
    /// video / audio / image；nil = 全部
    pub kind: Option<String>,
    /// 搜标题、简介、标签、来源
    pub search: Option<String>,
    /// 一页几条（0 = 默认 200，最多 1000）
    pub limit: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct LibraryPage {
    /// 按下载时间从新到旧
    pub items: Vec<MobileAsset>,
    /// 符合条件的一共几条（分页用）
    pub total: u64,
}

/// 手机上用得到的设置
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MobileSettings {
    /// 同时下载几个（1–10）
    pub concurrency: u32,
    /// M3U8 不选清晰度时几秒后自动用最高
    pub pick_seconds: u32,
    /// system（跟随系统：手机上读不到系统代理，等于不用）/ none / manual
    pub proxy_mode: String,
    /// 手动代理：127.0.0.1:7890、http://…、socks5://…
    pub proxy_url: String,
    /// 每天检查下载引擎有没有新版。iOS 上恒为 false（App Store 不允许在 App 里更新代码）；Android 可以开（M3 接上更新）
    pub auto_update: bool,
    /// 网页链接默认清晰度：「最高可用」/ 2160p / 1080p / 720p……（协议值，不翻译）
    pub default_quality: String,
    /// 网页链接默认格式：MP4 / WEBM /「保持原格式」
    pub default_format: String,
    /// 界面语言：空 = 跟随系统
    pub language: String,
}

/// 排错用：版本、平台、实际用的语言、证书校验、外部程序
#[derive(Debug, Clone, uniffi::Record)]
pub struct MobileInfo {
    pub version: String,
    pub platform: String,
    /// 实际用的语言代码（跟随系统时是算出来的那个）
    pub language: String,
    /// platform（系统证书校验）/ webpki（编进程序的根证书）
    pub tls: String,
    /// desktop / android / in-process
    pub runner: String,
}
