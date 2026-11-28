//! 在看不见的网页里读视频信息。
//!
//! 有的网站下载引擎读不了，网页自己却拿得到：抖音的详情接口要网页自己算的签名参数，yt-dlp 不带就回 403（见 `douyin.rs`）。
//! 桌面端在一块看不见的 webview 里打开网页（和登录窗口同一份存储，登录过、验证过的状态都在），页面一开始就挂上 [`init_script`]：
//! - 网页里的声音、视频一律静音（看不见的窗口里也会自动播放）
//! - 接住地址里含 [`APIS`] 某一段的 fetch / XHR 回应，让页面跳到 `https://captae-page.invalid/…`，回应原文放在地址里；
//!   桌面端在导航检查里认出这个地址（[`parse_report`]）、拦下来（网页不动），交给 [`Capture::judge`] 看能不能用
//!
//! 命令行、MCP 自己起的进程、手机端没有这块 webview（[`reader`] 是 None），照旧交给下载引擎。

use anyhow::Result;
use base64::Engine;
use futures_util::future::BoxFuture;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// 页面把接住的回应交出来时跳到的主机（`.invalid` 永远不会真的打开）
pub const REPORT_HOST: &str = "captae-page.invalid";

/// 页面里要接住哪些请求的回应：地址里含这一段的。桌面端建 webview 时按这张表生成脚本，所以要接新的请求得加在这里
pub const APIS: &[&str] = &["/aweme/v1/web/aweme/detail/"];

/// 接住的一份回应能不能用
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    /// 就要这一份，不用再等了
    Take,
    /// 不是要的（别的视频的、被拦下的），接着等
    Wait,
    /// 这个网页不会再给出能用的了（被拦了好几次），不用等到超时
    Stop,
}

/// 读一个网页
pub struct Capture {
    /// 打开哪个网页
    pub url: String,
    /// 最多等多久
    pub timeout: Duration,
    /// 看接住的每一份回应：(接口地址, 回应原文) → 要不要
    pub judge: Box<dyn FnMut(&str, &str) -> Verdict + Send>,
}

/// 读的结果
#[derive(Debug, Default, Clone)]
pub struct Seen {
    /// judge 说要的那一份
    pub taken: Option<String>,
    /// 最后接住的、没要的那一份的开头（报错原文用）
    pub last: Option<String>,
    /// 一共接住几份
    pub count: usize,
    /// judge 说不用再等了
    pub stopped: bool,
}

/// 看不见的网页，由桌面端实现
pub trait Reader: Send + Sync + 'static {
    fn capture(&self, c: Capture) -> BoxFuture<'static, Result<Seen>>;
}

static READER: RwLock<Option<Arc<dyn Reader>>> = RwLock::new(None);

/// 桌面端启动时登记
pub fn set_reader(r: Arc<dyn Reader>) {
    *READER.write().unwrap() = Some(r);
}

/// 有没有看不见的网页可用（只有桌面端有）
pub fn reader() -> Option<Arc<dyn Reader>> {
    READER.read().unwrap().clone()
}

/// 读完以后桌面端先在页面里执行它，页面就不再往外交（见 [`init_script`]），再换空白页
pub const STOP_SCRIPT: &str = "window.__captaePage&&(window.__captaeStop=true)";

/// 页面一开始挂的脚本（只在主框架里）：静音，接住 [`APIS`] 里的请求的回应，一份一份交出来（隔 150 毫秒，免得前一次跳转还没拦下就被下一次盖掉）
pub fn init_script() -> String {
    let apis = serde_json::to_string(APIS).unwrap_or_else(|_| "[]".into());
    SCRIPT.replace("__APIS__", &apis).replace("__HOST__", REPORT_HOST)
}

const SCRIPT: &str = r#"(() => {
  if (window.__captaePage) return;
  window.__captaePage = true;
  // 静音：看不见的窗口里网页照样会自动播放
  try {
    const P = HTMLMediaElement.prototype, md = Object.getOwnPropertyDescriptor(P, 'muted');
    const hush = (el) => { try { md.set.call(el, true); } catch (e) {} };
    Object.defineProperty(P, 'muted', { configurable: true, get() { return md.get.call(this); }, set() { hush(this); } });
    const play = P.play;
    P.play = function () { hush(this); return play.apply(this, arguments); };
    ['loadstart', 'play', 'playing', 'volumechange'].forEach((t) => document.addEventListener(t, (e) => { if (e.target instanceof HTMLMediaElement) hush(e.target); }, true));
    ['AudioContext', 'webkitAudioContext'].forEach((n) => {
      const A = window[n];
      if (!A) return;
      A.prototype.resume = function () { return Promise.resolve(); };
      window[n] = new Proxy(A, { construct(t, a) { const c = new t(...a); try { c.suspend(); } catch (e) {} return c; } });
    });
  } catch (e) {}
  const APIS = __APIS__;
  const q = [];
  let busy = false;
  const b64 = (t) => {
    const b = new TextEncoder().encode(t);
    let s = '';
    for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode.apply(null, b.subarray(i, i + 0x8000));
    return btoa(s).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');
  };
  // 桌面端读完了（window.__captaeStop）：不再往外交。排着的跳转会顶掉桌面端紧接着发的导航（换空白页、去下一个网页）
  const pump = () => {
    if (window.__captaeStop) q.length = 0;
    if (busy || !q.length) return;
    busy = true;
    const [u, t] = q.shift();
    try { location.href = 'https://__HOST__/r?u=' + encodeURIComponent(u) + '#' + b64(t); } catch (e) {}
    setTimeout(() => { busy = false; pump(); }, 150);
  };
  const report = (u, t) => {
    if (window.__captaeStop) return;
    try {
      const abs = new URL(String(u), location.href).href;
      if (!APIS.some((a) => abs.includes(a))) return;
      q.push([abs, String(t || '')]);
      pump();
    } catch (e) {}
  };
  const f = window.fetch;
  if (f) {
    window.fetch = function (...a) {
      return f.apply(this, a).then((r) => {
        try {
          const u = typeof a[0] === 'string' ? a[0] : (a[0] && a[0].url) || String(a[0]);
          if (APIS.some((x) => String(u).includes(x))) r.clone().text().then((t) => report(u, t), () => {});
        } catch (e) {}
        return r;
      });
    };
  }
  const X = XMLHttpRequest.prototype, open = X.open, send = X.send;
  X.open = function (m, u, ...rest) { this.__captaeUrl = u; return open.call(this, m, u, ...rest); };
  X.send = function (...a) {
    try {
      if (APIS.some((x) => String(this.__captaeUrl).includes(x))) {
        this.addEventListener('load', () => {
          try {
            const t = this.responseType === '' || this.responseType === 'text' ? this.responseText : JSON.stringify(this.response);
            report(this.__captaeUrl, t);
          } catch (e) {}
        });
      }
    } catch (e) {}
    return send.apply(this, a);
  };
})();"#;

/// 页面跳到的报告地址 → (接口地址, 回应原文)。不是报告地址、内容不对时返回 None
pub fn parse_report(url: &str) -> Option<(String, String)> {
    let u = url::Url::parse(url).ok()?;
    if u.host_str()? != REPORT_HOST {
        return None;
    }
    let api = u.query_pairs().find(|(k, _)| k == "u").map(|(_, v)| v.into_owned())?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(u.fragment()?).ok()?;
    Some((api, String::from_utf8(bytes).ok()?))
}

/// 是不是报告地址（桌面端的导航检查一律拦下）
pub fn is_report(u: &url::Url) -> bool {
    u.host_str() == Some(REPORT_HOST)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_roundtrip() {
        // 和页面里的 b64 一样：UTF-8 → base64url、去掉末尾的 =
        let body = r#"{"aweme_detail":{"desc":"熟悉的飞车BGM"}}"#;
        let enc = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(body.as_bytes());
        let api = "https://www.douyin.com/aweme/v1/web/aweme/detail/?aweme_id=1&a_bogus=x%2Fy";
        let url = format!("https://{REPORT_HOST}/r?u={}#{enc}", urlencoding(api));
        assert_eq!(parse_report(&url), Some((api.to_string(), body.to_string())));
        assert!(is_report(&url::Url::parse(&url).unwrap()));
        // 别的地址、坏的内容：不认
        assert_eq!(parse_report("https://example.com/r?u=x#e30"), None);
        assert_eq!(parse_report(&format!("https://{REPORT_HOST}/r?u=x#***")), None);
        assert_eq!(parse_report(&format!("https://{REPORT_HOST}/r#e30")), None);
    }

    #[test]
    fn script_has_apis() {
        let s = init_script();
        assert!(s.contains(REPORT_HOST) && s.contains("/aweme/v1/web/aweme/detail/"));
        assert!(!s.contains("__APIS__") && !s.contains("__HOST__"));
    }

    fn urlencoding(s: &str) -> String {
        url::form_urlencoded::byte_serialize(s.as_bytes()).collect()
    }
}
