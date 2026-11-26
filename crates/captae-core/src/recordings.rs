//! 录直播没整理就退出留下的原始数据（命令行录直播时按两次 Ctrl-C、录制中被结束或崩溃）：保留 7 天，之后启动时删掉。
//!
//! 原始数据有三种（`model::Rec::raw`）：
//! - M3U8 引擎：`<数据目录>/work/hls-rec-…/`，按名字就找得到。
//! - yt-dlp：保存位置下这个任务的中间目录 `.captae-tmp/<…>/`。保存位置可以是任何文件夹，所以命令行没整理就退出时
//!   把它记进 `<数据目录>/work/recordings-left.json`（`remember`），启动时按这份记录去找。
//! - 电台流：aria2 直接写在保存位置的那个文件，名字就是最后的文件名、本身能播放，不算中间文件，不删。
//!
//! 桌面端 queue.json 里还记着的（录制中途 Captae 退出了、等下次打开整理的）不管多旧都不删。
//! 「多旧」看最后一次写入（目录自己和里面最新的那个文件），录了好几天、还在录的不会被当成旧的。
//! 桌面端和命令行（还有桌面端没开时 MCP 自己起的 Core）启动时都会清一次（`Core::open`）。

use crate::config::Paths;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// 保留多久
pub const KEEP: Duration = Duration::from_secs(7 * 24 * 3600);

/// 命令行记下的 yt-dlp 中间目录（在 `<数据目录>/work/` 里）
const LEFT: &str = "recordings-left.json";
/// M3U8 引擎录直播的原始数据目录的名字开头（hls::record）
const HLS_REC: &str = "hls-rec-";

/// 这处原始数据会不会到时候自动删（命令行据此决定怎么说）：M3U8 引擎的目录、yt-dlp 的中间目录会，电台流的文件不会
pub fn swept(paths: &Paths, raw: &Path) -> bool {
    is_hls_rec(paths, raw) || is_ytdlp_tmp(raw)
}

fn is_hls_rec(paths: &Paths, p: &Path) -> bool {
    p.parent() == Some(paths.work.as_path()) && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(HLS_REC))
}

/// yt-dlp 一个任务的中间目录：`<保存位置>/.captae-tmp/<…>`（ytdlp::temp_dir）
fn is_ytdlp_tmp(p: &Path) -> bool {
    p.file_name().is_some() && p.parent().and_then(Path::file_name).is_some_and(|n| n == ".captae-tmp")
}

/// 命令行没整理就退出时调：记下保存位置里留下的 yt-dlp 中间目录，到时候启动时删。
/// `<数据目录>/work/hls-rec-…` 按名字就找得到，不用记；电台流的文件不删，也不记
pub fn remember(paths: &Paths, raws: &[PathBuf]) {
    let add: Vec<&PathBuf> = raws.iter().filter(|p| is_ytdlp_tmp(p)).collect();
    if add.is_empty() {
        return;
    }
    let mut list = load(paths);
    for p in add {
        if !list.contains(p) {
            list.push(p.clone());
        }
    }
    save(paths, &list);
}

fn load(paths: &Paths) -> Vec<PathBuf> {
    std::fs::read(paths.work.join(LEFT)).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save(paths: &Paths, list: &[PathBuf]) {
    let f = paths.work.join(LEFT);
    if list.is_empty() {
        let _ = std::fs::remove_file(&f);
        return;
    }
    let _ = std::fs::create_dir_all(&paths.work);
    // 先写到旁边再换过去：桌面端和命令行同时在清的时候，读到的不会是写了一半的
    let tmp = paths.work.join(format!("{LEFT}.{}", std::process::id()));
    if std::fs::write(&tmp, serde_json::to_vec(list).unwrap_or_default()).is_ok() {
        let _ = std::fs::rename(&tmp, &f);
    }
}

/// queue.json 里记着的录制原始数据：桌面端等下次打开整理的录制（被中断的），以及还留在列表里的失败任务的。
/// 读不懂（可能桌面端正好写到一半）时返回 None：这次什么都不删，下次再说
fn in_queue(paths: &Paths) -> Option<HashSet<PathBuf>> {
    let b = match std::fs::read(paths.data.join("queue.json")) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Some(HashSet::new()),
        Err(_) => return None,
    };
    let v = serde_json::from_slice::<Vec<serde_json::Value>>(&b).ok()?;
    Some(v.iter().filter_map(|x| x["job"]["rec"]["raw"].as_str()).map(PathBuf::from).collect())
}

/// 最后一次写入：文件看自己；目录看自己和里面（往下三层）最新的那个。不跟着符号链接走
fn last_write(p: &Path, depth: u32) -> Option<SystemTime> {
    let m = std::fs::symlink_metadata(p).ok()?;
    let mut t = m.modified().ok()?;
    if m.is_dir() && depth > 0 {
        for e in std::fs::read_dir(p).ok()?.flatten() {
            if let Some(c) = last_write(&e.path(), depth - 1) {
                t = t.max(c);
            }
        }
    }
    Some(t)
}

/// 最后一次写入在 `keep` 以前（时间在将来的——系统时间被调过——不算旧）
fn stale(p: &Path, keep: Duration) -> bool {
    last_write(p, 3).and_then(|t| t.elapsed().ok()).is_some_and(|age| age > keep)
}

/// 启动时调：删掉最后一次写入在 `keep` 以前的录制原始数据，返回删掉了哪些。queue.json 里记着的不删
pub fn sweep(paths: &Paths, keep: Duration) -> Vec<PathBuf> {
    let Some(busy) = in_queue(paths) else {
        tracing::info!("queue.json 这次读不懂，先不清理录直播留下的原始数据");
        return vec![];
    };
    let mut gone = vec![];
    // M3U8 引擎的：<数据目录>/work/hls-rec-…
    if let Ok(rd) = std::fs::read_dir(&paths.work) {
        for e in rd.flatten() {
            let p = e.path();
            if is_hls_rec(paths, &p) && p.is_dir() && !busy.contains(&p) && stale(&p, keep) && std::fs::remove_dir_all(&p).is_ok() {
                gone.push(p);
            }
        }
    }
    // yt-dlp 的：命令行退出时记下的
    let list = load(paths);
    if !list.is_empty() {
        let mut left = vec![];
        for p in list {
            // 已经没了（整理好了、用户删了、挪走了）；或者根本不是 yt-dlp 的中间目录（记录被改过）：不碰，这条记录也不要了
            if !p.is_dir() || !is_ytdlp_tmp(&p) {
                continue;
            }
            if busy.contains(&p) || !stale(&p, keep) {
                left.push(p);
                continue;
            }
            if std::fs::remove_dir_all(&p).is_ok() {
                // .captae-tmp 空了就一起删（里面还有别的任务的就留着）
                if let Some(parent) = p.parent() {
                    let _ = std::fs::remove_dir(parent);
                }
                gone.push(p);
            } else {
                left.push(p);
            }
        }
        save(paths, &left);
    }
    let days = keep.as_secs() / 86_400;
    for p in &gone {
        tracing::info!("删掉了 {days} 天以前没整理的录直播原始数据：{}", p.display());
    }
    gone
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 把一个文件或目录（连同里面的）最后修改时间改到 `ago` 以前
    fn age(p: &Path, ago: Duration) {
        let t = SystemTime::now() - ago;
        if p.is_dir() {
            for e in std::fs::read_dir(p).unwrap().flatten() {
                age(&e.path(), ago);
            }
        }
        std::fs::File::open(p).unwrap().set_modified(t).unwrap();
    }

    fn raw_dir(p: &Path) -> PathBuf {
        std::fs::create_dir_all(p).unwrap();
        std::fs::write(p.join("video"), b"ts").unwrap();
        p.to_path_buf()
    }

    const DAY: Duration = Duration::from_secs(86_400);

    #[test]
    fn keeps_seven_days_and_what_queue_still_needs() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        paths.ensure().unwrap();
        // M3U8 引擎录的：8 天前的、昨天的、8 天前但桌面端还等着整理的
        let old = raw_dir(&paths.work.join("hls-rec-aaaa-1"));
        let new = raw_dir(&paths.work.join("hls-rec-bbbb-2"));
        let waiting = raw_dir(&paths.work.join("hls-rec-cccc-3"));
        // 不是录直播的：普通 M3U8 下载留着续传的分片（hls-…）、别的中间文件
        let resume = raw_dir(&paths.work.join("hls-0123456789ab"));
        let other = raw_dir(&paths.work.join("subs-x"));
        // yt-dlp 录的：在保存位置的 .captae-tmp 下，命令行退出时记下的
        let save = dir.path().join("Elsewhere");
        let y_old = raw_dir(&save.join(".captae-tmp/0123456789ab"));
        let y_new = raw_dir(&dir.path().join("Movies/.captae-tmp/ba9876543210"));
        // 同一个 .captae-tmp 里还有一个没记下的（别的任务暂停着续传用的）：不碰
        let y_other = raw_dir(&dir.path().join("Movies/.captae-tmp/aaaaaaaaaaaa"));
        // 电台流的文件：不删，也不记
        let radio = dir.path().join("Music/radio.mp3");
        std::fs::create_dir_all(radio.parent().unwrap()).unwrap();
        std::fs::write(&radio, b"mp3").unwrap();
        remember(&paths, &[y_old.clone(), y_new.clone(), radio.clone(), old.clone()]);
        assert_eq!(load(&paths), vec![y_old.clone(), y_new.clone()], "只记 yt-dlp 的中间目录");
        remember(&paths, &[y_old.clone()]);
        assert_eq!(load(&paths).len(), 2, "记过的不重复记");
        for p in [&old, &waiting, &resume, &other, &y_old, &y_other, &radio] {
            age(p, 8 * DAY);
        }
        age(&new, DAY);
        age(&y_new, DAY);
        let saved = serde_json::json!([{"req": {"url": "https://a.invalid/live.m3u8", "record_live": true}, "title": "直播",
            "job": {"state": "run", "phase": "dl", "live": true, "rec": {"since": 1, "raw": waiting}}}]);
        std::fs::write(paths.data.join("queue.json"), serde_json::to_vec(&saved).unwrap()).unwrap();

        let gone = sweep(&paths, KEEP);
        assert_eq!(gone.len(), 2, "{gone:?}");
        assert!(!old.exists() && !y_old.exists(), "7 天以前的删掉");
        assert!(!save.join(".captae-tmp").exists(), "空了的 .captae-tmp 一起删");
        assert!(new.exists() && y_new.exists(), "7 天以内的留着");
        assert!(waiting.exists(), "queue.json 里还等着整理的不删");
        assert!(resume.exists() && other.exists() && y_other.exists() && radio.exists(), "别的不碰");
        assert_eq!(load(&paths), vec![y_new.clone()], "删掉的不再记着");

        // 录了很多天、还在写的：看最后一次写入，不算旧
        let long = raw_dir(&paths.work.join("hls-rec-dddd-4"));
        age(&long, 9 * DAY);
        std::fs::write(long.join("video"), b"ts ts").unwrap();
        assert!(sweep(&paths, KEEP).is_empty() && long.exists());

        // queue.json 读不懂（正写到一半）：这次什么都不删
        age(&long, 9 * DAY);
        std::fs::write(paths.data.join("queue.json"), b"[{\"req\":").unwrap();
        assert!(sweep(&paths, KEEP).is_empty() && long.exists());
        std::fs::remove_file(paths.data.join("queue.json")).unwrap();
        let mut g = sweep(&paths, KEEP);
        g.sort();
        let mut want = vec![long.clone(), waiting.clone()];
        want.sort();
        assert_eq!(g, want, "没有 queue.json 就照常清（queue.json 里不再记着的也一样）");

        // 过了 7 天，记着的那个也删掉；记录跟着没了
        age(&y_new, 8 * DAY);
        assert_eq!(sweep(&paths, KEEP), vec![y_new.clone()]);
        assert!(!paths.work.join(LEFT).exists());
        assert!(y_other.exists() && dir.path().join("Movies/.captae-tmp").exists(), "还有别的任务的 .captae-tmp 留着");
    }

    #[test]
    fn only_touches_its_own_kinds() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        paths.ensure().unwrap();
        // 记录被改过、指向别处的：不删，记录丢掉
        let mine = raw_dir(&dir.path().join("Documents/important"));
        age(&mine, 30 * DAY);
        save(&paths, &[mine.clone()]);
        assert!(sweep(&paths, KEEP).is_empty() && mine.exists());
        assert!(load(&paths).is_empty());
        // work/ 外面名字是 hls-rec- 开头的不碰
        let outside = raw_dir(&dir.path().join("hls-rec-zzzz-1"));
        age(&outside, 30 * DAY);
        assert!(sweep(&paths, KEEP).is_empty() && outside.exists());
        // 会不会自动删：命令行据此决定怎么说
        assert!(swept(&paths, &paths.work.join("hls-rec-x-1")));
        assert!(swept(&paths, &dir.path().join("Movies/.captae-tmp/0123456789ab")));
        assert!(!swept(&paths, &dir.path().join("Music/radio.mp3")));
        assert!(!swept(&paths, &outside));
    }
}
