//! MCP Server（stdio 传输，JSON-RPC 2.0，一行一条消息）。
//!
//! 由 `captae mcp --stdio` 启动。桌面端在运行时，工具调用转发给桌面端（队列、进度、调用记录都在界面上可见）；
//! 桌面端没开时在本进程里直接执行，共用同一个素材库。
//!
//! 除了协议本身，这个进程还做两件事：
//! - 盯着 settings.json：用户在界面上关掉 AI 访问或某个功能时，发 `notifications/tools/list_changed`，
//!   客户端重新取 tools/list 就看不到被关掉的工具了
//! - 把连上来的客户端（名字、版本、最近一次调用）写进 `<数据目录>/mcp_clients/<pid>.json`，
//!   桌面端「AI 助手」页据此显示真实的连接状态

use crate::agent;
use crate::tr;
use anyhow::{anyhow, bail, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};

pub const PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

pub type CallFut = Pin<Box<dyn Future<Output = Result<Value>> + Send>>;

/// 工具调用的执行方：本进程 Core 或远端桌面端。
pub trait Backend: Send + Sync + 'static {
    fn call(&self, client: String, name: String, args: Value) -> CallFut;
    fn settings(&self) -> crate::config::Settings;
}

/// 当前设置下 AI 能看到的工具（按 agent::TOOLS 的顺序）。AI 访问关掉时一个都没有。
pub fn visible_tools(s: &crate::config::Settings) -> Vec<&'static str> {
    if !s.mcp_on {
        return vec![];
    }
    agent::TOOLS.iter().filter(|t| !agent::tool_off(s, t.name)).map(|t| t.name).collect()
}

pub fn tools_list(s: &crate::config::Settings) -> Value {
    let names = visible_tools(s);
    let tools: Vec<Value> = agent::TOOLS
        .iter()
        .filter(|t| names.contains(&t.name))
        .map(|t| {
            // title 是 AI 客户端显示给用户看的名字：跟着界面语言（label 存的是中文原文）；description 是给 AI 看的，只有英文
            let title = crate::i18n::dynamic(t.label);
            json!({
                "name": t.name,
                "title": title,
                "description": t.description,
                "inputSchema": (t.schema)(),
                "annotations": {"title": title, "readOnlyHint": t.read_only, "destructiveHint": false, "idempotentHint": t.read_only, "openWorldHint": !t.read_only}
            })
        })
        .collect();
    json!({"tools": tools})
}

/// 给 AI 看的固定说明：只用英文，不跟界面语言（docs/I18N.md 第 1 节）
const INSTRUCTIONS: &str = "Captae is a local media tool on the user's computer. When you need a video, audio or image from a web page, use download \
(you don't need to deal with anti-scraping measures or formats yourself). Process existing files with convert / trim / subtitle. \
Check the library with library_query first in case the file is already there. To understand what a video is about, use share_video, \
which gives you a transcript and key frames. Only download content the user has the right to use. \
The tool list changes with the user's settings (tools the user turned off don't appear).";

/// 把工具结果转成 MCP content。share_video 的关键帧作为图片附上，agent 能直接看到画面。
pub fn to_content(name: &str, v: &Value) -> Value {
    let mut content = vec![];
    if name == "share_video" {
        if let Some(md) = v.get("markdown").and_then(Value::as_str) {
            content.push(json!({"type": "text", "text": md}));
        }
        if let Some(frames) = v.get("frames").and_then(Value::as_array) {
            for f in frames.iter().filter_map(Value::as_str).take(crate::ffmpeg::MAX_FRAMES) {
                if let Ok(b) = std::fs::read(f) {
                    content.push(json!({"type": "image", "mimeType": "image/jpeg", "data": base64::engine::general_purpose::STANDARD.encode(b)}));
                }
            }
        }
        let mut meta = v.clone();
        if let Some(o) = meta.as_object_mut() {
            o.remove("markdown");
        }
        content.push(json!({"type": "text", "text": serde_json::to_string_pretty(&meta).unwrap_or_default()}));
    } else {
        content.push(json!({"type": "text", "text": serde_json::to_string_pretty(v).unwrap_or_default()}));
    }
    json!({"content": content, "structuredContent": v, "isError": false})
}

/// 工具失败时的结果：`{"error": 机器读的代码, "message": 给人看的一句话（跟着界面语言）}`，放在 isError 的结果里。
/// 下载一类的失败另带 `code`（错误码）、`next`（下一步怎么办）、`action`、`raw`，见 `agent::error_json`。
pub fn error_content(e: &anyhow::Error) -> Value {
    let v = agent::error_json(e);
    json!({"content": [{"type": "text", "text": serde_json::to_string_pretty(&v).unwrap_or_default()}], "structuredContent": v, "isError": true})
}

// ------------------------------------------------------------------ 会话

/// 一个 stdio 连接的状态。
pub struct Session {
    /// 界面上显示的客户端名字（Claude Desktop / Cursor …）
    pub client: Mutex<String>,
    rec: Mutex<Option<ClientRecord>>,
    /// 客户端最近一次拿到的工具列表，设置变了和它比较，决定要不要发 list_changed
    listed: Mutex<Option<Vec<&'static str>>>,
    status_dir: Option<PathBuf>,
}

impl Session {
    pub fn new(data: Option<&Path>) -> Session {
        // 「AI 助手」是存进调用记录、状态文件的原文（界面显示时再翻译），不在这里翻
        Session { client: Mutex::new("AI 助手".into()), rec: Mutex::new(None), listed: Mutex::new(None), status_dir: data.map(clients_dir) }
    }

    fn touch(&self, f: impl FnOnce(&mut ClientRecord)) {
        let mut g = self.rec.lock().unwrap();
        if let Some(r) = g.as_mut() {
            f(r);
            if let Some(d) = &self.status_dir {
                let _ = write_record(d, r);
            }
        }
    }

    /// 设置变了以后调用：可见工具和客户端上次看到的不一样时返回 true（并记下新的列表）。
    pub fn tools_changed(&self, s: &crate::config::Settings) -> bool {
        let now = visible_tools(s);
        let mut l = self.listed.lock().unwrap();
        match l.as_ref() {
            Some(prev) if *prev != now => {
                *l = Some(now);
                true
            }
            _ => false,
        }
    }
}

pub async fn handle(backend: &Arc<dyn Backend>, session: &Session, msg: Value) -> Option<Value> {
    let id = msg.get("id").cloned();
    let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or(Value::Null);
    let result: std::result::Result<Value, (i64, String)> = match method {
        "initialize" => {
            let want = params.get("protocolVersion").and_then(Value::as_str).unwrap_or(PROTOCOL_VERSIONS[0]);
            let ver = if PROTOCOL_VERSIONS.contains(&want) { want } else { PROTOCOL_VERSIONS[0] };
            let raw = params.pointer("/clientInfo/name").and_then(Value::as_str).unwrap_or("").to_string();
            let name = if raw.is_empty() { "AI 助手".to_string() } else { pretty_client(&raw) };
            *session.client.lock().unwrap() = name.clone();
            let now = crate::util::now_ms();
            *session.rec.lock().unwrap() = Some(ClientRecord {
                pid: std::process::id(),
                name,
                raw_name: raw,
                version: params.pointer("/clientInfo/version").and_then(Value::as_str).unwrap_or("").to_string(),
                protocol: ver.to_string(),
                started: now,
                seen: now,
                ..Default::default()
            });
            session.touch(|_| {});
            Ok(json!({
                "protocolVersion": ver,
                "capabilities": {"tools": {"listChanged": true}},
                "serverInfo": {"name": "captae", "title": "Captae", "version": env!("CARGO_PKG_VERSION")},
                "instructions": INSTRUCTIONS
            }))
        }
        "ping" => Ok(json!({})),
        "tools/list" => {
            let s = backend.settings();
            *session.listed.lock().unwrap() = Some(visible_tools(&s));
            Ok(tools_list(&s))
        }
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("").to_string();
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let who = session.client.lock().unwrap().clone();
            session.touch(|r| {
                let now = crate::util::now_ms();
                r.seen = now;
                r.last_call = Some(now);
                r.last_tool = Some(agent::tool(&name).map(|t| agent::display_name(t.name)).unwrap_or_else(|| name.clone()));
                r.calls += 1;
            });
            match backend.call(who, name.clone(), args).await {
                Ok(v) => Ok(to_content(&name, &v)),
                // 工具执行失败按 MCP 约定放进结果里（isError），让 agent 能读到原因并调整
                Err(e) => Ok(error_content(&e)),
            }
        }
        "resources/list" => Ok(json!({"resources": []})),
        "prompts/list" => Ok(json!({"prompts": []})),
        m if m.starts_with("notifications/") => return None,
        _ => Err((-32601, format!("Method not found: {method}"))),
    };
    let id = id?;
    Some(match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}),
        Err((code, message)) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}}),
    })
}

fn pretty_client(n: &str) -> String {
    let l = n.to_ascii_lowercase();
    if l.contains("claude") && l.contains("code") {
        "Claude Code".into()
    } else if l.contains("claude") {
        "Claude Desktop".into()
    } else if l.contains("cursor") {
        "Cursor".into()
    } else if l.contains("pi") {
        "Pi Agent".into()
    } else {
        n.to_string()
    }
}

pub const LIST_CHANGED: &str = r#"{"jsonrpc":"2.0","method":"notifications/tools/list_changed"}"#;

pub struct ServeOpts {
    /// 数据目录：settings.json 在这里，连接状态也写在这里。None 时不写状态、每次轮询都重读设置
    pub data: Option<PathBuf>,
    /// 多久看一次 settings.json
    pub poll: Duration,
    /// 多久刷新一次连接状态文件
    pub heartbeat: Duration,
}

impl ServeOpts {
    pub fn new(data: Option<PathBuf>) -> ServeOpts {
        ServeOpts { data, poll: Duration::from_secs(2), heartbeat: HEARTBEAT }
    }
}

pub async fn serve_stdio(backend: Arc<dyn Backend>, data: Option<PathBuf>) -> Result<()> {
    serve(backend, ServeOpts::new(data), tokio::io::stdin(), tokio::io::stdout()).await
}

pub async fn serve<R, W>(backend: Arc<dyn Backend>, opts: ServeOpts, input: R, output: W) -> Result<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
    let writer = tokio::spawn(async move {
        let mut out = output;
        while let Some(v) = rx.recv().await {
            let mut line = serde_json::to_vec(&v).unwrap_or_default();
            line.push(b'\n');
            if out.write_all(&line).await.is_err() {
                break;
            }
            let _ = out.flush().await;
        }
    });
    let session = Arc::new(Session::new(opts.data.as_deref()));

    // 设置变了 → list_changed；顺带刷新连接状态的心跳
    let watcher = {
        let (b, s, tx) = (backend.clone(), session.clone(), tx.clone());
        let settings_file = opts.data.as_ref().map(|d| d.join("settings.json"));
        let (poll, beat) = (opts.poll, opts.heartbeat);
        tokio::spawn(async move {
            let mtime = |p: &Option<PathBuf>| p.as_ref().and_then(|p| std::fs::metadata(p).ok()).and_then(|m| m.modified().ok());
            let mut last_m = mtime(&settings_file);
            let mut last_beat = std::time::Instant::now();
            loop {
                tokio::time::sleep(poll).await;
                let m = mtime(&settings_file);
                if settings_file.is_none() || m != last_m {
                    last_m = m;
                    let fresh = b.settings();
                    // 界面语言改了：之后给人看的说法跟着换（给 AI 看的说明只有英文，不受影响）
                    crate::i18n::apply(&fresh.language);
                    if s.tools_changed(&fresh) {
                        let _ = tx.send(serde_json::from_str(LIST_CHANGED).unwrap());
                    }
                }
                if last_beat.elapsed() >= beat {
                    last_beat = std::time::Instant::now();
                    s.touch(|r| r.seen = crate::util::now_ms());
                }
            }
        })
    };

    let mut lines = BufReader::new(input).lines();
    while let Some(l) = lines.next_line().await? {
        if l.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&l) {
            Ok(v) => v,
            Err(_) => {
                let _ = tx.send(json!({"jsonrpc": "2.0", "id": null, "error": {"code": -32700, "message": "Parse error"}}));
                continue;
            }
        };
        // 批量请求（2025-03-26 允许）
        let msgs = if let Value::Array(a) = msg { a } else { vec![msg] };
        for m in msgs {
            let (b, s, tx) = (backend.clone(), session.clone(), tx.clone());
            // initialize 要先处理完再处理后续消息，保证 client 名字已知
            if m.get("method").and_then(Value::as_str) == Some("initialize") {
                if let Some(r) = handle(&b, &s, m).await {
                    let _ = tx.send(r);
                }
                continue;
            }
            tokio::spawn(async move {
                if let Some(r) = handle(&b, &s, m).await {
                    let _ = tx.send(r);
                }
            });
        }
    }
    watcher.abort();
    session.touch(|r| {
        r.seen = crate::util::now_ms();
        r.ended = true;
    });
    drop(tx);
    // 还在跑的工具调用各自持有 tx，等它们写完结果
    let _ = writer.await;
    Ok(())
}

// ------------------------------------------------------------------ 连接状态

/// 心跳间隔；超过 CONNECTED_WITHIN 没有心跳就算断开（进程被强杀时没机会写 ended）。
pub const HEARTBEAT: Duration = Duration::from_secs(20);
pub const CONNECTED_WITHIN_MS: i64 = 65_000;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientRecord {
    pub pid: u32,
    pub name: String,
    pub raw_name: String,
    pub version: String,
    pub protocol: String,
    pub started: i64,
    /// 最近一次心跳
    pub seen: i64,
    pub last_call: Option<i64>,
    pub last_tool: Option<String>,
    pub calls: u64,
    /// 客户端正常断开
    pub ended: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ClientStatus {
    pub name: String,
    pub version: String,
    /// 最近一次有动静（心跳或调用），毫秒时间戳
    pub last_seen: i64,
    pub last_call: Option<i64>,
    pub last_tool: Option<String>,
    pub calls: u64,
    pub connected: bool,
}

pub fn clients_dir(data: &Path) -> PathBuf {
    data.join("mcp_clients")
}

fn write_record(dir: &Path, r: &ClientRecord) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let p = dir.join(format!("{}.json", r.pid));
    let tmp = dir.join(format!(".{}.tmp", r.pid));
    std::fs::write(&tmp, serde_json::to_vec(r)?)?;
    std::fs::rename(tmp, p)?;
    Ok(())
}

/// 读出所有客户端的状态：同一个客户端（名字相同）只留最近的一条，连着的排前面。
/// 断开超过 30 天的记录顺手删掉。
pub fn client_status(data: &Path, now: i64) -> Vec<ClientStatus> {
    let dir = clients_dir(data);
    let mut all: Vec<ClientStatus> = vec![];
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.extension().map(|x| x != "json").unwrap_or(true) {
            continue;
        }
        let Some(r) = std::fs::read(&p).ok().and_then(|b| serde_json::from_slice::<ClientRecord>(&b).ok()) else { continue };
        let last_seen = r.seen.max(r.last_call.unwrap_or(0));
        let connected = !r.ended && now - r.seen < CONNECTED_WITHIN_MS;
        if !connected && now - last_seen > 30 * 86_400_000 {
            let _ = std::fs::remove_file(&p);
            continue;
        }
        all.push(ClientStatus { name: r.name, version: r.version, last_seen, last_call: r.last_call, last_tool: r.last_tool, calls: r.calls, connected });
    }
    all.sort_by(|a, b| b.connected.cmp(&a.connected).then(b.last_call.unwrap_or(0).cmp(&a.last_call.unwrap_or(0))).then(b.last_seen.cmp(&a.last_seen)));
    let mut out: Vec<ClientStatus> = vec![];
    for c in all {
        match out.iter_mut().find(|x| x.name == c.name) {
            // 同名的另一个进程（比如 Claude 重启过）：连接状态取“有一个连着就算”，调用时间取最近的
            Some(x) => {
                x.connected |= c.connected;
                x.last_call = x.last_call.max(c.last_call);
                x.last_seen = x.last_seen.max(c.last_seen);
            }
            None => out.push(c),
        }
    }
    out
}

// ------------------------------------------------------------------ 写入客户端配置

/// 客户端的 MCP 配置文件。`home` 是用户目录（测试时换成临时目录）。
/// - Claude Desktop：macOS `~/Library/Application Support/Claude/`；Windows `%APPDATA%\Claude\`，
///   商店版 / MSIX 版的 Claude 读的是包里的虚拟目录（`%LOCALAPPDATA%\Packages\Claude_<发布者>\LocalCache\Roaming\Claude\`），
///   那里已经有配置文件时写那一份；Linux（官方没有 Linux 版，社区打包的照 Electron 的惯例）`$XDG_CONFIG_HOME/Claude/`，默认 `~/.config/Claude/`
/// - Cursor：三个平台都是 `~/.cursor/mcp.json`（Windows 是 `%USERPROFILE%\.cursor\mcp.json`）
pub fn client_config_path(client: &str, home: &Path) -> Option<PathBuf> {
    const CLAUDE: &str = "claude_desktop_config.json";
    // 传进来的就是这个用户的主目录时，才用环境变量里的 %APPDATA% / %LOCALAPPDATA% / XDG_CONFIG_HOME（测试时 home 是临时目录）
    let real = home == crate::util::home_dir();
    let env_dir = |k: &str| real.then(|| std::env::var_os(k)).flatten().map(PathBuf::from).filter(|p| p.is_absolute());
    match client {
        "claude" => Some(if cfg!(target_os = "macos") {
            home.join("Library/Application Support/Claude").join(CLAUDE)
        } else if cfg!(windows) {
            let local = env_dir("LOCALAPPDATA").unwrap_or_else(|| home.join("AppData").join("Local"));
            let msix = std::fs::read_dir(local.join("Packages"))
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().starts_with("Claude_"))
                .map(|e| e.path().join("LocalCache").join("Roaming").join("Claude").join(CLAUDE))
                .find(|p| p.is_file());
            msix.unwrap_or_else(|| env_dir("APPDATA").unwrap_or_else(|| home.join("AppData").join("Roaming")).join("Claude").join(CLAUDE))
        } else {
            env_dir("XDG_CONFIG_HOME").unwrap_or_else(|| home.join(".config")).join("Claude").join(CLAUDE)
        }),
        "cursor" => Some(home.join(".cursor").join("mcp.json")),
        _ => None,
    }
}

/// 写进配置里的那一项：`"captae": {"command": ..., "args": ["mcp", "--stdio"]}`
pub fn server_entry(command: &str) -> Value {
    json!({"command": command, "args": ["mcp", "--stdio"]})
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigPlan {
    pub path: PathBuf,
    pub exists: bool,
    /// 要合并进去的内容（整段 JSON 文字，界面上原样给用户看）
    pub snippet: String,
    /// 原来已经有的 captae 设置（会被替换）
    pub previous: Option<Value>,
    /// 已经一模一样，不需要写
    pub unchanged: bool,
    /// 文件里其他的 MCP 服务器，合并后原样保留
    pub others: Vec<String>,
}

fn parse_config(text: &str) -> Result<serde_json::Map<String, Value>> {
    if text.trim().is_empty() {
        return Ok(Default::default());
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(m)) => Ok(m),
        Ok(_) => bail!(tr!("这个配置文件的内容不是 JSON 对象，Captae 没有改动它")),
        Err(e) => bail!(tr!("这个配置文件不是有效的 JSON（{error}），Captae 没有改动它。请先手动修好，或者用「复制」自己粘贴", error = e)),
    }
}

/// 把 `entry` 合并进已有配置（mcpServers.captae），其他内容保持原样。返回新文件内容。
pub fn merge_config(existing: &str, entry: &Value) -> Result<String> {
    let mut m = parse_config(existing)?;
    let servers = m.entry("mcpServers").or_insert_with(|| json!({}));
    let Some(obj) = servers.as_object_mut() else { bail!(tr!("配置文件里的 mcpServers 不是对象，Captae 没有改动它")) };
    obj.insert("captae".into(), entry.clone());
    let mut s = serde_json::to_string_pretty(&Value::Object(m))?;
    s.push('\n');
    Ok(s)
}

pub fn plan_config(path: &Path, entry: &Value) -> Result<ConfigPlan> {
    let exists = path.exists();
    let text = if exists { std::fs::read_to_string(path)? } else { String::new() };
    let m = parse_config(&text)?;
    let servers = m.get("mcpServers").and_then(Value::as_object);
    let previous = servers.and_then(|s| s.get("captae")).cloned();
    let others = servers.map(|s| s.keys().filter(|k| *k != "captae").cloned().collect()).unwrap_or_default();
    Ok(ConfigPlan {
        path: path.to_path_buf(),
        exists,
        snippet: serde_json::to_string_pretty(&json!({"mcpServers": {"captae": entry}}))?,
        unchanged: previous.as_ref() == Some(entry),
        previous,
        others,
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigWritten {
    pub path: PathBuf,
    /// 原文件的备份；原来没有这个文件时为空
    pub backup: Option<PathBuf>,
    pub unchanged: bool,
}

/// 合并写入客户端配置：先把原文件备份成 `.bak`，再原子替换。原文件解析失败时什么都不改。
pub fn write_config(path: &Path, entry: &Value) -> Result<ConfigWritten> {
    let plan = plan_config(path, entry)?;
    if plan.unchanged {
        return Ok(ConfigWritten { path: path.to_path_buf(), backup: None, unchanged: true });
    }
    let old = if plan.exists { std::fs::read_to_string(path)? } else { String::new() };
    let new = merge_config(&old, entry)?;
    let dir = path.parent().ok_or_else(|| anyhow!(tr!("配置文件路径不对")))?;
    std::fs::create_dir_all(dir)?;
    let backup = if plan.exists {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let b = crate::util::unique_path(dir.join(format!("{name}.bak")));
        std::fs::copy(path, &b)?;
        Some(b)
    } else {
        None
    };
    let tmp = dir.join(format!(".{}.captae-tmp", path.file_name().unwrap_or_default().to_string_lossy()));
    std::fs::write(&tmp, new)?;
    std::fs::rename(&tmp, path)?;
    Ok(ConfigWritten { path: path.to_path_buf(), backup, unchanged: false })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Settings;

    struct Echo(Mutex<Settings>);
    impl Backend for Echo {
        fn call(&self, client: String, name: String, args: Value) -> CallFut {
            Box::pin(async move {
                if name == "fail" {
                    anyhow::bail!("TOOL_OFF: nope")
                }
                Ok(json!({"client": client, "name": name, "args": args}))
            })
        }
        fn settings(&self) -> Settings {
            self.0.lock().unwrap().clone()
        }
    }
    fn echo(s: Settings) -> Arc<Echo> {
        Arc::new(Echo(Mutex::new(s)))
    }
    fn names(r: &Value) -> Vec<String> {
        r["result"]["tools"].as_array().unwrap().iter().map(|t| t["name"].as_str().unwrap().to_string()).collect()
    }

    #[tokio::test]
    async fn protocol() {
        let dir = tempfile::tempdir().unwrap();
        let b: Arc<dyn Backend> = echo(Settings { tools_off: vec!["captae.trim".into()], ..Default::default() });
        let s = Session::new(Some(dir.path()));
        let r = handle(&b, &s, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","clientInfo":{"name":"claude-ai","version":"0.9.2"}}})).await.unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(r["result"]["capabilities"]["tools"]["listChanged"], true);
        assert_eq!(*s.client.lock().unwrap(), "Claude Desktop");
        for v in PROTOCOL_VERSIONS {
            let r = handle(&b, &s, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":v}})).await.unwrap();
            assert_eq!(r["result"]["protocolVersion"], *v);
        }
        let r = handle(&b, &s, json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"1999-01-01","clientInfo":{"name":"claude-ai","version":"0.9.2"}}})).await.unwrap();
        assert_eq!(r["result"]["protocolVersion"], PROTOCOL_VERSIONS[0]);
        let r = handle(&b, &s, json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).await.unwrap();
        let n = names(&r);
        assert!(n.contains(&"download".into()) && !n.contains(&"trim".into()));
        let r = handle(&b, &s, json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"download","arguments":{"url":"u"}}})).await.unwrap();
        assert_eq!(r["result"]["structuredContent"]["args"]["url"], "u");
        let r = handle(&b, &s, json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"fail"}})).await.unwrap();
        assert_eq!(r["result"]["isError"], true);
        assert_eq!(r["result"]["structuredContent"]["error"], "TOOL_OFF");
        assert_eq!(r["result"]["structuredContent"]["message"], "nope");
        assert!(handle(&b, &s, json!({"jsonrpc":"2.0","method":"notifications/initialized"})).await.is_none());
        let r = handle(&b, &s, json!({"jsonrpc":"2.0","id":5,"method":"nope"})).await.unwrap();
        assert_eq!(r["error"]["code"], -32601);
        // 连接状态文件
        let st = client_status(dir.path(), crate::util::now_ms());
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].name, "Claude Desktop");
        assert_eq!(st[0].version, "0.9.2");
        assert!(st[0].connected);
        assert_eq!(st[0].calls, 2);
        assert_eq!(st[0].last_tool.as_deref(), Some("fail"));
        assert!(st[0].last_call.is_some());
    }

    #[test]
    fn list_filtering() {
        let all = Settings::default();
        assert_eq!(visible_tools(&all).len(), agent::TOOLS.len());
        // 总开关关掉：一个工具都不给
        let off = Settings { mcp_on: false, ..Default::default() };
        assert!(visible_tools(&off).is_empty());
        assert_eq!(tools_list(&off)["tools"].as_array().unwrap().len(), 0);
        // 单独关掉：界面上的名字、内部名字、旧版界面名字都认
        for o in ["captae.library.query", "library_query", "captae.library.add.tag", "captae.share.video"] {
            let s = Settings { tools_off: vec![o.into()], ..Default::default() };
            let v = visible_tools(&s);
            assert_eq!(v.len(), agent::TOOLS.len() - 1, "{o}");
            let hidden = agent::tool(o).unwrap().name;
            assert!(!v.contains(&hidden));
            assert!(!tools_list(&s)["tools"].as_array().unwrap().iter().any(|t| t["name"] == hidden));
        }
    }

    #[test]
    fn list_changed_tracking() {
        let s = Session::new(None);
        let on = Settings::default();
        // 客户端还没取过列表时不发通知
        assert!(!s.tools_changed(&Settings { mcp_on: false, ..Default::default() }));
        *s.listed.lock().unwrap() = Some(visible_tools(&on));
        assert!(!s.tools_changed(&on));
        // 和工具无关的设置变化不发
        assert!(!s.tools_changed(&Settings { concurrency: 9, ..Default::default() }));
        assert!(s.tools_changed(&Settings { tools_off: vec!["captae.trim".into()], ..Default::default() }));
        // 已经通知过，同样的状态不重复发
        assert!(!s.tools_changed(&Settings { tools_off: vec!["captae.trim".into()], ..Default::default() }));
        assert!(s.tools_changed(&Settings { mcp_on: false, ..Default::default() }));
        assert!(s.tools_changed(&on));
    }

    /// 真实的 serve 循环：settings.json 变了以后客户端收到 list_changed，再取列表时工具少了。
    #[tokio::test]
    async fn serve_sends_list_changed() {
        let dir = tempfile::tempdir().unwrap();
        let sp = dir.path().join("settings.json");
        Settings::default().save(&sp).unwrap();
        struct FileBackend(PathBuf);
        impl Backend for FileBackend {
            fn call(&self, _: String, _: String, _: Value) -> CallFut {
                Box::pin(async { Ok(json!({})) })
            }
            fn settings(&self) -> Settings {
                Settings::load(&self.0)
            }
        }
        let (client, server) = tokio::io::duplex(1 << 16);
        let (sr, sw) = tokio::io::split(server);
        let opts = ServeOpts { data: Some(dir.path().to_path_buf()), poll: Duration::from_millis(20), heartbeat: Duration::from_millis(50) };
        let task = tokio::spawn(serve(Arc::new(FileBackend(sp.clone())), opts, sr, sw));
        let (cr, mut cw) = tokio::io::split(client);
        let mut lines = BufReader::new(cr).lines();
        let send = |v: Value| {
            let mut l = serde_json::to_vec(&v).unwrap();
            l.push(b'\n');
            l
        };
        cw.write_all(&send(json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","clientInfo":{"name":"cursor","version":"1.2"}}}))).await.unwrap();
        let r: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(r["result"]["protocolVersion"], "2024-11-05");
        cw.write_all(&send(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"}))).await.unwrap();
        let r: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(names(&r).len(), agent::TOOLS.len());
        // 用户在界面上关掉了 AI 访问（mtime 精度可能是秒，等一下再写）
        tokio::time::sleep(Duration::from_millis(1100)).await;
        Settings { mcp_on: false, ..Default::default() }.save(&sp).unwrap();
        let n: Value = tokio::time::timeout(Duration::from_secs(5), lines.next_line()).await.unwrap().unwrap().map(|l| serde_json::from_str(&l).unwrap()).unwrap();
        assert_eq!(n["method"], "notifications/tools/list_changed");
        assert!(n.get("id").is_none());
        cw.write_all(&send(json!({"jsonrpc":"2.0","id":3,"method":"tools/list"}))).await.unwrap();
        let r: Value = serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert!(names(&r).is_empty());
        let st = client_status(dir.path(), crate::util::now_ms());
        assert_eq!(st[0].name, "Cursor");
        assert!(st[0].connected);
        cw.shutdown().await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), task).await.unwrap().unwrap().unwrap();
        // 客户端断开后记为未连接
        let st = client_status(dir.path(), crate::util::now_ms());
        assert!(!st[0].connected);
    }

    #[test]
    fn status_dedupe_and_staleness() {
        let dir = tempfile::tempdir().unwrap();
        let d = clients_dir(dir.path());
        let now = 10_000_000_000;
        let rec = |pid, name: &str, seen, last_call, ended| ClientRecord { pid, name: name.into(), seen, last_call, ended, ..Default::default() };
        write_record(&d, &rec(1, "Claude Desktop", now - 5_000, Some(now - 120_000), false)).unwrap();
        write_record(&d, &rec(2, "Claude Desktop", now - 900_000, Some(now - 60_000), true)).unwrap();
        // 被强杀、很久没心跳
        write_record(&d, &rec(3, "Cursor", now - 600_000, None, false)).unwrap();
        // 一个多月前断开的，会被清理
        write_record(&d, &rec(4, "Old", now - 40 * 86_400_000, None, true)).unwrap();
        let st = client_status(dir.path(), now);
        assert_eq!(st.len(), 2);
        assert_eq!(st[0].name, "Claude Desktop");
        assert!(st[0].connected);
        assert_eq!(st[0].last_call, Some(now - 60_000));
        assert_eq!(st[1].name, "Cursor");
        assert!(!st[1].connected);
        assert!(!d.join("4.json").exists());
    }

    #[test]
    fn config_merge() {
        let home = tempfile::tempdir().unwrap();
        let entry = server_entry("/Applications/Captae.app/Contents/MacOS/captae");
        // 没有文件：新建
        let p = client_config_path("cursor", home.path()).unwrap();
        assert!(p.ends_with(".cursor/mcp.json"));
        let plan = plan_config(&p, &entry).unwrap();
        assert!(!plan.exists && plan.previous.is_none() && !plan.unchanged);
        let w = write_config(&p, &entry).unwrap();
        assert!(w.backup.is_none());
        let v: Value = serde_json::from_str(&std::fs::read_to_string(&p).unwrap()).unwrap();
        assert_eq!(v["mcpServers"]["captae"], entry);
        // 再写一次：一模一样，不动文件也不备份
        assert!(write_config(&p, &entry).unwrap().unchanged);

        // 已有别的服务器和其他设置：保留，原文件备份
        let p = client_config_path("claude", home.path()).unwrap();
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        let orig = r#"{"globalShortcut":"Cmd+Space","mcpServers":{"fs":{"command":"npx","args":["x"]},"captae":{"command":"old"}}}"#;
        std::fs::write(&p, orig).unwrap();
        let plan = plan_config(&p, &entry).unwrap();
        assert_eq!(plan.others, vec!["fs".to_string()]);
        assert_eq!(plan.previous, Some(json!({"command": "old"})));
        assert!(plan.snippet.contains("\"captae\""));
        let w = write_config(&p, &entry).unwrap();
        let bak = w.backup.unwrap();
        assert!(bak.file_name().unwrap().to_string_lossy().starts_with("claude_desktop_config.json"));
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), orig);
        let text = std::fs::read_to_string(&p).unwrap();
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["globalShortcut"], "Cmd+Space");
        assert_eq!(v["mcpServers"]["fs"]["command"], "npx");
        assert_eq!(v["mcpServers"]["captae"], entry);
        // 原来的键顺序保留
        assert!(text.find("globalShortcut").unwrap() < text.find("mcpServers").unwrap());
        // 第二次备份不覆盖第一次的
        let entry2 = server_entry("captae");
        let w2 = write_config(&p, &entry2).unwrap();
        assert_ne!(w2.backup.unwrap(), bak);
        assert_eq!(std::fs::read_to_string(&bak).unwrap(), orig);

        // 坏掉的 JSON：拒绝，原文件不动
        std::fs::write(&p, "{oops").unwrap();
        assert!(write_config(&p, &entry).is_err());
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "{oops");
        std::fs::write(&p, r#"{"mcpServers": []}"#).unwrap();
        assert!(write_config(&p, &entry).is_err());
        // 空文件当成空配置
        std::fs::write(&p, "").unwrap();
        write_config(&p, &entry).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&std::fs::read_to_string(&p).unwrap()).unwrap()["mcpServers"]["captae"], entry);
    }

    /// 各平台客户端配置文件的位置；Windows 的路径写进 JSON 时反斜杠要转义
    #[test]
    fn config_paths_per_platform() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let claude = client_config_path("claude", home).unwrap();
        let rel = claude.strip_prefix(home).unwrap().to_string_lossy().replace('\\', "/");
        let want = if cfg!(target_os = "macos") {
            "Library/Application Support/Claude/claude_desktop_config.json"
        } else if cfg!(windows) {
            "AppData/Roaming/Claude/claude_desktop_config.json"
        } else {
            ".config/Claude/claude_desktop_config.json"
        };
        assert_eq!(rel, want);
        assert_eq!(client_config_path("cursor", home).unwrap(), home.join(".cursor").join("mcp.json"));
        assert!(client_config_path("nope", home).is_none());
        // Windows：MSIX 版 Claude 的虚拟目录里已经有配置文件时写那一份
        if cfg!(windows) {
            let v = home.join(r"AppData\Local\Packages\Claude_pzs8sxrjxfjjc\LocalCache\Roaming\Claude");
            std::fs::create_dir_all(&v).unwrap();
            std::fs::write(v.join("claude_desktop_config.json"), "{}").unwrap();
            assert_eq!(client_config_path("claude", home).unwrap(), v.join("claude_desktop_config.json"));
        }
        // captae.exe 的路径写进 JSON：反斜杠转义成 \\，读回来原样
        let entry = server_entry(r"C:\Program Files\Captae\captae.exe");
        let text = merge_config("", &entry).unwrap();
        assert!(text.contains(r#""command": "C:\\Program Files\\Captae\\captae.exe""#), "{text}");
        let v: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(v["mcpServers"]["captae"]["command"], r"C:\Program Files\Captae\captae.exe");
    }
}
