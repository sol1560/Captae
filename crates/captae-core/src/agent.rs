//! Agent 工具：MCP Server 和本地控制接口共用的工具定义与分发。
//!
//! 两层权限：
//! - 普通模式：只接受经过校验的参数，缺省值自动填充，产物从不覆盖已有文件
//!   （给了 `output` 且文件已存在时报 OUTPUT_EXISTS）
//! - 高级模式（设置里手动打开）：额外接受 `ffmpeg_args` / `ytdlp_args` / `aria2_args` / `aria2_threads`，
//!   直接透传给底层工具；convert / trim / subtitle 给了完整的 `output` 时可以传 `overwrite: true` 覆盖已有文件。
//!   给 Pi Agent 这类能自己调参重试的 agent 用
//!
//! 每次调用都记进素材库的 agent_calls 表，界面上「AI 最近做了什么」读的就是它。

use crate::ffmpeg::TrimOpts;
use crate::model::{AddRequest, Event, JobState};
use crate::queue::Core;
use crate::subtitle::{Source, SubOpts};
use crate::tr;
use anyhow::{anyhow, bail, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct CallRecord {
    pub id: i64,
    pub at: i64,
    pub who: String,
    pub tool: String,
    pub args: Value,
    /// run / ok / err
    pub st: String,
    pub res: Option<Value>,
    pub ms: Option<i64>,
}

pub struct ToolDef {
    pub name: &'static str,
    /// 界面上的人话描述（设计稿「AI 可以用的功能」）。存的是中文原文（表里用 tr_key! 标过），
    /// 给人看时用 `i18n::dynamic` 翻译（AI 客户端显示的 title 也是，见 mcp.rs）
    pub label: &'static str,
    /// 给 AI 看的说明：只用英文，不跟界面语言（docs/I18N.md 第 1 节）
    pub description: &'static str,
    pub schema: fn() -> Value,
    /// 只读工具（MCP annotations.readOnlyHint）
    pub read_only: bool,
}

/// 界面上显示的名字：library_query → captae.library.query，library_add_tag → captae.library.add_tag
/// （只把第一个下划线换成点，和设计稿一致）
pub fn display_name(n: &str) -> String {
    format!("captae.{}", n.replacen('_', ".", 1))
}

/// 用户是否单独关掉了这个工具。`tools_off` 里存的是界面上的名字，旧版本存过 captae.library.add.tag 这种写法，都认。
pub fn tool_off(s: &crate::config::Settings, name: &str) -> bool {
    s.tools_off.iter().any(|o| tool(o).map(|t| t.name == name).unwrap_or(false))
}

/// 工具失败时给 AI 的结构：`{"error": 大写代码, "message": 一句话（跟着界面语言）}`；下载一类的失败另带
/// `code`（错误码，见 errors.rs）、`action`（界面上的下一步）、`next`（给 AI 的下一步）、`raw`（原文）。
pub fn error_json(e: &anyhow::Error) -> Value {
    let (tag, message) = split_error(&e.to_string());
    let mut v = json!({"error": tag, "message": message});
    if let Some(f) = crate::errors::failure_in(e) {
        for (k, x) in f.to_json().as_object().into_iter().flatten() {
            if k != "message" {
                v[k] = x.clone();
            }
        }
    }
    v
}

/// "ACCESS_OFF: 用户关闭了…" → ("ACCESS_OFF", "用户关闭了…")；没有代码的算 FAILED。
pub fn split_error(msg: &str) -> (String, String) {
    match msg.split_once(": ") {
        Some((c, m)) if !c.is_empty() && c.chars().all(|x| x.is_ascii_uppercase() || x == '_') => (c.to_string(), m.to_string()),
        _ => ("FAILED".into(), msg.to_string()),
    }
}

// 给 AI 看的工具说明和参数说明只用英文，不跟界面语言（docs/I18N.md 第 1 节）
fn target_props() -> Value {
    json!({
        "path": {"type": "string", "description": "Local file path"},
        "asset_id": {"type": "integer", "description": "Id of an item in the Captae library (from library_query)"}
    })
}

fn merge(a: Value, b: Value) -> Value {
    let mut a = a;
    if let (Some(x), Some(y)) = (a.as_object_mut(), b.as_object()) {
        for (k, v) in y {
            x.insert(k.clone(), v.clone());
        }
    }
    a
}

fn adv_ffmpeg() -> Value {
    json!({"ffmpeg_args": {"type": "array", "items": {"type": "string"}, "description": "[Advanced mode] Extra ffmpeg arguments, added before the output file"}})
}

/// `what`：结果文件是什么（output file / subtitle file），写进给 AI 看的英文说明
fn output_props(what: &str) -> Value {
    json!({
        "output": {"type": "string", "description": format!("Full path of the {what}, including the file name. If left out, it's named automatically and saved next to the original file or in output_dir. If this file already exists, the call fails with OUTPUT_EXISTS; in normal mode Captae never overwrites files")},
        "overwrite": {"type": "boolean", "default": false, "description": "[Advanced mode] Overwrite the file given in output if it already exists (the original file itself can never be overwritten)"}
    })
}

pub const TOOLS: &[ToolDef] = &[
    ToolDef {
        name: "download",
        label: crate::tr_key!("下载视频、音乐或图片"),
        description: "Download a video, audio or image from a web page to the user's computer and add it to the Captae library. YouTube, Bilibili and 1500+ other sites go through yt-dlp, direct media links through aria2 with multiple connections, and M3U8 streams are merged into an MP4 automatically. By default it waits for the download to finish and returns the file path.",
        read_only: false,
        schema: || {
            json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "Video page URL, direct media link or m3u8 URL"},
                    "resolution": {"type": "string", "description": "Maximum resolution, such as 1080p or 720p. Leave it out for the best available"},
                    "format": {"type": "string", "description": "MP4 / WebM / MKV; for audio only, MP3 / AAC / FLAC", "default": "MP4"},
                    "audio_only": {"type": "boolean", "default": false},
                    "output_dir": {"type": "string", "description": "Folder to save to. Leave it out to save by type to Movies / Music / Pictures › Captae"},
                    "preset": {"type": "string", "description": "Process the file with one of the user's presets (see preset_list)"},
                    "wait": {"type": "boolean", "default": true, "description": "Wait for the download to finish before returning. With false, the job id is returned right away"},
                    "items": {"type": "array", "items": {"type": "string"}, "description": "Entries to download from a playlist or a post with several videos: the entries[].id or the number n returned earlier, or [\"all\"] for all of them. If left out and the link is a list, the call first returns status=choose with entries, so you (or the user) can choose"},
                    "record_live": {"type": "boolean", "default": false, "description": "If the link is a live stream in progress, record it from now on. Recording stops automatically after max_minutes, the recorded part is saved as a playable file in the library, and its path is returned (it also returns early if the stream ends or the user clicks “Stop and save” in Captae). Without it, a live stream stops the download with error code live"},
                    "max_minutes": {"type": "number", "exclusiveMinimum": 0, "maximum": 180, "default": 10, "description": "Maximum length in minutes when recording a live stream (record_live: true). Decimals are allowed. Defaults to 10 minutes, at most 180; more than 180 fails with BAD_ARGS"},
                    "ytdlp_args": {"type": "array", "items": {"type": "string"}, "description": "[Advanced mode] Extra yt-dlp arguments"},
                    "aria2_args": {"type": "array", "items": {"type": "string"}, "description": "[Advanced mode] Extra aria2c arguments"},
                    "aria2_threads": {"type": "integer", "minimum": 1, "maximum": 16, "description": "[Advanced mode] Number of aria2 threads"}
                },
                "required": ["url"]
            })
        },
    },
    ToolDef {
        name: "convert",
        label: crate::tr_key!("转换文件格式"),
        description: "Convert a local video, audio or image file to another format with ffmpeg. Video: MP4 / WebM / MOV / ProRes 422; audio: MP3 / AAC / FLAC / WAV; images: PNG / JPG / WebP / AVIF. When only the container needs to change, it doesn't re-encode. The result is added to the library.",
        read_only: false,
        schema: || {
            json!({"type": "object", "properties": merge(merge(target_props(), json!({
                "format": {"type": "string", "description": "Target format, such as MP4, ProRes 422, MP3 or WebP"},
                "output_dir": {"type": "string"}
            })), merge(output_props("output file"), adv_ffmpeg())), "required": ["format"]})
        },
    },
    ToolDef {
        name: "subtitle",
        label: crate::tr_key!("下载字幕或自动生成字幕"),
        description: "Get subtitles for a video: it downloads the site's own subtitles first, and if there are none, transcribes the speech locally with faster-whisper. It can export SRT / VTT / TXT, translate, and burn the subtitles into the video. The recognized text is saved to the library for search.",
        read_only: false,
        schema: || {
            json!({"type": "object", "properties": merge(merge(target_props(), json!({
                "source": {"type": "string", "enum": ["auto", "platform", "whisper"], "default": "auto"},
                "language": {"type": "string", "description": "Language code, such as zh or en. Leave it out to detect it automatically"},
                "format": {"type": "string", "enum": ["srt", "vtt", "txt"], "default": "srt"},
                "translate_to": {"type": "string", "description": "Language to translate into. Requires the user to set up a translation service in Captae's settings"},
                "burn": {"type": "boolean", "default": false, "description": "Also make a copy of the video with the subtitles burned in"}
            })), output_props("subtitle file"))})
        },
    },
    ToolDef {
        name: "trim",
        label: crate::tr_key!("截取视频片段"),
        description: "Cut a clip from a video by start and end time, optionally removing the audio or changing the resolution. By default it cuts quickly at keyframes; with precise=true it's frame-accurate (re-encodes).",
        read_only: false,
        schema: || {
            json!({"type": "object", "properties": merge(merge(target_props(), json!({
                "start": {"type": ["number", "string"], "description": "Start time: seconds, or mm:ss / hh:mm:ss"},
                "end": {"type": ["number", "string"], "description": "End time"},
                "mute": {"type": "boolean", "default": false},
                "precise": {"type": "boolean", "default": false},
                "height": {"type": "integer", "description": "Output height, such as 720"},
                "replace_audio": {"type": ["integer", "string"], "description": "Replace the audio with another sound: an audio item id or a local audio file path. If it's shorter than the clip, silence is added at the end"},
                "output_dir": {"type": "string"}
            })), merge(output_props("output file"), adv_ffmpeg())), "required": ["start", "end"]})
        },
    },
    ToolDef {
        name: "concat",
        label: crate::tr_key!("把几段视频拼成一个"),
        description: "Join several videos into one, in order. If codec, resolution and frame rate all match, they're joined losslessly; otherwise the frame size and frame rate are matched to the first video and the result is re-encoded to MP4. The result is added to the library.",
        read_only: false,
        schema: || {
            json!({"type": "object", "properties": {
                "inputs": {"type": "array", "minItems": 2, "items": {"type": ["integer", "string"]}, "description": "Videos to join, in playback order. Each item is a library item id (an integer, from library_query) or a local file path"},
                "output_dir": {"type": "string"}
            }, "required": ["inputs"]})
        },
    },
    ToolDef {
        name: "library_query",
        label: crate::tr_key!("在素材库里找东西"),
        description: "Search the user's local Captae library. Keywords match titles, descriptions, tags, sources and subtitle text; results can be filtered by type, source site, format, date and tags.",
        read_only: true,
        schema: || {
            json!({"type": "object", "properties": {
                "q": {"type": "string", "description": "Keywords"},
                "type": {"type": "string", "enum": ["video", "audio", "image"]},
                "source": {"type": "string", "description": "Source site as shown in the library, such as YouTube, 哔哩哔哩 (Bilibili) or blender.org"},
                "format": {"type": "string", "description": "Such as MP4, MP3 or JPG"},
                "days": {"type": "integer", "description": "Downloaded within the last N days"},
                "since": {"type": "string", "description": "Downloaded on or after this date (YYYY-MM-DD)"},
                "tags": {"type": "array", "items": {"type": "string"}},
                "limit": {"type": "integer", "default": 20, "maximum": 200}
            }})
        },
    },
    ToolDef {
        name: "library_add_tag",
        label: crate::tr_key!("给素材加标签"),
        description: "Add tags to an item in the library.",
        read_only: false,
        schema: || json!({"type": "object", "properties": merge(target_props(), json!({"tags": {"type": "array", "items": {"type": "string"}}})), "required": ["tags"]}),
    },
    ToolDef {
        name: "preset_run",
        label: crate::tr_key!("使用你的预设"),
        description: "Run one of the user's presets (a saved workflow) by name. With url, it runs the whole workflow starting with the download; with path or asset_id, it runs the steps after the download on an existing file.",
        read_only: false,
        schema: || json!({"type": "object", "properties": merge(target_props(), json!({"name": {"type": "string"}, "url": {"type": "string"}})), "required": ["name"]}),
    },
    ToolDef {
        name: "preset_list",
        label: crate::tr_key!("查看有哪些预设"),
        description: "List the presets the user allows AI to use, with their steps.",
        read_only: true,
        schema: || json!({"type": "object", "properties": {}}),
    },
    ToolDef {
        name: "share_video",
        label: crate::tr_key!("把视频整理成文字稿和关键帧"),
        description: "Turn a video into material an AI can read directly: the full transcript (with timestamps), several key frame images and metadata. Use it to understand what a video is about without processing the video yourself.",
        read_only: false,
        schema: || json!({"type": "object", "properties": merge(target_props(), json!({"url": {"type": "string"}, "max_frames": {"type": "integer", "minimum": 1, "maximum": 20, "description": "How many key frames to capture. Leave it out to let Captae choose by length (6 to 12, about one every 20 seconds)."}, "language": {"type": "string"}}))}),
    },
];

pub fn tool(name: &str) -> Option<&'static ToolDef> {
    let n = name.trim_start_matches("captae.").trim_start_matches("captae_").replace('.', "_");
    TOOLS.iter().find(|t| t.name == n)
}

fn s(a: &Value, k: &str) -> Option<String> {
    a.get(k).and_then(Value::as_str).map(str::to_string).filter(|s| !s.trim().is_empty())
}
fn strs(a: &Value, k: &str) -> Option<Vec<String>> {
    a.get(k).and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
}

/// "1:02" / "00:01:02.5" / 62 → 秒
pub fn parse_time(v: &Value) -> Result<f64> {
    if let Some(n) = v.as_f64() {
        return Ok(n);
    }
    let s = v.as_str().ok_or_else(|| anyhow!(tr!("时间格式不对")))?;
    let mut t = 0.0;
    for p in s.trim().split(':') {
        t = t * 60.0 + p.trim().parse::<f64>().map_err(|_| anyhow!(tr!("时间格式不对：{time}", time = s)))?;
    }
    Ok(t)
}

fn expand(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/") {
        if let Some(h) = directories::UserDirs::new() {
            return h.home_dir().join(rest);
        }
    }
    PathBuf::from(p)
}

/// 解析 path / asset_id，返回 (素材 id, 文件路径)。不在库里的本地文件先入库。
async fn target(core: &Core, a: &Value) -> Result<(i64, PathBuf)> {
    if let Some(id) = a.get("asset_id").and_then(Value::as_i64) {
        let x = core.lib.get(id)?.ok_or_else(|| anyhow!(tr!("素材库里没有 id 为 {id} 的素材", id = id)))?;
        if !x.exists {
            bail!(tr!("文件已经不在了：{path}", path = x.path.display()));
        }
        return Ok((id, x.path));
    }
    let p = expand(&s(a, "path").ok_or_else(|| anyhow!(tr!("需要 path 或 asset_id")))?);
    if !p.exists() {
        bail!("FILE_NOT_FOUND: {}", tr!("找不到文件 {path}", path = p.display()));
    }
    let id = match core.lib.by_path(&p)? {
        Some(x) => x.id,
        None => core.import_file(&p, None, None).await?,
    };
    Ok((id, p))
}

fn check_dir(d: &Option<String>) -> Result<Option<PathBuf>> {
    match d {
        None => Ok(None),
        Some(d) => {
            let p = expand(d);
            if std::fs::create_dir_all(&p).is_err() {
                bail!("OUTPUT_NOT_WRITABLE: {}", tr!("要保存到的位置「{path}」不能写入，可能是移动硬盘没有连接", path = p.display()));
            }
            Ok(Some(p))
        }
    }
}

fn asset_json(a: &crate::library::Asset) -> Value {
    json!({
        "asset_id": a.id, "title": a.title, "type": a.kind, "path": a.path, "url": if a.page.is_empty() { &a.url } else { &a.page },
        "source": a.src, "format": a.fmt, "resolution": a.res, "duration": a.duration, "size": a.size, "tags": a.tags,
        "downloaded": a.when, "has_transcript": a.has_transcript
    })
}

/// 执行一个工具调用，`client` 是 agent 的名字（来自 MCP initialize 的 clientInfo）。
pub async fn call(core: &Core, client: &str, name: &str, args: Value) -> Result<Value> {
    core.reload_settings();
    let st = core.settings();
    let def = tool(name).ok_or_else(|| anyhow!(tr!("没有这个工具：{name}", name = name)))?;
    let disp = display_name(def.name);
    let rid = core.lib.call_start(client, &disp, &args)?;
    let started = std::time::Instant::now();
    let emit_rec = |st: &str, res: Option<Value>, ms: Option<i64>| {
        core.emit(Event::AgentCall { call: CallRecord { id: rid, at: crate::util::now_ms(), who: client.to_string(), tool: disp.clone(), args: args.clone(), st: st.into(), res, ms } });
    };
    emit_rec("run", None, None);
    let adv_keys = ["ffmpeg_args", "ytdlp_args", "aria2_args", "aria2_threads"];
    // 大写的错误代码不翻（AI 和界面按它判断，split_error 拆得开）；冒号后面给人看的一句话跟着界面语言
    let r = if !st.mcp_on {
        Err(anyhow!("ACCESS_OFF: {}", tr!("用户在 Captae 里关闭了 AI 助手访问")))
    } else if tool_off(&st, def.name) {
        Err(anyhow!("TOOL_OFF: {}", tr!("用户关闭了「{tool}」这个功能", tool = crate::i18n::dynamic(def.label))))
    } else if !st.adv && (adv_keys.iter().any(|k| args.get(*k).is_some()) || flag(&args, "overwrite")) {
        Err(anyhow!("ADVANCED_MODE_OFF: {}", tr!("这些参数需要用户在 Captae 的「AI 助手」页里打开「高级模式」")))
    } else {
        dispatch(core, client, def.name, &args, st.adv).await
    };
    let ms = started.elapsed().as_millis() as i64;
    match &r {
        Ok(v) => {
            core.lib.call_end(rid, true, v, ms)?;
            emit_rec("ok", Some(v.clone()), Some(ms));
        }
        Err(e) => {
            let v = error_json(e);
            core.lib.call_end(rid, false, &v, ms)?;
            emit_rec("err", Some(v), Some(ms));
        }
    }
    r
}

/// download 的 `items`：条目 id 或序号，字符串和数字都认；`"all"` 表示全部。
fn item_list(a: &Value) -> Option<Vec<String>> {
    let v = a.get("items")?;
    let list: Vec<String> = match v {
        Value::Array(xs) => xs.iter().filter_map(|x| x.as_str().map(str::to_string).or_else(|| x.as_u64().map(|n| n.to_string()))).collect(),
        Value::String(x) => x.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect(),
        Value::Number(n) => vec![n.to_string()],
        _ => vec![],
    };
    (!list.is_empty()).then_some(list)
}

/// 一个下载任务的结果：完成的给路径和素材信息，失败的给错误码。
pub(crate) fn job_result(core: &Core, j: &crate::model::Job) -> Value {
    if j.state != JobState::Done {
        let f = j.fail.clone().unwrap_or_else(|| crate::errors::classify(j.err.as_deref().unwrap_or(&tr!("下载失败")), &j.url));
        return json!({"job_id": j.id, "title": j.title, "url": j.url, "ok": false, "error": f.to_json()});
    }
    let asset = j.asset_id.and_then(|id| core.lib.get(id).ok().flatten());
    let mut v = json!({
        "path": j.path, "asset_id": j.asset_id, "title": j.title, "engine": j.eng,
        // video / audio / image：单张图片按图片保存（第三阶段 FIXES 第 8 项），AI 据此知道拿到的是什么
        "type": asset.as_ref().map(|x| x.kind.clone()),
        "duration": asset.as_ref().and_then(|x| x.duration), "resolution": asset.as_ref().map(|x| x.res.clone()),
        "size": asset.as_ref().map(|x| x.size.clone())
    });
    // 录直播：录了多久、为什么停的
    if let Some(r) = &j.rec {
        // note 给 AI 转述给用户，跟着界面语言。full（录满设定的时长、自动停的）是固定字段：命令行按它判断，不看 note 的文字
        let note = match (r.full, r.interrupted, r.max) {
            (true, _, Some(m)) => tr!("录满设定的 {limit}，自动停止", limit = crate::queue::limit_label(m)),
            (_, true, _) => tr!("录制被中断（Captae 退出时还在录），录到的部分已经整理好"),
            (_, _, Some(m)) => tr!("录制在设定的 {limit} 以前停了：有人点了「停止并保存」，或者直播结束了", limit = crate::queue::limit_label(m)),
            _ => tr!("录制已经停了：有人点了「停止并保存」（或者在命令行里按了 Ctrl-C），或者直播结束了"),
        };
        v["recording"] = json!({"seconds": r.secs, "max_seconds": r.max, "full": r.full && r.max.is_some(), "note": note});
    }
    v
}

/// 命令行自己调用 download 时用的名字（crates/captae-cli 里 `b.call("命令行", …)`）：终端里的用户按 Ctrl-C 随时能停，
/// 录直播不套用给 AI 助手的默认时长和上限
pub const CLI_CLIENT: &str = "命令行";
/// AI 助手录直播：不说录多久时录 10 分钟，最多 180 分钟（不设上限的话，AI 的这次调用会一直挂着，等有人在界面上点停止）
pub const LIVE_DEFAULT_MIN: f64 = 10.0;
pub const LIVE_MAX_MIN: f64 = 180.0;

/// download 的 `max_minutes` → 最多录多少秒。AI 助手：不填按 10 分钟，超过 180 或者不是正数报 BAD_ARGS；
/// 命令行：不填就不限（一直录到 Ctrl-C），填了照填的（可以写小数，测试时录几秒）
fn live_limit(client: &str, a: &Value) -> Result<Option<u64>> {
    let mins = match a.get("max_minutes").filter(|v| !v.is_null()) {
        None if client == CLI_CLIENT => return Ok(None),
        None => LIVE_DEFAULT_MIN,
        Some(v) => v.as_f64().or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).ok_or_else(|| anyhow!("BAD_ARGS: {}", tr!("max_minutes 要写分钟数，比如 30")))?,
    };
    if !(mins.is_finite() && mins > 0.0) {
        bail!("BAD_ARGS: {}", tr!("max_minutes 要大于 0（单位是分钟）"));
    }
    if client != CLI_CLIENT && mins > LIVE_MAX_MIN {
        bail!("BAD_ARGS: {}", tr!("录直播最多录 {max} 分钟，max_minutes 请写 {max} 以内", max = LIVE_MAX_MIN));
    }
    // 按秒取整（0.1 分钟 × 60 在浮点里不一定正好是 6）
    Ok(Some(((mins * 60.0).round() as u64).max(1)))
}

fn flag(a: &Value, k: &str) -> bool {
    a.get(k).and_then(Value::as_bool).unwrap_or(false)
}

/// 高级模式的额外参数交给实际下载这个链接的引擎：网页 → yt-dlp，直链 → aria2，M3U8 → 都不接受。
pub fn raw_args_for(kind: Option<crate::detect::Kind>, ytdlp: Option<Vec<String>>, aria2: Option<Vec<String>>) -> Result<Option<Vec<String>>> {
    use crate::detect::Kind;
    match kind {
        Some(Kind::Page) | None => {
            if ytdlp.is_none() && aria2.is_some() {
                bail!("BAD_ARGS: {}", tr!("这个链接由 yt-dlp 下载，aria2_args 用不上，请改用 ytdlp_args"));
            }
            Ok(ytdlp)
        }
        Some(Kind::File) => {
            if aria2.is_none() && ytdlp.is_some() {
                bail!("BAD_ARGS: {}", tr!("这是媒体直链，由 aria2 下载，ytdlp_args 用不上，请改用 aria2_args"));
            }
            Ok(aria2)
        }
        Some(Kind::Hls) => {
            if ytdlp.is_some() || aria2.is_some() {
                bail!("BAD_ARGS: {}", tr!("M3U8 由 Captae 自己的引擎下载，不接受 yt-dlp / aria2 参数"));
            }
            Ok(None)
        }
    }
}

/// 按 `output` 算出最终的输出文件，检查覆盖规则：
/// - 必须是完整路径、扩展名要和产物一致（没写扩展名时自动补上）
/// - 不能是原文件本身
/// - 已存在时：普通模式一律拒绝；高级模式要显式传 overwrite: true
pub fn resolve_output(src: &Path, output: &str, ext: &str, adv: bool, overwrite: bool) -> Result<PathBuf> {
    let mut out = expand(output);
    if !out.is_absolute() {
        bail!("BAD_OUTPUT: {}", tr!("output 要写完整路径，比如 ~/Movies/片段.mp4"));
    }
    if out.is_dir() || output.ends_with('/') {
        bail!("BAD_OUTPUT: {}", tr!("output 要写到文件名；只想指定文件夹请用 output_dir"));
    }
    match out.extension() {
        None => {
            out.set_extension(ext);
        }
        Some(e) if !e.to_string_lossy().eq_ignore_ascii_case(ext) => bail!("BAD_OUTPUT: {}", tr!("这个结果是 .{ext} 文件，output 的扩展名要一致", ext = ext)),
        _ => {}
    }
    let same = match (std::fs::canonicalize(src), std::fs::canonicalize(&out)) {
        (Ok(a), Ok(b)) => a == b,
        _ => src == out,
    };
    if same {
        bail!("BAD_OUTPUT: {}", tr!("不能用结果覆盖原文件本身"));
    }
    if out.exists() {
        if !adv {
            bail!("OUTPUT_EXISTS: {}", tr!("「{path}」已经存在。普通模式下 Captae 不会覆盖已有文件，请换个文件名", path = out.display()));
        }
        if !overwrite {
            bail!("OUTPUT_EXISTS: {}", tr!("「{path}」已经存在。确实要覆盖的话传 overwrite: true", path = out.display()));
        }
    }
    Ok(out)
}

/// 把产物移到最终位置。`replace` 为 false 时，目标在这期间冒出来也不覆盖。
async fn place(produced: &Path, dst: &Path, replace: bool) -> Result<()> {
    if !replace && dst.exists() {
        bail!("OUTPUT_EXISTS: {}", tr!("「{path}」已经存在，Captae 不会覆盖它", path = dst.display()));
    }
    if tokio::fs::rename(produced, dst).await.is_err() {
        tokio::fs::copy(produced, dst).await.map_err(|e| anyhow!("OUTPUT_NOT_WRITABLE: {}", tr!("写不进「{path}」：{error}", path = dst.display(), error = e)))?;
        let _ = tokio::fs::remove_file(produced).await;
    }
    Ok(())
}

/// 先在目标旁边的临时文件夹里生成，成功后再换到 `dst`（失败时不会留下半个文件，也不会先删掉旧文件）。
async fn produce_at<F, Fut>(dst: &Path, replace: bool, make: F) -> Result<PathBuf>
where
    F: FnOnce(PathBuf) -> Fut,
    Fut: std::future::Future<Output = Result<PathBuf>>,
{
    let parent = dst.parent().ok_or_else(|| anyhow!("BAD_OUTPUT: {}", tr!("output 路径不对")))?;
    let tmp = parent.join(format!(".captae-{}-{}", std::process::id(), crate::util::now_ms()));
    if std::fs::create_dir_all(&tmp).is_err() {
        bail!("OUTPUT_NOT_WRITABLE: {}", tr!("要保存到的位置「{path}」不能写入，可能是移动硬盘没有连接", path = parent.display()));
    }
    let r = async {
        let produced = make(tmp.clone()).await?;
        place(&produced, dst, replace).await
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&tmp).await;
    r.map(|_| dst.to_path_buf())
}

async fn dispatch(core: &Core, client: &str, name: &str, a: &Value, adv: bool) -> Result<Value> {
    match name {
        "download" => {
            let url = s(a, "url").ok_or_else(|| anyhow!(tr!("需要 {name}", name = "url")))?;
            let raw_args = raw_args_for(crate::detect::detect(&url).map(|d| d.kind), strs(a, "ytdlp_args"), strs(a, "aria2_args"))?;
            let record_live = flag(a, "record_live");
            let req = AddRequest {
                url,
                record_live,
                record_max_secs: if record_live { live_limit(client, a)? } else { None },
                quality: s(a, "resolution"),
                format: s(a, "format"),
                audio_only: a.get("audio_only").and_then(Value::as_bool).unwrap_or(false),
                preset: s(a, "preset"),
                output_dir: check_dir(&s(a, "output_dir"))?,
                origin: Some(format!("agent:{client}")),
                raw_args,
                threads: a.get("aria2_threads").and_then(Value::as_u64).map(|x| x as u32),
                ..Default::default()
            };
            let items = item_list(a);
            let wait = a.get("wait").and_then(Value::as_bool).unwrap_or(true);
            // 同一个链接已经有一个停在「等选条目」的任务（上一次调用列出了条目）：直接在它上面选，不再重新读取
            let pending = items.as_ref().and_then(|_| core.jobs().into_iter().find(|j| j.choose && j.url == req.url));
            let j = match pending {
                Some(j) => j,
                None => {
                    let job = core.add(req)?;
                    if !wait && items.is_none() {
                        return Ok(json!({"job_id": job.id, "status": "queued"}));
                    }
                    core.wait(job.id).await?
                }
            };
            if j.choose {
                let Some(items) = items else {
                    return Ok(json!({
                        "job_id": j.id, "status": "choose", "title": j.title, "entries": j.entries,
                        "message": tr!("这里有 {n} 个视频。用同一个 url 再调用一次 download，在 items 里写要下载的条目（entries 里的 id 或序号 n），全部下载写 [\"all\"]", n = j.entries.len())
                    }));
                };
                let jobs = core.choose(j.id, &items)?;
                if !wait {
                    return Ok(json!({"status": "queued", "job_ids": jobs.iter().map(|x| x.id).collect::<Vec<_>>()}));
                }
                let mut out = vec![];
                let mut failed = 0;
                for nj in jobs {
                    let d = core.wait(nj.id).await?;
                    if d.state != JobState::Done {
                        failed += 1;
                    }
                    out.push(job_result(core, &d));
                }
                return Ok(json!({"items": out, "count": out.len(), "failed": failed}));
            }
            if j.state != JobState::Done {
                let failure = j.fail.clone().unwrap_or_else(|| crate::errors::classify(j.err.as_deref().unwrap_or(&tr!("下载失败")), &j.url));
                return Err(anyhow::Error::new(crate::errors::Tagged { tag: "DOWNLOAD_FAILED".into(), failure }));
            }
            Ok(job_result(core, &j))
        }
        "convert" => {
            let (id, p) = target(core, a).await?;
            let f = s(a, "format").ok_or_else(|| anyhow!(tr!("需要 {name}", name = "format")))?;
            let dst = match s(a, "output") {
                Some(o) => Some(resolve_output(&p, &o, crate::ffmpeg::target(&f)?.0, adv, flag(a, "overwrite"))?),
                None => None,
            };
            let dir = check_dir(&s(a, "output_dir"))?;
            let extra = strs(a, "ffmpeg_args").unwrap_or_default();
            let tools = core.toolbox();
            let replace = flag(a, "overwrite") && adv;
            let out = core
                .run_task("convert", format!("{} → {f}", p.file_name().unwrap_or_default().to_string_lossy()), Some(id), |rep| async move {
                    let out = match dst {
                        Some(dst) => produce_at(&dst, replace, |tmp| async move { crate::ffmpeg::convert(&tools, &p, &f, Some(&tmp), extra, |x| rep.pct(x)).await }).await?,
                        None => crate::ffmpeg::convert(&tools, &p, &f, dir.as_deref(), extra, |x| rep.pct(x)).await?,
                    };
                    Ok(json!(out))
                })
                .await?;
            let out: PathBuf = serde_json::from_value(out)?;
            let nid = core.import_file(&out, Some(id), None).await?;
            Ok(json!({"path": out, "asset_id": nid}))
        }
        "subtitle" => {
            let (id, p) = target(core, a).await?;
            let asset = core.lib.get(id)?.unwrap();
            let url = if asset.page.is_empty() { asset.url.clone() } else { asset.page.clone() };
            let o = SubOpts {
                source: match s(a, "source").as_deref() {
                    Some("platform") => Source::Platform,
                    Some("whisper") => Source::Whisper,
                    _ => Source::Auto,
                },
                language: s(a, "language"),
                format: s(a, "format").unwrap_or_else(|| "srt".into()),
                translate_to: s(a, "translate_to"),
                model: None,
            };
            let dst = match s(a, "output") {
                Some(out) => Some(resolve_output(&p, &out, crate::subtitle::render(&[], &o.format)?.1, adv, flag(a, "overwrite"))?),
                None => None,
            };
            let replace = flag(a, "overwrite") && adv;
            let burn = a.get("burn").and_then(Value::as_bool).unwrap_or(false);
            let c2 = core.clone();
            let p2 = p.clone();
            // 任务标题先算好：带参数的 tr! 有个临时参数表（&dyn Display，不是 Send），不能跨过下面的 .await
            let title = tr!("{title} 的字幕", title = asset.title);
            let r = core
                .run_task("subtitle", title, Some(id), |rep| async move {
                    let k = if burn { 0.6 } else { 1.0 };
                    let mut r = crate::subtitle::run(&c2, id, &url, &p2, &o, |x| rep.pct(x * k)).await?;
                    if let Some(dst) = dst {
                        if let Err(e) = place(&r.path, &dst, replace).await {
                            let _ = tokio::fs::remove_file(&r.path).await;
                            return Err(e);
                        }
                        r.path = dst;
                    }
                    let mut v = serde_json::to_value(&r)?;
                    // 嵌进画面用字幕内容本身（导出成 TXT 也能烧录）
                    if burn {
                        let burned = crate::subtitle::burn(&c2, &p2, &r.list, |x| rep.pct(0.6 + x * 0.4)).await?;
                        let bid = c2.import_file(&burned, Some(id), None).await?;
                        v["burned_path"] = json!(burned);
                        v["burned_asset_id"] = json!(bid);
                    }
                    Ok(v)
                })
                .await?;
            Ok(r)
        }
        "trim" => {
            let (id, p) = target(core, a).await?;
            let o = TrimOpts {
                start: parse_time(a.get("start").ok_or_else(|| anyhow!(tr!("需要 {name}", name = "start")))?)?,
                end: parse_time(a.get("end").ok_or_else(|| anyhow!(tr!("需要 {name}", name = "end")))?)?,
                mute: a.get("mute").and_then(Value::as_bool).unwrap_or(false),
                precise: a.get("precise").and_then(Value::as_bool).unwrap_or(false),
                scale_height: a.get("height").and_then(Value::as_u64).map(|x| x as u32),
                replace_audio: match a.get("replace_audio") {
                    None | Some(Value::Null) => None,
                    Some(v) => {
                        let t = match v {
                            Value::Number(n) => json!({"asset_id": n}),
                            Value::String(x) => match x.trim().parse::<i64>() {
                                Ok(n) if !std::path::Path::new(x).exists() => json!({"asset_id": n}),
                                _ => json!({"path": x}),
                            },
                            _ => bail!(tr!("replace_audio 要是素材 id 或文件路径")),
                        };
                        Some(target(core, &t).await?.1)
                    }
                },
                ..Default::default()
            };
            let dur = o.end - o.start;
            let ext = p.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_else(|| "mp4".into());
            let dst = match s(a, "output") {
                Some(out) => Some(resolve_output(&p, &out, &ext, adv, flag(a, "overwrite"))?),
                None => None,
            };
            let dir = check_dir(&s(a, "output_dir"))?;
            let extra = strs(a, "ffmpeg_args").unwrap_or_default();
            let tools = core.toolbox();
            let out = match dst {
                Some(dst) => produce_at(&dst, flag(a, "overwrite") && adv, |tmp| async move { crate::ffmpeg::trim(&tools, &p, &o, Some(&tmp), extra, |_| {}).await }).await?,
                None => crate::ffmpeg::trim(&tools, &p, &o, dir.as_deref(), extra, |_| {}).await?,
            };
            let nid = core.import_file(&out, Some(id), None).await?;
            Ok(json!({"path": out, "asset_id": nid, "duration": dur}))
        }
        "concat" => {
            let items = a.get("inputs").and_then(Value::as_array).ok_or_else(|| anyhow!(tr!("需要 inputs：两个以上的素材 id 或文件路径")))?;
            if items.len() < 2 {
                bail!(tr!("至少需要两段视频"));
            }
            let mut files = vec![];
            for it in items {
                let t = match it {
                    Value::Number(n) => json!({"asset_id": n}),
                    Value::String(p) => match p.trim().parse::<i64>() {
                        Ok(id) if !std::path::Path::new(p).exists() => json!({"asset_id": id}),
                        _ => json!({"path": p}),
                    },
                    _ => bail!(tr!("inputs 里的每一项要是素材 id 或文件路径")),
                };
                files.push(target(core, &t).await?);
            }
            let dir = check_dir(&s(a, "output_dir"))?;
            let first = files[0].0;
            let paths: Vec<PathBuf> = files.iter().map(|(_, p)| p.clone()).collect();
            let tools = core.toolbox();
            let title = tr!("拼接 {n} 段视频", n = paths.len());
            let out = core
                .run_task("concat", title, Some(first), |rep| async move {
                    let out = crate::ffmpeg::concat(&tools, &paths, dir.as_deref(), |x| rep.pct(x)).await?;
                    Ok(json!(out))
                })
                .await?;
            let out: PathBuf = serde_json::from_value(out)?;
            let nid = core.import_file(&out, Some(first), None).await?;
            let dur = core.lib.get(nid)?.and_then(|x| x.duration);
            Ok(json!({"path": out, "asset_id": nid, "duration": dur, "inputs": files.iter().map(|(id, _)| id).collect::<Vec<_>>()}))
        }
        "library_query" => {
            let mut q: crate::library::Query = serde_json::from_value(a.clone()).unwrap_or_default();
            q.limit = Some(q.limit.unwrap_or(20).min(200));
            let (items, total) = core.lib.query(&q)?;
            Ok(json!({"count": total, "items": items.iter().map(asset_json).collect::<Vec<_>>()}))
        }
        "library_add_tag" => {
            let (id, _) = target(core, a).await?;
            let tags = strs(a, "tags").ok_or_else(|| anyhow!(tr!("需要 {name}", name = "tags")))?;
            let all = core.lib.add_tags(id, &tags)?;
            core.emit(Event::Library);
            Ok(json!({"asset_id": id, "tags": all}))
        }
        "preset_list" => {
            let v: Vec<Value> = core.presets().into_iter().filter(|p| p.agent).map(|p| json!({"name": p.name, "steps": p.steps})).collect();
            Ok(json!({"presets": v}))
        }
        "preset_run" => {
            let n = s(a, "name").ok_or_else(|| anyhow!(tr!("需要 {name}", name = "name")))?;
            let p = core.preset(&n).ok_or_else(|| anyhow!("PRESET_NOT_FOUND: {}", tr!("没有叫「{name}」的预设", name = n)))?;
            if !p.agent {
                bail!("PRESET_NOT_ALLOWED: {}", tr!("用户没有允许 AI 使用预设「{name}」", name = n));
            }
            if let Some(url) = s(a, "url") {
                let job = core.add(AddRequest { url, preset: Some(n), origin: Some(format!("agent:{client}")), ..Default::default() })?;
                let j = core.wait(job.id).await?;
                if j.state != JobState::Done {
                    bail!("PRESET_FAILED: {}", j.err.unwrap_or_default());
                }
                return Ok(json!({"output": j.path, "asset_id": j.asset_id}));
            }
            let (id, _) = target(core, a).await?;
            let outs = crate::presets::run_post(core, &p, id, None).await?;
            Ok(json!({"outputs": outs, "output": outs.last()}))
        }
        "share_video" => {
            let o = crate::share::ShareOpts {
                url: s(a, "url"),
                asset_id: match (a.get("asset_id"), s(a, "path")) {
                    (Some(_), _) | (None, Some(_)) => Some(target(core, a).await?.0),
                    _ => None,
                },
                max_frames: a.get("max_frames").and_then(Value::as_u64).map(|x| x as usize),
                language: s(a, "language"),
                copy: false,
                ..Default::default()
            };
            let b = crate::share::share(core, o, |_, _| {}).await?;
            Ok(json!({"markdown": b.markdown, "frames": b.frames, "bundle_dir": b.dir, "asset_id": b.asset_id, "transcript_source": b.transcript_source}))
        }
        _ => bail!(tr!("没有这个工具：{name}", name = name)),
    }
}

/// 设计稿里调用记录一行的「结果」文字（跟着界面语言）。
pub fn describe(v: &Value) -> String {
    if let Some(p) = v.get("path").or(v.get("output")).and_then(Value::as_str) {
        return tr!("已保存到 {path}", path = p);
    }
    if let Some(n) = v.get("count") {
        return tr!("找到 {n} 个", n = n);
    }
    tr!("已完成§调用记录一行的结果")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names() {
        assert_eq!(display_name("library_query"), "captae.library.query");
        assert_eq!(display_name("library_add_tag"), "captae.library.add_tag");
        assert_eq!(display_name("download"), "captae.download");
        for t in TOOLS {
            assert_eq!(tool(&display_name(t.name)).unwrap().name, t.name);
        }
        assert_eq!(tool("captae.library.add.tag").unwrap().name, "library_add_tag");
        assert!(tool("captae.library.query").is_some());
        assert!(tool("library_add_tag").is_some());
        assert!(tool("nope").is_none());
        for t in TOOLS {
            let s = (t.schema)();
            assert_eq!(s["type"], "object", "{}", t.name);
            // Claude 等客户端要求工具名只含字母数字下划线
            assert!(t.name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        }
    }
    #[test]
    fn times() {
        assert_eq!(parse_time(&json!(62)).unwrap(), 62.0);
        assert_eq!(parse_time(&json!("1:02")).unwrap(), 62.0);
        assert_eq!(parse_time(&json!("01:00:01.5")).unwrap(), 3601.5);
        assert!(parse_time(&json!("x")).is_err());
    }

    #[test]
    fn download_items() {
        assert_eq!(item_list(&json!({"items": ["1", 3, "all"]})), Some(vec!["1".to_string(), "3".into(), "all".into()]));
        assert_eq!(item_list(&json!({"items": "2, 5"})), Some(vec!["2".to_string(), "5".into()]));
        assert_eq!(item_list(&json!({"items": 4})), Some(vec!["4".to_string()]));
        assert_eq!(item_list(&json!({"items": []})), None);
        assert_eq!(item_list(&json!({})), None);
    }

    /// 收尾修补第 2 项：录直播最多录多久。AI 助手不填 10 分钟、最多 180；命令行不填就不限
    #[test]
    fn live_limits() {
        let ai = "Claude Desktop";
        assert_eq!(live_limit(ai, &json!({})).unwrap(), Some(600));
        assert_eq!(live_limit(ai, &json!({"max_minutes": null})).unwrap(), Some(600));
        assert_eq!(live_limit(ai, &json!({"max_minutes": 30})).unwrap(), Some(1800));
        assert_eq!(live_limit(ai, &json!({"max_minutes": "45"})).unwrap(), Some(2700));
        assert_eq!(live_limit(ai, &json!({"max_minutes": 0.5})).unwrap(), Some(30));
        assert_eq!(live_limit(ai, &json!({"max_minutes": 180})).unwrap(), Some(10800));
        for bad in [json!(181), json!(0), json!(-3), json!("很久"), json!(true)] {
            let e = live_limit(ai, &json!({ "max_minutes": bad })).unwrap_err().to_string();
            assert!(e.starts_with("BAD_ARGS: "), "{bad} → {e}");
        }
        assert!(live_limit(ai, &json!({"max_minutes": 300})).unwrap_err().to_string().contains("180"));
        // 命令行：不填不限，填了照填（可以超过 180，可以是几秒）
        assert_eq!(live_limit(CLI_CLIENT, &json!({})).unwrap(), None);
        assert_eq!(live_limit(CLI_CLIENT, &json!({"max_minutes": 300})).unwrap(), Some(18000));
        assert_eq!(live_limit(CLI_CLIENT, &json!({"max_minutes": 0.1})).unwrap(), Some(6));
        assert!(live_limit(CLI_CLIENT, &json!({"max_minutes": 0})).is_err());
    }

    #[test]
    fn errors_and_toggles() {
        assert_eq!(split_error("TOOL_OFF: 关了"), ("TOOL_OFF".into(), "关了".into()));
        assert_eq!(split_error("找不到: x"), ("FAILED".into(), "找不到: x".into()));
        let s = crate::config::Settings { tools_off: vec!["captae.library.add_tag".into()], ..Default::default() };
        assert!(tool_off(&s, "library_add_tag"));
        assert!(!tool_off(&s, "library_query"));
    }

    #[test]
    fn raw_args_routing() {
        use crate::detect::Kind;
        let a = Some(vec!["-x".to_string()]);
        assert_eq!(raw_args_for(Some(Kind::Page), a.clone(), None).unwrap(), a);
        assert_eq!(raw_args_for(Some(Kind::File), None, a.clone()).unwrap(), a);
        assert_eq!(raw_args_for(Some(Kind::File), a.clone(), a.clone()).unwrap(), a);
        assert!(raw_args_for(Some(Kind::Page), None, a.clone()).is_err());
        assert!(raw_args_for(Some(Kind::File), a.clone(), None).is_err());
        assert!(raw_args_for(Some(Kind::Hls), a.clone(), None).is_err());
        assert_eq!(raw_args_for(Some(Kind::Hls), None, None).unwrap(), None);
    }

    #[test]
    fn overwrite_rules() {
        let d = tempfile::tempdir().unwrap();
        let src = d.path().join("a.mp4");
        std::fs::write(&src, b"src").unwrap();
        let taken = d.path().join("b.mp4");
        std::fs::write(&taken, b"old").unwrap();
        let o = |x: &str| d.path().join(x).display().to_string();
        // 新文件：普通模式也可以；没写扩展名自动补
        assert_eq!(resolve_output(&src, &o("c.mp4"), "mp4", false, false).unwrap(), d.path().join("c.mp4"));
        assert_eq!(resolve_output(&src, &o("c"), "mp4", false, false).unwrap(), d.path().join("c.mp4"));
        // 已存在：普通模式一律拒绝，overwrite 也不行（overwrite 在 call 里就会被 ADVANCED_MODE_OFF 挡住）
        let e = resolve_output(&src, &o("b.mp4"), "mp4", false, true).unwrap_err().to_string();
        assert!(e.starts_with("OUTPUT_EXISTS"), "{e}");
        // 高级模式：没说 overwrite 还是拒绝，说了才允许
        assert!(resolve_output(&src, &o("b.mp4"), "mp4", true, false).unwrap_err().to_string().starts_with("OUTPUT_EXISTS"));
        assert_eq!(resolve_output(&src, &o("b.mp4"), "mp4", true, true).unwrap(), taken);
        // 任何模式都不能覆盖原文件本身
        assert!(resolve_output(&src, &o("a.mp4"), "mp4", true, true).unwrap_err().to_string().starts_with("BAD_OUTPUT"));
        // 扩展名不一致、相对路径、文件夹
        assert!(resolve_output(&src, &o("c.mov"), "mp4", true, true).unwrap_err().to_string().starts_with("BAD_OUTPUT"));
        assert!(resolve_output(&src, "c.mp4", "mp4", true, true).unwrap_err().to_string().starts_with("BAD_OUTPUT"));
        assert!(resolve_output(&src, &d.path().display().to_string(), "mp4", true, true).unwrap_err().to_string().starts_with("BAD_OUTPUT"));
    }

    #[tokio::test]
    async fn produce_replaces_only_when_allowed() {
        let d = tempfile::tempdir().unwrap();
        let dst = d.path().join("out.txt");
        let make = |body: &'static str| move |tmp: PathBuf| async move {
            let p = tmp.join("x.txt");
            tokio::fs::write(&p, body).await?;
            Ok(p)
        };
        produce_at(&dst, false, make("one")).await.unwrap();
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "one");
        // 不允许覆盖：旧文件原样保留
        assert!(produce_at(&dst, false, make("two")).await.is_err());
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "one");
        produce_at(&dst, true, make("three")).await.unwrap();
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "three");
        // 生成失败：不留下临时文件夹，旧文件不动
        let r = produce_at(&dst, true, |_tmp: PathBuf| async { Err::<PathBuf, _>(anyhow!("ffmpeg 失败")) }).await;
        assert!(r.is_err());
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), "three");
        let left: Vec<_> = std::fs::read_dir(d.path()).unwrap().flatten().map(|e| e.file_name()).collect();
        assert_eq!(left.len(), 1, "{left:?}");
    }
}
