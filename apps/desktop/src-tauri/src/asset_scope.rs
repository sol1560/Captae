//! asset 协议（页面里的 <img>、<video> 读本机文件，`convertFileSrc`）能读哪些地方。
//!
//! 原来在 tauri.conf.json 里写死整个主目录（`$HOME/**`），主窗口里的页面能读主目录下的任何文件（收尾清单第 15 项）。
//! 现在配置里的范围是空的，启动以后按需放行：
//! - 启动时：数据目录里的缩略图（封面、胶片条，`<数据目录>/thumbs`），和保存位置（设置里选的，以及影片 / 音乐 / 图片下的 Captae）；
//! - 用到时：素材所在的文件夹（素材库加载列表、打开某一条素材、下载完入库、改了保存位置）。只放行那一层文件夹，不连子文件夹。
//!
//! 数据目录本身不放行：里面有素材库、设置、control.json（本地控制接口的令牌），页面用不着读它们。
//! 登录窗口网站那块另有内容拦截规则挡住 asset:（login.rs 的 blocker），和这里无关。

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::Manager;

/// 已经放行过的（文件夹、连不连子文件夹）：同一个文件夹不重复登记，范围里的条目不会越积越多
#[derive(Default)]
pub struct Allowed(Mutex<HashSet<(PathBuf, bool)>>);

/// 放行一个文件夹。`recursive`：连子文件夹（缩略图、保存位置）；素材所在的文件夹只放行那一层
pub fn allow_dir(app: &tauri::AppHandle, dir: &Path, recursive: bool) {
    if dir.as_os_str().is_empty() || !dir.is_absolute() {
        return;
    }
    let Some(st) = app.try_state::<Allowed>() else { return };
    {
        let mut seen = st.0.lock().unwrap();
        // 已经连子文件夹一起放行过的上层，不用再登记
        if seen.iter().any(|(d, rec)| (*rec && dir.starts_with(d)) || (d == dir && (*rec || !recursive))) {
            return;
        }
        seen.insert((dir.to_path_buf(), recursive));
    }
    match app.asset_protocol_scope().allow_directory(dir, recursive) {
        Ok(()) => tracing::debug!("asset 协议放行 {}{}", dir.display(), if recursive { "（连子文件夹）" } else { "" }),
        Err(err) => tracing::warn!("asset 协议没能放行 {}：{err}", dir.display()),
    }
}

/// 一个文件所在的文件夹（素材、下载好的文件）
pub fn allow_parent(app: &tauri::AppHandle, file: &Path) {
    if let Some(d) = file.parent() {
        allow_dir(app, d, false);
    }
}

/// 启动时：缩略图（封面、胶片条）和保存位置
pub fn init(app: &tauri::AppHandle, core: &captae_core::Core) {
    app.manage(Allowed::default());
    let p = &core.paths;
    allow_dir(app, &p.thumbs, true);
    for d in [&p.movies, &p.music, &p.pictures] {
        allow_dir(app, d, true);
    }
    allow_save_dir(app, &core.settings());
}

/// 设置里选的保存位置（改了保存位置以后新下的也能显示）
pub fn allow_save_dir(app: &tauri::AppHandle, s: &captae_core::Settings) {
    if !s.save_dir.trim().is_empty() {
        allow_dir(app, Path::new(s.save_dir.trim()), true);
    }
}

/// 素材库的一批素材：各自所在的文件夹（预览播放、图片素材的缩略图直接用原文件）；封面在缩略图目录里的启动时已经放行
pub fn allow_assets<'a>(app: &tauri::AppHandle, items: impl IntoIterator<Item = &'a captae_core::library::Asset>) {
    for a in items {
        allow_parent(app, &a.path);
        if let Some(t) = &a.thumb {
            allow_parent(app, t);
        }
    }
}

/// core 的事件里带着新文件的：下载完入库、改了保存位置
pub fn on_event(app: &tauri::AppHandle, ev: &captae_core::Event) {
    match ev {
        captae_core::Event::Finished { path, .. } => allow_parent(app, path),
        captae_core::Event::Settings { settings } => allow_save_dir(app, settings),
        _ => {}
    }
}
