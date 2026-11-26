//! 冒烟测试：只走 captae-mobile 的公开 API（和 Swift / Kotlin 调的是同一层），在模拟器上跑一遍 M1 的验收项：
//! 1. 直链下载，下到一部分时暂停 2 秒再继续（Range 续传），核对大小 = 服务器的 Content-Length、开头是 `ftyp`
//! 2. M3U8（TS 分片，主列表两种清晰度）：任务停在「选清晰度」，选第二种、马上开始，下完封装成 MP4（手机上没有 ffmpeg，走 Rust 封装）
//! 3. M3U8（fMP4 分片，画面和声音分两路）：下完封装成 MP4
//! 4. 读取信息中途取消；上架版的规矩（YouTube 在电脑上下、网页里找媒体地址）
//! 5. 素材库查询（类型、搜索、分页）
//!
//! 用法：`smoke <测试目录> --base <测试服务器地址> [--platform ios-store|android] [--direct <链接>] [--pause-at 0.3]`
//! - 测试目录的名字里要有 `captae-smoke`（开始前整个删掉重建，名字不对就不删、直接退出，免得删错）
//! - 测试服务器是 work/mobile/m1/serve.py（本机起的，iOS 模拟器用 127.0.0.1，Android 模拟器用 10.0.2.2）
//! - 下完的文件路径打印成 `OUTPUT <名字> <路径>`，电脑上再用 ffprobe 核对时长和音视频轨
//!
//! 全部通过打印 `SMOKE OK`、退出码 0；没通过 1。
use captae_mobile::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEFAULT_DIRECT: &str = "https://download.blender.org/durian/trailer/sintel_trailer-480p.mp4";

/// 打印事件：状态或阶段变了打一行
struct Printer {
    t0: Instant,
    last: Mutex<std::collections::HashMap<u64, (MobileJobState, String)>>,
}

impl CoreListener for Printer {
    fn on_job(&self, j: MobileJob) {
        let mut g = self.last.lock().unwrap();
        let key = (j.state, j.phase.clone());
        if g.get(&j.id) != Some(&key) {
            let why = j.failure.as_ref().map(|f| format!("{}：{}", f.code, f.message)).unwrap_or_default();
            println!("[{:6.2}s] 任务 {} 状态 {:?} 阶段 {} {why}", self.t0.elapsed().as_secs_f64(), j.id, j.state, j.phase);
            g.insert(j.id, key);
        }
    }
    fn on_removed(&self, id: u64) {
        println!("[{:6.2}s] 任务 {id} 被移除", self.t0.elapsed().as_secs_f64());
    }
    fn on_finished(&self, id: u64, title: String, path: String, asset_id: Option<i64>) {
        println!("[{:6.2}s] 任务 {id} 完成：{title} → {path}（素材 {asset_id:?}）", self.t0.elapsed().as_secs_f64());
    }
    fn on_library(&self) {}
    fn on_notice(&self, key: String, title: String, _text: String) {
        println!("[{:6.2}s] 提示 {key}：{title}", self.t0.elapsed().as_secs_f64());
    }
    fn on_resync(&self) {
        println!("[{:6.2}s] 事件积压，重新对齐", self.t0.elapsed().as_secs_f64());
    }
}

fn fail(msg: &str) -> ! {
    println!("SMOKE FAIL：{msg}");
    std::process::exit(1)
}

fn check(ok: bool, msg: &str) {
    if !ok {
        fail(msg);
    }
}

fn arg(args: &mut Vec<String>, name: &str) -> Option<String> {
    let i = args.iter().position(|a| a == name)?;
    let v = args.get(i + 1).cloned();
    args.drain(i..(i + 2).min(args.len()));
    v
}

/// 等任务结束（完成或失败），超时算没通过
fn wait_end(m: &MobileCore, id: u64, secs: u64) -> MobileJob {
    let t0 = Instant::now();
    loop {
        let j = m.job(id).unwrap_or_else(|| fail(&format!("任务 {id} 不见了")));
        match j.state {
            MobileJobState::Done => return j,
            MobileJobState::Failed => fail(&format!("任务 {id} 失败了：{:?}", j.failure)),
            _ => {}
        }
        if t0.elapsed() > Duration::from_secs(secs) {
            fail(&format!("任务 {id} {secs} 秒还没完：{j:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// 文件开头是不是 MP4（第 4–8 字节 `ftyp`）
fn is_mp4(p: &str) -> bool {
    let mut b = [0u8; 8];
    use std::io::Read;
    std::fs::File::open(p).and_then(|mut f| f.read_exact(&mut b)).is_ok() && &b[4..8] == b"ftyp"
}

fn block<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(f)
}

fn main() {
    let t0 = Instant::now();
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let base = arg(&mut args, "--base").unwrap_or_else(|| fail("要给 --base（测试服务器地址）"));
    let platform = match arg(&mut args, "--platform").as_deref() {
        Some("android") => Platform::Android,
        Some("ios-store") | None => Platform::IosStore,
        Some(x) => fail(&format!("不认识的平台 {x}")),
    };
    let direct = arg(&mut args, "--direct").unwrap_or_else(|| DEFAULT_DIRECT.to_string());
    let pause_at: f64 = arg(&mut args, "--pause-at").and_then(|s| s.parse().ok()).unwrap_or(0.3);
    let root = std::path::PathBuf::from(args.first().cloned().unwrap_or_else(|| fail("要给一个测试目录")));
    if !root.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.contains("captae-smoke")) {
        fail("测试目录的名字里要有 captae-smoke");
    }
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();

    let m = MobileCore::new(InitOptions {
        data_dir: root.join("data").display().to_string(),
        downloads_dir: root.join("Downloads").display().to_string(),
        platform,
        language: "zh-CN".into(),
        native_lib_dir: (platform == Platform::Android).then(|| root.join("no-native-libs").display().to_string()),
    })
    .unwrap_or_else(|e| fail(&format!("打开失败：{e}")));
    let info = m.info();
    println!("平台 {} / {}，语言 {}，证书校验 {}，外部程序 {}，数据目录 {}", info.platform, std::env::consts::ARCH, info.language, info.tls, info.runner, root.join("data").display());
    m.set_listener(Arc::new(Printer { t0, last: Mutex::new(Default::default()) }));

    // ---------------------------------------------------------------- 1. 直链 + 暂停继续
    println!("== 1. 直链：{direct}");
    let j = m.add(AddOptions { url: direct.clone(), ..Default::default() }).unwrap_or_else(|e| fail(&format!("加任务失败：{e}")));
    let mut paused = false;
    loop {
        let j = m.job(j.id).unwrap();
        if j.state == MobileJobState::Failed {
            fail(&format!("直链失败：{:?}", j.failure));
        }
        if j.state == MobileJobState::Done {
            break;
        }
        if !paused && j.total_bytes > 0 && j.got_bytes as f64 >= j.total_bytes as f64 * pause_at {
            m.action(j.id, MobileAction::Pause).unwrap();
            std::thread::sleep(Duration::from_secs(2));
            let p = m.job(j.id).unwrap();
            let on_disk = p.path.as_ref().and_then(|p| std::fs::metadata(p).ok()).map(|x| x.len()).unwrap_or(0);
            println!("[{:6.2}s] 暂停中：状态 {:?}，磁盘上已有 {on_disk} 字节；继续", t0.elapsed().as_secs_f64(), p.state);
            check(p.state == MobileJobState::Paused, "暂停以后状态不是 Paused");
            m.action(j.id, MobileAction::Resume).unwrap();
            paused = true;
        }
        if t0.elapsed() > Duration::from_secs(300) {
            fail("直链 300 秒还没下完");
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let done = wait_end(&m, j.id, 10);
    let path = done.path.clone().unwrap();
    let size = std::fs::metadata(&path).unwrap().len();
    let len = block(async { reqwest::Client::new().head(&direct).send().await.ok().and_then(|r| r.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok())) });
    println!("大小：{size} 字节；服务器 Content-Length：{len:?}；MP4 开头：{}；中途暂停过：{paused}", is_mp4(&path));
    check(Some(size) == len && is_mp4(&path) && paused, "直链核对没通过");
    check(!std::path::Path::new(&format!("{path}.aria2")).exists(), "续传标记没删");
    check(m.job_log(j.id).iter().any(|l| l.text.contains("接着下")), "任务记录里没有续传");
    println!("OUTPUT direct {path}");

    // ---------------------------------------------------------------- 2. M3U8（TS，两种清晰度，选第二种）
    let ts = format!("{base}/ts/master.m3u8");
    println!("== 2. M3U8（TS）：{ts}");
    let j = m.add(AddOptions { url: ts.clone(), page_title: Some("测试 M3U8 TS".into()), ..Default::default() }).unwrap_or_else(|e| fail(&format!("加任务失败：{e}")));
    let t1 = Instant::now();
    loop {
        let x = m.job(j.id).unwrap();
        if x.phase == "pick" && x.variants.len() >= 2 {
            println!("清晰度：{}；选第 2 种、马上开始", x.variants.iter().map(|v| format!("{} {}", v.label, v.resolution)).collect::<Vec<_>>().join(" / "));
            m.set_variant(j.id, 1).unwrap();
            m.action(j.id, MobileAction::PickGo).unwrap();
            break;
        }
        if x.state == MobileJobState::Failed || t1.elapsed() > Duration::from_secs(30) {
            fail(&format!("没有停在选清晰度：{x:?}"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let done = wait_end(&m, j.id, 120);
    let p = done.path.clone().unwrap();
    check(is_mp4(&p) && done.variant_index == 1, "TS 封装出来的不是 MP4，或者清晰度不对");
    println!("分片 {} 个，全部完成：{}", done.segments.len(), done.segments.iter().all(|s| *s == 2));
    println!("OUTPUT hls-ts {p}");

    // ---------------------------------------------------------------- 3. M3U8（fMP4，画面和声音分两路）
    let fm = format!("{base}/fmp4/master.m3u8");
    println!("== 3. M3U8（fMP4，声音单独一路）：{fm}");
    let j = m.add(AddOptions { url: fm, page_title: Some("测试 M3U8 fMP4".into()), ..Default::default() }).unwrap_or_else(|e| fail(&format!("加任务失败：{e}")));
    let t1 = Instant::now();
    let done = loop {
        let x = m.job(j.id).unwrap();
        if x.phase == "pick" {
            m.action(j.id, MobileAction::PickGo).unwrap();
        }
        if x.state == MobileJobState::Done {
            break x;
        }
        if x.state == MobileJobState::Failed || t1.elapsed() > Duration::from_secs(120) {
            fail(&format!("fMP4 没下完：{x:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let p = done.path.clone().unwrap();
    check(is_mp4(&p), "fMP4 封装出来的不是 MP4");
    println!("OUTPUT hls-fmp4 {p}");

    // ---------------------------------------------------------------- 4. 读取信息：取消、上架版的规矩
    println!("== 4. 读取信息");
    let slow = format!("{base}/slow/index.m3u8");
    let token = CancelToken::new();
    let t2 = token.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(1));
        t2.cancel();
    });
    let t1 = Instant::now();
    let r = block(m.probe(slow, Some(token)));
    let waited = t1.elapsed().as_secs_f64();
    match &r {
        Err(e) => println!("读取一直没回话的地址，1 秒时取消：{:.2} 秒后返回 {}（{}）", waited, e.detail().code, e.detail().message),
        Ok(p) => fail(&format!("取消了还拿到了结果：{p:?}")),
    }
    check(r.as_ref().is_err_and(|e| e.detail().code == "cancelled") && waited < 2.5, "取消没有马上返回");
    let p = block(m.probe(ts.clone(), None)).unwrap_or_else(|e| fail(&format!("读 M3U8 失败：{e}")));
    println!("读 M3U8：{} 种清晰度，时长 {:?}", p.qualities.len(), p.duration);
    if platform == Platform::IosStore {
        let p = block(m.probe(format!("{base}/page.html"), None)).unwrap_or_else(|e| fail(&format!("读网页失败：{e}")));
        println!("读网页：标题 {:?}，找到 {:?}", p.title, p.media_urls);
        check(p.resolved_url.as_deref() == Some(ts.as_str()), "网页里的 M3U8 没找到");
        let e = block(m.probe("https://www.youtube.com/watch?v=aqz-KE-bpKQ".into(), None)).unwrap_err();
        println!("上架版读 YouTube：{} / {} / 网站 {:?}", e.detail().code, e.detail().action, e.detail().site);
        check(e.detail().code == "unsupported_here", "YouTube 应该说在电脑上下");
    }

    // ---------------------------------------------------------------- 5. 素材库
    println!("== 5. 素材库");
    let all = m.library(LibraryQuery::default()).unwrap();
    for a in &all.items {
        println!("  素材 {}：{} [{}] {} · {:?} 秒 · {}×{} · {} 字节 · 来源 {}", a.id, a.title, a.kind, a.format, a.duration, a.width.unwrap_or(0), a.height.unwrap_or(0), a.bytes, a.site);
    }
    let videos = m.library(LibraryQuery { kind: Some("video".into()), limit: 2, ..Default::default() }).unwrap();
    let page2 = m.library(LibraryQuery { kind: Some("video".into()), limit: 2, offset: 2, ..Default::default() }).unwrap();
    let search = m.library(LibraryQuery { search: Some("fMP4".into()), ..Default::default() }).unwrap();
    println!("全部 {} 条；视频 {} 条（第一页 {} 条、第二页 {} 条）；搜「fMP4」{} 条", all.total, videos.total, videos.items.len(), page2.items.len(), search.items.len());
    check(all.total == 3 && videos.total == 3 && videos.items.len() == 2 && page2.items.len() == 1 && search.items.len() == 1, "素材库查询不对");

    println!("耗时 {:.2} 秒", t0.elapsed().as_secs_f64());
    println!("SMOKE OK");
}
