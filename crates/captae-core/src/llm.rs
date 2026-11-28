//! 「AI 模型」：用户自己填的 OpenAI 兼容接口（BYOK）。字幕翻译（subtitle.rs）和素材库分类（categorize.rs）共用。
//!
//! 只走 `/chat/completions` 一条路：DeepSeek、OpenAI、OpenRouter、Gemini 的兼容接口、本地 Ollama / LM Studio 都是这个格式。
//! 各家不一样的地方在这里处理：
//! - 要 JSON 时带 `response_format: json_object`；有的服务（LM Studio）不认，回 400 时去掉它再问一次，回来的文字里自己找 JSON
//! - 思考档位 `reasoning_effort` 只发给 DeepSeek（别家的普通模型收到不认识的参数会报 400）
//! - 图片用 data URL（`image_url`，`detail: low`，约 300 token 一张）
//!
//! 给模型看的提示词只用英文，不跟界面语言（docs/I18N.md 第 1 节）；要模型写给人看的文字时，在提示词里说用哪种语言（`lang_name`）。

use crate::tr;
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use serde::Serialize;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Llm {
    pub base_url: String,
    pub model: String,
    pub api_key: String,
}

impl Llm {
    /// 设置里填了地址和模型才算有（密钥可以没有：本地的 Ollama、LM Studio 不要）
    pub fn from_settings(s: &crate::Settings) -> Option<Llm> {
        if s.llm_base_url.trim().is_empty() || s.llm_model.trim().is_empty() {
            return None;
        }
        Some(Llm { base_url: s.llm_base_url.trim().into(), model: s.llm_model.trim().into(), api_key: s.llm_api_key.trim().into() })
    }

    /// 接受 `https://api.openai.com/v1`、`http://localhost:11434/v1`，也接受直接填到 `/chat/completions` 的地址。
    pub fn endpoint(&self) -> String {
        let b = self.base_url.trim().trim_end_matches('/');
        if b.ends_with("/chat/completions") {
            b.to_string()
        } else {
            format!("{b}/chat/completions")
        }
    }

    /// 接口的主机名（界面上「会发给 api.deepseek.com」）
    pub fn host(&self) -> String {
        reqwest::Url::parse(&self.endpoint()).ok().and_then(|u| u.host_str().map(str::to_string)).unwrap_or_else(|| self.base_url.clone())
    }

    /// DeepSeek 官方接口：认 `reasoning_effort`（none / low / high / max），默认开着思考
    fn is_deepseek(&self) -> bool {
        self.host().ends_with("deepseek.com")
    }
}

/// 消息里的一段：文字或一张图片
#[derive(Debug, Clone)]
pub enum Part {
    Text(String),
    /// 本地图片文件（缩略图）
    Image(PathBuf),
    /// 现成的图片字节和类型（测试连接用的小图）
    ImageBytes(&'static [u8], &'static str),
}

#[derive(Debug, Clone)]
pub struct Chat {
    pub system: String,
    pub user: Vec<Part>,
    /// 要模型只回 JSON
    pub json: bool,
    /// 让模型先想一想（只对认这个参数的服务有用，见 `Llm::is_deepseek`）。逐条归类这种简单活关掉，省钱也快
    pub think: bool,
    pub timeout: Duration,
}

impl Chat {
    pub fn new(system: impl Into<String>, user: Vec<Part>) -> Chat {
        Chat { system: system.into(), user, json: false, think: false, timeout: Duration::from_secs(180) }
    }
    pub fn json(mut self) -> Chat {
        self.json = true;
        self
    }
    pub fn think(mut self, on: bool) -> Chat {
        self.think = on;
        self
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Usage {
    pub prompt: u64,
    pub completion: u64,
}

impl Usage {
    pub fn add(&mut self, o: &Usage) {
        self.prompt += o.prompt;
        self.completion += o.completion;
    }
}

#[derive(Debug, Clone)]
pub struct Reply {
    pub text: String,
    pub usage: Usage,
}

fn mime_of(p: &Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref() {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "image/jpeg",
    }
}

fn image_part(bytes: &[u8], mime: &str) -> Value {
    let b = base64::engine::general_purpose::STANDARD.encode(bytes);
    json!({"type": "image_url", "image_url": {"url": format!("data:{mime};base64,{b}"), "detail": "low"}})
}

fn has_image(parts: &[Part]) -> bool {
    parts.iter().any(|p| !matches!(p, Part::Text(_)))
}

fn content(parts: &[Part]) -> Result<Value> {
    // 只有文字时发字符串（有的本地服务不认数组写法）
    if !has_image(parts) {
        let t: Vec<&str> = parts.iter().filter_map(|p| if let Part::Text(t) = p { Some(t.as_str()) } else { None }).collect();
        return Ok(json!(t.join("\n")));
    }
    let mut v = vec![];
    for p in parts {
        v.push(match p {
            Part::Text(t) => json!({"type": "text", "text": t}),
            Part::Image(path) => image_part(&std::fs::read(path).with_context(|| tr!("读不了图片：{path}", path = path.display()))?, mime_of(path)),
            Part::ImageBytes(b, m) => image_part(b, m),
        });
    }
    Ok(Value::Array(v))
}

/// 模型服务回了错误：按状态码给一句话（四种整句，不拼片段，docs/I18N.md 2.3）。`status` 留着，调用方据此决定要不要换个写法再试
#[derive(Debug)]
pub struct HttpError {
    pub status: u16,
    pub message: String,
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HttpError {}

fn http_error(code: u16, body: &str) -> HttpError {
    let body: String = body.chars().take(200).collect();
    let message = match code {
        401 | 403 => tr!("AI 模型返回了错误 {status}，检查一下设置里的密钥：{body}", status = code, body = body),
        404 => tr!("AI 模型返回了错误 {status}，检查一下设置里的地址和模型名：{body}", status = code, body = body),
        429 => tr!("AI 模型返回了错误 {status}，请求太频繁或额度用完了：{body}", status = code, body = body),
        _ => tr!("AI 模型返回了错误 {status}：{body}", status = code, body = body),
    };
    HttpError { status: code, message }
}

/// 问一次。要 JSON 而服务不认 `response_format` 时（回 400），去掉它再问一次
pub async fn chat(http: &reqwest::Client, llm: &Llm, c: &Chat) -> Result<Reply> {
    match send(http, llm, c, c.json).await {
        Err(e) if c.json && e.downcast_ref::<HttpError>().is_some_and(|h| h.status == 400) => send(http, llm, c, false).await,
        r => r,
    }
}

async fn send(http: &reqwest::Client, llm: &Llm, c: &Chat, json_format: bool) -> Result<Reply> {
    let mut body = json!({
        "model": llm.model,
        "stream": false,
        "messages": [
            {"role": "system", "content": c.system},
            {"role": "user", "content": content(&c.user)?}
        ]
    });
    if !c.think {
        body["temperature"] = json!(0.2);
    }
    if json_format {
        body["response_format"] = json!({"type": "json_object"});
    }
    if llm.is_deepseek() {
        body["reasoning_effort"] = json!(if c.think { "low" } else { "none" });
    }
    let mut r = http.post(llm.endpoint()).timeout(c.timeout).json(&body);
    if !llm.api_key.is_empty() {
        r = r.bearer_auth(&llm.api_key);
    }
    let resp = r.send().await.map_err(|e| anyhow!(tr!("连不上 AI 模型：{error}", error = e)))?;
    let status = resp.status();
    if !status.is_success() {
        let t = resp.text().await.unwrap_or_default();
        return Err(http_error(status.as_u16(), &t).into());
    }
    let v: Value = resp.json().await.context(tr!("AI 模型返回的内容无法解析"))?;
    let text = v["choices"][0]["message"]["content"].as_str().ok_or_else(|| anyhow!(tr!("AI 模型没有返回内容")))?.to_string();
    let u = &v["usage"];
    Ok(Reply { text, usage: Usage { prompt: u["prompt_tokens"].as_u64().unwrap_or(0), completion: u["completion_tokens"].as_u64().unwrap_or(0) } })
}

/// 从模型回的文字里取出 JSON 对象：容忍代码块包裹、前后多说的话
pub fn parse_json(text: &str) -> Result<Value> {
    let t = text.trim();
    if let Ok(v) = serde_json::from_str::<Value>(t) {
        if v.is_object() {
            return Ok(v);
        }
    }
    let (Some(a), Some(b)) = (t.find('{'), t.rfind('}')) else { bail!(tr!("AI 模型回的不是要的格式")) };
    if b <= a {
        bail!(tr!("AI 模型回的不是要的格式"));
    }
    serde_json::from_str(&t[a..=b]).map_err(|_| anyhow!(tr!("AI 模型回的不是要的格式")))
}

/// 问一次、按 JSON 解析
pub async fn chat_json(http: &reqwest::Client, llm: &Llm, c: &Chat) -> Result<(Value, Usage)> {
    let r = chat(http, llm, c).await?;
    Ok((parse_json(&r.text)?, r.usage))
}

/// 提示词里说「用某种语言写」时用的英文名字
pub fn lang_name(l: crate::i18n::Lang) -> &'static str {
    use crate::i18n::Lang;
    match l {
        Lang::ZhCn => "Simplified Chinese",
        Lang::ZhTw => "Traditional Chinese (Taiwan)",
        Lang::En => "English",
        Lang::Ja => "Japanese",
        Lang::Ko => "Korean",
        Lang::Es => "Spanish",
        Lang::Fr => "French",
        Lang::De => "German",
    }
}

/// 测试连接用的小图：64×64 的纯红色 PNG
const RED_PNG: &[u8] = include_bytes!("../assets/llm-test-red.png");

#[derive(Debug, Clone, Serialize)]
pub struct TestResult {
    /// 能看图：发一张红色小图，问是什么颜色，答对了才算
    pub vision: bool,
    pub ms: u64,
}

/// 设置里的「测试连接」：先问一句只要回 JSON 的话，再发一张红色小图问颜色
pub async fn test(http: &reqwest::Client, llm: &Llm) -> Result<TestResult> {
    let t0 = std::time::Instant::now();
    let c = Chat::new("You are a connection test. Reply with JSON only.", vec![Part::Text("Reply with {\"ok\": true}".into())]).json();
    let mut c = c;
    c.timeout = Duration::from_secs(60);
    chat(http, llm, &c).await?;
    let ms = t0.elapsed().as_millis() as u64;
    let mut img = Chat::new(
        "You look at an image and name its main color. Reply with JSON only.",
        vec![Part::Text("What is the main color of this image? Reply with {\"color\": \"<one English word>\"}".into()), Part::ImageBytes(RED_PNG, "image/png")],
    )
    .json();
    img.timeout = Duration::from_secs(60);
    let vision = match chat(http, llm, &img).await {
        Ok(r) => r.text.to_ascii_lowercase().contains("red"),
        Err(e) => {
            tracing::info!("测试看图没有通过：{e}");
            false
        }
    };
    Ok(TestResult { vision, ms })
}

#[cfg(test)]
pub(crate) mod testkit {
    //! 假的 OpenAI 兼容服务：按收到的请求调 `reply` 回内容，记下每次请求的 JSON
    use std::sync::{Arc, Mutex};

    pub type Handler = Arc<dyn Fn(&serde_json::Value) -> (u16, String) + Send + Sync>;

    pub async fn serve(reply: Handler) -> (String, Arc<Mutex<Vec<serde_json::Value>>>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let seen = Arc::new(Mutex::new(vec![]));
        let s2 = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = l.accept().await else { break };
                let reply = reply.clone();
                let seen = s2.clone();
                tokio::spawn(async move {
                    let mut buf = vec![];
                    let mut tmp = [0u8; 65536];
                    // 读到请求头和 Content-Length 那么长的正文
                    let body = loop {
                        let n = sock.read(&mut tmp).await.unwrap_or(0);
                        if n == 0 {
                            return;
                        }
                        buf.extend_from_slice(&tmp[..n]);
                        let text = String::from_utf8_lossy(&buf).to_string();
                        if let Some(i) = text.find("\r\n\r\n") {
                            let len = text[..i].lines().find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0))).unwrap_or(0);
                            if buf.len() >= i + 4 + len {
                                break buf[i + 4..i + 4 + len].to_vec();
                            }
                        }
                    };
                    let v: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();
                    seen.lock().unwrap().push(v.clone());
                    let (code, text) = reply(&v);
                    let out = if code == 200 {
                        serde_json::json!({"choices": [{"message": {"content": text}}], "usage": {"prompt_tokens": 100, "completion_tokens": 20}}).to_string()
                    } else {
                        text
                    };
                    let resp = format!("HTTP/1.1 {code} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{out}", out.len());
                    let _ = sock.write_all(resp.as_bytes()).await;
                });
            }
        });
        (format!("http://127.0.0.1:{port}/v1"), seen)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn json_from_messy_reply() {
        assert_eq!(parse_json("{\"a\": 1}").unwrap()["a"], 1);
        assert_eq!(parse_json("```json\n{\"a\": 2}\n```").unwrap()["a"], 2);
        assert_eq!(parse_json("Here you go:\n{\"items\": [{\"id\": 3}]}\nDone.").unwrap()["items"][0]["id"], 3);
        assert!(parse_json("no json here").is_err());
        assert!(parse_json("} {").is_err());
    }

    #[test]
    fn endpoint_and_host() {
        let l = Llm { base_url: "https://api.deepseek.com/".into(), model: "m".into(), api_key: String::new() };
        assert_eq!(l.endpoint(), "https://api.deepseek.com/chat/completions");
        assert_eq!(l.host(), "api.deepseek.com");
        assert!(l.is_deepseek());
        let l = Llm { base_url: "http://localhost:11434/v1/chat/completions".into(), model: "m".into(), api_key: String::new() };
        assert_eq!(l.endpoint(), "http://localhost:11434/v1/chat/completions");
        assert!(!l.is_deepseek());
        let s = crate::Settings { llm_base_url: " ".into(), llm_model: "x".into(), ..Default::default() };
        assert!(Llm::from_settings(&s).is_none());
    }

    /// 服务不认 response_format（回 400）：去掉再问一次；思考档位只发给 DeepSeek；图片按 data URL 发
    #[tokio::test]
    async fn retries_without_json_format_and_sends_images() {
        let (url, seen) = testkit::serve(Arc::new(|v: &Value| {
            if v.get("response_format").is_some() {
                (400, "{\"error\": \"response_format.type must be json_schema\"}".into())
            } else {
                (200, "sure: {\"color\": \"Red\"}".into())
            }
        }))
        .await;
        let llm = Llm { base_url: url, model: "local".into(), api_key: String::new() };
        let http = reqwest::Client::new();
        let r = test(&http, &llm).await.unwrap();
        assert!(r.vision);
        let seen = seen.lock().unwrap();
        // 文字一次（400）+ 去掉 response_format 再一次，图片同样两次
        assert_eq!(seen.len(), 4);
        assert!(seen.iter().all(|v| v.get("reasoning_effort").is_none()), "不是 DeepSeek 不发思考档位");
        let img = &seen[3]["messages"][1]["content"];
        assert!(img[1]["image_url"]["url"].as_str().unwrap().starts_with("data:image/png;base64,"));
        assert_eq!(seen[0]["messages"][1]["content"], "Reply with {\"ok\": true}", "只有文字时发字符串");
    }

    #[tokio::test]
    async fn error_messages_by_status() {
        let (url, _) = testkit::serve(Arc::new(|_: &Value| (401, "{\"error\": \"bad key\"}".into()))).await;
        let llm = Llm { base_url: url, model: "m".into(), api_key: "sk-x".into() };
        let e = chat(&reqwest::Client::new(), &llm, &Chat::new("s", vec![Part::Text("u".into())])).await.unwrap_err();
        assert_eq!(e.downcast_ref::<HttpError>().unwrap().status, 401);
        assert!(e.to_string().contains("密钥"), "{e}");
    }
}
