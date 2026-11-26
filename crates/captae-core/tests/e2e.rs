//! 端到端：本地起一个 HTTP 服务，用 ffmpeg 生成测试视频和 M3U8（含 AES-128 加密和多清晰度），
//! 走真实的 aria2 / yt-dlp / ffmpeg 子进程，验证下载、合并、入库、转换、剪辑、预设和 agent 工具。
//! 需要本机装有 ffmpeg、aria2c、yt-dlp；缺任何一个就跳过。

use axum::body::Body;
use axum::extract::{Path as AxPath, State};
use axum::http::{HeaderMap, Response, StatusCode};
use axum::routing::get;
use captae_core::*;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

fn have(bin: &str) -> bool {
    Command::new(bin).arg(if bin == "ffmpeg" { "-version" } else { "--version" }).output().is_ok()
}

/// 服务器收到过哪些请求（验证续传时不再要已经下好的分片）
static HITS: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

async fn serve_file(State(root): State<PathBuf>, AxPath(p): AxPath<String>, h: HeaderMap) -> Response<Body> {
    HITS.lock().unwrap().push(p.clone());
    let path = root.join(&p);
    let Ok(data) = tokio::fs::read(&path).await else { return Response::builder().status(404).body(Body::empty()).unwrap() };
    let ct = match path.extension().and_then(|e| e.to_str()) {
        Some("mp4") => "video/mp4",
        Some("m3u8") => "application/vnd.apple.mpegurl",
        Some("ts") => "video/mp2t",
        Some("html") => "text/html; charset=utf-8",
        _ => "application/octet-stream",
    };
    if let Some(r) = h.get("range").and_then(|v| v.to_str().ok()).and_then(|r| r.strip_prefix("bytes=")) {
        let (a, b) = r.split_once('-').unwrap();
        let a: usize = a.parse().unwrap_or(0);
        let b: usize = b.parse().unwrap_or(data.len() - 1).min(data.len() - 1);
        return Response::builder()
            .status(StatusCode::PARTIAL_CONTENT)
            .header("content-type", ct)
            .header("accept-ranges", "bytes")
            .header("content-range", format!("bytes {a}-{b}/{}", data.len()))
            .body(Body::from(data[a..=b].to_vec()))
            .unwrap();
    }
    Response::builder().header("content-type", ct).header("accept-ranges", "bytes").header("content-length", data.len()).body(Body::from(data)).unwrap()
}

fn ffmpeg(args: &[&str]) {
    let st = Command::new("ffmpeg").args(["-hide_banner", "-loglevel", "error", "-y"]).args(args).status().unwrap();
    assert!(st.success(), "ffmpeg {args:?}");
}

fn fixtures(root: &Path) {
    let s = |p: &str| root.join(p).display().to_string();
    // 6 秒测试视频（有画面变化，便于关键帧）
    ffmpeg(&["-f", "lavfi", "-i", "testsrc2=size=640x360:rate=25:duration=6", "-f", "lavfi", "-i", "sine=frequency=440:duration=6", "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest", &s("clip.mp4")]);
    // MOV，用来测「MOV → MP4」
    ffmpeg(&["-i", &s("clip.mp4"), "-c", "copy", &s("clip.mov")]);
    // 两个清晰度的 HLS，360p 那条 AES-128 加密
    std::fs::create_dir_all(root.join("hls/360")).unwrap();
    std::fs::create_dir_all(root.join("hls/180")).unwrap();
    std::fs::write(root.join("hls/360/key.bin"), [9u8; 16]).unwrap();
    std::fs::write(root.join("hls/360/keyinfo"), format!("key.bin\n{}\n", s("hls/360/key.bin"))).unwrap();
    ffmpeg(&["-i", &s("clip.mp4"), "-c", "copy", "-f", "hls", "-hls_time", "1", "-hls_playlist_type", "vod", "-hls_key_info_file", &s("hls/360/keyinfo"), "-hls_segment_filename", &s("hls/360/seg%03d.ts"), &s("hls/360/index.m3u8")]);
    // 每秒一个关键帧，才切得出 6 个分片（测续传要用）
    ffmpeg(&["-i", &s("clip.mp4"), "-vf", "scale=320:180", "-c:v", "libx264", "-g", "25", "-keyint_min", "25", "-sc_threshold", "0", "-c:a", "aac", "-f", "hls", "-hls_time", "1", "-hls_playlist_type", "vod", "-hls_segment_filename", &s("hls/180/seg%03d.ts"), &s("hls/180/index.m3u8")]);
    std::fs::write(
        root.join("hls/master.m3u8"),
        "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=900000,RESOLUTION=640x360\n360/index.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=300000,RESOLUTION=320x180\n180/index.m3u8\n",
    )
    .unwrap();
    // 分辨率、帧率都不一样、也没有声音的一小段，用来测拼接时的统一重新编码
    ffmpeg(&["-f", "lavfi", "-i", "testsrc=size=320x240:rate=30:duration=2", "-c:v", "libx264", "-pix_fmt", "yuv420p", &s("small.mp4")]);
    // 1 秒的声音，用来测「换一段声音」（比画面短，要补静音）
    ffmpeg(&["-f", "lavfi", "-i", "sine=frequency=660:duration=1", "-c:a", "libmp3lame", &s("beep.mp3")]);
    // 一个带 <video> 的网页，走 yt-dlp generic 提取器
    std::fs::write(root.join("watch.html"), "<html><head><title>Captae Test Page</title><meta property=\"og:title\" content=\"Captae 测试视频\"></head><body><video src=\"clip.mp4\" controls></video></body></html>").unwrap();
}

async fn done(core: &Core, id: JobId) -> Job {
    tokio::time::timeout(Duration::from_secs(120), core.wait(id)).await.expect("timeout").unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn end_to_end() {
    if !(have("ffmpeg") && have("aria2c") && have("yt-dlp")) {
        eprintln!("skip: ffmpeg/aria2c/yt-dlp not installed");
        return;
    }
    let tmp = tempfile::tempdir().unwrap();
    let www = tmp.path().join("www");
    std::fs::create_dir_all(&www).unwrap();
    fixtures(&www);
    let app = axum::Router::new().route("/{*p}", get(serve_file)).with_state(www.clone());
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://127.0.0.1:{}", l.local_addr().unwrap().port());
    tokio::spawn(async move { axum::serve(l, app).await.unwrap() });

    let paths = Paths::portable(&tmp.path().join("home"));
    paths.ensure().unwrap();
    // 界面语言设成中文：下面的断言是按中文写的，不管这台电脑是什么语言（i18n.rs）
    Settings { pick_seconds: 1, threads: 4, language: "zh-CN".into(), ..Default::default() }.save(&paths.settings()).unwrap();
    let core = Core::open(Options { paths: paths.clone(), sidecar_dir: None, restore: false }).await.unwrap();
    let mut events = core.subscribe();

    // 1. aria2 直链，MOV → MP4
    let j = core.add(AddRequest { url: format!("{base}/clip.mov"), format: Some("MP4".into()), ..Default::default() }).unwrap();
    assert_eq!(j.eng, Engine::Aria2);
    assert_eq!(j.fmt, "MOV → MP4");
    let j = done(&core, j.id).await;
    assert_eq!(j.state, JobState::Done, "{:?} {:?}", j.err, j.log);
    let p = j.path.clone().unwrap();
    assert_eq!(p.extension().unwrap(), "mp4");
    assert!(p.starts_with(&paths.movies));
    let a = core.lib.get(j.asset_id.unwrap()).unwrap().unwrap();
    assert_eq!((a.kind.as_str(), a.res.as_str(), a.src.as_str()), ("video", "640×360", "127.0.0.1"));
    assert!(a.thumb.as_ref().unwrap().exists());

    // 2. M3U8：两个清晰度，自动在 1 秒后选最高（360p，加密）
    let j = core.add(AddRequest { url: format!("{base}/hls/master.m3u8"), ..Default::default() }).unwrap();
    assert_eq!(j.eng, Engine::Hls);
    let mut saw_pick = false;
    let jid = j.id;
    let waiter = tokio::spawn({
        let c = core.clone();
        async move { done(&c, jid).await }
    });
    while !waiter.is_finished() {
        if let Ok(Ok(Event::Job { job })) = tokio::time::timeout(Duration::from_millis(200), events.recv()).await {
            if job.id == jid && job.phase == Phase::Pick {
                saw_pick = true;
                assert_eq!(job.vars.len(), 2);
            }
        }
    }
    let j = waiter.await.unwrap();
    assert!(saw_pick);
    assert_eq!(j.state, JobState::Done, "{:?} {:?}", j.err, j.log);
    assert!(j.segs.iter().all(|s| *s == SegState::Done));
    assert_eq!(j.fmt, "MP4 · 360p");
    let pr = ffmpeg::probe(&core.toolbox(), j.path.as_ref().unwrap()).await.unwrap();
    assert_eq!(pr.height, Some(360));
    assert!(pr.duration.unwrap() > 5.0);

    // 3. yt-dlp generic 网页提取
    let j = core.add(AddRequest { url: format!("{base}/watch.html"), ..Default::default() }).unwrap();
    assert_eq!(j.eng, Engine::YtDlp);
    let j = done(&core, j.id).await;
    assert_eq!(j.state, JobState::Done, "{:?} {:?}", j.err, j.log);
    assert!(j.path.as_ref().unwrap().exists());

    // 网页上有哪些字幕：这个测试页没有字幕，读得出来但是空的
    let tracks = subtitle::tracks(&core, &format!("{base}/watch.html")).await.unwrap();
    assert!(tracks.is_empty());

    // 4. 失败的链接给出人话
    let j = core.add(AddRequest { url: format!("{base}/missing.mp4"), ..Default::default() }).unwrap();
    let j = done(&core, j.id).await;
    assert_eq!(j.state, JobState::Failed);
    assert!(j.err.as_deref().unwrap().contains("404"), "{:?}", j.err);

    // 5. agent 工具：查询、转换、剪辑、加标签
    let r = agent::call(&core, "Test Agent", "library_query", json!({"type": "video"})).await.unwrap();
    assert_eq!(r["count"], 3);
    let id = r["items"][0]["asset_id"].as_i64().unwrap();
    let r = agent::call(&core, "Test Agent", "convert", json!({"asset_id": id, "format": "MP3"})).await.unwrap();
    assert!(r["path"].as_str().unwrap().ends_with(".mp3"));
    let r = agent::call(&core, "Test Agent", "trim", json!({"asset_id": id, "start": "0:01", "end": 3, "mute": true})).await.unwrap();
    let tp = PathBuf::from(r["path"].as_str().unwrap());
    let pr = ffmpeg::probe(&core.toolbox(), &tp).await.unwrap();
    assert!(!pr.has_audio);
    assert!((pr.duration.unwrap() - 2.0).abs() < 1.2);
    let r = agent::call(&core, "Test Agent", "library_add_tag", json!({"asset_id": id, "tags": ["参考"]})).await.unwrap();
    assert_eq!(r["tags"], json!(["参考"]));
    // 高级参数在普通模式下被拒绝
    let e = agent::call(&core, "Test Agent", "convert", json!({"asset_id": id, "format": "MP4", "ffmpeg_args": ["-crf", "30"]})).await.unwrap_err();
    assert!(e.to_string().starts_with("ADVANCED_MODE_OFF"));
    // 调用记录
    let calls = core.lib.calls(10).unwrap();
    assert!(calls.iter().any(|c| c.tool == "captae.trim" && c.st == "ok" && c.who == "Test Agent"));
    assert!(calls.iter().any(|c| c.st == "err"));

    // 5b. 拼接：编码、分辨率一样的两段直接接起来（agent 工具，一个用 id、一个用路径）
    let r = agent::call(&core, "Test Agent", "concat", json!({"inputs": [id, a.path.display().to_string()]})).await.unwrap();
    let cp = PathBuf::from(r["path"].as_str().unwrap());
    assert!(cp.file_name().unwrap().to_string_lossy().contains("(拼接)"));
    let pr = ffmpeg::probe(&core.toolbox(), &cp).await.unwrap();
    assert!((pr.duration.unwrap() - 12.0).abs() < 0.6, "{:?}", pr.duration);
    assert_eq!((pr.width, pr.height, pr.has_audio), (Some(640), Some(360), true));
    assert_eq!(core.lib.get(r["asset_id"].as_i64().unwrap()).unwrap().unwrap().parent_id, Some(id));
    assert!(agent::call(&core, "Test Agent", "concat", json!({"inputs": [id]})).await.is_err());
    // 分辨率、帧率不同，第二段还没有声音：以第一段为准重新编码，补静音
    let mut last = 0.0;
    let cp = ffmpeg::concat(&core.toolbox(), &[a.path.clone(), www.join("small.mp4")], Some(tmp.path()), |p| {
        assert!(p >= last);
        last = p;
    })
    .await
    .unwrap();
    assert_eq!(cp.extension().unwrap(), "mp4");
    let pr = ffmpeg::probe(&core.toolbox(), &cp).await.unwrap();
    assert_eq!((pr.width, pr.height, pr.has_audio), (Some(640), Some(360), true));
    assert!((pr.duration.unwrap() - 8.0).abs() < 0.6, "{:?}", pr.duration);
    assert!((pr.fps.unwrap() - 25.0).abs() < 0.1);
    assert_eq!(last, 1.0);

    // 5c. 换一段声音：画面 0–4 秒，换上 1 秒的 MP3，不够的补静音，长度跟画面走
    let o = ffmpeg::TrimOpts { start: 0.0, end: 4.0, replace_audio: Some(www.join("beep.mp3")), ..Default::default() };
    let rp = ffmpeg::trim(&core.toolbox(), &a.path, &o, Some(tmp.path()), vec![], |_| {}).await.unwrap();
    let pr = ffmpeg::probe(&core.toolbox(), &rp).await.unwrap();
    assert!(pr.has_audio);
    assert_eq!(pr.acodec.as_deref(), Some("aac"));
    assert!((pr.duration.unwrap() - 4.0).abs() < 0.6, "{:?}", pr.duration);
    // 精确剪切（重新编码），可以精确到 0.1 秒
    let o = ffmpeg::TrimOpts { start: 1.3, end: 2.8, precise: true, ..Default::default() };
    let rp = ffmpeg::trim(&core.toolbox(), &a.path, &o, Some(tmp.path()), vec![], |_| {}).await.unwrap();
    assert!(rp.file_name().unwrap().to_string_lossy().contains("0.01,3-0.02,8"), "{rp:?}");
    let pr = ffmpeg::probe(&core.toolbox(), &rp).await.unwrap();
    assert!((pr.duration.unwrap() - 1.5).abs() < 0.15, "{:?}", pr.duration);
    // 剪辑面板的胶片条和声音波形
    let frames = ffmpeg::strip_frames(&core.toolbox(), &a.path, 6.0, &tmp.path().join("strip"), 5).await.unwrap();
    assert_eq!(frames.len(), 5);
    let peaks = ffmpeg::peaks(&core.toolbox(), &www.join("beep.mp3"), 32).await.unwrap();
    assert_eq!(peaks.len(), 32);
    assert!(peaks.iter().cloned().fold(0.0f32, f32::max) > 0.99);
    // 转换：画质设置按编码生效，输出名和界面上显示的一致
    let want = ffmpeg::convert_out_path(&a.path, "MP4", None).unwrap();
    let o = ffmpeg::ConvertOpts { height: Some(180), quality: ffmpeg::Quality::Small };
    let cv = ffmpeg::convert_with(&core.toolbox(), &a.path, "MP4", None, &o, vec![], |_| {}).await.unwrap();
    assert_eq!(cv, want);
    assert_eq!(ffmpeg::probe(&core.toolbox(), &cv).await.unwrap().height, Some(180));
    // 烧录字幕：TXT 直接拒绝；ffmpeg 带 libass 时实际烧一次（Homebrew 的 ffmpeg 不带）
    let txt = tmp.path().join("s.txt");
    std::fs::write(&txt, "hello").unwrap();
    assert!(ffmpeg::burn_subtitles(&core.toolbox(), &a.path, &txt, |_| {}).await.is_err());
    if ffmpeg::has_filter(&core.toolbox(), "subtitles").await {
        let cues = vec![subtitle::Cue { start: 0.5, end: 2.0, text: "你好".into() }];
        let b = subtitle::burn(&core, &a.path, &cues, |_| {}).await.unwrap();
        assert!(b.exists());
    }

    // 6. 预设「参考片段」：下载 1080p → 只留前 30 秒 → 去掉声音 → 转为 WebM（对本地文件跑下载后的步骤）
    let p = core.preset("参考片段").unwrap();
    let outs = presets::run_post(&core, &p, id, None).await.unwrap();
    assert!(outs.last().unwrap().extension().unwrap() == "webm");

    // 7. 关键帧 + Share 打包（没有语音识别组件时也能出结果）
    let frames = ffmpeg::keyframes(&core.toolbox(), &a.path, &tmp.path().join("kf"), 6, a.duration).await.unwrap();
    assert!(!frames.is_empty() && frames.len() <= 6);

    // 8. 暂停和继续（aria2 断点续传）
    let j = core.add(AddRequest { url: format!("{base}/clip.mp4"), ..Default::default() }).unwrap();
    core.act(j.id, JobAction::Pause).unwrap();
    assert_eq!(core.job(j.id).unwrap().state, JobState::Pause);
    core.act(j.id, JobAction::Resume).unwrap();
    let j = done(&core, j.id).await;
    assert_eq!(j.state, JobState::Done, "{:?}", j.log);

    // 9. M3U8 续传：暂停前已经下好的分片（按引擎的命名放在工作目录里）不再向服务器要，拼出来的文件完整
    let media = format!("{base}/hls/180/index.m3u8");
    let segs = std::fs::read_dir(www.join("hls/180")).unwrap().filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().ends_with(".ts")).count();
    assert!(segs >= 5, "{segs}");
    let work = paths.work.join(format!("hls-{}", util::url_key(&media))).join("v");
    std::fs::create_dir_all(&work).unwrap();
    for i in 0..3 {
        std::fs::copy(www.join(format!("hls/180/seg{i:03}.ts")), work.join(format!("{i:05}.seg"))).unwrap();
    }
    HITS.lock().unwrap().clear();
    let j = core.add(AddRequest { url: media.clone(), ..Default::default() }).unwrap();
    let j = done(&core, j.id).await;
    assert_eq!(j.state, JobState::Done, "{:?}", j.log);
    let hits = HITS.lock().unwrap().clone();
    for i in 0..3 {
        assert!(!hits.iter().any(|h| h.ends_with(&format!("seg{i:03}.ts"))), "已经下好的第 {i} 片又要了一次：{hits:?}");
    }
    assert_eq!(hits.iter().filter(|h| h.ends_with(".ts")).count(), segs - 3, "{hits:?}");
    let pr = ffmpeg::probe(&core.toolbox(), j.path.as_ref().unwrap()).await.unwrap();
    assert!(pr.duration.unwrap_or(0.0) > 5.0, "{pr:?}");
}
