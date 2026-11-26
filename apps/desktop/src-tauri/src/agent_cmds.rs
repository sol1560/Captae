//! 「AI 助手」页用的命令：MCP 连接状态、写入 Claude / Cursor 配置、导出 Skill、预设试运行。

use super::{e, mcp_command, App, R};
use captae_core::mcp;
use captae_core::presets::{self, Preset};
use captae_core::tr;
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager, State};

/// 写给 AI 助手看的 Skill 说明，和仓库里的 skills/captae/SKILL.md 是同一份。
const SKILL_MD: &str = include_str!("../../../../skills/captae/SKILL.md");

fn home(app: &AppHandle) -> R<PathBuf> {
    app.path().home_dir().map_err(|_| tr!("找不到用户目录"))
}

fn config_path(app: &AppHandle, client: &str) -> R<PathBuf> {
    mcp::client_config_path(client, &home(app)?).ok_or_else(|| tr!("不支持的客户端：{client}", client = client))
}

/// 连上来的 AI 助手：名字、版本、最近一次动静、现在是否连着。
#[tauri::command]
pub fn mcp_status(st: State<App>) -> Value {
    let now = captae_core::util::now_ms();
    json!({"now": now, "clients": mcp::client_status(&st.core.paths.data, now)})
}

/// 页面一次要用的静态信息：用户目录（缩短路径用）、配置文件位置、MCP 命令、Skill 全文。
#[tauri::command]
pub fn agent_info(app: AppHandle, st: State<App>) -> R<Value> {
    let p = &st.core.paths;
    Ok(json!({
        "home": home(&app)?,
        "dirs": {"movies": p.movies, "music": p.music, "pictures": p.pictures, "shares": p.shares},
        "configs": {"claude": config_path(&app, "claude")?, "cursor": config_path(&app, "cursor")?},
        "mcpCommand": mcp_command(),
        "skill": SKILL_MD,
    }))
}

/// 「一键写入」前给用户看：会改哪个文件、合并进去的是什么。
#[tauri::command]
pub fn mcp_config_plan(app: AppHandle, client: String) -> R<mcp::ConfigPlan> {
    let p = config_path(&app, &client)?;
    mcp::plan_config(&p, &mcp::server_entry(&mcp_command())).map_err(e)
}

/// 用户确认后合并写入（原文件备份成 .bak，其他服务器保留）。
#[tauri::command]
pub fn mcp_config_write(app: AppHandle, client: String) -> R<mcp::ConfigWritten> {
    let p = config_path(&app, &client)?;
    mcp::write_config(&p, &mcp::server_entry(&mcp_command())).map_err(e)
}

async fn pick(app: &AppHandle, title: &str) -> R<Option<PathBuf>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title(title).pick_folder(move |f| {
        let _ = tx.send(f);
    });
    match rx.await.map_err(e)? {
        Some(d) => Ok(Some(d.into_path().map_err(e)?)),
        None => Ok(None),
    }
}

/// 选一个文件夹（不改任何设置），预设的「移动到指定文件夹」用。
#[tauri::command]
pub async fn pick_folder(app: AppHandle, title: Option<String>) -> R<Option<String>> {
    // 标题由界面传来（界面已经按语言翻译好）；没传时用默认的
    let title = title.unwrap_or_else(|| tr!("选择文件夹"));
    Ok(pick(&app, &title).await?.map(|p| p.display().to_string()))
}

/// 把 Skill 导出到用户选的文件夹：<文件夹>/captae/SKILL.md。已有不同内容的同名文件时先备份。
#[tauri::command]
pub async fn export_skill(app: AppHandle) -> R<Option<String>> {
    let Some(dir) = pick(&app, &tr!("导出 Skill 到…")).await? else { return Ok(None) };
    let dir = dir.join("captae");
    std::fs::create_dir_all(&dir).map_err(|x| tr!("这个文件夹不能写入：{err}", err = x))?;
    let f = dir.join("SKILL.md");
    if f.exists() && std::fs::read_to_string(&f).map(|t| t != SKILL_MD).unwrap_or(true) {
        let b = captae_core::util::unique_path(dir.join("SKILL.md.bak"));
        std::fs::rename(&f, &b).map_err(e)?;
    }
    std::fs::write(&f, SKILL_MD).map_err(e)?;
    Ok(Some(f.display().to_string()))
}

/// 预设试运行：拿素材库里的一个素材（没指定就用最近下载的）真的跑一遍下载之后的步骤。
/// 立即返回任务编号；每一步的开始和结果通过 `preset_step` 事件推给页面，整体进度走普通的 task 事件。
#[tauri::command]
pub async fn preset_trial(app: AppHandle, st: State<'_, App>, preset: Preset, asset_id: Option<i64>) -> R<Value> {
    let core = st.core.clone();
    let asset = match asset_id {
        Some(id) => core.lib.get(id).map_err(e)?.ok_or_else(|| tr!("素材不存在"))?,
        None => {
            let q = captae_core::library::Query { limit: Some(50), ..Default::default() };
            let (items, _) = core.lib.query(&q).map_err(e)?;
            items.into_iter().find(|a| a.exists && a.kind != "image").ok_or_else(|| tr!("素材库里还没有能用来试运行的视频或音频"))?
        }
    };
    if !asset.exists {
        return Err(tr!("文件已经不在了：{path}", path = asset.path.display()));
    }
    let aid = asset.id;
    let id = core.new_task_id();
    let title = tr!("试运行「{name}」· {title}", name = preset.name, title = asset.title);
    tauri::async_runtime::spawn(async move {
        let c2 = core.clone();
        let _ = core
            .run_task_as(id, "preset", title, Some(aid), |rep| async move {
                let n = preset.steps.len().max(1) as f64;
                let r = presets::run_steps(&c2, &preset, aid, true, |s| {
                    let _ = app.emit("captae://event", json!({"type": "preset_step", "task": id, "step": s}));
                    if s.status != "run" {
                        rep.pct((s.index + 1) as f64 / n);
                    }
                })
                .await;
                match r.failed {
                    Some(f) => Err(anyhow::anyhow!(f)),
                    None => Ok(serde_json::to_value(r)?),
                }
            })
            .await;
    });
    Ok(json!({"task": id, "assetId": aid, "title": asset.title}))
}
