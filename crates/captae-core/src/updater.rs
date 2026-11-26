//! 下载引擎（yt-dlp）自动更新（计划 2.2）。
//!
//! 流程：
//! 1. 每天查一次 GitHub 官方 release（走 Captae 的代理设置）
//! 2. 有新版就下载 `SHA2-256SUMS`、`SHA2-256SUMS.sig`，用随包附带的 yt-dlp 官方公钥验证签名
//! 3. 下载这个平台的官方目录版（macOS `yt-dlp_macos.zip`、Windows `yt-dlp_win.zip`、Linux `yt-dlp_linux.zip`），核对 SHA256 和签过名的清单一致
//! 4. 解压到 `<数据目录>/engines/yt-dlp/<版本>/`（macOS 用系统的 ditto，Windows、Linux 用 zip 包），
//!    macOS 检查代码签名，没有签名的加本地签名
//! 5. 自检：`--version` 对得上，再读一个固定的公开视频；通过了才换成新版
//! 6. 保留上一个版本。新版在使用中连续出「组件 / 未知」错误时做一次体检，确认是新版的问题就退回
//!
//! 任何一步失败都只记进状态文件（`engines/yt-dlp/state.json`），继续用原来的版本。
//! 设置里指定了 yt-dlp 路径时不自动更新（用户自己管）。
//!
//! Android 完整版（`Flavor::ANDROID`）同一套流程，区别只在下的文件：release 里的 `yt-dlp`（zipapp，纯 Python），
//! 不用解压、不用签名，放成 `engines/yt-dlp/<版本>/yt-dlp`；运行时由原生库目录里的启动器跑它（`tools::Runner::ytdlp_command`）。

use crate::errors::ErrorCode;
use crate::tools::{run_full, Toolbox};
use crate::tr;
use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const LATEST_API: &str = "https://api.github.com/repos/yt-dlp/yt-dlp/releases/latest";
pub const LATEST_PAGE: &str = "https://github.com/yt-dlp/yt-dlp/releases/latest";
pub const DOWNLOAD_BASE: &str = "https://github.com/yt-dlp/yt-dlp/releases/download";
/// 这个平台的官方目录版（自带 Python，启动比单文件版快，见 scripts/fetch-sidecars.sh 的说明）和解开后的主程序名。
/// 文件名照 2026.10.26 版的 SHA2-256SUMS（testdata 里那份）核对过：Windows 的 zip 里是 yt-dlp.exe 加 `_internal\`，
/// Linux 的是 yt-dlp_linux（ARM 是 yt-dlp_linux_aarch64）加 `_internal/`
#[cfg(target_os = "macos")]
pub const ASSET: &str = "yt-dlp_macos.zip";
#[cfg(target_os = "macos")]
pub const EXE: &str = "yt-dlp_macos";
#[cfg(all(windows, target_arch = "aarch64"))]
pub const ASSET: &str = "yt-dlp_win_arm64.zip";
#[cfg(all(windows, not(target_arch = "aarch64")))]
pub const ASSET: &str = "yt-dlp_win.zip";
#[cfg(windows)]
pub const EXE: &str = "yt-dlp.exe";
#[cfg(all(not(target_os = "macos"), not(windows), target_arch = "aarch64"))]
pub const ASSET: &str = "yt-dlp_linux_aarch64.zip";
#[cfg(all(not(target_os = "macos"), not(windows), target_arch = "aarch64"))]
pub const EXE: &str = "yt-dlp_linux_aarch64";
#[cfg(all(not(target_os = "macos"), not(windows), not(target_arch = "aarch64")))]
pub const ASSET: &str = "yt-dlp_linux.zip";
#[cfg(all(not(target_os = "macos"), not(windows), not(target_arch = "aarch64")))]
pub const EXE: &str = "yt-dlp_linux";
/// yt-dlp 官方签名公钥，原文来自 https://github.com/yt-dlp/yt-dlp/blob/master/public.key
const PUBLIC_KEY: &str = include_str!("../assets/yt-dlp-public.key");
/// 上面公钥的指纹（Simon Sawicki (yt-dlp signing key)），加载时核对，防止文件被换
pub const KEY_FINGERPRINT: &str = "ac0cbbe6848d6a873464af4e57cf65933b5a7581";
/// 自检读的视频：YouTube 上第一个视频（19 秒），公开、多年没变过
pub const SELFTEST_URL: &str = "https://www.youtube.com/watch?v=jNQXAC9IVRw";
const DAY_MS: i64 = 24 * 3600 * 1000;
/// 正在用的新版连续出这么多次「未知」错误，就体检一次（「组件」错误出一次就体检）
const UNKNOWN_FAULTS_BEFORE_CHECK: u32 = 3;

// ------------------------------------------------------------------ 状态

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct State {
    /// 正在用的版本（`engines/yt-dlp/<active>/<EXE>`）；None 表示用安装包自带的
    pub active: Option<String>,
    /// 上一个自动更新的版本，新版出问题时退回到它；None 表示退回到安装包自带的
    pub previous: Option<String>,
    /// 上次检查的时间（毫秒）
    pub last_check: Option<i64>,
    /// 上次查到的最新版本
    pub latest: Option<String>,
    /// 上次检查的结果（给人看的一句话，按检查时的界面语言写）
    pub last_result: Option<String>,
    /// 上次检查失败的原因
    pub last_error: Option<String>,
    /// 正在用的新版连续出错的次数（成功一次清零）
    pub faults: u32,
    /// 自检没通过或被退回过的版本，不再自动安装（手动「检查更新」时还会再试）
    pub bad: Vec<String>,
    /// 上次因为「未知」错误体检的时间（毫秒）：这类体检一小时最多一次，免得批量下载时反复体检
    pub last_health: Option<i64>,
    /// 最近的更新记录（时间、内容），最多 30 条
    pub log: Vec<(i64, String)>,
}

impl State {
    fn note(&mut self, text: impl Into<String>) {
        let t = text.into();
        tracing::info!("引擎更新：{t}");
        self.log.push((crate::util::now_ms(), t));
        let n = self.log.len();
        if n > 30 {
            self.log.drain(..n - 30);
        }
    }
}

pub fn engine_dir(data: &Path) -> PathBuf {
    data.join("engines").join("yt-dlp")
}

fn state_file(data: &Path) -> PathBuf {
    engine_dir(data).join("state.json")
}

pub fn load(data: &Path) -> State {
    std::fs::read(state_file(data)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(data: &Path, s: &State) -> Result<()> {
    let f = state_file(data);
    std::fs::create_dir_all(f.parent().unwrap())?;
    let tmp = f.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    std::fs::rename(tmp, f)?;
    Ok(())
}

/// 改状态文件：读出来、改、写回去。多个进程（桌面端、命令行、MCP）可能同时用，所以每次都重新读。
fn update_state<R>(data: &Path, f: impl FnOnce(&mut State) -> R) -> R {
    let mut s = load(data);
    let before = s.clone();
    let r = f(&mut s);
    if s != before {
        if let Err(e) = save(data, &s) {
            tracing::warn!("写引擎更新状态失败：{e}");
        }
    }
    r
}

/// 这个平台下哪个文件、装好以后叫什么
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Flavor {
    /// release 里的文件名（SHA2-256SUMS 里也是这个名字）
    pub asset: &'static str,
    /// 装好以后 `<版本>/` 目录里的程序（或 zipapp）
    pub exe: &'static str,
    /// 下的是压缩包（要解压、检查代码签名）
    pub zipped: bool,
}

impl Flavor {
    /// 桌面端（macOS、Windows、Linux）：这个平台的官方目录版压缩包（ASSET / EXE 按平台定；名字沿用 MACOS）
    pub const MACOS: Flavor = Flavor { asset: ASSET, exe: EXE, zipped: true };
    /// Android 完整版：官方 zipapp
    pub const ANDROID: Flavor = Flavor { asset: "yt-dlp", exe: "yt-dlp", zipped: false };
}

/// 按 Runner 选（`tools::Runner::kind`）：android → Android 的 zipapp，其他 → 桌面端这个平台的目录版
pub fn flavor_of(runner_kind: &str) -> Flavor {
    if runner_kind == "android" {
        Flavor::ANDROID
    } else {
        Flavor::MACOS
    }
}

/// 这里能不能自动更新：桌面端（官方有目录版的平台，`platform_ok`）、Android 完整版。别的（iOS）返回 None
fn flavor(tools: &Toolbox) -> Option<Flavor> {
    match tools.runner.kind() {
        "android" => Some(Flavor::ANDROID),
        "desktop" if platform_ok() => Some(Flavor::MACOS),
        _ => None,
    }
}

/// 目录里的主程序。官方包万一换了主程序的名字（比如 Linux 版叫 yt-dlp），也认
fn exe_in(dir: &Path) -> PathBuf {
    let alt: &[&str] = if cfg!(windows) { &["yt-dlp.exe"] } else { &["yt-dlp"] };
    std::iter::once(EXE).chain(alt.iter().copied()).map(|n| dir.join(n)).find(|p| p.is_file()).unwrap_or_else(|| dir.join(EXE))
}

fn exe_as(dir: &Path, f: Flavor) -> PathBuf {
    dir.join(f.exe)
}

/// 自动更新装好、正在用的 yt-dlp：(版本, 程序路径)。没有就 None。
pub fn active_engine(data: &Path) -> Option<(String, PathBuf)> {
    active_engine_as(data, Flavor::MACOS)
}

/// 同上，指定平台（Android 装的是 `<版本>/yt-dlp`）
pub fn active_engine_as(data: &Path, f: Flavor) -> Option<(String, PathBuf)> {
    let v = load(data).active?;
    let p = exe_as(&engine_dir(data).join(&v), f);
    p.is_file().then_some((v, p))
}

// ------------------------------------------------------------------ 版本、清单、签名

/// "2026.10.26" / "2026.10.26.1" → [2026, 10, 26, 1]；认不出的部分当 0。
pub fn parse_version(s: &str) -> Vec<u64> {
    s.trim().trim_start_matches('v').split('.').map(|p| p.chars().take_while(char::is_ascii_digit).collect::<String>().parse().unwrap_or(0)).collect()
}

/// a 比 b 新
pub fn newer(a: &str, b: &str) -> bool {
    let (mut x, mut y) = (parse_version(a), parse_version(b));
    let n = x.len().max(y.len());
    x.resize(n, 0);
    y.resize(n, 0);
    x > y
}

/// 版本号只能是数字和点（它会拿来拼目录名和下载地址）
pub fn valid_version(v: &str) -> bool {
    !v.is_empty() && v.len() <= 32 && v.chars().all(|c| c.is_ascii_digit() || c == '.') && v.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// `SHA2-256SUMS` 里某个文件的 sha256。
pub fn sum_of(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|l| {
        let mut it = l.split_whitespace();
        let (h, n) = (it.next()?, it.next()?);
        (n.trim_start_matches('*') == name && h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_ascii_lowercase())
    })
}

/// 用 yt-dlp 官方公钥验证 `SHA2-256SUMS.sig`（二进制或 ASCII armor 的分离签名都认）。
pub fn verify_sums(sums: &[u8], sig: &[u8]) -> Result<()> {
    use pgp::composed::{Deserializable, DetachedSignature, SignedPublicKey};
    use pgp::types::KeyDetails;
    let (key, _) = SignedPublicKey::from_string(PUBLIC_KEY).map_err(|e| anyhow!(tr!("读不了随包附带的 yt-dlp 公钥：{error}", error = e)))?;
    let fp = format!("{:x}", key.fingerprint());
    if fp != KEY_FINGERPRINT {
        bail!(tr!("随包附带的 yt-dlp 公钥指纹不对（{fingerprint}）", fingerprint = fp));
    }
    let sig = match DetachedSignature::from_bytes(sig) {
        Ok(s) => s,
        Err(_) => {
            let text = std::str::from_utf8(sig).map_err(|_| anyhow!(tr!("签名文件格式不对")))?;
            DetachedSignature::from_string(text).map_err(|e| anyhow!(tr!("签名文件格式不对：{error}", error = e)))?.0
        }
    };
    if sig.verify(&key, sums).is_ok() {
        return Ok(());
    }
    for sub in &key.public_subkeys {
        if sig.verify(sub, sums).is_ok() {
            return Ok(());
        }
    }
    bail!(tr!("SHA2-256SUMS 的签名验证没有通过（不是 yt-dlp 官方签的，或文件被改过）"))
}

// ------------------------------------------------------------------ 网络

/// GitHub 上最新的稳定版版本号。API 被限流时退回读「最新版」页面的跳转地址。
pub async fn latest_tag(http: &reqwest::Client) -> Result<String> {
    let api = async {
        let r = http.get(LATEST_API).header("Accept", "application/vnd.github+json").timeout(Duration::from_secs(20)).send().await?;
        if !r.status().is_success() {
            bail!(tr!("GitHub API 返回 {status}", status = r.status().as_u16()));
        }
        let v: Value = r.json().await?;
        v["tag_name"].as_str().map(str::to_string).ok_or_else(|| anyhow!(tr!("GitHub API 的结果里没有版本号")))
    };
    let tag = match api.await {
        Ok(t) => t,
        Err(e) => {
            tracing::debug!("GitHub API 失败，改读跳转地址：{e}");
            // 「最新版」页面会跳到 https://github.com/yt-dlp/yt-dlp/releases/tag/2026.10.26，跟随跳转后读最终地址
            let r = http.get(LATEST_PAGE).timeout(Duration::from_secs(20)).send().await.map_err(|e2| anyhow!(tr!("{error}；读 release 页面也失败：{error2}", error = e, error2 = e2)))?;
            let u = r.url().to_string();
            u.rsplit('/').next().map(str::to_string).filter(|t| valid_version(t)).ok_or_else(|| anyhow!(tr!("{error}；release 页面地址里没有版本号：{url}", error = e, url = u)))?
        }
    };
    if !valid_version(&tag) {
        bail!(tr!("版本号格式不对：{tag}", tag = tag));
    }
    Ok(tag)
}

async fn fetch_bytes(http: &reqwest::Client, url: &str) -> Result<Vec<u8>> {
    let r = http.get(url).timeout(Duration::from_secs(60)).send().await.map_err(|e| anyhow::Error::new(crate::errors::from_reqwest(&e, url)))?;
    if !r.status().is_success() {
        return Err(crate::errors::http_status(r.status().as_u16(), url).into());
    }
    Ok(r.bytes().await?.to_vec())
}

/// 边下边算 sha256，返回十六进制。两次收到数据之间超过 60 秒算失败。
async fn download_to(http: &reqwest::Client, url: &str, dst: &Path) -> Result<String> {
    use futures_util::StreamExt;
    use tokio::io::AsyncWriteExt;
    let r = http.get(url).send().await.map_err(|e| anyhow::Error::new(crate::errors::from_reqwest(&e, url)))?;
    if !r.status().is_success() {
        return Err(crate::errors::http_status(r.status().as_u16(), url).into());
    }
    let mut f = tokio::fs::File::create(dst).await?;
    let mut h = Sha256::new();
    let mut s = r.bytes_stream();
    while let Some(c) = tokio::time::timeout(Duration::from_secs(60), s.next()).await.map_err(|_| anyhow!(tr!("下载新版引擎时 60 秒没有收到数据")))? {
        let c = c?;
        h.update(&c);
        f.write_all(&c).await?;
    }
    f.flush().await?;
    Ok(hex::encode(h.finalize()))
}

// ------------------------------------------------------------------ 解压、签名、自检

async fn sh(prog: &str, args: &[&str], timeout: u64) -> Result<String> {
    let mut c = tokio::process::Command::new(prog);
    c.args(args).kill_on_drop(true).stdin(std::process::Stdio::null());
    let r = run_full(c, Duration::from_secs(timeout)).await.map_err(anyhow::Error::new)?;
    if !r.ok {
        bail!(tr!("{command} 失败：{error}", command = format!("{prog} {}", args.join(" ")), error = r.stderr.lines().last().unwrap_or("").trim()));
    }
    Ok(String::from_utf8_lossy(&r.stdout).to_string())
}

/// 解压官方 zip。macOS 用系统自带的 ditto（保留权限、符号链接和代码签名要的扩展属性，Python.framework 里有符号链接，
/// 签名校验要它们原样），Windows、Linux 用 zip 包（`extract_zip`），不依赖系统里有没有 unzip。
async fn unzip(zip: &Path, dst: &Path) -> Result<PathBuf> {
    tokio::fs::create_dir_all(dst).await?;
    if cfg!(target_os = "macos") {
        let (z, d) = (zip.display().to_string(), dst.display().to_string());
        sh("/usr/bin/ditto", &["-x", "-k", &z, &d], 300).await?;
    } else {
        let (z, d) = (zip.to_path_buf(), dst.to_path_buf());
        tokio::task::spawn_blocking(move || extract_zip(&z, &d)).await.map_err(|e| anyhow!("{e}"))??;
    }
    // 官方包里程序在最外层；万一多包了一层目录也认
    if exe_in(dst).is_file() {
        return Ok(dst.to_path_buf());
    }
    let mut rd = tokio::fs::read_dir(dst).await?;
    while let Some(e) = rd.next_entry().await? {
        if exe_in(&e.path()).is_file() {
            return Ok(e.path());
        }
    }
    bail!(tr!("压缩包里没有找到 {name}", name = EXE))
}

/// 用 zip 包解压：只许解到 `dst` 里面（`enclosed_name` 挡掉 `../` 和绝对路径）；unix 上照压缩包里记的权限设，
/// 符号链接照原样建（只许指向压缩包里面）；主程序没带执行权限时补上
pub fn extract_zip(zip: &Path, dst: &Path) -> Result<()> {
    let f = std::fs::File::open(zip)?;
    let mut a = ::zip::ZipArchive::new(std::io::BufReader::new(f)).map_err(|e| anyhow!(tr!("压缩包打不开：{error}", error = e)))?;
    for i in 0..a.len() {
        let mut e = a.by_index(i).map_err(|e| anyhow!(tr!("压缩包打不开：{error}", error = e)))?;
        let Some(rel) = e.enclosed_name() else { bail!(tr!("压缩包里有不安全的路径：{path}", path = e.name())) };
        let out = dst.join(rel);
        if e.is_dir() {
            std::fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        #[cfg(unix)]
        if e.is_symlink() {
            use std::io::Read;
            let mut target = String::new();
            e.read_to_string(&mut target)?;
            if !inside(dst, &out.parent().unwrap_or(dst).join(&target)) {
                bail!(tr!("压缩包里有不安全的路径：{path}", path = format!("{} -> {target}", e.name())));
            }
            let _ = std::fs::remove_file(&out);
            std::os::unix::fs::symlink(&target, &out)?;
            continue;
        }
        let mut w = std::fs::File::create(&out)?;
        std::io::copy(&mut e, &mut w)?;
        #[cfg(unix)]
        if let Some(mode) = e.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode & 0o777))?;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for exe in [dst.join(EXE)].into_iter().chain(std::fs::read_dir(dst).into_iter().flatten().flatten().map(|e| e.path().join(EXE))) {
            if let Ok(m) = std::fs::metadata(&exe) {
                if m.is_file() && m.permissions().mode() & 0o111 == 0 {
                    std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(m.permissions().mode() | 0o755))?;
                }
            }
        }
    }
    Ok(())
}

/// 按字面算 `p`（可以带 `..`）是不是在 `root` 里面
#[cfg(unix)]
fn inside(root: &Path, p: &Path) -> bool {
    use std::path::Component;
    let mut out: Vec<Component> = vec![];
    for c in p.components() {
        match c {
            Component::ParentDir => {
                if out.pop().is_none() {
                    return false;
                }
            }
            Component::CurDir => {}
            c => out.push(c),
        }
    }
    out.iter().collect::<PathBuf>().starts_with(root)
}

fn is_macho(p: &Path) -> bool {
    use std::io::Read;
    let mut b = [0u8; 4];
    std::fs::File::open(p).and_then(|mut f| f.read_exact(&mut b)).is_ok()
        && matches!(u32::from_be_bytes(b), 0xfeedface | 0xfeedfacf | 0xcefaedfe | 0xcffaedfe | 0xcafebabe | 0xbebafeca)
}

fn macho_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() {
            macho_files(&p, out);
        } else if ft.is_file() && is_macho(&p) {
            out.push(p);
        }
    }
}

/// 去掉隔离属性；主程序签名不对（或没有签名）就把目录里所有 Mach-O 文件加上本地签名。
/// Apple Silicon 上没签名的程序运行不了。返回做了什么（写进更新记录）。
async fn ensure_signed(dir: &Path) -> Result<String> {
    if !cfg!(target_os = "macos") {
        return Ok(tr!("不是 macOS，不用检查代码签名"));
    }
    let d = dir.display().to_string();
    let _ = sh("/usr/bin/xattr", &["-dr", "com.apple.quarantine", &d], 60).await;
    let exe = exe_in(dir).display().to_string();
    if sh("/usr/bin/codesign", &["-v", &exe], 60).await.is_ok() {
        return Ok(tr!("代码签名有效"));
    }
    let mut files = vec![];
    macho_files(dir, &mut files);
    // 先签库，最后签主程序
    files.sort_by_key(|p| p.file_name().map(|n| n == EXE).unwrap_or(false));
    for f in &files {
        let f = f.display().to_string();
        sh("/usr/bin/codesign", &["--force", "-s", "-", &f], 60).await.with_context(|| tr!("加本地签名失败：{path}", path = f))?;
    }
    sh("/usr/bin/codesign", &["-v", &exe], 60).await.context(tr!("加了本地签名还是验证不过"))?;
    Ok(tr!("没有有效的代码签名，已给 {n} 个程序文件加上本地签名", n = files.len()))
}

/// 0.2 秒的静音 WAV：读不到 YouTube 时用它做离线自检（`--enable-file-urls` 读本地文件）
fn tiny_wav() -> Vec<u8> {
    let n: u32 = 1600;
    let len = n * 2;
    let mut b = Vec::with_capacity(44 + len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    for x in [16u32.to_le_bytes(), [1, 0, 1, 0], 8000u32.to_le_bytes(), 16000u32.to_le_bytes(), [2, 0, 16, 0]] {
        b.extend_from_slice(&x);
    }
    b.extend_from_slice(b"data");
    b.extend_from_slice(&len.to_le_bytes());
    b.resize(44 + len as usize, 0);
    b
}

/// `<exe> --version`，返回版本号。第一次运行新程序时 macOS 要做安全检查，给足时间。
pub async fn exe_version(tools: &Toolbox, exe: &Path) -> Result<String> {
    let mut c = tools.ytdlp_at(exe);
    c.arg("--version");
    let r = run_full(c, Duration::from_secs(120)).await.map_err(anyhow::Error::new)?;
    let v = String::from_utf8_lossy(&r.stdout).lines().next().unwrap_or("").trim().to_string();
    if !r.ok || v.is_empty() {
        bail!(tr!("{path} --version 失败：{error}", path = exe.display(), error = r.stderr.lines().last().unwrap_or("").trim()));
    }
    Ok(v)
}

/// 读自检视频的信息。成功返回格式数量，失败返回分类后的原因。
/// YouTube 按会话做实验，偶尔不给公开视频：和平时读取一样，这类失败换一组客户端再试一次，免得把好好的新版当成坏的。
pub async fn probe_selftest(tools: &Toolbox, exe: &Path) -> std::result::Result<usize, crate::errors::Failure> {
    let alt = vec!["--extractor-args".to_string(), format!("youtube:player_client={}", crate::engine::ytdlp::YT_ALT_CLIENTS)];
    let mut last = None;
    for extra in [vec![], alt] {
        let mut c = tools.ytdlp_at(exe);
        c.args(["-J", "--no-playlist"]).args(&extra).arg(SELFTEST_URL);
        let r = run_full(c, Duration::from_secs(120)).await?;
        if !r.ok {
            let f = crate::errors::classify(&r.stderr, SELFTEST_URL);
            let again = crate::engine::ytdlp::youtube_flaky(&f);
            last = Some(f);
            if again {
                continue;
            }
            break;
        }
        let v: Value = serde_json::from_slice(&r.stdout).map_err(|e| crate::errors::Failure::new(ErrorCode::Tool, tr!("下载组件返回的信息读不了"), e.to_string()))?;
        let n = v["formats"].as_array().map(|a| a.len()).unwrap_or(0);
        if n == 0 {
            return Err(crate::errors::Failure::new(ErrorCode::NoMedia, tr!("自检视频没有读到任何格式"), "formats is empty"));
        }
        return Ok(n);
    }
    Err(last.unwrap_or_else(|| crate::errors::Failure::new(ErrorCode::Unknown, tr!("自检没有结果"), "")))
}

/// 离线自检：读一个本地的小音频文件，确认程序本身能正常解析。
async fn offline_selftest(tools: &Toolbox, exe: &Path, work: &Path) -> Result<()> {
    let wav = work.join("selftest.wav");
    tokio::fs::write(&wav, tiny_wav()).await?;
    let mut c = tools.ytdlp_at(exe);
    // Windows 的路径要写成 file:///C:/…（反斜杠、盘符直接拼进去 yt-dlp 不认）；macOS、Linux 照旧
    let url = if cfg!(windows) { url::Url::from_file_path(&wav).map(String::from).unwrap_or_else(|_| format!("file:///{}", wav.display())) } else { format!("file://{}", wav.display()) };
    c.args(["--enable-file-urls", "-J"]).arg(url);
    let r = run_full(c, Duration::from_secs(60)).await.map_err(anyhow::Error::new)?;
    let _ = tokio::fs::remove_file(&wav).await;
    let v: Value = serde_json::from_slice(&r.stdout).unwrap_or(Value::Null);
    if !r.ok || !(v["url"].is_string() || v["formats"].is_array()) {
        bail!(tr!("离线自检失败：{error}", error = r.stderr.lines().last().unwrap_or("").trim()));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// 新版能用
    Pass(String),
    /// 新版有问题（原因）
    Fail(String),
}

/// 自检新版：`--version` 对得上 → 读自检视频。新版读不到时拿现在用的版本对照：
/// 现在的能读、新版不能 → 新版有问题；两个都读不到（多半是连不上 YouTube）→ 改做离线自检。
pub async fn selftest(tools: &Toolbox, new_exe: &Path, expect: &str, current_exe: Option<&Path>, work: &Path) -> Verdict {
    match exe_version(tools, new_exe).await {
        Ok(v) if v == expect || v.starts_with(&format!("{expect} ")) => {}
        Ok(v) => return Verdict::Fail(tr!("版本号对不上：期望 {expect}，实际 {actual}", expect = expect, actual = v)),
        Err(e) => return Verdict::Fail(format!("{e:#}")),
    }
    let new_err = match probe_selftest(tools, new_exe).await {
        Ok(n) => return Verdict::Pass(tr!("读自检视频成功（{n} 个格式）", n = n)),
        Err(f) => f,
    };
    if let Some(cur) = current_exe {
        if probe_selftest(tools, cur).await.is_ok() {
            return Verdict::Fail(tr!("新版读不了自检视频（{reason}），现在用的版本能读", reason = new_err.message));
        }
    }
    match offline_selftest(tools, new_exe, work).await {
        Ok(()) => Verdict::Pass(tr!("连不上自检视频（{reason}），离线自检通过", reason = new_err.message)),
        Err(e) => Verdict::Fail(tr!("读不了自检视频（{reason}），离线自检也失败：{error}", reason = new_err.message, error = format!("{e:#}"))),
    }
}

// ------------------------------------------------------------------ 检查更新

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    /// updated 已更新 / latest 已经是最新 / skipped 没检查（关了、用的是自定义路径、平台不支持）/
    /// rejected 新版自检没通过 / failed 检查或下载失败
    pub result: &'static str,
    /// 给人看的一句话
    pub message: String,
    /// 检查前在用的版本
    pub current: Option<String>,
    pub latest: Option<String>,
    /// 检查后在用的自动更新版本（None = 安装包自带的）
    pub active: Option<String>,
    /// 每一步做了什么
    pub steps: Vec<String>,
}

static CHECKING: AtomicBool = AtomicBool::new(false);
static HEALTH: AtomicBool = AtomicBool::new(false);

pub fn checking() -> bool {
    CHECKING.load(Ordering::SeqCst)
}

/// 官方有目录版的平台：macOS、Windows（x64、ARM）、Linux（x64、ARM64，glibc）
fn platform_ok() -> bool {
    cfg!(target_os = "macos") || cfg!(windows) || (cfg!(target_os = "linux") && cfg!(any(target_arch = "x86_64", target_arch = "aarch64")))
}

/// 谁要检查更新。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum How {
    /// 每天自动查一次：之前自检没通过的版本跳过
    Daily,
    /// 用户点「检查更新」或运行 `captae update`：之前没通过的版本也再试一次
    Manual,
    /// 版本一样也重新装一遍（「修复」和测试用）
    Reinstall,
}

/// 检查并安装新版。
pub async fn check(tools: &Toolbox, http: &reqwest::Client, data: &Path, how: How) -> Report {
    if CHECKING.swap(true, Ordering::SeqCst) {
        return Report { result: "skipped", message: tr!("正在检查更新，稍等"), current: None, latest: None, active: load(data).active, steps: vec![] };
    }
    let r = check_inner(tools, http, data, how).await;
    CHECKING.store(false, Ordering::SeqCst);
    update_state(data, |s| {
        s.last_check = Some(crate::util::now_ms());
        s.last_result = Some(r.message.clone());
        if r.latest.is_some() {
            s.latest = r.latest.clone();
        }
        s.last_error = (r.result == "failed").then(|| r.message.clone());
        if r.result != "latest" && r.result != "skipped" {
            s.note(r.message.clone());
        }
    });
    r
}

async fn check_inner(tools: &Toolbox, http: &reqwest::Client, data: &Path, how: How) -> Report {
    let force = how == How::Reinstall;
    let mut steps = vec![];
    let state = load(data);
    let rep = |result: &'static str, message: String, current: Option<String>, latest: Option<String>, steps: Vec<String>| Report { result, message, current, latest, active: load(data).active, steps };
    if tools.ytdlp_source() == "custom" {
        return rep("skipped", tr!("设置里指定了 yt-dlp 的路径，不自动更新"), None, None, steps);
    }
    let Some(fl) = flavor(tools) else {
        return rep("skipped", tr!("这个系统还不支持自动更新下载引擎"), None, None, steps);
    };
    // 现在用的版本：自动更新装的，或者安装包自带的 / 系统里的
    let current_exe = tools.path(crate::tools::Tool::YtDlp).ok();
    let current = match (&state.active, active_engine_as(data, fl)) {
        (Some(v), Some(_)) => Some(v.clone()),
        _ => match &current_exe {
            Some(p) => exe_version(tools, p).await.ok(),
            None => None,
        },
    };
    steps.push(tr!("现在用的版本：{version}", version = current.clone().unwrap_or_else(|| tr!("没有找到§检查更新时，找不到现在用的下载引擎"))));
    let latest = match latest_tag(http).await {
        Ok(t) => t,
        Err(e) => return rep("failed", tr!("检查更新失败：{error}", error = format!("{e:#}")), current, None, steps),
    };
    steps.push(tr!("GitHub 上的最新版：{latest}", latest = latest));
    let is_newer = current.as_deref().map(|c| newer(&latest, c)).unwrap_or(true);
    if !force && !is_newer {
        return rep("latest", tr!("已经是最新版（{version}）", version = current.as_deref().unwrap_or(&latest)), current, Some(latest), steps);
    }
    if how == How::Daily && state.bad.iter().any(|b| b == &latest) {
        return rep("skipped", tr!("{latest} 之前自检没通过，不再自动安装", latest = latest), current, Some(latest), steps);
    }
    match install(tools, http, data, fl, &latest, current_exe.as_deref(), &mut steps).await {
        Ok(Verdict::Pass(why)) => {
            steps.push(tr!("自检通过：{why}", why = why));
            update_state(data, |s| {
                if s.active.as_deref() != Some(latest.as_str()) {
                    s.previous = s.active.take();
                }
                s.active = Some(latest.clone());
                s.faults = 0;
                s.bad.retain(|b| b != &latest);
            });
            prune(data);
            let msg = match &current {
                Some(c) if c != &latest => tr!("下载引擎已从 {from} 更新到 {to}", from = c, to = latest),
                _ => tr!("下载引擎已重新安装 {version}", version = latest),
            };
            rep("updated", msg, current, Some(latest), steps)
        }
        Ok(Verdict::Fail(why)) => {
            steps.push(tr!("自检没通过：{why}", why = why));
            update_state(data, |s| {
                if !s.bad.contains(&latest) {
                    s.bad.push(latest.clone());
                }
            });
            rep("rejected", tr!("新版 {latest} 自检没通过，继续用 {current}：{why}", latest = latest, current = current.clone().unwrap_or_else(|| tr!("原来的版本")), why = why), current, Some(latest), steps)
        }
        Err(e) => rep("failed", tr!("更新到 {latest} 失败，继续用原来的版本：{error}", latest = latest, error = format!("{e:#}")), current, Some(latest), steps),
    }
}

/// 下载、验签、核对、解压、签名、自检。自检通过时新版已经放在 `engines/yt-dlp/<版本>/`。
/// Android 的 zipapp 不解压、不签名，直接放进 `<版本>/`。
async fn install(tools: &Toolbox, http: &reqwest::Client, data: &Path, fl: Flavor, ver: &str, current_exe: Option<&Path>, steps: &mut Vec<String>) -> Result<Verdict> {
    let root = engine_dir(data);
    let dl = root.join(".download");
    tokio::fs::create_dir_all(&dl).await?;
    let base = format!("{DOWNLOAD_BASE}/{ver}");
    let sums = fetch_bytes(http, &format!("{base}/SHA2-256SUMS")).await.context(tr!("下载 {file} 失败", file = "SHA2-256SUMS"))?;
    let sig = fetch_bytes(http, &format!("{base}/SHA2-256SUMS.sig")).await.context(tr!("下载 {file} 失败", file = "SHA2-256SUMS.sig"))?;
    verify_sums(&sums, &sig)?;
    steps.push(tr!("SHA2-256SUMS 的签名验证通过（yt-dlp 官方公钥）"));
    let asset = fl.asset;
    let want = sum_of(&String::from_utf8_lossy(&sums), asset).ok_or_else(|| anyhow!(tr!("SHA2-256SUMS 里没有 {name}", name = asset)))?;
    let zip = dl.join(format!("{ver}-{asset}"));
    let got = download_to(http, &format!("{base}/{asset}"), &zip).await.context(tr!("下载新版引擎失败"))?;
    if got != want {
        let _ = tokio::fs::remove_file(&zip).await;
        bail!(tr!("{name} 的 SHA256 对不上（清单 {expected}，实际 {actual}）", name = asset, expected = want, actual = got));
    }
    steps.push(tr!("{name} 的 SHA256 和签过名的清单一致（{hash}…）", name = asset, hash = &want[..12]));
    let partial = root.join(format!(".{ver}.partial"));
    let _ = tokio::fs::remove_dir_all(&partial).await;
    let dir = if fl.zipped {
        let dir = unzip(&zip, &partial).await?;
        let _ = tokio::fs::remove_file(&zip).await;
        steps.push(tr!("已解压"));
        steps.push(ensure_signed(&dir).await?);
        dir
    } else {
        // zipapp：原样放进去（启动器里的 Python 解释它，不用解压、不用签名）
        tokio::fs::create_dir_all(&partial).await?;
        tokio::fs::rename(&zip, exe_as(&partial, fl)).await?;
        partial.clone()
    };
    let verdict = selftest(tools, &exe_as(&dir, fl), ver, current_exe, &dl).await;
    if let Verdict::Pass(_) = verdict {
        let target = root.join(ver);
        // 「重新安装」同一个版本时，正在用的那份先挪开
        if target.exists() {
            let old = root.join(format!(".{ver}.old"));
            let _ = tokio::fs::remove_dir_all(&old).await;
            tokio::fs::rename(&target, &old).await?;
            let _ = tokio::fs::remove_dir_all(&old).await;
        }
        tokio::fs::rename(&dir, &target).await?;
    }
    let _ = tokio::fs::remove_dir_all(&partial).await;
    Ok(verdict)
}

/// 只留正在用的和上一个版本，其他的删掉（每个约 100 MB）。
fn prune(data: &Path) {
    let s = load(data);
    let keep: Vec<&str> = [s.active.as_deref(), s.previous.as_deref()].into_iter().flatten().collect();
    let Ok(rd) = std::fs::read_dir(engine_dir(data)) else { return };
    for e in rd.flatten() {
        let n = e.file_name().to_string_lossy().to_string();
        if e.path().is_dir() && (valid_version(&n) && !keep.contains(&n.as_str()) || n.ends_with(".partial") || n.ends_with(".old")) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

// ------------------------------------------------------------------ 使用中出错 → 体检 → 退回

/// 读取信息的结果（只在用的是自动更新版本时记）。「组件」错误出一次、「未知」错误连续出 3 次（一小时最多一次）
/// 就当场体检；确认是新版的问题就退回上一个版本，返回 true，调用方用退回后的版本再试一次。
/// 不放到后台去做：命令行进程读完就退出，后台任务会被丢掉。
pub async fn note_result(tools: &Toolbox, outcome: std::result::Result<(), ErrorCode>) -> bool {
    let Some(data) = tools.data_dir.clone() else { return false };
    if tools.ytdlp_source() != "managed" {
        return false;
    }
    let now = crate::util::now_ms();
    let check_now = update_state(&data, |s| match outcome {
        Ok(()) => {
            s.faults = 0;
            false
        }
        Err(ErrorCode::Tool) => {
            s.faults += 1;
            true
        }
        Err(code) if code.engine_fault() => {
            s.faults += 1;
            let quiet = s.last_health.is_some_and(|t| now - t < 3600 * 1000);
            if s.faults >= UNKNOWN_FAULTS_BEFORE_CHECK && !quiet {
                s.last_health = Some(now);
                true
            } else {
                false
            }
        }
        Err(_) => false,
    });
    check_now && health_check(tools, &data).await
}

/// 体检正在用的自动更新版本：它读不了自检视频、而退回后要用的版本能读 → 退回。
/// 新版连 `--version` 都运行不了时直接退回。返回是否退回了。
pub async fn health_check(tools: &Toolbox, data: &Path) -> bool {
    if HEALTH.swap(true, Ordering::SeqCst) {
        return false;
    }
    let r = health_inner(tools, data).await;
    HEALTH.store(false, Ordering::SeqCst);
    r
}

async fn health_inner(tools: &Toolbox, data: &Path) -> bool {
    let fl = flavor_of(tools.runner.kind());
    let Some((ver, exe)) = active_engine_as(data, fl) else { return false };
    let managed_ok = match exe_version(tools, &exe).await {
        Ok(_) => probe_selftest(tools, &exe).await.map_err(|f| f.message),
        Err(e) => Err(format!("{e:#}")),
    };
    if managed_ok.is_ok() {
        update_state(data, |s| s.faults = 0);
        return false;
    }
    let why = managed_ok.err().unwrap_or_default();
    // 拿退回后要用的那个版本对照
    let s = load(data);
    let fallback = s.previous.as_ref().map(|p| exe_as(&engine_dir(data).join(p), fl)).filter(|p| p.is_file()).or_else(|| tools.base_ytdlp());
    let fallback_ok = match &fallback {
        Some(p) => probe_selftest(tools, p).await.is_ok(),
        None => false,
    };
    // 新版连 --version 都跑不起来，直接退回；只是读不了视频时，对照的版本能读才退回
    let broken = exe_version(tools, &exe).await.is_err();
    if !(broken || fallback_ok) {
        update_state(data, |s| {
            s.faults = 0;
            s.note(tr!("体检：{version} 读不了自检视频（{why}），对照的版本也读不了，多半是网络问题，先不退回", version = ver, why = why));
        });
        return false;
    }
    rollback(data, &tr!("{version} 连续出错，体检没通过（{why}）", version = ver, why = why));
    true
}

/// 退回上一个版本（没有就退回安装包自带的），把出问题的版本记进 bad。
pub fn rollback(data: &Path, why: &str) {
    let root = engine_dir(data);
    update_state(data, |s| {
        let Some(bad) = s.active.take() else { return };
        if !s.bad.contains(&bad) {
            s.bad.push(bad.clone());
        }
        // 上一个版本还在才退回到它（macOS 是 yt-dlp_macos，Android 是 zipapp）
        s.active = s.previous.take().filter(|p| [Flavor::MACOS, Flavor::ANDROID].iter().any(|f| exe_as(&root.join(p), *f).is_file()));
        s.faults = 0;
        // 「已退回到 2026.08.19」：版本号前面的空格由 tr! 自动加（docs/I18N.md 2.2）
        let to = s.active.clone().unwrap_or_else(|| tr!("安装包自带的版本"));
        let why: String = why.chars().take(300).collect();
        s.note(tr!("已退回到{to}：{why}", to = to, why = why));
        let _ = std::fs::remove_dir_all(root.join(&bad));
    });
}

// ------------------------------------------------------------------ 每天检查一次

/// 桌面端常驻时调用：启动 30 秒后检查一次（距上次满一天才查），之后每小时看一次。
pub fn spawn_daily(core: crate::Core) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(30)).await;
        loop {
            let s = core.settings();
            let data = core.paths.data.clone();
            let due = load(&data).last_check.map(|t| crate::util::now_ms() - t >= DAY_MS).unwrap_or(true);
            if s.auto_update && due {
                let r = check(&core.toolbox(), &core.http(), &data, How::Daily).await;
                tracing::info!("每日检查下载引擎：{}", r.message);
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
}

/// 设置页、命令行「引擎状态」：现在用的版本、从哪来、自动更新的状态。
pub async fn status(tools: &Toolbox, data: &Path, auto_update: bool) -> Value {
    let s = load(data);
    let path = tools.path(crate::tools::Tool::YtDlp).ok();
    let version = match &path {
        Some(p) => exe_version(tools, p).await.ok(),
        None => None,
    };
    let bundled = match tools.bundled_ytdlp() {
        Some(p) => exe_version(tools, &p).await.ok(),
        None => None,
    };
    json!({
        "version": version,
        "source": tools.ytdlp_source(),
        "path": path,
        "bundled": bundled,
        "autoUpdate": auto_update,
        "checking": checking(),
        "active": s.active,
        "previous": s.previous,
        "latest": s.latest,
        "lastCheck": s.last_check,
        "lastResult": s.last_result,
        "lastError": s.last_error,
        "bad": s.bad,
        "log": s.log.iter().rev().take(10).collect::<Vec<_>>(),
        "jsRuntime": tools.js_runtime(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUMS: &[u8] = include_bytes!("../testdata/yt-dlp-2026.10.26/SHA2-256SUMS");
    const SIG: &[u8] = include_bytes!("../testdata/yt-dlp-2026.10.26/SHA2-256SUMS.sig");

    #[test]
    fn versions() {
        assert!(newer("2026.10.26", "2026.08.19"));
        assert!(newer("2026.10.26.1", "2026.10.26"));
        assert!(!newer("2026.10.26", "2026.10.26"));
        assert!(!newer("2025.12.31", "2026.01.01"));
        assert!(valid_version("2026.10.26"));
        assert!(!valid_version("../x") && !valid_version("") && !valid_version("v1"));
    }

    #[test]
    fn sums_lookup() {
        let s = String::from_utf8_lossy(SUMS);
        assert_eq!(sum_of(&s, "yt-dlp_macos.zip").as_deref(), Some("20d98fa608998eff87d1d35a6e9fb65e4b874639a6cc89addc19da3becde3ef1"));
        assert_eq!(sum_of(&s, "yt-dlp_win.zip").as_deref(), Some("fd51b212cf8380c7a52ee08c0d6be7f3c59b2f2c832eaf7b45acfae6eab57419"));
        assert_eq!(sum_of(&s, "yt-dlp_linux.zip").as_deref(), Some("f34053415136e55073dd64ba5326d161ecc23ce0e98c081d8752867b37809450"));
        // 这个平台要下的包在清单里
        assert!(sum_of(&s, ASSET).is_some(), "{ASSET}");
        assert_eq!(sum_of(&s, "yt-dlp").as_deref(), Some("e51e69e89489f199530f13d32a4342da3ddb2cf34842f364603a066db05afbf3"));
        assert!(sum_of(&s, "nope.zip").is_none());
    }

    #[test]
    fn signature() {
        // 实测：2026.10.26 官方发布的清单和签名
        verify_sums(SUMS, SIG).unwrap();
        // 清单改一个字（换掉 zip 的哈希）就不认
        let mut bad = SUMS.to_vec();
        let i = bad.iter().position(|b| *b == b'2').unwrap();
        bad[i] = b'3';
        assert!(verify_sums(&bad, SIG).is_err());
        // 签名坏了也不认
        let mut sig = SIG.to_vec();
        let n = sig.len();
        sig[n - 10] ^= 0xff;
        assert!(verify_sums(SUMS, &sig).is_err());
        assert!(verify_sums(SUMS, b"not a signature").is_err());
    }

    #[test]
    fn wav_header() {
        let w = tiny_wav();
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(&w[8..16], b"WAVEfmt ");
        assert_eq!(w.len(), 44 + 3200);
    }

    #[test]
    fn state_roundtrip_and_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        assert!(active_engine(data).is_none());
        // 假装装过两个版本
        for v in ["2026.08.19", "2026.10.26"] {
            let d = engine_dir(data).join(v);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join(EXE), b"#!/bin/sh\n").unwrap();
        }
        update_state(data, |s| {
            s.active = Some("2026.10.26".into());
            s.previous = Some("2026.08.19".into());
        });
        assert_eq!(active_engine(data).map(|x| x.0).as_deref(), Some("2026.10.26"));
        rollback(data, "测试");
        let s = load(data);
        assert_eq!(s.active.as_deref(), Some("2026.08.19"));
        assert_eq!(s.previous, None);
        assert_eq!(s.bad, vec!["2026.10.26".to_string()]);
        assert!(!engine_dir(data).join("2026.10.26").exists());
        assert!(s.log.last().unwrap().1.contains("已退回到 2026.08.19"));
        // 再退回一次：没有上一个版本了，回到安装包自带的
        rollback(data, "测试");
        assert!(load(data).active.is_none() && active_engine(data).is_none());
    }

    #[tokio::test]
    async fn selftest_rejects_broken_engine() {
        use crate::tools::testkit::fake;
        let dir = tempfile::tempdir().unwrap();
        // 一个假的「新版」：--version 直接失败
        let exe = fake(dir.path(), "new-ytdlp", "echo 'Traceback (most recent call last):' >&2\nexit 1", "[Console]::Error.WriteLine('Traceback (most recent call last):')\nexit 1");
        let tb = Toolbox::default();
        assert!(matches!(selftest(&tb, &exe, "2026.10.26", None, dir.path()).await, Verdict::Fail(_)));
        // 版本号对不上
        let exe = fake(dir.path(), "old-ytdlp", "echo 2026.01.01", "Write-Output 2026.01.01");
        match selftest(&tb, &exe, "2026.10.26", None, dir.path()).await {
            Verdict::Fail(why) => assert!(why.contains("版本号对不上"), "{why}"),
            v => panic!("{v:?}"),
        }
    }

    /// Android：装的是 `<版本>/yt-dlp`（zipapp），macOS 的找法看不到它；退回照样能用
    #[test]
    fn android_state_and_rollback() {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path();
        assert_eq!((flavor_of("android"), flavor_of("desktop")), (Flavor::ANDROID, Flavor::MACOS));
        for v in ["2026.10.26", "2026.11.23"] {
            let d = engine_dir(data).join(v);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("yt-dlp"), b"PK").unwrap();
        }
        update_state(data, |s| {
            s.active = Some("2026.11.23".into());
            s.previous = Some("2026.10.26".into());
        });
        assert!(active_engine(data).is_none());
        let (v, p) = active_engine_as(data, Flavor::ANDROID).unwrap();
        assert_eq!((v.as_str(), p), ("2026.11.23", engine_dir(data).join("2026.11.23/yt-dlp")));
        rollback(data, "测试");
        let s = load(data);
        assert_eq!((s.active.as_deref(), s.bad.clone()), (Some("2026.10.26"), vec!["2026.11.23".to_string()]));
        rollback(data, "测试");
        assert!(active_engine_as(data, Flavor::ANDROID).is_none());
    }

    /// Android 的自检：新版 zipapp 由原生库目录里的启动器跑（CAPTAE_YTDLP 指过去）。
    /// 这里用 shell 脚本假装启动器和 zipapp：连不上自检视频时改做离线自检，通过；zipapp 坏了（--version 失败）不通过
    #[cfg(unix)]
    #[tokio::test]
    async fn android_selftest_through_launcher() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let lib = dir.path().join("lib");
        std::fs::create_dir_all(&lib).unwrap();
        let launcher = lib.join("libytdlp.so");
        std::fs::write(&launcher, "#!/bin/sh\nexec /bin/sh \"${CAPTAE_YTDLP:?}\" \"$@\"\n").unwrap();
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).unwrap();
        // 假的 zipapp：--version 报版本；-J 读 file:// 时给出信息，读网址时像连不上网一样失败
        let zipapp = dir.path().join("yt-dlp");
        std::fs::write(
            &zipapp,
            "for a in \"$@\"; do case \"$a\" in --version) echo 2026.11.23; exit 0;; file://*) echo '{\"url\": \"x\"}'; exit 0;; esac; done\n\
             echo 'ERROR: [youtube] jNQXAC9IVRw: Unable to download webpage: <urlopen error [Errno 101] Network is unreachable>' >&2; exit 1\n",
        )
        .unwrap();
        let tb = Toolbox::default().with_runner(std::sync::Arc::new(crate::tools::AndroidRunner::new(&lib)));
        match selftest(&tb, &zipapp, "2026.11.23", None, dir.path()).await {
            Verdict::Pass(why) => assert!(why.contains("离线自检"), "{why}"),
            v => panic!("{v:?}"),
        }
        // 坏的 zipapp（截断了、语法错误）
        std::fs::write(&zipapp, "echo 'SyntaxError: source code string cannot contain null bytes' >&2; exit 1\n").unwrap();
        assert!(matches!(selftest(&tb, &zipapp, "2026.11.23", None, dir.path()).await, Verdict::Fail(_)));
    }

    /// 用 zip 包解压：目录结构、执行权限、符号链接原样还原；`../` 这种路径不许解
    #[test]
    fn zip_extract() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let zp = dir.path().join("t.zip");
        {
            let mut z = ::zip::ZipWriter::new(std::fs::File::create(&zp).unwrap());
            let exe = ::zip::write::SimpleFileOptions::default().unix_permissions(0o755);
            let data = ::zip::write::SimpleFileOptions::default().unix_permissions(0o644);
            z.start_file(EXE, exe).unwrap();
            z.write_all(b"#!/bin/sh\necho 2026.10.26\n").unwrap();
            z.add_directory("_internal/", data).unwrap();
            z.start_file("_internal/base_library.zip", data).unwrap();
            z.write_all(b"py").unwrap();
            z.add_symlink("_internal/libpython.so", "base_library.zip", data).unwrap();
            z.finish().unwrap();
        }
        let out = dir.path().join("out");
        extract_zip(&zp, &out).unwrap();
        assert_eq!(std::fs::read(out.join("_internal/base_library.zip")).unwrap(), b"py");
        assert_eq!(exe_in(&out), out.join(EXE));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(out.join(EXE)).unwrap().permissions().mode() & 0o777, 0o755);
            assert_eq!(std::fs::read_link(out.join("_internal/libpython.so")).unwrap(), Path::new("base_library.zip"));
            assert_eq!(std::fs::read(out.join("_internal/libpython.so")).unwrap(), b"py");
        }
        // 往外跑的路径
        let bad = dir.path().join("bad.zip");
        {
            let mut z = ::zip::ZipWriter::new(std::fs::File::create(&bad).unwrap());
            z.start_file("../evil", ::zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        assert!(extract_zip(&bad, &dir.path().join("out2")).is_err());
        assert!(!dir.path().join("evil").exists());
        #[cfg(unix)]
        {
            let link = dir.path().join("link.zip");
            let mut z = ::zip::ZipWriter::new(std::fs::File::create(&link).unwrap());
            z.add_symlink("x", "../../etc/passwd", ::zip::write::SimpleFileOptions::default()).unwrap();
            z.finish().unwrap();
            assert!(extract_zip(&link, &dir.path().join("out3")).is_err());
        }
        assert!(platform_ok() || !cfg!(any(target_os = "macos", windows, target_os = "linux")));
    }
}
