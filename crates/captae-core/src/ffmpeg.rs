//! ffmpeg / ffprobe 封装：格式转换、剪辑、字幕烧录、关键帧、缩略图。
//! 全部走子进程，`-progress pipe:1` 拿进度。

use crate::tools::{run_ok, Tool, Toolbox};
use crate::tr;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Probe {
    pub kind: String,
    pub format: String,
    pub duration: Option<f64>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub vcodec: Option<String>,
    pub acodec: Option<String>,
    pub size: u64,
    pub has_audio: bool,
    /// 帧率，如 25、29.97
    pub fps: Option<f64>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u32>,
}

/// ffprobe 的 "30000/1001" → 29.97
fn parse_rate(s: &str) -> Option<f64> {
    let (a, b) = s.split_once('/').unwrap_or((s, "1"));
    let (a, b) = (a.trim().parse::<f64>().ok()?, b.trim().parse::<f64>().ok()?);
    (a > 0.0 && b > 0.0).then(|| a / b)
}

pub async fn probe(tools: &Toolbox, p: &Path) -> Result<Probe> {
    let mut c = tools.command(Tool::Ffprobe)?;
    c.args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"]).arg(p);
    let out = run_ok(c).await.context(tr!("读取文件信息失败"))?;
    let v: Value = serde_json::from_slice(&out)?;
    Ok(parse_probe(&v, p))
}

pub fn parse_probe(v: &Value, p: &Path) -> Probe {
    let streams = v.get("streams").and_then(Value::as_array).cloned().unwrap_or_default();
    let video = streams.iter().find(|s| {
        s.get("codec_type").and_then(Value::as_str) == Some("video")
            && s.get("disposition").and_then(|d| d.get("attached_pic")).and_then(Value::as_i64) != Some(1)
    });
    let audio = streams.iter().find(|s| s.get("codec_type").and_then(Value::as_str) == Some("audio"));
    let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    let by_ext = crate::detect::media_kind_of_ext(&ext);
    let fmt = v.get("format");
    let duration = fmt.and_then(|f| f.get("duration")).and_then(Value::as_str).and_then(|d| d.parse().ok());
    let kind = if by_ext == "image" {
        "image"
    } else if video.is_some() && duration.map(|d: f64| d > 0.5).unwrap_or(false) {
        "video"
    } else if audio.is_some() {
        "audio"
    } else if video.is_some() {
        "image"
    } else {
        by_ext
    };
    Probe {
        kind: kind.to_string(),
        format: ext.to_uppercase(),
        duration: if kind == "image" { None } else { duration },
        width: video.and_then(|s| s.get("width")).and_then(Value::as_u64).map(|x| x as u32),
        height: video.and_then(|s| s.get("height")).and_then(Value::as_u64).map(|x| x as u32),
        vcodec: video.and_then(|s| s.get("codec_name")).and_then(Value::as_str).map(str::to_string),
        acodec: audio.and_then(|s| s.get("codec_name")).and_then(Value::as_str).map(str::to_string),
        size: fmt.and_then(|f| f.get("size")).and_then(Value::as_str).and_then(|s| s.parse().ok()).unwrap_or(0),
        has_audio: audio.is_some(),
        fps: video.and_then(|s| s.get("avg_frame_rate").and_then(Value::as_str).and_then(parse_rate).or_else(|| s.get("r_frame_rate").and_then(Value::as_str).and_then(parse_rate))),
        sample_rate: audio.and_then(|s| s.get("sample_rate")).and_then(Value::as_str).and_then(|x| x.parse().ok()),
        channels: audio.and_then(|s| s.get("channels")).and_then(Value::as_u64).map(|x| x as u32),
    }
}

/// `-progress` 输出里的 `out_time_us=12345678` → 秒
pub fn parse_progress_time(l: &str) -> Option<f64> {
    let (k, v) = l.split_once('=')?;
    match k {
        "out_time_us" | "out_time_ms" => v.trim().parse::<f64>().ok().map(|x| x / 1e6),
        _ => None,
    }
}

/// 目标格式 → (扩展名, 编码参数)。
pub fn target(format: &str) -> Result<(&'static str, Vec<&'static str>)> {
    let f = format.to_ascii_lowercase().replace([' ', '_'], "");
    Ok(match f.as_str() {
        "mp4" | "h264" => ("mp4", vec!["-c:v", "libx264", "-preset", "medium", "-crf", "20", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"]),
        "webm" => ("webm", vec!["-c:v", "libvpx-vp9", "-crf", "32", "-b:v", "0", "-row-mt", "1", "-c:a", "libopus", "-b:a", "128k"]),
        "mov" => ("mov", vec!["-c:v", "libx264", "-crf", "18", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "256k"]),
        "mkv" => ("mkv", vec!["-c", "copy"]),
        "prores" | "prores422" => ("mov", vec!["-c:v", "prores_ks", "-profile:v", "2", "-vendor", "apl0", "-pix_fmt", "yuv422p10le", "-c:a", "pcm_s16le"]),
        "proreshq" | "prores422hq" => ("mov", vec!["-c:v", "prores_ks", "-profile:v", "3", "-vendor", "apl0", "-pix_fmt", "yuv422p10le", "-c:a", "pcm_s16le"]),
        "mp3" => ("mp3", vec!["-vn", "-c:a", "libmp3lame", "-q:a", "2"]),
        "aac" | "m4a" => ("m4a", vec!["-vn", "-c:a", "aac", "-b:a", "256k"]),
        "flac" => ("flac", vec!["-vn", "-c:a", "flac"]),
        "wav" => ("wav", vec!["-vn", "-c:a", "pcm_s16le"]),
        "png" => ("png", vec!["-frames:v", "1"]),
        "jpg" | "jpeg" => ("jpg", vec!["-frames:v", "1", "-q:v", "2"]),
        "webp" => ("webp", vec!["-frames:v", "1", "-c:v", "libwebp", "-quality", "90"]),
        "avif" => ("avif", vec!["-frames:v", "1", "-c:v", "libaom-av1", "-still-picture", "1", "-crf", "30"]),
        "gif" => ("gif", vec!["-vf", "fps=12,scale=480:-1:flags=lanczos"]),
        _ => bail!(tr!("不支持的目标格式：{format}", format = format)),
    })
}

/// 可以只换封装、不重新编码的情况（快很多，也不损失画质）。
fn can_remux(src: &Probe, format: &str) -> bool {
    let f = format.to_ascii_lowercase();
    let v = src.vcodec.as_deref().unwrap_or("");
    let a = src.acodec.as_deref().unwrap_or("aac");
    match f.as_str() {
        "mp4" | "mov" => matches!(v, "h264" | "hevc") && matches!(a, "aac" | "mp3" | "alac"),
        "mkv" => true,
        "webm" => matches!(v, "vp8" | "vp9" | "av1") && matches!(a, "opus" | "vorbis"),
        _ => false,
    }
}

pub struct Job<'a> {
    pub tools: &'a Toolbox,
    pub duration: Option<f64>,
    /// 高级模式下 agent 传入的额外参数，插在输出文件前面
    pub extra: Vec<String>,
}

impl Job<'_> {
    /// 跑 ffmpeg，边跑边回调进度（0..1）。
    pub async fn run(&self, pre: &[String], mid: &[String], out: &Path, mut on: impl FnMut(f64)) -> Result<()> {
        let mut c = self.tools.command(Tool::Ffmpeg)?;
        c.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-progress", "pipe:1"]);
        c.args(pre).args(mid).args(&self.extra).arg(out);
        c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        // 启动不了：带上错误码 tool。errors::from_anyhow 先看错误码，不再靠「启动 ffmpeg 失败」这几个字归类（说法跟着界面语言）。
        // 说法照旧，原文 raw 和 tools::spawn_failure 的写法一样
        let child = crate::tools::spawn(&mut c).map_err(|e| crate::errors::Failure::new(crate::errors::ErrorCode::Tool, tr!("启动 ffmpeg 失败"), tr!("启动外部工具失败：{tool}: {error}", tool = "ffmpeg", error = e)))?;
        // 从这里起 out 是这次写的：出错、或者任务被取消（整个 future 被丢掉）时删掉，不留半成品（tasks::Partial）。
        // child 在它后面重新绑定一次：丢掉时先结束 ffmpeg（连同它可能起的子进程，tools::Proc），再删文件
        let partial = crate::tasks::Partial::new(out);
        let mut child = child;
        let mut err = child.stderr.take().unwrap();
        let et = tokio::spawn(async move {
            let mut s = String::new();
            use tokio::io::AsyncReadExt;
            let _ = err.read_to_string(&mut s).await;
            s
        });
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        while let Some(l) = lines.next_line().await? {
            if let (Some(t), Some(d)) = (parse_progress_time(&l), self.duration) {
                if d > 0.0 {
                    on((t / d).clamp(0.0, 1.0));
                }
            }
        }
        let st = child.wait().await?;
        let e = et.await.unwrap_or_default();
        if !st.success() {
            // partial 在这里被丢掉，没写完的文件跟着删
            return Err(anyhow!(humanize(&e)));
        }
        partial.keep();
        on(1.0);
        Ok(())
    }
}

fn humanize(e: &str) -> String {
    let l = e.to_ascii_lowercase();
    if l.contains("no space left") {
        tr!("磁盘空间不够了")
    } else if l.contains("permission denied") || l.contains("read-only") {
        tr!("要保存到的位置没有写入权限")
    } else if l.contains("unknown encoder") {
        tr!("当前的 ffmpeg 不包含这个编码器")
    } else {
        e.lines().last().map(str::to_string).unwrap_or_else(|| tr!("处理失败"))
    }
}

pub fn out_path(src: &Path, dir: Option<&Path>, suffix: &str, ext: &str) -> PathBuf {
    let stem = src.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "output".into());
    let dir = dir.map(Path::to_path_buf).unwrap_or_else(|| src.parent().unwrap_or(Path::new(".")).to_path_buf());
    crate::util::unique_path(dir.join(format!("{stem}{suffix}.{ext}")))
}

/// 转换时的「画质」：和原来一样 / 更清晰 / 更省空间。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Quality {
    #[default]
    Same,
    High,
    Small,
}

impl Quality {
    pub fn parse(s: Option<&str>) -> Self {
        match s {
            Some("high") => Quality::High,
            Some("small") => Quality::Small,
            _ => Quality::Same,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ConvertOpts {
    /// 输出高度（只缩小不放大）
    pub height: Option<u32>,
    pub quality: Quality,
}

impl ConvertOpts {
    fn is_default(&self) -> bool {
        self.height.is_none() && self.quality == Quality::Same
    }
}

fn set_arg(v: &mut Vec<String>, k: &str, val: &str) {
    match v.iter().position(|x| x == k) {
        Some(i) if i + 1 < v.len() => v[i + 1] = val.to_string(),
        _ => v.extend([k.to_string(), val.to_string()]),
    }
}

/// 目标格式 + 画质 → (扩展名, 编码参数)。每种编码的「画质」旋钮不一样：
/// H.264 / VP9 调 crf，ProRes 换档位（LT / 422 / HQ），MP3 调 VBR 档，AAC 调码率，图片调质量。
/// FLAC / WAV / PNG 是无损的，画质选项不起作用。
pub fn target_with(format: &str, q: Quality) -> Result<(&'static str, Vec<String>)> {
    let f = format.to_ascii_lowercase().replace([' ', '_'], "");
    let (ext, args) = target(format)?;
    let mut v: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    if q == Quality::Same {
        return Ok((ext, v));
    }
    let hi = q == Quality::High;
    match f.as_str() {
        "mp4" | "h264" | "mov" => set_arg(&mut v, "-crf", if hi { "16" } else { "28" }),
        "mkv" => {
            v = ["-c:v", "libx264", "-preset", "medium", "-crf", if hi { "16" } else { "28" }, "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k"].map(String::from).to_vec();
        }
        "webm" => set_arg(&mut v, "-crf", if hi { "24" } else { "40" }),
        "prores" | "prores422" => set_arg(&mut v, "-profile:v", if hi { "3" } else { "1" }),
        "proreshq" | "prores422hq" => set_arg(&mut v, "-profile:v", if hi { "3" } else { "2" }),
        "mp3" => set_arg(&mut v, "-q:a", if hi { "0" } else { "5" }),
        "aac" | "m4a" => set_arg(&mut v, "-b:a", if hi { "320k" } else { "128k" }),
        "jpg" | "jpeg" => set_arg(&mut v, "-q:v", if hi { "1" } else { "6" }),
        "webp" => set_arg(&mut v, "-quality", if hi { "96" } else { "70" }),
        "avif" => set_arg(&mut v, "-crf", if hi { "20" } else { "40" }),
        _ => {}
    }
    Ok((ext, v))
}

/// 转换产物的路径（和 `convert` 实际写出的一致）：扩展名没变时加「 (转换)」，重名时再加编号。
/// 后缀跟着界面语言（英文「 (converted)」）
pub fn convert_out_path(src: &Path, format: &str, out_dir: Option<&Path>) -> Result<PathBuf> {
    let (ext, _) = target(format)?;
    let same_ext = src.extension().map(|e| e.to_string_lossy().eq_ignore_ascii_case(ext)).unwrap_or(false);
    let suffix = if same_ext { format!(" {}", tr!("(转换)§转换出来的文件名后缀：「Sintel (转换).mp4」")) } else { String::new() };
    Ok(out_path(src, out_dir, &suffix, ext))
}

pub async fn convert(tools: &Toolbox, src: &Path, format: &str, out_dir: Option<&Path>, extra: Vec<String>, on: impl FnMut(f64)) -> Result<PathBuf> {
    convert_with(tools, src, format, out_dir, &ConvertOpts::default(), extra, on).await
}

pub async fn convert_with(tools: &Toolbox, src: &Path, format: &str, out_dir: Option<&Path>, o: &ConvertOpts, extra: Vec<String>, on: impl FnMut(f64)) -> Result<PathBuf> {
    let info = probe(tools, src).await?;
    let (ext, mut args) = target_with(format, o.quality)?;
    if can_remux(&info, format) && extra.is_empty() && o.is_default() {
        args = vec!["-c".into(), "copy".into()];
        if ext == "mp4" || ext == "mov" {
            args.extend(["-movflags".into(), "+faststart".into()]);
        }
    }
    let video_out = matches!(ext, "mp4" | "mov" | "mkv" | "webm");
    if let (Some(h), true) = (o.height, video_out) {
        if info.height.map(|x| x > h).unwrap_or(true) {
            args.extend(["-vf".into(), format!("scale=-2:{h}")]);
        }
    }
    let out = convert_out_path(src, format, out_dir)?;
    let pre = vec!["-i".to_string(), src.display().to_string()];
    Job { tools, duration: info.duration, extra }.run(&pre, &args, &out, on).await?;
    Ok(out)
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct TrimOpts {
    pub start: f64,
    pub end: f64,
    pub mute: bool,
    /// 精确到帧（重新编码）；否则按关键帧切，速度快但起点可能提前一点
    pub precise: bool,
    pub scale_height: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub replace_audio: Option<PathBuf>,
}

/// 片段文件名里的时间：83.4 秒 → 「1.23,4」（文件名里不能有冒号）
fn trim_label(t: f64) -> String {
    let tenths = (t.max(0.0) * 10.0).round() as u64;
    let (s, f) = (tenths / 10, tenths % 10);
    let base = if s >= 3600 { format!("{}.{:02}.{:02}", s / 3600, s % 3600 / 60, s % 60) } else { format!("{}.{:02}", s / 60, s % 60) };
    if f > 0 {
        format!("{base},{f}")
    } else {
        base
    }
}

pub async fn trim(tools: &Toolbox, src: &Path, o: &TrimOpts, out_dir: Option<&Path>, extra: Vec<String>, on: impl FnMut(f64)) -> Result<PathBuf> {
    if o.end <= o.start {
        bail!(tr!("结束时间要晚于开始时间"));
    }
    let info = probe(tools, src).await?;
    let has_video = info.width.is_some() && info.kind != "audio";
    let src_ext = src.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_else(|| "mp4".into());
    let replace = o.replace_audio.as_ref().filter(|_| has_video);
    if let Some(a) = replace {
        if !a.exists() {
            bail!(tr!("找不到要换上的声音文件：{path}", path = a.display()));
        }
    }
    let reencode_v = has_video && (o.precise || o.scale_height.is_some() || o.bitrate_kbps.is_some());
    // 重新编码成 H.264 + AAC、或换上 AAC 声音时，WebM 之类的封装装不下，改存 MP4
    let ext = if has_video && (reencode_v || replace.is_some()) && !matches!(src_ext.as_str(), "mp4" | "mov" | "m4v" | "mkv") { "mp4".to_string() } else { src_ext.clone() };
    let out = out_path(src, out_dir, &format!(" {}-{}", trim_label(o.start), trim_label(o.end)), &ext);
    let mut pre = vec!["-ss".to_string(), format!("{:.3}", o.start), "-to".into(), format!("{:.3}", o.end), "-i".into(), src.display().to_string()];
    if let Some(a) = replace {
        pre.extend(["-i".into(), a.display().to_string()]);
    }
    let mut mid: Vec<String> = vec![];
    if replace.is_some() {
        // 新声音从头开始；比画面短时补静音，比画面长时截掉
        mid.extend(["-map", "0:v:0", "-map", "1:a:0", "-af", "apad", "-shortest"].map(String::from));
    }
    if reencode_v {
        mid.extend(["-c:v", "libx264", "-preset", "medium", "-crf", "20", "-pix_fmt", "yuv420p"].map(String::from));
        if let Some(h) = o.scale_height {
            mid.extend(["-vf".into(), format!("scale=-2:{h}")]);
        }
        if let Some(b) = o.bitrate_kbps {
            mid.extend(["-b:v".into(), format!("{b}k")]);
        }
    } else if has_video {
        mid.extend(["-c:v", "copy"].map(String::from));
    }
    if o.mute && replace.is_none() {
        mid.push("-an".into());
    } else if replace.is_some() || reencode_v {
        mid.extend(["-c:a", "aac", "-b:a", "192k"].map(String::from));
    } else {
        // 只有声音的文件：音频帧很短，直接复制就足够准
        mid.extend(["-c:a", "copy"].map(String::from));
    }
    if !reencode_v {
        mid.extend(["-avoid_negative_ts", "make_zero"].map(String::from));
    }
    if matches!(ext.as_str(), "mp4" | "mov" | "m4v" | "m4a") {
        mid.extend(["-movflags", "+faststart"].map(String::from));
    }
    Job { tools, duration: Some(o.end - o.start), extra }.run(&pre, &mid, &out, on).await?;
    Ok(out)
}

/// 两段视频能不能不重新编码直接接起来：编码、分辨率、帧率、声音参数都一样。
fn concat_compatible(infos: &[Probe]) -> bool {
    infos.windows(2).all(|w| {
        let (a, b) = (&w[0], &w[1]);
        a.vcodec == b.vcodec
            && a.width == b.width
            && a.height == b.height
            && match (a.fps, b.fps) {
                (Some(x), Some(y)) => (x - y).abs() < 0.01,
                (None, None) => true,
                _ => false,
            }
            && a.has_audio == b.has_audio
            && a.acodec == b.acodec
            && a.sample_rate == b.sample_rate
            && a.channels == b.channels
    })
}

/// 拼接多段视频，按传入顺序。编码、分辨率、帧率都一致时用 concat demuxer 无损拼（`-c copy`）；
/// 不一致时以第一段为准：缩放并加黑边到同样大小、统一帧率，声音统一成 AAC，重新编码成 MP4。
/// 没有声音的片段补一段静音，保证声画对齐。
pub async fn concat(tools: &Toolbox, inputs: &[PathBuf], out_dir: Option<&Path>, on: impl FnMut(f64)) -> Result<PathBuf> {
    if inputs.len() < 2 {
        bail!(tr!("至少需要两段视频"));
    }
    let mut infos = vec![];
    for f in inputs {
        let i = probe(tools, f).await?;
        if i.kind != "video" || i.width.is_none() {
            bail!(tr!("「{name}」不是视频，只能拼接视频", name = f.file_name().unwrap_or_default().to_string_lossy()));
        }
        infos.push(i);
    }
    let first = &inputs[0];
    let total: f64 = infos.iter().filter_map(|i| i.duration).sum();
    let job = Job { tools, duration: Some(total), extra: vec![] };
    // 输出文件名的后缀跟着界面语言（英文「 (combined)」）
    let suffix = format!(" {}", tr!("(拼接)§拼接出来的文件名后缀：「Sintel (拼接).mp4」"));
    if concat_compatible(&infos) {
        let ext = first.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_else(|| "mp4".into());
        let out = out_path(first, out_dir, &suffix, &ext);
        let list = std::env::temp_dir().join(format!("captae-concat-{}.txt", crate::util::now_ms()));
        let body: String = inputs.iter().map(|f| format!("file '{}'\n", f.display().to_string().replace('\'', "'\\''"))).collect();
        tokio::fs::write(&list, body).await?;
        let mut mid: Vec<String> = ["-map", "0", "-c", "copy"].map(String::from).to_vec();
        if matches!(ext.as_str(), "mp4" | "mov" | "m4v") {
            mid.extend(["-movflags", "+faststart"].map(String::from));
        }
        let r = job.run(&["-f".into(), "concat".into(), "-safe".into(), "0".into(), "-i".into(), list.display().to_string()], &mid, &out, on).await;
        let _ = tokio::fs::remove_file(&list).await;
        r?;
        return Ok(out);
    }
    let out = out_path(first, out_dir, &suffix, "mp4");
    let w = infos[0].width.unwrap_or(1280) / 2 * 2;
    let h = infos[0].height.unwrap_or(720) / 2 * 2;
    let fps = infos[0].fps.filter(|f| *f > 1.0 && *f <= 120.0).unwrap_or(30.0);
    let any_audio = infos.iter().any(|i| i.has_audio);
    let mut pre = vec![];
    let mut filter = String::new();
    for (i, (f, info)) in inputs.iter().zip(&infos).enumerate() {
        pre.extend(["-i".to_string(), f.display().to_string()]);
        filter.push_str(&format!(
            "[{i}:v:0]scale={w}:{h}:force_original_aspect_ratio=decrease,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={fps:.3},format=yuv420p[v{i}];"
        ));
        if any_audio {
            if info.has_audio {
                filter.push_str(&format!("[{i}:a:0]aresample=48000,aformat=sample_fmts=fltp:channel_layouts=stereo[a{i}];"));
            } else {
                let d = info.duration.unwrap_or(0.1).max(0.05);
                filter.push_str(&format!("anullsrc=r=48000:cl=stereo,atrim=duration={d:.3},aformat=sample_fmts=fltp:channel_layouts=stereo[a{i}];"));
            }
        }
    }
    for i in 0..inputs.len() {
        filter.push_str(&format!("[v{i}]"));
        if any_audio {
            filter.push_str(&format!("[a{i}]"));
        }
    }
    filter.push_str(&format!("concat=n={}:v=1:a={}[v]{}", inputs.len(), any_audio as u8, if any_audio { "[a]" } else { "" }));
    let mut mid: Vec<String> = ["-filter_complex", &filter, "-map", "[v]"].iter().map(|s| s.to_string()).collect();
    if any_audio {
        mid.extend(["-map", "[a]", "-c:a", "aac", "-b:a", "192k"].map(String::from));
    }
    mid.extend(["-c:v", "libx264", "-preset", "medium", "-crf", "20", "-pix_fmt", "yuv420p", "-movflags", "+faststart"].map(String::from));
    job.run(&pre, &mid, &out, on).await?;
    Ok(out)
}

/// 这个 ffmpeg 有没有某个滤镜（比如 Homebrew 的 ffmpeg 不带 libass，就没有 `subtitles`）。
pub async fn has_filter(tools: &Toolbox, name: &str) -> bool {
    let Ok(mut c) = tools.command(Tool::Ffmpeg) else { return false };
    c.args(["-hide_banner", "-filters"]);
    match crate::tools::output(&mut c).await {
        Ok(o) => String::from_utf8_lossy(&o.stdout).lines().any(|l| l.split_whitespace().nth(1) == Some(name)),
        Err(_) => false,
    }
}

/// 字幕烧录（硬字幕）。只接受带时间轴的字幕（SRT / VTT / ASS）。
pub async fn burn_subtitles(tools: &Toolbox, video: &Path, subs: &Path, on: impl FnMut(f64)) -> Result<PathBuf> {
    let sext = subs.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if !matches!(sext.as_str(), "srt" | "vtt" | "ass" | "ssa") {
        bail!(tr!("TXT 字幕没有时间轴，没法嵌进画面，换成 SRT 或 VTT"));
    }
    if !has_filter(tools, "subtitles").await {
        bail!(tr!("当前的 ffmpeg 不支持把字幕嵌进画面（缺少 libass），换一个完整版的 ffmpeg 后再试"));
    }
    let info = probe(tools, video).await?;
    let out = out_path(video, None, &format!(" {}", tr!("(字幕)§烧录了字幕的视频的文件名后缀：「Sintel (字幕).mp4」")), "mp4");
    // subtitles 滤镜的参数要转义，文件名里有引号、冒号时很容易出错：先复制到一个名字干净的临时文件
    let tmp = std::env::temp_dir().join(format!("captae-burn-{}.{sext}", crate::util::now_ms()));
    tokio::fs::copy(subs, &tmp).await.context(tr!("读取字幕文件失败"))?;
    let esc = tmp.display().to_string().replace('\\', "/").replace(':', "\\:").replace('\'', "\\'");
    let mid: Vec<String> = ["-vf", &format!("subtitles='{esc}'"), "-c:v", "libx264", "-crf", "20", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "192k", "-movflags", "+faststart"].iter().map(|s| s.to_string()).collect();
    let r = Job { tools, duration: info.duration, extra: vec![] }.run(&["-i".into(), video.display().to_string()], &mid, &out, on).await;
    let _ = tokio::fs::remove_file(&tmp).await;
    r?;
    Ok(out)
}

/// 剪辑面板的胶片条：均匀取 `n` 帧小图，存在 `dir` 里（已经有的不重复取）。
pub async fn strip_frames(tools: &Toolbox, src: &Path, duration: f64, dir: &Path, n: usize) -> Result<Vec<PathBuf>> {
    tokio::fs::create_dir_all(dir).await?;
    let n = n.clamp(1, 30);
    let jobs = (0..n).map(|k| {
        let p = dir.join(format!("{k:02}.jpg"));
        let t = duration * (k as f64 + 0.5) / n as f64;
        async move {
            if p.exists() {
                return Some(p);
            }
            let mut c = tools.command(Tool::Ffmpeg).ok()?;
            c.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-ss", &format!("{t:.2}"), "-i"]).arg(src).args(["-frames:v", "1", "-vf", "scale=-2:120", "-q:v", "5"]).arg(&p);
            run_ok(c).await.ok()?;
            p.exists().then_some(p)
        }
    });
    let v: Vec<PathBuf> = futures_util::future::join_all(jobs).await.into_iter().flatten().collect();
    if v.is_empty() {
        bail!(tr!("没能从视频里取出画面"));
    }
    Ok(v)
}

/// 声音波形：把音频解成 4kHz 单声道，按 `n` 段取峰值，归一化到 0..1。
pub async fn peaks(tools: &Toolbox, src: &Path, n: usize) -> Result<Vec<f32>> {
    let n = n.clamp(8, 1000);
    let mut c = tools.command(Tool::Ffmpeg)?;
    c.args(["-hide_banner", "-nostdin", "-loglevel", "error", "-i"]).arg(src).args(["-vn", "-ac", "1", "-ar", "4000", "-f", "s16le", "-"]);
    let raw = run_ok(c).await.context(tr!("读取声音失败"))?;
    Ok(bucket_peaks(&raw, n))
}

fn bucket_peaks(raw: &[u8], n: usize) -> Vec<f32> {
    let samples: Vec<i16> = raw.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
    if samples.is_empty() {
        return vec![0.0; n];
    }
    let mut v: Vec<f32> = (0..n)
        .map(|k| {
            let a = samples.len() * k / n;
            let b = (samples.len() * (k + 1) / n).max(a + 1).min(samples.len());
            samples[a..b].iter().map(|x| (*x as i32).unsigned_abs()).max().unwrap_or(0) as f32 / 32768.0
        })
        .collect();
    let max = v.iter().cloned().fold(0.0f32, f32::max);
    if max > 0.0 {
        v.iter_mut().for_each(|x| *x = (*x / max).clamp(0.0, 1.0));
    }
    v
}

/// 缩略图：视频取 10% 处一帧，图片直接缩小。
pub async fn thumbnail(tools: &Toolbox, src: &Path, kind: &str, duration: Option<f64>, out: &Path) -> Result<()> {
    let mut c = tools.command(Tool::Ffmpeg)?;
    c.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error"]);
    if kind == "video" {
        let t = duration.map(|d| (d * 0.1).min(30.0)).unwrap_or(1.0);
        c.args(["-ss", &format!("{t:.2}")]);
    }
    c.arg("-i").arg(src).args(["-frames:v", "1", "-vf", "scale=960:-2", "-q:v", "4"]).arg(out);
    run_ok(c).await?;
    Ok(())
}

/// 「发给 AI」最多截几张。Claude 网页版一条消息最多 20 个文件，再多粘贴不进去。
pub const MAX_FRAMES: usize = 20;

/// 没指定张数时截几张：大约每 20 秒一张，最少 6 张、最多 12 张。
/// 粘贴给 AI 时图片张数有上限：Claude 网页版 20 个；ChatGPT 官方没写，网上的说法从 9、10 到 20 都有。12 张是两边的折中，
/// 要更多时 AI 助手可以传 max_frames（最多 MAX_FRAMES）。
pub fn auto_frames(duration: Option<f64>) -> usize {
    match duration {
        Some(d) if d > 0.0 => ((d / 20.0).round() as usize).clamp(6, 12),
        _ => 8,
    }
}

/// 扫画面时的一帧：时间、和上一帧比的变化分数（0–1，ffmpeg 的 scene）、平均亮度（0–255）
#[derive(Debug, Clone, Copy, PartialEq)]
struct Sample {
    t: f64,
    scene: f64,
    luma: f64,
}

/// 变化分数到这个数算一次换镜头
const CUT: f64 = 0.25;
/// 换镜头以后等这么久再截：避开转场、淡入淡出的半透明画面和动态模糊
const SETTLE: f64 = 0.5;

/// 太黑或太白（黑场、闪白、淡入淡出的头尾）的画面不要
fn usable(s: &Sample) -> bool {
    s.luma > 16.0 && s.luma < 240.0
}

/// 每秒 4 帧、缩到 160 宽扫一遍，记下每帧的变化分数和亮度。解码整段视频，长视频要十几秒
async fn scan(tools: &Toolbox, src: &Path) -> Result<Vec<Sample>> {
    let mut c = tools.command(Tool::Ffmpeg)?;
    c.args(["-hide_banner", "-nostdin", "-loglevel", "error", "-i"]).arg(src).args([
        "-an",
        "-sn",
        "-dn",
        "-vf",
        "fps=4,scale=160:-2,format=yuv420p,select='gte(scene,0)',signalstats,metadata=print:file=-",
        "-f",
        "null",
        "-",
    ]);
    // 经过 tools::output：取消、退出时连同子进程一起结束
    let out = crate::tools::output(&mut c).await?;
    Ok(parse_scan(&String::from_utf8_lossy(&out.stdout)))
}

/// metadata=print 的输出：每帧一行 `frame:… pts_time:…`，下面跟着 `lavfi.scene_score=…`、`lavfi.signalstats.YAVG=…`
fn parse_scan(out: &str) -> Vec<Sample> {
    let mut v: Vec<Sample> = vec![];
    for l in out.lines() {
        if let Some(t) = l.split("pts_time:").nth(1).and_then(|s| s.split_whitespace().next()).and_then(|s| s.parse().ok()) {
            v.push(Sample { t, scene: 0.0, luma: 128.0 });
        } else if let Some(cur) = v.last_mut() {
            if let Some(x) = l.strip_prefix("lavfi.scene_score=").and_then(|s| s.trim().parse().ok()) {
                cur.scene = x;
            } else if let Some(x) = l.strip_prefix("lavfi.signalstats.YAVG=").and_then(|s| s.trim().parse().ok()) {
                cur.luma = x;
            }
        }
    }
    v
}

/// 挑 `n` 个时间点：整段按时长平均分成 n 段，每段截一张，保证从头到尾都照顾到（原来只取最前面的几次换镜头，长视频后半段一张都没有）。
/// 段里有换镜头的，取变化最大的那一次、等画面稳定（SETTLE）以后截，这一张最能代表这一段的新内容；
/// 没有换镜头的（一个人一直在讲），取这一段中间。截到的画面太黑太白时，换成这一段里离得最近的正常画面。
fn pick_times(samples: &[Sample], n: usize, duration: f64) -> Vec<f64> {
    let dur = if duration > 0.0 { duration } else { samples.last().map(|s| s.t + 0.25).unwrap_or(0.0) };
    if n == 0 || dur <= 0.0 {
        return vec![];
    }
    let mut out = vec![];
    for i in 0..n {
        let (a, b) = (dur * i as f64 / n as f64, dur * (i + 1) as f64 / n as f64);
        let seg: Vec<&Sample> = samples.iter().filter(|s| s.t >= a && s.t < b).collect();
        let cut = seg.iter().filter(|s| s.scene >= CUT).max_by(|x, y| x.scene.total_cmp(&y.scene));
        let mut t = match cut {
            Some(c) => {
                // 别跨过下一次换镜头，也别超出这一段
                let next = samples.iter().find(|s| s.t > c.t && s.scene >= CUT).map(|s| s.t).unwrap_or(f64::MAX);
                (c.t + SETTLE).min(next - 0.1).min(b - 0.05).max(c.t)
            }
            None => (a + b) / 2.0,
        };
        let near = |t: f64| seg.iter().min_by(|x, y| (x.t - t).abs().total_cmp(&(y.t - t).abs())).copied();
        if let Some(s) = near(t).filter(|s| !usable(s)) {
            if let Some(g) = seg.iter().filter(|x| usable(x)).min_by(|x, y| (x.t - s.t).abs().total_cmp(&(y.t - s.t).abs())) {
                t = g.t;
            }
        }
        out.push(t.clamp(0.0, (dur - 0.05).max(0.0)));
    }
    out
}

/// 关键画面：从头到尾平均分段，每段截一张最能代表它的画面（见 pick_times），一共 `max` 张。
/// 长边最多 1280（不放大小视频），JPEG。扫不了画面（读不了、没有画面）时按时长平均截，仍然截不到就返回空的。
pub async fn keyframes(tools: &Toolbox, src: &Path, dir: &Path, max: usize, duration: Option<f64>) -> Result<Vec<(f64, PathBuf)>> {
    tokio::fs::create_dir_all(dir).await?;
    let samples = match scan(tools, src).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("keyframes: scan failed: {e}");
            vec![]
        }
    };
    if samples.is_empty() && !duration.is_some_and(|d| d > 0.0) {
        return Ok(vec![]);
    }
    let times = pick_times(&samples, max, duration.unwrap_or(0.0));
    let mut frames = vec![];
    for (i, t) in times.into_iter().enumerate() {
        let p = dir.join(format!("frame-{:02}.jpg", i + 1));
        let mut c = tools.command(Tool::Ffmpeg)?;
        // -ss 放在 -i 前面：先跳到附近的关键帧再精确解到 t，长视频也快
        c.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-ss", &format!("{t:.3}"), "-i"]).arg(src).args([
            "-frames:v",
            "1",
            "-vf",
            "scale='if(gte(iw,ih),min(1280,iw),-2)':'if(gte(iw,ih),-2,min(1280,ih))'",
            "-q:v",
            "3",
        ]);
        c.arg(&p);
        if run_ok(c).await.is_ok() && p.exists() {
            frames.push((t, p));
        }
    }
    Ok(frames)
}

/// 给语音识别用：16kHz 单声道 WAV。
/// 文件的容器（ffprobe 的 format_name：mpegts、mov,mp4,m4a,3gp,3g2,mj2、matroska,webm……）。读不了是 None
pub async fn container(tools: &Toolbox, p: &Path) -> Option<String> {
    let mut c = tools.command(Tool::Ffprobe).ok()?;
    c.args(["-v", "error", "-show_entries", "format=format_name", "-of", "default=nw=1:nk=1"]).arg(p);
    let out = run_ok(c).await.ok()?;
    Some(String::from_utf8_lossy(&out).trim().to_string()).filter(|s| !s.is_empty())
}

/// 录直播停下来以后，把录到的原始数据原样复制（`-c copy`，不重新编码）成一个能正常播放、能拖动的文件，放进 `dir`：
/// yt-dlp 交给 ffmpeg 录的 MPEG-TS、Captae 自己的 M3U8 引擎接起来的分段、电台流（没有文件头的 MP3 / AAC），
/// 以及被直接结束、没写完的文件（最后一点残缺的数据丢掉）。`audio` 是单独录的另一路声音（M3U8 声音单独一路时）。
/// 扩展名按内容定：有画面 .mp4，MP3 .mp3，AAC .m4a，其他声音 .mka；装不进 MP4 的少见编码改成 .mkv。
pub async fn tidy_recording(tools: &Toolbox, raw: &Path, audio: Option<&Path>, dir: &Path, stem: &str) -> Result<PathBuf> {
    // 没有 ffmpeg（手机上）：M3U8 引擎录的 TS / fMP4 用 Rust 封装（engine/remux.rs）
    if !tools.has(Tool::Ffmpeg) {
        return crate::engine::remux::tidy_recording(raw, audio, dir, stem).await;
    }
    let p = probe(tools, raw).await.context(tr!("录到的部分读不了"))?;
    let video = p.vcodec.is_some() && p.width.unwrap_or(0) > 0;
    let acodec = match audio {
        Some(a) => probe(tools, a).await.ok().and_then(|x| x.acodec),
        None => p.acodec.clone(),
    };
    if !video && acodec.is_none() {
        bail!(tr!("录到的部分里没有画面也没有声音"));
    }
    let aac = acodec.as_deref() == Some("aac");
    let ext = if video {
        "mp4"
    } else {
        match acodec.as_deref() {
            Some("mp3") => "mp3",
            Some("aac") => "m4a",
            _ => "mka",
        }
    };
    tokio::fs::create_dir_all(dir).await?;
    let stem = crate::util::safe_name(stem);
    let stem = if stem.trim().is_empty() { tr!("录制§录直播得到的文件没有标题时的文件名") } else { stem };
    // 先按内容试一次（AAC 进 MP4 要转一下声音的封装），不行去掉转换再试，再不行装进 Matroska（什么编码都装得下）
    let mut tries: Vec<(&str, bool)> = vec![(ext, aac && matches!(ext, "mp4" | "m4a"))];
    if aac && matches!(ext, "mp4" | "m4a") {
        tries.push((ext, false));
    }
    tries.push((if video { "mkv" } else { "mka" }, false));
    let mut last = String::new();
    for (ext, bsf) in tries {
        let out = crate::util::unique_path(dir.join(format!("{stem}.{ext}")));
        let guard = crate::tasks::Partial::new(&out);
        let mut c = tools.command(Tool::Ffmpeg)?;
        c.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-fflags", "+genpts+discardcorrupt", "-i"]).arg(raw);
        match audio {
            Some(a) => {
                c.arg("-i").arg(a).args(["-map", "0:v:0?", "-map", "1:a:0"]);
            }
            None if video => {
                c.args(["-map", "0:v:0", "-map", "0:a:0?"]);
            }
            None => {
                c.args(["-map", "0:a:0"]);
            }
        }
        c.args(["-c", "copy"]);
        if bsf {
            c.args(["-bsf:a", "aac_adtstoasc"]);
        }
        if matches!(ext, "mp4" | "m4a") {
            c.args(["-movflags", "+faststart"]);
        }
        c.arg(&out);
        let r = crate::tools::run_full(c, std::time::Duration::from_secs(3600)).await?;
        if r.ok && probe(tools, &out).await.is_ok_and(|q| q.duration.unwrap_or(0.0) > 0.0) {
            guard.keep();
            return Ok(out);
        }
        last = r.stderr.lines().rev().take(3).collect::<Vec<_>>().join(" / ");
    }
    bail!(tr!("录到的部分整理不成能播放的文件：{error}", error = last))
}

pub async fn to_wav16k(tools: &Toolbox, src: &Path, out: &Path) -> Result<()> {
    let mut c = tools.command(Tool::Ffmpeg)?;
    c.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-i"]).arg(src).args(["-vn", "-ac", "1", "-ar", "16000", "-c:a", "pcm_s16le"]).arg(out);
    run_ok(c).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn smp(t: f64, scene: f64, luma: f64) -> Sample {
        Sample { t, scene, luma }
    }
    #[test]
    fn scan_output() {
        let out = "frame:0    pts:0       pts_time:0\nlavfi.scene_score=0.000000\nlavfi.signalstats.YAVG=125.305\nframe:1    pts:1       pts_time:0.25\nlavfi.scene_score=0.812\nlavfi.signalstats.YAVG=3.5\n";
        assert_eq!(parse_scan(out), vec![smp(0.0, 0.0, 125.305), smp(0.25, 0.812, 3.5)]);
    }
    #[test]
    fn frame_count() {
        assert_eq!(auto_frames(Some(70.0)), 6);
        assert_eq!(auto_frames(Some(200.0)), 10);
        assert_eq!(auto_frames(Some(3600.0)), 12);
        assert_eq!(auto_frames(None), 8);
    }
    #[test]
    fn picks_cover_whole_video_and_follow_cuts() {
        // 40 秒，每 0.25 秒一帧；5 秒、35 秒各换一次镜头；10–12 秒是黑场
        let s: Vec<Sample> = (0..160)
            .map(|i| {
                let t = i as f64 * 0.25;
                let scene = if t == 5.0 { 0.6 } else if t == 35.0 { 0.9 } else { 0.01 };
                let luma = if (10.0..12.0).contains(&t) { 2.0 } else { 120.0 };
                smp(t, scene, luma)
            })
            .collect();
        let t = pick_times(&s, 4, 40.0);
        assert_eq!(t.len(), 4);
        // 第一段（0–10）有换镜头：换完等 0.5 秒
        assert_eq!(t[0], 5.5);
        // 第二段（10–20）没有换镜头，中间 15 秒
        assert_eq!(t[1], 15.0);
        assert_eq!(t[2], 25.0);
        assert_eq!(t[3], 35.5);
        // 每段都有一张，从头到尾都照顾到（原来只取最前面的换镜头）
        assert!(t.windows(2).all(|w| w[0] < w[1]));
    }
    #[test]
    fn picks_skip_black() {
        // 中间那一段正中间是黑场：换成这一段里最近的正常画面
        let s: Vec<Sample> = (0..120).map(|i| { let t = i as f64 * 0.25; smp(t, 0.0, if (14.0..16.5).contains(&t) { 3.0 } else { 100.0 }) }).collect();
        let t = pick_times(&s, 3, 30.0);
        assert_eq!(t[1], 13.75);
        // 扫不到画面时按时长平均分
        assert_eq!(pick_times(&[], 2, 10.0), vec![2.5, 7.5]);
    }
    #[test]
    fn progress() {
        assert_eq!(parse_progress_time("out_time_us=1500000"), Some(1.5));
        assert_eq!(parse_progress_time("frame=10"), None);
    }
    #[test]
    fn targets() {
        assert_eq!(target("ProRes 422").unwrap().0, "mov");
        assert_eq!(target("MP3").unwrap().0, "mp3");
        assert!(target("xyz").is_err());
    }
    #[test]
    fn probe_parse() {
        let v: Value = serde_json::json!({
            "streams":[{"codec_type":"video","codec_name":"h264","width":1920,"height":818},{"codec_type":"audio","codec_name":"aac"}],
            "format":{"duration":"888.0","size":"612000000"}
        });
        let p = parse_probe(&v, Path::new("/a/Sintel.mp4"));
        assert_eq!((p.kind.as_str(), p.width, p.height, p.format.as_str()), ("video", Some(1920), Some(818), "MP4"));
        assert!(can_remux(&p, "MOV"));
        assert!(!can_remux(&p, "WebM"));
    }
    #[test]
    fn rates() {
        assert!((parse_rate("30000/1001").unwrap() - 29.97).abs() < 0.01);
        assert_eq!(parse_rate("25/1"), Some(25.0));
        assert_eq!(parse_rate("0/0"), None);
        let v: Value = serde_json::json!({"streams":[{"codec_type":"video","codec_name":"h264","width":640,"height":360,"avg_frame_rate":"25/1"},{"codec_type":"audio","codec_name":"aac","sample_rate":"44100","channels":2}],"format":{"duration":"6.0"}});
        let p = parse_probe(&v, Path::new("/a/x.mp4"));
        assert_eq!((p.fps, p.sample_rate, p.channels), (Some(25.0), Some(44100), Some(2)));
    }
    #[test]
    fn quality_per_codec() {
        let arg = |f: &str, q: Quality, k: &str| {
            let (_, v) = target_with(f, q).unwrap();
            v.iter().position(|x| x == k).map(|i| v[i + 1].clone())
        };
        assert_eq!(arg("MP4", Quality::High, "-crf").as_deref(), Some("16"));
        assert_eq!(arg("MP4", Quality::Same, "-crf").as_deref(), Some("20"));
        // ProRes 没有 crf，换档位
        assert_eq!(arg("ProRes 422", Quality::Small, "-crf"), None);
        assert_eq!(arg("ProRes 422", Quality::Small, "-profile:v").as_deref(), Some("1"));
        assert_eq!(arg("ProRes 422", Quality::High, "-profile:v").as_deref(), Some("3"));
        assert_eq!(arg("WebM", Quality::Small, "-crf").as_deref(), Some("40"));
        assert_eq!(arg("MP3", Quality::High, "-q:a").as_deref(), Some("0"));
        assert_eq!(arg("AAC", Quality::Small, "-b:a").as_deref(), Some("128k"));
        assert_eq!(arg("FLAC", Quality::Small, "-crf"), None);
        assert_eq!(arg("MKV", Quality::High, "-c:v").as_deref(), Some("libx264"));
    }
    #[test]
    fn convert_naming() {
        let p = convert_out_path(Path::new("/nonexistent/a/Sintel.mp4"), "MP4", None).unwrap();
        assert_eq!(p, Path::new("/nonexistent/a/Sintel (转换).mp4"));
        let p = convert_out_path(Path::new("/nonexistent/a/Sintel.mp4"), "ProRes 422", None).unwrap();
        assert_eq!(p, Path::new("/nonexistent/a/Sintel.mov"));
    }
    #[test]
    fn concat_check() {
        let v = |w: u32, fps: f64, a: bool| Probe { kind: "video".into(), vcodec: Some("h264".into()), width: Some(w), height: Some(360), fps: Some(fps), has_audio: a, acodec: a.then(|| "aac".into()), sample_rate: a.then_some(44100), channels: a.then_some(2), ..Default::default() };
        assert!(concat_compatible(&[v(640, 25.0, true), v(640, 25.0, true)]));
        assert!(!concat_compatible(&[v(640, 25.0, true), v(320, 25.0, true)]));
        assert!(!concat_compatible(&[v(640, 25.0, true), v(640, 30.0, true)]));
        assert!(!concat_compatible(&[v(640, 25.0, true), v(640, 25.0, false)]));
    }
    #[test]
    fn trim_labels() {
        assert_eq!(trim_label(0.0), "0.00");
        assert_eq!(trim_label(2.8), "0.02,8");
        assert_eq!(trim_label(83.4), "1.23,4");
        assert_eq!(trim_label(3723.0), "1.02.03");
    }
    /// 第三阶段 FIXES 第 15 项（主线程补充）：素材库里取消一个正在跑的转换（Tauri 的 task_cancel → cancel_task），
    /// ffmpeg 和它起的子进程都不能留下，没写完的文件也删掉（tasks::Partial）
    #[tokio::test]
    async fn cancel_conversion_leaves_no_ffmpeg() {
        use crate::tools::testkit;
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let pidf = dir.path().join("ffmpeg.pid");
        // 假的 ffprobe：一个 10 秒的 H.264 视频；假的 ffmpeg：往输出文件写一点，再起一个子进程一直等
        let json = r#"{"streams":[{"codec_type":"video","codec_name":"h264","width":640,"height":360}],"format":{"duration":"10.0","size":"1000"}}"#;
        testkit::fake(&bin, "ffprobe", &format!("echo '{json}'"), &format!("Write-Output '{json}'"));
        let ff = testkit::fake(
            &bin,
            "ffmpeg",
            &format!("for a; do last=\"$a\"; done\necho partial > \"$last\"\n{}", testkit::sh_wait_child(&pidf)),
            &format!("[IO.File]::WriteAllText($args[-1], 'partial')\n{}", testkit::ps_wait_child(&pidf)),
        );
        let paths = crate::config::Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        let core = crate::Core::open(crate::Options { paths, sidecar_dir: None, restore: false }).await.unwrap();
        core.patch_settings(&serde_json::json!({"ffmpegPath": ff.display().to_string()})).unwrap();
        let src = dir.path().join("in.mov");
        std::fs::write(&src, b"x").unwrap();
        let out = convert_out_path(&src, "MP4", None).unwrap();
        let id = core.new_task_id();
        let (c2, tools, src2) = (core.clone(), core.toolbox(), src.clone());
        let run = tokio::spawn(async move {
            c2.run_task_as(id, "convert", "in.mov → MP4".into(), None, |_rep| async move {
                let p = convert(&tools, &src2, "MP4", None, vec![], |_| {}).await?;
                Ok(serde_json::json!({"path": p}))
            })
            .await
        });
        let pids = testkit::read_pids(&pidf).await;
        assert!(out.exists(), "转换开始写输出了");
        assert!(core.cancel_task(id));
        let r = tokio::time::timeout(std::time::Duration::from_secs(5), run).await.expect("取消要马上生效").unwrap();
        assert!(r.is_err());
        for pid in pids {
            assert!(testkit::gone(pid).await, "取消以后进程 {pid} 还在");
        }
        assert!(!out.exists(), "没写完的文件要删掉");
    }

    #[test]
    fn wave_peaks() {
        let raw: Vec<u8> = [0i16, 100, -200, 50, 16000, -8000, 0, 0].iter().flat_map(|x| x.to_le_bytes()).collect();
        let p = bucket_peaks(&raw, 4);
        assert_eq!(p.len(), 4);
        assert_eq!(p[2], 1.0);
        assert!((p[1] - 200.0 / 16000.0).abs() < 1e-4);
        assert_eq!(p[3], 0.0);
        assert_eq!(bucket_peaks(&[], 3), vec![0.0; 3]);
    }
}
