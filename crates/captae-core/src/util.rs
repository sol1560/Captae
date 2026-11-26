use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// 秒数 → "9:56" / "1:02:03"，和设计稿缩略图角标一致。
pub fn clock(secs: f64) -> String {
    if !secs.is_finite() || secs <= 0.0 {
        return String::new();
    }
    let s = secs.round() as u64;
    let (h, m, s) = (s / 3600, (s % 3600) / 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// 毫秒时间戳 → "今天 10:12" / "昨天 22:15" / "3 天前"（跟着界面语言）。
pub fn when(ts_ms: i64) -> String {
    use crate::tr;
    let now = now_ms();
    let off = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    let to_local = |ms: i64| {
        time::OffsetDateTime::from_unix_timestamp(ms / 1000)
            .unwrap_or(time::OffsetDateTime::UNIX_EPOCH)
            .to_offset(off)
    };
    let (t, n) = (to_local(ts_ms), to_local(now));
    let days = (n.date() - t.date()).whole_days();
    let hm = format!("{:02}:{:02}", t.hour(), t.minute());
    match days {
        d if d <= 0 => {
            if now - ts_ms < 60_000 {
                tr!("刚刚")
            } else {
                tr!("今天 {time}", time = hm)
            }
        }
        1 => tr!("昨天 {time}", time = hm),
        d => tr!("{n} 天前", n = d),
    }
}

pub fn mb(bytes: f64) -> f64 {
    bytes / 1_000_000.0
}

/// 用户主目录（unix 的 HOME；Windows 先看 USERPROFILE：Git Bash 之类会设一个 HOME），都没有时是空路径
pub fn home_dir() -> std::path::PathBuf {
    let (a, b) = if cfg!(windows) { ("USERPROFILE", "HOME") } else { ("HOME", "USERPROFILE") };
    std::path::PathBuf::from(std::env::var_os(a).filter(|v| !v.is_empty()).or_else(|| std::env::var_os(b)).unwrap_or_default())
}

/// 文件名里不能出现的字符替换掉，长度截到 120。三个平台都按 Windows 的规矩来（文件可能被拷到别的电脑、同步盘上）：
/// 不许 `<>:"/\|?*` 和控制字符，开头结尾不留点和空格（Windows 会悄悄去掉结尾的点和空格，文件名就对不上了），
/// 不用 CON、PRN、AUX、NUL、COM1～9、LPT1～9 这些保留名（带扩展名也不行，比如 `con.mp4`），前面加 `_`
pub fn safe_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let edge = |c: char| c == '.' || c.is_whitespace();
    let t: String = cleaned.trim_matches(edge).chars().take(120).collect();
    // 截断以后结尾可能又是点或空格
    let t = t.trim_matches(edge).to_string();
    if t.is_empty() {
        return "captae".into();
    }
    if reserved_name(&t) {
        return format!("_{t}");
    }
    t
}

/// Windows 的保留设备名（不分大小写；`CON.mp4`、`com1.tar.gz` 也算：看第一个点前面那段）
pub fn reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or("").trim_end().to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
        || (stem.chars().count() == 4 && (stem.starts_with("COM") || stem.starts_with("LPT")) && stem[3..].chars().all(|c| matches!(c, '1'..='9' | '¹' | '²' | '³')))
}

/// 同名文件已存在时加 " (2)"、" (3)"。
pub fn unique_path(p: std::path::PathBuf) -> std::path::PathBuf {
    if !p.exists() {
        return p;
    }
    let stem = p.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = p.extension().map(|s| format!(".{}", s.to_string_lossy())).unwrap_or_default();
    let dir = p.parent().map(|d| d.to_path_buf()).unwrap_or_default();
    for n in 2.. {
        let c = dir.join(format!("{stem} ({n}){ext}"));
        if !c.exists() {
            return c;
        }
    }
    unreachable!()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn clock_formats() {
        assert_eq!(clock(596.0), "9:56");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(clock(0.0), "");
    }
    #[test]
    fn names() {
        assert_eq!(safe_name("a/b:c?"), "a_b_c_");
        assert_eq!(safe_name("  "), "captae");
        // Windows 不许的：结尾的点和空格、保留名
        assert_eq!(safe_name("abc . "), "abc");
        assert_eq!(safe_name("..hidden"), "hidden");
        assert_eq!(safe_name("CON"), "_CON");
        assert_eq!(safe_name("con.mp4"), "_con.mp4");
        assert_eq!(safe_name("Lpt3"), "_Lpt3");
        assert_eq!(safe_name("COM10"), "COM10");
        assert_eq!(safe_name("Console"), "Console");
        // 截到 120 个字以后结尾又是空格
        let long = format!("{} x", "a".repeat(119));
        assert_eq!(safe_name(&long), "a".repeat(119));
    }
    /// 相对时间跟着界面语言（docs/I18N.md 2.5）
    #[test]
    fn when_follows_language() {
        let now = now_ms();
        assert_eq!((when(now), when(now - 3 * 86_400_000)), ("刚刚".to_string(), "3 天前".to_string()));
        crate::i18n::with_lang(crate::i18n::Lang::En, || {
            assert_eq!((when(now), when(now - 3 * 86_400_000)), ("Just now".to_string(), "3 days ago".to_string()));
        });
    }
}

/// 按链接算一个稳定的短名字，用来命名临时目录。
/// 任务编号每次启动都从 1 开始，用编号命名会把别的任务留下的分片当成自己的。
pub fn url_key(s: &str) -> String {
    // FNV-1a 64
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}
