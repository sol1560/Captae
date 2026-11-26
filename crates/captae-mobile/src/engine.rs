//! 下载引擎（yt-dlp）的版本和应用内更新：Android 完整版用（F-Droid、GitHub Releases 允许在 App 里更新）。
//!
//! 流程全在 captae-core 的 `updater.rs`：官方公钥验 `SHA2-256SUMS.sig` → 核对 SHA256 → 自检（`--version` 对得上，再读一个公开视频；
//! 读不到时做离线自检）→ 通过了才切换，保留上一个版本；新版用起来连续出错时体检、退回。Android 下的是官方 zipapp，
//! 放在 `<数据目录>/engines/yt-dlp/<版本>/yt-dlp`，由原生库目录里的启动器（libytdlp.so）跑。
//! iOS 不许在 App 里更新代码：`check_engine_update` 返回 `skipped`。
//! 每天自动查一次由核心库自己做（设置里 `auto_update` 打开时，Android 才有），这里是「检查更新」按钮和「设置 › 关于」用的。

use crate::{MobileCore, MobileError};
use captae_core::updater::{self, How};
use serde_json::Value;

/// 检查更新的结果
#[derive(Debug, Clone, uniffi::Record)]
pub struct EngineUpdate {
    /// updated 已更新 / latest 已经是最新 / skipped 没检查（平台不支持、设置里指定了路径、正在检查）/
    /// rejected 新版自检没通过（继续用原来的）/ failed 检查或下载失败（继续用原来的）
    pub result: String,
    /// 给人看的一句话（当前语言）
    pub message: String,
    /// 检查前在用的版本
    pub current: Option<String>,
    /// GitHub 上的最新版
    pub latest: Option<String>,
    /// 检查后在用的自动更新版本（None = 随 App 带的那份）
    pub active: Option<String>,
    /// 每一步做了什么（验签、核对、自检……）
    pub steps: Vec<String>,
}

/// 更新记录的一行
#[derive(Debug, Clone, uniffi::Record)]
pub struct EngineLogLine {
    /// 毫秒时间戳
    pub at: i64,
    pub text: String,
}

/// 下载引擎现在的状态（「设置 › 关于」）
#[derive(Debug, Clone, uniffi::Record)]
pub struct EngineStatus {
    /// 现在实际用的 yt-dlp 版本（跑一次 `--version`；跑不起来是 None）
    pub version: Option<String>,
    /// 从哪来：managed（应用内更新装的）/ bundled（随 App 带的）/ custom（设置里指定的）/ system
    pub source: String,
    /// 随 App 带的那份的版本
    pub bundled: Option<String>,
    /// 应用内更新装的、正在用的版本
    pub active: Option<String>,
    /// 上一个版本（新版出问题时退回到它）
    pub previous: Option<String>,
    /// 上次查到的最新版
    pub latest: Option<String>,
    /// 上次检查的时间（毫秒）
    pub last_check: Option<i64>,
    pub last_result: Option<String>,
    pub last_error: Option<String>,
    /// 自检没通过或被退回过的版本（每日检查不再装它们）
    pub bad: Vec<String>,
    /// 设置里的「自动更新」
    pub auto_update: bool,
    /// 正在检查
    pub checking: bool,
    /// 最近的更新记录（新的在前，最多 10 条）
    pub log: Vec<EngineLogLine>,
}

fn s(v: &Value) -> Option<String> {
    v.as_str().map(str::to_string)
}

#[uniffi::export]
impl MobileCore {
    /// 检查并安装新版 yt-dlp（「检查更新」按钮）。`force` = true 时版本一样也重新装一遍（修复用），之前自检没通过的版本也再试。
    /// 失败不会报错：结果里 `result` 是 failed / rejected，继续用原来的版本。在后台跑，App 放弃等待也会做完
    pub async fn check_engine_update(&self, force: bool) -> Result<EngineUpdate, MobileError> {
        let core = self.core.clone();
        let how = if force { How::Reinstall } else { How::Manual };
        let task = self.rt.spawn(async move {
            let r = updater::check(&core.toolbox(), &core.http(), &core.paths.data, how).await;
            EngineUpdate { result: r.result.to_string(), message: r.message, current: r.current, latest: r.latest, active: r.active, steps: r.steps }
        });
        task.await.map_err(|e| MobileError::from(anyhow::anyhow!("engine update task: {e}")))
    }

    /// 下载引擎现在的状态：版本、从哪来、自动更新的记录
    pub async fn engine_status(&self) -> EngineStatus {
        let core = self.core.clone();
        let auto = self.settings().auto_update;
        let task = self.rt.spawn(async move { updater::status(&core.toolbox(), &core.paths.data, auto).await });
        let v = task.await.unwrap_or(Value::Null);
        let log = v["log"]
            .as_array()
            .map(|a| a.iter().filter_map(|x| Some(EngineLogLine { at: x[0].as_i64()?, text: x[1].as_str()?.to_string() })).collect())
            .unwrap_or_default();
        EngineStatus {
            version: s(&v["version"]),
            source: s(&v["source"]).unwrap_or_default(),
            bundled: s(&v["bundled"]),
            active: s(&v["active"]),
            previous: s(&v["previous"]),
            latest: s(&v["latest"]),
            last_check: v["lastCheck"].as_i64(),
            last_result: s(&v["lastResult"]),
            last_error: s(&v["lastError"]),
            bad: v["bad"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default(),
            auto_update: auto,
            checking: v["checking"].as_bool().unwrap_or(false),
            log,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{InitOptions, MobileCore, Platform};

    fn open(dir: &std::path::Path, platform: Platform, lib: Option<&std::path::Path>) -> std::sync::Arc<MobileCore> {
        // 语言是进程全局的：建 Core 时会写它，和别的测试（tests::language）错开
        let _g = crate::tests::lang_lock();
        MobileCore::new(InitOptions {
            data_dir: dir.join("data").display().to_string(),
            downloads_dir: dir.join("dl").display().to_string(),
            platform,
            // 空 = 跟随系统
            language: String::new(),
            native_lib_dir: lib.map(|p| p.display().to_string()),
        })
        .unwrap()
    }

    /// iOS 不更新；Android 的状态读的是原生库目录里启动器跑出来的版本，装了新版以后换成新版
    #[cfg(unix)]
    #[test]
    fn status_and_ios_skip() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let ios = open(&dir.path().join("ios"), Platform::IosFull, None);
        let r = ios.rt.block_on(ios.check_engine_update(false)).unwrap();
        assert_eq!(r.result, "skipped");

        // 假的启动器：跑 CAPTAE_YTDLP 指的脚本，没设时报随 App 带的版本
        let lib = dir.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        let launcher = lib.join("libytdlp.so");
        std::fs::write(&launcher, "#!/bin/sh\nif [ -n \"$CAPTAE_YTDLP\" ]; then exec /bin/sh \"$CAPTAE_YTDLP\" \"$@\"; fi\necho 2026.11.23\n").unwrap();
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
        let m = open(&dir.path().join("android"), Platform::Android, Some(&lib));
        let st = m.rt.block_on(m.engine_status());
        assert_eq!((st.version.as_deref(), st.source.as_str(), st.bundled.as_deref(), st.active.as_deref()), (Some("2026.11.23"), "bundled", Some("2026.11.23"), None));
        assert!(!st.auto_update);
        // 装了新版（自动更新放好的 zipapp + 状态文件）
        let eng = m.data_dir().join("engines/yt-dlp");
        std::fs::create_dir_all(eng.join("2026.12.01")).unwrap();
        std::fs::write(eng.join("2026.12.01/yt-dlp"), "echo 2026.12.01\n").unwrap();
        std::fs::write(eng.join("state.json"), r#"{"active":"2026.12.01","previous":null}"#).unwrap();
        let st = m.rt.block_on(m.engine_status());
        assert_eq!((st.version.as_deref(), st.source.as_str(), st.active.as_deref(), st.bundled.as_deref()), (Some("2026.12.01"), "managed", Some("2026.12.01"), Some("2026.11.23")));
    }
}
