//! 素材库的分类和「用 AI 整理」（core categorize.rs、library.rs 的分类部分）。

use crate::{e, App, R};
use captae_core::categorize::{self, Edit};
use captae_core::tr;
use serde_json::{json, Value};
use tauri::State;

/// 全部分类（各带条数）和未分类的条数
#[tauri::command]
pub fn library_categories(app: State<App>) -> R<Value> {
    let (items, none) = app.core.lib.categories().map_err(e)?;
    Ok(json!({"items": items, "none": none}))
}

/// 新建（不带 id）或改名、改说明。用户建的、用户改过名的都算用户的：重新整理时留着
#[tauri::command]
pub fn category_save(app: State<App>, id: Option<i64>, name: String, desc: Option<String>) -> R<i64> {
    let lib = &app.core.lib;
    let id = match id {
        Some(id) => {
            let (cats, _) = lib.categories().map_err(e)?;
            let cur = cats.iter().find(|c| c.id == id).ok_or_else(|| tr!("这个分类已经不在了"))?;
            let renamed = cur.name != name.trim();
            lib.update_category(id, renamed.then_some(name.as_str()), desc.as_deref(), renamed.then_some("user")).map_err(e)?;
            id
        }
        None => lib.add_category(&name, desc.as_deref().unwrap_or(""), "user").map_err(e)?,
    };
    app.core.emit_library();
    Ok(id)
}

/// 删掉分类，里面的素材放回未分类
#[tauri::command]
pub fn category_delete(app: State<App>, id: i64) -> R<()> {
    app.core.lib.delete_category(id).map_err(e)?;
    app.core.emit_library();
    Ok(())
}

#[tauri::command]
pub fn category_merge(app: State<App>, from: i64, into: i64) -> R<()> {
    app.core.lib.merge_category(from, into).map_err(e)?;
    app.core.emit_library();
    Ok(())
}

#[tauri::command]
pub fn category_reorder(app: State<App>, ids: Vec<i64>) -> R<()> {
    app.core.lib.reorder_categories(&ids).map_err(e)?;
    app.core.emit_library();
    Ok(())
}

/// 用户亲手把素材放进分类（cat 为空 = 未分类）：之后自动整理不再动它们
#[tauri::command]
pub fn asset_set_category(app: State<App>, ids: Vec<i64>, cat: Option<i64>) -> R<()> {
    app.core.lib.set_category(&ids, cat, "user").map_err(e)?;
    app.core.emit_library();
    Ok(())
}

/// 开始整理前：会发多少条、发给谁、大约多少 token
#[tauri::command]
pub fn categorize_estimate(app: State<App>) -> R<categorize::Estimate> {
    categorize::estimate(&app.core).map_err(e)
}

/// 整理（后台任务，kind = categorize，进度走 task 事件）：返回提议，用户确认前不改素材库
#[tauri::command]
pub async fn categorize_propose(st: State<'_, App>) -> R<Value> {
    let core = st.core.clone();
    let c2 = core.clone();
    core.run_task("categorize", tr!("用 AI 整理素材库"), None, |rep| async move { Ok(serde_json::to_value(categorize::propose(&c2, &rep).await?)?) })
        .await
        .map_err(e)
}

/// 用户确认了提议（edits：改名、合并、删掉）
#[tauri::command]
pub fn categorize_apply(app: State<App>, edits: Vec<Edit>) -> R<categorize::Applied> {
    categorize::apply(&app.core, &edits).map_err(e)
}

/// 不要这次的提议了
#[tauri::command]
pub fn categorize_discard(app: State<App>) {
    categorize::discard(&app.core);
}

/// 「给未分类的归类」：只往已有的分类里放（后台任务）
#[tauri::command]
pub async fn categorize_rest(st: State<'_, App>) -> R<Value> {
    let core = st.core.clone();
    let c2 = core.clone();
    let r = core
        .run_task("categorize", tr!("给未分类的素材归类"), None, |rep| async move {
            let (filed, none) = categorize::file_items(&c2, &captae_core::library::Pick::Uncategorized, Some(&rep)).await?;
            Ok(json!({"filed": filed, "none": none}))
        })
        .await
        .map_err(e)?;
    core.emit_library();
    Ok(r)
}

/// 清掉所有 AI 加的标签
#[tauri::command]
pub fn ai_tags_clear(app: State<App>) -> R<u64> {
    let n = app.core.lib.clear_ai_tags().map_err(e)?;
    app.core.emit_library();
    Ok(n)
}

/// 设置里的「测试连接」：通了没有、能不能看图（记进设置的 llmVision）
#[tauri::command]
pub async fn llm_test(st: State<'_, App>) -> R<Value> {
    let core = st.core.clone();
    let llm = captae_core::llm::Llm::from_settings(&core.settings()).ok_or_else(|| tr!("先填上地址和模型名"))?;
    let r = captae_core::llm::test(&core.http(), &llm).await.map_err(e)?;
    core.patch_settings(&json!({"llmVision": if r.vision { "yes" } else { "no" }})).map_err(e)?;
    Ok(json!(r))
}
