//! 素材库页面用的命令：单条素材、移除 / 删到废纸篓、发给 AI、胶片条、波形、
//! 网站字幕列表、选声音文件、拼接。

use super::{asset_path, e, App, R};
use captae_core::library::Asset;
use captae_core::tr;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::State;

fn get(app: &App, id: i64) -> R<Asset> {
    app.core.lib.get(id).map_err(e)?.ok_or_else(|| tr!("素材不存在"))
}

/// 文件修改时间（秒），用来让缓存跟着文件变
fn stamp(p: &Path) -> u64 {
    std::fs::metadata(p).and_then(|m| m.modified()).ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_secs()).unwrap_or(0)
}

/// 缩略图在我们自己的缓存目录里才删，用户自己的文件不动
fn drop_thumb(app: &App, a: &Asset) {
    if let Some(t) = &a.thumb {
        if t.starts_with(&app.core.paths.thumbs) {
            let _ = std::fs::remove_file(t);
        }
    }
}

/// 打开 `#asset=<id>` 时，这条素材可能不在已经加载的那一页里
#[tauri::command]
pub fn library_get(handle: tauri::AppHandle, app: State<App>, id: i64) -> R<Option<Asset>> {
    let a = app.core.lib.get(id).map_err(e)?;
    // 打开某一条素材：它所在的文件夹放行（预览播放）
    crate::asset_scope::allow_assets(&handle, a.iter());
    Ok(a)
}

/// 只从素材库里去掉记录，不碰文件（文件已经不在了时用）
#[tauri::command]
pub fn library_remove(app: State<App>, id: i64) -> R<()> {
    let a = get(&app, id)?;
    app.core.lib.remove(id).map_err(e)?;
    drop_thumb(&app, &a);
    app.core.emit_library();
    Ok(())
}

/// 把文件移到废纸篓，再从素材库里去掉
#[tauri::command]
pub async fn library_trash(app: State<'_, App>, id: i64) -> R<()> {
    let a = get(&app, id)?;
    if a.path.exists() {
        let p = a.path.clone();
        tokio::task::spawn_blocking(move || move_to_trash(&p)).await.map_err(e)?.map_err(trash_error)?;
    }
    app.core.lib.remove(id).map_err(e)?;
    drop_thumb(&app, &a);
    app.core.emit_library();
    Ok(())
}

/// 移不进去时给人看的话：系统里那个地方的叫法按系统换（macOS 废纸篓，Windows、Linux 回收站——GNOME、KDE 的中文界面都叫回收站）。
/// Linux 上外置盘、网络盘、只读的位置常常没有回收站（规范要求在那个盘的根目录建 .Trash-<uid>，建不了就移不进去），多说一句怎么办
fn trash_error(x: trash::Error) -> String {
    if cfg!(target_os = "macos") {
        tr!("没能移到废纸篓：{err}", err = x)
    } else if cfg!(windows) {
        tr!("没能移到回收站：{err}", err = x)
    } else {
        tr!("没能移到回收站：{err}。外置盘、网络盘上可能没有回收站，可以在文件管理器里直接删除", err = x)
    }
}

fn move_to_trash(p: &Path) -> Result<(), trash::Error> {
    #[cfg(target_os = "macos")]
    {
        // 默认的 Finder 方式会弹「要控制访达」的权限请求，这里直接用 NSFileManager
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut ctx = trash::TrashContext::default();
        ctx.set_delete_method(DeleteMethod::NsFileManager);
        ctx.delete(p)
    }
    #[cfg(not(target_os = "macos"))]
    {
        trash::delete(p)
    }
}

/// 发给 AI：整理文字稿和关键帧，放进剪贴板。作为后台任务跑，界面按 assetId 收进度。
#[tauri::command]
pub async fn library_share(app: State<'_, App>, id: i64) -> R<Value> {
    let core = app.core.clone();
    let (a, _) = asset_path(&core, id)?;
    let c2 = core.clone();
    // 任务标题先单独算好：tr! 的参数是借用（&dyn Display，不能跨线程），不能一直留到下面的 .await
    let title = tr!("把「{title}」发给 AI", title = a.title);
    let r = core
        .run_task("share", title, Some(id), |rep| async move {
            let o = captae_core::share::ShareOpts { asset_id: Some(id), copy: true, ..Default::default() };
            let b = captae_core::share::share(&c2, o, |p, _| rep.pct(p)).await?;
            Ok(json!({"copied": b.copied, "dir": b.dir, "frames": b.frames.len(), "transcript": b.transcript_source}))
        })
        .await
        .map_err(e)?;
    if r["copied"] != json!(true) {
        return Err(tr!("整理好了，但没能放进剪贴板"));
    }
    Ok(r)
}

/// 剪辑面板的胶片条：均匀取几帧小图，缓存在缩略图目录里。
#[tauri::command]
pub async fn library_frames(app: State<'_, App>, id: i64, n: Option<usize>) -> R<Vec<PathBuf>> {
    let core = app.core.clone();
    let (a, p) = asset_path(&core, id)?;
    let d = a.duration.filter(|d| *d > 0.0).ok_or_else(|| tr!("不知道这个视频有多长"))?;
    let n = n.unwrap_or(10).clamp(1, 30);
    let dir = core.paths.thumbs.join("strip").join(format!("{id}-{}-{n}", stamp(&p)));
    captae_core::ffmpeg::strip_frames(&core.toolbox(), &p, d, &dir, n).await.map_err(e)
}

/// 声音波形：`n` 个 0..1 的峰值，缓存成 JSON。
#[tauri::command]
pub async fn library_waveform(app: State<'_, App>, id: i64, n: Option<usize>) -> R<Vec<f32>> {
    let core = app.core.clone();
    let (_, p) = asset_path(&core, id)?;
    let n = n.unwrap_or(48).clamp(8, 400);
    let cache = core.paths.thumbs.join("wave").join(format!("{id}-{}-{n}.json", stamp(&p)));
    if let Ok(v) = std::fs::read(&cache).map_err(e).and_then(|b| serde_json::from_slice::<Vec<f32>>(&b).map_err(e)) {
        return Ok(v);
    }
    let v = captae_core::ffmpeg::peaks(&core.toolbox(), &p, n).await.map_err(e)?;
    if let Some(d) = cache.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let _ = std::fs::write(&cache, serde_json::to_vec(&v).unwrap_or_default());
    Ok(v)
}

/// 检查结果缓存一分钟：用户装好组件后不用重开 Captae
static CAPS: Mutex<Option<(Instant, bool, bool)>> = Mutex::new(None);

async fn capabilities(app: &App) -> (bool, bool) {
    if let Some((t, w, b)) = *CAPS.lock().unwrap() {
        if t.elapsed() < Duration::from_secs(60) {
            return (w, b);
        }
    }
    let tools = app.core.toolbox();
    let (w, b) = tokio::join!(captae_core::subtitle::whisper_available(&tools), captae_core::ffmpeg::has_filter(&tools, "subtitles"));
    *CAPS.lock().unwrap() = Some((Instant::now(), w, b));
    (w, b)
}

/// 字幕面板要知道的：网站上有哪些字幕（按素材缓存）、文件有没有声音、
/// 本地识别和烧录能不能用。
#[tauri::command]
pub async fn subtitle_tracks(app: State<'_, App>, id: i64, refresh: Option<bool>) -> R<Value> {
    let core = app.core.clone();
    let a = get(&app, id)?;
    let url = if a.page.is_empty() { a.url.clone() } else { a.page.clone() };
    const KEY: &str = "subtitle_tracks";
    let cached = if refresh.unwrap_or(false) {
        None
    } else {
        core.lib.cache_get(id, KEY).map_err(e)?.and_then(|(v, at)| {
            let v: Value = serde_json::from_str(&v).ok()?;
            // 读成功的留一周；读失败的（网络问题等）一小时后再试
            let ttl: i64 = if v["err"].is_null() { 7 * 86_400_000 } else { 3_600_000 };
            (captae_core::util::now_ms() - at < ttl).then_some(v)
        })
    };
    let site = match cached {
        Some(v) => v,
        None => {
            let v = match captae_core::subtitle::tracks(&core, &url).await {
                Ok(t) => json!({"tracks": t, "err": null}),
                Err(x) => json!({"tracks": [], "err": x.to_string()}),
            };
            core.lib.cache_set(id, KEY, &v.to_string()).map_err(e)?;
            v
        }
    };
    let has_audio = if a.exists { captae_core::ffmpeg::probe(&core.toolbox(), &a.path).await.map(|p| p.has_audio).unwrap_or(true) } else { false };
    let (whisper, burn) = capabilities(&app).await;
    Ok(json!({"tracks": site["tracks"], "err": site["err"], "hasAudio": has_audio, "whisper": whisper, "canBurn": burn}))
}

/// 「换一段声音」：从电脑里选一个声音文件（视频文件也行，用它的声音）
#[tauri::command]
pub async fn pick_audio_file(app: tauri::AppHandle) -> R<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title(tr!("选择一段声音"))
        .add_filter(tr!("声音§选声音文件的窗口里的文件类型"), &["mp3", "m4a", "aac", "wav", "flac", "ogg", "opus", "aiff", "aif", "caf", "mp4", "mov", "m4v", "webm", "mkv"])
        .pick_file(move |f| {
            let _ = tx.send(f);
        });
    let Some(f) = rx.await.map_err(e)? else { return Ok(None) };
    Ok(Some(f.into_path().map_err(e)?.display().to_string()))
}

/// 拼接：按 ids 的顺序，结果放在第一段旁边并加入素材库。
#[tauri::command]
pub async fn task_concat(app: State<'_, App>, ids: Vec<i64>) -> R<Value> {
    if ids.len() < 2 {
        return Err(tr!("至少选两段视频"));
    }
    let core = app.core.clone();
    let mut paths = vec![];
    for id in &ids {
        let (a, p) = asset_path(&core, *id)?;
        if a.kind != "video" {
            return Err(tr!("「{title}」不是视频，只能拼接视频", title = a.title));
        }
        paths.push(p);
    }
    let first = ids[0];
    let tools = core.toolbox();
    let n = paths.len();
    // 任务标题先单独算好（同 library_share）
    let title = tr!("拼接 {n} 段视频", n = n);
    let out = core
        .run_task("concat", title, Some(first), |rep| async move {
            let o = captae_core::ffmpeg::concat(&tools, &paths, None, |x| rep.pct(x)).await?;
            Ok(json!(o))
        })
        .await
        .map_err(e)?;
    let out: PathBuf = serde_json::from_value(out).map_err(e)?;
    let nid = core.import_file(&out, Some(first), None).await.map_err(e)?;
    Ok(json!({"path": out, "assetId": nid}))
}
