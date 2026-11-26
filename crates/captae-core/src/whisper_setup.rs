//! 一键安装语音识别：在数据目录里建一个独立的 Python 虚拟环境装 faster-whisper，
//! 再把识别模型下载到 `<数据目录>/models`。不动系统 Python，卸载 Captae 时一起删掉。
//!
//! 进度按真实下载的字节算：组件（pip，约 60 MB）和模型（small 约 480 MB）合成一条进度，
//! 剩余时间用最近的下载速度估算。模型的字节数由识别脚本的 `--download` 模式汇报——
//! 新版 huggingface_hub 走 xet 后端时文件最后才出现在 models 目录里，看目录大小是看不出进度的。

use crate::queue::Core;
use crate::tr;
use anyhow::{anyhow, bail, Context, Result};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};

/// faster-whisper 连同依赖的下载量（实测约 56 MB），pip 事先不告诉总量，用它当分母。
const PIP_EST: f64 = 60e6;

/// 各模型的大致体积，脚本报出真实总量之前先用它。
fn model_est(model: &str) -> f64 {
    match model {
        "tiny" => 78e6,
        "base" => 145e6,
        "small" => 484e6,
        "medium" => 1.53e9,
        _ => 3.1e9,
    }
}

fn mb(b: f64) -> String {
    let m = b / 1e6;
    if m >= 1000.0 {
        // 小数点跟着界面语言（德文、法文、西班牙文用逗号）
        format!("{} GB", crate::i18n::decimal(m / 1000.0, 1))
    } else {
        format!("{m:.0} MB")
    }
}

/// 下载速度 → 剩余秒数。速度取最近 10 秒的平均（比逐次加权稳，汇报间隔不规则也不会乱跳）；
/// 样本不到 3 秒时先不给。
struct Eta {
    samples: std::collections::VecDeque<(std::time::Instant, f64)>,
}

impl Eta {
    const WINDOW: f64 = 10.0;
    fn new() -> Self {
        Eta { samples: Default::default() }
    }
    fn push(&mut self, done: f64, total: f64) -> Option<u64> {
        let now = std::time::Instant::now();
        self.samples.push_back((now, done));
        while self.samples.len() > 2 && now.duration_since(self.samples[1].0).as_secs_f64() >= Self::WINDOW {
            self.samples.pop_front();
        }
        let (t0, d0) = *self.samples.front()?;
        let dt = now.duration_since(t0).as_secs_f64();
        let speed = (done - d0) / dt.max(1e-3);
        if dt < 3.0 || speed < 20e3 {
            return None;
        }
        Some(((total - done).max(0.0) / speed).ceil() as u64)
    }
}

/// 系统里能用来建虚拟环境的 Python 3：按顺序试，跑得起来、是 3.x、带 venv 的第一个。
/// Windows 先试 py（python.org 装的启动器），再试 python、python3：没装 Python 时 `python.exe`、`python3.exe` 是微软商店的占位程序
/// （%LOCALAPPDATA%\Microsoft\WindowsApps 里，运行只会提示去商店装），跑一下才分得出来
///
/// 先找语音识别组件（faster-whisper 依赖的 av、ctranslate2、onnxruntime）一定有现成 wheel 的几个版本（`PREFERRED`），
/// 电脑上装了好几个 Python 时不挑最新的：2026-11-25 Devin 的 Windows 上 `py` 默认是 3.15，这几个包还没有 3.15 的 wheel，装不上。
/// 这几个版本都没有，再用默认的那个。返回 (程序, 启动参数)：Windows 的 py 启动器要带 `-3.12` 这样的参数选版本
async fn system_python() -> Option<(PathBuf, Vec<String>)> {
    let mut tries: Vec<(&str, Vec<String>)> = vec![];
    if cfg!(windows) {
        tries.extend(PREFERRED.iter().map(|v| ("py", vec![format!("-{v}")])));
        tries.extend([("py", vec![]), ("python", vec![]), ("python3", vec![])]);
    } else {
        // python3.12 这类带版本号的程序名（deadsnakes、Fedora 的并装版本）
        let named: Vec<String> = PREFERRED.iter().map(|v| format!("python{v}")).collect();
        for n in &named {
            if let Some(p) = crate::tools::which(n) {
                if python_ok(&p, &[]).await {
                    return Some((p, vec![]));
                }
            }
        }
        tries.extend([("python3", vec![]), ("python", vec![])]);
    }
    for (n, args) in tries {
        let Some(p) = crate::tools::which(n) else { continue };
        if python_ok(&p, &args).await {
            return Some((p, args));
        }
    }
    None
}

/// 语音识别组件一定有现成 wheel 的 Python 版本，按先后挑
const PREFERRED: &[&str] = &["3.12", "3.13", "3.11", "3.10"];

/// 跑得起来、是 3.x、带 venv
async fn python_ok(p: &std::path::Path, args: &[String]) -> bool {
    let mut c = tokio::process::Command::new(p);
    c.args(args).args(["-c", "import sys, venv; print(sys.version_info[0])"]).kill_on_drop(true);
    crate::tools::no_window(&mut c);
    matches!(crate::tools::run_full(c, std::time::Duration::from_secs(30)).await, Ok(r) if r.ok && String::from_utf8_lossy(&r.stdout).trim() == "3")
}

/// 先把虚拟环境里的 pip 升到新版，返回 `--progress-bar` 用的值：pip 24.1 起有 raw（能按字节报进度），更老的用 off
async fn pip_progress_arg(py: &std::path::Path) -> &'static str {
    let mut c = tokio::process::Command::new(py);
    c.args(["-m", "pip", "install", "--disable-pip-version-check", "--quiet", "--upgrade", "pip"]).kill_on_drop(true).stdin(std::process::Stdio::null());
    crate::tools::no_window(&mut c);
    crate::tools::python_utf8(&mut c);
    match crate::tools::run_full(c, std::time::Duration::from_secs(300)).await {
        Ok(r) if r.ok => {}
        Ok(r) => tracing::warn!("升级 pip 失败：{}", r.stderr.trim()),
        Err(f) => tracing::warn!("升级 pip 失败：{}", f.raw),
    }
    let mut c = tokio::process::Command::new(py);
    c.args(["-m", "pip", "--version"]).kill_on_drop(true);
    crate::tools::no_window(&mut c);
    crate::tools::python_utf8(&mut c);
    let ver = crate::tools::run_full(c, std::time::Duration::from_secs(60)).await.ok().filter(|r| r.ok).map(|r| String::from_utf8_lossy(&r.stdout).to_string()).unwrap_or_default();
    tracing::info!("pip 版本：{}", ver.trim());
    if pip_has_raw(&ver) { "raw" } else { "off" }
}

/// `pip 24.1.2 from …` → 24.1 及以上才有 raw 进度
fn pip_has_raw(version_line: &str) -> bool {
    let v = version_line.split_whitespace().nth(1).unwrap_or("");
    let mut it = v.split('.').map(|x| x.parse::<u32>().unwrap_or(0));
    let (a, b) = (it.next().unwrap_or(0), it.next().unwrap_or(0));
    (a, b) >= (24, 1)
}

/// 这个 python 的版本（「3.14」），读不到是 None
async fn python_version(py: &std::path::Path) -> Option<String> {
    let mut c = tokio::process::Command::new(py);
    c.args(["-c", "import sys; print('%d.%d' % sys.version_info[:2])"]).kill_on_drop(true);
    crate::tools::no_window(&mut c);
    let r = crate::tools::run_full(c, std::time::Duration::from_secs(30)).await.ok()?;
    r.ok.then(|| String::from_utf8_lossy(&r.stdout).trim().to_string())
}

/// 没有 Python 3 时怎么装（按系统说）
fn no_python_hint() -> String {
    if cfg!(target_os = "macos") {
        tr!("这台电脑上没有 Python 3。macOS 可以在终端运行 xcode-select --install，或者用 Homebrew 安装 python")
    } else if cfg!(windows) {
        tr!("这台电脑上没有 Python 3。可以从 python.org 下载安装（安装时勾选「Add python.exe to PATH」），装好后再试")
    } else {
        tr!("这台电脑上没有 Python 3。请用系统的软件包管理器安装 python3 和 python3-venv（Ubuntu、Debian：sudo apt install python3 python3-venv）")
    }
}

impl Core {
    pub fn whisper_venv(&self) -> PathBuf {
        self.paths.data.join("venv")
    }

    fn venv_python(&self) -> PathBuf {
        let v = self.whisper_venv();
        if cfg!(windows) {
            v.join("Scripts").join("python.exe")
        } else {
            v.join("bin").join("python3")
        }
    }

    /// 装好后返回虚拟环境里的 python 路径（同时写进设置，之后识别都用它）。
    /// `on(进度 0–1, 现在在做什么, 估计还要几秒)`
    pub async fn install_whisper(&self, model: &str, on: impl Fn(f64, String, Option<u64>)) -> Result<PathBuf> {
        let py = self.venv_python();
        let mut total = PIP_EST + model_est(model);
        let mut eta = Eta::new();

        // ---- 1. Python 环境 ----
        if !py.exists() {
            on(0.0, tr!("准备 Python 环境（1/2）"), None);
            // 找一个系统里的 python3 来建环境（设置里的 python_path 可能已经指向旧的虚拟环境）
            let (base, base_args) = system_python().await.ok_or_else(|| anyhow!(no_python_hint()))?;
            tracing::info!("用 {} {} 建语音识别的 Python 环境", base.display(), base_args.join(" "));
            // tools::spawn：安装被取消时 Python 连同它起的进程一起结束
            let mut c = tokio::process::Command::new(&base);
            c.args(&base_args).arg("-m").arg("venv").arg(self.whisper_venv()).kill_on_drop(true).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
            crate::tools::no_window(&mut c);
            crate::tools::python_utf8(&mut c);
            let out = crate::tools::spawn(&mut c).context(tr!("启动 Python 失败"))?.output().await.context(tr!("启动 Python 失败"))?;
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                // Debian、Ubuntu 的 python3 不带 venv（要另装 python3-venv），报的是 ensurepip 不可用
                if cfg!(target_os = "linux") && (err.contains("ensurepip") || err.contains("python3-venv")) {
                    bail!(tr!("建 Python 环境失败：系统的 Python 没有 venv 组件，请先安装 python3-venv（Ubuntu、Debian：sudo apt install python3-venv）"));
                }
                bail!(tr!("建 Python 环境失败：{error}", error = err.lines().last().unwrap_or("")));
            }
        }

        // ---- 2. 组件：pip 的 raw 进度是「Progress 已下 of 本文件大小」，一个文件一个文件报 ----
        on(0.0, tr!("下载组件（1/2）"), None);
        // raw 进度是 pip 24.1 才有的。Ubuntu 22.04 的 python3-venv 建出来的环境里是 pip 22.0.2，不认 raw，一上来就报错退出
        // （2026-11-24 Devin 的 Ubuntu 上实测）。先把环境里的 pip 升到新版；升不了（离线、镜像没有）就按版本退回不报进度
        let progress = pip_progress_arg(&py).await;
        let mut c = tokio::process::Command::new(&py);
        // av<19：faster-whisper 1.2.1 传给 av.open 的 metadata_errors 在 PyAV 19 里删了，转文字会报 TypeError（识别脚本里另有兜底）。
        // --only-binary：只装现成的 wheel。系统的 Python 太新、某个依赖（ctranslate2、av……）还没有这个版本的 wheel 时，
        // pip 默认会下源码自己编（要编译器、要很久，进度一直停在 0%，2026-11-24 Devin 的 Windows 上碰到），这样会马上报「没有对应的版本」
        c.args(["-m", "pip", "install", "--disable-pip-version-check", "--progress-bar", progress, "--only-binary", ":all:", "--upgrade", "faster-whisper", "av<19"]);
        c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
        crate::tools::no_window(&mut c);
        crate::tools::python_utf8(&mut c);
        let mut child = crate::tools::spawn(&mut c).context(tr!("启动 pip 失败"))?;
        let err = child.stderr.take().unwrap();
        let err_task = tokio::spawn(async move {
            let mut s = String::new();
            let mut r = BufReader::new(err).lines();
            while let Ok(Some(l)) = r.next_line().await {
                s.push_str(&l);
                s.push('\n');
            }
            s
        });
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        let (mut finished, mut cur) = (0.0_f64, (0.0_f64, 0.0_f64));
        while let Some(l) = lines.next_line().await? {
            if let Some((x, y)) = parse_pip_progress(&l) {
                if cur.1 > 0.0 && (y != cur.1 || x < cur.0) {
                    finished += cur.1;
                }
                cur = (x, y);
                let got = finished + x;
                total = total.max(got + model_est(model));
                // 剩余时间只估这一步：pip 镜像和模型服务器速度差很多，拿这一步的速度去猜模型要多久会差出好几倍
                let left = eta.push(got, PIP_EST.max(got * 1.02));
                on((got / total).min(0.99), tr!("下载组件（1/2）· {done} / 约 {total}", done = mb(got), total = mb(PIP_EST.max(got))), left);
            } else if !l.trim().is_empty() {
                // 不是进度的行记进日志（卡住、失败时看得出卡在哪一步）
                tracing::info!("pip：{}", l.trim());
            }
            if l.starts_with("Installing collected packages") {
                on(((finished + cur.1) / total).min(0.99), tr!("安装组件（1/2）"), None);
            }
        }
        let st = child.wait().await?;
        let stderr = err_task.await.unwrap_or_default();
        if !st.success() {
            tracing::warn!("pip 失败：{}", stderr.trim());
            if stderr.contains("No matching distribution") || stderr.contains("Could not find a version") || stderr.contains("ResolutionImpossible") {
                let ver = python_version(&py).await.unwrap_or_default();
                // 这个环境的 Python 用不了：删掉，装好合适的 Python 以后再点安装会用它重建（不然一直用这个装不上的环境）
                let _ = tokio::fs::remove_dir_all(self.whisper_venv()).await;
                bail!(tr!("没有装好语音识别组件：这台电脑上的 Python {version} 还没有现成的语音识别组件。请装 Python 3.12（python.org），装好后再试", version = ver));
            }
            // pip 的报错大多以 ERROR 开头；参数不认这类是 optparse 打印的一行，没有 ERROR，退回最后一行非空的
            let last = stderr
                .lines()
                .rev()
                .find(|l| l.contains("ERROR"))
                .or_else(|| stderr.lines().rev().find(|l| !l.trim().is_empty()))
                .map(|l| l.trim().to_string())
                .unwrap_or_else(|| tr!("安装失败"));
            bail!(tr!("没有装好语音识别组件：{error}", error = last));
        }
        let pip_bytes = finished + cur.1;
        self.patch_settings(&serde_json::json!({"pythonPath": py.display().to_string(), "whisperModel": model}))?;

        // ---- 3. 模型 ----（中间安装组件那十几秒没有下载，重新开始测速）
        let mut eta = Eta::new();
        self.download_whisper_model(model, |done, model_total| {
            let all = pip_bytes + model_total;
            let left = eta.push(done, model_total);
            on(((pip_bytes + done) / all).min(0.99), tr!("下载模型（2/2）· {done} / {total}", done = mb(done), total = mb(model_total)), left);
        })
        .await?;
        on(1.0, tr!("语音转文字可以用了"), Some(0));
        Ok(py)
    }

    /// 已经下载好的模型：small、medium …（看 models 目录里 huggingface 缓存的 model.bin）
    pub fn whisper_models(&self) -> Vec<String> {
        let dir = self.paths.data.join("models");
        let Ok(rd) = std::fs::read_dir(&dir) else { return vec![] };
        let mut out: Vec<String> = rd
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let m = name.strip_prefix("models--Systran--faster-whisper-")?.to_string();
                let snaps = std::fs::read_dir(e.path().join("snapshots")).ok()?;
                snaps.flatten().any(|s| s.path().join("model.bin").exists()).then_some(m)
            })
            .collect();
        out.sort();
        out
    }

    pub fn whisper_model_ready(&self, model: &str) -> bool {
        self.whisper_models().iter().any(|m| m == model)
    }

    /// 只下载模型（组件已经装好、换了一个模型时用）。进度同安装。
    pub async fn fetch_whisper_model(&self, model: &str, on: impl Fn(f64, String, Option<u64>)) -> Result<()> {
        let mut eta = Eta::new();
        self.download_whisper_model(model, |done, total| {
            let left = eta.push(done, total);
            on((done / total).min(0.99), tr!("下载模型 · {done} / {total}", done = mb(done), total = mb(total)), left);
        })
        .await?;
        self.patch_settings(&serde_json::json!({"whisperModel": model}))?;
        on(1.0, tr!("识别模型下载好了"), Some(0));
        Ok(())
    }

    /// 让 faster-whisper 把模型拉到 models 目录（第一次识别就不用再等下载）。
    /// `on(已下载字节, 总字节)`，总字节在脚本报出来之前用估计值。
    pub async fn download_whisper_model(&self, model: &str, mut on: impl FnMut(f64, f64)) -> Result<()> {
        let script = self.paths.data.join("captae_whisper.py");
        if tokio::fs::read_to_string(&script).await.ok().as_deref() != Some(crate::subtitle::WHISPER_SCRIPT) {
            tokio::fs::write(&script, crate::subtitle::WHISPER_SCRIPT).await?;
        }
        let mut c = self.toolbox().command(crate::tools::Tool::Python)?;
        c.arg("-u").arg(&script).args(["--download", "--model", model, "--models-dir"]).arg(self.paths.data.join("models"));
        c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
        let mut child = crate::tools::spawn(&mut c).context(tr!("启动 Python 失败"))?;
        let err = child.stderr.take().unwrap();
        let err_task = tokio::spawn(async move {
            let mut s = String::new();
            let mut r = BufReader::new(err).lines();
            while let Ok(Some(l)) = r.next_line().await {
                s.push_str(&l);
                s.push('\n');
            }
            s
        });
        let est = model_est(model);
        on(0.0, est);
        let mut failed: Option<String> = None;
        let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
        while let Some(l) = lines.next_line().await? {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(&l) else { continue };
            match v["type"].as_str() {
                Some("download") => {
                    let done = v["done"].as_f64().unwrap_or(0.0);
                    // 刚开始可能只有几 KB 的配置文件在报进度，比模型估计小很多的不当作总量
                    let total = v["total"].as_f64().filter(|t| *t >= est * 0.5).unwrap_or(est).max(done);
                    on(done, total);
                }
                Some("error") => failed = Some(v["message"].as_str().unwrap_or("").to_string()),
                _ => {}
            }
        }
        let st = child.wait().await?;
        let stderr = err_task.await.unwrap_or_default();
        if st.success() && failed.is_none() {
            return Ok(());
        }
        let msg = failed.unwrap_or_else(|| stderr.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("").trim().to_string());
        let low = msg.to_ascii_lowercase();
        if ["connection", "max retries", "resolve", "timed out", "timeout", "network"].iter().any(|k| low.contains(k)) {
            bail!(tr!("下载识别模型时连不上网络（huggingface.co），可以稍后再试，或者第一次识别时会自动再下"));
        }
        bail!(tr!("下载识别模型失败：{error}", error = msg));
    }
}

/// "Progress 3145728 of 21545313" → (3145728, 21545313)
fn parse_pip_progress(l: &str) -> Option<(f64, f64)> {
    let rest = l.trim().strip_prefix("Progress ")?;
    let (a, b) = rest.split_once(" of ")?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pip_raw_by_version() {
        assert!(!pip_has_raw("pip 22.0.2 from /x/site-packages/pip (python 3.10)"));
        assert!(!pip_has_raw("pip 24.0 from /x (python 3.12)"));
        assert!(pip_has_raw("pip 24.1 from /x (python 3.12)"));
        assert!(pip_has_raw("pip 25.3 from /x (python 3.14)"));
        assert!(!pip_has_raw(""));
    }

    #[test]
    fn pip_progress() {
        assert_eq!(parse_pip_progress("Progress 3145728 of 21545313"), Some((3145728.0, 21545313.0)));
        assert_eq!(parse_pip_progress("Collecting tqdm"), None);
    }
    #[test]
    fn sizes() {
        assert_eq!(mb(484e6), "484 MB");
        assert_eq!(mb(1.53e9), "1.5 GB");
    }
}
