//! 直链的 Rust 下载器：手机上没有 aria2c，Runner 说没有 aria2c 时（`tools::Runner::has`）`aria2::download` 把直链交给这里。
//!
//! 一个连接，按 Range 续传。下到一半的文件直接写在保存位置，旁边放一个和 aria2 同名的标记文件 `<文件>.aria2`，
//! 队列里现成的规矩就都能接着用：同名文件旁边有标记 = 这个任务自己没下完、接着下；没有标记 = 已经有的文件、换个名字；
//! 取消、失败时连同标记一起删（`queue.rs` 的 `cleanup_partial`）。
//! 标记里存 JSON：链接、总大小、ETag / Last-Modified。续传时带 `If-Range`，服务器上的文件换过了就回整个文件，从头下。
//!
//! 以后（M1）：服务器支持时多路下载；iOS 的直链交给后台 URLSession（见 work/mobile/PLAN.md 5.4）。

use super::{Ctx, Output, SpeedMeter};
use crate::util::mb;
use anyhow::Result;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::io::AsyncWriteExt;

/// 网络断了最多接着试几次（和 aria2 的 `--max-tries=5` 一样），每次隔 2 秒。有进展就重新数
const TRIES: u32 = 5;
const RETRY_WAIT: Duration = Duration::from_secs(2);
/// 进度最多每 250 毫秒报一次（每次报都会推一个任务事件给界面）
const TICK: Duration = Duration::from_millis(250);

/// 标记文件里记的东西
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct Mark {
    url: String,
    size: Option<u64>,
    etag: Option<String>,
    modified: Option<String>,
}

fn mark_path(target: &Path) -> PathBuf {
    PathBuf::from(format!("{}.aria2", target.display()))
}

/// `Content-Range: bytes 100-199/1000` 里的总大小（`*` 表示服务器不知道）
fn total_of(content_range: &str) -> Option<u64> {
    content_range.rsplit('/').next().and_then(|s| s.trim().parse().ok())
}

/// 写文件出的错：磁盘满、没权限归成 `disk`，其他照原样
fn io_err(e: std::io::Error) -> anyhow::Error {
    match crate::errors::from_io(&e) {
        Some(f) => f.into(),
        None => e.into(),
    }
}

async fn save_mark(p: &Path, m: &Mark) -> Result<()> {
    tokio::fs::write(p, serde_json::to_vec(m)?).await.map_err(io_err)
}

/// 下载 `url` 到 `target`（文件名、换名字、任务上记的路径已经由 `aria2::download` 定好）
pub async fn run(ctx: &Ctx, url: &str, target: &Path) -> Result<Output> {
    if ctx.live() {
        // 电台流要 ffmpeg 整理成能拖动的文件，手机上还没有 ffmpeg（M3）
        anyhow::bail!("{}", crate::tr!("手机上还不能录电台直播"));
    }
    let req = ctx.h.snapshot().req;
    let jar = ctx.auth.jar();
    let mark_p = mark_path(target);
    // 接着下的前提：文件和标记都在、标记记的是同一个链接
    let old: Option<Mark> = match tokio::fs::read(&mark_p).await {
        Ok(b) if target.exists() => serde_json::from_slice(&b).ok().filter(|m: &Mark| m.url == url),
        _ => None,
    };
    let mut mark = match old {
        Some(m) => m,
        None => {
            // 从头下：旁边先放标记，表示「这是我没下完的」
            let m = Mark { url: url.to_string(), ..Default::default() };
            save_mark(&mark_p, &m).await?;
            tokio::fs::File::create(target).await.map_err(io_err)?;
            m
        }
    };
    let mut meter = SpeedMeter::default();
    let mut fails = 0u32;
    loop {
        let have = tokio::fs::metadata(target).await.map(|m| m.len()).unwrap_or(0);
        if mark.size.is_some_and(|t| t > 0 && have == t) {
            break;
        }
        let mut r = ctx.http.get(url);
        for (k, v) in &req.headers {
            r = r.header(k, v);
        }
        r = jar.apply(r, url, &req.headers);
        if have > 0 {
            r = r.header(reqwest::header::RANGE, format!("bytes={have}-"));
            if let Some(v) = mark.etag.as_ref().or(mark.modified.as_ref()) {
                r = r.header(reqwest::header::IF_RANGE, v);
            }
        }
        let resp = match r.send().await {
            Ok(r) => r,
            Err(e) => {
                let f = crate::errors::from_reqwest(&e, url);
                fails += 1;
                if !matches!(f.code, crate::errors::ErrorCode::Network) || fails > TRIES {
                    return Err(f.into());
                }
                tokio::time::sleep(RETRY_WAIT).await;
                continue;
            }
        };
        let status = resp.status().as_u16();
        let hv = |k: reqwest::header::HeaderName| resp.headers().get(k).and_then(|v| v.to_str().ok()).map(str::to_string);
        // 请求的范围超出文件：手上的已经是整个文件
        if status == 416 && have > 0 {
            let total = hv(reqwest::header::CONTENT_RANGE).as_deref().and_then(total_of);
            if total.is_none_or(|t| t == have) {
                mark.size = Some(have);
                break;
            }
        }
        if !resp.status().is_success() {
            return Err(crate::errors::http_status(status, url).into());
        }
        let resumed = status == 206 && have > 0;
        let (mut file, mut got) = if resumed {
            ctx.h.update(|j| j.log(crate::tr!("从 {size} MB 处接着下", size = crate::i18n::decimal(mb(have as f64), 1))));
            let f = tokio::fs::OpenOptions::new().append(true).open(target).await.map_err(io_err)?;
            if mark.size.is_none() {
                mark.size = hv(reqwest::header::CONTENT_RANGE).as_deref().and_then(total_of);
            }
            (f, have)
        } else {
            // 回的是整个文件（第一次下，或者服务器不认 Range / 文件换过了）：从头写，记下大小和校验值
            mark.size = resp.content_length().filter(|n| *n > 0);
            mark.etag = hv(reqwest::header::ETAG).filter(|e| !e.starts_with("W/"));
            mark.modified = hv(reqwest::header::LAST_MODIFIED);
            save_mark(&mark_p, &mark).await?;
            (tokio::fs::File::create(target).await.map_err(io_err)?, 0)
        };
        if let Some(t) = mark.size {
            ctx.h.update(|j| j.size = mb(t as f64));
        }
        let start = got;
        let mut last = Instant::now() - TICK;
        let mut stream = resp.bytes_stream();
        let mut broke: Option<reqwest::Error> = None;
        while let Some(chunk) = stream.next().await {
            match chunk {
                Ok(b) => {
                    file.write_all(&b).await.map_err(io_err)?;
                    got += b.len() as u64;
                    if last.elapsed() >= TICK {
                        last = Instant::now();
                        let sp = meter.push(got as f64);
                        ctx.h.update(|j| {
                            j.got = mb(got as f64);
                            j.speed = mb(sp);
                        });
                    }
                }
                Err(e) => {
                    broke = Some(e);
                    break;
                }
            }
        }
        file.flush().await.map_err(io_err)?;
        drop(file);
        let short = mark.size.is_some_and(|t| got < t);
        if broke.is_none() && !short {
            mark.size = Some(got);
            break;
        }
        // 下到一半断了：有进展就重新数次数，隔一会儿从断的地方接着下
        if got > start {
            fails = 0;
        }
        fails += 1;
        if fails > TRIES {
            return Err(match broke {
                Some(e) => crate::errors::from_reqwest(&e, url).into(),
                None => anyhow::anyhow!("{}", crate::tr!("服务器没把文件发完")),
            });
        }
        tokio::time::sleep(RETRY_WAIT).await;
    }
    let total = mark.size.unwrap_or(0) as f64;
    ctx.h.update(|j| {
        j.got = mb(total);
        j.size = mb(total);
        j.speed = 0.0;
    });
    // 下完了：去掉标记，这个文件就是成品
    let _ = tokio::fs::remove_file(&mark_p).await;
    Ok(Output { path: target.to_path_buf(), format_done: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_range_total() {
        assert_eq!(total_of("bytes 100-199/1000"), Some(1000));
        assert_eq!(total_of("bytes */4096"), Some(4096));
        assert_eq!(total_of("bytes 0-99/*"), None);
        assert_eq!(mark_path(Path::new("/a/b.mp4")), PathBuf::from("/a/b.mp4.aria2"));
    }
}
