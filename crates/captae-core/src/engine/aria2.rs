//! aria2 路径：媒体直链。先发一个 HEAD（失败就 GET Range 0-0）拿到大小、类型和文件名，
//! 然后交给 aria2c 多线程下载，`-c` 保证暂停后能接着下。

use super::{Ctx, MediaInfo, Output, SpeedMeter};
use crate::tools::Tool;
use crate::tr;
use crate::util::{mb, safe_name};
use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;

#[derive(Debug, Clone, Default)]
pub struct Head {
    pub size: Option<u64>,
    pub content_type: Option<String>,
    pub filename: String,
    /// 有 `icy-*` 头（icy-name、icy-br）：Icecast / SHOUTcast 电台
    pub icy: bool,
}

impl Head {
    /// 电台直播流：有 icy 头，或者是音频却没有大小（FIP、Radio Paradise）。当普通文件下会一直下下去（FIXES 第 13 项）
    pub fn is_stream(&self) -> bool {
        let audio = self.content_type.as_deref().is_some_and(|c| {
            let c = c.to_ascii_lowercase();
            (c.starts_with("audio/") || c.starts_with("application/ogg")) && !c.contains("mpegurl")
        });
        self.icy || (self.size.is_none() && audio)
    }

    /// 网页（Dropbox 分享页、Commons 文件页这种地址以 .mp4/.jpg 结尾的）
    pub fn is_html(&self) -> bool {
        self.content_type.as_deref().is_some_and(|c| {
            let c = c.to_ascii_lowercase();
            c.starts_with("text/html") || c.starts_with("application/xhtml")
        })
    }
}

pub fn filename_from(url: &str, disposition: Option<&str>, content_type: Option<&str>) -> String {
    static CD: Lazy<Regex> = Lazy::new(|| Regex::new(r#"filename\*?=(?:UTF-8'')?"?([^";]+)"?"#).unwrap());
    if let Some(d) = disposition {
        if let Some(c) = CD.captures(d) {
            let raw = c.get(1).unwrap().as_str();
            let dec = url::form_urlencoded::parse(format!("x={raw}").as_bytes()).next().map(|(_, v)| v.to_string()).unwrap_or(raw.to_string());
            return safe_name(&dec);
        }
    }
    let from_path = url::Url::parse(url)
        .ok()
        .and_then(|u| u.path_segments().and_then(|mut s| s.next_back().map(str::to_string)))
        .map(|s| url::form_urlencoded::parse(format!("x={s}").as_bytes()).next().map(|(_, v)| v.to_string()).unwrap_or(s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "download".into());
    let mut name = safe_name(&from_path);
    if !name.contains('.') {
        if let Some(ext) = content_type.and_then(ext_for_type) {
            name.push('.');
            name.push_str(ext);
        }
    }
    name
}

fn ext_for_type(ct: &str) -> Option<&'static str> {
    let ct = ct.split(';').next()?.trim().to_ascii_lowercase();
    Some(match ct.as_str() {
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "video/x-matroska" => "mkv",
        "audio/mpeg" => "mp3",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "audio/flac" => "flac",
        "audio/wav" | "audio/x-wav" => "wav",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        "image/avif" => "avif",
        "image/gif" => "gif",
        _ => return None,
    })
}

pub async fn head(http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, jar: &crate::login::Jar) -> Result<Head> {
    let mk = |m: reqwest::Method| {
        let mut r = http.request(m, url);
        for (k, v) in headers {
            r = r.header(k, v);
        }
        jar.apply(r, url, headers)
    };
    let resp = match mk(reqwest::Method::HEAD).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => mk(reqwest::Method::GET).header("Range", "bytes=0-0").send().await.map_err(|e| crate::errors::from_reqwest(&e, url))?,
    };
    if !resp.status().is_success() {
        return Err(crate::errors::http_status(resp.status().as_u16(), url).into());
    }
    let h = resp.headers();
    let get = |k: &str| h.get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
    // HEAD 的回应没有正文，reqwest 的 content_length() 给的是正文的长度（0），要直接读 Content-Length 头
    let declared = get("content-length").and_then(|s| s.trim().parse::<u64>().ok()).filter(|n| *n > 0);
    let size = get("content-range")
        .and_then(|r| r.rsplit('/').next().and_then(|s| s.parse().ok()))
        .or_else(|| if resp.status().as_u16() == 206 { None } else { declared.or_else(|| resp.content_length().filter(|n| *n > 0)) });
    let ct = get("content-type");
    let icy = h.keys().any(|k| k.as_str().starts_with("icy-"));
    // 电台流回 206 时 Content-Range 的总长是个假数（Radio Paradise：bytes 0-0/1073741823），不当真
    let size = if icy { None } else { size };
    Ok(Head { size, filename: filename_from(url, get("content-disposition").as_deref(), ct.as_deref()), content_type: ct, icy })
}

#[derive(Debug, PartialEq)]
pub struct Readout {
    pub got: f64,
    pub total: Option<f64>,
    pub conns: u32,
    pub speed: f64,
}

/// 解析 aria2 控制台读数：`[#2089b0 12MiB/100MiB(12%) CN:16 DL:4.2MiB ETA:18s]`；
/// 不知道总大小的（电台流）是 `[#274b0c 31KiB/0B CN:1 DL:15KiB]`，没有百分比，总大小写成 0B
pub fn parse_readout(l: &str) -> Option<Readout> {
    static RE: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r"\[#\w+ ([\d.]+[KMGT]?i?B)(?:/([\d.]+[KMGT]?i?B)(?:\(\d+%\))?)? CN:(\d+) DL:([\d.]+[KMGT]?i?B)").unwrap()
    });
    let c = RE.captures(l)?;
    Some(Readout {
        got: super::parse_size(c.get(1)?.as_str())?,
        total: c.get(2).and_then(|m| super::parse_size(m.as_str())).filter(|t| *t > 0.0),
        conns: c.get(3)?.as_str().parse().ok()?,
        speed: super::parse_size(c.get(4)?.as_str())?,
    })
}

pub fn args(dir: &Path, name: &str, threads: u32, headers: &BTreeMap<String, String>, raw: Option<&[String]>) -> Vec<String> {
    let n = threads.clamp(1, 16);
    let mut a = vec![
        format!("--dir={}", dir.display()),
        format!("--out={name}"),
        format!("--max-connection-per-server={n}"),
        format!("--split={n}"),
        "--min-split-size=1M".into(),
        "--continue=true".into(),
        // 每秒把进度写进 .aria2 控制文件。暂停是直接结束子进程（SIGKILL），aria2 来不及保存；
        // 默认 60 秒才保存一次，60 秒内暂停的话继续时会从头下（2026-11-23 实测）
        "--auto-save-interval=1".into(),
        "--auto-file-renaming=false".into(),
        "--allow-overwrite=false".into(),
        "--summary-interval=1".into(),
        "--console-log-level=warn".into(),
        "--download-result=hide".into(),
        "--show-console-readout=true".into(),
        "--enable-color=false".into(),
        "--max-tries=5".into(),
        "--retry-wait=2".into(),
        "--file-allocation=none".into(),
        // 不发 aria2 自己的 Want-Digest 头（浏览器和 curl 都不发，Wikimedia 限流时去掉它下成过一次，后来交替测一直被限流，没测出确定的差别），
        // 也不跟 metalink 去下别的东西
        "--no-want-digest-header=true".into(),
        "--follow-metalink=false".into(),
    ];
    // 没带 User-Agent 时用和 Captae 自己的请求一样的（net::UA，像 Chrome）：读取信息（head）用的就是它，
    // aria2 自己的 UA 有的网站直接 403，结果读取信息成功、下载却被拒（2026-11-24 w3schools 的样片就是这样）。
    // 带了登录状态时 Auth::aria2 的 --user-agent 排在后面，以它为准；高级模式自己传的参数也排在后面
    if !headers.keys().any(|k| k.eq_ignore_ascii_case("user-agent")) {
        a.push(format!("--user-agent={}", crate::net::UA));
    }
    for (k, v) in headers {
        a.push(format!("--header={k}: {v}"));
    }
    if let Some(r) = raw {
        a.extend(r.iter().cloned());
    }
    a
}

pub fn probe_info(h: &Head) -> MediaInfo {
    MediaInfo {
        title: Some(h.filename.clone()),
        filesize: h.size,
        ext: h.filename.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()),
        ..Default::default()
    }
}

pub async fn download(ctx: &Ctx, url: &str, out_dir: &Path, name: &str) -> Result<Output> {
    let req = ctx.h.snapshot().req;
    let threads = req.threads.unwrap_or(ctx.settings.threads);
    let target: PathBuf = out_dir.join(name);
    // 已经下完的同名文件：aria2 不覆盖，这里换个名字
    let target = if target.exists() && !out_dir.join(format!("{name}.aria2")).exists() { crate::util::unique_path(target) } else { target };
    let final_name = target.file_name().unwrap().to_string_lossy().to_string();
    // 任务上记的是实际写的文件（换过名字时不是同名的旧文件），取消、失败时删的才是这个任务自己的。
    // 马上存进 queue.json：下载中退出、重新打开以后，继续时接着下、取消时知道删哪个
    ctx.h.update(|j| j.path = Some(target.clone()));
    ctx.h.persist();
    // 没有 aria2c（手机上，按 Runner 判断，不看编译开关）：交给 Rust 自己的下载器，同样用 `<文件>.aria2` 标记没下完的文件（direct.rs）。
    // 桌面端的 Runner 一律说有，找不到 aria2c 时照旧报 tool
    if !ctx.tools.has(Tool::Aria2) {
        return super::direct::run(ctx, url, &target).await;
    }
    if ctx.live() {
        return record(ctx, url, out_dir, &final_name).await;
    }
    match run(ctx, url, out_dir, &final_name, threads).await {
        // 有的网站不让同一个文件同时开多个连接，多开的那一路回 403 / 429，整个下载就停了：改成一个连接，接着同一个文件下
        Err(e) if threads > 1 && crate::errors::failure_in(&e).is_some_and(parallel_refused) => {
            ctx.h.update(|j| j.warn(tr!("网站不让同时开多个连接下载，改成一个连接接着下")));
            run(ctx, url, out_dir, &final_name, 1).await
        }
        r => r,
    }
}

/// 多开连接被网站拒了（其中一路回 403 / 429）。Dropbox 的大文件 16 路时有一路 403，单路能下完（2026-11-24 实测 dropbox-8b4b13）；
/// Wikimedia 也按连接数限流
pub fn parallel_refused(f: &crate::errors::Failure) -> bool {
    f.raw.contains("status=403") || f.raw.contains("status=429")
}

/// 录电台流（会一直播下去的音频流）：aria2 一直写同一个文件，点了「停止并保存」让它收尾，
/// 再把录到的部分原样整理成能拖动、时长准确的文件（没有文件头的 MP3 / AAC 流，播放器算不准时长）。
/// 网络断了：录到了东西就照样保存
async fn record(ctx: &Ctx, url: &str, out_dir: &Path, final_name: &str) -> Result<Output> {
    let raw = out_dir.join(final_name);
    ctx.note_raw(&raw);
    let r = run(ctx, url, out_dir, final_name, 1).await;
    let _ = tokio::fs::remove_file(format!("{}.aria2", raw.display())).await;
    let got = tokio::fs::metadata(&raw).await.map(|m| m.len()).unwrap_or(0);
    let stopped = ctx.h.snapshot().rec.is_some_and(|r| r.stopping);
    match r {
        Err(e) if got == 0 => return Err(e),
        Err(e) if !stopped => {
            let f = crate::errors::from_anyhow(&e, url);
            ctx.h.update(|j| j.warn(tr!("录制中断了（{reason}），录到的部分照样保存", reason = f.message)));
            ctx.note_stopping();
        }
        _ if got == 0 => return Err(anyhow::anyhow!(tr!("还没有录到内容"))),
        _ => {
            if !stopped {
                // 电台自己断开了（直播结束）
                ctx.note_stopping();
            }
        }
    }
    ctx.h.update(|j| j.log(tr!("正在把录到的部分整理成能播放的文件")));
    let path = tidy_stream(&ctx.tools, &raw).await?;
    Ok(Output { path, format_done: true })
}

/// 录下来的电台流（aria2 直接写在保存位置的那个文件）整理成能拖动、时长准确的文件，换掉原来那个。
/// 先整理到旁边的中间目录，整理好再换（同名，不多出「(2)」；扩展名按内容定，没有扩展名的会补上）
pub(crate) async fn tidy_stream(tools: &crate::tools::Toolbox, raw: &Path) -> Result<PathBuf> {
    let out_dir = raw.parent().unwrap_or(Path::new("."));
    let tmp = out_dir.join(".captae-tmp").join(format!("rec-{}", &crate::util::url_key(&raw.display().to_string())[..12]));
    let stem = raw.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| tr!("录音"));
    let tidy = crate::ffmpeg::tidy_recording(tools, raw, None, &tmp, &stem).await?;
    let _ = tokio::fs::remove_file(raw).await;
    let dest = crate::util::unique_path(out_dir.join(format!("{stem}.{}", tidy.extension().unwrap_or_default().to_string_lossy())));
    tokio::fs::rename(&tidy, &dest).await?;
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    let _ = tokio::fs::remove_dir(out_dir.join(".captae-tmp")).await;
    Ok(dest)
}

async fn run(ctx: &Ctx, url: &str, out_dir: &Path, final_name: &str, threads: u32) -> Result<Output> {
    let req = ctx.h.snapshot().req;
    let live = ctx.live();
    let target = out_dir.join(final_name);
    let mut c = ctx.tools.command(Tool::Aria2)?;
    // 登录状态：临时 cookies 文件（--load-cookies），aria2 退出后 `_cookies` 被丢掉时删掉
    let (auth_args, _cookies) = ctx.auth.aria2()?;
    c.args(args(out_dir, final_name, threads, &req.headers, req.raw_args.as_deref())).args(auth_args);
    if live {
        // 录电台：不用磁盘缓存（默认先攒 16 MB 才写，128 kbps 的电台要攒十几分钟；Captae 中途被结束时缓存里的就没了）
        c.arg("--disk-cache=0");
    }
    c.arg(url);
    c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = crate::tools::spawn(&mut c).map_err(|e| crate::tools::spawn_failure("aria2c", &e))?;
    let pid = child.id();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    let err_task = tokio::spawn(async move {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s).await;
        s
    });
    // 读数行用 \r 刷新而不是 \n，所以按字节自己切
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut meter = SpeedMeter::default();
    // aria2 的报错写在 stdout（console-log-level=warn），留最后 40 行给错误分类
    let mut tail: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    // 录电台点了「停止并保存」：给 aria2c 发 SIGINT（它会把数据写完再退出），最多等到这个时刻
    let mut stop_by: Option<tokio::time::Instant> = None;
    loop {
        let n = match stop_by {
            Some(t) => match tokio::time::timeout_at(t, stdout.read(&mut chunk)).await {
                Ok(r) => r?,
                Err(_) => {
                    if let Some(p) = pid {
                        crate::tools::kill_tree(p);
                    }
                    0
                }
            },
            None if live => tokio::select! {
                r = stdout.read(&mut chunk) => r?,
                _ = ctx.finish.cancelled() => {
                    ctx.note_stopping();
                    // 请求送不到时不用等，马上结束（aria2 边下边写文件，结束了录到的部分也在）
                    let sent = pid.is_some_and(crate::tools::interrupt);
                    stop_by = Some(tokio::time::Instant::now() + std::time::Duration::from_secs(if sent { 15 } else { 0 }));
                    continue;
                }
            },
            None => stdout.read(&mut chunk).await?,
        };
        if n == 0 {
            break;
        }
        for &b in &chunk[..n] {
            if b == b'\r' || b == b'\n' {
                let line = String::from_utf8_lossy(&buf).to_string();
                buf.clear();
                if let Some(r) = parse_readout(&line) {
                    let sp = if r.speed > 0.0 { r.speed } else { meter.push(r.got) };
                    ctx.h.update(|j| {
                        j.got = mb(r.got);
                        if let Some(t) = r.total.filter(|_| !live) {
                            j.size = mb(t);
                        }
                        j.speed = mb(sp);
                    });
                } else if !line.trim().is_empty() {
                    tail.push_back(line);
                    if tail.len() > 40 {
                        tail.pop_front();
                    }
                }
            } else {
                buf.push(b);
            }
        }
    }
    let st = match stop_by {
        Some(_) => match tokio::time::timeout(std::time::Duration::from_secs(15), child.wait()).await {
            Ok(s) => s?,
            Err(_) => {
                if let Some(p) = pid {
                    crate::tools::kill_tree(p);
                }
                child.wait().await?
            }
        },
        None => child.wait().await?,
    };
    let err = err_task.await.unwrap_or_default();
    // 录电台点了停止：aria2c 被请求退出时报「没下完」，不算出错
    if stop_by.is_some() {
        return Ok(Output { path: target, format_done: false });
    }
    if !st.success() {
        let mut out: Vec<String> = tail.into_iter().collect();
        out.extend(err.lines().map(str::to_string));
        // 报错进任务记录和错误原文之前，先把 cookie 的值抹掉
        return Err(crate::errors::classify_aria2(st.code(), &ctx.auth.redact(&out.join("\n")), url).into());
    }
    Ok(Output { path: target, format_done: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readout() {
        let r = parse_readout("[#2089b0 12MiB/100MiB(12%) CN:16 DL:4.2MiB ETA:18s]").unwrap();
        assert_eq!(r.conns, 16);
        assert_eq!(r.total, Some(100.0 * 1024.0 * 1024.0));
        let r = parse_readout("[#2089b0 400KiB CN:1 DL:0B]").unwrap();
        assert_eq!((r.got, r.total), (400.0 * 1024.0, None));
        assert!(parse_readout("09/25 12:00:00 [NOTICE] Download complete").is_none());
        // 电台流（不知道总大小）：没有百分比，总大小是 0B（2026-11-24 实测 aria2 1.37）
        let r = parse_readout("[#274b0c 31KiB/0B CN:1 DL:15KiB]").unwrap();
        assert_eq!((r.got, r.total, r.speed), (31.0 * 1024.0, None, 15.0 * 1024.0));
    }
    /// dropbox-8b4b13：16 路下载时有一路被回 403（aria2 的原文），换成一个连接重下
    #[test]
    fn refused_parallel_connections() {
        let out = "Exception: [AbstractCommand.cc:351] errorCode=22 URI=https://uc.dropboxusercontent.com/cd/0/get/x/file\n  -> [HttpSkipResponseCommand.cc:240] errorCode=22 The response status is not successful. status=403\n\naria2 will resume download if the transfer is restarted.";
        let f = crate::errors::classify_aria2(Some(22), out, "https://www.dropbox.com/scl/fi/r2kd2skcy5ylbbta5y1pz/DJI_0003.MP4?dl=1");
        assert!(parallel_refused(&f));
        let f = crate::errors::classify_aria2(Some(22), "errorCode=22 The response status is not successful. status=429", "https://upload.wikimedia.org/a.jpg");
        assert!(parallel_refused(&f));
        let f = crate::errors::classify_aria2(Some(3), "errorCode=3 Resource not found", "https://a.com/x.mp4");
        assert!(!parallel_refused(&f));
    }

    /// 本机起一个只回响应头的 HTTP 服务（每个连接回同一段），给 head() 测试用
    async fn serve_head(resp: &'static str) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut s, _)) = l.accept().await {
                let mut buf = [0u8; 4096];
                let _ = s.read(&mut buf).await;
                let _ = s.write_all(resp.as_bytes()).await;
                let _ = s.shutdown().await;
            }
        });
        format!("http://{addr}")
    }

    /// 对照组发现：HEAD 回了 Content-Length 的普通 MP3（soundhelix-448aa3）、OGG（wikimedia-433921）以前读不到大小，被当成了电台流
    #[tokio::test]
    async fn head_reads_declared_size() {
        let http = reqwest::Client::builder().no_proxy().build().unwrap();
        let jar = crate::login::Jar::default();
        let none = BTreeMap::new();
        let mp3 = serve_head("HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nContent-Length: 8945229\r\nAccept-Ranges: bytes\r\nConnection: close\r\n\r\n").await;
        let h = head(&http, &format!("{mp3}/SoundHelix-Song-1.mp3"), &none, &jar).await.unwrap();
        assert_eq!(h.size, Some(8_945_229));
        assert!(!h.is_stream(), "有大小的 MP3 不是电台流");
        // FIP：音频、有 icy 头、没有 Content-Length
        let radio = serve_head("HTTP/1.1 200 OK\r\nContent-Type: audio/mpeg\r\nicy-br: 128\r\nicy-name: fip-midfi.mp3\r\nConnection: close\r\n\r\n").await;
        let h = head(&http, &format!("{radio}/fip-midfi.mp3"), &none, &jar).await.unwrap();
        assert!(h.icy && h.size.is_none() && h.is_stream());
    }

    #[test]
    fn radio_and_html() {
        // radiofrance-a01fec、radioparadise-0c5a37：HEAD 回 audio/mpeg、icy-*，没有 Content-Length
        let h = Head { size: None, content_type: Some("audio/mpeg".into()), filename: "fip-midfi.mp3".into(), icy: true };
        assert!(h.is_stream() && !h.is_html());
        assert!(Head { size: None, content_type: Some("audio/aac".into()), ..Default::default() }.is_stream());
        // 普通的 mp3 文件（有大小、没有 icy）不算
        assert!(!Head { size: Some(3_000_000), content_type: Some("audio/mpeg".into()), ..Default::default() }.is_stream());
        assert!(!Head { size: None, content_type: Some("audio/x-mpegurl".into()), ..Default::default() }.is_stream());
        // dropbox-8b4b13：分享页是网页
        assert!(Head { size: Some(90_000), content_type: Some("text/html; charset=utf-8".into()), ..Default::default() }.is_html());
    }
    #[test]
    fn plain_request_headers() {
        let a = args(Path::new("/o"), "a.webm", 16, &BTreeMap::new(), None);
        assert!(a.contains(&"--no-want-digest-header=true".to_string()) && a.contains(&"--follow-metalink=false".to_string()));
        // 没带 User-Agent：用和读取信息时一样的
        assert!(a.contains(&format!("--user-agent={}", crate::net::UA)));
        // 带了（插件转来的请求头，大小写不论）：不再加，只发带来的那个
        let hs = BTreeMap::from([("user-agent".to_string(), "Test/1.0".to_string())]);
        let a = args(Path::new("/o"), "a.webm", 16, &hs, None);
        assert!(!a.iter().any(|x| x.starts_with("--user-agent=")), "{a:?}");
        assert!(a.contains(&"--header=user-agent: Test/1.0".to_string()));
    }

    #[test]
    fn names() {
        assert_eq!(filename_from("https://a.com/x/sintel_trailer-1080p.mp4?t=1", None, None), "sintel_trailer-1080p.mp4");
        assert_eq!(filename_from("https://a.com/get", Some("attachment; filename=\"clip.mov\""), None), "clip.mov");
        assert_eq!(filename_from("https://a.com/v/abc", None, Some("video/mp4")), "abc.mp4");
        assert_eq!(filename_from("https://a.com/%E8%A7%86%E9%A2%91.mp4", None, None), "视频.mp4");
    }
}
