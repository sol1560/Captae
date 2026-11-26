//! 界面文字的多语言（i18n）。规矩写在 `docs/I18N.md`，这里只说用法。
//!
//! 写法和 gettext 一样：代码里直接写中文原文，中文（简体）就是原文，不需要词表。
//! - `tr!("没有这个任务")`、`tr!("这个视频要登录{site}才能看", site = name)`：占位符写成 `{名字}`，和 `format!` 一样。
//! - 带数量的句子把数量叫 `n`：`tr!("这里有 {n} 个视频", n = count)`。英文、德文、西班牙文、法文的词表里这条写成
//!   `{"one": "…", "other": "…"}`，按 `n` 选。
//! - 同一句中文在不同地方要译成不同的话时，在原文后面加「§说明」：`tr!("下载§按钮")`，中文显示时去掉「§」和后面的部分。
//! - 中文、日文里，占位符紧挨着汉字或假名、填进去的值又以字母或数字开头 / 结尾时，自动空一格：
//!   `"登录{site}"` 填 YouTube 是「登录 YouTube」，填哔哩哔哩是「登录哔哩哔哩」。所以中文原文里占位符两边不要自己加空格。
//! - 不在代码里写死的原文（站点名、存进数据里的来源名……）用 [`dynamic`] 查；这些原文要在某处用 `tr_key!("哔哩哔哩")` 标出来，
//!   `scripts/i18n.py check` 才知道它们要进词表。
//! - 不翻译的：只给开发者看的日志（tracing）、测试里的断言；给 AI 看的固定说明（MCP 的工具说明、`next_step`）只用英文，不走这里。
//!
//! 译文在 `locales/<语言>/<分册>.json`，编译时整个放进程序。查不到时：繁体中文退回原文，其他语言退回英文、再退回原文。
//! 当前语言整个进程共用：创建 `Core` 时、改设置时、命令行启动时按设置里的 `language` 定（空 = 跟随系统，
//! 系统的首选语言都不支持时用英文；环境变量 `CAPTAE_LANG` 优先，测试用）。从没设过时是中文（单元测试的断言按中文写）。

use std::cell::Cell;
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

/// 支持的语言。数值是 [`ALL`] 里的下标
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Lang {
    ZhCn = 0,
    En = 1,
    ZhTw = 2,
    Ja = 3,
    Ko = 4,
    Es = 5,
    Fr = 6,
    De = 7,
}

pub const ALL: [Lang; 8] = [Lang::ZhCn, Lang::En, Lang::ZhTw, Lang::Ja, Lang::Ko, Lang::Es, Lang::Fr, Lang::De];

impl Lang {
    /// 设置里存的、传给界面的代码
    pub fn code(self) -> &'static str {
        match self {
            Lang::ZhCn => "zh-CN",
            Lang::En => "en",
            Lang::ZhTw => "zh-TW",
            Lang::Ja => "ja",
            Lang::Ko => "ko",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::De => "de",
        }
    }

    /// 「zh-Hans-CN」「zh_TW」「en-US」「ja」「de_DE.UTF-8」→ 支持的语言；不支持的 None。
    /// 中文：带 Hant、TW、HK、MO 的算繁体，其余算简体
    pub fn parse(s: &str) -> Option<Lang> {
        let s = s.trim().replace('_', "-").to_ascii_lowercase();
        let s = s.split(['.', '@']).next().unwrap_or("");
        let mut parts = s.split('-');
        match parts.next()? {
            "zh" => Some(if parts.any(|p| matches!(p, "hant" | "tw" | "hk" | "mo")) { Lang::ZhTw } else { Lang::ZhCn }),
            "en" => Some(Lang::En),
            "ja" => Some(Lang::Ja),
            "ko" => Some(Lang::Ko),
            "es" => Some(Lang::Es),
            "fr" => Some(Lang::Fr),
            "de" => Some(Lang::De),
            _ => None,
        }
    }

    /// 中文、日文：占位符和汉字、假名之间按需要自动加空格
    fn cjk_spacing(self) -> bool {
        matches!(self, Lang::ZhCn | Lang::ZhTw | Lang::Ja)
    }

    /// 复数的类别（CLDR 的整数规则，够这几种语言用）
    fn plural(self, n: f64) -> &'static str {
        match self {
            Lang::En | Lang::De | Lang::Es => {
                if n == 1.0 {
                    "one"
                } else {
                    "other"
                }
            }
            // 法文：0 和 1 都是单数
            Lang::Fr => {
                if (0.0..2.0).contains(&n) {
                    "one"
                } else {
                    "other"
                }
            }
            _ => "other",
        }
    }
}

// ------------------------------------------------------------------ 当前语言

static CURRENT: AtomicU8 = AtomicU8::new(Lang::ZhCn as u8);

thread_local! {
    static OVERRIDE: Cell<Option<Lang>> = const { Cell::new(None) };
}

pub fn set(l: Lang) {
    CURRENT.store(l as u8, Ordering::Relaxed);
}

pub fn current() -> Lang {
    OVERRIDE.with(|o| o.get()).unwrap_or_else(|| ALL[CURRENT.load(Ordering::Relaxed) as usize])
}

/// 只在这个线程里临时换一种语言（测试用，不影响别的线程）
pub fn with_lang<R>(l: Lang, f: impl FnOnce() -> R) -> R {
    let prev = OVERRIDE.with(|o| o.replace(Some(l)));
    let r = f();
    OVERRIDE.with(|o| o.set(prev));
    r
}

fn env_override() -> Option<Lang> {
    std::env::var("CAPTAE_LANG").ok().and_then(|s| Lang::parse(&s))
}

/// 系统的首选语言里第一个支持的；都不支持时英文。`CAPTAE_LANG` 优先
pub fn system() -> Lang {
    if let Some(l) = env_override() {
        return l;
    }
    // 核心库自己的单元测试：不管这台电脑是什么语言都按中文（断言是按中文写的）
    if cfg!(test) {
        return Lang::ZhCn;
    }
    sys_locale::get_locales().find_map(|s| Lang::parse(&s)).unwrap_or(Lang::En)
}

/// 设置里的 `language`（空 = 跟随系统）→ 用哪种语言。`CAPTAE_LANG` 优先
pub fn resolve(setting: &str) -> Lang {
    env_override().or_else(|| Lang::parse(setting)).unwrap_or_else(system)
}

/// 按设置定下这个进程用的语言
pub fn apply(setting: &str) -> Lang {
    let l = resolve(setting);
    set(l);
    l
}

// ------------------------------------------------------------------ 词表

/// 词表的分册：几个人同时加译文时各改各的。download：下载流程、错误码、下载引擎；core：核心库其余部分；app：桌面端外壳、命令行
pub const PARTS: [&str; 3] = ["download", "core", "app"];

macro_rules! parts {
    ($lang:literal) => {
        &[
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/locales/", $lang, "/download.json")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/locales/", $lang, "/core.json")),
            include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/locales/", $lang, "/app.json")),
        ]
    };
}

fn sources(l: Lang) -> &'static [&'static str] {
    match l {
        Lang::ZhCn => &[],
        Lang::En => parts!("en"),
        Lang::ZhTw => parts!("zh-TW"),
        Lang::Ja => parts!("ja"),
        Lang::Ko => parts!("ko"),
        Lang::Es => parts!("es"),
        Lang::Fr => parts!("fr"),
        Lang::De => parts!("de"),
    }
}

/// 一条译文：一句话，或者按数量分的几种说法（one / other …）
#[derive(Debug)]
enum Entry {
    Text(String),
    Plural(HashMap<String, String>),
}

impl Entry {
    fn from_json(v: &serde_json::Value) -> Option<Entry> {
        match v {
            serde_json::Value::String(s) => Some(Entry::Text(s.clone())),
            serde_json::Value::Object(m) => Some(Entry::Plural(m.iter().filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string()))).collect())),
            _ => None,
        }
    }

    fn form(&self, l: Lang, n: Option<f64>) -> &str {
        match self {
            Entry::Text(s) => s,
            Entry::Plural(m) => n.and_then(|n| m.get(l.plural(n))).or_else(|| m.get("other")).map(String::as_str).unwrap_or(""),
        }
    }
}

static CATALOGS: [OnceLock<HashMap<String, Entry>>; 8] = [const { OnceLock::new() }; 8];

fn catalog(l: Lang) -> &'static HashMap<String, Entry> {
    CATALOGS[l as usize].get_or_init(|| {
        let mut m = HashMap::new();
        for src in sources(l) {
            match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(src) {
                Ok(obj) => {
                    for (k, v) in &obj {
                        if let Some(e) = Entry::from_json(v) {
                            m.insert(k.clone(), e);
                        }
                    }
                }
                Err(e) => tracing::error!("{} 的词表读不了：{e}", l.code()),
            }
        }
        m
    })
}

/// 原文去掉「§说明」
fn source(key: &str) -> &str {
    key.split('§').next().unwrap_or(key)
}

/// 取译文模板（还没填占位符）和它是哪种语言写的（决定要不要在汉字和字母之间加空格）
fn lookup(l: Lang, key: &str, n: Option<f64>) -> (String, Lang) {
    if l != Lang::ZhCn {
        if let Some(e) = catalog(l).get(key) {
            return (e.form(l, n).to_string(), l);
        }
        if l != Lang::ZhTw {
            if let Some(e) = catalog(Lang::En).get(key) {
                return (e.form(Lang::En, n).to_string(), Lang::En);
            }
        }
    }
    (source(key).to_string(), Lang::ZhCn)
}

/// `tr!("原文")`
pub fn text(key: &str) -> String {
    lookup(current(), key, None).0
}

/// `tr!("原文 {x}", x = …)`
pub fn fill(key: &str, args: &[(&str, &dyn Display)]) -> String {
    let vals: Vec<(&str, String)> = args.iter().map(|(k, v)| (*k, v.to_string())).collect();
    let n = vals.iter().find(|(k, _)| *k == "n").and_then(|(_, v)| v.trim().parse::<f64>().ok());
    let (tpl, l) = lookup(current(), key, n);
    render(&tpl, &vals, l.cjk_spacing())
}

/// 不是写在代码里的原文（站点名、存进数据里的来源名、预设的步骤……）：有译文用译文，没有原样返回
pub fn dynamic(s: &str) -> String {
    text(s)
}

/// 列表里的分隔：中文、日文「、」，其他语言「, 」。`join(&["1080p", "720p"])` → 「1080p、720p」
pub fn join<S: AsRef<str>>(items: &[S]) -> String {
    let sep = crate::tr!("、§列表里的分隔");
    items.iter().map(|s| s.as_ref()).collect::<Vec<_>>().join(&sep)
}

/// 带小数的数：德文、西班牙文、法文用逗号当小数点（「1,4 GB」）。`digits` 是保留几位小数
pub fn decimal(x: f64, digits: usize) -> String {
    let s = format!("{x:.digits$}");
    match current() {
        Lang::De | Lang::Es | Lang::Fr => s.replace('.', ","),
        _ => s,
    }
}

/// 填占位符。`cjk`：汉字、假名紧挨着字母或数字时中间空一格
fn render(tpl: &str, vals: &[(&str, String)], cjk: bool) -> String {
    let mut out = String::with_capacity(tpl.len() + 16);
    let mut rest = tpl;
    while let Some(i) = rest.find('{') {
        let Some(len) = rest[i..].find('}') else { break };
        let name = &rest[i + 1..i + len];
        let after = &rest[i + len + 1..];
        match vals.iter().find(|(k, _)| *k == name) {
            Some((_, v)) => {
                out.push_str(&rest[..i]);
                if cjk && needs_space(out.chars().last(), v.chars().next()) {
                    out.push(' ');
                }
                out.push_str(v);
                if cjk && needs_space(v.chars().last(), after.chars().next()) {
                    out.push(' ');
                }
            }
            None => out.push_str(&rest[..i + len + 1]),
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

fn is_cjk(c: char) -> bool {
    matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}' | '\u{F900}'..='\u{FAFF}' | '\u{3040}'..='\u{30FF}')
}

fn needs_space(a: Option<char>, b: Option<char>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => (is_cjk(a) && b.is_ascii_alphanumeric()) || (a.is_ascii_alphanumeric() && is_cjk(b)),
        _ => false,
    }
}

/// 界面文字：`tr!("原文")`、`tr!("登录{site}", site = name)`、`tr!("这里有 {n} 个视频", n = count)`。返回 `String`。
/// 原文必须是字符串字面量（`scripts/i18n.py` 从源码里找它们），不要用 `r"…"`。见模块说明
#[macro_export]
macro_rules! tr {
    ($key:literal $(,)?) => {
        $crate::i18n::text($key)
    };
    ($key:literal, $($name:ident = $val:expr),+ $(,)?) => {
        $crate::i18n::fill($key, &[$((stringify!($name), &$val as &dyn ::std::fmt::Display)),+])
    };
}

/// 只标出「这句原文要进词表」，原样返回字面量（gettext 的 N_）。用在常量表里，用的时候再 [`dynamic`]
#[macro_export]
macro_rules! tr_key {
    ($key:literal) => {
        $key
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_codes() {
        assert_eq!(Lang::parse("zh-Hans-CN"), Some(Lang::ZhCn));
        assert_eq!(Lang::parse("zh_CN"), Some(Lang::ZhCn));
        assert_eq!(Lang::parse("zh"), Some(Lang::ZhCn));
        assert_eq!(Lang::parse("zh-Hant-TW"), Some(Lang::ZhTw));
        assert_eq!(Lang::parse("zh-HK"), Some(Lang::ZhTw));
        assert_eq!(Lang::parse("en-US"), Some(Lang::En));
        assert_eq!(Lang::parse("de_DE.UTF-8"), Some(Lang::De));
        assert_eq!(Lang::parse("ja-JP"), Some(Lang::Ja));
        assert_eq!(Lang::parse("pt-BR"), None);
        assert_eq!(Lang::parse(""), None);
        for l in ALL {
            assert_eq!(Lang::parse(l.code()), Some(l));
            assert_eq!(ALL[l as usize], l);
        }
    }

    #[test]
    fn cjk_spacing_and_context() {
        with_lang(Lang::ZhCn, || {
            assert_eq!(tr!("登录{site}", site = "YouTube"), "登录 YouTube");
            assert_eq!(tr!("登录{site}", site = "哔哩哔哩"), "登录哔哩哔哩");
            assert_eq!(fill("在{site}验证", &[("site", &"X")]), "在 X 验证");
            assert_eq!(fill("共{n}个", &[("n", &3)]), "共 3 个");
            assert_eq!(fill("共 {n} 个", &[("n", &3)]), "共 3 个");
            assert_eq!(fill("「{title}」已经下载过了", &[("title", &"Sintel")]), "「Sintel」已经下载过了");
            assert_eq!(fill("没填的 {x} 留着", &[]), "没填的 {x} 留着");
            assert_eq!(text("下载§按钮"), "下载");
        });
        // 英文模板原样填，不加空格
        assert_eq!(render("Sign in to {site}", &[("site", "YouTube".into())], false), "Sign in to YouTube");
        with_lang(Lang::ZhCn, || assert_eq!((join(&["1080p", "720p"]), decimal(1.25, 1)), ("1080p、720p".to_string(), "1.2".to_string())));
        with_lang(Lang::De, || assert_eq!(decimal(1.5, 1), "1,5"));
    }

    #[test]
    fn plural_forms() {
        let e = Entry::Plural([("one".to_string(), "{n} video".to_string()), ("other".to_string(), "{n} videos".to_string())].into());
        assert_eq!(e.form(Lang::En, Some(1.0)), "{n} video");
        assert_eq!(e.form(Lang::En, Some(0.0)), "{n} videos");
        assert_eq!(e.form(Lang::Fr, Some(0.0)), "{n} video");
        assert_eq!(e.form(Lang::Ja, Some(1.0)), "{n} videos");
        assert_eq!(e.form(Lang::En, None), "{n} videos");
    }

    /// 每种语言的词表都是合法 JSON，而且只有英文以外的语言才可能缺条目（缺不缺由 scripts/i18n.py check 管）
    #[test]
    fn catalogs_parse() {
        for l in ALL {
            for (i, src) in sources(l).iter().enumerate() {
                assert!(serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(src).is_ok(), "{} 的 {} 不是合法的 JSON 对象", l.code(), PARTS[i]);
            }
        }
    }
}
