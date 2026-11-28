//! 错误码：把 yt-dlp / aria2 / ffmpeg / HTTP / 磁盘的报错归成 15 类（计划 2.4 的表）。
//!
//! 每个失败（`Failure`）带三样东西：
//! - `code`：机器读的错误码，界面、插件、AI 助手（MCP）、命令行用同一套
//! - `message`：给人看的一句话
//! - `raw`：原始报错（英文原文的最后几行），日志里能看到、能复制
//!
//! 下一步按钮由错误码决定（`ErrorCode::action`），界面不用自己猜。
//! 分类规则按顺序匹配，越具体的越靠前；匹配不到的算 `unknown`，给原文的最后一行。

use crate::i18n::dynamic;
use crate::{tr, tr_key};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// 年龄限制、会员、私密、要登录
    NeedLogin,
    /// 机器人检测、Cloudflare、抖音要新 cookies
    NeedVerify,
    /// DRM 保护
    Drm,
    /// 地区限制
    Geo,
    /// 证书不对、被网络拦截
    Blocked,
    /// 已删除、不存在
    Unavailable,
    /// 网站不认识
    Unsupported,
    /// 页面里没有媒体
    NoMedia,
    /// 直播还没开始
    LiveUpcoming,
    /// 正在直播
    Live,
    /// 429、请求太频繁
    RateLimited,
    /// 超时、断线、DNS
    Network,
    /// 磁盘满、没有写入权限
    Disk,
    /// 组件缺失或损坏
    Tool,
    /// 其他
    Unknown,
}

pub const ALL_CODES: [ErrorCode; 15] = [
    ErrorCode::NeedLogin,
    ErrorCode::NeedVerify,
    ErrorCode::Drm,
    ErrorCode::Geo,
    ErrorCode::Blocked,
    ErrorCode::Unavailable,
    ErrorCode::Unsupported,
    ErrorCode::NoMedia,
    ErrorCode::LiveUpcoming,
    ErrorCode::Live,
    ErrorCode::RateLimited,
    ErrorCode::Network,
    ErrorCode::Disk,
    ErrorCode::Tool,
    ErrorCode::Unknown,
];

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::NeedLogin => "need_login",
            ErrorCode::NeedVerify => "need_verify",
            ErrorCode::Drm => "drm",
            ErrorCode::Geo => "geo",
            ErrorCode::Blocked => "blocked",
            ErrorCode::Unavailable => "unavailable",
            ErrorCode::Unsupported => "unsupported",
            ErrorCode::NoMedia => "no_media",
            ErrorCode::LiveUpcoming => "live_upcoming",
            ErrorCode::Live => "live",
            ErrorCode::RateLimited => "rate_limited",
            ErrorCode::Network => "network",
            ErrorCode::Disk => "disk",
            ErrorCode::Tool => "tool",
            ErrorCode::Unknown => "unknown",
        }
    }

    pub fn parse(s: &str) -> Option<ErrorCode> {
        ALL_CODES.iter().copied().find(|c| c.as_str() == s)
    }

    /// 界面上的下一步按钮（计划 2.4「按钮」一列）。
    pub fn action(self) -> &'static str {
        match self {
            ErrorCode::NeedLogin => "login",
            ErrorCode::NeedVerify => "verify",
            ErrorCode::Drm | ErrorCode::Unavailable | ErrorCode::NoMedia => "none",
            ErrorCode::Geo | ErrorCode::Blocked | ErrorCode::Network => "retry",
            ErrorCode::Unsupported => "use_extension",
            ErrorCode::LiveUpcoming => "wait_live",
            ErrorCode::Live => "record_live",
            ErrorCode::RateLimited => "auto_retry",
            ErrorCode::Disk => "change_dir",
            ErrorCode::Tool => "repair",
            ErrorCode::Unknown => "copy_error",
        }
    }

    /// 给 AI 助手看的下一步（MCP 的结果里带上，AI 据此告诉用户该怎么办）。只用英文：给 AI 看的固定说明不跟界面语言（docs/I18N.md）
    pub fn next_step(self) -> &'static str {
        match self {
            ErrorCode::NeedLogin => "Ask the user to sign in to this site in Captae (or sign in in their browser and download with the Captae browser extension), then try again.",
            ErrorCode::NeedVerify => "Ask the user to complete the verification in the page Captae opens (or open this video once in their browser), then try again.",
            ErrorCode::Drm => "This content is DRM-protected and can't be downloaded. Don't retry.",
            ErrorCode::Geo => "This content isn't available in the user's region. It may work after switching network or proxy.",
            ErrorCode::Blocked => "The user's network blocked the connection. Ask the user to check their proxy settings, then try again.",
            ErrorCode::Unavailable => "The video was removed or doesn't exist. Don't retry.",
            ErrorCode::Unsupported => "Captae's download engine can't read this page (the link type isn't supported yet, the site changed, or it's an image post). The user can open the page in their browser and download it with the Captae browser extension.",
            ErrorCode::NoMedia => "There's no video or audio on this page. Check that the link is correct.",
            ErrorCode::LiveUpcoming => "The live stream hasn't started yet. Download it after it starts.",
            ErrorCode::Live => "This is a live stream in progress. To record from now on, call download with record_live: true, and set max_minutes to limit the length (default 10, at most 180).",
            ErrorCode::RateLimited => "The site asked to slow down. Wait a few minutes before trying again, and don't retry repeatedly.",
            ErrorCode::Network => "The network is unstable. Try again later.",
            ErrorCode::Disk => "The save location is full or not writable. Try again with a different output_dir.",
            ErrorCode::Tool => "A Captae download component is broken. Ask the user to check Captae's settings or reinstall Captae.",
            ErrorCode::Unknown => "Unknown error. Show the user the original text in raw.",
        }
    }

    /// 命令行里给人看的下一步，跟着界面语言。`next_step` 是给 AI 看的、只有英文；命令行打印给人看时用这个
    pub fn cli_hint(self) -> String {
        match self {
            ErrorCode::NeedLogin => crate::tr!("在 Captae 里登录这个网站（或者在浏览器里登录后用插件下载），然后重试"),
            ErrorCode::NeedVerify => crate::tr!("在 Captae 打开的网页里完成验证（或者在浏览器里打开一次这个视频），然后重试"),
            ErrorCode::Drm => crate::tr!("这个内容受 DRM 保护，下载不了"),
            ErrorCode::Geo => crate::tr!("这个内容在你所在的地区看不了，换个网络或代理以后可以重试"),
            ErrorCode::Blocked => crate::tr!("连接被你的网络拦截了，检查代理设置后重试"),
            ErrorCode::Unavailable => crate::tr!("视频已被删除或不存在"),
            ErrorCode::Unsupported => crate::tr!("下载引擎读不了这个网页（还不支持这种链接、网站改版了，或者是图片帖），可以在浏览器里打开页面，用 Captae 插件在网页里下载"),
            ErrorCode::NoMedia => crate::tr!("这个网页里没有视频或音频，看看链接对不对"),
            ErrorCode::LiveUpcoming => crate::tr!("直播还没开始，开始以后再下载"),
            ErrorCode::Live => crate::tr!("这是正在进行的直播，要从现在开始录，下载时加上 --record-live"),
            ErrorCode::RateLimited => crate::tr!("网站要求放慢请求，等几分钟再试"),
            ErrorCode::Network => crate::tr!("网络不稳定，稍后重试"),
            ErrorCode::Disk => crate::tr!("保存位置没有空间或者没有写入权限，换一个保存位置再试"),
            ErrorCode::Tool => crate::tr!("Captae 的下载组件有问题，打开 Captae 的设置看看，或者重新安装 Captae"),
            ErrorCode::Unknown => crate::tr!("没认出是什么问题，可以把报错原文发给我们"),
        }
    }

    /// 等一会儿或换个参数再试有可能成功（读取信息时自动重试一次）。
    pub fn transient(self) -> bool {
        matches!(self, ErrorCode::Network | ErrorCode::Unknown)
    }

    /// 可能是下载引擎本身坏了（自动更新据此决定要不要退回上一版）。
    pub fn engine_fault(self) -> bool {
        matches!(self, ErrorCode::Tool | ErrorCode::Unknown)
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 一次失败：错误码 + 给人看的一句话 + 原文。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Failure {
    pub code: ErrorCode,
    pub message: String,
    #[serde(default)]
    pub raw: String,
    /// 直播还没开始时的开始时间（毫秒时间戳）
    #[serde(default, rename = "startsAt", skip_serializing_if = "Option::is_none")]
    pub starts_at: Option<i64>,
    /// 说法里用的网站名字（`site_name`：「芒果TV」「哔哩哔哩」「YouTube」）：界面上的「登录芒果TV」按钮用同一个名字。
    /// 认不出的网站没有，界面退回主机名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    /// 同一个错误码下再细分的情况，界面、插件按它换说法和提示：`image_post`（图片帖）、`radio`（电台的直播流）。
    /// message 跟着界面语言变，不能再拿 message 里有没有「图片帖」「电台」来判断（docs/I18N.md）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// 去掉结尾「可以用浏览器插件在网页里下」这类提示的短说法：插件里显示它（用户已经在插件里了）。只有带这种提示的说法才有
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
}

impl Failure {
    pub fn new(code: ErrorCode, message: impl Into<String>, raw: impl Into<String>) -> Failure {
        Failure { code, message: message.into(), raw: tail(&raw.into()), starts_at: None, site: None, reason: None, brief: None }
    }

    pub fn with_reason(mut self, reason: &str) -> Failure {
        self.reason = Some(reason.into());
        self
    }

    pub fn with_brief(mut self, brief: impl Into<String>) -> Failure {
        self.brief = Some(brief.into());
        self
    }

    /// 给命令行、插件、MCP 的 JSON：{code, message, raw, action, next, site?}
    pub fn to_json(&self) -> Value {
        let mut v = json!({"code": self.code, "message": self.message, "raw": self.raw, "action": self.code.action(), "next": self.code.next_step()});
        if let Some(t) = self.starts_at {
            v["startsAt"] = json!(t);
        }
        if let Some(s) = &self.site {
            v["site"] = json!(s);
        }
        if let Some(r) = &self.reason {
            v["reason"] = json!(r);
        }
        if let Some(b) = &self.brief {
            v["brief"] = json!(b);
        }
        v
    }
}

/// 带固定代码的错误（不是下载失败那一套 Failure）：插件、界面要按代码判断的情况，比如 `NOT_RECORDING`（这个任务不在录制，停止并保存不了）。
/// 代码不随界面语言变，message 是给人看的一句话、跟着界面语言。经本机控制接口（api.rs）和插件宿主转发时代码不会丢
#[derive(Debug, Clone)]
pub struct Coded {
    pub code: String,
    pub message: String,
}

impl fmt::Display for Coded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Coded {}

pub fn coded(code: &str, message: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(Coded { code: code.into(), message: message.into() })
}

/// 错误链里 Coded 的代码
pub fn code_in(e: &anyhow::Error) -> Option<&str> {
    e.chain().find_map(|c| c.downcast_ref::<Coded>()).map(|c| c.code.as_str())
}

impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Failure {}

/// AI 工具的错误：保留原来的大写代码（DOWNLOAD_FAILED…），同时带上分类。
/// 显示成「DOWNLOAD_FAILED: 这个视频要登录 YouTube 才能看」，`agent::split_error` 照旧能拆。
#[derive(Debug, Clone)]
pub struct Tagged {
    pub tag: String,
    pub failure: Failure,
}

impl fmt::Display for Tagged {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.tag, self.failure.message)
    }
}

impl std::error::Error for Tagged {}

/// 在 anyhow 的错误链里找分类结果（Failure 或 Tagged）。
pub fn failure_in(e: &anyhow::Error) -> Option<&Failure> {
    e.chain().find_map(|c| c.downcast_ref::<Failure>().or_else(|| c.downcast_ref::<Tagged>().map(|t| &t.failure)))
}

/// 自己判断出错误码时（抖音从网页读取）：说法和 `classify` 认出这个错误码时一样（按网站），原文用 `raw`
pub fn by_code(code: ErrorCode, url: &str, raw: impl Into<String>) -> Failure {
    let raw = raw.into();
    let (message, brief) = msg(code, url, &raw.to_lowercase());
    let mut f = Failure::new(code, message, raw);
    f.brief = brief;
    f.site = site_name(url).map(str::to_string);
    f
}

/// 任意错误 → 分类。错误链里已经有 Failure 就直接用；再看 io / reqwest 错误；最后按文字归类。
pub fn from_anyhow(e: &anyhow::Error, url: &str) -> Failure {
    if let Some(f) = failure_in(e) {
        return f.clone();
    }
    // 找不到外部工具（tools.rs）：按类型认。它的说法跟着界面语言，不能再按「没有找到 yt-dlp」这几个字归类。
    // 说法和以前按文字归类时一样：下载用的几个组件说出名字，python（语音转文字）只说组件出了问题
    if let Some(m) = e.chain().find_map(|c| c.downcast_ref::<crate::tools::Missing>()) {
        let message = if ["yt-dlp", "ffmpeg", "ffprobe", "aria2c", "deno"].contains(&m.name.as_str()) {
            tr!("下载组件出了问题（没有找到 {name}）", name = m.name)
        } else {
            tr!("下载组件出了问题")
        };
        let mut f = Failure::new(ErrorCode::Tool, message, format!("{e:#}"));
        f.site = site_name(url).map(str::to_string);
        return f;
    }
    for c in e.chain() {
        if let Some(io) = c.downcast_ref::<std::io::Error>() {
            if let Some(f) = from_io(io) {
                return f;
            }
        }
        if let Some(r) = c.downcast_ref::<reqwest::Error>() {
            return from_reqwest(r, url);
        }
    }
    classify(&format!("{e:#}"), url)
}

// ------------------------------------------------------------------ 站点名

/// 链接 → 界面上的站点名（「要登录 YouTube」）。认不出来时 None。
/// 返回中文原文（存进 `fail.site`，界面显示时自己翻译）；拼进说法时用 `i18n::dynamic` 换成当前语言的名字
pub fn site_name(url: &str) -> Option<&'static str> {
    let host = url::Url::parse(url.trim()).ok()?.host_str()?.trim_start_matches("www.").to_ascii_lowercase();
    const SITES: &[(&str, &str)] = &[
        ("youtube.com", "YouTube"),
        ("youtu.be", "YouTube"),
        // 和登录窗口（login.rs 的 SITES）、素材库的来源、界面上的名字一样叫「哔哩哔哩」
        ("bilibili.com", tr_key!("哔哩哔哩")),
        ("b23.tv", tr_key!("哔哩哔哩")),
        ("x.com", "X"),
        ("twitter.com", "X"),
        ("instagram.com", "Instagram"),
        ("tiktok.com", "TikTok"),
        ("douyin.com", tr_key!("抖音")),
        ("iesdouyin.com", tr_key!("抖音")),
        ("vimeo.com", "Vimeo"),
        ("facebook.com", "Facebook"),
        ("fb.watch", "Facebook"),
        ("twitch.tv", "Twitch"),
        ("weibo.com", tr_key!("微博")),
        ("weibo.cn", tr_key!("微博")),
        ("xiaohongshu.com", tr_key!("小红书")),
        ("xhslink.com", tr_key!("小红书")),
        ("kuaishou.com", tr_key!("快手")),
        ("reddit.com", "Reddit"),
        ("soundcloud.com", "SoundCloud"),
        ("dailymotion.com", "Dailymotion"),
        ("threads.net", "Threads"),
        ("threads.com", "Threads"),
        ("weixin.qq.com", tr_key!("微信")),
        ("v.qq.com", tr_key!("腾讯视频")),
        ("iqiyi.com", tr_key!("爱奇艺")),
        ("youku.com", tr_key!("优酷")),
        ("nicovideo.jp", "niconico"),
        ("ixigua.com", tr_key!("西瓜视频")),
        ("zhihu.com", tr_key!("知乎")),
        ("acfun.cn", "AcFun"),
        ("rednote.com", tr_key!("小红书")),
        ("mgtv.com", tr_key!("芒果TV")),
        ("music.163.com", tr_key!("网易云音乐")),
        ("ximalaya.com", tr_key!("喜马拉雅")),
        ("patreon.com", "Patreon"),
        ("flickr.com", "Flickr"),
        ("pixiv.net", "pixiv"),
        ("9gag.com", "9GAG"),
        ("imgur.com", "Imgur"),
        ("lemon8-app.com", "Lemon8"),
    ];
    SITES.iter().find(|(d, _)| host == *d || host.ends_with(&format!(".{d}"))).map(|(_, n)| *n)
}

/// 微信视频号：yt-dlp 不支持，视频还加密，这一轮只给「暂不支持」。
pub fn is_wechat_channels(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url.trim()) else { return false };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    (host == "weixin.qq.com" && u.path().starts_with("/sph/")) || host == "channels.weixin.qq.com" || host.ends_with(".channels.weixin.qq.com")
}

pub fn wechat_channels(url: &str) -> Failure {
    Failure::new(ErrorCode::Unsupported, tr!("微信视频号暂不支持"), format!("Unsupported URL: {url}"))
}

/// 图片帖（小红书图文、抖音和 TikTok 图集、Instagram 多图、9GAG、Flickr、pixiv……）：Captae 只下视频和音频，图片交给插件。
/// 错误码用 unsupported，界面上的按钮是「在浏览器里打开」（action use_extension）。
/// 这是中文原文，给人看时用 `i18n::dynamic(IMAGE_POST)`（`image_post` 已经换好）；判断是不是图片帖看 `reason == "image_post"`
pub const IMAGE_POST: &str = tr_key!("这是图片帖，用浏览器插件在网页里下图片");

/// 图片帖的失败：说法跟着界面语言，细分是 `image_post`，插件里显示去掉插件提示的「这是图片帖」
pub fn image_post(raw: impl Into<String>) -> Failure {
    let d = image_post_detail();
    let mut f = Failure::new(d.code, d.message.unwrap_or_default(), raw);
    f.reason = d.reason.map(str::to_string);
    f.brief = d.brief;
    f
}

fn image_post_detail() -> Detail {
    Detail::says(ErrorCode::Unsupported, dynamic(IMAGE_POST)).reason("image_post").brief(tr!("这是图片帖"))
}

/// 电台的直播流（会一直播下去的音频流，没有长度）：错误码 live，细分 `radio`，界面据此提示「电台不会自己停」
pub fn radio(raw: impl Into<String>) -> Failure {
    Failure::new(ErrorCode::Live, tr!("这是电台的直播流，会一直播下去"), raw).with_reason("radio")
}

/// 「Captae 还不认识这个网站」：说法和去掉插件提示的短说法
fn unknown_site() -> (String, String) {
    (tr!("Captae 还不认识这个网站，可以用浏览器插件在网页里下"), tr!("Captae 还不认识这个网站"))
}

/// 从链接看得出是图片帖的网站和路径（yt-dlp 对它们说 Unsupported URL，或者干脆没有这个网站）。
pub fn is_image_post(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url.trim()) else { return false };
    let host = u.host_str().unwrap_or("").trim_start_matches("www.").to_ascii_lowercase();
    let path = u.path().to_ascii_lowercase();
    let on = |d: &str| host == d || host.ends_with(&format!(".{d}"));
    (on("douyin.com") && path.starts_with("/note/"))
        || (on("tiktok.com") && path.contains("/photo/"))
        || (on("pixiv.net") && (path.starts_with("/artworks/") || path.contains("/artworks/")))
        || (on("flickr.com") && path.starts_with("/photos/"))
        || (on("imgur.com") && (path.starts_with("/gallery/") || path.starts_with("/a/")))
        || (on("500px.com") && path.starts_with("/photo/"))
        || (on("artstation.com") && path.starts_with("/artwork/"))
        || (on("deviantart.com") && path.contains("/art/"))
        || (on("dribbble.com") && path.starts_with("/shots/"))
        || (on("pexels.com") && path.starts_with("/photo/"))
        || on("lemon8-app.com")
}

/// 「下载引擎（yt-dlp）跟不上网站」：yt-dlp 有这个网站的提取器，但网站改版以后读不出来了（第三阶段 FIXES 第 1 项）。
/// Captae 这边修不了，说清楚原因、交给插件（unsupported，按钮「在浏览器里打开」）；yt-dlp 更新以后自然就好了。
/// 返回说法（带去掉插件提示的短说法）；不是这一类时 None。`e` 是 ERROR 那几行（小写），`all` 是全文（小写）。
fn engine_lag(e: &str, all: &str, st: Option<u16>) -> Option<Detail> {
    let lag = |m: String, b: String| Some(Detail::says(ErrorCode::Unsupported, m).brief(b));
    let base = || lag(tr!("下载引擎暂时读不了这个网站（可能是网站改版了），可以用浏览器插件在网页里下"), tr!("下载引擎暂时读不了这个网站（可能是网站改版了）"));
    // yt-dlp 自己标成失效的提取器（Pluto TV、Telemundo、NDTV）：后面的 404 / 403 都不可信
    if all.contains("has been marked as broken") {
        return base();
    }
    // 专门提取器的接口 404：视频不存在和接口改了是同一句话（台湾 Yahoo）
    if e.contains("unable to download json metadata: http error 404") {
        return lag(
            tr!("下载引擎暂时读不了这个网站（可能是网站改版了，也可能视频已经删除），可以用浏览器插件在网页里下"),
            tr!("下载引擎暂时读不了这个网站（可能是网站改版了，也可能视频已经删除）"),
        );
    }
    // 提取器向网站接口发的请求被拒（400 一类）：链接失效、要登录、网站改版都会这样，单凭状态码分不清（Khan Academy、Medal、DR TV、Fox News）
    if let Some(s) = st.filter(|s| (400..500).contains(s) && !matches!(s, 401 | 403 | 404 | 410 | 418 | 429 | 451)) {
        if e.contains("unable to download") || e.contains("http error") {
            return lag(
                tr!("网站不接受下载引擎的请求（HTTP {status}）：链接可能失效了、要登录，或者网站改版了。可以用浏览器插件在网页里下", status = s),
                tr!("网站不接受下载引擎的请求（HTTP {status}）：链接可能失效了、要登录，或者网站改版了", status = s),
            );
        }
    }
    const LAG: &[&str] = &[
        "an extractor error has occurred",
        "unable to extract",
        "unable to find embedded",
        "can't find any video",
        "cannot parse data",
        "server reported error",
        "phantomjs not found",
        "response data has no m3u header",
        "list index out of range",
        "keyerror(",
        "'nonetype' object",
        "object has no attribute",
    ];
    if has(e, LAG) {
        return base();
    }
    // 提取器一路警告「unable to extract …」，最后说没有格式（美拍）
    if e.contains("no video formats found") && all.lines().any(|l| l.contains("warning:") && l.contains("unable to extract")) {
        return base();
    }
    None
}

/// 这次失败是不是「下载引擎跟不上网站」（自动更新的体检照样把它算作引擎出错；读取时照旧自动重试一次）
pub fn is_engine_lag(f: &Failure) -> bool {
    if f.code != ErrorCode::Unsupported {
        return false;
    }
    let raw = strip_ansi(&f.raw).to_string();
    let st = http_status_in(&focus(&raw));
    engine_lag(&focus(&raw), &raw.to_lowercase(), st).is_some()
}

/// 「Unsupported URL: <地址>」里的地址：yt-dlp 跟着跳转走完以后落到的那一页
fn landed_url(text: &str) -> Option<String> {
    let i = text.find("Unsupported URL: ")?;
    let rest = &text[i + "Unsupported URL: ".len()..];
    let u = rest.split_whitespace().next()?;
    url::Url::parse(u).ok().map(|_| u.to_string())
}

/// 这些网站 yt-dlp 的支持是完整的：它们的网页报 Unsupported URL，就是这一页没有视频（YouTube 首页、个人主页……）
fn full_support(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url.trim()) else { return false };
    let host = u.host_str().unwrap_or("").trim_start_matches("www.").to_ascii_lowercase();
    ["youtube.com", "youtu.be", "bilibili.com", "x.com", "twitter.com", "instagram.com", "vimeo.com", "facebook.com", "soundcloud.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

/// Unsupported URL 的几种情况（FIXES 第 2、3 项）：落到登录页、404 页、网站首页，图片帖，yt-dlp 没有这个网站的提取器，
/// 认识的网站这一页没有视频。
fn unsupported_url(text: &str, all: &str, url: &str) -> Detail {
    use ErrorCode::*;
    if is_wechat_channels(url) {
        return Detail::says(Unsupported, tr!("微信视频号暂不支持"));
    }
    let landed = landed_url(text).unwrap_or_else(|| url.to_string());
    let lu = url::Url::parse(&landed).ok();
    let lpath = lu.as_ref().map(|u| u.path().to_ascii_lowercase()).unwrap_or_default();
    let site = site_name(url).or_else(|| site_name(&landed));
    // 没登录时跳到了登录页（小红书短链 → rednote.com/login）。登录以后要回去的只是网站首页（redirectPath=/explore）时，
    // 链接本身没指向哪条笔记：分享链接已经失效了（xiaohongshu-a3612b），登录了也看不到
    if ["/login", "/signin", "/sign_in", "/sign-in", "/passport", "/account/login"].iter().any(|p| lpath.starts_with(p)) {
        let back = lu.as_ref().and_then(|u| {
            u.query_pairs().find(|(k, _)| ["redirectpath", "redirect", "redirect_uri", "redirecturl", "next", "returnurl", "return_to"].contains(&k.to_ascii_lowercase().as_str())).map(|(_, v)| v.to_string())
        });
        let home = back.as_deref().and_then(|b| url::Url::parse(b).ok().map(|u| u.path().to_string()).or_else(|| Some(b.split('?').next().unwrap_or("").to_string()))).is_some_and(|p| {
            matches!(p.trim_end_matches('/').to_ascii_lowercase().as_str(), "" | "/explore" | "/home" | "/discovery" | "/index")
        });
        if home {
            return Detail::says(Unavailable, tr!("这个分享链接已经失效了（打开以后只跳到了网站首页）"));
        }
        return Detail::says(
            NeedLogin,
            match site {
                Some(s) => tr!("这个视频要登录{site}才能看", site = dynamic(s)),
                None => tr!("这个视频要登录账号才能看"),
            },
        );
    }
    // 跳到了网站的 404 页
    if lpath.starts_with("/404") {
        return Detail::says(Unavailable, tr!("视频已被删除或不存在"));
    }
    if is_image_post(url) || is_image_post(&landed) {
        return image_post_detail();
    }
    // 短链接跳到了网站首页：分享链接已经失效了（抖音的旧短链）
    let host_of = |s: &str| url::Url::parse(s).ok().and_then(|u| u.host_str().map(str::to_string));
    if landed != url && matches!(lpath.as_str(), "" | "/") && host_of(&landed) != host_of(url) {
        return Detail::says(Unavailable, tr!("这个分享链接已经失效了（打开以后跳到了网站首页）"));
    }
    // yt-dlp 没有这个地址的提取器，交给了通用提取器：是 Captae（下载引擎）不认识，不是网页里没有视频
    let generic = all.contains("falling back on generic information extractor");
    if site.is_some() && !is_wechat_channels(url) && (!generic || full_support(url)) {
        return Detail::plain(NoMedia);
    }
    match site {
        Some(s) => {
            let name = dynamic(s);
            Detail::says(Unsupported, tr!("下载引擎还不支持这种{site}链接，可以用浏览器插件在网页里下", site = name)).brief(tr!("下载引擎还不支持这种{site}链接", site = name))
        }
        None => {
            let (m, b) = unknown_site();
            Detail::says(Unsupported, m).brief(b)
        }
    }
}

/// NRK 的「Kommer i morgen kl. 18:00」（明天 18:00 上线）→「这一集还没上线（明天 18:00）」
fn nrk_soon(e: &str) -> String {
    static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"kommer (i dag|i morgen)? ?kl\.? ?(\d{1,2}[:.]\d{2})").unwrap());
    match RE.captures(e) {
        Some(c) => {
            let time = c[2].replace('.', ":");
            match c.get(1).map(|m| m.as_str()) {
                Some("i dag") => tr!("这一集还没上线（挪威时间今天 {time}）", time = time),
                Some("i morgen") => tr!("这一集还没上线（挪威时间明天 {time}）", time = time),
                _ => tr!("这一集还没上线（挪威时间{time}）", time = time),
            }
        }
        None => tr!("这一集还没上线"),
    }
}

/// 地区限制的各种说法（含各国电视台自己的话）
fn is_geo(e: &str) -> bool {
    if has(
        e,
        &[
            "not available from your location",
            "available in your country",
            "available in your region",
            "not available in your area",
            "geo restriction",
            "geo-restriction",
            "geo restricted",
            "geo-restricted",
            "geoblock",
            "geo blocked",
            "geolocation",
            // yt-dlp 的 GeoRestrictedError 第二行（B 站那句是 try a vpn，已经算在 geo-restricted 里）
            "use a vpn or a proxy server",
            "utenfor norge",
            // TVer：REQUEST_FAILED: 163（この動画の視聴は許可されていません）
            "request_failed: 163",
        ],
    ) {
        return true;
    }
    // 「This video is only available in Sweden」「only available in Switzerland」；「only available in the app」不算
    e.match_indices("only available in ").any(|(i, k)| !e[i + k.len()..].starts_with("the app") && !e[i + k.len()..].starts_with("app"))
}

// ------------------------------------------------------------------ 文案

/// 归类的结果：错误码，加上（需要时）比 `msg` 更具体的说法、细分 `reason` 和去掉插件提示的短说法 `brief`（见 `Failure` 的同名字段）
struct Detail {
    code: ErrorCode,
    message: Option<String>,
    reason: Option<&'static str>,
    brief: Option<String>,
}

impl Detail {
    /// 只有错误码，说法按 `msg`
    fn plain(code: ErrorCode) -> Detail {
        Detail { code, message: None, reason: None, brief: None }
    }

    fn says(code: ErrorCode, message: String) -> Detail {
        Detail { code, message: Some(message), reason: None, brief: None }
    }

    fn reason(mut self, reason: &'static str) -> Detail {
        self.reason = Some(reason);
        self
    }

    /// 说法结尾带着「可以用浏览器插件在网页里下」这类提示时，去掉提示的那句
    fn brief(mut self, brief: String) -> Detail {
        self.brief = Some(brief);
        self
    }
}

/// 按错误码的一般说法。第二项：说法带插件提示时，去掉提示的短说法（见 `Failure::brief`）
fn msg(code: ErrorCode, url: &str, lower: &str) -> (String, Option<String>) {
    if code == ErrorCode::Unsupported {
        let (m, b) = unknown_site();
        return (m, Some(b));
    }
    (msg_text(code, url, lower), None)
}

fn msg_text(code: ErrorCode, url: &str, lower: &str) -> String {
    let site = site_name(url);
    // 拼进说法里的名字按当前语言（「哔哩哔哩」→「Bilibili」）
    let s = site.map(dynamic).unwrap_or_default();
    match code {
        ErrorCode::NeedLogin => {
            if lower.contains("confirm your age") || lower.contains("age-restricted") || lower.contains("age restricted") || lower.contains("inappropriate for some users") {
                if s.is_empty() {
                    tr!("这个视频有年龄限制，要登录账号才能看")
                } else {
                    tr!("这个视频有年龄限制，要登录{site}才能看", site = s)
                }
            } else if lower.contains("private video") || lower.contains("video is private") || lower.contains("set to private") {
                tr!("这是私密视频，要登录有权限的账号才能看")
            } else if lower.contains("members-only") || lower.contains("join this channel") || lower.contains("members only") {
                tr!("这是会员专属视频，要登录开通了会员的账号")
            } else if ["--video-password", "video password", "protected by a password", "password protected", "password-protected"].iter().any(|k| lower.contains(k)) {
                // 只认视频密码；「--username and --password」是 yt-dlp 提示怎么登录，不是视频有密码
                tr!("这个视频设置了密码")
            } else if s.is_empty() {
                tr!("这个视频要登录才能看")
            } else {
                tr!("这个视频要登录{site}才能看", site = s)
            }
        }
        // 按站点名的中文原文认（site_name 返回的是原文，不跟语言变）
        ErrorCode::NeedVerify => match site {
            Some("YouTube") => tr!("YouTube 要确认一下你不是机器人"),
            Some("抖音") => tr!("抖音要先在网页里打开一次（需要新的 cookies）"),
            Some(_) => tr!("{site}要先在网页里确认一下（人机验证）", site = s),
            None => tr!("网站要先在网页里确认一下（人机验证）"),
        },
        ErrorCode::Drm => tr!("这个视频受版权保护（DRM），Captae 不能下载"),
        ErrorCode::Geo => {
            if lower.contains("may be deleted") {
                // B 站等网站分不清：「可能删了，也可能有地区限制」
                tr!("这个视频可能已被删除，或者在你所在的地区看不了")
            } else {
                tr!("这个视频在你所在的地区看不了")
            }
        }
        ErrorCode::Blocked => tr!("连接被你的网络拦截了，检查代理设置"),
        ErrorCode::Unavailable => {
            if lower.contains("recording is not available") || lower.contains("downloadable vod is not available") {
                tr!("这场直播没有留下录像，或者录像已被删除")
            } else {
                tr!("视频已被删除或不存在")
            }
        }
        ErrorCode::Unsupported => unknown_site().0,
        ErrorCode::NoMedia => tr!("这个网页里没有找到视频或音频"),
        ErrorCode::LiveUpcoming => live_upcoming_text(lower),
        ErrorCode::Live => tr!("这是正在进行的直播"),
        ErrorCode::RateLimited => tr!("网站让我们慢一点，稍后自动重试"),
        ErrorCode::Network => {
            if let Some(st) = http_status_in(lower).filter(|c| (500..600).contains(c)) {
                tr!("对方网站暂时出了问题（HTTP {status}），稍后再试", status = st)
            } else if lower.contains("超时") || lower.contains("timed out") || lower.contains("timeout") {
                tr!("网络不稳定，连接超时了")
            } else {
                tr!("网络不稳定，没连上对方网站")
            }
        }
        ErrorCode::Disk => {
            if lower.contains("no space") || lower.contains("errno 28") || lower.contains("disk space") || lower.contains("磁盘满") || lower.contains("errorcode=9 ") {
                tr!("保存位置的磁盘满了")
            } else if lower.contains("permission denied") || lower.contains("operation not permitted") || lower.contains("read-only") || lower.contains("没有权限") {
                tr!("没有权限写入保存位置")
            } else {
                tr!("保存位置没有空间或没有权限")
            }
        }
        ErrorCode::Tool => {
            if lower.contains("no supported javascript runtime") || lower.contains("javascript runtime could be found") {
                tr!("下载组件出了问题（缺少 JS 运行时）")
            } else if lower.contains("challenge solving failed") {
                // 有 JS 运行时，但解不了 YouTube 这一版的验证：多半是下载引擎太旧
                tr!("下载引擎解不了 YouTube 的验证，可能要更新下载引擎")
            } else if lower.contains("no such option") {
                tr!("下载组件版本太旧，请更新 yt-dlp 或清空设置里指定的路径")
            } else if let Some(n) = ["yt-dlp", "ffmpeg", "ffprobe", "aria2c", "deno"].iter().find(|n| lower.contains(&format!("没有找到 {n}"))) {
                // 旧版记下的中文报错（现在找不到工具时按类型认，见 from_anyhow）
                tr!("下载组件出了问题（没有找到 {name}）", name = n)
            } else {
                tr!("下载组件出了问题")
            }
        }
        ErrorCode::Unknown => last_line(lower),
    }
}

/// 「This live event will begin in 3 hours.」→「直播大约 3 小时后开始」
fn live_upcoming_text(lower: &str) -> String {
    let premiere = lower.contains("premiere");
    if lower.contains("a few moments") {
        return if premiere { tr!("首映马上开始") } else { tr!("直播马上开始") };
    }
    static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"in (\d+) (minute|hour|day|week)s?").unwrap());
    if let Some(c) = RE.captures(lower) {
        let n = &c[1];
        return match (premiere, &c[2]) {
            (false, "minute") => tr!("直播大约 {n} 分钟后开始", n = n),
            (false, "hour") => tr!("直播大约 {n} 小时后开始", n = n),
            (false, "day") => tr!("直播大约 {n} 天后开始", n = n),
            (false, _) => tr!("直播大约 {n} 周后开始", n = n),
            (true, "minute") => tr!("首映大约 {n} 分钟后开始", n = n),
            (true, "hour") => tr!("首映大约 {n} 小时后开始", n = n),
            (true, "day") => tr!("首映大约 {n} 天后开始", n = n),
            (true, _) => tr!("首映大约 {n} 周后开始", n = n),
        };
    }
    if premiere {
        tr!("首映还没开始")
    } else {
        tr!("直播还没开始")
    }
}

/// 「直播 20:00 开始」：知道开始时间（yt-dlp 的 release_timestamp）时用。
pub fn live_at_text(starts_at_ms: i64) -> String {
    let off = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let Ok(t) = time::OffsetDateTime::from_unix_timestamp(starts_at_ms / 1000) else { return tr!("直播还没开始") };
    let t = t.to_offset(off);
    let now = time::OffsetDateTime::now_utc().to_offset(off);
    let hm = format!("{:02}:{:02}", t.hour(), t.minute());
    if t.date() == now.date() {
        tr!("直播 {time} 开始", time = hm)
    } else {
        tr!("直播 {date} {time} 开始", date = month_day(t.month() as u8, t.day()), time = hm)
    }
}

/// 「11月25日」这样的月、日。中文、日文照原来的写法：数字紧挨着「月」「日」，走 `tr!` 的话自动空格会在中间加上空格；
/// 其他语言按词表里的格式（英文「11/25」）
pub(crate) fn month_day(month: u8, day: u8) -> String {
    use crate::i18n::Lang;
    match crate::i18n::current() {
        Lang::ZhCn | Lang::ZhTw | Lang::Ja => format!("{month}月{day}日"),
        _ => tr!("{month}月{day}日§日期：月、日都是数字", month = month, day = day),
    }
}

/// 原文的最后一行（去掉 ERROR: 和「[extractor] id: 」前缀、去掉「请报告问题」的尾巴）。
fn last_line(text: &str) -> String {
    let line = text.lines().rev().map(str::trim).find(|l| l.contains("error")).or_else(|| text.lines().rev().map(str::trim).find(|l| !l.is_empty())).unwrap_or("");
    clean_line(line)
}

fn clean_line(line: &str) -> String {
    let mut l = line.trim();
    for p in ["ERROR:", "error:", "Error:"] {
        if let Some(r) = l.strip_prefix(p) {
            l = r.trim();
        }
    }
    // [youtube] abc123: 真正的原因
    if l.starts_with('[') {
        if let Some(end) = l.find("] ") {
            let rest = &l[end + 2..];
            l = match rest.split_once(": ") {
                Some((id, r)) if !id.contains(' ') && id.len() < 64 => r,
                _ => rest,
            };
        }
    }
    let l = l.split("; please report this issue").next().unwrap_or(l);
    let l = l.split(" please report this issue").next().unwrap_or(l).trim_end_matches([';', ',', ' ']);
    let s: String = l.chars().take(200).collect();
    if s.is_empty() { tr!("下载失败") } else { s }
}

/// 去掉终端颜色控制符（deno 的报错带颜色）。
pub fn strip_ansi(s: &str) -> std::borrow::Cow<'_, str> {
    static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"\x1b\[[0-9;?]*[A-Za-z]").unwrap());
    RE.replace_all(s, "")
}

/// 原文只留最后 12 行、4000 字，免得任务记录太大。
fn tail(raw: &str) -> String {
    let raw = strip_ansi(raw);
    let lines: Vec<&str> = raw.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect();
    let start = lines.len().saturating_sub(12);
    let s = lines[start..].join("\n");
    if s.chars().count() > 4000 {
        s.chars().rev().take(4000).collect::<Vec<_>>().into_iter().rev().collect()
    } else {
        s
    }
}

/// 文字里的 HTTP 状态码：「HTTP Error 403」「status=404」「返回 404」「HTTP 503」「Server returned 403」
fn http_status_in(lower: &str) -> Option<u16> {
    static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"(?:http error |status[= ]|status code |返回 |http |server returned |response status: )(\d{3})\b").unwrap()
    });
    RE.captures(lower).and_then(|c| c[1].parse().ok())
}

// ------------------------------------------------------------------ 归类

fn has(t: &str, keys: &[&str]) -> bool {
    keys.iter().any(|k| t.contains(k))
}

/// 主要看 ERROR 那几行（yt-dlp 的 WARNING 里也会提 cookies 之类，容易误判）；没有 ERROR 行就看全文。
/// ERROR 行后面紧跟的续行（不带 WARNING、不是「[提取器]」开头）也算：yt-dlp 的地区限制报错是多行的，
/// 第二、三行「This video is available in Sweden.」「You might want to use a VPN …」才是最稳的特征。
fn focus(text: &str) -> String {
    let mut err: Vec<&str> = vec![];
    let mut cont = false;
    for l in text.lines() {
        if l.contains("ERROR") || l.contains("errorCode=") || l.contains("Exception:") {
            err.push(l);
            cont = l.contains("ERROR");
        } else if cont && !l.trim().is_empty() && !l.contains("WARNING") && !l.trim_start().starts_with('[') {
            err.push(l);
        } else {
            cont = false;
        }
    }
    let t = if err.is_empty() { text.to_string() } else { err.join("\n") };
    t.to_lowercase().replace(['\u{2019}', '\u{2018}'], "'")
}

/// 按文字归类（yt-dlp、aria2、ffmpeg、Captae 自己的报错都走这里）。`url` 用来给出站点名。
pub fn classify(text: &str, url: &str) -> Failure {
    let text = &*strip_ansi(text);
    let d = classify_detail(text, url);
    let code = d.code;
    // 组件问题的线索常在 WARNING 行里（缺 JS 运行时），看全文；其他只看 ERROR 行
    let lower = if code == ErrorCode::Tool { text.to_lowercase() } else { focus(text) };
    let (message, brief) = match d.message {
        Some(m) => (m, d.brief),
        None if code == ErrorCode::Unknown => (last_line_orig(text), None),
        None => msg(code, url, &lower),
    };
    let mut f = Failure::new(code, message, text);
    f.reason = d.reason.map(str::to_string);
    f.brief = brief;
    // 和说法里用的是同一个名字：链接认不出时看跳转到了哪个网站（unsupported_url 里也这样找）
    f.site = site_name(url).or_else(|| landed_url(text).as_deref().and_then(site_name)).map(str::to_string);
    f
}

/// unknown 时给原文（保留大小写）的最后一行。
fn last_line_orig(text: &str) -> String {
    let line = text
        .lines()
        .rev()
        .map(str::trim)
        .find(|l| l.contains("ERROR") || l.contains("Error") || l.contains("error"))
        .or_else(|| text.lines().rev().map(str::trim).find(|l| !l.is_empty()))
        .unwrap_or("");
    clean_line(line)
}

pub fn classify_code(text: &str, url: &str) -> ErrorCode {
    classify_detail(&strip_ansi(text), url).code
}

/// 证书类的失败（小写后找）：Python ssl、curl、rustls、Apple Security 的说法
const CERT_ERRORS: &[&str] = &[
    "certificate verify failed",
    "certificate_verify_failed",
    "hostname mismatch",
    "certificate is not valid for",
    "no alternative certificate subject name",
    "self signed certificate",
    "self-signed certificate",
    "unable to get local issuer certificate",
    "ssl certificate problem",
    "certificate has expired",
    "certificate has been revoked",
    "certificate revoked",
    "certificate is not trusted",
    "invalid peer certificate",
    "notvalidforname",
    "unknownissuer",
    "invalidcertificate",
];

/// 证书有问题：错误码 blocked（桌面端、插件、AI 助手都靠那 15 个码，不新加），reason = cert，按钮照旧 retry
fn cert_detail() -> Detail {
    Detail::says(ErrorCode::Blocked, tr!("这个网站的证书有问题（过期、被吊销，或者网络在中途拦截），为了安全没有连接")).reason("cert").brief(tr!("这个网站的证书有问题"))
}

/// 归类，再加上（需要时）比 `msg` 更具体的一句话、细分（reason）和去掉插件提示的短说法（brief）。
fn classify_detail(text: &str, url: &str) -> Detail {
    let all = text.to_lowercase().replace(['\u{2019}', '\u{2018}'], "'");
    let e = focus(text);
    use ErrorCode::*;
    let plain = Detail::plain;

    // 1. 组件本身的问题：找不到、启动不了、崩溃、参数不认识
    if has(&e, &["没有找到 yt-dlp", "没有找到 ffmpeg", "没有找到 ffprobe", "没有找到 aria2c", "没有找到 python", "启动外部工具失败", "启动 yt-dlp 失败", "启动 aria2 失败", "启动 ffmpeg 失败", "返回的信息无法解析"])
        || has(&all, &["traceback (most recent call last)", "[pyi-", "failed to load python shared library", "modulenotfounderror", "importerror:", "bad cpu type", "exec format error", "no such option:", "killed: 9"])
        || has(&e, &["ffprobe and ffmpeg not found", "ffmpeg not found", "ffprobe not found", "ffmpeg is not installed", "--ffmpeg-location", "unknown encoder", "encoder not found", "errorcode=28 "])
    {
        return plain(Tool);
    }
    // 缺 JS 运行时：YouTube 报「This video is not available」或「Requested format is not available」，警告里有提示
    if has(&all, &["no supported javascript runtime", "javascript runtime could be found", "challenge solving failed"]) && has(&e, &["this video is not available", "requested format is not available", "no video formats found"]) {
        return plain(Tool);
    }
    // 2. 直播
    if has(
        &e,
        &[
            "live event will begin",
            "premieres in",
            "premiere will begin",
            "is not currently live",
            "stream has not started",
            "live event has not started",
            "waiting for scheduled stream",
            // CBC Gem
            "has not yet started",
            "channel offline",
            // Dailymotion 按出口地区翻成了印尼语
            "saluran offline",
        ],
    ) {
        return plain(LiveUpcoming);
    }
    // NRK：「Kommer i morgen kl. 18:00」（还没上线）
    if e.contains("nrk said: kommer") {
        return Detail::says(LiveUpcoming, nrk_soon(&e));
    }
    if has(&e, &["正在进行的直播", "is currently live", "this is a live stream"]) {
        return plain(Live);
    }
    // 3. DRM
    if has(&e, &["drm protected", "drm protection", "known to use drm", "widevine", "fairplay", "playready", "可能是 drm", "sample-aes"]) {
        return plain(Drm);
    }
    // 4. 人机验证（要在「登录」前面：YouTube 两种都写着 Sign in）
    if has(
        &e,
        &[
            "not a bot",
            "captcha",
            "cloudflare anti-bot",
            "anti-bot challenge",
            "fresh cookies",
            // 西瓜视频：Cookies (not necessarily logged in) are needed
            "not necessarily logged in",
            "verify you are human",
            "unusual traffic",
            "are you a robot",
            "bot detection",
            // Bandcamp：Got client challenge webpage response
            "client challenge",
        ],
    ) {
        return plain(NeedVerify);
    }
    // 5. 地区限制
    let st = http_status_in(&e);
    // TikTok 状态码 10204，yt-dlp 说成「IP 被封」；同一个网络下别的视频照样能下（tiktok-6436ff），多半是这条没了或者这里看不了
    if e.contains("ip address is blocked from accessing this post") {
        return Detail::says(
            Geo,
            match site_name(url) {
                Some(s) => tr!("这个视频可能已被删除，或者{site}在你现在的网络下不给看", site = dynamic(s)),
                None => tr!("这个视频可能已被删除，或者网站在你现在的网络下不给看"),
            },
        );
    }
    if is_geo(&e) || st == Some(451) {
        return plain(Geo);
    }
    // 6. 要登录：年龄、私密、会员、密码、401
    if has(
        &e,
        &[
            "confirm your age",
            "age-restricted",
            "age restricted",
            "inappropriate for some users",
            "private video",
            "video is private",
            "set to private",
            "members-only",
            "members only",
            "join this channel",
            "only available for registered users",
            "requires authentication",
            "login required",
            "login to",
            "log in to",
            "sign in",
            "account credentials",
            "use --cookies",
            "--cookies-from-browser",
            "--video-password",
            "password protected",
            "protected by a password",
            "premium members",
            "requires a subscription",
            "appropriate subscription",
            "authorization failed",
            "errorcode=24 ",
            "unauthorized",
            // Patreon：赞助者专享
            "you do not have access to this post",
            // Dailymotion 按出口地区翻成了印尼语：Konten pribadi（私密内容）
            "konten pribadi",
            "private content",
        ],
    ) || st == Some(401)
    {
        return plain(NeedLogin);
    }
    // 7. 请求太频繁
    if has(&e, &["too many requests", "rate-limit", "rate limit", "rate-limited", "ratelimit", "try again later", "slow down"]) || st == Some(429) {
        return plain(RateLimited);
    }
    // 8. 证书有问题（自签、被吊销、过期、主机名对不上、根证书不认；运营商劫持、公司网关换了证书也是这样）：错误码还是 blocked，细分 reason = cert
    if has(&e, CERT_ERRORS) {
        return cert_detail();
    }
    // 被网络拦截：TLS 握手失败、IP 被封
    if has(
        &e,
        &[
            "ssl/tls handshake failure",
            "tlsv1 alert",
            "wrong_version_number",
            "wrong version number",
            "ip address is blocked",
            "your ip is blocked",
            "access from your ip",
        ],
    ) {
        return plain(Blocked);
    }
    // 网站对当前出口整个拒绝（新浪对部分海外出口返回 418）：不是网络不稳
    if st == Some(418) {
        return Detail::says(Blocked, tr!("这个网站拒绝了你现在的网络（HTTP 418），换个网络或代理再试"));
    }
    // 9. 磁盘
    if has(
        &e,
        &[
            "no space left",
            "errno 28",
            "disk full",
            "not enough disk space",
            "errorcode=9 ",
            "read-only file system",
            "errno 30",
            "permission denied",
            "errno 13",
            "operation not permitted",
            "unable to open for writing",
            "unable to write data",
            "unable to create directory",
            "failed to make the directory",
            "could not create new file",
            "errorcode=13 ",
            "errorcode=14 ",
            "errorcode=15 ",
            "errorcode=16 ",
            "errorcode=17 ",
            "errorcode=18 ",
            "disk quota exceeded",
            "file name too long",
            "写不进",
            "磁盘满",
            "没有权限写入",
        ],
    ) {
        return plain(Disk);
    }
    // 10a. 下载引擎跟不上网站（要在「已删除」前面：PhantomJS not found 里有 not found，失效提取器后面的 404 不可信）
    // YouTube 读取时偶发的 400 照旧按网络问题算（自动重试一次、按钮是「重试」）：第一轮没见过它跟不上的情况
    let st_lag = if site_name(url) == Some("YouTube") { st.filter(|s| !(400..500).contains(s)) } else { st };
    if let Some(d) = engine_lag(&e, &all, st_lag) {
        return d;
    }
    // 10. 已删除、不存在
    if has(
        &e,
        &[
            "video unavailable",
            "has been removed",
            "no longer available",
            "does not exist",
            "doesn't exist",
            "has been deleted",
            "was deleted",
            "been terminated",
            "this video is not available",
            "this tweet is unavailable",
            "post is unavailable",
            "post isn't available",
            "content isn't available",
            "content is not available",
            "video is unavailable",
            "this video has been disabled",
            "recording is not available",
            // Weverse：直播结束了，没有可下载的回放
            "downloadable vod is not available",
            // Wistia：嵌入地址指向不存在的视频
            "error while getting the playlist: true",
            "incomplete youtube id",
            "looks truncated",
            "resource not found",
            "video not found",
            "page not found",
            "file not found",
            "errorcode=3 ",
            "errorcode=4 ",
            "文件不存在",
        ],
    ) || matches!(st, Some(404) | Some(410))
    {
        return plain(Unavailable);
    }
    // 11. 网站不认识 / 页面里没有媒体 / 图片帖
    if has(&e, &["unsupported url", "no suitable extractor", "is not a valid url"]) {
        return unsupported_url(text, &all, url);
    }
    // 网站先回了 429（写在 WARNING 里），后面才说没有格式：是请求太频繁，不是页面里没有视频（odysee-e5f804，对照组实测）
    if has(&e, &["no video formats found", "no formats found", "no video could be found"]) && has(&all, &["http error 429", "too many requests"]) {
        return plain(RateLimited);
    }
    // 图片帖：Flickr「not a video」、9GAG「does not contain a video」、Instagram 多图「There is no video in this post」、小红书图文「No video formats found」
    if has(&e, &["not a video", "does not contain a video", "there is no video in this post"])
        || (is_image_post(url) && has(&e, &["no video formats found", "no video could be found", "playlist has no entries", "没有视频"]))
        || (site_name(url) == Some("小红书") && e.contains("no video formats found"))
    {
        return image_post_detail();
    }
    // ffmpeg 读网址时遇到 HTTP 4xx / 5xx 退出码都是 8（AVERROR_HTTP_* 的低 8 位都是 0xF8）；yt-dlp 让它 quiet，看不到是哪个状态码。
    // 多半是请求太频繁、地区限制或者链接过期（britishpathe 是 429，ZDF 是按地区 403），不是「不是媒体」
    if e.contains("ffmpeg exited with code 8") {
        return Detail::says(Network, tr!("网站在下载时拒绝了请求（HTTP 错误），可能是请求太频繁、有地区限制或者链接过期了，稍后重试"));
    }
    // 下载下来、要转封装时才发现不是媒体（PDF、ZIP、网页……）
    if has(&e, &["error opening input files: invalid data", "invalid data found when processing input"]) || (all.contains("the video format is unknown and cannot be downloaded") && e.contains("ffmpeg exited")) {
        return Detail::says(NoMedia, tr!("下载到的不是视频或音频，可能是网页或别的文件"));
    }
    if has(
        &e,
        &[
            "no video formats found",
            "no video could be found",
            "there is no video",
            "there's no video",
            "does not contain any media",
            "does not contain a video",
            "no media found",
            "no formats found",
            "requested format is not available",
            "playlist does not have any entries",
            "没有找到可下载的清晰度",
            "分片列表是空的",
            "没有找到视频",
        ],
    ) {
        return plain(NoMedia);
    }
    // 12. 网站拒绝（403）：网页 403 多数是要在浏览器里确认；YouTube 下载时 403 是要确认不是机器人（先换客户端重试，见 queue.rs）。
    // 其他网站下载媒体时才 403（ZDF：信息接口不限地区，媒体地址按地区拦），多半是地区限制，别说成人机验证
    if st == Some(403) || has(&e, &["403 forbidden", "forbidden"]) {
        if has(&e, &["unable to download video data", "server returned 403"]) && site_name(url) != Some("YouTube") {
            return Detail::says(Geo, tr!("网站拒绝了下载（403），可能有地区限制，换个网络再试"));
        }
        return plain(NeedVerify);
    }
    // 13. 网络
    if matches!(st, Some(500..=599))
        || has(
            &e,
            &[
                "timed out",
                "timeout",
                "connection reset",
                "connection refused",
                "connection aborted",
                "network is unreachable",
                "no route to host",
                "temporary failure in name resolution",
                "name or service not known",
                "nodename nor servname",
                "could not resolve",
                "failed to resolve",
                "getaddrinfo",
                "dns error",
                "name resolution",
                "remote end closed connection",
                "incompleteread",
                "incomplete read",
                "bytes read",
                "eof occurred in violation of protocol",
                "connection broken",
                "broken pipe",
                "error sending request",
                "failed to establish connection",
                "network problem",
                "too slow downloading speed",
                "unable to download webpage",
                "unable to download api page",
                "unable to download json",
                "unable to download video data",
                "giving up after",
                "errorcode=2 ",
                "errorcode=5 ",
                "errorcode=6 ",
                "errorcode=19 ",
                "errorcode=22 ",
                "errorcode=23 ",
                "errorcode=29 ",
                "连不上",
                "超时",
                "没有收到数据",
                "一直断开",
                "一直没下载成功",
            ],
        )
    {
        return plain(Network);
    }
    plain(Unknown)
}

// ------------------------------------------------------------------ 各来源

/// HTTP 状态码 → 分类（reqwest 直接请求时用：M3U8、直链 HEAD）。
pub fn http_status(status: u16, url: &str) -> Failure {
    let raw = format!("HTTP {status}（{}）", url);
    let code = match status {
        // 直链、签名过期的地址回 400 多半是链接失效了
        400 | 404 | 410 => ErrorCode::Unavailable,
        401 => ErrorCode::NeedLogin,
        403 => ErrorCode::NeedVerify,
        418 => ErrorCode::Blocked,
        429 => ErrorCode::RateLimited,
        451 => ErrorCode::Geo,
        500..=599 => ErrorCode::Network,
        _ => ErrorCode::Unknown,
    };
    let (message, brief) = match code {
        ErrorCode::Unavailable if status == 400 => (tr!("对方网站不接受这个链接（HTTP 400），链接可能已经失效"), None),
        ErrorCode::Unavailable => (tr!("文件不存在（{status}），链接可能已经失效", status = status), None),
        ErrorCode::Blocked => (tr!("这个网站拒绝了你现在的网络（HTTP 418），换个网络或代理再试"), None),
        // 说法结尾带着「用浏览器插件下载」：插件里显示去掉它的短说法
        ErrorCode::NeedVerify => match site_name(url) {
            Some(n) => {
                let n = dynamic(n);
                (
                    tr!("{site}拒绝了下载请求（403），可能要先在网页里打开一次，或用浏览器插件下载", site = n),
                    Some(tr!("{site}拒绝了下载请求（403），可能要先在网页里打开一次", site = n)),
                )
            }
            None => (tr!("对方网站拒绝了下载请求（403），可能要用浏览器插件下载"), Some(tr!("对方网站拒绝了下载请求（403）"))),
        },
        ErrorCode::Unknown => (tr!("对方网站返回 {status}", status = status), None),
        c => msg(c, url, &format!("http {status}")),
    };
    let mut f = Failure::new(code, message, raw);
    f.brief = brief;
    f
}

/// aria2c 的退出码 + 输出 → 分类。aria2 的错误写在 stdout（console-log-level=warn），两路都要看。
pub fn classify_aria2(exit: Option<i32>, output: &str, url: &str) -> Failure {
    let lower = output.to_lowercase();
    // 「The response status is not successful. status=403」：按 HTTP 状态归
    if let Some(st) = http_status_in(&lower) {
        let mut f = http_status(st, url);
        f.raw = tail(output);
        return f;
    }
    let d = classify_detail(&strip_ansi(output), url);
    let by_text = d.code;
    let code = match exit {
        Some(2) | Some(5) | Some(6) | Some(7) | Some(8) | Some(19) | Some(22) | Some(23) | Some(29) | Some(32) => ErrorCode::Network,
        Some(3) | Some(4) => ErrorCode::Unavailable,
        Some(9) | Some(13) | Some(14) | Some(15) | Some(16) | Some(17) | Some(18) => ErrorCode::Disk,
        Some(24) => ErrorCode::NeedLogin,
        Some(28) => ErrorCode::Tool,
        _ => by_text,
    };
    let code = if code == ErrorCode::Network && by_text == ErrorCode::Blocked { ErrorCode::Blocked } else { code };
    // 说法用的是按文字归出来的那一句时，细分也跟着它
    let reason = if code == by_text { d.reason.map(str::to_string) } else { None };
    let (message, brief) = match code {
        ErrorCode::Unavailable => (tr!("文件不存在（{status}），链接可能已经失效", status = 404), None),
        ErrorCode::Unknown => (tr!("aria2 下载失败（错误码 {code}）", code = exit.unwrap_or(-1)), None),
        c if c == by_text && d.message.is_some() => (d.message.unwrap_or_default(), d.brief),
        c => msg(c, url, &lower),
    };
    let mut raw = tail(output);
    if raw.is_empty() {
        raw = format!("aria2c exit {}", exit.unwrap_or(-1));
    }
    Failure { code, message, raw, starts_at: None, site: None, reason, brief }
}

/// ffmpeg 的输出 → 分类（合并分片、转格式、截取样片时用）。
pub fn classify_ffmpeg(output: &str, url: &str) -> Failure {
    let lower = output.to_lowercase();
    // ffmpeg 读网络地址时：「Server returned 403 Forbidden (access denied)」
    if let Some(st) = http_status_in(&lower) {
        let mut f = http_status(st, url);
        f.raw = tail(output);
        return f;
    }
    let d = classify_detail(&strip_ansi(output), url);
    let code = d.code;
    let (message, brief) = match d.message {
        Some(m) => (m, d.brief),
        None if code == ErrorCode::Unknown => (tr!("生成文件失败：{error}", error = last_line_orig(output)), None),
        None => msg(code, url, &lower),
    };
    let mut f = Failure::new(code, message, output);
    f.reason = d.reason.map(str::to_string);
    f.brief = brief;
    f
}

/// io 错误：只认磁盘相关的（满了、没权限、只读），其他交给文字归类。
pub fn from_io(e: &std::io::Error) -> Option<Failure> {
    let raw = e.to_string();
    let message = match e.raw_os_error() {
        Some(28) | Some(69) => tr!("保存位置的磁盘满了"),
        Some(13) | Some(1) | Some(30) => tr!("没有权限写入保存位置"),
        Some(63) => tr!("文件名太长，保存不了"),
        _ => return None,
    };
    Some(Failure::new(ErrorCode::Disk, message, raw))
}

/// reqwest 的错误：有状态码按状态码；超时、连不上按网络；证书不对算被拦截。
pub fn from_reqwest(e: &reqwest::Error, url: &str) -> Failure {
    let url = e.url().map(|u| u.to_string()).unwrap_or_else(|| url.to_string());
    if let Some(st) = e.status() {
        return http_status(st.as_u16(), &url);
    }
    // 错误链里才有真正的原因（证书、DNS、超时……）
    let mut chain = e.to_string();
    let mut src = std::error::Error::source(e);
    while let Some(s) = src {
        chain.push_str(": ");
        chain.push_str(&s.to_string());
        src = s.source();
    }
    let lower = chain.to_lowercase();
    // 证书有问题（自签、被吊销、过期……）：blocked + reason cert，说法另换
    if has(&lower, CERT_ERRORS) || (lower.contains("certificate") && !lower.contains("handshake")) {
        let d = cert_detail();
        let mut f = Failure::new(d.code, d.message.unwrap_or_default(), chain);
        f.reason = d.reason.map(str::to_string);
        f.brief = d.brief;
        return f;
    }
    let code = if has(&lower, &["certificate", "notvalidforname", "unknownissuer", "invalid peer", "handshake"]) {
        ErrorCode::Blocked
    } else if e.is_timeout() || e.is_connect() || e.is_request() || e.is_body() || e.is_decode() {
        match classify_code(&chain, &url) {
            ErrorCode::Unknown => ErrorCode::Network,
            c => c,
        }
    } else {
        classify_code(&chain, &url)
    };
    let (message, brief) = if code == ErrorCode::Unknown { (clean_line(&chain), None) } else { msg(code, &url, &lower) };
    let mut f = Failure::new(code, message, chain);
    f.brief = brief;
    f
}

// ------------------------------------------------------------------ 测试

#[cfg(test)]
mod tests {
    use super::*;
    const YT: &str = "https://www.youtube.com/watch?v=dQw4w9WgXcQ";

    fn code(t: &str, url: &str) -> ErrorCode {
        classify(t, url).code
    }

    #[test]
    fn codes_roundtrip() {
        for c in ALL_CODES {
            assert_eq!(ErrorCode::parse(c.as_str()), Some(c));
            assert_eq!(serde_json::to_value(c).unwrap(), c.as_str());
            assert!(!c.action().is_empty() && !c.next_step().is_empty());
        }
        assert_eq!(ALL_CODES.len(), 15);
    }

    #[test]
    fn need_login() {
        let age = "ERROR: [youtube] 6kLq3WMV1nU: Sign in to confirm your age. This video may be inappropriate for some users. Use --cookies-from-browser or --cookies for the authentication. See  https://github.com/yt-dlp/yt-dlp/wiki/FAQ#how-do-i-pass-cookies-to-yt-dlp  for how to manually pass cookies. Also see  https://github.com/yt-dlp/yt-dlp/wiki/Extractors#exporting-youtube-cookies  for tips on effectively exporting YouTube cookies";
        let f = classify(age, YT);
        assert_eq!(f.code, ErrorCode::NeedLogin);
        assert!(f.message.contains("年龄限制") && f.message.contains("YouTube"), "{}", f.message);
        assert_eq!(f.code.action(), "login");
        let private = "ERROR: [youtube] abcdefghijk: Private video. Sign in if you've been granted access to this video. Use --cookies-from-browser or --cookies for the authentication. See  https://github.com/yt-dlp/yt-dlp/wiki/FAQ#how-do-i-pass-cookies-to-yt-dlp  for how to manually pass cookies";
        assert_eq!(classify(private, YT).message, "这是私密视频，要登录有权限的账号才能看");
        let nsfw = "ERROR: [twitter] 1712345678901234567: NSFW tweet requires authentication. Use --cookies, --cookies-from-browser, --username and --password, --netrc-cmd, or --netrc (twitter) to provide account credentials. See  https://github.com/yt-dlp/yt-dlp/wiki/FAQ#how-do-i-pass-cookies-to-yt-dlp  for how to manually pass cookies";
        let f = classify(nsfw, "https://x.com/a/status/1712345678901234567");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::NeedLogin, "这个视频要登录 X 才能看"));
        let ig = "ERROR: [Instagram] C9xYzAbCdEf: Requested content is not available, rate-limit reached or login required. Use --cookies, --cookies-from-browser, --username and --password, --netrc-cmd, or --netrc (instagram) to provide account credentials. See  https://github.com/yt-dlp/yt-dlp/wiki/FAQ#how-do-i-pass-cookies-to-yt-dlp  for how to manually pass cookies";
        assert_eq!(code(ig, "https://www.instagram.com/p/C9xYzAbCdEf/"), ErrorCode::NeedLogin);
        let members = "ERROR: [youtube] abcdefghijk: Join this channel to get access to members-only content like this video, and other exclusive perks.";
        assert_eq!(classify(members, YT).message, "这是会员专属视频，要登录开通了会员的账号");
        let bili = "ERROR: [BiliBili] BV1GJ411x7h7: This video is only available for registered users with the appropriate subscription. Use --cookies-from-browser or --cookies for the authentication. See  https://github.com/yt-dlp/yt-dlp/wiki/FAQ#how-do-i-pass-cookies-to-yt-dlp  for how to manually pass cookies";
        let f = classify(bili, "https://www.bilibili.com/video/BV1GJ411x7h7");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::NeedLogin, "这个视频要登录哔哩哔哩才能看"));
        assert_eq!(classify("ERROR: [Douyin] 1: Login required", "https://www.douyin.com/video/1").message, "这个视频要登录抖音才能看");
        let vimeo_pw = "ERROR: [vimeo] 123456: This video is protected by a password, use the --video-password option";
        assert_eq!(classify(vimeo_pw, "https://vimeo.com/123456").message, "这个视频设置了密码");
        // aria2 的 401
        let a = classify_aria2(Some(24), "Exception: [AbstractCommand.cc:351] errorCode=24 URI=https://a.com/x.mp4\n  -> [HttpSkipResponseCommand.cc:219] errorCode=24 Authorization failed.", "https://a.com/x.mp4");
        assert_eq!(a.code, ErrorCode::NeedLogin);
        assert_eq!(http_status(401, "https://a.com/x.m3u8").code, ErrorCode::NeedLogin);
    }

    #[test]
    fn need_verify() {
        let bot = "ERROR: [youtube] dQw4w9WgXcQ: Sign in to confirm you\u{2019}re not a bot. Use --cookies-from-browser or --cookies for the authentication. See  https://github.com/yt-dlp/yt-dlp/wiki/FAQ#how-do-i-pass-cookies-to-yt-dlp  for how to manually pass cookies. Also see  https://github.com/yt-dlp/yt-dlp/wiki/Extractors#exporting-youtube-cookies  for tips on effectively exporting YouTube cookies";
        let f = classify(bot, YT);
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::NeedVerify, "YouTube 要确认一下你不是机器人", "verify"));
        let douyin = "ERROR: [Douyin] 7451234567890123456: Fresh cookies (not necessarily logged in) are needed";
        let f = classify(douyin, "https://www.douyin.com/video/7451234567890123456");
        assert_eq!(f.code, ErrorCode::NeedVerify);
        assert!(f.message.contains("抖音"));
        let cf = "ERROR: [generic] Got HTTP Error 403 caused by Cloudflare anti-bot challenge; try again with  --extractor-args \"generic:impersonate\"";
        assert_eq!(code(cf, "https://example.com/watch/1"), ErrorCode::NeedVerify);
        let captcha = "ERROR: [youtube] abcdefghijk: This video is unavailable. YouTube is requiring a captcha challenge before playback";
        assert_eq!(code(captcha, YT), ErrorCode::NeedVerify);
        // 下载视频数据时 403（YouTube 要 PO Token 一类）
        let dl403 = "ERROR: unable to download video data: HTTP Error 403: Forbidden";
        assert_eq!(code(dl403, YT), ErrorCode::NeedVerify);
        assert_eq!(http_status(403, "https://cdn.example.com/a.mp4").code, ErrorCode::NeedVerify);
        let a = classify_aria2(Some(22), "Exception: [AbstractCommand.cc:351] errorCode=22 URI=https://cdn.a.com/x.mp4\n  -> [HttpSkipResponseCommand.cc:219] errorCode=22 The response status is not successful. status=403", "https://cdn.a.com/x.mp4");
        assert_eq!(a.code, ErrorCode::NeedVerify);
    }

    #[test]
    fn drm() {
        let yt = "ERROR: [youtube] abcdefghijk: This video is DRM protected";
        let f = classify(yt, YT);
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::Drm, "这个视频受版权保护（DRM），Captae 不能下载", "none"));
        let known = "ERROR: The requested site is known to use DRM protection. It will NOT be supported.\n       Please DO NOT open an issue, unless you have evidence that the video is not DRM protected";
        assert_eq!(code(known, "https://www.netflix.com/watch/80100172"), ErrorCode::Drm);
        assert_eq!(code("不支持的加密方式 SAMPLE-AES（可能是 DRM 保护的视频）", "https://a.com/x.m3u8"), ErrorCode::Drm);
    }

    #[test]
    fn geo() {
        let yt = "ERROR: [youtube] abcdefghijk: The uploader has not made this video available in your country";
        assert_eq!(code(yt, YT), ErrorCode::Geo);
        let generic = "ERROR: [BiliBiliBangumi] ep123: This video is not available from your location due to geo restriction. You might want to use a VPN or a proxy server (with --proxy) to workaround.";
        let f = classify(generic, "https://www.bilibili.com/bangumi/play/ep123");
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::Geo, "这个视频在你所在的地区看不了", "retry"));
        assert_eq!(http_status(451, "https://a.com/v").code, ErrorCode::Geo);
        // 实测（2026-11-23，海外的网络）：B 站说不清是删了还是地区限制
        let bili = "ERROR: [BiliBili] 1GJ411x7h7: This video may be deleted or geo-restricted. You might want to try a VPN or a proxy server (with --proxy)";
        let f = classify(bili, "https://www.bilibili.com/video/BV1GJ411x7h7");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Geo, "这个视频可能已被删除，或者在你所在的地区看不了"));
    }

    #[test]
    fn blocked() {
        // 实测：Vimeo 被运营商劫持，证书换成了运营商的
        let vimeo = "ERROR: [vimeo] 1084537: Unable to download webpage: [SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: Hostname mismatch, certificate is not valid for 'vimeo.com'. (_ssl.c:1010) (caused by CertificateVerifyError(\"[SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: Hostname mismatch, certificate is not valid for 'vimeo.com'. (_ssl.c:1010)\"))";
        let f = classify(vimeo, "https://vimeo.com/1084537");
        // 证书对不上：还是 blocked，细分 cert，说法说证书
        let cert_msg = "这个网站的证书有问题（过期、被吊销，或者网络在中途拦截），为了安全没有连接";
        assert_eq!((f.code, f.message.as_str(), f.reason.as_deref(), f.brief.as_deref(), f.code.action()), (ErrorCode::Blocked, cert_msg, Some("cert"), Some("这个网站的证书有问题"), "retry"));
        assert_eq!(f.to_json()["reason"], "cert");
        // 自签、被吊销、过期、根证书不认（rustls、Apple、Python 的说法）
        for raw in [
            "error sending request for url (https://self-signed.badssl.com/a.mp4): client error (Connect): invalid peer certificate: UnknownIssuer",
            "error sending request for url (https://revoked-isrgrootx1.letsencrypt.org/a.mp4): client error (Connect): invalid peer certificate: Revoked",
            "error sending request for url (https://expired.badssl.com/a.mp4): client error (Connect): invalid peer certificate: Expired",
            "client error (Connect): invalid peer certificate: Other(OtherError(\"“*.badssl.com” certificate is not trusted: -67843\"))",
            "ERROR: [generic] Unable to download webpage: [SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: certificate has expired (_ssl.c:1010)",
        ] {
            let f = classify(raw, "https://example.com/a.mp4");
            assert_eq!((f.code, f.reason.as_deref(), f.message.as_str()), (ErrorCode::Blocked, Some("cert"), cert_msg), "{raw}");
        }
        // 真正的网络拦截还是原来那句，没有 reason
        let f = classify("ERROR: [generic] Unable to download webpage: [SSL: TLSV1_ALERT_ACCESS_DENIED] tlsv1 alert access denied (_ssl.c:1010)", "https://example.com/v");
        assert_eq!((f.code, f.reason.as_deref()), (ErrorCode::Blocked, None));
        let curl = "ERROR: [vimeo] 1084537: Unable to download webpage: (60) SSL: no alternative certificate subject name matches target host name 'vimeo.com'";
        assert_eq!(code(curl, "https://vimeo.com/1084537"), ErrorCode::Blocked);
        let rustls = "error sending request for url (https://vimeo.com/1084537): client error (Connect): invalid peer certificate: NotValidForName";
        assert_eq!(code(rustls, "https://vimeo.com/1084537"), ErrorCode::Blocked);
        // 实测：aria2 连到被劫持的地址时 TLS 握手失败
        let a = classify_aria2(Some(1), "Exception: [AbstractCommand.cc:351] errorCode=1 URI=https://nonexistent.captae-test.invalid/a.mp4\n  -> [SocketCore.cc:1022] errorCode=1 SSL/TLS handshake failure: protocol error", "https://nonexistent.captae-test.invalid/a.mp4");
        assert_eq!((a.code, a.reason.as_deref(), a.message.as_str()), (ErrorCode::Blocked, None, "连接被你的网络拦截了，检查代理设置"));
    }

    #[test]
    fn unavailable() {
        let removed = "ERROR: [youtube] abcdefghijk: Video unavailable. This video has been removed by the uploader";
        let f = classify(removed, YT);
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::Unavailable, "视频已被删除或不存在", "none"));
        let term = "ERROR: [youtube] abcdefghijk: Video unavailable. This video is no longer available because the YouTube account associated with this video has been terminated.";
        assert_eq!(code(term, YT), ErrorCode::Unavailable);
        // 实测（2026-11-23）：已经结束、没有录像的直播
        let f = classify("ERROR: [youtube] jfKfPfyJRdk: This live stream recording is not available.", YT);
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Unavailable, "这场直播没有留下录像，或者录像已被删除"));
        // 实测：诊断里 YouTube A/B 实验的报错（公开与否要另外问 oEmbed）
        assert_eq!(code("ERROR: [youtube] mC73amqYFpQ: This video is not available", YT), ErrorCode::Unavailable);
        let http404 = "ERROR: [generic] Unable to download webpage: HTTP Error 404: Not Found (caused by <HTTPError 404: Not Found>)";
        assert_eq!(code(http404, "https://example.com/gone"), ErrorCode::Unavailable);
        // 实测：aria2 404 的输出（写在 stdout）
        let a404 = "\n11/23 22:02:29 [ERROR] CUID#7 - Download aborted. URI=https://download.blender.org/nonexistent-captae-test.mp4\nException: [AbstractCommand.cc:351] errorCode=3 URI=https://download.blender.org/nonexistent-captae-test.mp4\n  -> [HttpSkipResponseCommand.cc:219] errorCode=3 Resource not found\n\naria2 will resume download if the transfer is restarted.\nIf there are any errors, then see the log file. See '-l' option in help/man page for details.";
        let f = classify_aria2(Some(3), a404, "https://download.blender.org/nonexistent-captae-test.mp4");
        assert_eq!(f.code, ErrorCode::Unavailable);
        assert!(f.raw.contains("Resource not found"));
        assert_eq!(http_status(404, "https://a.com/x.mp4").message, "文件不存在（404），链接可能已经失效");
    }

    #[test]
    fn unsupported() {
        // 实测：视频号
        let wx = "ERROR: Unsupported URL: https://weixin.qq.com/sph/AdaU8QsrjC";
        let f = classify(wx, "https://weixin.qq.com/sph/AdaU8QsrjC");
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::Unsupported, "微信视频号暂不支持", "use_extension"));
        assert!(is_wechat_channels("https://weixin.qq.com/sph/AdaU8QsrjC"));
        assert!(is_wechat_channels("https://channels.weixin.qq.com/web/pages/feed?oid=1"));
        assert!(!is_wechat_channels("https://weixin.qq.com/"));
        assert_eq!(wechat_channels("https://weixin.qq.com/sph/x").message, "微信视频号暂不支持");
        let f = classify("ERROR: Unsupported URL: https://example.com/some/page", "https://example.com/some/page");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Unsupported, "Captae 还不认识这个网站，可以用浏览器插件在网页里下"));
    }

    #[test]
    fn no_media() {
        let tweet = "ERROR: [twitter] 1712345678901234567: No video could be found in this tweet";
        let f = classify(tweet, "https://x.com/a/status/1712345678901234567");
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::NoMedia, "这个网页里没有找到视频或音频", "none"));
        // Instagram 多图帖：图片帖，交给插件（第三阶段用户定的做法）
        let f = classify("ERROR: [Instagram] C9xYzAbCdEf: There is no video in this post", "https://www.instagram.com/p/C9xYzAbCdEf/");
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::Unsupported, IMAGE_POST, "use_extension"));
        assert_eq!(code("ERROR: [generic] index: No video formats found!", "https://example.com/index"), ErrorCode::NoMedia);
        // 认识的网站报 Unsupported URL：这一页不是视频
        assert_eq!(code("ERROR: Unsupported URL: https://www.youtube.com/about", "https://www.youtube.com/about"), ErrorCode::NoMedia);
        assert_eq!(code("没有找到可下载的清晰度", "https://a.com/x.m3u8"), ErrorCode::NoMedia);
    }

    #[test]
    fn live() {
        let f = classify("ERROR: [youtube] abcdefghijk: This live event will begin in 3 hours.", YT);
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::LiveUpcoming, "直播大约 3 小时后开始", "wait_live"));
        assert_eq!(classify("ERROR: [youtube] abcdefghijk: Premieres in 2 days", YT).message, "首映大约 2 天后开始");
        assert_eq!(classify("ERROR: [youtube] abcdefghijk: This live event will begin in a few moments.", YT).message, "直播马上开始");
        assert_eq!(code("ERROR: [twitch:stream] somechannel: The channel is not currently live", "https://www.twitch.tv/somechannel"), ErrorCode::LiveUpcoming);
        let f = classify("这是正在进行的直播，目前只支持回放和点播", "https://a.com/live.m3u8");
        assert_eq!((f.code, f.code.action()), (ErrorCode::Live, "record_live"));
        assert!(live_at_text(crate::util::now_ms() + 3_600_000).starts_with("直播 "));
    }

    #[test]
    fn rate_limited() {
        let yt = "ERROR: [youtube] abcdefghijk: This content isn't available, try again later. The current session has been rate-limited by YouTube for up to an hour. It is recommended to use `-t sleep` to add a delay between video requests to avoid exceeding the rate limit. For more information, refer to  https://github.com/yt-dlp/yt-dlp/wiki/Extractors#this-content-isnt-available-try-again-later";
        let f = classify(yt, YT);
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::RateLimited, "网站让我们慢一点，稍后自动重试", "auto_retry"));
        let tt = "ERROR: [TikTok] 7300000000000000000: Unable to download webpage: HTTP Error 429: Too Many Requests (caused by <HTTPError 429: Too Many Requests>)";
        assert_eq!(code(tt, "https://www.tiktok.com/@a/video/7300000000000000000"), ErrorCode::RateLimited);
        assert_eq!(http_status(429, "https://a.com").code, ErrorCode::RateLimited);
    }

    #[test]
    fn network() {
        let dns = "ERROR: [youtube] abcdefghijk: Unable to download API page: <urlopen error [Errno 8] nodename nor servname provided, or not known> (caused by TransportError('<urlopen error [Errno 8] nodename nor servname provided, or not known>'))";
        let f = classify(dns, YT);
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::Network, "网络不稳定，没连上对方网站", "retry"));
        assert_eq!(classify("ERROR: unable to download video data: The read operation timed out", YT).message, "网络不稳定，连接超时了");
        assert_eq!(code("ERROR: [download] Got error: 1048576 bytes read, 2097152 more expected. Giving up after 10 retries", YT), ErrorCode::Network);
        assert_eq!(classify_aria2(Some(2), "Exception: [AbstractCommand.cc:351] errorCode=2 URI=https://a.com/x.mp4\n  -> [AbstractCommand.cc:405] errorCode=2 Timeout.", "https://a.com/x.mp4").code, ErrorCode::Network);
        assert_eq!(classify_aria2(Some(19), "errorCode=19 Name resolution for a.example failed:nodename nor servname provided, or not known", "https://a.example/x.mp4").code, ErrorCode::Network);
        let f = http_status(503, "https://a.com/x.m3u8");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Network, "对方网站暂时出了问题（HTTP 503），稍后再试"));
        assert_eq!(code("第 3 段一直下载失败（30 秒没有收到数据）", "https://a.com/x.m3u8"), ErrorCode::Network);
    }

    #[test]
    fn disk() {
        let f = classify("ERROR: unable to write data: [Errno 28] No space left on device", YT);
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::Disk, "保存位置的磁盘满了", "change_dir"));
        let perm = "ERROR: unable to open for writing: [Errno 13] Permission denied: '/Volumes/Backup/Captae/video.f137.mp4.part'";
        assert_eq!(classify(perm, YT).message, "没有权限写入保存位置");
        assert_eq!(code("ERROR: unable to create directory [Errno 30] Read-only file system: '/Volumes/RO/Captae'", YT), ErrorCode::Disk);
        // 实测：aria2 建不了目录
        let a = classify_aria2(Some(18), "  -> [util.cc:1951] errNum=1 errorCode=18 Failed to make the directory /System/x, cause: Operation not permitted", "https://a.com/x.mp4");
        assert_eq!((a.code, a.message.as_str()), (ErrorCode::Disk, "没有权限写入保存位置"));
        assert_eq!(classify_aria2(Some(9), "errorCode=9 There is not enough disk space available.", "https://a.com/x.mp4").message, "保存位置的磁盘满了");
        let ff = classify_ffmpeg("[out#0/mp4 @ 0x600000c8c000] Error opening output /Volumes/RO/x.mp4: Permission denied\nError opening output file /Volumes/RO/x.mp4.", "");
        assert_eq!(ff.code, ErrorCode::Disk);
        let io = std::io::Error::from_raw_os_error(28);
        assert_eq!(from_io(&io).unwrap().message, "保存位置的磁盘满了");
        assert_eq!(from_io(&std::io::Error::from_raw_os_error(13)).unwrap().code, ErrorCode::Disk);
        assert!(from_io(&std::io::Error::from_raw_os_error(2)).is_none());
    }

    #[test]
    fn tool() {
        let f = classify("没有找到 yt-dlp，请在设置里指定路径或重新安装 Captae", YT);
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::Tool, "下载组件出了问题（没有找到 yt-dlp）", "repair"));
        let pyi = "[PYI-48211:ERROR] Failed to load Python shared library '/Users/a/engines/yt-dlp/2026.10.26/_internal/Python': dlopen(...): tried: '...' (code signature invalid)";
        assert_eq!(code(pyi, YT), ErrorCode::Tool);
        let tb = "Traceback (most recent call last):\n  File \"yt_dlp/__main__.py\", line 17, in <module>\nImportError: cannot import name 'main' from 'yt_dlp'";
        assert_eq!(code(tb, YT), ErrorCode::Tool);
        let old = "Usage: yt-dlp [OPTIONS] URL [URL...]\n\nyt-dlp: error: no such option: --js-runtimes";
        assert!(classify(old, YT).message.contains("版本太旧"));
        let nojs = "WARNING: [youtube] No supported JavaScript runtime could be found. Only deno is enabled by default; to use another runtime add  --js-runtimes RUNTIME[:PATH]  to your command/config. YouTube extraction without a JS runtime has been deprecated, and some formats may be missing. See  https://github.com/yt-dlp/yt-dlp/wiki/EJS  for details on installing one\nERROR: [youtube] mC73amqYFpQ: This video is not available";
        let f = classify(nojs, YT);
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Tool, "下载组件出了问题（缺少 JS 运行时）"));
        assert_eq!(code("ERROR: Postprocessing: ffprobe and ffmpeg not found. Please install or provide the path using --ffmpeg-location", YT), ErrorCode::Tool);
        // 实测：旧版 yt-dlp（2026.08.19）+ deno 解不了新的 YouTube 验证，deno 的报错带颜色控制符
        let old = "WARNING: [youtube] [jsc:deno] Error solving n challenge: \u{1b}[0m\u{1b}[1mTypeError\u{1b}[0m: x is not a function\nWARNING: [youtube] M7XYiGAYKJ4: n challenge solving failed: Some formats may be missing. Ensure you have a supported JavaScript runtime and challenge solver script distribution installed.\nERROR: [youtube] M7XYiGAYKJ4: Requested format is not available. Use --list-formats for a list of available formats";
        let f = classify(old, YT);
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Tool, "下载引擎解不了 YouTube 的验证，可能要更新下载引擎"));
        assert!(!f.raw.contains('\u{1b}'));
    }

    #[test]
    fn unknown() {
        let t = "ERROR: [someextractor] 1234567890: Something unexpected happened here; please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q= , filling out the appropriate issue template. Confirm you are on the latest version using  yt-dlp -U";
        let f = classify(t, "https://www.example.com/watch/?v=1234567890");
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::Unknown, "Something unexpected happened here", "copy_error"));
        assert!(f.raw.contains("please report this issue"));
        assert!(f.code.engine_fault() && f.code.transient());
    }

    #[test]
    fn warnings_dont_mislead() {
        // WARNING 里提到 cookies，但真正的错误是网络
        let t = "WARNING: [youtube] Use --cookies-from-browser or --cookies for the authentication\nERROR: [youtube] abcdefghijk: Unable to download API page: The read operation timed out";
        assert_eq!(code(t, YT), ErrorCode::Network);
    }

    #[test]
    fn anyhow_chain() {
        let f = Failure::new(ErrorCode::Geo, "这个视频在你所在的地区看不了", "x");
        let e = anyhow::Error::new(f.clone()).context("读取视频信息失败");
        assert_eq!(from_anyhow(&e, YT), f);
        let t = anyhow::Error::new(Tagged { tag: "DOWNLOAD_FAILED".into(), failure: f.clone() });
        assert_eq!(t.to_string(), "DOWNLOAD_FAILED: 这个视频在你所在的地区看不了");
        assert_eq!(failure_in(&t), Some(&f));
        let io = anyhow::Error::new(std::io::Error::from_raw_os_error(28)).context("写文件");
        assert_eq!(from_anyhow(&io, YT).code, ErrorCode::Disk);
        let plain = anyhow::anyhow!("这是正在进行的直播，目前只支持回放和点播");
        assert_eq!(from_anyhow(&plain, "https://a.com/l.m3u8").code, ErrorCode::Live);
        let j = f.to_json();
        assert_eq!((j["code"].as_str(), j["action"].as_str()), (Some("geo"), Some("retry")));
    }

    /// 第三阶段第一轮几百个网站实测（2026-11-24）的原始报错：「下载引擎跟不上网站」（错误码见 docs/DESIGN.md 第 4.2 节）
    #[test]
    fn round1_engine_lag() {
        const REPORT: &str = "; please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q= , filling out the appropriate issue template. Confirm you are on the latest version using  yt-dlp -U";
        let lag = |raw: String, url: &str| {
            let f = classify(&raw, url);
            assert_eq!((f.code, f.code.action()), (ErrorCode::Unsupported, "use_extension"), "{url}: {raw}");
            assert!(f.message.starts_with("下载引擎暂时读不了这个网站") || f.message.starts_with("网站不接受下载引擎的请求"), "{url}: {}", f.message);
            assert!(is_engine_lag(&f), "{url}");
            f
        };
        // abc-4d0df2、acfun-d6f18a、naver-2bf105、pbs-9c30da：KeyError
        lag(format!("ERROR: LE1601V001S00: An extractor error has occurred. (caused by KeyError('playlist')){REPORT}"), "https://iview.abc.net.au/video/LE1601V001S00");
        lag(format!("ERROR: aa6145863: An extractor error has occurred. (caused by KeyError('currentVideoInfo')){REPORT}"), "https://www.acfun.cn/bangumi/aa6145863");
        lag(format!("ERROR: An extractor error has occurred. (caused by KeyError('title')){REPORT}"), "https://www.pbs.org/newshour/show/news-wrap-stocks-tumble-as-worries-about-ai-rattle-market");
        // bbc-217199、imdb-15318e、snapchat-29c9f5、tubitv-abbaee、udemy-7581c1、douyu-7bb3e9、likee-c15f63：Unable to extract
        lag(format!("ERROR: [bbc] w3ct8md0: Unable to extract playlist data{REPORT}"), "https://www.bbc.co.uk/sounds/play/w3ct8md0");
        lag(format!("ERROR: [imdb] 386512153: Unable to extract next.js data{REPORT}"), "https://www.imdb.com/video/vi386512153/");
        lag(format!("ERROR: [udemy:course] course: Unable to extract course id{REPORT}"), "https://www.udemy.com/course/the-complete-web-development-bootcamp/");
        // douyu-4fa83f：PhantomJS not found（以前被「not found」当成已删除）
        let f = lag("ERROR: [DouyuShow] Qyz171rONnBMBJj9: PhantomJS not found, Please download it from https://phantomjs.org/download.html".into(), "https://v.douyu.com/show/Qyz171rONnBMBJj9");
        assert_ne!(f.code, ErrorCode::Unavailable);
        // facebook-05f0e8、iqiyi-4fed90、youku-2b5ef5、mit-c423cc、nbcnews-83b364
        lag(format!("ERROR: [facebook] 2347929699321330: Cannot parse data{REPORT}"), "https://fb.watch/L4dqgbIK4A/");
        lag(format!("ERROR: [iqiyi] Can't find any video{REPORT}"), "https://www.iqiyi.com/v_1nj820ys5vg.html");
        lag(format!("ERROR: [youku] XNjU0NTk3OTUwMA: Youku server reported error -6004: 该账号检测到有可疑违规、侵权等行为，已限制播放，如有疑问，请向我们反馈{REPORT}"), "https://v.youku.com/v_show/id_XNjU0NTk3OTUwMA==.html");
        lag(format!("WARNING: [ocw.mit.edu] unable to extract WT.cg_s{REPORT}\nERROR: [ocw.mit.edu] Unable to find embedded YouTube video.{REPORT}"), "https://ocw.mit.edu/courses/18-06-linear-algebra-spring-2010/resources/lecture-1-the-geometry-of-linear-equations/");
        lag("ERROR: list index out of range".into(), "https://www.nbcnews.com/watch");
        // yahoo-3325aa：专门提取器的接口 404（以前说成已删除）
        let f = lag("ERROR: [yahoo] 150114985: Unable to download JSON metadata: HTTP Error 404: Not Found (caused by <HTTPError 404: Not Found>)".into(), "https://tw.tv.yahoo.com/newsforfanpage/x-150114985.html");
        assert!(f.message.contains("也可能视频已经删除"), "{}", f.message);
        // pluto-208e3f、telemundo-cdf7c8、ndtv-d14cc4：yt-dlp 标成失效的提取器，后面的 404 / 403 不可信
        let broken = "WARNING: The program functionality for this site has been marked as broken, and will probably not work.";
        lag(format!("{broken}\nERROR: [PlutoTV] 617d575eb25019001ab2caa1: Unable to download JSON metadata: HTTP Error 404: Not Found (caused by <HTTPError 404: Not Found>)"), "https://pluto.tv/us/on-demand/series/617d575eb25019001ab2caa1");
        lag(format!("{broken}\nERROR: [Telemundo] tmvo1324653: Unable to download webpage: HTTP Error 403: Forbidden (caused by <HTTPError 403: Forbidden>)"), "https://www.telemundo.com/shows/juego-de-villanos/juego-de-villanos/video/juegodevillanos-s01-1119-tmvo13246537");
        lag(format!("{broken}\nERROR: [Telemundo] tmvo1324747: Response data has no m3u header{REPORT}"), "https://www.telemundo.com/noticias/x-tmvo13247474");
        // meipai-a2f2b1：一路「unable to extract」警告，最后说没有格式
        lag(format!("WARNING: [Meipai] unable to extract release date{REPORT}\nWARNING: [Meipai] unable to extract creator{REPORT}\nERROR: [Meipai] 581775227: No video formats found!{REPORT}"), "https://www.meipai.com/media/581775227");
        // khanacademy-ecf27f、dr-5c8cf5、medal-b81384、foxnews-54cdf5：接口 400（以前说成网络不稳）
        for (raw, u) in [
            ("ERROR: [khanacademy] math/x/v/why-we-do-the-same-thing-to-both-sides-simple-equations: Unable to download JSON metadata: HTTP Error 400: Bad Request (caused by <HTTPError 400: Bad Request>)", "https://www.khanacademy.org/math/x/v/why-we-do-the-same-thing-to-both-sides-simple-equations"),
            ("ERROR: [drtv] 00922604730: Unable to download JSON metadata: HTTP Error 400: Bad Request (caused by <HTTPError 400: Bad Request>)", "https://www.dr.dk/drtv/episode/x_634790"),
            ("ERROR: [MedalTV] nYiYMMGqugp5N9XW0: Unable to download JSON metadata: HTTP Error 400: Bad Request (caused by <HTTPError 400: Bad Request>)", "https://medal.tv/games/valorant/clips/nYiYMMGqugp5N9XW0"),
            ("ERROR: [foxnews] 1234567890123: Unable to download Akamai AMP feed: HTTP Error 400: Bad Request (caused by <HTTPError 400: Bad Request>)", "https://www.foxnews.com/video/1234567890123"),
        ] {
            let f = lag(raw.into(), u);
            assert!(f.message.contains("HTTP 400") && f.message.contains("要登录"), "{}", f.message);
            assert_ne!(f.code, ErrorCode::Network);
        }
        // YouTube 的 400 照旧是网络问题
        assert_eq!(code("ERROR: [youtube] abcdefghijk: Unable to download API page: HTTP Error 400: Bad Request (caused by <HTTPError 400: Bad Request>)", YT), ErrorCode::Network);
        // 已删除的照旧是已删除：普通网页 404 不算「引擎跟不上」
        assert_eq!(code("ERROR: [generic] Unable to download webpage: HTTP Error 404: Not Found (caused by <HTTPError 404: Not Found>)", "https://example.com/gone"), ErrorCode::Unavailable);
        assert!(!is_engine_lag(&classify("ERROR: Unsupported URL: https://example.com/a", "https://example.com/a")));
    }

    /// FIXES 第 2 项：零散的说法
    #[test]
    fn round1_misc() {
        let c = |raw: &str, url: &str| classify(raw, url);
        // bandcamp-855f1f：人机验证页
        assert_eq!(c("ERROR: [Bandcamp:album] hellbound-hammered: Got client challenge webpage response with chrome-152:windows-10; please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q= ", "https://devilshour.bandcamp.com/album/hellbound-hammered").code, ErrorCode::NeedVerify);
        // ixigua-2cad08
        let f = c("ERROR: [Ixigua] 704807693762061979: Cookies (not necessarily logged in) are needed", "https://www.ixigua.com/7048076937620619790");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::NeedVerify, "西瓜视频要先在网页里确认一下（人机验证）"));
        // patreon-a22409
        let f = c("ERROR: [patreon] 160575377: You do not have access to this post", "https://www.patreon.com/koalagardens/posts/your-exclusive-160575377");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::NeedLogin, "这个视频要登录 Patreon 才能看"));
        // sina-4ccaf0：418 是网站拒绝了这个出口，不是网络不稳
        let f = c("ERROR: [Sina] detail-inizrape8930495.d: Unable to download webpage: HTTP Error 418: I'm a teapot (caused by <HTTPError 418: I'm a teapot>)", "https://video.sina.com.cn/p/tech/2026-11-22/detail-inizrape8930495.d.html");
        assert_eq!((f.code, f.code.action()), (ErrorCode::Blocked, "retry"));
        assert!(f.message.contains("HTTP 418"), "{}", f.message);
        // tiktok-e3470b：构造的不存在的视频，yt-dlp 说 IP 被封
        let f = c("ERROR: [TikTok] 7000000000000000001: Your IP address is blocked from accessing this post", "https://www.tiktok.com/@tiktok/video/7000000000000000001");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Geo, "这个视频可能已被删除，或者 TikTok 在你现在的网络下不给看"));
        // weverse-76bf2b：直播结束了，没有回放
        let f = c("ERROR: [Weverse] 2-177971127: Livestream has ended and downloadable VOD is not available", "https://weverse.io/weversezone/live/2-177971127");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Unavailable, "这场直播没有留下录像，或者录像已被删除"));
        // wistia-f09a5b
        assert_eq!(c("ERROR: [Wistia] sh7fpupwlt: Error while getting the playlist: True", "https://fast.wistia.net/embed/iframe/sh7fpupwlt").code, ErrorCode::Unavailable);
        // dailymotion-c34624：按出口地区翻成了印尼语
        assert_eq!(c("ERROR: [dailymotion] x8qckyq: dailymotion said: Konten pribadi.", "https://www.dailymotion.com/video/x8qckyq").code, ErrorCode::NeedLogin);
        // xiaohongshu-a50820：短链没登录时跳到登录页
        let f = c("WARNING: [generic] Falling back on generic information extractor\nERROR: Unsupported URL: https://www.rednote.com/login?redirectPath=http%3A%2F%2Fwww.rednote.com%2Fdiscovery%2Fitem%2F6ae4810f000000002501c4bb", "https://xhslink.com/o/6m2K0LsSVOg");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::NeedLogin, "这个视频要登录小红书才能看"));
        // xiaohongshu-a3612b：也是跳到登录页，但登录以后要回去的只是首页（redirectPath=/explore）：分享链接失效了
        let f = c("WARNING: [generic] Falling back on generic information extractor\nERROR: Unsupported URL: https://www.rednote.com/login?redirectPath=http%3A%2F%2Fwww.rednote.com%2Fexplore%3FexSource%3Dnull", "https://xhslink.com/o/7v4dOqrL30K");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Unavailable, "这个分享链接已经失效了（打开以后只跳到了网站首页）"));
        // xiaohongshu-d5ec1c：跳到 404 页
        let f = c("WARNING: [generic] Falling back on generic information extractor\nERROR: Unsupported URL: https://www.rednote.com/404?source=/404/sec_hYbDwotg?redirectPath=x&error_code=300031", "https://xhslink.com/o/1WiQ1QI6Uc0");
        assert_eq!(f.code, ErrorCode::Unavailable);
        // flickr-467d16：图片页
        let f = c("ERROR: [Flickr] 55666900499: not a video", "https://www.flickr.com/photos/stillinthestream/55666900499/");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Unsupported, IMAGE_POST));
        // 直链的 400、418
        assert_eq!(http_status(400, "https://cdn.example.com/a.mp4?sig=old").code, ErrorCode::Unavailable);
        assert_eq!(http_status(418, "https://cdn.example.com/a.mp4").code, ErrorCode::Blocked);
        assert_eq!(http_status(410, "https://cdn.example.com/a.mp4").message, "文件不存在（410），链接可能已经失效");
    }

    /// FIXES 第 3 项：yt-dlp 不支持的地址，不再说成「网页里没有视频」
    #[test]
    fn round1_unsupported_sites() {
        let generic = |landed: &str| format!("WARNING: [generic] Falling back on generic information extractor\nERROR: Unsupported URL: {landed}");
        // kuaishou-2395ba、kuaishou-4b34e9、threads-12420a、douyin-af74d1
        let f = classify(&generic("https://www.kuaishou.com/short-video/3xbfsf6nje489hy"), "https://www.kuaishou.com/short-video/3xbfsf6nje489hy");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Unsupported, "下载引擎还不支持这种快手链接，可以用浏览器插件在网页里下"));
        let f = classify(&generic("https://www.threads.com/@nasa/post/Df-IxZ8AgOO"), "https://www.threads.net/@nasa/post/Df-IxZ8AgOO");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Unsupported, "下载引擎还不支持这种 Threads 链接，可以用浏览器插件在网页里下"));
        assert_eq!(classify(&generic("https://live.douyin.com/747833069881"), "https://live.douyin.com/747833069881").code, ErrorCode::Unsupported);
        // douyin-a77b88、tiktok-137d07：图集
        for u in ["https://www.douyin.com/note/7657170973843024753", "https://www.tiktok.com/@nasa/photo/7701866987609197837"] {
            let f = classify(&generic(u), u);
            assert_eq!((f.code, f.message.as_str()), (ErrorCode::Unsupported, IMAGE_POST), "{u}");
        }
        // douyin-860a38：短链跳到了抖音首页（分享链接失效）
        let f = classify(&generic("https://www.douyin.com/"), "https://v.douyin.com/dqv3dv/");
        assert_eq!(f.code, ErrorCode::Unavailable);
        // 认识的大网站、提取器自己说 Unsupported URL：这一页没有视频
        assert_eq!(code("ERROR: Unsupported URL: https://www.youtube.com/about", "https://www.youtube.com/about"), ErrorCode::NoMedia);
        assert_eq!(code(&generic("https://www.youtube.com/about"), "https://www.youtube.com/about"), ErrorCode::NoMedia);
        // 图片帖的网站（第一轮归成「图片帖」的 13 条）
        for u in ["https://500px.com/photo/rypjyDXi64Q", "https://www.pixiv.net/artworks/152301042", "https://www.pexels.com/photo/x-40120200/", "https://dribbble.com/shots/27904373-Airduct-Logo"] {
            assert_eq!(classify(&generic(u), u).message, IMAGE_POST, "{u}");
        }
        assert_eq!(classify("ERROR: [9gag] arWvBbd: The given url does not contain a video", "https://9gag.com/gag/arWvBbd").message, IMAGE_POST);
        assert_eq!(classify("ERROR: [XiaoHongShu] 6b0336bd000000001b01ff7f: No video formats found!; please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q= ", "https://www.xiaohongshu.com/explore/6b0336bd000000001b01ff7f").message, IMAGE_POST);
        assert_eq!(classify("playlist has no entries", "https://imgur.com/gallery/3ly5zsL").message, IMAGE_POST);
    }

    /// FIXES 第 4 项：地区限制、还没开播的其他说法
    #[test]
    fn round1_geo_and_upcoming() {
        let c = |raw: &str, url: &str| classify(raw, url).code;
        assert_eq!(c("ERROR: [bbc.co.uk] m0034qtv: bbc.co.uk returned error: geolocation", "https://www.bbc.co.uk/iplayer/episode/m0034qtv/eastenders-23112026"), ErrorCode::Geo);
        assert_eq!(c("ERROR: [NRK] KMTE30004526: NRK said: Ikke tilgjengelig utenfor Norge", "https://tv.nrk.no/serie/a-laget/sesong/3/episode/KMTE30004526"), ErrorCode::Geo);
        let srgssr = "ERROR: [SRGSSR] 4128cf0a-c704-3be2-ad9d-674c63dc19d7: For legal reasons, this video is only available in Switzerland.\nThis video is available in Switzerland.\nYou might want to use a VPN or a proxy server (with --proxy) to workaround.";
        let f = classify(srgssr, "https://www.rts.ch/play/tv/film/video/emilia-perez");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Geo, "这个视频在你所在的地区看不了"));
        // svtplay-901a25：前面一堆 403 警告，真正的原因在 ERROR 和它的续行里
        let svt = "WARNING: [svt:play] jmyWdJZ: Failed to download m3u8 information: HTTP Error 403: Forbidden\nWARNING: [svt:play] jmyWdJZ: Failed to download MPD manifest: HTTP Error 403: Forbidden\nERROR: [svt:play] KZm2Pad: This video is only available in Sweden\nThis video is available in Sweden.\nYou might want to use a VPN or a proxy server (with --proxy) to workaround.";
        assert_eq!(c(svt, "https://www.svtplay.se/video/KZm2Pad/det-grona-flyget"), ErrorCode::Geo);
        assert_eq!(c("ERROR: [TVer] epoziaoibh: REQUEST_FAILED: 163: この動画の視聴は許可されていません。; please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q= ", "https://tver.jp/episodes/epoziaoibh"), ErrorCode::Geo);
        // 只在 App 里能看的不算地区限制
        assert_ne!(c("ERROR: [x] 1: This video is only available in the app", "https://example.com/v/1"), ErrorCode::Geo);
        // 还没开播
        assert_eq!(c("ERROR: [gem.cbc.ca:live] 48589: This livestream has not yet started", "https://gem.cbc.ca/live-event/48589"), ErrorCode::LiveUpcoming);
        assert_eq!(c("ERROR: [dailymotion] x8ge5un: dailymotion said: Saluran offline.", "https://www.dailymotion.com/video/x8ge5un"), ErrorCode::LiveUpcoming);
        let f = classify("ERROR: [NRK] NNFA56112426: NRK said: Kommer i morgen kl. 18:00", "https://tv.nrk.no/serie/dagsnytt-atten-tv/sesong/202611/episode/NNFA56112426");
        assert_eq!((f.code, f.message.as_str(), f.code.action()), (ErrorCode::LiveUpcoming, "这一集还没上线（挪威时间明天 18:00）", "wait_live"));
        // 下载时才 403（ZDF 这种按地区拦媒体地址的）：别说成人机验证
        let f = classify("ERROR: unable to download video data: HTTP Error 403: Forbidden", "https://www.zdf.de/video/serien/das-traumschiff-104/vietnam-phu-quoc-102");
        assert_eq!(f.code, ErrorCode::Geo);
        assert!(!f.message.contains("人机验证"), "{}", f.message);
    }

    /// ffmpeg 退出码 8 是读网址时的 HTTP 错误（britishpathe-972a0d 第一轮：样片 code 8，真实下载 429）
    #[test]
    fn ffmpeg_http_error() {
        let f = classify("WARNING: The video format is unknown and cannot be downloaded by ffmpeg. Explicitly set the extension in the filename to attempt download in that format\nERROR: ffmpeg exited with code 8", "https://www.britishpathe.com/asset/187203/");
        assert_eq!((f.code, f.code.action()), (ErrorCode::Network, "retry"));
        assert!(f.message.contains("HTTP 错误"), "{}", f.message);
        // 183（数据无效）照旧算不是媒体
        assert_eq!(code("WARNING: The video format is unknown and cannot be downloaded by ffmpeg.\nERROR: ffmpeg exited with code 183", "https://www.rfc-editor.org/rfc/rfc9110.pdf"), ErrorCode::NoMedia);
    }

    /// FIXES 第 6 项：下载下来才发现不是媒体
    #[test]
    fn round1_not_media() {
        let f = classify("WARNING: The video format is unknown and cannot be downloaded by ffmpeg. Explicitly set the extension in the filename to attempt download in that format\nERROR: ffmpeg exited with code 183", "https://www.rfc-editor.org/rfc/rfc9110.pdf");
        assert_eq!(f.code, ErrorCode::NoMedia);
        let f = classify("ERROR: Postprocessing: Error opening input files: Invalid data found when processing input", "https://example.com/x");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::NoMedia, "下载到的不是视频或音频，可能是网页或别的文件"));
        let ff = classify_ffmpeg("[in#0 @ 0x14c0041b0] EBML header parsing failed\n[in#0 @ 0x14c004080] Error opening input: Invalid data found when processing input\nError opening input file https://commons.wikimedia.org/wiki/File:Shila_Waterfall_in_Zanaskar.webm.\nError opening input files: Invalid data found when processing input", "https://commons.wikimedia.org/wiki/File:Shila_Waterfall_in_Zanaskar.webm");
        assert_eq!(ff.code, ErrorCode::NoMedia);
    }

    /// 对照组：Odysee 先在 WARNING 里回 429，ERROR 才说没有格式
    #[test]
    fn warning_429_then_no_formats() {
        let t = "WARNING: [lbry] @videogamescores#d/2021-Vocal-Track-of-the-Year#a: Unable to download webpage: HTTP Error 429: Too Many Requests\nERROR: [lbry] ac9a87a4c08f15c70cd47cdbaa5d89b0d68395d0: No video formats found!; please report this issue on  https://github.com/yt-dlp/yt-dlp/issues?q= ";
        let f = classify(t, "https://odysee.com/@videogamescores:d/2021-Vocal-Track-of-the-Year:a");
        assert_eq!((f.code, f.code.action()), (ErrorCode::RateLimited, "auto_retry"));
        // 没有 429 的照旧是没有视频
        assert_eq!(code("ERROR: [generic] index: No video formats found!", "https://example.com/index"), ErrorCode::NoMedia);
    }

    /// ERROR 后面的续行算进来，WARNING 不算
    #[test]
    fn focus_keeps_continuation_lines() {
        let t = "WARNING: [x] cookies maybe\nERROR: [x] 1: For legal reasons\nThis video is available in Norway.\nYou might want to use a VPN or a proxy server (with --proxy) to workaround.\n[download] something else";
        let f = focus(t);
        assert!(f.contains("use a vpn") && !f.contains("cookies maybe") && !f.contains("something else"), "{f}");
    }

    #[test]
    fn raw_is_trimmed() {
        let long: String = (0..100).map(|i| format!("WARNING: line {i}\n")).collect();
        let f = classify(&format!("{long}ERROR: boom"), YT);
        assert!(f.raw.lines().count() <= 12);
        assert!(f.raw.ends_with("ERROR: boom"));
    }

    /// 收尾修补第 4 项：说法里用的网站名字一起交给界面（「登录芒果TV」按钮和上面那句话对得上）
    #[test]
    fn site_goes_with_message() {
        let login = "ERROR: [x] 1: This video is only available for registered users. Use --cookies-from-browser or --cookies for the authentication.";
        for (url, site, msg) in [
            // 名字结尾是英文字母的那一边加空格（和界面的 cjk() 同一条规矩）
            ("https://www.mgtv.com/b/1/2.html", Some("芒果TV"), "这个视频要登录芒果TV 才能看"),
            ("https://www.bilibili.com/video/BV1GJ411x7h7", Some("哔哩哔哩"), "这个视频要登录哔哩哔哩才能看"),
            ("https://www.youtube.com/watch?v=abcdefghijk", Some("YouTube"), "这个视频要登录 YouTube 才能看"),
            ("https://x.com/a/status/1", Some("X"), "这个视频要登录 X 才能看"),
            ("https://video.example.org/watch/1", None, "这个视频要登录才能看"),
        ] {
            let f = classify(login, url);
            assert_eq!((f.code, f.site.as_deref(), f.message.as_str()), (ErrorCode::NeedLogin, site, msg), "{url}");
            // JSON 里带着（没有名字的网站不带，界面退回主机名）
            assert_eq!(f.to_json().get("site").and_then(|v| v.as_str()), site);
        }
        // 链接认不出、跳到了认得的网站的登录页：说法和名字都按跳转后的网站
        let f = classify("ERROR: Unsupported URL: https://www.rednote.com/login?redirectPath=%2Fexplore%2F6655", "https://xhs.example/abc");
        assert_eq!((f.code, f.site.as_deref()), (ErrorCode::NeedLogin, Some("小红书")));
        assert!(f.message.contains("小红书"), "{}", f.message);
        // 存进 queue.json 再读回来，名字还在
        let back: Failure = serde_json::from_value(serde_json::to_value(&f).unwrap()).unwrap();
        assert_eq!(back.site.as_deref(), Some("小红书"));
    }
}
