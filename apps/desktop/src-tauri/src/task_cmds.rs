//! 后台任务（转换、字幕、剪辑、拼接、预设试运行……，core tasks.rs）的操作。

use crate::App;
use tauri::State;

/// 取消一个还在跑的后台任务：任务的 future 被丢掉，ffmpeg 等子进程跟着结束，写了一半的文件删掉（core tasks::Partial），
/// 不会进素材库。之后会收到一条 task 事件，`st` 是 `cancel`，发起任务的那个命令返回错误 `CANCELLED`。
/// 返回 false：已经结束了（或者没有这个任务）。
#[tauri::command]
pub fn task_cancel(app: State<App>, id: u64) -> bool {
    let ok = app.core.cancel_task(id);
    tracing::info!("取消后台任务 {id}：{}", if ok { "已取消" } else { "已经结束了" });
    ok
}
