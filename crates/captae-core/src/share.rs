//! Share to Agent：把一个视频变成 AI 能直接读的材料。
//!
//! 下载（720p 以内就够）→ 文字稿（平台字幕优先，没有就本地识别）→ 关键帧（场景变化 + 等间隔补齐）
//! → 打包成 `context.md` + `frames/*.jpg` + `meta.json` → 放进剪贴板（文字 + 图片文件）。
//! 用户在任何 AI 对话框里粘贴即可，agent 不用自己看视频。

use crate::model::AddRequest;
use crate::queue::Core;
use crate::subtitle::{self, Cue};
use crate::util::clock;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct Bundle {
    pub dir: PathBuf,
    pub markdown: String,
    pub frames: Vec<PathBuf>,
    pub asset_id: i64,
    pub transcript_source: Option<String>,
    pub copied: bool,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct ShareOpts {
    pub url: Option<String>,
    pub asset_id: Option<i64>,
    pub page_title: Option<String>,
    pub max_frames: Option<usize>,
    pub language: Option<String>,
    /// 放进剪贴板（agent 调用时一般不需要）
    pub copy: bool,
    pub headers: std::collections::BTreeMap<String, String>,
    /// 浏览器插件带来的这个网站的 cookies 和 User-Agent。规矩和下载一样：只给这一次按网址下载用（交给下载任务），
    /// 不进任务记录和错误原文、不存盘。AI 助手那条路（share_video 工具）不收这两个字段（agent.rs 自己构造，留空）
    pub cookies: Vec<crate::login::Cookie>,
    pub user_agent: Option<String>,
}

/// 文字稿按约 30 秒一段，带时间戳。
pub fn paragraphs(cues: &[Cue]) -> String {
    let mut out = String::new();
    let mut start: Option<f64> = None;
    let mut buf = String::new();
    for c in cues {
        if start.is_none() {
            start = Some(c.start);
        }
        if !buf.is_empty() {
            buf.push(' ');
        }
        buf.push_str(c.text.trim());
        if c.end - start.unwrap() >= 30.0 {
            out.push_str(&format!("[{}] {}\n\n", clock(start.unwrap()).max("0:00".into()), buf));
            buf.clear();
            start = None;
        }
    }
    if !buf.is_empty() {
        out.push_str(&format!("[{}] {}\n", clock(start.unwrap_or(0.0)).max("0:00".into()), buf));
    }
    out
}

pub struct Meta<'a> {
    pub site: &'a str,
    pub title: &'a str,
    pub uploader: &'a str,
    pub dur: Option<f64>,
    pub url: &'a str,
    pub desc: &'a str,
}

/// 「发给 AI」的 context.md。跟着界面语言写（用户把它粘贴进自己的 AI 对话框）
pub fn markdown(m: &Meta, transcript: Option<&str>, frame_times: &[f64]) -> String {
    use crate::tr;
    let mut s = String::new();
    // 站点名按界面语言显示（存的是原文：哔哩哔哩）；没有站点名时说「网页」。
    // 「来自 {site} 的视频」两边的空格是原来就有的，站点名是中文时也空着，中文显示和原来一字不差
    let site = if m.site.is_empty() { tr!("网页§context.md 开头「这是一个来自…的视频」里没有站点名时") } else { crate::i18n::dynamic(m.site) };
    let n = frame_times.len();
    // 有没有时长、作者：四种整句，不拼片段（docs/I18N.md 2.3）
    let intro = match (m.dur, m.uploader.is_empty()) {
        (None, true) => tr!("这是一个来自 {site} 的视频。下面是它的文字稿和 {n} 张关键帧，你不需要打开视频就能理解内容。", site = site, n = n),
        (Some(d), true) => tr!("这是一个来自 {site} 的视频，时长 {dur}。下面是它的文字稿和 {n} 张关键帧，你不需要打开视频就能理解内容。", site = site, dur = clock(d), n = n),
        (None, false) => tr!("这是一个来自 {site} 的视频，作者是 {uploader}。下面是它的文字稿和 {n} 张关键帧，你不需要打开视频就能理解内容。", site = site, uploader = m.uploader, n = n),
        (Some(d), false) => tr!("这是一个来自 {site} 的视频，时长 {dur}，作者是 {uploader}。下面是它的文字稿和 {n} 张关键帧，你不需要打开视频就能理解内容。", site = site, dur = clock(d), uploader = m.uploader, n = n),
    };
    s.push_str(&intro);
    s.push_str("\n\n");
    s.push_str(&format!("- {}\n- {}\n", tr!("标题：{title}", title = m.title), tr!("链接：{url}", url = m.url)));
    if !m.desc.trim().is_empty() {
        let d: String = m.desc.trim().chars().take(600).collect();
        s.push_str(&format!("- {}\n", tr!("简介：{text}", text = d.replace('\n', " "))));
    }
    s.push_str(&format!("\n## {}\n\n", tr!("文字稿§context.md 里的小标题")));
    match transcript {
        Some(t) if !t.trim().is_empty() => s.push_str(t),
        _ => {
            s.push_str(&tr!("（这个视频没有可识别的语音）"));
            s.push('\n');
        }
    }
    if !frame_times.is_empty() {
        let ts: Vec<String> = frame_times.iter().map(|t| clock(*t).max("0:00".into())).collect();
        s.push_str(&format!("\n## {}\n\n{}\n", tr!("关键帧§context.md 里的小标题"), tr!("附带的图片按时间顺序排列，分别来自 {times}。", times = crate::i18n::join(&ts))));
    }
    s
}

/// 「发给 AI」为自己开的下载任务：没等到结果就被丢掉（取消）时，把任务也取消。
struct JobGuard {
    core: Core,
    id: u64,
    armed: bool,
}

impl Drop for JobGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.core.act(self.id, crate::model::JobAction::Cancel);
        }
    }
}

pub async fn share(core: &Core, o: ShareOpts, on: impl Fn(f64, &str)) -> Result<Bundle> {
    use crate::tr;
    // 1. 拿到素材（每一步的说法给人看：插件卡片、界面上的进度）
    on(0.02, &tr!("正在下载视频"));
    let asset_id = match (o.asset_id, &o.url) {
        (Some(id), _) => id,
        (None, Some(url)) => {
            // 已经下过的直接用
            match core.lib.find_by_url(url, true)?.filter(|a| a.kind == "video") {
                Some(a) => a.id,
                None => {
                    let job = core.add(AddRequest {
                        url: url.clone(),
                        quality: Some("720p".into()),
                        format: Some("MP4".into()),
                        page_title: o.page_title.clone(),
                        headers: o.headers.clone(),
                        // 插件带来的登录状态交给下载任务（queue 那边不写进 queue.json、任务记录）
                        cookies: o.cookies.clone(),
                        user_agent: o.user_agent.clone(),
                        origin: Some("share".into()),
                        ..Default::default()
                    })?;
                    // 用户中途取消「发给 AI」时，这个 future 会被丢掉，顺手把为它开的下载也停掉
                    let mut guard = JobGuard { core: core.clone(), id: job.id, armed: true };
                    let done = core.wait(job.id).await?;
                    guard.armed = false;
                    done.asset_id.ok_or_else(|| anyhow!(done.err.unwrap_or_else(|| tr!("视频没有下载成功"))))?
                }
            }
        }
        _ => return Err(anyhow!(tr!("需要一个链接或素材"))),
    };
    let a = core.lib.get(asset_id)?.ok_or_else(|| anyhow!(tr!("素材不存在")))?;

    // 2. 文字稿
    on(0.45, &tr!("正在整理文字稿"));
    let src_url = if a.page.is_empty() { a.url.clone() } else { a.page.clone() };
    let work = core.paths.work.join(format!("share-subs-{asset_id}"));
    let (cues, source) = match subtitle::platform(core, &src_url, o.language.as_deref(), &work).await {
        Ok((c, _)) => (c, Some("platform".to_string())),
        Err(_) => match subtitle::whisper(core, &a.path, o.language.as_deref(), &core.settings().whisper_model, |p| on(0.45 + p * 0.3, &tr!("正在把语音转成文字"))).await {
            Ok((c, _)) => (c, Some("faster-whisper".to_string())),
            Err(e) => {
                tracing::warn!("share: no transcript: {e}");
                (vec![], None)
            }
        },
    };
    if !cues.is_empty() {
        core.lib.set_transcript(asset_id, &subtitle::to_txt(&cues))?;
    }

    // 3. 关键帧
    on(0.8, &tr!("正在提取关键帧"));
    // 截短以后结尾可能是空格或点（Windows 会悄悄去掉，目录名就对不上了），截完再理一次
    let slug = crate::util::safe_name(&crate::util::safe_name(&a.title).chars().take(40).collect::<String>());
    let dir = core.paths.shares.join(format!("{}-{slug}", crate::util::now_ms()));
    let frames = crate::ffmpeg::keyframes(&core.toolbox(), &a.path, &dir.join("frames"), o.max_frames.map(|n| n.clamp(1, crate::ffmpeg::MAX_FRAMES)).unwrap_or_else(|| crate::ffmpeg::auto_frames(a.duration)), a.duration).await?;

    // 4. 打包
    let para = paragraphs(&cues);
    let md = markdown(
        &Meta { site: &a.src, title: &a.title, uploader: &a.uploader, dur: a.duration, url: &src_url, desc: &a.desc },
        Some(&para).filter(|p| !p.is_empty()).map(|s| s.as_str()),
        &frames.iter().map(|(t, _)| *t).collect::<Vec<_>>(),
    );
    tokio::fs::write(dir.join("context.md"), &md).await?;
    if !cues.is_empty() {
        tokio::fs::write(dir.join("transcript.srt"), subtitle::to_srt(&cues)).await?;
    }
    let meta = serde_json::json!({
        "title": a.title, "platform": a.src, "uploader": a.uploader, "duration": a.duration,
        "url": src_url, "frames": frames.iter().map(|(t, p)| serde_json::json!({"time": t, "file": p.file_name().map(|f| f.to_string_lossy().to_string())})).collect::<Vec<_>>(),
        "transcript_source": source,
    });
    tokio::fs::write(dir.join("meta.json"), serde_json::to_vec_pretty(&meta)?).await?;
    let frame_paths: Vec<PathBuf> = frames.into_iter().map(|(_, p)| p).collect();

    // 5. 剪贴板
    let copied = if o.copy { copy_bundle(&md, &frame_paths).is_ok() } else { false };
    on(1.0, &tr!("完成§「发给 AI」整理完了，进度上的说法"));
    Ok(Bundle { dir, markdown: md, frames: frame_paths, asset_id, transcript_source: source, copied })
}

/// 文字放 text/plain 和 text/html，关键帧作为文件列表；在 AI 对话框里粘贴时两者都会带上。
#[cfg(feature = "clipboard")]
pub fn copy_bundle(md: &str, frames: &[PathBuf]) -> Result<()> {
    use clipboard_rs::{Clipboard, ClipboardContent, ClipboardContext};
    if crate::clipboard::writes_disabled() {
        tracing::info!("测试开关 CAPTAE_TEST_NO_CLIPBOARD 开着：「发给 AI」没有写系统剪贴板（文字 {} 字、关键画面 {} 张）", md.chars().count(), frames.len());
        return Ok(());
    }
    let ctx = ClipboardContext::new().map_err(|e| anyhow!("{e}"))?;
    let files: Vec<String> = frames.iter().map(|p| p.display().to_string()).collect();
    let html = format!("<pre>{}</pre>", md.replace('&', "&amp;").replace('<', "&lt;"));
    ctx.set(vec![ClipboardContent::Text(md.to_string()), ClipboardContent::Html(html), ClipboardContent::Files(files)]).map_err(|e| anyhow!("{e}"))?;
    Ok(())
}

#[cfg(not(feature = "clipboard"))]
pub fn copy_bundle(_: &str, _: &[PathBuf]) -> Result<()> {
    Err(anyhow!(crate::tr!("这个版本没有剪贴板支持")))
}

/// 插件卡片上的「再复制一次」：按任务结果（`Bundle` 序列化后的样子）把文字和关键帧重新放进剪贴板。
pub fn recopy(result: &serde_json::Value) -> Result<()> {
    let md = result.get("markdown").and_then(|v| v.as_str()).ok_or_else(|| anyhow!(crate::tr!("这次整理的内容已经找不到了")))?;
    let frames: Vec<PathBuf> = result.get("frames").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|p| p.as_str()).map(PathBuf::from).filter(|p| p.exists()).collect()).unwrap_or_default();
    copy_bundle(md, &frames)
}

/// 关键帧小图，给插件卡片用：每张缩到 160 宽再转成 data URL。
/// Native Messaging 从宿主发给插件的单条消息不能超过 1 MB，原图太大。
pub async fn frame_thumbs(tools: &crate::tools::Toolbox, frames: &[PathBuf]) -> Vec<String> {
    use base64::Engine;
    let mut out = vec![];
    for f in frames.iter().take(12) {
        let Ok(mut c) = tools.command(crate::tools::Tool::Ffmpeg) else { break };
        c.args(["-hide_banner", "-nostdin", "-loglevel", "error", "-i"]).arg(f).args(["-frames:v", "1", "-vf", "scale=160:-2", "-q:v", "6", "-f", "image2", "-c:v", "mjpeg", "pipe:1"]);
        match crate::tools::run_ok(c).await {
            Ok(b) if !b.is_empty() && b.len() < 60_000 => out.push(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(b))),
            _ => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 集成和收尾第 2 项：插件发来的「发给 AI」带着这个网站的 cookies 和 User-Agent，交给按网址下载那一步
    /// （yt-dlp 收到 --cookies 临时文件），值不进任务记录和错误原文，临时文件用完就删；
    /// AI 助手那条路（share_video 工具）不收这两个字段，传了也不带
    #[cfg(unix)]
    #[tokio::test]
    async fn share_carries_browser_login() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let calls = dir.path().join("calls.txt");
        let fake = dir.path().join("yt-dlp");
        // 假的 yt-dlp：记下参数；带了 --cookies 就把那个文件的内容也记下；读取信息时说不支持这个网址（发给 AI 在下载这一步就停了）
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\nfor a; do case \"$a\" in --list-impersonate-targets) exit 0;; esac; done\necho \"ARGS $*\" >> '{c}'\nprev=\"\"\nfor a; do if [ \"$prev\" = \"--cookies\" ]; then cat \"$a\" >> '{c}'; fi; prev=\"$a\"; done\necho 'ERROR: Unsupported URL: https://example.com/v/1' >&2\nexit 1\n",
                c = calls.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let paths = crate::config::Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        let core = Core::open(crate::queue::Options { paths: paths.clone(), sidecar_dir: None, restore: false }).await.unwrap();
        core.patch_settings(&serde_json::json!({"ytdlpPath": fake.display().to_string(), "ffmpegPath": "/nonexistent/captae-test/ffmpeg", "proxyMode": "none"})).unwrap();
        let ck = |v: &str| serde_json::json!({"name": "sid", "value": v, "domain": ".example.com", "path": "/", "secure": true, "httpOnly": true, "hostOnly": false});
        let chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
        // 插件、本地控制接口 /v1/share 收到的就是这样的 JSON
        let o: ShareOpts = serde_json::from_value(serde_json::json!({"url": "https://example.com/v/1", "page_title": "例子", "cookies": [ck("share-secret-1")], "user_agent": chrome})).unwrap();
        assert_eq!((o.cookies.len(), o.user_agent.as_deref()), (1, Some(chrome)));
        assert!(!format!("{o:?}").contains("share-secret-1"), "打印出来也看不到值");
        assert!(share(&core, o, |_, _| {}).await.is_err());
        let log = std::fs::read_to_string(&calls).unwrap();
        assert!(log.contains("--cookies") && log.contains("share-secret-1"), "下载那一步要带上插件给的 cookies：{log}");
        let jobs = core.jobs();
        assert_eq!(jobs.len(), 1);
        let j = &jobs[0];
        assert!(j.state == crate::model::JobState::Failed && j.log.iter().any(|l| l.1.contains("浏览器插件带来了这个网站的登录状态")));
        let snap = serde_json::to_string(j).unwrap();
        assert!(!snap.contains("share-secret-1") && !format!("{:?}", j.log).contains("share-secret-1"), "值不进任务记录和错误原文");
        let left: Vec<_> = std::fs::read_dir(crate::login::auth_dir(&paths)).map(|r| r.flatten().map(|e| e.path()).collect()).unwrap_or_default();
        assert!(left.is_empty(), "临时 cookies 文件用完就删：{left:?}");
        // AI 助手（MCP 的 share_video）：传了 cookies 也不带
        std::fs::write(&calls, "").unwrap();
        let r = crate::agent::call(&core, "测试", "share_video", serde_json::json!({"url": "https://example.com/v/2", "cookies": [ck("ai-secret-2")], "user_agent": chrome})).await;
        assert!(r.is_err());
        let log = std::fs::read_to_string(&calls).unwrap();
        assert!(log.contains("ARGS") && !log.contains("--cookies") && !log.contains("ai-secret-2"), "AI 助手那条路不收 cookies：{log}");
    }
    #[test]
    fn md() {
        let cues = vec![Cue { start: 0.0, end: 12.0, text: "a".into() }, Cue { start: 12.0, end: 31.0, text: "b".into() }, Cue { start: 31.0, end: 40.0, text: "c".into() }];
        let p = paragraphs(&cues);
        assert_eq!(p, "[0:00] a b\n\n[0:31] c\n");
        let m = markdown(&Meta { site: "YouTube", title: "Sintel", uploader: "Blender", dur: Some(888.0), url: "https://y/1", desc: "" }, Some(&p), &[12.0, 60.0]);
        assert!(m.starts_with("这是一个来自 YouTube 的视频，时长 14:48，作者是 Blender。"));
        assert!(m.contains("0:12、1:00"));
    }
    /// 「发给 AI」的 context.md 跟着界面语言：站点名、单复数、小标题
    #[test]
    fn md_follows_language() {
        crate::i18n::with_lang(crate::i18n::Lang::En, || {
            let m = markdown(&Meta { site: "哔哩哔哩", title: "Sintel", uploader: "", dur: Some(888.0), url: "https://b/1", desc: "" }, None, &[12.0]);
            assert!(m.starts_with("This is a 14:48 video from Bilibili. Below are its transcript and 1 key frame, so you can"), "{m}");
            assert!(m.contains("- Title: Sintel\n- Link: https://b/1\n") && m.contains("## Transcript") && m.contains("(This video has no recognizable speech)"), "{m}");
            assert!(m.contains("## Key frames\n\nThe attached images are in time order, taken at 0:12."), "{m}");
        });
    }
}
