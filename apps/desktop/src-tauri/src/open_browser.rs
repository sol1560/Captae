//! 用指定的浏览器打开一个地址（「浏览器插件」页、上手引导里的「打开扩展程序页面」，地址是 chrome://extensions 这类内部页面，
//! 系统的「用默认浏览器打开」不认，只能把地址当参数交给浏览器程序）。
//!
//! - macOS：`open -a <应用程序名> <地址>`，应用程序名来自浏览器表（`native_host::browsers` 的 `app`）。
//! - Windows：先查注册表的 App Paths（浏览器安装时登记的 chrome.exe、msedge.exe……，只有正式版登记），再找常见的安装位置
//!   （%LOCALAPPDATA%、%ProgramFiles%、%ProgramFiles(x86)% 下面）。
//! - Linux：PATH 上的命令（各发行版、官方 deb / rpm 的名字不一样，挨个试）；Flatpak 装的按浏览器表里的数据目录认出应用 ID，用 `flatpak run`。
//!
//! 浏览器表（哪几家、叫什么、数据目录）归核心库；这里只管「怎么把它起起来」。表里没有这一家时也照样按 id 试，只是名字用 id。

use captae_core::native_host::Browser;
use captae_core::tr;
#[cfg(not(target_os = "macos"))]
use std::path::PathBuf;

/// 打开 `url`。找不到这家浏览器、起不来时返回给人看的错误（界面退回「复制地址 + 显示插件文件夹」）
pub fn open(id: &str, browser: Option<&Browser>, url: &str) -> Result<(), String> {
    let name = browser.map(|b| b.name).unwrap_or(id);
    let not_found = || tr!("没有找到 {name}，请先安装它，或者换一个浏览器", name = name);
    #[cfg(target_os = "macos")]
    {
        let b = browser.ok_or_else(not_found)?;
        // 应用程序包的名字去掉 .app 就是 `open -a` 要的名字
        let app = b.app.trim_end_matches(".app");
        let ok = std::process::Command::new("open").args(["-a", app, url]).status().map(|s| s.success()).unwrap_or(false);
        if ok {
            Ok(())
        } else {
            Err(not_found())
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let (program, mut args) = command(id, browser).ok_or_else(not_found)?;
        args.push(url.into());
        tracing::info!("用 {} 打开 {url}", program.display());
        let mut child = std::process::Command::new(&program).args(&args).spawn().map_err(|err| {
            tracing::warn!("起不来 {}：{err}", program.display());
            not_found()
        })?;
        // 浏览器已经开着时，新起的这个进程把地址交给开着的那个就退出了：在后台等它，不留僵尸进程
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(())
    }
}

/// 要运行的程序和参数（地址另加在最后）
#[cfg(windows)]
fn command(id: &str, _browser: Option<&Browser>) -> Option<(PathBuf, Vec<String>)> {
    // (App Paths 里登记的名字（只有正式版有），相对安装位置的路径)
    let (registered, rel): (Option<&str>, &[&str]) = match id {
        "chrome" => (Some("chrome.exe"), &[r"Google\Chrome\Application\chrome.exe"]),
        "chrome-beta" => (None, &[r"Google\Chrome Beta\Application\chrome.exe"]),
        "chrome-dev" => (None, &[r"Google\Chrome Dev\Application\chrome.exe"]),
        "chrome-canary" => (None, &[r"Google\Chrome SxS\Application\chrome.exe"]),
        "chromium" => (None, &[r"Chromium\Application\chrome.exe"]),
        "edge" => (Some("msedge.exe"), &[r"Microsoft\Edge\Application\msedge.exe"]),
        "edge-beta" => (None, &[r"Microsoft\Edge Beta\Application\msedge.exe"]),
        "edge-dev" => (None, &[r"Microsoft\Edge Dev\Application\msedge.exe"]),
        "edge-canary" => (None, &[r"Microsoft\Edge SxS\Application\msedge.exe"]),
        "brave" => (Some("brave.exe"), &[r"BraveSoftware\Brave-Browser\Application\brave.exe"]),
        "brave-beta" => (None, &[r"BraveSoftware\Brave-Browser-Beta\Application\brave.exe"]),
        "brave-nightly" => (None, &[r"BraveSoftware\Brave-Browser-Nightly\Application\brave.exe"]),
        "vivaldi" => (Some("vivaldi.exe"), &[r"Vivaldi\Application\vivaldi.exe"]),
        "opera" => (Some("opera.exe"), &[r"Programs\Opera\opera.exe", r"Opera\opera.exe"]),
        "opera-gx" => (None, &[r"Programs\Opera GX\opera.exe", r"Opera GX\opera.exe"]),
        _ => return None,
    };
    if let Some(p) = registered.and_then(app_path) {
        return Some((p, vec![]));
    }
    let bases: Vec<PathBuf> = ["LOCALAPPDATA", "ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"].iter().filter_map(std::env::var_os).map(PathBuf::from).collect();
    bases.iter().flat_map(|b| rel.iter().map(move |r| b.join(r))).find(|p| p.is_file()).map(|p| (p, vec![]))
}

/// 注册表 `…\CurrentVersion\App Paths\<exe>` 的默认值（先当前用户、再整台电脑）
#[cfg(windows)]
fn app_path(exe: &str) -> Option<PathBuf> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;
    let key = format!(r"Software\Microsoft\Windows\CurrentVersion\App Paths\{exe}");
    [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE].into_iter().find_map(|hive| {
        let v: String = RegKey::predef(hive).open_subkey(&key).ok()?.get_value("").ok()?;
        let p = PathBuf::from(v.trim().trim_matches('"'));
        p.is_file().then_some(p)
    })
}

/// Linux：Flatpak 装的（数据目录在 ~/.var/app/<应用 ID>/ 下面）用 `flatpak run <应用 ID>`；其余在 PATH 上找命令
#[cfg(all(unix, not(target_os = "macos")))]
fn command(id: &str, browser: Option<&Browser>) -> Option<(PathBuf, Vec<String>)> {
    if let Some(app_id) = browser.and_then(|b| flatpak_id(&b.root)) {
        let flatpak = captae_core::tools::which("flatpak")?;
        return Some((flatpak, vec!["run".into(), app_id]));
    }
    let names: &[&str] = match id {
        "chrome" => &["google-chrome-stable", "google-chrome"],
        "chrome-beta" => &["google-chrome-beta"],
        "chrome-dev" => &["google-chrome-unstable", "google-chrome-dev"],
        "chromium" => &["chromium", "chromium-browser"],
        "edge" => &["microsoft-edge-stable", "microsoft-edge"],
        "edge-beta" => &["microsoft-edge-beta"],
        "edge-dev" => &["microsoft-edge-dev"],
        "brave" => &["brave-browser", "brave-browser-stable", "brave"],
        "brave-beta" => &["brave-browser-beta"],
        "brave-nightly" => &["brave-browser-nightly"],
        "vivaldi" => &["vivaldi-stable", "vivaldi"],
        "vivaldi-snapshot" => &["vivaldi-snapshot"],
        "opera" => &["opera"],
        "opera-beta" => &["opera-beta"],
        "opera-developer" => &["opera-developer"],
        _ => return None,
    };
    names.iter().find_map(|n| captae_core::tools::which(n)).map(|p| (p, vec![]))
}

/// `~/.var/app/com.google.Chrome/config/google-chrome` → `com.google.Chrome`
#[cfg(all(unix, not(target_os = "macos")))]
fn flatpak_id(root: &std::path::Path) -> Option<String> {
    let parts: Vec<_> = root.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
    let i = parts.windows(2).position(|w| w[0] == ".var" && w[1] == "app")?;
    parts.get(i + 2).filter(|s| !s.is_empty()).cloned()
}

#[cfg(all(test, all(unix, not(target_os = "macos"))))]
mod tests {
    #[test]
    fn flatpak_ids() {
        use std::path::Path;
        assert_eq!(super::flatpak_id(Path::new("/home/a/.var/app/com.google.Chrome/config/google-chrome")).as_deref(), Some("com.google.Chrome"));
        assert_eq!(super::flatpak_id(Path::new("/home/a/.config/google-chrome")), None);
    }
}
