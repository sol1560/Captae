//! Linux 上素材库里直接播放视频、音频用的本机小服务（只在 Linux 上启动）。
//!
//! WebKitGTK 的媒体播放（GStreamer）读不了 Tauri 的 asset 协议给的地址：同一个文件经 asset 协议一律「FormatError」、
//! 放不了，解码组件都装齐了也一样（2026-11-25 Devin 的 Ubuntu 22.04 / WebKitGTK 2.50 上 deb、AppImage 都实测过）；
//! 普通的 http 地址它能放。所以 Linux 上 `<video>`、`<audio>` 的地址改由这里给：
//!
//! - 只听 127.0.0.1 的随机端口；地址里带一个每次启动随机生成的令牌，别的程序猜不到；
//! - 只给页面要过的文件：`media_src` 命令先核对文件在 asset 协议放行的范围里（和 `<img>` 同一套规则，见 asset_scope.rs），
//!   再登记一个随机编号，地址是 `http://127.0.0.1:<端口>/<令牌>/<编号>`，编号以外的路径一律 404；
//! - 支持 Range（拖进度条、读 MP4 末尾的索引都要），一次只给一段，不支持多段。
//!
//! macOS、Windows 照旧用 asset 协议（`fileSrc`），不启动这个服务。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::extract::{Path as AxPath, State};
use axum::http::{header, HeaderMap, Method, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::Router;
use tauri::Manager;

#[derive(Clone)]
pub struct MediaServer {
    port: u16,
    token: String,
    files: Arc<Mutex<HashMap<String, PathBuf>>>,
}

fn random_hex(bytes: usize) -> String {
    use rand::RngCore;
    let mut b = vec![0u8; bytes];
    rand::rng().fill_bytes(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

impl MediaServer {
    /// 在 tauri 的运行时里起服务，登记到 app 的状态里
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn start(app: &tauri::AppHandle) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            match tokio::net::TcpListener::bind(("127.0.0.1", 0)).await {
                Ok(l) => {
                    let Ok(addr) = l.local_addr() else { return };
                    let s = MediaServer { port: addr.port(), token: random_hex(16), files: Default::default() };
                    let router = Router::new().route("/{token}/{id}", get(serve)).with_state(s.clone());
                    app.manage(s);
                    tracing::info!("播放用的本机服务在 127.0.0.1:{}", addr.port());
                    if let Err(e) = axum::serve(l, router).await {
                        tracing::warn!("播放用的本机服务停了：{e}");
                    }
                }
                Err(e) => tracing::warn!("播放用的本机服务起不来：{e}"),
            }
        });
    }

    /// 给一个文件登记编号，返回地址。同一个文件再要还是同一个编号
    fn url_for(&self, p: &Path) -> String {
        let mut files = self.files.lock().unwrap();
        let id = match files.iter().find(|(_, v)| v.as_path() == p) {
            Some((k, _)) => k.clone(),
            None => {
                let k = random_hex(12);
                files.insert(k.clone(), p.to_path_buf());
                k
            }
        };
        format!("http://127.0.0.1:{}/{}/{}", self.port, self.token, id)
    }
}

/// 页面要播放某个本机文件：返回能放的地址。Linux 以外没有这个服务，页面也不会调
#[tauri::command]
pub fn media_src(app: tauri::AppHandle, path: PathBuf) -> Result<String, String> {
    let s = app.try_state::<MediaServer>().ok_or_else(|| "media server not running".to_string())?;
    // 和 asset 协议同一套放行规则：页面本来就能用 <img> 读到的文件才给
    if !path.is_absolute() || !path.is_file() || !app.asset_protocol_scope().is_allowed(&path) {
        return Err("not allowed".into());
    }
    Ok(s.url_for(&path))
}

/// 按扩展名给 Content-Type（GStreamer 自己会看内容，这里给对了更稳）
fn mime(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("mp4" | "m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("ts") => "video/mp2t",
        Some("ogv") => "video/ogg",
        Some("mp3") => "audio/mpeg",
        Some("m4a") => "audio/mp4",
        Some("aac") => "audio/aac",
        Some("wav") => "audio/wav",
        Some("flac") => "audio/flac",
        Some("ogg" | "oga") => "audio/ogg",
        Some("opus") => "audio/opus",
        _ => "application/octet-stream",
    }
}

/// `Range: bytes=…` 解析成 [start, end]（含两端）。没有 Range 是 Ok(None)；格式不认、超出文件大小是 Err（回 416）。
/// 多段（逗号分开）只取第一段：播放器只会要一段
fn parse_range(h: Option<&str>, len: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(h) = h else { return Ok(None) };
    let spec = h.trim().strip_prefix("bytes=").ok_or(())?.split(',').next().unwrap_or("").trim();
    let (a, b) = spec.split_once('-').ok_or(())?;
    let (a, b) = (a.trim(), b.trim());
    if len == 0 {
        return Err(());
    }
    let r = if a.is_empty() {
        // 最后 n 个字节
        let n: u64 = b.parse().map_err(|_| ())?;
        if n == 0 {
            return Err(());
        }
        (len.saturating_sub(n), len - 1)
    } else {
        let start: u64 = a.parse().map_err(|_| ())?;
        let end = if b.is_empty() { len - 1 } else { b.parse::<u64>().map_err(|_| ())?.min(len - 1) };
        if start > end || start >= len {
            return Err(());
        }
        (start, end)
    };
    Ok(Some(r))
}

/// 令牌比较不因为前几位对上而快一点
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

async fn serve(State(s): State<MediaServer>, AxPath((token, id)): AxPath<(String, String)>, method: Method, h: HeaderMap) -> Response {
    let status = |c: StatusCode| Response::builder().status(c).body(Body::empty()).unwrap();
    if !same(&token, &s.token) {
        return status(StatusCode::NOT_FOUND);
    }
    let Some(path) = s.files.lock().unwrap().get(&id).cloned() else { return status(StatusCode::NOT_FOUND) };
    let Ok(mut f) = tokio::fs::File::open(&path).await else { return status(StatusCode::NOT_FOUND) };
    let Ok(len) = f.metadata().await.map(|m| m.len()) else { return status(StatusCode::NOT_FOUND) };
    let range = match parse_range(h.get(header::RANGE).and_then(|v| v.to_str().ok()), len) {
        Ok(r) => r,
        Err(()) => {
            return Response::builder().status(StatusCode::RANGE_NOT_SATISFIABLE).header(header::CONTENT_RANGE, format!("bytes */{len}")).body(Body::empty()).unwrap();
        }
    };
    let (start, end) = range.unwrap_or((0, len.saturating_sub(1)));
    let count = if len == 0 { 0 } else { end - start + 1 };
    let mut r = Response::builder()
        .status(if range.is_some() { StatusCode::PARTIAL_CONTENT } else { StatusCode::OK })
        .header(header::CONTENT_TYPE, mime(&path))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, count)
        .header(header::CACHE_CONTROL, "no-store");
    if range.is_some() {
        r = r.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
    }
    if method == Method::HEAD || count == 0 {
        return r.body(Body::empty()).unwrap();
    }
    use tokio::io::{AsyncReadExt, AsyncSeekExt};
    if f.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return status(StatusCode::INTERNAL_SERVER_ERROR);
    }
    r.body(Body::from_stream(tokio_util::io::ReaderStream::new(f.take(count)))).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges() {
        assert_eq!(parse_range(None, 100), Ok(None));
        assert_eq!(parse_range(Some("bytes=0-"), 100), Ok(Some((0, 99))));
        assert_eq!(parse_range(Some("bytes=10-19"), 100), Ok(Some((10, 19))));
        assert_eq!(parse_range(Some("bytes=90-200"), 100), Ok(Some((90, 99))));
        assert_eq!(parse_range(Some("bytes=-10"), 100), Ok(Some((90, 99))));
        assert_eq!(parse_range(Some("bytes=-500"), 100), Ok(Some((0, 99))));
        assert_eq!(parse_range(Some("bytes=0-1, 5-6"), 100), Ok(Some((0, 1))));
        assert_eq!(parse_range(Some("bytes=100-"), 100), Err(()));
        assert_eq!(parse_range(Some("bytes=5-1"), 100), Err(()));
        assert_eq!(parse_range(Some("items=0-1"), 100), Err(()));
        assert_eq!(parse_range(Some("bytes=-0"), 100), Err(()));
        assert_eq!(parse_range(Some("bytes=0-"), 0), Err(()));
    }

    #[test]
    fn token_compare() {
        assert!(same("abc", "abc"));
        assert!(!same("abc", "abd") && !same("abc", "ab"));
    }

    /// 起一个真的服务：整段、Range、HEAD、令牌不对、编号不对
    #[tokio::test]
    async fn serves_ranges() {
        let dir = tempfile::tempdir().unwrap();
        let f = dir.path().join("a.mp4");
        std::fs::write(&f, (0u8..=99).collect::<Vec<_>>()).unwrap();
        let s = MediaServer { port: 0, token: "t0k".into(), files: Default::default() };
        let l = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let port = l.local_addr().unwrap().port();
        let s = MediaServer { port, ..s };
        let url = s.url_for(&f);
        assert_eq!(url, s.url_for(&f), "同一个文件同一个编号");
        let router = Router::new().route("/{token}/{id}", get(serve)).with_state(s.clone());
        tokio::spawn(async move { axum::serve(l, router).await });
        let c = reqwest::Client::new();
        let r = c.get(&url).send().await.unwrap();
        assert_eq!(r.status(), 200);
        assert_eq!(r.headers()["content-type"], "video/mp4");
        assert_eq!(r.bytes().await.unwrap().len(), 100);
        let r = c.get(&url).header("Range", "bytes=10-19").send().await.unwrap();
        assert_eq!(r.status(), 206);
        assert_eq!(r.headers()["content-range"], "bytes 10-19/100");
        assert_eq!(r.bytes().await.unwrap().to_vec(), (10u8..=19).collect::<Vec<_>>());
        let r = c.head(&url).send().await.unwrap();
        assert_eq!((r.status().as_u16(), r.headers()["content-length"].to_str().unwrap()), (200, "100"));
        let r = c.get(&url).header("Range", "bytes=200-").send().await.unwrap();
        assert_eq!(r.status(), 416);
        let bad = url.replace("/t0k/", "/t0x/");
        assert_eq!(c.get(&bad).send().await.unwrap().status(), 404);
        assert_eq!(c.get(format!("http://127.0.0.1:{port}/t0k/nope")).send().await.unwrap().status(), 404);
    }
}
