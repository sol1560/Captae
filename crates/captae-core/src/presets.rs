//! 预设工作流。
//!
//! 步骤用人话存（「下载最高画质」「转为 ProRes 422」「只留前 30 秒」），和设计稿的步骤编辑器一致，
//! 这里负责把它解析成可执行的动作。agent 通过 `preset_run` 按名字调用同一套逻辑。
//!
//! 存的步骤是中文原文（存储格式，旧版也要能读，一个字都不能改）。给人看时用 [`step_label`] 翻成当前语言，见 docs/I18N.md 2.4。

use crate::queue::Core;
use crate::model::AddRequest;
use crate::queue::JobHandle;
use crate::tr;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Preset {
    pub name: String,
    pub steps: Vec<String>,
    /// AI 助手也能用
    #[serde(default = "yes")]
    pub agent: bool,
    /// 「移动到…文件夹」的目标，没填时按步骤文字推断
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dir: Option<PathBuf>,
}

fn yes() -> bool {
    true
}

/// 设计稿步骤编辑器里的选项。存的是中文原文（tr_key! 标出来进词表），显示时用 [`step_label`]
pub const STEP_OPTIONS: &[&str] = &[
    crate::tr_key!("下载最高画质"),
    crate::tr_key!("下载 1080p"),
    crate::tr_key!("只下载声音（MP3）"),
    crate::tr_key!("去除水印"),
    crate::tr_key!("转为 MP4"),
    crate::tr_key!("转为 ProRes 422"),
    crate::tr_key!("转为 WebM"),
    crate::tr_key!("下载网站字幕"),
    crate::tr_key!("语音转文字"),
    crate::tr_key!("只留前 30 秒"),
    crate::tr_key!("去掉声音"),
    crate::tr_key!("移动到指定文件夹"),
];

/// 不在 STEP_OPTIONS 里、默认预设用到的完整步骤：显示时整句查译文
const MORE_STEPS: &[&str] = &[crate::tr_key!("导出文字稿"), crate::tr_key!("保存到「文稿」文件夹"), crate::tr_key!("移动到 Final Cut 素材文件夹")];

/// 「移动到…」「保存到…」里认得的文件夹说法（`target_dir` 按它们找真正的文件夹）：显示时查译文
const DIR_NAMES: &[&str] = &[crate::tr_key!("「文稿」文件夹"), crate::tr_key!("Final Cut 素材文件夹")];

/// 步骤 → 给人看的说法（任务记录里用），跟着界面语言。只管显示，存的步骤不变。和界面的 stepLabel（pages/agent/model.ts）同一套规则、同样的英文：
/// - 简体中文：原样返回（存的就是原文，和改之前一字不差）
/// - 表里的完整步骤（STEP_OPTIONS、MORE_STEPS）整句查译文
/// - 带参数的按样子套模板：「下载 1080p」「转为 MP4」「只留前 30 秒」「只留前 2 分钟」「移动到{文件夹}」「保存到{文件夹}」
/// - 认不出的原样返回
pub fn step_label(s: &str) -> String {
    use crate::i18n::{current, decimal, dynamic, Lang};
    if current() == Lang::ZhCn {
        return s.to_string();
    }
    let t = s.trim();
    if STEP_OPTIONS.contains(&t) || MORE_STEPS.contains(&t) {
        return dynamic(t);
    }
    if let Some(q) = t.strip_prefix("下载 ").map(str::trim).filter(|q| !q.is_empty()) {
        return tr!("下载 {quality}", quality = q);
    }
    if let Some(f) = t.strip_prefix("转为 ").map(str::trim).filter(|f| !f.is_empty()) {
        return tr!("转为 {format}", format = f);
    }
    if let Some(rest) = t.strip_prefix("只留前") {
        let (num, minutes) = match (rest.strip_suffix("分钟"), rest.strip_suffix("秒")) {
            (Some(x), _) => (x.trim(), true),
            (None, Some(x)) => (x.trim(), false),
            _ => ("", false),
        };
        if !num.is_empty() && num.chars().all(|c| c.is_ascii_digit() || c == '.') {
            if let Ok(x) = num.parse::<f64>() {
                // 小数照原来的位数写，小数点跟着语言（德文、法文、西班牙文用逗号）
                let n = decimal(x, num.split_once('.').map(|(_, f)| f.len()).unwrap_or(0));
                return if minutes { tr!("只留前 {n} 分钟", n = n) } else { tr!("只留前 {n} 秒", n = n) };
            }
        }
        return s.to_string();
    }
    for (prefix, save) in [("移动到", false), ("保存到", true)] {
        if let Some(d) = t.strip_prefix(prefix).map(str::trim).filter(|d| !d.is_empty()) {
            let dir = if DIR_NAMES.contains(&d) { dynamic(d) } else { d.to_string() };
            return if save { tr!("保存到{dir}", dir = dir) } else { tr!("移动到{dir}", dir = dir) };
        }
    }
    s.to_string()
}

#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    Download { quality: Option<String>, audio: Option<String> },
    NoWatermark,
    Convert(String),
    PlatformSubs,
    Transcribe,
    ExportTranscript,
    TrimHead(f64),
    Mute,
    MoveTo(Option<String>),
    Unknown(String),
}

impl Step {
    pub fn parse(s: &str) -> Step {
        let t = s.trim();
        if t.starts_with("只下载声音") || t == "仅下载音频" {
            let fmt = if t.contains("FLAC") { "FLAC" } else if t.contains("AAC") { "AAC" } else { "MP3" };
            return Step::Download { quality: None, audio: Some(fmt.into()) };
        }
        if let Some(q) = t.strip_prefix("下载 ") {
            return Step::Download { quality: Some(q.trim().into()), audio: None };
        }
        if t == "下载最高画质" || t == "下载" {
            return Step::Download { quality: None, audio: None };
        }
        if t.contains("水印") {
            return Step::NoWatermark;
        }
        if let Some(f) = t.strip_prefix("转为 ").or_else(|| t.strip_prefix("转成 ")) {
            return Step::Convert(f.trim().into());
        }
        if t.contains("网站字幕") || t.contains("平台字幕") {
            return Step::PlatformSubs;
        }
        if t.contains("语音转文字") || t.contains("识别") {
            return Step::Transcribe;
        }
        if t.contains("导出文字稿") || t.contains("导出 TXT") {
            return Step::ExportTranscript;
        }
        if t.starts_with("只留前") {
            let n: String = t.chars().filter(|c| c.is_ascii_digit() || *c == '.').collect();
            let mul = if t.contains('分') { 60.0 } else { 1.0 };
            return Step::TrimHead(n.parse::<f64>().unwrap_or(30.0) * mul);
        }
        if t.contains("去掉声音") || t == "静音" {
            return Step::Mute;
        }
        if t.starts_with("移动到") || t.starts_with("保存到") {
            let name = t.trim_start_matches("移动到").trim_start_matches("保存到").trim();
            return Step::MoveTo(Some(name.to_string()).filter(|s| s != "指定文件夹"));
        }
        Step::Unknown(t.into())
    }
}

impl Preset {
    /// 把下载相关的步骤写进下载请求。
    pub fn apply_download(&self, req: &mut AddRequest) {
        for s in &self.steps {
            if let Step::Download { quality, audio } = Step::parse(s) {
                if let Some(q) = quality {
                    req.quality = Some(q);
                }
                if let Some(a) = audio {
                    req.audio_only = true;
                    req.format = Some(a);
                }
            }
        }
    }

    pub fn target_dir(&self, label: Option<&str>) -> PathBuf {
        if let Some(d) = &self.dir {
            return d.clone();
        }
        let ud = directories::UserDirs::new();
        let home = ud.as_ref().map(|u| u.home_dir().to_path_buf()).unwrap_or_else(|| PathBuf::from("."));
        let l = label.unwrap_or("");
        if l.contains("文稿") {
            return ud.as_ref().and_then(|u| u.document_dir().map(Path::to_path_buf)).unwrap_or_else(|| home.join("Documents")).join("Captae");
        }
        if l.contains("Final Cut") {
            // 建的文件夹名跟着界面语言（英文 Final Cut Media）
            return ud.as_ref().and_then(|u| u.video_dir().map(Path::to_path_buf)).unwrap_or_else(|| home.join("Movies")).join(tr!("Final Cut 素材"));
        }
        let clean = l.trim_matches(|c| c == '「' || c == '」').replace("文件夹", "");
        home.join(if clean.trim().is_empty() { "Captae" } else { clean.trim() })
    }
}

/// 默认的三个预设。名字按当前界面语言生成（还没保存过时每次都重新生成）；步骤是存储格式，照旧写中文原文
pub fn defaults() -> Vec<Preset> {
    let p = |name: String, steps: &[&str], agent: bool| Preset { name, steps: steps.iter().map(|s| s.to_string()).collect(), agent, dir: None };
    vec![
        p(tr!("抖音素材"), &["下载最高画质", "去除水印", "转为 ProRes 422", "移动到 Final Cut 素材文件夹"], true),
        p(tr!("播客转文字"), &["只下载声音（MP3）", "语音转文字", "导出文字稿", "保存到「文稿」文件夹"], true),
        p(tr!("参考片段"), &["下载 1080p", "只留前 30 秒", "去掉声音", "转为 WebM"], false),
    ]
}

pub fn load(p: &Path) -> Vec<Preset> {
    std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_else(defaults)
}

pub fn save(p: &Path, v: &[Preset]) -> Result<()> {
    std::fs::write(p, serde_json::to_vec_pretty(v)?)?;
    Ok(())
}

/// 一步的执行结果（界面上「试运行」逐步显示）。
#[derive(Debug, Clone, Serialize)]
pub struct StepResult {
    pub index: usize,
    pub step: String,
    /// run / ok / skip / err
    pub status: &'static str,
    /// 给人看的说明：跳过的原因、出错信息（跟着界面语言）
    pub detail: String,
    /// 因为看不懂这一步而跳过（任务记录按它判断，不看 detail 的文字：detail 跟着界面语言）
    #[serde(skip)]
    pub unknown: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct RunReport {
    pub steps: Vec<StepResult>,
    pub outputs: Vec<PathBuf>,
    /// 最后得到的素材（转换、剪辑后的新素材）
    #[serde(rename = "assetId")]
    pub asset_id: i64,
    /// 有一步失败时的原因，后面的步骤不再执行
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
}

/// 对一个已入库的素材执行预设里「下载之后」的步骤，返回产生的文件。
pub async fn run_post(core: &Core, p: &Preset, asset_id: i64, h: Option<JobHandle>) -> Result<Vec<PathBuf>> {
    let n = p.steps.len().max(1);
    let r = run_steps(core, p, asset_id, false, |r| {
        let Some(h) = &h else { return };
        // 任务记录：步骤按当前语言显示（存的是中文原文）
        let msg = match r.status {
            "run" => tr!("预设第 {index} 步：{step}", index = r.index + 1, step = step_label(&r.step)),
            "skip" if r.unknown => tr!("跳过不认识的步骤「{step}」", step = r.step),
            _ => return,
        };
        h.update(|j| {
            j.merge = r.index as f64 / n as f64 * 100.0;
            j.log(msg);
        });
    })
    .await;
    match r.failed {
        Some(e) => Err(anyhow!(e)),
        None => Ok(r.outputs),
    }
}

/// 逐步执行预设（下载步骤跳过，素材已经在库里），每一步开始和结束都回调 `on`。
///
/// `trial` 是界面上的「试运行」：拿用户已有的素材来跑，「移动到…」这一步如果要搬的是原素材本身，
/// 改成复制一份过去，原文件留在原处。
pub async fn run_steps(core: &Core, p: &Preset, asset_id: i64, trial: bool, mut on: impl FnMut(&StepResult)) -> RunReport {
    let mut rep = RunReport { asset_id, ..Default::default() };
    let mut cur_id = asset_id;
    let mut cur = match core.lib.get(asset_id) {
        Ok(Some(a)) if a.exists => a.path,
        Ok(Some(a)) => {
            rep.failed = Some(tr!("文件已经不在了：{path}", path = a.path.display()));
            PathBuf::new()
        }
        _ => {
            rep.failed = Some(tr!("素材不存在"));
            PathBuf::new()
        }
    };
    let mut transcript: Option<PathBuf> = None;
    // 处理媒体的步骤（转格式、截取、去声音）：中间结果放临时目录、不进素材库，
    // 只有最后一步的结果放到原文件旁边，叫「原文件名（预设名）」
    let last_media = p.steps.iter().rposition(|s| matches!(Step::parse(s), Step::Convert(_) | Step::TrimHead(_) | Step::Mute));
    crate::tasks::sweep_scratch(&core.paths.work);
    let mc = MediaCtx { tmp: core.paths.work.join(format!("preset-{}", crate::util::now_ms())), orig: cur.clone(), name: p.name.clone() };
    // 中间结果：跑完、出错、试运行被取消都删（最后一步的结果放在原文件旁边，不在这里面）
    let _tmp = crate::tasks::Scratch::new(&mc.tmp);
    for (i, s) in p.steps.iter().enumerate() {
        let mut res = StepResult { index: i, step: s.clone(), status: "skip", detail: String::new(), unknown: false, output: None };
        if rep.failed.is_some() {
            res.detail = tr!("前面的步骤没有完成，这一步没有执行");
            on(&res);
            rep.steps.push(res);
            continue;
        }
        let step = Step::parse(s);
        let skip = match &step {
            Step::Download { .. } if trial => Some(tr!("试运行用的是已经下载好的素材，跳过下载")),
            Step::Download { .. } => Some(tr!("下载时已经处理")),
            Step::NoWatermark => Some(tr!("去水印在下载时处理")),
            Step::Unknown(_) => Some(tr!("看不懂这一步，跳过了")),
            _ => None,
        };
        if let Some(why) = skip {
            res.detail = why;
            res.unknown = matches!(step, Step::Unknown(_));
            on(&res);
            rep.steps.push(res);
            continue;
        }
        res.status = "run";
        on(&res);
        let r = run_one(core, p, step, asset_id, trial, &mut cur_id, &mut cur, &mut transcript, &mc, Some(i) == last_media).await;
        match r {
            Ok(out) => {
                res.status = "ok";
                if let Some(o) = &out {
                    rep.outputs.push(o.clone());
                }
                res.output = out;
            }
            Err(e) => {
                res.status = "err";
                res.detail = e.to_string();
                rep.failed = Some(e.to_string());
            }
        }
        on(&res);
        rep.steps.push(res);
    }
    rep.asset_id = cur_id;
    core.emit(crate::model::Event::Library);
    rep
}

struct MediaCtx {
    /// 中间结果的临时目录，跑完删掉
    tmp: PathBuf,
    /// 预设开始时的原文件
    orig: PathBuf,
    name: String,
}

/// 媒体步骤的结果：不是最后一步就只往下传；最后一步改名放到原文件旁边并入库。
async fn place_media(core: &Core, mc: &MediaCtx, out: PathBuf, last: bool, cur_id: &mut i64, cur: &mut PathBuf) -> Result<Option<PathBuf>> {
    if !last {
        *cur = out;
        return Ok(None);
    }
    let dir = mc.orig.parent().map(Path::to_path_buf).unwrap_or_default();
    let stem = mc.orig.file_stem().unwrap_or_default().to_string_lossy().to_string();
    let ext = out.extension().unwrap_or_default().to_string_lossy().to_string();
    // 「原文件名（预设名）」：跟着界面语言（英文「原文件名 (预设名)」）
    let name = tr!("{name}（{preset}）§预设最后一步的输出文件名：原文件名（预设名）", name = stem, preset = crate::util::safe_name(&mc.name));
    let dst = crate::util::unique_path(dir.join(format!("{name}.{ext}")));
    if tokio::fs::rename(&out, &dst).await.is_err() {
        tokio::fs::copy(&out, &dst).await?;
        let _ = tokio::fs::remove_file(&out).await;
    }
    *cur_id = core.import_file(&dst, Some(*cur_id), None).await?;
    *cur = dst.clone();
    Ok(Some(dst))
}

#[allow(clippy::too_many_arguments)]
async fn run_one(
    core: &Core,
    p: &Preset,
    step: Step,
    orig_id: i64,
    trial: bool,
    cur_id: &mut i64,
    cur: &mut PathBuf,
    transcript: &mut Option<PathBuf>,
    mc: &MediaCtx,
    last_media: bool,
) -> Result<Option<PathBuf>> {
    let tools = core.toolbox();
    if matches!(step, Step::Convert(_) | Step::TrimHead(_) | Step::Mute) {
        tokio::fs::create_dir_all(&mc.tmp).await?;
    }
    Ok(match step {
        Step::Convert(f) => {
            let out = crate::ffmpeg::convert(&tools, cur, &f, Some(&mc.tmp), vec![], |_| {}).await?;
            place_media(core, mc, out, last_media, cur_id, cur).await?
        }
        Step::TrimHead(secs) => {
            let dur = crate::ffmpeg::probe(&tools, cur).await?.duration.unwrap_or(secs);
            let o = crate::ffmpeg::TrimOpts { start: 0.0, end: secs.min(dur), ..Default::default() };
            let out = crate::ffmpeg::trim(&tools, cur, &o, Some(&mc.tmp), vec![], |_| {}).await?;
            place_media(core, mc, out, last_media, cur_id, cur).await?
        }
        Step::Mute => {
            let dur = crate::ffmpeg::probe(&tools, cur).await?.duration.unwrap_or(0.0);
            let o = crate::ffmpeg::TrimOpts { start: 0.0, end: dur.max(0.01), mute: true, ..Default::default() };
            let out = crate::ffmpeg::trim(&tools, cur, &o, Some(&mc.tmp), vec![], |_| {}).await?;
            place_media(core, mc, out, last_media, cur_id, cur).await?
        }
        Step::PlatformSubs => {
            let a = core.lib.get(*cur_id)?.ok_or_else(|| anyhow!(tr!("素材不存在")))?;
            let url = if a.page.is_empty() { a.url.clone() } else { a.page.clone() };
            if url.is_empty() {
                return Err(anyhow!(tr!("这个素材没有来源网址，下载不了网站字幕")));
            }
            let o = crate::subtitle::SubOpts { source: crate::subtitle::Source::Platform, ..Default::default() };
            let r = crate::subtitle::run(core, *cur_id, &url, cur, &o, |_| {}).await?;
            *transcript = Some(r.path.clone());
            Some(r.path)
        }
        Step::Transcribe => {
            let o = crate::subtitle::SubOpts { source: crate::subtitle::Source::Whisper, format: "srt".into(), ..Default::default() };
            let r = crate::subtitle::run(core, *cur_id, "", cur, &o, |_| {}).await?;
            *transcript = Some(r.path.clone());
            Some(r.path)
        }
        Step::ExportTranscript => {
            let text = core.lib.transcript(*cur_id)?;
            if text.trim().is_empty() {
                return Err(anyhow!(tr!("还没有文字稿，前面需要先有「语音转文字」或「下载网站字幕」")));
            }
            let out = crate::util::unique_path(cur.with_extension("txt"));
            tokio::fs::write(&out, text).await?;
            *transcript = Some(out.clone());
            Some(out)
        }
        Step::MoveTo(label) => {
            let dir = p.target_dir(label.as_deref());
            tokio::fs::create_dir_all(&dir).await.map_err(|e| anyhow!(tr!("文件夹「{path}」不能写入：{error}", path = dir.display(), error = e)))?;
            // 有文字稿的流程（播客转文字）只搬文字稿，媒体文件留在素材库目录
            let src = transcript.clone().unwrap_or_else(|| cur.clone());
            let dst = crate::util::unique_path(dir.join(src.file_name().unwrap_or_default()));
            if trial && transcript.is_none() && *cur_id == orig_id {
                // 试运行不动用户原来的文件：复制一份过去，作为新素材
                tokio::fs::copy(&src, &dst).await?;
                *cur_id = core.import_file(&dst, Some(orig_id), None).await?;
                *cur = dst.clone();
                return Ok(Some(dst));
            }
            if tokio::fs::rename(&src, &dst).await.is_err() {
                tokio::fs::copy(&src, &dst).await?;
                tokio::fs::remove_file(&src).await?;
            }
            if transcript.is_none() {
                core.lib.set_path(*cur_id, &dst)?;
                *cur = dst.clone();
            } else {
                *transcript = Some(dst.clone());
            }
            Some(dst)
        }
        Step::Download { .. } | Step::NoWatermark | Step::Unknown(_) => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_steps() {
        assert_eq!(Step::parse("下载 1080p"), Step::Download { quality: Some("1080p".into()), audio: None });
        assert_eq!(Step::parse("只下载声音（MP3）"), Step::Download { quality: None, audio: Some("MP3".into()) });
        assert_eq!(Step::parse("转为 ProRes 422"), Step::Convert("ProRes 422".into()));
        assert_eq!(Step::parse("只留前 30 秒"), Step::TrimHead(30.0));
        assert_eq!(Step::parse("只留前 2 分钟"), Step::TrimHead(120.0));
        assert_eq!(Step::parse("去除水印"), Step::NoWatermark);
        assert_eq!(Step::parse("保存到「文稿」文件夹"), Step::MoveTo(Some("「文稿」文件夹".into())));
        assert_eq!(Step::parse("移动到指定文件夹"), Step::MoveTo(None));
        for s in STEP_OPTIONS {
            assert!(!matches!(Step::parse(s), Step::Unknown(_)), "{s}");
        }
    }
    /// 步骤的显示（任务记录里用）：中文原样；其他语言整句查表、带参数的套模板、认不出的原样
    #[test]
    fn step_labels() {
        use crate::i18n::{with_lang, Lang};
        for s in ["下载 1080p", "只留前30秒", "转成 MP4", "移动到 ~/Desktop/clips", "做个魔法"] {
            assert_eq!(step_label(s), s);
        }
        with_lang(Lang::En, || {
            assert_eq!(step_label("下载最高画质"), "Download best quality");
            assert_eq!(step_label("下载 720p"), "Download 720p");
            assert_eq!(step_label("转为 MOV"), "Convert to MOV");
            assert_eq!(step_label("只留前 1 分钟"), "Keep the first 1 minute");
            assert_eq!(step_label("只留前45秒"), "Keep the first 45 seconds");
            assert_eq!(step_label("只留前 1.5 分钟"), "Keep the first 1.5 minutes");
            assert_eq!(step_label("移动到指定文件夹"), "Move to a chosen folder");
            assert_eq!(step_label("移动到 Final Cut 素材文件夹"), "Move to the Final Cut Media folder");
            assert_eq!(step_label("保存到「文稿」文件夹"), "Save to the Documents folder");
            assert_eq!(step_label("移动到「文稿」文件夹"), "Move to the Documents folder");
            assert_eq!(step_label("移动到 ~/Desktop/clips"), "Move to ~/Desktop/clips");
            assert_eq!(step_label("做个魔法"), "做个魔法");
            // 表里的步骤都有英文
            for s in STEP_OPTIONS.iter().chain(MORE_STEPS) {
                let l = step_label(s);
                assert!(!l.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)), "{s} → {l}");
            }
            assert_eq!(defaults()[2].name, "Reference clip");
        });
    }
    #[test]
    fn apply() {
        let mut r = AddRequest::default();
        defaults()[1].apply_download(&mut r);
        assert!(r.audio_only);
        assert_eq!(r.format.as_deref(), Some("MP3"));
        let mut r = AddRequest::default();
        defaults()[2].apply_download(&mut r);
        assert_eq!(r.quality.as_deref(), Some("1080p"));
    }

    /// 试运行：下载步骤跳过、「移动到」只复制不搬原文件、失败后后面的步骤不执行。需要本机有 ffmpeg。
    #[tokio::test]
    async fn trial_run_steps() {
        if std::process::Command::new("ffmpeg").arg("-version").output().is_err() {
            eprintln!("没有 ffmpeg，跳过");
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let core = Core::open(crate::Options { paths: crate::Paths::portable(d.path()), sidecar_dir: None, restore: false }).await.unwrap();
        let src = d.path().join("clip.mp4");
        let st = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc=size=160x120:rate=10:duration=2", "-f", "lavfi", "-i", "sine=duration=2", "-shortest", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac"])
            .arg(&src)
            .status()
            .unwrap();
        assert!(st.success());
        let id = core.import_file(&src, None, None).await.unwrap();
        let out = d.path().join("out");
        let p = Preset { name: "t".into(), steps: ["下载 1080p", "移动到指定文件夹", "做个魔法", "去掉声音"].map(String::from).to_vec(), agent: true, dir: Some(out.clone()) };
        let mut seen = vec![];
        let r = run_steps(&core, &p, id, true, |s| seen.push((s.index, s.status))).await;
        assert!(r.failed.is_none(), "{:?}", r.failed);
        let st: Vec<&str> = r.steps.iter().map(|s| s.status).collect();
        assert_eq!(st, ["skip", "ok", "skip", "ok"]);
        assert!(seen.contains(&(1, "run")) && seen.contains(&(1, "ok")));
        // 原文件还在原处，库里的路径也没变；复制出来的是新素材
        assert!(src.exists());
        assert_eq!(core.lib.get(id).unwrap().unwrap().path, src);
        assert!(out.join("clip.mp4").exists());
        assert_ne!(r.asset_id, id);
        assert_eq!(r.outputs.len(), 2);

        // 失败的步骤：后面的不再执行
        let p = Preset { name: "t2".into(), steps: ["导出文字稿", "去掉声音"].map(String::from).to_vec(), agent: true, dir: None };
        let r = run_steps(&core, &p, id, true, |_| {}).await;
        assert!(r.failed.is_some());
        assert_eq!(r.steps[0].status, "err");
        assert_eq!(r.steps[1].status, "skip");
        assert!(run_post(&core, &p, id, None).await.is_err());
    }
}

