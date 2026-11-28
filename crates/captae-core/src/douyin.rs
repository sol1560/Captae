//! 抖音：从看不见的网页里读视频信息（下载引擎读不了；看不见的网页见 `webpage.rs`，只有桌面端有）。
//!
//! yt-dlp 的抖音提取器只带视频编号去请求 `/aweme/v1/web/aweme/detail/`，抖音要网页自己算的签名参数（a_bogus 等），不带就回 403，
//! 模拟 Safari 时 504；cookies 多新、登没登录都一样（yt-dlp 源码里这里写着 TODO，2026-11-28 用 2026.11.23 实测）。
//! 网页自己请求到的详情里有各档清晰度的 mp4 直链（不带 Referer 也能下，带声音），所以在看不见的网页里打开视频页、接住这份详情，
//! 换成 yt-dlp 的信息 JSON（格式表照 yt-dlp `_parse_aweme_video_app` 的做法），后面选清晰度、下载、转格式、入库都照旧走 `--load-info-json`。
//!
//! 网页自己的详情请求有时会被抖音拦下（回 `Blocked by ArgusSecurityPlugin Sign Invalid`，网页改播推荐流里别的视频）：
//! 2026-11-28 离屏连开 8 次被拦 3 次。被拦就重新打开，最多 [`ATTEMPTS`] 次；都不行按「要验证」报
//! （在登录窗口里打开一次抖音网页，那份存储里的状态会好一些，读取用的是同一份）。

use crate::engine::ytdlp::{info_from_json, Probed};
use crate::errors::{self, ErrorCode, Failure};
use crate::tr;
use crate::webpage::{self, Capture, Verdict};
use serde_json::{json, Map, Value};
use std::time::Duration;

/// 最多打开几次网页
pub const ATTEMPTS: usize = 3;
/// 每次最多等多久（网页 850 KB，详情一般几秒内就请求到了）
const PAGE_TIMEOUT: Duration = Duration::from_secs(20);
/// 同一个网页里被拦几次就不等了（网页自己会再请求四五次，然后改播推荐流）
const BLOCKED_ENOUGH: usize = 3;
/// 标题最多多少个字（和 yt-dlp 一样；完整的文字放在简介里）
const TITLE_CHARS: usize = 72;

fn host_of(url: &str) -> Option<(String, url::Url)> {
    let u = url::Url::parse(url.trim()).ok()?;
    let h = u.host_str()?.to_ascii_lowercase();
    Some((h, u))
}

fn under(host: &str, d: &str) -> bool {
    host == d || host.ends_with(&format!(".{d}"))
}

/// 抖音的分享短链（`v.douyin.com/xxxx/`），要联网跟一次跳转才知道是哪个视频
pub fn is_short(url: &str) -> bool {
    host_of(url).is_some_and(|(h, _)| h == "v.douyin.com")
}

/// 链接里的视频编号：`douyin.com/video/<id>`、`iesdouyin.com/share/video/<id>`、`douyin.com/…?modal_id=<id>`（精选页、个人主页里点开的视频）。
/// 图集（`/note/`）、直播（`live.douyin.com`）、短链不算
pub fn video_id(url: &str) -> Option<String> {
    let (h, u) = host_of(url)?;
    if !(under(&h, "douyin.com") || under(&h, "iesdouyin.com")) || h == "live.douyin.com" || h == "v.douyin.com" {
        return None;
    }
    let digits = |s: &str| (s.len() >= 6 && s.chars().all(|c| c.is_ascii_digit())).then(|| s.to_string());
    let segs: Vec<&str> = u.path_segments().map(|s| s.filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    let from_path = match segs.as_slice() {
        ["video", id, ..] | ["share", "video", id, ..] => digits(id),
        _ => None,
    };
    from_path.or_else(|| u.query_pairs().find(|(k, _)| k == "modal_id").and_then(|(_, v)| digits(&v)))
}

/// 这个链接下载时由这里读（桌面端、抖音的视频或短链）。下载前信息过期了要重新读时，yt-dlp 自己读不了，得再从网页读一次
pub fn handles(url: &str) -> bool {
    webpage::reader().is_some() && (video_id(url).is_some() || is_short(url))
}

/// 短链跟一次跳转，返回落到的地址
async fn follow(http: &reqwest::Client, url: &str) -> Option<String> {
    let r = tokio::time::timeout(Duration::from_secs(15), http.get(url).send()).await.ok()?.ok()?;
    Some(r.url().to_string())
}

/// 抖音的视频：从看不见的网页里读。不是抖音的视频、或者没有看不见的网页（命令行、手机）时返回 None，照旧交给下载引擎
pub async fn probe(http: &reqwest::Client, url: &str, note: &(dyn Fn(String) + Send + Sync)) -> Option<Result<Probed, Failure>> {
    let reader = webpage::reader()?;
    let id = match video_id(url) {
        Some(id) => id,
        None if is_short(url) => video_id(&follow(http, url).await?)?,
        None => return None,
    };
    let page = format!("https://www.douyin.com/video/{id}");
    tracing::info!("抖音 {id}：下载引擎读不了，从看不见的网页里读（{page}）");
    let mut last: Option<String> = None;
    for n in 1..=ATTEMPTS {
        let want = format!("aweme_id={id}");
        let mut blocked = 0usize;
        let judge = Box::new(move |api: &str, body: &str| {
            // 网页还会请求推荐视频的详情：只要这个视频的
            if !api.contains(&want) {
                return Verdict::Wait;
            }
            if usable(body) {
                return Verdict::Take;
            }
            blocked += 1;
            if blocked >= BLOCKED_ENOUGH {
                Verdict::Stop
            } else {
                Verdict::Wait
            }
        });
        let seen = match reader.capture(Capture { url: page.clone(), timeout: PAGE_TIMEOUT, judge }).await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("抖音 {id}：看不见的网页打不开：{e:#}");
                return None;
            }
        };
        tracing::info!("抖音 {id}：第 {n} 次，接住 {} 份详情，{}", seen.count, if seen.taken.is_some() { "拿到了" } else { "没拿到" });
        if let Some(body) = seen.taken {
            let detail: Value = match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(e) => return Some(Err(errors::by_code(ErrorCode::Unknown, &page, format!("douyin page: bad aweme detail JSON: {e}")))),
            };
            return Some(to_info(&detail, &page).map(|raw| Probed::Video { info: info_from_json(&raw), raw, playlist: None, extra: vec![] }));
        }
        last = seen.last.or(last);
        if n < ATTEMPTS {
            note(tr!("抖音网页这次没给出视频信息，重新打开再试一次"));
        }
    }
    let raw = format!("douyin page: no aweme detail after {ATTEMPTS} tries (last: {}): {page}", last.as_deref().map(|s| s.chars().take(120).collect::<String>()).unwrap_or_else(|| "no response".into()));
    Some(Err(errors::by_code(ErrorCode::NeedVerify, &page, raw)))
}

/// 网页接住的这份详情能不能用：有视频的，或者明说了视频没了的（status_code 0、aweme_detail 是 null）。
/// 被拦下的（「Blocked by ArgusSecurityPlugin Sign Invalid」）、空的、别的错误都不能用
fn usable(body: &str) -> bool {
    let t = body.trim_start();
    if !t.starts_with('{') {
        return false;
    }
    let Ok(v) = serde_json::from_str::<Value>(t) else { return false };
    v.get("aweme_detail").is_some_and(Value::is_object) || (v.get("status_code").and_then(Value::as_i64) == Some(0) && v.get("aweme_detail").is_some_and(Value::is_null) && v.get("filter_detail").is_some())
}

fn s<'a>(v: &'a Value, path: &[&str]) -> Option<&'a str> {
    path.iter().try_fold(v, |v, k| v.get(*k))?.as_str().filter(|s| !s.is_empty())
}

fn n(v: &Value, path: &[&str]) -> Option<i64> {
    let x = path.iter().try_fold(v, |v, k| v.get(*k))?;
    x.as_i64().or_else(|| x.as_str().and_then(|s| s.parse().ok()))
}

/// 一组镜像地址里挑一个：先用 CDN 的，`www.douyin.com/aweme/v1/play/` 那种接口地址放最后（yt-dlp 说接口下载可能被拦）
fn pick_url(list: &Value) -> Option<String> {
    let urls: Vec<&str> = list.as_array()?.iter().filter_map(Value::as_str).filter(|u| u.starts_with("http")).collect();
    urls.iter().find(|u| !u.contains("aweme/v1")).or(urls.first()).map(|u| u.to_string())
}

/// 标题：简介从第一个话题（#…）或 @人 处截断，太长再截；截完是空的就用原文，原文也是空的用视频编号。
/// 话题常紧挨着前文（「撞个满怀。#虎扑俯卧撑」）；前面紧挨着英文字母、数字的 # 和 @ 不算（「C#」、邮箱）
fn title_of(desc: &str, id: &str) -> String {
    let mut prev: Option<char> = None;
    let mut cut = desc.len();
    for (i, c) in desc.char_indices() {
        if (c == '#' || c == '@') && !prev.is_some_and(|p| p.is_ascii_alphanumeric()) {
            cut = i;
            break;
        }
        prev = Some(c);
    }
    let t = desc[..cut].split_whitespace().collect::<Vec<_>>().join(" ");
    let t = t.trim();
    let t = if t.is_empty() { desc.trim() } else { t };
    let t = if t.is_empty() { id } else { t };
    if t.chars().count() > TITLE_CHARS {
        format!("{}...", t.chars().take(TITLE_CHARS - 3).collect::<String>())
    } else {
        t.to_string()
    }
}

/// 攒格式表：同一个地址（url_key）只要一次
struct Formats {
    list: Vec<Value>,
    keys: Vec<String>,
    headers: Value,
}

impl Formats {
    fn add(&mut self, id: String, addr: &Value, h265: bool, tbr: Option<f64>, fps: Option<i64>) {
        let key = s(addr, &["url_key"]).map(str::to_string).unwrap_or_else(|| id.clone());
        if key.contains("bytevc2") || self.keys.contains(&key) {
            return;
        }
        let Some(url) = addr.get("url_list").and_then(pick_url) else { return };
        self.keys.push(key);
        let mut f = Map::new();
        f.insert("format_id".into(), json!(id));
        f.insert("url".into(), json!(url));
        f.insert("ext".into(), json!("mp4"));
        f.insert("protocol".into(), json!("https"));
        f.insert("vcodec".into(), json!(if h265 { "h265" } else { "h264" }));
        f.insert("acodec".into(), json!("aac"));
        f.insert("http_headers".into(), self.headers.clone());
        for (k, x) in [("width", n(addr, &["width"])), ("height", n(addr, &["height"])), ("filesize", n(addr, &["data_size"])), ("fps", fps)] {
            if let Some(x) = x.filter(|x| *x > 0) {
                f.insert(k.into(), json!(x));
            }
        }
        if let Some(t) = tbr.filter(|t| *t > 0.0) {
            f.insert("tbr".into(), json!(t));
        }
        self.list.push(Value::Object(f));
    }
}

/// 去掉码率比最好的 h264 还低的 h265。抖音给的 2160p、1440p 多半是低码率的 h265（`adapt_lowest_4_1`：4K 只有 2.4 Mbps，
/// 1080p 的 h264 有 5.8 Mbps，2026-11-28 实测），清晰度菜单把它们排在最前面；存成 MP4 时下载又优先选 h264（`ytdlp::download_args` 的 -S），
/// 选了「2160p」下到的是 1080p，菜单上写的大小也对不上。没有 h264 时 h265 全留着；码率更高的 h265 照留
fn drop_weak_h265(list: Vec<Value>) -> Vec<Value> {
    let tbr = |f: &Value| f.get("tbr").and_then(Value::as_f64).unwrap_or(0.0);
    let h264 = |f: &Value| f.get("vcodec").and_then(Value::as_str) == Some("h264");
    let Some(best) = list.iter().filter(|f| h264(f)).map(tbr).reduce(f64::max) else { return list };
    list.into_iter().filter(|f| h264(f) || tbr(f) > best).collect()
}

/// 网页的详情 → yt-dlp 的信息 JSON（`--load-info-json` 用）。格式表照 yt-dlp `_parse_aweme_video_app`：
/// - 每档 `bit_rate` 一个格式，`is_h265` / `is_bytevc1` 是 h265，字节自家的 bytevc2（h266）放不了，不要
/// - `download_addr` 带水印（还是 720×720 的方块），不要；`play_addr` 和某一档重复，只有没有 `bit_rate` 时才用
/// - 都是带声音的 mp4（2026-11-28 实测：h264 + aac）
/// - 码率比最好的 h264 还低的 h265 不要（见 [`drop_weak_h265`]）
pub fn to_info(detail: &Value, page: &str) -> Result<Value, Failure> {
    let Some(a) = detail.get("aweme_detail").filter(|a| a.is_object()) else {
        let why = s(detail, &["filter_detail", "detail_msg"]).or_else(|| s(detail, &["filter_detail", "filter_reason"])).unwrap_or("aweme_detail is null");
        return Err(errors::by_code(ErrorCode::Unavailable, page, format!("douyin page: {why}: {page}")));
    };
    let id = s(a, &["aweme_id"]).map(str::to_string).or_else(|| video_id(page)).unwrap_or_default();
    if a.get("images").and_then(Value::as_array).is_some_and(|v| !v.is_empty()) {
        return Err(errors::image_post(format!("douyin page: image post {id}: {page}")));
    }
    let v = a.get("video").cloned().unwrap_or(Value::Null);
    // 只带来源；User-Agent 交给 yt-dlp（带登录状态时它按 --impersonate / --user-agent 定，写死在这里会和模拟的浏览器对不上）
    let headers = json!({"Referer": "https://www.douyin.com/"});
    let mut t = Formats { list: vec![], keys: vec![], headers };
    let is_h265 = |b: &Value| n(b, &["is_h265"]).unwrap_or(0) != 0 || n(b, &["is_bytevc1"]).unwrap_or(0) != 0;
    for b in v.get("bit_rate").and_then(Value::as_array).into_iter().flatten() {
        let Some(addr) = b.get("play_addr") else { continue };
        let fid = s(b, &["gear_name"]).map(str::to_string).unwrap_or_else(|| format!("br{}", n(b, &["bit_rate"]).unwrap_or(0)));
        t.add(fid, addr, is_h265(b), n(b, &["bit_rate"]).map(|x| x as f64 / 1000.0), n(b, &["FPS"]));
    }
    if t.list.is_empty() {
        if let Some(addr) = v.get("play_addr") {
            t.add("play_addr".into(), addr, is_h265(&v), None, None);
        }
    }
    let formats = drop_weak_h265(t.list);
    if formats.is_empty() {
        return Err(errors::by_code(ErrorCode::NoMedia, page, format!("douyin page: no playable address in aweme detail: {page}")));
    }
    let desc = s(a, &["desc"]).unwrap_or("");
    let nickname = s(a, &["author", "nickname"]);
    let sec_uid = s(a, &["author", "sec_uid"]);
    let cover = ["origin_cover", "cover"].iter().find_map(|k| v.get(*k).and_then(|c| c.get("url_list")).and_then(pick_url));
    let mut info = Map::new();
    info.insert("_type".into(), json!("video"));
    info.insert("id".into(), json!(id));
    info.insert("title".into(), json!(title_of(desc, &id)));
    info.insert("description".into(), json!(desc));
    info.insert("webpage_url".into(), json!(format!("https://www.douyin.com/video/{id}")));
    info.insert("original_url".into(), json!(page));
    info.insert("webpage_url_domain".into(), json!("douyin.com"));
    info.insert("extractor".into(), json!("Douyin"));
    info.insert("extractor_key".into(), json!("Douyin"));
    info.insert("ext".into(), json!("mp4"));
    info.insert("epoch".into(), json!(crate::util::now_ms() / 1000));
    let opt = [
        ("timestamp", n(a, &["create_time"]).map(|x| json!(x))),
        ("duration", n(&v, &["duration"]).filter(|d| *d > 0).map(|d| json!(d as f64 / 1000.0))),
        ("uploader", nickname.map(|x| json!(x))),
        ("channel", nickname.map(|x| json!(x))),
        ("uploader_id", s(a, &["author", "unique_id"]).or_else(|| s(a, &["author", "uid"])).map(|x| json!(x))),
        ("channel_id", sec_uid.map(|x| json!(x))),
        ("uploader_url", sec_uid.map(|x| json!(format!("https://www.douyin.com/user/{x}")))),
        ("thumbnail", cover.map(|x| json!(x))),
        ("width", n(&v, &["play_addr", "width"]).filter(|x| *x > 0).map(|x| json!(x))),
        ("height", n(&v, &["play_addr", "height"]).filter(|x| *x > 0).map(|x| json!(x))),
        ("like_count", n(a, &["statistics", "digg_count"]).map(|x| json!(x))),
        ("comment_count", n(a, &["statistics", "comment_count"]).map(|x| json!(x))),
        ("repost_count", n(a, &["statistics", "share_count"]).map(|x| json!(x))),
    ];
    for (k, x) in opt {
        if let Some(x) = x {
            info.insert(k.into(), x);
        }
    }
    info.insert("formats".into(), Value::Array(formats));
    Ok(Value::Object(info))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids() {
        assert_eq!(video_id("https://www.douyin.com/video/7693510493986868901?previous_page=web_code_link").as_deref(), Some("7693510493986868901"));
        assert_eq!(video_id("https://www.iesdouyin.com/share/video/7704278089505695333/?region=CN").as_deref(), Some("7704278089505695333"));
        assert_eq!(video_id("https://www.douyin.com/jingxuan?modal_id=7693510493986868901").as_deref(), Some("7693510493986868901"));
        assert_eq!(video_id("https://www.douyin.com/user/MS4wLjABAAAA?modal_id=7693510493986868901").as_deref(), Some("7693510493986868901"));
        // 图集、直播、短链、别的网站：不算
        for u in ["https://www.douyin.com/note/7657170973843024753", "https://live.douyin.com/747833069881", "https://v.douyin.com/AbCd1234/", "https://www.tiktok.com/@a/video/7693510493986868901", "https://www.douyin.com/video/abc"] {
            assert_eq!(video_id(u), None, "{u}");
        }
        assert!(is_short("https://v.douyin.com/AbCd1234/") && !is_short("https://www.douyin.com/video/1"));
    }

    #[test]
    fn titles() {
        assert_eq!(title_of("熟悉的飞车BGM配俯卧撑，青春和力量撞个满怀。#虎扑俯卧撑 #徒手健身 @东风奕派 #奕派M8", "1"), "熟悉的飞车BGM配俯卧撑，青春和力量撞个满怀。");
        assert_eq!(title_of("#只有话题 #没有正文", "1"), "#只有话题 #没有正文");
        // 紧挨着英文字母、数字的 # @ 不是话题
        assert_eq!(title_of("三天学会C#编程 写信给a@b.com #编程", "1"), "三天学会C#编程 写信给a@b.com");
        assert_eq!(title_of("", "7693510493986868901"), "7693510493986868901");
        let long = "长".repeat(100);
        assert_eq!(title_of(&long, "1").chars().count(), TITLE_CHARS);
    }

    #[test]
    fn usable_bodies() {
        assert!(usable(r#"{"aweme_detail":{"aweme_id":"1"},"status_code":0}"#));
        assert!(usable(r#"{"aweme_detail":null,"filter_detail":{"detail_msg":"作品已失效"},"status_code":0}"#));
        assert!(!usable("Blocked by ArgusSecurityPlugin Sign Invalid"));
        assert!(!usable(""));
        assert!(!usable(r#"{"status_code":8,"status_msg":"x"}"#));
    }

    /// 2026-11-28 抖音网页里接住的详情（7693510493986868901，横屏 1080p），只留下用得到的字段、地址缩短
    fn sample() -> Value {
        let addr = |key: &str, w: i64, h: i64, size: i64| json!({"url_key": key, "width": w, "height": h, "data_size": size, "url_list": [format!("https://v5-hl-mly-ov.zjcdn.com/{key}/"), format!("https://v3-dy-o.zjcdn.com/{key}/"), format!("https://www.douyin.com/aweme/v1/play/?video_id={key}")]});
        let br = |gear: &str, h265: i64, key: &str, w: i64, h: i64, size: i64, rate: i64, fps: i64| json!({"gear_name": gear, "is_h265": h265, "is_bytevc1": h265, "format": "mp4", "FPS": fps, "bit_rate": rate, "play_addr": addr(key, w, h, size)});
        json!({"status_code": 0, "aweme_detail": {
            "aweme_id": "7693510493986868901", "aweme_type": 0, "images": null, "create_time": 1791285000,
            "desc": "熟悉的飞车BGM配俯卧撑，青春和力量撞个满怀。#虎扑俯卧撑 #徒手健身 @东风奕派",
            "author": {"nickname": "某位作者", "uid": "100000000000001", "unique_id": "author_1", "sec_uid": "MS4wLjABAAAAexample"},
            "statistics": {"digg_count": 218016, "comment_count": 8766, "share_count": 70630},
            "video": {
                "duration": 12967, "width": 3840, "height": 2160, "has_watermark": true, "is_h265": 0,
                "cover": {"url_list": ["https://p3-pc-sign.douyinpic.com/cover.jpeg"]},
                "origin_cover": {"url_list": ["https://p3-pc-sign.douyinpic.com/origin.jpeg"]},
                "play_addr": addr("v28_h264_1080p_5801956", 1920, 1080, 9404247),
                "download_addr": addr("v28_watermark", 720, 720, 3971753),
                "bit_rate": [
                    br("normal_1080_0", 0, "v28_h264_1080p_5801956", 1920, 1080, 9404247, 5801956, 60),
                    br("normal_720_0", 0, "v28_h264_720p_3109908", 1280, 720, 5040773, 3109908, 60),
                    br("adapt_lowest_4_1", 1, "v28_bytevc1_4k_2396617", 3840, 2160, 3884618, 2396617, 60),
                    br("normal_540_0", 0, "v28_h264_540p_1271032", 1024, 576, 2065427, 1271032, 30),
                    br("vvc_1080", 1, "v28_bytevc2_1080p_1", 1920, 1080, 1, 1, 30),
                    br("dup_1080", 0, "v28_h264_1080p_5801956", 1920, 1080, 9404247, 5801956, 60)
                ]
            }
        }})
    }

    #[test]
    fn info_json() {
        let v = to_info(&sample(), "https://www.douyin.com/video/7693510493986868901").unwrap();
        assert_eq!(v["id"], "7693510493986868901");
        assert_eq!(v["title"], "熟悉的飞车BGM配俯卧撑，青春和力量撞个满怀。");
        assert!(v["description"].as_str().unwrap().contains("#虎扑俯卧撑"));
        assert_eq!(v["duration"], 12.967);
        assert_eq!(v["uploader"], "某位作者");
        assert_eq!(v["thumbnail"], "https://p3-pc-sign.douyinpic.com/origin.jpeg");
        assert_eq!((v["width"].as_i64(), v["height"].as_i64()), (Some(1920), Some(1080)));
        let f = v["formats"].as_array().unwrap();
        let ids: Vec<&str> = f.iter().map(|x| x["format_id"].as_str().unwrap()).collect();
        // bytevc2 不要、重复的不要、带水印的 download_addr 不要、码率比 1080p h264 低的 4K h265 不要
        assert_eq!(ids, ["normal_1080_0", "normal_720_0", "normal_540_0"]);
        assert!(f.iter().all(|x| x["vcodec"] == "h264"));
        assert_eq!(f[0]["acodec"], "aac");
        assert_eq!(f[0]["url"], "https://v5-hl-mly-ov.zjcdn.com/v28_h264_1080p_5801956/");
        assert_eq!(f[0]["filesize"], 9404247);
        assert_eq!(f[0]["tbr"], 5801.956);
        assert_eq!(f[0]["http_headers"]["Referer"], "https://www.douyin.com/");
        // Captae 自己的格式表、清晰度菜单照 yt-dlp 的结果算
        let info = info_from_json(&v);
        assert_eq!(info.title.as_deref(), Some("熟悉的飞车BGM配俯卧撑，青春和力量撞个满怀。"));
        let p = crate::probe::from_ytdlp(&v);
        assert_eq!(p.qualities.iter().map(|q| q.label.as_str()).collect::<Vec<_>>(), ["1080p", "720p", "576p"]);
        assert_eq!(p.site.as_deref(), Some("Douyin"));
    }

    #[test]
    fn strong_h265_stays() {
        let f = |id: &str, codec: &str, tbr: f64| json!({"format_id": id, "vcodec": codec, "tbr": tbr});
        let ids = |v: Vec<Value>| v.iter().map(|x| x["format_id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
        // 码率比 h264 高的 h265 4K 照留，低的不要
        assert_eq!(ids(drop_weak_h265(vec![f("4k", "h265", 9000.0), f("1080", "h264", 5800.0), f("1440", "h265", 1400.0)])), ["4k", "1080"]);
        // 没有 h264：h265 全留着
        assert_eq!(ids(drop_weak_h265(vec![f("4k", "h265", 2400.0), f("1080", "h265", 600.0)])), ["4k", "1080"]);
    }

    #[test]
    fn info_failures() {
        let gone = json!({"status_code": 0, "aweme_detail": null, "filter_detail": {"detail_msg": "作品已失效"}});
        let f = to_info(&gone, "https://www.douyin.com/video/1234567").unwrap_err();
        assert_eq!(f.code, ErrorCode::Unavailable);
        assert!(f.raw.contains("作品已失效"));
        let mut img = sample();
        img["aweme_detail"]["images"] = json!([{"url_list": ["https://p3/1.jpeg"]}]);
        assert_eq!(to_info(&img, "https://www.douyin.com/video/7693510493986868901").unwrap_err().reason.as_deref(), Some("image_post"));
        let mut none = sample();
        none["aweme_detail"]["video"] = json!({});
        assert_eq!(to_info(&none, "https://www.douyin.com/video/7693510493986868901").unwrap_err().code, ErrorCode::NoMedia);
    }
}
