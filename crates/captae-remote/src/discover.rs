//! 用 Rust 做 Bonjour 发现（feature `discover`，mdns-sd 自己收发组播）。
//! iOS 上收发组播要 Apple 特批的权限（com.apple.developer.networking.multicast），所以 iOS 不用这里，改用系统的 `NWBrowser`
//! 找 `_captae._tcp`（Info.plist 写 `NSBonjourServices`），拿到地址、端口和 TXT 里的 fp 以后照样交给 [`crate::client`]。
//! Android 可以用这里，也可以用系统的 NSD。

use crate::proto;
use anyhow::Result;
use std::time::{Duration, Instant};

/// 发现的一台电脑。TXT 里的 fp 没有经过验证，只用来认出「这是配对过的哪一台」；连接时照样按配对时的指纹钉死
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// 电脑名（TXT 的 name，没有就用实例名）
    pub name: String,
    pub fp: Option<String>,
    pub hosts: Vec<String>,
    pub port: u16,
}

/// 找 `wait` 这么久，回找到的电脑（按指纹去重）
pub async fn discover(wait: Duration) -> Result<Vec<Found>> {
    tokio::task::spawn_blocking(move || {
        let mdns = mdns_sd::ServiceDaemon::new()?;
        let rx = mdns.browse(proto::SERVICE_TYPE)?;
        let until = Instant::now() + wait;
        let mut out: Vec<Found> = vec![];
        while let Some(left) = until.checked_duration_since(Instant::now()) {
            let Ok(ev) = rx.recv_timeout(left) else { break };
            if let mdns_sd::ServiceEvent::ServiceResolved(info) = ev {
                let mut hosts: Vec<String> = info.get_addresses().iter().map(|a| a.to_string()).collect();
                hosts.sort_by_key(|h| h.contains(':'));
                let inst = info.get_fullname().trim_end_matches(proto::SERVICE_TYPE).trim_end_matches('.').to_string();
                let f = Found {
                    name: info.get_property_val_str("name").map(str::to_string).unwrap_or(inst),
                    fp: info.get_property_val_str("fp").map(str::to_string),
                    hosts,
                    port: info.get_port(),
                };
                match out.iter_mut().find(|x| x.fp.is_some() && x.fp == f.fp) {
                    Some(x) => *x = f,
                    None => out.push(f),
                }
            }
        }
        let _ = mdns.shutdown();
        Ok(out)
    })
    .await?
}
