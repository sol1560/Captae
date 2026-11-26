//! 遥控电脑的测试：在本机起一个电脑端的服务（只听 127.0.0.1、不广播 Bonjour），手机这边用 `RemoteHub` 去连。
//! 覆盖：扫码和 6 位数配对、令牌只经安全存储回调、离线队列（存盘、重连后自动发、改到手机）、撤销后的状态、
//! 合并视图、电脑素材库和存到手机。

use super::*;
use crate::{InitOptions, Platform};
use captae_core::tools::InProcessRunner;
use captae_core::{Core, Options, Paths};
use captae_remote::server::{Opts, Remote};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpListener};

const PHONE: &str = "测试手机";

/// 内存里的安全存储；`fail` 时写不进去
#[derive(Default)]
struct MemStore {
    map: Mutex<HashMap<String, String>>,
    fail: AtomicBool,
}

impl SecureStore for MemStore {
    fn read(&self, key: String) -> Option<String> {
        self.map.lock().unwrap().get(&key).cloned()
    }
    fn write(&self, key: String, value: String) -> bool {
        if self.fail.load(Ordering::SeqCst) {
            return false;
        }
        self.map.lock().unwrap().insert(key, value);
        true
    }
    fn delete(&self, key: String) {
        self.map.lock().unwrap().remove(&key);
    }
}

/// 把回调记成一行行文字
#[derive(Default)]
struct Log(Mutex<Vec<String>>);

impl Log {
    fn has(&self, prefix: &str) -> bool {
        self.0.lock().unwrap().iter().any(|l| l.starts_with(prefix))
    }
    fn push(&self, s: String) {
        self.0.lock().unwrap().push(s);
    }
}

impl RemoteListener for Log {
    fn on_job(&self, job: DeviceJob) {
        self.push(format!("job {} {:?} {:?}", job.job.id, job.job.state, job.from));
    }
    fn on_removed(&self, _device: String, id: u64) {
        self.push(format!("removed {id}"));
    }
    fn on_finished(&self, _device: String, id: u64, _title: String, asset_id: Option<i64>) {
        self.push(format!("finished {id} {asset_id:?}"));
    }
    fn on_library(&self, _device: String) {
        self.push("library".into());
    }
    fn on_resync(&self, _device: String) {
        self.push("resync".into());
    }
    fn on_device(&self, d: MobileDevice) {
        self.push(format!("device {:?}", d.status));
    }
    fn on_devices(&self) {
        self.push("devices".into());
    }
    fn on_pending(&self) {
        self.push("pending".into());
    }
    fn on_pending_sent(&self, task: PendingTask, job: DeviceJob) {
        self.push(format!("sent {} -> {} {:?}", task.url, job.job.id, job.from));
    }
    fn on_save(&self, s: SaveProgress) {
        self.push(format!("save {:?} {}/{}", s.state, s.got_bytes, s.total_bytes));
    }
    fn on_saves_restored(&self, saves: Vec<SaveProgress>) {
        self.push(format!("restored {:?}", saves.iter().map(|s| s.id).collect::<Vec<_>>()));
    }
}

/// 电脑那一头：自己的 Runtime、核心库（Rust 自己下直链，不用 aria2c）、远程服务
struct Desk {
    _dir: tempfile::TempDir,
    rt: tokio::runtime::Runtime,
    core: Core,
    remote: Remote,
}

impl Desk {
    fn new() -> Desk {
        let dir = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
        let core = {
            let _g = crate::tests::lang_lock();
            rt.block_on(Core::open_with(Options { paths: Paths::portable(dir.path()), sidecar_dir: None, restore: false }, Arc::new(InProcessRunner))).unwrap()
        };
        let remote = Remote::new(core.clone(), Opts { name: "测试电脑".into(), on_change: None, bind: Some(IpAddr::V4(Ipv4Addr::LOCALHOST)), bonjour: false });
        let st = rt.block_on(remote.set_enabled(true)).unwrap();
        assert!(st.running);
        Desk { _dir: dir, rt, core, remote }
    }
    fn qr(&self) -> String {
        self.remote.pairing(false).unwrap().uri
    }
    fn port(&self) -> u16 {
        self.remote.status().port.unwrap()
    }
    fn enable(&self, on: bool) {
        self.rt.block_on(self.remote.set_enabled(on)).unwrap();
    }
}

struct Phone {
    _dir: tempfile::TempDir,
    core: Arc<MobileCore>,
    secure: Arc<MemStore>,
    hub: Arc<RemoteHub>,
    log: Arc<Log>,
}

fn phone_with(platform: Platform) -> Phone {
    let dir = tempfile::tempdir().unwrap();
    let _g = crate::tests::lang_lock();
    let core = MobileCore::new(InitOptions {
        data_dir: dir.path().join("data").display().to_string(),
        downloads_dir: dir.path().join("dl").display().to_string(),
        platform,
        language: "zh-CN".into(),
        native_lib_dir: None,
    })
    .unwrap();
    let secure = Arc::new(MemStore::default());
    let hub = RemoteHub::new(core.clone(), opts(dir.path()), secure.clone()).unwrap();
    let log = Arc::new(Log::default());
    hub.set_listener(log.clone());
    Phone { _dir: dir, core, secure, hub, log }
}

fn opts(dir: &Path) -> RemoteOptions {
    RemoteOptions { phone_name: PHONE.into(), cache_dir: dir.join("cache").display().to_string() }
}

/// UniFFI 的 async 在 App 里由 Swift / Kotlin 驱动，不在 tokio 里；测试里也用一个不相干的小执行器
fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(f)
}

fn wait(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < until {
        if ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("等了 {secs} 秒还没有：{what}");
}

/// 本机文件服务器：GET / HEAD 任何路径都回同一份数据（`n` 字节）
fn files(n: usize) -> (u16, Vec<u8>) {
    let data: Vec<u8> = (0..n).map(|i| (i * 7 % 251) as u8).collect();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    let body = data.clone();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let body = body.clone();
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).starts_with("HEAD");
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                if !head {
                    let _ = s.write_all(&body);
                }
            });
        }
    });
    (port, data)
}

fn pair(p: &Phone, d: &Desk) -> MobileDevice {
    block(p.hub.pair_qr(d.qr())).unwrap()
}

fn online(p: &Phone, fp: &str) {
    wait("电脑在线", 20, || p.hub.status_of(fp) == Some(DeviceStatus::Online));
}

#[test]
fn pairing_keeps_token_in_secure_store() {
    let d = Desk::new();
    let p = phone_with(Platform::IosStore);

    // 不是 Captae 的二维码
    assert!(p.hub.parse_pair_link("https://example.com".into()).is_none());
    let e = block(p.hub.pair_qr("hello".into())).unwrap_err();
    assert_eq!(convert::code_of(&e), code::BAD_LINK);

    let uri = d.qr();
    let info = p.hub.parse_pair_link(uri.clone()).unwrap();
    assert_eq!((info.name.as_str(), info.known), ("测试电脑", false));
    let dev = block(p.hub.pair_qr(uri.clone())).unwrap();
    assert_eq!(dev.id, info.fingerprint);
    assert_eq!((dev.name.as_str(), dev.status), ("测试电脑", DeviceStatus::Idle));
    assert!(p.log.has("devices"));
    // 同一个二维码再扫：用过一次就作废
    let e = block(p.hub.pair_qr(uri.clone())).unwrap_err();
    assert_eq!(convert::code_of(&e), code::PAIR_CLOSED);
    assert!(p.hub.parse_pair_link(uri).unwrap().known);

    // 令牌只在安全存储里：256 位，computers.json 里没有
    let token = p.secure.read(store::token_key(&dev.id)).unwrap();
    assert_eq!(token.len(), 43);
    let file = std::fs::read_to_string(p.core.data_dir().join("remote/computers.json")).unwrap();
    assert!(file.contains(&dev.id) && !file.contains(&token), "{file}");
    // 电脑上记下了这台手机的名字
    assert_eq!(d.remote.status().devices[0].name, PHONE);

    // 连上
    p.hub.set_active(true);
    online(&p, &dev.id);
    assert!(p.log.has("device Online"));
    let devs = p.hub.devices();
    assert_eq!(devs.len(), 2);
    assert_eq!((devs[0].id.as_str(), devs[0].local, devs[0].name.as_str()), (LOCAL_DEVICE, true, PHONE));

    // 6 位数配对同一台：还是一台，令牌换了新的
    let code6 = d.remote.pairing(false).unwrap().code;
    let e = block(p.hub.pair_code("127.0.0.1".into(), d.port(), if code6 == "000000" { "111111".into() } else { "000000".into() })).unwrap_err();
    assert_eq!(convert::code_of(&e), code::PAIR_MISMATCH);
    let dev2 = block(p.hub.pair_code("127.0.0.1".into(), d.port(), code6)).unwrap();
    assert_eq!(dev2.id, dev.id);
    assert_eq!(p.hub.devices().len(), 2);
    assert_ne!(p.secure.read(store::token_key(&dev.id)).unwrap(), token);
    online(&p, &dev.id);

    // 改名只改手机这边
    let r = p.hub.rename(dev.id.clone(), " 书房的 Mac ".into()).unwrap();
    assert_eq!((r.name.as_str(), r.computer_name.as_str()), ("书房的 Mac", "测试电脑"));

    // 重新打开：令牌从安全存储读；安全存储空了（钥匙串被清）就要重新配对
    p.hub.set_active(false);
    let again = RemoteHub::new(p.core.clone(), opts(p._dir.path()), p.secure.clone()).unwrap();
    assert_eq!(again.device(dev.id.clone()).unwrap().status, DeviceStatus::Idle);
    assert_eq!(again.device(dev.id.clone()).unwrap().name, "书房的 Mac");
    let empty = RemoteHub::new(p.core.clone(), opts(p._dir.path()), Arc::new(MemStore::default())).unwrap();
    assert_eq!(empty.device(dev.id.clone()).unwrap().status, DeviceStatus::NeedsPairing);
    let e = block(empty.send(dev.id.clone(), SendOptions { url: "https://example.com/a.mp4".into(), ..Default::default() })).unwrap_err();
    assert_eq!(convert::code_of(&e), code::NEEDS_PAIRING);

    // 安全存储写不进去：配对算失败，不留下没有令牌的电脑
    let p2 = phone_with(Platform::Android);
    p2.secure.fail.store(true, Ordering::SeqCst);
    let e = block(p2.hub.pair_qr(d.qr())).unwrap_err();
    assert_eq!(convert::code_of(&e), code::NEEDS_PAIRING);
    assert_eq!(p2.hub.devices().len(), 1);

    // 删掉：令牌一起删
    p.hub.remove(dev.id.clone()).unwrap();
    assert!(p.secure.read(store::token_key(&dev.id)).is_none());
    assert_eq!(p.hub.devices().len(), 1);
}

#[test]
fn offline_queue_resends_after_reconnect() {
    let d = Desk::new();
    let (fport, data) = files(4 * 1024 * 1024);
    let p = phone_with(Platform::IosStore);
    let dev = pair(&p, &d);
    p.hub.set_active(true);
    online(&p, &dev.id);

    // 电脑关掉「让手机连接这台电脑」：手机看到离线
    d.enable(false);
    wait("电脑离线", 20, || p.hub.status_of(&dev.id) == Some(DeviceStatus::Offline));

    let url = format!("http://127.0.0.1:{fport}/queued.mp4");
    let r = block(p.hub.send(dev.id.clone(), SendOptions { url: url.clone(), title: Some("等电脑".into()), ..Default::default() })).unwrap();
    let SendResult::Queued { task } = r else { panic!("应该进队列") };
    assert_eq!((task.url.as_str(), task.title.as_deref(), task.can_run_here), (url.as_str(), Some("等电脑"), true));
    // 上架版不能在手机上下的网站也能排队，但改不到手机上
    let yt = block(p.hub.send(dev.id.clone(), SendOptions { url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ".into(), ..Default::default() })).unwrap();
    let SendResult::Queued { task: yt } = yt else { panic!() };
    assert!(!yt.can_run_here);
    // 坏链接不进队列
    let e = block(p.hub.send(dev.id.clone(), SendOptions { url: "ftp://x".into(), ..Default::default() })).unwrap_err();
    assert_eq!(convert::code_of(&e), code::BAD_REQUEST);
    assert_eq!(p.hub.pending().len(), 2);
    assert_eq!(p.hub.device(dev.id.clone()).unwrap().waiting, 2);
    assert!(p.log.has("pending"));
    // 存在数据目录里：重新打开还在
    let again = RemoteHub::new(p.core.clone(), opts(p._dir.path()), p.secure.clone()).unwrap();
    assert_eq!(again.pending().len(), 2);
    drop(again);

    // 合并视图：电脑那组里有等它上线的任务
    let g = p.hub.merged_jobs(None, None);
    assert_eq!(g.len(), 2);
    assert_eq!((g[0].device.id.as_str(), g[0].pending.len()), (dev.id.as_str(), 2));
    assert_eq!(g[1].device.id, LOCAL_DEVICE);
    assert_eq!(p.hub.merged_jobs(None, Some("等电脑".into()))[0].pending.len(), 1);

    // YouTube 那条先删掉，剩下的等电脑上线自动发
    p.hub.remove_pending(yt.id).unwrap();
    d.enable(true);
    // App 发现到电脑（Bonjour 的 TXT 里有指纹）：马上再找，不等重连间隔
    let svc = DiscoveredService { name: "测试电脑".into(), hosts: vec!["127.0.0.1".into()], port: d.port(), txt: HashMap::from([("fp".to_string(), dev.id.clone()), ("v".into(), "1".into())]) };
    assert_eq!(p.hub.discovered(svc), Some(dev.id.clone()));
    online(&p, &dev.id);
    wait("队列发出去了", 20, || p.hub.pending().is_empty());
    wait("回调 on_pending_sent", 5, || p.log.has(&format!("sent {url}")));
    // 电脑上的任务记着是这台手机加的
    wait("电脑上有这个任务", 10, || d.core.jobs().iter().any(|j| j.url == url && j.from.as_deref() == Some(PHONE)));
    wait("手机缓存里有电脑上的任务", 10, || p.hub.remote_jobs(dev.id.clone()).iter().any(|j| j.url == url));
    let g = p.hub.merged_jobs(Some(dev.id.clone()), None);
    assert_eq!(g.len(), 1);
    assert!(g[0].jobs.windows(2).all(|w| w[0].job.id > w[1].job.id), "新的在前");
    assert!(g[0].jobs.iter().any(|j| j.job.url == url && j.from.as_deref() == Some(PHONE)));
    // 下完：电脑的素材库里有它，存到手机字节一样
    wait("电脑上下完", 30, || p.log.has("finished"));
    let secs = block(p.hub.merged_library(LibraryQuery::default(), None));
    assert_eq!(secs.len(), 2);
    assert_eq!(secs[0].device.id, dev.id);
    assert!(secs[0].error.is_none(), "{:?}", secs[0].error);
    let a = secs[0].items.iter().find(|a| a.asset.url == url).expect("电脑素材库里有它").asset.clone();
    assert!(a.path.is_empty(), "电脑上的路径不给手机");
    assert_eq!(block(p.hub.remote_asset(dev.id.clone(), a.id)).unwrap().unwrap().id, a.id);
    // 没有 ffmpeg，电脑上没有封面
    assert_eq!(block(p.hub.remote_thumbnail(dev.id.clone(), a.id, false)).unwrap(), None);
    let dest = p._dir.path().join("saved/queued.mp4");
    let s = p.hub.save_to_phone(dev.id.clone(), a.id, dest.display().to_string()).unwrap();
    wait("存到手机", 20, || p.hub.saves().iter().any(|x| x.id == s.id && x.state == SaveState::Done));
    assert_eq!(std::fs::read(&dest).unwrap(), data);
    assert!(!dest.with_extension("mp4.etag").exists());
    // 存完自动进了这台手机的素材库，带着电脑素材库里的标题、来源
    let saved = p.hub.saves().into_iter().find(|x| x.id == s.id).unwrap();
    let local = p.core.asset(saved.local_asset_id.expect("存完要导入手机素材库")).unwrap().unwrap();
    assert_eq!((local.title.as_str(), local.url.as_str(), local.site.as_str(), local.bytes), (a.title.as_str(), a.url.as_str(), a.site.as_str(), data.len() as u64));
    assert_eq!(local.path, dest.display().to_string());
    // 存到一半马上暂停、马上继续，反复几次：续传的位置对，字节一样
    let dest2 = p._dir.path().join("saved/again.mp4");
    let s2 = p.hub.save_to_phone(dev.id.clone(), a.id, dest2.display().to_string()).unwrap();
    for _ in 0..5 {
        std::thread::sleep(Duration::from_millis(3));
        p.hub.pause_save(s2.id).unwrap();
        p.hub.resume_save(s2.id).unwrap();
    }
    wait("暂停继续以后存完", 20, || p.hub.saves().iter().any(|x| x.id == s2.id && x.state == SaveState::Done));
    assert_eq!(std::fs::read(&dest2).unwrap(), data);
    // 取消：删掉下了一半的文件
    let dest3 = p._dir.path().join("saved/cancel.mp4");
    let s3 = p.hub.save_to_phone(dev.id.clone(), a.id, dest3.display().to_string()).unwrap();
    let c = p.hub.cancel_save(s3.id).unwrap();
    assert!(matches!(c.state, SaveState::Cancelled | SaveState::Done));
    if c.state == SaveState::Cancelled {
        // 还在路上的写入落到删掉的那个文件上，路径上不会再冒出来
        std::thread::sleep(Duration::from_millis(200));
        assert!(!dest3.exists());
    }
    assert!(p.hub.saves().iter().all(|x| x.id != s3.id));

    // 再离线、再排一条，这次改成在手机上下（直链，上架版也行）
    d.enable(false);
    wait("电脑离线", 20, || p.hub.status_of(&dev.id) == Some(DeviceStatus::Offline));
    let url2 = format!("http://127.0.0.1:{fport}/phone.mp4");
    let SendResult::Queued { task: t2 } = block(p.hub.send(dev.id.clone(), SendOptions { url: url2.clone(), ..Default::default() })).unwrap() else { panic!() };
    let m = p.hub.move_to_phone(vec![]);
    assert_eq!((m.moved.len(), m.kept.len()), (1, 0));
    assert_eq!(m.moved[0].url, url2);
    assert!(p.hub.pending().iter().all(|x| x.id != t2.id));
    let local = p.hub.merged_jobs(Some(LOCAL_DEVICE.into()), None);
    assert!(local[0].jobs.iter().any(|j| j.job.url == url2 && j.device == LOCAL_DEVICE));
}

#[test]
fn revoked_computer_is_locked_out() {
    let d = Desk::new();
    let p = phone_with(Platform::Android);
    let dev = pair(&p, &d);
    p.hub.set_active(true);
    online(&p, &dev.id);
    wait("收到任务快照", 10, || p.log.has("resync"));

    let id = d.remote.status().devices[0].id.clone();
    d.remote.revoke(&id).unwrap();
    wait("手机看到被撤销", 20, || p.hub.status_of(&dev.id) == Some(DeviceStatus::Revoked));
    assert!(p.log.has("device Revoked"));
    // 令牌删掉了，computers.json 记着被撤销
    assert!(p.secure.read(store::token_key(&dev.id)).is_none());
    let file = std::fs::read_to_string(p.core.data_dir().join("remote/computers.json")).unwrap();
    assert!(file.contains("\"revoked\": true"), "{file}");
    // 再发任务、查素材库：直接拒绝，不进队列
    let e = block(p.hub.send(dev.id.clone(), SendOptions { url: "https://example.com/a.mp4".into(), ..Default::default() })).unwrap_err();
    assert_eq!((convert::code_of(&e), e.detail().action.as_str()), (code::UNAUTHORIZED, "pair"));
    assert!(p.hub.pending().is_empty());
    let e = block(p.hub.remote_library(dev.id.clone(), LibraryQuery::default())).unwrap_err();
    assert_eq!(convert::code_of(&e), code::UNAUTHORIZED);
    let secs = block(p.hub.merged_library(LibraryQuery::default(), None));
    assert_eq!(secs[0].error.as_ref().map(|e| e.code.as_str()), Some(code::UNAUTHORIZED));
    // 重新打开还是被撤销；set_active 不会再去连
    let again = RemoteHub::new(p.core.clone(), opts(p._dir.path()), p.secure.clone()).unwrap();
    assert_eq!(again.device(dev.id.clone()).unwrap().status, DeviceStatus::Revoked);
    again.set_active(true);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(again.device(dev.id.clone()).unwrap().status, DeviceStatus::Revoked);

    // 电脑上已经撤销、手机这边没发现（不在线时被撤销）：第一次调接口拿到 401 时记下
    let d2 = Desk::new();
    let p2 = phone_with(Platform::IosStore);
    let dev2 = pair(&p2, &d2);
    let id2 = d2.remote.status().devices[0].id.clone();
    d2.remote.revoke(&id2).unwrap();
    let e = block(p2.hub.refresh_remote_jobs(dev2.id.clone())).unwrap_err();
    assert_eq!(convert::code_of(&e), code::UNAUTHORIZED);
    assert_eq!(p2.hub.status_of(&dev2.id), Some(DeviceStatus::Revoked));
    assert!(p2.secure.read(store::token_key(&dev2.id)).is_none());
}

#[test]
fn rejected_pending_task_is_not_resent() {
    let d = Desk::new();
    let p = phone_with(Platform::IosStore);
    let dev = pair(&p, &d);
    // 电脑只收 4096 个字以内的链接：手机这边看着是个链接，电脑会拒
    let long = format!("https://example.com/{}.mp4", "a".repeat(5000));
    // App 在后台（没 set_active）：发的任务进队列，不去连
    let SendResult::Queued { task } = block(p.hub.send(dev.id.clone(), SendOptions { url: long, ..Default::default() })).unwrap() else { panic!("后台时进队列") };
    assert_eq!(p.hub.status_of(&dev.id), Some(DeviceStatus::Idle));
    // 到前台：连上、发出去；电脑拒绝了就记下错误、留在队列里、不再自动发
    p.hub.set_active(true);
    online(&p, &dev.id);
    wait("记下电脑拒绝的原因", 20, || p.hub.pending().iter().any(|t| t.id == task.id && t.error.is_some()));
    let t = p.hub.pending().into_iter().find(|t| t.id == task.id).unwrap();
    assert_eq!(t.error.as_ref().unwrap().code, code::BAD_REQUEST);
    // 重连一次也不会再发
    p.hub.set_active(false);
    p.hub.set_active(true);
    online(&p, &dev.id);
    std::thread::sleep(Duration::from_millis(500));
    assert!(d.core.jobs().is_empty());
    // 重试：照样被拒；删掉
    p.hub.retry_pending(t.id).unwrap();
    wait("又被拒", 20, || p.hub.pending().iter().any(|x| x.id == t.id && x.error.is_some()));
    p.hub.remove_pending(t.id).unwrap();
    assert!(p.hub.pending().is_empty());
}

fn sha256(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(b))
}

/// 电脑上下一个 `n` 字节的文件、进素材库，返回素材 id 和原数据
fn desk_asset(p: &Phone, d: &Desk, dev: &str, n: usize) -> (i64, Vec<u8>) {
    let (fport, data) = files(n);
    let url = format!("http://127.0.0.1:{fport}/big.mp4");
    let SendResult::Sent { job } = block(p.hub.send(dev.to_string(), SendOptions { url, ..Default::default() })).unwrap() else { panic!("电脑在线时应该直接发") };
    wait("电脑上下完", 60, || d.core.job(job.id).is_some_and(|j| j.asset_id.is_some()));
    (d.core.job(job.id).unwrap().asset_id.unwrap(), data)
}

#[test]
fn save_resumes_after_reopen() {
    let d = Desk::new();
    let p = phone_with(Platform::IosStore);
    let dev = pair(&p, &d);
    p.hub.set_active(true);
    online(&p, &dev.id);
    let (asset, data) = desk_asset(&p, &d, &dev.id, 24 * 1024 * 1024);

    // App 存到一半被结束：丢掉 RemoteHub（后台任务一起停），saves.json 里还是「在下」。
    // 本机很快，可能来不及停在中间：换个文件名再来，最多 8 次
    let size = |f: &Path| std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
    let mut hit = None;
    let mut hub = p.hub.clone();
    for k in 0..8 {
        let dest = p._dir.path().join(format!("saved/resume-{k}.mp4"));
        let s = hub.save_to_phone(dev.id.clone(), asset, dest.display().to_string()).unwrap();
        let until = Instant::now() + Duration::from_secs(10);
        while size(&dest) == 0 && Instant::now() < until {
            std::thread::sleep(Duration::from_micros(200));
        }
        drop(hub);
        std::thread::sleep(Duration::from_millis(300));
        let file = std::fs::read_to_string(p.core.data_dir().join("remote/saves.json")).unwrap();
        hub = RemoteHub::new(p.core.clone(), opts(p._dir.path()), p.secure.clone()).unwrap();
        let got = size(&dest);
        if got > 0 && (got as usize) < data.len() {
            assert!(file.contains("\"running\""), "{file}");
            hit = Some((s.id, dest, got));
            break;
        }
    }
    let p_hub = hub;
    let (id, dest, got) = hit.expect("8 次都没停在中间");
    // 重开：找回这一条（进度按文件实际大小），到前台时发 on_saves_restored，电脑连上就接着下
    let r = p_hub.saves().into_iter().find(|x| x.id == id).unwrap();
    assert_eq!((r.state, r.got_bytes), (SaveState::Running, got));
    let log = Arc::new(Log::default());
    p_hub.set_listener(log.clone());
    p_hub.set_active(true);
    assert!(log.has(&format!("restored [{id}]")), "{:?}", log.0.lock().unwrap());
    wait("重开后接着存完", 30, || p_hub.saves().iter().any(|x| x.id == id && x.state == SaveState::Done));
    let out = std::fs::read(&dest).unwrap();
    assert_eq!(sha256(&out), sha256(&data), "续传后哈希一致");
    let done = p_hub.saves().into_iter().find(|x| x.id == id).unwrap();
    assert!(done.done_at.is_some());
    // 再重开：下完的记录还在（7 天内），不会再发 on_saves_restored
    let again = RemoteHub::new(p.core.clone(), opts(p._dir.path()), p.secure.clone()).unwrap();
    let log2 = Arc::new(Log::default());
    again.set_listener(log2.clone());
    let kept = again.saves().into_iter().find(|x| x.id == id).unwrap();
    assert_eq!((kept.state, kept.got_bytes, kept.done_at), (SaveState::Done, data.len() as u64, done.done_at));
    again.set_active(true);
    assert!(!log2.has("restored"));
    again.set_active(false);
    p_hub.set_active(false);
}

#[test]
fn finished_saves_expire_after_seven_days() {
    let d = Desk::new();
    let p = phone_with(Platform::Android);
    let dev = pair(&p, &d);
    let day = 24 * 3600 * 1000;
    let rec = |id: u64, state: &str, done_at: Option<i64>, device: &str| store::SaveRec {
        id,
        device: device.into(),
        asset_id: id as i64,
        path: p._dir.path().join(format!("saved/{id}.mp4")).display().to_string(),
        got: 100,
        total: 100,
        state: state.into(),
        created_at: now_ms() - 9 * day,
        done_at,
        error_code: None,
        error_message: None,
        local_asset_id: None,
    };
    let st = store::Store::open(p.core.data_dir().join("remote")).unwrap();
    st.save_saves(
        &[
            rec(1, "done", Some(now_ms() - 8 * day), &dev.id),   // 下完 8 天：清掉
            rec(2, "done", Some(now_ms() - day), &dev.id),       // 下完 1 天：留着
            rec(3, "paused", None, &dev.id),                     // 没下完的不按时间清
            rec(4, "done", Some(now_ms() - day), "别的电脑"),     // 电脑已经删掉了：清掉
            rec(5, "failed", None, &dev.id),
        ],
        6,
    )
    .unwrap();
    let hub = RemoteHub::new(p.core.clone(), opts(p._dir.path()), p.secure.clone()).unwrap();
    let ids: Vec<u64> = hub.saves().iter().map(|s| s.id).collect();
    assert_eq!(ids, vec![2, 3, 5]);
    let s2 = hub.saves().into_iter().find(|s| s.id == 2).unwrap();
    assert_eq!((s2.state, s2.done_at.is_some()), (SaveState::Done, true));
    // 文件里也清掉了；新的编号接着排
    let (left, next) = st.saves();
    assert_eq!(left.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2, 3, 5]);
    assert_eq!(next, 6);
    // 暂停、失败的不算「找回」，到前台不自动下
    let log = Arc::new(Log::default());
    hub.set_listener(log.clone());
    hub.set_active(true);
    assert!(!log.has("restored"));
    // 忘掉一条：文件里也没了
    hub.forget_save(5);
    assert_eq!(st.saves().0.iter().map(|r| r.id).collect::<Vec<_>>(), vec![2, 3]);
    // 删掉这台电脑：它的记录全清
    hub.remove(dev.id.clone()).unwrap();
    assert!(hub.saves().is_empty() && st.saves().0.is_empty());
}
