//! 回归测试：重开以后，已经结束（完成 / 失败）的任务原样留着，不会被重新排队、再下一遍。
//!
//! 原来的毛病：`restore_queue` 每恢复一条都先 `add`（`add` 里会把整个队列写进 queue.json，那时这一条还是「排队 / 读取」），
//! 再在内存里改回「完成」，这一步不写盘。所以最后恢复的那一条（文件里排在最后的，就是最新完成的那条）在 queue.json 里一直是 queue/probe；
//! 打开以后要是什么都没下就退出（手机上被系统结束、桌面端打开看一眼就关），下次打开它就被当成「退出时还在读取」重新下一遍，素材库里多一条。
//!
//! 这里用真的直链下载走一遍完成流程，再照它的样子补上 M3U8、yt-dlp 两种完成的任务，三种轮流放在文件最后，各重开几次，
//! 每次都核对：任务状态、queue.json 里的状态、素材数量都不变，测试服务器也没有再收到请求。只用本机的小服务器，不连外网。

use captae_core::library::NewAsset;
use captae_core::tools::InProcessRunner;
use captae_core::*;
use serde_json::Value;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 本机直链服务器：任何路径都回同一份数据，记下收到了几次 GET
fn serve(hits: Arc<AtomicUsize>) -> u16 {
    let data: Vec<u8> = (0..256 * 1024).map(|i| (i * 13 % 251) as u8).collect();
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = l.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for s in l.incoming() {
            let Ok(mut s) = s else { continue };
            let (data, hits) = (data.clone(), hits.clone());
            std::thread::spawn(move || {
                let mut buf = [0u8; 4096];
                let n = s.read(&mut buf).unwrap_or(0);
                let get = !String::from_utf8_lossy(&buf[..n]).starts_with("HEAD");
                if get {
                    hits.fetch_add(1, Ordering::SeqCst);
                }
                let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Type: video/mp4\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", data.len());
                if get {
                    let _ = s.write_all(&data);
                }
            });
        }
    });
    port
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap()
}

/// 像 App / 桌面端那样打开（恢复上次的队列）。手机的 Runner：直链 Rust 自己下、M3U8 Rust 自己封装，和桌面端走同一个 `restore_queue`
fn open(rt: &tokio::runtime::Runtime, dir: &Path) -> Core {
    rt.block_on(Core::open_with(Options { paths: Paths::portable(dir), sidecar_dir: None, restore: true }, Arc::new(InProcessRunner))).unwrap()
}

/// 桌面端那样打开（`Core::open` = 桌面端的 Runner）。只用来重开、不下东西；每日检查新版要等 30 秒才开始，而且设置里关了
fn open_desktop(rt: &tokio::runtime::Runtime, dir: &Path) -> Core {
    rt.block_on(Core::open(Options { paths: Paths::portable(dir), sidecar_dir: None, restore: true })).unwrap()
}

fn wait(what: &str, secs: u64, mut ok: impl FnMut() -> bool) {
    let until = Instant::now() + Duration::from_secs(secs);
    while Instant::now() < until {
        if ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("等了 {secs} 秒还没有：{what}");
}

fn saved(dir: &Path) -> Vec<Value> {
    serde_json::from_slice(&std::fs::read(dir.join("data/queue.json")).unwrap()).unwrap()
}

/// queue.json 里每一条的 (链接, 状态)
fn saved_states(dir: &Path) -> Vec<(String, String)> {
    saved(dir).iter().map(|x| (x["req"]["url"].as_str().unwrap_or_default().to_string(), x["job"]["state"].as_str().unwrap_or_default().to_string())).collect()
}

/// 照着一条真的完成任务，换成别的链接、引擎，配一条素材
fn clone_done(core: &Core, real: &Value, url: &str, eng: &str, title: &str) -> Value {
    let id = core
        .lib
        .add(&NewAsset { kind: "video".into(), title: title.into(), source_url: url.into(), page_url: url.into(), path: core.paths.movies.join(format!("{title}.mp4")), format: "MP4".into(), size: 1, ..Default::default() })
        .unwrap();
    let mut x = real.clone();
    x["req"]["url"] = url.into();
    x["title"] = title.into();
    x["job"]["url"] = url.into();
    x["job"]["title"] = title.into();
    x["job"]["eng"] = eng.into();
    x["job"]["assetId"] = id.into();
    x
}

#[test]
fn finished_jobs_stay_finished_after_reopening() {
    let d = tempfile::tempdir().unwrap();
    let dir = d.path();
    std::fs::create_dir_all(dir.join("data")).unwrap();
    std::fs::write(dir.join("data/settings.json"), r#"{"autoUpdate": false, "language": "zh-CN", "proxyMode": "none"}"#).unwrap();
    let hits = Arc::new(AtomicUsize::new(0));
    let port = serve(hits.clone());

    // 第一次打开：真的下一个直链，走完完成流程（进素材库、写 queue.json）
    let direct = format!("http://127.0.0.1:{port}/sintel_trailer-1080p.mp4");
    let entries = {
        let rt = rt();
        let core = open(&rt, dir);
        let j = {
            let _g = rt.enter();
            core.add(AddRequest { url: direct.clone(), origin: Some("ui".into()), ..Default::default() }).unwrap()
        };
        wait("直链下完", 30, || core.job(j.id).is_some_and(|j| j.state == JobState::Done && j.asset_id.is_some()));
        // 手机上的完成记录不写「影片 › Captae」
        let log: Vec<String> = core.job(j.id).unwrap().log.iter().map(|l| l.1.clone()).collect();
        assert!(log.iter().any(|l| l == "完成，已存进素材库") && !log.iter().any(|l| l.contains("影片")), "{log:?}");
        // 完成那一下写过盘
        wait("queue.json 里是 done", 5, || saved_states(dir).iter().any(|(u, s)| *u == direct && s == "done"));
        let real = saved(dir).into_iter().find(|x| x["req"]["url"] == direct.as_str()).unwrap();
        let hls = clone_done(&core, &real, &format!("http://127.0.0.1:{port}/hls/master.m3u8"), "M3U8", "M3U8 完成的");
        let ytdlp = clone_done(&core, &real, "https://www.youtube.com/watch?v=aqz-KE-bpKQ", "yt-dlp", "yt-dlp 完成的");
        let mut failed = real.clone();
        failed["req"]["url"] = format!("http://127.0.0.1:{port}/broken.mp4").into();
        failed["job"]["state"] = "failed".into();
        failed["job"]["assetId"] = Value::Null;
        failed["job"]["fail"] = serde_json::json!({"code": "network", "message": "网络断了", "raw": ""});
        vec![real, hls, ytdlp, failed]
    };
    let assets = {
        let rt = rt();
        let core = Core::open_with(Options { paths: Paths::portable(dir), sidecar_dir: None, restore: false }, Arc::new(InProcessRunner));
        rt.block_on(core).unwrap().lib.count().unwrap()
    };
    assert_eq!(assets, 3);
    let before_hits = hits.load(Ordering::SeqCst);

    // 直链、M3U8、yt-dlp 各轮一次放在文件最后（最后恢复的那一条），各重开 3 次
    for last in 0..3 {
        let mut order: Vec<Value> = entries.iter().enumerate().filter(|(i, _)| *i != last).map(|(_, x)| x.clone()).collect();
        order.push(entries[last].clone());
        std::fs::write(dir.join("data/queue.json"), serde_json::to_vec(&order).unwrap()).unwrap();
        let want: Vec<(String, String)> = {
            let mut v: Vec<(String, String)> = order.iter().map(|x| (x["req"]["url"].as_str().unwrap().to_string(), x["job"]["state"].as_str().unwrap().to_string())).collect();
            v.sort();
            v
        };
        for round in 1..=3 {
            let rt = rt();
            // 手机、桌面端、手机轮流：两边走的是同一个 restore_queue
            let desktop = round == 2;
            let core = if desktop { open_desktop(&rt, dir) } else { open(&rt, dir) };
            let what = format!("最后一条是第 {last} 种，第 {round} 次重开（{}）", if desktop { "桌面端" } else { "手机" });
            // 打开以后（调度器转过几圈）：没有任何任务在排队或在跑
            std::thread::sleep(Duration::from_millis(1500));
            let jobs = core.jobs();
            assert_eq!(jobs.len(), 4, "{what}");
            for j in &jobs {
                assert!(matches!(j.state, JobState::Done | JobState::Failed), "{what}：{} 变成了 {:?}", j.url, j.state);
            }
            // 什么都没下就退出：queue.json 里的状态也和原来一样
            let mut got = saved_states(dir);
            got.sort();
            assert_eq!(got, want, "{what}：queue.json");
            assert_eq!(core.lib.count().unwrap(), assets, "{what}：素材数量");
            assert_eq!(hits.load(Ordering::SeqCst), before_hits, "{what}：不该再去下");
            drop(core);
            drop(rt);
        }
    }
}
