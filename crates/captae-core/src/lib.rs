//! Captae 核心库。桌面端（Tauri）、命令行和 MCP Server 共用。
//!
//! 外部工具（yt-dlp、ffmpeg、aria2、faster-whisper）一律通过子进程调用，
//! 本库不链接它们的代码。

pub mod agent;
// 只有桌面端用（desktop feature），手机端不编进去，见 Cargo.toml
#[cfg(feature = "desktop")]
pub mod api;
pub mod clipboard;
pub mod config;
pub mod detect;
pub mod douyin;
pub mod engine;
pub mod errors;
pub mod ffmpeg;
pub mod i18n;
pub mod library;
pub mod login;
// 只有桌面端用（desktop feature），手机端不编进去，见 Cargo.toml
#[cfg(feature = "desktop")]
pub mod mcp;
pub mod model;
// 只有桌面端用（desktop feature），手机端不编进去，见 Cargo.toml
#[cfg(feature = "desktop")]
pub mod native_host;
pub mod net;
pub mod presets;
pub mod probe;
pub mod queue;
pub mod recordings;
pub mod sample;
pub mod share;
pub mod sniff;
pub mod subtitle;
pub mod tasks;
pub mod tools;
pub mod updater;
pub mod util;
pub mod webpage;
pub mod whisper_setup;

pub use config::{Paths, Settings};
pub use model::*;
pub use queue::{Core, Options};
