//! 浏览器插件推来的嗅探结果。桌面端侧栏「浏览器里发现的」就是这个列表。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Sniffed {
    /// 文件名或标题
    pub t: String,
    /// 在线视频 / MP4 视频 / 图片
    pub m: String,
    pub host: String,
    pub url: String,
    /// 图标：list(m3u8) / film / music / image
    pub i: String,
    pub page_url: String,
    pub page_title: String,
    pub size: Option<u64>,
    pub content_type: Option<String>,
    /// M3U8 主列表里的清晰度（插件已解析）
    pub variants: Vec<String>,
    pub headers: BTreeMap<String, String>,
    pub at: i64,
}

impl Sniffed {
    pub fn from_url(url: &str, content_type: Option<&str>) -> Option<Sniffed> {
        let d = crate::detect::detect(url)?;
        let kind = content_type.and_then(crate::detect::from_content_type).unwrap_or(d.kind);
        let name = url::Url::parse(url).ok()?.path_segments()?.next_back()?.to_string();
        let ext = d.ext.clone().unwrap_or_default();
        // m 是给人看的类型说法（侧栏「浏览器里发现的」），生成时按当时的界面语言写
        let (m, i) = match kind {
            crate::detect::Kind::Hls => (crate::tr!("在线视频"), "list"),
            _ => match crate::detect::media_kind_of_ext(&ext) {
                "image" => (crate::tr!("图片§浏览器里发现的一条的类型"), "image"),
                "audio" => (crate::tr!("{format} 音频", format = ext.to_uppercase()), "music"),
                _ if ext.is_empty() => (crate::tr!("媒体 视频§浏览器里发现的视频，链接里没有扩展名、说不出格式时"), "film"),
                _ => (crate::tr!("{format} 视频", format = ext.to_uppercase()), "film"),
            },
        };
        Some(Sniffed { t: name, m, host: d.host, url: url.to_string(), i: i.into(), content_type: content_type.map(str::to_string), at: crate::util::now_ms(), ..Default::default() })
    }
}
