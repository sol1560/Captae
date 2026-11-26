//! 三条下载路径：
//! - `ytdlp`：YouTube、B 站等有 extractor 的平台，以及未知网页（yt-dlp 的 generic 提取器）
//! - `aria2`：媒体直链，多线程分段 + 断点续传。没有 aria2c 时（手机上，看 `tools::Runner::has`）交给 `direct`（Rust 自己下，一个连接、Range 续传）
//! - `hls`：M3U8 流媒体。自己解析播放列表、并发拉分片、AES-128 解密，再交给 ffmpeg 封装成 MP4。
//!   思路来自猫抓的 m3u8 解析页，但代码是独立实现的（猫抓是 GPL-3.0，这里没有拷贝它的代码）。
//!   没有 ffmpeg 时（手机上）用 `remux` 自己封装（纯 Rust，只换封装不转码）。

pub mod aria2;
pub mod direct;
pub mod hls;
pub mod remux;
pub mod ytdlp;

use crate::config::{Paths, Settings};
use crate::queue::JobHandle;
use crate::tools::Toolbox;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 读取阶段拿到的元数据，入库时写进素材库。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MediaInfo {
    pub title: Option<String>,
    pub description: Option<String>,
    pub uploader: Option<String>,
    pub duration: Option<f64>,
    pub thumbnail: Option<String>,
    pub webpage_url: Option<String>,
    pub extractor: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub ext: Option<String>,
    pub filesize: Option<u64>,
    pub subtitles: Vec<String>,
    /// yt-dlp 的 live_status（was_live / post_live 这类直播回放，时长本来就不准，下完不拿它核对）
    #[serde(default)]
    pub live_status: Option<String>,
}

/// 引擎运行时需要的东西。
pub struct Ctx {
    pub tools: Toolbox,
    pub http: reqwest::Client,
    pub paths: Paths,
    pub settings: Settings,
    pub h: JobHandle,
    /// 这个任务这次要带的登录状态（没有就是空的），见 `login::Auth`
    pub auth: crate::login::Auth,
    /// 录直播时「停止并保存」：引擎收到以后让录制的进程收尾，录到的部分整理好照常返回（不是取消）
    pub finish: tokio_util::sync::CancellationToken,
}

impl Ctx {
    /// 这个任务是在录直播（读取时认出来是直播、而且要从现在开始录）
    pub fn live(&self) -> bool {
        self.h.snapshot().live
    }

    /// 记下录制的原始数据写在哪（文件或目录）。马上存进 queue.json：Captae 在录制中途退出时，下次打开据此整理
    pub fn note_raw(&self, raw: &Path) {
        let raw = raw.to_path_buf();
        self.h.update(|j| {
            if let Some(r) = j.rec.as_mut() {
                r.raw = Some(raw);
            }
        });
        self.h.persist();
    }

    /// 点了「停止并保存」：任务进入「正在保存录到的部分」
    pub fn note_stopping(&self) {
        self.h.update(|j| {
            j.phase = crate::model::Phase::Merge;
            j.merge = 0.0;
            j.speed = 0.0;
            if let Some(r) = j.rec.as_mut() {
                r.stopping = true;
                r.until.get_or_insert(crate::util::now_ms());
            }
        });
    }
}

/// 录直播时每秒看一次已经录了多少（`path` 是文件或目录，目录就把里面的文件加起来）。
/// yt-dlp 把直播交给 ffmpeg 录，录的过程中不报进度，只能看文件。丢掉返回值就不看了
pub(crate) struct SizeWatch(tokio::task::JoinHandle<()>);

impl Drop for SizeWatch {
    fn drop(&mut self) {
        self.0.abort();
    }
}

pub(crate) fn watch_size(h: JobHandle, path: PathBuf) -> SizeWatch {
    SizeWatch(tokio::spawn(async move {
        let mut meter = SpeedMeter::default();
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let n = size_under(&path) as f64;
            let sp = meter.push(n);
            h.update(|j| {
                if j.phase == crate::model::Phase::Dl {
                    j.got = crate::util::mb(n);
                    j.speed = crate::util::mb(sp);
                }
            });
        }
    }))
}

/// 文件的大小；目录就是里面所有文件加起来
pub(crate) fn size_under(p: &Path) -> u64 {
    match std::fs::metadata(p) {
        Ok(m) if m.is_dir() => std::fs::read_dir(p).map(|rd| rd.flatten().map(|e| size_under(&e.path())).sum()).unwrap_or(0),
        Ok(m) => m.len(),
        Err(_) => 0,
    }
}

/// 目录里最大的那个文件（录制被直接结束时，yt-dlp 的中间目录里写了一半的那个就是录到的部分）
pub(crate) fn largest_file(dir: &Path) -> Option<PathBuf> {
    let mut best: Option<(u64, PathBuf)> = None;
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            match e.metadata() {
                Ok(m) if m.is_dir() => stack.push(p),
                Ok(m) if m.len() > 0 && best.as_ref().is_none_or(|(n, _)| m.len() > *n) => best = Some((m.len(), p)),
                _ => {}
            }
        }
    }
    best.map(|(_, p)| p)
}

/// 引擎的产物：一个或多个落盘文件（yt-dlp 可能同时给出视频和字幕）。
#[derive(Debug, Clone)]
pub struct Output {
    pub path: PathBuf,
    /// 引擎已经按请求处理好了格式，不需要再转
    pub format_done: bool,
}

/// 把 "12.5MiB" / "340KiB" / "1.2GB" 转成字节。
pub(crate) fn parse_size(s: &str) -> Option<f64> {
    let s = s.trim();
    let idx = s.find(|c: char| c.is_ascii_alphabetic()).unwrap_or(s.len());
    let (num, unit) = s.split_at(idx);
    let n: f64 = num.trim().parse().ok()?;
    let mul = match unit.trim() {
        "" | "B" => 1.0,
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024f64.powi(4),
        "KB" | "kB" => 1e3,
        "MB" => 1e6,
        "GB" => 1e9,
        _ => return None,
    };
    Some(n * mul)
}

/// 速度平滑：指数移动平均，避免界面上的数字乱跳。
#[derive(Debug, Default)]
pub(crate) struct SpeedMeter {
    last_bytes: f64,
    last_t: Option<std::time::Instant>,
    pub ema: f64,
}

impl SpeedMeter {
    pub fn push(&mut self, bytes: f64) -> f64 {
        let now = std::time::Instant::now();
        if let Some(t) = self.last_t {
            let dt = now.duration_since(t).as_secs_f64();
            if dt >= 0.25 {
                let inst = ((bytes - self.last_bytes).max(0.0)) / dt;
                self.ema = if self.ema == 0.0 { inst } else { self.ema * 0.7 + inst * 0.3 };
                self.last_bytes = bytes;
                self.last_t = Some(now);
            }
        } else {
            self.last_bytes = bytes;
            self.last_t = Some(now);
        }
        self.ema
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sizes() {
        assert_eq!(parse_size("1KiB"), Some(1024.0));
        assert_eq!(parse_size("1.5MiB"), Some(1.5 * 1024.0 * 1024.0));
        assert_eq!(parse_size("0B"), Some(0.0));
        assert_eq!(parse_size("x"), None);
    }
}
