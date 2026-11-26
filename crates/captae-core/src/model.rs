//! 队列任务的数据模型。
//!
//! 序列化后的字段名和前端 `apps/desktop/src/lib/types.ts` 的 `Job` 一一对应（见 docs/DESIGN.md「下载引擎」）
//! （id / title / src / eng / fmt / size / got / speed / state / phase / segs …），
//! 前端拿到就能直接渲染，不需要再做一层转换。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub type JobId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    /// 等待空位
    Queue,
    /// 正在跑
    Run,
    /// 用户暂停
    Pause,
    Done,
    Failed,
}

impl JobState {
    pub fn is_active(self) -> bool {
        matches!(self, JobState::Queue | JobState::Run | JobState::Pause)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// 读取视频信息
    Probe,
    /// 等用户选清晰度（M3U8 多码率时）
    Pick,
    /// 下载中
    Dl,
    /// 合并分片 / 生成视频文件
    Merge,
    /// 下载后处理：转格式、预设步骤
    Post,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Engine {
    #[serde(rename = "yt-dlp")]
    YtDlp,
    #[serde(rename = "aria2")]
    Aria2,
    #[serde(rename = "M3U8")]
    Hls,
}

/// 分片状态，数值与设计稿 `.segstrip i.s0..s4` 对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SegState {
    Wait = 0,
    Loading = 1,
    Done = 2,
    Retrying = 3,
    Failed = 4,
}

impl Serialize for SegState {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u8(*self as u8)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Variant {
    /// 1920×1080
    pub r: String,
    /// 1080p
    pub p: String,
    /// 码率 Mbps
    pub b: f64,
    /// 预估大小 MB
    pub mb: f64,
    /// 原始码率（bit/s，主列表的 AVERAGE-BANDWIDTH，没有时 BANDWIDTH）。`b` 只留一位小数给人看，估大小用这个
    #[serde(skip)]
    pub bps: f64,
    #[serde(skip)]
    pub uri: String,
    /// 独立音轨（EXT-X-MEDIA TYPE=AUDIO）
    #[serde(skip)]
    pub audio: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogLine(pub i64, pub String, pub &'static str);

/// 下载请求。来自输入框、浏览器插件、剪贴板或 Agent。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AddRequest {
    pub url: String,
    /// "最高可用" / "2160p" / "1080p" / "720p" …
    pub quality: Option<String>,
    /// MP4 / WebM / MKV / MP3 / AAC / FLAC / 保持原格式
    pub format: Option<String>,
    pub audio_only: bool,
    pub preset: Option<String>,
    pub output_dir: Option<PathBuf>,
    /// 来源页面（插件嗅探时带上）
    pub page_url: Option<String>,
    pub page_title: Option<String>,
    /// 插件抓到的请求头（Referer / Cookie / User-Agent），M3U8 和直链需要它们才能通过防盗链
    pub headers: BTreeMap<String, String>,
    /// 来自插件嗅探：进入排队，不插队
    pub from_sniff: bool,
    /// 谁发起的：ui / extension / clipboard / agent:<client>
    pub origin: Option<String>,
    /// 高级模式下 agent 直接透传给底层工具的参数
    pub raw_args: Option<Vec<String>>,
    /// aria2 线程数
    pub threads: Option<u32>,
    /// 多视频帖子里的第几个（从 1 开始）：yt-dlp `--playlist-items`。选条目时生成的任务用
    pub playlist_item: Option<u32>,
    /// 正在直播时从现在开始录（不传时遇到直播会停下来，错误码 live）
    pub record_live: bool,
    /// 录直播最多录多少秒：录满了自动「停止并保存」（AI 助手默认 10 分钟、命令行 `--max-minutes`）；不传就一直录到有人点停止
    pub record_max_secs: Option<u64>,
    /// 浏览器插件带来的 cookies（`chrome.cookies.getAll` 的结果原样传过来，见 `login::Cookie`）。
    /// 只给这一次下载用：不写进 queue.json、不进任务记录和错误原文、不给 AI 助手；`Debug` 也不打印值
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub cookies: Vec<crate::login::Cookie>,
    /// 浏览器的 User-Agent（和 cookies 一起来，下载时用同一个，网站看到的是同一个浏览器）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

impl AddRequest {
    /// 存进 queue.json 的样子：去掉 cookies 和请求头里的 `Cookie`（登录状态不长期留在磁盘上）
    pub fn without_secrets(&self) -> AddRequest {
        let mut r = self.clone();
        r.cookies.clear();
        r.headers.retain(|k, _| !k.eq_ignore_ascii_case("cookie"));
        r
    }
}

/// 播放列表、多视频帖子里的一个条目（任务停下来等用户选时列出来）。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// 条目 id，选条目时传回来
    pub id: String,
    pub title: String,
    /// 时长（秒）
    pub dur: Option<f64>,
    pub thumb: Option<String>,
    /// 条目的页面地址（多视频帖子里各条目和帖子是同一个地址）
    pub url: String,
    /// 在列表里排第几（从 1 开始）
    pub n: u32,
    /// 有值时按序号下载（`--playlist-items`），没有时直接下载 `url`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<u32>,
}

/// YouTube `watch?v=…&list=…`：默认只下这一个，记下它属于哪个播放列表。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlaylistRef {
    pub id: String,
    /// 整个列表的地址：界面「改成下载整个列表」时拿它新建任务
    pub url: String,
    /// 一共几个（B 站分 P：读一次分 P 列表才知道，读不到就没有）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// 这一个是第几个（B 站链接里的 `?p=`）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
}

/// 录直播：什么时候开始录、录到哪里。存进 queue.json：Captae 在录制中途退出（或者崩溃）时，下次打开据此把录到的部分整理好
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Rec {
    /// 开始录的时刻（毫秒）：界面上「已录 12:34」从这里算
    pub since: i64,
    /// 停下的时刻（点了「停止并保存」、直播自己结束、录制被中断）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub until: Option<i64>,
    /// 正在写的原始数据：yt-dlp 是它这个任务的中间目录，M3U8 是 `<数据目录>/work/hls-rec-…` 目录，电台流是 aria2 写的那个文件
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<PathBuf>,
    /// 整理好的文件放在哪个文件夹（保存位置）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub out: Option<PathBuf>,
    /// 一共录了多少秒（整理好以后按文件的实际时长）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secs: Option<f64>,
    /// 点了「停止并保存」，正在让录制的进程收尾
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub stopping: bool,
    /// 录制被中断：Captae 退出时还在录，这次打开时才整理好
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub interrupted: bool,
    /// 最多录多少秒（请求里的 `record_max_secs`），界面上写「录满 10 分钟自动停止」
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
    /// 录满了设定的时长、自动停下的（不是有人点了「停止并保存」）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub full: bool,
}

fn ser_fail<S: serde::Serializer>(f: &Option<crate::errors::Failure>, s: S) -> Result<S::Ok, S::Error> {
    match f {
        Some(f) => f.to_json().serialize(s),
        None => s.serialize_none(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobAction {
    Toggle,
    Pause,
    Resume,
    Now,
    Up,
    Down,
    Cancel,
    Remove,
    Retry,
    Segretry,
    Pickgo,
    /// 录直播：停止并保存（让录制的进程收尾，录到的部分整理好放进素材库）。只对正在录制的任务有效
    Stop,
}

#[derive(Debug, Clone, Serialize)]
pub struct Job {
    pub id: JobId,
    pub title: String,
    /// host + path，展示用
    pub src: String,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plat: Option<String>,
    pub eng: Engine,
    pub fmt: String,
    pub dur: String,
    /// 缩略图：远程 URL 或本地文件路径
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumb: Option<String>,
    pub audio: bool,
    /// MB
    pub size: f64,
    /// MB
    pub got: f64,
    /// MB/s
    pub speed: f64,
    pub state: JobState,
    pub phase: Phase,
    pub pri: i64,
    pub boost: bool,
    /// 失败时给人看的一句话（旧界面用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
    /// 失败的详情：{code, message, raw, action, next, startsAt?}，见 `errors::Failure::to_json`
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "ser_fail")]
    pub fail: Option<crate::errors::Failure>,
    /// 播放列表 / 多视频帖子：停在 state=pause、phase=probe，等用户选条目
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub choose: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<Entry>,
    /// 这个视频属于哪个播放列表（YouTube `watch?v=…&list=…`）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playlist: Option<PlaylistRef>,
    /// 网站要求放慢（rate_limited）时，什么时候自动重试（毫秒时间戳）
    #[serde(rename = "retryAt", skip_serializing_if = "Option::is_none")]
    pub retry_at: Option<i64>,
    /// 已经自动重试了几次（rate_limited）
    #[serde(skip)]
    pub auto_retries: u8,
    /// 读取信息时额外用过的 yt-dlp 参数（YouTube 换客户端重试成功时），下载时照用
    #[serde(skip)]
    pub probe_args: Vec<String>,
    /// 报过 need_login / need_verify：之后重试时 YouTube 也带登录状态（其他网站登录过就一直带）
    #[serde(skip)]
    pub needs_login: bool,
    /// 读取时认出来是正在进行的直播，而且要从现在开始录（`record_live`）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub live: bool,
    /// 录直播的进度（开始录了才有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rec: Option<Rec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub when: Option<String>,
    /// 标题还是读取前先放的占位（「youtube.com · 正在读取标题…」），真标题还没拿到。界面、插件据此显示「正在读取」。
    /// 占位的文字跟着界面语言变，不能再拿标题里有没有「正在读取」来判断（旧版的 queue.json 没有这一项，恢复时照旧看标题）
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub untitled: bool,
    /// 这一次读取信息从什么时候开始（毫秒时间戳），界面上「读了多久」从这里算。
    /// 原来是在下载记录里找「正在读取视频信息」那一行，记录跟着界面语言变，不能再按文字找
    #[serde(rename = "probeAt", skip_serializing_if = "Option::is_none")]
    pub probe_at: Option<i64>,
    /// 从哪台手机加的（`origin` 是 `remote:<手机名>` 时的手机名）：桌面端下载列表写「来自 iPhone」。见 crates/captae-remote
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    /// 完成时间（毫秒时间戳），界面据此显示「3 分钟前」「昨天 22:15」
    #[serde(rename = "doneAt", skip_serializing_if = "Option::is_none")]
    pub done_at: Option<i64>,
    pub log: Vec<LogLine>,
    // ---- M3U8 专用 ----
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub segs: Vec<SegState>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tries: Vec<u8>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vars: Vec<Variant>,
    #[serde(rename = "vIdx")]
    pub v_idx: usize,
    #[serde(rename = "pickUntil", skip_serializing_if = "Option::is_none")]
    pub pick_until: Option<i64>,
    pub merge: f64,
    // ---- 结果 ----
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(rename = "assetId", skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<i64>,
    /// yt-dlp 这个任务自己的中间文件目录（`<保存位置>/.captae-tmp/<…>`）：失败、取消时整个删掉
    #[serde(skip)]
    pub tmp: Option<PathBuf>,
    #[serde(skip)]
    pub req: AddRequest,
    #[serde(skip)]
    pub meta: Option<crate::engine::MediaInfo>,
}

impl Job {
    /// 正在录直播（录制的进程在跑、还没点停止）：只有这时「停止并保存」有效，也不能暂停
    pub fn recording(&self) -> bool {
        self.state == JobState::Run && self.phase == Phase::Dl && self.rec.as_ref().is_some_and(|r| r.until.is_none() && !r.stopping)
    }

    pub fn log(&mut self, text: impl Into<String>) {
        self.log_lv(text, "");
    }
    pub fn warn(&mut self, text: impl Into<String>) {
        self.log_lv(text, "warn");
    }
    pub fn error(&mut self, text: impl Into<String>) {
        self.log_lv(text, "err");
    }
    fn log_lv(&mut self, text: impl Into<String>, lv: &'static str) {
        self.log.push(LogLine(crate::util::now_ms(), text.into(), lv));
        if self.log.len() > 200 {
            self.log.remove(0);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    /// 某个任务有变化（进度、状态、日志）
    Job { job: Box<Job> },
    /// 任务被移除
    JobRemoved { id: JobId },
    /// 任务完成并入库
    Finished { id: JobId, title: String, path: PathBuf, asset_id: Option<i64> },
    /// 素材库变化
    Library,
    /// Agent 调用记录新增或更新
    AgentCall { call: crate::agent::CallRecord },
    /// 浏览器插件推来的嗅探资源
    Sniffed { item: crate::sniff::Sniffed },
    /// 要问一句「你刚复制了一个视频链接」：复制以后 1 分钟内把 Captae 拿到前面时桌面端发（规则见 clipboard.rs）
    Clipboard { url: String },
    /// 后台任务（转换、字幕、剪辑、预设）的进度
    Task { task: crate::tasks::TaskStatus },
    /// 设置变了（界面各页据此刷新自己的快照）
    Settings { settings: crate::config::Settings },
    /// 只提示一次的事（比如第一次用 YouTube 账号下载），界面弹一条提示
    Notice { key: String, title: String, text: String },
}
