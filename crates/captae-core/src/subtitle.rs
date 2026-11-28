//! 字幕：两条路径。
//! - 平台字幕：yt-dlp `--write-subs --write-auto-subs`，速度快、准确
//! - 本地识别：faster-whisper（参考 Buzz 的做法，以 sidecar 进程运行，模型按需下载）
//!
//! 导出 SRT / VTT / TXT；可选调用 OpenAI 兼容接口（本地 Ollama 或远程）翻译；
//! 烧录见 `ffmpeg::burn_subtitles`。识别出的文字写回素材库，参与全文检索。

use crate::queue::Core;
use crate::tools::Tool;
use crate::tr;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, BufReader};

pub const WHISPER_SCRIPT: &str = include_str!("../../../scripts/captae_whisper.py");

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// 先试平台字幕，没有再本地识别
    #[default]
    Auto,
    Platform,
    Whisper,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct SubOpts {
    pub source: Source,
    /// 语言代码，如 zh / en；None 表示自动
    pub language: Option<String>,
    /// srt / vtt / txt
    pub format: String,
    /// 翻译成哪种语言（需要在设置里配置翻译接口）
    pub translate_to: Option<String>,
    pub model: Option<String>,
}

impl Default for SubOpts {
    fn default() -> Self {
        SubOpts { source: Source::Auto, language: None, format: "srt".into(), translate_to: None, model: None }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubResult {
    pub path: PathBuf,
    pub source: &'static str,
    pub language: Option<String>,
    pub cues: usize,
    /// 字幕内容本身（烧录时用，不管导出的是什么格式）
    #[serde(skip)]
    pub list: Vec<Cue>,
}

/// 网站上能下载的一条字幕。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    /// yt-dlp 的语言代码，如 en、zh-Hans、en-orig
    pub code: String,
    /// 界面上显示的名字，如 English、中文（简体）
    pub name: String,
    /// 网站自动生成的字幕
    pub auto: bool,
}

/// 从 yt-dlp `-J` 的信息里读出字幕列表。人工字幕全列；自动字幕只列原始语言
/// （YouTube 会把自动字幕机翻成上百种语言，那些不列）。
pub fn tracks_from_info(v: &serde_json::Value) -> Vec<Track> {
    let skip = |k: &str| matches!(k, "live_chat" | "danmaku" | "rechat" | "comments");
    let name_of = |k: &str, e: &serde_json::Value| {
        e.as_array()
            .and_then(|a| a.iter().find_map(|x| x.get("name").and_then(serde_json::Value::as_str)))
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| k.to_string())
    };
    let mut out: Vec<Track> = vec![];
    if let Some(m) = v.get("subtitles").and_then(serde_json::Value::as_object) {
        for (k, e) in m {
            if !skip(k) {
                out.push(Track { code: k.clone(), name: name_of(k, e), auto: false });
            }
        }
    }
    let base = |c: &str| c.split(['-', '_']).next().unwrap_or(c).to_ascii_lowercase();
    if let Some(m) = v.get("automatic_captions").and_then(serde_json::Value::as_object) {
        let orig: Vec<&String> = m.keys().filter(|k| k.ends_with("-orig")).collect();
        let keys: Vec<&String> = if !orig.is_empty() {
            orig
        } else if m.len() <= 6 {
            m.keys().collect()
        } else {
            vec![]
        };
        for k in keys {
            if skip(k) || out.iter().any(|t| !t.auto && base(&t.code) == base(k)) {
                continue;
            }
            let n = name_of(k, &m[k]);
            let n = n.trim_end_matches(" (Original)").trim_end_matches("（原始）").to_string();
            // 名字不带「（自动生成）」：界面按 auto 自己加（跟着界面语言）
            out.push(Track { code: k.clone(), name: n, auto: true });
        }
    }
    out.sort_by(|a, b| (a.auto, &a.code).cmp(&(b.auto, &b.code)));
    out
}

/// 问 yt-dlp 这个网页有哪些字幕（不下载视频）。
pub async fn tracks(core: &Core, url: &str) -> Result<Vec<Track>> {
    if !url.starts_with("http") {
        return Ok(vec![]);
    }
    let mut c = core.toolbox().command(Tool::YtDlp)?;
    c.args(["-J", "--skip-download", "--no-playlist", "--no-warnings"]).arg(url);
    let out = tokio::time::timeout(std::time::Duration::from_secs(90), crate::tools::run_ok(c)).await.map_err(|_| anyhow!(tr!("读取网站信息超时了")))??;
    let v: serde_json::Value = serde_json::from_slice(&out).context(tr!("网站信息无法解析"))?;
    Ok(tracks_from_info(&v))
}

/// 本地识别组件（faster-whisper）装好了没有。
pub async fn whisper_available(tools: &crate::tools::Toolbox) -> bool {
    match tools.command(Tool::Python) {
        Ok(mut c) => crate::tools::output(c.args(["-c", "import faster_whisper"])).await.map(|o| o.status.success()).unwrap_or(false),
        Err(_) => false,
    }
}

/// 把字幕烧进画面。不管导出的是 SRT、VTT 还是 TXT，都先写一份临时 SRT 给 ffmpeg 用。
pub async fn burn(core: &Core, video: &Path, cues: &[Cue], on: impl FnMut(f64)) -> Result<PathBuf> {
    if cues.is_empty() {
        bail!(tr!("没有字幕内容，没法嵌进画面"));
    }
    tokio::fs::create_dir_all(&core.paths.work).await?;
    crate::tasks::sweep_scratch(&core.paths.work);
    let tmp = core.paths.work.join(format!("burn-{}.srt", crate::util::now_ms()));
    // 做完、出错、被取消都删
    let _tmp = crate::tasks::Scratch::new(&tmp);
    tokio::fs::write(&tmp, to_srt(cues)).await?;
    crate::ffmpeg::burn_subtitles(&core.toolbox(), video, &tmp, on).await
}

fn ts(t: f64, sep: char) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as u64;
    format!("{:02}:{:02}:{:02}{sep}{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

pub fn to_srt(c: &[Cue]) -> String {
    c.iter().enumerate().map(|(i, c)| format!("{}\n{} --> {}\n{}\n\n", i + 1, ts(c.start, ','), ts(c.end, ','), c.text)).collect()
}

pub fn to_vtt(c: &[Cue]) -> String {
    let mut s = String::from("WEBVTT\n\n");
    for c in c {
        s.push_str(&format!("{} --> {}\n{}\n\n", ts(c.start, '.'), ts(c.end, '.'), c.text));
    }
    s
}

pub fn to_txt(c: &[Cue]) -> String {
    c.iter().map(|c| c.text.as_str()).collect::<Vec<_>>().join("\n")
}

fn parse_ts(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    let (h, m, sec) = match parts.len() {
        3 => (parts[0].parse::<f64>().ok()?, parts[1].parse::<f64>().ok()?, parts[2].parse::<f64>().ok()?),
        2 => (0.0, parts[0].parse::<f64>().ok()?, parts[1].parse::<f64>().ok()?),
        _ => return None,
    };
    Some(h * 3600.0 + m * 60.0 + sec)
}

/// 读 SRT 或 VTT。
pub fn parse(text: &str) -> Vec<Cue> {
    let mut out = vec![];
    let norm = text.replace("\r\n", "\n");
    for block in norm.split("\n\n") {
        let lines: Vec<&str> = block.lines().filter(|l| !l.trim().is_empty()).collect();
        let Some(k) = lines.iter().position(|l| l.contains("-->")) else { continue };
        let (a, b) = lines[k].split_once("-->").unwrap();
        let b = b.split_whitespace().next().unwrap_or("");
        let (Some(start), Some(end)) = (parse_ts(a), parse_ts(b)) else { continue };
        let text = lines[k + 1..].iter().map(|l| strip_tags(l)).collect::<Vec<_>>().join("\n");
        if text.trim().is_empty() {
            continue;
        }
        // YouTube 自动字幕会逐词滚动重复，去掉和上一条相同的
        if out.last().map(|c: &Cue| c.text == text).unwrap_or(false) {
            out.last_mut().unwrap().end = end;
            continue;
        }
        out.push(Cue { start, end, text });
    }
    out
}

fn strip_tags(s: &str) -> String {
    let mut o = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' if depth > 0 => depth -= 1,
            c if depth == 0 => o.push(c),
            _ => {}
        }
    }
    o.trim().to_string()
}

pub fn render(c: &[Cue], fmt: &str) -> Result<(String, &'static str)> {
    Ok(match fmt.to_ascii_lowercase().as_str() {
        "srt" => (to_srt(c), "srt"),
        "vtt" => (to_vtt(c), "vtt"),
        "txt" => (to_txt(c), "txt"),
        f => bail!(tr!("不支持的字幕格式 {format}", format = f)),
    })
}

/// 平台字幕。返回 (字幕, 语言)。
pub async fn platform(core: &Core, url: &str, lang: Option<&str>, work: &Path) -> Result<(Vec<Cue>, String)> {
    if url.is_empty() {
        bail!(tr!("这个素材没有来源链接，没法下载网站字幕"));
    }
    tokio::fs::create_dir_all(work).await?;
    // 下载下来的字幕文件：读完、出错（比如这个视频没有字幕）、被取消都删
    let _work = crate::tasks::Scratch::new(work);
    // 短代码（zh、en）匹配这门语言的所有变体；从字幕列表里选的完整代码（en-orig、zh-Hans）只下那一条
    let langs = match lang {
        Some(l) if l.len() <= 3 => format!("{l}.*,{l}"),
        Some(l) => l.to_string(),
        None => "zh.*,en.*,ja.*".into(),
    };
    let mut c = core.toolbox().command(Tool::YtDlp)?;
    c.args(["--skip-download", "--write-subs", "--write-auto-subs", "--no-playlist", "--sub-format", "vtt/srt/best", "--sub-langs", &langs, "-P"])
        .arg(work)
        .args(["-o", "sub.%(ext)s"])
        .arg(url);
    crate::tools::run_ok(c).await.context(tr!("下载网站字幕失败"))?;
    let mut best: Option<(PathBuf, String)> = None;
    let mut rd = tokio::fs::read_dir(work).await?;
    while let Some(e) = rd.next_entry().await? {
        let n = e.file_name().to_string_lossy().to_string();
        if let Some(rest) = n.strip_prefix("sub.") {
            let mut parts = rest.rsplitn(2, '.');
            let _ext = parts.next();
            let l = parts.next().unwrap_or("").to_string();
            // 和要的语言完全一样的优先；否则优先不带后缀的（YouTube 机翻字幕带 -xx 后缀）
            let exact = lang.map(|x| x == l).unwrap_or(false);
            let cur_exact = best.as_ref().map(|(_, b)| lang.map(|x| x == b).unwrap_or(false)).unwrap_or(false);
            if best.is_none() || exact || (!cur_exact && !l.contains('-')) {
                best = Some((e.path(), l));
            }
        }
    }
    let (p, l) = best.ok_or_else(|| anyhow!(tr!("这个视频在网站上没有字幕")))?;
    let cues = parse(&tokio::fs::read_to_string(&p).await?);
    if cues.is_empty() {
        bail!(tr!("网站字幕是空的"));
    }
    Ok((cues, l))
}

/// 本地识别。
pub async fn whisper(core: &Core, media: &Path, lang: Option<&str>, model: &str, mut on: impl FnMut(f64)) -> Result<(Vec<Cue>, String)> {
    // 模型还没下载：先带进度下完（占前 40%），不然识别会在 0% 停好几分钟
    let split = if core.whisper_model_ready(model) { 0.0 } else { 0.4 };
    if split > 0.0 {
        core.download_whisper_model(model, |d, t| on(split * (d / t).min(1.0))).await?;
    }
    let mut on = move |x: f64| on(split + (1.0 - split) * x);
    let tools = core.toolbox();
    let work = core.paths.work.join(format!("whisper-{}", crate::util::now_ms()));
    tokio::fs::create_dir_all(&work).await?;
    // 转出来的音频（长视频有几百 MB）：识别完、出错、被取消都删
    let _work = crate::tasks::Scratch::new(&work);
    let wav = work.join("audio.wav");
    crate::ffmpeg::to_wav16k(&tools, media, &wav).await?;
    let script = core.paths.data.join("captae_whisper.py");
    if tokio::fs::read_to_string(&script).await.ok().as_deref() != Some(WHISPER_SCRIPT) {
        tokio::fs::write(&script, WHISPER_SCRIPT).await?;
    }
    let mut c = tools.command(Tool::Python)?;
    c.arg(&script).arg(&wav).args(["--model", model, "--models-dir"]).arg(core.paths.data.join("models"));
    if let Some(l) = lang {
        c.args(["--language", l]);
    }
    c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    // tools::spawn：取消、超时时（整个 future 被丢掉）Python 和它起的子进程一起结束，不留在后台继续识别
    let mut child = crate::tools::spawn(&mut c).context(tr!("启动语音识别失败"))?;
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    let mut cues = vec![];
    let mut language = lang.unwrap_or("").to_string();
    let mut err: Option<String> = None;
    while let Some(l) = lines.next_line().await? {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(&l) else { continue };
        match v["type"].as_str() {
            Some("segment") => cues.push(Cue { start: v["start"].as_f64().unwrap_or(0.0), end: v["end"].as_f64().unwrap_or(0.0), text: v["text"].as_str().unwrap_or("").to_string() }),
            Some("progress") => on(v["value"].as_f64().unwrap_or(0.0)),
            Some("done") => language = v["language"].as_str().unwrap_or("").to_string(),
            Some("error") => err = Some(v["message"].as_str().map(str::to_string).unwrap_or_else(|| tr!("识别失败"))),
            _ => {}
        }
    }
    let st = child.wait().await?;
    if let Some(e) = err {
        bail!(e);
    }
    if !st.success() {
        bail!(tr!("语音识别没有完成"));
    }
    on(1.0);
    Ok((cues, language))
}

/// 用设置里的「AI 模型」（OpenAI 兼容接口，llm.rs）逐批翻译，保持时间轴不变。
pub async fn translate(core: &Core, cues: &[Cue], to: &str) -> Result<Vec<Cue>> {
    let s = core.settings();
    if s.llm_base_url.trim().is_empty() {
        bail!(tr!("还没有设置 AI 模型，在设置里填一个 OpenAI 兼容的地址（本地 Ollama 也可以）"));
    }
    if s.llm_model.trim().is_empty() {
        bail!(tr!("还没有填 AI 模型的名字，在设置里填上（比如 deepseek-flash、qwen2.5）"));
    }
    let llm = crate::llm::Llm::from_settings(&s).expect("地址和模型都有");
    translate_with(&core.http(), &llm, cues, to).await
}

pub use crate::llm::Llm;

/// 一批最多多少条。太多模型容易漏行，太少请求次数多。
const BATCH: usize = 40;

/// 解析模型回来的「编号<TAB>译文」。容忍「1. 译文」「1: 译文」「[1] 译文」和代码块包裹。
pub fn parse_numbered(text: &str) -> std::collections::HashMap<usize, String> {
    let re = regex::Regex::new(r"^\s*\[?(\d+)\]?\s*(?:\t|[.:：、)）]\s*|\s+)(.*)$").unwrap();
    let mut map = std::collections::HashMap::new();
    for l in text.lines() {
        if l.trim_start().starts_with("```") {
            continue;
        }
        if let Some(c) = re.captures(l) {
            let t = c[2].trim();
            if let (Ok(n), false) = (c[1].parse::<usize>(), t.is_empty()) {
                map.entry(n).or_insert_with(|| t.to_string());
            }
        }
    }
    map
}

pub async fn translate_with(http: &reqwest::Client, llm: &Llm, cues: &[Cue], to: &str) -> Result<Vec<Cue>> {
    let mut out = Vec::with_capacity(cues.len());
    for chunk in cues.chunks(BATCH) {
        let numbered: String = chunk.iter().enumerate().map(|(i, c)| format!("{}\t{}\n", i + 1, c.text.replace('\n', " "))).collect();
        // 给模型看的提示词：只用英文，不跟界面语言（docs/I18N.md 第 1 节）
        let system = format!("You are a subtitle translator. Translate each subtitle line the user sends into {to}. Each line is in the form \"number<TAB>original text\". Reply in the same form, \"number<TAB>translation\", keeping the numbers and the number of lines unchanged. Don't merge or split lines, and don't output anything else.");
        let r = crate::llm::chat(http, llm, &crate::llm::Chat::new(system, vec![crate::llm::Part::Text(numbered)])).await?;
        let map = parse_numbered(&r.text);
        for (i, c) in chunk.iter().enumerate() {
            out.push(Cue { start: c.start, end: c.end, text: map.get(&(i + 1)).cloned().unwrap_or_else(|| c.text.clone()) });
        }
    }
    Ok(out)
}

/// 完整流程：取字幕 → 可选翻译 → 写文件 → 写回素材库。
pub async fn run(core: &Core, asset_id: i64, url: &str, media: &Path, o: &SubOpts, mut on: impl FnMut(f64)) -> Result<SubResult> {
    crate::tasks::sweep_scratch(&core.paths.work);
    let work = core.paths.work.join(format!("subs-{asset_id}-{}", crate::util::now_ms()));
    let model = o.model.clone().unwrap_or_else(|| core.settings().whisper_model);
    let (cues, lang, source) = match o.source {
        Source::Platform => {
            let (c, l) = platform(core, url, o.language.as_deref(), &work).await?;
            (c, l, "platform")
        }
        Source::Whisper => {
            let (c, l) = whisper(core, media, o.language.as_deref(), &model, &mut on).await?;
            (c, l, "faster-whisper")
        }
        Source::Auto => match platform(core, url, o.language.as_deref(), &work).await {
            Ok((c, l)) => (c, l, "platform"),
            Err(_) => {
                let (c, l) = whisper(core, media, o.language.as_deref(), &model, &mut on).await?;
                (c, l, "faster-whisper")
            }
        },
    };
    let cues = match &o.translate_to {
        Some(to) => translate(core, &cues, to).await?,
        None => cues,
    };
    let (body, ext) = render(&cues, &o.format)?;
    // 语言代码来自 AI 助手的参数、识别结果：当文件名的一部分用，先清理（不许带 / \ 这类，免得写到别的目录去）
    let suffix = match &o.translate_to {
        Some(t) => format!(".{}", crate::util::safe_name(t)),
        None if !lang.is_empty() => format!(".{}", crate::util::safe_name(&lang)),
        None => String::new(),
    };
    let stem = media.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let out = crate::util::unique_path(media.with_file_name(format!("{stem}{suffix}.{ext}")));
    tokio::fs::write(&out, body).await?;
    core.lib.set_transcript(asset_id, &to_txt(&cues))?;
    core.emit(crate::model::Event::Library);
    Ok(SubResult { path: out, source, language: Some(lang).filter(|l| !l.is_empty()), cues: cues.len(), list: cues })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 集成和收尾第 3 项：语音识别跑到一半取消，Python 和它起的子进程（faster-whisper 的工作进程）都要结束，转出来的音频也删掉
    #[tokio::test]
    async fn cancel_whisper_kills_python() {
        use crate::tools::testkit;
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::config::Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        // 假的 ffmpeg：把输出文件（最后一个参数）写出来；假的 Python：起一个子进程，报一次进度，然后一直等
        let ff = testkit::fake(dir.path(), "ffmpeg", "for a; do last=\"$a\"; done\nprintf RIFF > \"$last\"", "[IO.File]::WriteAllText($args[-1], 'RIFF')");
        let pidf = dir.path().join("py.pid");
        let py = testkit::fake(
            dir.path(),
            "python3",
            &format!("echo '{{\"type\":\"progress\",\"value\":0.1}}'\n{}", testkit::sh_wait_child(&pidf)),
            &format!("[Console]::Out.WriteLine('{{\"type\":\"progress\",\"value\":0.1}}')\n[Console]::Out.Flush()\n{}", testkit::ps_wait_child(&pidf)),
        );
        let snap = paths.data.join("models/models--Systran--faster-whisper-small/snapshots/x");
        std::fs::create_dir_all(&snap).unwrap();
        std::fs::write(snap.join("model.bin"), b"x").unwrap();
        let media = dir.path().join("a.mp4");
        std::fs::write(&media, b"x").unwrap();
        let core = Core::open(crate::queue::Options { paths: paths.clone(), sidecar_dir: None, restore: false }).await.unwrap();
        core.patch_settings(&serde_json::json!({"ffmpegPath": ff.display().to_string(), "pythonPath": py.display().to_string()})).unwrap();
        let id = core.new_task_id();
        let c2 = core.clone();
        let task = tokio::spawn(async move {
            let c3 = c2.clone();
            c2.run_task_as(id, "subtitle", "测试".into(), None, |rep| async move { whisper(&c3, &media, None, "small", |p| rep.pct(p)).await.map(|_| serde_json::json!({})) }).await
        });
        let pids = testkit::read_pids(&pidf).await;
        assert!(std::fs::read_dir(&paths.work).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("whisper-")), "转出来的音频在 work/whisper-…");
        assert!(core.cancel_task(id));
        let r = task.await.unwrap();
        assert!(r.is_err(), "取消以后任务报 CANCELLED");
        for p in pids {
            assert!(testkit::gone(p).await, "取消以后 Python 或它的子进程（{p}）还在");
        }
        assert!(!std::fs::read_dir(&paths.work).unwrap().flatten().any(|e| e.file_name().to_string_lossy().starts_with("whisper-")), "转出来的音频要删掉");
    }
    #[test]
    fn roundtrip() {
        let c = vec![Cue { start: 107.0, end: 110.5, text: "What brings you to the land of the gatekeepers?".into() }, Cue { start: 3723.25, end: 3724.0, text: "A dragon.".into() }];
        let srt = to_srt(&c);
        assert!(srt.contains("00:01:47,000 --> 00:01:50,500"));
        assert!(srt.contains("01:02:03,250"));
        assert_eq!(parse(&srt), c);
        let vtt = to_vtt(&c);
        assert!(vtt.starts_with("WEBVTT"));
        assert_eq!(parse(&vtt), c);
        assert_eq!(to_txt(&c), "What brings you to the land of the gatekeepers?\nA dragon.");
    }
    #[test]
    fn youtube_auto() {
        let v = "WEBVTT\nKind: captions\n\n00:00:01.000 --> 00:00:02.000 align:start position:0%\nhello<00:00:01.500><c> world</c>\n\n00:00:02.000 --> 00:00:03.000\nhello world\n\n";
        let c = parse(v);
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].text, "hello world");
        assert_eq!(c[0].end, 3.0);
    }
    #[test]
    fn site_tracks() {
        let v = serde_json::json!({
            "subtitles": {"en": [{"ext": "vtt", "name": "English"}], "de": [{"ext": "vtt", "name": "Deutsch"}], "live_chat": [{}]},
            "automatic_captions": {"en-orig": [{"ext": "vtt", "name": "English (Original)"}], "fr": [{"name": "French"}], "ja": [{}], "ko": [{}], "es": [{}], "it": [{}], "pt": [{}]}
        });
        let t = tracks_from_info(&v);
        // 人工字幕 de、en；自动字幕只列原始语言，而且英文已经有人工字幕，不再列
        assert_eq!(t.iter().map(|x| x.code.as_str()).collect::<Vec<_>>(), vec!["de", "en"]);
        assert!(t.iter().all(|x| !x.auto));
        let v = serde_json::json!({"automatic_captions": {"zh-Hans-orig": [{"name": "Chinese (Simplified) (Original)"}], "en": [{}], "de": [{}], "fr": [{}], "ja": [{}], "ko": [{}], "es": [{}]}});
        let t = tracks_from_info(&v);
        assert_eq!(t, vec![Track { code: "zh-Hans-orig".into(), name: "Chinese (Simplified)".into(), auto: true }]);
        assert!(tracks_from_info(&serde_json::json!({"title": "x"})).is_empty());
    }
    #[test]
    fn numbered() {
        let m = parse_numbered("```\n1\t你好\n2. 世界\n3：第三行\n[4] 四\n5 五\n6\t\n```");
        assert_eq!(m[&1], "你好");
        assert_eq!(m[&2], "世界");
        assert_eq!(m[&3], "第三行");
        assert_eq!(m[&4], "四");
        assert_eq!(m[&5], "五");
        assert!(!m.contains_key(&6));
    }
    #[test]
    fn endpoint() {
        let l = |u: &str| Llm { base_url: u.into(), model: "m".into(), api_key: String::new() }.endpoint();
        assert_eq!(l("https://api.openai.com/v1/"), "https://api.openai.com/v1/chat/completions");
        assert_eq!(l("http://x/v1/chat/completions"), "http://x/v1/chat/completions");
    }

    /// 本机起一个假的 OpenAI 兼容接口：按「编号<TAB>原文」回「编号<TAB>[译]原文」，
    /// 检查分批、鉴权头、时间轴不变，以及漏行时保留原文。
    #[tokio::test]
    async fn translate_mock_server() {
        use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
        use std::sync::{Arc, Mutex};
        type Seen = Arc<Mutex<Vec<(Option<String>, String, usize)>>>;
        async fn chat(State(seen): State<Seen>, h: HeaderMap, Json(v): Json<serde_json::Value>) -> Json<serde_json::Value> {
            let user = v["messages"][1]["content"].as_str().unwrap_or("").to_string();
            let lines: Vec<&str> = user.lines().collect();
            seen.lock().unwrap().push((h.get("authorization").map(|x| x.to_str().unwrap().to_string()), v["model"].as_str().unwrap().to_string(), lines.len()));
            // 故意漏掉编号 3 的那一行
            let reply: String = lines.iter().filter_map(|l| l.split_once('\t')).filter(|(n, _)| *n != "3").map(|(n, t)| format!("{n}\t[译]{t}\n")).collect();
            Json(serde_json::json!({"choices": [{"message": {"role": "assistant", "content": reply}}]}))
        }
        let seen: Seen = Default::default();
        let app = Router::new().route("/v1/chat/completions", post(chat)).with_state(seen.clone());
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        tokio::spawn(async move { axum::serve(l, app).await.unwrap() });
        let cues: Vec<Cue> = (0..45).map(|i| Cue { start: i as f64, end: i as f64 + 0.8, text: format!("line {i}\nsecond") }).collect();
        let llm = Llm { base_url: format!("http://127.0.0.1:{port}/v1"), model: "test-model".into(), api_key: "sk-test".into() };
        let out = translate_with(&reqwest::Client::new(), &llm, &cues, "中文").await.unwrap();
        assert_eq!(out.len(), 45);
        assert_eq!(out[0].text, "[译]line 0 second");
        assert_eq!((out[44].start, out[44].end), (44.0, 44.8));
        assert_eq!(out[44].text, "[译]line 44 second");
        // 漏掉的行保留原文（每批的第 3 行）
        assert_eq!(out[2].text, "line 2\nsecond");
        assert_eq!(out[42].text, "line 42\nsecond");
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0], (Some("Bearer sk-test".into()), "test-model".into(), 40));
        assert_eq!(seen[1].2, 5);
        // 服务报错时给出人话
        let bad = Llm { base_url: format!("http://127.0.0.1:{port}/nope"), model: "m".into(), api_key: String::new() };
        let e = translate_with(&reqwest::Client::new(), &bad, &cues[..1], "中文").await.unwrap_err().to_string();
        assert!(e.contains("404"), "{e}");
    }
}
