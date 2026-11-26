//! `<数据目录>/remote/state.json`：开关、端口、已配对的手机（令牌只存哈希）。目录 0700、文件 0600。

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Store {
    /// 「让手机连接这台电脑」
    pub enabled: bool,
    /// 上次监听的端口（手机记着它，下次还用这个）
    pub port: Option<u16>,
    pub devices: Vec<Device>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Device {
    pub id: String,
    pub name: String,
    /// ios / android / other
    pub platform: String,
    /// 设备令牌的 SHA-256（十六进制）。令牌本身只在配对那一刻发给手机
    pub token_hash: String,
    pub paired_at: i64,
    #[serde(default)]
    pub last_seen: Option<i64>,
}

impl Store {
    pub fn load(dir: &Path) -> Store {
        std::fs::read(dir.join("state.json")).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }

    /// 先写临时文件再改名（写到一半退出不会留下坏文件）
    pub fn save(&self, dir: &Path) -> Result<()> {
        private_dir(dir)?;
        let tmp = dir.join("state.json.tmp");
        write_private(&tmp, &serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, dir.join("state.json"))?;
        Ok(())
    }
}

/// 建目录，权限 0700
pub fn private_dir(dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

/// 写文件，一建出来就是 0600（不是先写好再改权限）
pub fn write_private(p: &Path, b: &[u8]) -> Result<()> {
    use std::io::Write;
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        o.mode(0o600);
    }
    let mut f = o.open(p)?;
    f.write_all(b)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}
