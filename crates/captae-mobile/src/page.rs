//! 上架版读网页：不用 yt-dlp，只找网页里直接写着的媒体地址（`<video src>`、`og:video`、脚本里的 .m3u8 / .mp4 地址）。
//! 找到的地址交给核心库照常读取信息、下载（直链、M3U8）。PLAN 5.4「读取视频信息只认直链、M3U8 和网页里直接写着的媒体地址」。

use regex::Regex;
use std::sync::LazyLock;
use std::time::Duration;
use url::Url;

/// 网页最多读多少（够放下一般网页的 HTML，不读到视频里去）
const MAX_HTML: usize = 3 << 20;
const TIMEOUT: Duration = Duration::from_secs(20);
const EXTS: &str = "m3u8|mp4|m4v|mov|webm|mp3|m4a|aac|ogg|oga|opus|flac|wav";

#[derive(Debug, Default, PartialEq)]
pub struct Found {
    /// 网页标题（og:title 优先，再是 <title>）
    pub title: Option<String>,
    /// 媒体地址，按在网页里出现的顺序，og:video 排最前
    pub media: Vec<String>,
}

/// 把网页里转义过的地址还原（JSON 里的 `\/`、`/`，HTML 的 `&amp;`）
fn unescape(s: &str) -> String {
    s.replace("\\/", "/").replace("\\u002F", "/").replace("\\u002f", "/").replace("&amp;", "&").replace("&#x2F;", "/").replace("&#47;", "/")
}

fn html_text(s: &str) -> String {
    s.replace("&amp;", "&").replace("&quot;", "\"").replace("&#39;", "'").replace("&lt;", "<").replace("&gt;", ">").split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 相对地址按网页地址补全，只要 http / https
fn resolve(base: &Url, raw: &str) -> Option<String> {
    let u = base.join(&unescape(raw.trim())).ok()?;
    matches!(u.scheme(), "http" | "https").then(|| u.to_string())
}

fn add(media: &mut Vec<String>, u: String) {
    if !media.contains(&u) {
        media.push(u);
    }
}

/// 从 HTML 里找标题和媒体地址
pub fn extract(html: &str, base: &Url) -> Found {
    static OG_VIDEO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)<meta[^>]+(?:property|name)\s*=\s*["']og:video(?::secure_url|:url)?["'][^>]*content\s*=\s*["']([^"']+)["']"#).unwrap());
    static OG_VIDEO2: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)<meta[^>]+content\s*=\s*["']([^"']+)["'][^>]*(?:property|name)\s*=\s*["']og:video(?::secure_url|:url)?["']"#).unwrap());
    static OG_TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"(?i)<meta[^>]+(?:property|name)\s*=\s*["']og:title["'][^>]*content\s*=\s*["']([^"']+)["']"#).unwrap());
    static TITLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").unwrap());
    // 属性里的（可以是相对地址）
    static ATTR: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r#"(?i)\b(?:src|href|content|data-src|data-video|data-url)\s*=\s*["']([^"'<>\s]+?\.(?:{EXTS})(?:[?#][^"'<>\s]*)?)["']"#)).unwrap());
    // 脚本、JSON 里的完整地址
    static ABS: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r#"(?i)(?:https?:)?//[^\s"'<>()\\]+?\.(?:{EXTS})(?:\?[^\s"'<>\\]*)?"#)).unwrap());

    let mut media: Vec<String> = vec![];
    // og:video 也可能是播放器页面（不是媒体），只留扩展名像媒体的
    let looks_media = |s: &str| captae_core::detect::detect(s).is_some_and(|d| d.kind != captae_core::detect::Kind::Page);
    for re in [&*OG_VIDEO, &*OG_VIDEO2] {
        for c in re.captures_iter(html) {
            if let Some(u) = resolve(base, &c[1]).filter(|u| looks_media(u)) {
                add(&mut media, u);
            }
        }
    }
    let un = unescape(html);
    for c in ATTR.captures_iter(&un) {
        if let Some(u) = resolve(base, &c[1]) {
            add(&mut media, u);
        }
    }
    for m in ABS.find_iter(&un) {
        if let Some(u) = resolve(base, m.as_str()) {
            add(&mut media, u);
        }
    }
    let title = OG_TITLE.captures(html).map(|c| html_text(&c[1])).or_else(|| TITLE.captures(html).map(|c| html_text(&c[1]))).filter(|t| !t.is_empty());
    Found { title, media }
}

/// 读网页、找媒体地址。网页地址本身回的就是媒体（没有扩展名的直链）时，媒体就是它自己
pub async fn scan(http: &captae_core::net::HttpClient, url: &str) -> anyhow::Result<Found> {
    let base = Url::parse(url)?;
    let mut resp = http
        .get(url)
        .header("Accept", "text/html,application/xhtml+xml,*/*;q=0.8")
        .timeout(TIMEOUT)
        .send()
        .await
        .map_err(|e| anyhow::Error::new(captae_core::errors::from_reqwest(&e, url)))?;
    let status = resp.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(captae_core::errors::http_status(status, url).into());
    }
    let ct = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("").to_ascii_lowercase();
    if ct.starts_with("video/") || ct.starts_with("audio/") || ct.contains("mpegurl") {
        return Ok(Found { title: None, media: vec![url.to_string()] });
    }
    let mut body: Vec<u8> = vec![];
    while body.len() < MAX_HTML {
        match resp.chunk().await.map_err(|e| anyhow::Error::new(captae_core::errors::from_reqwest(&e, url)))? {
            Some(c) => body.extend_from_slice(&c),
            None => break,
        }
    }
    Ok(extract(&String::from_utf8_lossy(&body), &base))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_media() {
        let html = r#"<html><head><title> 一段
            演示 &amp; 测试 </title>
            <meta property="og:video" content="https://cdn.example.com/v/clip.mp4?x=1&amp;y=2">
            <meta property="og:video:url" content="https://player.example.com/embed/42">
            </head><body>
            <video src="media/movie.webm"></video>
            <source src='/hls/master.m3u8'>
            <script>var c={"hls":"https:\/\/cdn.example.com\/live\/index.m3u8","dup":"https://cdn.example.com/v/clip.mp4?x=1&y=2"};</script>
            <img src="poster.jpg"><a href="mailto:a@b.mp4">x</a>
            </body></html>"#;
        let f = extract(html, &Url::parse("https://example.com/watch/page.html").unwrap());
        assert_eq!(f.title.as_deref(), Some("一段 演示 & 测试"));
        assert_eq!(
            f.media,
            vec![
                "https://cdn.example.com/v/clip.mp4?x=1&y=2",
                "https://example.com/watch/media/movie.webm",
                "https://example.com/hls/master.m3u8",
                "https://cdn.example.com/live/index.m3u8",
            ]
        );
    }

    #[test]
    fn nothing_found() {
        let f = extract("<html><title>空</title><img src=a.png></html>", &Url::parse("https://example.com/").unwrap());
        assert_eq!(f, Found { title: Some("空".into()), media: vec![] });
    }
}
