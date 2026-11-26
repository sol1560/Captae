//! 手机上记住的东西（`<数据目录>/remote/`）：
//! - `computers.json`：配对过的电脑（名字、改的名字、指纹、地址、端口、电脑给的编号、时间）。**没有令牌**：令牌只经 `SecureStore` 回调存进钥匙串 / Keystore。
//! - `pending.json`：「等电脑上线」的任务。
//! - `saves.json`：「存到手机」的记录（没下完的 App 重开后接着下；下完的留 7 天，界面显示「已存到手机」）。
//! 都是整个文件写到临时文件再改名，写到一半被结束也不会坏。

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// 一台配对过的电脑（不含令牌）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Computer {
    /// 证书指纹：设备 id
    pub fp: String,
    /// 电脑自己报的名字
    pub name: String,
    /// 用户在手机上改的名字
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nickname: Option<String>,
    pub hosts: Vec<String>,
    pub port: u16,
    /// 电脑给这台手机编的号（撤销用，电脑那边看得到）
    pub device: String,
    pub paired_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used: Option<i64>,
    /// 在电脑上被撤销了（令牌已经删了），等用户重新配对或删掉
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub revoked: bool,
}

/// 队列里的一条（`PendingTask` 存盘的样子）
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Pending {
    pub id: u64,
    pub device: String,
    pub url: String,
    pub title: Option<String>,
    pub quality: Option<String>,
    pub format: Option<String>,
    pub audio_only: bool,
    pub preset: Option<String>,
    pub created_at: i64,
    /// 电脑拒绝时的错误代码和说法（不再自动发）
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

/// 「存到手机」的一条记录（`SaveProgress` 存盘的样子）。进度不实时写盘，重开时按文件实际大小算
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SaveRec {
    pub id: u64,
    pub device: String,
    pub asset_id: i64,
    pub path: String,
    pub got: u64,
    pub total: u64,
    /// running / paused / done / failed（取消了的不存）
    pub state: String,
    pub created_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    /// 下完导入手机素材库以后的素材 id
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_asset_id: Option<i64>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct SavesFile {
    next_id: u64,
    saves: Vec<SaveRec>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct ComputersFile {
    computers: Vec<Computer>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct PendingFile {
    next_id: u64,
    tasks: Vec<Pending>,
}

pub struct Store {
    dir: PathBuf,
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

fn read_json<T: Default + for<'de> Deserialize<'de>>(path: &Path) -> T {
    std::fs::read(path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

impl Store {
    pub fn open(dir: PathBuf) -> std::io::Result<Store> {
        std::fs::create_dir_all(&dir)?;
        Ok(Store { dir })
    }

    pub fn computers(&self) -> Vec<Computer> {
        read_json::<ComputersFile>(&self.dir.join("computers.json")).computers
    }

    pub fn save_computers(&self, list: &[Computer]) -> std::io::Result<()> {
        let f = ComputersFile { computers: list.to_vec() };
        write_atomic(&self.dir.join("computers.json"), &serde_json::to_vec_pretty(&f)?)
    }

    /// 队列和下一个编号
    pub fn pending(&self) -> (Vec<Pending>, u64) {
        let f = read_json::<PendingFile>(&self.dir.join("pending.json"));
        let next = f.next_id.max(f.tasks.iter().map(|t| t.id + 1).max().unwrap_or(1)).max(1);
        (f.tasks, next)
    }

    pub fn save_pending(&self, tasks: &[Pending], next_id: u64) -> std::io::Result<()> {
        let f = PendingFile { next_id, tasks: tasks.to_vec() };
        write_atomic(&self.dir.join("pending.json"), &serde_json::to_vec_pretty(&f)?)
    }
}

impl Store {
    /// 存到手机的记录和下一个编号
    pub fn saves(&self) -> (Vec<SaveRec>, u64) {
        let f = read_json::<SavesFile>(&self.dir.join("saves.json"));
        let next = f.next_id.max(f.saves.iter().map(|t| t.id + 1).max().unwrap_or(1)).max(1);
        (f.saves, next)
    }

    pub fn save_saves(&self, saves: &[SaveRec], next_id: u64) -> std::io::Result<()> {
        let f = SavesFile { next_id, saves: saves.to_vec() };
        write_atomic(&self.dir.join("saves.json"), &serde_json::to_vec_pretty(&f)?)
    }
}

/// 安全存储里令牌的键
pub fn token_key(fp: &str) -> String {
    format!("captae.remote.token.{fp}")
}
