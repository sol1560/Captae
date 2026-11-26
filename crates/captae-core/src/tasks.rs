//! 下载以外的后台任务：转换、字幕、剪辑、拼接、对本地文件跑预设、Share to Agent。
//! 界面上的进度条（素材库检视面板里的「转码中 N%」）来自这里的事件。

use crate::model::Event;
use crate::queue::Core;
use anyhow::{anyhow, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// 取消任务时返回的错误文字，插件据此显示「已取消」而不是「没准备好」。
pub const CANCELLED: &str = "CANCELLED";

/// 正在写的输出文件。还没 `keep()` 就被丢掉（任务被取消时整个 future 被丢掉、或者中途出错返回）时，把文件删掉，不留半成品。
/// `cancel_task` 只是丢掉正在跑的 future（子进程 kill_on_drop），自己往最终位置写文件的步骤挂一个这个（ffmpeg 的 Job::run）。
#[derive(Debug)]
pub struct Partial(Option<std::path::PathBuf>);

/// 正在写、还没 `keep()` 也没丢掉的输出文件。Captae 直接退出（⌘Q、命令行收到 SIGTERM）时 future 不会被丢掉，
/// `Partial` 没机会删：退出前 `tools::kill_all` 结束 ffmpeg 以后调 `discard_partials` 把它们删掉（转到一半的 .mov 放不了）
static PARTIALS: Mutex<std::collections::BTreeSet<std::path::PathBuf>> = Mutex::new(std::collections::BTreeSet::new());

impl Partial {
    pub fn new(p: impl Into<std::path::PathBuf>) -> Partial {
        let p = p.into();
        PARTIALS.lock().unwrap().insert(p.clone());
        Partial(Some(p))
    }

    /// 写完了，留着
    pub fn keep(mut self) {
        if let Some(p) = self.0.take() {
            PARTIALS.lock().unwrap().remove(&p);
        }
    }
}

impl Drop for Partial {
    fn drop(&mut self) {
        if let Some(p) = self.0.take() {
            PARTIALS.lock().unwrap().remove(&p);
            if std::fs::remove_file(&p).is_ok() {
                tracing::info!("删掉了没写完的文件：{}", p.display());
            }
        }
    }
}

/// 进程要退出了：删掉所有还没写完的输出文件（先结束写它们的 ffmpeg，见 `tools::kill_all`）
pub fn discard_partials() {
    let all: Vec<std::path::PathBuf> = std::mem::take(&mut *PARTIALS.lock().unwrap()).into_iter().collect();
    for p in all {
        if std::fs::remove_file(&p).is_ok() {
            tracing::info!("退出时删掉了没写完的文件：{}", p.display());
        }
    }
}

/// `<数据目录>/work/` 里自己建的中间文件或文件夹（网站字幕、语音识别的音频、嵌字幕用的 SRT、预设的中间结果）。
/// 不管做完、出错还是任务被取消（future 被丢掉），离开作用域就删。
/// 为了不误删，只删名字是这几种开头的（见 `SCRATCH`）；下载的断点续传文件不在里面。
#[derive(Debug)]
pub struct Scratch(std::path::PathBuf);

/// 中间文件的名字开头。新加一种中间文件时加在这里，`sweep_scratch` 也认
pub const SCRATCH: [&str; 5] = ["subs-", "share-subs-", "whisper-", "burn-", "preset-"];

fn is_scratch(p: &std::path::Path) -> bool {
    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| SCRATCH.iter().any(|s| n.starts_with(s)))
}

impl Scratch {
    pub fn new(p: impl Into<std::path::PathBuf>) -> Scratch {
        Scratch(p.into())
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let p = &self.0;
        if !is_scratch(p) {
            return;
        }
        let r = if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
        if r.is_ok() {
            tracing::debug!("删掉了中间文件：{}", p.display());
        }
    }
}

/// 上次没来得及删的中间文件（Captae 被强制退出、断电时 `Scratch` 来不及删）：
/// 删掉 `work/` 里一天以前、名字是 `SCRATCH` 这几种开头的。开始一个会建中间文件的任务前调一次。
pub fn sweep_scratch(work: &std::path::Path) {
    let Ok(rd) = std::fs::read_dir(work) else { return };
    let day = std::time::Duration::from_secs(24 * 3600);
    for e in rd.flatten() {
        let p = e.path();
        let old = e.metadata().and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).is_some_and(|age| age > day);
        if old && is_scratch(&p) {
            let _ = if p.is_dir() { std::fs::remove_dir_all(&p) } else { std::fs::remove_file(&p) };
        }
    }
}

#[derive(Debug, Default)]
pub struct Tasks {
    next: AtomicU64,
    /// 最近的任务状态，浏览器插件没有事件通道，靠轮询这里看「发给 AI」的进度
    recent: Mutex<std::collections::VecDeque<TaskStatus>>,
    /// 还在跑的任务的取消开关
    cancels: Mutex<HashMap<u64, CancellationToken>>,
    /// 浏览器插件最近一次打招呼（桌面端「浏览器插件」页显示连接状态）
    ext: Mutex<Option<ExtSeen>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExtSeen {
    /// 毫秒时间戳
    pub at: i64,
    pub version: Option<String>,
    pub browser: Option<String>,
}

/// 插件每分钟打一次招呼，两分钟内见过就算连着。
pub const EXT_ALIVE_MS: i64 = 120_000;

#[derive(Debug, Clone, Serialize)]
pub struct ExtStatus {
    pub connected: bool,
    #[serde(rename = "lastSeen")]
    pub last_seen: Option<i64>,
    pub version: Option<String>,
    pub browser: Option<String>,
}

impl Tasks {
    fn record(&self, s: &TaskStatus) {
        let mut r = self.recent.lock().unwrap();
        if let Some(x) = r.iter_mut().find(|x| x.id == s.id) {
            *x = s.clone();
        } else {
            r.push_back(s.clone());
            while r.len() > 50 {
                r.pop_front();
            }
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskStatus {
    pub id: u64,
    /// convert / subtitle / trim / concat / burn / preset / share
    pub kind: String,
    pub title: String,
    #[serde(rename = "assetId", skip_serializing_if = "Option::is_none")]
    pub asset_id: Option<i64>,
    pub pct: f64,
    /// run / ok / err / cancel
    pub st: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
    /// 现在在做什么，给界面直接显示：「正在下载识别模型 · 312 / 484 MB」
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// 估计还要多少秒，算不出来时为空
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eta: Option<u64>,
}

#[derive(Clone)]
pub struct Reporter {
    core: Core,
    status: std::sync::Arc<std::sync::Mutex<TaskStatus>>,
    last: std::sync::Arc<std::sync::Mutex<std::time::Instant>>,
    cancel: CancellationToken,
}

impl Reporter {
    /// 用户点了取消。任务本身在 `run_task_as` 里会被中止，这里给需要自己收尾的步骤用。
    pub fn token(&self) -> CancellationToken {
        self.cancel.clone()
    }

    pub fn pct(&self, p: f64) {
        self.update(p, None);
    }

    /// 进度加说明：note 是给人看的一句话，eta 是估计剩余秒数。
    pub fn progress(&self, p: f64, note: impl Into<String>, eta: Option<u64>) {
        self.update(p, Some((note.into(), eta)));
    }

    fn update(&self, p: f64, note: Option<(String, Option<u64>)>) {
        let mut last = self.last.lock().unwrap();
        if last.elapsed().as_millis() < 150 && p < 1.0 {
            return;
        }
        *last = std::time::Instant::now();
        let s = {
            let mut s = self.status.lock().unwrap();
            s.pct = (p * 100.0).clamp(0.0, 100.0);
            if let Some((n, eta)) = note {
                s.note = Some(n);
                s.eta = eta;
            }
            s.clone()
        };
        self.core.tasks.record(&s);
        self.core.emit(Event::Task { task: s });
    }
}

impl Core {
    /// 跑一个后台任务，期间发进度事件，返回结果。
    pub async fn run_task<F, Fut>(&self, kind: &str, title: String, asset_id: Option<i64>, f: F) -> Result<serde_json::Value>
    where
        F: FnOnce(Reporter) -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value>>,
    {
        let id = self.new_task_id();
        self.run_task_as(id, kind, title, asset_id, f).await
    }

    pub fn new_task_id(&self) -> u64 {
        self.tasks.next.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn task(&self, id: u64) -> Option<TaskStatus> {
        self.tasks.recent.lock().unwrap().iter().find(|t| t.id == id).cloned()
    }

    /// 取消一个还在跑的任务。已经结束的返回 false。
    pub fn cancel_task(&self, id: u64) -> bool {
        match self.tasks.cancels.lock().unwrap().get(&id) {
            Some(t) => {
                t.cancel();
                true
            }
            None => false,
        }
    }

    /// 插件打了招呼（宿主转来的 hello）。
    pub fn note_extension(&self, version: Option<String>, browser: Option<String>) {
        *self.tasks.ext.lock().unwrap() = Some(ExtSeen { at: crate::util::now_ms(), version, browser });
    }

    pub fn extension_status(&self) -> ExtStatus {
        let seen = self.tasks.ext.lock().unwrap().clone();
        let now = crate::util::now_ms();
        match seen {
            Some(s) => ExtStatus { connected: now - s.at <= EXT_ALIVE_MS, last_seen: Some(s.at), version: s.version, browser: s.browser },
            None => ExtStatus { connected: false, last_seen: None, version: None, browser: None },
        }
    }

    /// 同 run_task，但编号由调用方先拿好（接口要先把编号回给插件，再在后台跑）。
    pub async fn run_task_as<F, Fut>(&self, id: u64, kind: &str, title: String, asset_id: Option<i64>, f: F) -> Result<serde_json::Value>
    where
        F: FnOnce(Reporter) -> Fut,
        Fut: std::future::Future<Output = Result<serde_json::Value>>,
    {
        let st = TaskStatus { id, kind: kind.into(), title, asset_id, pct: 0.0, st: "run", result: None, err: None, note: None, eta: None };
        self.tasks.record(&st);
        self.emit(Event::Task { task: st.clone() });
        let cancel = CancellationToken::new();
        self.tasks.cancels.lock().unwrap().insert(id, cancel.clone());
        let rep = Reporter {
            core: self.clone(),
            status: std::sync::Arc::new(std::sync::Mutex::new(st)),
            last: std::sync::Arc::new(std::sync::Mutex::new(std::time::Instant::now())),
            cancel: cancel.clone(),
        };
        // 取消时直接丢掉正在跑的 future：子进程都是 kill_on_drop，需要额外收尾的步骤自己挂 drop guard（写文件的用 Partial）
        let r = tokio::select! {
            r = f(rep.clone()) => r,
            _ = cancel.cancelled() => Err(anyhow!(CANCELLED)),
        };
        self.tasks.cancels.lock().unwrap().remove(&id);
        let mut s = rep.status.lock().unwrap().clone();
        match &r {
            Ok(v) => {
                s.st = "ok";
                s.pct = 100.0;
                s.result = Some(v.clone());
            }
            Err(_) if cancel.is_cancelled() => {
                s.st = "cancel";
                s.err = Some(CANCELLED.into());
            }
            Err(e) => {
                s.st = "err";
                s.err = Some(e.to_string());
            }
        }
        self.tasks.record(&s);
        self.emit(Event::Task { task: s });
        r
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Options, Paths};
    use std::time::Duration;

    async fn core(dir: &std::path::Path) -> Core {
        Core::open(Options { paths: Paths::portable(dir), sidecar_dir: None, restore: false }).await.unwrap()
    }

    #[tokio::test]
    async fn cancel_running_task() {
        let dir = tempfile::tempdir().unwrap();
        let core = core(dir.path()).await;
        let id = core.new_task_id();
        let c2 = core.clone();
        let run = tokio::spawn(async move {
            c2.run_task_as(id, "share", "t".into(), None, |rep| async move {
                rep.pct(0.3);
                tokio::time::sleep(Duration::from_secs(30)).await;
                Ok(serde_json::json!({}))
            })
            .await
        });
        while core.task(id).is_none() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(core.cancel_task(id));
        let r = tokio::time::timeout(Duration::from_secs(2), run).await.expect("cancel is prompt").unwrap();
        assert_eq!(r.unwrap_err().to_string(), CANCELLED);
        let t = core.task(id).unwrap();
        assert_eq!((t.st, t.err.as_deref()), ("cancel", Some(CANCELLED)));
        // 已经结束的任务取消不了
        assert!(!core.cancel_task(id));
        assert!(!core.cancel_task(9999));
    }

    /// 假的 ffmpeg：往输出（最后一个参数）写一点，然后 `then`（`wait` 一直等 / `ok` 正常退出 / `fail` 出错退出）
    fn fake_ffmpeg(dir: &std::path::Path, then: &str) -> crate::tools::Toolbox {
        let (sh, ps) = match then {
            "wait" => ("exec sleep 30", "Start-Sleep -Seconds 30"),
            "ok" => ("exit 0", "exit 0"),
            _ => ("exit 1", "exit 1"),
        };
        let exe = crate::tools::testkit::fake(
            dir,
            &format!("ffmpeg-{then}"),
            &format!("for a; do out=\"$a\"; done\nprintf 'half' > \"$out\"\n{sh}"),
            &format!("[IO.File]::WriteAllText($args[-1], 'half')\n{ps}"),
        );
        let s = crate::Settings { ffmpeg_path: exe.display().to_string(), ..Default::default() };
        crate::tools::Toolbox::from_settings(&s, None, None)
    }

    /// 取消一个正在写文件的转换：写了一半的文件要删掉；正常写完的留着；ffmpeg 出错的照旧删掉
    #[tokio::test]
    async fn cancel_removes_partial_output() {
        let dir = tempfile::tempdir().unwrap();
        let core = core(dir.path()).await;
        let job = |tools: crate::tools::Toolbox, out: std::path::PathBuf| {
            let c2 = core.clone();
            let id = core.new_task_id();
            let run = tokio::spawn(async move {
                c2.run_task_as(id, "convert", "Sintel → MP4".into(), Some(1), |rep| async move {
                    crate::ffmpeg::Job { tools: &tools, duration: Some(60.0), extra: vec![] }.run(&[], &[], &out, |p| rep.pct(p)).await?;
                    Ok(serde_json::json!({}))
                })
                .await
            });
            (id, run)
        };
        // 转到一半被取消
        let out = dir.path().join("Sintel (转换).mp4");
        let (id, run) = job(fake_ffmpeg(dir.path(), "wait"), out.clone());
        let t0 = std::time::Instant::now();
        while !out.exists() {
            assert!(t0.elapsed() < Duration::from_secs(10), "假的 ffmpeg 没有开始写");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(core.cancel_task(id));
        let r = tokio::time::timeout(Duration::from_secs(2), run).await.expect("cancel is prompt").unwrap();
        assert_eq!(r.unwrap_err().to_string(), CANCELLED);
        assert_eq!(core.task(id).unwrap().st, "cancel");
        assert!(!out.exists(), "取消以后不留半成品");
        // 正常写完：留着
        let done = dir.path().join("Sintel (转换 2).mp4");
        let (_, run) = job(fake_ffmpeg(dir.path(), "ok"), done.clone());
        run.await.unwrap().unwrap();
        assert_eq!(std::fs::read_to_string(&done).unwrap(), "half");
        // ffmpeg 自己出错：也删掉（原来就是这样）
        let bad = dir.path().join("Sintel (转换 3).mp4");
        let (_, run) = job(fake_ffmpeg(dir.path(), "fail"), bad.clone());
        assert!(run.await.unwrap().is_err());
        assert!(!bad.exists());
    }

    #[tokio::test]
    async fn finished_task_is_not_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let core = core(dir.path()).await;
        let v = core.run_task("convert", "x".into(), None, |_| async { Ok(serde_json::json!({"a": 1})) }).await.unwrap();
        assert_eq!(v["a"], 1);
        let t = core.task(1).unwrap();
        assert_eq!(t.st, "ok");
        assert!(!core.cancel_task(1));
    }

    #[tokio::test]
    async fn extension_seen() {
        let dir = tempfile::tempdir().unwrap();
        let core = core(dir.path()).await;
        let s = core.extension_status();
        assert!(!s.connected && s.last_seen.is_none());
        core.note_extension(Some("0.2.0".into()), Some("chrome".into()));
        let s = core.extension_status();
        assert!(s.connected);
        assert_eq!(s.version.as_deref(), Some("0.2.0"));
        // 太久没打招呼就算断开
        core.tasks.ext.lock().unwrap().as_mut().unwrap().at -= EXT_ALIVE_MS + 1;
        assert!(!core.extension_status().connected);
    }
}

#[cfg(test)]
mod scratch_tests {
    use super::*;

    /// 正常走完、中途出错、future 被丢掉（取消）三种情况，中间文件和文件夹都删掉；名字不对的不删
    #[tokio::test]
    async fn scratch_is_removed_however_it_ends() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        std::fs::create_dir_all(&work).unwrap();
        // 正常走完：文件夹连里面的东西一起删
        {
            let d = work.join("whisper-1");
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("audio.wav"), b"x").unwrap();
            let _g = Scratch::new(&d);
        }
        assert!(!work.join("whisper-1").exists());
        // 中途出错返回：文件也删
        let f = work.join("burn-2.srt");
        let r: Result<()> = (|| {
            std::fs::write(&f, b"1")?;
            let _g = Scratch::new(&f);
            anyhow::bail!("出错了")
        })();
        assert!(r.is_err() && !f.exists());
        // 任务被取消：future 停在 await 上被丢掉
        let d = work.join("subs-9-3");
        let d2 = d.clone();
        let fut = async move {
            std::fs::create_dir_all(&d2).unwrap();
            let _g = Scratch::new(&d2);
            std::future::pending::<()>().await;
        };
        let h = tokio::spawn(fut);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(d.exists());
        h.abort();
        let _ = h.await;
        assert!(!d.exists(), "取消以后中间文件夹还在");
        // 名字不是中间文件的：不删（防止误用删到别的东西）
        let keep = work.join("abc.info.json");
        std::fs::write(&keep, b"{}").unwrap();
        drop(Scratch::new(&keep));
        assert!(keep.exists());
    }

    /// 只清一天以前、名字对得上的；新的、下载的续传文件都不动
    #[test]
    fn sweep_only_old_scratch() {
        let dir = tempfile::tempdir().unwrap();
        let w = dir.path();
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 24 * 3600);
        let age = |p: &std::path::Path| std::fs::File::open(p).unwrap().set_modified(old).unwrap();
        std::fs::create_dir_all(w.join("preset-1")).unwrap();
        age(&w.join("preset-1"));
        std::fs::write(w.join("burn-1.srt"), b"1").unwrap();
        age(&w.join("burn-1.srt"));
        std::fs::create_dir_all(w.join("whisper-2")).unwrap(); // 新的：可能正在用
        std::fs::write(w.join("k1.mp4.part"), b"1").unwrap(); // 下载的续传文件：再旧也不动
        age(&w.join("k1.mp4.part"));
        sweep_scratch(w);
        assert!(!w.join("preset-1").exists() && !w.join("burn-1.srt").exists());
        assert!(w.join("whisper-2").exists() && w.join("k1.mp4.part").exists());
    }
}
