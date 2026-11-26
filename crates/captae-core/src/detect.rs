//! 链接识别：决定一个 URL 走 yt-dlp、aria2 还是 M3U8 分片下载。
//! 规则和前端 `apps/desktop/src/lib/detect.ts` 的 `detect()` 保持一致（见 docs/DESIGN.md「下载引擎」），前端用它做即时提示，
//! 后端用它做真正的路由；yt-dlp 读不出来的网页会再退回 aria2 直链。

use crate::model::Engine;
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Detected {
    /// 给人看的类型说法（「YouTube 视频」「MP4 文件」）。后端没有地方显示它（界面用 lib/detect.ts 自己的说法），
    /// 所以没走 tr!、不进词表；哪天要显示了，先换成 tr!
    pub label: String,
    pub plat: Option<&'static str>,
    pub eng: Engine,
    pub host: String,
    pub kind: Kind,
    /// 文件扩展名（直链时）
    pub ext: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Page,
    Hls,
    File,
}

const PLAT: &[(&str, &str, Option<&str>)] = &[
    ("youtube.com", "YouTube", Some("youtube")),
    ("youtu.be", "YouTube", Some("youtube")),
    ("bilibili.com", "哔哩哔哩", Some("bilibili")),
    ("b23.tv", "哔哩哔哩", Some("bilibili")),
    ("douyin.com", "抖音", Some("tiktok")),
    ("tiktok.com", "TikTok", Some("tiktok")),
    ("x.com", "X", Some("x")),
    ("twitter.com", "X", Some("x")),
    ("instagram.com", "Instagram", Some("instagram")),
    ("vimeo.com", "Vimeo", None),
];

const VIDEO: &[&str] = &["mp4", "mov", "webm", "mkv", "m4v", "flv", "ts", "ogv"];
const AUDIO: &[&str] = &["mp3", "m4a", "flac", "wav", "aac", "ogg", "opus", "oga"];
const IMAGE: &[&str] = &["png", "jpg", "jpeg", "webp", "avif", "gif"];

// ------------------------------------------------------------------ 交给下载引擎之前把链接理顺

/// 不联网就能理顺的几种链接（`Core::add`、读取信息、样片都先过一遍）：
/// - 跳转包装：YouTube `/redirect?q=`、Google `/url?q=`（或 `url=`）、`l.facebook.com/l.php?u=`、`l.instagram.com/?u=`、
///   `out.reddit.com/?url=`、Steam `linkfilter/?url=`、Tumblr `t.umblr.com/redirect?z=`。这几种不是 HTTP 跳转，
///   yt-dlp 不认（YouTube 的被当成频道地址，说「视频已被删除」）
/// - B 站带语言前缀的地址：`bilibili.com/zh-Hans/video/BV…` → `bilibili.com/video/BV…`（海外打开会跳到这种，yt-dlp 不认，
///   读取只拿到标题、没有清晰度）。规则和插件 `extension/lib/sniff.js` 的 `canonicalPage` 一样
/// - Dropbox 文件分享页：`?dl=0` → `?dl=1`（官方的直接下载参数），不然下到的是网页
/// - Wikimedia Commons 文件页 `wiki/File:x.webm` → `wiki/Special:FilePath/x.webm`（直接给文件），不然下到的是网页
pub fn canonical(input: &str) -> String {
    let mut cur = input.trim().to_string();
    // 包装里还可能再套一层
    for _ in 0..3 {
        match unwrap_once(&cur) {
            Some(n) if n != cur => cur = n,
            _ => break,
        }
    }
    let Ok(mut u) = Url::parse(&cur) else { return cur };
    let host = u.host_str().unwrap_or("").to_ascii_lowercase();
    let on = |d: &str| host == d || host.ends_with(&format!(".{d}"));
    if on("bilibili.com") {
        static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"^/[a-z]{2}(?:-[A-Za-z]{2,4})?(/(?:video|bangumi)/)").unwrap());
        let p = RE.replace(u.path(), "$1").to_string();
        if p != u.path() {
            u.set_path(&p);
            return u.to_string();
        }
    } else if on("dropbox.com") && (u.path().starts_with("/s/") || u.path().starts_with("/scl/fi/")) {
        let pairs: Vec<(String, String)> = u.query_pairs().map(|(k, v)| (k.to_string(), v.to_string())).collect();
        if pairs.iter().all(|(k, v)| k != "dl" || v != "1") {
            let mut q = u.query_pairs_mut();
            q.clear();
            let mut seen = false;
            for (k, v) in &pairs {
                if k == "dl" {
                    seen = true;
                    q.append_pair("dl", "1");
                } else {
                    q.append_pair(k, v);
                }
            }
            if !seen {
                q.append_pair("dl", "1");
            }
            drop(q);
            return u.to_string();
        }
    } else if on("rednote.com") && (u.path().starts_with("/explore/") || u.path().starts_with("/discovery/item/")) {
        // 小红书笔记在 rednote.com 上的地址：yt-dlp 的小红书提取器只认 www.xiaohongshu.com，换回去（xsec_token 等参数保留）
        if u.set_host(Some("www.xiaohongshu.com")).is_ok() {
            return u.to_string();
        }
    } else if host == "commons.wikimedia.org" || host == "commons.m.wikimedia.org" {
        if let Some(name) = u.path().strip_prefix("/wiki/File:") {
            let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
            if VIDEO.contains(&ext.as_str()) || AUDIO.contains(&ext.as_str()) || IMAGE.contains(&ext.as_str()) {
                return format!("https://commons.wikimedia.org/wiki/Special:FilePath/{name}");
            }
        }
    }
    cur
}

/// 拆一层跳转包装，拆不出来时 None。
fn unwrap_once(s: &str) -> Option<String> {
    let u = Url::parse(s).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    let bare = host.trim_start_matches("www.").trim_start_matches("m.");
    let path = u.path();
    let param = |names: &[&str]| -> Option<String> {
        let v = u.query_pairs().find(|(k, _)| names.contains(&k.as_ref())).map(|(_, v)| v.to_string())?;
        let t = Url::parse(v.trim()).ok().filter(|t| matches!(t.scheme(), "http" | "https"))?;
        Some(t.to_string())
    };
    let google = bare == "google.com" || bare.starts_with("google.");
    if (bare == "youtube.com" && path == "/redirect") || (google && path == "/url") {
        return param(&["q", "url"]);
    }
    if matches!(host.as_str(), "l.facebook.com" | "lm.facebook.com" | "l.messenger.com") && path == "/l.php" {
        return param(&["u"]);
    }
    if host == "l.instagram.com" {
        return param(&["u"]);
    }
    if host == "out.reddit.com" {
        return param(&["url"]);
    }
    if bare == "steamcommunity.com" && path.starts_with("/linkfilter") {
        return param(&["url", "u"]);
    }
    if host == "t.umblr.com" && path == "/redirect" {
        return param(&["z"]);
    }
    None
}

/// LinkedIn 的 lnkd.in 短链打开是一个「你要离开 LinkedIn」的提示页，目标写在页面里的按钮上
/// （`<a … data-tracking-control-name="external_url_click" … href="…">`）。
pub fn lnkd_target(html: &str) -> Option<String> {
    let i = html.find("external_url_click")?;
    let start = html[..i].rfind("<a")?;
    let end = i + html[i..].find('>')?;
    let tag = &html[start..end];
    let h = tag.find("href=\"")? + 6;
    let href = &tag[h..h + tag[h..].find('"')?];
    let href = href.replace("&amp;", "&").replace("&#x3d;", "=").replace("&#61;", "=").replace("&quot;", "\"");
    Url::parse(&href).ok().filter(|t| matches!(t.scheme(), "http" | "https")).map(|t| t.to_string())
}

/// 小红书分享短链（xhslink.com）跳到的地址 → 笔记链接。大陆以外跳到 rednote.com，yt-dlp 不认，要换回 xiaohongshu.com；
/// 有时先跳到登录页，笔记地址在 `redirectPath` 里。只认笔记页，首页（分享链接失效了）、404 页都返回 None，交给 yt-dlp 去报错
pub fn xhs_target(landed: &str) -> Option<String> {
    let u = Url::parse(landed).ok()?;
    let host = u.host_str()?.to_ascii_lowercase();
    let ours = |h: &str| ["rednote.com", "xiaohongshu.com"].iter().any(|d| h == *d || h.ends_with(&format!(".{d}")));
    if !ours(&host) {
        return None;
    }
    let note = if u.path().starts_with("/login") {
        let p = u.query_pairs().find(|(k, _)| k == "redirectPath").map(|(_, v)| v.to_string())?;
        // redirectPath 可能是完整地址，也可能只是路径
        Url::parse(&p).or_else(|_| u.join(&p)).ok()?
    } else {
        u
    };
    if !note.host_str().is_some_and(ours) || !(note.path().starts_with("/explore/") || note.path().starts_with("/discovery/item/")) {
        return None;
    }
    // 登录页拼进来的 exSource=null 没用，去掉
    let mut note = note;
    let keep: Vec<(String, String)> = note.query_pairs().filter(|(k, _)| k != "exSource").map(|(k, v)| (k.to_string(), v.to_string())).collect();
    if keep.is_empty() {
        note.set_query(None);
    } else {
        note.query_pairs_mut().clear().extend_pairs(keep);
    }
    let _ = note.set_scheme("https");
    let _ = note.set_host(Some("www.xiaohongshu.com"));
    Some(note.to_string())
}

/// 要联网才拆得开的包装（lnkd.in、小红书分享短链 xhslink.com）：拆开了返回目标链接（再过一遍 `canonical`）。
pub async fn unwrap_remote(http: &reqwest::Client, url: &str) -> Option<String> {
    let host = Url::parse(url.trim()).ok()?.host_str()?.to_ascii_lowercase();
    if host == "xhslink.com" || host.ends_with(".xhslink.com") {
        // 不带 cookies 跟着跳一遍就知道是哪条笔记；yt-dlp 自己跟着跳会落到 rednote.com，走通用提取器下不了
        let r = http.get(url).timeout(std::time::Duration::from_secs(15)).send().await.ok()?;
        return xhs_target(r.url().as_str());
    }
    if host != "lnkd.in" {
        return None;
    }
    let r = http.get(url).timeout(std::time::Duration::from_secs(15)).send().await.ok()?;
    // 有时 lnkd.in 直接 HTTP 跳转到目标
    let fin = r.url().clone();
    if fin.host_str().is_some_and(|h| h != "lnkd.in" && !h.ends_with("linkedin.com")) {
        return Some(canonical(fin.as_str()));
    }
    let html = r.text().await.ok()?;
    lnkd_target(&html).map(|t| canonical(&t))
}

pub fn media_kind_of_ext(ext: &str) -> &'static str {
    let e = ext.to_ascii_lowercase();
    if VIDEO.contains(&e.as_str()) {
        "video"
    } else if AUDIO.contains(&e.as_str()) {
        "audio"
    } else if IMAGE.contains(&e.as_str()) {
        "image"
    } else if matches!(e.as_str(), "srt" | "vtt" | "ass") {
        "subtitle"
    } else {
        "other"
    }
}

/// 这个链接是不是已知要交给 yt-dlp 的网站（YouTube、哔哩哔哩……，`PLAT` 表里的），是就返回名字（存的是中文原文，显示时用 `i18n::dynamic`）。
/// 手机上架版据此说「这个网站在电脑上下」（crates/captae-mobile）
pub fn known_site(input: &str) -> Option<&'static str> {
    let url = Url::parse(input.trim()).ok()?;
    let host = url.host_str()?.trim_start_matches("www.").to_ascii_lowercase();
    PLAT.iter().find(|(k, _, _)| host == *k || host.ends_with(&format!(".{k}"))).map(|(_, name, _)| *name)
}

pub fn detect(input: &str) -> Option<Detected> {
    let url = Url::parse(input.trim()).ok()?;
    if !matches!(url.scheme(), "http" | "https" | "ftp") {
        return None;
    }
    let host = url.host_str()?.trim_start_matches("www.").to_string();
    let path = url.path().to_ascii_lowercase();
    if let Some((_, name, plat)) = PLAT.iter().find(|(k, _, _)| host == *k || host.ends_with(&format!(".{k}"))) {
        return Some(Detected { label: format!("{name} 视频"), plat: *plat, eng: Engine::YtDlp, host, kind: Kind::Page, ext: None });
    }
    if path.ends_with(".m3u8") {
        return Some(Detected { label: "在线视频".into(), plat: None, eng: Engine::Hls, host, kind: Kind::Hls, ext: Some("m3u8".into()) });
    }
    let ext = path.rsplit_once('.').map(|(_, e)| e.to_string()).filter(|e| !e.contains('/'));
    if let Some(e) = ext.as_deref() {
        if VIDEO.contains(&e) || AUDIO.contains(&e) || IMAGE.contains(&e) {
            return Some(Detected { label: format!("{} 文件", e.to_uppercase()), plat: None, eng: Engine::Aria2, host, kind: Kind::File, ext: Some(e.to_string()) });
        }
    }
    Some(Detected { label: "网页".into(), plat: None, eng: Engine::YtDlp, host, kind: Kind::Page, ext: None })
}

/// 从 Content-Type 判断（插件嗅探时 URL 常常没有扩展名）。
pub fn from_content_type(ct: &str) -> Option<Kind> {
    let ct = ct.to_ascii_lowercase();
    if ct.contains("mpegurl") {
        Some(Kind::Hls)
    } else if ct.starts_with("video/") || ct.starts_with("audio/") || ct.starts_with("image/") {
        Some(Kind::File)
    } else {
        None
    }
}

/// "download.blender.org/demo/movies/ToS"，队列行里展示用。
pub fn short_src(u: &str) -> String {
    match Url::parse(u) {
        Ok(url) => {
            let host = url.host_str().unwrap_or("").trim_start_matches("www.");
            let mut s = format!("{host}{}", url.path());
            if let Some(q) = url.query() {
                if host.contains("youtube") {
                    s.push('?');
                    s.push_str(q);
                }
            }
            s.chars().take(64).collect()
        }
        Err(_) => u.chars().take(64).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn known_sites() {
        assert_eq!(known_site("https://www.youtube.com/watch?v=x"), Some("YouTube"));
        assert_eq!(known_site("https://m.bilibili.com/video/BV1"), Some("哔哩哔哩"));
        assert_eq!(known_site("https://example.com/a.mp4"), None);
        assert_eq!(known_site("不是链接"), None);
    }

    #[test]
    fn routes() {
        assert_eq!(detect("https://www.youtube.com/watch?v=aqz-KE-bpKQ").unwrap().eng, Engine::YtDlp);
        assert_eq!(detect("https://www.youtube.com/watch?v=x").unwrap().plat, Some("youtube"));
        assert_eq!(detect("https://m.bilibili.com/video/BV1").unwrap().plat, Some("bilibili"));
        assert_eq!(detect("https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8").unwrap().kind, Kind::Hls);
        let f = detect("https://download.blender.org/durian/trailer/sintel_trailer-1080p.mp4").unwrap();
        assert_eq!((f.eng, f.kind, f.ext.as_deref()), (Engine::Aria2, Kind::File, Some("mp4")));
        assert_eq!(detect("https://example.com/some/page").unwrap().kind, Kind::Page);
        assert!(detect("javascript:alert(1)").is_none());
        assert!(detect("vimeo.com/1").is_none());
    }
    /// 第三阶段 FIXES 第 9、11 项和插件发现的 B 站语言前缀（链接都是第一轮测试里的真实链接）
    #[test]
    fn canonical_links() {
        // youtube-89ba86：YouTube 简介里的外链
        assert_eq!(canonical("https://www.youtube.com/redirect?q=https%3A%2F%2Fwww.bilibili.com%2Fvideo%2FBV1jT421a72z"), "https://www.bilibili.com/video/BV1jT421a72z");
        assert_eq!(canonical("https://www.youtube.com/redirect?event=video_description&redir_token=x&q=https%3A%2F%2Fexample.com%2Fv.mp4&v=abc"), "https://example.com/v.mp4");
        assert_eq!(canonical("https://www.google.com/url?sa=t&url=https%3A%2F%2Fyoutu.be%2FmC73amqYFpQ&ved=1"), "https://youtu.be/mC73amqYFpQ");
        assert_eq!(canonical("https://www.google.co.jp/url?q=https://vimeo.com/1084537"), "https://vimeo.com/1084537");
        assert_eq!(canonical("https://l.facebook.com/l.php?u=https%3A%2F%2Fwww.youtube.com%2Fwatch%3Fv%3DmC73amqYFpQ&h=AT0"), "https://www.youtube.com/watch?v=mC73amqYFpQ");
        assert_eq!(canonical("https://l.instagram.com/?u=https%3A%2F%2Fyoutu.be%2FmC73amqYFpQ&e=AT1"), "https://youtu.be/mC73amqYFpQ");
        assert_eq!(canonical("https://out.reddit.com/t3_abc?url=https%3A%2F%2Fv.redd.it%2Fxyz&token=1"), "https://v.redd.it/xyz");
        assert_eq!(canonical("https://steamcommunity.com/linkfilter/?url=https://www.youtube.com/watch?v=mC73amqYFpQ"), "https://www.youtube.com/watch?v=mC73amqYFpQ");
        // 包装里没有像样的目标：原样
        assert_eq!(canonical("https://www.youtube.com/redirect?q=javascript:alert(1)"), "https://www.youtube.com/redirect?q=javascript:alert(1)");
        assert_eq!(canonical("https://www.youtube.com/watch?v=mC73amqYFpQ"), "https://www.youtube.com/watch?v=mC73amqYFpQ");
        // B 站带语言前缀（插件 sniff.js 第 16–17 行的规则）
        assert_eq!(canonical("https://www.bilibili.com/zh-Hans/video/BV1LpD3YsETa/"), "https://www.bilibili.com/video/BV1LpD3YsETa/");
        assert_eq!(canonical("https://www.bilibili.com/en/video/BV1LpD3YsETa/?p=2"), "https://www.bilibili.com/video/BV1LpD3YsETa/?p=2");
        assert_eq!(canonical("https://www.bilibili.com/zh-Hant/bangumi/play/ep123"), "https://www.bilibili.com/bangumi/play/ep123");
        assert_eq!(canonical("https://www.bilibili.com/video/BV1LpD3YsETa/"), "https://www.bilibili.com/video/BV1LpD3YsETa/");
        assert_eq!(detect(&canonical("https://www.bilibili.com/zh-Hans/video/BV1LpD3YsETa/")).unwrap().plat, Some("bilibili"));
        // dropbox-8b4b13、dropbox-e2db04：分享页 → 直接下载
        assert_eq!(
            canonical("https://www.dropbox.com/scl/fi/r2kd2skcy5ylbbta5y1pz/DJI_0003.MP4?dl=0&rlkey=wcdgqangn7t3lnmmv6li9mu9h"),
            "https://www.dropbox.com/scl/fi/r2kd2skcy5ylbbta5y1pz/DJI_0003.MP4?dl=1&rlkey=wcdgqangn7t3lnmmv6li9mu9h"
        );
        assert_eq!(canonical("https://www.dropbox.com/s/nelirfsxnmcfbfh/a.mp4"), "https://www.dropbox.com/s/nelirfsxnmcfbfh/a.mp4?dl=1");
        // 文件夹分享不动（dl=1 会下成 zip）
        assert_eq!(canonical("https://www.dropbox.com/scl/fo/abc/def?dl=0"), "https://www.dropbox.com/scl/fo/abc/def?dl=0");
        // wikimedia-07cb1f、wikimedia-e4ba2c：文件页 → 文件本身
        assert_eq!(canonical("https://commons.wikimedia.org/wiki/File:Shila_Waterfall_in_Zanaskar.webm"), "https://commons.wikimedia.org/wiki/Special:FilePath/Shila_Waterfall_in_Zanaskar.webm");
        let c = canonical("https://commons.wikimedia.org/wiki/File:Pointe_Saint_Mathieu_-_Phare.jpg");
        assert_eq!(c, "https://commons.wikimedia.org/wiki/Special:FilePath/Pointe_Saint_Mathieu_-_Phare.jpg");
        assert_eq!(detect(&c).unwrap().kind, Kind::File);
        assert_eq!(canonical("https://commons.wikimedia.org/wiki/Main_Page"), "https://commons.wikimedia.org/wiki/Main_Page");
        // 小红书被跳到 rednote.com 以后地址栏里的笔记链接：yt-dlp 只认 xiaohongshu.com
        assert_eq!(
            canonical("https://www.rednote.com/explore/6b0336bd000000001b01ff7f?xsec_token=AB1&xsec_source=pc_feed"),
            "https://www.xiaohongshu.com/explore/6b0336bd000000001b01ff7f?xsec_token=AB1&xsec_source=pc_feed"
        );
        assert_eq!(canonical("https://rednote.com/discovery/item/6b0336bd000000001b01ff7f"), "https://www.xiaohongshu.com/discovery/item/6b0336bd000000001b01ff7f");
        // 登录页、首页不动
        assert_eq!(canonical("https://www.rednote.com/login?redirectPath=%2Fexplore"), "https://www.rednote.com/login?redirectPath=%2Fexplore");
    }

    /// xhslink 短链跳到的地址（2026-11-26 实测）→ yt-dlp 认的 xiaohongshu.com 笔记链接
    #[test]
    fn xhs_short_link_targets() {
        assert_eq!(
            xhs_target("https://www.rednote.com/discovery/item/6ae4810f000000002501c4bb?xsec_source=app_share&type=video&xsec_token=CB8=").as_deref(),
            Some("https://www.xiaohongshu.com/discovery/item/6ae4810f000000002501c4bb?xsec_source=app_share&type=video&xsec_token=CB8%3D")
        );
        // xiaohongshu-a50820：先跳到登录页，笔记在 redirectPath 里
        assert_eq!(
            xhs_target("https://www.rednote.com/login?redirectPath=http%3A%2F%2Fwww.rednote.com%2Fdiscovery%2Fitem%2F6ae4810f000000002501c4bb%3Fxsec_token%3DAB%26exSource%3Dnull").as_deref(),
            Some("https://www.xiaohongshu.com/discovery/item/6ae4810f000000002501c4bb?xsec_token=AB")
        );
        assert_eq!(xhs_target("https://www.xiaohongshu.com/explore/6655").as_deref(), Some("https://www.xiaohongshu.com/explore/6655"));
        // xiaohongshu-a3612b：登录后只回首页（分享链接失效了）、404 页：不换，让 yt-dlp 报错
        assert_eq!(xhs_target("https://www.rednote.com/login?redirectPath=http%3A%2F%2Fwww.rednote.com%2Fexplore%3FexSource%3Dnull"), None);
        assert_eq!(xhs_target("https://www.rednote.com/404?source=/404/sec_x&error_code=300031"), None);
        assert_eq!(xhs_target("https://evil.example/discovery/item/1"), None);
    }

    /// linkedin-1ba5e6：lnkd.in 提示页（2026-11-24 抓的原文）
    #[test]
    fn lnkd_page() {
        let html = "<h1 class=\"t-24 t-bold t-black mb2\">This link will take you to a page that’s not on LinkedIn</h1>\n            <h2 class=\"t-16 t-black mb3\">Because this is an external link, we’re unable to verify it for safety.</h2>\n            <a class=\"artdeco-button artdeco-button--tertiary\" data-tracking-control-name=\"external_url_click\" data-tracking-will-navigate href=\"https://youtu.be/gXMqoHhh6TE?si=xg7xkT36YzzvifXl\">\n                https://youtu.be/gXMqoHhh6TE?si=xg7xkT36YzzvifXl\n            </a>";
        assert_eq!(lnkd_target(html).as_deref(), Some("https://youtu.be/gXMqoHhh6TE?si=xg7xkT36YzzvifXl"));
        let amp = "<a data-tracking-control-name=\"external_url_click\" href=\"https://www.youtube.com/watch?v=k8r3BKWbMoc&amp;t=3s\">x</a>";
        assert_eq!(lnkd_target(amp).as_deref(), Some("https://www.youtube.com/watch?v=k8r3BKWbMoc&t=3s"));
        assert_eq!(lnkd_target("<html>nothing</html>"), None);
    }

    #[test]
    fn short() {
        assert_eq!(short_src("https://www.youtube.com/watch?v=aqz"), "youtube.com/watch?v=aqz");
        assert_eq!(short_src("https://download.blender.org/demo/movies/ToS"), "download.blender.org/demo/movies/ToS");
    }
}
