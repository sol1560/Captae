//! 登录和人机验证（计划第 3 节）：网站规则、什么时候带登录状态、临时 cookies 文件、给下载工具的参数。
//!
//! 登录状态从三个地方来，同一次下载只用其中一个（按这个顺序）：
//! 1. 浏览器插件带来的 cookies（`AddRequest.cookies` 和 `user_agent`）
//! 2. Captae 登录窗口的存储：桌面端实现 [`Store`]，按网站的域名读出来
//! 3. 设置里打开了「也读取浏览器里的登录状态」：yt-dlp `--cookies-from-browser`
//!
//! YouTube 例外：用账号的 cookies 大量下载，账号可能被限制（yt-dlp 官方的提醒），所以默认不带，
//! 只有这个任务报过 `need_login` / `need_verify`（年龄限制、会员、要确认不是机器人）以后才带。
//!
//! 规矩：cookie 的值只在内存里和临时文件里出现。临时文件放在 `<data>/work/auth/`（目录 700、文件 600），
//! 只给这一次的子进程用，进程结束（成功、失败、取消）就删；不写进 queue.json、任务记录、错误原文，不给 AI 助手。
//! 子进程的报错在分类之前先用 [`Auth::redact`] 把 cookie 的值抹掉。

use crate::config::Paths;
use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ------------------------------------------------------------------ 网站

/// 一个内置了规则的网站。
#[derive(Debug)]
pub struct Site {
    /// 代号：youtube / bilibili / x …（界面的图标、设置里的一行都用它）
    pub id: &'static str,
    /// 网站名。中文名（哔哩哔哩、抖音）存的是原文，显示时用 `i18n::dynamic` 翻译（表里用 tr_key! 标过）
    pub name: &'static str,
    /// 这些域名和它们的子域名都算这个网站；从登录存储里也只挑这些域名的 cookies
    pub domains: &'static [&'static str],
    /// 出现其中任何一个（有值、没过期）就算登录了。规则来自 yt-dlp 各网站提取器认的 cookie
    pub login_cookies: &'static [&'static str],
    /// 登录窗口打开的地址
    pub login_url: &'static str,
    /// 登录以后有什么用（设置里那一行的说明）。中文原文，显示时用 `i18n::dynamic` 翻译
    pub gain: &'static str,
    /// 设置里默认列出来（没登录也列，方便主动登录）
    pub featured: bool,
    /// 「退出」时另外要清掉的域名（YouTube 是用 Google 账号登录的，退出时 Google 的登录也清掉）
    pub logout_also: &'static [&'static str],
}

/// YouTube 网页上「登录」按钮的地址：Google 登录页，登录好跳回 youtube.com（YouTube 的 cookies 在跳回来时种上）
pub const YOUTUBE_LOGIN: &str = "https://accounts.google.com/ServiceLogin?service=youtube&uilel=3&passive=true&continue=https%3A%2F%2Fwww.youtube.com%2Fsignin%3Faction_handle_signin%3Dtrue%26app%3Ddesktop%26hl%3Dzh-CN%26next%3Dhttps%253A%252F%252Fwww.youtube.com%252F&hl=zh-CN";

pub static SITES: &[Site] = &[
    Site {
        id: "youtube",
        name: "YouTube",
        domains: &["youtube.com", "youtu.be", "youtube-nocookie.com"],
        // yt-dlp youtube/_base.py：SAPISID，没有时用 __Secure-3PAPISID
        login_cookies: &["SAPISID", "__Secure-3PAPISID"],
        login_url: YOUTUBE_LOGIN,
        gain: crate::tr_key!("只在年龄限制、会员视频、要确认不是机器人时才带上"),
        featured: true,
        logout_also: &["google.com"],
    },
    Site {
        id: "bilibili",
        name: crate::tr_key!("哔哩哔哩"),
        domains: &["bilibili.com", "b23.tv"],
        login_cookies: &["SESSDATA"],
        login_url: "https://passport.bilibili.com/login",
        gain: crate::tr_key!("可以下 1080P 以上和大会员视频"),
        featured: true,
        logout_also: &[],
    },
    Site {
        id: "x",
        name: "X",
        domains: &["x.com", "twitter.com"],
        login_cookies: &["auth_token"],
        login_url: "https://x.com/i/flow/login",
        gain: crate::tr_key!("受限的帖子要登录才能下"),
        featured: true,
        logout_also: &[],
    },
    Site {
        id: "instagram",
        name: "Instagram",
        domains: &["instagram.com"],
        login_cookies: &["sessionid"],
        login_url: "https://www.instagram.com/accounts/login/",
        gain: crate::tr_key!("大部分帖子要登录才能下"),
        featured: true,
        logout_also: &[],
    },
    Site {
        id: "douyin",
        name: crate::tr_key!("抖音"),
        domains: &["douyin.com", "iesdouyin.com"],
        // yt-dlp tiktok.py：登录看 sid_tt；「要新的 cookies」只要在网页里打开过一次（s_v_web_id），不一定要登录
        login_cookies: &["sid_tt", "sessionid"],
        login_url: "https://www.douyin.com/",
        gain: crate::tr_key!("提示要新的 cookies 时，在网页里打开一次就行"),
        featured: true,
        logout_also: &[],
    },
    Site {
        id: "tiktok",
        name: "TikTok",
        domains: &["tiktok.com"],
        login_cookies: &["sid_tt", "sessionid"],
        login_url: "https://www.tiktok.com/login",
        gain: crate::tr_key!("年龄限制、私密视频要登录"),
        featured: true,
        logout_also: &[],
    },
    Site {
        id: "facebook",
        name: "Facebook",
        domains: &["facebook.com", "fb.watch"],
        login_cookies: &["c_user"],
        login_url: "https://www.facebook.com/login/",
        gain: crate::tr_key!("只对好友可见的视频要登录"),
        featured: false,
        logout_also: &[],
    },
    Site {
        id: "twitch",
        name: "Twitch",
        domains: &["twitch.tv"],
        login_cookies: &["auth-token"],
        login_url: "https://www.twitch.tv/login",
        gain: crate::tr_key!("订阅专享的回放要登录"),
        featured: false,
        logout_also: &[],
    },
    Site {
        id: "reddit",
        name: "Reddit",
        domains: &["reddit.com", "redd.it"],
        login_cookies: &["reddit_session"],
        login_url: "https://www.reddit.com/login/",
        gain: crate::tr_key!("成人内容和私密社区要登录"),
        featured: false,
        logout_also: &[],
    },
    Site {
        id: "niconico",
        name: "niconico",
        domains: &["nicovideo.jp"],
        login_cookies: &["user_session"],
        login_url: "https://account.nicovideo.jp/login",
        gain: crate::tr_key!("高画质和会员视频要登录"),
        featured: false,
        logout_also: &[],
    },
    Site {
        id: "xiaohongshu",
        name: crate::tr_key!("小红书"),
        // 2026-11 起大陆以外打开 xiaohongshu.com 会 302 到 rednote.com，登录后的 web_session 种在 .rednote.com 上，
        // 不列进来就认不出已经登录，下载时也带不上（yt-dlp 请求 xiaohongshu.com 时同样会被跳到 rednote.com）
        domains: &["xiaohongshu.com", "xhslink.com", "rednote.com"],
        login_cookies: &["web_session"],
        // 不直接写 rednote.com：大陆打开不跳，留在 xiaohongshu.com，让网站自己决定
        login_url: "https://www.xiaohongshu.com/explore",
        gain: crate::tr_key!("部分笔记要登录才能看"),
        featured: false,
        logout_also: &[],
    },
];

/// 登录存储的 UUID（WKWebsiteDataStore(forIdentifier:)，所有网站共用这一个）。
pub const STORE_UUID: [u8; 16] = [0x5c, 0x1a, 0x7e, 0x0f, 0x3b, 0x9d, 0x4d, 0x2e, 0x8a, 0x61, 0xc7, 0xf0, 0xe4, 0xb2, 0xd9, 0x13];

/// 正式版用固定的 [`STORE_UUID`]；测试版（设了 CAPTAE_HOME）由数据目录算一个，测试碰不到正式版的登录存储。
pub fn store_uuid(test_home: Option<&Path>) -> [u8; 16] {
    let Some(h) = test_home else { return STORE_UUID };
    use sha2::Digest;
    let d = sha2::Sha256::digest(format!("captae-login-store:{}", h.display()).as_bytes());
    let mut b = [0u8; 16];
    b.copy_from_slice(&d[..16]);
    // 标成第 4 版 UUID
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    b
}

/// 网址的主机名（小写、去掉 www.）
pub fn host_of(url: &str) -> Option<String> {
    let u = url::Url::parse(url.trim()).ok()?;
    if !matches!(u.scheme(), "http" | "https") {
        return None;
    }
    let h = u.host_str()?.to_ascii_lowercase();
    Some(h.strip_prefix("www.").map(str::to_string).unwrap_or(h))
}

/// `host` 是 `domain` 本身或它的子域名
pub fn host_under(host: &str, domain: &str) -> bool {
    let (h, d) = (host.trim_start_matches('.'), domain.trim_start_matches('.'));
    !d.is_empty() && (h.eq_ignore_ascii_case(d) || (h.len() > d.len() && h.to_ascii_lowercase().ends_with(&format!(".{}", d.to_ascii_lowercase()))))
}

pub fn site_by_id(id: &str) -> Option<&'static Site> {
    SITES.iter().find(|s| s.id == id)
}

/// 链接属于哪个内置网站
pub fn site_of(url: &str) -> Option<&'static Site> {
    let h = host_of(url)?;
    SITES.iter().find(|s| s.domains.iter().any(|d| host_under(&h, d)))
}

pub fn is_youtube(url: &str) -> bool {
    site_of(url).is_some_and(|s| s.id == "youtube")
}

/// 链接 → 网站代号：内置网站用 id，其他网站用 `host:主机名`（去掉 www.）
pub fn site_key(url: &str) -> Option<String> {
    if let Some(s) = site_of(url) {
        return Some(s.id.to_string());
    }
    host_of(url).map(|h| format!("host:{h}"))
}

/// 网站代号 → 登录存储里要挑哪些域名的 cookies
pub fn key_domains(key: &str) -> Vec<String> {
    match key.strip_prefix("host:") {
        Some(h) => vec![h.to_string()],
        None => site_by_id(key).map(|s| s.domains.iter().map(|d| d.to_string()).collect()).unwrap_or_default(),
    }
}

/// 这个链接属于这个网站代号吗（内置网站按域名；`host:` 按主机名，子域名也算）
pub fn url_in_key(url: &str, key: &str) -> bool {
    match (key.strip_prefix("host:"), host_of(url)) {
        (Some(k), Some(h)) => host_under(&h, k),
        (None, _) => site_of(url).is_some_and(|s| s.id == key),
        _ => false,
    }
}

/// 网站代号 → 显示名（「哔哩哔哩」「example.com」）。返回的是原文（存进数据里的名字），给人看时再 `i18n::dynamic`
pub fn key_name(key: &str) -> String {
    match key.strip_prefix("host:") {
        Some(h) => h.to_string(),
        None => site_by_id(key).map(|s| s.name.to_string()).unwrap_or_else(|| key.to_string()),
    }
}

/// 这个网站在这些 cookies 里是不是登录着（看网站规则里的登录 cookie）。没有规则的网站（host:…）永远 false
pub fn logged_in(key: &str, cookies: &[Cookie], now: i64) -> bool {
    let Some(site) = site_by_id(key) else { return false };
    cookies.iter().any(|c| site.login_cookies.contains(&c.name.as_str()) && !c.value.is_empty() && !c.expired(now) && site.domains.iter().any(|d| host_under(c.bare_domain(), d)))
}

// ------------------------------------------------------------------ cookie

/// 一个 cookie。来自登录窗口的存储或浏览器插件（`chrome.cookies.getAll` 的结果可以原样传过来：
/// name / value / domain / path / secure / httpOnly / hostOnly / expirationDate）。
///
/// 值只在内存和临时文件里出现：`Debug` 不打印值，`AddRequest` 写 queue.json 前会把它去掉。
#[derive(Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cookie {
    pub name: String,
    #[serde(default)]
    pub value: String,
    /// `.bilibili.com`（以点开头：子域名也带）或 `www.bilibili.com`（只给这一个主机）
    pub domain: String,
    #[serde(default = "root_path")]
    pub path: String,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    /// chrome.cookies 的 hostOnly：为 true 时只给 `domain` 这一个主机（不管前面有没有点）
    #[serde(default)]
    pub host_only: bool,
    /// 过期时间（Unix 秒）；没有 = 会话 cookie。插件传 `expirationDate`（带小数的秒）也认
    #[serde(default, alias = "expirationDate", deserialize_with = "de_secs", skip_serializing_if = "Option::is_none")]
    pub expires: Option<i64>,
}

fn root_path() -> String {
    "/".into()
}

fn de_secs<'de, D: Deserializer<'de>>(d: D) -> std::result::Result<Option<i64>, D::Error> {
    let v = Option::<serde_json::Value>::deserialize(d)?;
    Ok(v.and_then(|v| v.as_f64()).filter(|x| x.is_finite() && *x > 0.0).map(|x| x as i64))
}

impl fmt::Debug for Cookie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 值不打印：AddRequest、任务快照可能被 {:?} 打进日志
        f.debug_struct("Cookie").field("name", &self.name).field("domain", &self.domain).field("path", &self.path).field("value", &"<隐藏>").finish()
    }
}

impl Cookie {
    pub fn bare_domain(&self) -> &str {
        self.domain.trim_start_matches('.')
    }
    /// 只给 `domain` 这一个主机（没有前导点，或者插件说了 hostOnly）
    pub fn is_host_only(&self) -> bool {
        self.host_only || !self.domain.starts_with('.')
    }
    pub fn expired(&self, now: i64) -> bool {
        self.expires.is_some_and(|t| t <= now)
    }
    /// 能安全写进 Netscape 文件、放进 Cookie 请求头：名字不空，名字和值里没有制表符、换行和其他控制字符，
    /// 名字里没有 `=` `;`。不合格的跳过——yt-dlp 读到格式不对的行会把整行（连值）打进报错
    pub fn writable(&self) -> bool {
        let bad = |s: &str| s.chars().any(|c| c.is_control());
        !self.name.is_empty() && !bad(&self.name) && !bad(&self.value) && !self.name.contains(['=', ';', ' ']) && !self.value.contains(';') && !self.bare_domain().is_empty() && !bad(&self.domain) && !bad(&self.path) && !self.domain.contains(' ')
    }
    /// 和这个域名有关：cookie 的域名是它本身、它的子域名，或者（不是只给一个主机的）它的上级域名
    pub fn related_to(&self, domain: &str) -> bool {
        let cd = self.bare_domain();
        host_under(cd, domain) || (!self.is_host_only() && host_under(domain, cd))
    }
    /// 浏览器会不会把它发给这个地址（域名、路径、https、过期）
    pub fn applies_to(&self, url: &url::Url, now: i64) -> bool {
        let Some(host) = url.host_str() else { return false };
        let domain_ok = if self.is_host_only() { host.eq_ignore_ascii_case(self.bare_domain()) } else { host_under(host, self.bare_domain()) };
        let p = self.path.as_str();
        let path_ok = p.is_empty() || p == "/" || url.path() == p || url.path().starts_with(&format!("{}/", p.trim_end_matches('/'))) || (p.ends_with('/') && url.path().starts_with(p));
        domain_ok && path_ok && (!self.secure || url.scheme() == "https") && !self.expired(now) && self.writable()
    }
}

/// 插件抓到的 `Cookie:` 请求头（`a=1; b=2`）→ 只给这个网址的主机的 cookies。
pub fn from_header(header: &str, url: &str) -> Vec<Cookie> {
    let Ok(u) = url::Url::parse(url) else { return vec![] };
    let Some(host) = u.host_str() else { return vec![] };
    header
        .split(';')
        .filter_map(|p| {
            let (n, v) = p.trim().split_once('=')?;
            let n = n.trim();
            (!n.is_empty()).then(|| Cookie {
                name: n.to_string(),
                value: v.trim().to_string(),
                domain: host.to_ascii_lowercase(),
                path: "/".into(),
                secure: u.scheme() == "https",
                http_only: false,
                host_only: true,
                expires: None,
            })
        })
        .collect()
}

/// Netscape 格式（yt-dlp `--cookies`、aria2 `--load-cookies` 都认）。
/// 不写 `#HttpOnly_` 前缀：aria2 会把这种行当注释跳过，对下载工具也没有意义。
/// 第一列以点开头 ⇔ 第二列 TRUE，两边不一致 yt-dlp 会报错并把整行打出来。
pub fn netscape(cookies: &[Cookie], now: i64) -> String {
    let mut s = String::from("# Netscape HTTP Cookie File\n# Written by Captae for a single download; deleted when it ends.\n\n");
    for c in cookies.iter().filter(|c| c.writable() && !c.expired(now)) {
        let (domain, sub) = if c.is_host_only() { (c.bare_domain().to_string(), "FALSE") } else { (format!(".{}", c.bare_domain()), "TRUE") };
        let path = if c.path.is_empty() { "/" } else { c.path.as_str() };
        s.push_str(&format!("{domain}\t{sub}\t{path}\t{}\t{}\t{}\t{}\n", if c.secure { "TRUE" } else { "FALSE" }, c.expires.unwrap_or(0), c.name, c.value));
    }
    s
}

/// 放临时 cookies 文件的目录：`<data>/work/auth`
pub fn auth_dir(paths: &Paths) -> PathBuf {
    paths.work.join("auth")
}

/// 启动时清掉上次异常退出留下的临时 cookies 文件（比 `older` 还旧的）
pub fn sweep(paths: &Paths, older: std::time::Duration) {
    let Ok(rd) = std::fs::read_dir(auth_dir(paths)) else { return };
    for e in rd.flatten() {
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_none_or(|age| age >= older);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// 临时 cookies 文件：权限 600，丢掉这个值时删除（成功、失败、取消都会走到）。
pub struct CookieFile {
    path: PathBuf,
}

impl CookieFile {
    pub fn write(dir: &Path, cookies: &[Cookie]) -> Result<CookieFile> {
        std::fs::create_dir_all(dir).with_context(|| crate::tr!("建不了 {path}", path = dir.display()))?;
        let name: String = {
            use rand::Rng;
            let mut r = rand::rng();
            (0..12).map(|_| format!("{:02x}", r.random::<u8>())).collect()
        };
        let path = dir.join(format!("c-{name}.txt"));
        let body = netscape(cookies, crate::util::now_ms() / 1000);
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            // 建文件时就是 600（不是先建再改权限，中间没有别人能读的空档）
            let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path)?;
            f.write_all(body.as_bytes())?;
        }
        #[cfg(not(unix))]
        std::fs::write(&path, body)?;
        Ok(CookieFile { path })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for CookieFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

impl fmt::Debug for CookieFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CookieFile").field("path", &self.path).finish()
    }
}

// ------------------------------------------------------------------ 登录过的网站

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum How {
    /// 登录了账号
    Login,
    /// 做过人机验证（没有固定的 cookie，用户点了「完成」）
    Verify,
}

/// 在 Captae 里登录过 / 验证过的一个网站。只有网站和时间，没有任何 cookie。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// 网站代号（youtube / host:example.com）
    pub site: String,
    pub how: How,
    /// 什么时候（毫秒）；登录存储里本来就有、Captae 没记下时间的是 None
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<i64>,
}

/// `<data>/logins.json`：登录过哪些网站、提示过哪些事。下载时据此决定要不要去读登录存储。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Logins {
    pub sites: Vec<Record>,
    /// 已经提示过的事（比如 "youtube"：第一次用 YouTube 账号下载时提示过）
    pub told: Vec<String>,
}

impl Logins {
    pub fn load(p: &Path) -> Logins {
        std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
    pub fn save(&self, p: &Path) -> Result<()> {
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(tmp, p)?;
        Ok(())
    }
    pub fn get(&self, key: &str) -> Option<&Record> {
        self.sites.iter().find(|r| r.site == key)
    }
    /// 记一笔（同一个网站只留一条）。登录过的再做验证，仍算登录
    pub fn mark(&mut self, key: &str, how: How, at: Option<i64>) {
        match self.sites.iter_mut().find(|r| r.site == key) {
            Some(r) => {
                if how == How::Login || r.how != How::Login {
                    r.how = how;
                }
                if at.is_some() {
                    r.at = at;
                }
            }
            None => self.sites.push(Record { site: key.to_string(), how, at }),
        }
    }
    pub fn forget(&mut self, key: &str) -> bool {
        let n = self.sites.len();
        self.sites.retain(|r| r.site != key);
        n != self.sites.len()
    }
    /// 这个链接属于哪个登录过 / 验证过的网站：内置网站按代号；其他网站按主机名，子域名也算
    pub fn find_for_url(&self, url: &str) -> Option<&Record> {
        let key = site_key(url)?;
        if !key.starts_with("host:") {
            return self.get(&key);
        }
        let h = host_of(url)?;
        self.sites.iter().filter_map(|r| r.site.strip_prefix("host:").map(|d| (r, d))).filter(|(_, d)| host_under(&h, d)).max_by_key(|(_, d)| d.len()).map(|(r, _)| r)
    }
    /// 第一次返回 true（并记下），之后都是 false
    pub fn tell_once(&mut self, what: &str) -> bool {
        if self.told.iter().any(|t| t == what) {
            return false;
        }
        self.told.push(what.to_string());
        true
    }
}

// ------------------------------------------------------------------ 什么时候带

/// `--cookies-from-browser` 能用的浏览器（yt-dlp 2026.10.26 的 cookies.py；Arc 不支持：它的钥匙串项 yt-dlp 解不开）。
/// 这是三个系统的总表，Safari 只在 macOS 上有；这台电脑上能选的见 [`browsers`]
pub const BROWSERS: &[(&str, &str)] = &[
    ("chrome", "Chrome"),
    ("safari", "Safari"),
    ("edge", "Edge"),
    ("brave", "Brave"),
    ("firefox", "Firefox"),
    ("vivaldi", "Vivaldi"),
    ("opera", "Opera"),
    ("chromium", "Chromium"),
];

/// 这台电脑上能选的浏览器（设置里列的顺序）。
/// - macOS：总表原样。
/// - Windows：没有 Safari；Firefox 放第一个，因为 Chrome 127 起（Edge 等 Chromium 一家陆续跟进）cookie 用了 app-bound 加密，
///   yt-dlp 只能按 DPAPI 解，解不开就报 `Failed to decrypt with DPAPI` 并让整个下载失败；浏览器开着时 cookie 数据库还被锁着
///   （`Could not copy Chrome cookie database`）。yt-dlp 到 2026 年也没有解决（issue #10927），所以 Chromium 一家标成「通常读不了」（[`browser_readable`]）。
/// - Linux：没有 Safari。Chromium 一家的 cookie 用 GNOME Keyring / KWallet 里的密钥解密，yt-dlp 按桌面环境自己挑（`_choose_linux_keyring`），
///   Captae 不加 `chrome+gnomekeyring` 这种写法：挑错的情况（比如 LXQt 走 BASICTEXT）手动指定也只是换一种错法，不如交给 yt-dlp。
pub fn browsers() -> Vec<(&'static str, &'static str)> {
    let mut v: Vec<(&str, &str)> = BROWSERS.iter().copied().filter(|(id, _)| cfg!(target_os = "macos") || *id != "safari").collect();
    if cfg!(windows) {
        v.sort_by_key(|(id, _)| *id != "firefox");
    }
    v
}

/// 这个浏览器的登录状态在这台电脑上通常读不读得了（设置里据此写说明）。Windows 上 Chromium 一家读不了，见 [`browsers`]
pub fn browser_readable(id: &str) -> bool {
    !(cfg!(windows) && id != "firefox")
}

/// 设置里的值 → `--cookies-from-browser` 的值；空、不认识的、这个系统上没有的（别的系统上的设置里选了 Safari）→ None
pub fn browser_arg(setting: &str) -> Option<&'static str> {
    let s = setting.trim().to_ascii_lowercase();
    browsers().into_iter().find(|(id, _)| *id == s).map(|(id, _)| id)
}

/// 浏览器的名字（Chrome、Safari……不翻译）；不认识的说「浏览器」（跟着界面语言）
pub fn browser_name(id: &str) -> String {
    BROWSERS.iter().find(|(b, _)| *b == id).map(|(_, n)| n.to_string()).unwrap_or_else(|| crate::tr!("浏览器"))
}

/// yt-dlp 读浏览器的登录状态失败了：给人看的一句话（不认识的报错返回 None，照常分类）。
/// 原文是 yt-dlp cookies.py 里的报错，读浏览器失败会让整个读取 / 下载失败，所以要说清楚是哪一步、怎么办
pub fn browser_failure_hint(text: &str, browser: &str) -> Option<String> {
    let l = text.to_ascii_lowercase();
    let name = browser_name(browser);
    if l.contains("failed to decrypt with dpapi") || l.contains("cannot decrypt v20") || l.contains("app-bound") {
        // Windows：Chrome 127 起的 app-bound 加密
        Some(crate::tr!("读不了 {browser} 里的登录状态：{browser} 在 Windows 上给登录信息加了只有它自己能解开的加密。请在设置里改读 Firefox，或者在 Captae 里登录这个网站", browser = name))
    } else if l.contains("could not copy chrome cookie database") {
        // Windows：浏览器开着时把 cookie 数据库锁住了
        Some(crate::tr!("读不了 {browser} 里的登录状态：{browser} 开着时不让别的程序读。关掉 {browser} 再试，或者在 Captae 里登录这个网站", browser = name))
    } else if l.contains("failed to read from keyring") || l.contains("secretstorage not available") || l.contains("kwallet-query") || l.contains("failed to read networkwallet") {
        // Linux：GNOME Keyring / KWallet 里取不到解密用的密钥
        Some(crate::tr!("读不了 {browser} 里的登录状态：系统的密钥环（GNOME Keyring / KWallet）里取不到解密用的密钥。确认密钥环已经解锁，或者在 Captae 里登录这个网站", browser = name))
    } else if l.contains("could not find") && l.contains("cookies database") {
        Some(crate::tr!("没找到 {browser} 的登录信息：{browser} 没装，或者装在了 yt-dlp 找不到的地方（Flatpak、Snap 版）", browser = name))
    } else {
        None
    }
}

/// 这次下载用哪一份登录状态。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    /// 不带
    None,
    /// 浏览器插件带来的 cookies
    Extension,
    /// Captae 登录窗口的存储里这个网站的 cookies（网站代号）
    Store(String),
    /// yt-dlp `--cookies-from-browser`
    Browser(&'static str),
}

/// 决定带不带登录状态（纯逻辑，单元测试覆盖）。
/// - `has_ext`：插件带了 cookies
/// - `needed`：这个任务报过 need_login / need_verify（YouTube 只有这时才带）
/// - `browser`：设置里「也读取浏览器里的登录状态」选的浏览器（空 = 关）
pub fn plan(url: &str, page_url: Option<&str>, has_ext: bool, needed: bool, logins: &Logins, browser: &str) -> Plan {
    let youtube = is_youtube(url) || page_url.is_some_and(is_youtube);
    if youtube && !needed {
        return Plan::None;
    }
    if has_ext {
        return Plan::Extension;
    }
    if let Some(r) = logins.find_for_url(url).or_else(|| page_url.and_then(|p| logins.find_for_url(p))) {
        return Plan::Store(r.site.clone());
    }
    match browser_arg(browser) {
        Some(b) => Plan::Browser(b),
        None => Plan::None,
    }
}

/// 有没有现成的登录状态可以试（YouTube 报 need_login 时，据此决定要不要当场带上再读一次）
pub fn available(url: &str, page_url: Option<&str>, has_ext: bool, logins: &Logins, browser: &str) -> bool {
    plan(url, page_url, has_ext, true, logins, browser) != Plan::None
}

/// 登录窗口的存储，由桌面端实现（命令行、MCP 自己起的进程没有）。
pub trait Store: Send + Sync + 'static {
    /// 和这些域名有关的 cookies（域名本身、子域名、上级域名的；见 [`Cookie::related_to`]）
    fn cookies(&self, domains: Vec<String>) -> futures_util::future::BoxFuture<'static, Result<Vec<Cookie>>>;
}

/// 桌面端从 WebView2 读到的、网站那块实际用的 User-Agent（Windows 上不改它，见 [`login_user_agent`]）
static LOGIN_UA: std::sync::RwLock<Option<String>> = std::sync::RwLock::new(None);

/// 桌面端告诉核心库：网站那块实际用的 User-Agent（Windows 上建好 webview 后从 WebView2 读出来）
pub fn set_login_user_agent(ua: String) {
    if !ua.trim().is_empty() {
        *LOGIN_UA.write().unwrap() = Some(ua);
    }
}

/// 登录窗口网站那块用的 User-Agent，下载时带同一个（Cloudflare 一类的验证结果和 User-Agent 绑在一起）。
/// - macOS、Linux：Safari 的（[`safari_user_agent`]）。macOS 的内核本来就是 Safari 的 WebKit；Linux 的 WebKitGTK 也是 WebKit，
///   它自己的 User-Agent 写着 X11 / Linux，Google 一类的网站会当成不常见的浏览器，照 macOS 的做法装成 Safari。
/// - Windows：WebView2 自己的（和 Edge 浏览器的写法一样）。WebView2 是 Chromium 内核，装成 Safari 反而对不上（TLS 指纹、Client Hints 都是 Chromium 的）。
///   桌面端读到以前用 [`edge_ua_for`] 拼一个。
pub fn login_user_agent() -> String {
    if let Some(ua) = LOGIN_UA.read().unwrap().clone() {
        return ua;
    }
    if cfg!(windows) {
        edge_ua_for("142")
    } else {
        safari_user_agent()
    }
}

/// Windows 上 Edge / WebView2 的 User-Agent（Chromium 只报主版本号，后面三段固定是 0）
pub fn edge_ua_for(major: &str) -> String {
    format!("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{major}.0.0.0 Safari/537.36 Edg/{major}.0.0.0")
}

/// 这台 Mac 上 Safari 的 User-Agent：登录窗口和下载用同一个，Cloudflare 一类的验证结果才能接着用。
/// 版本号从 /Applications/Safari.app 读；读不到（以及 Linux 上）用一个近期的版本。
pub fn safari_user_agent() -> String {
    static UA: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| safari_ua_for(&safari_version().unwrap_or_else(|| "18.6".into())));
    UA.clone()
}

pub fn safari_ua_for(version: &str) -> String {
    format!("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/{version} Safari/605.1.15")
}

fn safari_version() -> Option<String> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let plist = "/Applications/Safari.app/Contents/Info.plist";
    let out = std::process::Command::new("/usr/bin/plutil").args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-", plist]).output().ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !v.is_empty() && v.len() < 16 && v.chars().all(|c| c.is_ascii_digit() || c == '.')).then_some(v)
}

/// User-Agent → yt-dlp `--impersonate` 的浏览器（Chromium 一家都用 chrome 的 TLS 指纹）
pub fn ua_family(ua: &str) -> Option<&'static str> {
    let l = ua.to_ascii_lowercase();
    if l.contains("firefox/") {
        Some("firefox")
    } else if l.contains("chrome/") || l.contains("chromium/") || l.contains("edg/") {
        Some("chrome")
    } else if l.contains("safari/") && l.contains("version/") {
        Some("safari")
    } else {
        None
    }
}

/// `yt-dlp --list-impersonate-targets` 的输出 → [(浏览器, 系统)]，都是小写；标了 unavailable 的不要
pub fn parse_targets(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter(|l| !l.contains("unavailable") && !l.starts_with('[') && !l.starts_with('-'))
        .filter_map(|l| {
            let mut it = l.split_whitespace();
            let (client, os) = (it.next()?, it.next()?);
            let c = client.split('-').next()?.to_ascii_lowercase();
            let o = os.split('-').next()?.to_ascii_lowercase();
            (c != "client" && it.next().is_some()).then_some((c, o))
        })
        .collect()
}

/// User-Agent 里写的系统 → yt-dlp `--impersonate` 的系统名（windows / macos / linux / android / ios）
pub fn ua_os(ua: &str) -> Option<&'static str> {
    let l = ua.to_ascii_lowercase();
    if l.contains("windows nt") {
        Some("windows")
    } else if l.contains("iphone") || l.contains("ipad") {
        Some("ios")
    } else if l.contains("android") {
        Some("android")
    } else if l.contains("macintosh") || l.contains("mac os x") {
        Some("macos")
    } else if l.contains("linux") || l.contains("x11") {
        Some("linux")
    } else {
        None
    }
}

/// 选 `--impersonate` 的值：优先这个浏览器在 `os` 上的版本（和 User-Agent 写的系统一致），没有就不限系统，都没有返回 None（不加这个参数）
pub fn pick_impersonate(targets: &[(String, String)], family: &str, os: Option<&str>) -> Option<String> {
    if let Some(os) = os.filter(|o| targets.iter().any(|(c, t)| c == family && t == o)) {
        return Some(format!("{family}:{os}"));
    }
    targets.iter().any(|(c, _)| c == family).then(|| family.to_string())
}

/// 这个 yt-dlp 能模拟哪些浏览器（每个 yt-dlp 只问一次）。自己指定的 yt-dlp 可能没带 curl_cffi，
/// 这时加 `--impersonate` 会直接报错，所以先问。
pub async fn impersonate_targets(tools: &crate::tools::Toolbox) -> Vec<(String, String)> {
    static CACHE: once_cell::sync::Lazy<std::sync::Mutex<BTreeMap<PathBuf, Vec<(String, String)>>>> = once_cell::sync::Lazy::new(Default::default);
    let Ok(exe) = tools.path(crate::tools::Tool::YtDlp) else { return vec![] };
    if let Some(v) = CACHE.lock().unwrap().get(&exe) {
        return v.clone();
    }
    let mut c = tools.ytdlp_at(&exe);
    c.arg("--list-impersonate-targets");
    let v = match crate::tools::run_full(c, std::time::Duration::from_secs(60)).await {
        Ok(r) if r.ok => parse_targets(&format!("{}\n{}", String::from_utf8_lossy(&r.stdout), r.stderr)),
        _ => vec![],
    };
    CACHE.lock().unwrap().insert(exe, v.clone());
    v
}

/// 插件带来的 cookies → 这次要带的登录状态：同一个 User-Agent，模拟同一家浏览器（TLS 指纹）。
/// 交给 yt-dlp 时，模拟 Chrome 一家就用模拟出来的 User-Agent（见 `Auth::ytdlp`）；aria2 和 Captae 自己的请求用浏览器的
pub async fn extension_auth(cookies: Vec<Cookie>, user_agent: Option<String>, tools: &crate::tools::Toolbox, dir: PathBuf) -> Auth {
    let impersonate = match user_agent.as_deref().and_then(ua_family) {
        Some(f) => pick_impersonate(&impersonate_targets(tools).await, f, user_agent.as_deref().and_then(ua_os)),
        None => None,
    };
    let cookies = cookies.into_iter().filter(Cookie::writable).collect();
    Auth { cookies, user_agent, impersonate, browser: None, from: crate::tr!("浏览器插件带来的登录状态"), dir }
}

/// 登录窗口存储里读出来的 cookies → 这次要带的登录状态：和登录窗口同一个 User-Agent（[`login_user_agent`]），模拟同一家浏览器
/// （macOS、Linux 模拟 Safari，Windows 模拟 Chrome；Cloudflare 一类的验证结果和 User-Agent、TLS 指纹绑在一起）。
/// 模拟 Chrome 一家时 yt-dlp 不另加 User-Agent（见 [`Auth::ytdlp`]），aria2 和 Captae 自己的请求照样用登录窗口的
pub async fn store_auth(key: &str, how: How, cookies: Vec<Cookie>, tools: &crate::tools::Toolbox, dir: PathBuf) -> Auth {
    let ua = login_user_agent();
    let impersonate = pick_impersonate(&impersonate_targets(tools).await, ua_family(&ua).unwrap_or("safari"), ua_os(&ua));
    // 网站名按界面语言显示；「的 YouTube 账号」这类中英文之间的空格由 tr! 自动加（docs/I18N.md 2.2）
    let name = crate::i18n::dynamic(&key_name(key));
    let from = match how {
        How::Login => crate::tr!("Captae 里登录的{site}账号", site = name),
        How::Verify => crate::tr!("在 Captae 里验证过的{site}", site = name),
    };
    Auth { cookies: cookies.into_iter().filter(Cookie::writable).collect(), user_agent: Some(ua), impersonate, browser: None, from, dir }
}

// ------------------------------------------------------------------ 准备好的登录状态

/// 一个任务这次要带的登录状态。`Debug` 不打印 cookie 的值。
#[derive(Clone, Default)]
pub struct Auth {
    pub cookies: Vec<Cookie>,
    pub user_agent: Option<String>,
    /// yt-dlp `--impersonate`（safari:macos / chrome:macos …），这个 yt-dlp 不支持时是 None
    pub impersonate: Option<String>,
    /// yt-dlp `--cookies-from-browser`
    pub browser: Option<String>,
    /// 给人看的来源：「Captae 里登录的哔哩哔哩」「浏览器插件带来的」「Chrome 里的」
    pub from: String,
    /// 临时 cookies 文件放哪（`<data>/work/auth`）
    pub dir: PathBuf,
}

impl fmt::Debug for Auth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Auth").field("cookies", &self.cookies.len()).field("from", &self.from).field("impersonate", &self.impersonate).field("browser", &self.browser).finish()
    }
}

impl Auth {
    pub fn is_empty(&self) -> bool {
        self.cookies.is_empty() && self.browser.is_none()
    }

    /// yt-dlp 的参数。带 cookies 时写一个临时文件，返回的 `CookieFile` 要活到 yt-dlp 退出（丢掉就删文件）
    pub fn ytdlp(&self) -> Result<(Vec<String>, Option<CookieFile>)> {
        let mut a: Vec<String> = vec![];
        let mut file = None;
        if !self.cookies.is_empty() {
            let f = CookieFile::write(&self.dir, &self.cookies)?;
            a.extend(["--cookies".into(), f.path().display().to_string()]);
            file = Some(f);
        } else if let Some(b) = &self.browser {
            a.extend(["--cookies-from-browser".into(), b.clone()]);
        }
        if self.is_empty() {
            return Ok((a, file));
        }
        // 模拟 Chromium 一家（chrome / edge）时不另外改 User-Agent，用模拟自带的那个。
        // 2026-11-24 插件子任务实测 B 站：只加浏览器的 User-Agent、只加模拟 Chrome，都能读出 15 个格式；
        // 两个一起加，一个格式都没有（User-Agent 写成和模拟的同一个版本也一样，原因没查清）。
        // 代价：和 User-Agent 绑在一起的 Cloudflare 验证（cf_clearance）可能对不上，遇到了照常报「要验证」，在 Captae 的窗口里验证一次。
        // Safari（登录窗口那条路）照样带同一个 User-Agent，实测没问题；这个 yt-dlp 不能模拟时也带。
        let chromium = self.impersonate.as_deref().is_some_and(|t| t.starts_with("chrome") || t.starts_with("edge"));
        if let Some(ua) = self.user_agent.as_ref().filter(|_| !chromium) {
            a.extend(["--user-agent".into(), ua.clone()]);
        }
        if let Some(t) = &self.impersonate {
            a.extend(["--impersonate".into(), t.clone()]);
        }
        Ok((a, file))
    }

    /// aria2c 的参数（直链）：`--load-cookies`、`--user-agent`
    pub fn aria2(&self) -> Result<(Vec<String>, Option<CookieFile>)> {
        if self.cookies.is_empty() {
            return Ok((vec![], None));
        }
        let f = CookieFile::write(&self.dir, &self.cookies)?;
        let mut a = vec![format!("--load-cookies={}", f.path().display())];
        if let Some(ua) = &self.user_agent {
            a.push(format!("--user-agent={ua}"));
        }
        Ok((a, Some(f)))
    }

    /// 自己发 HTTP 请求的地方（M3U8 引擎、直链读取信息）用的 cookie 罐子
    pub fn jar(&self) -> Jar {
        Jar(Arc::new(JarInner { cookies: self.cookies.clone(), user_agent: self.user_agent.clone().filter(|_| !self.cookies.is_empty()) }))
    }

    /// 把文字里出现的 cookie 值换成 `***`（子进程的报错在分类、写进记录之前先过一遍）。
    /// 太短的值（少于 6 个字符）不换，免得把正常的字抹掉；那种值也不是登录凭据。
    pub fn redact(&self, text: &str) -> String {
        redact(text, &self.cookies)
    }
}

pub fn redact(text: &str, cookies: &[Cookie]) -> String {
    let mut vals: Vec<&str> = cookies.iter().map(|c| c.value.as_str()).filter(|v| v.chars().count() >= 6).collect();
    // 长的先换：一个值包含另一个值时不会只换掉一半
    vals.sort_by_key(|v| std::cmp::Reverse(v.len()));
    vals.dedup();
    let mut out = text.to_string();
    for v in vals {
        if out.contains(v) {
            out = out.replace(v, "***");
        }
        // 网址里的值可能被百分号编码过
        let enc: String = url::form_urlencoded::byte_serialize(v.as_bytes()).collect();
        if enc != v && out.contains(&enc) {
            out = out.replace(&enc, "***");
        }
    }
    out
}

#[derive(Default)]
struct JarInner {
    cookies: Vec<Cookie>,
    user_agent: Option<String>,
}

/// 自己发请求时用的 cookie 罐子：每个请求按地址挑该带的 cookie（不会把一个网站的 cookie 发给别的 CDN）。
#[derive(Clone, Default)]
pub struct Jar(Arc<JarInner>);

impl fmt::Debug for Jar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Jar({} cookies)", self.0.cookies.len())
    }
}

impl Jar {
    pub fn is_empty(&self) -> bool {
        self.0.cookies.is_empty()
    }
    /// 这个地址该带的 `Cookie` 请求头
    pub fn header_for(&self, url: &str) -> Option<String> {
        let u = url::Url::parse(url).ok()?;
        let now = crate::util::now_ms() / 1000;
        let mut v: Vec<&Cookie> = self.0.cookies.iter().filter(|c| c.applies_to(&u, now)).collect();
        // 路径长的在前（和浏览器一样）
        v.sort_by_key(|c| std::cmp::Reverse(c.path.len()));
        let s: Vec<String> = v.iter().map(|c| format!("{}={}", c.name, c.value)).collect();
        (!s.is_empty()).then(|| s.join("; "))
    }
    /// 给一个请求加上 cookie（和浏览器的 User-Agent，请求头里没指定时）
    pub fn apply(&self, mut r: reqwest::RequestBuilder, url: &str, headers: &BTreeMap<String, String>) -> reqwest::RequestBuilder {
        if let Some(c) = self.header_for(url) {
            r = r.header("Cookie", c);
        }
        if let Some(ua) = &self.0.user_agent {
            if !headers.keys().any(|k| k.eq_ignore_ascii_case("user-agent")) {
                r = r.header("User-Agent", ua);
            }
        }
        r
    }
}

/// 插件放在 `headers` 里的 `Cookie` 请求头搬进 cookies（只给这个网址的主机），请求头里不再留：
/// 请求头会原样拼进 yt-dlp / aria2 的命令行参数，别的程序用 `ps` 就能看到。
pub fn take_cookie_header(headers: &mut BTreeMap<String, String>, url: &str) -> Vec<Cookie> {
    let keys: Vec<String> = headers.keys().filter(|k| k.eq_ignore_ascii_case("cookie")).cloned().collect();
    let mut out = vec![];
    for k in keys {
        if let Some(v) = headers.remove(&k) {
            out.extend(from_header(&v, url));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ck(name: &str, value: &str, domain: &str) -> Cookie {
        Cookie { name: name.into(), value: value.into(), domain: domain.into(), path: "/".into(), secure: true, http_only: true, host_only: false, expires: Some(4_102_444_800) }
    }

    #[test]
    fn sites_and_keys() {
        assert_eq!(site_key("https://www.youtube.com/watch?v=x").as_deref(), Some("youtube"));
        assert_eq!(site_key("https://youtu.be/abc").as_deref(), Some("youtube"));
        assert_eq!(site_key("https://music.youtube.com/watch?v=x").as_deref(), Some("youtube"));
        assert_eq!(site_key("https://b23.tv/x").as_deref(), Some("bilibili"));
        assert_eq!(site_key("https://twitter.com/a/status/1").as_deref(), Some("x"));
        assert_eq!(site_key("https://www.example.com/v/1").as_deref(), Some("host:example.com"));
        assert_eq!(site_key("ftp://example.com/a"), None);
        // 「notyoutube.com」不是 YouTube
        assert_eq!(site_key("https://notyoutube.com/x").as_deref(), Some("host:notyoutube.com"));
        assert_eq!(key_domains("x"), vec!["x.com", "twitter.com"]);
        assert_eq!(key_domains("host:example.com"), vec!["example.com"]);
        assert_eq!(key_name("bilibili"), "哔哩哔哩");
        assert_eq!(key_name("host:example.com"), "example.com");
    }

    /// 各网站的登录判断规则（计划 3.2）
    #[test]
    fn login_rules() {
        let now = 1_800_000_000;
        let yes = |key: &str, c: Cookie| logged_in(key, &[c], now);
        assert!(yes("youtube", ck("SAPISID", "abc", ".youtube.com")));
        assert!(yes("youtube", ck("__Secure-3PAPISID", "abc", ".youtube.com")));
        // Google 账号本身的 cookie（.google.com）不算 YouTube 登录了
        assert!(!yes("youtube", ck("SAPISID", "abc", ".google.com")));
        assert!(!yes("youtube", ck("VISITOR_INFO1_LIVE", "abc", ".youtube.com")));
        assert!(yes("bilibili", ck("SESSDATA", "abc", ".bilibili.com")));
        assert!(!yes("bilibili", ck("buvid3", "abc", ".bilibili.com")));
        assert!(yes("x", ck("auth_token", "abc", ".x.com")));
        assert!(yes("x", ck("auth_token", "abc", ".twitter.com")));
        assert!(!yes("x", ck("guest_id", "abc", ".x.com")));
        assert!(yes("instagram", ck("sessionid", "abc", ".instagram.com")));
        assert!(!yes("instagram", ck("csrftoken", "abc", ".instagram.com")));
        assert!(yes("douyin", ck("sid_tt", "abc", ".douyin.com")));
        assert!(!yes("douyin", ck("s_v_web_id", "abc", ".douyin.com")));
        // 小红书：大陆打开留在 xiaohongshu.com，大陆以外跳到 rednote.com，两边的 web_session 都算
        assert!(yes("xiaohongshu", ck("web_session", "abc", ".xiaohongshu.com")));
        assert!(yes("xiaohongshu", ck("web_session", "abc", ".rednote.com")));
        assert!(!yes("xiaohongshu", ck("a1", "abc", ".rednote.com")));
        assert_eq!(site_key("https://www.rednote.com/explore/6b0336bd000000001b01ff7f").as_deref(), Some("xiaohongshu"));
        // 过期、空值的不算
        let mut old = ck("SESSDATA", "abc", ".bilibili.com");
        old.expires = Some(now - 1);
        assert!(!yes("bilibili", old));
        assert!(!yes("bilibili", ck("SESSDATA", "", ".bilibili.com")));
        // 会话 cookie（没有过期时间）算
        let mut sess = ck("SESSDATA", "abc", ".bilibili.com");
        sess.expires = None;
        assert!(yes("bilibili", sess));
        // 没有规则的网站：没法判断，永远是 false（人机验证等用户点「完成」）
        assert!(!yes("host:example.com", ck("cf_clearance", "abc", ".example.com")));
    }

    /// 什么时候带 cookies（计划 3.2 / 3.4），尤其 YouTube 的例外
    #[test]
    fn when_to_send() {
        let mut l = Logins::default();
        let yt = "https://www.youtube.com/watch?v=mC73amqYFpQ";
        let bili = "https://www.bilibili.com/video/BV121yjzQEWf";
        // 什么都没有：不带
        assert_eq!(plan(bili, None, false, false, &l, ""), Plan::None);
        // B 站在 Captae 里登录过：一开始就带
        l.mark("bilibili", How::Login, Some(1));
        assert_eq!(plan(bili, None, false, false, &l, ""), Plan::Store("bilibili".into()));
        // YouTube 登录过也不带，除非这个任务报过 need_login / need_verify
        l.mark("youtube", How::Login, Some(1));
        assert_eq!(plan(yt, None, false, false, &l, ""), Plan::None);
        assert_eq!(plan(yt, None, false, true, &l, ""), Plan::Store("youtube".into()));
        // 插件带来的 cookies 优先，YouTube 照样只在需要时
        assert_eq!(plan(bili, None, true, false, &l, ""), Plan::Extension);
        assert_eq!(plan(yt, None, true, false, &l, ""), Plan::None);
        assert_eq!(plan(yt, None, true, true, &l, ""), Plan::Extension);
        // 插件嗅探到的 M3U8：媒体地址在 CDN 上，按来源页面算网站
        let cdn = "https://upos-sz-mirror.bilivideo.com/a.m3u8";
        assert_eq!(plan(cdn, Some(bili), false, false, &l, ""), Plan::Store("bilibili".into()));
        assert_eq!(plan(cdn, Some(yt), false, false, &l, ""), Plan::None);
        // 设置里打开了读浏览器：没在 Captae 里登录的网站用浏览器的；YouTube 照样只在需要时
        let ig = "https://www.instagram.com/p/C9xYzAbCdEf/";
        assert_eq!(plan(ig, None, false, false, &l, "chrome"), Plan::Browser("chrome"));
        assert_eq!(plan(bili, None, false, false, &l, "chrome"), Plan::Store("bilibili".into()));
        l.forget("youtube");
        assert_eq!(plan(yt, None, false, false, &l, "firefox"), Plan::None);
        assert_eq!(plan(yt, None, false, true, &l, "firefox"), Plan::Browser("firefox"));
        // Safari 只在 macOS 上有：别的系统上等于没开（从 Mac 上拷过来的设置）
        assert_eq!(plan(yt, None, false, true, &l, "safari") == Plan::Browser("safari"), cfg!(target_os = "macos"));
        // 不认识的浏览器（比如 arc）等于没开
        assert_eq!(plan(ig, None, false, false, &l, "arc"), Plan::None);
        assert!(!available(yt, None, false, &l, ""));
        assert!(available(yt, None, false, &l, "chrome"));
        // 验证过的普通网站：子域名也算
        l.mark("host:example.com", How::Verify, Some(2));
        assert_eq!(plan("https://video.example.com/v/1", None, false, false, &l, ""), Plan::Store("host:example.com".into()));
        assert_eq!(plan("https://example.org/v/1", None, false, false, &l, ""), Plan::None);
    }

    #[test]
    fn records() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("logins.json");
        let mut l = Logins::load(&p);
        assert!(l.sites.is_empty());
        l.mark("douyin", How::Verify, Some(10));
        // 先验证、后登录：算登录；登录过的再验证，还是登录
        l.mark("douyin", How::Login, Some(20));
        l.mark("douyin", How::Verify, Some(30));
        assert_eq!(l.get("douyin").map(|r| (r.how, r.at)), Some((How::Login, Some(30))));
        assert!(l.tell_once("youtube"));
        assert!(!l.tell_once("youtube"));
        l.save(&p).unwrap();
        let back = Logins::load(&p);
        assert_eq!(back, l);
        // 文件里只有网站和时间，没有 cookie
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(text.contains("douyin") && !text.contains("value"));
        let mut back = back;
        assert!(back.forget("douyin") && !back.forget("douyin"));
    }

    /// Netscape 格式：yt-dlp、aria2 都要能读
    #[test]
    fn netscape_format() {
        let mut host_only = ck("ct0", "v2", "x.com");
        host_only.secure = false;
        host_only.expires = None;
        let mut bad = ck("evil", "a\tb", ".x.com");
        bad.http_only = false;
        let mut gone = ck("old", "zzz", ".x.com");
        gone.expires = Some(100);
        let text = netscape(&[ck("auth_token", "secret-1", ".x.com"), host_only, bad, gone], 1_000);
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "# Netscape HTTP Cookie File");
        let rows: Vec<Vec<&str>> = lines.iter().filter(|l| !l.starts_with('#') && !l.is_empty()).map(|l| l.split('\t').collect()).collect();
        assert_eq!(rows.len(), 2, "值里有制表符的、过期的都跳过：{text}");
        assert_eq!(rows[0], vec![".x.com", "TRUE", "/", "TRUE", "4102444800", "auth_token", "secret-1"]);
        // 只给一个主机：第一列没有点、第二列 FALSE；会话 cookie 的过期时间写 0
        assert_eq!(rows[1], vec!["x.com", "FALSE", "/", "FALSE", "0", "ct0", "v2"]);
        assert!(!text.contains("#HttpOnly_"));
        // 插件说 hostOnly 的，就算域名前面有点也只给这一个主机
        let mut ho = ck("a", "b", ".m.x.com");
        ho.host_only = true;
        assert!(netscape(&[ho], 1_000).contains("\nm.x.com\tFALSE\t"));
    }

    /// 临时文件：权限 600、目录 700、用完（丢掉）就删
    #[cfg(unix)]
    #[test]
    fn temp_file_lifecycle() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let adir = dir.path().join("work/auth");
        let f = CookieFile::write(&adir, &[ck("SESSDATA", "s3cr3t-value", ".bilibili.com")]).unwrap();
        let p = f.path().to_path_buf();
        assert_eq!(std::fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(&adir).unwrap().permissions().mode() & 0o777, 0o700);
        assert!(std::fs::read_to_string(&p).unwrap().contains("s3cr3t-value"));
        drop(f);
        assert!(!p.exists(), "丢掉就删");
        // 上次异常退出留下的：启动时清掉
        let paths = Paths::portable(dir.path());
        std::fs::create_dir_all(auth_dir(&paths)).unwrap();
        let left = auth_dir(&paths).join("c-left.txt");
        std::fs::write(&left, "x").unwrap();
        sweep(&paths, std::time::Duration::from_secs(600));
        assert!(left.exists(), "刚写的（可能正在用）不删");
        sweep(&paths, std::time::Duration::ZERO);
        assert!(!left.exists());
    }

    #[test]
    fn ytdlp_args() {
        let dir = tempfile::tempdir().unwrap();
        // 登录窗口的存储：cookies 文件 + Safari 的 User-Agent + 模拟 Safari
        let a = Auth { cookies: vec![ck("SESSDATA", "v", ".bilibili.com")], user_agent: Some(safari_ua_for("27.2")), impersonate: Some("safari:macos".into()), browser: None, from: "x".into(), dir: dir.path().to_path_buf() };
        let (args, file) = a.ytdlp().unwrap();
        let file = file.unwrap();
        assert_eq!(args[0], "--cookies");
        assert_eq!(args[1], file.path().display().to_string());
        assert!(args.windows(2).any(|w| w[0] == "--user-agent" && w[1].contains("Version/27.2 Safari/605.1.15")));
        assert!(args.windows(2).any(|w| w[0] == "--impersonate" && w[1] == "safari:macos"));
        // 读浏览器：--cookies-from-browser，不写文件，也不改 User-Agent
        let b = Auth { browser: Some("chrome".into()), dir: dir.path().to_path_buf(), ..Default::default() };
        let (args, file) = b.ytdlp().unwrap();
        assert_eq!(args, vec!["--cookies-from-browser", "chrome"]);
        assert!(file.is_none());
        // 插件带来的（Chrome 一家）：模拟 chrome，不改 User-Agent（两个一起加 B 站会回空的格式列表）；aria2 照样用浏览器的 User-Agent
        let chrome_ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
        let c = Auth { cookies: vec![ck("SESSDATA", "v", ".bilibili.com")], user_agent: Some(chrome_ua.into()), impersonate: Some("chrome:macos".into()), browser: None, from: "x".into(), dir: dir.path().to_path_buf() };
        let (args, cf) = c.ytdlp().unwrap();
        assert!(args.windows(2).any(|w| w[0] == "--impersonate" && w[1] == "chrome:macos"));
        assert!(!args.iter().any(|x| x == "--user-agent"), "{args:?}");
        drop(cf);
        let (args, cf) = c.aria2().unwrap();
        assert!(args.iter().any(|x| x == &format!("--user-agent={chrome_ua}")));
        drop(cf);
        // 这个 yt-dlp 不能模拟（没带 curl_cffi）：照样带浏览器的 User-Agent
        let d = Auth { impersonate: None, ..c.clone() };
        let (args, cf) = d.ytdlp().unwrap();
        assert!(args.windows(2).any(|w| w[0] == "--user-agent" && w[1] == chrome_ua));
        drop(cf);
        // 什么都没有：一个参数都不加
        let (args, file) = Auth::default().ytdlp().unwrap();
        assert!(args.is_empty() && file.is_none());
        // aria2
        let (args, file) = a.aria2().unwrap();
        assert_eq!(args[0], format!("--load-cookies={}", file.as_ref().unwrap().path().display()));
        assert!(args[1].starts_with("--user-agent=Mozilla/5.0 (Macintosh"));
        drop(file);
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1, "用完的文件都删了，只剩上面那个还在用的");
    }

    #[test]
    fn impersonate_targets_parse() {
        let out = "[info] Available impersonate targets\nClient          OS           Source\n--------------------------------------\nChrome-133      Macos-15     curl_cffi\nSafari-27.0     Macos-27     curl_cffi\nSafari-26.0     Ios-26.0     curl_cffi\nEdge-101        Windows-10   curl_cffi\nFirefox-135     Macos-14     curl_cffi (unavailable)\n";
        let t = parse_targets(out);
        assert!(t.contains(&("safari".into(), "macos".into())) && t.contains(&("edge".into(), "windows".into())));
        assert!(!t.iter().any(|(c, _)| c == "firefox" || c == "client"));
        assert_eq!(pick_impersonate(&t, "safari", Some("macos")).as_deref(), Some("safari:macos"));
        // 按 User-Agent 写的系统挑：Edge 有 Windows 的目标，Chrome 没有就不限系统
        assert_eq!(pick_impersonate(&t, "edge", Some("windows")).as_deref(), Some("edge:windows"));
        assert_eq!(pick_impersonate(&t, "edge", None).as_deref(), Some("edge"));
        assert_eq!(pick_impersonate(&t, "chrome", Some("windows")).as_deref(), Some("chrome"));
        assert_eq!(pick_impersonate(&t, "chrome", Some("macos")).as_deref(), Some("chrome:macos"));
        assert_eq!(pick_impersonate(&t, "firefox", Some("macos")), None);
        assert_eq!(pick_impersonate(&[], "safari", Some("macos")), None);
        assert_eq!(ua_family(&safari_ua_for("27.2")), Some("safari"));
        // 系统
        assert_eq!(ua_os(&safari_ua_for("27.2")), Some("macos"));
        assert_eq!(ua_os(&edge_ua_for("142")), Some("windows"));
        assert_eq!(ua_family(&edge_ua_for("142")), Some("chrome"));
        assert_eq!(ua_os("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36"), Some("linux"));
        assert_eq!(ua_os("Mozilla/5.0 (Linux; Android 14) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Mobile Safari/537.36"), Some("android"));
        assert_eq!(ua_os("Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1"), Some("ios"));
        assert_eq!(ua_os("curl/8"), None);
        assert_eq!(ua_family("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36 Edg/142.0.0.0"), Some("chrome"));
        assert_eq!(ua_family("Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:145.0) Gecko/20100101 Firefox/145.0"), Some("firefox"));
    }

    /// 登录窗口的 User-Agent：Windows 是 WebView2（Chromium）的，macOS、Linux 是 Safari 的；桌面端读到实际的以后用实际的
    #[test]
    fn login_window_user_agent() {
        let ua = login_user_agent();
        if cfg!(windows) {
            assert_eq!((ua_family(&ua), ua_os(&ua)), (Some("chrome"), Some("windows")));
        } else {
            assert_eq!((ua_family(&ua), ua_os(&ua)), (Some("safari"), Some("macos")));
        }
        set_login_user_agent("  ".into());
        assert_eq!(login_user_agent(), ua, "空的不算");
    }

    /// 每个系统能选哪些浏览器；读浏览器失败的报错认得出来
    #[test]
    fn browsers_per_system() {
        let ids: Vec<&str> = browsers().iter().map(|(id, _)| *id).collect();
        assert_eq!(ids.contains(&"safari"), cfg!(target_os = "macos"));
        assert!(ids.contains(&"firefox") && ids.contains(&"chrome") && ids.contains(&"edge"));
        if cfg!(windows) {
            assert_eq!(ids[0], "firefox");
            assert!(!browser_readable("chrome") && browser_readable("firefox"));
        } else {
            assert!(browser_readable("chrome") && browser_readable("firefox"));
        }
        assert_eq!(browser_arg(" Firefox "), Some("firefox"));
        let dpapi = "ERROR: Failed to decrypt with DPAPI. See  https://github.com/yt-dlp/yt-dlp/issues/10927  for more info";
        assert!(browser_failure_hint(dpapi, "chrome").unwrap().contains("Chrome"));
        let locked = "ERROR: Could not copy Chrome cookie database. See  https://github.com/yt-dlp/yt-dlp/issues/7271  for more info";
        assert!(browser_failure_hint(locked, "edge").unwrap().contains("Edge"));
        assert!(browser_failure_hint("WARNING: failed to read from keyring", "chrome").is_some());
        assert!(browser_failure_hint("ERROR: could not find firefox cookies database in \"/home/a/.mozilla/firefox\"", "firefox").is_some());
        assert_eq!(browser_failure_hint("ERROR: [youtube] x: Sign in to confirm your age", "chrome"), None);
    }

    /// 日志、错误原文里没有 cookie 的值
    #[test]
    fn redaction() {
        let a = Auth { cookies: vec![ck("SESSDATA", "a1b2c3d4%2Ce5", ".bilibili.com"), ck("x", "abc", ".bilibili.com")], ..Default::default() };
        let err = "ERROR: [BiliBili] 1: HTTP Error 412 for https://api.bilibili.com/x?SESSDATA=a1b2c3d4%2Ce5 cookie a1b2c3d4%252Ce5";
        let r = a.redact(err);
        assert!(!r.contains("a1b2c3d4%2Ce5") && !r.contains("a1b2c3d4%252Ce5"), "{r}");
        assert!(r.contains("SESSDATA=***"));
        // 太短的值不换（不是凭据，换了会把正常的字抹掉）
        assert!(a.redact("abc def").contains("abc"));
        // Debug 不打印值
        let d = format!("{:?} {:?}", a.cookies[0], a);
        assert!(!d.contains("a1b2c3d4"), "{d}");
    }

    #[test]
    fn jar_picks_by_url() {
        let mut path = ck("p", "on-path", ".cdn.example.com");
        path.path = "/videos".into();
        let mut plain = ck("plain", "only-https-no", "media.example.com");
        plain.secure = true;
        let jar = Auth { cookies: vec![ck("auth", "tok", ".example.com"), path, plain, ck("other", "nope", ".other.net")], ..Default::default() }.jar();
        assert_eq!(jar.header_for("https://cdn.example.com/videos/1.ts").as_deref(), Some("p=on-path; auth=tok"));
        assert_eq!(jar.header_for("https://cdn.example.com/img/1.png").as_deref(), Some("auth=tok"));
        // 只给一个主机的：子域名拿不到；secure 的：http 拿不到
        assert_eq!(jar.header_for("https://a.media.example.com/x").as_deref(), Some("auth=tok"));
        assert_eq!(jar.header_for("http://media.example.com/x"), None, "secure 的 cookie 不走 http");
        assert_eq!(jar.header_for("https://media.example.com/x").as_deref(), Some("auth=tok; plain=only-https-no"));
        // 别的网站（CDN）拿不到这个网站的 cookie
        assert_eq!(jar.header_for("https://evil.net/x"), None);
        assert!(Jar::default().header_for("https://example.com/").is_none());
    }

    #[test]
    fn cookie_header_moves_out_of_headers() {
        let mut h = BTreeMap::from([("Referer".to_string(), "https://a.com/".to_string()), ("cookie".to_string(), "sid=12345678; theme=dark".to_string())]);
        let c = take_cookie_header(&mut h, "https://media.a.com/v.m3u8");
        assert_eq!(h.len(), 1);
        assert_eq!(c.len(), 2);
        assert_eq!((c[0].name.as_str(), c[0].value.as_str(), c[0].domain.as_str(), c[0].is_host_only()), ("sid", "12345678", "media.a.com", true));
    }

    /// 插件直接传 chrome.cookies.getAll 的结果
    #[test]
    fn chrome_cookie_shape() {
        let v = serde_json::json!({"name": "SESSDATA", "value": "v", "domain": ".bilibili.com", "path": "/", "secure": true, "httpOnly": true, "hostOnly": false,
            "session": false, "expirationDate": 1795450609.123, "sameSite": "no_restriction", "storeId": "0"});
        let c: Cookie = serde_json::from_value(v).unwrap();
        assert_eq!((c.expires, c.http_only, c.is_host_only()), (Some(1795450609), true, false));
        let s: Cookie = serde_json::from_value(serde_json::json!({"name": "a", "value": "b", "domain": "x.com", "session": true})).unwrap();
        assert_eq!((s.expires, s.path.as_str(), s.is_host_only()), (None, "/", true));
        // 转发（宿主 → 桌面端）时原样带过去
        let back: Cookie = serde_json::from_value(serde_json::to_value(&c).unwrap()).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn store_ids() {
        assert_eq!(store_uuid(None), STORE_UUID);
        let a = store_uuid(Some(Path::new("/tmp/a")));
        assert_ne!(a, STORE_UUID);
        assert_eq!(a, store_uuid(Some(Path::new("/tmp/a"))), "同一个测试目录每次一样");
        assert_ne!(a, store_uuid(Some(Path::new("/tmp/b"))));
        assert_eq!((a[6] >> 4, a[8] >> 6), (4, 2));
        assert_eq!((STORE_UUID[6] >> 4, STORE_UUID[8] >> 6), (4, 2));
        assert_eq!(site_by_id("youtube").unwrap().logout_also, &["google.com"]);
    }

    #[test]
    fn related_domains() {
        assert!(ck("a", "b", ".youtube.com").related_to("youtube.com"));
        assert!(ck("a", "b", "www.youtube.com").related_to("youtube.com"));
        assert!(!ck("a", "b", ".google.com").related_to("youtube.com"));
        // 上级域名上的 cookie（cf_clearance 常种在 .example.com）也算 video.example.com 的
        assert!(ck("cf_clearance", "b", ".example.com").related_to("video.example.com"));
        assert!(!ck("a", "b", "example.com").related_to("video.example.com"));
        assert!(host_under("a.b.example.com", "example.com") && !host_under("badexample.com", "example.com"));
    }
}
