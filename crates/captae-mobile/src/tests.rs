//! captae-mobile 的 API 测试（在电脑上跑，用的是手机的 Runner：iOS 那份什么外部程序都没有）。
//! 要网络的部分只连本机临时起的小服务器。

use super::*;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::time::{Duration, Instant};

/// 界面语言是进程里的全局状态（`i18n::apply`），建 Core 时按设置改一次。测试并发跑时，建 Core 和核对语言的测试都拿这把锁，
/// 免得别的测试建 Core 时把语言改掉（remote/tests.rs 也用它）
static LANG: Mutex<()> = Mutex::new(());

pub(crate) fn lang_lock() -> std::sync::MutexGuard<'static, ()> {
    LANG.lock().unwrap_or_else(|e| e.into_inner())
}

fn open(dir: &Path, platform: Platform, language: &str) -> Arc<MobileCore> {
    let _g = lang_lock();
    open_unlocked(dir, platform, language)
}

fn open_unlocked(dir: &Path, platform: Platform, language: &str) -> Arc<MobileCore> {
    MobileCore::new(InitOptions {
        data_dir: dir.join("data").display().to_string(),
        downloads_dir: dir.join("dl").display().to_string(),
        platform,
        language: language.into(),
        native_lib_dir: None,
    })
    .unwrap()
}

fn code(e: &MobileError) -> &str {
    &e.detail().code
}

/// 本机的小服务器：`/page.html` 是带 <video> 的网页，`/v/index.m3u8` 是分段列表，`/slow…` 收下请求不回话，别的 404
fn serve() -> u16 {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let req = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
                let (status, ct, body) = match path.as_str() {
                    "/page.html" => ("200 OK", "text/html; charset=utf-8", r#"<html><head><title>演示网页</title></head><body><video src="/v/index.m3u8"></video></body></html>"#.to_string()),
                    "/empty.html" => ("200 OK", "text/html", "<html><title>空</title></html>".to_string()),
                    "/v/index.m3u8" => ("200 OK", "application/vnd.apple.mpegurl", "#EXTM3U\n#EXT-X-TARGETDURATION:2\n#EXTINF:2.0,\nseg0.ts\n#EXTINF:2.0,\nseg1.ts\n#EXT-X-ENDLIST\n".to_string()),
                    p if p.starts_with("/slow") => {
                        std::thread::sleep(Duration::from_secs(30));
                        return;
                    }
                    _ => ("404 Not Found", "text/plain", "no".to_string()),
                };
                let _ = write!(s, "HTTP/1.1 {status}\r\nContent-Type: {ct}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
            });
        }
    });
    port
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(f)
}

#[test]
fn init_settings_and_info() {
    let d = tempfile::tempdir().unwrap();
    let m = open(d.path(), Platform::IosStore, "");
    let i = m.info();
    assert_eq!((i.platform.as_str(), i.runner.as_str()), ("ios-store", "in-process"));
    let s = m.settings();
    // 第一次打开：不用代理、不自动更新，默认清晰度和格式是协议值
    assert_eq!((s.proxy_mode.as_str(), s.auto_update, s.default_quality.as_str(), s.default_format.as_str()), ("none", false, "最高可用", "MP4"));
    // iOS 上打开自动更新也不算；超出范围的收回来
    let s2 = m.set_settings(MobileSettings { concurrency: 99, auto_update: true, proxy_mode: "bogus".into(), default_quality: "720p".into(), ..s.clone() }).unwrap();
    assert_eq!((s2.concurrency, s2.auto_update, s2.proxy_mode.as_str(), s2.default_quality.as_str()), (10, false, "none", "720p"));
    drop(m);
    // 重新打开还在（mobile.json、settings.json）
    let m = open(d.path(), Platform::IosStore, "");
    assert_eq!(m.settings().default_quality, "720p");
    assert_eq!(m.settings().concurrency, 10);
}

#[test]
fn android_allows_auto_update_and_finds_tools() {
    let d = tempfile::tempdir().unwrap();
    let lib = d.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    std::fs::write(lib.join("libffmpeg.so"), b"").unwrap();
    let _g = lang_lock();
    let m = MobileCore::new(InitOptions {
        data_dir: d.path().join("data").display().to_string(),
        downloads_dir: d.path().join("dl").display().to_string(),
        platform: Platform::Android,
        language: String::new(),
        native_lib_dir: Some(lib.display().to_string()),
    })
    .unwrap();
    assert_eq!(m.info().runner, "android");
    let tb = m.core.toolbox();
    assert!(tb.has(captae_core::tools::Tool::Ffmpeg));
    assert!(!tb.has(captae_core::tools::Tool::Aria2));
    assert_eq!(tb.path(captae_core::tools::Tool::Ffmpeg).unwrap(), lib.join("libffmpeg.so"));
    let s = m.settings();
    assert!(m.set_settings(MobileSettings { auto_update: true, ..s }).unwrap().auto_update);
}

/// 上架版：YouTube 这类在电脑上下；别的网页先读取信息；直链、M3U8 照常加
#[test]
fn ios_store_rules() {
    let d = tempfile::tempdir().unwrap();
    let m = open(d.path(), Platform::IosStore, "");
    let e = m.add(AddOptions { url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ".into(), ..Default::default() }).unwrap_err();
    assert_eq!((code(&e), e.detail().action.as_str(), e.detail().site.as_deref()), ("unsupported_here", "use_computer", Some("YouTube")));
    let e = m.add(AddOptions { url: "https://example.com/some/page".into(), ..Default::default() }).unwrap_err();
    assert_eq!((code(&e), e.detail().action.as_str()), ("probe_first", "probe"));
    let e = block(m.probe("https://b23.tv/abc".into(), None)).unwrap_err();
    assert_eq!(code(&e), "unsupported_here");
    // 完整版收下（之后交给 yt-dlp，现在 iOS 上没有，任务会失败成 tool）
    let d2 = tempfile::tempdir().unwrap();
    let full = open(d2.path(), Platform::IosFull, "");
    let j = full.add(AddOptions { url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ".into(), ..Default::default() }).unwrap();
    // 默认清晰度、格式只给网页链接
    assert!(j.format.starts_with("MP4"), "{}", j.format);
    let t0 = Instant::now();
    loop {
        let j = full.job(j.id).unwrap();
        if j.state == MobileJobState::Failed {
            assert_eq!(j.failure.unwrap().code, "tool");
            break;
        }
        assert!(t0.elapsed() < Duration::from_secs(20), "任务没有失败：{j:?}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// 上架版读网页：找到网页里写着的 M3U8，信息是它的，标题用网页的
#[test]
fn ios_store_probe_finds_media_in_page() {
    let port = serve();
    let d = tempfile::tempdir().unwrap();
    let m = open(d.path(), Platform::IosStore, "");
    let p = block(m.probe(format!("http://127.0.0.1:{port}/page.html"), None)).unwrap();
    assert_eq!(p.resolved_url.as_deref(), Some(format!("http://127.0.0.1:{port}/v/index.m3u8").as_str()));
    assert_eq!(p.title.as_deref(), Some("演示网页"));
    assert_eq!(p.kind, "page");
    assert_eq!(p.duration, Some(4.0));
    let e = block(m.probe(format!("http://127.0.0.1:{port}/empty.html"), None)).unwrap_err();
    assert_eq!(code(&e), "unsupported_here");
}

/// 读取信息中途取消：马上返回 cancelled
#[test]
fn probe_cancel() {
    let port = serve();
    let d = tempfile::tempdir().unwrap();
    let m = open(d.path(), Platform::Android, "");
    let t = CancelToken::new();
    let t2 = t.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(300));
        t2.cancel();
    });
    let t0 = Instant::now();
    let e = block(m.probe(format!("http://127.0.0.1:{port}/slow/index.m3u8"), Some(t.clone()))).unwrap_err();
    assert_eq!(code(&e), "cancelled");
    assert!(t0.elapsed() < Duration::from_secs(3), "{:?}", t0.elapsed());
    assert!(t.is_cancelled());
    // 已经取消的 token：一开始就返回
    let e = block(m.probe(format!("http://127.0.0.1:{port}/slow/x.m3u8"), Some(t))).unwrap_err();
    assert_eq!(code(&e), "cancelled");
}

/// 任务失败带着错误码，事件推给回调，动作、记录都能用
#[test]
fn job_failure_events_and_actions() {
    struct L(std::sync::mpsc::Sender<String>);
    impl CoreListener for L {
        fn on_job(&self, j: MobileJob) {
            let _ = self.0.send(format!("job {} {:?}", j.id, j.state));
        }
        fn on_removed(&self, id: u64) {
            let _ = self.0.send(format!("removed {id}"));
        }
        fn on_finished(&self, id: u64, _: String, _: String, _: Option<i64>) {
            let _ = self.0.send(format!("finished {id}"));
        }
        fn on_library(&self) {
            let _ = self.0.send("library".into());
        }
        fn on_notice(&self, key: String, _: String, _: String) {
            let _ = self.0.send(format!("notice {key}"));
        }
        fn on_resync(&self) {}
    }
    let d = tempfile::tempdir().unwrap();
    let m = open(d.path(), Platform::IosStore, "");
    let (tx, rx) = std::sync::mpsc::channel();
    m.set_listener(Arc::new(L(tx)));
    // 没人听的端口：连不上，归成 network
    let j = m.add(AddOptions { url: "http://127.0.0.1:9/clip.mp4".into(), ..Default::default() }).unwrap();
    let t0 = Instant::now();
    let failed = loop {
        let j = m.job(j.id).unwrap();
        if j.state == MobileJobState::Failed {
            break j;
        }
        assert!(t0.elapsed() < Duration::from_secs(60), "{j:?}");
        std::thread::sleep(Duration::from_millis(100));
    };
    let f = failed.failure.unwrap();
    assert_eq!((f.code.as_str(), f.action.as_str()), ("network", "retry"));
    assert!(!m.job_log(j.id).is_empty());
    let seen: Vec<String> = rx.try_iter().collect();
    assert!(seen.iter().any(|s| s == &format!("job {} Failed", j.id)), "{seen:?}");
    m.action(j.id, MobileAction::Remove).unwrap();
    assert!(m.job(j.id).is_none());
    let seen: Vec<String> = rx.recv_timeout(Duration::from_secs(2)).into_iter().chain(rx.try_iter()).collect();
    assert!(seen.iter().any(|s| s == &format!("removed {}", j.id)), "{seen:?}");
    assert!(m.action(424242, MobileAction::Pause).is_err());
}

#[test]
fn library_query_delete() {
    let d = tempfile::tempdir().unwrap();
    let m = open(d.path(), Platform::IosStore, "");
    let dl = d.path().join("dl");
    for (i, (kind, title)) in [("video", "Sintel 预告片"), ("video", "Big Buck Bunny"), ("audio", "测试声音")].iter().enumerate() {
        let p = dl.join(format!("f{i}.bin"));
        std::fs::write(&p, b"x").unwrap();
        let thumb = d.path().join(format!("t{i}.jpg"));
        std::fs::write(&thumb, b"j").unwrap();
        m.core
            .lib
            .add(&captae_core::library::NewAsset { kind: kind.to_string(), title: title.to_string(), path: p, site: "本地".into(), format: "MP4".into(), size: 1, thumb: Some(thumb), ..Default::default() })
            .unwrap();
    }
    let all = m.library(LibraryQuery::default()).unwrap();
    assert_eq!(all.total, 3);
    let v = m.library(LibraryQuery { kind: Some("video".into()), limit: 1, ..Default::default() }).unwrap();
    assert_eq!((v.total, v.items.len()), (2, 1));
    let v2 = m.library(LibraryQuery { kind: Some("video".into()), limit: 1, offset: 1, ..Default::default() }).unwrap();
    assert_ne!(v.items[0].id, v2.items[0].id);
    let s = m.library(LibraryQuery { search: Some("Sintel".into()), ..Default::default() }).unwrap();
    assert_eq!(s.items.len(), 1);
    let a = &s.items[0];
    assert!(a.exists);
    assert_eq!(m.asset(a.id).unwrap().unwrap().title, "Sintel 预告片");
    let thumb = m.thumbnail_path(a.id).unwrap();
    m.delete_asset(a.id, true).unwrap();
    assert!(!Path::new(&a.path).exists() && !Path::new(&thumb).exists());
    assert!(m.asset(a.id).unwrap().is_none());
    assert_eq!(m.library(LibraryQuery::default()).unwrap().total, 2);
    // 只从素材库去掉，文件留着
    let b = &m.library(LibraryQuery { kind: Some("audio".into()), ..Default::default() }).unwrap().items[0];
    m.delete_asset(b.id, false).unwrap();
    assert!(Path::new(&b.path).exists());
}

/// 换语言：之后生成的说法用新语言（这个测试单独改全局语言，只断言错误码和语言代码，不断言别的测试的文字）
#[test]
fn language() {
    let _g = lang_lock();
    let d = tempfile::tempdir().unwrap();
    let m = open_unlocked(d.path(), Platform::IosStore, "en");
    assert_eq!(m.info().language, "en");
    assert_eq!(m.set_language("ja".into()).unwrap(), "ja");
    assert_eq!(m.settings().language, "ja");
}
