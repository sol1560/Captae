//! 网络：代理（跟随系统 / 不用 / 手动）和共用的 HTTP 客户端。
//!
//! reqwest 自带的「读系统代理」只读 HTTP、HTTPS 两项，不认系统设置里「忽略这些主机」的例外列表，
//! 也不会自动绕过 localhost；aria2c、ffmpeg 根本不读系统代理。只开系统代理、没开增强模式 / TUN 的用户，
//! M3U8 和直链就会连不上（DIAGNOSIS 第 6 节）。所以这里自己读系统设置（macOS `scutil --proxy`；Windows 注册表
//! Internet Settings；Linux 环境变量、GNOME 的 gsettings、KDE 的 kioslaverc），
//! 算出「实际要用的代理」，reqwest、aria2c、ffmpeg、yt-dlp 都用同一份：
//! - reqwest：`Proxy::http/https` + 例外列表
//! - aria2c：`--all-proxy` + `--no-proxy`（aria2 只支持 HTTP 代理）
//! - ffmpeg / ffprobe / yt-dlp：`http_proxy`、`https_proxy`、`no_proxy` 环境变量
//! - yt-dlp 手动模式另加 `--proxy`，「不用代理」时加 `--proxy ""`
//!
//! 设置里手动填的代理优先于系统代理。

use crate::config::Settings;
use serde::Serialize;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// 共用的 HTTP 客户端类型（命令行等不直接依赖 reqwest 的地方用这个名字）
pub use reqwest::Client as HttpClient;

/// 和 Chrome 一样的 UA：有的网站对陌生 UA 直接 403。
pub const UA: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/129.0 Safari/537.36";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// 跟随系统
    System,
    /// 不用代理
    Off,
    /// 手动指定
    Manual,
}

impl Mode {
    pub fn parse(s: &str) -> Mode {
        match s.trim() {
            "none" | "off" | "direct" => Mode::Off,
            "manual" => Mode::Manual,
            _ => Mode::System,
        }
    }
}

/// 系统设置里的代理（macOS `scutil --proxy`、Windows 注册表、Linux 环境变量 / GNOME / KDE 的结果）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct SystemProxy {
    /// "127.0.0.1:6152"
    pub http: Option<String>,
    pub https: Option<String>,
    pub socks: Option<String>,
    /// 不走代理的主机（系统设置里「忽略这些主机与域的代理设置」）
    pub exceptions: Vec<String>,
    /// 开了「自动代理配置（PAC）」：Captae 算不了 PAC，按直连处理
    pub pac: bool,
}

/// 解析 `scutil --proxy` 的输出。
pub fn parse_scutil(text: &str) -> SystemProxy {
    let mut kv = std::collections::HashMap::new();
    let mut exceptions = vec![];
    let mut in_exc = false;
    for l in text.lines().map(str::trim) {
        if l.starts_with("ExceptionsList") {
            in_exc = true;
            continue;
        }
        if in_exc {
            if l.starts_with('}') {
                in_exc = false;
            } else if let Some((_, v)) = l.split_once(" : ") {
                exceptions.push(v.trim().to_string());
            }
            continue;
        }
        if let Some((k, v)) = l.split_once(" : ") {
            kv.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    let on = |k: &str| kv.get(k).map(|v| v == "1").unwrap_or(false);
    let hp = |enable: &str, host: &str, port: &str| -> Option<String> {
        if !on(enable) {
            return None;
        }
        let h = kv.get(host).filter(|h| !h.is_empty())?;
        Some(match kv.get(port) {
            Some(p) if !p.is_empty() && p != "0" => format!("{h}:{p}"),
            _ => h.clone(),
        })
    };
    SystemProxy {
        http: hp("HTTPEnable", "HTTPProxy", "HTTPPort"),
        https: hp("HTTPSEnable", "HTTPSProxy", "HTTPSPort"),
        socks: hp("SOCKSEnable", "SOCKSProxy", "SOCKSPort"),
        exceptions,
        pac: on("ProxyAutoConfigEnable"),
    }
}

#[cfg(target_os = "macos")]
fn read_system() -> Option<SystemProxy> {
    let out = std::process::Command::new("/usr/sbin/scutil").arg("--proxy").stdin(std::process::Stdio::null()).output().ok()?;
    out.status.success().then(|| parse_scutil(&String::from_utf8_lossy(&out.stdout)))
}

/// Windows：读「设置 › 网络 › 代理」（`HKCU\Software\Microsoft\Windows\CurrentVersion\Internet Settings`）。
/// 没开手动代理时退回环境变量（命令行用户常这样设），都没有就是直连
#[cfg(windows)]
fn read_system() -> Option<SystemProxy> {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let env = from_env(|k| std::env::var(k).ok());
    let Ok(k) = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Internet Settings") else { return Some(env) };
    let enable: u32 = k.get_value("ProxyEnable").unwrap_or(0);
    let server: String = k.get_value("ProxyServer").unwrap_or_default();
    let bypass: String = k.get_value("ProxyOverride").unwrap_or_default();
    let pac: String = k.get_value("AutoConfigURL").unwrap_or_default();
    let sys = parse_windows(enable != 0, &server, &bypass, &pac);
    if sys.http.is_none() && sys.https.is_none() && sys.socks.is_none() && (env.http.is_some() || env.https.is_some() || env.socks.is_some()) {
        return Some(SystemProxy { pac: sys.pac, ..env });
    }
    Some(sys)
}

/// Linux：先看环境变量（`http_proxy` 这些，终端、很多发行版的登录脚本会设）；没有再读桌面的设置：
/// KDE 桌面先读 KDE 的（`~/.config/kioslaverc`），别的桌面先读 GNOME 的（`gsettings`，Ubuntu、Fedora 默认的桌面），
/// gsettings 不在就算了
#[cfg(all(unix, not(target_os = "macos")))]
fn read_system() -> Option<SystemProxy> {
    let env = from_env(|k| std::env::var(k).ok());
    if env.http.is_some() || env.https.is_some() || env.socks.is_some() {
        return Some(env);
    }
    let kde_first = std::env::var("XDG_CURRENT_DESKTOP").is_ok_and(|d| d.to_ascii_uppercase().contains("KDE"));
    let gnome = || {
        let out = std::process::Command::new("gsettings").args(["list-recursively", "org.gnome.system.proxy"]).stdin(std::process::Stdio::null()).stderr(std::process::Stdio::null()).output().ok()?;
        out.status.success().then(|| parse_gsettings(&String::from_utf8_lossy(&out.stdout))).flatten()
    };
    let kde = || {
        let home = crate::util::home_dir();
        let cfg = std::env::var_os("XDG_CONFIG_HOME").map(std::path::PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".config"));
        parse_kioslaverc(&std::fs::read_to_string(cfg.join("kioslaverc")).ok()?)
    };
    let found = if kde_first { kde().or_else(gnome) } else { gnome().or_else(kde) };
    Some(found.unwrap_or(env))
}

#[cfg(not(any(unix, windows)))]
fn read_system() -> Option<SystemProxy> {
    Some(from_env(|k| std::env::var(k).ok()))
}

/// 「http://127.0.0.1:7890/」→「127.0.0.1:7890」（系统代理里只记主机和端口，协议另外按种类加）
fn host_port(u: &str) -> String {
    let u = u.trim();
    let u = u.split_once("://").map(|(_, r)| r).unwrap_or(u);
    u.trim_end_matches('/').to_string()
}

/// 环境变量里的代理：`http_proxy`、`https_proxy`、`all_proxy`（socks 开头的当 socks）、`no_proxy`，大小写都认
pub fn from_env(get: impl Fn(&str) -> Option<String>) -> SystemProxy {
    let env = |ks: &[&str]| ks.iter().find_map(|k| get(k).filter(|v| !v.trim().is_empty()));
    let all = env(&["all_proxy", "ALL_PROXY"]);
    let all_socks = all.as_deref().is_some_and(|a| a.trim().to_ascii_lowercase().starts_with("socks"));
    let (all_http, socks) = match all {
        Some(a) if all_socks => (None, Some(host_port(&a))),
        Some(a) => (Some(host_port(&a)), None),
        None => (None, None),
    };
    SystemProxy {
        http: env(&["http_proxy", "HTTP_PROXY"]).map(|u| host_port(&u)).or_else(|| all_http.clone()),
        https: env(&["https_proxy", "HTTPS_PROXY"]).map(|u| host_port(&u)).or(all_http),
        socks,
        exceptions: env(&["no_proxy", "NO_PROXY"]).map(|v| v.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()).unwrap_or_default(),
        pac: false,
    }
}

/// Windows 的代理设置。`server` 有两种写法：`127.0.0.1:7890`（所有协议用同一个），或者按协议分开的
/// `http=127.0.0.1:7890;https=127.0.0.1:7890;socks=127.0.0.1:7891`（「socks=」是 SOCKS，按 socks5 用）。
/// `bypass`（ProxyOverride）用分号隔开：`<local>` 表示不带点的主机名，这里认不了，跳过；`127.*`、`192.168.*` 换成网段写法。
/// `pac` 是 AutoConfigURL（PAC 脚本地址），Captae 算不了 PAC，按直连处理（和 macOS 一样）
pub fn parse_windows(enable: bool, server: &str, bypass: &str, pac: &str) -> SystemProxy {
    let mut p = SystemProxy { pac: !pac.trim().is_empty(), ..Default::default() };
    if !enable || server.trim().is_empty() {
        return p;
    }
    if server.contains('=') {
        for part in server.split(';').map(str::trim).filter(|x| !x.is_empty()) {
            let Some((k, v)) = part.split_once('=') else { continue };
            let v = host_port(v);
            if v.is_empty() {
                continue;
            }
            match k.trim().to_ascii_lowercase().as_str() {
                "http" => p.http = Some(v),
                "https" => p.https = Some(v),
                "socks" => p.socks = Some(v),
                _ => {}
            }
        }
    } else {
        let v = host_port(server.split(';').next().unwrap_or(""));
        p.http = Some(v.clone());
        p.https = Some(v);
    }
    p.exceptions = bypass.split(';').map(str::trim).filter(|e| !e.is_empty() && *e != "<local>").map(wildcard_net).collect();
    p
}

/// `127.*` → `127.0.0.0/8`、`192.168.*` → `192.168.0.0/16`（reqwest、aria2 认网段，不认这种通配）；别的原样
fn wildcard_net(e: &str) -> String {
    let Some(head) = e.strip_suffix(".*") else { return e.to_string() };
    let parts: Vec<&str> = head.split('.').collect();
    if parts.is_empty() || parts.len() > 3 || !parts.iter().all(|x| x.parse::<u8>().is_ok()) {
        return e.to_string();
    }
    let mut oct: Vec<&str> = parts.clone();
    oct.resize(4, "0");
    format!("{}/{}", oct.join("."), parts.len() * 8)
}

/// GNOME 的代理设置（`gsettings list-recursively org.gnome.system.proxy` 的输出）。
/// mode 是 none 时返回直连；auto（自动配置）当 PAC；manual 读各协议的主机、端口和 ignore-hosts。
/// 没有 mode 这一行（输出不对）时 None
pub fn parse_gsettings(text: &str) -> Option<SystemProxy> {
    let mut kv = std::collections::HashMap::new();
    for l in text.lines() {
        let mut it = l.trim().splitn(3, ' ');
        let (Some(schema), Some(key), Some(val)) = (it.next(), it.next(), it.next()) else { continue };
        kv.insert(format!("{schema} {key}"), val.trim().to_string());
    }
    let unq = |v: &str| v.trim().trim_matches('\'').to_string();
    let mode = unq(kv.get("org.gnome.system.proxy mode")?);
    let mut p = SystemProxy::default();
    match mode.as_str() {
        "manual" => {}
        "auto" => {
            p.pac = true;
            return Some(p);
        }
        _ => return Some(p),
    }
    let hp = |proto: &str| -> Option<String> {
        let host = unq(kv.get(&format!("org.gnome.system.proxy.{proto} host"))?);
        if host.is_empty() {
            return None;
        }
        // 端口：「8080」或「uint32 8080」
        let port = kv.get(&format!("org.gnome.system.proxy.{proto} port")).and_then(|v| v.split_whitespace().last()).and_then(|v| v.parse::<u32>().ok()).filter(|p| *p > 0);
        Some(match port {
            Some(port) => format!("{}:{port}", host_port(&host)),
            None => host_port(&host),
        })
    };
    p.http = hp("http");
    p.https = hp("https");
    p.socks = hp("socks");
    if let Some(list) = kv.get("org.gnome.system.proxy ignore-hosts") {
        p.exceptions = list.trim_start_matches("@as").trim().trim_start_matches('[').trim_end_matches(']').split(',').map(unq).filter(|x| !x.is_empty()).collect();
    }
    Some(p)
}

/// KDE 的代理设置（`~/.config/kioslaverc` 的 [Proxy Settings]）。ProxyType：0 直连、1 手动、2 PAC 脚本、3 自动发现（当 PAC）、
/// 4 用环境变量（上面已经读过环境变量了，这里当直连）。地址写成 `http://127.0.0.1 8080`（空格隔开端口）或 `http://127.0.0.1:8080`。
/// 没有 [Proxy Settings] 时 None
pub fn parse_kioslaverc(text: &str) -> Option<SystemProxy> {
    let mut in_sec = false;
    let mut kv = std::collections::HashMap::new();
    for l in text.lines().map(str::trim) {
        if l.starts_with('[') {
            in_sec = l == "[Proxy Settings]";
            continue;
        }
        if in_sec {
            if let Some((k, v)) = l.split_once('=') {
                kv.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    if kv.is_empty() {
        return None;
    }
    let mut p = SystemProxy::default();
    match kv.get("ProxyType").map(String::as_str) {
        Some("1") => {}
        Some("2") | Some("3") => {
            p.pac = true;
            return Some(p);
        }
        _ => return Some(p),
    }
    let hp = |k: &str| -> Option<String> {
        let v = kv.get(k)?.trim();
        if v.is_empty() {
            return None;
        }
        let v = host_port(v);
        Some(match v.split_once(' ') {
            Some((h, port)) if !port.trim().is_empty() && port.trim() != "0" => format!("{h}:{}", port.trim()),
            Some((h, _)) => h.to_string(),
            None => v,
        })
    };
    p.http = hp("httpProxy");
    p.https = hp("httpsProxy");
    p.socks = hp("socksProxy");
    // ReversedException=true 是「只有这些走代理」，Captae 表达不了，不加例外
    if kv.get("ReversedException").map(String::as_str) != Some("true") {
        p.exceptions = kv.get("NoProxyFor").map(|v| v.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect()).unwrap_or_default();
    }
    Some(p)
}

type SysCache = Mutex<Option<(Instant, Option<SystemProxy>)>>;
static SYS: once_cell::sync::Lazy<SysCache> = once_cell::sync::Lazy::new(|| Mutex::new(None));
const SYS_TTL: Duration = Duration::from_secs(30);

/// 系统代理（30 秒内复用上一次读到的结果；读不了时 None）。
pub fn system_proxy() -> Option<SystemProxy> {
    let mut g = SYS.lock().unwrap();
    if let Some((t, v)) = g.as_ref() {
        if t.elapsed() < SYS_TTL {
            return v.clone();
        }
    }
    let v = read_system();
    *g = Some((Instant::now(), v.clone()));
    v
}

/// 实际要用的代理。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Effective {
    /// system / manual / none
    pub mode: &'static str,
    /// 给 http:// 地址用的代理：`http://host:port` 或 `socks5://host:port`；None = 直连
    pub http: Option<String>,
    /// 给 https:// 地址用的代理
    pub https: Option<String>,
    /// 不走代理的主机（已经把 `*.local` 换成 `.local`，并加上 localhost）
    #[serde(rename = "noProxy")]
    pub no_proxy: Vec<String>,
    /// 系统代理读不出来（scutil 出错这类）：交给 reqwest 自己读
    #[serde(skip)]
    pub unknown: bool,
    /// 系统开了 PAC，Captae 按直连处理
    pub pac: bool,
}

impl Effective {
    pub fn direct() -> Effective {
        Effective { mode: "none", http: None, https: None, no_proxy: vec![], unknown: false, pac: false }
    }
    /// aria2c / ffmpeg 能用的 HTTP 代理（它们不支持 socks）
    pub fn http_only(&self) -> Option<&str> {
        self.https.as_deref().or(self.http.as_deref()).filter(|u| u.starts_with("http://") || u.starts_with("https://"))
    }
    pub fn no_proxy_list(&self) -> String {
        self.no_proxy.join(",")
    }
    /// 有没有在走代理（给界面显示）
    pub fn active(&self) -> bool {
        self.http.is_some() || self.https.is_some()
    }
}

/// 「127.0.0.1:6152」→「http://127.0.0.1:6152」；已经带协议的原样返回。
fn with_scheme(s: &str, scheme: &str) -> String {
    if s.contains("://") {
        s.to_string()
    } else {
        format!("{scheme}://{s}")
    }
}

fn normalize_exceptions(list: &[String]) -> Vec<String> {
    let mut out: Vec<String> = list
        .iter()
        .map(|e| e.trim())
        .filter(|e| !e.is_empty() && *e != "*")
        .map(|e| e.strip_prefix('*').unwrap_or(e).to_string())
        .collect();
    for d in ["localhost", "127.0.0.1", "::1"] {
        if !out.iter().any(|x| x == d) {
            out.push(d.to_string());
        }
    }
    out
}

/// 按设置算出实际要用的代理。
pub fn effective(s: &Settings) -> Effective {
    effective_from(&s.proxy_mode, &s.proxy_url)
}

pub fn effective_from(mode: &str, manual: &str) -> Effective {
    match Mode::parse(mode) {
        Mode::Off => Effective::direct(),
        Mode::Manual => {
            let u = manual.trim();
            if u.is_empty() {
                return Effective { mode: "manual", ..Effective::direct() };
            }
            let u = with_scheme(u, "http");
            Effective { mode: "manual", http: Some(u.clone()), https: Some(u), no_proxy: normalize_exceptions(&[]), unknown: false, pac: false }
        }
        Mode::System => match system_proxy() {
            None => Effective { mode: "system", unknown: true, ..Effective::direct() },
            Some(sp) => {
                let socks = sp.socks.as_deref().map(|x| with_scheme(x, "socks5"));
                let http = sp.http.as_deref().map(|x| with_scheme(x, "http")).or_else(|| socks.clone());
                let https = sp.https.as_deref().map(|x| with_scheme(x, "http")).or_else(|| socks.clone());
                Effective { mode: "system", http, https, no_proxy: normalize_exceptions(&sp.exceptions), unknown: false, pac: sp.pac }
            }
        },
    }
}

/// 共用的 HTTP 客户端（M3U8、直链 HEAD、自动更新、翻译接口）。
pub fn build_client(s: &Settings) -> reqwest::Client {
    build_with(&effective(s))
}

pub fn build_with(eff: &Effective) -> reqwest::Client {
    let mut b = reqwest::Client::builder().user_agent(UA).connect_timeout(Duration::from_secs(15));
    if eff.unknown {
        // 读不到系统代理：交给 reqwest 自己读（macos-system-configuration / 环境变量）
    } else if !eff.active() {
        b = b.no_proxy();
    } else {
        let np = || reqwest::NoProxy::from_string(&eff.no_proxy_list());
        if let Some(Ok(p)) = eff.http.as_deref().map(reqwest::Proxy::http) {
            b = b.proxy(p.no_proxy(np()));
        }
        if let Some(Ok(p)) = eff.https.as_deref().map(reqwest::Proxy::https) {
            b = b.proxy(p.no_proxy(np()));
        }
    }
    #[cfg(any(target_os = "ios", target_os = "android"))]
    if let Some(tls) = platform_tls() {
        b = b.use_preconfigured_tls(tls);
    }
    b.build().unwrap_or_else(|_| reqwest::Client::new())
}

// ------------------------------------------------------------------ 手机上的证书校验

/// Android 上系统的证书校验要先在 Kotlin 那边初始化（JNI 拿到 Context，见 crates/captae-mobile 的 CaptaeTls）。
/// 初始化过才用；没初始化过（比如在 adb shell 里直接跑的冒烟程序，没有 JVM）退回编进程序的 Mozilla 根证书（webpki-roots），
/// 不然一校验就 panic。iOS 不用初始化，一直用系统的
#[cfg(target_os = "android")]
static PLATFORM_TLS_READY: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Android：系统证书校验初始化好了（captae-mobile 的 JNI 入口调）。要在建 Core 之前调，Core 建好以后不会换掉已有的客户端
#[cfg(target_os = "android")]
pub fn set_platform_tls_ready() {
    PLATFORM_TLS_READY.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// 现在用的是哪种证书校验：platform（系统的）/ webpki（编进程序的 Mozilla 根证书）。排错、冒烟测试打印用
pub fn tls_kind() -> &'static str {
    #[cfg(target_os = "ios")]
    return "platform";
    #[cfg(target_os = "android")]
    return if PLATFORM_TLS_READY.load(std::sync::atomic::Ordering::SeqCst) { "platform" } else { "webpki" };
    #[cfg(not(any(target_os = "ios", target_os = "android")))]
    return "webpki";
}

/// 用系统证书校验的 rustls 配置。和 reqwest 0.12 自己建的一样用 ring，只换掉证书校验
#[cfg(any(target_os = "ios", target_os = "android"))]
fn platform_tls() -> Option<rustls::ClientConfig> {
    use rustls_platform_verifier::BuilderVerifierExt;
    if tls_kind() != "platform" {
        return None;
    }
    let provider = std::sync::Arc::new(rustls::crypto::ring::default_provider());
    let r = rustls::ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions().map_err(|e| e.to_string()).and_then(|b| b.with_platform_verifier().map_err(|e| e.to_string()));
    match r {
        Ok(b) => Some(b.with_no_client_auth()),
        Err(e) => {
            tracing::warn!("系统证书校验用不了，退回编进程序的根证书：{e}");
            None
        }
    }
}

/// 子进程（yt-dlp、ffmpeg、ffprobe、aria2c）的代理环境变量。「不用代理」时清掉继承来的。
pub fn apply_env(c: &mut tokio::process::Command, eff: &Effective) {
    if eff.unknown {
        return;
    }
    for k in ["http_proxy", "HTTP_PROXY", "https_proxy", "HTTPS_PROXY", "all_proxy", "ALL_PROXY", "no_proxy", "NO_PROXY"] {
        c.env_remove(k);
    }
    if let Some(h) = &eff.http {
        c.env("http_proxy", h);
    }
    if let Some(h) = &eff.https {
        c.env("https_proxy", h);
    }
    if eff.active() {
        c.env("no_proxy", eff.no_proxy_list());
    }
}

/// aria2c 的代理参数。aria2 不支持 socks，系统只开了 socks 时不加。
pub fn aria2_args(eff: &Effective) -> Vec<String> {
    if eff.unknown {
        return vec![];
    }
    match eff.http_only() {
        Some(p) => vec![format!("--all-proxy={p}"), format!("--no-proxy={}", eff.no_proxy_list())],
        // 不用代理：明确告诉 aria2 别用（它会读 http_proxy 等环境变量）
        None => vec!["--all-proxy=".into()],
    }
}

/// yt-dlp 的代理参数：手动 → `--proxy 地址`；不用 → `--proxy ""`；跟随系统 → 不加（环境变量 + 它自己读系统设置）。
pub fn ytdlp_args(eff: &Effective) -> Vec<String> {
    match eff.mode {
        "manual" => eff.https.clone().map(|u| vec!["--proxy".to_string(), u]).unwrap_or_default(),
        "none" => vec!["--proxy".into(), String::new()],
        _ => vec![],
    }
}

/// 设置页和命令行用：现在走的是什么代理。
pub fn status(s: &Settings) -> serde_json::Value {
    let eff = effective(s);
    serde_json::json!({
        "mode": eff.mode,
        "http": eff.http,
        "https": eff.https,
        "noProxy": eff.no_proxy,
        "pac": eff.pac,
        "system": system_proxy(),
        "aria2": eff.http_only().is_some() || !eff.active(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // 实测：本机（Surge）`scutil --proxy` 的输出
    const SURGE: &str = "<dictionary> {
  ExceptionsList : <array> {
    0 : 127.0.0.1
    1 : 192.168.0.0/16
    2 : 10.0.0.0/8
    3 : 172.16.0.0/12
    4 : 100.64.0.0/10
    5 : localhost
    6 : *.local
    7 : *.ts.net
  }
  ExcludeSimpleHostnames : 1
  FTPPassive : 1
  HTTPEnable : 1
  HTTPPort : 6152
  HTTPProxy : 127.0.0.1
  HTTPSEnable : 1
  HTTPSPort : 6152
  HTTPSProxy : 127.0.0.1
  ProxyAutoConfigEnable : 0
  ProxyAutoDiscoveryEnable : 0
  SOCKSEnable : 0
}";

    #[test]
    fn scutil() {
        let p = parse_scutil(SURGE);
        assert_eq!(p.http.as_deref(), Some("127.0.0.1:6152"));
        assert_eq!(p.https.as_deref(), Some("127.0.0.1:6152"));
        assert_eq!(p.socks, None);
        assert_eq!(p.exceptions.len(), 8);
        assert_eq!(p.exceptions[6], "*.local");
        assert!(!p.pac);
        // 没开代理
        let off = parse_scutil("<dictionary> {\n  HTTPEnable : 0\n  HTTPSEnable : 0\n}");
        assert_eq!(off, SystemProxy::default());
        // 只开了 socks
        let s = parse_scutil("<dictionary> {\n  SOCKSEnable : 1\n  SOCKSPort : 1080\n  SOCKSProxy : 10.0.0.2\n}");
        assert_eq!(s.socks.as_deref(), Some("10.0.0.2:1080"));
    }

    #[test]
    fn modes() {
        let manual = Settings { proxy_mode: "manual".into(), proxy_url: "127.0.0.1:7890".into(), ..Default::default() };
        let e = effective(&manual);
        assert_eq!(e.https.as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(aria2_args(&e), vec!["--all-proxy=http://127.0.0.1:7890".to_string(), "--no-proxy=localhost,127.0.0.1,::1".to_string()]);
        assert_eq!(ytdlp_args(&e), vec!["--proxy".to_string(), "http://127.0.0.1:7890".to_string()]);
        let socks = Settings { proxy_mode: "manual".into(), proxy_url: "socks5://127.0.0.1:1080".into(), ..Default::default() };
        let e = effective(&socks);
        // aria2 不支持 socks：不给它代理（也不强制直连）
        assert_eq!(e.http_only(), None);
        assert_eq!(aria2_args(&e), vec!["--all-proxy=".to_string()]);
        assert_eq!(ytdlp_args(&e)[1], "socks5://127.0.0.1:1080");
        let off = Settings { proxy_mode: "none".into(), ..Default::default() };
        let e = effective(&off);
        assert!(!e.active());
        assert_eq!(ytdlp_args(&e), vec!["--proxy".to_string(), String::new()]);
        assert_eq!(aria2_args(&e), vec!["--all-proxy=".to_string()]);
        // 手动但没填地址：等于直连
        assert!(!effective(&Settings { proxy_mode: "manual".into(), ..Default::default() }).active());
    }

    #[test]
    fn exceptions_normalized() {
        let ex = normalize_exceptions(&["*.local".into(), "192.168.0.0/16".into(), "localhost".into(), "*".into()]);
        assert_eq!(ex, vec![".local", "192.168.0.0/16", "localhost", "127.0.0.1", "::1"]);
    }

    /// 打印全部环境变量的命令（unix 的 env，Windows 的 cmd /c set）
    fn env_cmd() -> tokio::process::Command {
        if cfg!(windows) {
            let mut c = tokio::process::Command::new("cmd");
            c.args(["/c", "set"]);
            c
        } else {
            tokio::process::Command::new("/usr/bin/env")
        }
    }

    #[tokio::test]
    async fn env_for_children() {
        let e = Effective { mode: "system", http: Some("http://127.0.0.1:6152".into()), https: Some("http://127.0.0.1:6152".into()), no_proxy: vec![".local".into(), "localhost".into()], unknown: false, pac: false };
        let mut c = env_cmd();
        apply_env(&mut c, &e);
        let out = c.output().await.unwrap();
        let s = String::from_utf8_lossy(&out.stdout);
        assert!(s.contains("https_proxy=http://127.0.0.1:6152"), "{s}");
        assert!(s.contains("no_proxy=.local,localhost"), "{s}");
        // 不用代理：继承来的环境变量也清掉
        let mut c = env_cmd();
        c.env("HTTPS_PROXY", "http://x:1");
        apply_env(&mut c, &Effective::direct());
        let s = String::from_utf8_lossy(&c.output().await.unwrap().stdout).to_string();
        assert!(!s.to_lowercase().contains("https_proxy"), "{s}");
    }

    #[test]
    fn windows_settings() {
        // 所有协议用同一个（Clash、v2rayN 这样写）；例外里的 <local> 认不了、跳过，127.* 换成网段
        let p = parse_windows(true, "127.0.0.1:7890", "localhost;127.*;10.*;172.16.*;192.168.*;*.local;<local>", "");
        assert_eq!((p.http.as_deref(), p.https.as_deref(), p.socks.as_deref()), (Some("127.0.0.1:7890"), Some("127.0.0.1:7890"), None));
        assert_eq!(p.exceptions, vec!["localhost", "127.0.0.0/8", "10.0.0.0/8", "172.16.0.0/16", "192.168.0.0/16", "*.local"]);
        assert!(!p.pac);
        // 按协议分开写
        let p = parse_windows(true, "http=127.0.0.1:8080;https=127.0.0.1:8443;ftp=x:21;socks=127.0.0.1:1080", "", "");
        assert_eq!((p.http.as_deref(), p.https.as_deref(), p.socks.as_deref()), (Some("127.0.0.1:8080"), Some("127.0.0.1:8443"), Some("127.0.0.1:1080")));
        // 只有 socks：http、https 都走 socks（effective 里按 socks5）
        let p = parse_windows(true, "socks=127.0.0.1:1080", "", "");
        assert_eq!((p.http, p.socks.as_deref()), (None, Some("127.0.0.1:1080")));
        // 有的软件写成带协议的
        assert_eq!(parse_windows(true, "http://127.0.0.1:7890/", "", "").https.as_deref(), Some("127.0.0.1:7890"));
        // 关着：直连（地址还留在注册表里也不用）
        assert_eq!(parse_windows(false, "127.0.0.1:7890", "localhost", ""), SystemProxy::default());
        // 只开了「使用设置脚本」（PAC）
        let p = parse_windows(false, "", "", "http://wpad/proxy.pac");
        assert!(p.pac && p.http.is_none());
    }

    #[test]
    fn gnome_settings() {
        // Ubuntu 22.04 上 `gsettings list-recursively org.gnome.system.proxy` 的样子
        let manual = "org.gnome.system.proxy use-same-proxy true
org.gnome.system.proxy mode 'manual'
org.gnome.system.proxy autoconfig-url ''
org.gnome.system.proxy ignore-hosts ['localhost', '127.0.0.0/8', '::1', '*.local']
org.gnome.system.proxy.ftp host ''
org.gnome.system.proxy.ftp port 0
org.gnome.system.proxy.socks host '127.0.0.1'
org.gnome.system.proxy.socks port 7891
org.gnome.system.proxy.http host '127.0.0.1'
org.gnome.system.proxy.http port 7890
org.gnome.system.proxy.http use-authentication false
org.gnome.system.proxy.http enabled false
org.gnome.system.proxy.https host '127.0.0.1'
org.gnome.system.proxy.https port 7890
";
        let p = parse_gsettings(manual).unwrap();
        assert_eq!((p.http.as_deref(), p.https.as_deref(), p.socks.as_deref()), (Some("127.0.0.1:7890"), Some("127.0.0.1:7890"), Some("127.0.0.1:7891")));
        assert_eq!(p.exceptions, vec!["localhost", "127.0.0.0/8", "::1", "*.local"]);
        // 没开：直连
        let none = manual.replace("mode 'manual'", "mode 'none'");
        assert_eq!(parse_gsettings(&none).unwrap(), SystemProxy::default());
        // 自动：当 PAC
        assert!(parse_gsettings(&manual.replace("mode 'manual'", "mode 'auto'")).unwrap().pac);
        // 手动但没填 https、例外是空列表
        let p = parse_gsettings("org.gnome.system.proxy mode 'manual'\norg.gnome.system.proxy ignore-hosts @as []\norg.gnome.system.proxy.http host 'proxy.lan'\norg.gnome.system.proxy.http port 3128\norg.gnome.system.proxy.https host ''\norg.gnome.system.proxy.https port 0\n").unwrap();
        assert_eq!((p.http.as_deref(), p.https.as_deref(), p.exceptions.len()), (Some("proxy.lan:3128"), None, 0));
        // 输出不对
        assert!(parse_gsettings("No such schema").is_none());
    }

    #[test]
    fn kde_settings() {
        let rc = "[$Version]\nupdate_info=kioslave.upd:change-proxy-types\n\n[Proxy Settings]\nNoProxyFor=localhost,127.0.0.1,.lan\nProxyType=1\nReversedException=false\nhttpProxy=http://127.0.0.1 7890\nhttpsProxy=http://127.0.0.1:7890\nsocksProxy=socks://127.0.0.1 7891\n";
        let p = parse_kioslaverc(rc).unwrap();
        assert_eq!((p.http.as_deref(), p.https.as_deref(), p.socks.as_deref()), (Some("127.0.0.1:7890"), Some("127.0.0.1:7890"), Some("127.0.0.1:7891")));
        assert_eq!(p.exceptions, vec!["localhost", "127.0.0.1", ".lan"]);
        assert_eq!(parse_kioslaverc(&rc.replace("ProxyType=1", "ProxyType=0")).unwrap(), SystemProxy::default());
        assert!(parse_kioslaverc(&rc.replace("ProxyType=1", "ProxyType=2")).unwrap().pac);
        // 「只有这些走代理」：例外不加
        assert!(parse_kioslaverc(&rc.replace("ReversedException=false", "ReversedException=true")).unwrap().exceptions.is_empty());
        assert!(parse_kioslaverc("[General]\nx=1\n").is_none());
    }

    #[test]
    fn env_proxy() {
        let vars = |pairs: &'static [(&'static str, &'static str)]| move |k: &str| pairs.iter().find(|(n, _)| *n == k).map(|(_, v)| v.to_string());
        let p = from_env(vars(&[("https_proxy", "http://127.0.0.1:7890/"), ("HTTP_PROXY", "127.0.0.1:7890"), ("no_proxy", "localhost, .lan")]));
        assert_eq!((p.http.as_deref(), p.https.as_deref(), p.socks), (Some("127.0.0.1:7890"), Some("127.0.0.1:7890"), None));
        assert_eq!(p.exceptions, vec!["localhost", ".lan"]);
        // all_proxy：socks 开头的当 socks，别的当 http、https 都用
        let p = from_env(vars(&[("ALL_PROXY", "socks5://127.0.0.1:1080")]));
        assert_eq!((p.http, p.socks.as_deref()), (None, Some("127.0.0.1:1080")));
        let p = from_env(vars(&[("all_proxy", "http://10.0.0.2:3128")]));
        assert_eq!((p.http.as_deref(), p.https.as_deref()), (Some("10.0.0.2:3128"), Some("10.0.0.2:3128")));
        assert_eq!(from_env(|_| None), SystemProxy::default());
    }
}
