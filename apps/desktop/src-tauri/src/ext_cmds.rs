//! 浏览器插件相关：启动时注册 Native Messaging 宿主，「浏览器插件」页的连接状态和测试。

use crate::App;
use captae_core::native_host::{self, Install};
use captae_core::tr;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tauri::State;

/// 宿主程序：桌面端旁边的 `captae`（发布版是 sidecar，开发时是 target/<profile>/captae）。
/// Linux 的 AppImage：AppImage 运行时挂在 /tmp/.mount_xxx/ 下，Captae 一退出就没了，浏览器清单不能指过去；
/// 这时把 captae 拷一份到数据目录的 `host/` 再登记。宿主平时只转发给桌面端、桌面端没开时按 app_path 拉起 AppImage；
/// 桌面端没开时宿主自己读视频信息用的是自动更新下载的 yt-dlp（数据目录里的），附带的那份它够不着
fn cli(data: &Path) -> Option<PathBuf> {
    let exe = native_host::cli_next_to(&std::env::current_exe().ok()?)?;
    #[cfg(target_os = "linux")]
    if let Some(mount) = std::env::var_os("APPDIR").map(PathBuf::from).filter(|_| std::env::var_os("APPIMAGE").is_some()) {
        if exe.starts_with(&mount) {
            return copy_host(&exe, &data.join("host")).map_err(|e| tracing::warn!("没能把插件宿主拷到数据目录：{e}")).ok();
        }
    }
    let _ = data;
    Some(exe)
}

/// 内容不一样（AppImage 换了新版）才重拷：先写临时文件再改名，宿主正在跑时也不会读到半个文件
#[cfg(target_os = "linux")]
fn copy_host(exe: &Path, dir: &Path) -> std::io::Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let dst = dir.join("captae");
    let new = std::fs::read(exe)?;
    if std::fs::read(&dst).ok().as_deref() != Some(new.as_slice()) {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(".captae.tmp");
        std::fs::write(&tmp, &new)?;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
        std::fs::rename(&tmp, &dst)?;
    }
    Ok(dst)
}

fn install_opts(data: &Path) -> Option<Install> {
    let mut o = Install::new(cli(data)?, data.to_path_buf());
    // 便携模式：宿主也要用同一个数据目录，才找得到这个桌面端
    o.env_home = std::env::var_os("CAPTAE_HOME").map(PathBuf::from);
    Some(o)
}

/// 启动时调用：清单缺了或指向别的程序就重写。找不到 captae 时什么也不做（只编了桌面端）。
pub fn register_native_host(data: &Path) {
    let Some(o) = install_opts(data) else {
        tracing::warn!("没有找到 captae 命令行程序，浏览器插件暂时连不上桌面端");
        return;
    };
    match native_host::ensure(&o) {
        Ok(true) => tracing::info!("已注册浏览器插件宿主：{}", o.exe.display()),
        Ok(false) => {}
        Err(e) => tracing::warn!("注册浏览器插件宿主失败：{e}"),
    }
}

fn host_status(data: &Path) -> Value {
    match install_opts(data) {
        Some(o) => json!(native_host::status(&o)),
        None => json!({"ok": false, "missing": true}),
    }
}

/// 插件连接状态：两分钟内打过招呼就算连着。`host` 是宿主注册情况。
#[tauri::command]
pub fn extension_status(app: State<App>) -> Value {
    let mut v = json!(app.core.extension_status());
    v["host"] = host_status(&app.core.paths.data);
    v
}

/// 重新注册宿主（「浏览器插件」页的「修复」）。
#[tauri::command]
pub fn extension_register(app: State<App>) -> Result<Value, String> {
    let o = install_opts(&app.core.paths.data).ok_or_else(|| tr!("没有找到 captae 命令行程序"))?;
    native_host::install(&o).map_err(|e| e.to_string())?;
    Ok(host_status(&app.core.paths.data))
}

/// 测试连接：像浏览器一样启动宿主，发一条 hello，看它能不能找到这个桌面端。
/// 插件本身有没有连上看 `extension_status`（桌面端没法主动联系插件）。
/// 只要宿主程序在（至少给一家浏览器登记过）就真的叫它一次；某家装了但没登记，不影响别的浏览器能不能用，
/// 登记情况照样在 `host` 里，页面会单独说是哪家没登记。
#[tauri::command]
pub async fn extension_test(app: State<'_, App>) -> Result<Value, String> {
    let data = app.core.paths.data.clone();
    let mut v = json!({"host": host_status(&data)});
    let wrapper = match install_opts(&data).map(|o| o.wrapper()) {
        Some(w) if w.is_file() => w,
        _ => {
            v["reply"] = Value::Null;
            v["error"] = json!(tr!("宿主还没有注册"));
            v["extension"] = json!(app.core.extension_status());
            return Ok(v);
        }
    };
    match talk(&wrapper).await {
        Ok(r) => {
            v["reply"] = r;
            v["error"] = Value::Null;
        }
        Err(e) => {
            v["reply"] = Value::Null;
            v["error"] = json!(e);
        }
    }
    v["extension"] = json!(app.core.extension_status());
    Ok(v)
}

async fn talk(wrapper: &Path) -> Result<Value, String> {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let mut c = tokio::process::Command::new(wrapper);
    c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true);
    let mut child = c.spawn().map_err(|e| tr!("宿主启动不了：{err}", err = e))?;
    let mut buf = vec![];
    native_host::write_msg(&mut buf, &json!({"id": 1, "type": "hello", "test": true})).map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or_else(|| tr!("宿主没有输入"))?;
    stdin.write_all(&buf).await.map_err(|e| e.to_string())?;
    let mut out = child.stdout.take().ok_or_else(|| tr!("宿主没有输出"))?;
    let read = async {
        let mut len = [0u8; 4];
        out.read_exact(&mut len).await?;
        let mut b = vec![0u8; u32::from_ne_bytes(len) as usize];
        out.read_exact(&mut b).await?;
        Ok::<_, std::io::Error>(b)
    };
    let b = tokio::time::timeout(std::time::Duration::from_secs(8), read).await.map_err(|_| tr!("宿主没有回应"))?.map_err(|e| tr!("宿主提前退出了：{err}", err = e))?;
    drop(stdin);
    let _ = child.kill().await;
    let v: Value = serde_json::from_slice(&b).map_err(|e| e.to_string())?;
    if v["ok"] != true {
        // 宿主回的报错是它按同一份设置的语言写的，原样给界面；没写原因时用这一句
        return Err(v["error"].as_str().map(str::to_string).unwrap_or_else(|| tr!("宿主回了错误")));
    }
    Ok(v["data"].clone())
}
