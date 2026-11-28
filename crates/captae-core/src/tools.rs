//! 外部命令行工具：yt-dlp、ffmpeg、ffprobe、aria2c、python(faster-whisper)、deno（JS 运行时）。
//!
//! 全部通过子进程调用，不链接它们的源码。这是许可证策略的前提：
//! ffmpeg(GPL 构建)、aria2(GPLv2+) 作为独立程序随包分发，属于「聚合」而非衍生作品，
//! Captae 自己的代码用什么许可证不受它们约束（现在是 AGPL-3.0，以后按路线图放宽）。详见 docs/LICENSING.md。
//!
//! 找工具的顺序：设置里指定的 → （yt-dlp）自动更新下载的新版 → 安装包附带的 → PATH 和 Homebrew。
//! 启动子进程时统一加上：yt-dlp 的 `--ignore-config` 和 `--js-runtimes`，以及所有联网工具的代理。

use crate::config::Settings;
use crate::errors::{ErrorCode, Failure};
use crate::tr;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;

/// 找不到外部工具（设置里没指定、安装包里没有、PATH 里也没有）。说法跟着界面语言，
/// 所以错误归类按这个类型认（`errors::from_anyhow`），不看说法里的字
#[derive(Debug, Clone)]
pub struct Missing {
    /// 找的是哪个：yt-dlp、ffmpeg、ffprobe、aria2c、python3、deno
    pub name: String,
    message: String,
}

impl std::fmt::Display for Missing {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Missing {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    YtDlp,
    Ffmpeg,
    Ffprobe,
    Aria2,
    Python,
    /// JS 运行时：yt-dlp 解 YouTube 的 JS 验证要用。默认是安装包自带的 deno，
    /// 也可能是设置里指定的、或系统里装的 node / bun
    Deno,
}

impl Tool {
    pub fn exe(self) -> &'static str {
        match self {
            Tool::YtDlp => "yt-dlp",
            Tool::Ffmpeg => "ffmpeg",
            Tool::Ffprobe => "ffprobe",
            Tool::Aria2 => "aria2c",
            Tool::Python => "python3",
            Tool::Deno => "deno",
        }
    }
}

/// yt-dlp 能用的 JS 运行时。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct JsRuntime {
    /// deno / node / bun / quickjs
    pub kind: &'static str,
    pub path: PathBuf,
    /// custom（设置里指定）/ bundled（安装包自带）/ system（PATH、Homebrew）
    pub source: &'static str,
}

impl JsRuntime {
    /// `--js-runtimes` 的值：deno:/Applications/Captae.app/Contents/MacOS/deno
    pub fn arg(&self) -> String {
        format!("{}:{}", self.kind, self.path.display())
    }
}

/// 路径 → 运行时种类（看文件名）。
pub fn js_kind(p: &Path) -> &'static str {
    let n = p.file_name().map(|n| n.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
    if n.contains("node") {
        "node"
    } else if n.contains("bun") {
        "bun"
    } else if n.contains("qjs") || n.contains("quickjs") {
        "quickjs"
    } else {
        "deno"
    }
}

// ------------------------------------------------------------------ Runner：外部程序在这个平台上怎么找、怎么跑

/// 外部程序（yt-dlp、ffmpeg、ffprobe、aria2c、python、JS 运行时）在这个平台上怎么找、能不能跑。
///
/// - 桌面端（`DesktopRunner`，默认）：照原来的查找顺序找程序，起子进程。
/// - Android 完整版（`AndroidRunner`）：也是子进程，但 Android 10 起只许执行 App 原生库目录里的文件，
///   程序打包时改名成 `lib<名字>.so`（`libffmpeg.so`、`libaria2c.so`……），目录由 App 启动时传进来。
/// - iOS（`InProcessRunner`）：不能起子进程，以后（M6）yt-dlp 在进程里的 Python 跑、ffmpeg 调库函数。
///   现在什么工具都没有，要用时返回 `Missing`（归成 `tool` 错误码）。
///
/// 起进程、结束进程树（`spawn` / `kill_tree` / `kill_all`）、取消、半成品清理不归 Runner 管，子进程的两种 Runner 共用原来那一套。
pub trait Runner: Send + Sync + std::fmt::Debug {
    /// 日志、排错用：desktop / android / in-process
    fn kind(&self) -> &'static str;

    /// 找这个工具的程序
    fn locate(&self, tb: &Toolbox, t: Tool) -> Result<PathBuf>;

    /// 有没有这个工具（不启动它）。引擎据此选路：没有 aria2c 时直链交给 Rust 自己下（engine/direct.rs），
    /// 没有 ffmpeg 时 M3U8 用 Rust 封装成 MP4（engine/remux.rs）。
    /// 桌面端一律说有：找不到时照旧在启动那一刻报 `tool`（行为和以前一样，不悄悄换成别的下载方式）
    fn has(&self, tb: &Toolbox, t: Tool) -> bool;

    /// 启动程序时另外要设的环境变量（Android：`LD_LIBRARY_PATH` 指向原生库目录，ffmpeg 的动态库在那里）
    fn env(&self) -> Vec<(String, String)> {
        vec![]
    }

    /// 能不能起子进程。iOS 不能：要用的工具在进程里跑（M6 接上）
    fn spawns(&self) -> bool {
        true
    }

    /// 起某一份 yt-dlp 的命令（还没加参数）。`target` 是 `locate(YtDlp)` 找到的、自动更新装的、或者自检要试的那一份。
    /// 桌面端它就是程序本身；Android 上新版 yt-dlp 是 zipapp（纯数据，不能直接执行），要由原生库目录里的启动器去跑
    fn ytdlp_command(&self, target: &Path) -> Command {
        Command::new(target)
    }

    /// 随 App 带的 yt-dlp（桌面端由 `Toolbox::bundled_ytdlp` 按安装包目录找，这里返回 None；Android 是原生库目录里的启动器）
    fn bundled_ytdlp(&self) -> Option<PathBuf> {
        None
    }
}

/// 桌面端：设置里指定的 → （yt-dlp）自动更新的 → 安装包附带的 → PATH 和 Homebrew
#[derive(Debug, Clone, Copy, Default)]
pub struct DesktopRunner;

impl Runner for DesktopRunner {
    fn kind(&self) -> &'static str {
        "desktop"
    }
    fn locate(&self, tb: &Toolbox, t: Tool) -> Result<PathBuf> {
        tb.desktop_path(t)
    }
    fn has(&self, _tb: &Toolbox, _t: Tool) -> bool {
        true
    }
}

/// Android 完整版：程序在 App 的原生库目录（`ApplicationInfo.nativeLibraryDir`）里，名字是 `lib<名字>.so`。
/// 设置里指定的路径照样优先（调试用）
#[derive(Debug, Clone)]
pub struct AndroidRunner {
    pub lib_dir: PathBuf,
}

impl AndroidRunner {
    pub fn new(lib_dir: impl Into<PathBuf>) -> AndroidRunner {
        AndroidRunner { lib_dir: lib_dir.into() }
    }

    /// 打包时的文件名：yt-dlp → libytdlp.so、aria2c → libaria2c.so、python3 → libpython3.so、JS 运行时 → libqjs.so（QuickJS）
    pub fn file_name(t: Tool) -> &'static str {
        match t {
            Tool::YtDlp => "libytdlp.so",
            Tool::Ffmpeg => "libffmpeg.so",
            Tool::Ffprobe => "libffprobe.so",
            Tool::Aria2 => "libaria2c.so",
            Tool::Python => "libpython3.so",
            Tool::Deno => "libqjs.so",
        }
    }
}

impl Runner for AndroidRunner {
    fn kind(&self) -> &'static str {
        "android"
    }
    fn locate(&self, tb: &Toolbox, t: Tool) -> Result<PathBuf> {
        let key = if t == Tool::Deno { "js-runtime" } else { t.exe() };
        if let Some(p) = tb.override_of(key) {
            return Ok(p);
        }
        // 自动更新装好、正在用的 yt-dlp（zipapp）：启动器还在才用（ytdlp_command 由启动器跑它）
        if t == Tool::YtDlp && self.lib_dir.join(Self::file_name(t)).is_file() {
            if let Some((_, p)) = tb.managed_ytdlp() {
                return Ok(p);
            }
        }
        let p = self.lib_dir.join(Self::file_name(t));
        if p.is_file() {
            Ok(p)
        } else {
            Err(missing(t.exe()))
        }
    }
    fn has(&self, tb: &Toolbox, t: Tool) -> bool {
        self.locate(tb, t).is_ok()
    }
    fn env(&self) -> Vec<(String, String)> {
        vec![("LD_LIBRARY_PATH".into(), self.lib_dir.display().to_string())]
    }
    /// 启动器（libytdlp.so）本身：跑随 App 带的那份 yt-dlp。别的（自动更新装的、自检要试的、设置里指定的）都是 zipapp，
    /// 用 `CAPTAE_YTDLP` 告诉启动器跑哪个（Android 10 起可写目录里的文件不能直接执行，zipapp 由启动器里的 Python 解释）
    fn ytdlp_command(&self, target: &Path) -> Command {
        let launcher = self.lib_dir.join(Self::file_name(Tool::YtDlp));
        let mut c = Command::new(&launcher);
        if target != launcher {
            c.env("CAPTAE_YTDLP", target);
        }
        c
    }
    fn bundled_ytdlp(&self) -> Option<PathBuf> {
        let p = self.lib_dir.join(Self::file_name(Tool::YtDlp));
        p.is_file().then_some(p)
    }
}

/// iOS：不能起子进程。M6 在这里接进程内的 Python（yt-dlp）和 ffmpeg 库；现在哪个工具都没有，
/// 要用时返回 `Missing`（`errors::from_anyhow` 归成 `tool`）
#[derive(Debug, Clone, Copy, Default)]
pub struct InProcessRunner;

impl Runner for InProcessRunner {
    fn kind(&self) -> &'static str {
        "in-process"
    }
    fn locate(&self, _tb: &Toolbox, t: Tool) -> Result<PathBuf> {
        Err(anyhow::Error::new(Missing { name: t.exe().into(), message: tr!("这台设备上还不能运行 {name}", name = t.exe()) }))
    }
    fn has(&self, _tb: &Toolbox, _t: Tool) -> bool {
        false
    }
    fn spawns(&self) -> bool {
        false
    }
}

fn missing(name: &str) -> anyhow::Error {
    anyhow::Error::new(Missing { name: name.into(), message: tr!("没有找到 {name}，请在设置里指定路径或重新安装 Captae", name = name) })
}

#[derive(Debug, Clone)]
pub struct Toolbox {
    /// 随安装包附带的 sidecar 目录（Tauri 的 externalBin 会放在可执行文件旁边）
    pub sidecar_dir: Option<PathBuf>,
    pub overrides: Vec<(String, PathBuf)>,
    /// 数据目录：自动更新下载的 yt-dlp 在 `<data>/engines/yt-dlp/<版本>/`
    pub data_dir: Option<PathBuf>,
    /// 代理设置（system / none / manual），子进程启动时按它设代理
    pub proxy_mode: String,
    pub proxy_url: String,
    /// 外部程序在这个平台上怎么找、能不能跑（默认桌面端）
    pub runner: std::sync::Arc<dyn Runner>,
}

impl Default for Toolbox {
    fn default() -> Self {
        Toolbox { sidecar_dir: None, overrides: vec![], data_dir: None, proxy_mode: String::new(), proxy_url: String::new(), runner: std::sync::Arc::new(DesktopRunner) }
    }
}

impl Toolbox {
    /// 换一个 Runner（手机端用）
    pub fn with_runner(mut self, runner: std::sync::Arc<dyn Runner>) -> Self {
        self.runner = runner;
        self
    }

    /// 有没有这个工具（不启动它）。见 `Runner::has`
    pub fn has(&self, t: Tool) -> bool {
        self.runner.has(self, t)
    }

    pub fn from_settings(s: &Settings, sidecar_dir: Option<PathBuf>, data_dir: Option<PathBuf>) -> Self {
        let mut overrides = vec![];
        for (k, v) in [("yt-dlp", &s.ytdlp_path), ("ffmpeg", &s.ffmpeg_path), ("aria2c", &s.aria2_path), ("python3", &s.python_path), ("js-runtime", &s.js_runtime_path)] {
            if !v.trim().is_empty() {
                overrides.push((k.to_string(), PathBuf::from(v.trim())));
            }
        }
        // ffprobe 跟着 ffmpeg 走：同一个目录，文件名里的 ffmpeg 换成 ffprobe（ffmpeg.exe → ffprobe.exe、ffmpeg-9 → ffprobe-9），
        // 文件名里没有 ffmpeg 的就叫 ffprobe
        if !s.ffmpeg_path.trim().is_empty() {
            let p = PathBuf::from(s.ffmpeg_path.trim());
            if let Some(dir) = p.parent() {
                let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let probe = if name.to_ascii_lowercase().contains("ffmpeg") { replace_ci(&name, "ffmpeg", "ffprobe") } else { exe_name("ffprobe") };
                overrides.push(("ffprobe".into(), dir.join(probe)));
            }
        }
        Toolbox { sidecar_dir, overrides, data_dir, proxy_mode: s.proxy_mode.clone(), proxy_url: s.proxy_url.clone(), ..Default::default() }
    }

    fn override_of(&self, key: &str) -> Option<PathBuf> {
        self.overrides.iter().find(|(k, _)| k == key).map(|(_, p)| p.clone())
    }

    /// 安装包附带的 yt-dlp：官方目录版放在 Tauri 的资源目录的 `yt-dlp/` 下，没有就找 sidecar 目录里的单文件版。
    /// 资源目录各平台不同（`bundled_ytdlp_candidates`）。Android：原生库目录里的启动器（跑随 App 带的那份）
    pub fn bundled_ytdlp(&self) -> Option<PathBuf> {
        if let Some(p) = self.runner.bundled_ytdlp() {
            return Some(p);
        }
        let dir = self.sidecar_dir.as_ref()?;
        bundled_ytdlp_candidates(dir).into_iter().find(|p| p.is_file())
    }

    /// 自动更新下载、自检通过、正在用的 yt-dlp（没有就 None）。
    pub fn managed_ytdlp(&self) -> Option<(String, PathBuf)> {
        crate::updater::active_engine_as(self.data_dir.as_deref()?, crate::updater::flavor_of(self.runner.kind()))
    }

    /// 桌面端用不用自动更新装的那份：有，而且不比安装包自带的旧。
    /// Captae 更新带来了更新的 yt-dlp、而 yt-dlp 的自动更新关着（或者还没来得及查）时，用安装包自带的
    fn managed_ytdlp_in_use(&self) -> Option<PathBuf> {
        let (v, p) = self.managed_ytdlp()?;
        let bundled = self.bundled_ytdlp().as_deref().and_then(bundled_ytdlp_version);
        match bundled {
            Some(b) if crate::updater::newer(&b, &v) => None,
            _ => Some(p),
        }
    }

    /// yt-dlp 从哪来：custom / managed / bundled / system
    pub fn ytdlp_source(&self) -> &'static str {
        if self.override_of("yt-dlp").is_some() {
            "custom"
        } else if self.managed_ytdlp_in_use().is_some() {
            "managed"
        } else if self.bundled_ytdlp().is_some() {
            "bundled"
        } else {
            "system"
        }
    }

    /// 不算自动更新的那一份：设置里指定的 → 安装包自带的 → PATH。自动更新拿它比版本、出问题时退回到它。
    pub fn base_ytdlp(&self) -> Option<PathBuf> {
        self.override_of("yt-dlp").or_else(|| self.bundled_ytdlp()).or_else(|| which("yt-dlp"))
    }

    /// 工具的程序在哪（按 Runner 找；手机上找不到、iOS 上一律是 `Missing`）
    pub fn path(&self, t: Tool) -> Result<PathBuf> {
        self.runner.locate(self, t)
    }

    /// 桌面端的查找顺序（`DesktopRunner` 用）
    fn desktop_path(&self, t: Tool) -> Result<PathBuf> {
        let name = t.exe();
        if t == Tool::Deno {
            return self.js_runtime().map(|r| r.path).ok_or_else(|| anyhow::Error::new(Missing { name: name.into(), message: tr!("没有找到 deno，请在设置里指定 JS 运行时或重新安装 Captae") }));
        }
        if let Some(p) = self.override_of(name) {
            return Ok(p);
        }
        if t == Tool::YtDlp {
            if let Some(p) = self.managed_ytdlp_in_use() {
                return Ok(p);
            }
            if let Some(p) = self.bundled_ytdlp() {
                return Ok(p);
            }
        } else if let Some(dir) = &self.sidecar_dir {
            let p = dir.join(exe_name(name));
            if p.exists() {
                return Ok(p);
            }
        }
        if t == Tool::Python {
            for n in ["python3", "python"] {
                if let Some(p) = which(n) {
                    return Ok(p);
                }
            }
        }
        which(name).ok_or_else(|| missing(name))
    }

    /// yt-dlp 用的 JS 运行时：设置里指定的 → 安装包自带的 deno → PATH / Homebrew 里的 deno、node、bun。
    /// 手机上按 Runner 找（Android 是原生库目录里的 QuickJS）
    pub fn js_runtime(&self) -> Option<JsRuntime> {
        if let Some(p) = self.override_of("js-runtime") {
            return Some(JsRuntime { kind: js_kind(&p), path: p, source: "custom" });
        }
        if self.runner.kind() != "desktop" {
            return self.runner.locate(self, Tool::Deno).ok().map(|p| JsRuntime { kind: js_kind(&p), path: p, source: "bundled" });
        }
        if let Some(dir) = &self.sidecar_dir {
            let p = dir.join(exe_name("deno"));
            if p.is_file() {
                return Some(JsRuntime { kind: "deno", path: p, source: "bundled" });
            }
        }
        // deno、bun 官方安装脚本装在用户目录下（Windows 也是 %USERPROFILE%\.deno\bin\deno.exe）
        let home = crate::util::home_dir();
        for (kind, extra) in [("deno", home.join(".deno/bin").join(exe_name("deno"))), ("node", PathBuf::new()), ("bun", home.join(".bun/bin").join(exe_name("bun")))] {
            if let Some(p) = which(kind).or_else(|| is_exe(&extra).then(|| extra.clone())) {
                return Some(JsRuntime { kind, path: p, source: "system" });
            }
        }
        None
    }

    /// 现在实际要用的代理（跟随系统时每 30 秒重新读一次系统设置）。
    pub fn proxy(&self) -> crate::net::Effective {
        crate::net::effective_from(&self.proxy_mode, &self.proxy_url)
    }

    pub fn command(&self, t: Tool) -> Result<Command> {
        let p = self.path(t)?;
        let c = if t == Tool::YtDlp { self.runner.ytdlp_command(&p) } else { Command::new(&p) };
        let mut c = self.prepare(c, t);
        for (k, v) in self.runner.env() {
            c.env(k, v);
        }
        Ok(c)
    }

    /// 用指定位置的 yt-dlp（自动更新的自检、体检用），参数、JS 运行时、代理和平时一样。
    /// Android 上 `exe` 是 zipapp，由启动器跑（`Runner::ytdlp_command`）
    pub fn ytdlp_at(&self, exe: &Path) -> Command {
        let mut c = self.prepare(self.runner.ytdlp_command(exe), Tool::YtDlp);
        for (k, v) in self.runner.env() {
            c.env(k, v);
        }
        c
    }

    fn prepare(&self, mut c: Command, t: Tool) -> Command {
        c.kill_on_drop(true).stdin(std::process::Stdio::null());
        match t {
            Tool::YtDlp => {
                // 用户自己的 yt-dlp 配置（~/.config/yt-dlp/config 里的 -o、-f、--write-subs、--embed-thumbnail …）
                // 会叠加到 Captae 的参数上，改掉文件名、格式和下载位置，所以一律不读
                c.arg("--ignore-config");
                python_utf8(&mut c);
                // 从访达打开时 PATH 里没有 deno，yt-dlp 自己找不到 JS 运行时，YouTube 会报 This video is not available
                // （DIAGNOSIS 第 1 节）。这里明确告诉它用哪个，先清掉默认值，免得 PATH 里的旧 deno 抢先
                if let Some(js) = self.js_runtime() {
                    c.arg("--no-js-runtimes").arg("--js-runtimes").arg(js.arg());
                }
                let eff = self.proxy();
                c.args(crate::net::ytdlp_args(&eff));
                crate::net::apply_env(&mut c, &eff);
                // 往管道里打印（`--print after_move:…%(filepath)s`、进度）一律用 UTF-8。yt-dlp 打印时按 stdout 的编码 `encode(…, 'ignore')`，
                // Windows 上是 ANSI 代码页：全角的「？」「：」「｜」被直接丢掉，Captae 拿到的路径和磁盘上的文件名对不上，报「不是视频或音频」
                // （2026-11-25 Devin 实测）。PyInstaller 打包的 yt-dlp 不认 PYTHONUTF8 这类环境变量，只能用它自己的 --encoding
                c.args(["--encoding", "utf-8"]);
            }
            Tool::Aria2 => {
                let eff = self.proxy();
                c.args(crate::net::aria2_args(&eff));
                crate::net::apply_env(&mut c, &eff);
                // Linux：静态编译的 aria2c 默认读编译时写死的证书包路径，那个文件不在时只报警告、不退回系统默认位置，
                // https 一律验证失败；明确告诉它系统证书包在哪（用户自己设了 SSL_CERT_FILE 时不动）
                if cfg!(target_os = "linux") && std::env::var_os("SSL_CERT_FILE").is_none() {
                    if let Some(ca) = system_ca_file() {
                        c.arg(format!("--ca-certificate={}", ca.display()));
                    }
                }
                if let Some(m) = aria2_modules(Path::new(c.as_std().get_program())) {
                    c.env("OPENSSL_MODULES", m);
                }
            }
            Tool::Ffmpeg | Tool::Ffprobe => crate::net::apply_env(&mut c, &self.proxy()),
            // 语音识别脚本打印的字幕里有各种语言的字（Windows 上按 ANSI 代码页写会出错）
            Tool::Python => python_utf8(&mut c),
            _ => {}
        }
        if matches!(t, Tool::Ffmpeg | Tool::Ffprobe | Tool::YtDlp | Tool::Aria2) {
            ca_bundle(&mut c);
        }
        no_window(&mut c);
        c
    }

    /// 各工具版本，用于「设置 › 关于」和排错。
    pub async fn versions(&self) -> Vec<(String, Option<String>)> {
        let mut out = vec![];
        for (t, arg) in [(Tool::YtDlp, "--version"), (Tool::Ffmpeg, "-version"), (Tool::Aria2, "--version"), (Tool::Deno, "--version")] {
            let v = match self.command(t) {
                Ok(mut c) => output(c.arg(arg)).await.ok().map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").to_string()),
                Err(_) => None,
            };
            out.push((t.exe().to_string(), v));
        }
        out
    }
}

/// 上手引导和「设置 › 关于」用：每个外部工具找没找到、版本、实际路径。
#[derive(Debug, Clone, serde::Serialize)]
pub struct DepStatus {
    /// ytdlp / ffmpeg / aria2 / deno / whisper
    pub key: &'static str,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// 从哪来：custom / managed / bundled / system（yt-dlp、JS 运行时才有）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<&'static str>,
}

impl Toolbox {
    pub async fn status(&self) -> Vec<DepStatus> {
        let mut out = vec![];
        for (key, t, arg) in [("ytdlp", Tool::YtDlp, "--version"), ("ffmpeg", Tool::Ffmpeg, "-version"), ("aria2", Tool::Aria2, "--version"), ("deno", Tool::Deno, "--version")] {
            let path = self.path(t).ok();
            let version = match self.command(t) {
                Ok(mut c) => output(c.arg(arg)).await.ok().filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).lines().next().unwrap_or("").trim().to_string()),
                Err(_) => None,
            };
            let source = match t {
                Tool::YtDlp => Some(self.ytdlp_source()),
                Tool::Deno => self.js_runtime().map(|r| r.source),
                _ => None,
            };
            out.push(DepStatus { key, ok: version.is_some(), version, path, source });
        }
        // 语音转文字：python 能 import faster_whisper 才算可用
        let path = self.path(Tool::Python).ok();
        let version = match self.command(Tool::Python) {
            Ok(mut c) => output(c.args(["-c", "import faster_whisper,sys;sys.stdout.write(faster_whisper.__version__)"]))
                .await
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()),
            Err(_) => None,
        };
        out.push(DepStatus { key: "whisper", ok: version.is_some(), version, path, source: None });
        out
    }
}

/// 安装包附带的 yt-dlp 可能在的位置（`dir` 是可执行文件所在的目录），按顺序找：
/// - macOS：`Contents/Resources/yt-dlp/yt-dlp_macos`（可执行文件在 `Contents/MacOS/`）
/// - Windows（NSIS）：资源目录就是安装目录，`yt-dlp\yt-dlp.exe`（官方 `yt-dlp_win.zip` 解开就是 yt-dlp.exe 加 `_internal\`）
/// - Linux：.deb 的程序在 `/usr/bin/`、资源在 `/usr/lib/<productName>/`，AppImage 里也是 `usr/bin` 和 `usr/lib/<productName>`；
///   官方 `yt-dlp_linux.zip` 解开是 yt-dlp_linux 加 `_internal/`
/// - 最后：可执行文件旁边的单文件版 yt-dlp（开发时、只放了单文件版的打包）
/// 安装包自带的 yt-dlp 是哪一版：打包时写在它旁边的 `version.txt`（fetch-sidecars 三个脚本），不用运行它
pub fn bundled_ytdlp_version(exe: &Path) -> Option<String> {
    let v = std::fs::read_to_string(exe.parent()?.join("version.txt")).ok()?;
    let v = v.trim();
    crate::updater::valid_version(v).then(|| v.to_string())
}

fn bundled_ytdlp_candidates(dir: &Path) -> Vec<PathBuf> {
    let mut v = vec![];
    if cfg!(target_os = "macos") {
        v.push(dir.join("../Resources/yt-dlp/yt-dlp_macos"));
    } else if cfg!(windows) {
        v.push(dir.join("yt-dlp").join("yt-dlp.exe"));
    } else {
        // 官方目录版的主程序名：x86_64 是 yt-dlp_linux，ARM 是 yt-dlp_linux_aarch64
        let main = if cfg!(target_arch = "aarch64") { "yt-dlp/yt-dlp_linux_aarch64" } else { "yt-dlp/yt-dlp_linux" };
        for name in ["Captae", "captae", "captae-desktop"] {
            v.push(dir.join("../lib").join(name).join(main));
        }
        v.push(dir.join(main));
    }
    v.push(dir.join(exe_name("yt-dlp")));
    v
}

/// 安装包附带程序所在的目录。一般就是程序自己所在的目录（macOS 的 Contents/MacOS、Windows 的安装目录）；
/// Linux 的 deb 和 AppImage 把附带程序平铺在 Tauri 的资源目录 `<exe 目录>/../lib/Captae/` 里（放 /usr/bin 会和系统的 ffmpeg、aria2c 冲突，
/// 见 scripts/build-release-linux.sh），那里有 captae 时用那里。桌面端、命令行、找插件宿主程序都用它
pub fn sidecar_dir_of(exe: &Path) -> Option<PathBuf> {
    let dir = exe.parent()?;
    if cfg!(target_os = "linux") {
        let lib = dir.join("../lib/Captae");
        if lib.join("captae").is_file() {
            return Some(lib.canonicalize().unwrap_or(lib));
        }
    }
    Some(dir.to_path_buf())
}

/// 不分大小写，把第一个 `from` 换成 `to`（文件名里的 FFmpeg.exe 也认）
fn replace_ci(s: &str, from: &str, to: &str) -> String {
    match s.to_ascii_lowercase().find(&from.to_ascii_lowercase()) {
        Some(i) => format!("{}{to}{}", &s[..i], &s[i + from.len()..]),
        None => s.to_string(),
    }
}

/// 让 Python 程序（yt-dlp 是打包好的 Python）往管道里写字时用 UTF-8、不缓冲。
/// Windows 上不设的话用系统的 ANSI 代码页（cp1252、cp936……）：yt-dlp 把文件名里的 `?` `:` 换成全角的 `？` `：`，
/// 打印最终路径（`--print after_move:…%(filepath)s`）时全角字编不进代码页，Captae 拿到的路径和磁盘上的文件对不上，
/// 下载完报「不是视频或音频」（2026-11-24 Devin 上 TED、Dailymotion 实测）；别的语言的标题还会让按行读输出直接出错。
/// pip 的进度不缓冲才能一行行及时读到。macOS、Linux 上从图形界面打开时语言环境可能是 C，设上也一样稳妥
pub fn python_utf8(c: &mut Command) {
    c.env("PYTHONIOENCODING", "utf-8").env("PYTHONUTF8", "1").env("PYTHONUNBUFFERED", "1");
}

fn exe_name(n: &str) -> String {
    if cfg!(windows) {
        format!("{n}.exe")
    } else {
        n.to_string()
    }
}

pub fn which(name: impl AsRef<OsStr>) -> Option<PathBuf> {
    let name = name.as_ref();
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH").map(|p| std::env::split_paths(&p).collect()).unwrap_or_default();
    // 从访达/程序坞打开的 app、被 Chrome 拉起的插件宿主，PATH 只有 /usr/bin:/bin:/usr/sbin:/sbin，
    // 找不到 Homebrew 和 pipx 装的工具，这里补上常见位置（排在 PATH 之后）。
    // Linux 从桌面菜单打开的程序 PATH 里常常没有 ~/.local/bin（pipx、pip --user 装的 yt-dlp 在这里）
    let home = crate::util::home_dir();
    let extra: Vec<PathBuf> = if cfg!(target_os = "macos") {
        vec![PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin"), home.join(".local/bin")]
    } else if cfg!(target_os = "linux") && !home.as_os_str().is_empty() {
        vec![home.join(".local/bin"), PathBuf::from("/usr/local/bin")]
    } else {
        vec![]
    };
    for d in extra {
        if !dirs.contains(&d) {
            dirs.push(d);
        }
    }
    // Windows：没写扩展名的按 .exe 找（不认 .bat / .cmd：Captae 直接启动它们会经过 cmd.exe，参数里的特殊字符不安全）
    let win_exe = cfg!(windows) && Path::new(name).extension().is_none();
    for dir in dirs {
        let p = dir.join(name);
        if is_exe(&p) {
            return Some(p);
        }
        if win_exe {
            let p = dir.join(format!("{}.exe", name.to_string_lossy()));
            if is_exe(&p) {
                return Some(p);
            }
        }
    }
    None
}

fn is_exe(p: &Path) -> bool {
    p.is_file()
}

/// Windows：命令行程序起来时不弹控制台窗口（桌面端是窗口程序，不加这个每起一个工具就闪一下黑窗）。
/// 不经过 `Toolbox::command` 起的程序（建 Python 环境、pip）也要调它
#[cfg(windows)]
pub fn no_window(c: &mut Command) {
    c.creation_flags(windows_sys::Win32::System::Threading::CREATE_NO_WINDOW);
}
#[cfg(not(windows))]
pub fn no_window(_: &mut Command) {}

/// 附带的 ffmpeg 9 是静态编译的 OpenSSL：默认校验证书，却找不到系统的根证书，读 https 地址一律
/// 「certificate verify failed」（2026-11-23 实测，直连也一样）。告诉它用 macOS 自带的根证书包 /etc/ssl/cert.pem，
/// 不关掉校验。yt-dlp 也设：它在 `--download-sections`、部分直播流时会调用 ffmpeg 读网址（它自己用 certifi，不受影响），
/// 交给 aria2c 下载时 aria2c 也继承这个设置。
/// aria2c 也设：附带的 aria2c 从 Homebrew 拷来，用 OpenSSL 校验证书，默认读打包那台电脑上 Homebrew 的 cert.pem，
/// 没装 Homebrew 的 Mac 上所有 https 直链都「unable to get local issuer certificate」（2026-11-24 挡住 Homebrew 目录实测）。
///
/// Linux：附带的 ffmpeg（BtbN 的静态构建）静态链了 OpenSSL，默认读 /etc/ssl/cert.pem，Debian / Ubuntu 上没有这个文件
/// （它们的证书包在 /etc/ssl/certs/ca-certificates.crt），静态编译的 aria2c 也一样；按发行版的惯例找系统证书包，找到哪个用哪个。
/// Windows 不用设：附带的 ffmpeg（BtbN 的 Windows 构建，用 schannel，2026-11-24 核对 FFmpeg-Builds 的 scripts.d/50-schannel.sh、25-openssl.sh：
/// OpenSSL 只给 Linux 用）和 aria2c（官方 Windows 版用 WinTLS）都读系统的证书库；yt-dlp 用它自带的 certifi。
/// 用户自己已经设了 SSL_CERT_FILE 时不动
fn ca_bundle(c: &mut Command) {
    if std::env::var_os("SSL_CERT_FILE").is_some() {
        return;
    }
    if let Some(ca) = system_ca_file() {
        c.env("SSL_CERT_FILE", ca);
    }
}

/// 系统的根证书包（PEM），没有时 None
pub fn system_ca_file() -> Option<&'static Path> {
    ca_candidates().iter().map(Path::new).find(|p| p.is_file())
}

fn ca_candidates() -> &'static [&'static str] {
    if cfg!(target_os = "macos") {
        &["/etc/ssl/cert.pem"]
    } else if cfg!(target_os = "linux") {
        &[
            // Debian、Ubuntu、Arch、Gentoo
            "/etc/ssl/certs/ca-certificates.crt",
            // Fedora、RHEL
            "/etc/pki/tls/certs/ca-bundle.crt",
            "/etc/pki/ca-trust/extracted/pem/tls-ca-bundle.pem",
            // openSUSE
            "/etc/ssl/ca-bundle.pem",
            "/var/lib/ca-certificates/ca-bundle.pem",
            // Alpine、别的
            "/etc/ssl/cert.pem",
        ]
    } else {
        &[]
    }
}

/// 安装包附带的 aria2c 要用的 OpenSSL 模块目录（Contents/Resources/aria2-libs/ossl-modules），不是安装包里那个 aria2c 时是 None。
/// aria2c 是从 Homebrew 拷来的，它带的 libcrypto 把模块目录写死成打包那台电脑上的 /opt/homebrew/Cellar/openssl@3/<版本>/lib/ossl-modules；
/// aria2 1.37 一启动就加载其中的 legacy 模块，加载不了直接退出（「OSSL_PROVIDER_load 'legacy' failed」）。
/// 别人的 Mac 上没有那个目录；Developer ID 签名的包开着库校验，也不许加载别人签的模块。所以模块随包带着（scripts/fetch-sidecars.sh），
/// 启动 aria2c 时用 OPENSSL_MODULES 指过去（yt-dlp 把下载交给 aria2c 时也一样，aria2c 继承 yt-dlp 的环境变量）。
/// 设置里指定的、Homebrew 自己的 aria2c 旁边没有这个目录，不管
pub fn aria2_modules(aria2: &Path) -> Option<PathBuf> {
    let d = aria2.parent()?.join("../Resources/aria2-libs/ossl-modules");
    d.join("legacy.dylib").is_file().then_some(d)
}

/// 子进程启动不了（文件没有、没有执行权限、CPU 架构不对）→ 组件问题。
pub fn spawn_failure(what: &str, e: &std::io::Error) -> Failure {
    Failure::new(ErrorCode::Tool, tr!("下载组件出了问题（{tool} 启动不了）", tool = what), tr!("启动外部工具失败：{tool}: {error}", tool = what, error = e))
}

// ------------------------------------------------------------------ 子进程：连同它启动的子孙进程一起结束

/// 用 `spawn` 启动的外部工具。被丢掉时（超时、暂停、取消：整个 future 被丢掉）如果还在跑，
/// 把它和它启动的子孙进程一起结束（`kill_tree`）。
///
/// 以前只靠 `kill_on_drop`：只给直接子进程（yt-dlp）发 SIGKILL，yt-dlp 来不及收拾它自己启动的 ffmpeg，
/// ffmpeg 留成孤儿继续跑（第三阶段 FIXES 第 15 项：腾讯视频的样片超时以后 ffmpeg 在后台跑了 23 分钟）。
///
/// 没有让工具进自己的进程组（`process_group(0)` + killpg）：那样从外面对 Captae 所在的进程组发信号
/// （终端里按 Ctrl-C、测试脚本 `run.py` 超时时 `os.killpg(…, SIGKILL)`）就到不了 yt-dlp 和 ffmpeg，反而会留下孤儿。
/// 现在它们还和 Captae 在同一个进程组，外面结束整个组照样都结束；Captae 自己结束时按进程树找。
///
/// Windows：每个工具起来以后放进自己的 Job Object（设了「Job 句柄关掉时结束组里所有进程」），它起的 ffmpeg、aria2c 自动也在组里。
/// `kill_tree` 结束整个 Job；`Proc` 被丢掉时关掉 Job 句柄，组里还剩的进程（包括工具正常退出后留下的孙进程）一起结束；
/// Captae 自己崩了、被任务管理器结束时，系统关掉它的句柄，工具也跟着结束。
pub struct Proc {
    child: tokio::process::Child,
    /// 启动时的进程号（登记在 LIVE 里）
    pid: Option<u32>,
}

impl std::ops::Deref for Proc {
    type Target = tokio::process::Child;
    fn deref(&self) -> &tokio::process::Child {
        &self.child
    }
}

impl std::ops::DerefMut for Proc {
    fn deref_mut(&mut self) -> &mut tokio::process::Child {
        &mut self.child
    }
}

impl Proc {
    /// 等它结束，同时读完 stdout、stderr（和 `Command::output` 一样，但被丢掉时整棵进程树都会结束）
    pub async fn output(&mut self) -> std::io::Result<std::process::Output> {
        use tokio::io::AsyncReadExt;
        let mut out = self.child.stdout.take();
        let mut err = self.child.stderr.take();
        let fo = async {
            let mut v = Vec::new();
            if let Some(o) = out.as_mut() {
                o.read_to_end(&mut v).await?;
            }
            Ok::<_, std::io::Error>(v)
        };
        let fe = async {
            let mut v = Vec::new();
            if let Some(e) = err.as_mut() {
                e.read_to_end(&mut v).await?;
            }
            Ok::<_, std::io::Error>(v)
        };
        let (status, stdout, stderr) = tokio::try_join!(self.child.wait(), fo, fe)?;
        Ok(std::process::Output { status, stdout, stderr })
    }
}

impl Drop for Proc {
    fn drop(&mut self) {
        // 已经正常结束（wait 过了）的不用管；还在跑的连同子孙一起结束
        if matches!(self.child.try_wait(), Ok(None)) {
            if let Some(pid) = self.child.id() {
                kill_tree(pid);
            }
        }
        if let Some(pid) = self.pid {
            LIVE.lock().unwrap().remove(&pid);
            // 关掉 Job 句柄：组里还剩的进程（工具自己退出了、它起的子进程还在）一起结束
            #[cfg(windows)]
            win::release(pid);
        }
    }
}

/// 正在跑的外部工具（`spawn` 启动、还没丢掉的）。命令行、插件宿主收到 SIGTERM 时用 `kill_all` 把它们连同子孙一起结束：
/// 进程直接退出时 future 不会被丢掉，`Proc` 没机会收拾
static LIVE: std::sync::Mutex<std::collections::BTreeSet<u32>> = std::sync::Mutex::new(std::collections::BTreeSet::new());

/// 启动外部工具（`Toolbox::command` 准备好的命令）。返回的 `Proc` 被丢掉时，它启动的 ffmpeg、aria2c 也一起结束。
pub fn spawn(c: &mut Command) -> std::io::Result<Proc> {
    let child = c.spawn()?;
    let pid = child.id();
    if let Some(p) = pid {
        // Windows：马上放进它自己的 Job。放进去之前那一瞬间它已经起的子进程不在组里，`kill_tree` 再按进程快照补上
        #[cfg(windows)]
        if let Some(h) = child.raw_handle() {
            win::adopt(p, h);
        }
        LIVE.lock().unwrap().insert(p);
    }
    Ok(Proc { child, pid })
}

/// 跑完一个命令、拿到全部输出（和 `Command::output` 一样），只是经过 `spawn`：登记在正在跑的外部工具里，
/// Captae 退出时还没跑完的连同子进程一起结束（检查组件、查版本、找关键画面这类）；被取消时也一样
pub async fn output(c: &mut Command) -> std::io::Result<std::process::Output> {
    c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    spawn(c)?.output().await
}

/// 结束所有还在跑的外部工具和它们的子孙（进程要退出时调用），再删掉它们没写完的输出文件（转换、剪辑到一半的，见 `tasks::Partial`）。
/// 下载的断点续传文件、录直播录到的部分不在里面（下次打开接着下、整理好）
pub fn kill_all() {
    let pids: Vec<u32> = LIVE.lock().unwrap().iter().copied().collect();
    for p in pids {
        kill_tree(p);
    }
    crate::tasks::discard_partials();
}

/// 结束一个进程和它的全部子孙：先从上往下逐个 SIGSTOP（停住的进程不能再起新的子进程，父进程也还在，
/// 子进程不会被交给 launchd、从树上丢掉），全部停住以后再一起 SIGKILL。
#[cfg(unix)]
pub fn kill_tree(root: u32) {
    let me = std::process::id() as i32;
    let mut seen: Vec<i32> = vec![];
    let mut stack = vec![root as i32];
    // 第二遍：停住以后再看一遍，接住正在 fork 的那一刻冒出来的
    for _ in 0..2 {
        while let Some(p) = stack.pop() {
            if p <= 1 || p == me || seen.contains(&p) {
                continue;
            }
            unsafe { sys::kill(p, sys::SIGSTOP) };
            seen.push(p);
            stack.extend(sys::children(p));
        }
        stack = seen.iter().flat_map(|p| sys::children(*p)).filter(|c| !seen.contains(c)).collect();
        if stack.is_empty() {
            break;
        }
    }
    for p in seen {
        unsafe { sys::kill(p, sys::SIGKILL) };
    }
}

/// 结束一个进程和它的全部子孙（Windows）：先结束它的 Job（`spawn` 时放进去的，组里的子孙一起结束），
/// 再按进程快照找不在组里的子孙逐个结束（放进 Job 之前那一瞬间起的、Job 没建成的），看两遍，接住正在起的。
/// Windows 的进程号会复用，父进程号对得上还要看创建时间：比父进程早创建的不是它的孩子
#[cfg(windows)]
pub fn kill_tree(root: u32) {
    let me = std::process::id();
    if root == 0 || root == me {
        return;
    }
    win::terminate_job(root);
    // 找到的进程（进程号，创建时间）。第一遍从根往下找、全部结束；第二遍从找到的这些往下再找一遍，只结束新冒出来的
    // （结束过的不再碰：它的进程号可能已经被别的程序复用了）
    let mut seen: Vec<(u32, u64)> = vec![(root, win::created(root).unwrap_or(0))];
    let mut done = 0;
    for _ in 0..2 {
        let table = win::snapshot();
        let mut stack = seen.clone();
        while let Some((p, born)) = stack.pop() {
            for &(c, _) in table.iter().filter(|(c, parent)| *parent == p && *c != 0 && *c != me && *c != p) {
                if seen.iter().any(|(s, _)| *s == c) {
                    continue;
                }
                let t = win::created(c);
                if t.is_some_and(|t| t < born) {
                    continue;
                }
                seen.push((c, t.unwrap_or(born)));
                stack.push((c, t.unwrap_or(born)));
            }
        }
        if seen.len() == done {
            break;
        }
        for (p, _) in &seen[done..] {
            win::terminate(*p);
        }
        done = seen.len();
    }
}

#[cfg(not(any(unix, windows)))]
pub fn kill_tree(_root: u32) {}

/// 请一个进程自己收尾（SIGINT，和在终端里按 Ctrl-C 一样），只发给这一个进程、不发给它的子进程。
/// 录直播「停止并保存」用：yt-dlp 收到以后让它起的 ffmpeg 写完文件（往 ffmpeg 的输入里写 q），直播当作正常结束接着整理；
/// aria2c 收到以后把缓存写进文件再退出。收不了尾的，调用方过一会儿再用 `kill_tree` 结束整棵进程树。
/// 返回请求送没送到：送不到时调用方不用等，马上结束进程树、自己整理录到的部分
#[cfg(unix)]
pub fn interrupt(pid: u32) -> bool {
    pid > 1 && pid as i32 != std::process::id() as i32 && unsafe { sys::kill(pid as i32, sys::SIGINT) } == 0
}

/// Windows 没有 SIGINT。外部工具起的时候带 CREATE_NO_WINDOW，各有一个看不见的控制台；往那个控制台发 Ctrl-C
/// （`GenerateConsoleCtrlEvent(CTRL_C_EVENT)`），控制台里的进程都收到：
/// - yt-dlp（Python）把 Ctrl-C 当 KeyboardInterrupt，录直播时当作正常结束、接着整理（`ExternalFD.real_download` 里 `is_live` 的那段），
///   它起的 ffmpeg 在同一个控制台里也收到 Ctrl-C，自己写完文件（yt-dlp 源码里的注释：Windows 上 ffmpeg 这样收尾得到的文件能播）；
/// - aria2c 收到 Ctrl-C（SIGINT）把缓存写进文件再退出。
///
/// 没用 CTRL_BREAK + CREATE_NEW_PROCESS_GROUP：Python 对 Ctrl-Break 默认直接结束进程（不进 KeyboardInterrupt），yt-dlp 收不了尾；
/// aria2c 也只接 SIGINT。新进程组还会让组里的进程忽略 Ctrl-C。
///
/// 只能往自己连着的控制台发，而换控制台（FreeConsole / AttachConsole）是整个进程的事，命令行自己的控制台、别的线程都会受影响，
/// 所以交给一个一次性的辅助进程去发：`captae __ctrl-c <pid>`（见 `send_ctrl_c`），它没有控制台、发完就退出。
/// 找不到 captae（只编了桌面端）或者发不出去时返回 false，调用方走兜底：结束进程树，用 ffmpeg 原样整理录到的部分
#[cfg(windows)]
pub fn interrupt(pid: u32) -> bool {
    use std::os::windows::process::CommandExt;
    if pid == 0 || pid == std::process::id() {
        return false;
    }
    let Some(helper) = ctrl_c_helper() else {
        tracing::info!("没有找到 captae，发不了 Ctrl-C，直接结束进程树");
        return false;
    };
    let mut c = std::process::Command::new(&helper);
    c.arg(CTRL_C_ARG).arg(pid.to_string()).stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
    c.creation_flags(windows_sys::Win32::System::Threading::DETACHED_PROCESS);
    match c.status() {
        Ok(s) => s.success(),
        Err(e) => {
            tracing::warn!("启动 {} 发 Ctrl-C 失败：{e}", helper.display());
            false
        }
    }
}

#[cfg(not(any(unix, windows)))]
pub fn interrupt(_pid: u32) -> bool {
    false
}

/// 命令行的隐藏子命令：`captae __ctrl-c <pid>`，给 `interrupt` 当辅助进程（Windows）
pub const CTRL_C_ARG: &str = "__ctrl-c";

/// 发 Ctrl-C 的辅助程序：自己就是 captae（命令行）就用自己；桌面端用旁边的 captae.exe（安装包里的 sidecar）；
/// 开发时测试程序在 target/<profile>/deps/ 里，captae.exe 在上一层
#[cfg(windows)]
fn ctrl_c_helper() -> Option<PathBuf> {
    let me = std::env::current_exe().ok()?;
    if me.file_stem().is_some_and(|s| s.eq_ignore_ascii_case("captae")) {
        return Some(me);
    }
    let dir = me.parent()?;
    [dir.join("captae.exe"), dir.join("../captae.exe")].into_iter().find(|p| p.is_file())
}

/// `captae __ctrl-c <pid>` 里做的事：连到那个进程的控制台，发一次 Ctrl-C。只在辅助进程里调用：
/// 它会换掉本进程的控制台，并让本进程从此忽略 Ctrl-C。送到了返回 true
#[cfg(windows)]
pub fn send_ctrl_c(pid: u32) -> bool {
    use windows_sys::Win32::System::Console::{AttachConsole, FreeConsole, GenerateConsoleCtrlEvent, SetConsoleCtrlHandler, CTRL_C_EVENT};
    unsafe {
        FreeConsole();
        if AttachConsole(pid) == 0 {
            return false;
        }
        // 自己也连在这个控制台上，先让自己忽略 Ctrl-C
        SetConsoleCtrlHandler(None, 1);
        let ok = GenerateConsoleCtrlEvent(CTRL_C_EVENT, 0) != 0;
        // Ctrl-C 是另起线程送到各个进程的，等一会儿再断开
        std::thread::sleep(Duration::from_millis(300));
        FreeConsole();
        ok
    }
}

#[cfg(not(windows))]
pub fn send_ctrl_c(_pid: u32) -> bool {
    false
}

/// 测试用：这个进程还在不在（unix：kill 0 只看不发信号，结束了没被收走的僵尸进程也算在；Windows：`alive`）
#[cfg(all(test, unix))]
pub(crate) fn test_alive(pid: u32) -> bool {
    unsafe { sys::kill(pid as i32, 0) == 0 }
}

#[cfg(all(test, windows))]
pub(crate) fn test_alive(pid: u32) -> bool {
    alive(pid)
}

/// 这个进程还在不在（kill 0 只看不发信号；进程在、但不归我们管时回 EPERM，也算在）。
/// 插件宿主据此分清桌面端是「没开」还是「一时没回话」
#[cfg(unix)]
pub fn alive(pid: u32) -> bool {
    if pid <= 1 {
        return false;
    }
    let r = unsafe { sys::kill(pid as i32, 0) };
    r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(1)
}

/// 这个进程还在不在（Windows：打开它、看退出码是不是 STILL_ACTIVE；打不开但回「拒绝访问」的是别人的进程，也算在）
#[cfg(windows)]
pub fn alive(pid: u32) -> bool {
    pid != 0 && win::alive(pid)
}

#[cfg(not(any(unix, windows)))]
pub fn alive(_pid: u32) -> bool {
    true
}

#[cfg(windows)]
pub(crate) mod win {
    use std::collections::BTreeMap;
    use std::sync::Mutex;
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED, FILETIME, HANDLE, INVALID_HANDLE_VALUE, STILL_ACTIVE};
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation, SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    };
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, GetProcessTimes, OpenProcess, TerminateProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_TERMINATE};

    /// 每个外部工具的 Job：进程号 → Job 句柄（存成 usize：裸指针不能放进 static）
    static JOBS: Mutex<BTreeMap<u32, usize>> = Mutex::new(BTreeMap::new());

    /// 建一个「句柄关掉就结束组里所有进程」的 Job，把刚起的工具放进去。失败（比如 Captae 自己所在的 Job 不许嵌套）就算了，
    /// `kill_tree` 退回按进程快照找子孙
    pub fn adopt(pid: u32, process: std::os::windows::io::RawHandle) {
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return;
            }
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let ok = SetInformationJobObject(job, JobObjectExtendedLimitInformation, &info as *const _ as *const _, std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32) != 0
                && AssignProcessToJobObject(job, process as HANDLE) != 0;
            if !ok {
                tracing::debug!("外部工具 {pid} 放不进 Job（{}），结束时按进程树找", GetLastError());
                CloseHandle(job);
                return;
            }
            if let Some(old) = JOBS.lock().unwrap().insert(pid, job as usize) {
                CloseHandle(old as HANDLE);
            }
        }
    }

    /// 结束这个工具的 Job 里所有进程（没有 Job 时什么也不做）
    pub fn terminate_job(pid: u32) {
        if let Some(j) = JOBS.lock().unwrap().get(&pid) {
            unsafe { TerminateJobObject(*j as HANDLE, 1) };
        }
    }

    /// 关掉 Job 句柄（KILL_ON_JOB_CLOSE：组里还在的进程一起结束）
    pub fn release(pid: u32) {
        if let Some(j) = JOBS.lock().unwrap().remove(&pid) {
            unsafe { CloseHandle(j as HANDLE) };
        }
    }

    /// 所有进程的（进程号，父进程号）
    pub fn snapshot() -> Vec<(u32, u32)> {
        let mut out = vec![];
        unsafe {
            let snap = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
            if snap == INVALID_HANDLE_VALUE || snap.is_null() {
                return out;
            }
            let mut e: PROCESSENTRY32W = std::mem::zeroed();
            e.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            if Process32FirstW(snap, &mut e) != 0 {
                loop {
                    out.push((e.th32ProcessID, e.th32ParentProcessID));
                    if Process32NextW(snap, &mut e) == 0 {
                        break;
                    }
                }
            }
            CloseHandle(snap);
        }
        out
    }

    /// 直接子进程（按快照的父进程号，比父进程早创建的不算：那是进程号被复用了）
    #[cfg(test)]
    pub fn children(pid: u32) -> Vec<u32> {
        let born = created(pid).unwrap_or(0);
        snapshot().into_iter().filter(|(c, p)| *p == pid && *c != 0 && *c != pid && created(*c).is_none_or(|t| t >= born)).map(|(c, _)| c).collect()
    }

    /// 进程的创建时间（FILETIME，100 纳秒为单位），打不开时 None
    pub fn created(pid: u32) -> Option<u64> {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let z = FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
            let (mut c, mut e, mut k, mut u) = (z, z, z, z);
            let ok = GetProcessTimes(h, &mut c, &mut e, &mut k, &mut u) != 0;
            CloseHandle(h);
            ok.then(|| (c.dwHighDateTime as u64) << 32 | c.dwLowDateTime as u64)
        }
    }

    /// 进程的程序完整路径（QueryFullProcessImageNameW），打不开时 None。插件宿主认浏览器用
    pub fn image_path(pid: u32) -> Option<std::path::PathBuf> {
        use std::os::windows::ffi::OsStringExt;
        use windows_sys::Win32::System::Threading::{QueryFullProcessImageNameW, PROCESS_NAME_WIN32};
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return None;
            }
            let mut buf = vec![0u16; 32 * 1024];
            let mut len = buf.len() as u32;
            let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0;
            CloseHandle(h);
            ok.then(|| std::path::PathBuf::from(std::ffi::OsString::from_wide(&buf[..len as usize])))
        }
    }

    pub fn terminate(pid: u32) {
        unsafe {
            let h = OpenProcess(PROCESS_TERMINATE, 0, pid);
            if !h.is_null() {
                TerminateProcess(h, 1);
                CloseHandle(h);
            }
        }
    }

    pub fn alive(pid: u32) -> bool {
        unsafe {
            let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
            if h.is_null() {
                return GetLastError() == ERROR_ACCESS_DENIED;
            }
            let mut code = 0u32;
            let ok = GetExitCodeProcess(h, &mut code) != 0;
            CloseHandle(h);
            ok && code == STILL_ACTIVE as u32
        }
    }
}

#[cfg(unix)]
mod sys {
    extern "C" {
        pub fn kill(pid: i32, sig: i32) -> i32;
    }
    pub const SIGKILL: i32 = 9;
    pub const SIGINT: i32 = 2;
    // Android 的 target_os 是 "android" 不是 "linux"，信号编号和 Linux 一样（SIGSTOP 是 19；17 在 Linux 上是 SIGCHLD）
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub const SIGSTOP: i32 = 19;
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub const SIGSTOP: i32 = 17;

    /// 直接子进程（macOS：libproc 的 proc_listchildpids，在系统库 libSystem 里）
    #[cfg(target_os = "macos")]
    pub fn children(pid: i32) -> Vec<i32> {
        extern "C" {
            fn proc_listchildpids(ppid: i32, buffer: *mut std::ffi::c_void, buffersize: i32) -> i32;
        }
        let mut buf = vec![0i32; 1024];
        let n = unsafe { proc_listchildpids(pid, buf.as_mut_ptr().cast(), (buf.len() * std::mem::size_of::<i32>()) as i32) };
        if n <= 0 {
            return vec![];
        }
        buf.truncate((n as usize).min(buf.len()));
        buf.retain(|p| *p > 1);
        buf
    }

    /// 直接子进程（Linux、Android：扫 /proc/*/stat 的父进程号）
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn children(pid: i32) -> Vec<i32> {
        let Ok(rd) = std::fs::read_dir("/proc") else { return vec![] };
        rd.flatten()
            .filter_map(|e| e.file_name().to_str().and_then(|n| n.parse::<i32>().ok()))
            .filter(|c| {
                std::fs::read_to_string(format!("/proc/{c}/stat"))
                    .ok()
                    .and_then(|s| s.rsplit_once(')').and_then(|(_, rest)| rest.split_whitespace().nth(1).and_then(|x| x.parse::<i32>().ok())))
                    == Some(pid)
            })
            .collect()
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "android")))]
    pub fn children(_pid: i32) -> Vec<i32> {
        vec![]
    }
}

/// 跑一个命令直到结束，失败时把 stderr 最后几行带进错误里。
pub async fn run_ok(mut c: Command) -> Result<Vec<u8>> {
    let what = c.as_std().get_program().to_string_lossy().to_string();
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut p = spawn(&mut c).map_err(|e| spawn_failure(&what, &e))?;
    let out = p.output().await.map_err(|e| spawn_failure(&what, &e))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = err.lines().rev().take(4).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        return Err(anyhow!("{}", tail.join("\n")));
    }
    Ok(out.stdout)
}

/// 跑完一个命令，stdout、stderr 都留着（分类要看全部的 WARNING / ERROR）。
pub struct Ran {
    pub ok: bool,
    pub code: Option<i32>,
    pub stdout: Vec<u8>,
    pub stderr: String,
}

/// 带超时地跑一个命令。启动不了 → 组件问题；超时 → 网络问题（子进程连同它启动的 ffmpeg 一起结束，见 `Proc`）。
pub async fn run_full(mut c: Command, timeout: Duration) -> std::result::Result<Ran, Failure> {
    let what = c.as_std().get_program().to_string_lossy().to_string();
    c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut p = spawn(&mut c).map_err(|e| spawn_failure(&what, &e))?;
    let fut = p.output();
    match tokio::time::timeout(timeout, fut).await {
        Err(_) => Err(Failure::new(ErrorCode::Network, tr!("等了 {n} 秒还没有结果，网络可能不稳定", n = timeout.as_secs()), format!("{what}: timed out after {}s", timeout.as_secs()))),
        Ok(Err(e)) => Err(spawn_failure(&what, &e)),
        Ok(Ok(o)) => Ok(Ran { ok: o.status.success(), code: o.status.code(), stdout: o.stdout, stderr: String::from_utf8_lossy(&o.stderr).to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_runtime_order() {
        let dir = tempfile::tempdir().unwrap();
        // 安装包里的 deno 优先于系统里的
        let deno = dir.path().join(exe_name("deno"));
        std::fs::write(&deno, b"").unwrap();
        let tb = Toolbox { sidecar_dir: Some(dir.path().to_path_buf()), ..Default::default() };
        let r = tb.js_runtime().unwrap();
        assert_eq!((r.kind, r.source, r.path.clone()), ("deno", "bundled", deno.clone()));
        assert_eq!(r.arg(), format!("deno:{}", deno.display()));
        // 设置里指定的最优先，种类看文件名
        let s = Settings { js_runtime_path: "/opt/homebrew/bin/node".into(), ..Default::default() };
        let tb = Toolbox::from_settings(&s, Some(dir.path().to_path_buf()), None);
        let r = tb.js_runtime().unwrap();
        assert_eq!((r.kind, r.source), ("node", "custom"));
        assert_eq!(tb.path(Tool::Deno).unwrap(), PathBuf::from("/opt/homebrew/bin/node"));
        assert_eq!(js_kind(Path::new("/Users/a/.bun/bin/bun")), "bun");
        assert_eq!(js_kind(Path::new("/x/qjs")), "quickjs");
    }

    #[test]
    fn sidecar_dir_on_linux_lib() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("usr/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let exe = bin.join("captae-desktop");
        // 旁边没有 lib/Captae：就是程序所在的目录
        assert_eq!(sidecar_dir_of(&exe), Some(bin.clone()));
        let lib = dir.path().join("usr/lib/Captae");
        std::fs::create_dir_all(&lib).unwrap();
        std::fs::write(lib.join("captae"), b"").unwrap();
        let got = sidecar_dir_of(&exe).unwrap();
        if cfg!(target_os = "linux") {
            assert_eq!(got.canonicalize().unwrap(), lib.canonicalize().unwrap());
        } else {
            assert_eq!(got, bin);
        }
    }

    #[tokio::test]
    async fn ytdlp_gets_js_runtime_and_proxy() {
        let dir = tempfile::tempdir().unwrap();
        // 假的 yt-dlp：把收到的参数和代理环境变量打印出来
        let fake = testkit::fake(dir.path(), "yt-dlp", "echo \"$@\"\necho \"proxy=$https_proxy\"", "Write-Output ($args -join ' ')\nWrite-Output \"proxy=$env:https_proxy\"");
        std::fs::write(dir.path().join(exe_name("deno")), b"").unwrap();
        let s = Settings { ytdlp_path: fake.display().to_string(), proxy_mode: "manual".into(), proxy_url: "127.0.0.1:7890".into(), ..Default::default() };
        let tb = Toolbox::from_settings(&s, Some(dir.path().to_path_buf()), None);
        assert_eq!(tb.ytdlp_source(), "custom");
        let mut c = tb.command(Tool::YtDlp).unwrap();
        c.arg("URL");
        let out = String::from_utf8(c.output().await.unwrap().stdout).unwrap();
        assert!(out.starts_with("--ignore-config --no-js-runtimes --js-runtimes deno:"), "{out}");
        assert!(out.contains("--proxy http://127.0.0.1:7890 --encoding utf-8 URL"), "{out}");
        assert!(out.contains("proxy=http://127.0.0.1:7890"), "{out}");
        // Python 往管道里写 UTF-8（Windows 上不设会按 ANSI 代码页写，全角的文件名对不上）
        let c = tb.command(Tool::YtDlp).unwrap();
        let env = |k: &str| c.as_std().get_envs().find(|(n, _)| *n == OsStr::new(k)).and_then(|(_, v)| v.map(|v| v.to_string_lossy().to_string()));
        assert_eq!((env("PYTHONIOENCODING").as_deref(), env("PYTHONUTF8").as_deref()), (Some("utf-8"), Some("1")));
        assert!(out.contains("--encoding utf-8"), "{out}");
    }

    /// 安装包里的 aria2c 启动时带上 OPENSSL_MODULES（指向随包带的 OpenSSL 模块）；别的 aria2c 不带。
    /// 只看命令上设了什么，不运行它（运行的话结果会受跑测试的这个进程自己的环境变量影响）
    #[test]
    fn bundled_aria2_gets_openssl_modules() {
        let dir = tempfile::tempdir().unwrap();
        // 和安装包一样的布局：Contents/MacOS/aria2c、Contents/Resources/aria2-libs/ossl-modules/legacy.dylib
        let macos = dir.path().join("Contents/MacOS");
        let modules = dir.path().join("Contents/Resources/aria2-libs/ossl-modules");
        std::fs::create_dir_all(&macos).unwrap();
        std::fs::create_dir_all(&modules).unwrap();
        let aria2 = macos.join(exe_name("aria2c"));
        std::fs::write(&aria2, b"").unwrap();
        // 不用代理：免得测试去读这台电脑的系统代理设置
        let tb = Toolbox { sidecar_dir: Some(macos.clone()), proxy_mode: "off".into(), ..Default::default() };
        // 命令上设的 OPENSSL_MODULES（没设就是 None）
        let modules_of = |c: &Command| {
            c.as_std().get_envs().find(|(k, _)| *k == OsStr::new("OPENSSL_MODULES")).and_then(|(_, v)| v.map(PathBuf::from))
        };
        let same = |a: &Path, b: &Path| a.canonicalize().unwrap() == b.canonicalize().unwrap();

        // 模块不在：不带
        assert!(modules_of(&tb.command(Tool::Aria2).unwrap()).is_none());
        assert!(aria2_modules(&aria2).is_none());
        // 模块在：带上，指的就是那个目录
        std::fs::write(modules.join("legacy.dylib"), b"").unwrap();
        let got = modules_of(&tb.command(Tool::Aria2).unwrap()).expect("应该带上 OPENSSL_MODULES");
        assert!(same(&got, &modules), "{}", got.display());
        assert!(same(&aria2_modules(&aria2).unwrap(), &modules));
        // 根证书：和 ffmpeg 一样用 macOS 自带的（附带的 aria2c 默认读 Homebrew 的 cert.pem）
        if cfg!(target_os = "macos") && std::env::var_os("SSL_CERT_FILE").is_none() && Path::new("/etc/ssl/cert.pem").is_file() {
            let c = tb.command(Tool::Aria2).unwrap();
            let ca = c.as_std().get_envs().find(|(k, _)| *k == OsStr::new("SSL_CERT_FILE")).and_then(|(_, v)| v);
            assert_eq!(ca, Some(OsStr::new("/etc/ssl/cert.pem")));
        }
        // 别的工具不带模块目录
        std::fs::write(macos.join(exe_name("ffmpeg")), b"").unwrap();
        assert!(modules_of(&tb.command(Tool::Ffmpeg).unwrap()).is_none());
        // Homebrew 那种布局（旁边没有 Resources/aria2-libs）：不带
        assert!(aria2_modules(Path::new("/opt/homebrew/bin/aria2c")).is_none());
    }

    /// 超时以后，脚本自己起的子进程（相当于 yt-dlp 起的 ffmpeg）也要结束（第三阶段 FIXES 第 15 项）
    #[tokio::test]
    async fn timeout_kills_grandchildren() {
        let dir = tempfile::tempdir().unwrap();
        let pidf = dir.path().join("pids");
        let fake = testkit::fake(dir.path(), "fake-ytdlp", &testkit::sh_wait_child(&pidf), &testkit::ps_wait_child(&pidf));
        let pidf2 = pidf.clone();
        let watch = tokio::spawn(async move { testkit::read_pids(&pidf2).await });
        // 超时给 2.5 秒：一起跑的测试多（有几个在用 ffmpeg 编码）时，脚本 800ms 内可能还没起子进程就被结束了（2026-11-24 偶发过一次）。
        // Windows 上假程序是 PowerShell，起来要一两秒，多给一些
        let wait = if cfg!(windows) { 8000 } else { 2500 };
        let f = run_full(Command::new(&fake), Duration::from_millis(wait)).await.err().unwrap();
        assert_eq!(f.code, ErrorCode::Network);
        for p in watch.await.unwrap() {
            assert!(testkit::gone(p).await, "超时以后进程 {p} 还在跑");
        }
    }

    /// 暂停、取消：整个 future 被丢掉，Proc 跟着丢掉，孙进程也要结束
    #[tokio::test]
    async fn dropped_proc_kills_grandchildren() {
        let dir = tempfile::tempdir().unwrap();
        let pidf = dir.path().join("pids");
        let fake = testkit::fake(dir.path(), "fake-ytdlp", &testkit::sh_wait_child(&pidf), &testkit::ps_wait_child(&pidf));
        let mut c = Command::new(&fake);
        c.kill_on_drop(true).stdout(std::process::Stdio::piped());
        let p = spawn(&mut c).unwrap();
        let root = p.id().unwrap();
        let pids = testkit::read_pids(&pidf).await;
        // 能列出直接子进程。unix：脚本（root）起了 sleep；Windows：cmd（root）起了 PowerShell，PowerShell 起了 ping
        #[cfg(unix)]
        assert_eq!(sys::children(root as i32), vec![pids[1] as i32], "能列出直接子进程");
        #[cfg(windows)]
        assert!(win::children(root).contains(&pids[0]) && win::children(pids[0]).contains(&pids[1]), "能列出直接子进程");
        drop(p);
        assert!(testkit::gone(root).await, "丢掉以后 {root} 还在");
        for p in pids {
            assert!(testkit::gone(p).await, "丢掉以后 {p} 还在");
        }
        // 正常结束的不受影响
        let mut c = if cfg!(windows) { Command::new("cmd") } else { Command::new("/bin/echo") };
        if cfg!(windows) {
            c.args(["/c", "echo", "hi"]);
        } else {
            c.arg("hi");
        }
        c.stdout(std::process::Stdio::piped());
        let mut p = spawn(&mut c).unwrap();
        let o = p.output().await.unwrap();
        assert_eq!(String::from_utf8_lossy(&o.stdout).trim(), "hi");
    }

    /// 手机的 Runner：Android 在原生库目录里找 lib*.so（设置里指定的优先），iOS 一个都没有、报 Missing（归成 tool）；
    /// 桌面端的照旧说什么都有（找不到时在启动那一刻报错）
    #[test]
    fn runners() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("libffmpeg.so"), b"").unwrap();
        std::fs::write(dir.path().join("libqjs.so"), b"").unwrap();
        let tb = Toolbox::default().with_runner(std::sync::Arc::new(AndroidRunner::new(dir.path())));
        assert_eq!(tb.path(Tool::Ffmpeg).unwrap(), dir.path().join("libffmpeg.so"));
        assert!(tb.has(Tool::Ffmpeg) && !tb.has(Tool::Aria2) && !tb.has(Tool::YtDlp));
        let e = tb.path(Tool::Aria2).unwrap_err();
        assert_eq!(crate::errors::from_anyhow(&e, "").code, ErrorCode::Tool);
        // JS 运行时是原生库目录里的 QuickJS
        let js = tb.js_runtime().unwrap();
        assert_eq!((js.kind, js.path.clone()), ("quickjs", dir.path().join("libqjs.so")));
        // 启动时带上 LD_LIBRARY_PATH（ffmpeg 的动态库在原生库目录里）
        let c = tb.command(Tool::Ffmpeg).unwrap();
        let ld = c.as_std().get_envs().find(|(k, _)| *k == OsStr::new("LD_LIBRARY_PATH")).and_then(|(_, v)| v.map(PathBuf::from));
        assert_eq!(ld, Some(dir.path().to_path_buf()));
        // 设置里指定的优先
        let s = Settings { ffmpeg_path: "/custom/ffmpeg".into(), ..Default::default() };
        let tb = Toolbox::from_settings(&s, None, None).with_runner(std::sync::Arc::new(AndroidRunner::new(dir.path())));
        assert_eq!(tb.path(Tool::Ffmpeg).unwrap(), PathBuf::from("/custom/ffmpeg"));
        // iOS：什么都没有
        let tb = Toolbox::default().with_runner(std::sync::Arc::new(InProcessRunner));
        assert!(!tb.has(Tool::Ffmpeg) && !tb.runner.spawns());
        let e = tb.command(Tool::YtDlp).unwrap_err();
        assert_eq!(crate::errors::from_anyhow(&e, "").code, ErrorCode::Tool);
        assert!(tb.js_runtime().is_none());
        // 桌面端：默认就是它，一律说有
        let tb = Toolbox::default();
        assert_eq!(tb.runner.kind(), "desktop");
        assert!(tb.has(Tool::Aria2));
    }

    /// Android 的 yt-dlp：平时是原生库目录里的启动器；自动更新装了新版（zipapp）以后用新版，由启动器跑（CAPTAE_YTDLP）。
    /// 桌面端的起法不变（程序本身，不加环境变量）
    #[test]
    fn android_ytdlp_launcher() {
        let lib = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let launcher = lib.path().join("libytdlp.so");
        let tb = Toolbox { data_dir: Some(data.path().to_path_buf()), ..Default::default() }.with_runner(std::sync::Arc::new(AndroidRunner::new(lib.path())));
        assert!(!tb.has(Tool::YtDlp) && tb.bundled_ytdlp().is_none());
        std::fs::write(&launcher, b"").unwrap();
        let envs = |c: &Command| c.as_std().get_envs().filter_map(|(k, v)| Some((k.to_string_lossy().to_string(), v?.to_string_lossy().to_string()))).collect::<Vec<_>>();
        let ytdlp_env = |c: &Command| envs(c).into_iter().find(|(k, _)| k == "CAPTAE_YTDLP").map(|(_, v)| v);
        // 随 App 带的：启动器本身
        assert_eq!((tb.path(Tool::YtDlp).unwrap(), tb.ytdlp_source()), (launcher.clone(), "bundled"));
        assert_eq!(tb.base_ytdlp(), Some(launcher.clone()));
        let c = tb.command(Tool::YtDlp).unwrap();
        assert_eq!((PathBuf::from(c.as_std().get_program()), ytdlp_env(&c)), (launcher.clone(), None));
        // 装了新版
        let d = crate::updater::engine_dir(data.path()).join("2026.11.23");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("yt-dlp"), b"PK").unwrap();
        std::fs::write(crate::updater::engine_dir(data.path()).join("state.json"), br#"{"active":"2026.11.23"}"#).unwrap();
        let zipapp = d.join("yt-dlp");
        assert_eq!((tb.path(Tool::YtDlp).unwrap(), tb.ytdlp_source()), (zipapp.clone(), "managed"));
        let c = tb.command(Tool::YtDlp).unwrap();
        assert_eq!(PathBuf::from(c.as_std().get_program()), launcher);
        assert_eq!(ytdlp_env(&c), Some(zipapp.display().to_string()));
        assert!(c.as_std().get_args().any(|a| a == "--ignore-config"));
        // 自检用的 ytdlp_at：同样由启动器跑，带 LD_LIBRARY_PATH
        let c = tb.ytdlp_at(&zipapp);
        assert_eq!(PathBuf::from(c.as_std().get_program()), launcher);
        assert!(envs(&c).iter().any(|(k, _)| k == "LD_LIBRARY_PATH"));
        // 桌面端：ytdlp_at 就是程序本身，不加环境变量
        let c = Toolbox::default().ytdlp_at(Path::new("/x/yt-dlp_macos"));
        assert_eq!(PathBuf::from(c.as_std().get_program()), PathBuf::from("/x/yt-dlp_macos"));
        assert!(ytdlp_env(&c).is_none() && !envs(&c).iter().any(|(k, _)| k == "LD_LIBRARY_PATH"));
    }

    /// `alive`：自己在；跑完的子进程不在；明显不存在的进程号不在
    #[tokio::test]
    async fn alive_checks() {
        assert!(alive(std::process::id()));
        let mut c = if cfg!(windows) { Command::new("cmd") } else { Command::new("/bin/sh") };
        c.args(if cfg!(windows) { ["/c", "exit 0"] } else { ["-c", "exit 0"] });
        let mut child = c.spawn().unwrap();
        let pid = child.id().unwrap();
        child.wait().await.unwrap();
        // unix：wait 过就被收走了；Windows：句柄还在时进程对象还在，但退出码不是 STILL_ACTIVE
        assert!(!alive(pid), "{pid} 已经结束了");
        assert!(!alive(0));
    }

    #[tokio::test]
    async fn spawn_errors_are_tool_failures() {
        let c = Command::new("/nonexistent/captae-test-tool");
        let e = run_ok(c).await.unwrap_err();
        let f = crate::errors::from_anyhow(&e, "");
        assert_eq!(f.code, ErrorCode::Tool);
        let c = Command::new("/nonexistent/captae-test-tool");
        assert_eq!(run_full(c, Duration::from_secs(5)).await.err().unwrap().code, ErrorCode::Tool);
        let mut c = if cfg!(windows) { Command::new("ping") } else { Command::new("/bin/sleep") };
        if cfg!(windows) {
            c.args(["-n", "6", "127.0.0.1"]);
        } else {
            c.arg("5");
        }
        c.kill_on_drop(true);
        let f = run_full(c, Duration::from_millis(200)).await.err().unwrap();
        assert_eq!(f.code, ErrorCode::Network);
    }

    #[test]
    fn ffprobe_follows_ffmpeg() {
        let probe = |ff: &str| Toolbox::from_settings(&Settings { ffmpeg_path: ff.into(), ..Default::default() }, None, None).override_of("ffprobe").unwrap();
        assert_eq!(probe("/opt/homebrew/bin/ffmpeg"), Path::new("/opt/homebrew/bin/ffprobe"));
        assert_eq!(probe("/x/FFmpeg.exe"), Path::new("/x/ffprobe.exe"));
        assert_eq!(probe("/x/ffmpeg-9"), Path::new("/x/ffprobe-9"));
        assert_eq!(probe("/x/fake.cmd"), Path::new("/x").join(exe_name("ffprobe")));
    }

    /// 自动更新装的 yt-dlp 比安装包自带的旧（Captae 更新带来了更新的 yt-dlp、yt-dlp 的自动更新关着）时用自带的
    #[test]
    fn newer_bundled_ytdlp_wins() {
        let dir = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let bundled = bundled_ytdlp_candidates(&bin).remove(0);
        std::fs::create_dir_all(bundled.parent().unwrap()).unwrap();
        std::fs::write(&bundled, b"").unwrap();
        let tb = Toolbox { sidecar_dir: Some(bin.clone()), data_dir: Some(data.path().to_path_buf()), ..Default::default() };
        // 自动更新装了 2026.11.23
        let f = crate::updater::flavor_of(tb.runner.kind());
        let managed = crate::updater::exe_as(&crate::updater::engine_dir(data.path()).join("2026.11.23"), f);
        std::fs::create_dir_all(managed.parent().unwrap()).unwrap();
        std::fs::write(&managed, b"").unwrap();
        std::fs::write(crate::updater::engine_dir(data.path()).join("state.json"), br#"{"active":"2026.11.23"}"#).unwrap();
        // 自带的没写版本（老的安装包）：照旧用自动更新装的
        assert_eq!((tb.path(Tool::YtDlp).unwrap(), tb.ytdlp_source()), (managed.clone(), "managed"));
        // 自带的更旧：还是自动更新装的
        std::fs::write(bundled.parent().unwrap().join("version.txt"), "2026.10.26\n").unwrap();
        assert_eq!(bundled_ytdlp_version(&bundled).as_deref(), Some("2026.10.26"));
        assert_eq!(tb.ytdlp_source(), "managed");
        // 自带的更新：用自带的
        std::fs::write(bundled.parent().unwrap().join("version.txt"), "2026.12.24\n").unwrap();
        assert_eq!((tb.path(Tool::YtDlp).unwrap(), tb.ytdlp_source()), (bundled.clone(), "bundled"));
        // 版本号乱写的不认
        std::fs::write(bundled.parent().unwrap().join("version.txt"), "../x").unwrap();
        assert_eq!(bundled_ytdlp_version(&bundled), None);
    }

    #[test]
    fn bundled_ytdlp_layout() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let tb = Toolbox { sidecar_dir: Some(bin.clone()), ..Default::default() };
        assert!(tb.bundled_ytdlp().is_none());
        // 各平台安装包里官方目录版的位置
        let want = if cfg!(target_os = "macos") {
            dir.path().join("Resources/yt-dlp/yt-dlp_macos")
        } else if cfg!(windows) {
            bin.join("yt-dlp/yt-dlp.exe")
        } else {
            dir.path().join(if cfg!(target_arch = "aarch64") { "lib/Captae/yt-dlp/yt-dlp_linux_aarch64" } else { "lib/Captae/yt-dlp/yt-dlp_linux" })
        };
        std::fs::create_dir_all(want.parent().unwrap()).unwrap();
        std::fs::write(&want, b"").unwrap();
        let got = tb.bundled_ytdlp().unwrap();
        assert_eq!(got.canonicalize().unwrap(), want.canonicalize().unwrap());
        assert_eq!(tb.ytdlp_source(), "bundled");
    }

    #[test]
    fn ca_file_for_children() {
        let mut c = Command::new("x");
        ca_bundle(&mut c);
        let set = c.as_std().get_envs().find(|(k, _)| *k == OsStr::new("SSL_CERT_FILE")).and_then(|(_, v)| v.map(PathBuf::from));
        if std::env::var_os("SSL_CERT_FILE").is_some() || cfg!(windows) {
            assert!(set.is_none(), "Windows 不设、用户自己设了的不动");
        } else if let Some(ca) = system_ca_file() {
            assert_eq!(set.as_deref(), Some(ca));
        }
        if cfg!(target_os = "linux") {
            assert!(system_ca_file().is_some(), "Linux 上应该找得到系统证书包");
        }
    }

    /// Windows：「停止并保存」往工具的控制台发 Ctrl-C（经过 captae __ctrl-c 辅助进程），工具自己收尾。
    /// 要先编出 captae.exe（`cargo build -p captae-cli`），没有就跳过
    #[cfg(windows)]
    #[tokio::test]
    async fn interrupt_lets_tool_finish() {
        if ctrl_c_helper().is_none() {
            eprintln!("跳过：没有 captae.exe（先 cargo build -p captae-cli）");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let (ready, saved) = (dir.path().join("ready"), dir.path().join("saved"));
        let q = |p: &Path| p.display().to_string().replace('\'', "''");
        // 收到 Ctrl-C 时 PowerShell 停下脚本、跑 finally（相当于 yt-dlp 的 KeyboardInterrupt 收尾）
        let ps = format!("[IO.File]::WriteAllText('{}', 'x')\ntry {{ while ($true) {{ Start-Sleep -Milliseconds 100 }} }} finally {{ [IO.File]::WriteAllText('{}', 'saved') }}", q(&ready), q(&saved));
        let fake = testkit::fake(dir.path(), "fake-live", "", &ps);
        let mut c = Command::new(&fake);
        no_window(&mut c);
        c.stdin(std::process::Stdio::null()).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null());
        let mut p = spawn(&mut c).unwrap();
        let t0 = std::time::Instant::now();
        while !ready.exists() {
            assert!(t0.elapsed() < Duration::from_secs(20), "假程序没有起来");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        assert!(interrupt(p.id().unwrap()), "Ctrl-C 要送得到");
        let t0 = std::time::Instant::now();
        while !saved.exists() {
            assert!(t0.elapsed() < Duration::from_secs(15), "收到 Ctrl-C 以后没有收尾");
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let _ = tokio::time::timeout(Duration::from_secs(10), p.wait()).await;
    }
}

/// 测试用的假外部程序：unix 是 sh 脚本，Windows 是一个 .cmd 调 PowerShell 跑同名的 .ps1
/// （Captae 启动 .cmd 会经过 cmd.exe，所以 Windows 上的进程树是 cmd → powershell → 它起的程序）
#[cfg(test)]
pub(crate) mod testkit {
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    /// 写一个假程序，返回它的路径。`sh` 是 sh 脚本正文（不含第一行 #!），`ps` 是 PowerShell 正文（参数在 `$args` 里）
    pub fn fake(dir: &Path, name: &str, sh: &str, ps: &str) -> PathBuf {
        if cfg!(windows) {
            let ps1 = dir.join(format!("{name}.ps1"));
            // 带 BOM 的 UTF-8：Windows PowerShell 5 才不会把中文路径读成乱码
            let mut body = vec![0xEF, 0xBB, 0xBF];
            body.extend_from_slice(ps.as_bytes());
            std::fs::write(&ps1, body).unwrap();
            let cmd = dir.join(format!("{name}.cmd"));
            std::fs::write(&cmd, "@echo off\r\npowershell.exe -NoLogo -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"%~dpn0.ps1\" %*\r\nexit /b %ERRORLEVEL%\r\n").unwrap();
            cmd
        } else {
            let p = dir.join(name);
            std::fs::write(&p, format!("#!/bin/sh\n{sh}\n")).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
            p
        }
    }

    fn ps_quote(p: &Path) -> String {
        p.display().to_string().replace('\'', "''")
    }

    /// sh：起一个一直等的子进程，把「脚本自己 子进程」两个进程号写进 `pidf`，然后等它
    pub fn sh_wait_child(pidf: &Path) -> String {
        format!("sleep 300 &\necho \"$$ $!\" > '{}'\nwait", pidf.display())
    }

    /// PowerShell：起一个一直等的子进程（ping 300 秒），把「PowerShell 自己 子进程」两个进程号写进 `pidf`，然后等它
    pub fn ps_wait_child(pidf: &Path) -> String {
        format!(
            "$si = New-Object Diagnostics.ProcessStartInfo 'ping.exe', '-n 300 127.0.0.1'\n$si.UseShellExecute = $false\n$si.RedirectStandardOutput = $true\n$si.CreateNoWindow = $true\n$c = [Diagnostics.Process]::Start($si)\n[IO.File]::WriteAllText('{}', \"$PID $($c.Id)\")\n$c.WaitForExit()",
            ps_quote(pidf)
        )
    }

    /// 读 `sh_wait_child` / `ps_wait_child` 写的进程号（最多等 10 秒）
    pub async fn read_pids(f: &Path) -> Vec<u32> {
        for _ in 0..500 {
            if let Some(v) = std::fs::read_to_string(f).ok().map(|s| s.split_whitespace().filter_map(|x| x.parse().ok()).collect::<Vec<u32>>()).filter(|v| v.len() == 2) {
                return v;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        panic!("子进程没有起来");
    }

    /// 等这个进程结束（最多 3 秒）。unix 上结束的孙进程要等 launchd / init 收走
    pub async fn gone(pid: u32) -> bool {
        for _ in 0..60 {
            if !super::test_alive(pid) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }
}
