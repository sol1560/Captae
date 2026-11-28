//! 目录约定和用户设置。
//!
//! 设置存成 `<data_dir>/settings.json`，桌面端和 `captae` 命令行共用同一份，
//! 所以在界面里关掉「允许 AI 助手使用」，MCP 进程下一次调用就会被拒绝。

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Paths {
    /// SQLite、设置、control.json、日志
    pub data: PathBuf,
    /// 下载中的临时文件（M3U8 分片等）
    pub work: PathBuf,
    /// 影片 › Captae
    pub movies: PathBuf,
    /// 音乐 › Captae
    pub music: PathBuf,
    /// 图片 › Captae
    pub pictures: PathBuf,
    /// 缩略图缓存
    pub thumbs: PathBuf,
    /// Share to Agent 打包目录
    pub shares: PathBuf,
}

impl Paths {
    /// 数据目录：
    /// - macOS：`~/Library/Application Support/app.Captae.Captae`
    /// - Windows：`%LOCALAPPDATA%\Captae\data`。不放 %APPDATA%（漫游目录）：素材库、缩略图、自动更新的引擎（每个约 100 MB）、
    ///   语音识别模型（几百 MB 到几 GB）不该跟着漫游配置文件在域里的电脑之间同步；也不用 directories 默认的「组织\应用」两层
    /// - Linux：`$XDG_DATA_HOME/captae`（默认 `~/.local/share/captae`）
    ///
    /// 保存位置：系统的「视频」「音乐」「图片」文件夹下的 Captae（Linux 没设 XDG 用户目录时退回 ~/Videos 这些）
    pub fn default_for_user() -> Result<Self> {
        let none = || anyhow::anyhow!(crate::tr!("找不到用户目录"));
        let data = if cfg!(windows) {
            directories::ProjectDirs::from_path(PathBuf::from("Captae")).ok_or_else(none)?.data_local_dir().to_path_buf()
        } else {
            directories::ProjectDirs::from("app", "Captae", "Captae").ok_or_else(none)?.data_dir().to_path_buf()
        };
        let ud = directories::UserDirs::new().ok_or_else(none)?;
        let home = ud.home_dir().to_path_buf();
        let videos = if cfg!(target_os = "macos") { "Movies" } else { "Videos" };
        let movies = ud.video_dir().map(Path::to_path_buf).unwrap_or_else(|| home.join(videos));
        let music = ud.audio_dir().map(Path::to_path_buf).unwrap_or_else(|| home.join("Music"));
        let pics = ud.picture_dir().map(Path::to_path_buf).unwrap_or_else(|| home.join("Pictures"));
        Ok(Self::under(data, movies.join("Captae"), music.join("Captae"), pics.join("Captae")))
    }

    /// 测试和便携模式：全部放在一个目录下面。
    pub fn portable(root: &Path) -> Self {
        Self::under(root.join("data"), root.join("Movies"), root.join("Music"), root.join("Pictures"))
    }

    fn under(data: PathBuf, movies: PathBuf, music: PathBuf, pictures: PathBuf) -> Self {
        Paths {
            work: data.join("work"),
            thumbs: data.join("thumbs"),
            shares: data.join("shares"),
            data,
            movies,
            music,
            pictures,
        }
    }

    pub fn ensure(&self) -> Result<()> {
        for d in [&self.data, &self.work, &self.movies, &self.music, &self.pictures, &self.thumbs, &self.shares] {
            std::fs::create_dir_all(d)?;
        }
        Ok(())
    }

    pub fn db(&self) -> PathBuf {
        self.data.join("library.sqlite")
    }
    pub fn settings(&self) -> PathBuf {
        self.data.join("settings.json")
    }
    pub fn presets(&self) -> PathBuf {
        self.data.join("presets.json")
    }
    /// 桌面端运行时写入 {port, token, pid}，命令行和 MCP 进程靠它找到桌面端。
    pub fn control(&self) -> PathBuf {
        self.data.join("control.json")
    }
    /// 自动更新下载的引擎：`<data>/engines/yt-dlp/<版本>/`
    pub fn engines(&self) -> PathBuf {
        self.data.join("engines")
    }
    /// 在 Captae 里登录过 / 验证过的网站（只有网站和时间，没有 cookie），见 `login::Logins`
    pub fn logins(&self) -> PathBuf {
        self.data.join("logins.json")
    }

    pub fn dir_for_kind(&self, kind: &str) -> &Path {
        match kind {
            "audio" => &self.music,
            "image" => &self.pictures,
            _ => &self.movies,
        }
    }

    /// "~/Movies/Captae/x.mp4" → "影片 › Captae"，界面上的人话路径。
    /// 跟着界面语言，文件夹名照系统在那种语言里的叫法（macOS 英文 Movies › Captae，docs/I18N.md 8.2）；
    /// Windows、Linux 的视频文件夹叫「视频」（英文 Videos），音乐、图片和 macOS 一样
    pub fn human_dir(&self, p: &Path) -> String {
        let movies = if cfg!(target_os = "macos") { crate::tr!("影片 › Captae") } else { crate::tr!("视频 › Captae") };
        let pairs = [(&self.movies, movies), (&self.music, crate::tr!("音乐 › Captae")), (&self.pictures, crate::tr!("图片 › Captae"))];
        for (d, label) in pairs {
            if p.starts_with(d) {
                return label;
            }
        }
        p.parent().map(|d| d.display().to_string()).unwrap_or_default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// 同时下载几个
    pub concurrency: u32,
    /// aria2 每个文件的线程数
    pub threads: u32,
    /// 剪贴板监听
    pub clipboard: bool,
    /// 允许 AI 助手使用 Captae（MCP 总开关）
    pub mcp_on: bool,
    /// 高级模式：agent 可以直接传 ffmpeg / yt-dlp / aria2 参数
    pub adv: bool,
    /// 单独关掉的工具
    pub tools_off: Vec<String>,
    /// M3U8 没选清晰度时几秒后自动用最高
    pub pick_seconds: u32,
    /// faster-whisper 模型
    pub whisper_model: String,
    /// 「AI 模型」：OpenAI 兼容接口（DeepSeek、OpenAI、本地 Ollama……），字幕翻译和素材库分类共用。地址或模型为空就不提供这两样
    pub llm_base_url: String,
    pub llm_model: String,
    /// 密钥：不写进 settings.json，单独存在同一目录的 secrets.json（权限 600），见 `save` / `load`。
    /// 界面拿不到它（序列化时跳过），只拿到 `llm_key_hint`；界面发来的 `llmApiKey` 照常收（填新的密钥）
    #[serde(skip_serializing)]
    pub llm_api_key: String,
    /// 密钥的提示（「…3f9a」），给界面显示「已设置」用。由 `with_hint` 算，不存盘
    #[serde(skip_deserializing)]
    pub llm_key_hint: String,
    /// 这个模型能不能看图：空 = 还不知道 / yes / no（设置里「测试连接」时测出来，见 llm.rs）
    pub llm_vision: String,
    /// 素材库整理过以后，新下载的自动放进分类（categorize.rs）
    pub ai_auto_categorize: bool,
    /// 分类时，标题、简介看不出内容的素材附上封面给模型看
    pub ai_send_covers: bool,
    /// 自定义工具路径，为空时依次找：随包 sidecar → PATH
    pub ytdlp_path: String,
    pub ffmpeg_path: String,
    pub aria2_path: String,
    pub python_path: String,
    /// 下载完成后是否自动用 aria2 加速 yt-dlp 的直链分段
    pub ytdlp_use_aria2: bool,
    /// 自定义保存位置；为空时按类型存到 影片 / 音乐 / 图片 › Captae
    pub save_dir: String,
    /// 上手引导里选的浏览器（chrome / edge / brave / arc），决定打开哪个扩展页
    pub browser: String,
    /// 自定义 JS 运行时（deno / node / bun 的路径），yt-dlp 解 YouTube 的验证要用；
    /// 为空时依次用：安装包自带的 deno → PATH 和 Homebrew 里的 deno、node、bun
    pub js_runtime_path: String,
    /// 每天自动检查一次下载引擎（yt-dlp）有没有新版
    pub auto_update: bool,
    /// 代理：system 跟随系统 / none 不用代理 / manual 手动指定
    pub proxy_mode: String,
    /// 手动代理地址，如 http://127.0.0.1:7890、socks5://127.0.0.1:1080（proxy_mode 为 manual 时用）
    pub proxy_url: String,
    /// 也读取浏览器里的登录状态（yt-dlp `--cookies-from-browser`）：空 = 不读（默认）；
    /// chrome / safari / edge / brave / firefox / vivaldi / opera / chromium。YouTube 照样只在需要时才带
    pub cookies_from_browser: String,
    /// 界面语言：空 = 跟随系统（系统的首选语言都不支持时用英文）；zh-CN / en / zh-TW / ja / ko / es / fr / de。见 i18n.rs
    pub language: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            concurrency: 3,
            threads: 16,
            clipboard: true,
            mcp_on: true,
            adv: false,
            tools_off: vec![],
            pick_seconds: 6,
            whisper_model: "small".into(),
            llm_base_url: String::new(),
            llm_model: String::new(),
            llm_api_key: String::new(),
            llm_key_hint: String::new(),
            llm_vision: String::new(),
            ai_auto_categorize: true,
            ai_send_covers: true,
            ytdlp_path: String::new(),
            ffmpeg_path: String::new(),
            aria2_path: String::new(),
            python_path: String::new(),
            ytdlp_use_aria2: false,
            save_dir: String::new(),
            browser: "chrome".into(),
            js_runtime_path: String::new(),
            auto_update: true,
            proxy_mode: "system".into(),
            proxy_url: String::new(),
            cookies_from_browser: String::new(),
            language: String::new(),
        }
    }
}

impl Settings {
    /// 密钥文件：和 settings.json 同一个目录
    pub fn secrets_path(p: &Path) -> PathBuf {
        p.with_file_name("secrets.json")
    }

    pub fn load(p: &Path) -> Settings {
        let raw: Option<serde_json::Value> = std::fs::read(p).ok().and_then(|b| serde_json::from_slice(&b).ok());
        let mut s: Settings = raw.clone().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
        let sp = Self::secrets_path(p);
        match std::fs::read(&sp).ok().and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok()) {
            Some(v) => s.llm_api_key = v["llmApiKey"].as_str().unwrap_or("").to_string(),
            // 旧版把密钥写在 settings.json 里：挪到 secrets.json，settings.json 去掉它（失败了下次再挪，这次照常用）
            None if !s.llm_api_key.is_empty() && raw.as_ref().is_some_and(|v| v.get("llmApiKey").is_some()) => {
                if let Err(e) = s.save(p) {
                    tracing::warn!("把密钥挪到 secrets.json 失败：{e}");
                }
            }
            None => {}
        }
        s.with_hint()
    }

    /// 算好给界面看的密钥提示
    pub fn with_hint(mut self) -> Self {
        let k = self.llm_api_key.trim();
        self.llm_key_hint = if k.is_empty() {
            String::new()
        } else {
            let tail: String = k.chars().rev().take(4).collect::<Vec<_>>().into_iter().rev().collect();
            format!("…{tail}")
        };
        self
    }

    pub fn save(&self, p: &Path) -> Result<()> {
        let mut v = serde_json::to_value(self)?;
        if let Some(o) = v.as_object_mut() {
            o.remove("llmKeyHint");
        }
        write_private(&Self::secrets_path(p), &serde_json::to_vec_pretty(&serde_json::json!({"llmApiKey": self.llm_api_key.trim()}))?)?;
        let tmp = p.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec_pretty(&v)?)?;
        std::fs::rename(tmp, p)?;
        Ok(())
    }
}

/// 只有自己能读写的文件（unix 600；Windows 上数据目录在 %LOCALAPPDATA%，本来只有这个用户能读）。先写临时文件再换过去
fn write_private(p: &Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    let tmp = p.with_extension("json.tmp");
    let _ = std::fs::remove_file(&tmp);
    let mut o = std::fs::OpenOptions::new();
    o.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut o, 0o600);
    let mut f = o.open(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, p)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 「影片 › Captae」这类人话路径跟着界面语言，英文用 macOS 的文件夹名（docs/I18N.md 8.2）
    #[test]
    fn human_dir_follows_language() {
        let p = Paths::portable(Path::new("/nonexistent/captae-test"));
        let f = p.movies.join("a.mp4");
        let mac = cfg!(target_os = "macos");
        assert_eq!(p.human_dir(&f), if mac { "影片 › Captae" } else { "视频 › Captae" });
        crate::i18n::with_lang(crate::i18n::Lang::En, || {
            assert_eq!(p.human_dir(&f), if mac { "Movies › Captae" } else { "Videos › Captae" });
            assert_eq!(p.human_dir(&p.music.join("a.mp3")), "Music › Captae");
            assert_eq!(p.human_dir(&p.pictures.join("a.jpg")), "Pictures › Captae");
        });
        assert_eq!(p.human_dir(Path::new("/nonexistent/x/a.mp4")), "/nonexistent/x");
    }

    /// 密钥不进 settings.json：存在 secrets.json（600），读回来照样有；旧版写在 settings.json 里的打开时挪过去
    #[test]
    fn api_key_lives_in_secrets_file() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("settings.json");
        let s = Settings { llm_base_url: "https://api.deepseek.com".into(), llm_api_key: "sk-test-1234abcd".into(), ..Default::default() };
        s.save(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(!text.contains("sk-test") && !text.contains("llmApiKey") && !text.contains("llmKeyHint"), "{text}");
        assert!(std::fs::read_to_string(Settings::secrets_path(&p)).unwrap().contains("sk-test-1234abcd"));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(std::fs::metadata(Settings::secrets_path(&p)).unwrap().permissions().mode() & 0o777, 0o600);
        }
        let back = Settings::load(&p);
        assert_eq!((back.llm_api_key.as_str(), back.llm_key_hint.as_str()), ("sk-test-1234abcd", "…abcd"));
        // 给界面的 JSON 里没有密钥，只有提示
        let ui = serde_json::to_value(&back).unwrap();
        assert!(ui.get("llmApiKey").is_none());
        assert_eq!(ui["llmKeyHint"], "…abcd");
        // 界面发来新的密钥照常收
        let patched: Settings = serde_json::from_value(serde_json::json!({"llmApiKey": "sk-new"})).unwrap();
        assert_eq!(patched.llm_api_key, "sk-new");

        // 旧版：密钥在 settings.json 里，没有 secrets.json
        let old = dir.path().join("old").join("settings.json");
        std::fs::create_dir_all(old.parent().unwrap()).unwrap();
        std::fs::write(&old, r#"{"llmBaseUrl":"http://localhost:11434/v1","llmModel":"qwen","llmApiKey":"sk-old-9999"}"#).unwrap();
        let s = Settings::load(&old);
        assert_eq!(s.llm_api_key, "sk-old-9999");
        assert!(!std::fs::read_to_string(&old).unwrap().contains("sk-old"), "打开以后 settings.json 里不再有密钥");
        assert_eq!(Settings::load(&old).llm_api_key, "sk-old-9999");
    }

    /// 默认数据目录（不设 CAPTAE_HOME 时）：Windows 在 %LOCALAPPDATA%\Captae\data，Linux 在 XDG 数据目录，macOS 照旧
    #[test]
    fn default_data_dir() {
        let Ok(p) = Paths::default_for_user() else { return };
        let d = p.data.to_string_lossy().replace('\\', "/");
        if cfg!(target_os = "macos") {
            assert!(d.ends_with("Library/Application Support/app.Captae.Captae"), "{d}");
        } else if cfg!(windows) {
            assert!(d.ends_with("/Captae/data") && !d.contains("Roaming"), "{d}");
        } else {
            assert!(d.ends_with("/captae"), "{d}");
        }
        assert!(p.movies.ends_with("Captae"));
    }
}
