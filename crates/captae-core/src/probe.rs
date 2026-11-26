//! 下载前看一眼：标题、时长、封面、每种清晰度大概多大。
//! 浏览器插件的清晰度菜单、复制链接提示和弹出面板用它显示「1080p · 约 420 MB」。
//!
//! - 有提取器的网站（YouTube、B 站……）：`yt-dlp -J`，按格式表估算每个清晰度的大小
//! - M3U8：读主列表和最高清晰度的分段列表，按码率 × 时长估算
//! - 媒体直链：HEAD 拿大小
//!
//! 结果缓存 10 分钟：同一个视频，菜单、面板、复制提示会各问一次。
//!
//! 播放列表、多视频帖子：`choose: true` 加 `entries`（标题、时长、缩略图），qualities 是空的。
//! 失败时错误链里带着 `errors::Failure`（错误码、一句话、原文），插件宿主和命令行据此返回错误码。
//!
//! 插件可以带上浏览器里这个网站的 cookies（`probe_ext`）：YouTube 照样只在报要登录 / 要验证以后才带。

use crate::detect::{self, Kind};
use crate::errors::ErrorCode;
use crate::login::{Auth, Cookie, Jar};
use crate::model::{Entry, PlaylistRef};
use crate::tools::Toolbox;
use crate::tr;
use anyhow::{anyhow, Result};
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Default, PartialEq)]
pub struct Quality {
    /// 1080p
    pub label: String,
    pub height: Option<u32>,
    /// 估算的字节数
    pub bytes: Option<u64>,
    /// M3U8 这一路的分段列表地址，插件选定清晰度后直接下这一路
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    /// M3U8 这一路的声音在单独的列表里（直接下 uri 会没有声音）
    #[serde(rename = "separateAudio", skip_serializing_if = "std::ops::Not::not")]
    pub separate_audio: bool,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct ProbeInfo {
    /// page / hls / file
    pub kind: String,
    pub title: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub site: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// 从高到低
    pub qualities: Vec<Quality>,
    /// 「只要声音」大概多大
    pub audio: Option<u64>,
    /// 直链文件的大小
    pub size: Option<u64>,
    /// 播放列表、多视频帖子：要用户选条目
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub choose: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub entries: Vec<Entry>,
    /// YouTube `watch?v=…&list=…`：这个视频属于哪个播放列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub playlist: Option<PlaylistRef>,
    /// 直播（`record_live` 时才会读到）
    #[serde(rename = "isLive", skip_serializing_if = "std::ops::Not::not")]
    pub is_live: bool,
}

type Cache = Mutex<HashMap<String, (Instant, ProbeInfo)>>;
static CACHE: once_cell::sync::Lazy<Cache> = once_cell::sync::Lazy::new(Default::default);
const TTL: Duration = Duration::from_secs(600);

fn cached(url: &str) -> Option<ProbeInfo> {
    let c = CACHE.lock().unwrap();
    c.get(url).filter(|(t, _)| t.elapsed() < TTL).map(|(_, v)| v.clone())
}

fn remember(url: &str, v: &ProbeInfo) {
    let mut c = CACHE.lock().unwrap();
    c.retain(|_, (t, _)| t.elapsed() < TTL);
    if c.len() > 200 {
        c.clear();
    }
    c.insert(url.to_string(), (Instant::now(), v.clone()));
}

pub async fn probe(tools: &Toolbox, http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>) -> Result<ProbeInfo> {
    probe_with(tools, http, url, headers, &Auth::default()).await
}

/// 插件带着浏览器里的 cookies 来问（`cookies` 是 chrome.cookies.getAll 的结果，`user_agent` 是浏览器的）。
/// YouTube：先不带，报要登录 / 要验证再带上读一次（和下载时的规矩一样）。临时 cookies 文件放 `dir`，用完就删
pub async fn probe_ext(tools: &Toolbox, http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, cookies: Vec<Cookie>, user_agent: Option<String>, dir: PathBuf) -> Result<ProbeInfo> {
    if cookies.is_empty() {
        return probe(tools, http, url, headers).await;
    }
    let auth = crate::login::extension_auth(cookies, user_agent, tools, dir).await;
    if !crate::login::is_youtube(url) {
        return probe_with(tools, http, url, headers, &auth).await;
    }
    match probe(tools, http, url, headers).await {
        Err(e) if crate::errors::failure_in(&e).is_some_and(|f| matches!(f.code, ErrorCode::NeedLogin | ErrorCode::NeedVerify)) => probe_with(tools, http, url, headers, &auth).await,
        r => r,
    }
}

async fn probe_with(tools: &Toolbox, http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, auth: &Auth) -> Result<ProbeInfo> {
    // 跳转包装、B 站语言前缀、Dropbox 分享页、Commons 文件页先理顺（detect::canonical）
    let url = &crate::detect::canonical(url);
    // 带了 cookies 的结果另外缓存（登录以后能看到的清晰度可能更多）
    let key = if auth.is_empty() { url.to_string() } else { format!("{url}#login") };
    if let Some(v) = cached(&key) {
        return Ok(v);
    }
    let d = detect::detect(url).ok_or_else(|| anyhow!(tr!("链接不完整")))?;
    let jar = auth.jar();
    let v = match d.kind {
        Kind::Hls => hls(http, url, headers, &jar).await?,
        Kind::File => match file(http, url, headers, &jar).await? {
            Some(v) => v,
            // 地址以 .mp4/.jpg 结尾、打开却是网页（Dropbox 分享页、Commons 文件页）：交给下载引擎读（FIXES 第 9 项）
            None => page(tools, http, url, headers, auth).await?,
        },
        Kind::Page => page(tools, http, url, headers, auth).await?,
    };
    remember(&key, &v);
    Ok(v)
}

async fn page(tools: &Toolbox, http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, auth: &Auth) -> Result<ProbeInfo> {
    // 读取本身最多 2 分钟，还可能自动重试，这里多留一点
    tokio::time::timeout(Duration::from_secs(300), ytdlp(tools, http, url, headers, auth))
        .await
        .map_err(|_| anyhow::Error::new(crate::errors::Failure::new(crate::errors::ErrorCode::Network, tr!("读取视频信息太久了，网络可能不稳定"), format!("probe timed out: {url}"))))?
}

async fn ytdlp(tools: &Toolbox, http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, auth: &Auth) -> Result<ProbeInfo> {
    use crate::engine::ytdlp::{probe_url, ProbeReq, Probed};
    let note = |s: String| tracing::info!("读取 {url}：{s}");
    let r = ProbeReq { url, headers, item: None, record_live: false, note: &note, auth: (!auth.is_empty()).then_some(auth), extra: vec![] };
    Ok(match probe_url(tools, http, &r).await? {
        Probed::Video { raw, playlist, .. } => ProbeInfo { playlist, ..from_ytdlp(&raw) },
        Probed::List { title, entries } => ProbeInfo { kind: "page".into(), title, thumbnail: entries.iter().find_map(|e| e.thumb.clone()), choose: true, entries, ..Default::default() },
    })
}

/// yt-dlp `-J` 的结果 → 各清晰度大小。单独拆出来方便测试。
pub fn from_ytdlp(v: &Value) -> ProbeInfo {
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_string);
    let dur = v.get("duration").and_then(Value::as_f64);
    let formats = v.get("formats").and_then(Value::as_array).cloned().unwrap_or_default();
    let size_of = |f: &Value| -> Option<u64> {
        f.get("filesize").and_then(Value::as_u64).or_else(|| f.get("filesize_approx").and_then(Value::as_u64)).or_else(|| {
            let tbr = f.get("tbr").and_then(Value::as_f64)?;
            Some((tbr * 1000.0 / 8.0 * dur?) as u64)
        })
    };
    let codec = |f: &Value, k: &str| f.get(k).and_then(Value::as_str).map(|c| c != "none");
    // 只有声音的格式里挑最好的一路
    let audio = formats
        .iter()
        .filter(|f| codec(f, "vcodec") == Some(false) && codec(f, "acodec") != Some(false))
        .max_by(|a, b| {
            let r = |f: &Value| f.get("abr").or_else(|| f.get("tbr")).and_then(Value::as_f64).unwrap_or(0.0);
            r(a).partial_cmp(&r(b)).unwrap_or(std::cmp::Ordering::Equal)
        });
    let audio_bytes = audio.and_then(size_of);
    // 每个清晰度取码率最高的视频格式；没有声音的加上声音那一路。
    // 清晰度按短边叫：竖屏的 1080×1920 是 1080p，不是 1920p（插件发现：B 站竖屏列成「1920p、854p、640p」）
    let mut best: BTreeMap<u32, (f64, Option<u64>)> = BTreeMap::new();
    for f in &formats {
        if codec(f, "vcodec") == Some(false) {
            continue;
        }
        let Some(h) = short_edge(f).filter(|h| *h >= 144) else { continue };
        let tbr = f.get("tbr").and_then(Value::as_f64).unwrap_or(0.0);
        let mut bytes = size_of(f);
        if codec(f, "acodec") == Some(false) {
            bytes = bytes.map(|b| b + audio_bytes.unwrap_or(0));
        }
        let e = best.entry(h).or_insert((-1.0, None));
        if tbr > e.0 || (e.1.is_none() && bytes.is_some()) {
            *e = (tbr, bytes);
        }
    }
    let mut qualities: Vec<Quality> = best.into_iter().rev().map(|(h, (_, bytes))| Quality { label: format!("{h}p"), height: Some(h), bytes, ..Default::default() }).collect();
    if qualities.is_empty() {
        // 没有格式表（直链网页之类）：至少给出整体大小
        if let Some(h) = short_edge(v) {
            qualities.push(Quality { label: format!("{h}p"), height: Some(h), bytes: size_of(v), ..Default::default() });
        }
    }
    ProbeInfo {
        kind: "page".into(),
        title: s("title"),
        duration: dur,
        thumbnail: s("thumbnail"),
        site: s("extractor_key").or_else(|| s("extractor")),
        width: v.get("width").and_then(Value::as_u64).map(|x| x as u32),
        height: qualities.first().and_then(|q| q.height).or_else(|| short_edge(v)),
        qualities,
        audio: audio_bytes,
        size: None,
        is_live: v.get("live_status").and_then(Value::as_str) == Some("is_live"),
        ..Default::default()
    }
}

/// 格式的清晰度数字：宽高都有时取短边，只有高度时用高度
pub fn short_edge(f: &Value) -> Option<u32> {
    let n = |k: &str| f.get(k).and_then(Value::as_u64).map(|x| x as u32).filter(|x| *x > 0);
    match (n("width"), n("height")) {
        (Some(w), Some(h)) => Some(w.min(h)),
        (_, h) => h,
    }
}

async fn hls(http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, jar: &Jar) -> Result<ProbeInfo> {
    let p = crate::engine::hls::probe(http, url, headers, jar).await?;
    let qualities = p
        .variants
        .iter()
        .map(|v| Quality {
            label: v.p.clone(),
            height: v.p.trim_end_matches('p').parse().ok(),
            bytes: (v.mb > 0.0).then(|| (v.mb * 1e6) as u64),
            uri: Some(v.uri.clone()),
            separate_audio: v.audio.is_some(),
        })
        .collect::<Vec<_>>();
    // 只要声音：粗估 128 kbps
    let audio = p.info.duration.map(|d| (d * 128_000.0 / 8.0) as u64);
    Ok(ProbeInfo { kind: "hls".into(), duration: p.info.duration, height: p.info.height, qualities, audio, ..Default::default() })
}

/// 直链：HEAD 拿大小。打开是网页时返回 None（交给下载引擎）；电台流按直播报错（FIXES 第 13 项）
async fn file(http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, jar: &Jar) -> Result<Option<ProbeInfo>> {
    let h = crate::engine::aria2::head(http, url, headers, jar).await?;
    if h.is_html() {
        return Ok(None);
    }
    if h.content_type.as_deref().is_some_and(|c| c.to_ascii_lowercase().contains("mpegurl")) {
        // 地址没有 .m3u8 后缀，但其实是 M3U8
        return hls(http, url, headers, jar).await.map(Some);
    }
    if h.is_stream() {
        return Err(anyhow::Error::new(crate::errors::radio(format!("audio stream without length (icy={}): {url}", h.icy))));
    }
    // HEAD 的回应没有正文，reqwest 的 content_length() 给不出大小，直接读头
    let size = match h.size {
        Some(s) => Some(s),
        None => {
            let mut r = http.head(url);
            for (k, v) in headers {
                r = r.header(k, v);
            }
            r = jar.apply(r, url, headers);
            r.send().await.ok().and_then(|r| r.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|s| s.parse().ok())).filter(|n: &u64| *n > 0)
        }
    };
    Ok(Some(ProbeInfo { kind: "file".into(), title: Some(h.filename), size, ..Default::default() }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn ytdlp_sizes() {
        let v = json!({
            "title": "Sintel", "duration": 100.0, "thumbnail": "https://i/x.jpg", "extractor_key": "Youtube",
            "formats": [
                {"format_id": "140", "vcodec": "none", "acodec": "mp4a", "abr": 128.0, "filesize": 1_600_000},
                {"format_id": "139", "vcodec": "none", "acodec": "mp4a", "abr": 48.0, "filesize": 600_000},
                {"format_id": "137", "vcodec": "avc1", "acodec": "none", "height": 1080, "tbr": 4000.0, "filesize": 50_000_000},
                {"format_id": "248", "vcodec": "vp9", "acodec": "none", "height": 1080, "tbr": 2500.0, "filesize": 31_000_000},
                {"format_id": "22", "vcodec": "avc1", "acodec": "mp4a", "height": 720, "tbr": 2000.0},
                {"format_id": "sb0", "vcodec": "none", "acodec": "none", "format_note": "storyboard"},
                {"format_id": "18", "vcodec": "avc1", "acodec": "mp4a", "height": 360, "tbr": 600.0, "filesize_approx": 7_500_000}
            ]
        });
        let p = from_ytdlp(&v);
        assert_eq!(p.title.as_deref(), Some("Sintel"));
        assert_eq!(p.height, Some(1080));
        let labels: Vec<_> = p.qualities.iter().map(|q| (q.label.as_str(), q.bytes)).collect();
        // 1080p：最高码率的那一路 + 最好的声音；720p 没有大小就按码率 × 时长
        assert_eq!(labels, vec![("1080p", Some(51_600_000)), ("720p", Some(25_000_000)), ("360p", Some(7_500_000))]);
        assert_eq!(p.audio, Some(1_600_000));
    }

    /// 竖屏的清晰度按短边叫（插件发现：B 站竖屏列成「1920p、854p、640p」）。bilibili-d0e5fb 是 864×1920
    #[test]
    fn vertical_labels() {
        let v = json!({"title": "竖屏", "duration": 100.0, "width": 864, "height": 1920, "formats": [
            {"format_id": "100026", "vcodec": "av01", "acodec": "none", "width": 864, "height": 1920, "tbr": 1000.0},
            {"format_id": "100024", "vcodec": "av01", "acodec": "none", "width": 480, "height": 854, "tbr": 500.0},
            {"format_id": "100023", "vcodec": "av01", "acodec": "none", "width": 360, "height": 640, "tbr": 300.0},
            {"format_id": "30280", "vcodec": "none", "acodec": "mp4a", "abr": 128.0, "filesize": 1_600_000}]});
        let p = from_ytdlp(&v);
        assert_eq!(p.qualities.iter().map(|q| q.label.as_str()).collect::<Vec<_>>(), ["864p", "480p", "360p"]);
        assert_eq!(p.height, Some(864));
        // 横屏照旧按高度
        let h = json!({"duration": 10.0, "formats": [{"vcodec": "avc1", "width": 1920, "height": 1080, "tbr": 4000.0}, {"vcodec": "avc1", "width": 1920, "height": 800, "tbr": 3000.0}]});
        assert_eq!(from_ytdlp(&h).qualities.iter().map(|q| q.label.as_str()).collect::<Vec<_>>(), ["1080p", "800p"]);
        assert_eq!(short_edge(&json!({"height": 720})), Some(720));
    }

    #[test]
    fn cache_roundtrip() {
        let v = ProbeInfo { kind: "file".into(), size: Some(3), ..Default::default() };
        remember("https://cache.test/a.mp4", &v);
        assert_eq!(cached("https://cache.test/a.mp4").unwrap().size, Some(3));
        assert!(cached("https://cache.test/b.mp4").is_none());
    }
}
