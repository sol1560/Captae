//! 遥控电脑的端到端冒烟程序：用 captae-mobile 的 API（`RemoteHub`）连电脑上的测试版 Captae。在 Mac 上直接跑，或者编成 iOS 模拟器的程序用 `simctl spawn` 跑。
//! 分几步跑，中间停 / 开测试版、在电脑上撤销由外面的脚本做（work/mobile/m-remote-test/run.sh）：
//!
//! ```text
//! remote_smoke <目录> pair <二维码内容>            扫码配对
//! remote_smoke <目录> run <直链>                   连上 → 加任务 → 进度 → 暂停继续 → 下完 → 素材库 → 封面 → 存到手机（中途暂停再续）
//! remote_smoke <目录> queue <直链>                 电脑停了：加任务进「等电脑上线」
//! remote_smoke <目录> resend                       电脑开了：队列自动发出去，等它下完
//! remote_smoke <目录> revoked                      电脑上撤销以后：看到被撤销、令牌删掉、再发被拒
//! 选项：--platform ios-store|android（默认 ios-store）、--name <手机名>
//! ```
//!
//! 钥匙串用 `<目录>/fake-keychain.json` 代替（只是这个测试程序这么做；App 里是钥匙串 / Keystore，库本身不写令牌文件）。

use captae_mobile::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// 测试用的「钥匙串」：一个 JSON 文件
struct FileKeychain(PathBuf);

impl FileKeychain {
    fn load(&self) -> HashMap<String, String> {
        std::fs::read(&self.0).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
}

impl SecureStore for FileKeychain {
    fn read(&self, key: String) -> Option<String> {
        self.load().get(&key).cloned()
    }
    fn write(&self, key: String, value: String) -> bool {
        let mut m = self.load();
        m.insert(key, value);
        std::fs::write(&self.0, serde_json::to_vec_pretty(&m).unwrap()).is_ok()
    }
    fn delete(&self, key: String) {
        let mut m = self.load();
        m.remove(&key);
        let _ = std::fs::write(&self.0, serde_json::to_vec_pretty(&m).unwrap());
    }
}

/// 回调记下来，主线程轮询
struct Events {
    t0: Instant,
    jobs: Mutex<HashMap<u64, MobileJob>>,
    lines: Mutex<Vec<String>>,
    job_events: Mutex<u32>,
}

impl Events {
    fn say(&self, s: String) {
        println!("[{:6.2}s] {s}", self.t0.elapsed().as_secs_f64());
        self.lines.lock().unwrap().push(s);
    }
    fn has(&self, p: &str) -> bool {
        self.lines.lock().unwrap().iter().any(|l| l.contains(p))
    }
    fn job(&self, id: u64) -> Option<MobileJob> {
        self.jobs.lock().unwrap().get(&id).cloned()
    }
}

impl RemoteListener for Events {
    fn on_job(&self, j: DeviceJob) {
        *self.job_events.lock().unwrap() += 1;
        self.jobs.lock().unwrap().insert(j.job.id, j.job);
    }
    fn on_removed(&self, _d: String, id: u64) {
        self.say(format!("事件：任务 {id} 移除了"));
    }
    fn on_finished(&self, _d: String, id: u64, title: String, asset_id: Option<i64>) {
        self.say(format!("事件：任务 {id} 下完了：{title}（素材 {asset_id:?}）"));
    }
    fn on_library(&self, _d: String) {}
    fn on_resync(&self, d: String) {
        self.say(format!("事件：任务列表快照（{}）", &d[..8.min(d.len())]));
    }
    fn on_device(&self, d: MobileDevice) {
        self.say(format!("事件：设备 {} 状态 {:?}，正在下 {}，等上线 {}", d.name, d.status, d.running, d.waiting));
    }
    fn on_devices(&self) {}
    fn on_pending(&self) {
        self.say("事件：等电脑上线的队列变了".into());
    }
    fn on_pending_sent(&self, t: PendingTask, j: DeviceJob) {
        self.say(format!("事件：队列里的 {} 自动发出去了 → 电脑上的任务 {}（from {:?}）", t.url, j.job.id, j.from));
    }
    fn on_save(&self, s: SaveProgress) {
        if s.state != SaveState::Running {
            self.say(format!("事件：存到手机 {:?} {}/{} 字节", s.state, s.got_bytes, s.total_bytes));
        }
    }
    fn on_saves_restored(&self, saves: Vec<SaveProgress>) {
        self.say(format!("事件：找回 {} 个上次没存完的", saves.len()));
    }
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread().build().unwrap().block_on(f)
}

fn wait(ev: &Events, what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < until {
        if ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    ev.say(format!("失败：等了 {secs} 秒还没有：{what}"));
    std::process::exit(1);
}

fn check(ev: &Events, ok: bool, what: &str) {
    if !ok {
        ev.say(format!("失败：{what}"));
        std::process::exit(1);
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("用法：remote_smoke <目录> pair|run|queue|resend|revoked [参数] [--platform ios-store|android] [--name 手机名]");
        std::process::exit(2);
    }
    let dir = PathBuf::from(&args[1]);
    let step = args[2].as_str();
    let arg = args.get(3).filter(|a| !a.starts_with("--")).cloned().unwrap_or_default();
    let opt = |k: &str| args.iter().position(|a| a == k).and_then(|i| args.get(i + 1)).cloned();
    let platform = match opt("--platform").as_deref() {
        Some("android") => Platform::Android,
        _ => Platform::IosStore,
    };
    let name = opt("--name").unwrap_or_else(|| "Smoke iPhone".into());
    std::fs::create_dir_all(&dir).unwrap();

    let core = MobileCore::new(InitOptions {
        data_dir: dir.join("data").display().to_string(),
        downloads_dir: dir.join("dl").display().to_string(),
        platform,
        language: "zh-CN".into(),
        native_lib_dir: None,
    })
    .unwrap();
    let keychain = Arc::new(FileKeychain(dir.join("fake-keychain.json")));
    let hub = RemoteHub::new(core.clone(), RemoteOptions { phone_name: name.clone(), cache_dir: dir.join("cache").display().to_string() }, keychain.clone()).unwrap();
    let ev = Arc::new(Events { t0: Instant::now(), jobs: Default::default(), lines: Default::default(), job_events: Mutex::new(0) });
    hub.set_listener(ev.clone());
    let info = core.info();
    ev.say(format!("平台 {} / {}，手机名 {name}，步骤 {step}", info.platform, std::env::consts::ARCH));
    let computer = || hub.devices().into_iter().find(|d| !d.local);
    let status = |fp: &str| hub.device(fp.to_string()).map(|d| d.status);

    match step {
        "pair" => {
            let link = hub.parse_pair_link(arg.clone());
            check(&ev, link.is_some(), "二维码认不出来");
            let link = link.unwrap();
            ev.say(format!("二维码：{}，地址 {:?}:{}，指纹 {}…", link.name, link.hosts, link.port, &link.fingerprint[..10]));
            let d = block(hub.pair_qr(arg.clone())).unwrap_or_else(|e| {
                ev.say(format!("失败：配对 {} {}", e.detail().code, e.detail().message));
                std::process::exit(1)
            });
            let token = keychain.read(format!("captae.remote.token.{}", d.id)).unwrap_or_default();
            let file = std::fs::read_to_string(dir.join("data/remote/computers.json")).unwrap_or_default();
            ev.say(format!("配对成功：{}（{}…），令牌 {} 个字符在「钥匙串」里，computers.json 里有令牌：{}", d.name, &d.id[..10], token.len(), file.contains(&token)));
            check(&ev, token.len() == 43 && !file.contains(&token), "令牌没按规矩存");
            let again = block(hub.pair_qr(arg));
            ev.say(format!("同一个二维码再扫：{}", again.err().map(|e| e.detail().code.clone()).unwrap_or("成功了？".into())));
        }
        "run" => {
            let d = computer().expect("先配对");
            hub.set_active(true);
            wait(&ev, "电脑在线", 30, || status(&d.id) == Some(DeviceStatus::Online));
            let r = block(hub.send(d.id.clone(), SendOptions { url: arg.clone(), ..Default::default() })).unwrap_or_else(|e| {
                ev.say(format!("失败：加任务 {} {}", e.detail().code, e.detail().message));
                std::process::exit(1)
            });
            let SendResult::Sent { job } = r else {
                ev.say("失败：电脑在线却进了队列".into());
                std::process::exit(1)
            };
            let id = job.id;
            ev.say(format!("加任务：电脑上的任务 {id}，{}", job.url));
            wait(&ev, "进度", 30, || ev.job(id).is_some_and(|j| j.got_bytes > 0 && j.state == MobileJobState::Running));
            let j = ev.job(id).unwrap();
            ev.say(format!("进度：{:.1} / {:.1} MB，{:.1} MB/s", j.got_bytes as f64 / 1e6, j.total_bytes as f64 / 1e6, j.speed as f64 / 1e6));
            block(hub.remote_action(d.id.clone(), id, MobileAction::Pause)).unwrap();
            wait(&ev, "暂停", 10, || ev.job(id).is_some_and(|j| j.state == MobileJobState::Paused));
            let at = ev.job(id).unwrap().got_bytes;
            std::thread::sleep(Duration::from_secs(2));
            let jobs = block(hub.refresh_remote_jobs(d.id.clone())).unwrap();
            let still = jobs.iter().find(|j| j.id == id).unwrap();
            ev.say(format!("暂停：停在 {:.1} MB，2 秒后电脑上 {:?}、{:.1} MB", at as f64 / 1e6, still.state, still.got_bytes as f64 / 1e6));
            check(&ev, still.state == MobileJobState::Paused, "暂停后又在跑");
            block(hub.remote_action(d.id.clone(), id, MobileAction::Resume)).unwrap();
            ev.say("继续".into());
            wait(&ev, "下完", 180, || ev.has(&format!("任务 {id} 下完了")));
            let done = ev.job(id).unwrap();
            ev.say(format!("下完：{:?}，素材 {:?}，共收到 {} 条任务事件", done.state, done.asset_id, ev.job_events.lock().unwrap()));
            let log = hub.remote_job_log(d.id.clone(), id);
            ev.say(format!("下载记录第一行：{}", log.first().map(|l| l.text.as_str()).unwrap_or("")));

            let secs = block(hub.merged_library(LibraryQuery { limit: 5, ..Default::default() }, None));
            for s in &secs {
                ev.say(format!("素材库分段：{}（{:?}）共 {} 条，错误 {:?}", s.device.name, s.device.status, s.total, s.error.as_ref().map(|e| &e.code)));
            }
            let a = secs[0].items.iter().find(|a| Some(a.asset.id) == done.asset_id).map(|a| a.asset.clone());
            check(&ev, a.is_some(), "电脑素材库里没有刚下完的");
            let a = a.unwrap();
            ev.say(format!("素材 {}：{} [{}] {} 字节，path 是空的：{}", a.id, a.title, a.kind, a.bytes, a.path.is_empty()));
            let thumb = block(hub.remote_thumbnail(d.id.clone(), a.id, false)).unwrap();
            ev.say(format!("封面：{:?}（{} 字节）", thumb, thumb.as_ref().and_then(|t| std::fs::metadata(t).ok()).map(|m| m.len()).unwrap_or(0)));

            let dest = dir.join("saved").join(format!("asset-{}.mp4", a.id));
            let _ = std::fs::remove_file(&dest);
            let s = hub.save_to_phone(d.id.clone(), a.id, dest.display().to_string()).unwrap();
            let size = |p: &Path| std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
            let until = Instant::now() + Duration::from_secs(20);
            while size(&dest) == 0 && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(1));
            }
            hub.pause_save(s.id).unwrap();
            // 还在路上的那一笔写入落盘以后再量
            std::thread::sleep(Duration::from_millis(300));
            let p1 = size(&dest);
            std::thread::sleep(Duration::from_millis(800));
            let p2 = size(&dest);
            ev.say(format!("存到手机：暂停在 {p1} 字节，0.8 秒后 {p2} 字节（共 {}）", a.bytes));
            check(&ev, p1 > 0 && p1 == p2 && p2 < a.bytes, "没在中途停住（文件太小或者暂停没生效）");
            hub.resume_save(s.id).unwrap();
            wait(&ev, "存完", 60, || hub.saves().iter().any(|x| x.id == s.id && x.state == SaveState::Done));
            ev.say(format!("存完：{} 字节 → {}", size(&dest), dest.display()));
            check(&ev, size(&dest) == a.bytes, "大小不对");
            hub.set_active(false);
        }
        "queue" => {
            let d = computer().expect("先配对");
            hub.set_active(true);
            wait(&ev, "看到电脑离线", 30, || status(&d.id) == Some(DeviceStatus::Offline));
            let r = block(hub.send(d.id.clone(), SendOptions { url: arg.clone(), title: Some("离线时加的".into()), ..Default::default() })).unwrap();
            match r {
                SendResult::Queued { task } => ev.say(format!("进队列：{} {}，能改到手机上下：{}", task.id, task.url, task.can_run_here)),
                SendResult::Sent { .. } => {
                    ev.say("失败：电脑停了却发出去了".into());
                    std::process::exit(1)
                }
            }
            let file = std::fs::read_to_string(dir.join("data/remote/pending.json")).unwrap_or_default();
            ev.say(format!("pending.json 里有这条：{}；设备上等上线的 {} 条", file.contains(&arg), hub.device(d.id.clone()).unwrap().waiting));
            hub.set_active(false);
        }
        "resend" => {
            let d = computer().expect("先配对");
            let before = hub.pending();
            ev.say(format!("队列里 {} 条，打开连接", before.len()));
            check(&ev, !before.is_empty(), "队列是空的");
            hub.set_active(true);
            wait(&ev, "自动发出去", 40, || hub.pending().is_empty() && ev.has("自动发出去了"));
            let id = ev.lines.lock().unwrap().iter().find(|l| l.contains("自动发出去了")).and_then(|l| l.split("电脑上的任务 ").nth(1)).and_then(|x| x.split('（').next()).and_then(|x| x.trim().parse::<u64>().ok());
            if let Some(id) = id {
                wait(&ev, "队列发的任务下完", 180, || ev.has(&format!("任务 {id} 下完了")));
            }
            let jobs = block(hub.refresh_remote_jobs(d.id.clone())).unwrap();
            ev.say(format!("电脑上现在 {} 个任务", jobs.len()));
            hub.set_active(false);
        }
        "revoked" => {
            let d = computer().expect("先配对");
            hub.set_active(true);
            wait(&ev, "看到被撤销", 30, || status(&d.id) == Some(DeviceStatus::Revoked));
            let token = keychain.read(format!("captae.remote.token.{}", d.id));
            ev.say(format!("被撤销：钥匙串里的令牌还在：{}", token.is_some()));
            let e = block(hub.send(d.id.clone(), SendOptions { url: arg.clone(), ..Default::default() })).err();
            ev.say(format!("再发任务：{:?}", e.as_ref().map(|e| (e.detail().code.clone(), e.detail().message.clone()))));
            check(&ev, e.is_some_and(|e| e.detail().code == "unauthorized") && token.is_none() && hub.pending().is_empty(), "撤销后没被拒");
            hub.set_active(false);
        }
        _ => {
            eprintln!("不认识的步骤 {step}");
            std::process::exit(2);
        }
    }
    println!("SMOKE OK {step}");
}
