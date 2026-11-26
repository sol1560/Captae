//! 遥控电脑：把 captae-remote 的客户端包给 Swift / Kotlin（`RemoteHub`）。接入说明见 crates/captae-mobile/REMOTE.md。
//!
//! - 配对：扫码（`pair_qr`）或者发现 + 6 位数（`pair_code`，SPAKE2）。令牌只经 App 给的 `SecureStore` 存进钥匙串 / Keystore，
//!   其余（名字、地址、端口、时间）存 `<数据目录>/remote/computers.json`。
//! - 连接：`set_active(true)` 以后每台电脑一个 WebSocket 订阅，按配对时的指纹钉死证书；断了自动重连（最多 15 秒找一次），
//!   App 发现到新地址（`discovered`）或回到前台时马上再找。被撤销了就删令牌、不再连。
//! - 电脑上的任务：事件来了先更新缓存（`remote_jobs` 直接读），再回调 `RemoteListener`；操作、选清晰度、选条目、读取信息、素材库、封面。
//! - 「等电脑上线」：电脑不在线时 `send` 把任务存进 `<数据目录>/remote/pending.json`，连上以后自动按顺序发出去（`on_pending_sent`）；
//!   能改成在手机上下（`move_to_phone`，iOS 上架版只改得动直链和 M3U8）。
//! - 存到手机：`save_to_phone` 断点续传下到 App 给的路径，可以暂停、继续、取消，进度回调 `on_save`。
//! - 合并视图：`devices`、`merged_jobs`、`merged_library`，每条带设备 id（这台手机是 `"phone"`，电脑是证书指纹）。
//!
//! 所有联网的事都在 `MobileCore` 的 tokio Runtime 上跑（UniFFI 的 async 由 Swift / Kotlin 那边驱动，不在 tokio 里）。
//! 锁里不调回调、不联网。

mod convert;
mod store;
mod types;

#[cfg(test)]
mod tests;

pub use types::*;

use crate::types::code_str;
use crate::{AbortOnDrop, AddOptions, CancelToken, LibraryPage, LibraryQuery, MobileAction, MobileAsset, MobileCore, MobileError, MobileFailure, MobileJob, MobileLogLine, MobileProbe};
use captae_core::tr;
use captae_remote::client::{self as rc, Client, Credentials, RemoteEvent};
use captae_remote::proto::{AddJob, LibQuery, PairLink, PhoneHello};
use convert::code;
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};
use store::{token_key, Computer, Pending, SaveRec};
use tokio::task::JoinHandle;

/// 断线以后最多隔多久再找一次（PLAN 5.3：App 在前台时每 15 秒找一次电脑）
const RETRY_MAX: Duration = Duration::from_secs(15);
/// 存到手机的进度最多多久报一次
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
/// 「存到手机」下完的记录留多久（界面显示「已存到手机」用）
const SAVE_KEEP_MS: i64 = 7 * 24 * 3600 * 1000;

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// 这台手机在合并视图里的设备 id（`"phone"`）
#[uniffi::export]
pub fn local_device_id() -> String {
    LOCAL_DEVICE.to_string()
}

/// `RemoteHub::new` 的参数
#[derive(Debug, Clone, uniffi::Record)]
pub struct RemoteOptions {
    /// 手机名：电脑上「已配对的手机」和下载列表的「来自 iPhone」用它（iOS 的 `UIDevice.name`，Android 的设备名）
    pub phone_name: String,
    /// App 的缓存目录（电脑素材的封面缓存在 `<缓存目录>/captae-remote-thumbs/` 下，系统清掉了也没关系）
    pub cache_dir: String,
}

/// 一台配对过的电脑在内存里的样子
struct Conn {
    info: Computer,
    status: DeviceStatus,
    client: Option<Arc<Client>>,
    task: Option<JoinHandle<()>>,
    /// 电脑上的任务（电脑推来的 JSON），按 id
    jobs: BTreeMap<u64, Value>,
    /// 正在往这台电脑发队列里的任务
    flushing: bool,
}

impl Conn {
    fn stop(&mut self) {
        if let Some(t) = self.task.take() {
            t.abort();
        }
        self.client = None;
    }
    fn running(&self) -> u32 {
        self.jobs.values().filter(|j| j["state"] == "run").count() as u32
    }
}

struct SaveSlot {
    p: SaveProgress,
    task: Option<JoinHandle<()>>,
    created_at: i64,
}

#[derive(Default)]
struct State {
    comps: Vec<Conn>,
    pending: Vec<Pending>,
    next_pending: u64,
    nearby: Vec<NearbyComputer>,
    saves: BTreeMap<u64, SaveSlot>,
    next_save: u64,
    /// 重开后找回的、上次没存完的（第一次 set_active(true) 时发 on_saves_restored）
    restored: Vec<u64>,
}

impl State {
    fn find(&self, fp: &str) -> Option<&Conn> {
        self.comps.iter().find(|c| c.info.fp == fp)
    }
    fn find_mut(&mut self, fp: &str) -> Option<&mut Conn> {
        self.comps.iter_mut().find(|c| c.info.fp == fp)
    }
    fn waiting(&self, fp: &str) -> u32 {
        self.pending.iter().filter(|p| p.device == fp).count() as u32
    }
    fn device(&self, c: &Conn) -> MobileDevice {
        let i = &c.info;
        MobileDevice {
            id: i.fp.clone(),
            name: i.nickname.clone().filter(|n| !n.trim().is_empty()).unwrap_or_else(|| i.name.clone()),
            computer_name: i.name.clone(),
            local: false,
            status: c.status,
            last_seen: i.last_seen,
            last_used: i.last_used,
            paired_at: Some(i.paired_at),
            running: if c.status == DeviceStatus::Online { c.running() } else { 0 },
            waiting: self.waiting(&i.fp),
            hosts: i.hosts.clone(),
            port: i.port,
        }
    }
}

/// 遥控电脑。App 启动时在 `MobileCore` 之后建一个、一直用着
#[derive(uniffi::Object)]
pub struct RemoteHub {
    me: Weak<RemoteHub>,
    core: Arc<MobileCore>,
    secure: Arc<dyn SecureStore>,
    phone: PhoneHello,
    cache: PathBuf,
    store: store::Store,
    state: Mutex<State>,
    listener: Mutex<Option<Arc<dyn RemoteListener>>>,
    active: AtomicBool,
}

impl Drop for RemoteHub {
    fn drop(&mut self) {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        for c in st.comps.iter_mut() {
            c.stop();
        }
        for s in st.saves.values_mut() {
            if let Some(t) = s.task.take() {
                t.abort();
            }
        }
    }
}

fn link_info(l: &PairLink, known: bool) -> PairLinkInfo {
    PairLinkInfo { name: l.name.clone(), hosts: l.hosts.clone(), port: l.port, fingerprint: l.fp.clone(), known }
}

fn valid_url(url: &str) -> bool {
    url::Url::parse(url.trim()).is_ok_and(|u| matches!(u.scheme(), "http" | "https") && u.host_str().is_some())
}

fn no_device() -> MobileError {
    convert::coded(code::NO_DEVICE, tr!("没有这台电脑，可能已经删掉了"))
}

fn needs_pairing() -> MobileError {
    convert::coded(code::NEEDS_PAIRING, tr!("这台电脑要重新配对"))
}

fn revoked() -> MobileError {
    convert::coded(code::UNAUTHORIZED, tr!("这台手机在电脑上被撤销了，要重新配对"))
}

fn no_task() -> MobileError {
    convert::coded(code::NO_TASK, tr!("没有这一条了"))
}

fn matches(search: &Option<String>, fields: &[&str]) -> bool {
    match search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None => true,
        Some(q) => {
            let q = q.to_lowercase();
            fields.iter().any(|f| f.to_lowercase().contains(&q))
        }
    }
}

impl RemoteHub {
    fn listener(&self) -> Option<Arc<dyn RemoteListener>> {
        self.listener.lock().unwrap().clone()
    }

    fn emit(&self, f: impl FnOnce(&dyn RemoteListener)) {
        if let Some(l) = self.listener() {
            f(l.as_ref());
        }
    }

    fn save_computers(&self, st: &State) {
        let list: Vec<Computer> = st.comps.iter().map(|c| c.info.clone()).collect();
        if let Err(e) = self.store.save_computers(&list) {
            tracing::warn!("写 computers.json 失败：{e}");
        }
    }

    fn save_pending(&self, st: &State) {
        if let Err(e) = self.store.save_pending(&st.pending, st.next_pending) {
            tracing::warn!("写 pending.json 失败：{e}");
        }
    }

    fn pending_task(&self, p: &Pending) -> PendingTask {
        PendingTask {
            id: p.id,
            device: p.device.clone(),
            url: p.url.clone(),
            title: p.title.clone(),
            quality: p.quality.clone(),
            format: p.format.clone(),
            audio_only: p.audio_only,
            preset: p.preset.clone(),
            created_at: p.created_at,
            error: p.error_code.as_ref().map(|c| MobileFailure::coded(c, p.error_message.clone().unwrap_or_default(), None)),
            can_run_here: self.core.store_rule(&captae_core::detect::canonical(p.url.trim())).is_ok(),
        }
    }

    fn local_device(&self) -> MobileDevice {
        let running = self.core.core.jobs().iter().filter(|j| j.state == captae_core::JobState::Run).count() as u32;
        MobileDevice {
            id: LOCAL_DEVICE.into(),
            name: self.phone.name.clone(),
            computer_name: String::new(),
            local: true,
            status: DeviceStatus::Local,
            last_seen: None,
            last_used: None,
            paired_at: None,
            running,
            waiting: 0,
            hosts: vec![],
            port: 0,
        }
    }

    fn device_now(&self, fp: &str) -> Option<MobileDevice> {
        let st = self.state.lock().unwrap();
        st.find(fp).map(|c| st.device(c))
    }

    fn emit_device(&self, fp: &str) {
        if let Some(d) = self.device_now(fp) {
            self.emit(|l| l.on_device(d));
        }
    }

    /// 在 Runtime 上跑一个联网的 future，等它的结果（调用方放弃等待时它也停下）
    async fn on_rt<T: Send + 'static>(&self, fut: impl Future<Output = anyhow::Result<T>> + Send + 'static) -> anyhow::Result<T> {
        let mut t = AbortOnDrop(self.core.rt.spawn(fut));
        (&mut t.0).await.map_err(|e| anyhow::anyhow!("remote task: {e}"))?
    }

    /// 配对信息（令牌从安全存储里读）
    fn credentials(&self, fp: &str) -> Result<Credentials, MobileError> {
        let info = {
            let st = self.state.lock().unwrap();
            let c = st.find(fp).ok_or_else(no_device)?;
            if c.info.revoked {
                return Err(revoked());
            }
            c.info.clone()
        };
        let token = self.secure.read(token_key(fp)).filter(|t| !t.is_empty()).ok_or_else(needs_pairing)?;
        Ok(Credentials { name: info.name, fp: info.fp, hosts: info.hosts, port: info.port, token, device: info.device })
    }

    /// 调接口用的客户端：连着就用连着的那个，没连（App 在后台、还没 `set_active`）就临时建一个
    fn client_for(&self, fp: &str) -> Result<Arc<Client>, MobileError> {
        {
            let st = self.state.lock().unwrap();
            let c = st.find(fp).ok_or_else(no_device)?;
            if c.info.revoked {
                return Err(revoked());
            }
            if let Some(cl) = &c.client {
                return Ok(cl.clone());
            }
        }
        let creds = match self.credentials(fp) {
            Ok(c) => c,
            Err(e) => {
                if convert::code_of(&e) == code::NEEDS_PAIRING {
                    self.set_status(fp, DeviceStatus::NeedsPairing);
                }
                return Err(e);
            }
        };
        let _g = self.core.rt.enter();
        Client::new(creds).map(Arc::new).map_err(|e| convert::remote(&e))
    }

    /// 联网出错：被撤销了就记下、删令牌
    fn failed(&self, fp: &str, e: &anyhow::Error) -> MobileError {
        let m = convert::remote(e);
        if convert::code_of(&m) == code::UNAUTHORIZED {
            self.mark_revoked(fp);
        }
        m
    }

    /// 调一个电脑上的接口
    async fn call<T, Fut>(&self, fp: &str, f: impl FnOnce(Arc<Client>) -> Fut) -> Result<T, MobileError>
    where
        T: Send + 'static,
        Fut: Future<Output = anyhow::Result<T>> + Send + 'static,
    {
        let client = self.client_for(fp)?;
        self.on_rt(f(client)).await.map_err(|e| self.failed(fp, &e))
    }

    fn set_status(&self, fp: &str, s: DeviceStatus) {
        let changed = {
            let mut st = self.state.lock().unwrap();
            match st.find_mut(fp) {
                Some(c) if c.status != s => {
                    c.status = s;
                    true
                }
                _ => false,
            }
        };
        if changed {
            self.emit_device(fp);
        }
    }

    /// 在电脑上被撤销了：删令牌、断开、清掉电脑上任务的缓存，等用户重新配对或删掉
    fn mark_revoked(&self, fp: &str) {
        let changed = {
            let mut st = self.state.lock().unwrap();
            let Some(c) = st.find_mut(fp) else { return };
            let was = c.info.revoked;
            c.info.revoked = true;
            c.status = DeviceStatus::Revoked;
            c.stop();
            c.jobs.clear();
            self.save_computers(&st);
            !was
        };
        self.secure.delete(token_key(fp));
        if changed {
            tracing::info!("这台手机在电脑上被撤销了：{fp}");
            self.emit_device(fp);
            self.emit(|l| l.on_resync(fp.to_string()));
        }
    }

    /// 开始连一台电脑（已经在连的先停掉重来：马上找一次，不等重连的间隔）
    fn start(&self, fp: &str) {
        let creds = {
            let st = self.state.lock().unwrap();
            match st.find(fp) {
                Some(c) if !c.info.revoked => {}
                _ => return,
            }
            drop(st);
            match self.credentials(fp) {
                Ok(c) => c,
                Err(_) => {
                    self.set_status(fp, DeviceStatus::NeedsPairing);
                    return;
                }
            }
        };
        let client = {
            let _g = self.core.rt.enter();
            match Client::new(creds) {
                Ok(c) => Arc::new(c),
                Err(e) => {
                    tracing::warn!("建远程客户端失败：{e:#}");
                    return;
                }
            }
        };
        {
            let mut st = self.state.lock().unwrap();
            let Some(c) = st.find_mut(fp) else { return };
            c.stop();
            c.client = Some(client.clone());
            c.status = DeviceStatus::Connecting;
        }
        let (weak, fp2, cl) = (self.me.clone(), fp.to_string(), client.clone());
        let task = self.core.rt.spawn(async move {
            let mut sub = cl.subscribe_every(RETRY_MAX);
            while let Some(ev) = sub.recv().await {
                let Some(hub) = weak.upgrade() else { return };
                hub.handle(&fp2, &cl, ev);
            }
        });
        {
            let mut st = self.state.lock().unwrap();
            match st.find_mut(fp) {
                Some(c) if c.client.as_ref().is_some_and(|x| Arc::ptr_eq(x, &client)) => c.task = Some(task),
                _ => task.abort(),
            }
        }
        self.emit_device(fp);
    }

    /// 订阅来的一条事件。不是当前这个客户端的（已经换掉了）不管
    fn handle(&self, fp: &str, client: &Arc<Client>, ev: RemoteEvent) {
        let current = {
            let st = self.state.lock().unwrap();
            st.find(fp).and_then(|c| c.client.as_ref()).is_some_and(|x| Arc::ptr_eq(x, client))
        };
        if !current {
            return;
        }
        let device = fp.to_string();
        match ev {
            RemoteEvent::Connected => {
                {
                    let mut st = self.state.lock().unwrap();
                    if let Some(c) = st.find_mut(fp) {
                        c.status = DeviceStatus::Online;
                        let creds = client.credentials();
                        c.info.hosts = creds.hosts;
                        c.info.port = creds.port;
                        c.info.last_seen = Some(now_ms());
                    }
                    self.save_computers(&st);
                }
                self.emit_device(fp);
                self.flush(fp);
                self.resume_restored(fp);
            }
            RemoteEvent::Jobs(list) => {
                {
                    let mut st = self.state.lock().unwrap();
                    if let Some(c) = st.find_mut(fp) {
                        c.jobs = list.into_iter().filter_map(|v| Some((v["id"].as_u64()?, v))).collect();
                    }
                }
                self.emit(|l| l.on_resync(device));
                self.emit_device(fp);
            }
            RemoteEvent::Job(v) => {
                let Some(job) = convert::job(&v) else { return };
                let from = convert::job_from(&v);
                let changed = {
                    let mut st = self.state.lock().unwrap();
                    let Some(c) = st.find_mut(fp) else { return };
                    let before = c.running();
                    c.jobs.insert(job.id, v);
                    c.running() != before
                };
                self.emit(|l| l.on_job(DeviceJob { device, job, from }));
                if changed {
                    self.emit_device(fp);
                }
            }
            RemoteEvent::JobRemoved(id) => {
                {
                    let mut st = self.state.lock().unwrap();
                    if let Some(c) = st.find_mut(fp) {
                        c.jobs.remove(&id);
                    }
                }
                self.emit(|l| l.on_removed(device, id));
                self.emit_device(fp);
            }
            RemoteEvent::Finished { id, title, asset_id } => self.emit(|l| l.on_finished(device, id, title, asset_id)),
            RemoteEvent::Library => self.emit(|l| l.on_library(device)),
            RemoteEvent::Disconnected { reason, .. } => {
                tracing::debug!("和电脑断开了（{fp}）：{reason}");
                let changed = {
                    let mut st = self.state.lock().unwrap();
                    let Some(c) = st.find_mut(fp) else { return };
                    if c.status == DeviceStatus::Online {
                        c.info.last_seen = Some(now_ms());
                    }
                    let changed = c.status != DeviceStatus::Offline;
                    c.status = DeviceStatus::Offline;
                    if changed {
                        self.save_computers(&st);
                    }
                    changed
                };
                if changed {
                    self.emit_device(fp);
                }
            }
            RemoteEvent::Revoked => self.mark_revoked(fp),
        }
    }

    /// 把队列里发给这台电脑的任务按顺序发出去（连上时、重试时）。同一台电脑同时只有一个在发
    fn flush(&self, fp: &str) {
        {
            let mut st = self.state.lock().unwrap();
            let Some(c) = st.find_mut(fp) else { return };
            if c.flushing || c.status != DeviceStatus::Online || c.client.is_none() {
                return;
            }
            c.flushing = true;
        }
        let (weak, fp) = (self.me.clone(), fp.to_string());
        self.core.rt.spawn(async move {
            loop {
                let Some(hub) = weak.upgrade() else { return };
                let next = {
                    let st = hub.state.lock().unwrap();
                    let c = st.find(&fp);
                    let client = c.filter(|c| c.status == DeviceStatus::Online).and_then(|c| c.client.clone());
                    let task = st.pending.iter().find(|p| p.device == fp && p.error_code.is_none()).cloned();
                    client.zip(task)
                };
                let Some((client, task)) = next else { break };
                let add = AddJob { url: task.url.clone(), quality: task.quality.clone(), format: task.format.clone(), audio_only: task.audio_only, preset: task.preset.clone() };
                match client.add(&add).await {
                    Ok(v) => {
                        let sent = hub.pending_task(&task);
                        {
                            let mut st = hub.state.lock().unwrap();
                            st.pending.retain(|p| p.id != task.id);
                            if let Some(c) = st.find_mut(&fp) {
                                c.info.last_used = Some(now_ms());
                            }
                            hub.save_pending(&st);
                            hub.save_computers(&st);
                        }
                        tracing::info!("等电脑上线的任务发出去了：{}", task.url);
                        if let Some(job) = convert::job(&v) {
                            let dj = DeviceJob { device: fp.clone(), from: convert::job_from(&v), job };
                            hub.emit(|l| l.on_pending_sent(sent, dj));
                        }
                        hub.emit(|l| l.on_pending());
                        hub.emit_device(&fp);
                    }
                    Err(e) => {
                        let m = convert::remote(&e);
                        // 又断了（连不上、证书不对，或者不是电脑回的错）：留着，下次连上再发
                        let lost = matches!(convert::code_of(&m), code::UNREACHABLE | code::CERT_MISMATCH) || rc::remote_error(&e).is_none();
                        match convert::code_of(&m) {
                            _ if lost => break,
                            code::UNAUTHORIZED => {
                                hub.mark_revoked(&fp);
                                break;
                            }
                            // 电脑收到了但不接受（链接不对、白名单外）：记下错误，不再自动发
                            _ => {
                                {
                                    let mut st = hub.state.lock().unwrap();
                                    if let Some(p) = st.pending.iter_mut().find(|p| p.id == task.id) {
                                        p.error_code = Some(m.detail().code.clone());
                                        p.error_message = Some(m.detail().message.clone());
                                    }
                                    hub.save_pending(&st);
                                }
                                hub.emit(|l| l.on_pending());
                            }
                        }
                    }
                }
            }
            if let Some(hub) = weak.upgrade() {
                let mut st = hub.state.lock().unwrap();
                if let Some(c) = st.find_mut(&fp) {
                    c.flushing = false;
                }
            }
        });
    }

    /// 配对成功：令牌进安全存储，其余存 computers.json，开始连
    fn add_paired(&self, creds: Credentials) -> Result<MobileDevice, MobileError> {
        if !self.secure.write(token_key(&creds.fp), creds.token.clone()) {
            return Err(convert::coded(code::NEEDS_PAIRING, tr!("配对成功了，但是存不进手机的安全存储，再配对一次试试")));
        }
        let fp = creds.fp.clone();
        let fresh = {
            let mut st = self.state.lock().unwrap();
            st.nearby.retain(|n| n.fingerprint.as_deref() != Some(fp.as_str()));
            let fresh = st.find(&fp).is_none();
            if fresh {
                st.comps.push(Conn { info: Computer { fp: fp.clone(), ..Default::default() }, status: DeviceStatus::Idle, client: None, task: None, jobs: BTreeMap::new(), flushing: false });
            }
            let c = st.find_mut(&fp).expect("刚放进去");
            c.stop();
            c.info.name = creds.name.clone();
            c.info.hosts = creds.hosts.clone();
            c.info.port = creds.port;
            c.info.device = creds.device.clone();
            c.info.paired_at = now_ms();
            c.info.revoked = false;
            c.status = DeviceStatus::Idle;
            self.save_computers(&st);
            fresh
        };
        tracing::info!("配对了一台电脑：{}（新的：{fresh}）", creds.name);
        if self.active.load(Ordering::SeqCst) {
            self.start(&fp);
        }
        self.emit(|l| l.on_devices());
        self.device_now(&fp).ok_or_else(no_device)
    }

    /// 存到手机：开始（或接着）下。设状态、起任务、记句柄在同一个锁里做，任务结束得再快也不会被改回「在下」
    fn run_save(&self, id: u64) -> Result<(), MobileError> {
        let (fp, asset, path) = {
            let st = self.state.lock().unwrap();
            let s = st.saves.get(&id).ok_or_else(no_task)?;
            (s.p.device.clone(), s.p.asset_id, PathBuf::from(&s.p.path))
        };
        let client = self.client_for(&fp)?;
        let weak = self.me.clone();
        let mut st = self.state.lock().unwrap();
        let Some(slot) = st.saves.get_mut(&id) else { return Err(no_task()) };
        slot.p.state = SaveState::Running;
        slot.p.error = None;
        slot.p.done_at = None;
        let task = self.core.rt.spawn(async move {
            let mut last = Instant::now() - PROGRESS_EVERY;
            let mut mark: Option<(Instant, u64)> = None;
            let w = weak.clone();
            let r = client
                .download_to(asset, &path, move |got, total| {
                    let now = Instant::now();
                    let (t0, g0) = *mark.get_or_insert((now, got));
                    if now.duration_since(last) < PROGRESS_EVERY && got != total {
                        return;
                    }
                    last = now;
                    let secs = now.duration_since(t0).as_secs_f64();
                    let speed = if secs > 0.2 { ((got.saturating_sub(g0)) as f64 / secs) as u64 } else { 0 };
                    if let Some(h) = w.upgrade() {
                        h.save_update(id, |p| {
                            p.got_bytes = got;
                            p.total_bytes = total;
                            p.speed = speed;
                        });
                    }
                })
                .await;
            let Some(hub) = weak.upgrade() else { return };
            match r {
                Ok(n) => {
                    // 进这台手机的素材库（带着电脑素材库里的信息）。导入失败不算存失败：文件已经在手机上了
                    let local = hub.import_saved(&client, &fp, asset, &path).await;
                    hub.save_update(id, |p| {
                        p.got_bytes = n;
                        p.total_bytes = n;
                        p.speed = 0;
                        p.state = SaveState::Done;
                        p.error = None;
                        p.done_at = Some(now_ms());
                        p.local_asset_id = local;
                    });
                }
                Err(e) => {
                    let m = hub.failed(&fp, &e);
                    hub.save_update(id, |p| {
                        p.speed = 0;
                        p.state = SaveState::Failed;
                        p.error = Some(m.detail().clone());
                    });
                }
            }
            let mut st = hub.state.lock().unwrap();
            if let Some(s) = st.saves.get_mut(&id) {
                s.task = None;
            }
            hub.persist_saves(&st);
        });
        if let Some(old) = slot.task.replace(task) {
            old.abort();
        }
        self.persist_saves(&st);
        Ok(())
    }

    /// 存到手机下完：向电脑要这条素材的信息（标题、来源、时长……）和封面，导入这台手机的素材库，返回手机素材库里的 id。
    /// 电脑那边要不到信息（中途断了）也照样导入，标题用文件名
    async fn import_saved(&self, client: &Arc<Client>, fp: &str, asset: i64, path: &Path) -> Option<i64> {
        let meta = client.asset(asset).await.ok();
        let mut thumb = self.cached_thumb(fp, asset);
        if thumb.is_none() && meta.as_ref().is_some_and(|m| m["hasThumb"].as_bool() == Some(true)) {
            if let Ok(bytes) = client.thumb(asset).await {
                let p = self.thumb_path(fp, asset);
                if p.parent().is_some_and(|d| std::fs::create_dir_all(d).is_ok()) && std::fs::write(&p, bytes).is_ok() {
                    thumb = Some(p.display().to_string());
                }
            }
        }
        let info = meta.as_ref().map(|m| convert::import_info(m, thumb.clone())).unwrap_or(crate::ImportInfo { thumb_path: thumb, ..Default::default() });
        let (core, path) = (self.core.clone(), path.to_path_buf());
        match tokio::task::spawn_blocking(move || core.import_path(&path, info)).await {
            Ok(Ok(a)) => Some(a.id),
            Ok(Err(e)) => {
                tracing::warn!("存到手机以后导入素材库失败：{}", e.detail().message);
                None
            }
            Err(e) => {
                tracing::warn!("存到手机以后导入素材库失败：{e}");
                None
            }
        }
    }

    /// 存到手机的记录写盘：取消了的不存，下完超过 7 天的清掉
    fn persist_saves(&self, st: &State) {
        let cutoff = now_ms() - SAVE_KEEP_MS;
        let recs: Vec<SaveRec> = st
            .saves
            .values()
            .filter_map(|s| {
                let p = &s.p;
                let state = match p.state {
                    SaveState::Running => "running",
                    SaveState::Paused => "paused",
                    SaveState::Done => "done",
                    SaveState::Failed => "failed",
                    SaveState::Cancelled => return None,
                };
                if p.state == SaveState::Done && p.done_at.unwrap_or(0) < cutoff {
                    return None;
                }
                Some(SaveRec {
                    id: p.id,
                    device: p.device.clone(),
                    asset_id: p.asset_id,
                    path: p.path.clone(),
                    got: p.got_bytes,
                    total: p.total_bytes,
                    state: state.into(),
                    created_at: s.created_at,
                    done_at: p.done_at,
                    error_code: p.error.as_ref().map(|e| e.code.clone()),
                    error_message: p.error.as_ref().map(|e| e.message.clone()),
                    local_asset_id: p.local_asset_id,
                })
            })
            .collect();
        if let Err(e) = self.store.save_saves(&recs, st.next_save) {
            tracing::warn!("写 saves.json 失败：{e}");
        }
    }

    /// 这台电脑连上了：接着下重开后找回的、还没开始的「存到手机」
    fn resume_restored(&self, fp: &str) {
        let ids: Vec<u64> = {
            let st = self.state.lock().unwrap();
            st.saves.values().filter(|s| s.p.device == fp && s.p.state == SaveState::Running && s.task.is_none()).map(|s| s.p.id).collect()
        };
        for id in ids {
            if let Err(e) = self.run_save(id) {
                let p = {
                    let mut st = self.state.lock().unwrap();
                    let Some(s) = st.saves.get_mut(&id) else { continue };
                    s.p.state = SaveState::Failed;
                    s.p.error = Some(e.detail().clone());
                    let p = s.p.clone();
                    self.persist_saves(&st);
                    p
                };
                self.emit(|l| l.on_save(p));
            }
        }
    }

    /// 改一条存到手机的进度并回调（已经暂停、取消了的不再被进度改回去）
    fn save_update(&self, id: u64, f: impl FnOnce(&mut SaveProgress)) {
        let p = {
            let mut st = self.state.lock().unwrap();
            let Some(s) = st.saves.get_mut(&id) else { return };
            if s.p.state != SaveState::Running {
                return;
            }
            f(&mut s.p);
            s.p.clone()
        };
        self.emit(|l| l.on_save(p));
    }

    fn thumb_path(&self, fp: &str, id: i64) -> PathBuf {
        self.cache.join("captae-remote-thumbs").join(fp).join(format!("{id}.jpg"))
    }

    fn cached_thumb(&self, fp: &str, id: i64) -> Option<String> {
        let p = self.thumb_path(fp, id);
        p.is_file().then(|| p.display().to_string())
    }

    fn remote_page(&self, fp: &str, v: &Value) -> LibraryPage {
        let items: Vec<MobileAsset> = v["items"].as_array().map(|a| a.iter().filter_map(|x| convert::asset(x, x["id"].as_i64().and_then(|id| self.cached_thumb(fp, id)))).collect()).unwrap_or_default();
        LibraryPage { total: v["total"].as_u64().unwrap_or(items.len() as u64), items }
    }

    fn lib_query(q: &LibraryQuery) -> LibQuery {
        LibQuery {
            q: q.search.clone().filter(|s| !s.trim().is_empty()),
            kind: q.kind.clone().filter(|k| !k.is_empty() && k != "all"),
            limit: (q.limit > 0).then_some(q.limit),
            offset: (q.offset > 0).then_some(q.offset),
            ..Default::default()
        }
    }
}

#[uniffi::export]
impl RemoteHub {
    /// 读出配对过的电脑（令牌在不在安全存储里也看一眼）。不联网，要连调 `set_active(true)`
    #[uniffi::constructor]
    pub fn new(core: Arc<MobileCore>, options: RemoteOptions, secure: Arc<dyn SecureStore>) -> Result<Arc<Self>, MobileError> {
        let store = store::Store::open(core.data_dir().join("remote")).map_err(|e| crate::types::to_mobile(&e.into(), ""))?;
        let mut st = State::default();
        for info in store.computers() {
            let status = if info.revoked {
                DeviceStatus::Revoked
            } else if secure.read(token_key(&info.fp)).is_some_and(|t| !t.is_empty()) {
                DeviceStatus::Idle
            } else {
                DeviceStatus::NeedsPairing
            };
            st.comps.push(Conn { info, status, client: None, task: None, jobs: BTreeMap::new(), flushing: false });
        }
        (st.pending, st.next_pending) = store.pending();
        // 存到手机的记录：电脑删掉了的、下完超过 7 天的不要；没下完的按文件实际大小算进度，「在下」的等那台电脑连上接着下
        let (recs, next_save) = store.saves();
        let cutoff = now_ms() - SAVE_KEEP_MS;
        for r in recs {
            if st.find(&r.device).is_none() {
                continue;
            }
            let state = match r.state.as_str() {
                "running" => SaveState::Running,
                "paused" => SaveState::Paused,
                "done" => SaveState::Done,
                "failed" => SaveState::Failed,
                _ => continue,
            };
            if state == SaveState::Done && r.done_at.unwrap_or(0) < cutoff {
                continue;
            }
            let got = if state == SaveState::Done { r.got } else { std::fs::metadata(&r.path).map(|m| m.len()).unwrap_or(0) };
            let error = r.error_code.map(|c| MobileFailure::coded(&c, r.error_message.unwrap_or_default(), None));
            if state == SaveState::Running {
                st.restored.push(r.id);
            }
            let p = SaveProgress { id: r.id, device: r.device, asset_id: r.asset_id, path: r.path, got_bytes: got, total_bytes: r.total, speed: 0, state, error, done_at: r.done_at, local_asset_id: r.local_asset_id };
            st.saves.insert(r.id, SaveSlot { p, task: None, created_at: r.created_at });
        }
        st.next_save = next_save;
        let platform = if core.platform.ios() { "ios" } else { "android" };
        let phone = PhoneHello { name: options.phone_name.trim().chars().take(40).collect(), platform: platform.into() };
        let hub = Arc::new_cyclic(|me| RemoteHub {
            me: me.clone(),
            core,
            secure,
            phone,
            cache: PathBuf::from(options.cache_dir),
            store,
            state: Mutex::new(st),
            listener: Mutex::new(None),
            active: AtomicBool::new(false),
        });
        // 过期的、电脑删掉了的记录从文件里也去掉
        hub.persist_saves(&hub.state.lock().unwrap());
        Ok(hub)
    }

    /// 设置回调（再设一次换掉上一个）
    pub fn set_listener(&self, listener: Arc<dyn RemoteListener>) {
        *self.listener.lock().unwrap() = Some(listener);
    }

    /// App 到前台时 `true`：连上所有配对过的电脑（离线的马上再找一次）；到后台时 `false`：断开（iOS 在后台不找电脑）
    pub fn set_active(&self, active: bool) {
        self.active.store(active, Ordering::SeqCst);
        if active {
            let restored: Vec<SaveProgress> = {
                let mut st = self.state.lock().unwrap();
                let ids = std::mem::take(&mut st.restored);
                ids.iter().filter_map(|id| st.saves.get(id)).filter(|s| s.p.state == SaveState::Running).map(|s| s.p.clone()).collect()
            };
            if !restored.is_empty() {
                tracing::info!("找回 {} 个上次没存完的，电脑连上就接着下", restored.len());
                self.emit(|l| l.on_saves_restored(restored));
            }
        }
        let fps: Vec<String> = self.state.lock().unwrap().comps.iter().map(|c| c.info.fp.clone()).collect();
        for fp in fps {
            if active {
                let need = {
                    let st = self.state.lock().unwrap();
                    st.find(&fp).is_some_and(|c| !c.info.revoked && (c.task.is_none() || c.status != DeviceStatus::Online))
                };
                if need {
                    self.start(&fp);
                }
            } else {
                {
                    let mut st = self.state.lock().unwrap();
                    if let Some(c) = st.find_mut(&fp) {
                        c.stop();
                        if matches!(c.status, DeviceStatus::Online | DeviceStatus::Offline | DeviceStatus::Connecting) {
                            c.status = DeviceStatus::Idle;
                        }
                    }
                }
                self.emit_device(&fp);
            }
        }
    }

    // ------------------------------------------------------------------ 配对

    /// 扫到的内容是不是 Captae 的配对二维码；是的话先给用户看连的是哪台电脑。不联网
    pub fn parse_pair_link(&self, text: String) -> Option<PairLinkInfo> {
        let l = PairLink::parse(&text)?;
        let known = self.state.lock().unwrap().find(&l.fp).is_some();
        Some(link_info(&l, known))
    }

    /// 扫码配对：按二维码里的指纹钉死证书，用一次性码换令牌。成功后这台电脑进设备列表，`set_active(true)` 着就马上连
    pub async fn pair_qr(&self, text: String) -> Result<MobileDevice, MobileError> {
        let link = PairLink::parse(&text).ok_or_else(|| convert::coded(code::BAD_LINK, tr!("这不是 Captae 的配对二维码")))?;
        let phone = self.phone.clone();
        let creds = self.on_rt(async move { rc::pair_qr(&link, &phone).await }).await.map_err(|e| convert::remote(&e))?;
        self.add_paired(creds)
    }

    /// 6 位数配对（SPAKE2）：`host`、`port` 是 App 发现到的电脑（`nearby()` 里的一台），`digits` 是电脑上显示的配对码
    pub async fn pair_code(&self, host: String, port: u16, digits: String) -> Result<MobileDevice, MobileError> {
        let phone = self.phone.clone();
        let creds = self.on_rt(async move { rc::pair_code(&host, port, &digits, &phone).await }).await.map_err(|e| convert::remote(&e))?;
        // 发现时还知道别的地址：一起记下
        let more: Vec<String> = {
            let st = self.state.lock().unwrap();
            st.nearby.iter().filter(|n| n.fingerprint.as_deref() == Some(creds.fp.as_str()) || n.hosts.iter().any(|h| creds.hosts.contains(h))).flat_map(|n| n.hosts.clone()).collect()
        };
        let mut creds = creds;
        for h in more {
            if !creds.hosts.contains(&h) {
                creds.hosts.push(h);
            }
        }
        self.add_paired(creds)
    }

    // ------------------------------------------------------------------ 发现

    /// App 用 NWBrowser / NSD 发现了一个 `_captae._tcp` 服务（解析好地址以后调）。
    /// 按 TXT 里的 `fp` 认出是配对过的哪台：记下新地址，没连上的马上再找一次，返回它的设备 id；没配对过的放进 `nearby()`，返回 nil
    pub fn discovered(&self, service: DiscoveredService) -> Option<String> {
        let fp = service.txt.get("fp").map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
        let hosts: Vec<String> = service.hosts.iter().map(|h| h.trim().trim_start_matches('[').trim_end_matches(']').to_string()).filter(|h| !h.is_empty()).collect();
        if let Some(fp) = fp.as_deref() {
            let restart = {
                let mut st = self.state.lock().unwrap();
                if let Some(c) = st.find_mut(fp) {
                    for h in hosts.iter().rev() {
                        c.info.hosts.retain(|x| x != h);
                        c.info.hosts.insert(0, h.clone());
                    }
                    if service.port != 0 {
                        c.info.port = service.port;
                    }
                    if let Some(cl) = &c.client {
                        cl.add_hosts(&hosts, (service.port != 0).then_some(service.port));
                    }
                    let restart = !c.info.revoked && c.status != DeviceStatus::Online && self.active.load(Ordering::SeqCst);
                    self.save_computers(&st);
                    Some(restart)
                } else {
                    None
                }
            };
            if let Some(restart) = restart {
                if restart {
                    self.start(fp);
                }
                return Some(fp.to_string());
            }
        }
        if hosts.is_empty() || service.port == 0 {
            return None;
        }
        let name = service.txt.get("name").filter(|n| !n.is_empty()).cloned().unwrap_or_else(|| service.name.clone());
        {
            let mut st = self.state.lock().unwrap();
            st.nearby.retain(|n| !(n.name == name || (fp.is_some() && n.fingerprint == fp)));
            st.nearby.push(NearbyComputer { name, hosts, port: service.port, fingerprint: fp, seen_at: now_ms() });
        }
        self.emit(|l| l.on_devices());
        None
    }

    /// App 那边这个服务不见了（NWBrowser 的 removed / NSD 的 onServiceLost），按实例名去掉
    pub fn lost(&self, name: String) {
        let changed = {
            let mut st = self.state.lock().unwrap();
            let n = st.nearby.len();
            st.nearby.retain(|x| x.name != name);
            st.nearby.len() != n
        };
        if changed {
            self.emit(|l| l.on_devices());
        }
    }

    /// 附近发现的、还没配对过的电脑
    pub fn nearby(&self) -> Vec<NearbyComputer> {
        self.state.lock().unwrap().nearby.clone()
    }

    // ------------------------------------------------------------------ 设备

    /// 设备菜单：这台手机在最前，后面是配对过的电脑（按配对的先后）
    pub fn devices(&self) -> Vec<MobileDevice> {
        let mut v = vec![self.local_device()];
        let st = self.state.lock().unwrap();
        v.extend(st.comps.iter().map(|c| st.device(c)));
        v
    }

    pub fn device(&self, id: String) -> Option<MobileDevice> {
        if id == LOCAL_DEVICE {
            return Some(self.local_device());
        }
        self.device_now(&id)
    }

    /// 在手机上给电脑改个名（空 = 用电脑自己报的名字）。不改电脑那边
    pub fn rename(&self, id: String, name: String) -> Result<MobileDevice, MobileError> {
        {
            let mut st = self.state.lock().unwrap();
            let c = st.find_mut(&id).ok_or_else(no_device)?;
            let n: String = name.trim().chars().take(60).collect();
            c.info.nickname = (!n.is_empty()).then_some(n);
            self.save_computers(&st);
        }
        self.emit_device(&id);
        self.device_now(&id).ok_or_else(no_device)
    }

    /// 删掉一台电脑：断开、删令牌、去掉等它上线的任务、停掉正在从它存到手机的（下了一半的文件留着）。
    /// 电脑那边「已配对的手机」里还有这台手机，要在电脑上撤销
    pub fn remove(&self, id: String) -> Result<(), MobileError> {
        let had_pending = {
            let mut st = self.state.lock().unwrap();
            let i = st.comps.iter().position(|c| c.info.fp == id).ok_or_else(no_device)?;
            let mut c = st.comps.remove(i);
            c.stop();
            let n = st.pending.len();
            st.pending.retain(|p| p.device != id);
            let had = st.pending.len() != n;
            for s in st.saves.values_mut().filter(|s| s.p.device == id) {
                if let Some(t) = s.task.take() {
                    t.abort();
                }
            }
            st.saves.retain(|_, s| s.p.device != id);
            self.save_computers(&st);
            self.save_pending(&st);
            self.persist_saves(&st);
            had
        };
        self.secure.delete(token_key(&id));
        self.emit(|l| l.on_devices());
        if had_pending {
            self.emit(|l| l.on_pending());
        }
        Ok(())
    }

    // ------------------------------------------------------------------ 电脑上的任务

    /// 电脑上的任务（最近一次收到的，按 id 排）。不联网；电脑离线时是断开前最后看到的
    pub fn remote_jobs(&self, device: String) -> Vec<MobileJob> {
        let st = self.state.lock().unwrap();
        st.find(&device).map(|c| c.jobs.values().filter_map(convert::job).collect()).unwrap_or_default()
    }

    /// 重新向电脑要一次任务列表（下拉刷新）
    pub async fn refresh_remote_jobs(&self, device: String) -> Result<Vec<MobileJob>, MobileError> {
        let list = self.call(&device, |c| async move { c.jobs().await }).await?;
        {
            let mut st = self.state.lock().unwrap();
            if let Some(c) = st.find_mut(&device) {
                c.jobs = list.iter().filter_map(|v| Some((v["id"].as_u64()?, v.clone()))).collect();
            }
        }
        Ok(list.iter().filter_map(convert::job).collect())
    }

    /// 电脑上一个任务的下载记录
    pub fn remote_job_log(&self, device: String, id: u64) -> Vec<MobileLogLine> {
        let st = self.state.lock().unwrap();
        st.find(&device).and_then(|c| c.jobs.get(&id)).map(convert::job_log).unwrap_or_default()
    }

    /// 发一个任务给电脑。电脑在线就直接发；不在线（或者发的时候连不上）放进「等电脑上线」，连上后自动发
    pub async fn send(&self, device: String, options: SendOptions) -> Result<SendResult, MobileError> {
        let url = options.url.trim().to_string();
        if !valid_url(&url) {
            return Err(convert::coded(code::BAD_REQUEST, tr!("链接好像不完整，检查一下是不是少复制了开头的 https://")));
        }
        let online = {
            let mut st = self.state.lock().unwrap();
            let c = st.find_mut(&device).ok_or_else(no_device)?;
            match c.status {
                DeviceStatus::Revoked => return Err(revoked()),
                DeviceStatus::NeedsPairing => return Err(needs_pairing()),
                _ => {}
            }
            c.info.last_used = Some(now_ms());
            let online = (c.status == DeviceStatus::Online).then(|| c.client.clone()).flatten();
            self.save_computers(&st);
            online
        };
        if let Some(client) = online {
            let add = AddJob { url: url.clone(), quality: options.quality.clone(), format: options.format.clone(), audio_only: options.audio_only, preset: options.preset.clone() };
            match self.on_rt(async move { client.add(&add).await }).await {
                Ok(v) => {
                    let job = convert::job(&v).ok_or_else(|| convert::coded(code::FAILED, tr!("和电脑通信出错了")))?;
                    return Ok(SendResult::Sent { job });
                }
                Err(e) => {
                    let m = self.failed(&device, &e);
                    if convert::code_of(&m) != code::UNREACHABLE {
                        return Err(m);
                    }
                }
            }
        }
        let task = {
            let mut st = self.state.lock().unwrap();
            let id = st.next_pending;
            st.next_pending += 1;
            let p = Pending {
                id,
                device: device.clone(),
                url,
                title: options.title.filter(|t| !t.trim().is_empty()),
                quality: options.quality,
                format: options.format,
                audio_only: options.audio_only,
                preset: options.preset,
                created_at: now_ms(),
                error_code: None,
                error_message: None,
            };
            st.pending.push(p.clone());
            self.save_pending(&st);
            p
        };
        tracing::info!("电脑不在线，任务先存在手机上：{}", task.url);
        self.emit(|l| l.on_pending());
        self.emit_device(&device);
        // 在前台、但这台电脑看起来离线：马上再找一次
        let idle = self.state.lock().unwrap().find(&device).is_some_and(|c| c.status != DeviceStatus::Online);
        if idle && self.active.load(Ordering::SeqCst) {
            self.start(&device);
        }
        Ok(SendResult::Queued { task: self.pending_task(&task) })
    }

    /// 任务操作（和桌面端的 `JobAction` 一样）
    pub async fn remote_action(&self, device: String, id: u64, action: MobileAction) -> Result<(), MobileError> {
        let a = code_str(&action.core());
        self.call(&device, move |c| async move { c.action(id, &a).await }).await
    }

    /// M3U8 选清晰度（`MobileJob.variants` 的下标）
    pub async fn remote_set_variant(&self, device: String, id: u64, index: u32) -> Result<(), MobileError> {
        self.call(&device, move |c| async move { c.set_variant(id, index as usize).await }).await
    }

    /// 播放列表选条目（`["all"]` = 全部），返回电脑上新建的任务
    pub async fn remote_choose_entries(&self, device: String, id: u64, ids: Vec<String>) -> Result<Vec<MobileJob>, MobileError> {
        let v = self.call(&device, move |c| async move { c.choose_entries(id, &ids).await }).await?;
        Ok(v.iter().filter_map(convert::job).collect())
    }

    /// 在电脑上读取信息（不带电脑上的登录状态）。`cancel` 取消时马上返回 `cancelled`
    pub async fn remote_probe(&self, device: String, url: String, cancel: Option<Arc<CancelToken>>) -> Result<MobileProbe, MobileError> {
        let token = cancel.map(|c| c.token.clone()).unwrap_or_default();
        let client = self.client_for(&device)?;
        let r = self
            .on_rt(async move {
                tokio::select! {
                    biased;
                    _ = token.cancelled() => Ok(None),
                    r = client.probe(&url) => r.map(Some),
                }
            })
            .await;
        match r {
            Ok(Some(v)) => Ok(convert::probe(&v)),
            Ok(None) => Err(crate::cancelled()),
            Err(e) => Err(self.failed(&device, &e)),
        }
    }

    // ------------------------------------------------------------------ 电脑的素材库

    /// 查电脑的素材库（按下载时间从新到旧）。素材的 `path` 是空的；`thumb` 是缓存过的封面
    pub async fn remote_library(&self, device: String, query: LibraryQuery) -> Result<LibraryPage, MobileError> {
        let q = Self::lib_query(&query);
        let v = self.call(&device, move |c| async move { c.library(&q).await }).await?;
        Ok(self.remote_page(&device, &v))
    }

    pub async fn remote_asset(&self, device: String, id: i64) -> Result<Option<MobileAsset>, MobileError> {
        match self.call(&device, move |c| async move { c.asset(id).await }).await {
            Ok(v) => Ok(convert::asset(&v, self.cached_thumb(&device, id))),
            Err(e) if convert::code_of(&e) == code::NOT_FOUND => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// 电脑素材的封面：缓存到 App 的缓存目录，返回文件路径；电脑上没有封面时 nil。`refresh` 时不用缓存
    pub async fn remote_thumbnail(&self, device: String, id: i64, refresh: bool) -> Result<Option<String>, MobileError> {
        if !refresh {
            if let Some(p) = self.cached_thumb(&device, id) {
                return Ok(Some(p));
            }
        }
        let path = self.thumb_path(&device, id);
        let r = self
            .call(&device, move |c| async move {
                let bytes = c.thumb(id).await?;
                if let Some(d) = path.parent() {
                    tokio::fs::create_dir_all(d).await?;
                }
                let tmp = path.with_extension("part");
                tokio::fs::write(&tmp, &bytes).await?;
                tokio::fs::rename(&tmp, &path).await?;
                Ok(path.display().to_string())
            })
            .await;
        match r {
            Ok(p) => Ok(Some(p)),
            Err(e) if convert::code_of(&e) == code::NOT_FOUND => Ok(None),
            Err(e) => Err(e),
        }
    }

    // ------------------------------------------------------------------ 存到手机

    /// 把电脑素材库里的文件下到 `path`（App 的目录里，下完以后 App 存相册 / 「文件」）。
    /// `path` 已经有下了一半的（同一个文件）就接着下；旁边会有一个 `<path>.etag` 记着电脑上文件的版本，下完删掉。
    /// 记录存在 `<数据目录>/remote/saves.json`：App 被结束了，重开后自动接着下（`on_saves_restored`），下完的留 7 天
    pub fn save_to_phone(&self, device: String, asset_id: i64, path: String) -> Result<SaveProgress, MobileError> {
        if path.trim().is_empty() {
            return Err(convert::coded(code::BAD_REQUEST, tr!("没有给保存的位置")));
        }
        if let Some(d) = Path::new(&path).parent() {
            std::fs::create_dir_all(d).map_err(|e| crate::types::to_mobile(&e.into(), ""))?;
        }
        let id = {
            let mut st = self.state.lock().unwrap();
            st.find(&device).ok_or_else(no_device)?;
            let id = st.next_save;
            st.next_save += 1;
            let have = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            let p = SaveProgress { id, device: device.clone(), asset_id, path, got_bytes: have, total_bytes: 0, speed: 0, state: SaveState::Running, error: None, done_at: None, local_asset_id: None };
            st.saves.insert(id, SaveSlot { p, task: None, created_at: now_ms() });
            id
        };
        if let Err(e) = self.run_save(id) {
            let mut st = self.state.lock().unwrap();
            st.saves.remove(&id);
            self.persist_saves(&st);
            return Err(e);
        }
        Ok(self.state.lock().unwrap().saves.get(&id).map(|s| s.p.clone()).ok_or_else(no_task)?)
    }

    /// 暂停（下了的部分留着，`resume_save` 接着下）
    pub fn pause_save(&self, id: u64) -> Result<SaveProgress, MobileError> {
        let p = {
            let mut st = self.state.lock().unwrap();
            let s = st.saves.get_mut(&id).ok_or_else(no_task)?;
            if s.p.state == SaveState::Running {
                if let Some(t) = s.task.take() {
                    t.abort();
                }
                s.p.state = SaveState::Paused;
                s.p.speed = 0;
            }
            let p = s.p.clone();
            self.persist_saves(&st);
            p
        };
        self.emit(|l| l.on_save(p.clone()));
        Ok(p)
    }

    /// 接着下（暂停了的、失败了的、重开后找回还在等电脑的）
    pub fn resume_save(&self, id: u64) -> Result<SaveProgress, MobileError> {
        {
            let st = self.state.lock().unwrap();
            let s = st.saves.get(&id).ok_or_else(no_task)?;
            let waiting = s.p.state == SaveState::Running && s.task.is_none();
            if !waiting && !matches!(s.p.state, SaveState::Paused | SaveState::Failed) {
                return Ok(s.p.clone());
            }
        }
        self.run_save(id)?;
        let p = self.state.lock().unwrap().saves.get(&id).map(|s| s.p.clone()).ok_or_else(no_task)?;
        self.emit(|l| l.on_save(p.clone()));
        Ok(p)
    }

    /// 取消：停下、删掉下了一半的文件（已经下完的不删）
    pub fn cancel_save(&self, id: u64) -> Result<SaveProgress, MobileError> {
        let p = {
            let mut st = self.state.lock().unwrap();
            let mut s = st.saves.remove(&id).ok_or_else(no_task)?;
            if let Some(t) = s.task.take() {
                t.abort();
            }
            if s.p.state != SaveState::Done {
                let _ = std::fs::remove_file(&s.p.path);
                let _ = std::fs::remove_file(format!("{}.etag", s.p.path));
                s.p.state = SaveState::Cancelled;
            }
            s.p.speed = 0;
            self.persist_saves(&st);
            s.p
        };
        self.emit(|l| l.on_save(p.clone()));
        Ok(p)
    }

    /// 存到手机的全部记录（按开始的先后）：这次在下的、重开后找回的、7 天内下完的
    pub fn saves(&self) -> Vec<SaveProgress> {
        self.state.lock().unwrap().saves.values().map(|s| s.p.clone()).collect()
    }

    /// 从记录里去掉已经完成、失败的（不删文件）
    pub fn forget_save(&self, id: u64) {
        let mut st = self.state.lock().unwrap();
        if st.saves.get(&id).is_some_and(|s| s.p.state != SaveState::Running) {
            st.saves.remove(&id);
            self.persist_saves(&st);
        }
    }

    // ------------------------------------------------------------------ 等电脑上线

    /// 「等电脑上线」队列（按加的先后）
    pub fn pending(&self) -> Vec<PendingTask> {
        let list = self.state.lock().unwrap().pending.clone();
        list.iter().map(|p| self.pending_task(p)).collect()
    }

    pub fn remove_pending(&self, id: u64) -> Result<(), MobileError> {
        let device = {
            let mut st = self.state.lock().unwrap();
            let i = st.pending.iter().position(|p| p.id == id).ok_or_else(no_task)?;
            let p = st.pending.remove(i);
            self.save_pending(&st);
            p.device
        };
        self.emit(|l| l.on_pending());
        self.emit_device(&device);
        Ok(())
    }

    /// 电脑拒绝过的一条再发一次（清掉错误；电脑在线就马上发）
    pub fn retry_pending(&self, id: u64) -> Result<(), MobileError> {
        let device = {
            let mut st = self.state.lock().unwrap();
            let p = st.pending.iter_mut().find(|p| p.id == id).ok_or_else(no_task)?;
            p.error_code = None;
            p.error_message = None;
            let d = p.device.clone();
            self.save_pending(&st);
            d
        };
        self.emit(|l| l.on_pending());
        self.flush(&device);
        Ok(())
    }

    /// 改成在这台手机上下（`ids` 空 = 全部）。iOS 上架版只改得动直链和 M3U8，其余留在队列里
    pub fn move_to_phone(&self, ids: Vec<u64>) -> MoveResult {
        let picked: Vec<Pending> = {
            let st = self.state.lock().unwrap();
            st.pending.iter().filter(|p| ids.is_empty() || ids.contains(&p.id)).cloned().collect()
        };
        let (mut moved, mut kept, mut done_ids, mut errs) = (vec![], vec![], vec![], vec![]);
        for p in picked {
            let o = AddOptions { url: p.url.clone(), quality: p.quality.clone(), format: p.format.clone(), audio_only: p.audio_only, page_url: None, page_title: p.title.clone() };
            match self.core.add(o) {
                Ok(j) => {
                    moved.push(j);
                    done_ids.push(p.id);
                }
                Err(e) => {
                    let d = e.detail();
                    // 上架版不能在手机上下的：照旧留着等电脑，不算出错
                    if !matches!(d.code.as_str(), "unsupported_here" | "probe_first") {
                        errs.push((p.id, d.code.clone(), d.message.clone()));
                    }
                }
            }
        }
        {
            let mut st = self.state.lock().unwrap();
            st.pending.retain(|p| !done_ids.contains(&p.id));
            for (id, c, m) in errs {
                if let Some(p) = st.pending.iter_mut().find(|p| p.id == id) {
                    p.error_code = Some(c);
                    p.error_message = Some(m);
                }
            }
            self.save_pending(&st);
            let left: Vec<Pending> = st.pending.iter().filter(|p| ids.is_empty() || ids.contains(&p.id)).cloned().collect();
            drop(st);
            kept.extend(left.iter().map(|p| self.pending_task(p)));
        }
        if !moved.is_empty() {
            self.emit(|l| l.on_pending());
            self.emit(|l| l.on_devices());
        }
        MoveResult { moved, kept }
    }

    // ------------------------------------------------------------------ 合并视图（「全部设备」）

    /// 下载页的分组：每台电脑一组（电脑上的任务 + 等它上线的任务），最后是这台手机。
    /// `device` 给了就只要那一台；`search` 给了只留标题、链接、来源里有它的（不分大小写）。空的组也在，界面自己决定藏不藏
    pub fn merged_jobs(&self, device: Option<String>, search: Option<String>) -> Vec<JobGroup> {
        let want = |id: &str| device.as_deref().is_none_or(|d| d == id);
        let mut groups = vec![];
        let (comps, pending): (Vec<(MobileDevice, Vec<Value>)>, Vec<Pending>) = {
            let st = self.state.lock().unwrap();
            (st.comps.iter().filter(|c| want(&c.info.fp)).map(|c| (st.device(c), c.jobs.values().cloned().collect())).collect(), st.pending.clone())
        };
        for (dev, jobs) in comps {
            let jobs: Vec<DeviceJob> = jobs
                .iter()
                .filter_map(|v| {
                    let job = convert::job(v)?;
                    matches(&search, &[&job.title, &job.url, &job.src, job.plat.as_deref().unwrap_or_default()]).then(|| DeviceJob { device: dev.id.clone(), from: convert::job_from(v), job })
                })
                .collect();
            let pend: Vec<PendingTask> = pending.iter().filter(|p| p.device == dev.id && matches(&search, &[p.title.as_deref().unwrap_or_default(), &p.url])).map(|p| self.pending_task(p)).collect();
            // 新的在前（JobGroup.jobs 的约定）
            let jobs = jobs.into_iter().rev().collect();
            groups.push(JobGroup { device: dev, jobs, pending: pend });
        }
        if want(LOCAL_DEVICE) {
            let jobs: Vec<DeviceJob> = self
                .core
                .jobs()
                .into_iter()
                .filter(|j| matches(&search, &[&j.title, &j.url, &j.src, j.plat.as_deref().unwrap_or_default()]))
                .map(|job| DeviceJob { device: LOCAL_DEVICE.into(), job, from: None })
                .collect();
            let jobs = jobs.into_iter().rev().collect();
            groups.push(JobGroup { device: self.local_device(), jobs, pending: vec![] });
        }
        groups
    }

    /// 素材库的分段：每台电脑一段（在线的去查，离线的带 `error`），最后是这台手机。几台电脑同时查。
    /// `device` 给了就只查那一台；`query` 对每一段都一样（分页按段各自算）
    pub async fn merged_library(&self, query: LibraryQuery, device: Option<String>) -> Vec<LibrarySection> {
        let want = |id: &str| device.as_deref().is_none_or(|d| d == id);
        let comps: Vec<MobileDevice> = {
            let st = self.state.lock().unwrap();
            st.comps.iter().filter(|c| want(&c.info.fp)).map(|c| st.device(c)).collect()
        };
        let mut waits = vec![];
        for dev in comps {
            let client = match dev.status {
                DeviceStatus::Revoked => Err(revoked()),
                DeviceStatus::NeedsPairing => Err(needs_pairing()),
                _ => self.client_for(&dev.id),
            };
            let q = Self::lib_query(&query);
            let t = client.map(|c| AbortOnDrop(self.core.rt.spawn(async move { c.library(&q).await })));
            waits.push((dev, t));
        }
        let mut out = vec![];
        for (dev, t) in waits {
            let r = match t {
                Err(e) => Err(e),
                Ok(mut t) => match (&mut t.0).await {
                    Ok(Ok(v)) => Ok(v),
                    Ok(Err(e)) => Err(self.failed(&dev.id, &e)),
                    Err(e) => Err(convert::remote(&anyhow::anyhow!("remote task: {e}"))),
                },
            };
            out.push(match r {
                Ok(v) => LibrarySection::from_page(dev.clone(), self.remote_page(&dev.id, &v)),
                Err(e) => LibrarySection { device: dev, items: vec![], total: 0, error: Some(e.detail().clone()) },
            });
        }
        if want(LOCAL_DEVICE) {
            let dev = self.local_device();
            out.push(match self.core.library(query) {
                Ok(p) => LibrarySection::from_page(dev, p),
                Err(e) => LibrarySection { device: dev, items: vec![], total: 0, error: Some(e.detail().clone()) },
            });
        }
        out
    }

    /// 本机任务里正在跑的有几个（设备菜单的小字），和 `devices()` 里这台手机的 `running` 一样
    pub fn local_running(&self) -> u32 {
        self.local_device().running
    }
}

impl RemoteHub {
    /// 测试用：这台电脑现在的状态
    #[cfg(test)]
    fn status_of(&self, fp: &str) -> Option<DeviceStatus> {
        self.state.lock().unwrap().find(fp).map(|c| c.status)
    }

}
