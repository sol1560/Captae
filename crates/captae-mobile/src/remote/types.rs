//! 遥控电脑用的 UniFFI 类型：设备、等电脑上线的任务、存到手机的进度、合并视图、回调。
//! 任务、素材、读取信息的结果复用本机那套（`MobileJob`、`MobileAsset`、`MobileProbe`），只是外面包一层设备。

use crate::{LibraryPage, MobileAsset, MobileFailure, MobileJob};
use std::collections::HashMap;

/// 这台手机在合并视图里的设备 id（电脑的设备 id 是它的证书指纹）
pub const LOCAL_DEVICE: &str = "phone";

/// 安全存储：App 实现，iOS 用钥匙串（`kSecClassGenericPassword`，建议 `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`），
/// Android 用 Keystore 加密后的存储（EncryptedSharedPreferences 或自己用 Keystore 的 AES 密钥加密）。
/// Rust 只经它读写设备令牌，不写进任何文件。键形如 `captae.remote.token.<证书指纹>`
#[uniffi::export(with_foreign)]
pub trait SecureStore: Send + Sync {
    /// 没有时返回 nil
    fn read(&self, key: String) -> Option<String>;
    /// 写成功返回 true（写不进去时配对算失败，不会留下一台没有令牌的电脑）
    fn write(&self, key: String, value: String) -> bool;
    fn delete(&self, key: String);
}

/// 一台设备现在的状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum DeviceStatus {
    /// 这台手机自己
    Local,
    /// App 在后台（`set_active(false)`），没在连
    Idle,
    /// 正在连（刚打开、刚配对、刚换了地址）
    Connecting,
    Online,
    /// 连不上（不在同一个 Wi-Fi、电脑没开 Captae、没打开「让手机连接」），过一会儿自动再找
    Offline,
    /// 在电脑上被撤销了：令牌已经删掉，要重新配对
    Revoked,
    /// 安全存储里没有这台电脑的令牌（钥匙串被清过、换了手机恢复备份），要重新配对
    NeedsPairing,
}

/// 设备菜单里的一台设备：这台手机，或者配对过的一台电脑
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct MobileDevice {
    /// 这台手机是 `"phone"`；电脑是证书指纹（配对时拿到的，地址会变、它不变）
    pub id: String,
    /// 显示的名字：改过名就是改的名字，否则是电脑自己报的名字（这台手机是 App 传进来的手机名）
    pub name: String,
    /// 电脑自己报的名字（改名以后也留着）
    pub computer_name: String,
    pub local: bool,
    pub status: DeviceStatus,
    /// 上次连上（毫秒时间戳），没连上过时没有
    pub last_seen: Option<i64>,
    /// 上次往这台电脑发任务的时间：「发到…」默认选最近用过的那台
    pub last_used: Option<i64>,
    pub paired_at: Option<i64>,
    /// 正在下的任务数（电脑离线时是 0）
    pub running: u32,
    /// 等电脑上线的任务数
    pub waiting: u32,
    /// 电脑的地址（按最近连上的排）和端口
    pub hosts: Vec<String>,
    pub port: u16,
}

/// 二维码内容解析出来的样子（扫到以后先给用户看「要连 Sol 的 MacBook Pro 吗」）
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PairLinkInfo {
    pub name: String,
    pub hosts: Vec<String>,
    pub port: u16,
    /// 证书指纹，就是配对以后的设备 id
    pub fingerprint: String,
    /// 这台电脑已经配对过（再扫一次会换一个新令牌）
    pub known: bool,
}

/// App 用 NWBrowser / NSD 发现的一个 `_captae._tcp` 服务
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct DiscoveredService {
    /// 实例名（电脑名）
    pub name: String,
    /// 解析出来的地址（IPv4 或不带方括号的 IPv6）
    pub hosts: Vec<String>,
    pub port: u16,
    /// TXT 记录：`v`、`fp`、`name`
    pub txt: HashMap<String, String>,
}

/// 附近发现的、还没配对过的电脑（「添加电脑」里列出来，点了输 6 位数）
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct NearbyComputer {
    pub name: String,
    pub hosts: Vec<String>,
    pub port: u16,
    /// TXT 里的指纹（没经过验证，只用来认出是哪一台）
    pub fingerprint: Option<String>,
    /// 最后一次发现的时间
    pub seen_at: i64,
}

/// 发给电脑的任务。字段和电脑的白名单一样（保存位置、yt-dlp 参数这些手机给不了）
#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct SendOptions {
    pub url: String,
    /// 「最高可用」/ 2160p / 1080p……（协议值，不翻译）
    pub quality: Option<String>,
    /// MP4 / WebM / MKV / MP3 / AAC / FLAC /「保持原格式」
    pub format: Option<String>,
    pub audio_only: bool,
    /// 电脑上的预设名
    pub preset: Option<String>,
    /// 手机这边已经知道的标题（读取过信息时），只在「等电脑上线」里显示，不发给电脑
    pub title: Option<String>,
}

/// 「等电脑上线」队列里的一条：电脑不在线时加的任务，存在手机的数据目录里，连上后自动发过去
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct PendingTask {
    pub id: u64,
    /// 要发去的电脑（证书指纹）
    pub device: String,
    pub url: String,
    pub title: Option<String>,
    pub quality: Option<String>,
    pub format: Option<String>,
    pub audio_only: bool,
    pub preset: Option<String>,
    pub created_at: i64,
    /// 电脑收到以后拒绝了（链接不对这类）：不再自动发，等用户重试、改到手机或删掉
    pub error: Option<MobileFailure>,
    /// 能不能改成在这台手机上下（iOS 上架版只有直链和 M3U8 能）
    pub can_run_here: bool,
}

/// 往电脑发任务的结果
#[derive(Debug, Clone, uniffi::Enum)]
pub enum SendResult {
    /// 电脑收下了
    Sent { job: MobileJob },
    /// 电脑不在线：放进「等电脑上线」，连上后自动发
    Queued { task: PendingTask },
}

/// 「都改成在这台手机上下」的结果
#[derive(Debug, Clone, uniffi::Record)]
pub struct MoveResult {
    /// 已经在手机上开始的任务（从队列里去掉了）
    pub moved: Vec<MobileJob>,
    /// 留在队列里的（iOS 上架版不能在手机上下的网站，或者加的时候出错了）
    pub kept: Vec<PendingTask>,
}

/// 存到手机的状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SaveState {
    Running,
    Paused,
    Done,
    Failed,
    Cancelled,
}

/// 存到手机（从电脑的素材库把文件下到手机）的进度
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct SaveProgress {
    pub id: u64,
    /// 哪台电脑
    pub device: String,
    pub asset_id: i64,
    /// 存到哪个文件（App 给的路径）
    pub path: String,
    pub got_bytes: u64,
    /// 还不知道时是 0
    pub total_bytes: u64,
    /// 字节 / 秒
    pub speed: u64,
    pub state: SaveState,
    pub error: Option<MobileFailure>,
    /// 下完的时间（毫秒时间戳）：界面写「已存到手机」。下完的记录留 7 天
    pub done_at: Option<i64>,
    /// 下完以后自动导入了这台手机的素材库（`MobileCore::import_file`，带着电脑上的标题、来源、时长、封面）：手机素材库里的 id。导入失败时没有
    pub local_asset_id: Option<i64>,
}

/// 带着设备的任务（合并视图、实时事件用）
#[derive(Debug, Clone, uniffi::Record)]
pub struct DeviceJob {
    /// `"phone"` 或电脑的证书指纹
    pub device: String,
    pub job: MobileJob,
    /// 电脑上的任务是从哪台手机加的（手机名）；电脑上自己加的、这台手机本地的没有
    pub from: Option<String>,
}

/// 合并任务列表里的一组（一台设备）
#[derive(Debug, Clone, uniffi::Record)]
pub struct JobGroup {
    pub device: MobileDevice,
    /// 按 id 从新到旧
    pub jobs: Vec<DeviceJob>,
    /// 等这台电脑上线的任务（这台手机的组里是空的）
    pub pending: Vec<PendingTask>,
}

/// 带着设备的素材
#[derive(Debug, Clone, uniffi::Record)]
pub struct DeviceAsset {
    pub device: String,
    /// 电脑上的素材：`path` 是空的（电脑上的路径不给手机），`thumb` 是缓存到手机上的封面（拿过才有，用 `remote_thumbnail` 拿）
    pub asset: MobileAsset,
}

/// 合并素材库里的一段（一台设备）
#[derive(Debug, Clone, uniffi::Record)]
pub struct LibrarySection {
    pub device: MobileDevice,
    pub items: Vec<DeviceAsset>,
    pub total: u64,
    /// 这台电脑没查到（离线、被撤销）：`items` 是空的
    pub error: Option<MobileFailure>,
}

impl LibrarySection {
    pub fn from_page(device: MobileDevice, page: LibraryPage) -> LibrarySection {
        let id = device.id.clone();
        LibrarySection { device, total: page.total, items: page.items.into_iter().map(|asset| DeviceAsset { device: id.clone(), asset }).collect(), error: None }
    }
}

/// 遥控电脑的事件。和 `CoreListener` 同一个样子，多了设备 id；在 Rust 的后台线程上调，界面自己切主线程
#[uniffi::export(with_foreign)]
pub trait RemoteListener: Send + Sync {
    /// 电脑上某个任务变了（进度最多每 250 毫秒一次）
    fn on_job(&self, job: DeviceJob);
    fn on_removed(&self, device: String, id: u64);
    /// 电脑上下完、进了电脑的素材库
    fn on_finished(&self, device: String, id: u64, title: String, asset_id: Option<i64>);
    /// 电脑的素材库变了
    fn on_library(&self, device: String);
    /// 这台电脑的任务列表整个换了（刚连上、重连上）：重新调 `remote_jobs` / `merged_jobs`
    fn on_resync(&self, device: String);
    /// 一台设备的状态、名字变了（在线 / 离线 / 被撤销……）
    fn on_device(&self, device: MobileDevice);
    /// 设备列表变了（配对了新电脑、删了一台）：重新调 `devices()`
    fn on_devices(&self);
    /// 「等电脑上线」队列变了：重新调 `pending()`
    fn on_pending(&self);
    /// 队列里的一条自动发出去了（电脑上线了）
    fn on_pending_sent(&self, task: PendingTask, job: DeviceJob);
    /// 存到手机的进度、暂停、完成、失败（进度最多每 250 毫秒一次）
    fn on_save(&self, save: SaveProgress);
    /// App 重开后找回了上次没存完的（第一次 `set_active(true)` 时发一次）：把进度条显示回来。
    /// 这些记录是 `Running`，那台电脑连上就自动接着下，之后照常 `on_save`
    fn on_saves_restored(&self, saves: Vec<SaveProgress>);
}
