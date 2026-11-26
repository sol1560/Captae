//! 「设置 › 手机」：让手机在同一个 Wi-Fi 下遥控这台电脑（crates/captae-remote）。
//! 服务随桌面端启动：上次开着就接着开（开关存在 `<数据目录>/remote/state.json`，不在 settings.json 里）。
//! 有变化（配对了、撤销了、手机连上 / 断开、开关变了）时给页面发 `captae://event` `{type: "remote"}`，页面再来取状态。

use captae_core::Core;
use captae_remote::server::{computer_name, Opts, PairingView, Remote, Status};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};

pub struct RemoteState(pub Remote);

/// Info.plist 的 `NSLocalNetworkUsageDescription`（万一 macOS 问「本地网络」权限时系统显示的说明）的原文。
/// 这里只为把它标进词表（tr_key!）；build.rs 按这句原文查各语言的译文，生成 `<语言>.lproj/InfoPlist.strings`
// 程序里不用它（系统从 InfoPlist.strings 读），只给词表和测试用
#[allow(dead_code)]
pub const LOCAL_NETWORK_USAGE: &str = captae_core::tr_key!("让同一个 Wi-Fi 下配对过的手机找到这台电脑");

type R<T> = Result<T, String>;

/// 建好服务（还不监听），交给 Tauri 管；上次开着的在后台打开
pub fn init(app: &AppHandle, core: &Core) {
    let h = app.clone();
    let on_change = std::sync::Arc::new(move || {
        let _ = h.emit("captae://event", json!({"type": "remote"}));
    });
    let mut opts = Opts { name: computer_name(), on_change: Some(on_change), bind: None, bonjour: true };
    // 开发调试：只听 127.0.0.1、不广播 Bonjour（实测时不弹防火墙和「本地网络」的询问，局域网上也看不到测试版；正式版不包含）
    #[cfg(debug_assertions)]
    if let Some(ip) = std::env::var("CAPTAE_TEST_REMOTE_BIND").ok().and_then(|s| s.parse().ok()) {
        tracing::info!("测试开关 CAPTAE_TEST_REMOTE_BIND 开着：远程控制只听 {ip}，不广播 Bonjour");
        opts.bind = Some(ip);
        opts.bonjour = false;
    }
    let remote = Remote::new(core.clone(), opts);
    app.manage(RemoteState(remote.clone()));
    tauri::async_runtime::spawn(async move { remote.start_if_enabled().await });
}

/// 退出时：停止监听，Bonjour 发告别（手机那边马上知道电脑走了）
pub fn shutdown(app: &AppHandle) {
    if let Some(r) = app.try_state::<RemoteState>() {
        r.0.stop();
    }
}

/// 开关、端口、地址、已配对的手机
#[tauri::command]
pub fn remote_status(st: State<RemoteState>) -> Status {
    st.0.status()
}

/// 打开 / 关掉「让手机连接这台电脑」
#[tauri::command]
pub async fn remote_enable(st: State<'_, RemoteState>, on: bool) -> R<Status> {
    st.0.set_enabled(on).await.map_err(|e| e.to_string())
}

/// 现在的二维码内容和 6 位数（过期了、`refresh` 时换新的）
#[tauri::command]
pub fn remote_pairing(st: State<RemoteState>, refresh: Option<bool>) -> R<PairingView> {
    st.0.pairing(refresh.unwrap_or(false)).map_err(|e| e.to_string())
}

/// 撤销一台手机
#[tauri::command]
pub fn remote_revoke(st: State<RemoteState>, id: String) -> R<Status> {
    st.0.revoke(&id).map_err(|e| e.to_string())?;
    Ok(st.0.status())
}

#[cfg(test)]
mod tests {
    use super::LOCAL_NETWORK_USAGE;

    /// Info.plist 里写的英文和词表一致，build.rs 生成的各语言说明也查得到这句原文（改说法时三处一起改）
    #[test]
    fn local_network_usage() {
        let plist = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/Info.plist")).unwrap();
        let en: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../crates/captae-core/locales/en/app.json")).unwrap()).unwrap();
        let text = en[LOCAL_NETWORK_USAGE].as_str().expect("英文词表里有这句");
        assert!(plist.contains(&format!("<string>{text}</string>")), "Info.plist 的 NSLocalNetworkUsageDescription 和英文词表不一致");
        assert!(plist.contains("<key>NSBonjourServices</key>") && plist.contains("<string>_captae._tcp</string>"));
        assert!(include_str!("../build.rs").contains(LOCAL_NETWORK_USAGE), "build.rs 查的原文和这里不一致");
    }
}
