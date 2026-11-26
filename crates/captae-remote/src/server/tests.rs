//! 远程控制的单元测试：服务只听 127.0.0.1、不广播 Bonjour，用本 crate 的客户端去连。
//! 覆盖：二维码配对和用一次作废、配对码过期、6 位数错 5 次作废、撤销后被拒、白名单外的请求全被拒、Range 下载、事件推送。

use super::*;
use crate::client::{self, error_code, pair_code, pair_qr, Client, Credentials, RemoteEvent};
use crate::proto::{AddJob, LibQuery, PairLink, PhoneHello};
use captae_core::{Options, Paths};
use serde_json::{json, Value};
use std::time::Duration;

struct T {
    _dir: tempfile::TempDir,
    core: Core,
    remote: Remote,
}

async fn setup() -> T {
    let dir = tempfile::tempdir().unwrap();
    let core = Core::open(Options { paths: Paths::portable(dir.path()), sidecar_dir: None, restore: false }).await.unwrap();
    let remote = Remote::new(core.clone(), Opts { name: "测试电脑".into(), on_change: None, bind: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)), bonjour: false });
    let st = remote.set_enabled(true).await.unwrap();
    assert!(st.enabled && st.running);
    assert_eq!(st.hosts, vec!["127.0.0.1".to_string()]);
    T { _dir: dir, core, remote }
}

fn phone() -> PhoneHello {
    PhoneHello { name: "iPhone".into(), platform: "ios".into() }
}

async fn paired(t: &T) -> (Credentials, Arc<Client>) {
    let link = PairLink::parse(&t.remote.pairing(false).unwrap().uri).unwrap();
    let creds = pair_qr(&link, &phone()).await.unwrap();
    let c = Arc::new(Client::connect(creds.clone()).await.unwrap());
    (creds, c)
}

#[tokio::test]
async fn qr_pairing_is_one_time_and_revoke_locks_out() {
    let t = setup().await;
    let view = t.remote.pairing(false).unwrap();
    let link = PairLink::parse(&view.uri).unwrap();
    assert_eq!(link.fp, t.remote.fingerprint().unwrap());
    assert_eq!(link.name, "测试电脑");
    let creds = pair_qr(&link, &phone()).await.unwrap();
    assert_eq!(creds.name, "测试电脑");
    assert_eq!(creds.token.len(), 43, "256 位令牌");
    // 同一个二维码再扫一次：作废了
    let e = pair_qr(&link, &phone()).await.unwrap_err();
    assert_eq!(error_code(&e), Some(code::PAIR_CLOSED));

    // 电脑上只存哈希，文件 0600
    let dir = t.core.paths.data.join("remote");
    let state = std::fs::read_to_string(dir.join("state.json")).unwrap();
    assert!(!state.contains(&creds.token) && state.contains(&proto::token_hash(&creds.token)));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for f in ["state.json", "key.der", "cert.der"] {
            assert_eq!(std::fs::metadata(dir.join(f)).unwrap().permissions().mode() & 0o777, 0o600, "{f}");
        }
        assert_eq!(std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777, 0o700);
    }

    let c = Client::connect(creds.clone()).await.unwrap();
    assert_eq!(c.me().await.unwrap()["computer"], "测试电脑");
    let st = t.remote.status();
    assert_eq!(st.devices.len(), 1);
    assert_eq!((st.devices[0].name.as_str(), st.devices[0].platform.as_str()), ("iPhone", "ios"));

    // 撤销：马上被拒
    t.remote.revoke(&creds.device).unwrap();
    let e = c.me().await.unwrap_err();
    assert_eq!(error_code(&e), Some(code::UNAUTHORIZED));
    let e = c.jobs().await.unwrap_err();
    assert_eq!(error_code(&e), Some(code::UNAUTHORIZED));
    assert!(t.remote.status().devices.is_empty());
    assert_eq!(captae_core::errors::code_in(&t.remote.revoke(&creds.device).unwrap_err()), Some(code::NOT_FOUND));

    // 别的指纹（不是这台电脑）：连不上，报证书不对
    let mut fake = creds.clone();
    fake.fp = proto::fingerprint(b"another computer");
    let e = Client::connect(fake).await.err().unwrap();
    assert_eq!(error_code(&e), Some(client::code::CERT_MISMATCH));
}

#[tokio::test]
async fn pairing_codes_expire() {
    let t = setup().await;
    let view = t.remote.pairing(false).unwrap();
    let link = PairLink::parse(&view.uri).unwrap();
    // 5 分钟有效
    let ttl = view.expires_at - now_ms();
    assert!(ttl > proto::PAIR_TTL_MS - 5_000 && ttl <= proto::PAIR_TTL_MS);
    t.remote.0.pairing.lock().unwrap().as_mut().unwrap().expires = now_ms() - 1;
    let e = pair_qr(&link, &phone()).await.unwrap_err();
    assert_eq!(error_code(&e), Some(code::PAIR_EXPIRED));
    let e = pair_code("127.0.0.1", link.port, &view.code, &phone()).await.unwrap_err();
    assert_eq!(error_code(&e), Some(code::PAIR_EXPIRED));
    // 过期了再要：自动换一轮新的
    let v2 = t.remote.pairing(false).unwrap();
    assert_ne!(v2.uri, view.uri);
    assert!(v2.expires_at > now_ms());
    // 关掉「让手机连接」：配对码全部作废，也要不到新的
    t.remote.set_enabled(false).await.unwrap();
    assert_eq!(captae_core::errors::code_in(&t.remote.pairing(false).unwrap_err()), Some(code::PAIR_CLOSED));
    let e = pair_qr(&PairLink::parse(&v2.uri).unwrap(), &phone()).await.unwrap_err();
    assert_eq!(error_code(&e), Some(client::code::UNREACHABLE));
}

#[tokio::test]
async fn six_digits_void_after_five_wrong() {
    let t = setup().await;
    let view = t.remote.pairing(false).unwrap();
    let port = t.remote.status().port.unwrap();
    let wrong = if view.code == "000000" { "111111" } else { "000000" };
    for k in 1..=5 {
        let e = pair_code("127.0.0.1", port, wrong, &phone()).await.unwrap_err();
        assert_eq!(error_code(&e), Some(client::code::PAIR_MISMATCH), "第 {k} 次");
        assert_eq!(t.remote.pairing(false).unwrap().left, 5 - k);
    }
    // 第 6 次：就算对了也不行
    let e = pair_code("127.0.0.1", port, &view.code, &phone()).await.unwrap_err();
    assert_eq!(error_code(&e), Some(code::PAIR_VOID));
    let v = t.remote.pairing(false).unwrap();
    assert!(v.void && v.left == 0 && v.code == view.code, "作废以后不自动换（免得一直猜）");
    // 换一轮新的，对的 6 位数：配上，拿到的指纹就是这台电脑的
    let v = t.remote.pairing(true).unwrap();
    assert!(!v.void);
    let creds = pair_code("127.0.0.1", port, &v.code, &PhoneHello { name: "Pixel 9".into(), platform: "android".into() }).await.unwrap();
    assert_eq!(creds.fp, t.remote.fingerprint().unwrap());
    let c = Client::connect(creds).await.unwrap();
    assert_eq!(c.me().await.unwrap()["name"], "Pixel 9");
    // 用过了，整轮作废
    let e = pair_code("127.0.0.1", port, &v.code, &phone()).await.unwrap_err();
    assert_eq!(error_code(&e), Some(code::PAIR_CLOSED));
}

#[tokio::test]
async fn only_whitelisted_requests() {
    let t = setup().await;
    let (creds, _c) = paired(&t).await;
    let tls = client::tls(Some(&creds.fp));
    let http = client::http_client(&tls, Some(Duration::from_secs(10))).unwrap();
    let base = proto::base_url("127.0.0.1", creds.port);
    let tok = creds.token.clone();
    use reqwest::Method;
    // 白名单外：改设置、AI 助手、cookies、删素材、插件那套接口、本机控制接口的路由……一律 403
    let forbidden: &[(Method, &str)] = &[
        (Method::GET, "/"),
        (Method::GET, "/v1/settings"),
        (Method::POST, "/v1/settings"),
        (Method::PATCH, "/v1/settings"),
        (Method::POST, "/v1/call"),
        (Method::GET, "/v1/cookies"),
        (Method::GET, "/v1/logins"),
        (Method::POST, "/v1/share"),
        (Method::POST, "/v1/sniffed"),
        (Method::POST, "/v1/focus"),
        (Method::POST, "/v1/check"),
        (Method::GET, "/v1/agent"),
        (Method::DELETE, "/v1/library/1"),
        (Method::POST, "/v1/library/1"),
        (Method::DELETE, "/v1/library/1/file"),
        (Method::PUT, "/v1/library/1/file"),
        (Method::DELETE, "/v1/jobs"),
        (Method::PUT, "/v1/jobs"),
        (Method::DELETE, "/v1/jobs/1"),
        (Method::GET, "/v1/jobs/1/wait"),
        (Method::POST, "/v1/tasks/1/cancel"),
        (Method::GET, "/v1/presets"),
        (Method::GET, "/v1/../v1/settings"),
    ];
    for (m, p) in forbidden {
        let r = http.request(m.clone(), format!("{base}{p}")).bearer_auth(&tok).body("{}").header("content-type", "application/json").send().await.unwrap();
        assert_eq!(r.status().as_u16(), 403, "{m} {p}");
        let v: Value = r.json().await.unwrap();
        assert_eq!(v["code"], code::NOT_ALLOWED, "{m} {p}");
    }
    // 加任务带了不许带的字段（保存位置、yt-dlp / ffmpeg 参数、来源、请求头、cookies）：整个拒掉，一个任务都不加
    for extra in [
        json!({"output_dir": "/tmp"}),
        json!({"raw_args": ["--exec", "touch /tmp/x"]}),
        json!({"ytdlp_args": ["-o", "/tmp/x"]}),
        json!({"ffmpeg_args": ["-y"]}),
        json!({"origin": "ui"}),
        json!({"headers": {"Cookie": "a=b"}}),
        json!({"cookies": [{"name": "a", "value": "b"}]}),
        json!({"threads": 16}),
        json!({"record_live": true}),
    ] {
        let mut body = json!({"url": "https://example.com/a.mp4"});
        body.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        let r = http.post(format!("{base}/v1/jobs")).bearer_auth(&tok).json(&body).send().await.unwrap();
        assert_eq!(r.status().as_u16(), 400, "{body}");
        assert_eq!(r.json::<Value>().await.unwrap()["code"], code::BAD_REQUEST);
    }
    // 只有 http / https 链接
    for u in ["file:///etc/passwd", "ftp://x/a.mp4", "javascript:alert(1)", ""] {
        let r = http.post(format!("{base}/v1/jobs")).bearer_auth(&tok).json(&json!({"url": u})).send().await.unwrap();
        assert_eq!(r.status().as_u16(), 400, "{u}");
    }
    assert!(t.core.jobs().is_empty());
    // 动作也只认 JobAction 那几个
    let r = http.post(format!("{base}/v1/jobs/1/action")).bearer_auth(&tok).json(&json!({"action": "delete_files"})).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 400);
    // 没带令牌、令牌不对、令牌对但用在别处：401
    for (p, auth) in [("/v1/jobs", None), ("/v1/jobs", Some("x".repeat(43))), ("/v1/library", Some(String::new())), ("/v1/events", None), ("/v1/library/1/file", None)] {
        let mut rq = http.get(format!("{base}{p}"));
        if let Some(a) = auth {
            rq = rq.bearer_auth(a);
        }
        let r = rq.send().await.unwrap();
        assert_eq!(r.status().as_u16(), 401, "{p}");
    }
    // 不用令牌的只有这几个
    let r = http.get(format!("{base}/v1/info")).send().await.unwrap();
    assert_eq!(r.status().as_u16(), 200);
    assert!(r.json::<Value>().await.unwrap().get("fp").is_none());
}

#[tokio::test]
async fn range_download() {
    let t = setup().await;
    let (_, c) = paired(&t).await;
    // 放一个已知内容的文件进素材库
    let src = t.core.paths.movies.join("片段 1.mp4");
    std::fs::create_dir_all(src.parent().unwrap()).unwrap();
    let data: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 251) as u8).collect();
    std::fs::write(&src, &data).unwrap();
    let id = t
        .core
        .lib
        .add(&captae_core::library::NewAsset { kind: "video".into(), title: "片段 1".into(), path: src.clone(), format: "MP4".into(), size: data.len() as u64, ..Default::default() })
        .unwrap();
    let lib = c.library(&LibQuery::default()).await.unwrap();
    assert_eq!(lib["total"], 1);
    let item = &lib["items"][0];
    assert_eq!(item["title"], "片段 1");
    assert!(item.get("path").is_none(), "不给电脑上的路径");
    assert_eq!(item["hasThumb"], false);
    assert_eq!(error_code(&c.thumb(id).await.unwrap_err()), Some(code::NOT_FOUND));
    assert_eq!(error_code(&c.asset(id + 99).await.unwrap_err()), Some(code::NOT_FOUND));

    // 一段
    let (b, total) = c.file_range(id, 10, 19).await.unwrap();
    assert_eq!((b.as_slice(), total), (&data[10..20], data.len() as u64));
    let (b, _) = c.file_range(id, 299_990, 400_000).await.unwrap();
    assert_eq!(b.as_slice(), &data[299_990..]);
    // 超出范围：416
    let e = c.file_range(id, 300_000, 300_010).await.unwrap_err();
    assert_eq!(error_code(&e), Some(code::RANGE));

    // 整个下载，再从一半接着下
    let out = t._dir.path().join("phone.mp4");
    let n = c.download_to(id, &out, |_, _| {}).await.unwrap();
    assert_eq!((n, std::fs::read(&out).unwrap()), (data.len() as u64, data.clone()));
    // 模拟下到一半断了：留前 100000 字节和 etag
    let tls = client::tls(Some(&c.credentials().fp));
    let http = client::http_client(&tls, None).unwrap();
    let r = http.head(format!("{}/v1/library/{id}/file", proto::base_url("127.0.0.1", c.credentials().port))).bearer_auth(&c.credentials().token).send().await.unwrap();
    assert_eq!(r.headers()["accept-ranges"], "bytes");
    let etag = r.headers()["etag"].to_str().unwrap().to_string();
    std::fs::write(&out, &data[..100_000]).unwrap();
    std::fs::write(t._dir.path().join("phone.mp4.etag"), &etag).unwrap();
    let mut first = None;
    let n = c
        .download_to(id, &out, |got, _| {
            first.get_or_insert(got);
        })
        .await
        .unwrap();
    assert_eq!(first, Some(100_000), "从断的地方接着下");
    assert_eq!((n, std::fs::read(&out).unwrap()), (data.len() as u64, data.clone()));
    // 电脑上的文件换过了（etag 不一样）：从头下
    std::fs::write(&out, b"stale").unwrap();
    std::fs::write(t._dir.path().join("phone.mp4.etag"), "\"old\"").unwrap();
    c.download_to(id, &out, |_, _| {}).await.unwrap();
    assert_eq!(std::fs::read(&out).unwrap(), data);
}

/// 等下一条满足条件的事件（最多 10 秒）
async fn next_where(s: &mut client::Subscription, f: impl Fn(&RemoteEvent) -> bool) -> RemoteEvent {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let e = s.recv().await.expect("订阅断了");
            if f(&e) {
                return e;
            }
        }
    })
    .await
    .expect("10 秒内没等到")
}

#[tokio::test]
async fn events_are_pushed() {
    let t = setup().await;
    let (creds, c) = paired(&t).await;
    let mut sub = c.subscribe();
    assert_eq!(next_where(&mut sub, |_| true).await, RemoteEvent::Connected);
    assert_eq!(next_where(&mut sub, |_| true).await, RemoteEvent::Jobs(vec![]));
    assert!(t.remote.status().devices[0].online);
    // 手机加任务：桌面端记来源「iPhone」，推回来的任务也带着；任务记录写「来自 iPhone」
    let j = c.add(&AddJob { url: "http://127.0.0.1:9/movie.mp4".into(), ..Default::default() }).await.unwrap();
    assert_eq!(j["from"], "iPhone");
    let id = j["id"].as_u64().unwrap();
    let core_job = t.core.job(id).unwrap();
    assert_eq!(core_job.from.as_deref(), Some("iPhone"));
    assert!(core_job.log.iter().any(|l| l.1.contains("iPhone")));
    let ev = next_where(&mut sub, |e| matches!(e, RemoteEvent::Job(v) if v["id"] == id)).await;
    let RemoteEvent::Job(v) = ev else { unreachable!() };
    assert_eq!(v["from"], "iPhone");
    assert!(v.get("path").is_none());
    // 任务列表、取消
    assert!(c.jobs().await.unwrap().iter().any(|x| x["id"] == id));
    c.action(id, "cancel").await.unwrap();
    assert_eq!(next_where(&mut sub, |e| matches!(e, RemoteEvent::JobRemoved(_))).await, RemoteEvent::JobRemoved(id));
    assert_eq!(error_code(&c.action(id, "pause").await.unwrap_err()), Some(code::NOT_FOUND));
    // 素材库变了
    t.core.emit_library();
    assert_eq!(next_where(&mut sub, |e| matches!(e, RemoteEvent::Library)).await, RemoteEvent::Library);
    // 撤销：推送那条连接马上断开，客户端不再重连
    t.remote.revoke(&creds.device).unwrap();
    assert_eq!(next_where(&mut sub, |e| !matches!(e, RemoteEvent::Job(_) | RemoteEvent::JobRemoved(_))).await, RemoteEvent::Revoked);
    assert!(tokio::time::timeout(Duration::from_secs(3), sub.recv()).await.unwrap().is_none());
}

#[test]
fn ranges() {
    use super::routes::parse_range;
    assert_eq!(parse_range("bytes=0-9", 100), Some(Ok((0, 9))));
    assert_eq!(parse_range("bytes=90-", 100), Some(Ok((90, 99))));
    assert_eq!(parse_range("bytes=-10", 100), Some(Ok((90, 99))));
    assert_eq!(parse_range("bytes=50-500", 100), Some(Ok((50, 99))));
    assert_eq!(parse_range("bytes=100-", 100), Some(Err(())));
    assert_eq!(parse_range("bytes=5-1", 100), Some(Err(())));
    assert_eq!(parse_range("bytes=0-1,5-6", 100), None);
    assert_eq!(parse_range("items=0-1", 100), None);
}

#[test]
fn names_are_cleaned() {
    assert_eq!(clean_name("  iPhone\n\u{7}  ").as_deref(), Some("iPhone"));
    assert_eq!(clean_name("   "), None);
    assert_eq!(clean_name(&"长".repeat(80)).unwrap().chars().count(), 40);
}
