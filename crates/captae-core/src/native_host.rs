//! 浏览器插件的 Native Messaging 宿主。
//!
//! - 注册：给装了的 Chromium 系浏览器写宿主清单（Chrome 及 Beta/Dev/Canary、Chrome for Testing、Chromium、
//!   Edge 及 Beta/Dev/Canary、Brave、Vivaldi、Opera、Opera GX、Arc、Aside；Windows 另写注册表），
//!   清单指向数据目录里的一个小脚本，脚本再执行 `captae native-host`。
//!   桌面端启动时检查一次，清单缺了或指向别处就重写；命令行 `captae install-native-host` 也走这里。
//! - 运行：浏览器启动 `captae native-host`，消息是 4 字节长度（本机字节序）加 JSON。
//!   每条消息单独处理，「发给 AI」这种要跑一会儿的请求不会挡住进度查询。
//! - 认浏览器：宿主启动时看父进程是哪个程序（macOS：`proc_pidpath`），对上面那张 16 家的表，
//!   插件打招呼时把认出来的告诉桌面端（插件自己只认得几家，Arc、Aside 这些会说成 Chrome）。
//!
//! 不支持 Firefox（产品决定）。

use crate::api::{Client, FocusTarget};
use crate::config::{Paths, Settings};
use crate::library::Library;
use crate::model::{AddRequest, JobAction};
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const HOST_NAME: &str = "app.captae.bridge";
/// 插件 ID，由 manifest.json 里的 `key` 固定。开发时加载的未打包插件 ID 不同，可以用 --extension-id 覆盖。
pub const CHROME_EXTENSION_ID: &str = "docjnhebpmdchlahmhkjladmehlcfkcp";

// ------------------------------------------------------------------ 注册

#[derive(Debug, Clone)]
pub struct Install {
    /// `captae` 命令行程序
    pub exe: PathBuf,
    /// 数据目录，包装脚本（Windows：宿主清单）放这里
    pub data: PathBuf,
    /// 用户主目录，浏览器的配置都在它下面
    pub home: PathBuf,
    pub ext_id: Option<String>,
    /// true：所有浏览器都写；false：只写装了的（一个都没装就写 Chrome）
    pub all: bool,
    /// 便携模式（CAPTAE_HOME）：宿主也要用同一个数据目录，写进包装脚本
    pub env_home: Option<PathBuf>,
    /// 找浏览器 .app 的目录（判断装没装），默认 /Applications 和 ~/Applications（只在 macOS 上用）
    pub app_dirs: Vec<PathBuf>,
    /// Windows：注册表键写在 HKCU 下的哪里。None 就是 HKCU 本身（浏览器读的位置）；测试时换成 `Software\CaptaeTest\…`，不碰真的设置
    pub reg_prefix: Option<String>,
}

impl Install {
    pub fn new(exe: PathBuf, data: PathBuf) -> Install {
        let home = home_dir();
        let app_dirs = if cfg!(target_os = "macos") { vec![PathBuf::from("/Applications"), home.join("Applications")] } else { vec![] };
        Install { exe, data, home, ext_id: None, all: false, env_home: None, app_dirs, reg_prefix: None }
    }
    fn ext_id(&self) -> &str {
        self.ext_id.as_deref().unwrap_or(CHROME_EXTENSION_ID)
    }
    /// Windows 上清单直接指向 captae.exe（浏览器把 `chrome-extension://…/` 当第一个参数传进来，命令行据此当成 native-host），
    /// 不用 .bat：.bat 要经过 cmd.exe，中文路径、特殊字符容易出事。只有便携模式（要带 CAPTAE_HOME）才包一层 .bat
    fn direct(&self) -> bool {
        cfg!(windows) && self.env_home.is_none()
    }
    /// 清单里 `path` 指向的程序：包装脚本，或者（Windows 非便携模式）captae.exe 本身
    pub fn wrapper(&self) -> PathBuf {
        if self.direct() {
            return self.exe.clone();
        }
        self.data.join(if cfg!(windows) { "captae-native-host.bat" } else { "captae-native-host" })
    }
    /// Windows：宿主清单只写一份，放在数据目录，注册表指向它
    pub fn win_manifest(&self) -> PathBuf {
        self.data.join(format!("{HOST_NAME}.json"))
    }
    /// 一个浏览器要写的清单文件（Windows 是数据目录里那一份）
    fn manifests_of(&self, b: &Browser) -> Vec<PathBuf> {
        if cfg!(windows) {
            vec![self.win_manifest()]
        } else {
            b.manifests()
        }
    }
    /// Flatpak 版浏览器的清单指向沙盒里看得到的包装脚本（见 `Sandbox::Flatpak`）
    fn wrapper_of(&self, b: &Browser) -> PathBuf {
        match &b.sandbox {
            Some(Sandbox::Flatpak(_)) => b.dirs[0].join("captae-native-host"),
            _ => self.wrapper(),
        }
    }
}

pub fn home_dir() -> PathBuf {
    crate::util::home_dir()
}

/// 浏览器装在沙盒里（Linux）
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", content = "id", rename_all = "lowercase")]
pub enum Sandbox {
    /// Flatpak 版（应用 id，如 com.google.Chrome）。宿主在沙盒里启动，看不到也不能运行装在外面的 captae：
    /// 清单指向放在沙盒能看到的目录（`~/.var/app/<id>/…`）里的包装脚本，脚本用 `flatpak-spawn --host` 到沙盒外运行 captae。
    /// 这要求浏览器有 `org.freedesktop.Flatpak` 的通话权限，Flathub 上的浏览器默认没有，要用户自己开一次：
    /// `flatpak override --user --talk-name=org.freedesktop.Flatpak <id>`（KeePassXC、1Password 等也是这样要求的）。
    /// Captae 不替用户开：这是放宽浏览器沙盒的安全设置
    Flatpak(&'static str),
    /// Snap 版 Chromium：清单放在 `~/snap/chromium/common/chromium/NativeMessagingHosts`。严格沙盒不许运行装在外面的程序，
    /// 多半连不上；照样写清单（以后 Snap 放开了就能用），状态里说明
    Snap,
}

#[derive(Debug, Clone, Serialize)]
pub struct Browser {
    /// chrome / chrome-beta / edge / brave / vivaldi / opera / arc / aside / chrome-flatpak …
    pub id: &'static str,
    pub name: &'static str,
    /// 浏览器的默认用户数据目录
    pub root: PathBuf,
    /// 宿主清单要写的目录（macOS、Linux）。第一个是这个浏览器自己的；Opera 还读 Chrome 的目录，所以两处都写。Windows 是空的（清单在数据目录）
    pub dirs: Vec<PathBuf>,
    /// 应用程序包的名字（macOS），用来判断装没装
    pub app: &'static str,
    /// Windows：这个浏览器去 HKCU 下哪些键找宿主（不含宿主名，按它查找的先后）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<&'static str>,
    /// Windows、Linux：浏览器主程序可能在的位置（判断装没装；桌面端用它打开扩展程序页面，见 `exe()`）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exes: Vec<PathBuf>,
    /// Windows、Linux：认父进程用的路径规则（不分大小写，从后往前比，`*` 匹配一段里的任意字符），见 `browser_of_path`
    #[serde(skip)]
    pub marks: Vec<&'static str>,
    /// 装在沙盒里（Linux 的 Flatpak、Snap）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<Sandbox>,
}

impl Browser {
    pub fn manifest(&self) -> PathBuf {
        self.dirs.first().map(|d| d.join(format!("{HOST_NAME}.json"))).unwrap_or_default()
    }
    pub fn manifests(&self) -> Vec<PathBuf> {
        self.dirs.iter().map(|d| d.join(format!("{HOST_NAME}.json"))).collect()
    }
    /// 装了：浏览器运行过（用户数据目录里有 `Local State`），或者能找到它的 .app / 主程序（Flatpak、Snap 看应用装没装）。
    /// 只看目录在不在不准：1Password 这类软件会替没装的浏览器建好 NativeMessagingHosts 目录。
    pub fn installed(&self, app_dirs: &[PathBuf]) -> bool {
        self.root.join("Local State").is_file() || (!self.app.is_empty() && app_dirs.iter().any(|d| d.join(self.app).exists())) || self.exes.iter().any(|p| p.is_file())
    }
    /// 浏览器主程序（Windows、Linux；找不到时 None），桌面端用它打开扩展程序页面。
    /// Flatpak 版没有能直接运行的主程序（`exes` 里放的是判断装没装用的应用目录），是 None：用 `flatpak run <id>` 启动
    pub fn exe(&self) -> Option<PathBuf> {
        if matches!(self.sandbox, Some(Sandbox::Flatpak(_))) {
            return None;
        }
        self.exes.iter().find(|p| p.is_file()).cloned()
    }
}

/// Linux：Flatpak 应用装没装（用户自己装的、系统装的两个位置）
#[cfg(all(unix, not(target_os = "macos")))]
fn flatpak_installed(home: &Path, id: &str) -> Vec<PathBuf> {
    vec![home.join(".local/share/flatpak/app").join(id).join("current/active/metadata"), PathBuf::from("/var/lib/flatpak/app").join(id).join("current/active/metadata")]
}

/// 各浏览器读宿主清单的位置。
///
/// macOS（Chromium 在 macOS 上的规则：默认用户数据目录 + NativeMessagingHosts）：
/// - Arc 的默认用户数据目录是 `Arc/User Data`（1Password、passvalet 也写在这里，2026-11-23 核实）
/// - Opera 读 Chrome 的目录（Opera 论坛里官方的答复）；Anthropic、OpenAI 的插件写在 `com.operasoftware.Opera`，两处都写
/// - Chrome for Testing 读 Chromium 的目录（chromium-extensions 邮件组 2025-08 的答复），也写它自己的目录
/// - Aside：`Aside/NativeMessagingHosts`
///
/// Windows（清单放在 Captae 的数据目录，HKCU 下的注册表键默认值指向它。2026-11-24 核对 Chromium 源码
/// `chrome/browser/extensions/api/messaging/launch_context_win.cc` 和微软文档）：
/// - Chrome 各渠道（正式版、Beta、Dev、Canary）都只读 `Software\Google\Chrome\NativeMessagingHosts`（写死在源码里，不分渠道）
/// - 非 Google 品牌的 Chromium 构建（Chromium、Brave、Vivaldi、Opera、Arc）先读 `Software\Chromium\NativeMessagingHosts`，
///   没有再读 Chrome 的；Brave 没有改这段（brave-core 里没有这个文件的补丁，Brave 社区 2022 年的帖子也说它不读别家的键之外的位置）
/// - Edge 先读 `Software\Microsoft\Edge\NativeMessagingHosts`，再读 Chromium、Chrome 的（微软文档 Native messaging 一节）
/// - 宿主是 .exe 时浏览器直接启动它（不经过 cmd.exe，crbug.com/41084583），参数是 `chrome-extension://<id>/` 和 `--parent-window=<句柄>`
///
/// Linux（`~/.config/<浏览器>/NativeMessagingHosts`）：
/// - Chrome Dev 的目录叫 google-chrome-unstable；Edge Beta / Dev 是 microsoft-edge-beta / -dev
/// - Brave 的源码（brave_main_delegate.cc）把用户级宿主目录指到「默认用户数据目录」下，Opera 读 Chrome 的目录：两处都写
/// - Flatpak、Snap 版见 `Sandbox`
pub fn browsers(home: &Path) -> Vec<Browser> {
    #[cfg(target_os = "macos")]
    {
        let s = home.join("Library/Application Support");
        let nmh = |p: &str| s.join(p).join("NativeMessagingHosts");
        let mk = |id, name, root: &str, app, also: &[&str]| {
            let mut dirs = vec![nmh(root)];
            dirs.extend(also.iter().map(|a| nmh(a)));
            Browser { id, name, root: s.join(root), dirs, app, keys: vec![], exes: vec![], marks: vec![], sandbox: None }
        };
        vec![
            mk("chrome", "Chrome", "Google/Chrome", "Google Chrome.app", &[]),
            mk("chrome-beta", "Chrome Beta", "Google/Chrome Beta", "Google Chrome Beta.app", &[]),
            mk("chrome-dev", "Chrome Dev", "Google/Chrome Dev", "Google Chrome Dev.app", &[]),
            mk("chrome-canary", "Chrome Canary", "Google/Chrome Canary", "Google Chrome Canary.app", &[]),
            mk("chrome-for-testing", "Chrome for Testing", "Google/Chrome for Testing", "Google Chrome for Testing.app", &["Chromium"]),
            mk("chromium", "Chromium", "Chromium", "Chromium.app", &[]),
            mk("edge", "Edge", "Microsoft Edge", "Microsoft Edge.app", &[]),
            mk("edge-beta", "Edge Beta", "Microsoft Edge Beta", "Microsoft Edge Beta.app", &[]),
            mk("edge-dev", "Edge Dev", "Microsoft Edge Dev", "Microsoft Edge Dev.app", &[]),
            mk("edge-canary", "Edge Canary", "Microsoft Edge Canary", "Microsoft Edge Canary.app", &[]),
            mk("brave", "Brave", "BraveSoftware/Brave-Browser", "Brave Browser.app", &[]),
            mk("vivaldi", "Vivaldi", "Vivaldi", "Vivaldi.app", &[]),
            mk("opera", "Opera", "com.operasoftware.Opera", "Opera.app", &["Google/Chrome"]),
            mk("opera-gx", "Opera GX", "com.operasoftware.OperaGX", "Opera GX.app", &["Google/Chrome"]),
            mk("arc", "Arc", "Arc/User Data", "Arc.app", &[]),
            mk("aside", "Aside", "Aside", "Aside.app", &[]),
        ]
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let c = home.join(".config");
        let nmh = |p: &Path| p.join("NativeMessagingHosts");
        let mk = |id, name, root: PathBuf, also: &[&str], exes: &[&str], marks: Vec<&'static str>| {
            let mut dirs = vec![nmh(&root)];
            dirs.extend(also.iter().map(|a| nmh(&c.join(a))));
            Browser { id, name, root, dirs, app: "", keys: vec![], exes: exes.iter().map(PathBuf::from).collect(), marks, sandbox: None }
        };
        // Flatpak 版：配置在 ~/.var/app/<id>/config/<目录> 下；主程序在沙盒里，「装没装」看 Flatpak 的应用目录
        let flat = |id, name, app_id: &'static str, sub: &str| {
            let root = home.join(".var/app").join(app_id).join("config").join(sub);
            Browser { id, name, dirs: vec![nmh(&root)], root, app: "", keys: vec![], exes: flatpak_installed(home, app_id), marks: vec![], sandbox: Some(Sandbox::Flatpak(app_id)) }
        };
        vec![
            mk("chrome", "Chrome", c.join("google-chrome"), &[], &["/opt/google/chrome/chrome"], vec!["/opt/google/chrome/chrome"]),
            mk("chrome-beta", "Chrome Beta", c.join("google-chrome-beta"), &[], &["/opt/google/chrome-beta/chrome"], vec!["/opt/google/chrome-beta/chrome"]),
            mk("chrome-dev", "Chrome Dev", c.join("google-chrome-unstable"), &[], &["/opt/google/chrome-unstable/chrome"], vec!["/opt/google/chrome-unstable/chrome"]),
            mk(
                "chromium",
                "Chromium",
                c.join("chromium"),
                &[],
                &["/usr/lib/chromium/chromium", "/usr/lib/chromium-browser/chromium-browser", "/usr/lib64/chromium-browser/chromium-browser"],
                vec!["/usr/lib/chromium/chromium", "/usr/lib/chromium-browser/chromium-browser", "/usr/lib64/chromium-browser/chromium-browser", "/usr/lib/chromium/chrome"],
            ),
            mk("edge", "Edge", c.join("microsoft-edge"), &[], &["/opt/microsoft/msedge/msedge"], vec!["/opt/microsoft/msedge/msedge"]),
            mk("edge-beta", "Edge Beta", c.join("microsoft-edge-beta"), &[], &["/opt/microsoft/msedge-beta/msedge"], vec!["/opt/microsoft/msedge-beta/msedge"]),
            mk("edge-dev", "Edge Dev", c.join("microsoft-edge-dev"), &[], &["/opt/microsoft/msedge-dev/msedge"], vec!["/opt/microsoft/msedge-dev/msedge"]),
            mk("brave", "Brave", c.join("BraveSoftware/Brave-Browser"), &["google-chrome"], &["/opt/brave.com/brave/brave"], vec!["/opt/brave.com/brave/brave", "/opt/brave.com/brave-beta/brave"]),
            mk("vivaldi", "Vivaldi", c.join("vivaldi"), &[], &["/opt/vivaldi/vivaldi-bin"], vec!["/opt/vivaldi/vivaldi-bin", "/opt/vivaldi-snapshot/vivaldi-bin"]),
            mk(
                "opera",
                "Opera",
                c.join("opera"),
                &["google-chrome"],
                &["/usr/lib/x86_64-linux-gnu/opera/opera", "/usr/lib64/opera/opera"],
                vec!["/usr/lib/x86_64-linux-gnu/opera/opera", "/usr/lib64/opera/opera", "/snap/opera/*/usr/lib/x86_64-linux-gnu/opera/opera"],
            ),
            flat("chrome-flatpak", "Chrome (Flatpak)", "com.google.Chrome", "google-chrome"),
            flat("chromium-flatpak", "Chromium (Flatpak)", "org.chromium.Chromium", "chromium"),
            flat("brave-flatpak", "Brave (Flatpak)", "com.brave.Browser", "BraveSoftware/Brave-Browser"),
            flat("edge-flatpak", "Edge (Flatpak)", "com.microsoft.Edge", "microsoft-edge"),
            {
                let root = home.join("snap/chromium/common/chromium");
                Browser {
                    id: "chromium-snap",
                    name: "Chromium (Snap)",
                    dirs: vec![nmh(&root)],
                    root,
                    app: "",
                    keys: vec![],
                    exes: vec![PathBuf::from("/snap/bin/chromium")],
                    marks: vec!["/snap/chromium/*/usr/lib/chromium-browser/chrome"],
                    sandbox: Some(Sandbox::Snap),
                }
            },
        ]
    }
    #[cfg(windows)]
    {
        const CHROME: &str = r"Software\Google\Chrome\NativeMessagingHosts";
        const CHROMIUM: &str = r"Software\Chromium\NativeMessagingHosts";
        const EDGE: &str = r"Software\Microsoft\Edge\NativeMessagingHosts";
        let local = home.join("AppData").join("Local");
        let roaming = home.join("AppData").join("Roaming");
        let pf: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"].iter().filter_map(std::env::var_os).map(PathBuf::from).chain([PathBuf::from(r"C:\Program Files"), PathBuf::from(r"C:\Program Files (x86)")]).collect();
        // 主程序可能在「Program Files」（给所有用户装的）或 %LOCALAPPDATA%（只给自己装的）下
        let exes = |rel: &str| -> Vec<PathBuf> { pf.iter().chain(std::iter::once(&local)).map(|d| d.join(rel)).collect() };
        let mk = |id, name, root: PathBuf, keys: Vec<&'static str>, exe: &str, marks: Vec<&'static str>| Browser { id, name, root, dirs: vec![], app: "", keys, exes: exes(exe), marks, sandbox: None };
        vec![
            mk("chrome", "Chrome", local.join(r"Google\Chrome\User Data"), vec![CHROME], r"Google\Chrome\Application\chrome.exe", vec!["google/chrome/application/chrome.exe"]),
            mk("chrome-beta", "Chrome Beta", local.join(r"Google\Chrome Beta\User Data"), vec![CHROME], r"Google\Chrome Beta\Application\chrome.exe", vec!["google/chrome beta/application/chrome.exe"]),
            mk("chrome-dev", "Chrome Dev", local.join(r"Google\Chrome Dev\User Data"), vec![CHROME], r"Google\Chrome Dev\Application\chrome.exe", vec!["google/chrome dev/application/chrome.exe"]),
            mk("chrome-canary", "Chrome Canary", local.join(r"Google\Chrome SxS\User Data"), vec![CHROME], r"Google\Chrome SxS\Application\chrome.exe", vec!["google/chrome sxs/application/chrome.exe"]),
            mk("chromium", "Chromium", local.join(r"Chromium\User Data"), vec![CHROMIUM, CHROME], r"Chromium\Application\chrome.exe", vec!["chromium/application/chrome.exe", "chrome-win/chrome.exe"]),
            mk("edge", "Edge", local.join(r"Microsoft\Edge\User Data"), vec![EDGE, CHROMIUM, CHROME], r"Microsoft\Edge\Application\msedge.exe", vec!["microsoft/edge/application/msedge.exe"]),
            mk("edge-beta", "Edge Beta", local.join(r"Microsoft\Edge Beta\User Data"), vec![EDGE, CHROMIUM, CHROME], r"Microsoft\Edge Beta\Application\msedge.exe", vec!["microsoft/edge beta/application/msedge.exe"]),
            mk("edge-dev", "Edge Dev", local.join(r"Microsoft\Edge Dev\User Data"), vec![EDGE, CHROMIUM, CHROME], r"Microsoft\Edge Dev\Application\msedge.exe", vec!["microsoft/edge dev/application/msedge.exe"]),
            mk("edge-canary", "Edge Canary", local.join(r"Microsoft\Edge SxS\User Data"), vec![EDGE, CHROMIUM, CHROME], r"Microsoft\Edge SxS\Application\msedge.exe", vec!["microsoft/edge sxs/application/msedge.exe"]),
            mk("brave", "Brave", local.join(r"BraveSoftware\Brave-Browser\User Data"), vec![CHROMIUM, CHROME], r"BraveSoftware\Brave-Browser\Application\brave.exe", vec!["bravesoftware/brave-browser/application/brave.exe"]),
            mk("vivaldi", "Vivaldi", local.join(r"Vivaldi\User Data"), vec![CHROMIUM, CHROME], r"Vivaldi\Application\vivaldi.exe", vec!["vivaldi/application/vivaldi.exe"]),
            // Opera 的用户数据在 Roaming，主程序在 %LOCALAPPDATA%\Programs\Opera\（launcher.exe，真正的浏览器进程在版本号目录里）
            mk("opera", "Opera", roaming.join(r"Opera Software\Opera Stable"), vec![CHROMIUM, CHROME], r"Programs\Opera\opera.exe", vec!["programs/opera/*/opera.exe", "programs/opera/opera.exe"]),
            mk("opera-gx", "Opera GX", roaming.join(r"Opera Software\Opera GX Stable"), vec![CHROMIUM, CHROME], r"Programs\Opera GX\opera.exe", vec!["programs/opera gx/*/opera.exe", "programs/opera gx/opera.exe"]),
            // Arc for Windows 是 MSIX 包（装在 WindowsApps 里，路径带版本号和发布者哈希），用户数据在包的 LocalCache 里；
            // 打包应用读 HKCU 时看得到真正的注册表，所以照样读 Chromium、Chrome 的键
            Browser {
                id: "arc",
                name: "Arc",
                root: arc_root(&local).unwrap_or_else(|| local.join(r"Packages\TheBrowserCompany.Arc\LocalCache\Local\Arc\User Data")),
                dirs: vec![],
                app: "",
                keys: vec![CHROMIUM, CHROME],
                exes: vec![],
                marks: vec!["windowsapps/thebrowsercompany.arc_*/arc.exe"],
                sandbox: None,
            },
        ]
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = home;
        vec![]
    }
}

/// Arc for Windows 的用户数据目录：`%LOCALAPPDATA%\Packages\TheBrowserCompany.Arc_<发布者哈希>\LocalCache\Local\Arc\User Data`
#[cfg(windows)]
fn arc_root(local: &Path) -> Option<PathBuf> {
    std::fs::read_dir(local.join("Packages"))
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().to_ascii_lowercase().starts_with("thebrowsercompany.arc_"))
        .map(|e| e.path().join(r"LocalCache\Local\Arc\User Data"))
}

pub fn wrapper_script(exe: &Path, env_home: Option<&Path>) -> String {
    if cfg!(windows) {
        let env = env_home.map(|h| format!("set \"CAPTAE_HOME={}\"\r\n", h.display())).unwrap_or_default();
        // chcp 65001：中文路径按 UTF-8 读
        format!("@echo off\r\nchcp 65001 >nul\r\n{env}\"{}\" native-host %*\r\n", exe.display())
    } else {
        let env = env_home.map(|h| format!("CAPTAE_HOME='{}'\nexport CAPTAE_HOME\n", sh_quote_inner(&h.display().to_string()))).unwrap_or_default();
        format!("#!/bin/sh\n{env}exec \"{}\" native-host \"$@\"\n", exe.display())
    }
}

/// 单引号里的内容：`'` 换成 `'\''`
fn sh_quote_inner(s: &str) -> String {
    s.replace('\'', "'\\''")
}

/// Flatpak 版浏览器用的包装脚本：在沙盒里被浏览器启动，用 `flatpak-spawn --host` 到沙盒外运行 captae
pub fn flatpak_wrapper_script(exe: &Path, env_home: Option<&Path>) -> String {
    let env = env_home.map(|h| format!(" '--env=CAPTAE_HOME={}'", sh_quote_inner(&h.display().to_string()))).unwrap_or_default();
    format!("#!/bin/sh\nexec flatpak-spawn --host{env} '{}' native-host \"$@\"\n", sh_quote_inner(&exe.display().to_string()))
}

/// Flatpak 应用有没有和 org.freedesktop.Flatpak 通话的权限（`flatpak-spawn --host` 要它）：看用户级、系统级的 override 文件
#[cfg(all(unix, not(target_os = "macos")))]
pub fn flatpak_host_access(home: &Path, app_id: &str) -> bool {
    [home.join(".local/share/flatpak/overrides"), PathBuf::from("/var/lib/flatpak/overrides")].iter().any(|d| {
        std::fs::read_to_string(d.join(app_id)).is_ok_and(|s| s.lines().any(|l| {
            let l = l.trim();
            l.starts_with("org.freedesktop.Flatpak=") && (l.ends_with("=talk") || l.ends_with("=own"))
        }))
    })
}

pub fn manifest(wrapper: &Path, ext_id: &str) -> Value {
    json!({
        "name": HOST_NAME, "description": "Captae desktop bridge", "path": wrapper, "type": "stdio",
        "allowed_origins": [format!("chrome-extension://{ext_id}/")]
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct BrowserStatus {
    pub id: &'static str,
    pub name: &'static str,
    pub installed: bool,
    /// 所有该写的位置都写好了
    pub registered: bool,
    /// 主位置的清单
    pub manifest: PathBuf,
    /// 另外也写了清单的位置（Opera 还读 Chrome 的目录）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub also: Vec<PathBuf>,
    /// Windows：这个浏览器读的注册表键（HKCU 下）
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub keys: Vec<String>,
    /// 装在沙盒里（Linux 的 Flatpak、Snap）：清单写好了也可能连不上，界面据此给说明
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<Sandbox>,
    /// Flatpak：浏览器有没有到沙盒外运行程序的权限（没有时要用户运行一次 `fix` 里的命令）
    #[serde(rename = "hostAccess", skip_serializing_if = "Option::is_none")]
    pub host_access: Option<bool>,
    /// 用户要自己做的一步（Flatpak：开权限的命令）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    pub exe: PathBuf,
    pub wrapper: PathBuf,
    /// 包装脚本指向当前的 captae
    #[serde(rename = "wrapperOk")]
    pub wrapper_ok: bool,
    pub browsers: Vec<BrowserStatus>,
    /// 该注册的都注册好了
    pub ok: bool,
}

fn manifest_ok(f: &Path, wrapper: &Path, ext_id: &str) -> bool {
    let Ok(v) = std::fs::read(f).map(|b| serde_json::from_slice::<Value>(&b).unwrap_or_default()) else { return false };
    v["name"] == HOST_NAME
        && v["path"].as_str().map(Path::new) == Some(wrapper)
        && v["allowed_origins"].as_array().is_some_and(|a| a.iter().any(|o| o.as_str() == Some(&format!("chrome-extension://{ext_id}/"))))
}

/// 要写清单的浏览器：装了的；一个都没装时写 Chrome，装上 Chrome 就能直接用。
fn targets(o: &Install) -> Vec<Browser> {
    let all = browsers(&o.home);
    if o.all {
        return all;
    }
    let present: Vec<Browser> = all.iter().filter(|b| b.installed(&o.app_dirs)).cloned().collect();
    if present.is_empty() {
        all.into_iter().take(1).collect()
    } else {
        present
    }
}

/// 包装脚本是最新的（Windows 直接指向 captae.exe 时：它在就行）
fn wrapper_ok(o: &Install) -> bool {
    if o.direct() {
        return o.exe.is_file();
    }
    std::fs::read_to_string(o.wrapper()).map(|s| s == wrapper_script(&o.exe, o.env_home.as_deref())).unwrap_or(false)
}

/// 一个浏览器的清单（和 Flatpak 的包装脚本、Windows 的注册表键）都写好了
fn registered(o: &Install, b: &Browser) -> bool {
    let w = o.wrapper_of(b);
    if !o.manifests_of(b).iter().all(|m| manifest_ok(m, &w, o.ext_id())) {
        return false;
    }
    if let Some(Sandbox::Flatpak(_)) = b.sandbox {
        if std::fs::read_to_string(&w).ok().as_deref() != Some(flatpak_wrapper_script(&o.exe, o.env_home.as_deref()).as_str()) {
            return false;
        }
    }
    #[cfg(windows)]
    {
        let want = o.win_manifest();
        if !b.keys.iter().all(|k| reg::read(o, k).is_some_and(|v| Path::new(&v) == want)) {
            return false;
        }
    }
    true
}

pub fn status(o: &Install) -> Status {
    let wrapper = o.wrapper();
    let wrapper_ok = wrapper_ok(o);
    let want: Vec<&'static str> = targets(o).iter().map(|b| b.id).collect();
    let browsers: Vec<BrowserStatus> = browsers(&o.home)
        .into_iter()
        .map(|b| {
            let ms = o.manifests_of(&b);
            #[cfg(all(unix, not(target_os = "macos")))]
            let host_access = match b.sandbox {
                Some(Sandbox::Flatpak(id)) => Some(flatpak_host_access(&o.home, id)),
                _ => None,
            };
            #[cfg(not(all(unix, not(target_os = "macos"))))]
            let host_access: Option<bool> = None;
            let fix = match (&b.sandbox, host_access) {
                (Some(Sandbox::Flatpak(id)), Some(false)) => Some(format!("flatpak override --user --talk-name=org.freedesktop.Flatpak {id}")),
                _ => None,
            };
            BrowserStatus {
                id: b.id,
                name: b.name,
                installed: b.installed(&o.app_dirs),
                registered: registered(o, &b),
                manifest: ms.first().cloned().unwrap_or_default(),
                also: ms.into_iter().skip(1).collect(),
                keys: b.keys.iter().map(|k| format!(r"HKCU\{k}\{HOST_NAME}")).collect(),
                sandbox: b.sandbox.clone(),
                host_access,
                fix,
            }
        })
        .collect();
    let ok = wrapper_ok && browsers.iter().filter(|b| want.contains(&b.id)).all(|b| b.registered);
    Status { exe: o.exe.clone(), wrapper, wrapper_ok, browsers, ok }
}

/// 写包装脚本和各浏览器的宿主清单（Windows 另写注册表），返回写了哪些文件。
pub fn install(o: &Install) -> Result<Vec<PathBuf>> {
    std::fs::create_dir_all(&o.data)?;
    let wrapper = o.wrapper();
    let mut written = vec![];
    // Chrome 要求宿主是单个可执行文件路径，不能带参数；用一个小脚本包一层（Windows 非便携模式直接用 captae.exe）
    if !o.direct() {
        std::fs::write(&wrapper, wrapper_script(&o.exe, o.env_home.as_deref()))?;
        set_exec(&wrapper)?;
        written.push(wrapper.clone());
    }
    let m = serde_json::to_vec_pretty(&manifest(&wrapper, o.ext_id()))?;
    let targets = targets(o);
    #[cfg(windows)]
    {
        let f = o.win_manifest();
        std::fs::write(&f, &m)?;
        written.push(f.clone());
        let mut keys: Vec<&'static str> = targets.iter().flat_map(|b| b.keys.iter().copied()).collect();
        keys.sort();
        keys.dedup();
        for k in keys {
            reg::write(o, k, &f).with_context(|| crate::tr!("写不进注册表 {path}", path = format!(r"HKCU\{k}\{HOST_NAME}")))?;
        }
    }
    #[cfg(not(windows))]
    for b in targets {
        let w = o.wrapper_of(&b);
        let m = match b.sandbox {
            Some(Sandbox::Flatpak(_)) => serde_json::to_vec_pretty(&manifest(&w, o.ext_id()))?,
            _ => m.clone(),
        };
        for (dir, f) in b.dirs.iter().zip(b.manifests()) {
            if written.contains(&f) {
                continue;
            }
            std::fs::create_dir_all(dir).with_context(|| crate::tr!("写不进 {path}", path = dir.display()))?;
            std::fs::write(&f, &m)?;
            written.push(f);
        }
        if let Some(Sandbox::Flatpak(_)) = b.sandbox {
            std::fs::write(&w, flatpak_wrapper_script(&o.exe, o.env_home.as_deref()))?;
            set_exec(&w)?;
            written.push(w);
        }
    }
    Ok(written)
}

fn set_exec(_p: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(_p, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(())
}

/// Windows 注册表：HKCU\<键>\app.captae.bridge 的默认值（用 winreg，不调 reg 命令）
#[cfg(windows)]
mod reg {
    use super::{Install, HOST_NAME};
    use std::path::Path;
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    fn path(o: &Install, key: &str) -> String {
        match &o.reg_prefix {
            Some(p) => format!(r"{p}\{key}\{HOST_NAME}"),
            None => format!(r"{key}\{HOST_NAME}"),
        }
    }

    pub fn read(o: &Install, key: &str) -> Option<String> {
        RegKey::predef(HKEY_CURRENT_USER).open_subkey(path(o, key)).ok()?.get_value::<String, _>("").ok()
    }

    pub fn write(o: &Install, key: &str, manifest: &Path) -> std::io::Result<()> {
        let (k, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(path(o, key))?;
        k.set_value("", &manifest.display().to_string())
    }

    /// 测试用：删掉整个测试前缀
    #[cfg(test)]
    pub fn drop_prefix(prefix: &str) {
        let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(prefix);
    }
}

/// 桌面端启动时调用：没注册或者指向别的程序时重新注册。返回是否写了文件。
pub fn ensure(o: &Install) -> Result<bool> {
    if status(o).ok {
        return Ok(false);
    }
    install(o)?;
    Ok(true)
}

/// 桌面端旁边的 `captae`：发布版在 Contents/MacOS/captae（sidecar），开发时在 target/<profile>/captae。
pub fn cli_next_to(exe: &Path) -> Option<PathBuf> {
    // Linux 的 deb / AppImage：captae 在 ../lib/Captae/ 里（tools::sidecar_dir_of）
    let p = crate::tools::sidecar_dir_of(exe)?.join(if cfg!(windows) { "captae.exe" } else { "captae" });
    p.is_file().then_some(p)
}

/// 浏览器直接启动 captae.exe 当宿主时（Windows），第一个参数是 `chrome-extension://<插件 id>/`：命令行据此当成 `native-host`
pub fn is_browser_origin(arg: &str) -> bool {
    arg.starts_with("chrome-extension://")
}

// ------------------------------------------------------------------ 认浏览器

/// 一个程序的路径是 `browsers()` 表里的哪一家，返回 id（chrome / arc / edge-beta……）。
/// macOS：先看路径里的 .app 包名，从外往里第一个对得上的（`Google Chrome.app/…/Google Chrome Helper.app/…` 是 Chrome，
/// 装在别的位置、被系统挪到 AppTranslocation 里也一样）；包改过名时再看 `Contents/MacOS/` 下的程序名（改包名不会改它）。
/// Windows、Linux：按表里的 `marks` 从路径末尾往前比（不分大小写，`\` 当 `/`），装在哪个盘、哪个用户目录下都认
pub fn browser_of_path(path: &Path, table: &[Browser]) -> Option<&'static str> {
    let parts: Vec<String> = path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    let known: Vec<&Browser> = table.iter().filter(|b| !b.app.is_empty()).collect();
    for p in parts.iter().filter(|p| p.to_ascii_lowercase().ends_with(".app")) {
        if let Some(b) = known.iter().find(|b| b.app.eq_ignore_ascii_case(p)) {
            return Some(b.id);
        }
    }
    if let [.., contents, macos, exe] = parts.as_slice() {
        if contents == "Contents" && macos == "MacOS" {
            if let Some(b) = known.iter().find(|b| b.app.trim_end_matches(".app").eq_ignore_ascii_case(exe)) {
                return Some(b.id);
            }
        }
    }
    let norm = path.to_string_lossy().replace('\\', "/").to_lowercase();
    let segs: Vec<&str> = norm.split('/').filter(|s| !s.is_empty()).collect();
    table.iter().find(|b| b.marks.iter().any(|m| mark_matches(m, &segs))).map(|b| b.id)
}

/// `marks` 里的一条规则对不对得上路径（都已经小写、按 `/` 切好）：从末尾往前逐段比，`*` 匹配一段里的任意字符
fn mark_matches(mark: &str, segs: &[&str]) -> bool {
    let m: Vec<&str> = mark.split('/').filter(|s| !s.is_empty()).collect();
    m.len() <= segs.len() && m.iter().rev().zip(segs.iter().rev()).all(|(p, s)| glob_seg(p, s))
}

/// 一段里的通配：只认 `*`
fn glob_seg(pat: &str, s: &str) -> bool {
    match pat.split_once('*') {
        None => pat == s,
        Some((head, tail)) => s.len() >= head.len() + tail.len() && s.starts_with(head) && glob_seg_rest(tail, &s[head.len()..]),
    }
}

fn glob_seg_rest(pat: &str, s: &str) -> bool {
    match pat.split_once('*') {
        None => s.ends_with(pat),
        Some((mid, tail)) => (0..=s.len()).filter(|i| s.is_char_boundary(*i)).any(|i| s[i..].starts_with(mid) && glob_seg_rest(tail, &s[i + mid.len()..])),
    }
}

/// 往上找浏览器最多看几层进程。包装脚本用 `exec`，正常第一层（父进程）就是浏览器；
/// 用户自己又包了一层脚本、或者 sh 没有 exec 时，父进程是 shell，再往上找。
const MAX_HOPS: usize = 4;

/// shell、env 这类只是转一手的程序：遇到它们再往上找。别的程序（认不出的 .app、桌面端自己的「测试连接」）就停下，
/// 不再往上猜，免得把启动浏览器的程序认成浏览器。Windows 上便携模式的 .bat 经过 cmd.exe
fn is_launcher(path: &Path) -> bool {
    // 按 / 和 \ 都切一次：在哪个系统上都认得出 Windows 的路径（测试里用）
    let full = path.to_string_lossy().to_ascii_lowercase();
    let name = full.rsplit(['/', '\\']).next().unwrap_or("");
    let name = name.strip_suffix(".exe").unwrap_or(name);
    matches!(name, "sh" | "bash" | "zsh" | "dash" | "ksh" | "tcsh" | "csh" | "fish" | "env" | "nohup" | "cmd")
}

/// 从进程 `pid` 开始往上找浏览器。`lookup` 给出一个进程的程序路径和它的父进程号（测试里换成假的进程表）。
fn detect_from(mut pid: u32, lookup: impl Fn(u32) -> Option<(PathBuf, u32)>, table: &[Browser]) -> Option<&'static str> {
    for _ in 0..MAX_HOPS {
        // 0 是内核（Windows 是空闲进程），1 是 launchd / init：到顶了
        if pid <= 1 {
            return None;
        }
        let (path, parent) = lookup(pid)?;
        if let Some(id) = browser_of_path(&path, table) {
            return Some(id);
        }
        if !is_launcher(&path) {
            return None;
        }
        pid = parent;
    }
    None
}

/// 是哪家浏览器启动了这个宿主（`browsers()` 的 id）；认不出时 None，桌面端退回插件自己报的。
/// Flatpak 版浏览器经过 `flatpak-spawn --host`，父进程是 flatpak-session-helper，认不出（退回插件报的）
pub fn detect_browser() -> Option<&'static str> {
    #[cfg(unix)]
    let parent = std::os::unix::process::parent_id();
    #[cfg(windows)]
    let parent = proc_info(std::process::id()).map(|(_, p)| p).unwrap_or(0);
    #[cfg(any(unix, windows))]
    {
        detect_from(parent, proc_info, &browsers(&home_dir()))
    }
    #[cfg(not(any(unix, windows)))]
    {
        None
    }
}

/// 一个进程的程序路径和父进程号。两个函数都在 libSystem 里（libproc.h），核心库没有直接依赖 libc，这里自己声明。
#[cfg(target_os = "macos")]
fn proc_info(pid: u32) -> Option<(PathBuf, u32)> {
    use std::os::raw::{c_int, c_void};
    use std::os::unix::ffi::OsStringExt;
    extern "C" {
        fn proc_pidpath(pid: c_int, buffer: *mut c_void, buffersize: u32) -> c_int;
        fn proc_pidinfo(pid: c_int, flavor: c_int, arg: u64, buffer: *mut c_void, buffersize: c_int) -> c_int;
    }
    // PROC_PIDPATHINFO_MAXSIZE = 4 * MAXPATHLEN
    let mut buf = vec![0u8; 4 * 1024];
    let n = unsafe { proc_pidpath(pid as c_int, buf.as_mut_ptr().cast(), buf.len() as u32) };
    if n <= 0 {
        return None;
    }
    buf.truncate(n as usize);
    let path = PathBuf::from(std::ffi::OsString::from_vec(buf));
    // PROC_PIDT_SHORTBSDINFO（13）：struct proc_bsdshortinfo 一共 64 字节，开头是 pbsi_pid、pbsi_ppid 两个 uint32
    const PROC_PIDT_SHORTBSDINFO: c_int = 13;
    let mut info = [0u32; 16];
    let size = std::mem::size_of_val(&info) as c_int;
    let got = unsafe { proc_pidinfo(pid as c_int, PROC_PIDT_SHORTBSDINFO, 0, info.as_mut_ptr().cast(), size) };
    let parent = if got == size { info[1] } else { 0 };
    Some((path, parent))
}

/// 一个进程的程序路径和父进程号（Linux：/proc/<pid>/exe 和 /proc/<pid>/stat 的第 4 项）
#[cfg(all(unix, not(target_os = "macos")))]
fn proc_info(pid: u32) -> Option<(PathBuf, u32)> {
    let path = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // 程序名在括号里，可能带空格和括号：从最后一个 ) 往后数
    let parent = stat.rsplit_once(')')?.1.split_whitespace().nth(1)?.parse().ok()?;
    Some((path, parent))
}

/// 一个进程的程序路径和父进程号（Windows：进程快照里的父进程号，QueryFullProcessImageNameW 拿路径）
#[cfg(windows)]
fn proc_info(pid: u32) -> Option<(PathBuf, u32)> {
    let parent = crate::tools::win::snapshot().into_iter().find(|(p, _)| *p == pid)?.1;
    Some((crate::tools::win::image_path(pid)?, parent))
}

// ------------------------------------------------------------------ 帧

pub fn read_msg(r: &mut impl Read) -> Result<Option<Value>> {
    let mut len = [0u8; 4];
    if r.read_exact(&mut len).is_err() {
        return Ok(None);
    }
    let n = u32::from_ne_bytes(len) as usize;
    if n > 8 * 1024 * 1024 {
        bail!("message too large");
    }
    let mut buf = vec![0u8; n];
    r.read_exact(&mut buf)?;
    Ok(Some(serde_json::from_slice(&buf)?))
}

pub fn write_msg(w: &mut impl Write, v: &Value) -> Result<()> {
    let b = serde_json::to_vec(v)?;
    w.write_all(&(b.len() as u32).to_ne_bytes())?;
    w.write_all(&b)?;
    w.flush()?;
    Ok(())
}

// ------------------------------------------------------------------ 查重

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CheckReq {
    /// 要下载的链接（M3U8 可以把各清晰度的地址一起带上）
    pub urls: Vec<String>,
    /// 整页下载时的页面地址：来源页面是它的视频也算下过
    pub page_url: Option<String>,
}

/// 这个链接是不是已经下载过：回 {found, assetId, title, where, dur, kind}。
pub fn check(lib: &Library, paths: &Paths, r: &CheckReq) -> Result<Value> {
    let mut hit = None;
    for u in &r.urls {
        if let Some(a) = lib.find_by_url(u, false)? {
            hit = Some(a);
            break;
        }
    }
    if hit.is_none() {
        if let Some(p) = &r.page_url {
            hit = lib.find_by_url(p, true)?;
        }
    }
    Ok(match hit {
        Some(a) => json!({"found": true, "assetId": a.id, "title": a.title, "where": paths.human_dir(&a.path), "dur": a.dur, "kind": a.kind}),
        None => json!({"found": false}),
    })
}

/// 插件卡片和面板要用的任务字段（宿主的 jobs 回复）。录直播带上 `rec` 里的几项：在录（`since` 开始录的时刻、`max` 最多录多少秒）、
/// 正在保存（`stopping`）、录完（`until`、`secs`、`full` 录满自动停的）。录到哪个文件夹这类本机路径不带
pub fn job_lite(j: &Value) -> Value {
    let rec = j["rec"].as_object().map(|r| {
        let keep = ["since", "until", "stopping", "secs", "max", "full", "interrupted"];
        Value::Object(r.iter().filter(|(k, _)| keep.contains(&k.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect())
    });
    json!({"id": j["id"], "title": j["title"], "state": j["state"], "phase": j["phase"], "got": j["got"], "size": j["size"], "err": j["err"], "path": j["path"], "fmt": j["fmt"], "assetId": j["assetId"],
        "fail": j["fail"], "choose": j["choose"], "entries": j["entries"], "live": j["live"], "rec": rec})
}

// ------------------------------------------------------------------ 运行

/// 桌面端没开时尝试把它打开（路径由桌面端启动时写入 app_path）。
async fn ensure_app(paths: &Paths) -> Option<Client> {
    // 桌面端开着、只是一时忙得没回话：等它一下，不要再 `open` 一次
    if let Some(c) = Client::connect_patient(&paths.control()).await {
        return Some(c);
    }
    let app = std::fs::read_to_string(paths.data.join("app_path")).ok()?;
    let app = app.trim();
    tracing::info!("桌面端连不上，打开 {app} 再等它（最多 10 秒）");
    if !launch_app(Path::new(app), &paths.data.join("launch.log")) {
        return None;
    }
    for _ in 0..40 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if let Some(c) = Client::connect(&paths.control()).await {
            return Some(c);
        }
    }
    None
}

/// 在后台打开桌面端，和宿主脱开：浏览器关掉宿主（关插件、关浏览器）时桌面端不跟着结束，也不占着浏览器和宿主之间的管道。
/// macOS 交给 `open -g -a`；Windows 直接起 exe（DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP，浏览器把宿主放进了 Job 时再脱离 Job）；
/// Linux 直接起程序，放进新的会话（setsid），不在宿主的进程组里。
/// 桌面端的报错输出写进 `log`（数据目录 `launch.log`，每次覆盖）：被插件拉起时没有终端，一起来就退出也看得出为什么
/// （2026-11-24 Devin 的 Linux 上 AppImage 被拉起后马上退出）。起来以后有个线程等着它，退出了收走，不留僵尸进程
fn launch_app(app: &Path, log: &Path) -> bool {
    use std::process::{Command, Stdio};
    #[cfg(target_os = "macos")]
    let mut c = {
        let mut c = Command::new("open");
        c.args(["-g", "-a"]).arg(app);
        c
    };
    #[cfg(not(target_os = "macos"))]
    let mut c = {
        let mut c = Command::new(app);
        c.arg("--background");
        c
    };
    c.stdin(Stdio::null()).stdout(Stdio::null());
    match std::fs::File::create(log) {
        Ok(f) => {
            c.stderr(f);
            if std::env::var_os("CAPTAE_LOG").is_none() {
                c.env("CAPTAE_LOG", "info");
            }
        }
        Err(_) => {
            c.stderr(Stdio::null());
        }
    }
    let started = |r: std::io::Result<std::process::Child>| match r {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let st = child.wait();
                tracing::info!("拉起的桌面端退出了：{st:?}");
            });
            true
        }
        Err(e) => {
            tracing::warn!("拉起桌面端失败：{e}");
            false
        }
    };
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::System::Threading::{CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
        // Job 不许脱离时带着 CREATE_BREAKAWAY_FROM_JOB 启动会失败（拒绝访问），再不带它试一次
        c.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB);
        if let Ok(child) = c.spawn() {
            return started(Ok(child));
        }
        c.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::os::unix::process::CommandExt;
        // Chrome 给宿主设的 DBUS_SESSION_BUS_ADDRESS=disabled: 不传下去（桌面端连会话总线会失败，见桌面端 main）
        if std::env::var("DBUS_SESSION_BUS_ADDRESS").is_ok_and(|v| v.starts_with("disabled")) {
            c.env_remove("DBUS_SESSION_BUS_ADDRESS");
        }
        extern "C" {
            fn setsid() -> i32;
        }
        // SAFETY：子进程 fork 以后、exec 之前只调 setsid（async-signal-safe）
        unsafe {
            c.pre_exec(|| {
                setsid();
                Ok(())
            });
        }
    }
    started(c.spawn())
}

/// `captae native-host`：从 stdin 读插件的消息，回复写 stdout。
pub async fn run(paths: Paths, sidecar_dir: Option<PathBuf>) -> Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        while let Ok(Some(v)) = read_msg(&mut stdin) {
            if tx.send(v).is_err() {
                break;
            }
        }
    });
    let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
    let writer = tokio::task::spawn_blocking(move || -> Result<()> {
        let mut out = std::io::stdout().lock();
        while let Some(v) = out_rx.blocking_recv() {
            write_msg(&mut out, &v)?;
        }
        Ok(())
    });
    // 是哪家浏览器启动了宿主：启动时认一次（一个宿主进程只对应一个浏览器）
    let host = std::sync::Arc::new(Host { paths, sidecar_dir, browser: detect_browser(), stale: Default::default(), busy: Default::default() });
    while let Some(msg) = rx.recv().await {
        let (host, out_tx) = (host.clone(), out_tx.clone());
        host.busy.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        tokio::spawn(async move {
            let id = msg.get("id").cloned().unwrap_or(Value::Null);
            let ty = msg.get("type").and_then(Value::as_str).unwrap_or("");
            let t0 = std::time::Instant::now();
            let reply = match host.handle(ty, &msg).await {
                Ok(v) => json!({"id": id.clone(), "ok": true, "data": v}),
                Err(e) => error_reply(id.clone(), ty, &msg, &e),
            };
            // 排查用（CAPTAE_LOG=captae_core::native_host=debug）：每条消息用了多久、成没成；超过 3 秒的照常记一行
            let ms = t0.elapsed().as_millis();
            let ok = reply["ok"].as_bool() == Some(true);
            if ms > 3000 {
                tracing::info!("插件消息 {ty}（{id}）用了 {ms}ms，{}", if ok { "成功" } else { reply["error"].as_str().unwrap_or("失败") });
            } else {
                tracing::debug!("插件消息 {ty}（{id}）{ms}ms {}", if ok { "成功" } else { reply["error"].as_str().unwrap_or("失败") });
            }
            let _ = out_tx.send(reply);
            // Captae 更新过了（hello 里发现的）：手头的消息都回完了就退出，留一点时间把回复写出去
            if host.busy.fetch_sub(1, std::sync::atomic::Ordering::SeqCst) == 1 && host.stale.load(std::sync::atomic::Ordering::SeqCst) {
                tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                if host.busy.load(std::sync::atomic::Ordering::SeqCst) == 0 {
                    tracing::info!("宿主退出，等插件用新版本重新连接");
                    std::process::exit(0);
                }
            }
        });
    }
    drop(out_tx);
    let _ = writer.await;
    Ok(())
}

/// 失败的回复：`error` 照旧是一句话（插件现在就显示它），另加 `code` 和 `fail`（错误码、原文、下一步）。
/// 读取信息、下载这类和链接有关的失败一定有错误码；APP_NOT_RUNNING 这类宿主自己的错误没有。
pub fn error_reply(id: Value, ty: &str, msg: &Value, e: &anyhow::Error) -> Value {
    let f = match crate::errors::failure_in(e) {
        Some(f) => Some(f.clone()),
        None if matches!(ty, "probe" | "download") && e.to_string() != "APP_NOT_RUNNING" => {
            let url = msg["url"].as_str().or_else(|| msg["req"]["url"].as_str()).unwrap_or("");
            Some(crate::errors::from_anyhow(e, url))
        }
        None => None,
    };
    match f {
        Some(f) => json!({"id": id, "ok": false, "error": f.message, "code": f.code, "fail": f.to_json()}),
        None => {
            let mut v = json!({"id": id, "ok": false, "error": e.to_string()});
            // 固定代码（NOT_RECORDING……）：插件按代码判断，不看那句话（那句话跟着界面语言变）
            if let Some(c) = crate::errors::code_in(e) {
                v["code"] = json!(c);
            }
            v
        }
    }
}

struct Host {
    paths: Paths,
    sidecar_dir: Option<PathBuf>,
    /// 看父进程认出来的浏览器（`browsers()` 的 id），认不出是 None
    browser: Option<&'static str>,
    /// Captae 已经更新到比这个宿主新的版本：手头的消息回完就退出
    stale: std::sync::atomic::AtomicBool,
    /// 正在处理的消息数
    busy: std::sync::atomic::AtomicUsize,
}

fn not_running() -> anyhow::Error {
    anyhow!("APP_NOT_RUNNING")
}

impl Host {
    /// 桌面端在运行就连上。一时没回话（正忙）时多等一会儿，不马上回 APP_NOT_RUNNING：
    /// 插件的卡片收到它就说「Captae 关掉了」（第三轮插件回归 t3 的误报）
    async fn app(&self) -> Option<Client> {
        Client::connect_patient(&self.paths.control()).await
    }

    async fn handle(&self, ty: &str, m: &Value) -> Result<Value> {
        let paths = &self.paths;
        match ty {
            // 插件启动和每分钟打一次招呼；test 为真时是桌面端自己在测宿主，不算插件来过。
            // 浏览器：宿主认出来的（detected）为准，插件自己报的（browser）留作后备，桌面端选
            "hello" => {
                let c = self.app().await;
                let mut app_version: Option<String> = None;
                if let (Some(c), false) = (&c, m["test"].as_bool().unwrap_or(false)) {
                    if let Ok(v) = c.hello(m["version"].as_str(), m["browser"].as_str(), self.browser).await {
                        app_version = v["version"].as_str().map(String::from);
                    }
                }
                // Captae 更新过了、这个宿主还是浏览器先前用旧的程序起的：回完手头的消息就退出，插件下次连接时浏览器起的是新版本（run 里看 stale）
                if app_version.as_deref().is_some_and(|v| crate::updater::newer(v, env!("CARGO_PKG_VERSION"))) {
                    tracing::info!("Captae 已经是 {}，这个宿主还是 {}：回完手头的消息就退出", app_version.as_deref().unwrap_or(""), env!("CARGO_PKG_VERSION"));
                    self.stale.store(true, std::sync::atomic::Ordering::SeqCst);
                }
                let s = Settings::load(&paths.settings());
                // 界面语言：langSet 是设置里写的（空 = 跟随系统），lang 是实际用的。插件在 Captae 里选了语言时跟着它，跟随系统时用浏览器的界面语言
                let lang = crate::i18n::apply(&s.language);
                // appVersion：正在运行的 Captae 的版本（没开时为空）。插件比它旧时从文件夹重新加载自己（background.js 的 followApp）
                Ok(json!({"app": c.is_some(), "version": env!("CARGO_PKG_VERSION"), "appVersion": app_version, "clipboard": s.clipboard, "lang": lang.code(), "langSet": s.language}))
            }
            "sniffed" => {
                let item: crate::sniff::Sniffed = serde_json::from_value(m["item"].clone())?;
                // 嗅探只是告知，桌面端没开就不打扰
                match self.app().await {
                    Some(c) => c.sniffed(&item).await,
                    None => Ok(json!({"app": false})),
                }
            }
            // 网页里复制了一个视频链接：只记下来（什么链接、什么时候），不打开 Captae、不弹任何东西。
            // 1 分钟内打开 Captae 时，桌面端读它问一句要不要下载（Captae 开着时它自己看得到剪贴板，见 clipboard.rs）
            "copied" => {
                let url = m["url"].as_str().unwrap_or_default();
                if let Some(url) = crate::clipboard::is_media_link(url) {
                    crate::clipboard::write_hint(&paths.data, &url)?;
                }
                Ok(json!({}))
            }
            "download" => {
                let req: AddRequest = serde_json::from_value(m["req"].clone()).context(crate::tr!("请求格式不对"))?;
                let c = ensure_app(paths).await.ok_or_else(not_running)?;
                c.add(&AddRequest { origin: Some("extension".into()), ..req }).await
            }
            "share" => {
                let c = ensure_app(paths).await.ok_or_else(not_running)?;
                c.share(m["opts"].clone()).await
            }
            // 打开窗口，可以带上要去的地方：{page} / {asset} / {job} / {paste}
            "open" => {
                let c = ensure_app(paths).await.ok_or_else(not_running)?;
                let t: FocusTarget = serde_json::from_value(m["target"].clone()).unwrap_or_default();
                c.focus(&t).await
            }
            // 插件轮询下载进度：只回它关心的几个任务，字段也只留卡片要用的
            "jobs" => {
                let c = self.app().await.ok_or_else(not_running)?;
                let ids: Vec<u64> = serde_json::from_value(m["ids"].clone()).unwrap_or_default();
                let all = c.jobs().await?;
                let list: Vec<Value> = all
                    .as_array()
                    .map(|a| a.iter().filter(|j| j["id"].as_u64().is_some_and(|i| ids.contains(&i))).cloned().collect())
                    .unwrap_or_default();
                Ok(Value::Array(list.iter().map(job_lite).collect()))
            }
            // 停止 / 暂停 / 继续一个下载
            "job" => {
                let c = self.app().await.ok_or_else(not_running)?;
                let action: JobAction = serde_json::from_value(m["action"].clone()).context(crate::tr!("不认识的操作"))?;
                c.job_action(m["job"].as_u64().unwrap_or(0), action).await
            }
            // 下载前查一下是不是已经下过；桌面端没开就直接读素材库
            "check" => {
                let r: CheckReq = serde_json::from_value(m["req"].clone()).unwrap_or_default();
                match self.app().await {
                    Some(c) => c.check(&r).await,
                    None if paths.db().exists() => check(&Library::open(&paths.db())?, paths, &r),
                    None => Ok(json!({"found": false})),
                }
            }
            // 标题、时长、封面、各清晰度大小；桌面端没开就在宿主里自己读。
            // 插件可以带上浏览器里这个网站的 cookies 和 User-Agent（`cookies`、`user_agent`），只用这一次
            "probe" => {
                let url = m["url"].as_str().ok_or_else(|| anyhow!(crate::tr!("缺少链接")))?;
                let headers: std::collections::BTreeMap<String, String> = serde_json::from_value(m["headers"].clone()).unwrap_or_default();
                let cookies: Vec<crate::login::Cookie> = serde_json::from_value(m["cookies"].clone()).unwrap_or_default();
                let ua = m["user_agent"].as_str().map(str::to_string);
                match self.app().await {
                    Some(c) => c.probe(url, &headers, &cookies, ua.as_deref()).await,
                    None => {
                        let s = Settings::load(&paths.settings());
                        let tools = crate::tools::Toolbox::from_settings(&s, self.sidecar_dir.clone(), Some(paths.data.clone()));
                        let http = crate::net::build_client(&s);
                        let mut headers = headers;
                        let mut cookies = cookies;
                        cookies.extend(crate::login::take_cookie_header(&mut headers, url));
                        Ok(serde_json::to_value(crate::probe::probe_ext(&tools, &http, url, &headers, cookies, ua, crate::login::auth_dir(paths)).await?)?)
                    }
                }
            }
            "task" => {
                let c = self.app().await.ok_or_else(not_running)?;
                c.task(m["task"].as_u64().unwrap_or(0)).await
            }
            "task-cancel" => {
                let c = self.app().await.ok_or_else(not_running)?;
                c.task_cancel(m["task"].as_u64().unwrap_or(0)).await
            }
            "recopy" => {
                let c = self.app().await.ok_or_else(not_running)?;
                c.task_copy(m["task"].as_u64().unwrap_or(0)).await
            }
            _ => bail!("unknown message type {ty}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(root: &Path) -> Install {
        let exe = root.join(if cfg!(windows) { "bin/captae.exe" } else { "bin/captae" });
        std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
        std::fs::write(&exe, b"").unwrap();
        // Windows：注册表键写在一个测试用的前缀下（测试完删掉），不碰浏览器真的会读的位置
        let reg_prefix = cfg!(windows).then(|| format!(r"Software\CaptaeTest\{}", root.file_name().unwrap().to_string_lossy()));
        Install { exe, data: root.join("data"), home: root.join("home"), ext_id: None, all: false, env_home: None, app_dirs: vec![root.join("Applications")], reg_prefix }
    }

    /// 测试结束时删掉测试用的注册表前缀（Windows）
    struct RegCleanup(Option<String>);
    impl Drop for RegCleanup {
        fn drop(&mut self) {
            #[cfg(windows)]
            if let Some(p) = &self.0 {
                reg::drop_prefix(p);
            }
            #[cfg(not(windows))]
            let _ = &self.0;
        }
    }

    /// 收尾修补第 1 项：录直播的任务带上录制状态（插件卡片显示「已录 12:34」、给「停止并保存」），不带本机路径
    #[test]
    fn job_lite_carries_recording() {
        let j = json!({"id": 7, "title": "直播", "state": "run", "phase": "dl", "got": 3.2, "size": 0.0, "live": true,
            "rec": {"since": 1000, "max": 600, "raw": "/Users/x/.captae-tmp/a", "out": "/Users/x/Movies/Captae"}, "log": [[1, "很长的记录", ""]]});
        let v = job_lite(&j);
        assert_eq!((v["live"].as_bool(), v["rec"]["since"].as_i64(), v["rec"]["max"].as_u64()), (Some(true), Some(1000), Some(600)));
        assert!(v["rec"].get("raw").is_none() && v["rec"].get("out").is_none() && v.get("log").is_none(), "{v}");
        // 保存中、录完
        let v = job_lite(&json!({"id": 7, "state": "run", "phase": "merge", "rec": {"since": 1000, "until": 9000, "stopping": true}}));
        assert_eq!(v["rec"]["stopping"].as_bool(), Some(true));
        // 不是录直播的任务：rec 是 null
        assert!(job_lite(&json!({"id": 8, "state": "run", "phase": "dl"}))["rec"].is_null());
    }

    #[test]
    fn framing_roundtrip() {
        let mut buf = vec![];
        write_msg(&mut buf, &json!({"id": 1, "type": "hello"})).unwrap();
        write_msg(&mut buf, &json!({"id": 2, "type": "jobs", "ids": [3]})).unwrap();
        assert_eq!(u32::from_ne_bytes(buf[..4].try_into().unwrap()) as usize, serde_json::to_vec(&json!({"id": 1, "type": "hello"})).unwrap().len());
        let mut r = std::io::Cursor::new(buf);
        assert_eq!(read_msg(&mut r).unwrap().unwrap()["type"], "hello");
        assert_eq!(read_msg(&mut r).unwrap().unwrap()["ids"][0], 3);
        assert!(read_msg(&mut r).unwrap().is_none());
    }

    #[test]
    fn install_into_temp_home() {
        let dir = tempfile::tempdir().unwrap();
        let o = opts(dir.path());
        let _clean = RegCleanup(o.reg_prefix.clone());
        // 一个浏览器都没装：只写 Chrome
        assert!(!status(&o).ok);
        assert!(ensure(&o).unwrap());
        let st = status(&o);
        assert!(st.ok && st.wrapper_ok);
        let chrome = &st.browsers[0];
        assert_eq!(chrome.id, "chrome");
        // 清单：macOS、Linux 在浏览器的目录里；Windows 在数据目录，注册表指过去
        assert!(chrome.registered && chrome.manifest.starts_with(if cfg!(windows) { &o.data } else { &o.home }), "{chrome:?}");
        let m: Value = serde_json::from_slice(&std::fs::read(&chrome.manifest).unwrap()).unwrap();
        assert_eq!(m["path"].as_str().map(PathBuf::from), Some(o.wrapper()));
        assert_eq!(m["allowed_origins"][0], format!("chrome-extension://{CHROME_EXTENSION_ID}/"));
        if cfg!(windows) {
            // Windows 直接指向 captae.exe，不写 .bat
            assert_eq!(o.wrapper(), o.exe);
            assert!(!o.data.join("captae-native-host.bat").exists());
            assert_eq!(chrome.keys, vec![format!(r"HKCU\Software\Google\Chrome\NativeMessagingHosts\{HOST_NAME}")]);
        } else {
            assert!(std::fs::read_to_string(o.wrapper()).unwrap().contains(&o.exe.display().to_string()));
            assert!(st.browsers[1..].iter().all(|b| !b.registered));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(o.wrapper()).unwrap().permissions().mode() & 0o111, 0o111);
        }
        // 已经注册好就不再写
        assert!(!ensure(&o).unwrap());
        // 别的软件替没装的浏览器建了目录：不算装了
        let edge = browsers(&o.home).into_iter().find(|b| b.id == "edge").unwrap();
        std::fs::create_dir_all(edge.root.join("NativeMessagingHosts")).unwrap();
        assert!(!edge.installed(&o.app_dirs));
        assert!(status(&o).ok);
        // 装了 Edge（运行过，有 Local State）：Edge 也要注册（Chrome 这时没装，就不再要求）
        std::fs::write(edge.root.join("Local State"), b"{}").unwrap();
        assert!(!status(&o).ok);
        assert!(ensure(&o).unwrap());
        assert!(status(&o).browsers.iter().find(|b| b.id == "edge").unwrap().registered);
        // captae 换了位置（比如 App 挪了地方）：包装脚本（Windows：清单）要重写
        let moved_exe = dir.path().join(if cfg!(windows) { "elsewhere/captae.exe" } else { "elsewhere/captae" });
        std::fs::create_dir_all(moved_exe.parent().unwrap()).unwrap();
        std::fs::write(&moved_exe, b"").unwrap();
        let moved = Install { exe: moved_exe, ..o.clone() };
        assert!(!status(&moved).ok);
        assert!(ensure(&moved).unwrap());
        assert!(status(&moved).ok);
        if !cfg!(windows) {
            assert!(std::fs::read_to_string(o.wrapper()).unwrap().contains("elsewhere"));
        }
        // 便携模式：包装脚本带上数据目录（Windows 这时才写 .bat）
        let home = dir.path().join("it's portable");
        let portable = Install { env_home: Some(home.clone()), ..o.clone() };
        assert!(ensure(&portable).unwrap());
        let w = std::fs::read_to_string(portable.wrapper()).unwrap();
        // sh 里单引号要写成 '\''；.bat 里原样
        let want = if cfg!(windows) { home.display().to_string() } else { home.display().to_string().replace('\'', "'\\''") };
        assert!(w.contains(&want), "{w}");
        if cfg!(windows) {
            assert!(portable.wrapper().ends_with("captae-native-host.bat"));
        }
        // 宿主启动时真的能拿到（unix 上跑一下包装脚本）
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let fake = dir.path().join("fake-captae");
            std::fs::write(&fake, "#!/bin/sh\nprintf '%s|%s' \"$CAPTAE_HOME\" \"$1\"\n").unwrap();
            std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
            let run = Install { exe: fake, ..portable.clone() };
            assert!(ensure(&run).unwrap());
            let out = std::process::Command::new(run.wrapper()).output().unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout), format!("{}|native-host", home.display()));
        }
        // 只装了 Opera 的 .app（macOS）：Opera 自己的目录和 Chrome 的目录都写
        #[cfg(target_os = "macos")]
        {
            let opera_only = Install { home: dir.path().join("home2"), app_dirs: vec![dir.path().join("Apps2")], ..o.clone() };
            std::fs::create_dir_all(dir.path().join("Apps2/Opera.app")).unwrap();
            let w = install(&opera_only).unwrap();
            assert!(w.iter().any(|f| f.to_string_lossy().contains("com.operasoftware.Opera/NativeMessagingHosts")));
            assert!(w.iter().any(|f| f.to_string_lossy().contains("Google/Chrome/NativeMessagingHosts")));
            let arc = browsers(&o.home).into_iter().find(|b| b.id == "arc").unwrap();
            assert!(arc.manifest().to_string_lossy().contains("Arc/User Data/NativeMessagingHosts"));
            let aside = browsers(&o.home).into_iter().find(|b| b.id == "aside").unwrap();
            assert!(aside.manifest().ends_with("Library/Application Support/Aside/NativeMessagingHosts/app.captae.bridge.json"));
            assert!(status(&opera_only).ok);
        }
        // --all：所有浏览器都写（同一个位置只写一次）
        let all = Install { all: true, ..o.clone() };
        let written = install(&all).unwrap();
        if cfg!(windows) {
            assert_eq!(written, vec![all.win_manifest()]);
        } else {
            let mut uniq: Vec<PathBuf> = browsers(&o.home).iter().flat_map(|b| b.manifests()).collect();
            uniq.sort();
            uniq.dedup();
            let flatpaks = browsers(&o.home).iter().filter(|b| matches!(b.sandbox, Some(Sandbox::Flatpak(_)))).count();
            assert_eq!(written.len(), 1 + uniq.len() + flatpaks);
        }
        assert!(status(&all).browsers.iter().all(|b| b.registered), "{:?}", status(&all).browsers.iter().filter(|b| !b.registered).map(|b| b.id).collect::<Vec<_>>());
    }

    /// Linux：Flatpak 版的清单指向沙盒里看得到的包装脚本（flatpak-spawn --host 到外面跑 captae），没开权限时状态里给出命令
    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn flatpak_wrapper() {
        let dir = tempfile::tempdir().unwrap();
        let o = Install { all: true, ..opts(dir.path()) };
        install(&o).unwrap();
        let b = browsers(&o.home).into_iter().find(|b| b.id == "chrome-flatpak").unwrap();
        assert!(b.dirs[0].starts_with(o.home.join(".var/app/com.google.Chrome/config/google-chrome")));
        let m: Value = serde_json::from_slice(&std::fs::read(b.manifest()).unwrap()).unwrap();
        let w = PathBuf::from(m["path"].as_str().unwrap());
        assert!(w.starts_with(&b.dirs[0]), "{}", w.display());
        let script = std::fs::read_to_string(&w).unwrap();
        assert!(script.contains("flatpak-spawn --host") && script.contains(&o.exe.display().to_string()), "{script}");
        let st = status(&o);
        let s = st.browsers.iter().find(|x| x.id == "chrome-flatpak").unwrap();
        assert!(s.registered && s.host_access == Some(false));
        assert_eq!(s.fix.as_deref(), Some("flatpak override --user --talk-name=org.freedesktop.Flatpak com.google.Chrome"));
        // 用户开了权限
        let ov = o.home.join(".local/share/flatpak/overrides");
        std::fs::create_dir_all(&ov).unwrap();
        std::fs::write(ov.join("com.google.Chrome"), "[Session Bus Policy]\norg.freedesktop.Flatpak=talk\n").unwrap();
        assert_eq!(status(&o).browsers.iter().find(|x| x.id == "chrome-flatpak").unwrap().host_access, Some(true));
        // Snap 版、Chrome Dev 的目录
        let snap = browsers(&o.home).into_iter().find(|b| b.id == "chromium-snap").unwrap();
        assert_eq!(snap.sandbox, Some(Sandbox::Snap));
        assert!(snap.manifest().ends_with("snap/chromium/common/chromium/NativeMessagingHosts/app.captae.bridge.json"));
        let dev = browsers(&o.home).into_iter().find(|b| b.id == "chrome-dev").unwrap();
        assert!(dev.manifest().starts_with(o.home.join(".config/google-chrome-unstable")));
        // 便携模式：CAPTAE_HOME 经 flatpak-spawn 的 --env 带过去
        assert!(flatpak_wrapper_script(Path::new("/opt/x/captae"), Some(Path::new("/h/it's"))).contains("'--env=CAPTAE_HOME=/h/it'\\''s'"));
    }

    /// Windows、Linux 的路径规则（不依赖这台电脑是什么系统：表是手写的）
    #[test]
    fn marks_match_paths() {
        let b = |id: &'static str, marks: Vec<&'static str>| Browser { id, name: id, root: PathBuf::new(), dirs: vec![], app: "", keys: vec![], exes: vec![], marks, sandbox: None };
        let t = vec![
            b("chrome", vec!["google/chrome/application/chrome.exe", "/opt/google/chrome/chrome"]),
            b("chrome-canary", vec!["google/chrome sxs/application/chrome.exe"]),
            b("edge", vec!["microsoft/edge/application/msedge.exe", "/opt/microsoft/msedge/msedge"]),
            b("opera", vec!["programs/opera/*/opera.exe", "programs/opera/opera.exe"]),
            b("arc", vec!["windowsapps/thebrowsercompany.arc_*/arc.exe"]),
            b("chromium-snap", vec!["/snap/chromium/*/usr/lib/chromium-browser/chrome"]),
        ];
        let id = |p: &str| browser_of_path(Path::new(p), &t);
        assert_eq!(id(r"C:\Program Files\Google\Chrome\Application\chrome.exe"), Some("chrome"));
        assert_eq!(id(r"C:\Users\张三\AppData\Local\Google\Chrome\Application\chrome.exe"), Some("chrome"));
        assert_eq!(id(r"c:\program files (x86)\microsoft\edge\application\MSEDGE.EXE"), Some("edge"));
        assert_eq!(id(r"C:\Users\a\AppData\Local\Google\Chrome SxS\Application\chrome.exe"), Some("chrome-canary"));
        assert_eq!(id(r"C:\Users\a\AppData\Local\Programs\Opera\115.0.5322.68\opera.exe"), Some("opera"));
        assert_eq!(id(r"C:\Program Files\WindowsApps\TheBrowserCompany.Arc_1.43.0.0_x64__ttt1ap7aakyb4\Arc.exe"), Some("arc"));
        assert_eq!(id("/opt/google/chrome/chrome"), Some("chrome"));
        assert_eq!(id("/opt/microsoft/msedge/msedge"), Some("edge"));
        assert_eq!(id("/snap/chromium/3013/usr/lib/chromium-browser/chrome"), Some("chromium-snap"));
        // 认不出的
        assert_eq!(id(r"C:\Windows\System32\cmd.exe"), None);
        assert_eq!(id(r"C:\Program Files\Captae\captae-desktop.exe"), None);
        assert_eq!(id("/usr/bin/bash"), None);
        assert_eq!(id(r"C:\x\Google\Chrome\Application\chrome_proxy.exe"), None);
        assert!(is_launcher(Path::new(r"C:\Windows\System32\CMD.EXE")) && is_launcher(Path::new("/bin/sh")));
        assert!(!is_launcher(Path::new(r"C:\Windows\explorer.exe")));
    }

    #[test]
    fn origin_arg() {
        assert!(is_browser_origin("chrome-extension://docjnhebpmdchlahmhkjladmehlcfkcp/"));
        assert!(!is_browser_origin("native-host") && !is_browser_origin("--parent-window=0"));
    }

    /// 父进程的程序路径 → 认成哪家（macOS 的表）
    #[cfg(target_os = "macos")]
    #[test]
    fn browser_from_path() {
        let t = browsers(Path::new("/Users/x"));
        let id = |p: &str| browser_of_path(Path::new(p), &t);
        assert_eq!(id("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"), Some("chrome"));
        assert_eq!(id("/Applications/Arc.app/Contents/MacOS/Arc"), Some("arc"));
        assert_eq!(id("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"), Some("edge"));
        assert_eq!(id("/Applications/Brave Browser.app/Contents/MacOS/Brave Browser"), Some("brave"));
        assert_eq!(id("/Applications/Vivaldi.app/Contents/MacOS/Vivaldi"), Some("vivaldi"));
        assert_eq!(id("/Users/x/Applications/Aside.app/Contents/MacOS/Aside"), Some("aside"));
        assert_eq!(id("/Applications/Opera GX.app/Contents/MacOS/Opera"), Some("opera-gx"));
        assert_eq!(id("/Applications/Opera.app/Contents/MacOS/Opera"), Some("opera"));
        assert_eq!(id("/Applications/Chromium.app/Contents/MacOS/Chromium"), Some("chromium"));
        assert_eq!(id("/Applications/Google Chrome Beta.app/Contents/MacOS/Google Chrome Beta"), Some("chrome-beta"));
        assert_eq!(id("/Applications/Microsoft Edge Canary.app/Contents/MacOS/Microsoft Edge Canary"), Some("edge-canary"));
        // 不在「应用程序」里的：Puppeteer 下载的 Chrome for Testing、系统挪到 AppTranslocation 里的
        assert_eq!(
            id("/Users/x/.cache/puppeteer/chrome/mac_arm-131/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing"),
            Some("chrome-for-testing")
        );
        assert_eq!(id("/private/var/folders/ab/T/AppTranslocation/1234/d/Arc.app/Contents/MacOS/Arc"), Some("arc"));
        // 浏览器里的辅助程序：按外面那层包认
        assert_eq!(
            id("/Applications/Google Chrome.app/Contents/Frameworks/Google Chrome Framework.framework/Versions/131/Helpers/Google Chrome Helper.app/Contents/MacOS/Google Chrome Helper"),
            Some("chrome")
        );
        // 包改过名：看程序名
        assert_eq!(id("/Applications/Chrome 工作.app/Contents/MacOS/Google Chrome"), Some("chrome"));
        // 认不出的：Safari、shell、Captae 自己
        assert_eq!(id("/Applications/Safari.app/Contents/MacOS/Safari"), None);
        assert_eq!(id("/bin/bash"), None);
        assert_eq!(id("/Applications/Captae.app/Contents/MacOS/captae-desktop"), None);
    }

    /// 往上找：shell 才继续，最多看 MAX_HOPS 层（表是手写的 macOS 那几家，哪个系统上都跑）
    #[test]
    fn browser_from_parents() {
        let mk = |id: &'static str, app: &'static str| Browser { id, name: id, root: PathBuf::new(), dirs: vec![], app, keys: vec![], exes: vec![], marks: vec![], sandbox: None };
        let t = vec![mk("chrome", "Google Chrome.app"), mk("arc", "Arc.app")];
        let procs: std::collections::HashMap<u32, (&str, u32)> = [
            (500, ("/Applications/Arc.app/Contents/MacOS/Arc", 1)),
            (600, ("/bin/sh", 500)),
            (601, ("/bin/zsh", 600)),
            (700, ("/Applications/Safari.app/Contents/MacOS/Safari", 1)),
            (710, ("/bin/sh", 700)),
            (800, ("/usr/bin/env", 801)),
            (801, ("/bin/bash", 802)),
            (802, ("/bin/sh", 803)),
            (803, ("/bin/sh", 500)),
            (900, ("/Applications/Captae.app/Contents/MacOS/captae-desktop", 1)),
            (950, ("/bin/sh", 900)),
        ]
        .into_iter()
        .collect();
        let look = |pid: u32| procs.get(&pid).map(|(p, parent)| (PathBuf::from(p), *parent));
        // 包装脚本 exec 了：父进程就是浏览器
        assert_eq!(detect_from(500, look, &t), Some("arc"));
        // 中间隔了一两层 shell
        assert_eq!(detect_from(600, look, &t), Some("arc"));
        assert_eq!(detect_from(601, look, &t), Some("arc"));
        // 认不出的程序：停下，不再往上猜
        assert_eq!(detect_from(700, look, &t), None);
        assert_eq!(detect_from(710, look, &t), None);
        // shell 套了四层：超过上限，不找了
        assert_eq!(detect_from(800, look, &t), None);
        // 桌面端「测试连接」启动的宿主
        assert_eq!(detect_from(950, look, &t), None);
        // 到顶了、进程已经没了
        assert_eq!(detect_from(1, look, &t), None);
        assert_eq!(detect_from(4242, look, &t), None);
    }

    /// 读进程的程序路径和父进程号（macOS 的 proc_pidpath / proc_pidinfo、Linux 的 /proc、Windows 的进程快照）：拿测试进程自己核对
    #[cfg(any(unix, windows))]
    #[test]
    fn proc_info_of_self() {
        let (path, parent) = proc_info(std::process::id()).expect("读得到自己");
        assert_eq!(path.canonicalize().unwrap(), std::env::current_exe().unwrap().canonicalize().unwrap());
        #[cfg(unix)]
        assert_eq!(parent, std::os::unix::process::parent_id());
        #[cfg(windows)]
        assert!(parent != 0 && parent != std::process::id());
        assert!(proc_info(0x7fff_fff0).is_none());
    }

    /// 宿主转给桌面端的招呼：认出来的浏览器为准，认不出时用插件自己报的
    #[tokio::test]
    async fn hello_carries_detected_browser() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        let core = crate::Core::open(crate::Options { paths: paths.clone(), sidecar_dir: None, restore: false }).await.unwrap();
        crate::api::serve(core.clone(), std::sync::Arc::new(|_| {})).await.unwrap();
        let hello = json!({"type": "hello", "version": "0.4.0", "browser": "chrome"});
        let host = Host { paths: paths.clone(), sidecar_dir: None, browser: Some("arc"), stale: Default::default(), busy: Default::default() };
        let r = host.handle("hello", &hello).await.unwrap();
        assert_eq!(r["app"], true);
        // 回复里带着正在运行的 Captae 的版本；和宿主同一个版本，宿主不用退出
        assert_eq!(r["appVersion"], env!("CARGO_PKG_VERSION"));
        assert!(!host.stale.load(std::sync::atomic::Ordering::SeqCst));
        let s = core.extension_status();
        assert!(s.connected);
        assert_eq!((s.browser.as_deref(), s.version.as_deref()), (Some("arc"), Some("0.4.0")));
        let host = Host { browser: None, ..host };
        host.handle("hello", &hello).await.unwrap();
        assert_eq!(core.extension_status().browser.as_deref(), Some("chrome"));
        // 桌面端自己测宿主：不算插件来过，不改记下的浏览器
        let host = Host { browser: Some("edge"), ..host };
        host.handle("hello", &json!({"type": "hello", "test": true})).await.unwrap();
        assert_eq!(core.extension_status().browser.as_deref(), Some("chrome"));
    }

    #[test]
    fn cli_next_to_exe() {
        let dir = tempfile::tempdir().unwrap();
        let app = dir.path().join("captae-desktop");
        assert!(cli_next_to(&app).is_none());
        let cli = dir.path().join(if cfg!(windows) { "captae.exe" } else { "captae" });
        std::fs::write(&cli, b"").unwrap();
        assert_eq!(cli_next_to(&app), Some(cli));
    }

    #[test]
    fn check_finds_downloads() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        paths.ensure().unwrap();
        let lib = Library::memory().unwrap();
        let file = paths.movies.join("v.mp4");
        std::fs::write(&file, b"x").unwrap();
        let id = lib
            .add(&crate::library::NewAsset { kind: "video".into(), title: "V".into(), source_url: "https://a.com/720.m3u8".into(), page_url: "https://a.com/watch/1".into(), path: file, ..Default::default() })
            .unwrap();
        let r = check(&lib, &paths, &CheckReq { urls: vec!["https://a.com/1080.m3u8".into(), "https://a.com/720.m3u8".into()], page_url: None }).unwrap();
        let movies = if cfg!(target_os = "macos") { "影片 › Captae" } else { "视频 › Captae" };
        assert_eq!((r["found"].as_bool(), r["assetId"].as_i64(), r["where"].as_str()), (Some(true), Some(id), Some(movies)));
        let r = check(&lib, &paths, &CheckReq { urls: vec!["https://a.com/other.mp4".into()], page_url: Some("https://a.com/watch/1".into()) }).unwrap();
        assert_eq!(r["assetId"].as_i64(), Some(id));
        let r = check(&lib, &paths, &CheckReq { urls: vec!["https://a.com/other.mp4".into()], page_url: None }).unwrap();
        assert_eq!(r["found"], false);
    }
}

#[cfg(test)]
mod find_by_url_tests {
    use crate::library::*;
    #[test]
    fn find_by_url() {
        let dir = tempfile::tempdir().unwrap();
        let f = |n: &str| {
            let p = dir.path().join(n);
            std::fs::write(&p, b"x").unwrap();
            p
        };
        let l = Library::memory().unwrap();
        let v = f("v.mp4");
        let id = l
            .add(&NewAsset { kind: "video".into(), title: "V".into(), source_url: "https://a.com/v.m3u8".into(), page_url: "https://a.com/watch/1".into(), path: v.clone(), ..Default::default() })
            .unwrap();
        // 转换产物带着同样的来源，但原始下载优先
        l.add(&NewAsset { kind: "audio".into(), title: "V mp3".into(), source_url: "https://a.com/v.m3u8".into(), path: f("v.mp3"), parent_id: Some(id), ..Default::default() }).unwrap();
        // 同一页面上的图片不算这个视频
        l.add(&NewAsset { kind: "image".into(), title: "img".into(), source_url: "https://a.com/i.jpg".into(), page_url: "https://a.com/watch/2".into(), path: f("i.jpg"), ..Default::default() }).unwrap();
        assert_eq!(l.find_by_url("https://a.com/v.m3u8#t=3", false).unwrap().map(|a| a.id), Some(id));
        assert_eq!(l.find_by_url("https://a.com/watch/1", false).unwrap().map(|a| a.id), None);
        assert_eq!(l.find_by_url("https://a.com/watch/1", true).unwrap().map(|a| a.id), Some(id));
        assert!(l.find_by_url("https://a.com/watch/2", true).unwrap().is_none());
        assert!(l.find_by_url("", true).unwrap().is_none());
        // 文件删掉了就当没下过
        std::fs::remove_file(&v).unwrap();
        let left = l.find_by_url("https://a.com/v.m3u8", false).unwrap().unwrap();
        assert_eq!(left.kind, "audio");
    }
}
