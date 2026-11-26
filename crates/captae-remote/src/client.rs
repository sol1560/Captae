//! 手机端的 Rust 客户端（feature `client`），之后由 crates/captae-mobile 包给 Swift / Kotlin。接入说明见 CLIENT.md。
//!
//! - 配对：扫二维码（[`pair_qr`]，按二维码里的指纹钉死证书）或者 6 位数（[`pair_code`]，SPAKE2，看到的证书指纹绑进确认码）。
//!   拿到的 [`Credentials`] 里有设备令牌，手机存进 iOS 钥匙串 / Android Keystore。
//! - 之后每个连接都按指纹钉死证书（[`Client`]），不看系统根证书、不看主机名。
//! - 实时事件（[`Client::subscribe`]）：WebSocket，断了自动重连（1、2、4… 最多 30 秒），每次连上先收一份任务快照；被撤销了就停。
//! - 错误：服务端的回应变成 [`RemoteError`]（`code` 见 `proto::code`）；连不上、证书对不上另有代码（[`code`]）。
//!   给人看的文字由 App 按代码自己写（这里的 message 只给开发者看，不跟界面语言走）。

use crate::proto::{self, AddJob, LibQuery, PairLink, Paired, PhoneHello, SpakeFinish, SpakeStart, SpakeStarted};
use anyhow::{anyhow, Context, Result};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{DigitallySignedStruct, SignatureScheme};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;

/// 客户端自己的错误代码（服务端的见 `proto::code`）
pub mod code {
    /// 所有地址都连不上（不在同一个 Wi-Fi、电脑没开 Captae、没打开「让手机连接」）
    pub const UNREACHABLE: &str = "UNREACHABLE";
    /// 连上的机器证书对不上（不是配对过的那台电脑，或者电脑重新生成了证书）
    pub const CERT_MISMATCH: &str = "CERT_MISMATCH";
    /// 6 位数配对：电脑的确认码对不上（6 位数输错了，或者中间有人冒充电脑）
    pub const PAIR_MISMATCH: &str = "PAIR_MISMATCH";
    /// 二维码不是 Captae 的
    pub const BAD_LINK: &str = "BAD_LINK";
}

/// 服务端回的错误，或者客户端认出来的几种情况
#[derive(Debug, Clone)]
pub struct RemoteError {
    /// HTTP 状态码；客户端自己认出来的是 0
    pub status: u16,
    pub code: String,
    pub message: String,
    /// 下载失败的详情（和桌面端同一套错误码：need_login、network……），`code` 是 FAILED 时可能有
    pub fail: Option<Value>,
    /// 配对码错了还能试几次
    pub left: Option<u32>,
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}（{}）", self.message, self.code)
    }
}

impl std::error::Error for RemoteError {}

fn local_err(code: &str, msg: &str) -> anyhow::Error {
    anyhow::Error::new(RemoteError { status: 0, code: code.into(), message: msg.into(), fail: None, left: None })
}

/// 错误链里的代码（服务端的或者客户端的）
pub fn error_code(e: &anyhow::Error) -> Option<&str> {
    e.chain().find_map(|c| c.downcast_ref::<RemoteError>()).map(|r| r.code.as_str())
}

pub fn remote_error(e: &anyhow::Error) -> Option<&RemoteError> {
    e.chain().find_map(|c| c.downcast_ref::<RemoteError>())
}

// ------------------------------------------------------------------ 钉死证书

/// 证书校验：只认指纹（`want` 为 None 时什么证书都收，只记下看到的指纹，6 位数配对的第一步用）。
/// 握手签名照常校验：对方确实有这张证书的私钥
#[derive(Debug)]
struct Pin {
    want: Option<String>,
    seen: Arc<Mutex<Option<String>>>,
    mismatch: Arc<AtomicBool>,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for Pin {
    fn verify_server_cert(&self, end: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
        let got = proto::fingerprint(end.as_ref());
        *self.seen.lock().unwrap() = Some(got.clone());
        match &self.want {
            Some(w) if !proto::same(w, &got) => {
                self.mismatch.store(true, Ordering::SeqCst);
                Err(rustls::Error::General("certificate fingerprint mismatch".into()))
            }
            _ => Ok(ServerCertVerified::assertion()),
        }
    }

    fn verify_tls12_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(&self, message: &[u8], cert: &CertificateDer<'_>, dss: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

pub(crate) struct Tls {
    pub(crate) config: Arc<rustls::ClientConfig>,
    seen: Arc<Mutex<Option<String>>>,
    mismatch: Arc<AtomicBool>,
}

pub(crate) fn tls(want: Option<&str>) -> Tls {
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let (seen, mismatch) = (Arc::new(Mutex::new(None)), Arc::new(AtomicBool::new(false)));
    let pin = Pin { want: want.map(str::to_string), seen: seen.clone(), mismatch: mismatch.clone(), provider: provider.clone() };
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("ring 支持默认的 TLS 版本")
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(pin))
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Tls { config: Arc::new(config), seen, mismatch }
}

pub(crate) fn http_client(t: &Tls, total: Option<Duration>) -> Result<reqwest::Client> {
    let mut b = reqwest::Client::builder()
        .use_preconfigured_tls((*t.config).clone())
        // 局域网地址，不走手机上设的代理
        .no_proxy()
        .connect_timeout(Duration::from_secs(4))
        .read_timeout(Duration::from_secs(30))
        .user_agent(concat!("Captae-Remote/", env!("CARGO_PKG_VERSION")));
    if let Some(t) = total {
        b = b.timeout(t);
    }
    Ok(b.build()?)
}

/// 回应 → JSON；不是 2xx 的变成 RemoteError
async fn json_of(r: reqwest::Response) -> Result<Value> {
    let st = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if st.is_success() {
        return Ok(v);
    }
    Err(anyhow::Error::new(RemoteError {
        status: st.as_u16(),
        code: v["code"].as_str().unwrap_or("FAILED").to_string(),
        message: v["error"].as_str().unwrap_or("request failed").to_string(),
        fail: v.get("fail").cloned().filter(|f| !f.is_null()),
        left: v["left"].as_u64().map(|n| n as u32),
    }))
}

/// 发请求出错时：证书对不上就说证书对不上，其余是连不上
fn send_err(t: &Tls, e: reqwest::Error) -> anyhow::Error {
    if t.mismatch.load(Ordering::SeqCst) {
        return local_err(code::CERT_MISMATCH, "the computer's certificate does not match the paired one");
    }
    if e.is_connect() || e.is_timeout() {
        return local_err(code::UNREACHABLE, &format!("cannot reach the computer: {e}"));
    }
    anyhow::Error::new(e)
}

// ------------------------------------------------------------------ 配对

/// 手机记住的一台电脑：配对时拿到，存进钥匙串 / Keystore（令牌是秘密，其余可以放普通存储）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    /// 电脑名
    pub name: String,
    /// 证书指纹：这台电脑的身份（地址会变，指纹不变）
    pub fp: String,
    /// 地址，按优先顺序（上次连上的放第一个）
    pub hosts: Vec<String>,
    pub port: u16,
    /// 设备令牌（256 位）
    pub token: String,
    /// 电脑给这台手机编的号
    pub device: String,
}

/// 扫二维码配对：按二维码里的指纹连，用一次性码换令牌。地址挨个试
pub async fn pair_qr(link: &PairLink, phone: &PhoneHello) -> Result<Credentials> {
    let t = tls(Some(&link.fp));
    let http = http_client(&t, Some(Duration::from_secs(15)))?;
    let mut last = None;
    for h in &link.hosts {
        let url = format!("{}/v1/pair", proto::base_url(h, link.port));
        let body = json!({"code": link.code, "name": phone.name, "platform": phone.platform});
        match http.post(&url).json(&body).send().await {
            Ok(r) => {
                let p: Paired = serde_json::from_value(json_of(r).await?)?;
                let mut hosts = vec![h.clone()];
                hosts.extend(link.hosts.iter().filter(|x| *x != h).cloned());
                return Ok(Credentials { name: p.name, fp: link.fp.clone(), hosts, port: link.port, token: p.token, device: p.device });
            }
            Err(e) => last = Some(send_err(&t, e)),
        }
        if t.mismatch.load(Ordering::SeqCst) {
            break;
        }
    }
    Err(last.unwrap_or_else(|| local_err(code::BAD_LINK, "no address in the link")))
}

/// 6 位数配对（Bonjour 发现了这台电脑、用户输了电脑上显示的 6 位数）。
/// 第一步什么证书都收、记下看到的指纹；电脑的确认码里绑着它自己的指纹，对上了才说明对面知道 6 位数、而且就是这张证书。
/// 第二步按这个指纹钉死再连，拿令牌（中间人换不了证书、也拿不到令牌）
pub async fn pair_code(host: &str, port: u16, digits: &str, phone: &PhoneHello) -> Result<Credentials> {
    use spake2::{Ed25519Group, Identity, Password, Spake2};
    let digits: String = digits.chars().filter(|c| c.is_ascii_digit()).collect();
    let base = proto::base_url(host, port);
    let (st, msg_phone) = Spake2::<Ed25519Group>::start_a(&Password::new(digits.as_bytes()), &Identity::new(proto::SPAKE_ID_PHONE), &Identity::new(proto::SPAKE_ID_DESKTOP));
    let t = tls(None);
    let http = http_client(&t, Some(Duration::from_secs(15)))?;
    let r = http.post(format!("{base}/v1/pair/start")).json(&SpakeStart { msg: proto::b64(&msg_phone) }).send().await.map_err(|e| send_err(&t, e))?;
    let started: SpakeStarted = serde_json::from_value(json_of(r).await?)?;
    let fp = t.seen.lock().unwrap().clone().ok_or_else(|| anyhow!("no certificate seen"))?;
    let msg_desktop = proto::unb64(&started.msg).context("bad server message")?;
    let key = st.finish(&msg_desktop).map_err(|_| local_err(code::PAIR_MISMATCH, "SPAKE2 failed"))?;
    let theirs = proto::unb64(&started.confirm).unwrap_or_default();
    if !proto::spake_check(&key, b"desktop", &fp, &msg_phone, &msg_desktop, &theirs) {
        return Err(local_err(code::PAIR_MISMATCH, "the code is wrong, or this is not the computer it claims to be"));
    }
    let mine = proto::spake_confirm(&key, b"phone", &fp, &msg_phone, &msg_desktop);
    // 第二步：按刚确认过的指纹钉死
    let t2 = tls(Some(&fp));
    let http2 = http_client(&t2, Some(Duration::from_secs(15)))?;
    let fin = SpakeFinish { session: started.session, confirm: proto::b64(&mine), phone: phone.clone() };
    let r = http2.post(format!("{base}/v1/pair/finish")).json(&fin).send().await.map_err(|e| send_err(&t2, e))?;
    let p: Paired = serde_json::from_value(json_of(r).await?)?;
    Ok(Credentials { name: p.name, fp, hosts: vec![host.to_string()], port, token: p.token, device: p.device })
}

// ------------------------------------------------------------------ 已配对以后

/// 连着一台配对过的电脑
pub struct Client {
    creds: Mutex<Credentials>,
    /// 现在用的地址
    host: Mutex<String>,
    tls: Tls,
    http: reqwest::Client,
    /// 下文件用：没有总时长限制（只有读超时）
    dl: reqwest::Client,
}

/// 实时事件
#[derive(Debug, Clone, PartialEq)]
pub enum RemoteEvent {
    /// 连上了（每次重连也发）
    Connected,
    /// 任务快照：连上时、推送落下了时。收到它就用它整个替换电脑上的任务列表
    Jobs(Vec<Value>),
    Job(Value),
    JobRemoved(u64),
    Finished { id: u64, title: String, asset_id: Option<i64> },
    Library,
    /// 断开了，等一会儿自动重连。`reason` 只给开发者看
    Disconnected { reason: String, retry_in: Duration },
    /// 这台手机在电脑上被撤销了：不再重连，App 应该删掉这台电脑的令牌
    Revoked,
}

impl Client {
    /// 建好客户端（不联网）
    pub fn new(creds: Credentials) -> Result<Client> {
        let t = tls(Some(&creds.fp));
        let http = http_client(&t, Some(Duration::from_secs(60)))?;
        let dl = http_client(&t, None)?;
        let host = creds.hosts.first().cloned().unwrap_or_default();
        Ok(Client { creds: Mutex::new(creds), host: Mutex::new(host), tls: t, http, dl })
    }

    /// 建好客户端并找一个连得上的地址（挨个试 `GET /v1/me`）。连上的地址排到第一个（`credentials()` 拿去存）
    pub async fn connect(creds: Credentials) -> Result<Client> {
        let c = Client::new(creds)?;
        c.reconnect().await?;
        Ok(c)
    }

    /// 重新找地址（换了 Wi-Fi、电脑换了地址、Bonjour 发现了新地址以后）
    pub async fn reconnect(&self) -> Result<Value> {
        let (hosts, port) = {
            let c = self.creds.lock().unwrap();
            (c.hosts.clone(), c.port)
        };
        let mut last = local_err(code::UNREACHABLE, "no address");
        for h in hosts {
            let r = self.http.get(format!("{}/v1/me", proto::base_url(&h, port))).timeout(Duration::from_secs(4)).bearer_auth(self.token()).send().await;
            match r {
                Ok(r) => {
                    // 401（被撤销）等服务端的回答直接交出去：地址是对的
                    let v = json_of(r).await;
                    *self.host.lock().unwrap() = h.clone();
                    let mut c = self.creds.lock().unwrap();
                    c.hosts.retain(|x| *x != h);
                    c.hosts.insert(0, h);
                    return v;
                }
                Err(e) => {
                    last = send_err(&self.tls, e);
                    if error_code(&last) == Some(code::CERT_MISMATCH) {
                        break;
                    }
                }
            }
        }
        Err(last)
    }

    /// 换地址（Bonjour 按指纹找到这台电脑的新地址）。新的排在前面，旧的留着
    pub fn add_hosts(&self, hosts: &[String], port: Option<u16>) {
        let mut c = self.creds.lock().unwrap();
        for h in hosts.iter().rev() {
            c.hosts.retain(|x| x != h);
            c.hosts.insert(0, h.clone());
        }
        if let Some(p) = port {
            c.port = p;
        }
    }

    /// 现在的配对信息（地址按最近连上的排了序）
    pub fn credentials(&self) -> Credentials {
        self.creds.lock().unwrap().clone()
    }

    fn token(&self) -> String {
        self.creds.lock().unwrap().token.clone()
    }

    fn base(&self) -> String {
        proto::base_url(&self.host.lock().unwrap(), self.creds.lock().unwrap().port)
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let r = self.http.get(format!("{}{path}", self.base())).bearer_auth(self.token()).send().await.map_err(|e| send_err(&self.tls, e))?;
        json_of(r).await
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let r = self.http.post(format!("{}{path}", self.base())).bearer_auth(self.token()).json(body).send().await.map_err(|e| send_err(&self.tls, e))?;
        json_of(r).await
    }

    /// `{device, name, computer, version}`
    pub async fn me(&self) -> Result<Value> {
        self.get("/v1/me").await
    }

    /// 电脑上的全部任务（字段和桌面端 `Job` 一样，见 apps/desktop/src/lib/types.ts；没有 path，本机封面去掉了）
    pub async fn jobs(&self) -> Result<Vec<Value>> {
        Ok(self.get("/v1/jobs").await?.as_array().cloned().unwrap_or_default())
    }

    /// 加一个任务，回新任务
    pub async fn add(&self, a: &AddJob) -> Result<Value> {
        self.post("/v1/jobs", &serde_json::to_value(a)?).await
    }

    /// 任务操作：toggle / pause / resume / now / up / down / cancel / remove / retry / segretry / pickgo / stop（和桌面端 `JobAction` 一样）
    pub async fn action(&self, id: u64, action: &str) -> Result<()> {
        self.post(&format!("/v1/jobs/{id}/action"), &json!({"action": action})).await.map(|_| ())
    }

    /// M3U8 选清晰度（`vars` 里的第几档）
    pub async fn set_variant(&self, id: u64, index: usize) -> Result<()> {
        self.post(&format!("/v1/jobs/{id}/variant"), &json!({"index": index})).await.map(|_| ())
    }

    /// 播放列表选条目（`entries[].id`，`["all"]` = 全部），回新建的任务
    pub async fn choose_entries(&self, id: u64, ids: &[String]) -> Result<Vec<Value>> {
        Ok(self.post(&format!("/v1/jobs/{id}/entries"), &json!({"ids": ids})).await?.as_array().cloned().unwrap_or_default())
    }

    /// 读取视频信息（标题、时长、封面、各清晰度大小），电脑上读
    pub async fn probe(&self, url: &str) -> Result<Value> {
        self.post("/v1/probe", &json!({"url": url})).await
    }

    /// 素材库：`{items: [...], total}`
    pub async fn library(&self, q: &LibQuery) -> Result<Value> {
        let mut u = url::Url::parse(&format!("{}/v1/library", self.base()))?;
        {
            let mut qp = u.query_pairs_mut();
            if let Value::Object(m) = serde_json::to_value(q)? {
                for (k, v) in m {
                    let s = match v {
                        Value::String(s) => s,
                        other => other.to_string(),
                    };
                    qp.append_pair(&k, &s);
                }
            }
        }
        let r = self.http.get(u).bearer_auth(self.token()).send().await.map_err(|e| send_err(&self.tls, e))?;
        json_of(r).await
    }

    pub async fn asset(&self, id: i64) -> Result<Value> {
        self.get(&format!("/v1/library/{id}")).await
    }

    /// 素材的封面（JPEG）
    pub async fn thumb(&self, id: i64) -> Result<Vec<u8>> {
        let r = self.http.get(format!("{}/v1/library/{id}/thumb", self.base())).bearer_auth(self.token()).send().await.map_err(|e| send_err(&self.tls, e))?;
        if !r.status().is_success() {
            return Err(json_of(r).await.unwrap_err());
        }
        Ok(r.bytes().await?.to_vec())
    }

    /// 取素材文件的一段（含两端），回这段数据和文件总大小
    pub async fn file_range(&self, id: i64, start: u64, end: u64) -> Result<(Vec<u8>, u64)> {
        let r = self
            .dl
            .get(format!("{}/v1/library/{id}/file", self.base()))
            .bearer_auth(self.token())
            .header("range", format!("bytes={start}-{end}"))
            .send()
            .await
            .map_err(|e| send_err(&self.tls, e))?;
        if r.status().as_u16() != 206 {
            return Err(json_of(r).await.err().unwrap_or_else(|| anyhow!("server ignored Range")));
        }
        let total = total_of(r.headers()).unwrap_or(0);
        Ok((r.bytes().await?.to_vec(), total))
    }

    /// 把素材文件下到 `dest`，断点续传：`dest` 已经有一部分就从那里接着下（`dest` 旁边的 `.etag` 记着文件的版本，
    /// 电脑上的文件换过了就从头下）。`progress(已下, 总共)`。回文件总大小
    pub async fn download_to(&self, id: i64, dest: &Path, mut progress: impl FnMut(u64, u64) + Send) -> Result<u64> {
        let tag_file = {
            let mut s = dest.as_os_str().to_owned();
            s.push(".etag");
            std::path::PathBuf::from(s)
        };
        let have = tokio::fs::metadata(dest).await.map(|m| m.len()).unwrap_or(0);
        let etag = tokio::fs::read_to_string(&tag_file).await.ok().filter(|t| !t.is_empty());
        let mut req = self.dl.get(format!("{}/v1/library/{id}/file", self.base())).bearer_auth(self.token());
        if have > 0 {
            if let Some(t) = &etag {
                req = req.header("range", format!("bytes={have}-")).header("if-range", t.trim());
            }
        }
        let mut r = req.send().await.map_err(|e| send_err(&self.tls, e))?;
        let st = r.status().as_u16();
        if st == 416 && have > 0 {
            // 已经下完了
            let _ = tokio::fs::remove_file(&tag_file).await;
            return Ok(have);
        }
        if st != 200 && st != 206 {
            return Err(json_of(r).await.unwrap_err());
        }
        if let Some(t) = r.headers().get("etag").and_then(|v| v.to_str().ok()) {
            tokio::fs::write(&tag_file, t).await?;
        }
        // 按位置写、不用追加：上一次下载被中途丢掉时，tokio 可能还有一笔写入在后台没落盘（暂停后马上继续），
        // 追加会把那笔写到新数据后面；按位置写的话同一个偏移量上的内容一样，先落后落都对。
        // 从头下时先删掉旧文件再建新的：还没落盘的旧写入落到已经删掉的那个文件上，不碰新文件
        let (mut f, mut got, total) = if st == 206 {
            use tokio::io::AsyncSeekExt;
            let mut f = tokio::fs::OpenOptions::new().write(true).open(dest).await?;
            f.seek(std::io::SeekFrom::Start(have)).await?;
            (f, have, total_of(r.headers()).unwrap_or(0))
        } else {
            let _ = tokio::fs::remove_file(dest).await;
            let f = tokio::fs::File::create(dest).await?;
            (f, 0, r.content_length().unwrap_or(0))
        };
        progress(got, total);
        while let Some(chunk) = r.chunk().await.map_err(|e| send_err(&self.tls, e))? {
            f.write_all(&chunk).await?;
            got += chunk.len() as u64;
            progress(got, total);
        }
        f.flush().await?;
        if total > 0 && got != total {
            return Err(local_err(code::UNREACHABLE, "connection closed before the file finished"));
        }
        let _ = tokio::fs::remove_file(&tag_file).await;
        Ok(got)
    }

    /// 订阅实时事件。断了自动重连，丢掉返回的 [`Subscription`] 就停
    pub fn subscribe(self: &Arc<Self>) -> Subscription {
        self.subscribe_every(Duration::from_secs(30))
    }

    /// 同 [`Client::subscribe`]，重连的等待时间最多 `max_wait`（1、2、4… 封顶）。
    /// 手机在前台时每 15 秒找一次电脑（work/mobile/PLAN.md 5.3「离线队列」），crates/captae-mobile 用它
    pub fn subscribe_every(self: &Arc<Self>, max_wait: Duration) -> Subscription {
        let (tx, rx) = mpsc::channel(256);
        let me = self.clone();
        let max_wait = max_wait.max(Duration::from_secs(1));
        let task = tokio::spawn(async move { me.event_loop(tx, max_wait).await });
        Subscription { rx, task }
    }

    async fn event_loop(self: Arc<Self>, tx: mpsc::Sender<RemoteEvent>, max_wait: Duration) {
        use futures_util::StreamExt;
        use tokio_tungstenite::tungstenite::Message;
        let mut wait = Duration::from_secs(1);
        loop {
            let reason = match self.ws_connect().await {
                Ok(mut ws) => {
                    wait = Duration::from_secs(1);
                    if tx.send(RemoteEvent::Connected).await.is_err() {
                        return;
                    }
                    loop {
                        // 电脑每 20 秒 ping 一次：60 秒什么都没收到就当断了
                        match tokio::time::timeout(Duration::from_secs(60), ws.next()).await {
                            Err(_) => break "no message for 60s".to_string(),
                            Ok(None) => break "closed".to_string(),
                            Ok(Some(Err(e))) => break e.to_string(),
                            Ok(Some(Ok(Message::Close(f)))) => {
                                let c = f.as_ref().map(|f| u16::from(f.code)).unwrap_or(0);
                                if c == proto::CLOSE_REVOKED {
                                    let _ = tx.send(RemoteEvent::Revoked).await;
                                    return;
                                }
                                break if c == proto::CLOSE_STOPPED { "the computer stopped accepting phones".into() } else { format!("closed ({c})") };
                            }
                            Ok(Some(Ok(Message::Text(t)))) => {
                                if let Some(ev) = parse_event(t.as_str()) {
                                    if tx.send(ev).await.is_err() {
                                        return;
                                    }
                                }
                            }
                            Ok(Some(Ok(_))) => {}
                        }
                    }
                }
                Err(e) => {
                    // 升级时就被拒（401）：被撤销了
                    if error_code(&e) == Some(proto::code::UNAUTHORIZED) {
                        let _ = tx.send(RemoteEvent::Revoked).await;
                        return;
                    }
                    // 连不上当前地址：换着地址再找
                    let _ = self.reconnect().await;
                    format!("{e:#}")
                }
            };
            if tx.send(RemoteEvent::Disconnected { reason, retry_in: wait }).await.is_err() {
                return;
            }
            tokio::time::sleep(wait).await;
            wait = (wait * 2).min(max_wait);
        }
    }

    async fn ws_connect(&self) -> Result<tokio_tungstenite::WebSocketStream<tokio_rustls::client::TlsStream<tokio::net::TcpStream>>> {
        use tokio_tungstenite::tungstenite::client::IntoClientRequest;
        let host = self.host.lock().unwrap().clone();
        let port = self.creds.lock().unwrap().port;
        let tcp = tokio::time::timeout(Duration::from_secs(4), tokio::net::TcpStream::connect((host.as_str(), port)))
            .await
            .map_err(|_| local_err(code::UNREACHABLE, "connect timeout"))?
            .map_err(|e| local_err(code::UNREACHABLE, &e.to_string()))?;
        let name = ServerName::try_from(host.clone()).map_err(|_| anyhow!("bad host {host}"))?;
        let tls = tokio_rustls::TlsConnector::from(self.tls.config.clone()).connect(name, tcp).await.map_err(|e| {
            if self.tls.mismatch.load(Ordering::SeqCst) {
                local_err(code::CERT_MISMATCH, "the computer's certificate does not match the paired one")
            } else {
                local_err(code::UNREACHABLE, &e.to_string())
            }
        })?;
        let mut req = format!("{}/v1/events", proto::base_url(&host, port).replacen("https://", "wss://", 1)).into_client_request()?;
        req.headers_mut().insert("authorization", format!("Bearer {}", self.token()).parse()?);
        match tokio_tungstenite::client_async(req, tls).await {
            Ok((ws, _)) => Ok(ws),
            Err(tokio_tungstenite::tungstenite::Error::Http(r)) => {
                let st = r.status().as_u16();
                let code = if st == 401 { proto::code::UNAUTHORIZED } else { "FAILED" };
                Err(anyhow::Error::new(RemoteError { status: st, code: code.into(), message: format!("websocket refused ({st})"), fail: None, left: None }))
            }
            Err(e) => Err(e.into()),
        }
    }
}

fn total_of(h: &reqwest::header::HeaderMap) -> Option<u64> {
    h.get("content-range")?.to_str().ok()?.rsplit('/').next()?.parse().ok()
}

/// 服务端推来的一条 → 事件
pub fn parse_event(s: &str) -> Option<RemoteEvent> {
    let v: Value = serde_json::from_str(s).ok()?;
    Some(match v["type"].as_str()? {
        "jobs" => RemoteEvent::Jobs(v["jobs"].as_array().cloned().unwrap_or_default()),
        "job" => RemoteEvent::Job(v["job"].clone()),
        "job_removed" => RemoteEvent::JobRemoved(v["id"].as_u64()?),
        "finished" => RemoteEvent::Finished { id: v["id"].as_u64()?, title: v["title"].as_str().unwrap_or_default().to_string(), asset_id: v["asset_id"].as_i64() },
        "library" => RemoteEvent::Library,
        _ => return None,
    })
}

/// 实时事件的订阅。丢掉它就断开、不再重连
pub struct Subscription {
    rx: mpsc::Receiver<RemoteEvent>,
    task: tokio::task::JoinHandle<()>,
}

impl Subscription {
    /// 下一条事件；被撤销以后返回 `Revoked`，再之后返回 None
    pub async fn recv(&mut self) -> Option<RemoteEvent> {
        self.rx.recv().await
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.task.abort();
    }
}
