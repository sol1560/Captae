//! Captae 远程控制：手机在同一个 Wi-Fi 下遥控电脑上的 Captae。
//!
//! - `server`（默认开）：桌面端的局域网 HTTPS 服务、配对、白名单接口、实时事件、Bonjour 广播。见 [`server`]。
//! - `client`（默认开）：手机端的客户端，按证书指纹钉死连接。见 [`client`] 和 CLIENT.md。
//! - `discover`：用 Rust 做 Bonjour 发现（iOS 不开，用系统的 NWBrowser）。
//!
//! 设计见 work/mobile/PLAN.md 5.3，结果和实测记录见 work/mobile/M2.md。

pub mod proto;

#[cfg(feature = "server")]
pub mod server;

#[cfg(feature = "client")]
pub mod client;

#[cfg(feature = "discover")]
pub mod discover;
