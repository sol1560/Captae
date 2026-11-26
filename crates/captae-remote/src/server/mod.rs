//! 电脑端：「让手机连接这台电脑」打开以后，在局域网上开一个 HTTPS 服务，手机配对以后经它遥控 Captae。
//!
//! - 和只听 127.0.0.1 的本机控制接口（captae-core 的 api.rs）完全分开：端口、令牌、路由都不共用。
//! - 自签证书第一次打开时生成，放在 `<数据目录>/remote/`（目录 0700、文件 0600）。手机按二维码里的指纹钉死它。
//! - 配对：二维码里的一次性码（5 分钟、用一次作废），或者 6 位数（5 分钟、错 5 次作废，走 SPAKE2，见 proto.rs）。
//!   换来 256 位的设备令牌，电脑上只存它的 SHA-256。每台手机能单独撤销。
//! - 开关和已配对的手机存在 `<数据目录>/remote/state.json`，不进 settings.json（AI 助手、命令行改设置时动不到它）。
//! - 能做的事只有白名单里那些（routes.rs），其余一律 403。
//! - Bonjour 广播 `_captae._tcp`：交给系统的 mDNSResponder（bonjour.rs，不自己收发组播），TXT 里有协议版本、证书指纹、电脑名，手机据此在换了地址以后找回这台电脑。

mod bonjour;
mod routes;
mod store;
mod tls;

use crate::proto::{self, code, PairLink, PhoneHello};
use anyhow::{anyhow, Context, Result};
use captae_core::errors::coded;
use captae_core::{tr, Core};
use serde::Serialize;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use store::{Device, Store};
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

/// 有变化时（配对了、撤销了、手机连上 / 断开、开关变了）通知桌面端界面刷新
pub type OnChange = Arc<dyn Fn() + Send + Sync>;

pub struct Opts {
    /// 电脑名（「Sol 的 MacBook Pro」），二维码、Bonjour、配对回应里用
    pub name: String,
    pub on_change: Option<OnChange>,
    /// 监听哪个地址。None = 所有网卡（0.0.0.0）。测试时用 127.0.0.1：不弹 macOS 防火墙的询问、局域网上也看不到
    pub bind: Option<IpAddr>,
    /// 广播 Bonjour。测试时关掉
    pub bonjour: bool,
}

/// 「设置 › 手机」看到的状态
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub enabled: bool,
    pub running: bool,
    pub port: Option<u16>,
    /// 二维码里写的局域网地址
    pub hosts: Vec<String>,
    pub name: String,
    /// 打开时出的错（端口被占、证书写不进去……）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub devices: Vec<DeviceView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub platform: String,
    pub paired_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<i64>,
    /// 现在连着（收着实时事件）
    pub online: bool,
}

/// 现在显示的配对码
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PairingView {
    /// 二维码内容 `captae://pair?…`
    pub uri: String,
    /// 6 位数
    pub code: String,
    /// 毫秒时间戳
    pub expires_at: i64,
    /// 6 位数错了 5 次、作废了（二维码照样能用，到期前不自动换，免得一直猜）
    pub void: bool,
    /// 6 位数还能试几次
    pub left: u32,
}

/// 一轮配对码：二维码里的一次性码和 6 位数一起生成、一起过期。任何一种配对成功后整轮作废
struct Pairing {
    gen: u64,
    qr: String,
    digits: String,
    expires: i64,
    /// 二维码码错的次数（128 位随机数猜不中，错多了也作废，挡住乱试）
    qr_wrong: u32,
    /// 6 位数试了几次（每次 SPAKE2 开始算一次：一次开始只能验证一个猜测）
    tries: u32,
    /// 6 位数试满、最后一次也没对：作废
    void: bool,
}

/// 6 位数配对进行到一半（电脑已经回了消息和确认码，等手机的确认码）
struct Session {
    gen: u64,
    key: Vec<u8>,
    msg_phone: Vec<u8>,
    msg_desktop: Vec<u8>,
    until: i64,
}

struct Running {
    port: u16,
    hosts: Vec<String>,
    cancel: CancellationToken,
    /// Bonjour 广播；丢掉就撤回
    advert: Option<bonjour::Advert>,
}

pub(crate) struct Shared {
    core: Core,
    dir: PathBuf,
    opts: Opts,
    store: Mutex<Store>,
    pairing: Mutex<Option<Pairing>>,
    sessions: Mutex<HashMap<String, Session>>,
    run: Mutex<Option<Running>>,
    /// 撤销一次加一，连着的实时事件据此检查自己还在不在
    revoked: watch::Sender<u64>,
    /// 每台手机连着几条实时事件
    online: Mutex<HashMap<String, usize>>,
    /// 每台手机上次把「上次连接」写进文件的时刻（不每个请求都写）
    seen_saved: Mutex<HashMap<String, i64>>,
    /// 打开时出的错
    error: Mutex<Option<String>>,
    next_gen: std::sync::atomic::AtomicU64,
}

/// 电脑端的远程控制服务。克隆出来的是同一个
#[derive(Clone)]
pub struct Remote(pub(crate) Arc<Shared>);

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

impl Remote {
    /// 读开关和已配对的手机（`<数据目录>/remote/state.json`），还不监听
    pub fn new(core: Core, opts: Opts) -> Remote {
        let dir = core.paths.data.join("remote");
        let store = Store::load(&dir);
        let (revoked, _) = watch::channel(0);
        Remote(Arc::new(Shared {
            core,
            dir,
            opts,
            store: Mutex::new(store),
            pairing: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            run: Mutex::new(None),
            revoked,
            online: Mutex::new(HashMap::new()),
            seen_saved: Mutex::new(HashMap::new()),
            error: Mutex::new(None),
            next_gen: std::sync::atomic::AtomicU64::new(1),
        }))
    }

    /// 桌面端启动时：上次开着就接着开。出错只记下（「设置 › 手机」里显示），不挡启动
    pub async fn start_if_enabled(&self) {
        if !self.0.store.lock().unwrap().enabled {
            return;
        }
        if let Err(e) = self.listen().await {
            tracing::warn!("远程控制没打开：{e:#}");
            *self.0.error.lock().unwrap() = Some(e.to_string());
        }
    }

    /// 「让手机连接这台电脑」开关
    pub async fn set_enabled(&self, on: bool) -> Result<Status> {
        if on {
            self.listen().await?;
            let mut st = self.0.store.lock().unwrap();
            st.enabled = true;
            st.save(&self.0.dir)?;
        } else {
            self.stop();
            let mut st = self.0.store.lock().unwrap();
            st.enabled = false;
            st.save(&self.0.dir)?;
        }
        *self.0.error.lock().unwrap() = None;
        self.0.changed();
        Ok(self.status())
    }

    pub fn status(&self) -> Status {
        let s = &self.0;
        let st = s.store.lock().unwrap();
        let run = s.run.lock().unwrap();
        let online = s.online.lock().unwrap();
        Status {
            enabled: st.enabled,
            running: run.is_some(),
            port: run.as_ref().map(|r| r.port),
            hosts: run.as_ref().map(|r| r.hosts.clone()).unwrap_or_default(),
            name: s.opts.name.clone(),
            error: s.error.lock().unwrap().clone(),
            devices: st
                .devices
                .iter()
                .map(|d| DeviceView { id: d.id.clone(), name: d.name.clone(), platform: d.platform.clone(), paired_at: d.paired_at, last_seen: d.last_seen, online: online.get(&d.id).is_some_and(|n| *n > 0) })
                .collect(),
        }
    }

    /// 现在的配对码；没有、过期了、`refresh` 时换一轮新的。服务没开时报错
    pub fn pairing(&self, refresh: bool) -> Result<PairingView> {
        let s = &self.0;
        let (port, hosts) = {
            let run = s.run.lock().unwrap();
            let r = run.as_ref().ok_or_else(|| coded(code::PAIR_CLOSED, tr!("先打开「让手机连接这台电脑」")))?;
            (r.port, r.hosts.clone())
        };
        let fp = tls::fingerprint_of(&s.dir)?;
        let mut p = s.pairing.lock().unwrap();
        let stale = p.as_ref().is_none_or(|x| x.expires <= now_ms());
        if refresh || stale {
            *p = Some(Pairing {
                gen: s.next_gen.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                qr: proto::random_b64(16),
                digits: six_digits(),
                expires: now_ms() + proto::PAIR_TTL_MS,
                qr_wrong: 0,
                tries: 0,
                void: false,
            });
            s.sessions.lock().unwrap().clear();
        }
        let x = p.as_ref().unwrap();
        let link = PairLink { hosts, port, fp, code: x.qr.clone(), name: s.opts.name.clone() };
        Ok(PairingView { uri: link.to_uri(), code: x.digits.clone(), expires_at: x.expires, void: x.void, left: proto::PAIR_MAX_TRIES.saturating_sub(x.tries) })
    }

    /// 撤销一台手机：它的令牌马上失效，连着的实时事件马上断开
    pub fn revoke(&self, id: &str) -> Result<()> {
        let s = &self.0;
        {
            let mut st = s.store.lock().unwrap();
            let n = st.devices.len();
            st.devices.retain(|d| d.id != id);
            if st.devices.len() == n {
                return Err(coded(code::NOT_FOUND, tr!("没有这台手机")));
            }
            st.save(&s.dir)?;
        }
        s.revoked.send_modify(|g| *g += 1);
        s.changed();
        Ok(())
    }

    /// 停止监听（关掉开关、桌面端退出）。已经连着的实时事件马上断开，配对码作废
    pub fn stop(&self) {
        let s = &self.0;
        if let Some(r) = s.run.lock().unwrap().take() {
            r.cancel.cancel();
            // 撤回 Bonjour 广播
            drop(r.advert);
            tracing::info!("远程控制已关闭（端口 {}）", r.port);
        }
        *s.pairing.lock().unwrap() = None;
        s.sessions.lock().unwrap().clear();
    }

    /// 证书指纹（第一次调用时生成证书）
    pub fn fingerprint(&self) -> Result<String> {
        tls::fingerprint_of(&self.0.dir)
    }

    async fn listen(&self) -> Result<()> {
        let s = self.0.clone();
        if s.run.lock().unwrap().is_some() {
            return Ok(());
        }
        let (config, fp) = tls::load_or_create(&s.dir).context(tr!("证书没有准备好"))?;
        let ip = s.opts.bind.unwrap_or(IpAddr::V4(Ipv4Addr::UNSPECIFIED));
        let saved = s.store.lock().unwrap().port;
        // 先用上次的端口（手机记着它），没有就用默认端口，都被占了用随机端口
        let mut tcp = None;
        for port in [saved.unwrap_or(proto::DEFAULT_PORT), proto::DEFAULT_PORT, 0] {
            if let Ok(l) = tokio::net::TcpListener::bind(SocketAddr::new(ip, port)).await {
                tcp = Some(l);
                break;
            }
        }
        let tcp = tcp.ok_or_else(|| anyhow!(tr!("没有找到能用的端口")))?;
        let port = tcp.local_addr()?.port();
        if saved != Some(port) {
            let mut st = s.store.lock().unwrap();
            st.port = Some(port);
            st.save(&s.dir)?;
        }
        let hosts = if ip.is_loopback() { vec![ip.to_string()] } else { lan_hosts() };
        let cancel = CancellationToken::new();
        let listener = tls::Listener::spawn(tcp, config, cancel.clone());
        let app = routes::router(self.clone());
        {
            let c = cancel.clone();
            tokio::spawn(async move {
                let r = axum::serve(listener, app).with_graceful_shutdown(async move { c.cancelled().await }).await;
                if let Err(e) = r {
                    tracing::warn!("远程控制服务停了：{e}");
                }
            });
        }
        let advert = if s.opts.bonjour {
            match bonjour::Advert::register(&s.opts.name, port, &fp) {
                Ok(m) => Some(m),
                Err(e) => {
                    // 广播不了只是手机自动发现不到，扫码照样能连
                    tracing::warn!("Bonjour 广播没开成：{e:#}");
                    None
                }
            }
        } else {
            None
        };
        tracing::info!("远程控制已打开：端口 {port}，地址 {hosts:?}");
        *s.run.lock().unwrap() = Some(Running { port, hosts, cancel, advert });
        Ok(())
    }
}

impl Shared {
    fn changed(&self) {
        if let Some(f) = &self.opts.on_change {
            f();
        }
    }

    /// 令牌 → 手机；顺便记「上次连接」（每台手机最多一分钟写一次文件）
    fn device_by_token(&self, token: &str) -> Option<Device> {
        let h = proto::token_hash(token);
        let now = now_ms();
        let (dev, save) = {
            let mut st = self.store.lock().unwrap();
            let d = st.devices.iter_mut().find(|d| proto::same(&d.token_hash, &h))?;
            d.last_seen = Some(now);
            let mut saved = self.seen_saved.lock().unwrap();
            let save = saved.get(&d.id).is_none_or(|t| now - t > 60_000);
            if save {
                saved.insert(d.id.clone(), now);
            }
            (d.clone(), save)
        };
        if save {
            let st = self.store.lock().unwrap();
            if let Err(e) = st.save(&self.dir) {
                tracing::warn!("记不下手机的上次连接时间：{e}");
            }
            drop(st);
            self.changed();
        }
        Some(dev)
    }

    fn has_device(&self, id: &str) -> bool {
        self.store.lock().unwrap().devices.iter().any(|d| d.id == id)
    }

    fn set_online(&self, id: &str, delta: isize) {
        {
            let mut m = self.online.lock().unwrap();
            let n = m.entry(id.to_string()).or_default();
            *n = (*n as isize + delta).max(0) as usize;
        }
        self.changed();
    }

    fn cancel_token(&self) -> Option<CancellationToken> {
        self.run.lock().unwrap().as_ref().map(|r| r.cancel.clone())
    }

    /// 配对成功：记下这台手机，发令牌，整轮配对码作废
    fn add_device(&self, phone: &PhoneHello) -> Result<proto::Paired> {
        let token = proto::random_b64(32);
        let platform = match phone.platform.trim().to_ascii_lowercase().as_str() {
            p @ ("ios" | "android") => p.to_string(),
            _ => "other".to_string(),
        };
        let name = clean_name(&phone.name).unwrap_or_else(|| match platform.as_str() {
            "ios" => "iPhone".to_string(),
            "android" => tr!("Android 手机"),
            _ => tr!("手机"),
        });
        let dev = Device { id: proto::random_b64(6), name, platform, token_hash: proto::token_hash(&token), paired_at: now_ms(), last_seen: Some(now_ms()) };
        {
            let mut st = self.store.lock().unwrap();
            st.devices.push(dev.clone());
            st.save(&self.dir)?;
        }
        *self.pairing.lock().unwrap() = None;
        self.sessions.lock().unwrap().clear();
        tracing::info!("配对了一台手机：{}（{}）", dev.name, dev.platform);
        self.changed();
        Ok(proto::Paired { token, device: dev.id, name: self.opts.name.clone() })
    }
}

/// 手机报的名字：去掉控制字符和首尾空白，最多 40 个字
fn clean_name(s: &str) -> Option<String> {
    let t: String = s.chars().filter(|c| !c.is_control()).take(40).collect();
    let t = t.trim().to_string();
    (!t.is_empty()).then_some(t)
}

fn six_digits() -> String {
    use rand::Rng;
    format!("{:06}", rand::rng().random_range(0..1_000_000u32))
}

/// 二维码里写的局域网地址：IPv4、不是本机回环和 169.254 的；私有网段排前面
fn lan_hosts() -> Vec<String> {
    let mut v: Vec<(bool, String)> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|i| match i.ip() {
            IpAddr::V4(a) if !a.is_loopback() && !a.is_link_local() && !a.is_unspecified() => Some((a.is_private(), a.to_string())),
            _ => None,
        })
        .collect();
    v.sort_by_key(|(private, _)| !*private);
    v.dedup_by(|a, b| a.1 == b.1);
    v.into_iter().map(|(_, h)| h).take(4).collect()
}

/// 这台电脑的名字（「Sol 的 MacBook Pro」）：macOS 读「共享」里的电脑名，其他系统用主机名
pub fn computer_name() -> String {
    #[cfg(target_os = "macos")]
    if let Ok(o) = std::process::Command::new("/usr/sbin/scutil").args(["--get", "ComputerName"]).output() {
        let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
        if o.status.success() && !s.is_empty() {
            return s;
        }
    }
    std::process::Command::new("hostname")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().trim_end_matches(".local").to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Captae".into())
}

#[cfg(test)]
mod tests;
