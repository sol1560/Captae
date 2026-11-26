//! 手机上「读取不要做两遍」：输入卡先读取信息（`probe`），加任务以后队列的读取阶段直接用刚才 yt-dlp 读到的结果（engine/ytdlp.rs 的缓存）。
//! 用一个假的 yt-dlp（Android 的启动器 libytdlp.so 换成 shell 脚本，记下每次被调用时的参数），数它被要了几次信息（`-J`）。
//! 覆盖：手机上读一次就够；带登录状态的不缓存；桌面端照旧读两遍（行为不变）。缓存是整个进程共用的，每种情况用不同的链接。
#![cfg(unix)]

use captae_core::engine::ytdlp::{probe_url, ProbeReq};
use captae_core::login::Auth;
use captae_core::tools::AndroidRunner;
use captae_core::*;
use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

const INFO: &str = r#"{"id":"aqz-KE-bpKQ","title":"Big Buck Bunny","duration":12,"ext":"mp4","extractor_key":"Youtube","width":320,"height":180,"formats":[{"format_id":"18","url":"https://rr1.googlevideo.com/videoplayback","ext":"mp4","vcodec":"avc1","acodec":"mp4a","width":320,"height":180}]}"#;

/// 假的 yt-dlp：`-J` 给信息；下载时在 `out` 里「下好」一个文件。每次被调用都记一行参数
fn fake_ytdlp(path: &Path, calls: &Path, out: &Path) {
    let file = out.join("bunny.mp4");
    std::fs::write(
        path,
        format!(
            "#!/bin/sh\necho \"$*\" >> '{c}'\nfor a; do if [ \"$a\" = \"-J\" ]; then echo '{INFO}'; exit 0; fi; done\nmkdir -p '{o}'\nprintf x > '{f}'\necho 'CAPTAE|F|{f}'\nexit 0\n",
            c = calls.display(),
            o = out.display(),
            f = file.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// (要信息的次数, 用 --load-info-json 下载的次数)
fn count(calls: &Path) -> (usize, usize) {
    let s = std::fs::read_to_string(calls).unwrap_or_default();
    (s.lines().filter(|l| l.split(' ').any(|a| a == "-J")).count(), s.lines().filter(|l| l.contains("--load-info-json")).count())
}

async fn probe_then_add(core: &Core, url: &str, out: &Path) -> Job {
    let p = probe::probe(&core.toolbox(), &core.http(), url, &BTreeMap::new()).await.unwrap();
    assert_eq!(p.title.as_deref(), Some("Big Buck Bunny"));
    let j = core.add(AddRequest { url: url.into(), origin: Some("ui".into()), output_dir: Some(out.to_path_buf()), ..Default::default() }).unwrap();
    tokio::time::timeout(Duration::from_secs(30), core.wait(j.id)).await.expect("30 秒内下完").unwrap()
}

fn settings(core: &Core, ytdlp: Option<&Path>) {
    let mut s = serde_json::json!({"proxyMode": "none", "ffmpegPath": "/nonexistent/captae-test/ffmpeg", "ytdlpUseAria2": false, "autoUpdate": false});
    if let Some(p) = ytdlp {
        s["ytdlpPath"] = p.display().to_string().into();
    }
    core.patch_settings(&s).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn phone_reads_once_desktop_unchanged() {
    let d = tempfile::tempdir().unwrap();
    let out = d.path().join("out");

    // ---- 手机（Android 的 Runner：yt-dlp 是原生库目录里的 libytdlp.so）
    let lib = d.path().join("lib");
    std::fs::create_dir_all(&lib).unwrap();
    let calls = d.path().join("phone-calls.txt");
    fake_ytdlp(&lib.join("libytdlp.so"), &calls, &out);
    let core = Core::open_with(Options { paths: Paths::portable(&d.path().join("phone")), sidecar_dir: None, restore: false }, Arc::new(AndroidRunner::new(lib.display().to_string()))).await.unwrap();
    settings(&core, None);
    let j = probe_then_add(&core, "https://www.youtube.com/watch?v=probeOnce001", &out).await;
    assert_eq!(j.state, JobState::Done, "{:?}", j.fail);
    assert_eq!(j.title, "Big Buck Bunny");
    assert_eq!(count(&calls), (1, 1), "手机上读取一次，下载用这份信息：{}", std::fs::read_to_string(&calls).unwrap());

    // 带登录状态的读取不缓存（读两遍、两遍都真的去读），也不会用到上面没带登录状态的结果
    let auth = Auth { browser: Some("chrome".into()), ..Default::default() };
    let note = |_: String| {};
    let headers = BTreeMap::new();
    for _ in 0..2 {
        let r = ProbeReq { url: "https://www.youtube.com/watch?v=probeOnce001", headers: &headers, item: None, record_live: false, note: &note, auth: Some(&auth), extra: vec![] };
        probe_url(&core.toolbox(), &core.http(), &r).await.unwrap();
    }
    assert_eq!(count(&calls).0, 3, "带登录状态的每次都去读");
    // 额外参数（YouTube 换客户端重读）也不用缓存
    let r = ProbeReq { url: "https://www.youtube.com/watch?v=probeOnce001", headers: &headers, item: None, record_live: false, note: &note, auth: None, extra: vec!["--extractor-args".into(), "youtube:player_client=tv".into()] };
    probe_url(&core.toolbox(), &core.http(), &r).await.unwrap();
    assert_eq!(count(&calls).0, 4);
    // 同一条链接选了别的条目（多视频帖子）是另一份
    let r = ProbeReq { url: "https://www.youtube.com/watch?v=probeOnce001", headers: &headers, item: Some(2), record_live: false, note: &note, auth: None, extra: vec![] };
    probe_url(&core.toolbox(), &core.http(), &r).await.unwrap();
    assert_eq!(count(&calls).0, 5);

    // ---- 桌面端（DesktopRunner，设置里指定 yt-dlp）：照旧读两遍
    let fake = d.path().join("yt-dlp");
    let dcalls = d.path().join("desktop-calls.txt");
    fake_ytdlp(&fake, &dcalls, &out);
    let desk = Core::open(Options { paths: Paths::portable(&d.path().join("desktop")), sidecar_dir: None, restore: false }).await.unwrap();
    settings(&desk, Some(&fake));
    let j = probe_then_add(&desk, "https://www.youtube.com/watch?v=probeOnce002", &out).await;
    assert_eq!(j.state, JobState::Done, "{:?}", j.fail);
    assert_eq!(count(&dcalls), (2, 1), "桌面端行为不变：{}", std::fs::read_to_string(&dcalls).unwrap());
}
