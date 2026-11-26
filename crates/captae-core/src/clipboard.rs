//! 剪贴板提示：复制了一个视频链接、而且 1 分钟内把 Captae 拿到前面（或者打开 Captae），才在界面里问一句要不要下载。
//! 复制的那一刻什么都不弹（原来复制了就提示，用户觉得打扰：复制链接多半是要发给别人）。
//!
//! - Captae 开着时：桌面端每半秒看一眼系统剪贴板的变更次数（只看次数，不读内容），记下最近一次变化的时间、
//!   那时 Captae 在不在前台（`AskGate`）；Captae 被拿到前面时才读一次剪贴板，是视频链接才问。
//!   在 Captae 自己里面复制的不问，一次复制最多问一次。
//! - Captae 没开时没人看剪贴板：浏览器插件在网页里看到复制了视频链接，经宿主写一个 `copied-link.json`
//!   （什么链接、什么时候，`write_hint`），打开 Captae 时读走它（`take_hint`），剪贴板里还是这个链接、又在 1 分钟内，就问。
//!   别的程序里复制、Captae 又没开的，不知道是什么时候复制的，不问。
//!
//! 只认一整段就是链接的文本，而且得是平台视频页、媒体直链或 m3u8，普通网页链接不打扰。

use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::detect::{detect, Kind};

/// 复制以后多久内把 Captae 拿到前面才问
pub const ASK_WITHIN: Duration = Duration::from_secs(60);

/// 插件记下的复制（Captae 没开时），放在数据目录里
const HINT_FILE: &str = "copied-link.json";

/// 调试版的测试开关：环境变量 `CAPTAE_TEST_NO_CLIPBOARD` 设了以后，Captae 不写系统剪贴板（「发给 AI」、复制按钮），
/// 写的地方改成记一行日志。实测时一律开着，免得覆盖用户剪贴板里的东西。正式版编译不进去，永远是 false
pub fn writes_disabled() -> bool {
    #[cfg(debug_assertions)]
    {
        std::env::var_os("CAPTAE_TEST_NO_CLIPBOARD").is_some()
    }
    #[cfg(not(debug_assertions))]
    {
        false
    }
}

pub fn is_media_link(text: &str) -> Option<String> {
    let t = text.trim();
    if t.contains(char::is_whitespace) || t.len() > 2048 {
        return None;
    }
    let d = detect(t)?;
    match d.kind {
        Kind::Hls | Kind::File => Some(t.to_string()),
        Kind::Page if d.plat.is_some() || d.host.contains("vimeo") => Some(t.to_string()),
        _ => None,
    }
}

/// Captae 被拿到前面时，要不要读剪贴板
#[derive(Debug, PartialEq, Eq)]
pub enum Ask {
    /// 不问
    No,
    /// 读剪贴板，是视频链接就问
    Any,
    /// 读剪贴板，还是这个链接才问（插件记下的那次复制：别的程序里后来又复制过，就不知道是什么时候复制的了）
    Only(String),
}

/// 剪贴板最近一次变化的记录，决定 Captae 被拿到前面时问不问（规则见文件开头）。时间都由调用方传进来，方便测试
#[derive(Debug, Default)]
pub struct AskGate {
    /// 上次看到的变更次数；None = 还没看过（刚启动时剪贴板里已经有的东西不算新复制的）
    seen: Option<isize>,
    last: Option<Change>,
}

#[derive(Debug)]
struct Change {
    at: Instant,
    /// 变化时 Captae 在前台：是在 Captae 里面复制的
    front: bool,
    /// 插件记下的链接（Captae 没开时复制的）
    url: Option<String>,
    /// 这次复制已经问过，或者判断过不问
    done: bool,
}

impl AskGate {
    pub const fn new() -> Self {
        Self { seen: None, last: None }
    }

    /// 看一眼剪贴板的变更次数（后台定时看；Captae 被拿到前面时也先看一眼）。front：Captae 这时在不在前台
    pub fn observe(&mut self, count: isize, now: Instant, front: bool) {
        if self.seen.is_some_and(|s| s != count) {
            self.last = Some(Change { at: now, front, url: None, done: false });
        }
        self.seen = Some(count);
    }

    /// 插件记下的复制：at 时复制了 url（那时 Captae 没开）
    pub fn hinted(&mut self, url: String, at: Instant) {
        self.last = Some(Change { at, front: false, url: Some(url), done: false });
    }

    /// Captae 被拿到前面了。count 是这一刻的变更次数：在后台两次查看之间复制、马上切过来的，算在切过来之前（不是在 Captae 里复制的）
    pub fn activate(&mut self, count: isize, now: Instant, within: Duration) -> Ask {
        self.observe(count, now, false);
        let Some(c) = self.last.as_mut() else { return Ask::No };
        if c.done {
            return Ask::No;
        }
        c.done = true;
        if c.front || now.saturating_duration_since(c.at) > within {
            return Ask::No;
        }
        match c.url.take() {
            Some(u) => Ask::Only(u),
            None => Ask::Any,
        }
    }
}

/// 插件：网页里复制了视频链接（宿主调用）。Captae 开没开都写，打开 Captae 时才读
pub fn write_hint(data_dir: &Path, url: &str) -> std::io::Result<()> {
    let at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let tmp = data_dir.join(format!("{HINT_FILE}.tmp"));
    std::fs::write(&tmp, serde_json::json!({ "url": url, "at": at }).to_string())?;
    std::fs::rename(tmp, data_dir.join(HINT_FILE))
}

/// 打开 Captae 时：读走插件记下的复制（读完就删），返回链接和过去了多久；超过 within 的、时间在将来的（改过钟）不要
pub fn take_hint(data_dir: &Path, within: Duration) -> Option<(String, Duration)> {
    let f = data_dir.join(HINT_FILE);
    let s = std::fs::read_to_string(&f).ok()?;
    let _ = std::fs::remove_file(&f);
    let v: serde_json::Value = serde_json::from_str(&s).ok()?;
    let url = v["url"].as_str()?.to_string();
    let at = UNIX_EPOCH + Duration::from_millis(v["at"].as_u64()?);
    let age = SystemTime::now().duration_since(at).ok()?;
    (age <= within).then_some((url, age))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links() {
        assert!(is_media_link("https://www.youtube.com/watch?v=1").is_some());
        assert!(is_media_link(" https://a.com/x.mp4 ").is_some());
        assert!(is_media_link("https://vimeo.com/1084537").is_some());
        assert!(is_media_link("https://news.example.com/article").is_none());
        assert!(is_media_link("看这个 https://youtube.com/watch?v=1").is_none());
    }

    #[test]
    fn ask_gate() {
        let t0 = Instant::now();
        let s = |n: u64| t0 + Duration::from_secs(n);
        let w = ASK_WITHIN;

        // 刚启动时剪贴板里已有的不算
        let mut g = AskGate::new();
        g.observe(5, s(0), false);
        assert_eq!(g.activate(5, s(1), w), Ask::No);
        // 在别的程序里复制，30 秒后切过来：问；再切过来一次：不再问
        g.observe(6, s(10), false);
        assert_eq!(g.activate(6, s(40), w), Ask::Any);
        assert_eq!(g.activate(6, s(45), w), Ask::No);
        // 复制以后过了 1 分钟才切过来：不问
        g.observe(7, s(100), false);
        assert_eq!(g.activate(7, s(161), w), Ask::No);
        // 正好 1 分钟：还问
        g.observe(8, s(200), false);
        assert_eq!(g.activate(8, s(260), w), Ask::Any);
        // 在 Captae 里面复制的（那时 Captae 在前台）：不问
        g.observe(9, s(300), true);
        assert_eq!(g.activate(9, s(310), w), Ask::No);
        // 复制完马上切过来，后台还没看到这次变化：算在切过来之前，问
        assert_eq!(g.activate(10, s(400), w), Ask::Any);
        // 从没变过（只是切来切去）：不问
        assert_eq!(g.activate(10, s(405), w), Ask::No);

        // 插件记下的（Captae 没开时复制的）：只认这个链接
        let mut g = AskGate::new();
        g.hinted("https://youtu.be/x".into(), s(0));
        g.observe(3, s(5), false);
        assert_eq!(g.activate(3, s(20), w), Ask::Only("https://youtu.be/x".into()));
        assert_eq!(g.activate(3, s(25), w), Ask::No);
        // 插件记下以后又复制了别的：以后来的为准
        let mut g = AskGate::new();
        g.hinted("https://youtu.be/x".into(), s(0));
        g.observe(3, s(1), false);
        g.observe(4, s(2), false);
        assert_eq!(g.activate(4, s(3), w), Ask::Any);
        // 插件记下的超过 1 分钟才打开：不问
        let mut g = AskGate::new();
        g.hinted("https://youtu.be/x".into(), s(0));
        assert_eq!(g.activate(1, s(61), w), Ask::No);
    }

    #[test]
    fn hint_file() {
        let d = tempfile::tempdir().unwrap();
        write_hint(d.path(), "https://youtu.be/x").unwrap();
        let (url, age) = take_hint(d.path(), ASK_WITHIN).unwrap();
        assert_eq!(url, "https://youtu.be/x");
        assert!(age < Duration::from_secs(5));
        // 读走就删了
        assert!(!d.path().join(HINT_FILE).exists());
        assert!(take_hint(d.path(), ASK_WITHIN).is_none());
        // 太旧的不要，也删掉
        let old = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64 - 120_000;
        std::fs::write(d.path().join(HINT_FILE), format!(r#"{{"url":"https://youtu.be/y","at":{old}}}"#)).unwrap();
        assert!(take_hint(d.path(), ASK_WITHIN).is_none());
        assert!(!d.path().join(HINT_FILE).exists());
        // 时间在将来（改过钟）的不要
        let later = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as u64 + 600_000;
        std::fs::write(d.path().join(HINT_FILE), format!(r#"{{"url":"https://youtu.be/z","at":{later}}}"#)).unwrap();
        assert!(take_hint(d.path(), ASK_WITHIN).is_none());
    }
}
