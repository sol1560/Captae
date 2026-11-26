//! 只下开头一小段，给几百个网站的批量测试用（命令行 `captae download <链接> --sample <MB> --json`，计划第 4 节）。
//!
//! 读取信息和真实下载走同一套逻辑（错误码、YouTube 换客户端、播放列表、代理），但不进队列、不入素材库：
//! - 网页（yt-dlp）：先读信息，再 `--load-info-json` 加 `--download-sections "*0-N"` 只下开头 N 秒。
//!   N 按选中格式的码率从 MB 换算（3–120 秒）；视频比 N 短就整段下。截取由 ffmpeg 完成，出来的文件是完整的。
//!   不用 `--max-filesize`：它遇到大文件是整个跳过，不是截断
//! - 直链：先做和 aria2 引擎一样的探测（HEAD），再用 Range 只下前 N MB。MP4 / MOV 这类把索引放在文件末尾的，
//!   开头那一段读不了，这时改用 ffmpeg 从网址截前 N MB（ffmpeg 会自己去文件末尾读索引）
//! - M3U8：选最高清晰度，按顺序下分片攒够 N MB，再用 ffmpeg 封装成 MP4
//!
//! 下完都用 ffprobe 检查文件能不能读。

use crate::detect::{self, Kind};
use crate::engine::{aria2, hls, ytdlp};
use crate::errors::{self, ErrorCode, Failure};
use crate::model::AddRequest;
use crate::tools::{run_full, Tool, Toolbox};
use crate::tr;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

pub struct Opts {
    pub url: String,
    /// 下多少 MB（1 MB = 1,000,000 字节）
    pub mb: f64,
    /// 样片放在这个目录下（每次一个子目录）
    pub dir: PathBuf,
    pub headers: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sampled {
    /// page / hls / file
    pub kind: &'static str,
    /// 怎么截的：ytdlp-sections（yt-dlp 只下开头）/ ytdlp-whole（视频很短，整段下）/
    /// range（直链 Range）/ whole（直链整个文件比 N MB 小）/ ffmpeg（直链改用 ffmpeg 截）/ hls-segments
    pub method: &'static str,
    pub title: Option<String>,
    pub path: PathBuf,
    pub bytes: u64,
    /// ffprobe 读到的：种类、格式、时长、分辨率、编码
    pub media: crate::ffmpeg::Probe,
    /// 播放列表 / 多视频帖子一共几个条目（只下了第一个）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entries: Option<usize>,
    /// 过程中的说明（自动重试、换客户端、选了哪个清晰度……）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

fn io_fail(e: std::io::Error) -> Failure {
    errors::from_io(&e).unwrap_or_else(|| Failure::new(ErrorCode::Unknown, tr!("写样片文件失败"), e.to_string()))
}

/// 下一小段。失败时给出分类好的原因。整个过程最多 900 秒（测试脚本等 960 秒；到点时 yt-dlp、ffmpeg 一起结束）
pub async fn run(tools: &Toolbox, http: &reqwest::Client, o: &Opts) -> Result<Sampled, Failure> {
    // 跳转包装、B 站语言前缀、Dropbox 分享页、Commons 文件页先理顺（和真实下载一样）
    let o = &Opts { url: detect::canonical(&o.url), mb: o.mb, dir: o.dir.clone(), headers: o.headers.clone() };
    match tokio::time::timeout(Duration::from_secs(900), run_inner(tools, http, o)).await {
        Ok(r) => r,
        Err(_) => Err(Failure::new(ErrorCode::Network, tr!("样片等了 900 秒还没下完，网络可能不稳定"), format!("sample timed out after 900s: {}", o.url))),
    }
}

async fn run_inner(tools: &Toolbox, http: &reqwest::Client, o: &Opts) -> Result<Sampled, Failure> {
    let d = detect::detect(&o.url).ok_or_else(|| Failure::new(ErrorCode::Unsupported, tr!("链接不完整，检查一下是不是少复制了开头的 https://"), format!("invalid url: {}", o.url)))?;
    let key = crate::util::url_key(&o.url);
    let dir = o.dir.join(format!("{}-{}", crate::util::now_ms(), &key[..8]));
    tokio::fs::create_dir_all(&dir).await.map_err(io_fail)?;
    let max = (o.mb.max(0.1) * 1e6) as u64;
    let notes = Mutex::new(vec![]);
    let r = match d.kind {
        Kind::Page => page(tools, http, o, &dir, max, &notes).await,
        Kind::Hls => hls_sample(tools, http, &o.url, &o.headers, &dir, max, &notes).await,
        Kind::File => file(tools, http, &o.url, &o.headers, &dir, max, &notes).await,
    };
    let mut s = r?;
    s.notes = notes.into_inner().unwrap_or_default();
    Ok(s)
}

/// ffprobe 检查：要有时长大于 0 的视频或音频（图片要有宽度）。
async fn check(tools: &Toolbox, path: &Path, url: &str) -> Result<crate::ffmpeg::Probe, Failure> {
    let p = match crate::ffmpeg::probe(tools, path).await {
        Ok(p) => p,
        Err(e) => {
            let f = errors::from_anyhow(&e, url);
            if f.code == ErrorCode::Tool {
                return Err(f);
            }
            return Err(Failure::new(ErrorCode::Unknown, tr!("样片下载了，但 ffprobe 读不了"), f.raw));
        }
    };
    let ok = match p.kind.as_str() {
        "video" | "audio" => p.duration.is_some_and(|d| d > 0.0),
        // 宽、高都要大于 0：Commons 的 .jpg 文件页下到的是网页，ffprobe 按扩展名读出 0×0（第一轮唯一的假成功）
        "image" => p.width.unwrap_or(0) > 0 && p.height.unwrap_or(0) > 0,
        _ => false,
    };
    if !ok {
        return Err(Failure::new(ErrorCode::Unknown, tr!("样片下载了，但里面没有能读的视频或音频"), format!("ffprobe: kind={} format={} duration={:?}", p.kind, p.format, p.duration)));
    }
    Ok(p)
}

async fn size_of(p: &Path) -> u64 {
    tokio::fs::metadata(p).await.map(|m| m.len()).unwrap_or(0)
}

// ------------------------------------------------------------------ 网页（yt-dlp）

/// MB → 秒：按选中格式的码率（kbps）换算，夹在 3–120 秒；视频本身不比这长就整段下（None）。
pub fn sample_secs(raw: &Value, max_bytes: u64) -> Option<f64> {
    let dur = raw.get("duration").and_then(Value::as_f64).filter(|d| *d > 0.0);
    let num = |v: &Value, k: &str| v.get(k).and_then(Value::as_f64).filter(|x| *x > 0.0);
    let tbr = raw
        .get("requested_formats")
        .and_then(Value::as_array)
        .map(|fs| fs.iter().filter_map(|f| num(f, "tbr")).sum::<f64>())
        .filter(|t| *t > 0.0)
        .or_else(|| num(raw, "tbr"))
        .or_else(|| {
            let size = num(raw, "filesize").or_else(|| num(raw, "filesize_approx"))?;
            Some(size * 8.0 / 1000.0 / dur?)
        });
    let secs = match tbr {
        Some(t) => max_bytes as f64 * 8.0 / 1000.0 / t,
        None => 20.0,
    }
    .clamp(3.0, 120.0);
    match dur {
        Some(d) if d <= secs + 1.0 => None,
        _ => Some(secs),
    }
}

async fn page(tools: &Toolbox, http: &reqwest::Client, o: &Opts, dir: &Path, max: u64, notes: &Mutex<Vec<String>>) -> Result<Sampled, Failure> {
    let note = |s: String| notes.lock().unwrap().push(s);
    let (raw, extra, entries) = probe_first(tools, http, o, &note, vec![]).await?;
    let r = match sample_page(tools, o, dir, max, &raw, &extra, &note).await {
        // YouTube 下载时偶发 403：和真实下载一样，换一组客户端重新读一次、再下一次（FIXES 第 17 项）
        Err(f) if ytdlp::is_youtube(&o.url) && f.code == ErrorCode::NeedVerify && f.raw.contains("403") && !extra.iter().any(|a| a.contains("player_client")) => {
            note(tr!("YouTube 拒绝了下载（403），换一组客户端（{clients}）重新读取，再下一次", clients = ytdlp::YT_ALT_CLIENTS));
            let (raw, extra, _) = probe_first(tools, http, o, &note, ytdlp::alt_clients()).await?;
            sample_page(tools, o, dir, max, &raw, &extra, &note).await
        }
        r => r,
    };
    r.map(|mut s| {
        s.entries = entries;
        s
    })
}

/// 读取信息；播放列表、多视频帖子读第一个。返回 (信息, 读取时额外用过的参数, 一共几个条目)
async fn probe_first(tools: &Toolbox, http: &reqwest::Client, o: &Opts, note: &(dyn Fn(String) + Send + Sync), extra: Vec<String>) -> Result<(Value, Vec<String>, Option<usize>), Failure> {
    let pr = ytdlp::ProbeReq { url: &o.url, headers: &o.headers, item: None, record_live: false, note, auth: None, extra: extra.clone() };
    Ok(match ytdlp::probe_url(tools, http, &pr).await? {
        ytdlp::Probed::Video { raw, extra, .. } => (raw, extra, None),
        ytdlp::Probed::List { entries, .. } => {
            // 播放列表、多视频帖子：只下第一个
            let n = entries.len();
            let e = entries.into_iter().next().ok_or_else(|| Failure::new(ErrorCode::NoMedia, tr!("这个列表里没有视频"), "playlist has no entries"))?;
            note(tr!("这里有 {n} 个视频，只下第一个：{title}", n = n, title = e.title));
            let url = if e.item.is_some() { o.url.clone() } else { e.url.clone() };
            let pr = ytdlp::ProbeReq { url: &url, headers: &o.headers, item: e.item, record_live: false, note, auth: None, extra };
            match ytdlp::probe_url(tools, http, &pr).await? {
                ytdlp::Probed::Video { raw, extra, .. } => (raw, extra, Some(n)),
                ytdlp::Probed::List { .. } => return Err(Failure::new(ErrorCode::NoMedia, tr!("列表里的第一个还是列表，没有能直接下的视频"), "nested playlist")),
            }
        }
    })
}

/// 用和真实下载一样的格式选择（`-f` + `-S`）问 yt-dlp 最后会下哪一路，按那一路的码率算秒数（FIXES 第 5 项：
/// 以前按读取时默认格式的码率算，YouTube 实际下的是另一路，样片是要求的 5–10 倍）。问不到时用读取时的信息
async fn selected(tools: &Toolbox, args: &[String], info: &Path) -> Option<Value> {
    let mut c = tools.command(Tool::YtDlp).ok()?;
    c.args(args).arg("--load-info-json").arg(info).args(["--simulate", "-j"]);
    let r = run_full(c, Duration::from_secs(60)).await.ok().filter(|r| r.ok)?;
    String::from_utf8_lossy(&r.stdout).lines().rev().find_map(|l| serde_json::from_str::<Value>(l.trim()).ok())
}

async fn sample_page(tools: &Toolbox, o: &Opts, dir: &Path, max: u64, raw: &Value, extra: &[String], note: &(dyn Fn(String) + Send + Sync)) -> Result<Sampled, Failure> {
    let mut raw = raw.clone();
    ytdlp::prepare_info(&mut raw);
    let info = dir.join("info.json");
    tokio::fs::write(&info, serde_json::to_vec(&raw).unwrap_or_default()).await.map_err(io_fail)?;
    let shape = ytdlp::Shape::of(Some(&ytdlp::info_from_json(&raw)));
    let ffmpeg = tools.path(Tool::Ffmpeg).ok();
    let req = AddRequest { url: o.url.clone(), headers: o.headers.clone(), ..Default::default() };
    let tmp = dir.join(".captae-tmp");
    let mut args = ytdlp::download_args(&req, dir, &tmp, ffmpeg.as_deref(), None, false, shape, true);
    // 读取时 YouTube 换过客户端：下载时照用
    args.extend(extra.iter().cloned());
    let title = raw.get("title").and_then(Value::as_str).map(str::to_string);
    let sel = selected(tools, &args, &info).await;
    let dur = sel.as_ref().unwrap_or(&raw).get("duration").and_then(Value::as_f64).filter(|d| *d > 0.0);
    let secs = sample_secs(sel.as_ref().unwrap_or(&raw), max);
    let done = |path: PathBuf, media: crate::ffmpeg::Probe, bytes: u64, method: &'static str| Sampled { kind: "page", method, title: title.clone(), path, bytes, media, entries: None, notes: vec![] };

    // 1. 有时长、视频比要的长：只截开头 N 秒（ffmpeg 截，出来的文件是完整的）
    let mut why_whole = None;
    if let (Some(s), Some(_), false) = (secs, dur, shape.image) {
        let mut a = args.clone();
        a.push("--download-sections".into());
        a.push(format!("*0-{}", s.ceil() as u64));
        match ytdlp_file(tools, &a, &info, &o.url).await {
            Ok(path) => {
                let media = check(tools, &path, &o.url).await?;
                let _ = tokio::fs::remove_file(&info).await;
                return Ok(done(path.clone(), media, size_of(&path).await, "ytdlp-sections"));
            }
            Err(f) if f.code == ErrorCode::Tool => return Err(f),
            // 截不了的格式（拆开的 HLS/DASH、服务器不配合 ffmpeg 读）：改成整段下、够数就停
            Err(f) => why_whole = Some(f),
        }
    }
    // 2. 整段下（视频很短、没有时长、图片、截取失败）：下到 N MB 就停，检查已经下到的那一段
    if let Some(f) = &why_whole {
        note(tr!("截取开头失败（{reason}），改成整段下载、够 {size} MB 就停", reason = f.message, size = max / 1_000_000));
    } else if dur.is_none() && !shape.image {
        note(tr!("没有时长，整段下载、够 {size} MB 就停", size = max / 1_000_000));
    }
    let r = capped(tools, &args, &info, &tmp, max, &o.url).await;
    let _ = tokio::fs::remove_file(&info).await;
    match r {
        Ok((path, whole)) => {
            let media = check(tools, &path, &o.url).await?;
            let bytes = size_of(&path).await;
            Ok(done(path, media, bytes, if whole { "ytdlp-whole" } else { "ytdlp-capped" }))
        }
        // 截取也失败过：给截取那一次的原因（更能说明问题）
        Err(e) => Err(why_whole.filter(|w| w.code != ErrorCode::Unknown || e.code == ErrorCode::Unknown).unwrap_or(e)),
    }
}

/// 跑一次 yt-dlp 下载，返回它报出的文件路径
async fn ytdlp_file(tools: &Toolbox, args: &[String], info: &Path, url: &str) -> Result<PathBuf, Failure> {
    let mut c = tools.command(Tool::YtDlp).map_err(|e| errors::from_anyhow(&e, url))?;
    c.args(args).arg("--load-info-json").arg(info);
    let r = run_full(c, Duration::from_secs(600)).await?;
    let out = String::from_utf8_lossy(&r.stdout).to_string();
    if !r.ok {
        return Err(errors::classify(&r.stderr, url));
    }
    out.lines()
        .find_map(|l| match ytdlp::parse_line(l) {
            ytdlp::Line::File(p) => Some(p),
            _ => None,
        })
        .ok_or_else(|| Failure::new(ErrorCode::Unknown, tr!("yt-dlp 没有返回文件路径"), format!("{}\n{}", r.stderr, out)))
}

/// 整段下载，下到 `max` 字节就结束 yt-dlp（连同它的 ffmpeg，tools::Proc），返回 (文件, 是不是整个下完了)。
/// 没下完时拿中间目录里的主文件（`x.mp4.part`，不是 `-FragN` 分片）来检查：HLS 的分片、DASH 的分段、MP3 从开头就能读。
/// 总大小已经知道、又不大（不到要的 5 倍，最少 20 MB）时整个下完：MP4 的索引（moov）常常在文件末尾，下一半读不了（giphy-1d28e6）
async fn capped(tools: &Toolbox, args: &[String], info: &Path, tmp: &Path, max: u64, url: &str) -> Result<(PathBuf, bool), Failure> {
    use tokio::io::{AsyncBufReadExt, BufReader};
    let mut c = tools.command(Tool::YtDlp).map_err(|e| errors::from_anyhow(&e, url))?;
    c.args(args).arg("--load-info-json").arg(info);
    c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = crate::tools::spawn(&mut c).map_err(|e| crate::tools::spawn_failure("yt-dlp", &e))?;
    let mut err = child.stderr.take().unwrap();
    let et = tokio::spawn(async move {
        let mut s = String::new();
        use tokio::io::AsyncReadExt;
        let _ = err.read_to_string(&mut s).await;
        s
    });
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut per: std::collections::HashMap<String, (f64, Option<f64>)> = Default::default();
    let mut file = None;
    let mut enough = false;
    let small = (max as f64 * 5.0).max(20e6);
    let read = async {
        while let Ok(Some(l)) = lines.next_line().await {
            match ytdlp::parse_line(&l) {
                ytdlp::Line::Progress { got, total, fmt, .. } => {
                    per.insert(fmt, (got, total));
                    let whole = per.values().all(|(_, t)| t.is_some_and(|t| t <= small));
                    if per.values().map(|(g, _)| g).sum::<f64>() >= max as f64 && !whole {
                        enough = true;
                        break;
                    }
                }
                ytdlp::Line::File(p) => file = Some(p),
                _ => {}
            }
        }
    };
    if tokio::time::timeout(Duration::from_secs(600), read).await.is_err() {
        return Err(Failure::new(ErrorCode::Network, tr!("样片等了 600 秒还没下够，网络可能不稳定"), format!("capped sample timed out: {url}")));
    }
    if enough {
        // 够了：结束 yt-dlp 和它起的 ffmpeg（丢掉 Proc 就是 kill_tree），拿已经下到的那一段
        drop(child);
        let _ = et.await;
        let mut found = vec![];
        for p in walk(tmp) {
            let n = tokio::fs::metadata(&p).await.map(|m| m.len()).unwrap_or(0);
            found.push((p, n));
        }
        return partial_of(found).map(|p| (p, false)).ok_or_else(|| Failure::new(ErrorCode::Unknown, tr!("样片下了，但没找到下到一半的文件"), format!("no partial file in {}", tmp.display())));
    }
    let st = child.wait().await.map_err(|e| Failure::new(ErrorCode::Unknown, tr!("等 yt-dlp 结束失败"), e.to_string()))?;
    let stderr = et.await.unwrap_or_default();
    if !st.success() {
        return Err(errors::classify(&stderr, url));
    }
    file.map(|p| (p, true)).ok_or_else(|| Failure::new(ErrorCode::Unknown, tr!("yt-dlp 没有返回文件路径"), stderr))
}

/// 下到一半时拿哪个文件检查：主文件（分片按顺序接在它后面，开头有初始化段）优先，没有才拿最大的分片；
/// yt-dlp 记分片进度的 `.ytdl` 不算
fn partial_of(found: Vec<(PathBuf, u64)>) -> Option<PathBuf> {
    let name = |p: &Path| p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    found
        .into_iter()
        .filter(|(p, n)| *n > 0 && !name(p).ends_with(".ytdl"))
        .max_by_key(|(p, n)| (!name(p).contains("-Frag"), *n))
        .map(|(p, _)| p)
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = vec![];
    let Ok(rd) = std::fs::read_dir(dir) else { return out };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

// ------------------------------------------------------------------ 直链

async fn file(tools: &Toolbox, http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, dir: &Path, max: u64, notes: &Mutex<Vec<String>>) -> Result<Sampled, Failure> {
    let head = aria2::head(http, url, headers, &crate::login::Jar::default()).await.map_err(|e| errors::from_anyhow(&e, url))?;
    if head.content_type.as_deref().is_some_and(|c| c.contains("mpegurl")) {
        // 地址没有 .m3u8 后缀，但其实是 M3U8
        return hls_sample(tools, http, url, headers, dir, max, notes).await;
    }
    if head.is_html() {
        // 地址以 .mp4/.jpg 结尾、打开却是网页：和真实下载一样交给下载引擎（FIXES 第 9 项）
        notes.lock().unwrap().push(tr!("这个地址打开是网页，不是文件，改用下载引擎读取"));
        let o = Opts { url: url.to_string(), mb: max as f64 / 1e6, dir: dir.to_path_buf(), headers: headers.clone() };
        return page(tools, http, &o, dir, max, notes).await;
    }
    if head.is_stream() {
        return Err(errors::radio(format!("audio stream without length (icy={}): {url}", head.icy)));
    }
    let path = dir.join(&head.filename);
    let got = range_get(http, url, headers, max, &path).await?;
    let whole = head.size.is_some_and(|s| got >= s);
    let first = check(tools, &path, url).await;
    match first {
        Ok(media) => {
            return Ok(Sampled { kind: "file", method: if whole { "whole" } else { "range" }, title: Some(head.filename.clone()), bytes: got, path, media, entries: None, notes: vec![] });
        }
        // 整个文件都下下来了还读不了：就是读不了
        Err(f) if whole => return Err(f),
        Err(f) => notes.lock().unwrap().push(tr!("开头 {size} MB 读不了（{reason}），改用 ffmpeg 截取", size = got / 1_000_000, reason = f.message)),
    }
    let out = dir.join(format!("ffmpeg-{}", head.filename));
    ffmpeg_cut(tools, url, headers, max, &out).await?;
    let media = check(tools, &out, url).await?;
    let _ = tokio::fs::remove_file(&path).await;
    Ok(Sampled { kind: "file", method: "ffmpeg", title: Some(head.filename), bytes: size_of(&out).await, path: out, media, entries: None, notes: vec![] })
}

/// Range 请求下前 `max` 字节；服务器不认 Range（回 200）时读到 `max` 就停。返回下了多少字节。
async fn range_get(http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, max: u64, path: &Path) -> Result<u64, Failure> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let mut r = http.get(url).header("Range", format!("bytes=0-{}", max.saturating_sub(1)));
    for (k, v) in headers {
        r = r.header(k, v);
    }
    let resp = r.send().await.map_err(|e| errors::from_reqwest(&e, url))?;
    if !resp.status().is_success() {
        return Err(errors::http_status(resp.status().as_u16(), url));
    }
    let mut f = tokio::fs::File::create(path).await.map_err(io_fail)?;
    let mut got = 0u64;
    let mut s = resp.bytes_stream();
    while let Some(c) = tokio::time::timeout(Duration::from_secs(30), s.next()).await.map_err(|_| Failure::new(ErrorCode::Network, tr!("网络不稳定，30 秒没有收到数据"), format!("no data for 30s: {url}")))? {
        let c = c.map_err(|e| errors::from_reqwest(&e, url))?;
        let take = ((max - got) as usize).min(c.len());
        f.write_all(&c[..take]).await.map_err(io_fail)?;
        got += take as u64;
        if got >= max {
            break;
        }
    }
    f.flush().await.map_err(io_fail)?;
    Ok(got)
}

/// ffmpeg 直接读网址，`-c copy` 截前 `max` 字节左右，写出的是完整的文件。
/// 不能只靠 `-fs`：ffmpeg 写文件前要把画面和声音按时间排好，一路的数据在原文件里离得远时会先攒在内存里（最多 10 秒），
/// `-fs` 只看已经写进文件的大小，攒着的那些最后一起写出来（Dropbox 的 4K 文件 3 MB 样片写出了 166 MB，正好约 10 秒）。
/// 所以先用 ffprobe 读出码率，算出要几秒，用 `-t` 截；`-fs` 只留着放宽到 3 倍兜着。读不出码率时照旧只用 `-fs`
async fn ffmpeg_cut(tools: &Toolbox, url: &str, headers: &BTreeMap<String, String>, max: u64, out: &Path) -> Result<(), Failure> {
    let secs = cut_seconds(tools, url, headers, max).await;
    let mut c = tools.command(Tool::Ffmpeg).map_err(|e| errors::from_anyhow(&e, url))?;
    c.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-user_agent", crate::net::UA]);
    if !headers.is_empty() {
        c.arg("-headers").arg(headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect::<String>());
    }
    c.arg("-i").arg(url).args(["-map", "0:v?", "-map", "0:a?", "-c", "copy"]);
    match secs {
        Some(t) => {
            c.arg("-t").arg(format!("{t:.2}")).arg("-fs").arg((max * 3).to_string());
        }
        None => {
            c.arg("-fs").arg(max.to_string());
        }
    }
    c.arg(out);
    let r = run_full(c, Duration::from_secs(600)).await?;
    if !r.ok {
        return Err(errors::classify_ffmpeg(&r.stderr, url));
    }
    Ok(())
}

/// 截前 `max` 字节要几秒：ffprobe 读网址的总码率（读不到就用 大小 × 8 ÷ 时长），最少 1 秒。读不出来时 None
async fn cut_seconds(tools: &Toolbox, url: &str, headers: &BTreeMap<String, String>, max: u64) -> Option<f64> {
    let mut c = tools.command(Tool::Ffprobe).ok()?;
    c.args(["-v", "error", "-user_agent", crate::net::UA]);
    if !headers.is_empty() {
        c.arg("-headers").arg(headers.iter().map(|(k, v)| format!("{k}: {v}\r\n")).collect::<String>());
    }
    c.args(["-show_entries", "format=bit_rate,duration,size", "-of", "json"]).arg(url);
    let r = run_full(c, Duration::from_secs(120)).await.ok().filter(|r| r.ok)?;
    let v: serde_json::Value = serde_json::from_slice(&r.stdout).ok()?;
    cut_seconds_of(&v, max)
}

/// ffprobe `format` 那一段 → 截 `max` 字节要几秒
fn cut_seconds_of(v: &serde_json::Value, max: u64) -> Option<f64> {
    let f = v.get("format")?;
    let num = |k: &str| f.get(k).and_then(|x| x.as_str()).and_then(|x| x.parse::<f64>().ok()).filter(|x| *x > 0.0);
    let rate = num("bit_rate").or_else(|| Some(num("size")? * 8.0 / num("duration")?))?;
    let t = (max as f64 * 8.0 / rate).max(1.0);
    Some(match num("duration") {
        Some(d) => t.min(d),
        None => t,
    })
}

// ------------------------------------------------------------------ M3U8

async fn hls_sample(tools: &Toolbox, http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, dir: &Path, max: u64, notes: &Mutex<Vec<String>>) -> Result<Sampled, Failure> {
    let p = hls::probe(http, url, headers, &crate::login::Jar::default()).await.map_err(|e| errors::from_anyhow(&e, url))?;
    let (media, audio, label) = match p.variants.first() {
        Some(v) => (v.uri.clone(), v.audio.clone(), Some(v.p.clone())),
        None => (p.media_url.clone().unwrap_or_else(|| url.to_string()), None, None),
    };
    if let Some(l) = &label {
        notes.lock().unwrap().push(if audio.is_some() {
            tr!("有 {n} 种清晰度，下的是最高的 {quality}（声音单独一路）", n = p.variants.len(), quality = l)
        } else {
            tr!("有 {n} 种清晰度，下的是最高的 {quality}", n = p.variants.len(), quality = l)
        });
    }
    let out = dir.join("sample.mp4");
    let parts = dir.join("parts");
    hls::sample(http, tools, &media, audio.as_deref(), headers, max, &parts, &out).await.map_err(|e| errors::from_anyhow(&e, url))?;
    let _ = tokio::fs::remove_dir_all(&parts).await;
    let m = check(tools, &out, url).await?;
    Ok(Sampled { kind: "hls", method: "hls-segments", title: label, bytes: size_of(&out).await, path: out, media: m, entries: None, notes: vec![] })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 集成和收尾第 4 项：直链样片按码率算要截几秒（Dropbox 4K 那个 135 Mbps，3 MB 不到 0.2 秒，最少截 1 秒）
    #[test]
    fn cut_seconds_by_bitrate() {
        let v = serde_json::json!({"format": {"bit_rate": "8000000", "duration": "120.0", "size": "120000000"}});
        assert_eq!(cut_seconds_of(&v, 3_000_000), Some(3.0));
        let v = serde_json::json!({"format": {"duration": "223.5", "size": "3774375295"}});
        assert_eq!(cut_seconds_of(&v, 3_000_000), Some(1.0));
        // 文件比要的还短：截整个时长
        let v = serde_json::json!({"format": {"bit_rate": "64000", "duration": "10.0"}});
        assert_eq!(cut_seconds_of(&v, 3_000_000), Some(10.0));
        assert_eq!(cut_seconds_of(&serde_json::json!({"format": {}}), 3_000_000), None);
    }
    use serde_json::json;

    /// brightcove-a0c9d3：下到一半时目录里有主文件和正在下的分片，分片没有初始化段，ffprobe 读不了
    #[test]
    fn partial_file_choice() {
        let p = |s: &str, n: u64| (PathBuf::from(s), n);
        let found = vec![
            p("/t/Living Desert Giraffe.mp4.part-Frag2.part", 900_000),
            p("/t/Living Desert Giraffe.mp4.part", 600_000),
            p("/t/Living Desert Giraffe.mp4.ytdl", 2_000),
        ];
        assert_eq!(partial_of(found), Some(PathBuf::from("/t/Living Desert Giraffe.mp4.part")));
        // 主文件还是空的：拿分片（TS 分片自己就能读）
        assert_eq!(partial_of(vec![p("/t/a.mp4.part", 0), p("/t/a.mp4.part-Frag1.part", 10)]), Some(PathBuf::from("/t/a.mp4.part-Frag1.part")));
        assert_eq!(partial_of(vec![]), None);
    }

    #[test]
    fn seconds_from_bitrate() {
        // 两路（视频 2000 + 声音 128 kbps），5 MB ≈ 18.8 秒
        let v = json!({"duration": 600.0, "requested_formats": [{"tbr": 2000.0}, {"tbr": 128.0}]});
        let s = sample_secs(&v, 5_000_000).unwrap();
        assert!((s - 18.796).abs() < 0.01, "{s}");
        // 只有顶层 tbr
        assert!((sample_secs(&json!({"duration": 600.0, "tbr": 8000.0}), 5_000_000).unwrap() - 5.0).abs() < 1e-9);
        // 码率很低：最多 120 秒
        assert_eq!(sample_secs(&json!({"duration": 3600.0, "tbr": 32.0}), 5_000_000), Some(120.0));
        // 码率很高：最少 3 秒
        assert_eq!(sample_secs(&json!({"duration": 600.0, "tbr": 100000.0}), 5_000_000), Some(3.0));
        // 没有码率：按大小 / 时长算；都没有就 20 秒
        assert!((sample_secs(&json!({"duration": 100.0, "filesize": 50_000_000}), 5_000_000).unwrap() - 10.0).abs() < 1e-9);
        assert_eq!(sample_secs(&json!({}), 5_000_000), Some(20.0));
        // 视频比算出来的还短：整段下
        assert_eq!(sample_secs(&json!({"duration": 12.0, "tbr": 1000.0}), 5_000_000), None);
    }
}
