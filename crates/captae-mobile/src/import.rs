//! 把手机上已经有的文件放进这台手机的素材库（`MobileCore::import_file`）。
//! 用在：从电脑「存到手机」下完（`RemoteHub` 自动调，带上电脑素材库里的标题、来源、时长等），以后也可以给「从相册 / 文件导入」用。
//! 文件不复制、不移动：素材记的就是这个路径。App 存进相册以后要留着这个文件（删了素材就显示「文件不在了」）。

use crate::{MobileAsset, MobileCore, MobileError};
use captae_core::library::NewAsset;
use captae_core::tr;
use std::path::{Path, PathBuf};

/// 导入时已经知道的信息（都可以不给：类型按扩展名猜，时长、宽高从 MP4 / M4A 的文件头里读）
#[derive(Debug, Clone, Default, PartialEq, uniffi::Record)]
pub struct ImportInfo {
    pub title: Option<String>,
    /// video / audio / image；不给时按扩展名猜
    pub kind: Option<String>,
    pub description: String,
    /// 原来下载的链接
    pub source_url: String,
    /// 来源网页
    pub page_url: String,
    /// 来源网站名（存原文，显示时翻译；电脑素材库给的就是原文）
    pub site: String,
    pub uploader: String,
    /// MP4 / MP3……；不给时用扩展名
    pub format: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// 秒
    pub duration: Option<f64>,
    pub tags: Vec<String>,
    /// 封面图片文件（比如电脑素材的封面缓存）：复制一份到素材库自己的缩略图目录，原文件删了也不影响
    pub thumb_path: Option<String>,
    pub live: bool,
}

fn err(msg: String) -> MobileError {
    crate::types::to_mobile(&captae_core::errors::coded("import", msg), "")
}

impl MobileCore {
    /// `import_file` 的实际做法（RemoteHub 在后台线程上直接调）
    pub(crate) fn import_path(&self, path: &Path, info: ImportInfo) -> Result<MobileAsset, MobileError> {
        let meta = std::fs::metadata(path).map_err(|_| err(tr!("要导入的文件不在了")))?;
        if !meta.is_file() {
            return Err(err(tr!("要导入的不是文件")));
        }
        // 同一个文件已经在素材库里：直接给它（存到手机中途重开、续完以后再导入一次也不会多一条）
        if let Some(a) = self.core.lib.by_path(path)? {
            return Ok(MobileAsset::from(&a));
        }
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_string();
        let kind = info.kind.filter(|k| matches!(k.as_str(), "video" | "audio" | "image")).unwrap_or_else(|| match captae_core::detect::media_kind_of_ext(&ext) {
            k @ ("video" | "audio" | "image") => k.to_string(),
            _ => "video".to_string(),
        });
        // MP4 / M4A 从文件头读时长、宽高（手机上没有 ffprobe），给了的优先
        let probed = captae_core::engine::remux::mp4_info(path);
        let title = info.title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
        let thumb = info.thumb_path.as_deref().map(Path::new).filter(|p| p.is_file()).and_then(|src| {
            let dst: PathBuf = self.core.paths.thumbs.join(format!("import-{}.jpg", captae_core::util::url_key(&path.display().to_string())));
            std::fs::create_dir_all(&self.core.paths.thumbs).ok()?;
            std::fs::copy(src, &dst).ok().map(|_| dst)
        });
        let a = NewAsset {
            kind,
            title,
            description: info.description,
            source_url: info.source_url,
            page_url: info.page_url,
            site: info.site,
            uploader: info.uploader,
            path: path.to_path_buf(),
            format: info.format.filter(|f| !f.is_empty()).unwrap_or_else(|| ext.to_uppercase()),
            width: info.width.or(probed.as_ref().and_then(|p| p.width)),
            height: info.height.or(probed.as_ref().and_then(|p| p.height)),
            duration: info.duration.or(probed.as_ref().and_then(|p| p.duration)),
            size: meta.len(),
            thumb,
            tags: info.tags,
            parent_id: None,
            live: info.live,
        };
        let id = self.core.lib.add(&a)?;
        self.core.emit_library();
        let asset = self.core.lib.get(id)?.ok_or_else(|| err(tr!("要导入的文件不在了")))?;
        Ok(MobileAsset::from(&asset))
    }
}

#[uniffi::export]
impl MobileCore {
    /// 把手机上的一个文件放进这台手机的素材库（不复制、不移动文件），返回素材。同一个文件导入过就返回原来那条。
    /// 素材库变了会回调 `CoreListener.on_library`
    pub fn import_file(&self, path: String, info: ImportInfo) -> Result<MobileAsset, MobileError> {
        self.import_path(Path::new(&path), info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InitOptions, LibraryQuery, Platform};

    #[test]
    fn import_keeps_info_and_is_idempotent() {
        let d = tempfile::tempdir().unwrap();
        let core = {
            let _g = crate::tests::lang_lock();
            MobileCore::new(InitOptions {
                data_dir: d.path().join("data").display().to_string(),
                downloads_dir: d.path().join("dl").display().to_string(),
                platform: Platform::IosStore,
                language: "zh-CN".into(),
                native_lib_dir: None,
            })
            .unwrap()
        };
        let f = d.path().join("saved/sintel.mp4");
        std::fs::create_dir_all(f.parent().unwrap()).unwrap();
        std::fs::write(&f, vec![7u8; 1234]).unwrap();
        let cover = d.path().join("cover.jpg");
        std::fs::write(&cover, b"jpeg").unwrap();
        let info = ImportInfo {
            title: Some("Sintel 预告片".into()),
            source_url: "https://download.blender.org/durian/trailer/sintel_trailer-480p.mp4".into(),
            page_url: "https://durian.blender.org/".into(),
            site: "本地".into(),
            uploader: "Blender".into(),
            format: Some("MP4".into()),
            width: Some(854),
            height: Some(480),
            duration: Some(52.2),
            tags: vec!["开放电影".into()],
            thumb_path: Some(cover.display().to_string()),
            ..Default::default()
        };
        let a = core.import_file(f.display().to_string(), info.clone()).unwrap();
        assert_eq!((a.kind.as_str(), a.title.as_str(), a.bytes, a.width, a.duration), ("video", "Sintel 预告片", 1234, Some(854), Some(52.2)));
        assert_eq!((a.url.as_str(), a.uploader.as_str(), a.tags.clone()), (info.source_url.as_str(), "Blender", vec!["开放电影".to_string()]));
        assert_eq!(a.path, f.display().to_string());
        // 封面复制进了素材库自己的目录，原来的删了也还在
        let t = PathBuf::from(a.thumb.clone().unwrap());
        std::fs::remove_file(&cover).unwrap();
        assert!(t.is_file() && t.starts_with(d.path().join("data")));
        // 同一个文件再导入：还是这一条
        let again = core.import_file(f.display().to_string(), ImportInfo::default()).unwrap();
        assert_eq!(again.id, a.id);
        assert_eq!(core.library(LibraryQuery::default()).unwrap().total, 1);
        // 什么都不给：标题用文件名，类型按扩展名
        let mp3 = d.path().join("saved/歌.mp3");
        std::fs::write(&mp3, b"x").unwrap();
        let b = core.import_file(mp3.display().to_string(), ImportInfo::default()).unwrap();
        assert_eq!((b.kind.as_str(), b.title.as_str(), b.format.as_str()), ("audio", "歌", "MP3"));
        // 文件不在
        let e = core.import_file(d.path().join("nope.mp4").display().to_string(), ImportInfo::default()).unwrap_err();
        assert_eq!(e.detail().code, "import");
    }
}
