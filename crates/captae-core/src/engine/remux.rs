//! 不用 ffmpeg，把 M3U8 下好的分片封装成 MP4（手机上用；桌面端照旧用 ffmpeg，见 `tools::Runner::has`）。
//!
//! - MPEG-TS（H.264 / HEVC 画面、ADTS 封装的 AAC 声音）：拆出每一帧。画面从 Annex B（起始码）改成带长度的格式，
//!   参数集（SPS / PPS / VPS）放进 avcC / hvcC；声音去掉 ADTS 头、生成 AudioSpecificConfig（和 ffmpeg 的 `aac_adtstoasc` 一样）
//! - fMP4（带 EXT-X-MAP 初始化分片的）：读 moov 和每个 moof 里的样本表，样本原样搬过去，sample entry（avc1 / mp4a……）原样用
//! - 画面和单独一路声音（EXT-X-MEDIA）合进同一个 MP4，两路各自的起点用 edit list 对齐（ffmpeg `-c copy` 也是这样做的）
//!
//! 输出是普通（不分片）的 MP4，moov 在前（和 `-movflags +faststart` 一样），不重新编码。
//! 只认上面这几种编码，别的（MP3、AC-3、E-AC-3……）报错说清楚。
//! 全部是同步读写，调用方用 `remux_async`（放进 `spawn_blocking`，被丢掉时中途停下、删掉没写完的文件）。
//! 拆出来的画面、声音先写进工作目录里的临时文件（不全放内存：手机上一部电影的分片有几个 GB），最后按顺序抄进 MP4。

use crate::tr;
use anyhow::{anyhow, bail, Context, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

const TS: usize = 188;
/// ADTS 的采样率表（sampling_frequency_index）
const AAC_RATES: [u32; 13] = [96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350];

/// 一帧（一个 MP4 样本）。时间都按所在轨道的 timescale
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Sample {
    /// 解码时间（TS 是 90 kHz 的 DTS，解开了 33 位回绕）
    dts: i64,
    dur: u32,
    /// 显示时间 - 解码时间（有 B 帧时不是 0）
    cto: i32,
    size: u32,
    sync: bool,
    /// 数据在 `Track::data` 文件里的位置
    off: u64,
}

#[derive(Debug)]
struct Track {
    video: bool,
    timescale: u32,
    /// 完整的 sample entry box（avc1 / hvc1 / mp4a，或者 fMP4 初始化分片里原样拿来的）
    entry: Vec<u8>,
    width: u32,
    height: u32,
    samples: Vec<Sample>,
    data: PathBuf,
    /// fMP4 初始化分片里原有的 edit list：(开头空着几秒, 媒体时间轴上从哪开始显示)。
    /// 空着的一段是各路之间的起点差，media_time 是 B 帧的显示延迟、AAC 编码器的预读样本这类。TS 没有，按第一帧的显示时间算
    edit: Option<(f64, i64)>,
}

impl Track {
    fn media_dur(&self) -> u64 {
        self.samples.iter().map(|s| s.dur as u64).sum()
    }
    /// 最早显示的那一帧的显示时间
    fn first_pts(&self) -> i64 {
        self.samples.iter().map(|s| s.dts + s.cto as i64).min().unwrap_or(0)
    }
    fn first_dts(&self) -> i64 {
        self.samples.first().map(|s| s.dts).unwrap_or(0)
    }
    /// 写出去的 edit list 的 media_time：从这一路的第一帧（解码时间）起跳过多少才开始显示
    /// （有 B 帧时是第一帧的显示偏移；fMP4 照原来的 edit list）
    fn media_time(&self) -> i64 {
        match self.edit {
            Some((_, mt)) => mt - self.first_dts(),
            None => self.first_pts() - self.first_dts(),
        }
        .max(0)
    }
    /// 这一路从什么时候开始显示（秒）。TS 各路共用一个时钟（PTS）；fMP4 照原来 edit list 开头空着的那一段
    fn start_secs(&self) -> f64 {
        match self.edit {
            Some((empty, _)) => empty,
            None => self.first_pts() as f64 / self.timescale as f64,
        }
    }
}

/// 封装完的结果
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Remuxed {
    pub video: bool,
    pub audio: bool,
    /// 时长（秒）
    pub secs: f64,
}

// ------------------------------------------------------------------ 入口

/// 把 `video`（TS 或 fMP4，可能也带着声音）和可选的单独一路声音 `audio` 封装成 `out`。
/// `work` 放拆出来的临时文件，做完删掉；`on` 报进度（0..1）；`stop` 变成 true 时中途停下
fn remux_with(video: &Path, audio: Option<&Path>, out: &Path, work: &Path, on: &mut dyn FnMut(f64), stop: &AtomicBool) -> Result<Remuxed> {
    std::fs::create_dir_all(work)?;
    let r = (|| {
        // 有单独一路声音时，画面那个文件里只要画面（和 ffmpeg 的 `-map 0:v:0 -map 1:a:0` 一样）
        let mut tracks = load(video, work, "v", true, audio.is_none())?;
        if let Some(a) = audio {
            tracks.extend(load(a, work, "a", false, true)?);
        }
        if tracks.is_empty() {
            bail!(tr!("下载到的分片里没有画面也没有声音"));
        }
        write_mp4(&tracks, out, on, stop)
    })();
    let _ = std::fs::remove_dir_all(work);
    if r.is_err() {
        let _ = std::fs::remove_file(out);
    }
    r
}

/// 同步版（测试、命令行用）
pub fn remux(video: &Path, audio: Option<&Path>, out: &Path, work: &Path, mut on: impl FnMut(f64)) -> Result<Remuxed> {
    remux_with(video, audio, out, work, &mut on, &AtomicBool::new(false))
}

/// 放到后台线程里封装。这个 future 被丢掉（任务暂停、取消）时让后台线程停下、删掉没写完的 `out`
pub async fn remux_async(video: PathBuf, audio: Option<PathBuf>, out: PathBuf, work: PathBuf, mut on: impl FnMut(f64) + Send + 'static) -> Result<Remuxed> {
    struct StopOnDrop(Arc<AtomicBool>);
    impl Drop for StopOnDrop {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }
    let stop = Arc::new(AtomicBool::new(false));
    let _guard = StopOnDrop(stop.clone());
    tokio::task::spawn_blocking(move || remux_with(&video, audio.as_deref(), &out, &work, &mut on, &stop)).await.map_err(|e| anyhow!("remux: {e}"))?
}

/// 录直播录到的原始数据（M3U8 引擎写的 video.ts / audio.ts，或者 fMP4）整理成 `dir` 下能播放的文件：
/// 有画面的是 `<stem>.mp4`，只有声音的是 `<stem>.m4a`。手机上 `ffmpeg::tidy_recording` 交给这里
pub async fn tidy_recording(raw: &Path, audio: Option<&Path>, dir: &Path, stem: &str) -> Result<PathBuf> {
    tokio::fs::create_dir_all(dir).await?;
    let stem = crate::util::safe_name(stem);
    let stem = if stem.trim().is_empty() { tr!("录制§录直播得到的文件没有标题时的文件名") } else { stem };
    let out = crate::util::unique_path(dir.join(format!("{stem}.mp4")));
    let work = dir.join(".captae-tmp").join(format!("remux-{}", &crate::util::url_key(&out.display().to_string())[..12]));
    let r = remux_async(raw.to_path_buf(), audio.map(Path::to_path_buf), out.clone(), work, |_| {}).await?;
    let _ = tokio::fs::remove_dir(dir.join(".captae-tmp")).await;
    if r.video {
        return Ok(out);
    }
    let m4a = crate::util::unique_path(dir.join(format!("{stem}.m4a")));
    tokio::fs::rename(&out, &m4a).await?;
    Ok(m4a)
}

enum Input {
    Ts,
    Mp4,
}

fn read_full(r: &mut impl Read, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        match r.read(&mut buf[n..]) {
            Ok(0) => break,
            Ok(k) => n += k,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
    Ok(n)
}

fn sniff(p: &Path) -> Result<Input> {
    let mut b = [0u8; TS + 1];
    let n = read_full(&mut File::open(p).with_context(|| format!("open {}", p.display()))?, &mut b)?;
    if n > 0 && b[0] == 0x47 && (n <= TS || b[TS] == 0x47) {
        return Ok(Input::Ts);
    }
    if n >= 8 && matches!(&b[4..8], b"ftyp" | b"styp" | b"moov" | b"moof" | b"sidx") {
        return Ok(Input::Mp4);
    }
    bail!(tr!("下载到的分片不是 MPEG-TS 也不是 MP4，封装不了"))
}

fn load(p: &Path, work: &Path, tag: &str, want_video: bool, want_audio: bool) -> Result<Vec<Track>> {
    match sniff(p)? {
        Input::Ts => demux_ts(p, work, tag, want_video, want_audio),
        Input::Mp4 => demux_fmp4(p, want_video, want_audio),
    }
}

/// 33 位的 PTS / DTS 回绕：取离上一个值最近的那一圈
fn unwrap_ts(prev: Option<i64>, raw: i64) -> i64 {
    const W: i64 = 1 << 33;
    match prev {
        None => raw,
        Some(p) => raw + (p - raw + W / 2).div_euclid(W) * W,
    }
}

// ------------------------------------------------------------------ MPEG-TS

/// PES 包头里的 PTS / DTS（5 字节，33 位）
fn pes_ts(b: &[u8]) -> i64 {
    (((b[0] as i64 >> 1) & 7) << 30) | ((b[1] as i64) << 22) | ((b[2] as i64 >> 1) << 15) | ((b[3] as i64) << 7) | (b[4] as i64 >> 1)
}

/// 一个完整的 PES → (PTS, DTS, 负载)
fn parse_pes(b: &[u8]) -> Option<(Option<i64>, Option<i64>, &[u8])> {
    if b.len() < 9 || b[0..3] != [0, 0, 1] {
        return None;
    }
    let len = u16::from_be_bytes([b[4], b[5]]) as usize;
    let end = if len == 0 { b.len() } else { (6 + len).min(b.len()) };
    let flags = b[7] >> 6;
    let hl = 9 + b[8] as usize;
    if hl > end {
        return None;
    }
    let pts = (flags & 2 != 0 && b.len() >= 14).then(|| pes_ts(&b[9..14]));
    let dts = (flags == 3 && b.len() >= 19).then(|| pes_ts(&b[14..19]));
    Some((pts, dts, &b[hl..end]))
}

/// PAT → 第一个节目的 PMT 的 PID
fn parse_pat(pl: &[u8]) -> Option<u16> {
    let s = pl.get(1 + *pl.first()? as usize..)?;
    if s.len() < 8 || s[0] != 0 {
        return None;
    }
    let len = (((s[1] & 0x0F) as usize) << 8) | s[2] as usize;
    let end = (3 + len).saturating_sub(4).min(s.len());
    let mut j = 8;
    while j + 4 <= end {
        let prog = u16::from_be_bytes([s[j], s[j + 1]]);
        if prog != 0 {
            return Some((((s[j + 2] & 0x1F) as u16) << 8) | s[j + 3] as u16);
        }
        j += 4;
    }
    None
}

/// PMT 里的各路：(stream_type, PID)
fn parse_pmt(pl: &[u8]) -> Vec<(u8, u16)> {
    let mut out = vec![];
    let Some(s) = pl.first().and_then(|p| pl.get(1 + *p as usize..)) else { return out };
    if s.len() < 12 || s[0] != 2 {
        return out;
    }
    let len = (((s[1] & 0x0F) as usize) << 8) | s[2] as usize;
    let end = (3 + len).saturating_sub(4).min(s.len());
    let pil = (((s[10] & 0x0F) as usize) << 8) | s[11] as usize;
    let mut j = 12 + pil;
    while j + 5 <= end {
        let st = s[j];
        let pid = (((s[j + 1] & 0x1F) as u16) << 8) | s[j + 2] as u16;
        let eil = (((s[j + 3] & 0x0F) as usize) << 8) | s[j + 4] as usize;
        out.push((st, pid));
        j += 5 + eil;
    }
    out
}

/// stream_type 的名字（报「封装不了」时说是什么编码）
fn stream_name(st: u8) -> String {
    match st {
        0x03 | 0x04 => "MP3".into(),
        0x11 => "AAC (LATM)".into(),
        0x81 => "AC-3".into(),
        0x87 => "E-AC-3".into(),
        0x02 => "MPEG-2".into(),
        0x10 => "MPEG-4 Part 2".into(),
        0x33 => "VVC".into(),
        x => format!("stream_type 0x{x:02x}"),
    }
}

/// Annex B 的字节流按起始码（00 00 01 / 00 00 00 01）切成一个个 NAL
fn annexb(d: &[u8]) -> Vec<&[u8]> {
    let mut out = vec![];
    let mut start: Option<usize> = None;
    let mut i = 0;
    while i + 2 < d.len() {
        if d[i] == 0 && d[i + 1] == 0 && d[i + 2] == 1 {
            if let Some(s) = start {
                let mut e = i;
                while e > s && d[e - 1] == 0 {
                    e -= 1;
                }
                out.push(&d[s..e]);
            }
            i += 3;
            start = Some(i);
            continue;
        }
        i += 1;
    }
    if let Some(s) = start {
        let mut e = d.len();
        while e > s && d[e - 1] == 0 {
            e -= 1;
        }
        out.push(&d[s..e]);
    }
    out.retain(|n| !n.is_empty());
    out
}

/// 拆出来的数据先写进临时文件
struct Es {
    path: PathBuf,
    out: BufWriter<File>,
    off: u64,
}

impl Es {
    fn new(path: PathBuf) -> Result<Es> {
        let out = BufWriter::with_capacity(1 << 20, File::create(&path)?);
        Ok(Es { path, out, off: 0 })
    }
    fn write(&mut self, b: &[u8]) -> Result<()> {
        self.out.write_all(b)?;
        self.off += b.len() as u64;
        Ok(())
    }
}

struct VideoSink {
    hevc: bool,
    es: Es,
    samples: Vec<Sample>,
    last: Option<i64>,
    /// 第一次见到的参数集：H.264 是 [SPS, PPS]，HEVC 是 [VPS, SPS, PPS]
    ps: [Option<Vec<u8>>; 3],
}

impl VideoSink {
    fn push(&mut self, pts: Option<i64>, dts: Option<i64>, data: &[u8]) -> Result<()> {
        let start = self.es.off;
        let mut sync = false;
        for nal in annexb(data) {
            let slot = if self.hevc {
                match (nal[0] >> 1) & 0x3F {
                    // AUD、填充：MP4 里不要
                    35 | 38 => continue,
                    32 => Some(0),
                    33 => Some(1),
                    34 => Some(2),
                    16..=21 => {
                        sync = true;
                        None
                    }
                    _ => None,
                }
            } else {
                match nal[0] & 0x1F {
                    9 | 12 => continue,
                    7 => Some(0),
                    8 => Some(1),
                    5 => {
                        sync = true;
                        None
                    }
                    _ => None,
                }
            };
            if let Some(k) = slot {
                // 参数集放进 avcC / hvcC；和第一次的一样就不再留在帧里，不一样的（换了分辨率）留在帧里，解码器照样认
                match &self.ps[k] {
                    None => {
                        self.ps[k] = Some(nal.to_vec());
                        continue;
                    }
                    Some(p) if p.as_slice() == nal => continue,
                    _ => {}
                }
            }
            self.es.write(&(nal.len() as u32).to_be_bytes())?;
            self.es.write(nal)?;
        }
        let size = (self.es.off - start) as u32;
        if size == 0 {
            return Ok(());
        }
        match pts {
            Some(p) => {
                let p = unwrap_ts(self.last, p);
                let d = dts.map(|d| unwrap_ts(Some(p), d)).unwrap_or(p);
                self.last = Some(d);
                self.samples.push(Sample { dts: d, dur: 0, cto: (p - d) as i32, size, sync, off: start });
            }
            // 没带时间戳的 PES：是上一帧的后半截
            None => {
                if let Some(s) = self.samples.last_mut() {
                    s.size += size;
                    s.sync |= sync;
                }
            }
        }
        Ok(())
    }

    fn finish(mut self) -> Result<Option<Track>> {
        self.es.out.flush()?;
        if self.samples.is_empty() {
            return Ok(None);
        }
        fill_durations(&mut self.samples, 3000);
        let (entry, w, h) = if self.hevc {
            let [Some(vps), Some(sps), Some(pps)] = &self.ps else { bail!(tr!("画面里没有找到参数集（VPS / SPS / PPS），封装不了")) };
            let info = hevc_sps(sps).ok_or_else(|| anyhow!(tr!("画面的参数集（SPS）读不懂，封装不了")))?;
            let cfg = hvcc(&info, vps, sps, pps);
            (visual_entry(b"hvc1", info.width, info.height, &cfg), info.width, info.height)
        } else {
            let [Some(sps), Some(pps), _] = &self.ps else { bail!(tr!("画面里没有找到参数集（SPS / PPS），封装不了")) };
            let info = avc_sps(sps).ok_or_else(|| anyhow!(tr!("画面的参数集（SPS）读不懂，封装不了")))?;
            let cfg = avcc(&info, sps, pps);
            (visual_entry(b"avc1", info.width, info.height, &cfg), info.width, info.height)
        };
        Ok(Some(Track { video: true, timescale: 90_000, entry, width: w, height: h, samples: self.samples, data: self.es.path, edit: None }))
    }
}

struct AudioSink {
    es: Es,
    samples: Vec<Sample>,
    /// 这个 PES 的 PTS（90 kHz，解开了回绕）
    pts: Option<i64>,
    last: Option<i64>,
    /// 下一帧的时间（按采样率）
    next: Option<i64>,
    rate: u32,
    chans: u16,
    asc: Option<[u8; 2]>,
    /// 跨 PES 的半帧
    carry: Vec<u8>,
}

impl AudioSink {
    fn push(&mut self, pts: Option<i64>, data: &[u8]) -> Result<()> {
        if let Some(p) = pts {
            let p = unwrap_ts(self.last, p);
            self.last = Some(p);
            self.pts = Some(p);
            // 中间断了一截（不连续）：时间跟着 PTS 跳过去；平时按帧数一帧帧往后排，不受 PTS 的抖动影响
            if let (Some(n), true, true) = (self.next, self.carry.is_empty(), self.rate > 0) {
                let at = p * self.rate as i64 / 90_000;
                if (at - n).abs() > self.rate as i64 / 2 {
                    self.next = Some(at);
                }
            }
        }
        let mut buf = std::mem::take(&mut self.carry);
        buf.extend_from_slice(data);
        let mut i = 0;
        while i + 7 <= buf.len() {
            let h = &buf[i..];
            // 同步字 0xFFF、layer 00
            if !(h[0] == 0xFF && h[1] & 0xF6 == 0xF0) {
                i += 1;
                continue;
            }
            let crc = h[1] & 1 == 0;
            let profile = h[2] >> 6;
            let sfi = (h[2] >> 2) & 0x0F;
            let ch = ((h[2] & 1) << 2) | (h[3] >> 6);
            let flen = (((h[3] & 3) as usize) << 11) | ((h[4] as usize) << 3) | ((h[5] as usize) >> 5);
            let blocks = (h[6] & 3) as u32 + 1;
            let hl = if crc { 9 } else { 7 };
            if sfi as usize >= AAC_RATES.len() || flen <= hl {
                i += 1;
                continue;
            }
            if i + flen > buf.len() {
                break;
            }
            if self.asc.is_none() {
                self.rate = AAC_RATES[sfi as usize];
                self.chans = if ch == 0 { 2 } else if ch == 7 { 8 } else { ch as u16 };
                // AudioSpecificConfig：audioObjectType（5 位）= ADTS 的 profile + 1，采样率序号（4 位），声道（4 位）
                self.asc = Some([((profile + 1) << 3) | (sfi >> 1), ((sfi & 1) << 7) | (ch << 3)]);
            }
            let next = *self.next.get_or_insert_with(|| self.pts.unwrap_or(0) * self.rate as i64 / 90_000);
            let start = self.es.off;
            self.es.write(&buf[i + hl..i + flen])?;
            let dur = 1024 * blocks;
            self.samples.push(Sample { dts: next, dur, cto: 0, size: (flen - hl) as u32, sync: true, off: start });
            self.next = Some(next + dur as i64);
            i += flen;
        }
        self.carry = buf[i..].to_vec();
        Ok(())
    }

    fn finish(mut self) -> Result<Option<Track>> {
        self.es.out.flush()?;
        let Some(asc) = self.asc.filter(|_| !self.samples.is_empty()) else { return Ok(None) };
        let entry = audio_entry(self.chans, self.rate, &asc);
        Ok(Some(Track { video: false, timescale: self.rate, entry, width: 0, height: 0, samples: self.samples, data: self.es.path, edit: None }))
    }
}

/// 每帧的时长 = 下一帧的解码时间 - 这一帧的；最后一帧照前一帧
fn fill_durations(s: &mut [Sample], fallback: u32) {
    for i in 0..s.len() {
        if i + 1 < s.len() {
            s[i].dur = (s[i + 1].dts - s[i].dts).clamp(1, u32::MAX as i64) as u32;
        } else {
            s[i].dur = if i > 0 { s[i - 1].dur } else { fallback };
        }
    }
}

fn demux_ts(p: &Path, work: &Path, tag: &str, want_video: bool, want_audio: bool) -> Result<Vec<Track>> {
    let mut r = BufReader::with_capacity(1 << 20, File::open(p)?);
    let mut pkt = [0u8; TS];
    let mut pmt: Option<u16> = None;
    let mut streams: Option<Vec<(u8, u16)>> = None;
    let mut vpid: Option<u16> = None;
    let mut apid: Option<u16> = None;
    let mut vsink: Option<VideoSink> = None;
    let mut asink: Option<AudioSink> = None;
    let mut bufs: HashMap<u16, Vec<u8>> = HashMap::new();

    let flush = |pid: u16, pes: Vec<u8>, v: &mut Option<VideoSink>, a: &mut Option<AudioSink>, vpid: Option<u16>| -> Result<()> {
        let Some((pts, dts, data)) = parse_pes(&pes) else { return Ok(()) };
        if Some(pid) == vpid {
            if let Some(v) = v.as_mut() {
                v.push(pts, dts, data)?;
            }
        } else if let Some(a) = a.as_mut() {
            a.push(pts, data)?;
        }
        Ok(())
    };

    loop {
        if read_full(&mut r, &mut pkt)? < TS {
            break;
        }
        // 丢了同步：找到下一个 0x47 接着读
        if pkt[0] != 0x47 {
            let Some(i) = pkt.iter().position(|&b| b == 0x47) else { continue };
            pkt.copy_within(i.., 0);
            if read_full(&mut r, &mut pkt[TS - i..])? < i {
                break;
            }
        }
        if pkt[1] & 0x80 != 0 {
            continue;
        }
        let pusi = pkt[1] & 0x40 != 0;
        let pid = (((pkt[1] & 0x1F) as u16) << 8) | pkt[2] as u16;
        let afc = (pkt[3] >> 4) & 3;
        if afc & 1 == 0 {
            continue;
        }
        let at = if afc & 2 != 0 { 5 + pkt[4] as usize } else { 4 };
        if at >= TS {
            continue;
        }
        let pl = &pkt[at..];
        if pid == 0 {
            if pusi && pmt.is_none() {
                pmt = parse_pat(pl);
            }
            continue;
        }
        if Some(pid) == pmt {
            if pusi && streams.is_none() {
                let s = parse_pmt(pl);
                if want_video {
                    if let Some(&(st, id)) = s.iter().find(|(st, _)| matches!(st, 0x1B | 0x24)) {
                        vpid = Some(id);
                        vsink = Some(VideoSink { hevc: st == 0x24, es: Es::new(work.join(format!("{tag}-video.es")))?, samples: vec![], last: None, ps: [None, None, None] });
                    }
                }
                if want_audio {
                    if let Some(&(_, id)) = s.iter().find(|(st, _)| *st == 0x0F) {
                        apid = Some(id);
                        asink = Some(AudioSink { es: Es::new(work.join(format!("{tag}-audio.es")))?, samples: vec![], pts: None, last: None, next: None, rate: 0, chans: 2, asc: None, carry: vec![] });
                    }
                }
                streams = Some(s);
            }
            continue;
        }
        if Some(pid) != vpid && Some(pid) != apid {
            continue;
        }
        let b = bufs.entry(pid).or_default();
        if pusi {
            if !b.is_empty() {
                let pes = std::mem::take(b);
                flush(pid, pes, &mut vsink, &mut asink, vpid)?;
            }
        } else if b.is_empty() {
            // 开头不在文件里的半个 PES：丢掉
            continue;
        }
        bufs.entry(pid).or_default().extend_from_slice(pl);
    }
    for (pid, pes) in std::mem::take(&mut bufs) {
        if !pes.is_empty() {
            flush(pid, pes, &mut vsink, &mut asink, vpid)?;
        }
    }
    let Some(streams) = streams else { bail!(tr!("下载到的分片里没有节目表（PAT / PMT），封装不了")) };
    let mut out = vec![];
    if let Some(v) = vsink {
        out.extend(v.finish()?);
    }
    if let Some(a) = asink {
        out.extend(a.finish()?);
    }
    if out.is_empty() {
        // 有画面 / 声音，只是编码不认得
        let names: Vec<String> = streams.iter().filter(|(st, _)| !matches!(st, 0x15 | 0x86 | 0x06 | 0x0F | 0x1B | 0x24)).map(|(st, _)| stream_name(*st)).collect();
        if !names.is_empty() {
            bail!(tr!("这种编码在手机上还封装不了：{codec}", codec = crate::i18n::join(&names)));
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ 参数集

/// 读位（去掉了防竞争字节的 RBSP）
struct Bits<'a> {
    d: &'a [u8],
    p: usize,
}

impl Bits<'_> {
    fn u(&mut self, n: usize) -> Option<u32> {
        let mut v = 0u32;
        for _ in 0..n {
            let byte = *self.d.get(self.p / 8)?;
            v = (v << 1) | ((byte >> (7 - self.p % 8)) & 1) as u32;
            self.p += 1;
        }
        Some(v)
    }
    fn skip(&mut self, n: usize) -> Option<()> {
        self.p += n;
        (self.p <= self.d.len() * 8).then_some(())
    }
    fn ue(&mut self) -> Option<u32> {
        let mut zeros = 0;
        while self.u(1)? == 0 {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        Some((1u32 << zeros) - 1 + self.u(zeros)?)
    }
    fn se(&mut self) -> Option<i32> {
        let k = self.ue()? as i64;
        Some(if k % 2 == 1 { ((k + 1) / 2) as i32 } else { -(k / 2) as i32 })
    }
}

/// 去掉防竞争字节（00 00 03 里的 03）
fn rbsp(nal: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(nal.len());
    let mut zeros = 0;
    for &b in nal {
        if zeros >= 2 && b == 3 {
            zeros = 0;
            continue;
        }
        zeros = if b == 0 { zeros + 1 } else { 0 };
        out.push(b);
    }
    out
}

#[derive(Debug, Clone, Default, PartialEq)]
struct SpsInfo {
    width: u32,
    height: u32,
    chroma: u32,
    depth_luma: u32,
    depth_chroma: u32,
    /// HEVC：profile_tier_level 的前 12 字节，最多几层子层、是否嵌套
    ptl: Vec<u8>,
    sub_layers: u32,
    nested: u32,
}

fn crop_units(chroma: u32) -> (u32, u32) {
    match chroma {
        1 => (2, 2),
        2 => (2, 1),
        _ => (1, 1),
    }
}

/// H.264 的 SPS（含 1 字节 NAL 头）
fn avc_sps(nal: &[u8]) -> Option<SpsInfo> {
    let d = rbsp(nal.get(1..)?);
    let mut b = Bits { d: &d, p: 0 };
    let profile = b.u(8)?;
    b.skip(16)?;
    b.ue()?;
    let (mut chroma, mut dl, mut dc) = (1, 0, 0);
    if matches!(profile, 100 | 110 | 122 | 244 | 44 | 83 | 86 | 118 | 128 | 138 | 139 | 134 | 135) {
        chroma = b.ue()?;
        if chroma == 3 {
            b.skip(1)?;
        }
        dl = b.ue()?;
        dc = b.ue()?;
        b.skip(1)?;
        if b.u(1)? == 1 {
            for i in 0..if chroma == 3 { 12 } else { 8 } {
                if b.u(1)? == 1 {
                    let n = if i < 6 { 16 } else { 64 };
                    let (mut last, mut next) = (8i32, 8i32);
                    for _ in 0..n {
                        if next != 0 {
                            next = (last + b.se()? + 256) % 256;
                        }
                        last = if next == 0 { last } else { next };
                    }
                }
            }
        }
    }
    b.ue()?;
    match b.ue()? {
        0 => {
            b.ue()?;
        }
        1 => {
            b.skip(1)?;
            b.se()?;
            b.se()?;
            for _ in 0..b.ue()? {
                b.se()?;
            }
        }
        _ => {}
    }
    b.ue()?;
    b.skip(1)?;
    let wmb = b.ue()? + 1;
    let hmu = b.ue()? + 1;
    let frame_mbs_only = b.u(1)?;
    if frame_mbs_only == 0 {
        b.skip(1)?;
    }
    b.skip(1)?;
    let mut w = wmb * 16;
    let mut h = (2 - frame_mbs_only) * hmu * 16;
    if b.u(1)? == 1 {
        let (l, r, t, bo) = (b.ue()?, b.ue()?, b.ue()?, b.ue()?);
        let (cx, cy) = if chroma == 0 { (1, 2 - frame_mbs_only) } else { let (x, y) = crop_units(chroma); (x, y * (2 - frame_mbs_only)) };
        w = w.saturating_sub(cx * (l + r));
        h = h.saturating_sub(cy * (t + bo));
    }
    Some(SpsInfo { width: w, height: h, chroma, depth_luma: dl, depth_chroma: dc, ..Default::default() })
}

/// HEVC 的 SPS（含 2 字节 NAL 头）
fn hevc_sps(nal: &[u8]) -> Option<SpsInfo> {
    let d = rbsp(nal.get(2..)?);
    if d.len() < 13 {
        return None;
    }
    let mut b = Bits { d: &d, p: 0 };
    b.skip(4)?;
    let msl = b.u(3)?;
    let nested = b.u(1)?;
    let ptl = d[1..13].to_vec();
    b.skip(96)?;
    let mut pp = vec![];
    for _ in 0..msl {
        pp.push((b.u(1)?, b.u(1)?));
    }
    if msl > 0 {
        b.skip(2 * (8 - msl as usize))?;
    }
    for (p, l) in pp {
        if p == 1 {
            b.skip(88)?;
        }
        if l == 1 {
            b.skip(8)?;
        }
    }
    b.ue()?;
    let chroma = b.ue()?;
    if chroma == 3 {
        b.skip(1)?;
    }
    let mut w = b.ue()?;
    let mut h = b.ue()?;
    if b.u(1)? == 1 {
        let (l, r, t, bo) = (b.ue()?, b.ue()?, b.ue()?, b.ue()?);
        let (cx, cy) = crop_units(chroma);
        w = w.saturating_sub(cx * (l + r));
        h = h.saturating_sub(cy * (t + bo));
    }
    let dl = b.ue()?;
    let dc = b.ue()?;
    Some(SpsInfo { width: w, height: h, chroma, depth_luma: dl, depth_chroma: dc, ptl, sub_layers: msl, nested })
}

fn avcc(info: &SpsInfo, sps: &[u8], pps: &[u8]) -> Vec<u8> {
    bx(b"avcC", |b| {
        b.extend_from_slice(&[1, sps[1], sps[2], sps[3], 0xFF, 0xE1]);
        b.extend_from_slice(&(sps.len() as u16).to_be_bytes());
        b.extend_from_slice(sps);
        b.push(1);
        b.extend_from_slice(&(pps.len() as u16).to_be_bytes());
        b.extend_from_slice(pps);
        if matches!(sps[1], 100 | 110 | 122 | 144) {
            b.extend_from_slice(&[0xFC | info.chroma as u8, 0xF8 | info.depth_luma as u8, 0xF8 | info.depth_chroma as u8, 0]);
        }
    })
}

fn hvcc(info: &SpsInfo, vps: &[u8], sps: &[u8], pps: &[u8]) -> Vec<u8> {
    bx(b"hvcC", |b| {
        b.push(1);
        b.extend_from_slice(&info.ptl);
        b.extend_from_slice(&[0xF0, 0x00, 0xFC, 0xFC | info.chroma as u8, 0xF8 | info.depth_luma as u8, 0xF8 | info.depth_chroma as u8, 0, 0]);
        b.push((((info.sub_layers + 1) & 7) << 3) as u8 | (info.nested << 2) as u8 | 3);
        b.push(3);
        for (t, nal) in [(32u8, vps), (33, sps), (34, pps)] {
            b.push(0x80 | t);
            b.extend_from_slice(&1u16.to_be_bytes());
            b.extend_from_slice(&(nal.len() as u16).to_be_bytes());
            b.extend_from_slice(nal);
        }
    })
}

// ------------------------------------------------------------------ fMP4

fn be32(b: &[u8], i: usize) -> u32 {
    u32::from_be_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
fn be64(b: &[u8], i: usize) -> u64 {
    u64::from_be_bytes(b[i..i + 8].try_into().unwrap())
}

/// 一层里的各个 box：(类型, 整个 box, box 的内容)
fn children(d: &[u8]) -> Vec<([u8; 4], &[u8], &[u8])> {
    let mut out = vec![];
    let mut i = 0;
    while i + 8 <= d.len() {
        let mut size = be32(d, i) as usize;
        let typ: [u8; 4] = d[i + 4..i + 8].try_into().unwrap();
        let mut hl = 8;
        if size == 1 {
            if i + 16 > d.len() {
                break;
            }
            size = be64(d, i + 8) as usize;
            hl = 16;
        } else if size == 0 {
            size = d.len() - i;
        }
        if size < hl || i + size > d.len() {
            break;
        }
        out.push((typ, &d[i..i + size], &d[i + hl..i + size]));
        i += size;
    }
    out
}

fn child<'a>(d: &'a [u8], typ: &[u8; 4]) -> Option<&'a [u8]> {
    children(d).into_iter().find(|(t, _, _)| t == typ).map(|(_, _, b)| b)
}

fn path<'a>(mut d: &'a [u8], types: &[&[u8; 4]]) -> Option<&'a [u8]> {
    for t in types {
        d = child(d, t)?;
    }
    Some(d)
}

#[derive(Debug, Clone, Default)]
struct FTrack {
    id: u32,
    video: bool,
    timescale: u32,
    entry: Vec<u8>,
    width: u32,
    height: u32,
    def_dur: u32,
    def_size: u32,
    def_flags: u32,
    samples: Vec<Sample>,
    next: i64,
    edit: Option<(f64, i64)>,
}

fn parse_moov(moov: &[u8]) -> Result<Vec<FTrack>> {
    let mut tracks = vec![];
    // edit list 里空着的那一段按影片的 timescale 记
    let movie_ts = child(moov, b"mvhd").and_then(|m| match m.first() {
        Some(1) if m.len() >= 24 => Some(be32(m, 20)),
        Some(0) if m.len() >= 16 => Some(be32(m, 12)),
        _ => None,
    });
    for (t, _, trak) in children(moov) {
        if &t != b"trak" {
            continue;
        }
        let (Some(tkhd), Some(mdhd), Some(hdlr), Some(stsd)) =
            (child(trak, b"tkhd"), path(trak, &[b"mdia", b"mdhd"]), path(trak, &[b"mdia", b"hdlr"]), path(trak, &[b"mdia", b"minf", b"stbl", b"stsd"]))
        else {
            continue;
        };
        let video = match hdlr.get(8..12) {
            Some(b"vide") => true,
            Some(b"soun") => false,
            _ => continue,
        };
        if tkhd.len() < 84 || mdhd.len() < 24 || stsd.len() < 16 {
            continue;
        }
        let id = if tkhd[0] == 1 { be32(tkhd, 20) } else { be32(tkhd, 12) };
        let timescale = if mdhd[0] == 1 { be32(mdhd, 20) } else { be32(mdhd, 12) };
        let Some((et, entry, _)) = children(&stsd[8..]).into_iter().next() else { continue };
        if matches!(&et, b"encv" | b"enca") {
            bail!(tr!("这个视频是加密的，封装不了"));
        }
        let n = tkhd.len();
        tracks.push(FTrack { id, video, timescale, entry: entry.to_vec(), width: be32(tkhd, n - 8) >> 16, height: be32(tkhd, n - 4) >> 16, edit: movie_ts.filter(|t| *t > 0).and_then(|t| elst_start(trak, t)), ..Default::default() });
    }
    if let Some(mvex) = child(moov, b"mvex") {
        for (t, _, trex) in children(mvex) {
            if &t == b"trex" && trex.len() >= 24 {
                if let Some(tr) = tracks.iter_mut().find(|x| x.id == be32(trex, 4)) {
                    tr.def_dur = be32(trex, 12);
                    tr.def_size = be32(trex, 16);
                    tr.def_flags = be32(trex, 20);
                }
            }
        }
    }
    Ok(tracks)
}

/// edit list：开头空着的几段加起来几秒、第一段非空的 media_time（media_time 是 -1 的是空的一段）
fn elst_start(trak: &[u8], movie_ts: u32) -> Option<(f64, i64)> {
    let e = path(trak, &[b"edts", b"elst"])?;
    let v1 = *e.first()? == 1;
    let n = be32(e.get(..8)?, 4) as usize;
    let step = if v1 { 20 } else { 12 };
    let mut empty = 0u64;
    for i in 0..n {
        let at = 8 + i * step;
        let (dur, mt) = if v1 { (be64(e.get(..at + 8)?, at), be64(e.get(..at + 16)?, at + 8) as i64) } else { (be32(e.get(..at + 4)?, at) as u64, be32(e.get(..at + 8)?, at + 4) as i32 as i64) };
        if mt < 0 {
            empty += dur;
        } else {
            return Some((empty as f64 / movie_ts as f64, mt));
        }
    }
    None
}

fn parse_moof(moof: &[u8], moof_at: u64, tracks: &mut [FTrack]) {
    for (t, _, traf) in children(moof) {
        if &t != b"traf" {
            continue;
        }
        let Some(tfhd) = child(traf, b"tfhd").filter(|b| b.len() >= 8) else { continue };
        let fl = be32(tfhd, 0) & 0xFF_FFFF;
        let Some(tr) = tracks.iter_mut().find(|x| x.id == be32(tfhd, 4)) else { continue };
        let mut j = 8;
        let mut opt = |flag: u32, n: usize| -> Option<u64> {
            if fl & flag == 0 || j + n > tfhd.len() {
                return None;
            }
            let v = if n == 8 { be64(tfhd, j) } else { be32(tfhd, j) as u64 };
            j += n;
            Some(v)
        };
        let base = opt(0x1, 8).unwrap_or(moof_at);
        opt(0x2, 4);
        let dd = opt(0x8, 4).map(|v| v as u32).unwrap_or(tr.def_dur);
        let ds = opt(0x10, 4).map(|v| v as u32).unwrap_or(tr.def_size);
        let df = opt(0x20, 4).map(|v| v as u32).unwrap_or(tr.def_flags);
        if let Some(tfdt) = child(traf, b"tfdt").filter(|b| b.len() >= 8) {
            tr.next = if tfdt[0] == 1 && tfdt.len() >= 12 { be64(tfdt, 4) as i64 } else { be32(tfdt, 4) as i64 };
        }
        let mut pos = base;
        for (t, _, trun) in children(traf) {
            if &t != b"trun" || trun.len() < 8 {
                continue;
            }
            let v1 = trun[0] == 1;
            let tf = be32(trun, 0) & 0xFF_FFFF;
            let count = be32(trun, 4) as usize;
            let mut k = 8;
            let mut take = |flag: u32| -> Option<u32> {
                if tf & flag == 0 || k + 4 > trun.len() {
                    return None;
                }
                let v = be32(trun, k);
                k += 4;
                Some(v)
            };
            if let Some(off) = take(0x1) {
                pos = (base as i64 + off as i32 as i64) as u64;
            }
            let first_flags = take(0x4);
            for n in 0..count {
                let dur = take(0x100).unwrap_or(dd);
                let size = take(0x200).unwrap_or(ds);
                let flags = take(0x400).or(if n == 0 { first_flags } else { None }).unwrap_or(df);
                let cto = take(0x800).map(|c| if v1 { c as i32 } else { c.min(i32::MAX as u32) as i32 }).unwrap_or(0);
                let sync = !tr.video || flags & 0x1_0000 == 0;
                tr.samples.push(Sample { dts: tr.next, dur, cto, size, sync, off: pos });
                tr.next += dur as i64;
                pos += size as u64;
            }
        }
    }
}

fn demux_fmp4(p: &Path, want_video: bool, want_audio: bool) -> Result<Vec<Track>> {
    let mut f = File::open(p)?;
    let len = f.metadata()?.len();
    let mut pos = 0u64;
    let mut tracks: Vec<FTrack> = vec![];
    let mut hdr = [0u8; 16];
    while pos + 8 <= len {
        f.seek(SeekFrom::Start(pos))?;
        read_full(&mut f, &mut hdr[..8])?;
        let mut size = be32(&hdr, 0) as u64;
        let typ: [u8; 4] = hdr[4..8].try_into().unwrap();
        let mut hl = 8;
        if size == 1 {
            read_full(&mut f, &mut hdr[8..16])?;
            size = be64(&hdr, 8);
            hl = 16;
        } else if size == 0 {
            size = len - pos;
        }
        if size < hl || pos + size > len {
            break;
        }
        if matches!(&typ, b"moov" | b"moof") {
            let mut body = vec![0u8; (size - hl) as usize];
            read_full(&mut f, &mut body)?;
            if &typ == b"moov" {
                // 每个分片前都可能重复一份初始化分片（拼接时只写了一次，这里也只认第一份）
                if tracks.is_empty() {
                    tracks = parse_moov(&body)?;
                }
            } else {
                parse_moof(&body, pos, &mut tracks);
            }
        }
        pos += size;
    }
    if tracks.is_empty() {
        bail!(tr!("下载到的 MP4 分片里没有初始化信息（moov），封装不了"));
    }
    let mut out = vec![];
    for want in [true, false] {
        if (want && !want_video) || (!want && !want_audio) {
            continue;
        }
        if let Some(t) = tracks.iter_mut().find(|t| t.video == want && !t.samples.is_empty()) {
            out.push(Track { video: t.video, timescale: t.timescale.max(1), entry: std::mem::take(&mut t.entry), width: t.width, height: t.height, samples: std::mem::take(&mut t.samples), data: p.to_path_buf(), edit: t.edit });
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ 写 MP4

fn bx(typ: &[u8; 4], f: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    let mut b = vec![0, 0, 0, 0];
    b.extend_from_slice(typ);
    f(&mut b);
    let n = b.len() as u32;
    b[..4].copy_from_slice(&n.to_be_bytes());
    b
}

/// full box：版本 + 标志
fn fbx(typ: &[u8; 4], ver: u8, flags: u32, f: impl FnOnce(&mut Vec<u8>)) -> Vec<u8> {
    bx(typ, |b| {
        b.extend_from_slice(&((ver as u32) << 24 | flags).to_be_bytes());
        f(b);
    })
}

fn p16(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&(v as u16).to_be_bytes());
}
fn p32(b: &mut Vec<u8>, v: u32) {
    b.extend_from_slice(&v.to_be_bytes());
}
fn p64(b: &mut Vec<u8>, v: u64) {
    b.extend_from_slice(&v.to_be_bytes());
}

const MATRIX: [u32; 9] = [0x0001_0000, 0, 0, 0, 0x0001_0000, 0, 0, 0, 0x4000_0000];

fn visual_entry(typ: &[u8; 4], w: u32, h: u32, cfg: &[u8]) -> Vec<u8> {
    bx(typ, |b| {
        b.extend_from_slice(&[0; 6]);
        p16(b, 1);
        b.extend_from_slice(&[0; 16]);
        p16(b, w);
        p16(b, h);
        p32(b, 0x0048_0000);
        p32(b, 0x0048_0000);
        p32(b, 0);
        p16(b, 1);
        b.extend_from_slice(&[0; 32]);
        p16(b, 0x18);
        p16(b, 0xFFFF);
        b.extend_from_slice(cfg);
    })
}

fn audio_entry(chans: u16, rate: u32, asc: &[u8]) -> Vec<u8> {
    // MPEG-4 描述符：标签 + 长度（都小于 128，一个字节）
    let desc = |tag: u8, body: &[u8]| -> Vec<u8> {
        let mut v = vec![tag, body.len() as u8];
        v.extend_from_slice(body);
        v
    };
    let dsi = desc(5, asc);
    let mut dcd = vec![0x40, 0x15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    dcd.extend_from_slice(&dsi);
    let dcd = desc(4, &dcd);
    let mut es = vec![0, 1, 0];
    es.extend_from_slice(&dcd);
    es.extend_from_slice(&desc(6, &[2]));
    let es = desc(3, &es);
    let esds = fbx(b"esds", 0, 0, |b| b.extend_from_slice(&es));
    bx(b"mp4a", |b| {
        b.extend_from_slice(&[0; 6]);
        p16(b, 1);
        b.extend_from_slice(&[0; 8]);
        p16(b, chans as u32);
        p16(b, 16);
        p32(b, 0);
        p32(b, rate.min(0xFFFF) << 16);
        b.extend_from_slice(&esds);
    })
}

/// 一个块：同一路里连着的几帧（最多 1 秒），写在 mdat 里挨着
struct Chunk {
    track: usize,
    first: usize,
    n: usize,
    key: f64,
    bytes: u64,
}

/// 每一路的块：(这块几帧, 在文件里的位置)
type ChunkMap = Vec<Vec<(u32, u64)>>;

fn trak(t: &Track, id: u32, chunks: &[(u32, u64)], co64: bool, t0: f64) -> Vec<u8> {
    let ts = t.timescale as f64;
    let media_dur = t.media_dur();
    let media_time = t.media_time();
    let empty_ms = ((t.start_secs() - t0) * 1000.0).round().max(0.0) as u64;
    let show_ms = (((media_dur as i64 - media_time).max(0)) as f64 / ts * 1000.0).round() as u64;
    let track_ms = empty_ms + show_ms;
    let tkhd = fbx(b"tkhd", 0, 3, |b| {
        p32(b, 0);
        p32(b, 0);
        p32(b, id);
        p32(b, 0);
        p32(b, track_ms.min(u32::MAX as u64) as u32);
        b.extend_from_slice(&[0; 8]);
        p16(b, 0);
        p16(b, if t.video { 0 } else { 1 });
        p16(b, if t.video { 0 } else { 0x0100 });
        p16(b, 0);
        for m in MATRIX {
            p32(b, m);
        }
        p32(b, t.width << 16);
        p32(b, t.height << 16);
    });
    let elst = fbx(b"elst", 0, 0, |b| {
        let mut entries: Vec<(u32, i32)> = vec![];
        if empty_ms > 0 {
            entries.push((empty_ms as u32, -1));
        }
        entries.push((show_ms.min(u32::MAX as u64) as u32, media_time as i32));
        p32(b, entries.len() as u32);
        for (d, m) in entries {
            p32(b, d);
            p32(b, m as u32);
            p32(b, 0x0001_0000);
        }
    });
    let edts = bx(b"edts", |b| b.extend_from_slice(&elst));
    let big = media_dur > u32::MAX as u64;
    let mdhd = fbx(b"mdhd", big as u8, 0, |b| {
        if big {
            p64(b, 0);
            p64(b, 0);
            p32(b, t.timescale);
            p64(b, media_dur);
        } else {
            p32(b, 0);
            p32(b, 0);
            p32(b, t.timescale);
            p32(b, media_dur as u32);
        }
        p16(b, 0x55C4);
        p16(b, 0);
    });
    let hdlr = fbx(b"hdlr", 0, 0, |b| {
        p32(b, 0);
        b.extend_from_slice(if t.video { b"vide" } else { b"soun" });
        b.extend_from_slice(&[0; 12]);
        b.extend_from_slice(if t.video { b"VideoHandler\0" } else { b"SoundHandler\0" });
    });
    let xmhd = if t.video {
        fbx(b"vmhd", 0, 1, |b| b.extend_from_slice(&[0; 8]))
    } else {
        fbx(b"smhd", 0, 0, |b| b.extend_from_slice(&[0; 4]))
    };
    let dinf = bx(b"dinf", |b| {
        b.extend_from_slice(&fbx(b"dref", 0, 0, |b| {
            p32(b, 1);
            b.extend_from_slice(&fbx(b"url ", 0, 1, |_| {}));
        }))
    });
    let s = &t.samples;
    let stsd = fbx(b"stsd", 0, 0, |b| {
        p32(b, 1);
        b.extend_from_slice(&t.entry);
    });
    let stts = fbx(b"stts", 0, 0, |b| {
        let mut runs: Vec<(u32, u32)> = vec![];
        for x in s {
            match runs.last_mut() {
                Some((n, d)) if *d == x.dur => *n += 1,
                _ => runs.push((1, x.dur)),
            }
        }
        p32(b, runs.len() as u32);
        for (n, d) in runs {
            p32(b, n);
            p32(b, d);
        }
    });
    let ctts = s.iter().any(|x| x.cto != 0).then(|| {
        let neg = s.iter().any(|x| x.cto < 0);
        fbx(b"ctts", neg as u8, 0, |b| {
            let mut runs: Vec<(u32, i32)> = vec![];
            for x in s {
                match runs.last_mut() {
                    Some((n, c)) if *c == x.cto => *n += 1,
                    _ => runs.push((1, x.cto)),
                }
            }
            p32(b, runs.len() as u32);
            for (n, c) in runs {
                p32(b, n);
                p32(b, c as u32);
            }
        })
    });
    let stss = (t.video && s.iter().any(|x| !x.sync)).then(|| {
        fbx(b"stss", 0, 0, |b| {
            let idx: Vec<u32> = s.iter().enumerate().filter(|(_, x)| x.sync).map(|(i, _)| i as u32 + 1).collect();
            p32(b, idx.len() as u32);
            for i in idx {
                p32(b, i);
            }
        })
    });
    let stsc = fbx(b"stsc", 0, 0, |b| {
        let mut runs: Vec<(u32, u32)> = vec![];
        for (i, (n, _)) in chunks.iter().enumerate() {
            if runs.last().is_none_or(|(_, m)| m != n) {
                runs.push((i as u32 + 1, *n));
            }
        }
        p32(b, runs.len() as u32);
        for (first, n) in runs {
            p32(b, first);
            p32(b, n);
            p32(b, 1);
        }
    });
    let stsz = fbx(b"stsz", 0, 0, |b| {
        p32(b, 0);
        p32(b, s.len() as u32);
        for x in s {
            p32(b, x.size);
        }
    });
    let stco = if co64 {
        fbx(b"co64", 0, 0, |b| {
            p32(b, chunks.len() as u32);
            for (_, o) in chunks {
                p64(b, *o);
            }
        })
    } else {
        fbx(b"stco", 0, 0, |b| {
            p32(b, chunks.len() as u32);
            for (_, o) in chunks {
                p32(b, *o as u32);
            }
        })
    };
    let stbl = bx(b"stbl", |b| {
        for part in [Some(stsd), Some(stts), ctts, stss, Some(stsc), Some(stsz), Some(stco)].into_iter().flatten() {
            b.extend_from_slice(&part);
        }
    });
    let minf = bx(b"minf", |b| {
        b.extend_from_slice(&xmhd);
        b.extend_from_slice(&dinf);
        b.extend_from_slice(&stbl);
    });
    let mdia = bx(b"mdia", |b| {
        b.extend_from_slice(&mdhd);
        b.extend_from_slice(&hdlr);
        b.extend_from_slice(&minf);
    });
    bx(b"trak", |b| {
        b.extend_from_slice(&tkhd);
        b.extend_from_slice(&edts);
        b.extend_from_slice(&mdia);
    })
}

fn moov(tracks: &[Track], chunks: &ChunkMap, co64: bool) -> Vec<u8> {
    let t0 = tracks.iter().map(Track::start_secs).fold(f64::INFINITY, f64::min);
    let movie_ms = tracks
        .iter()
        .map(|t| {
            let empty = ((t.start_secs() - t0) * 1000.0).round().max(0.0) as u64;
            empty + (((t.media_dur() as i64 - t.media_time()).max(0)) as f64 / t.timescale as f64 * 1000.0).round() as u64
        })
        .max()
        .unwrap_or(0);
    let big = movie_ms > u32::MAX as u64;
    let mvhd = fbx(b"mvhd", big as u8, 0, |b| {
        if big {
            p64(b, 0);
            p64(b, 0);
            p32(b, 1000);
            p64(b, movie_ms);
        } else {
            p32(b, 0);
            p32(b, 0);
            p32(b, 1000);
            p32(b, movie_ms as u32);
        }
        p32(b, 0x0001_0000);
        p16(b, 0x0100);
        b.extend_from_slice(&[0; 10]);
        for m in MATRIX {
            p32(b, m);
        }
        b.extend_from_slice(&[0; 24]);
        p32(b, tracks.len() as u32 + 1);
    });
    bx(b"moov", |b| {
        b.extend_from_slice(&mvhd);
        for (i, t) in tracks.iter().enumerate() {
            b.extend_from_slice(&trak(t, i as u32 + 1, &chunks[i], co64, t0));
        }
    })
}

fn write_mp4(tracks: &[Track], out: &Path, on: &mut dyn FnMut(f64), stop: &AtomicBool) -> Result<Remuxed> {
    // 分块：每一路每块最多 1 秒，各路的块按时间交错排（播放时不用来回跳着读）
    let t0 = tracks.iter().map(Track::start_secs).fold(f64::INFINITY, f64::min);
    let mut list: Vec<Chunk> = vec![];
    for (ti, t) in tracks.iter().enumerate() {
        let s = &t.samples;
        let shift = t.start_secs() - t0;
        let mut i = 0;
        while i < s.len() {
            let mut j = i;
            let mut bytes = 0u64;
            while j < s.len() && (j == i || (s[j].dts - s[i].dts) < t.timescale as i64) && j - i < 1024 {
                bytes += s[j].size as u64;
                j += 1;
            }
            let key = (s[i].dts - s[0].dts) as f64 / t.timescale as f64 + shift;
            list.push(Chunk { track: ti, first: i, n: j - i, key, bytes });
            i = j;
        }
    }
    list.sort_by(|a, b| a.key.total_cmp(&b.key).then(a.track.cmp(&b.track)));
    let total: u64 = list.iter().map(|c| c.bytes).sum();

    let ftyp = bx(b"ftyp", |b| {
        b.extend_from_slice(b"isom");
        p32(b, 0x200);
        b.extend_from_slice(b"isomiso2avc1mp41");
    });
    let mut map: ChunkMap = vec![vec![]; tracks.len()];
    for c in &list {
        map[c.track].push((c.n as u32, 0));
    }
    // 第一遍定 moov 有多大（位置的值不影响大小），再决定 32 位还是 64 位的块位置
    let mut co64 = false;
    let probe_len = moov(tracks, &map, false).len() as u64;
    if ftyp.len() as u64 + probe_len + 16 + total > u32::MAX as u64 {
        co64 = true;
    }
    let moov_len = moov(tracks, &map, co64).len() as u64;
    let large = total + 8 > u32::MAX as u64;
    let hdr_len = if large { 16 } else { 8 };
    let mut at = ftyp.len() as u64 + moov_len + hdr_len;
    for m in map.iter_mut() {
        m.clear();
    }
    for c in &list {
        map[c.track].push((c.n as u32, at));
        at += c.bytes;
    }
    let mv = moov(tracks, &map, co64);
    debug_assert_eq!(mv.len() as u64, moov_len);

    let mut w = BufWriter::with_capacity(1 << 20, File::create(out).map_err(|e| crate::errors::from_io(&e).map(anyhow::Error::from).unwrap_or_else(|| e.into()))?);
    w.write_all(&ftyp)?;
    w.write_all(&mv)?;
    if large {
        w.write_all(&1u32.to_be_bytes())?;
        w.write_all(b"mdat")?;
        w.write_all(&(total + 16).to_be_bytes())?;
    } else {
        w.write_all(&((total + 8) as u32).to_be_bytes())?;
        w.write_all(b"mdat")?;
    }
    let mut readers: Vec<(BufReader<File>, u64)> = tracks.iter().map(|t| Ok((BufReader::with_capacity(1 << 20, File::open(&t.data)?), 0u64))).collect::<Result<_>>()?;
    let mut buf = vec![];
    let mut done = 0u64;
    let mut last_pct = -1i64;
    for c in &list {
        if stop.load(Ordering::Relaxed) {
            bail!("remux stopped");
        }
        let (r, pos) = &mut readers[c.track];
        for s in &tracks[c.track].samples[c.first..c.first + c.n] {
            if *pos != s.off {
                r.seek(SeekFrom::Start(s.off))?;
            }
            buf.resize(s.size as usize, 0);
            if read_full(r, &mut buf)? < buf.len() {
                bail!(tr!("下载到的分片不完整，封装不了"));
            }
            *pos = s.off + s.size as u64;
            w.write_all(&buf)?;
        }
        done += c.bytes;
        let pct = (done * 100 / total.max(1)) as i64;
        if pct != last_pct {
            last_pct = pct;
            on(done as f64 / total.max(1) as f64);
        }
    }
    w.flush()?;
    let secs = tracks.iter().map(|t| t.media_dur() as f64 / t.timescale as f64).fold(0.0, f64::max);
    Ok(Remuxed { video: tracks.iter().any(|t| t.video), audio: tracks.iter().any(|t| !t.video), secs })
}

// ------------------------------------------------------------------ 读 MP4 的基本信息（没有 ffprobe 时入库用）

/// 没有 ffprobe 时（手机上）从 MP4 / M4A / MOV 的 moov 读出类型、宽高、时长、编码，素材库里照样有这些信息。
/// 不是这几种文件、读不懂时返回 None（入库照旧，只是少这些信息）
pub fn mp4_info(p: &Path) -> Option<crate::ffmpeg::Probe> {
    let mut f = File::open(p).ok()?;
    let len = f.metadata().ok()?.len();
    let mut pos = 0u64;
    let mut hdr = [0u8; 16];
    while pos + 8 <= len {
        f.seek(SeekFrom::Start(pos)).ok()?;
        read_full(&mut f, &mut hdr[..8]).ok()?;
        let mut size = be32(&hdr, 0) as u64;
        let typ: [u8; 4] = hdr[4..8].try_into().ok()?;
        let mut hl = 8;
        if size == 1 {
            read_full(&mut f, &mut hdr[8..16]).ok()?;
            size = be64(&hdr, 8);
            hl = 16;
        } else if size == 0 {
            size = len - pos;
        }
        if size < hl || pos + size > len {
            return None;
        }
        if &typ == b"moov" {
            // moov 只有几 MB；太大的不是正常文件
            if size - hl > 64 << 20 {
                return None;
            }
            let mut body = vec![0u8; (size - hl) as usize];
            read_full(&mut f, &mut body).ok()?;
            return Some(moov_info(&body, p, len));
        }
        pos += size;
    }
    None
}

fn moov_info(moov: &[u8], p: &Path, size: u64) -> crate::ffmpeg::Probe {
    let mut out = crate::ffmpeg::Probe { size, format: p.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default(), ..Default::default() };
    if let Some(m) = child(moov, b"mvhd") {
        let (ts, dur) = match m.first() {
            Some(1) if m.len() >= 32 => (be32(m, 20), be64(m, 24)),
            Some(0) if m.len() >= 20 => (be32(m, 12), be32(m, 16) as u64),
            _ => (0, 0),
        };
        if ts > 0 && dur > 0 {
            out.duration = Some(dur as f64 / ts as f64);
        }
    }
    for (t, _, trak) in children(moov) {
        if &t != b"trak" {
            continue;
        }
        let hdlr = path(trak, &[b"mdia", b"hdlr"]).and_then(|h| h.get(8..12));
        let entry = path(trak, &[b"mdia", b"minf", b"stbl", b"stsd"]).and_then(|s| s.get(8..)).and_then(|s| children(s).into_iter().next().map(|(t, _, _)| t));
        let codec = |e: Option<[u8; 4]>| {
            e.map(|t| match &t {
                b"avc1" | b"avc3" => "h264".to_string(),
                b"hvc1" | b"hev1" => "hevc".to_string(),
                b"mp4a" => "aac".to_string(),
                other => String::from_utf8_lossy(other).trim().to_string(),
            })
        };
        match hdlr {
            Some(b"vide") if out.vcodec.is_none() => {
                if let Some(tk) = child(trak, b"tkhd").filter(|t| t.len() >= 84) {
                    let n = tk.len();
                    out.width = Some(be32(tk, n - 8) >> 16).filter(|w| *w > 0);
                    out.height = Some(be32(tk, n - 4) >> 16).filter(|h| *h > 0);
                }
                out.vcodec = codec(entry);
            }
            Some(b"soun") if out.acodec.is_none() => {
                out.acodec = codec(entry);
                out.has_audio = true;
            }
            _ => {}
        }
    }
    out.kind = if out.vcodec.is_some() && out.width.is_some() {
        "video".into()
    } else if out.has_audio {
        "audio".into()
    } else {
        String::new()
    };
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_unwrap() {
        let w = 1i64 << 33;
        assert_eq!(unwrap_ts(None, 5), 5);
        assert_eq!(unwrap_ts(Some(w - 10), 20), w + 20);
        assert_eq!(unwrap_ts(Some(w + 20), w - 10), w - 10);
        assert_eq!(unwrap_ts(Some(100), 3100), 3100);
    }

    #[test]
    fn annexb_split() {
        let d = [0, 0, 0, 1, 0x67, 1, 2, 0, 0, 1, 0x68, 3, 0, 0, 0, 1, 0x65, 9, 9, 0];
        let n = annexb(&d);
        assert_eq!(n, vec![&[0x67, 1, 2][..], &[0x68, 3][..], &[0x65, 9, 9][..]]);
        assert!(annexb(&[1, 2, 3]).is_empty());
    }

    #[test]
    fn emulation_prevention() {
        assert_eq!(rbsp(&[0, 0, 3, 1, 0, 0, 3, 0]), vec![0, 0, 1, 0, 0, 0]);
    }

    #[test]
    fn adts_to_asc() {
        // AAC-LC、44.1 kHz、立体声、一帧 10 字节（7 字节头 + 3 字节数据）
        let frame = [0xFF, 0xF1, 0x50, 0x80, 0x01, 0x5F, 0xFC, 0xAA, 0xBB, 0xCC];
        let dir = tempfile::tempdir().unwrap();
        let mut a = AudioSink { es: Es::new(dir.path().join("a")).unwrap(), samples: vec![], pts: None, last: None, next: None, rate: 0, chans: 2, asc: None, carry: vec![] };
        // 第一帧被切成两个 PES
        a.push(Some(90_000), &frame[..5]).unwrap();
        a.push(None, &[&frame[5..], &frame[..]].concat()).unwrap();
        assert_eq!(a.asc, Some([0x12, 0x10]));
        assert_eq!(a.rate, 44100);
        assert_eq!(a.samples.len(), 2);
        assert_eq!((a.samples[0].dts, a.samples[1].dts, a.samples[0].size), (44100, 44100 + 1024, 3));
        let t = a.finish().unwrap().unwrap();
        assert_eq!(std::fs::read(&t.data).unwrap(), vec![0xAA, 0xBB, 0xCC, 0xAA, 0xBB, 0xCC]);
    }

    fn have(bin: &str) -> bool {
        std::process::Command::new(bin).arg("-version").output().is_ok_and(|o| o.status.success())
    }

    fn ffmpeg(args: &[&str]) -> bool {
        std::process::Command::new("ffmpeg").args(["-hide_banner", "-loglevel", "error", "-y"]).args(args).status().is_ok_and(|s| s.success())
    }

    /// ffprobe 读出来的：(编码列表, 时长, 宽, 高)
    fn probe(p: &Path) -> (Vec<String>, f64, u32, u32) {
        let o = std::process::Command::new("ffprobe").args(["-v", "error", "-show_entries", "stream=codec_name,width,height:format=duration", "-of", "json"]).arg(p).output().unwrap();
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        let st = v["streams"].as_array().unwrap();
        let codecs = st.iter().map(|s| s["codec_name"].as_str().unwrap_or("").to_string()).collect();
        let w = st.iter().find_map(|s| s["width"].as_u64()).unwrap_or(0) as u32;
        let h = st.iter().find_map(|s| s["height"].as_u64()).unwrap_or(0) as u32;
        (codecs, v["format"]["duration"].as_str().and_then(|d| d.parse().ok()).unwrap_or(0.0), w, h)
    }

    /// 整个文件解一遍，ffmpeg 没报错
    fn decodes(p: &Path) -> bool {
        let o = std::process::Command::new("ffmpeg").args(["-v", "error", "-i"]).arg(p).args(["-f", "null", "-"]).output().unwrap();
        o.status.success() && o.stderr.is_empty()
    }

    const SRC: [&str; 8] = ["-f", "lavfi", "-i", "testsrc2=size=320x240:rate=25:duration=3", "-f", "lavfi", "-i", "sine=frequency=440:duration=3"];

    /// 本机有 ffmpeg 时：TS（H.264 + AAC，带 B 帧）→ MP4，ffprobe 和解码核对
    #[test]
    fn ts_h264_aac() {
        if !have("ffmpeg") || !have("ffprobe") {
            eprintln!("没有 ffmpeg，跳过");
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let ts = d.path().join("in.ts");
        assert!(ffmpeg(&[&SRC[..], &["-c:v", "libx264", "-bf", "2", "-g", "25", "-c:a", "aac", "-f", "mpegts", ts.to_str().unwrap()]].concat()));
        let out = d.path().join("out.mp4");
        let mut steps = 0;
        let r = remux(&ts, None, &out, &d.path().join("work"), |_| steps += 1).unwrap();
        assert!(r.video && r.audio && steps > 0);
        let (codecs, dur, w, _) = probe(&out);
        assert_eq!(codecs, vec!["h264", "aac"]);
        assert!((dur - 3.0).abs() < 0.2, "{dur}");
        assert_eq!(w, 320);
        assert!(decodes(&out));
        assert!(!d.path().join("work").exists(), "临时文件没删");
        // 没有 ffprobe 时入库用的基本信息
        let info = mp4_info(&out).unwrap();
        assert_eq!((info.kind.as_str(), info.width, info.height, info.vcodec.as_deref(), info.acodec.as_deref(), info.format.as_str()), ("video", Some(320), Some(240), Some("h264"), Some("aac"), "MP4"));
        assert!((info.duration.unwrap() - 3.0).abs() < 0.2, "{:?}", info.duration);
        assert!(mp4_info(&ts).is_none());
        // 和 ffmpeg `-c copy -bsf:a aac_adtstoasc` 封装出来的对照：每一路的帧数、起点一样（音画对得齐）
        let reference = d.path().join("ref.mp4");
        assert!(ffmpeg(&["-i", ts.to_str().unwrap(), "-c", "copy", "-bsf:a", "aac_adtstoasc", reference.to_str().unwrap()]));
        // 起点允许差 2 毫秒：edit list 按毫秒记，不同版本的 ffmpeg 写出来的起点有零点几毫秒的出入（比如 0.023000 和 0.023222）
        let (a, b) = (starts(&out), starts(&reference));
        assert_eq!(a.iter().map(|x| (&x.0, &x.1)).collect::<Vec<_>>(), b.iter().map(|x| (&x.0, &x.1)).collect::<Vec<_>>());
        for (x, y) in a.iter().zip(&b) {
            assert!((x.2 - y.2).abs() < 0.002, "{} 的起点 {} 和 ffmpeg 的 {} 差太多（{a:?} / {b:?}）", x.0, x.2, y.2);
        }
    }

    /// 每一路的 (编码, 帧数, 起点秒数)
    fn starts(p: &Path) -> Vec<(String, String, f64)> {
        streams(p).into_iter().map(|(c, n, t)| (c, n, t.trim_matches('"').parse().unwrap_or(0.0))).collect()
    }

    /// 画面比声音晚几秒（第一路画面、第二路声音）
    fn offset(s: &[(String, String, f64)]) -> f64 {
        s[0].2 - s[1].2
    }

    /// 编码、帧数和 ffmpeg 的一样；画面比声音晚的秒数是 `want`（edit list 按毫秒记，容差 2 毫秒）
    fn same_tracks(out: &Path, reference: &Path, want: f64) {
        let (a, b) = (starts(out), starts(reference));
        assert_eq!(a.iter().map(|x| (&x.0, &x.1)).collect::<Vec<_>>(), b.iter().map(|x| (&x.0, &x.1)).collect::<Vec<_>>());
        assert!((offset(&a) - want).abs() < 0.002, "两路起点差 {} 应该是 {want}（{a:?}）", offset(&a));
    }

    /// 每一路的 (编码, 帧数, 起点)
    fn streams(p: &Path) -> Vec<(String, String, String)> {
        let o = std::process::Command::new("ffprobe").args(["-v", "error", "-count_packets", "-show_entries", "stream=codec_name,nb_read_packets,start_time", "-of", "json"]).arg(p).output().unwrap();
        let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
        v["streams"].as_array().unwrap().iter().map(|s| (s["codec_name"].to_string(), s["nb_read_packets"].to_string(), s["start_time"].to_string())).collect()
    }

    /// 画面和声音分两路（EXT-X-MEDIA）
    #[test]
    fn ts_separate_audio() {
        if !have("ffmpeg") || !have("ffprobe") {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let (v, a) = (d.path().join("v.ts"), d.path().join("a.ts"));
        assert!(ffmpeg(&["-f", "lavfi", "-i", "testsrc2=size=320x180:rate=25:duration=3", "-c:v", "libx264", "-f", "mpegts", v.to_str().unwrap()]));
        assert!(ffmpeg(&["-f", "lavfi", "-i", "sine=duration=3", "-c:a", "aac", "-f", "mpegts", a.to_str().unwrap()]));
        let out = d.path().join("out.mp4");
        remux(&v, Some(&a), &out, &d.path().join("work"), |_| {}).unwrap();
        let (codecs, dur, w, h) = probe(&out);
        assert_eq!((w, h), (320, 180), "SPS 里的裁剪没算对");
        assert_eq!(codecs, vec!["h264", "aac"]);
        assert!((dur - 3.0).abs() < 0.2, "{dur}");
        assert!(decodes(&out));
    }

    /// HEVC（本机 ffmpeg 有 libx265 时）
    #[test]
    fn ts_hevc() {
        if !have("ffmpeg") || !have("ffprobe") {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let ts = d.path().join("in.ts");
        if !ffmpeg(&[&SRC[..], &["-c:v", "libx265", "-x265-params", "log-level=error", "-c:a", "aac", "-f", "mpegts", ts.to_str().unwrap()]].concat()) {
            eprintln!("ffmpeg 没有 libx265，跳过");
            return;
        }
        let out = d.path().join("out.mp4");
        remux(&ts, None, &out, &d.path().join("work"), |_| {}).unwrap();
        let (codecs, dur, w, _) = probe(&out);
        assert_eq!(codecs, vec!["hevc", "aac"]);
        assert!((dur - 3.0).abs() < 0.2, "{dur}");
        assert_eq!(w, 320);
        assert!(decodes(&out));
    }

    /// fMP4 的分片（初始化分片 + 各分片直接拼起来，和 M3U8 引擎的 concat 一样）
    #[test]
    fn fmp4_segments() {
        if !have("ffmpeg") || !have("ffprobe") {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let m3u8 = d.path().join("v.m3u8");
        assert!(ffmpeg(&[&SRC[..], &["-c:v", "libx264", "-g", "25", "-c:a", "aac", "-f", "hls", "-hls_time", "1", "-hls_segment_type", "fmp4", "-hls_playlist_type", "vod", m3u8.to_str().unwrap()]].concat()));
        let list = std::fs::read_to_string(&m3u8).unwrap();
        let mut joined = std::fs::read(d.path().join("init.mp4")).unwrap();
        for l in list.lines().filter(|l| l.ends_with(".m4s")) {
            joined.extend(std::fs::read(d.path().join(l)).unwrap());
        }
        let src = d.path().join("joined.mp4");
        std::fs::write(&src, joined).unwrap();
        let out = d.path().join("out.mp4");
        remux(&src, None, &out, &d.path().join("work"), |_| {}).unwrap();
        let (codecs, dur, _, _) = probe(&out);
        assert_eq!(codecs, vec!["h264", "aac"]);
        assert!((dur - 3.0).abs() < 0.2, "{dur}");
        assert!(decodes(&out));
        // 带 B 帧的 fMP4 原来就有 edit list（开头空一段 + 跳过显示延迟）：帧数和 ffmpeg 封装的一样，两路的起点差和源文件一样（音画对得齐）
        let reference = d.path().join("ref.mp4");
        assert!(ffmpeg(&["-i", src.to_str().unwrap(), "-c", "copy", reference.to_str().unwrap()]));
        same_tracks(&out, &reference, offset(&starts(&src)));
    }

    /// fMP4 画面和声音分两路（EXT-X-MEDIA），各自带 edit list：和 ffmpeg `-map 0:v -map 1:a -c copy` 的结果对照
    #[test]
    fn fmp4_separate_audio() {
        if !have("ffmpeg") || !have("ffprobe") {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let mut parts = vec![];
        for (tag, map) in [("v", "0:v"), ("a", "1:a")] {
            let sub = d.path().join(tag);
            std::fs::create_dir_all(&sub).unwrap();
            let m3u8 = sub.join("x.m3u8");
            assert!(ffmpeg(&[&SRC[..], &["-map", map, "-c:v", "libx264", "-bf", "2", "-g", "25", "-c:a", "aac", "-f", "hls", "-hls_time", "1", "-hls_segment_type", "fmp4", "-hls_playlist_type", "vod", m3u8.to_str().unwrap()]].concat()));
            let list = std::fs::read_to_string(&m3u8).unwrap();
            let mut joined = std::fs::read(sub.join("init.mp4")).unwrap();
            for l in list.lines().filter(|l| l.ends_with(".m4s")) {
                joined.extend(std::fs::read(sub.join(l)).unwrap());
            }
            let f = d.path().join(format!("{tag}.mp4"));
            std::fs::write(&f, joined).unwrap();
            parts.push(f);
        }
        let out = d.path().join("out.mp4");
        remux(&parts[0], Some(&parts[1]), &out, &d.path().join("work"), |_| {}).unwrap();
        assert!(decodes(&out));
        // ffmpeg 两个输入各自从 0 算（不看两路共用的时间轴），起点差不拿它比，拿两个源文件自己的起点差比
        let reference = d.path().join("ref.mp4");
        assert!(ffmpeg(&["-i", parts[0].to_str().unwrap(), "-i", parts[1].to_str().unwrap(), "-map", "0:v", "-map", "1:a", "-c", "copy", reference.to_str().unwrap()]));
        let want = starts(&parts[0])[0].2 - starts(&parts[1])[0].2;
        same_tracks(&out, &reference, want);
    }

    /// 认不得的编码（MP3 进 TS）说清楚
    #[test]
    fn unsupported_codec() {
        if !have("ffmpeg") {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let ts = d.path().join("in.ts");
        if !ffmpeg(&["-f", "lavfi", "-i", "sine=duration=1", "-c:a", "libmp3lame", "-f", "mpegts", ts.to_str().unwrap()]) {
            return;
        }
        let e = remux(&ts, None, &d.path().join("o.mp4"), &d.path().join("w"), |_| {}).unwrap_err();
        assert!(format!("{e}").contains("MP3"), "{e}");
        assert!(!d.path().join("o.mp4").exists());
    }
}
