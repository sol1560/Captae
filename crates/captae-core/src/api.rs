//! 本地控制接口：桌面端运行时在 127.0.0.1 上开一个随机端口的 HTTP 服务，
//! 地址和一次性令牌写进 `<data>/control.json`（只有当前用户可读）。
//! `captae` 命令行、MCP 进程、浏览器插件的 Native Messaging 宿主都通过它把请求交给桌面端。

use crate::model::{AddRequest, JobAction};
use crate::queue::Core;
use crate::sniff::Sniffed;
use anyhow::{anyhow, Result};
use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlInfo {
    pub port: u16,
    pub token: String,
    pub pid: u32,
}

/// 插件、命令行要求把窗口拿到前面时，顺便要去的地方。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct FocusTarget {
    /// downloads / library / agent / extension
    pub page: Option<String>,
    /// 素材库里选中这条素材
    pub asset: Option<i64>,
    /// 下载页里选中这个任务（插件「在 Captae 里查看」还没下好的任务）
    pub job: Option<u64>,
    /// 下载页，并打开粘贴链接的输入框
    pub paste: bool,
}

impl FocusTarget {
    /// 窗口要去的界面路由（单页应用，`#/library?asset=5` 里 `#` 后面那段）；None 表示只把窗口拿到前面，不换页。
    pub fn href(&self) -> Option<String> {
        if let Some(id) = self.asset {
            return Some(format!("/library?asset={id}"));
        }
        if let Some(id) = self.job {
            return Some(format!("/downloads?t={id}"));
        }
        if self.paste {
            return Some("/downloads?paste".into());
        }
        match self.page.as_deref() {
            Some(p @ ("downloads" | "library" | "agent" | "extension")) => Some(format!("/{p}")),
            _ => None,
        }
    }
}

pub type OnFocus = std::sync::Arc<dyn Fn(FocusTarget) + Send + Sync>;

#[derive(Clone)]
struct AppState {
    core: Core,
    token: String,
    /// 桌面端想知道的额外事件（比如插件要求打开窗口）
    on_focus: OnFocus,
}

struct ApiError(anyhow::Error);
impl IntoResponse for ApiError {
    /// `{"error": 一句话}`；分类过的失败另带 `fail`（错误码、原文……），客户端据此还原，MCP、插件才拿得到错误码。
    fn into_response(self) -> Response {
        let mut v = json!({"error": self.0.to_string()});
        if let Some(f) = crate::errors::failure_in(&self.0) {
            v["fail"] = f.to_json();
        }
        if let Some(c) = crate::errors::code_in(&self.0) {
            v["code"] = json!(c);
        }
        (StatusCode::BAD_REQUEST, Json(v)).into_response()
    }
}

/// 失败的回应 → 错误。带 `fail` 时还原成 `Failure`（有大写代码的还原成 `Tagged`），错误码不会在转发时丢掉。
fn api_error(v: &Value) -> anyhow::Error {
    let msg = v["error"].as_str().map(str::to_string).unwrap_or_else(|| crate::tr!("请求失败"));
    match serde_json::from_value::<crate::errors::Failure>(v["fail"].clone()) {
        Ok(failure) => {
            let (tag, _) = crate::agent::split_error(&msg);
            if tag == "FAILED" {
                anyhow::Error::new(failure)
            } else {
                anyhow::Error::new(crate::errors::Tagged { tag, failure })
            }
        }
        // 带固定代码的（NOT_RECORDING……）还原成 Coded，插件宿主转发时代码还在
        Err(_) => match v["code"].as_str() {
            Some(c) => crate::errors::coded(c, msg),
            None => anyhow!("{msg}"),
        },
    }
}
impl<E: Into<anyhow::Error>> From<E> for ApiError {
    fn from(e: E) -> Self {
        ApiError(e.into())
    }
}

fn authed(s: &AppState, h: &HeaderMap) -> Result<(), StatusCode> {
    let ok = h.get("authorization").and_then(|v| v.to_str().ok()).map(|v| v == format!("Bearer {}", s.token)).unwrap_or(false);
    if ok { Ok(()) } else { Err(StatusCode::UNAUTHORIZED) }
}

#[derive(Deserialize)]
struct CallBody {
    client: String,
    tool: String,
    #[serde(default)]
    args: Value,
}

async fn ping(State(s): State<AppState>, h: HeaderMap) -> Result<Json<Value>, StatusCode> {
    authed(&s, &h)?;
    Ok(Json(json!({"app": "captae", "version": env!("CARGO_PKG_VERSION"), "pid": std::process::id()})))
}

async fn call(State(s): State<AppState>, h: HeaderMap, Json(b): Json<CallBody>) -> Result<Json<Value>, Response> {
    authed(&s, &h).map_err(IntoResponse::into_response)?;
    crate::agent::call(&s.core, &b.client, &b.tool, b.args).await.map(Json).map_err(|e| ApiError(e).into_response())
}

async fn add(State(s): State<AppState>, h: HeaderMap, Json(mut r): Json<AddRequest>) -> Result<Json<Value>, Response> {
    authed(&s, &h).map_err(IntoResponse::into_response)?;
    r.origin.get_or_insert_with(|| "api".into());
    let j = s.core.add(r).map_err(|e| ApiError(e).into_response())?;
    Ok(Json(serde_json::to_value(j).unwrap()))
}

async fn jobs(State(s): State<AppState>, h: HeaderMap) -> Result<Json<Value>, StatusCode> {
    authed(&s, &h)?;
    Ok(Json(serde_json::to_value(s.core.jobs()).unwrap()))
}

async fn job_wait(State(s): State<AppState>, h: HeaderMap, AxPath(id): AxPath<u64>) -> Result<Json<Value>, Response> {
    authed(&s, &h).map_err(IntoResponse::into_response)?;
    let j = s.core.wait(id).await.map_err(|e| ApiError(e).into_response())?;
    Ok(Json(serde_json::to_value(j).unwrap()))
}

async fn sniffed(State(s): State<AppState>, h: HeaderMap, Json(v): Json<Sniffed>) -> Result<Json<Value>, StatusCode> {
    authed(&s, &h)?;
    s.core.push_sniffed(v);
    Ok(Json(json!({"ok": true})))
}

async fn share(State(s): State<AppState>, h: HeaderMap, Json(mut o): Json<crate::share::ShareOpts>) -> Result<Json<Value>, StatusCode> {
    authed(&s, &h)?;
    o.copy = true;
    let core = s.core.clone();
    let title = o.page_title.clone().unwrap_or_else(|| o.url.clone().unwrap_or_default());
    let id = core.new_task_id();
    tokio::spawn(async move {
        let c2 = core.clone();
        let _ = core
            .run_task_as(id, "share", title, None, |rep| async move {
                let b = crate::share::share(&c2, o, |p, _| rep.pct(p.min(0.99))).await?;
                let mut v = serde_json::to_value(&b)?;
                // 插件卡片上的关键帧小图
                v["thumbs"] = json!(crate::share::frame_thumbs(&c2.toolbox(), &b.frames).await);
                if let Ok(Some(a)) = c2.lib.get(b.asset_id) {
                    v["title"] = json!(a.title);
                    v["site"] = json!(a.src);
                    v["dur"] = json!(a.dur);
                }
                Ok(v)
            })
            .await;
    });
    Ok(Json(json!({"accepted": true, "task": id})))
}

async fn task_cancel(State(s): State<AppState>, h: HeaderMap, AxPath(id): AxPath<u64>) -> Result<Json<Value>, StatusCode> {
    authed(&s, &h)?;
    Ok(Json(json!({"ok": s.core.cancel_task(id)})))
}

/// 「再复制一次」：文字和关键帧重新放进剪贴板
async fn task_copy(State(s): State<AppState>, h: HeaderMap, AxPath(id): AxPath<u64>) -> Result<Json<Value>, Response> {
    authed(&s, &h).map_err(IntoResponse::into_response)?;
    let t = s.core.task(id).ok_or_else(|| ApiError(anyhow!(crate::tr!("这次整理的内容已经找不到了"))).into_response())?;
    let r = t.result.ok_or_else(|| ApiError(anyhow!(crate::tr!("还没整理好"))).into_response())?;
    crate::share::recopy(&r).map_err(|e| ApiError(e).into_response())?;
    Ok(Json(json!({"ok": true})))
}

#[derive(Deserialize)]
struct ActBody {
    action: JobAction,
}

async fn job_action(State(s): State<AppState>, h: HeaderMap, AxPath(id): AxPath<u64>, Json(b): Json<ActBody>) -> Result<Json<Value>, Response> {
    authed(&s, &h).map_err(IntoResponse::into_response)?;
    s.core.act(id, b.action).map_err(|e| ApiError(e).into_response())?;
    Ok(Json(json!({"ok": true})))
}

async fn check(State(s): State<AppState>, h: HeaderMap, Json(b): Json<crate::native_host::CheckReq>) -> Result<Json<Value>, Response> {
    authed(&s, &h).map_err(IntoResponse::into_response)?;
    crate::native_host::check(&s.core.lib, &s.core.paths, &b).map(Json).map_err(|e| ApiError(e).into_response())
}

#[derive(Deserialize)]
struct ProbeBody {
    url: String,
    #[serde(default)]
    headers: std::collections::BTreeMap<String, String>,
    /// 插件带来的浏览器 cookies 和 User-Agent（只用这一次，不留）
    #[serde(default)]
    cookies: Vec<crate::login::Cookie>,
    #[serde(default)]
    user_agent: Option<String>,
}

async fn probe(State(s): State<AppState>, h: HeaderMap, Json(b): Json<ProbeBody>) -> Result<Json<Value>, Response> {
    authed(&s, &h).map_err(IntoResponse::into_response)?;
    let mut headers = b.headers;
    let mut cookies = b.cookies;
    cookies.extend(crate::login::take_cookie_header(&mut headers, &b.url));
    let dir = crate::login::auth_dir(&s.core.paths);
    let p = crate::probe::probe_ext(&s.core.toolbox(), &s.core.http(), &b.url, &headers, cookies, b.user_agent, dir).await.map_err(|e| ApiError(e).into_response())?;
    Ok(Json(serde_json::to_value(p).unwrap()))
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct HelloBody {
    version: Option<String>,
    /// 插件自己猜的浏览器（background.js 只认得几家，认不出的都说 chrome）
    browser: Option<String>,
    /// 宿主看父进程认出来的浏览器（native_host::browsers 的 id）
    detected: Option<String>,
}

/// 插件打招呼（每分钟一次），桌面端据此显示「插件已连上 · Arc」。浏览器用宿主认出来的，认不出时用插件自己报的
async fn hello(State(s): State<AppState>, h: HeaderMap, Json(b): Json<HelloBody>) -> Result<Json<Value>, StatusCode> {
    authed(&s, &h)?;
    let browser = b.detected.filter(|d| !d.is_empty()).or(b.browser);
    s.core.note_extension(b.version, browser);
    Ok(Json(json!({"ok": true})))
}

async fn task(State(s): State<AppState>, h: HeaderMap, AxPath(id): AxPath<u64>) -> Result<Json<Value>, StatusCode> {
    authed(&s, &h)?;
    s.core.task(id).map(|t| Json(serde_json::to_value(t).unwrap())).ok_or(StatusCode::NOT_FOUND)
}

async fn focus(State(s): State<AppState>, h: HeaderMap, body: Option<Json<FocusTarget>>) -> Result<Json<Value>, StatusCode> {
    authed(&s, &h)?;
    (s.on_focus)(body.map(|b| b.0).unwrap_or_default());
    Ok(Json(json!({"ok": true})))
}

/// 启动控制接口，返回写入 control.json 的信息。
pub async fn serve(core: Core, on_focus: OnFocus) -> Result<ControlInfo> {
    let token: String = {
        use rand::Rng;
        let mut r = rand::rng();
        (0..32).map(|_| format!("{:02x}", r.random::<u8>())).collect()
    };
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let st = AppState { core: core.clone(), token: token.clone(), on_focus };
    let app = Router::new()
        .route("/v1/ping", get(ping))
        .route("/v1/call", post(call))
        .route("/v1/jobs", post(add).get(jobs))
        .route("/v1/jobs/{id}/wait", get(job_wait))
        .route("/v1/jobs/{id}/action", post(job_action))
        .route("/v1/sniffed", post(sniffed))
        .route("/v1/share", post(share))
        .route("/v1/tasks/{id}", get(task))
        .route("/v1/tasks/{id}/cancel", post(task_cancel))
        .route("/v1/tasks/{id}/copy", post(task_copy))
        .route("/v1/check", post(check))
        .route("/v1/probe", post(probe))
        .route("/v1/hello", post(hello))
        .route("/v1/focus", post(focus))
        .with_state(st);
    tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    let info = ControlInfo { port, token, pid: std::process::id() };
    write_private(&core.paths.control(), &serde_json::to_vec(&info)?)?;
    Ok(info)
}

fn write_private(p: &Path, b: &[u8]) -> Result<()> {
    std::fs::write(p, b)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

pub fn clear(core: &Core) {
    let _ = std::fs::remove_file(core.paths.control());
}

/// 客户端：命令行和 MCP 进程用。
#[derive(Clone)]
pub struct Client {
    base: String,
    token: String,
    http: reqwest::Client,
}

impl Client {
    /// 桌面端在运行就返回客户端，否则 None。
    pub async fn connect(control: &Path) -> Option<Client> {
        Self::connect_within(control, 800).await
    }

    async fn connect_within(control: &Path, ms: u64) -> Option<Client> {
        let info: ControlInfo = serde_json::from_slice(&std::fs::read(control).ok()?).ok()?;
        let c = Client { base: format!("http://127.0.0.1:{}", info.port), token: info.token, http: reqwest::Client::builder().no_proxy().build().ok()? };
        let ok = c.http.get(format!("{}/v1/ping", c.base)).bearer_auth(&c.token).timeout(std::time::Duration::from_millis(ms)).send().await.ok()?.status().is_success();
        ok.then_some(c)
    }

    /// 和 `connect` 一样，但 control.json 里的桌面端进程还活着、只是一时没回 ping 时（正忙：几个下载同时收尾、入库截图）
    /// 再等一等、多问两次，不把「忙」当成「没开」。插件宿主用：浏览器插件的卡片曾因为一次 800ms 没回话就说「Captae 关掉了」。
    /// 真的没开时（没有 control.json，或者里面的进程已经不在了）和 `connect` 一样马上返回
    pub async fn connect_patient(control: &Path) -> Option<Client> {
        for ms in [800, 2000, 4000] {
            if let Some(c) = Self::connect_within(control, ms).await {
                return Some(c);
            }
            let info: ControlInfo = serde_json::from_slice(&std::fs::read(control).ok()?).ok()?;
            if !crate::tools::alive(info.pid) {
                return None;
            }
            tracing::info!("桌面端（进程 {}）{ms}ms 没回话，再问一次", info.pid);
        }
        None
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let r = self.http.post(format!("{}{path}", self.base)).bearer_auth(&self.token).json(body).send().await?;
        let st = r.status();
        let v: Value = r.json().await.unwrap_or(json!({}));
        if !st.is_success() {
            return Err(api_error(&v));
        }
        Ok(v)
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let r = self.http.get(format!("{}{path}", self.base)).bearer_auth(&self.token).send().await?;
        let st = r.status();
        let v: Value = r.json().await.unwrap_or(json!({}));
        if !st.is_success() {
            return Err(api_error(&v));
        }
        Ok(v)
    }

    pub async fn call(&self, client: &str, tool: &str, args: Value) -> Result<Value> {
        self.post("/v1/call", &json!({"client": client, "tool": tool, "args": args})).await
    }
    pub async fn add(&self, r: &AddRequest) -> Result<Value> {
        self.post("/v1/jobs", &serde_json::to_value(r)?).await
    }
    pub async fn sniffed(&self, s: &Sniffed) -> Result<Value> {
        self.post("/v1/sniffed", &serde_json::to_value(s)?).await
    }
    pub async fn share(&self, v: Value) -> Result<Value> {
        self.post("/v1/share", &v).await
    }
    pub async fn jobs(&self) -> Result<Value> {
        self.get("/v1/jobs").await
    }
    pub async fn task(&self, id: u64) -> Result<Value> {
        self.get(&format!("/v1/tasks/{id}")).await
    }
    pub async fn focus(&self, target: &FocusTarget) -> Result<Value> {
        self.post("/v1/focus", &serde_json::to_value(target)?).await
    }
    pub async fn job_action(&self, id: u64, action: JobAction) -> Result<Value> {
        self.post(&format!("/v1/jobs/{id}/action"), &json!({"action": action})).await
    }
    pub async fn task_cancel(&self, id: u64) -> Result<Value> {
        self.post(&format!("/v1/tasks/{id}/cancel"), &json!({})).await
    }
    pub async fn task_copy(&self, id: u64) -> Result<Value> {
        self.post(&format!("/v1/tasks/{id}/copy"), &json!({})).await
    }
    pub async fn check(&self, req: &crate::native_host::CheckReq) -> Result<Value> {
        self.post("/v1/check", &serde_json::to_value(req)?).await
    }
    pub async fn probe(&self, url: &str, headers: &std::collections::BTreeMap<String, String>, cookies: &[crate::login::Cookie], user_agent: Option<&str>) -> Result<Value> {
        self.post("/v1/probe", &json!({"url": url, "headers": headers, "cookies": cookies, "user_agent": user_agent})).await
    }
    /// `browser` 是插件自己报的，`detected` 是宿主认出来的（认不出是 None）
    pub async fn hello(&self, version: Option<&str>, browser: Option<&str>, detected: Option<&str>) -> Result<Value> {
        self.post("/v1/hello", &json!({"version": version, "browser": browser, "detected": detected})).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn focus_href() {
        assert_eq!(FocusTarget::default().href(), None);
        assert_eq!(FocusTarget { asset: Some(5), ..Default::default() }.href().as_deref(), Some("/library?asset=5"));
        assert_eq!(FocusTarget { paste: true, ..Default::default() }.href().as_deref(), Some("/downloads?paste"));
        assert_eq!(FocusTarget { page: Some("extension".into()), ..Default::default() }.href().as_deref(), Some("/extension"));
        assert_eq!(FocusTarget { page: Some("../../etc".into()), ..Default::default() }.href(), None);
        let t: FocusTarget = serde_json::from_value(json!({})).unwrap();
        assert_eq!(t, FocusTarget::default());
        // 插件「在 Captae 里查看」还没下好的任务：{page: 'downloads', job: 12}
        let t: FocusTarget = serde_json::from_value(json!({"page": "downloads", "job": 12})).unwrap();
        assert_eq!(t.href().as_deref(), Some("/downloads?t=12"));
        // 任务号是空的（插件那边还没拿到）：只去下载页
        let t: FocusTarget = serde_json::from_value(json!({"page": "downloads", "job": null})).unwrap();
        assert_eq!(t.href().as_deref(), Some("/downloads"));
        // 素材优先（下好了就看素材）
        assert_eq!(FocusTarget { asset: Some(5), job: Some(12), ..Default::default() }.href().as_deref(), Some("/library?asset=5"));
    }

    #[tokio::test]
    async fn routes_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let core = Core::open(crate::Options { paths: crate::Paths::portable(dir.path()), sidecar_dir: None, restore: false }).await.unwrap();
        let seen = std::sync::Arc::new(std::sync::Mutex::new(vec![]));
        let s2 = seen.clone();
        serve(core.clone(), std::sync::Arc::new(move |t| s2.lock().unwrap().push(t))).await.unwrap();
        let c = Client::connect(&core.paths.control()).await.expect("api is up");
        c.focus(&FocusTarget { asset: Some(7), ..Default::default() }).await.unwrap();
        assert_eq!(seen.lock().unwrap()[0].asset, Some(7));
        c.focus(&FocusTarget { page: Some("downloads".into()), job: Some(12), ..Default::default() }).await.unwrap();
        assert_eq!(seen.lock().unwrap()[1].href().as_deref(), Some("/downloads?t=12"));
        // 浏览器：宿主认出来的为准，认不出（空的、没给）时用插件自己报的
        c.hello(Some("0.2.0"), Some("chrome"), Some("arc")).await.unwrap();
        assert!(core.extension_status().connected);
        assert_eq!(core.extension_status().browser.as_deref(), Some("arc"));
        c.hello(Some("0.2.0"), Some("chrome"), None).await.unwrap();
        assert_eq!(core.extension_status().browser.as_deref(), Some("chrome"));
        c.post("/v1/hello", &json!({"version": "0.2.0", "browser": "edge", "detected": ""})).await.unwrap();
        assert_eq!(core.extension_status().browser.as_deref(), Some("edge"));
        let r = c.check(&crate::native_host::CheckReq { urls: vec!["https://nope.test/a.mp4".into()], page_url: None }).await.unwrap();
        assert_eq!(r["found"], false);
        assert_eq!(c.task_cancel(42).await.unwrap()["ok"], false);
        assert!(c.task_copy(42).await.is_err());
        assert!(c.job_action(42, JobAction::Cancel).await.is_err());
    }

    #[test]
    fn errors_keep_their_code() {
        use crate::errors::{failure_in, ErrorCode, Failure};
        // 桌面端回的失败：MCP 进程、插件宿主还原以后错误码还在
        let f = Failure::new(ErrorCode::Blocked, "连接被你的网络拦截了，检查代理设置", "ERROR: [vimeo] 1084537: Unable to download webpage: certificate verify failed");
        let e = api_error(&json!({"error": "DOWNLOAD_FAILED: 连接被你的网络拦截了，检查代理设置", "fail": f.to_json()}));
        assert_eq!(failure_in(&e).map(|x| x.code), Some(ErrorCode::Blocked));
        let j = crate::agent::error_json(&e);
        assert_eq!((j["error"].as_str(), j["code"].as_str(), j["action"].as_str()), (Some("DOWNLOAD_FAILED"), Some("blocked"), Some("retry")));
        assert!(j["next"].as_str().is_some_and(|n| n.contains("proxy")));
        // 没有大写代码：还原成 Failure 本身
        let e = api_error(&json!({"error": "微信视频号暂不支持", "fail": crate::errors::wechat_channels("https://weixin.qq.com/sph/x").to_json()}));
        assert_eq!(e.to_string(), "微信视频号暂不支持");
        assert_eq!(failure_in(&e).map(|x| x.code), Some(ErrorCode::Unsupported));
        // 普通错误照旧
        let e = api_error(&json!({"error": "没有这个任务"}));
        assert!(failure_in(&e).is_none() && e.to_string() == "没有这个任务");
        // 固定代码：转发以后还在
        let e = api_error(&json!({"error": "只有正在录制的直播才能「停止并保存」", "code": "NOT_RECORDING"}));
        assert_eq!((crate::errors::code_in(&e), e.to_string().as_str()), (Some("NOT_RECORDING"), "只有正在录制的直播才能「停止并保存」"));
    }
}
