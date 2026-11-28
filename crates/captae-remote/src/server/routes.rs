//! 白名单接口。这里没列的路径和方法一律 403 `NOT_ALLOWED`（路由的 fallback 和 method_not_allowed_fallback）。
//! 请求体自己解析（`deny_unknown_fields`）：带了保存位置、yt-dlp / ffmpeg 参数这类字段的请求整个拒掉，不悄悄丢掉那几项。
//!
//! 不用登录：`GET /v1/info`、`POST /v1/pair`、`POST /v1/pair/start`、`POST /v1/pair/finish`
//! 要带 `Authorization: Bearer <设备令牌>`：其余全部。

use super::{now_ms, Remote, Session, Shared};
use crate::proto::{self, code, AddJob, LibQuery, PairReq, SpakeFinish, SpakeStart, SpakeStarted};
use axum::body::{Body, Bytes};
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Extension, Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use captae_core::errors::{code_in, failure_in};
use captae_core::model::{AddRequest, Event, Job, JobAction};
use captae_core::tr;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

type St = State<Remote>;

/// 已经验过令牌的手机（中间件放进请求里）
#[derive(Clone)]
struct Phone {
    id: String,
    name: String,
}

pub fn router(r: Remote) -> Router {
    let authed = Router::new()
        .route("/v1/me", get(me))
        .route("/v1/jobs", get(jobs).post(add))
        .route("/v1/jobs/{id}/action", post(action))
        .route("/v1/jobs/{id}/variant", post(variant))
        .route("/v1/jobs/{id}/entries", post(entries))
        .route("/v1/probe", post(probe))
        .route("/v1/library", get(library))
        .route("/v1/library/{id}", get(asset))
        .route("/v1/library/{id}/thumb", get(thumb))
        .route("/v1/library/{id}/file", get(file))
        .route("/v1/events", get(events))
        .route_layer(middleware::from_fn_with_state(r.clone(), auth));
    Router::new()
        .route("/v1/info", get(info))
        .route("/v1/pair", post(pair))
        .route("/v1/pair/start", post(pair_start))
        .route("/v1/pair/finish", post(pair_finish))
        .merge(authed)
        .fallback(not_allowed)
        .method_not_allowed_fallback(not_allowed)
        .with_state(r)
}

// ------------------------------------------------------------------ 回应

fn reply(status: StatusCode, code: &str, msg: String) -> Response {
    (status, Json(json!({"error": msg, "code": code}))).into_response()
}

async fn not_allowed() -> Response {
    reply(StatusCode::FORBIDDEN, code::NOT_ALLOWED, tr!("手机上不能做这件事"))
}

fn bad(msg: String) -> Response {
    reply(StatusCode::BAD_REQUEST, code::BAD_REQUEST, msg)
}

fn not_found() -> Response {
    reply(StatusCode::NOT_FOUND, code::NOT_FOUND, tr!("找不到了，可能已经在电脑上删掉"))
}

/// 核心库的错误 → 回应。分类过的下载失败带上 `fail`（和桌面端同一套错误码），有固定代码的带上代码
fn fail(e: anyhow::Error) -> Response {
    let msg = e.to_string();
    if let Some(f) = failure_in(&e) {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": msg, "code": code::FAILED, "fail": f.to_json()}))).into_response();
    }
    let c = code_in(&e).unwrap_or(code::FAILED).to_string();
    let st = match c.as_str() {
        code::NOT_FOUND => StatusCode::NOT_FOUND,
        code::PAIR_EXPIRED | code::PAIR_VOID => StatusCode::GONE,
        code::PAIR_CLOSED => StatusCode::CONFLICT,
        _ => StatusCode::BAD_REQUEST,
    };
    reply(st, &c, msg)
}

/// 请求体 → 结构。多了字段、类型不对都拒掉
fn parse<T: DeserializeOwned>(b: &Bytes) -> Result<T, Response> {
    serde_json::from_slice(b).map_err(|e| {
        tracing::debug!("手机发来的请求不对：{e}");
        bad(tr!("请求的格式不对"))
    })
}

fn path_id(p: Result<Path<u64>, axum::extract::rejection::PathRejection>) -> Result<u64, Response> {
    p.map(|x| x.0).map_err(|_| not_found())
}

/// 任务给手机看的样子：去掉电脑上的文件路径、录直播的中间目录、本机的封面文件路径
pub(crate) fn job_json(j: &Job) -> Value {
    let mut v = serde_json::to_value(j).unwrap_or_default();
    if let Some(o) = v.as_object_mut() {
        o.remove("path");
        if let Some(r) = o.get_mut("rec").and_then(Value::as_object_mut) {
            r.remove("raw");
            r.remove("out");
        }
        if o.get("thumb").and_then(Value::as_str).is_some_and(|t| !t.starts_with("http://") && !t.starts_with("https://")) {
            o.remove("thumb");
        }
    }
    v
}

/// 素材给手机看的样子：没有电脑上的路径，封面只说有没有（用 `/v1/library/{id}/thumb` 拿）
fn asset_json(a: &captae_core::library::Asset) -> Value {
    let mut v = serde_json::to_value(a).unwrap_or_default();
    if let Some(o) = v.as_object_mut() {
        o.remove("path");
        let has = a.thumb.as_ref().is_some_and(|p| p.exists());
        o.remove("thumb");
        o.insert("hasThumb".into(), json!(has));
    }
    v
}

// ------------------------------------------------------------------ 登录

async fn auth(State(r): St, mut req: Request, next: Next) -> Response {
    let token = req.headers().get(header::AUTHORIZATION).and_then(|v| v.to_str().ok()).and_then(|v| v.strip_prefix("Bearer ")).map(str::trim).map(str::to_string);
    match token.and_then(|t| r.0.device_by_token(&t)) {
        Some(d) => {
            req.extensions_mut().insert(Phone { id: d.id, name: d.name });
            next.run(req).await
        }
        None => reply(StatusCode::UNAUTHORIZED, code::UNAUTHORIZED, tr!("这台手机还没配对，或者已经在电脑上撤销了")),
    }
}

async fn info(State(r): St) -> Json<Value> {
    let s = &r.0;
    Json(json!({"app": "captae", "v": proto::VERSION, "version": env!("CARGO_PKG_VERSION"), "name": s.opts.name}))
}

async fn me(State(r): St, Extension(p): Extension<Phone>) -> Json<Value> {
    Json(json!({"device": p.id, "name": p.name, "computer": r.0.opts.name, "version": env!("CARGO_PKG_VERSION")}))
}

// ------------------------------------------------------------------ 配对

/// 现在有效的一轮配对码（没有、过期了就报错）
fn live_pairing(s: &Shared) -> Result<(), Response> {
    let p = s.pairing.lock().unwrap();
    match p.as_ref() {
        None => Err(reply(StatusCode::CONFLICT, code::PAIR_CLOSED, tr!("电脑上没有在显示配对码，在电脑上打开 Captae › 设置 › 手机"))),
        Some(x) if x.expires <= now_ms() => Err(reply(StatusCode::GONE, code::PAIR_EXPIRED, tr!("配对码过期了，在电脑上换一个新的"))),
        Some(_) => Ok(()),
    }
}

/// 扫二维码配对：一次性码对了就发令牌
async fn pair(State(r): St, body: Bytes) -> Response {
    let req: PairReq = match parse(&body) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let s = &r.0;
    if let Err(e) = live_pairing(s) {
        return e;
    }
    {
        let mut p = s.pairing.lock().unwrap();
        let x = p.as_mut().unwrap();
        if !proto::same(&x.qr, req.code.trim()) {
            x.qr_wrong += 1;
            // 128 位的码猜不中；有人乱试 20 次，整轮作废，电脑上会换新的
            if x.qr_wrong >= 20 {
                *p = None;
                drop(p);
                s.changed();
            }
            return reply(StatusCode::FORBIDDEN, code::PAIR_WRONG, tr!("这个二维码已经换过了，扫电脑上现在显示的"));
        }
    }
    match s.add_device(&req.phone) {
        Ok(x) => Json(x).into_response(),
        Err(e) => fail(e),
    }
}

/// 6 位数配对第一步：收手机的 SPAKE2 消息，回电脑的消息和确认码。每次开始算试一次（一次只能验证一个猜测）
async fn pair_start(State(r): St, body: Bytes) -> Response {
    use spake2::{Ed25519Group, Identity, Password, Spake2};
    let req: SpakeStart = match parse(&body) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let Some(msg_phone) = proto::unb64(&req.msg) else { return bad(tr!("请求的格式不对")) };
    let s = &r.0;
    if let Err(e) = live_pairing(s) {
        return e;
    }
    let (gen, digits) = {
        let mut p = s.pairing.lock().unwrap();
        let x = p.as_mut().unwrap();
        if x.void || x.tries >= proto::PAIR_MAX_TRIES {
            x.void = true;
            return reply(StatusCode::GONE, code::PAIR_VOID, tr!("配对码错了 5 次，已经作废，在电脑上换一个新的"));
        }
        x.tries += 1;
        let gen = x.gen;
        let digits = x.digits.clone();
        drop(p);
        s.changed();
        (gen, digits)
    };
    let fp = match super::tls::fingerprint_of(&s.dir) {
        Ok(f) => f,
        Err(e) => return fail(e),
    };
    let (st, msg_desktop) = Spake2::<Ed25519Group>::start_b(&Password::new(digits.as_bytes()), &Identity::new(proto::SPAKE_ID_PHONE), &Identity::new(proto::SPAKE_ID_DESKTOP));
    let key = match st.finish(&msg_phone) {
        Ok(k) => k,
        Err(_) => return bad(tr!("请求的格式不对")),
    };
    let confirm = proto::spake_confirm(&key, b"desktop", &fp, &msg_phone, &msg_desktop);
    let session = proto::random_b64(16);
    {
        let mut ss = s.sessions.lock().unwrap();
        let now = now_ms();
        ss.retain(|_, v| v.until > now);
        ss.insert(session.clone(), Session { gen, key, msg_phone, msg_desktop: msg_desktop.clone(), until: now + 60_000 });
    }
    Json(SpakeStarted { session, msg: proto::b64(&msg_desktop), confirm: proto::b64(&confirm), name: s.opts.name.clone() }).into_response()
}

/// 6 位数配对第二步：手机的确认码对了（手机知道 6 位数、看到的是这张证书）就发令牌
async fn pair_finish(State(r): St, body: Bytes) -> Response {
    let req: SpakeFinish = match parse(&body) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let s = &r.0;
    let Some(ss) = s.sessions.lock().unwrap().remove(&req.session) else {
        return reply(StatusCode::GONE, code::PAIR_EXPIRED, tr!("配对码过期了，在电脑上换一个新的"));
    };
    if ss.until <= now_ms() {
        return reply(StatusCode::GONE, code::PAIR_EXPIRED, tr!("配对码过期了，在电脑上换一个新的"));
    }
    if let Err(e) = live_pairing(s) {
        return e;
    }
    let fp = match super::tls::fingerprint_of(&s.dir) {
        Ok(f) => f,
        Err(e) => return fail(e),
    };
    let ok = proto::unb64(&req.confirm).is_some_and(|c| proto::spake_check(&ss.key, b"phone", &fp, &ss.msg_phone, &ss.msg_desktop, &c));
    {
        let mut p = s.pairing.lock().unwrap();
        let x = p.as_mut().unwrap();
        if x.gen != ss.gen {
            return reply(StatusCode::GONE, code::PAIR_EXPIRED, tr!("配对码过期了，在电脑上换一个新的"));
        }
        if !ok {
            let left = proto::PAIR_MAX_TRIES.saturating_sub(x.tries);
            if left == 0 {
                x.void = true;
                drop(p);
                s.changed();
                return reply(StatusCode::GONE, code::PAIR_VOID, tr!("配对码错了 5 次，已经作废，在电脑上换一个新的"));
            }
            return (StatusCode::FORBIDDEN, Json(json!({"error": tr!("配对码不对，再看一眼电脑上的 6 位数"), "code": code::PAIR_WRONG, "left": left}))).into_response();
        }
    }
    match s.add_device(&req.phone) {
        Ok(x) => Json(x).into_response(),
        Err(e) => fail(e),
    }
}

// ------------------------------------------------------------------ 任务

async fn jobs(State(r): St) -> Json<Value> {
    Json(Value::Array(r.0.core.jobs().iter().map(job_json).collect()))
}

/// 加任务：只认链接、清晰度、格式、只要声音、预设名。来源记成 `remote:<手机名>`，桌面端写「来自 iPhone」
async fn add(State(r): St, Extension(p): Extension<Phone>, body: Bytes) -> Response {
    let a: AddJob = match parse(&body) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let url = a.url.trim();
    let ok = url.len() <= 4096 && url::Url::parse(url).is_ok_and(|u| matches!(u.scheme(), "http" | "https"));
    if !ok {
        return bad(tr!("链接好像不完整，检查一下是不是少复制了开头的 https://"));
    }
    let req = AddRequest {
        url: url.to_string(),
        quality: a.quality,
        format: a.format,
        audio_only: a.audio_only,
        preset: a.preset,
        origin: Some(format!("remote:{}", p.name)),
        ..Default::default()
    };
    match r.0.core.add(req) {
        Ok(j) => Json(job_json(&j)).into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActBody {
    action: JobAction,
}

async fn action(State(r): St, id: Result<Path<u64>, axum::extract::rejection::PathRejection>, body: Bytes) -> Response {
    let id = match path_id(id) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let b: ActBody = match parse(&body) {
        Ok(x) => x,
        Err(e) => return e,
    };
    if r.0.core.job(id).is_none() {
        return not_found();
    }
    match r.0.core.act(id, b.action) {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VariantBody {
    index: usize,
}

/// M3U8 选清晰度（倒计时里换一档）。选好马上开始用 action `pickgo`
async fn variant(State(r): St, id: Result<Path<u64>, axum::extract::rejection::PathRejection>, body: Bytes) -> Response {
    let id = match path_id(id) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let b: VariantBody = match parse(&body) {
        Ok(x) => x,
        Err(e) => return e,
    };
    if r.0.core.job(id).is_none() {
        return not_found();
    }
    match r.0.core.set_variant(id, b.index) {
        Ok(()) => Json(json!({"ok": true})).into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntriesBody {
    ids: Vec<String>,
}

/// 播放列表、多视频帖子选条目：每个条目一个新任务（`["all"]` = 全部）
async fn entries(State(r): St, id: Result<Path<u64>, axum::extract::rejection::PathRejection>, body: Bytes) -> Response {
    let id = match path_id(id) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let b: EntriesBody = match parse(&body) {
        Ok(x) => x,
        Err(e) => return e,
    };
    if r.0.core.job(id).is_none() {
        return not_found();
    }
    match r.0.core.choose(id, &b.ids) {
        Ok(js) => Json(Value::Array(js.iter().map(job_json).collect())).into_response(),
        Err(e) => fail(e),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProbeBody {
    url: String,
}

/// 读取视频信息（标题、时长、封面、各清晰度大小）。不带任何登录状态
async fn probe(State(r): St, body: Bytes) -> Response {
    let b: ProbeBody = match parse(&body) {
        Ok(x) => x,
        Err(e) => return e,
    };
    let url = b.url.trim().to_string();
    if !url::Url::parse(&url).is_ok_and(|u| matches!(u.scheme(), "http" | "https")) {
        return bad(tr!("链接好像不完整，检查一下是不是少复制了开头的 https://"));
    }
    let core = &r.0.core;
    let (tools, http) = (core.toolbox(), core.http());
    match captae_core::probe::probe(&tools, &http, &url, &Default::default()).await {
        Ok(p) => Json(p).into_response(),
        Err(e) => fail(e),
    }
}

// ------------------------------------------------------------------ 素材库

async fn library(State(r): St, q: Result<Query<LibQuery>, axum::extract::rejection::QueryRejection>) -> Response {
    let Ok(Query(q)) = q else { return bad(tr!("请求的格式不对")) };
    let query = captae_core::library::Query {
        q: q.q,
        kind: q.kind,
        site: q.site,
        format: q.format,
        days: q.days,
        since: None,
        tags: q.tag.into_iter().collect(),
        // 按分类筛手机端还没有（协议里没有这个参数），以后手机端做分类时再加
        cat: None,
        category: None,
        limit: Some(q.limit.unwrap_or(60).clamp(1, 200)),
        offset: q.offset,
    };
    match r.0.core.lib.query(&query) {
        Ok((items, total)) => Json(json!({"items": items.iter().map(asset_json).collect::<Vec<_>>(), "total": total})).into_response(),
        Err(e) => fail(e),
    }
}

fn get_asset(r: &Remote, id: Result<Path<u64>, axum::extract::rejection::PathRejection>) -> Result<captae_core::library::Asset, Response> {
    let id = path_id(id)?;
    match r.0.core.lib.get(id as i64) {
        Ok(Some(a)) => Ok(a),
        Ok(None) => Err(not_found()),
        Err(e) => Err(fail(e)),
    }
}

async fn asset(State(r): St, id: Result<Path<u64>, axum::extract::rejection::PathRejection>) -> Response {
    match get_asset(&r, id) {
        Ok(a) => Json(asset_json(&a)).into_response(),
        Err(e) => e,
    }
}

async fn thumb(State(r): St, id: Result<Path<u64>, axum::extract::rejection::PathRejection>, h: HeaderMap) -> Response {
    let a = match get_asset(&r, id) {
        Ok(a) => a,
        Err(e) => return e,
    };
    match a.thumb {
        Some(p) if p.is_file() => send_file(&p, &h, false).await,
        _ => not_found(),
    }
}

/// 下载素材文件。支持 `Range: bytes=a-b`（断点续传）和 `If-Range`
async fn file(State(r): St, id: Result<Path<u64>, axum::extract::rejection::PathRejection>, h: HeaderMap) -> Response {
    let a = match get_asset(&r, id) {
        Ok(a) => a,
        Err(e) => return e,
    };
    if !a.path.is_file() {
        return not_found();
    }
    send_file(&a.path, &h, true).await
}

fn mime_of(p: &std::path::Path) -> &'static str {
    match p.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("mp4" | "m4v") => "video/mp4",
        Some("mov") => "video/quicktime",
        Some("webm") => "video/webm",
        Some("mkv") => "video/x-matroska",
        Some("mp3") => "audio/mpeg",
        Some("m4a" | "aac") => "audio/mp4",
        Some("flac") => "audio/flac",
        Some("wav") => "audio/wav",
        Some("ogg" | "opus") => "audio/ogg",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("srt") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

/// `Range: bytes=a-b` / `bytes=a-` / `bytes=-n` → [起, 止]（含）。只认一段；多段、看不懂的当成没有 Range
pub(crate) fn parse_range(v: &str, len: u64) -> Option<Result<(u64, u64), ()>> {
    let spec = v.trim().strip_prefix("bytes=")?;
    if spec.contains(',') {
        return None;
    }
    let (a, b) = spec.split_once('-')?;
    let (a, b) = (a.trim(), b.trim());
    let r = if a.is_empty() {
        let n: u64 = b.parse().ok()?;
        if n == 0 {
            return Some(Err(()));
        }
        (len.saturating_sub(n), len.checked_sub(1)?)
    } else {
        let s: u64 = a.parse().ok()?;
        let e: u64 = if b.is_empty() { len.saturating_sub(1) } else { b.parse::<u64>().ok()?.min(len.saturating_sub(1)) };
        (s, e)
    };
    if len == 0 || r.0 >= len || r.0 > r.1 {
        return Some(Err(()));
    }
    Some(Ok(r))
}

/// 文件的 ETag：大小 + 修改时间（文件变了，续传就从头来）
fn etag_of(m: &std::fs::Metadata) -> String {
    let t = m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_millis()).unwrap_or(0);
    format!("\"{:x}-{:x}\"", m.len(), t)
}

/// 文件名放进 Content-Disposition（RFC 5987：UTF-8 百分号编码）
fn disposition(p: &std::path::Path) -> String {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let enc: String = name
        .bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"-._~".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect();
    format!("attachment; filename*=UTF-8''{enc}")
}

async fn send_file(p: &std::path::Path, h: &HeaderMap, attach: bool) -> Response {
    let Ok(mut f) = tokio::fs::File::open(p).await else { return not_found() };
    let Ok(meta) = f.metadata().await else { return not_found() };
    let len = meta.len();
    let etag = etag_of(&meta);
    // If-Range 对不上（文件换过了）：整个重新给
    let if_range_ok = h.get(header::IF_RANGE).and_then(|v| v.to_str().ok()).is_none_or(|v| v == etag);
    let range = h.get(header::RANGE).and_then(|v| v.to_str().ok()).filter(|_| if_range_ok).and_then(|v| parse_range(v, len));
    let mut res = Response::builder()
        .header(header::CONTENT_TYPE, mime_of(p))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::ETAG, etag);
    if attach {
        if let Ok(v) = HeaderValue::from_str(&disposition(p)) {
            res = res.header(header::CONTENT_DISPOSITION, v);
        }
    }
    let (status, start, n) = match range {
        None => (StatusCode::OK, 0, len),
        Some(Err(())) => {
            return (StatusCode::RANGE_NOT_SATISFIABLE, [(header::CONTENT_RANGE, format!("bytes */{len}"))], Json(json!({"error": tr!("请求的范围超出了文件大小"), "code": code::RANGE}))).into_response();
        }
        Some(Ok((a, b))) => {
            res = res.header(header::CONTENT_RANGE, format!("bytes {a}-{b}/{len}"));
            (StatusCode::PARTIAL_CONTENT, a, b - a + 1)
        }
    };
    if start > 0 && f.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return not_found();
    }
    let body = Body::from_stream(tokio_util::io::ReaderStream::with_capacity(f.take(n), 256 * 1024));
    res.status(status).header(header::CONTENT_LENGTH, n).body(body).unwrap_or_else(|_| not_found())
}

// ------------------------------------------------------------------ 实时事件

/// 推给手机的事件：只有任务（`job`、`job_removed`、`finished`）和素材库（`library`），设置、AI 调用、剪贴板这些不推
pub(crate) fn event_json(e: &Event) -> Option<Value> {
    match e {
        Event::Job { job } => Some(json!({"type": "job", "job": job_json(job)})),
        Event::JobRemoved { id } => Some(json!({"type": "job_removed", "id": id})),
        Event::Finished { id, title, asset_id, .. } => Some(json!({"type": "finished", "id": id, "title": title, "asset_id": asset_id})),
        Event::Library => Some(json!({"type": "library"})),
        _ => None,
    }
}

async fn events(State(r): St, Extension(p): Extension<Phone>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |sock| ws_loop(sock, r.0.clone(), p))
}

fn text(v: &Value) -> Message {
    Message::Text(v.to_string().into())
}

fn close(code: u16, why: &str) -> Message {
    Message::Close(Some(CloseFrame { code, reason: why.to_string().into() }))
}

/// 一条实时事件连接：先发一份任务快照（`{"type":"jobs","jobs":[…]}`，断线重连后据此对齐），再一条条推。
/// 推得太慢落下了也发快照。这台手机被撤销、服务关掉时用 4001 / 4002 关掉
async fn ws_loop(mut ws: WebSocket, s: Arc<Shared>, p: Phone) {
    let Some(cancel) = s.cancel_token() else { return };
    let mut rx = s.core.subscribe();
    let mut rev = s.revoked.subscribe();
    s.set_online(&p.id, 1);
    let snap = |s: &Shared| json!({"type": "jobs", "jobs": s.core.jobs().iter().map(job_json).collect::<Vec<_>>()});
    let mut ok = ws.send(text(&snap(&s))).await.is_ok();
    let mut ping = tokio::time::interval(Duration::from_secs(20));
    ping.tick().await;
    while ok {
        tokio::select! {
            _ = cancel.cancelled() => {
                let _ = ws.send(close(proto::CLOSE_STOPPED, "stopped")).await;
                break;
            }
            c = rev.changed() => {
                if c.is_err() || !s.has_device(&p.id) {
                    let _ = ws.send(close(proto::CLOSE_REVOKED, "revoked")).await;
                    break;
                }
            }
            ev = rx.recv() => match ev {
                Ok(e) => {
                    if let Some(v) = event_json(&e) {
                        ok = ws.send(text(&v)).await.is_ok();
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => ok = ws.send(text(&snap(&s))).await.is_ok(),
                Err(_) => break,
            },
            m = ws.recv() => match m {
                None | Some(Err(_)) | Some(Ok(Message::Close(_))) => break,
                _ => {}
            },
            _ = ping.tick() => ok = ws.send(Message::Ping(Bytes::new())).await.is_ok(),
        }
    }
    s.set_online(&p.id, -1);
}
