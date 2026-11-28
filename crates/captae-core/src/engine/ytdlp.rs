//! yt-dlp 路径。先 `-J` 读信息（界面上的「读取视频信息」），再用 `--load-info-json` 下载，
//! 避免重复解析页面；进度用 `--progress-template` 输出成一行一条的机器可读格式。

use super::{Ctx, MediaInfo, Output, SpeedMeter};
use crate::errors::{self, ErrorCode, Failure};
use crate::login::Auth;
use crate::model::{AddRequest, Entry, PlaylistRef};
use crate::tr;
use crate::tools::{Tool, Toolbox};
use crate::util::{clock, mb, safe_name};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};

const TAG: &str = "CAPTAE|";

/// 读取信息最多等多久（YouTube 带 deno 平均 3 秒；慢的网站、长播放列表要久一些）
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(120);
/// 播放列表最多列多少条（频道、超长列表只列前面这些）
pub const MAX_ENTRIES: u32 = 500;
/// YouTube 默认客户端读不到公开视频时换用的一组 player_client。
/// 2026-11-23 用 yt-dlp 2026.10.26 实测（换一组客户端的做法见 docs/DESIGN.md 第 4.1 节）
pub const YT_ALT_CLIENTS: &str = "tv_simply,web_embedded";

pub fn info_from_json(v: &Value) -> MediaInfo {
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let mut subs: Vec<String> = v
        .get("subtitles")
        .and_then(Value::as_object)
        .map(|m| m.keys().filter(|k| *k != "live_chat").cloned().collect())
        .unwrap_or_default();
    subs.sort();
    MediaInfo {
        title: s("title"),
        description: s("description"),
        uploader: s("uploader").or_else(|| s("channel")),
        duration: v.get("duration").and_then(Value::as_f64),
        thumbnail: s("thumbnail"),
        webpage_url: s("webpage_url"),
        extractor: s("extractor_key").or_else(|| s("extractor")),
        width: v.get("width").and_then(Value::as_u64).map(|x| x as u32),
        height: v.get("height").and_then(Value::as_u64).map(|x| x as u32),
        ext: s("ext"),
        filesize: v.get("filesize").or_else(|| v.get("filesize_approx")).and_then(Value::as_u64),
        subtitles: subs,
        live_status: s("live_status"),
    }
}

/// 下载时要知道的视频形状：竖屏（清晰度按短边，选格式要按宽度）、单张图片（不装进 MP4）
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Shape {
    pub vertical: bool,
    pub image: bool,
}

impl Shape {
    pub fn of(info: Option<&MediaInfo>) -> Shape {
        let Some(m) = info else { return Shape::default() };
        Shape {
            vertical: matches!((m.width, m.height), (Some(w), Some(h)) if h > w),
            image: m.ext.as_deref().is_some_and(|e| crate::detect::media_kind_of_ext(e) == "image"),
        }
    }
}

/// 画质选项 → yt-dlp 格式选择器。清晰度按短边叫：竖屏视频的「1080p」是宽 1080，按宽度选
pub fn format_selector(req: &AddRequest, no_watermark: bool, vertical: bool) -> String {
    let side = if vertical { "width" } else { "height" };
    let base = if req.audio_only {
        "ba/b".to_string()
    } else {
        match req.quality.as_deref().and_then(|q| q.trim_end_matches('p').parse::<u32>().ok()) {
            Some(h) => format!("bv*[{side}<={h}]+ba/b[{side}<={h}]/bv*+ba/b"),
            None => "bv*+ba/b".to_string(),
        }
    };
    if no_watermark {
        // TikTok / 抖音提取器会给带水印的格式标 format_note=watermarked，优先选没有水印的那条
        let clean: Vec<String> = base.split('/').map(|s| format!("{s}[format_note!*=watermark]")).collect();
        format!("{}/{}", clean.join("/"), base)
    } else {
        base
    }
}

/// 每个任务自己的中间文件目录（`<保存位置>/.captae-tmp/<任务的链接算出来的名字>`）：暂停后继续还是同一个目录，能接着下；
/// 失败、取消时能整个删掉，不会碰到别的任务的（FIXES 第 6 项：失败以后 .captae-tmp 里留下 dummy.unknown_video）
pub fn temp_dir(out_dir: &Path, url: &str, item: Option<u32>) -> PathBuf {
    let key = match item {
        Some(n) => crate::util::url_key(&format!("{url}#item={n}")),
        None => crate::util::url_key(url),
    };
    out_dir.join(".captae-tmp").join(&key[..12.min(key.len())])
}

/// 构建下载参数（不含 URL）。单独拆出来方便测试。`remux`：MP4 时要不要让 yt-dlp 自己转封装（转不了时改由 Captae 转码）
#[allow(clippy::too_many_arguments)]
pub fn download_args(req: &AddRequest, out_dir: &Path, tmp: &Path, ffmpeg: Option<&Path>, use_aria2: Option<(&Path, u32)>, no_watermark: bool, shape: Shape, remux: bool) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "--newline".into(),
        "--progress".into(),
        "--no-playlist".into(),
        "--no-mtime".into(),
        "-c".into(),
        "--progress-template".into(),
        format!("download:{TAG}P|%(progress.status)s|%(progress.downloaded_bytes)s|%(progress.total_bytes)s|%(progress.total_bytes_estimate)s|%(info.format_id)s"),
        "--print".into(),
        format!("after_move:{TAG}N|%(format_note)s"),
        "--print".into(),
        format!("after_move:{TAG}F|%(filepath)s"),
        "-P".into(),
        out_dir.display().to_string(),
        "-P".into(),
        format!("temp:{}", tmp.display()),
        "-o".into(),
        "%(title).150B.%(ext)s".into(),
        "-f".into(),
        format_selector(req, no_watermark, shape.vertical),
    ];
    if let Some(f) = ffmpeg {
        a.push("--ffmpeg-location".into());
        a.push(f.display().to_string());
    }
    let fmt = req.format.as_deref().unwrap_or("MP4").to_ascii_uppercase();
    if shape.image {
        // 单张图片（picsum、Truth Social 引用帖）：照原样存，不装进 MP4（FIXES 第 8 项）
    } else if req.audio_only {
        a.push("-x".into());
        let af = match fmt.as_str() {
            "MP3" => Some("mp3"),
            "AAC" | "M4A" => Some("m4a"),
            "FLAC" => Some("flac"),
            "WAV" => Some("wav"),
            _ => None,
        };
        if let Some(af) = af {
            a.extend(["--audio-format".into(), af.into(), "--audio-quality".into(), "0".into()]);
        }
    } else {
        match fmt.as_str() {
            // 编码未知时 vcodec:h264 分不出先后，再按容器排（Archive.org 只有 mp4 240p 和 ogv 304p，选了 ogv 转不进 MP4，FIXES 第 16 项）
            "MP4" if remux => a.extend(["-S".into(), "vcodec:h264,ext:mp4:m4a,res,acodec:m4a".into(), "--merge-output-format".into(), "mp4".into(), "--remux-video".into(), "mp4".into()]),
            "MP4" => a.extend(["-S".into(), "vcodec:h264,ext:mp4:m4a,res,acodec:m4a".into()]),
            "WEBM" => a.extend(["-S".into(), "ext:webm".into(), "--merge-output-format".into(), "webm".into()]),
            "MKV" => a.extend(["--merge-output-format".into(), "mkv".into()]),
            _ => {}
        }
    }
    if let Some(n) = req.playlist_item {
        // 多视频帖子里选中的那一个（信息过期、要重新解析时才用得上）
        a.extend(["--playlist-items".into(), n.to_string()]);
    }
    if let Some(n) = req.threads {
        // DASH / HLS 分段站点：同时下几段，跟着「下载加速」走
        a.extend(["-N".into(), (n / 2).clamp(1, 8).to_string()]);
    }
    if let Some((aria, n)) = use_aria2 {
        a.extend([
            "--downloader".into(),
            format!("{}", aria.display()),
            "--downloader".into(),
            "m3u8,dash:native".into(),
            "--downloader-args".into(),
            format!("aria2c:-x{n} -s{n} -k1M"),
        ]);
    }
    for (k, v) in &req.headers {
        a.push("--add-headers".into());
        a.push(format!("{k}:{v}"));
    }
    if let Some(raw) = &req.raw_args {
        a.extend(raw.iter().cloned());
    }
    a
}

// ------------------------------------------------------------------ 读取信息

/// 读取信息要的参数。
pub struct ProbeReq<'a> {
    pub url: &'a str,
    pub headers: &'a BTreeMap<String, String>,
    /// 多视频帖子里的第几个（`--playlist-items`）
    pub item: Option<u32>,
    /// 正在直播也照样读（从现在开始录）
    pub record_live: bool,
    /// 自动重试时写一句给人看的说明（任务日志）
    pub note: &'a (dyn Fn(String) + Send + Sync),
    /// 这次要带的登录状态（没有就是 None）
    pub auth: Option<&'a Auth>,
    /// 一开始就加上的参数（YouTube 下载时 403，换一组客户端重新读：`youtube:player_client=…`）
    pub extra: Vec<String>,
}

/// 读取信息的结果。
#[derive(Debug, Clone)]
pub enum Probed {
    /// 单个视频。`raw` 是 yt-dlp 的原始 JSON（下载时 `--load-info-json` 用）；
    /// `extra` 是读取时额外加过的参数（YouTube 换了客户端），下载时照用
    Video { info: MediaInfo, raw: Value, playlist: Option<PlaylistRef>, extra: Vec<String> },
    /// 播放列表、多视频帖子：等用户选条目
    List { title: Option<String>, entries: Vec<Entry> },
}

pub fn is_youtube(url: &str) -> bool {
    errors::site_name(url) == Some("YouTube")
}

/// YouTube 链接里的视频 id（watch?v=、youtu.be/、shorts/、live/、embed/）。
pub fn youtube_id(url: &str) -> Option<String> {
    let u = url::Url::parse(url.trim()).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    let ok = |s: &str| (s.len() == 11 && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')).then(|| s.to_string());
    if host == "youtu.be" {
        return ok(u.path_segments()?.next()?);
    }
    if !(host == "youtube.com" || host.ends_with(".youtube.com") || host.ends_with("youtube-nocookie.com")) {
        return None;
    }
    if let Some((_, v)) = u.query_pairs().find(|(k, _)| k == "v") {
        return ok(&v);
    }
    let mut seg = u.path_segments()?;
    match seg.next()? {
        "shorts" | "live" | "embed" | "v" => ok(seg.next()?),
        _ => None,
    }
}

/// YouTube `watch?v=…&list=…`：这个视频属于哪个播放列表。
pub fn youtube_list_of(url: &str) -> Option<PlaylistRef> {
    youtube_id(url)?;
    let u = url::Url::parse(url.trim()).ok()?;
    let id = u.query_pairs().find(|(k, _)| k == "list").map(|(_, v)| v.to_string())?;
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return None;
    }
    Some(PlaylistRef { url: format!("https://www.youtube.com/playlist?list={id}"), id, count: None, index: None })
}

/// B 站多 P 视频：链接里没写 `?p=` 时把各 P 都列出来让用户选（FIXES 第 18 项）；写了就只读那一 P
pub fn bilibili_parts(url: &str) -> bool {
    let Ok(u) = url::Url::parse(url.trim()) else { return false };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    (host == "bilibili.com" || host.ends_with(".bilibili.com")) && u.path().starts_with("/video/") && !u.query_pairs().any(|(k, _)| k == "p")
}

/// B 站 `…/video/BV…?p=2`：这一 P 属于哪个分 P 视频（整套的地址就是去掉 p 参数）
pub fn bilibili_set_of(url: &str) -> Option<PlaylistRef> {
    let mut u = url::Url::parse(url.trim()).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    if !(host == "bilibili.com" || host.ends_with(".bilibili.com")) || !u.path().starts_with("/video/") {
        return None;
    }
    let p = u.query_pairs().find(|(k, _)| k == "p").map(|(_, v)| v.to_string())?;
    let index = p.parse::<u32>().ok()?;
    let id = u.path().trim_start_matches("/video/").trim_end_matches('/').to_string();
    let rest: Vec<(String, String)> = u.query_pairs().filter(|(k, _)| k != "p").map(|(k, v)| (k.to_string(), v.to_string())).collect();
    u.set_query(None);
    if !rest.is_empty() {
        u.query_pairs_mut().extend_pairs(rest);
    }
    Some(PlaylistRef { id, url: u.to_string(), count: None, index: Some(index) })
}

/// B 站写了 `?p=` 的链接：再读一次分 P 列表，知道一共几 P（界面上写「第 2 P，共 5 P」，可以去选别的）。
/// 只有 1 P 的（分享链接常带 `?p=1`）不算分 P 视频；列表读不到时照旧，只是不写共几 P
pub async fn bilibili_set(http: &reqwest::Client, url: &str) -> Option<PlaylistRef> {
    let mut set = bilibili_set_of(url)?;
    let api = format!("https://api.bilibili.com/x/player/pagelist?bvid={}", set.id);
    let v: Option<Value> = async { http.get(&api).timeout(Duration::from_secs(10)).send().await.ok()?.json::<Value>().await.ok() }.await;
    set.count = v.as_ref().and_then(pagelist_count);
    (set.count != Some(1)).then_some(set)
}

/// B 站分 P 列表接口（`x/player/pagelist`）回来的 JSON → 一共几 P
pub fn pagelist_count(v: &Value) -> Option<u32> {
    if v.get("code").and_then(Value::as_i64).unwrap_or(0) != 0 {
        return None;
    }
    let n = v.get("data")?.as_array()?.len() as u32;
    (n > 0).then_some(n)
}

/// YouTube oEmbed：Some(true) 公开视频，Some(false) 不公开或不存在，None 没问到（网络问题）。
pub async fn youtube_public(http: &reqwest::Client, url: &str) -> Option<bool> {
    let id = youtube_id(url)?;
    let watch = format!("https://www.youtube.com/watch?v={id}");
    let api = url::Url::parse_with_params("https://www.youtube.com/oembed", &[("url", watch.as_str()), ("format", "json")]).ok()?;
    let r = http.get(api).timeout(Duration::from_secs(15)).send().await.ok()?;
    match r.status().as_u16() {
        200 => Some(true),
        400 | 401 | 403 | 404 => Some(false),
        _ => None,
    }
}

/// YouTube 按会话做实验时的几种报错（DIAGNOSIS 第 1 节）：公开视频也可能这样，换一组客户端常常就好了。
pub(crate) fn youtube_flaky(f: &Failure) -> bool {
    if f.code == ErrorCode::Tool {
        return false;
    }
    let r = f.raw.to_lowercase();
    ["this video is not available", "requested format is not available", "no video formats found", "page needs to be reloaded", "video unavailable"].iter().any(|k| r.contains(k))
}

async fn run_probe(tools: &Toolbox, url: &str, headers: &BTreeMap<String, String>, item: Option<u32>, extra: &[String], auth: Option<&Auth>) -> std::result::Result<Value, Failure> {
    // 登录状态：临时 cookies 文件只给这一次的 yt-dlp 用，这个函数返回（或者被取消）时 `_cookies` 被丢掉，文件就删了
    let (auth_args, _cookies) = match auth {
        Some(a) => a.ytdlp().map_err(|e| errors::from_anyhow(&e, url))?,
        None => (vec![], None),
    };
    let mut c = tools.command(Tool::YtDlp).map_err(|e| errors::from_anyhow(&e, url))?;
    c.arg("-J");
    // B 站多 P：yt-dlp 把分 P 当播放列表，加了 --no-playlist 就只给第 1 P
    if !bilibili_parts(url) {
        c.arg("--no-playlist");
    }
    match item {
        Some(n) => {
            c.arg("--playlist-items").arg(n.to_string());
        }
        // 播放列表只列条目（标题、时长、缩略图），不逐个解析，快很多
        None => {
            c.args(["--flat-playlist", "--playlist-end"]).arg(MAX_ENTRIES.to_string());
        }
    }
    for (k, v) in headers {
        c.arg("--add-headers").arg(format!("{k}:{v}"));
    }
    c.args(auth_args).args(extra).arg(url);
    let r = crate::tools::run_full(c, PROBE_TIMEOUT).await?;
    // 报错进任务记录、错误原文、AI 助手之前，先把 cookie 的值抹掉
    let stderr = match auth {
        Some(a) => a.redact(&r.stderr),
        None => r.stderr,
    };
    if !r.ok {
        return Err(errors::classify(&stderr, url));
    }
    serde_json::from_slice(&r.stdout).map_err(|e| Failure::new(ErrorCode::Tool, tr!("下载组件返回的信息读不了"), format!("{}\n{stderr}", tr!("yt-dlp 返回的 JSON 无法解析：{error}", error = e))))
}

/// yt-dlp `-J` 的结果里，每个格式的 `cookies` 字段是这个格式适用的 cookies（明文，见 YoutubeDL._calc_headers）。
/// 结果要存成 info.json 给下载用，存之前去掉其中我们带进去的登录 cookie（按值，或按名字加域名认）；
/// 网站在读取过程中自己种下的留着（有的网站下载时要用）。下载时会重新带上临时 cookies 文件。
pub fn strip_auth_cookies(v: &mut Value, auth: &Auth) {
    if auth.cookies.is_empty() {
        return;
    }
    match v {
        Value::Object(m) => {
            if let Some(Value::String(s)) = m.get("cookies") {
                let kept = filter_cookie_field(s, auth);
                if kept.is_empty() {
                    m.remove("cookies");
                } else {
                    m.insert("cookies".into(), Value::String(kept));
                }
            }
            if let Some(Value::Object(h)) = m.get_mut("http_headers") {
                h.retain(|k, _| !k.eq_ignore_ascii_case("cookie"));
            }
            for (_, x) in m.iter_mut() {
                strip_auth_cookies(x, auth);
            }
        }
        Value::Array(a) => {
            for x in a {
                strip_auth_cookies(x, auth);
            }
        }
        _ => {}
    }
}

/// `"SESSDATA=v; Domain=.bilibili.com; Path=/; Secure; Expires=1; buvid3=x; Domain=.bilibili.com"` → 去掉登录状态里的那几个
fn filter_cookie_field(s: &str, auth: &Auth) -> String {
    const ATTRS: &[&str] = &["domain", "path", "expires", "max-age", "secure", "httponly", "version", "samesite", "comment"];
    let mut groups: Vec<Vec<&str>> = vec![];
    for tok in s.split(';').map(str::trim).filter(|t| !t.is_empty()) {
        let key = tok.split('=').next().unwrap_or("").trim().to_ascii_lowercase();
        match groups.last_mut() {
            Some(g) if ATTRS.contains(&key.as_str()) => g.push(tok),
            _ => groups.push(vec![tok]),
        }
    }
    groups
        .into_iter()
        .filter(|g| {
            let (name, val) = g[0].split_once('=').unwrap_or((g[0], ""));
            let val = val.trim().trim_matches('"');
            let dom = g.iter().find_map(|t| t.split_once('=').filter(|(k, _)| k.trim().eq_ignore_ascii_case("domain")).map(|(_, d)| d.trim().trim_start_matches('.')));
            !auth.cookies.iter().any(|c| (c.value.len() >= 6 && c.value == val) || (c.name == name.trim() && dom.is_some_and(|d| d.eq_ignore_ascii_case(c.bare_domain()))))
        })
        .map(|g| g.join("; "))
        .collect::<Vec<_>>()
        .join("; ")
}

/// 读取视频信息：超时、YouTube 换客户端重试、网络问题重试一次、播放列表列条目、直播判断，都在这里。
// ------------------------------------------------------------------ 读取结果的缓存（手机端：读取不做两遍）
//
// 手机上输入卡先「读取信息」（probe.rs，界面上有流光），用户点下载以后队列的读取阶段又调一次 yt-dlp，YouTube 一次要 20 多秒。
// 所以手机上把 yt-dlp 读到的结果按「链接 + 条目 + 是否录直播 + 请求头」记 10 分钟（和 probe.rs 的缓存一样长），
// 加任务时同一条链接直接用（下载照旧 `--load-info-json` 这份 JSON，等于原来读第二遍的结果）。
// 带了登录状态（cookies）或额外参数（YouTube 换客户端重读）的不记、也不用：登录以后能看到的不一样，也不能把登录状态留在内存里。
// 桌面端（DesktopRunner）不开，行为和原来一样。

const PROBE_TTL: Duration = Duration::from_secs(600);

type ProbeCache = std::sync::Mutex<HashMap<String, (std::time::Instant, Probed)>>;
static PROBE_CACHE: std::sync::LazyLock<ProbeCache> = std::sync::LazyLock::new(Default::default);

/// 缓存的键；这次不该用缓存时 None
fn probe_cache_key(tools: &Toolbox, r: &ProbeReq<'_>) -> Option<String> {
    if tools.runner.kind() == "desktop" || r.auth.is_some_and(|a| !a.is_empty()) || !r.extra.is_empty() {
        return None;
    }
    let headers = serde_json::to_string(r.headers).unwrap_or_default();
    Some(format!("{}\n{:?}\n{}\n{headers}", crate::detect::canonical(r.url), r.item, r.record_live))
}

fn probe_cached(key: &str) -> Option<Probed> {
    let c = PROBE_CACHE.lock().unwrap();
    c.get(key).filter(|(t, _)| t.elapsed() < PROBE_TTL).map(|(_, v)| v.clone())
}

fn probe_remember(key: String, v: &Probed) {
    let mut c = PROBE_CACHE.lock().unwrap();
    c.retain(|_, (t, _)| t.elapsed() < PROBE_TTL);
    if c.len() > 100 {
        c.clear();
    }
    c.insert(key, (std::time::Instant::now(), v.clone()));
}

/// 用 yt-dlp 读一个链接的信息（`-J`）。手机上 10 分钟内读过同一条链接（同样的条目、请求头，没带登录状态）直接用上次的结果，见上面
pub async fn probe_url(tools: &Toolbox, http: &reqwest::Client, r: &ProbeReq<'_>) -> std::result::Result<Probed, Failure> {
    let key = probe_cache_key(tools, r);
    if let Some(v) = key.as_deref().and_then(probe_cached) {
        tracing::info!("10 分钟内读过 {}，直接用上次读到的信息", r.url);
        return Ok(v);
    }
    let v = probe_url_uncached(tools, http, r).await?;
    if let Some(k) = key {
        probe_remember(k, &v);
    }
    Ok(v)
}

async fn probe_url_uncached(tools: &Toolbox, http: &reqwest::Client, r: &ProbeReq<'_>) -> std::result::Result<Probed, Failure> {
    if errors::is_wechat_channels(r.url) {
        return Err(errors::wechat_channels(r.url));
    }
    // 跳转包装先拆开：YouTube 的 /redirect?q=、lnkd.in 的提示页……（FIXES 第 11 项）
    let start = match crate::detect::unwrap_remote(http, r.url).await {
        Some(t) => {
            (r.note)(tr!("这是跳转链接，目标是 {target}", target = t));
            t
        }
        None => crate::detect::canonical(r.url),
    };
    // 抖音的视频：yt-dlp 读不了，从看不见的网页里读（只有桌面端有，见 douyin.rs）
    if let Some(res) = crate::douyin::probe(http, &start, r.note).await {
        return res;
    }
    let mut url = start.clone();
    let mut extra: Vec<String> = r.extra.clone();
    let mut tried_clients = extra.iter().any(|a| a.contains("player_client"));
    let (mut tried_again, mut followed, mut rolled_back) = (false, false, false);
    let mut channel: Option<String> = None;
    let mut public = None;
    loop {
        let res = run_probe(tools, &url, r.headers, r.item, &extra, r.auth).await;
        // 用的是自动更新的新版：出错时可能当场体检并退回上一个版本，退回了就用旧版再试一次。
        // 「下载引擎跟不上网站」以前算 unknown，体检照旧把它算作引擎出错（新版把某个提取器改坏了也是这样报）
        let outcome = res.as_ref().map(|_| ()).map_err(|f| if errors::is_engine_lag(f) { ErrorCode::Unknown } else { f.code });
        if crate::updater::note_result(tools, outcome).await && !rolled_back {
            rolled_back = true;
            (r.note)(tr!("下载引擎的新版出了问题，已经退回上一个版本，再试一次"));
            continue;
        }
        let mut f = match res {
            Ok(v) => match interpret(v, &url, r.item, r.record_live)? {
                Interp::Done(Probed::Video { mut info, mut raw, .. }) => {
                    // yt-dlp 没说出来的几样：试看片段、正在直播、不是媒体的直链、付费电影的预告片。
                    // 付费电影按这次实际读的地址比视频 id（跟过一层链接时，用户贴的地址里的 id 不算数）
                    if vet(http, &start, &url, &raw, r.record_live).await? {
                        // 要从现在开始录的直播：告诉 yt-dlp 这是直播（通用提取器找到的直播 HLS 它自己认不出来），
                        // 它才会交给 ffmpeg 一直录下去，收到「停止」（SIGINT）时当作正常结束、照常整理
                        mark_live(&mut raw);
                        info.live_status = Some("is_live".into());
                    }
                    let playlist = match youtube_list_of(&start) {
                        Some(p) => Some(p),
                        None => bilibili_set(http, &start).await,
                    };
                    return Ok(Probed::Video { info, raw, playlist, extra });
                }
                Interp::Done(Probed::List { title, mut entries }) => {
                    if bilibili_parts(&url) {
                        name_bilibili_parts(http, &url, &mut entries).await;
                    }
                    let title = match &channel {
                        Some(c) => Some(tr!("{channel} · 频道里最新的视频", channel = c)),
                        None => title,
                    };
                    return Ok(Probed::List { title, entries });
                }
                // YouTube 频道主页：「条目」是 Videos / Live / Shorts 几个标签页，改读「视频」标签（FIXES 第 14 项）
                Interp::Tab { url: u, channel: c, label } if !followed => {
                    followed = true;
                    (r.note)(tr!("这是频道主页，列出「{tab}」标签里最新的视频（{url}）", tab = tab_name(label), url = u));
                    channel = Some(c);
                    url = u;
                    continue;
                }
                // 只有一条、而且是个链接（还没解析）：接着读那条
                Interp::Follow(u) if !followed => {
                    followed = true;
                    url = u;
                    continue;
                }
                Interp::Follow(u) | Interp::Tab { url: u, .. } => return Err(Failure::new(ErrorCode::NoMedia, tr!("这个网页里没有找到视频或音频"), format!("playlist entry is only a link: {u}"))),
            },
            Err(f) => f,
        };
        // Dailymotion 的报错按出口地区翻译了（印尼语「Konten pribadi」「Saluran offline」），看接口里的错误号
        if f.raw.contains("dailymotion said:") {
            if let Some(g) = dailymotion_failure(http, &url, &f).await {
                f = g;
            }
        }
        // YouTube：「不可用」一类先问 oEmbed 是不是公开视频，公开的换一组客户端再试一次
        if is_youtube(&start) && !tried_clients && youtube_flaky(&f) {
            tried_clients = true;
            public = youtube_public(http, &start).await;
            if public == Some(true) {
                (r.note)(tr!("YouTube 没给出视频（{reason}），视频是公开的，换一组客户端（{clients}）再试一次", reason = short(&f), clients = YT_ALT_CLIENTS));
                extra = alt_clients();
                continue;
            }
        }
        // 网络问题、说不清的，和「下载引擎跟不上网站」（以前算 unknown，照旧再试一次）
        if !tried_again && (f.code.transient() || errors::is_engine_lag(&f)) {
            tried_again = true;
            (r.note)(tr!("读取失败（{reason}），2 秒后自动重试", reason = short(&f)));
            tokio::time::sleep(Duration::from_secs(2)).await;
            continue;
        }
        // 公开视频换了客户端还是读不到：YouTube 这次不给，过一会儿多半就好了
        if public == Some(true) && youtube_flaky(&f) {
            // 说法换了，原来的细分和短说法不再对得上
                f = Failure { code: ErrorCode::RateLimited, message: tr!("YouTube 这次没给出视频（视频是公开的），稍后自动重试"), reason: None, brief: None, ..f };
        }
        return Err(f);
    }
}

/// YouTube 换一组 player_client 的参数（读取时 A/B 实验、下载时 403 都用这一组）
pub fn alt_clients() -> Vec<String> {
    vec!["--extractor-args".into(), format!("youtube:player_client={YT_ALT_CLIENTS}")]
}

// ------------------------------------------------------------------ 读到以后再看一遍

/// 读到了单个视频，再看几样 yt-dlp 没说出来的（FIXES 第 6、7、10、13 项）。
/// `url` 是用户给的地址（理顺过的），`probed` 是这次交给 yt-dlp 读的地址（跟过一层链接时两个不一样）。
/// 返回这是不是要从现在开始录的直播（`record_live` 而且确实在直播）
async fn vet(http: &reqwest::Client, url: &str, probed: &str, v: &Value, record_live: bool) -> std::result::Result<bool, Failure> {
    if let Some(f) = youtube_paid(probed, v) {
        return Err(f);
    }
    if let Some(f) = preview(url, v) {
        return Err(f);
    }
    if v.get("direct").and_then(Value::as_bool) == Some(true) {
        if let Some(f) = direct_not_media(http, url, v).await {
            return Err(f);
        }
    }
    let live = match (record_live, v.get("live_status").and_then(Value::as_str) == Some("is_live")) {
        (true, true) => true,
        (true, false) => live_guess(http, v).await.is_some(),
        (false, _) => {
            if let Some(f) = live_guess(http, v).await {
                return Err(f);
            }
            false
        }
    };
    if live {
        return Ok(true);
    }
    if let Some(f) = vip_preview(http, url, v).await {
        return Err(f);
    }
    if let Some(f) = audius_gated(http, v).await {
        return Err(f);
    }
    Ok(false)
}

/// 标成正在直播（存进 info.json，下载时 `--load-info-json` 读的就是它）
pub fn mark_live(v: &mut Value) {
    if let Some(o) = v.as_object_mut() {
        o.insert("is_live".into(), Value::Bool(true));
        o.insert("live_status".into(), Value::String("is_live".into()));
        o.remove("duration");
        // yt-dlp 每处理一次正在直播的信息，就在标题后面加一次「 2026-11-24 06:11」（YoutubeDL.process_video_result）：
        // 读取（-J）时已经加过，下载（--load-info-json）时还会再加一次，文件名里就有两个日期。存盘前去掉读取时加的那个
        if let Some(t) = o.get("title").and_then(Value::as_str).map(strip_live_date) {
            o.insert("title".into(), Value::String(t));
        }
    }
}

/// 去掉标题末尾 yt-dlp 给直播加的日期（「 2026-11-24 06:11」）
pub fn strip_live_date(t: &str) -> String {
    static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r" \d{4}-\d{2}-\d{2} \d{2}:\d{2}$").unwrap());
    RE.replace(t, "").to_string()
}

/// Audius 付费解锁的曲目（页面写着 PAY TO UNLOCK）：yt-dlp 给的是完整时长，真去下才 403（FIXES 第 2 项 audius-9d89b6）。
/// 读一次接口：`access.stream` 是 false 就是没买
async fn audius_gated(http: &reqwest::Client, v: &Value) -> Option<Failure> {
    if v.get("extractor_key").and_then(Value::as_str) != Some("Audius") {
        return None;
    }
    let id = v.get("id").and_then(Value::as_str)?;
    let api = format!("https://api.audius.co/v1/tracks/{id}");
    let d: Value = http.get(&api).timeout(Duration::from_secs(15)).send().await.ok()?.json().await.ok()?;
    let d = d.get("data")?;
    let no_access = d.get("access").and_then(|a| a.get("stream")).and_then(Value::as_bool) == Some(false);
    let gated = d.get("is_stream_gated").and_then(Value::as_bool) == Some(true) && d.get("stream_conditions").is_some_and(|c| !c.is_null());
    let locked = no_access || gated;
    locked.then(|| Failure::new(ErrorCode::NeedLogin, tr!("这首歌要付费解锁，没买只能在网页上听预览；完整版要登录买过的 Audius 账号"), format!("{api}: is_stream_gated, access.stream=false")))
}

/// 格式自己带的请求头（yt-dlp 读到的 User-Agent、Referer……），Captae 自己去读格式地址时照用
fn headers_of(v: &Value) -> BTreeMap<String, String> {
    v.get("http_headers").and_then(Value::as_object).map(|m| m.iter().filter_map(|(k, x)| x.as_str().map(|x| (k.clone(), x.to_string()))).collect()).unwrap_or_default()
}

/// 「30 秒」「5 分钟」「2 分 27 秒」
fn human_len(secs: f64) -> String {
    let s = secs.round().max(1.0) as u64;
    match (s / 60, s % 60) {
        (0, x) => tr!("{n} 秒", n = x),
        (m, 0) => tr!("{n} 分钟", n = m),
        (m, x) => tr!("{m} 分 {s} 秒", m = m, s = x),
    }
}

/// 会员、付费内容没登录只拿到试看片段：算失败（need_login），不进素材库（用户定的做法，FIXES 第 7 项）
pub fn preview_failure(url: &str, secs: Option<f64>, audio: bool, raw: String) -> Failure {
    let site = errors::site_name(url);
    // 知不知道片段多长、是试听还是试看、认不认得网站：各是一整句，不拼片段（docs/I18N.md 2.3）
    let message = match (site.map(crate::i18n::dynamic), secs.map(human_len), audio) {
        (Some(site), Some(len), false) => tr!("这是会员内容，没登录只能下到 {len}的试看片段，完整版要登录开了会员的{site}账号", len = len, site = site),
        (Some(site), Some(len), true) => tr!("这是会员内容，没登录只能下到 {len}的试听片段，完整版要登录开了会员的{site}账号", len = len, site = site),
        (Some(site), None, false) => tr!("这是会员内容，没登录只能下到试看片段，完整版要登录开了会员的{site}账号", site = site),
        (Some(site), None, true) => tr!("这是会员内容，没登录只能下到试听片段，完整版要登录开了会员的{site}账号", site = site),
        (None, Some(len), false) => tr!("这是会员内容，没登录只能下到 {len}的试看片段，完整版要登录开了会员的账号", len = len),
        (None, Some(len), true) => tr!("这是会员内容，没登录只能下到 {len}的试听片段，完整版要登录开了会员的账号", len = len),
        (None, None, false) => tr!("这是会员内容，没登录只能下到试看片段，完整版要登录开了会员的账号"),
        (None, None, true) => tr!("这是会员内容，没登录只能下到试听片段，完整版要登录开了会员的账号"),
    };
    let mut f = Failure::new(ErrorCode::NeedLogin, message, raw);
    // 「登录芒果TV」按钮和这句话用同一个名字（存中文原文，界面显示时自己翻）
    f.site = site.map(str::to_string);
    f
}

/// YouTube 付费电影（购买、租借）：yt-dlp 读到的是它的预告片，视频 id 和链接里的不一样（youtube-f7f8d2：TrD-Woa_8Rk → XbR2BJZyaRo）。
/// 买了以后也是 Widevine 加密的，登录了也下不了，所以直接说 DRM（这一点和其他会员试看不同，见 FIXLOG）
fn youtube_paid(url: &str, v: &Value) -> Option<Failure> {
    let want = youtube_id(url)?;
    let got = v.get("id").and_then(Value::as_str)?;
    if got == want || v.get("extractor_key").and_then(Value::as_str) != Some("Youtube") {
        return None;
    }
    let message = match v.get("duration").and_then(Value::as_f64) {
        Some(d) => tr!("这是付费电影，没购买只能下到 {len}的预告片；完整版受版权保护（DRM），Captae 不能下载", len = human_len(d)),
        None => tr!("这是付费电影，没购买只能下到预告片；完整版受版权保护（DRM），Captae 不能下载"),
    };
    Some(Failure::new(ErrorCode::Drm, message, format!("youtube paid content: {want} → trailer {got}")))
}

/// 读取时就认得出来的试看、试听片段：
/// - 每个格式都标着 preview / sample（SoundCloud Go+ 的 `http_mp3_1_0_preview`、喜马拉雅的 `format_note: Sample`）
/// - 只有声音、大小和码率算出来的长度远短于时长（网易云 VIP 歌：320 秒，给的流 48 万字节、128 kbps，只有 30 秒）
fn preview(url: &str, v: &Value) -> Option<Failure> {
    let formats: Vec<&Value> = match v.get("formats").and_then(Value::as_array) {
        Some(a) if !a.is_empty() => a.iter().collect(),
        _ => vec![v],
    };
    let low = |f: &Value, k: &str| f.get(k).and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    let is_prev = |f: &Value| {
        let (id, note, u) = (low(f, "format_id"), low(f, "format_note"), low(f, "url"));
        id.contains("preview") || matches!(note.as_str(), "sample" | "preview" | "trial") || u.contains("/preview/") || u.contains("_preview")
    };
    let dur = v.get("duration").and_then(Value::as_f64).filter(|d| *d > 0.0);
    let audio = low(v, "vcodec") == "none" || formats.iter().all(|f| low(f, "vcodec") == "none");
    if formats.iter().all(|f| is_prev(f)) {
        return Some(preview_failure(url, dur, audio, format!("all formats are previews: {}", formats.iter().map(|f| low(f, "format_id")).collect::<Vec<_>>().join(","))));
    }
    if audio {
        let size = v.get("filesize").and_then(Value::as_f64).filter(|x| *x > 0.0)?;
        let rate = v.get("abr").or_else(|| v.get("tbr")).and_then(Value::as_f64).filter(|x| *x > 0.0)?;
        let d = dur?;
        let secs = size * 8.0 / (rate * 1000.0);
        if d >= 60.0 && secs < d * 0.5 && d - secs >= 20.0 {
            return Some(preview_failure(url, Some(secs), true, format!("stream is {secs:.0}s ({size} bytes at {rate} kbps), duration says {d}s")));
        }
    }
    None
}

/// 会员试看的网站，分片列表的总时长对不上节目时长就是试看（芒果 TV：标 80 分钟，分片一共 5 分钟）。只多读一个分片列表
async fn vip_preview(http: &reqwest::Client, url: &str, v: &Value) -> Option<Failure> {
    const VIP: &[&str] = &["mgtv.com", "iqiyi.com", "v.qq.com", "youku.com", "miguvideo.com", "tv.sohu.com", "le.com"];
    let host = url::Url::parse(url).ok()?.host_str()?.trim_start_matches("www.").to_ascii_lowercase();
    if !VIP.iter().any(|d| host == *d || host.ends_with(&format!(".{d}"))) {
        return None;
    }
    let d = v.get("duration").and_then(Value::as_f64).filter(|d| *d >= 60.0)?;
    if !v.get("protocol").and_then(Value::as_str).unwrap_or("").starts_with("m3u8") {
        return None;
    }
    let furl = v.get("url").and_then(Value::as_str)?;
    let m = super::hls::media_of(http, furl, &headers_of(v)).await.ok()?;
    let total = m.duration();
    (m.endlist && total > 0.0 && total < d * 0.6 && d - total >= 30.0).then(|| preview_failure(url, Some(total), false, format!("segments add up to {total:.0}s, duration says {d}s ({furl})")))
}

/// 文件开头几个字节是什么
#[derive(Debug, PartialEq)]
pub enum Magic {
    Media,
    /// 不是媒体：PDF、压缩包、网页、JSON、文本
    Not(&'static str),
    Unknown,
}

pub fn magic(b: &[u8]) -> Magic {
    let head = |p: &[u8]| b.starts_with(p);
    if (b.len() >= 8 && &b[4..8] == b"ftyp") || head(&[0x1a, 0x45, 0xdf, 0xa3]) || head(b"ID3") || head(b"OggS") || head(b"fLaC") || head(b"RIFF") || head(b"FLV") || head(b"#EXTM3U") {
        return Magic::Media;
    }
    if b.len() >= 2 && b[0] == 0xff && (b[1] & 0xe0) == 0xe0 {
        // MPEG 音频帧、ADTS
        return Magic::Media;
    }
    if head(&[0xff, 0xd8, 0xff]) || head(&[0x89, b'P', b'N', b'G']) || head(b"GIF8") || (b.len() >= 12 && &b[8..12] == b"WEBP") {
        return Magic::Media;
    }
    if head(b"%PDF") {
        return Magic::Not("PDF 文档");
    }
    if head(b"PK\x03\x04") || head(&[0x1f, 0x8b]) || head(b"7z\xbc\xaf") || head(b"Rar!") {
        return Magic::Not("压缩包");
    }
    let t = String::from_utf8_lossy(b).trim_start().to_ascii_lowercase();
    if t.starts_with("<!doctype") || t.starts_with("<html") || t.starts_with("<?xml") || t.starts_with("<head") {
        return Magic::Not("网页");
    }
    if t.starts_with('{') || t.starts_with('[') {
        return Magic::Not("一段数据（JSON）");
    }
    if !b.is_empty() && std::str::from_utf8(b).is_ok() && b.iter().all(|c| !matches!(c, 0..=8 | 14..=31)) {
        return Magic::Not("文本");
    }
    Magic::Unknown
}

/// `Magic::Not` 里的种类（中文原文，测试按它比较）→ 放进说法里的名字
fn kind_label(what: &str) -> String {
    match what {
        "PDF 文档" => tr!("PDF 文档"),
        "压缩包" => tr!("压缩包"),
        "网页" => tr!("网页§文件的种类，放在句中"),
        "一段数据（JSON）" => tr!("一段数据（JSON）"),
        "文本" => tr!("文本§文件的种类，放在句中"),
        x => x.to_string(),
    }
}

/// 通用提取器把不是网页的地址一律当成直链视频（`direct: true`，ext 多半是 unknown_video）：看一眼文件开头（FIXES 第 6 项）
async fn direct_not_media(http: &reqwest::Client, url: &str, v: &Value) -> Option<Failure> {
    let ext = v.get("ext").and_then(Value::as_str).unwrap_or("");
    if matches!(crate::detect::media_kind_of_ext(ext), "video" | "audio" | "image") {
        return None;
    }
    let target = v.get("url").and_then(Value::as_str).unwrap_or(url);
    let mut req = http.get(target).header("Range", "bytes=0-63").timeout(Duration::from_secs(15));
    for (k, x) in headers_of(v) {
        if !k.eq_ignore_ascii_case("accept-encoding") {
            req = req.header(k, x);
        }
    }
    let resp = req.send().await.ok()?;
    use futures_util::StreamExt;
    let mut s = resp.bytes_stream();
    let mut b = Vec::new();
    while let Ok(Some(Ok(c))) = tokio::time::timeout(Duration::from_secs(10), s.next()).await {
        b.extend_from_slice(&c);
        if b.len() >= 64 {
            break;
        }
    }
    b.truncate(64);
    let Magic::Not(what) = magic(&b) else { return None };
    let kind = kind_label(what);
    let raw = format!("generic direct link is not media ({kind}, ext={ext}): {target}");
    // 失效的短链接（b23.tv 不跳转，回一段「啥都木有」的 JSON）
    let short = url::Url::parse(url).ok().and_then(|u| u.host_str().map(str::to_string)).is_some_and(|h| ["b23.tv", "v.douyin.com", "xhslink.com", "t.co", "bit.ly", "fb.watch", "vm.tiktok.com"].contains(&h.as_str()));
    Some(if short { Failure::new(ErrorCode::Unavailable, tr!("这个短链接已经失效了"), raw) } else { Failure::new(ErrorCode::NoMedia, tr!("这个链接是一个文件（{kind}），不是视频或音频", kind = kind), raw) })
}

/// 没有 live_status、也没有时长的直播（FIXES 第 10、13 项）：提取器名字带 Live（虎牙）、HLS 分段列表没有结尾（OK.ru、DW 官网直播页）、
/// 通用提取器的音频直链是电台流（Radio Paradise）
async fn live_guess(http: &reqwest::Client, v: &Value) -> Option<Failure> {
    if v.get("duration").and_then(Value::as_f64).is_some_and(|d| d > 0.0) {
        return None;
    }
    let live = |why: String| Some(Failure::new(ErrorCode::Live, tr!("这是正在进行的直播"), why));
    if v.get("is_live").and_then(Value::as_bool) == Some(true) {
        return live("is_live=true".into());
    }
    if live_extractor(v) {
        let ek = v.get("extractor_key").and_then(Value::as_str).unwrap_or("");
        return live(format!("no duration, extractor {ek}"));
    }
    let proto = v.get("protocol").and_then(Value::as_str).unwrap_or("");
    let furl = v.get("url").and_then(Value::as_str)?;
    if proto.starts_with("m3u8") {
        let m = super::hls::media_of(http, furl, &headers_of(v)).await.ok()?;
        return if m.endlist { None } else { live(format!("no duration, HLS playlist has no #EXT-X-ENDLIST ({furl})")) };
    }
    let audio = v.get("vcodec").and_then(Value::as_str) == Some("none") || v.get("video_ext").and_then(Value::as_str) == Some("none");
    if v.get("direct").and_then(Value::as_bool) == Some(true) && audio && proto.starts_with("http") {
        let h = super::aria2::head(http, furl, &headers_of(v), &crate::login::Jar::default()).await.ok()?;
        if h.is_stream() {
            return Some(errors::radio(format!("audio stream without length (icy={}): {furl}", h.icy)));
        }
    }
    None
}

/// 提取器本身是看直播的（虎牙的 HuyaLive / huya:live）。只认名字的一节是 live，「Olive」「Delivery」这种不算
fn live_extractor(v: &Value) -> bool {
    let key = v.get("extractor_key").and_then(Value::as_str).unwrap_or("");
    let name = v.get("extractor").and_then(Value::as_str).unwrap_or("").to_ascii_lowercase();
    key.ends_with("Live") || name.split([':', ' ', '_', '-']).any(|p| p == "live")
}

/// Dailymotion 只把接口的 error.title 放进报错（还按出口地区翻译了），读一次接口拿错误号。
/// 要带上 `embedder`（视频页地址）和 `locale=en`：不带时接口一律回 DM016「不能在这个网站上看」，和 yt-dlp 看到的不一样
/// （2026-11-24 实测 x8qckyq：不带是 DM016，带上是 DM020 私密；x8ge5un 是 DM003 频道没开播）
async fn dailymotion_failure(http: &reqwest::Client, url: &str, f: &Failure) -> Option<Failure> {
    let u = url::Url::parse(url).ok()?;
    let xid = u.path_segments()?.skip_while(|s| *s != "video").nth(1)?.split('_').next()?.to_string();
    let api = format!("https://www.dailymotion.com/player/metadata/video/{xid}");
    let page = format!("https://www.dailymotion.com/video/{xid}");
    let v: Value = http.get(&api).query(&[("embedder", page.as_str()), ("locale", "en")]).timeout(Duration::from_secs(15)).send().await.ok()?.json().await.ok()?;
    let e = v.get("error")?;
    let code = e.get("code").and_then(Value::as_str)?;
    let title = e.get("title").and_then(Value::as_str).unwrap_or("");
    let detail = e.get("raw_message").and_then(Value::as_str).unwrap_or("");
    let g = dailymotion_said(code, title, detail, url)?;
    let mut n = Failure::new(g.code, g.message, format!("{}\n{api}: {code} {title} {detail}", f.raw));
    n.reason = g.reason;
    n.brief = g.brief;
    Some(n)
}

/// Dailymotion 接口的错误号 → 错误码和说法。没见过的号按英文的 title / raw_message 归类，归不了就 None（照用 yt-dlp 的报错）
pub fn dailymotion_code(code: &str, title: &str, detail: &str, url: &str) -> Option<(ErrorCode, String)> {
    dailymotion_said(code, title, detail, url).map(|f| (f.code, f.message))
}

/// 同 `dailymotion_code`，另外带上细分（reason）和去掉插件提示的短说法（brief）。raw 是空的，用的地方补上
fn dailymotion_said(code: &str, title: &str, detail: &str, url: &str) -> Option<Failure> {
    let says = |c: ErrorCode, m: String| Failure::new(c, m, "");
    Some(match code {
        "DM003" => says(ErrorCode::LiveUpcoming, tr!("直播还没开始")),
        "DM007" => says(ErrorCode::Geo, tr!("这个视频在你所在的地区看不了")),
        "DM010" | "DM020" => says(ErrorCode::NeedLogin, tr!("这是私密视频，要登录有权限的账号才能看")),
        "DM008" | "DM009" => says(ErrorCode::NeedLogin, tr!("这个视频有年龄限制，要登录{site}才能看", site = "Dailymotion")),
        "DM001" | "DM002" | "DM005" => says(ErrorCode::Unavailable, tr!("视频已被删除或不存在")),
        "DM004" => says(ErrorCode::Unavailable, tr!("这个视频因为版权原因下架了")),
        // 作者只让在 Dailymotion 自己的网站上播（「has limited its access to Dailymotion」）
        "DM016" => says(ErrorCode::Unsupported, tr!("作者设置了这个视频只能在 Dailymotion 网站上看，可以用浏览器插件在网页里下")).with_brief(tr!("作者设置了这个视频只能在 Dailymotion 网站上看")),
        _ => {
            let g = errors::classify(&format!("ERROR: [dailymotion] x: dailymotion said: {title} {detail}"), url);
            if g.code == ErrorCode::Unknown {
                return None;
            }
            g
        }
    })
}

/// B 站多 P 的条目（`--flat-playlist` 只有 `?p=N` 的地址）：读一次分 P 列表补上每一 P 的名字和时长
async fn name_bilibili_parts(http: &reqwest::Client, url: &str, entries: &mut [Entry]) {
    let Some(bv) = url::Url::parse(url).ok().and_then(|u| u.path_segments().and_then(|mut s| s.nth(1).map(str::to_string))) else { return };
    let api = format!("https://api.bilibili.com/x/player/pagelist?bvid={bv}");
    let pages: Option<Vec<Value>> = async { http.get(&api).timeout(Duration::from_secs(10)).send().await.ok()?.json::<Value>().await.ok()?.get("data")?.as_array().cloned() }.await;
    for e in entries.iter_mut() {
        let p = pages.as_ref().and_then(|ps| ps.iter().find(|x| x.get("page").and_then(Value::as_u64) == Some(e.n as u64)));
        let part = p.and_then(|x| x.get("part")).and_then(Value::as_str).filter(|s| !s.is_empty());
        e.title = match part {
            Some(t) => format!("P{} {t}", e.n),
            None if e.title == default_entry_title(e.n) => format!("P{}", e.n),
            None => e.title.clone(),
        };
        if e.dur.is_none() {
            e.dur = p.and_then(|x| x.get("duration")).and_then(Value::as_f64);
        }
        if e.thumb.is_none() {
            e.thumb = p.and_then(|x| x.get("first_frame")).and_then(Value::as_str).map(str::to_string);
        }
    }
}

/// 没有标题的条目叫什么（「第 3 个」）。`name_bilibili_parts` 拿它认出「这一条没有标题」
fn default_entry_title(n: u32) -> String {
    tr!("第 {n} 个", n = n)
}

/// 频道标签页的名字（`Interp::Tab` 里是中文原文，测试按它比较）→ 给人看的
fn tab_name(label: &str) -> String {
    match label {
        "视频" => tr!("视频§YouTube 频道的标签页"),
        "直播" => tr!("直播§YouTube 频道的标签页"),
        x => x.to_string(),
    }
}

fn short(f: &Failure) -> String {
    f.raw.lines().rev().find(|l| l.contains("ERROR")).map(|l| l.trim_start_matches("ERROR:").trim().chars().take(120).collect()).unwrap_or_else(|| f.message.clone())
}

enum Interp {
    Done(Probed),
    Follow(String),
    /// YouTube 频道主页：改读这个标签页（`/@xxx/videos`）。`label` 是标签页给人看的名字（视频 / Shorts / 直播）
    Tab { url: String, channel: String, label: &'static str },
}

/// YouTube 频道主页 `--flat-playlist` 的「条目」是 Videos / Live / Shorts 几个标签页（每个本身又是列表）：
/// 返回要读的标签页地址、频道名和标签页的名字。有「Videos」就读它；只有 Shorts 或直播的频道读那一个。
/// t.co → bit.ly → 频道页这种，按 yt-dlp 最后落到的 webpage_url 算
fn channel_videos_tab(v: &Value, url: &str) -> Option<(String, String, &'static str)> {
    let base = v.get("webpage_url").and_then(Value::as_str).unwrap_or(url);
    if !is_youtube(base) {
        return None;
    }
    let entries = v.get("entries")?.as_array()?;
    let tab = |e: &Value| {
        e.get("_type").and_then(Value::as_str) == Some("playlist")
            || e.get("url").and_then(Value::as_str).is_some_and(|u| ["/videos", "/shorts", "/streams", "/live", "/podcasts", "/releases"].iter().any(|t| u.trim_end_matches('/').ends_with(t)))
    };
    if entries.is_empty() || !entries.iter().all(tab) {
        return None;
    }
    let u = url::Url::parse(base).ok()?;
    let segs: Vec<&str> = u.path_segments()?.filter(|s| !s.is_empty()).collect();
    let root = match segs.first() {
        Some(s) if s.starts_with('@') => segs[..1].join("/"),
        Some(&("channel" | "c" | "user")) if segs.len() >= 2 => segs[..2].join("/"),
        _ => return None,
    };
    let name = v.get("channel").or_else(|| v.get("uploader")).or_else(|| v.get("title")).and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| tr!("这个频道"));
    // 标签页的标题是「频道名 - Videos」，没解析的条目地址以 /videos 结尾
    let has = |suffix: &str, path: &str| {
        entries.iter().any(|e| e.get("title").and_then(Value::as_str).is_some_and(|t| t.ends_with(suffix)) || e.get("url").and_then(Value::as_str).is_some_and(|u| u.trim_end_matches('/').ends_with(path)))
    };
    let (path, label) = if has(" - Videos", "/videos") {
        ("videos", "视频")
    } else if has(" - Shorts", "/shorts") {
        ("shorts", "Shorts")
    } else if has(" - Live", "/streams") {
        ("streams", "直播")
    } else {
        ("videos", "视频")
    };
    Some((format!("https://www.youtube.com/{root}/{path}"), name, label))
}

fn is_flat(e: &Value) -> bool {
    matches!(e.get("_type").and_then(Value::as_str), Some("url") | Some("url_transparent"))
}

/// yt-dlp `-J` 的结果 → 单个视频 / 播放列表。
fn interpret(v: Value, url: &str, item: Option<u32>, record_live: bool) -> std::result::Result<Interp, Failure> {
    let ty = v.get("_type").and_then(Value::as_str).unwrap_or("video");
    if ty != "playlist" && ty != "multi_video" {
        return single(v, url, record_live).map(Interp::Done);
    }
    if item.is_none() {
        if let Some((tab, channel, label)) = channel_videos_tab(&v, url) {
            return Ok(Interp::Tab { url: tab, channel, label });
        }
    }
    let entries: Vec<Value> = v.get("entries").and_then(Value::as_array).map(|a| a.iter().filter(|e| e.is_object()).cloned().collect()).unwrap_or_default();
    if item.is_some() || entries.len() == 1 {
        let Some(e) = entries.into_iter().next() else {
            return Err(Failure::new(ErrorCode::NoMedia, tr!("这个网页里没有找到视频或音频"), "playlist has no entries"));
        };
        if is_flat(&e) {
            let u = e.get("url").and_then(Value::as_str).unwrap_or("").to_string();
            return Ok(Interp::Follow(u));
        }
        return single(e, url, record_live).map(Interp::Done);
    }
    if entries.is_empty() {
        if errors::is_image_post(url) {
            return Err(errors::image_post(format!("playlist has no entries ({url})")));
        }
        return Err(Failure::new(ErrorCode::NoMedia, tr!("这个列表里没有视频"), "playlist has no entries"));
    }
    let title = v.get("title").and_then(Value::as_str).map(str::to_string);
    Ok(Interp::Done(Probed::List { title, entries: entries_from(&entries, url) }))
}

/// 列表条目 → 界面上的条目。还没解析的（`_type: url`）直接下它自己的地址；
/// 已经解析好的（多视频帖子）和帖子是同一个地址，按序号下载。
pub fn entries_from(list: &[Value], parent: &str) -> Vec<Entry> {
    list.iter()
        .enumerate()
        .map(|(i, e)| {
            let s = |k: &str| e.get(k).and_then(Value::as_str).map(str::to_string).filter(|x| !x.is_empty());
            let n = e.get("playlist_index").and_then(Value::as_u64).map(|x| x as u32).unwrap_or(i as u32 + 1);
            let flat = is_flat(e);
            let own = if flat { s("url").filter(|u| u.starts_with("http")) } else { None };
            let thumb = s("thumbnail").or_else(|| e.get("thumbnails").and_then(Value::as_array).and_then(|a| a.iter().rev().find_map(|t| t.get("url").and_then(Value::as_str).map(str::to_string))));
            Entry {
                id: s("id").unwrap_or_else(|| n.to_string()),
                title: s("title").unwrap_or_else(|| default_entry_title(n)),
                dur: e.get("duration").and_then(Value::as_f64),
                thumb,
                url: own.clone().or_else(|| s("webpage_url")).unwrap_or_else(|| parent.to_string()),
                n,
                item: if own.is_some() { None } else { Some(n) },
            }
        })
        .collect()
}

/// 单个视频：直播、还没开始的直播、没有格式，这里拦下来。
fn single(v: Value, url: &str, record_live: bool) -> std::result::Result<Probed, Failure> {
    match v.get("live_status").and_then(Value::as_str) {
        Some("is_live") if !record_live => {
            return Err(Failure::new(ErrorCode::Live, tr!("这是正在进行的直播"), "live_status=is_live"));
        }
        Some("is_upcoming") => {
            let at = v.get("release_timestamp").and_then(Value::as_i64).map(|t| t * 1000);
            let mut f = Failure::new(ErrorCode::LiveUpcoming, at.map(errors::live_at_text).unwrap_or_else(|| tr!("直播还没开始")), "live_status=is_upcoming");
            f.starts_at = at;
            return Err(f);
        }
        _ => {}
    }
    let has_formats = v.get("formats").and_then(Value::as_array).is_some_and(|a| !a.is_empty()) || v.get("url").and_then(Value::as_str).is_some();
    if !has_formats {
        return Err(Failure::new(ErrorCode::NoMedia, tr!("这个网页里没有找到视频或音频"), format!("no formats ({url})")));
    }
    Ok(Probed::Video { info: info_from_json(&v), raw: v, playlist: None, extra: vec![] })
}

#[derive(Debug, PartialEq)]
pub enum Line {
    Progress { status: String, got: f64, total: Option<f64>, fmt: String },
    File(PathBuf),
    /// 实际选中的格式说明（抖音 / TikTok 带水印的会标 watermarked）
    Note(String),
    Other(String),
}

pub fn parse_line(l: &str) -> Line {
    if let Some(rest) = l.strip_prefix(TAG) {
        if let Some(p) = rest.strip_prefix("F|") {
            return Line::File(PathBuf::from(p.trim()));
        }
        if let Some(p) = rest.strip_prefix("N|") {
            return Line::Note(p.trim().to_string());
        }
        if let Some(p) = rest.strip_prefix("P|") {
            let f: Vec<&str> = p.split('|').collect();
            let num = |s: &str| s.trim().parse::<f64>().ok();
            if f.len() >= 5 {
                return Line::Progress {
                    status: f[0].to_string(),
                    got: num(f[1]).unwrap_or(0.0),
                    total: num(f[2]).or_else(|| num(f[3])),
                    fmt: f.get(4).unwrap_or(&"").to_string(),
                };
            }
        }
    }
    Line::Other(l.to_string())
}

pub async fn download(ctx: &Ctx, url: &str, info_json: Option<&Path>, out_dir: &Path, no_watermark: bool) -> Result<Output> {
    let snap = ctx.h.snapshot();
    let shape = Shape::of(snap.meta.as_ref());
    let tmp = temp_dir(out_dir, url, snap.req.playlist_item);
    let mp4 = !snap.req.audio_only && !shape.image && snap.req.format.as_deref().unwrap_or("MP4").eq_ignore_ascii_case("MP4");
    let r = run_download(ctx, url, info_json, out_dir, &tmp, no_watermark, shape, true).await;
    let out = match r {
        // 只有 MP4 装不下的格式（Theora 这类）：yt-dlp 转封装失败。按原格式下，再由 Captae 用 ffmpeg 转成 MP4（FIXES 第 16 项）。
        // 录直播不重下（直播回不去了），录到的部分在 run_download 里已经整理过
        Err(e) if mp4 && !snap.live && errors::failure_in(&e).is_some_and(|f| f.raw.to_ascii_lowercase().contains("conversion failed")) => {
            ctx.h.update(|j| j.warn(tr!("这个视频的格式装不进 MP4，先按原格式下完，再转成 MP4")));
            let mut o = run_download(ctx, url, info_json, out_dir, &tmp, no_watermark, shape, false).await?;
            o.format_done = false;
            o
        }
        r => r?,
    };
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    let _ = tokio::fs::remove_dir(out_dir.join(".captae-tmp")).await;
    Ok(out)
}

/// 存成 info.json 给下载用之前理一下。Giphy 的媒体地址看到浏览器式的 `Accept: text/html` 就回网页
/// （下到的 .mp4 其实是 6 万字节的 HTML，FIXES 第 8 项），改成 `*/*`（浏览器的 <video> 就是这样请求的）
pub fn prepare_info(v: &mut Value) {
    fn fix(f: &mut Value) {
        let giphy = f.get("url").and_then(Value::as_str).and_then(|u| url::Url::parse(u).ok()).and_then(|u| u.host_str().map(|h| h == "giphy.com" || h.ends_with(".giphy.com"))).unwrap_or(false);
        if giphy {
            if let Some(Value::Object(h)) = f.get_mut("http_headers") {
                h.insert("Accept".into(), Value::String("*/*".into()));
                h.remove("Sec-Fetch-Mode");
            }
        }
    }
    fix(v);
    for k in ["formats", "requested_formats"] {
        if let Some(Value::Array(fs)) = v.get_mut(k) {
            for f in fs {
                fix(f);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn run_download(ctx: &Ctx, url: &str, info_json: Option<&Path>, out_dir: &Path, tmp: &Path, no_watermark: bool, shape: Shape, remux: bool) -> Result<Output> {
    let snap = ctx.h.snapshot();
    let live = snap.live;
    let mut req = snap.req;
    let ffmpeg = ctx.tools.path(Tool::Ffmpeg).ok();
    // 录直播不交给 aria2c（它录不了直播；`m3u8:native` 也会被 yt-dlp 改交给 ffmpeg，这里干脆不加）
    let aria = if ctx.settings.ytdlp_use_aria2 && !live { ctx.tools.path(Tool::Aria2).ok() } else { None };
    req.threads = req.threads.or(Some(ctx.settings.threads));
    let mut args = download_args(&req, out_dir, tmp, ffmpeg.as_deref(), aria.as_deref().map(|p| (p, ctx.settings.threads)), no_watermark, shape, remux);
    // 读取信息时 YouTube 换过客户端：信息过期重新解析时也用那一组
    args.extend(snap.probe_args.iter().cloned());
    // 登录状态：临时 cookies 文件只给这一次的 yt-dlp 用，yt-dlp 退出后 `_cookies` 被丢掉时删掉（取消时也一样）
    let (auth_args, _cookies) = ctx.auth.ytdlp()?;
    let mut c = ctx.tools.command(Tool::YtDlp)?;
    c.args(&args).args(auth_args);
    // 交给安装包里的 aria2c 下载时，aria2c 继承 yt-dlp 的环境变量：一样告诉它 OpenSSL 模块在哪（见 tools::aria2_modules）
    if let Some(m) = aria.as_deref().and_then(crate::tools::aria2_modules) {
        c.env("OPENSSL_MODULES", m);
    }
    match info_json {
        Some(p) => c.arg("--load-info-json").arg(p),
        None => c.arg(url),
    };
    c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    // 暂停、取消时整个 future 被丢掉：yt-dlp 和它启动的 ffmpeg、aria2c 一起结束（tools::Proc）
    let mut child = crate::tools::spawn(&mut c).map_err(|e| crate::tools::spawn_failure("yt-dlp", &e))?;
    let pid = child.id();
    // 录直播：录到的东西在这个任务的中间目录里（Captae 中途退出时，下次打开从这里整理）；yt-dlp 交给 ffmpeg 录时不报进度，看文件大小
    let mut watch = None;
    if live {
        ctx.note_raw(tmp);
        watch = Some(super::watch_size(ctx.h.clone(), tmp.to_path_buf()));
    }
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let err_task = tokio::spawn(async move {
        let mut buf = String::new();
        let mut r = BufReader::new(stderr).lines();
        while let Ok(Some(l)) = r.next_line().await {
            buf.push_str(&l);
            buf.push('\n');
        }
        buf
    });

    // 视频和音频分开下时会有两轮进度，按 format_id 累加
    let mut per_fmt: HashMap<String, (f64, f64)> = HashMap::new();
    let mut meter = SpeedMeter::default();
    let mut last_emit = std::time::Instant::now() - std::time::Duration::from_secs(1);
    let mut file: Option<PathBuf> = None;
    let mut merging_logged = false;
    let mut lines = BufReader::new(stdout).lines();
    // 录直播点了「停止并保存」：给 yt-dlp 发 SIGINT（Windows 是往它的控制台发 Ctrl-C，见 tools::interrupt），让它自己收尾，最多等到这个时刻
    let mut stop_by: Option<tokio::time::Instant> = None;
    loop {
        let next = match stop_by {
            Some(t) => match tokio::time::timeout_at(t, lines.next_line()).await {
                Ok(l) => l?,
                Err(_) => {
                    // 没按时收尾：结束整棵进程树，录到的部分下面由 Captae 自己整理
                    ctx.h.update(|j| j.warn(tr!("下载引擎没有按时收尾，直接结束它，录到的部分由 Captae 整理")));
                    if let Some(p) = pid {
                        crate::tools::kill_tree(p);
                    }
                    None
                }
            },
            None if live => tokio::select! {
                l = lines.next_line() => l?,
                _ = ctx.finish.cancelled() => {
                    ctx.note_stopping();
                    watch = None;
                    // 请求送不到（Windows 上没有 captae 帮忙发 Ctrl-C 时）：不用等，马上结束进程树，录到的部分由 Captae 整理
                    let sent = pid.is_some_and(crate::tools::interrupt);
                    stop_by = Some(tokio::time::Instant::now() + if sent { STOP_WAIT } else { Duration::ZERO });
                    continue;
                }
            },
            None => lines.next_line().await?,
        };
        let Some(l) = next else { break };
        match parse_line(&l) {
            Line::Progress { status, got, total, fmt } => {
                let e = per_fmt.entry(fmt).or_insert((0.0, 0.0));
                e.0 = got;
                if let Some(t) = total {
                    e.1 = t;
                }
                if status == "finished" {
                    e.1 = e.1.max(got);
                }
                let got_all: f64 = per_fmt.values().map(|v| v.0).sum();
                let total_all: f64 = per_fmt.values().map(|v| v.1.max(v.0)).sum();
                let sp = meter.push(got_all);
                if last_emit.elapsed().as_millis() >= 250 || status == "finished" {
                    last_emit = std::time::Instant::now();
                    ctx.h.update(|j| {
                        j.got = mb(got_all);
                        // 预估总大小只增不减，避免进度条倒退。录直播没有总大小
                        if !live {
                            j.size = j.size.max(mb(total_all)).max(j.got);
                        }
                        j.speed = mb(sp);
                    });
                }
            }
            Line::File(p) => file = Some(p),
            Line::Note(n) => {
                if no_watermark && n.to_ascii_lowercase().contains("watermark") {
                    ctx.h.update(|j| j.warn(tr!("这个视频没有无水印的版本，下载的是带水印的")));
                }
            }
            Line::Other(o) => {
                if (o.contains("[Merger]") || o.contains("[VideoRemuxer]") || o.contains("[ExtractAudio]")) && !merging_logged {
                    merging_logged = true;
                    ctx.h.update(|j| {
                        j.speed = 0.0;
                        j.log(tr!("下载完成，正在生成文件"));
                    });
                }
            }
        }
    }
    drop(watch);
    let status = match stop_by {
        // 收尾时 yt-dlp 已经关了输出，进程最多再等一会儿
        Some(_) => match tokio::time::timeout(Duration::from_secs(15), child.wait()).await {
            Ok(s) => Some(s?),
            Err(_) => {
                if let Some(p) = pid {
                    crate::tools::kill_tree(p);
                }
                None
            }
        },
        None => Some(child.wait().await?),
    };
    let stderr = ctx.auth.redact(&err_task.await.unwrap_or_default());
    let ok = status.is_some_and(|s| s.success());
    if live {
        // 收尾成功（停止并保存、直播自己结束）：yt-dlp 已经整理好、放到了保存位置（给出了路径就是放好了）。
        // 它录直播写的是 MPEG-TS；画面和声音分两路（交给 ffmpeg 合在一起）时不会再转成 MP4（YoutubeDL 的 fixup 遇到 requested_formats 就跳过），
        // 文件叫 .mp4、里面却是 TS，QuickTime 放不了（2026-11-24 录 YouTube 直播 144p 实测）：这种就原样整理成真正的 MP4
        if let Some(p) = file.filter(|p| p.exists()) {
            let p = match crate::ffmpeg::container(&ctx.tools, &p).await {
                Some(c) if c.contains("mpegts") => {
                    ctx.h.update(|j| j.log(tr!("录到的是 MPEG-TS，整理成 MP4")));
                    super::aria2::tidy_stream(&ctx.tools, &p).await?
                }
                _ => p,
            };
            return Ok(Output { path: p, format_done: true });
        }
        // 没收好尾（被直接结束、网络断了）：中间目录里写了一半的那个就是录到的部分
        let why = (!ok && stop_by.is_none()).then(|| errors::classify(&stderr, url));
        return leftover(ctx, tmp, out_dir, why).await;
    }
    if !ok {
        return Err(errors::classify(&stderr, url).into());
    }
    let path = file.ok_or_else(|| anyhow!(tr!("yt-dlp 没有返回文件路径")))?;
    Ok(Output { path, format_done: true })
}

/// 点了「停止并保存」以后，最多等 yt-dlp 自己收尾多久（ffmpeg 写完文件，再把 MPEG-TS 原样转成 MP4；长的录像要多等一会儿）
const STOP_WAIT: Duration = Duration::from_secs(120);

/// 录直播没有正常收尾：把中间目录里录到的部分整理成能播放的文件，放进保存位置。`why` 是录制中断的原因（自己点的停止时没有）。
/// 什么都没录到时报原来的错
async fn leftover(ctx: &Ctx, tmp: &Path, out_dir: &Path, why: Option<Failure>) -> Result<Output> {
    let raw = super::largest_file(tmp).filter(|p| std::fs::metadata(p).is_ok_and(|m| m.len() > 4096));
    let Some(raw) = raw else {
        return Err(why.map(anyhow::Error::new).unwrap_or_else(|| anyhow!(tr!("还没有录到内容"))));
    };
    if let Some(f) = &why {
        let msg = f.message.clone();
        ctx.h.update(|j| j.warn(tr!("录制中断了（{reason}），录到的部分照样保存", reason = msg)));
        ctx.note_stopping();
    }
    ctx.h.update(|j| j.log(tr!("正在把录到的部分整理成能播放的文件")));
    let snap = ctx.h.snapshot();
    let stem = if snap.untitled { raw.file_name().map(|n| n.to_string_lossy().split('.').next().unwrap_or("").to_string()).unwrap_or_default() } else { snap.title.clone() };
    let path = crate::ffmpeg::tidy_recording(&ctx.tools, &raw, None, out_dir, &stem).await?;
    let _ = tokio::fs::remove_dir_all(tmp).await;
    Ok(Output { path, format_done: true })
}

/// 用于标题：没读到时用 host 兜住。
pub fn title_or(info: &MediaInfo, fallback: &str) -> String {
    info.title.clone().filter(|t| !t.trim().is_empty()).unwrap_or_else(|| fallback.to_string())
}

pub fn fmt_label(req: &AddRequest, info: &MediaInfo) -> String {
    if req.audio_only {
        // 界面按「 · 」拆这一串，分隔符留在外面
        return format!("{} · {}", req.format.clone().unwrap_or_else(|| "MP3".into()), tr!("高音质"));
    }
    let f = req.format.clone().filter(|f| f != "保持原格式").unwrap_or_else(|| info.ext.clone().unwrap_or_default().to_uppercase());
    match info.height {
        Some(h) => format!("{f} · {h}p"),
        None => f,
    }
}

pub fn dur_label(info: &MediaInfo) -> String {
    info.duration.map(clock).unwrap_or_default()
}

pub fn default_name(info: &MediaInfo) -> String {
    safe_name(info.title.as_deref().unwrap_or("video"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_progress() {
        let l = "CAPTAE|P|downloading|1048576|10485760|NA|137";
        assert_eq!(parse_line(l), Line::Progress { status: "downloading".into(), got: 1048576.0, total: Some(10485760.0), fmt: "137".into() });
        let l = "CAPTAE|P|downloading|100|NA|2000.5|251";
        assert!(matches!(parse_line(l), Line::Progress { total: Some(t), .. } if t == 2000.5));
        assert_eq!(parse_line("CAPTAE|F|/a/b.mp4"), Line::File("/a/b.mp4".into()));
        assert!(matches!(parse_line("[download] foo"), Line::Other(_)));
    }
    #[test]
    fn selectors() {
        let mut r = AddRequest::default();
        assert_eq!(format_selector(&r, false, false), "bv*+ba/b");
        r.quality = Some("1080p".into());
        assert_eq!(format_selector(&r, false, false), "bv*[height<=1080]+ba/b[height<=1080]/bv*+ba/b");
        // 竖屏的「1080p」是宽 1080（清晰度按短边叫）
        assert_eq!(format_selector(&r, false, true), "bv*[width<=1080]+ba/b[width<=1080]/bv*+ba/b");
        r.audio_only = true;
        assert_eq!(format_selector(&r, false, false), "ba/b");
        assert_eq!(format_selector(&r, true, false), "ba[format_note!*=watermark]/b[format_note!*=watermark]/ba/b");
    }
    #[test]
    fn youtube_links() {
        assert_eq!(youtube_id("https://www.youtube.com/watch?v=mC73amqYFpQ&t=3s").as_deref(), Some("mC73amqYFpQ"));
        assert_eq!(youtube_id("https://youtu.be/eXOYqJ6XdqA?si=x").as_deref(), Some("eXOYqJ6XdqA"));
        assert_eq!(youtube_id("https://m.youtube.com/shorts/3xEXpOE92VE").as_deref(), Some("3xEXpOE92VE"));
        assert_eq!(youtube_id("https://www.youtube.com/@someone"), None);
        assert_eq!(youtube_id("https://vimeo.com/1084537"), None);
        let p = youtube_list_of("https://www.youtube.com/watch?v=M7XYiGAYKJ4&list=PLx0sYbCqOb8TBPRdmBHs5Iftvv9TPboYG&index=2").unwrap();
        assert_eq!(p.url, "https://www.youtube.com/playlist?list=PLx0sYbCqOb8TBPRdmBHs5Iftvv9TPboYG");
        assert!(youtube_list_of("https://www.youtube.com/watch?v=M7XYiGAYKJ4").is_none());
        assert!(youtube_list_of("https://www.youtube.com/playlist?list=PLabc").is_none());
    }

    #[test]
    fn playlists() {
        use serde_json::json;
        // 多视频帖子：条目已经解析好（有 formats），和帖子同一个地址 → 按序号下载
        let tweet = json!({"_type": "playlist", "id": "2108384911245869197", "title": "Higgsfield - post",
            "entries": [
                {"id": "1", "title": "Higgsfield - post #1", "duration": 12.5, "thumbnail": "https://pbs.twimg.com/a.jpg", "webpage_url": "https://twitter.com/higgsfield/status/2108384911245869197", "formats": [{"url": "https://video.twimg.com/a.mp4"}]},
                {"id": "2", "title": "Higgsfield - post #2", "duration": 8.0, "webpage_url": "https://twitter.com/higgsfield/status/2108384911245869197", "formats": [{"url": "https://video.twimg.com/b.mp4"}]}
            ]});
        let u = "https://x.com/higgsfield/status/2108384911245869197";
        let Ok(Interp::Done(Probed::List { title, entries })) = interpret(tweet.clone(), u, None, false) else { panic!() };
        assert_eq!(title.as_deref(), Some("Higgsfield - post"));
        assert_eq!(entries.len(), 2);
        assert_eq!((entries[1].n, entries[1].item, entries[1].dur), (2, Some(2), Some(8.0)));
        assert_eq!(entries[0].thumb.as_deref(), Some("https://pbs.twimg.com/a.jpg"));
        // 选了第 2 个：--playlist-items 2 的结果只剩一条
        let one = json!({"_type": "playlist", "entries": [tweet["entries"][1].clone()]});
        let Ok(Interp::Done(Probed::Video { info, .. })) = interpret(one, u, Some(2), false) else { panic!() };
        assert_eq!(info.title.as_deref(), Some("Higgsfield - post #2"));
        // YouTube 播放列表（--flat-playlist）：条目只是链接 → 各自下载自己的地址
        let yt = json!({"_type": "playlist", "title": "Mix", "entries": [
            {"_type": "url", "ie_key": "Youtube", "id": "mC73amqYFpQ", "url": "https://www.youtube.com/watch?v=mC73amqYFpQ", "title": "A", "duration": 61.0, "thumbnails": [{"url": "https://i.ytimg.com/vi/mC73amqYFpQ/hq.jpg"}]},
            {"_type": "url", "ie_key": "Youtube", "id": "eXOYqJ6XdqA", "url": "https://www.youtube.com/watch?v=eXOYqJ6XdqA", "title": "B"}
        ]});
        let Ok(Interp::Done(Probed::List { entries, .. })) = interpret(yt, "https://www.youtube.com/playlist?list=PLx", None, false) else { panic!() };
        assert_eq!((entries[0].url.as_str(), entries[0].item), ("https://www.youtube.com/watch?v=mC73amqYFpQ", None));
        assert_eq!(entries[0].thumb.as_deref(), Some("https://i.ytimg.com/vi/mC73amqYFpQ/hq.jpg"));
        // 只有一条、还是个链接：接着读那条
        let single_link = json!({"_type": "playlist", "entries": [{"_type": "url", "url": "https://www.youtube.com/watch?v=mC73amqYFpQ"}]});
        assert!(matches!(interpret(single_link, "https://x", None, false), Ok(Interp::Follow(u)) if u.ends_with("mC73amqYFpQ")));
        // 空列表
        assert_eq!(interpret(json!({"_type": "playlist", "entries": []}), "https://x", None, false).err().unwrap().code, ErrorCode::NoMedia);
    }

    #[test]
    fn live_checks() {
        use serde_json::json;
        let live = json!({"id": "x", "title": "Live", "live_status": "is_live", "formats": [{"url": "https://a/b.m3u8"}]});
        assert_eq!(single(live.clone(), "https://www.youtube.com/watch?v=x", false).err().unwrap().code, ErrorCode::Live);
        assert!(single(live, "https://www.youtube.com/watch?v=x", true).is_ok());
        let soon = json!({"id": "x", "live_status": "is_upcoming", "release_timestamp": 4_102_444_800i64});
        let f = single(soon, "https://www.youtube.com/watch?v=x", false).err().unwrap();
        assert_eq!((f.code, f.starts_at), (ErrorCode::LiveUpcoming, Some(4_102_444_800_000)));
        assert!(f.to_json()["startsAt"].is_i64());
        assert_eq!(single(json!({"id": "x", "formats": []}), "https://a", false).err().unwrap().code, ErrorCode::NoMedia);
    }

    /// FIXES 第 10 项：没有时长时，靠提取器名字认直播（huya-61010b 是 HuyaLive / huya:live）
    #[test]
    fn live_extractor_names() {
        use serde_json::json;
        assert!(live_extractor(&json!({"extractor_key": "HuyaLive", "extractor": "huya:live"})));
        assert!(live_extractor(&json!({"extractor_key": "X", "extractor": "afreecatv:live"})));
        // 名字里碰巧有 live 这几个字母的不算
        assert!(!live_extractor(&json!({"extractor_key": "OliveTV", "extractor": "olivetv"})));
        assert!(!live_extractor(&json!({"extractor_key": "VideoDelivery", "extractor": "videodelivery"})));
        assert!(!live_extractor(&json!({"extractor_key": "Odnoklassniki", "extractor": "Odnoklassniki"})));
    }

    /// FIXES 第 2、4 项：Dailymotion 接口的错误号（2026-11-24 带 embedder 和 locale=en 读到的原文）
    #[test]
    fn dailymotion_codes() {
        let u = "https://www.dailymotion.com/video/x8qckyq";
        // dailymotion-c34624：yt-dlp 说「Konten pribadi」，接口是 DM020 私密
        let (c, m) = dailymotion_code("DM020", "Private content.", "We're sorry, but we cannot display this content because it has been labelled private by its author.", u).unwrap();
        assert_eq!((c, m.as_str()), (ErrorCode::NeedLogin, "这是私密视频，要登录有权限的账号才能看"));
        // dailymotion-e539f8：频道没开播
        assert_eq!(dailymotion_code("DM003", "Channel offline.", "Follow jovempannews to receive updates about upcoming live streams.", u).unwrap().0, ErrorCode::LiveUpcoming);
        // 不带 embedder 时的 DM016：作者只让在 Dailymotion 网站上播
        assert_eq!(dailymotion_code("DM016", "Content not Available", "The video you requested cannot be displayed on this website. trans7officialstream has limited its access to Dailymotion.", u).unwrap().0, ErrorCode::Unsupported);
        // 没见过的号：按英文原文归类，归不了就不改
        assert_eq!(dailymotion_code("DM099", "This video is not available from your location", "", u).unwrap().0, ErrorCode::Geo);
        assert!(dailymotion_code("DM098", "Something odd", "", u).is_none());
    }

    #[test]
    fn flaky_youtube_errors() {
        let f = errors::classify("ERROR: [youtube] mC73amqYFpQ: This video is not available", "https://www.youtube.com/watch?v=mC73amqYFpQ");
        assert!(youtube_flaky(&f));
        let f = errors::classify("ERROR: [youtube] 3xEXpOE92VE: Requested format is not available. Use --list-formats for a list of available formats", "https://www.youtube.com/watch?v=3xEXpOE92VE");
        assert!(youtube_flaky(&f));
        let f = errors::classify("ERROR: [youtube] mC73amqYFpQ: The page needs to be reloaded.", "https://www.youtube.com/watch?v=mC73amqYFpQ");
        assert!(youtube_flaky(&f) && f.code.transient());
        let f = errors::classify("ERROR: [youtube] x: Private video. Sign in if you've been granted access to this video", "https://www.youtube.com/watch?v=x");
        assert!(!youtube_flaky(&f));
    }

    #[test]
    fn info_json_without_login_cookies() {
        use crate::login::{Auth, Cookie};
        use serde_json::json;
        let ck = |n: &str, v: &str, d: &str| Cookie { name: n.into(), value: v.into(), domain: d.into(), path: "/".into(), secure: true, http_only: true, host_only: false, expires: None };
        let auth = Auth { cookies: vec![ck("SESSDATA", "sess-secret-1", ".bilibili.com"), ck("cf_clearance", "cf-secret-22", ".example.com")], ..Default::default() };
        let mut v = json!({
            "id": "1", "url": "https://example.com/v.mp4",
            "cookies": "cf_clearance=cf-secret-22; Domain=.example.com; Path=/; Secure; Expires=1795450609; seen=1; Domain=example.com; Path=/",
            "http_headers": {"User-Agent": "x", "Cookie": "SESSDATA=sess-secret-1"},
            "formats": [{"url": "https://api.bilibili.com/a", "cookies": "SESSDATA=\"sess-secret-1\"; Domain=.bilibili.com; Path=/; Secure; buvid3=keep-me; Domain=.bilibili.com; Path=/"},
                        {"url": "https://cdn.example.com/b", "cookies": "SESSDATA=other-value-9; Domain=.bilibili.com; Path=/"}]
        });
        strip_auth_cookies(&mut v, &auth);
        let text = v.to_string();
        assert!(!text.contains("sess-secret-1") && !text.contains("cf-secret-22"), "{text}");
        // 网站自己种下的留着
        assert_eq!(v["cookies"], "seen=1; Domain=example.com; Path=/");
        assert_eq!(v["formats"][0]["cookies"], "buvid3=keep-me; Domain=.bilibili.com; Path=/");
        // 名字和域名都对上的（值被网站换过）也去掉；整个字段空了就删掉
        assert!(v["formats"][1].get("cookies").is_none());
        assert!(v["http_headers"].get("Cookie").is_none());
        // 没有带登录状态时原样不动
        let mut w = json!({"cookies": "a=b; Domain=x.com"});
        strip_auth_cookies(&mut w, &Auth::default());
        assert_eq!(w["cookies"], "a=b; Domain=x.com");
    }

    /// 带登录状态读取：临时 cookies 文件在 yt-dlp 运行时是 600、有内容；结束就删；报错原文里没有 cookie 的值。
    /// 先试 GNU 的 `stat -c`（Linux），不行再用 BSD 的 `stat -f`（macOS）：反过来的话 GNU stat 的 -f 是看文件系统，不报错，退不过去。
    /// 只在 unix 上跑：看的是文件权限 600（Windows 上临时文件在用户自己的数据目录里，没有这一项）
    #[cfg(unix)]
    #[tokio::test]
    async fn probe_with_login_cleans_up() {
        use crate::login::{Auth, Cookie};
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let seen = dir.path().join("seen.txt");
        // 假的 yt-dlp：记下 --cookies 文件的权限、内容和位置，再报一个带着 cookie 值的错
        let fake = dir.path().join("yt-dlp");
        let script = format!(
            "#!/bin/sh\nf=''; prev=''\nfor a in \"$@\"; do if [ \"$prev\" = \"--cookies\" ]; then f=\"$a\"; fi; prev=\"$a\"; done\n\
             echo \"path=$f\" > '{0}'\necho \"mode=$(stat -c %a \"$f\" 2>/dev/null || stat -f %Lp \"$f\")\" >> '{0}'\ncat \"$f\" >> '{0}'\necho \"args=$*\" >> '{0}'\n\
             echo 'ERROR: [BiliBili] 1: HTTP Error 403: Forbidden (sent SESSDATA=sess-secret-1)' >&2\nexit 1\n",
            seen.display()
        );
        std::fs::write(&fake, script).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let s = crate::config::Settings { ytdlp_path: fake.display().to_string(), proxy_mode: "none".into(), ..Default::default() };
        let tools = Toolbox::from_settings(&s, None, None);
        let ck = Cookie { name: "SESSDATA".into(), value: "sess-secret-1".into(), domain: ".bilibili.com".into(), path: "/".into(), secure: true, http_only: true, host_only: false, expires: None };
        let auth_dir = dir.path().join("work/auth");
        let auth = Auth { cookies: vec![ck], user_agent: Some(crate::login::safari_ua_for("27.2")), impersonate: Some("safari:macos".into()), browser: None, from: "测试".into(), dir: auth_dir.clone() };
        let headers = BTreeMap::new();
        let note = |_: String| {};
        let pr = ProbeReq { url: "https://www.bilibili.com/video/BV121yjzQEWf", headers: &headers, item: None, record_live: false, note: &note, auth: Some(&auth), extra: vec![] };
        let f = probe_url(&tools, &reqwest::Client::new(), &pr).await.unwrap_err();
        assert!(!f.raw.contains("sess-secret-1") && !f.message.contains("sess-secret-1"), "{f:?}");
        assert!(f.raw.contains("SESSDATA=***"), "{}", f.raw);
        let seen = std::fs::read_to_string(&seen).unwrap();
        assert!(seen.contains("mode=600"), "{seen}");
        assert!(seen.contains(".bilibili.com\tTRUE\t/\tTRUE\t0\tSESSDATA\tsess-secret-1"), "{seen}");
        assert!(seen.contains("--user-agent Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.2 Safari/605.1.15 --impersonate safari:macos"), "{seen}");
        let used = seen.lines().find_map(|l| l.strip_prefix("path=")).unwrap();
        assert!(used.starts_with(&auth_dir.display().to_string()));
        assert!(!Path::new(used).exists(), "yt-dlp 结束就删");
        assert_eq!(std::fs::read_dir(&auth_dir).unwrap().count(), 0);
    }

    /// 第三阶段 FIXES 第 7 项：读取时就认出试看、试听片段（字段取自 2026-11-24 用附带的 yt-dlp 读到的原始 JSON）
    #[test]
    fn previews() {
        use serde_json::json;
        // soundcloud-846f28：SoundCloud Go+ 曲目，每个格式都是 preview
        let sc = json!({"id": "2439118695", "extractor_key": "Soundcloud", "duration": 30.0, "vcodec": "none", "acodec": "mp3", "format_id": "http_mp3_1_0_preview",
            "formats": [{"format_id": "hls_mp3_1_0_preview", "vcodec": "none", "url": "https://cf-hls-media.sndcdn.com/playlist/0/30/C4rmdfHXMaI5.128.mp3/playlist.m3u8"},
                        {"format_id": "http_mp3_1_0_preview", "vcodec": "none", "url": "https://cf-preview-media.sndcdn.com/preview/0/30/C4rmdfHXMaI5.128.mp3"}]});
        let f = preview("https://soundcloud.com/tomorrowxtogether/angel-bite", &sc).unwrap();
        assert_eq!((f.code, f.code.action()), (ErrorCode::NeedLogin, "login"));
        assert_eq!(f.message, "这是会员内容，没登录只能下到 30 秒的试听片段，完整版要登录开了会员的 SoundCloud 账号");
        // ximalaya-b5dfef：喜马拉雅付费有声书，format_note: Sample
        let xm = json!({"id": "573096483", "duration": 180, "vcodec": "none", "format_id": "vip", "format_note": "Sample",
            "formats": [{"format_id": "vip", "format_note": "Sample", "vcodec": "none", "url": "http://audiofreepay.ali.xmcdn.com/download/1.0.0/storages/99c9-audiopay/62/D9/GKwRINsHCwReAFDeOwGpFfg7-aacv2-48K_preview_1138282.m4a"}]});
        assert_eq!(preview("https://www.ximalaya.com/sound/573096483", &xm).unwrap().message, "这是会员内容，没登录只能下到 3 分钟的试听片段，完整版要登录开了会员的喜马拉雅账号");
        // 163-bca24d：网易云 VIP 歌，元数据 320 秒，给的流只有 481115 字节、128 kbps（30 秒）
        let ne = json!({"id": "287398", "duration": 320, "vcodec": "none", "acodec": "mp3", "abr": 128, "tbr": 128, "filesize": 481115, "format_id": "standard",
            "formats": [{"format_id": "standard", "vcodec": "none", "acodec": "mp3", "abr": 128, "filesize": 481115, "url": "http://m802.music.126.net/x.mp3"}]});
        assert_eq!(preview("https://music.163.com/#/song?id=287398", &ne).unwrap().message, "这是会员内容，没登录只能下到 30 秒的试听片段，完整版要登录开了会员的网易云音乐账号");
        // 普通的歌、普通的视频不算
        let ok = json!({"duration": 320, "vcodec": "none", "abr": 128, "filesize": 5_120_000, "formats": [{"format_id": "standard", "vcodec": "none", "url": "https://a/x.mp3"}]});
        assert!(preview("https://music.163.com/#/song?id=1", &ok).is_none());
        let yt = json!({"duration": 212, "vcodec": "avc1", "formats": [{"format_id": "18", "url": "https://rr3---sn.googlevideo.com/videoplayback"}, {"format_id": "140", "vcodec": "none", "url": "https://rr3---sn.googlevideo.com/videoplayback"}]});
        assert!(preview("https://www.youtube.com/watch?v=dQw4w9WgXcQ", &yt).is_none());
        // youtube-f7f8d2：付费电影，yt-dlp 读到的是预告片（id 不一样）
        let movie = json!({"id": "XbR2BJZyaRo", "extractor_key": "Youtube", "duration": 147, "title": "KUNG FU PANDA 4", "channel": "YouTube Movies"});
        let f = youtube_paid("https://www.youtube.com/watch?v=TrD-Woa_8Rk", &movie).unwrap();
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Drm, "这是付费电影，没购买只能下到 2 分 27 秒的预告片；完整版受版权保护（DRM），Captae 不能下载"));
        assert!(youtube_paid("https://www.youtube.com/watch?v=XbR2BJZyaRo", &movie).is_none());
        // 播放列表嵌入地址 /embed/videoseries：路径里的「videoseries」正好 11 个字符，像一个视频 id。
        // 读到的是跟过去的那一条，按实际读的地址比，不会误报付费电影
        assert!(youtube_paid("https://www.youtube.com/watch?v=XbR2BJZyaRo", &movie).is_none() && youtube_id("https://www.youtube.com/embed/videoseries?list=PLx").as_deref() == Some("videoseries"));
        assert_eq!(human_len(4849.0), "80 分 49 秒");
        assert_eq!(human_len(300.0), "5 分钟");
    }

    /// FIXES 第 6 项：通用提取器当成直链的文件，看开头几个字节
    #[test]
    fn file_magic() {
        assert_eq!(magic(b"%PDF-1.7\n%\xe2\xe3"), Magic::Not("PDF 文档"));
        assert_eq!(magic(b"PK\x03\x04\x14\x00\x00\x00"), Magic::Not("压缩包"));
        assert_eq!(magic("{\"code\":-404,\"message\":\"啥都木有\"}".as_bytes()), Magic::Not("一段数据（JSON）"));
        assert_eq!(magic(b"<!DOCTYPE html><html>"), Magic::Not("网页"));
        assert_eq!(magic(b"5f32a366  yt-dlp\n0123abcd  yt-dlp.exe\n"), Magic::Not("文本"));
        assert_eq!(magic(b"\x00\x00\x00\x20ftypisom\x00\x00\x02\x00"), Magic::Media);
        assert_eq!(magic(&[0x1a, 0x45, 0xdf, 0xa3, 0x01]), Magic::Media);
        assert_eq!(magic(b"ID3\x04\x00"), Magic::Media);
        assert_eq!(magic(&[0xff, 0xfb, 0x90, 0x64]), Magic::Media);
        assert_eq!(magic(&[0xff, 0xd8, 0xff, 0xe0]), Magic::Media);
        assert_eq!(magic(&[0x00, 0x01, 0x02, 0x9f, 0x80]), Magic::Unknown);
    }

    /// FIXES 第 14 项：YouTube 频道主页的「条目」是标签页（youtube-a237de 的原始 JSON 里是三个嵌套的 playlist）
    #[test]
    fn channel_tabs() {
        use serde_json::json;
        let ch = json!({"_type": "playlist", "id": "UCBR8-60-B28hp2BmDPdntcQ", "title": "YouTube", "channel": "YouTube", "webpage_url": "https://www.youtube.com/@YouTube",
            "entries": [{"_type": "playlist", "id": "UCBR8-60-B28hp2BmDPdntcQ", "title": "YouTube - Videos"},
                        {"_type": "playlist", "id": "UCBR8-60-B28hp2BmDPdntcQ", "title": "YouTube - Live"},
                        {"_type": "playlist", "id": "UCBR8-60-B28hp2BmDPdntcQ", "title": "YouTube - Shorts"}]});
        let Ok(Interp::Tab { url, channel, label }) = interpret(ch.clone(), "https://www.youtube.com/@YouTube", None, false) else { panic!() };
        assert_eq!(label, "视频");
        assert_eq!((url.as_str(), channel.as_str()), ("https://www.youtube.com/@YouTube/videos", "YouTube"));
        // x-86abcd：t.co → bit.ly → 频道页，按 yt-dlp 最后落到的地址
        assert!(matches!(interpret(ch, "https://t.co/CjqYlRu1cK", None, false), Ok(Interp::Tab { url, .. }) if url == "https://www.youtube.com/@YouTube/videos"));
        let c2 = json!({"_type": "playlist", "webpage_url": "https://www.youtube.com/channel/UCabc/featured", "entries": [{"_type": "url", "url": "https://www.youtube.com/channel/UCabc/videos"}, {"_type": "url", "url": "https://www.youtube.com/channel/UCabc/shorts"}]});
        assert!(matches!(interpret(c2, "https://www.youtube.com/channel/UCabc", None, false), Ok(Interp::Tab { url, .. }) if url == "https://www.youtube.com/channel/UCabc/videos"));
        // 只有 Shorts 的频道：读 Shorts 标签，不去读不存在的「视频」标签
        let shorts = json!({"_type": "playlist", "channel": "S", "webpage_url": "https://www.youtube.com/@onlyshorts", "entries": [{"_type": "playlist", "title": "S - Shorts"}, {"_type": "playlist", "title": "S - Live"}]});
        assert!(matches!(interpret(shorts, "https://www.youtube.com/@onlyshorts", None, false), Ok(Interp::Tab { url, label: "Shorts", .. }) if url == "https://www.youtube.com/@onlyshorts/shorts"));
        // 普通播放列表不受影响
        let pl = json!({"_type": "playlist", "webpage_url": "https://www.youtube.com/playlist?list=PLx", "entries": [{"_type": "url", "url": "https://www.youtube.com/watch?v=mC73amqYFpQ"}, {"_type": "url", "url": "https://www.youtube.com/watch?v=eXOYqJ6XdqA"}]});
        assert!(matches!(interpret(pl, "https://www.youtube.com/playlist?list=PLx", None, false), Ok(Interp::Done(Probed::List { .. }))));
    }

    /// FIXES 第 18 项：B 站多 P（bilibili-d0e5fb，--yes-playlist --flat-playlist 的条目是 ?p=1、?p=2）
    /// 集成和收尾第 4 项：写了 `?p=` 的 B 站链接，分 P 列表里有几 P（接口回来的 JSON）；只有 1 P 的不算分 P 视频
    #[test]
    fn bilibili_part_count() {
        let v = serde_json::json!({"code": 0, "message": "0", "data": [{"cid": 1, "page": 1, "part": "a", "duration": 60}, {"cid": 2, "page": 2, "part": "b", "duration": 70}, {"cid": 3, "page": 3, "part": "c", "duration": 80}]});
        assert_eq!(pagelist_count(&v), Some(3));
        assert_eq!(pagelist_count(&serde_json::json!({"code": -404, "message": "啥都木有"})), None);
        assert_eq!(pagelist_count(&serde_json::json!({"code": 0, "data": []})), None);
        let r = bilibili_set_of("https://www.bilibili.com/video/BV1LpD3YsETa?p=3").unwrap();
        assert_eq!((r.index, r.count), (Some(3), None));
        // 标成直播：下载时 yt-dlp 读到 is_live，交给 ffmpeg 一直录
        let mut v = serde_json::json!({"id": "x", "duration": 0.0, "url": "https://a/live.m3u8", "title": "christmas lofi radio 🎄 cozy ambience 2026-11-24 06:11"});
        mark_live(&mut v);
        assert_eq!((v["is_live"].as_bool(), v["live_status"].as_str(), v.get("duration")), (Some(true), Some("is_live"), None));
        // 读取时 yt-dlp 已经给标题加过日期，下载时还会再加：存盘前去掉读取时加的那个
        assert_eq!(v["title"].as_str(), Some("christmas lofi radio 🎄 cozy ambience"));
        assert_eq!(strip_live_date("2026 年终直播"), "2026 年终直播");
    }

    #[test]
    fn bilibili_multi_part() {
        use serde_json::json;
        assert!(bilibili_parts("https://www.bilibili.com/video/BV1LpD3YsETa/"));
        assert!(!bilibili_parts("https://www.bilibili.com/video/BV1LpD3YsETa/?p=2"));
        assert!(!bilibili_parts("https://www.bilibili.com/bangumi/play/ep123"));
        let r = bilibili_set_of("https://www.bilibili.com/video/BV1LpD3YsETa/?p=2&spm_id_from=333").unwrap();
        assert_eq!((r.id.as_str(), r.url.as_str()), ("BV1LpD3YsETa", "https://www.bilibili.com/video/BV1LpD3YsETa/?spm_id_from=333"));
        assert!(bilibili_set_of("https://www.bilibili.com/video/BV1LpD3YsETa/").is_none());
        let v = json!({"_type": "playlist", "id": "BV1LpD3YsETa", "title": "【教学】b站视频 手机怎么分集分p ?", "entries": [
            {"_type": "url", "ie_key": "BiliBili", "url": "https://www.bilibili.com/video/BV1LpD3YsETa?p=1"},
            {"_type": "url", "ie_key": "BiliBili", "url": "https://www.bilibili.com/video/BV1LpD3YsETa?p=2"}]});
        let Ok(Interp::Done(Probed::List { entries, .. })) = interpret(v, "https://www.bilibili.com/video/BV1LpD3YsETa/", None, false) else { panic!() };
        assert_eq!(entries.len(), 2);
        assert_eq!((entries[1].url.as_str(), entries[1].item), ("https://www.bilibili.com/video/BV1LpD3YsETa?p=2", None));
    }

    /// FIXES 第 8、16 项和竖屏：下载参数
    #[test]
    fn args_shape() {
        let r = AddRequest { format: Some("MP4".into()), quality: Some("1080p".into()), ..Default::default() };
        let tmp = temp_dir(Path::new("/o"), "https://archive.org/details/Popeye_forPresident", None);
        assert!(tmp.starts_with("/o/.captae-tmp/") && tmp != Path::new("/o/.captae-tmp"));
        assert_eq!(tmp, temp_dir(Path::new("/o"), "https://archive.org/details/Popeye_forPresident", None), "暂停后继续还是同一个目录");
        let a = download_args(&r, Path::new("/o"), &tmp, None, None, false, Shape::default(), true);
        let at = |k: &str| a.iter().position(|x| x == k).map(|i| a[i + 1].clone());
        // 编码未知时按容器排：mp4 先于 ogv
        assert_eq!(at("-S").as_deref(), Some("vcodec:h264,ext:mp4:m4a,res,acodec:m4a"));
        assert_eq!(at("--remux-video").as_deref(), Some("mp4"));
        assert_eq!(at("-P").as_deref(), Some("/o"));
        assert!(a.contains(&format!("temp:{}", tmp.display())));
        // 转封装失败以后重下：不让 yt-dlp 转，交给 Captae
        let a = download_args(&r, Path::new("/o"), &tmp, None, None, false, Shape::default(), false);
        assert!(!a.contains(&"--remux-video".to_string()) && !a.contains(&"--merge-output-format".to_string()));
        // 单张图片：不装进 MP4
        let a = download_args(&r, Path::new("/o"), &tmp, None, None, false, Shape { vertical: false, image: true }, true);
        assert!(!a.contains(&"-S".to_string()) && !a.contains(&"--remux-video".to_string()));
        // 竖屏：按宽度选
        let a = download_args(&r, Path::new("/o"), &tmp, None, None, false, Shape { vertical: true, image: false }, true);
        assert!(a.iter().any(|x| x.contains("[width<=1080]")));
        let info = MediaInfo { width: Some(864), height: Some(1920), ext: Some("mp4".into()), ..Default::default() };
        assert!(Shape::of(Some(&info)).vertical);
        let pic = MediaInfo { ext: Some("jpg".into()), ..Default::default() };
        assert!(Shape::of(Some(&pic)).image && !Shape::of(None).image);
    }

    /// giphy-1d28e6：媒体地址看到 Accept: text/html 就回网页
    #[test]
    fn giphy_accept() {
        use serde_json::json;
        let mut v = json!({"url": "https://media0.giphy.com/media/v1.x/TkpA7ELrXTnueydAqv/giphy1080p.mp4?cc=en", "http_headers": {"Accept": "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8", "Sec-Fetch-Mode": "navigate", "Referer": "https://giphy.com/clips/x"},
            "formats": [{"url": "https://media0.giphy.com/media/v1.x/a/giphy.mp4", "http_headers": {"Accept": "text/html", "Sec-Fetch-Mode": "navigate"}}]});
        prepare_info(&mut v);
        assert_eq!(v["http_headers"]["Accept"], "*/*");
        assert!(v["http_headers"].get("Sec-Fetch-Mode").is_none() && v["http_headers"]["Referer"] == "https://giphy.com/clips/x");
        assert_eq!(v["formats"][0]["http_headers"]["Accept"], "*/*");
        let mut yt = json!({"url": "https://rr3---sn.googlevideo.com/videoplayback", "http_headers": {"Accept": "text/html"}});
        prepare_info(&mut yt);
        assert_eq!(yt["http_headers"]["Accept"], "text/html", "别的网站不动");
    }

    #[test]
    fn args_audio() {
        let r = AddRequest { audio_only: true, format: Some("MP3".into()), ..Default::default() };
        let a = download_args(&r, Path::new("/o"), Path::new("/o/.captae-tmp/x"), None, None, false, Shape::default(), true);
        assert!(a.windows(2).any(|w| w[0] == "--audio-format" && w[1] == "mp3"));
        assert!(a.contains(&"-x".to_string()));
    }
}
