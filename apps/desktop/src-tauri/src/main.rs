//! Captae 桌面端（Tauri 2）。
//!
//! 界面是 React 单页应用（apps/desktop/src，`npm run build` 输出到 apps/desktop/dist，打包时嵌进来），这里只做三件事：
//! 1. 启动 captae-core（下载队列、素材库、剪贴板监听）和本地控制接口（给 CLI / MCP / 浏览器插件用）
//! 2. 把 core 的事件转发给页面（`captae://event`）
//! 3. 提供页面调用的命令

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use captae_core::ffmpeg::{ConvertOpts, Quality, TrimOpts};
use captae_core::library::Query;
use captae_core::presets::{Preset, Step};
use captae_core::subtitle::SubOpts;
use captae_core::tr;
use captae_core::{AddRequest, Core, Job, JobAction, Options, Paths, Settings};
use serde_json::{json, Value};
use std::path::PathBuf;
use tauri::{Emitter, Manager, State};

mod agent_cmds;
mod app_update;
mod asset_scope;
mod clip_ask;
mod ext_cmds;
mod login;
mod media_server;
mod remote_cmds;
#[cfg(target_os = "macos")]
mod menu;
mod open_browser;
mod task_cmds;

type R<T> = Result<T, String>;
fn e<E: std::fmt::Display>(x: E) -> String {
    x.to_string()
}

mod library_cmds;

struct App {
    core: Core,
}

fn mcp_command() -> String {
    // 装在 PATH 上就写 captae，否则给出安装包里的完整路径
    if captae_core::tools::which("captae").is_some() {
        return "captae".into();
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| captae_core::native_host::cli_next_to(&p))
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "captae".into())
}

#[tauri::command]
fn boot(app: State<App>) -> R<Value> {
    let c = &app.core;
    let tools: Vec<Value> = captae_core::agent::TOOLS.iter().map(|t| json!({"name": captae_core::agent::display_name(t.name), "label": t.label})).collect();
    let sniffed: Vec<Value> = c.sniffed().into_iter().map(|s| serde_json::to_value(s).unwrap()).collect();
    Ok(json!({
        // 界面语言：settings.language 为空（跟随系统）时用 systemLang；lang 是现在实际用的（i18n.rs）
        "systemLang": captae_core::i18n::system().code(),
        "lang": captae_core::i18n::current().code(),
        "settings": c.settings(),
        "jobs": c.jobs(),
        "sniffed": sniffed,
        "presets": c.presets(),
        "libraryCount": c.lib.count().map_err(e)?,
        "tools": tools,
        "calls": c.lib.calls(100).map_err(e)?,
        "mcpCommand": mcp_command(),
    }))
}

/// 只传改动的字段：`{"concurrency": 4}`。返回合并后的完整设置。
#[tauri::command]
fn set_settings(handle: tauri::AppHandle, app: State<App>, patch: Value) -> R<Settings> {
    let s = app.core.patch_settings(&patch).map_err(e)?;
    // 改了界面语言（patch_settings 里已经换好）：macOS 的菜单栏按新的语言重建
    #[cfg(target_os = "macos")]
    if patch.get("language").is_some() {
        menu::apply(&handle);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = handle;
    Ok(s)
}

#[tauri::command]
async fn deps_status(st: State<'_, App>) -> R<Value> {
    let tools = st.core.toolbox();
    let mut v = json!(tools.status().await);
    // 语音转文字：顺带告诉界面哪些模型已经下载好了
    if let Some(w) = v.as_array_mut().and_then(|a| a.iter_mut().find(|d| d["key"] == "whisper")) {
        w["models"] = json!(st.core.whisper_models());
    }
    Ok(v)
}

/// 一键安装语音识别（faster-whisper + 模型），作为后台任务跑，进度走 task 事件（kind = setup）。
#[tauri::command]
async fn install_whisper(st: State<'_, App>, model: Option<String>) -> R<Value> {
    let core = st.core.clone();
    let model = model.filter(|m| !m.is_empty()).unwrap_or_else(|| core.settings().whisper_model);
    let c2 = core.clone();
    // 组件已经能用、只是这个模型还没下载：只下模型
    let ready = core.toolbox().status().await.iter().any(|d| d.key == "whisper" && d.ok);
    core.run_task("setup", tr!("语音转文字"), None, |rep| async move {
        if ready {
            c2.fetch_whisper_model(&model, |p, note, eta| rep.progress(p, note, eta)).await?;
            return Ok(json!({"model": model}));
        }
        let py = c2.install_whisper(&model, |p, note, eta| rep.progress(p, note, eta)).await?;
        Ok(json!({"python": py, "model": model}))
    })
    .await
    .map_err(e)
}

/// 插件文件夹：安装包里随附在 Resources/extension，开发时用仓库里的 extension/。
/// 仓库的位置是编译时写进程序的绝对路径（里面有编译人的用户名），只在调试版里编译，正式版只认安装包里的（scripts/build-release.sh）
fn extension_dir(app: &tauri::AppHandle) -> Option<PathBuf> {
    let bundled = app.path().resource_dir().ok().map(|d| d.join("extension"));
    // Linux 的 AppImage：用拷到数据目录里的那份（ext_cmds::stable_extension_dir）
    let bundled = match (bundled, app.try_state::<App>()) {
        (Some(b), Some(a)) if b.join("manifest.json").exists() => Some(ext_cmds::stable_extension_dir(b, &a.core.paths.data)),
        (b, _) => b,
    };
    #[cfg(debug_assertions)]
    let dev = Some(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../extension"));
    #[cfg(not(debug_assertions))]
    let dev: Option<PathBuf> = None;
    [bundled, dev].into_iter().flatten().find(|p| p.join("manifest.json").exists()).map(|p| p.canonicalize().unwrap_or(p))
}

#[tauri::command]
fn extension_path(app: tauri::AppHandle) -> R<PathBuf> {
    extension_dir(&app).ok_or_else(|| tr!("没有找到插件文件夹"))
}

/// 插件还没上架商店：在文件管理器（访达 / 文件资源管理器）里显示插件文件夹，并用对应浏览器打开扩展程序页面，用户照着「加载已解压的扩展程序」装。
/// 认得的浏览器和插件宿主登记的是同一张表（`native_host::browsers`）；`browser` 是表里的 id（chrome / edge-beta / vivaldi / aside……）。
/// 怎么把浏览器起起来按系统分（open_browser.rs）
#[tauri::command]
fn extension_install_help(app: tauri::AppHandle, browser: Option<String>) -> R<PathBuf> {
    let dir = extension_dir(&app).ok_or_else(|| tr!("没有找到插件文件夹"))?;
    let _ = reveal_path(&dir);
    let id = browser.unwrap_or_else(|| "chrome".into());
    let list = captae_core::native_host::browsers(&captae_core::native_host::home_dir());
    // macOS 上表里没有这一家时退回 Chrome（和原来一样）；Windows / Linux 照样按 id 去找
    let b = list.iter().find(|b| b.id == id).or_else(|| if cfg!(target_os = "macos") { list.iter().find(|b| b.id == "chrome") } else { None });
    let id = b.map(|b| b.id).unwrap_or(&id);
    open_browser::open(id, b, extensions_page(id))?;
    Ok(dir)
}

/// 各家浏览器的扩展程序页面。Chromium 系的分支大多也认 chrome://，认不出的一律用它
fn extensions_page(id: &str) -> &'static str {
    match id {
        x if x.starts_with("edge") => "edge://extensions",
        "brave" => "brave://extensions",
        "arc" => "arc://extensions",
        "vivaldi" => "vivaldi://extensions",
        x if x.starts_with("opera") => "opera://extensions",
        _ => "chrome://extensions",
    }
}

/// 来源链接等外部网址用系统浏览器打开（Tauri 里 target=_blank 不会自己开）。
#[tauri::command]
fn open_url(url: String) -> R<()> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err(tr!("只能打开网页链接"));
    }
    tauri_plugin_opener::open_url(url, None::<&str>).map_err(e)
}

#[tauri::command]
fn set_presets(app: State<App>, presets: Vec<Preset>) -> R<()> {
    app.core.set_presets(presets).map_err(e)
}

#[tauri::command]
fn preset_check(preset: Preset) -> Vec<bool> {
    preset.steps.iter().map(|s| !matches!(Step::parse(s), Step::Unknown(_))).collect()
}

#[tauri::command]
fn take_sniffed(app: State<App>, url: String) {
    app.core.take_sniffed(&url);
}

#[tauri::command]
fn library_count(app: State<App>) -> R<u64> {
    app.core.lib.count().map_err(e)
}

#[tauri::command]
fn add_job(app: State<App>, req: AddRequest) -> R<Job> {
    app.core.add(req).map_err(e)
}

#[tauri::command]
fn job_action(app: State<App>, id: u64, action: JobAction) -> R<()> {
    app.core.act(id, action).map_err(e)
}

#[tauri::command]
fn set_variant(app: State<App>, id: u64, idx: usize) -> R<()> {
    app.core.set_variant(id, idx).map_err(e)
}

#[tauri::command]
fn pause_all(app: State<App>) {
    app.core.pause_all();
}

#[tauri::command]
fn resume_all(app: State<App>) {
    app.core.resume_all();
}

#[tauri::command]
fn reveal(path: Option<PathBuf>) -> R<()> {
    let p = path.ok_or_else(|| tr!("文件还没有生成"))?;
    reveal_path(&p)
}

/// 在文件管理器里显示（选中这个文件）。
/// Linux 上 opener 走 D-Bus 的 org.freedesktop.FileManager1（Nautilus、Dolphin、Nemo 都有），没有的话走桌面门户打开文件夹；
/// 两样都没有（精简的桌面、窗口管理器）时再退一步，用默认程序（xdg-open）打开所在的文件夹，不选中
fn reveal_path(p: &std::path::Path) -> R<()> {
    match tauri_plugin_opener::reveal_item_in_dir(p) {
        Ok(()) => Ok(()),
        #[cfg(all(unix, not(target_os = "macos")))]
        Err(err) => {
            tracing::info!("没能在文件管理器里选中 {}（{err}），改成打开所在的文件夹", p.display());
            let dir = if p.is_dir() { p } else { p.parent().unwrap_or(p) };
            tauri_plugin_opener::open_path(dir, None::<&str>).map_err(e)
        }
        #[cfg(not(all(unix, not(target_os = "macos"))))]
        Err(err) => Err(e(err)),
    }
}

#[tauri::command]
fn open_path(path: PathBuf) -> R<()> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(e)
}

#[tauri::command]
fn copy_text(text: String) -> R<()> {
    // 调试版的测试开关 CAPTAE_TEST_NO_CLIPBOARD：不写系统剪贴板，只记一行（不记内容）
    if captae_core::clipboard::writes_disabled() {
        tracing::info!("测试开关 CAPTAE_TEST_NO_CLIPBOARD 开着：没有写系统剪贴板（{} 字）", text.chars().count());
        return Ok(());
    }
    clip::set_text(&text)
}

#[tauri::command]
fn read_clipboard() -> R<String> {
    clip_ask::read_text().ok_or_else(|| tr!("剪贴板里没有文字"))
}

mod clip {
    use clipboard_rs::{Clipboard, ClipboardContext};
    pub fn set_text(t: &str) -> Result<(), String> {
        ClipboardContext::new().and_then(|c| c.set_text(t.to_string())).map_err(|e| e.to_string())
    }
}

#[tauri::command]
async fn choose_save_dir(app: tauri::AppHandle, st: State<'_, App>) -> R<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title(tr!("选择保存位置")).pick_folder(move |f| {
        let _ = tx.send(f);
    });
    let Some(dir) = rx.await.map_err(e)? else { return Ok(None) };
    let dir = dir.into_path().map_err(e)?;
    st.core.patch_settings(&json!({"saveDir": dir.display().to_string()})).map_err(e)?;
    Ok(Some(dir.display().to_string()))
}

#[tauri::command]
fn save_dir_label(app: State<App>) -> String {
    let s = app.core.settings();
    if s.save_dir.is_empty() {
        // 系统里那个文件夹的叫法：macOS「影片」，Windows、Linux（GNOME、KDE 的中文界面）「视频」
        if cfg!(target_os = "macos") {
            tr!("影片 › Captae")
        } else {
            tr!("视频 › Captae")
        }
    } else {
        s.save_dir
    }
}

#[tauri::command]
fn window_ctl(window: tauri::Window, action: String) -> R<()> {
    match action.as_str() {
        "close" => window.close(),
        "min" => window.minimize(),
        "max" => {
            if window.is_maximized().unwrap_or(false) {
                window.unmaximize()
            } else {
                window.maximize()
            }
        }
        _ => Ok(()),
    }
    .map_err(e)
}

/// 素材库一页：`items` 是这一页，`total` 是符合筛选的总数，`all` 是库里一共多少，
/// `sites` / `tags` 给筛选条用。翻页用 q.limit / q.offset。
#[tauri::command]
fn library_query(handle: tauri::AppHandle, app: State<App>, q: Query) -> R<Value> {
    let lib = &app.core.lib;
    let (items, total) = lib.query(&q).map_err(e)?;
    // 这一页素材所在的文件夹（网格里图片素材直接用原文件、预览播放用原文件）
    asset_scope::allow_assets(&handle, &items);
    Ok(json!({"items": items, "total": total, "all": lib.count().map_err(e)?, "sites": lib.sites().map_err(e)?, "tags": lib.tags().map_err(e)?}))
}

#[tauri::command]
fn library_tags(app: State<App>, id: i64, add: Option<String>, remove: Option<String>) -> R<Vec<String>> {
    let lib = &app.core.lib;
    let tags = if let Some(t) = remove { lib.remove_tag(id, &t) } else { lib.add_tags(id, &add.into_iter().collect::<Vec<_>>()) }.map_err(e)?;
    app.core.emit_library();
    Ok(tags)
}

fn asset_path(core: &Core, id: i64) -> R<(captae_core::library::Asset, PathBuf)> {
    let a = core.lib.get(id).map_err(e)?.ok_or_else(|| tr!("素材不存在"))?;
    if !a.exists {
        return Err(tr!("文件已经不在了：{path}", path = a.path.display()));
    }
    let p = a.path.clone();
    Ok((a, p))
}

#[tauri::command]
async fn task_convert(st: State<'_, App>, id: i64, format: String, height: Option<u32>, quality: Option<String>) -> R<Value> {
    let core = st.core.clone();
    let (a, p) = asset_path(&core, id)?;
    let opts = ConvertOpts { height, quality: Quality::parse(quality.as_deref()) };
    let tools = core.toolbox();
    let f2 = format.clone();
    let out = core
        .run_task("convert", format!("{} → {format}", a.title), Some(id), |rep| async move {
            let o = captae_core::ffmpeg::convert_with(&tools, &p, &f2, None, &opts, vec![], |x| rep.pct(x)).await?;
            Ok(json!(o))
        })
        .await
        .map_err(e)?;
    let out: PathBuf = serde_json::from_value(out).map_err(e)?;
    let nid = core.import_file(&out, Some(id), None).await.map_err(e)?;
    Ok(json!({"path": out, "assetId": nid}))
}

#[tauri::command]
async fn task_subtitle(st: State<'_, App>, id: i64, opts: SubOpts, burn: bool) -> R<Value> {
    let core = st.core.clone();
    let (a, p) = asset_path(&core, id)?;
    let url = if a.page.is_empty() { a.url.clone() } else { a.page.clone() };
    let c2 = core.clone();
    let p2 = p.clone();
    // 任务标题先单独算好：tr! 的参数是借用（&dyn Display，不能跨线程），不能一直留到下面的 .await
    let title = tr!("{title} 的字幕", title = a.title);
    let r = core
        .run_task("subtitle", title, Some(id), |rep| async move {
            let r = captae_core::subtitle::run(&c2, id, &url, &p2, &opts, |x| rep.pct(x * if burn { 0.6 } else { 1.0 })).await?;
            let mut v = serde_json::to_value(&r)?;
            v["preview"] = json!(r.list.iter().take(6).map(|c| (captae_core::util::clock(c.start).max("0:00".into()), c.text.clone())).collect::<Vec<_>>());
            if burn {
                // 导出的格式可能是 TXT（没有时间轴），烧录一律用字幕内容另写的 SRT
                let b = captae_core::subtitle::burn(&c2, &p2, &r.list, |x| rep.pct(0.6 + x * 0.4)).await?;
                v["burnedId"] = json!(c2.import_file(&b, Some(id), None).await?);
                v["burned"] = json!(b);
            }
            Ok(v)
        })
        .await
        .map_err(e)?;
    Ok(r)
}

#[tauri::command]
async fn task_trim(st: State<'_, App>, id: i64, opts: TrimOpts) -> R<Value> {
    let core = st.core.clone();
    let (a, p) = asset_path(&core, id)?;
    let tools = core.toolbox();
    // 任务标题先单独算好（同 task_subtitle）
    let title = tr!("{title} 片段", title = a.title);
    let out = core
        .run_task("trim", title, Some(id), |rep| async move {
            let o = captae_core::ffmpeg::trim(&tools, &p, &opts, None, vec![], |x| rep.pct(x)).await?;
            Ok(json!(o))
        })
        .await
        .map_err(e)?;
    let out: PathBuf = serde_json::from_value(out).map_err(e)?;
    let nid = core.import_file(&out, Some(id), None).await.map_err(e)?;
    Ok(json!({"path": out, "assetId": nid}))
}

fn show_main(app: &tauri::AppHandle) {
    // 开发调试：测试版不把窗口拿到前面（插件实测时点「在 Captae 里查看」「打开 Captae」会让测试版弹到屏幕最上面、抢走焦点，
    // 用户在用电脑时会被打扰、甚至当成自己的 Captae 来用）。页面照样跳到要去的地方，用调试入口读页面核对。正式版不包含
    #[cfg(debug_assertions)]
    if std::env::var_os("CAPTAE_TEST_NO_SHOW").is_some() {
        tracing::info!("测试开关 CAPTAE_TEST_NO_SHOW 开着：没有把窗口拿到前面");
        return;
    }
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

fn fit_to_screen(w: &tauri::WebviewWindow) {
    let Ok(Some(m)) = w.current_monitor().or_else(|_| w.primary_monitor()) else { return };
    let scale = m.scale_factor();
    let area = m.work_area();
    let (aw, ah) = (area.size.width as f64 / scale, area.size.height as f64 / scale);
    let Ok(size) = w.outer_size() else { return };
    let (ww, wh) = (size.width as f64 / scale, size.height as f64 / scale);
    if ww <= aw && wh <= ah {
        return;
    }
    let nw = ww.min(aw * 0.94).max(360.0);
    let nh = wh.min(ah * 0.94).max(560.0);
    let _ = w.set_size(tauri::LogicalSize::new(nw, nh));
    let _ = w.center();
}

/// 插件、命令行要求打开窗口时带的目的地（core 的 `FocusTarget::href`，比如 /library?asset=5），
/// 交给页面自己跳（`window.captaeGo`，见 src/lib/route.ts），不重新加载页面。
fn go_to(app: &tauri::AppHandle, href: &str) {
    let Some(w) = app.get_webview_window("main") else { return };
    let js = format!("window.captaeGo && window.captaeGo({})", serde_json::to_string(&route_of(href)).unwrap_or_default());
    let _ = w.eval(&js);
}

/// 路由照原样用；旧界面的地址也认（app-library.html#asset=5 → /library?asset=5，app-downloads.html#paste → /downloads?paste，
/// app-agent.html → /agent），以免哪里还有旧写法。认不出的页面一律去下载页。
fn route_of(href: &str) -> String {
    let (path, hash) = match href.split_once('#') {
        Some(x) => x,
        None => href.split_once('?').unwrap_or((href, "")),
    };
    let page = path.trim_start_matches('/').trim_start_matches("app-").trim_end_matches(".html");
    let page = if matches!(page, "downloads" | "library" | "agent" | "extension") { page } else { "downloads" };
    if hash.is_empty() {
        format!("/{page}")
    } else {
        format!("/{page}?{hash}")
    }
}

/// 写进 `<数据目录>/app_path`，插件宿主在桌面端没开时用它把桌面端拉起来
fn app_path() -> Option<PathBuf> {
    // Linux 的 AppImage：程序实际跑在每次启动都换地方的临时挂载点里（/tmp/.mount_XXXX/usr/bin/…），
    // 要写的是 .AppImage 文件本身（AppImage 的运行时把它放在环境变量 APPIMAGE 里）
    #[cfg(target_os = "linux")]
    if let Some(p) = std::env::var_os("APPIMAGE").map(PathBuf::from).filter(|p| p.is_file()) {
        return Some(p);
    }
    let exe = std::env::current_exe().ok()?;
    // macOS：写 .app 包路径，给 `open -a` 用
    if cfg!(target_os = "macos") {
        if let Some(app) = exe.ancestors().find(|p| p.extension().map(|e| e == "app").unwrap_or(false)) {
            return Some(app.to_path_buf());
        }
    }
    // Windows：安装目录里的 captae-desktop.exe；Linux 的 deb：/usr/bin/captae-desktop（current_exe 已经解开了符号链接）
    Some(exe)
}

/// 页面脚本出错时写进日志，方便排查用户反馈。
#[tauri::command]
fn js_log(level: String, msg: String) {
    if level == "error" {
        tracing::error!(target: "ui", "{msg}");
    } else {
        tracing::info!(target: "ui", "{msg}");
    }
}

// ------------------------------------------------------------------ 下载可靠性（第一阶段）新加的命令

/// 下载引擎（yt-dlp）的状态：版本、来源（custom / managed / bundled / system）、自动更新的记录、JS 运行时。
#[tauri::command]
async fn engine_status(st: State<'_, App>) -> R<Value> {
    let c = &st.core;
    Ok(captae_core::updater::status(&c.toolbox(), &c.paths.data, c.settings().auto_update).await)
}

/// 「检查更新」：查 GitHub 上 yt-dlp 的最新版，有就下载、验签、自检后换上。`force` 为真时同一版本也重装一遍。
#[tauri::command]
async fn engine_update(st: State<'_, App>, force: Option<bool>) -> R<captae_core::updater::Report> {
    use captae_core::updater::{check, How};
    let c = st.core.clone();
    let how = if force.unwrap_or(false) { How::Reinstall } else { How::Manual };
    Ok(check(&c.toolbox(), &c.http(), &c.paths.data, how).await)
}

/// 现在走的代理：{mode, http, https, noProxy, pac, system, aria2}。
#[tauri::command]
async fn proxy_status(st: State<'_, App>) -> R<Value> {
    Ok(captae_core::net::status(&st.core.settings()))
}

/// 播放列表 / 多视频帖子选好了条目：每个条目单独成一个下载任务，原来等选择的那个任务去掉。
/// `ids` 是 `entries[].id`（也认序号 `n`），`["all"]` 表示全部。返回新建的任务。
#[tauri::command]
fn choose_entries(app: State<App>, id: u64, ids: Vec<String>) -> R<Vec<Job>> {
    app.core.choose(id, &ids).map_err(e)
}

/// 数据目录：设了 CAPTAE_HOME（测试版）就用它下面的，否则用正式的
fn data_paths() -> anyhow::Result<Paths> {
    match std::env::var_os("CAPTAE_HOME") {
        Some(h) => Ok(Paths::portable(&PathBuf::from(h))),
        None => Paths::default_for_user(),
    }
}

fn main() {
    // Linux：Chrome 给它启动的程序（插件宿主，宿主再拉起 Captae）设 DBUS_SESSION_BUS_ADDRESS=disabled:，
    // 单实例插件连会话总线时认不得这个写法，一启动就 panic（2026-11-25 Devin 上插件拉起 AppImage 实测）。
    // 去掉它，按默认位置（$XDG_RUNTIME_DIR/bus）找会话总线。要在起任何线程之前做
    #[cfg(target_os = "linux")]
    if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok_and(|v| v.trim().is_empty() || v.starts_with("disabled")) {
        std::env::remove_var("DBUS_SESSION_BUS_ADDRESS");
    }
    // 日志写 stderr（和命令行一样）：插件宿主拉起桌面端时把 stderr 接到 launch.log，Windows 的图形程序 stdout 常常没有地方去
    tracing_subscriber::fmt().with_writer(std::io::stderr).with_env_filter(tracing_subscriber::EnvFilter::from_env("CAPTAE_LOG")).init();
    let args: Vec<String> = std::env::args().collect();
    let background = args.iter().any(|a| a == "--background");
    // --page library|agent：直接打开某一页（托盘菜单、系统通知点进来用）
    let start_page = args.iter().position(|a| a == "--page").and_then(|i| args.get(i + 1)).cloned();
    // 界面语言：macOS 的菜单栏在 setup 之前就建好装上（menu.rs），先按设置定下来（Core::open 里会按同一份设置再定一次）
    captae_core::i18n::apply(&data_paths().map(|p| Settings::load(&p.settings()).language).unwrap_or_default());
    let builder = tauri::Builder::default();
    // macOS：菜单栏换成自己建的、跟着界面语言走的（Tauri 默认的那份是写死的英文）；其他平台没有菜单栏，保持现状
    #[cfg(target_os = "macos")]
    let builder = builder.menu(menu::initial);
    builder
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| show_main(app)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // Captae 自己的更新（app_update.rs）：只在 Rust 里用，页面不直接调插件（没给 updater:* 权限）
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(move |app| {
            let paths = data_paths()?;
            let sidecar = std::env::current_exe().ok().and_then(|p| captae_core::tools::sidecar_dir_of(&p));
            let core = tauri::async_runtime::block_on(Core::open(Options { paths: paths.clone(), sidecar_dir: sidecar, restore: true }))?;
            let _ = core.lib.interrupt_running_calls();
            // 页面能用 asset 协议读的地方：缩略图和保存位置，素材所在的文件夹用到时再放行（asset_scope.rs）
            asset_scope::init(app.handle(), &core);
            // Linux：素材库播放走本机小服务（WebKitGTK 放不了 asset 协议的视频，见 media_server.rs）
            #[cfg(target_os = "linux")]
            media_server::MediaServer::start(app.handle());
            // 登录窗口的存储交给核心库（下载时按网站读 cookies），越早越好：恢复的任务马上就会开始
            app.manage(login::Login::default());
            login::prepare(app.handle());
            core.set_login_store(std::sync::Arc::new(login::DesktopStore(app.handle().clone())));
            if !login::separate_store() {
                tracing::info!("macOS 14 以下没有单独的网页数据存储：登录窗口用默认存储（和 Captae 界面共用）");
            }
            // 本地控制接口：CLI、MCP、浏览器插件通过它找到这个窗口
            let h = app.handle().clone();
            let on_focus: captae_core::api::OnFocus = std::sync::Arc::new(move |t: captae_core::api::FocusTarget| {
                let h2 = h.clone();
                let _ = h.run_on_main_thread(move || {
                    show_main(&h2);
                    if let Some(href) = t.href() {
                        go_to(&h2, &href);
                    }
                });
            });
            tauri::async_runtime::block_on(captae_core::api::serve(core.clone(), on_focus))?;
            // 让手机连接这台电脑（局域网，和上面的本机控制接口分开）：上次开着就接着开（remote_cmds.rs）
            remote_cmds::init(app.handle(), &core);
            if let Some(p) = app_path() {
                let _ = std::fs::write(paths.data.join("app_path"), p.display().to_string());
            }
            // 浏览器插件的宿主：没注册或指向别的 captae 就重新注册。
            // 用 CAPTAE_HOME 跑的测试版不注册（不然会把浏览器里的插件指到测试版上），要测插件时设 CAPTAE_REGISTER_HOST=1
            if std::env::var_os("CAPTAE_HOME").is_none() || std::env::var_os("CAPTAE_REGISTER_HOST").is_some() {
                ext_cmds::register_native_host(&paths.data);
            }
            // 剪贴板提示：复制了视频链接、1 分钟内把 Captae 拿到前面才问（clip_ask.rs）
            clip_ask::start(app.handle().clone(), &paths.data);
            // 预热外部工具：macOS 第一次运行一个陌生程序要做安全检查（yt-dlp 要 10 秒左右），
            // 启动时在后台先各跑一次，用户第一次下载就不用等
            {
                let c = core.clone();
                tauri::async_runtime::spawn(async move {
                    let t = std::time::Instant::now();
                    let _ = c.toolbox().status().await;
                    tracing::info!("外部工具预热用了 {:.1}s", t.elapsed().as_secs_f64());
                });
            }
            // core 事件 → 页面
            let mut rx = core.subscribe();
            let h = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(ev) => {
                            // 新下载好的文件、换了的保存位置：先放行，页面收到事件去读的时候就能显示
                            asset_scope::on_event(&h, &ev);
                            let _ = h.emit("captae://event", &ev);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(_) => break,
                    }
                }
            });
            app.manage(App { core });
            // AppImage：启动时就把插件文件夹拷到数据目录（换了新版的 AppImage 时更新它），插件从这里重新加载自己
            #[cfg(target_os = "linux")]
            let _ = extension_dir(app.handle());
            // Captae 自己的更新：读记录、算装不装得了，打开 1 分钟后查一次，之后每 24 小时一次（app_update.rs）
            app_update::init(app.handle(), &paths.data);
            // 被 kill（SIGTERM）、终端关掉（SIGHUP）、终端里按 Ctrl-C（SIGINT）时也和 ⌘Q 一样收拾再退出
            quit_on_signals(app.handle().clone());
            // --page library：页面还没加载，直接把地址换成 index.html#/library（单页应用按 # 选页面）
            if let (Some(p), Some(w)) = (start_page.as_deref(), app.get_webview_window("main")) {
                if matches!(p, "downloads" | "library" | "agent" | "extension") {
                    if let Ok(mut u) = w.url() {
                        u.set_path("/index.html");
                        u.set_fragment(Some(&format!("/{p}")));
                        let _ = w.navigate(u);
                    }
                }
            }
            // 开发调试：登录窗口的测试钩子（<数据目录>/login-test.json，见 login.rs，正式版不包含）
            #[cfg(debug_assertions)]
            login::debug_hooks(app.handle().clone(), paths.data.clone());
            // 开发调试：台前调度开着时，没抢到前台的测试版主窗口会被收进侧边条（报出来的大小是缩略图那么小）、WebKit 也不画。
            // 实测时设 CAPTAE_TEST_ONTOP 让主窗口浮在最上面、照常画（和登录窗口一样；正式版不包含）
            #[cfg(all(debug_assertions, target_os = "macos"))]
            if std::env::var_os("CAPTAE_TEST_ONTOP").is_some() {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.set_always_on_top(true);
                    let _ = w.with_webview(login::keep_painting_pv);
                }
            }
            // 开发调试：把 JS 写进 <数据目录>/eval.js，主窗口会执行它（用于自动化走查界面，正式版不包含）
            #[cfg(debug_assertions)]
            {
                let h = app.handle().clone();
                let f = paths.data.join("eval.js");
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        if let Ok(js) = std::fs::read_to_string(&f) {
                            let _ = std::fs::remove_file(&f);
                            if let Some(w) = h.get_webview_window("main") {
                                let _ = w.eval(&js);
                            }
                        }
                    }
                });
            }
            // 默认 1440×900 比小屏幕（13 寸笔记本）的可用区域大时，按屏幕缩小并居中，
            // 否则系统会把窗口压小而网页层不跟着变，内容整体下移、底部被截掉。
            if let Some(w) = app.get_webview_window("main") {
                fit_to_screen(&w);
            }
            if background {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.hide();
                }
            }
            // Windows / Linux 的主窗口一开始就是无边框的（tauri.windows.conf.json、tauri.linux.conf.json），窗口按钮由界面画（shell/WindowControls.tsx）；
            // 不在这里运行时再去边框，免得先画出系统标题栏再消失、闪一下。
            // 这两个配置里主窗口一开始是藏着的：按屏幕缩好大小以后再显示，带 --background 启动（插件宿主拉起来的）就一直不显示，不会先闪一下再藏起来
            #[cfg(not(target_os = "macos"))]
            if !background {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.show();
                }
            }
            Ok(())
        })
        .on_window_event(|w, ev| {
            // 剪贴板提示要知道 Captae 在不在前台、主窗口什么时候被拿到前面（登录窗口里复制的也算在 Captae 里复制的）。
            // Windows 上焦点在 WebView2 的子窗口里，这个事件不可靠，由 clip_ask 自己看前台窗口
            #[cfg(not(windows))]
            if let tauri::WindowEvent::Focused(f) = ev {
                clip_ask::focus_changed(w.app_handle(), w.label(), *f);
            }
            if w.label() == login::WINDOW {
                login::on_window_event(w, ev);
                return;
            }
            // 只有主窗口关了才算退出：删掉 control.json（命令行、MCP、插件靠它找桌面端），登录窗口也一起关
            if let (tauri::WindowEvent::Destroyed, "main") = (ev, w.label()) {
                if let Some(a) = w.app_handle().try_state::<App>() {
                    captae_core::api::clear(&a.core);
                }
                login::close_all(w.app_handle());
                remote_cmds::shutdown(w.app_handle());
                // 退出时还在下载、转换：把 yt-dlp、ffmpeg、aria2c 连同它们的子进程结束，不留在后台继续跑
                captae_core::tools::kill_all();
            }
        })
        .invoke_handler(invoke_handler())
        .build(tauri::generate_context!())
        .expect("Captae 启动失败")
        .run(|app, ev| {
            // ⌘Q（菜单里的「退出」）走的是 NSApp terminate：主窗口来不及收到 Destroyed，进程就结束了。
            // 这里同样删掉 control.json，把还在跑的 yt-dlp、ffmpeg、aria2c、Python 连同子进程结束。
            // 收到 SIGTERM、SIGHUP、SIGINT（Windows 是控制台的 Ctrl-C、Ctrl-Break、关掉控制台）时也走这里（quit_on_signals 调 AppHandle::exit）。
            // Windows 关机、注销：系统发 WM_ENDSESSION，tao 收到后结束事件循环，Tauri 发 RunEvent::Exit，也走这里，
            // 在这个回调返回之前做完（返回以后系统才结束进程），见 docs/DESIGN.md 第 11 节「退出」
            // 录直播的原始数据是边写边能读的格式，留在原处，下次打开时整理好（queue.rs 的 recover_recording）
            if let tauri::RunEvent::Exit = ev {
                cleanup_on_exit(app);
                // 有下好的新版：正常退出时装上，下次打开就是新版（收到结束信号、关机时不装，见 app_update::on_exit）
                app_update::on_exit(app, QUITTING.load(std::sync::atomic::Ordering::SeqCst));
            }
        });
}

/// 退出时要做的收拾：删 control.json（命令行、MCP、插件靠它找桌面端）、关掉给手机的服务、结束还在跑的 yt-dlp、ffmpeg、aria2c 连同子进程。
/// Windows 上装更新时插件会直接结束进程，运行安装程序之前也调它（app_update.rs 的 on_before_exit）
fn cleanup_on_exit(app: &tauri::AppHandle) {
    if let Some(a) = app.try_state::<App>() {
        captae_core::api::clear(&a.core);
    }
    remote_cmds::shutdown(app);
    captae_core::tools::kill_all();
}

/// 已经开始退出了（收到过一个结束信号）
static QUITTING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// 收到 SIGTERM（`kill`、系统关机时结束进程）、SIGHUP（启动它的终端关掉了）、SIGINT（终端里按 Ctrl-C）：
/// 走 `AppHandle::exit(0)`，触发 `RunEvent::Exit`，和 ⌘Q 一样删掉 control.json、结束还在跑的 yt-dlp、ffmpeg、aria2c 连同子进程。
/// 不接住的话进程直接没了，退出流程一步都不走，它们留成孤儿接着下载、接着录。录直播的原始数据留在原处，下次打开时整理好（recover_recording）。
/// 兜底：正常退出 5 秒还没走完（界面卡住了），或者又收到一个信号，就自己收拾完直接退出。Linux 上也是这一份（注销、关机时 systemd 先发 SIGTERM）
#[cfg(unix)]
fn quit_on_signals(app: tauri::AppHandle) {
    use tokio::signal::unix::{signal, SignalKind};
    for (kind, name) in [(SignalKind::terminate(), "SIGTERM"), (SignalKind::hangup(), "SIGHUP"), (SignalKind::interrupt(), "SIGINT")] {
        let app = app.clone();
        // signal() 要挂到 tokio 运行时上，得在运行时里调：setup 跑在主线程上，不在运行时里（直接调会 panic）
        tauri::async_runtime::spawn(async move {
            let mut s = match signal(kind) {
                Ok(s) => s,
                Err(err) => {
                    tracing::warn!("接不住 {name}：{err}");
                    return;
                }
            };
            while s.recv().await.is_some() {
                quit_on(&app, name);
            }
        });
    }
}

/// Windows：正式版是窗口程序、没有控制台，关机和注销走 WM_ENDSESSION（见 main 里 `RunEvent::Exit` 的注释），这里不用管。
/// 调试版、或者从命令行带控制台跑的时候，接住控制台的 Ctrl-C、Ctrl-Break、关掉控制台窗口，和 unix 收到信号一样收拾完再退出
#[cfg(windows)]
fn quit_on_signals(app: tauri::AppHandle) {
    use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close};
    macro_rules! on {
        ($make:expr, $name:literal) => {{
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let mut s = match $make {
                    Ok(s) => s,
                    Err(err) => {
                        tracing::warn!("接不住 {}：{err}", $name);
                        return;
                    }
                };
                while s.recv().await.is_some() {
                    quit_on(&app, $name);
                }
            });
        }};
    }
    on!(ctrl_c(), "Ctrl-C");
    on!(ctrl_break(), "Ctrl-Break");
    on!(ctrl_close(), "关掉控制台");
}

#[cfg(not(any(unix, windows)))]
fn quit_on_signals(_app: tauri::AppHandle) {}

/// 收到一个结束信号：第一次走正常的退出流程，5 秒没走完就直接收拾完退出；又来一个就不等了
#[cfg(any(unix, windows))]
fn quit_on(app: &tauri::AppHandle, name: &str) {
    use std::sync::atomic::Ordering;
    if QUITTING.swap(true, Ordering::SeqCst) {
        tracing::info!("又收到 {name}：不等了，直接收拾完退出");
        force_quit(app);
    }
    tracing::info!("收到 {name}：和 ⌘Q 一样收拾完再退出");
    app.exit(0);
    let a2 = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(5));
        tracing::warn!("正常退出 5 秒还没走完：直接收拾完退出");
        force_quit(&a2);
    });
}

/// 不走 Tauri 的退出流程，自己做 `RunEvent::Exit` 里那两件事再退出
#[cfg(any(unix, windows))]
fn force_quit(app: &tauri::AppHandle) -> ! {
    if let Some(a) = app.try_state::<App>() {
        captae_core::api::clear(&a.core);
    }
    captae_core::tools::kill_all();
    std::process::exit(0);
}

/// 页面能调的命令。登录窗口里的 webview 先过一层白名单（login::allowed）：网站那块什么都不能调，
/// 工具条只能调它自己那几个。（Tauri 对远程来源本来就要过权限检查，这是第二道。）
fn invoke_handler() -> impl Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static {
    // 让编译器知道 generate_handler! 生成的闭包收的是什么
    fn typed<F: Fn(tauri::ipc::Invoke) -> bool + Send + Sync + 'static>(f: F) -> F {
        f
    }
    let handler = typed(tauri::generate_handler![
            boot, set_settings, set_presets, preset_check, take_sniffed, library_count, add_job, job_action, set_variant, pause_all, resume_all,
            reveal, open_path, copy_text, read_clipboard, clip_ask::clip_check, media_server::media_src, choose_save_dir, save_dir_label, window_ctl, library_query, library_tags,
            task_convert, task_subtitle, task_trim, js_log, deps_status, open_url, install_whisper, extension_path, extension_install_help,
            library_cmds::library_get, library_cmds::library_remove, library_cmds::library_trash, library_cmds::library_share,
            library_cmds::library_frames, library_cmds::library_waveform, library_cmds::subtitle_tracks, library_cmds::pick_audio_file,
            library_cmds::task_concat,
            agent_cmds::mcp_status, agent_cmds::agent_info, agent_cmds::mcp_config_plan, agent_cmds::mcp_config_write,
            agent_cmds::pick_folder, agent_cmds::export_skill, agent_cmds::preset_trial,
            ext_cmds::extension_status, ext_cmds::extension_register, ext_cmds::extension_test,
            engine_status, engine_update, proxy_status, choose_entries,
            login::login_open, login::login_close, login::login_sites, login::login_logout,
            login::login_bar_state, login::login_done, login::login_cancel,
            task_cmds::task_cancel,
            remote_cmds::remote_status, remote_cmds::remote_enable, remote_cmds::remote_pairing, remote_cmds::remote_revoke,
            app_update::app_update_status, app_update::app_update_check, app_update::app_update_download, app_update::app_update_restart
        ]);
    move |invoke| {
        let (label, cmd) = (invoke.message.webview_ref().label().to_string(), invoke.message.command().to_string());
        if !login::allowed(&label, &cmd) {
            tracing::warn!("拒绝了 {label} 调用 {cmd}");
            invoke.resolver.reject(tr!("{cmd} 不允许在这里调用", cmd = cmd));
            return true;
        }
        handler(invoke)
    }
}

#[cfg(test)]
mod tests {
    use super::route_of;

    #[test]
    fn routes() {
        // core 现在给的路由
        assert_eq!(route_of("/library?asset=5"), "/library?asset=5");
        assert_eq!(route_of("/downloads?paste"), "/downloads?paste");
        assert_eq!(route_of("/extension"), "/extension");
        // 插件「在 Captae 里查看」某个任务（core FocusTarget::job）：下载页认 ?t=
        let job = captae_core::api::FocusTarget { page: Some("downloads".into()), job: Some(12), ..Default::default() };
        assert_eq!(route_of(&job.href().unwrap()), "/downloads?t=12");
        assert_eq!(route_of("/downloads?t=12"), "/downloads?t=12");
        // 旧界面的写法也认
        assert_eq!(route_of("app-library.html#asset=5"), "/library?asset=5");
        assert_eq!(route_of("app-downloads.html#paste"), "/downloads?paste");
        assert_eq!(route_of("app-downloads.html#t=12"), "/downloads?t=12");
        assert_eq!(route_of("app-agent.html"), "/agent");
        // 认不出的去下载页
        assert_eq!(route_of("nowhere.html"), "/downloads");
    }
}
