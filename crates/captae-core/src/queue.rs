//! 核心对象 `Core` 和下载队列调度。
//!
//! 调度规则照设计稿：
//! - 同时下载数 = 设置里的并发数；按优先级 `pri` 从小到大占位
//! - 用户手动开始的（粘贴链接、点「马上开始」、重试）带 `boost`，不排队，并发满时临时 +1
//! - 插件嗅探来的进入队尾排队
//! - 暂停 = 结束子进程但保留已下载部分；继续 = 重新入队，yt-dlp / aria2 用 `-c` 续传，M3U8 跳过已有分片

use crate::config::{Paths, Settings};
use crate::detect::{self, Kind};
use crate::engine::{aria2, hls, ytdlp, Ctx};
use crate::library::{Library, NewAsset};
use crate::login;
use crate::model::*;
use crate::presets::{Preset, Step};
use crate::sniff::Sniffed;
use crate::tools::Toolbox;
use crate::tr;
use crate::util::{clock, mb, now_ms};
use anyhow::{anyhow, Result};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::sync::{broadcast, Notify};
use tokio_util::sync::CancellationToken;

pub(crate) struct Running {
    cancel: CancellationToken,
    pick: Arc<Notify>,
    /// 录直播「停止并保存」：让录制的进程收尾（不是取消，录到的部分照常整理、入库）
    finish: CancellationToken,
}

pub struct Inner {
    pub paths: Paths,
    pub(crate) settings: RwLock<Settings>,
    pub(crate) tools: RwLock<Toolbox>,
    sidecar_dir: Option<PathBuf>,
    /// 外部程序怎么找、能不能跑（桌面端 / Android / iOS，见 `tools::Runner`）。设置变了重建 Toolbox 时带上
    runner: Arc<dyn crate::tools::Runner>,
    /// 共用的 HTTP 客户端和建它时的代理设置；代理变了就换新的（见 `Core::http`）
    http: RwLock<(crate::net::Effective, reqwest::Client)>,
    pub lib: Library,
    jobs: Mutex<BTreeMap<JobId, Job>>,
    running: Mutex<HashMap<JobId, Running>>,
    next_id: AtomicU64,
    pub(crate) tx: broadcast::Sender<Event>,
    wake: Notify,
    pub(crate) presets: RwLock<Vec<Preset>>,
    pub(crate) sniffed: Mutex<Vec<Sniffed>>,
    pub(crate) tasks: crate::tasks::Tasks,
    /// 这个 Core 管着 queue.json（恢复过队列的那个，也就是常驻的桌面端）。命令行、MCP 在桌面端没开时临时起的
    /// Core 不写它，否则会用自己的几个任务把桌面端保存的未完成任务和最近完成的记录整个覆盖掉
    owns_queue: bool,
    /// 正在从 queue.json 恢复：这期间不写盘（见 restore_queue）
    restoring: AtomicBool,
    /// 点「重新打开」装 Captae 的更新之前停下来、等新版打开后接着下的任务（`hold_for_update`）。
    /// 写进 queue.json 那一条的 `held`：任务编号每次打开都重新排（恢复时逐条 `add`），所以记号跟着任务走，不记编号
    held: Mutex<std::collections::HashSet<JobId>>,
    /// `held` 里从 queue.json 恢复出来的那些（恢复时省掉了「点继续接着下」，不接着下时 release_held 补上）
    restored: Mutex<std::collections::HashSet<JobId>>,
    /// 每个任务结束时通知等待者（agent 的同步下载调用）
    done: Mutex<HashMap<JobId, Vec<tokio::sync::oneshot::Sender<Job>>>>,
    /// 登录窗口的存储（桌面端接上；命令行、MCP 自己起的进程没有）
    login_store: RwLock<Option<Arc<dyn login::Store>>>,
    /// 在 Captae 里登录过 / 验证过的网站（logins.json，只有网站和时间）
    logins: Mutex<login::Logins>,
}

#[derive(Clone)]
pub struct Core(pub Arc<Inner>);

impl std::ops::Deref for Core {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

#[derive(Clone)]
pub struct JobHandle {
    pub id: JobId,
    core: Core,
}

impl JobHandle {
    pub fn update<R>(&self, f: impl FnOnce(&mut Job) -> R) -> Option<R> {
        let (r, snap) = {
            let mut jobs = self.core.jobs.lock().unwrap();
            let j = jobs.get_mut(&self.id)?;
            let r = f(j);
            (r, j.clone())
        };
        let _ = self.core.tx.send(Event::Job { job: Box::new(snap) });
        Some(r)
    }
    pub fn snapshot(&self) -> Job {
        self.core.jobs.lock().unwrap().get(&self.id).cloned().expect("job exists")
    }
    /// 马上把队列存进 queue.json（录直播开始写文件时：Captae 中途退出，下次打开要知道录到了哪）
    pub fn persist(&self) {
        self.core.persist_queue();
    }
}

pub struct Options {
    pub paths: Paths,
    /// 随包附带的 yt-dlp / ffmpeg / aria2 所在目录
    pub sidecar_dir: Option<PathBuf>,
    /// 恢复上次没下完的任务。只有常驻的桌面端传 true，它同时会每天检查一次下载引擎有没有新版
    pub restore: bool,
}

impl Core {
    /// 桌面端、命令行：外部程序照原来的方式找（`tools::DesktopRunner`）
    pub async fn open(o: Options) -> Result<Core> {
        Self::open_with(o, Arc::new(crate::tools::DesktopRunner)).await
    }

    /// 指定外部程序怎么找、能不能跑（手机端：Android 的原生库目录、iOS 的进程内实现，见 `tools::Runner`）
    pub async fn open_with(o: Options, runner: Arc<dyn crate::tools::Runner>) -> Result<Core> {
        o.paths.ensure()?;
        let settings = Settings::load(&o.paths.settings());
        // 界面语言：这个进程里生成的任务记录、错误说法都用它（i18n.rs）
        crate::i18n::apply(&settings.language);
        let tools = Toolbox::from_settings(&settings, o.sidecar_dir.clone(), Some(o.paths.data.clone())).with_runner(runner.clone());
        let lib = Library::open(&o.paths.db())?;
        let presets = crate::presets::load(&o.paths.presets());
        let (tx, _) = broadcast::channel(1024);
        let eff = crate::net::effective(&settings);
        let http = crate::net::build_with(&eff);
        let logins = login::Logins::load(&o.paths.logins());
        if o.restore {
            // 上次异常退出留下的临时 cookies 文件（正常情况下用完就删了）。只删 10 分钟以前的：
            // 这时可能正好有插件宿主在自己读取信息，它的临时文件只用几秒
            login::sweep(&o.paths, std::time::Duration::from_secs(600));
        }
        // 录直播没整理就退出留下的原始数据（命令行按两次 Ctrl-C、录制中被结束）：7 天以前的删掉，
        // 桌面端 queue.json 里还等着整理的不删。桌面端、命令行、MCP 启动时都清一次，放到后台线程，不耽误启动
        {
            let p = o.paths.clone();
            tokio::task::spawn_blocking(move || crate::recordings::sweep(&p, crate::recordings::KEEP));
        }
        let core = Core(Arc::new(Inner {
            paths: o.paths,
            settings: RwLock::new(settings),
            tools: RwLock::new(tools),
            sidecar_dir: o.sidecar_dir,
            runner,
            http: RwLock::new((eff, http)),
            lib,
            jobs: Mutex::new(BTreeMap::new()),
            running: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            tx,
            wake: Notify::new(),
            presets: RwLock::new(presets),
            sniffed: Mutex::new(vec![]),
            tasks: crate::tasks::Tasks::default(),
            owns_queue: o.restore,
            restoring: AtomicBool::new(false),
            held: Mutex::new(std::collections::HashSet::new()),
            restored: Mutex::new(std::collections::HashSet::new()),
            done: Mutex::new(HashMap::new()),
            login_store: RwLock::new(None),
            logins: Mutex::new(logins),
        }));
        if o.restore {
            core.restore_queue();
            // 每日检查新版 yt-dlp：桌面端（macOS 目录版）和 Android 完整版（zipapp，updater::Flavor）。iOS 不许在 App 里更新代码，不跑
            if matches!(core.runner.kind(), "desktop" | "android") {
                crate::updater::spawn_daily(core.clone());
            }
        }
        let c = core.clone();
        tokio::spawn(async move { c.scheduler().await });
        Ok(core)
    }

    /// 共用的 HTTP 客户端（M3U8、直链、自动更新、翻译接口、oEmbed），跟着代理设置走：
    /// 设置里的代理或系统代理变了，就换一个新的客户端。
    pub fn http(&self) -> reqwest::Client {
        let eff = crate::net::effective(&self.settings());
        {
            let g = self.http.read().unwrap();
            if g.0 == eff {
                return g.1.clone();
            }
        }
        let c = crate::net::build_with(&eff);
        *self.http.write().unwrap() = (eff, c.clone());
        c
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.tx.subscribe()
    }
    pub(crate) fn emit(&self, e: Event) {
        let _ = self.tx.send(e);
    }
    /// 界面上直接改了素材（标签等）之后通知各页面刷新。
    pub fn emit_library(&self) {
        self.emit(Event::Library);
    }
    /// 问一句要不要下载剪贴板里的视频链接（桌面端在复制后 1 分钟内被拿到前面时调，见 clipboard.rs）
    pub fn emit_clipboard(&self, url: String) {
        self.emit(Event::Clipboard { url });
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap().clone()
    }
    pub fn set_settings(&self, s: Settings) -> Result<()> {
        s.save(&self.paths.settings())?;
        crate::i18n::apply(&s.language);
        *self.tools.write().unwrap() = Toolbox::from_settings(&s, self.sidecar_dir.clone(), Some(self.paths.data.clone())).with_runner(self.runner.clone());
        *self.settings.write().unwrap() = s;
        self.wake.notify_one();
        Ok(())
    }
    /// 只改传入的字段（界面每次只发改动的那几项，避免拿旧快照把别的设置盖回去）。
    pub fn patch_settings(&self, patch: &serde_json::Value) -> Result<Settings> {
        let mut cur = serde_json::to_value(self.settings())?;
        if let (Some(c), Some(p)) = (cur.as_object_mut(), patch.as_object()) {
            for (k, v) in p {
                c.insert(k.clone(), v.clone());
            }
        }
        let s: Settings = serde_json::from_value(cur)?;
        self.set_settings(s.clone())?;
        self.emit(Event::Settings { settings: s.clone() });
        Ok(s)
    }
    /// 其他进程（CLI / MCP）改过 settings.json 时重新读取
    pub fn reload_settings(&self) {
        let s = Settings::load(&self.paths.settings());
        crate::i18n::apply(&s.language);
        *self.tools.write().unwrap() = Toolbox::from_settings(&s, self.sidecar_dir.clone(), Some(self.paths.data.clone())).with_runner(self.runner.clone());
        *self.settings.write().unwrap() = s;
    }
    pub fn toolbox(&self) -> Toolbox {
        self.tools.read().unwrap().clone()
    }

    pub fn jobs(&self) -> Vec<Job> {
        self.jobs.lock().unwrap().values().cloned().collect()
    }
    pub fn job(&self, id: JobId) -> Option<Job> {
        self.jobs.lock().unwrap().get(&id).cloned()
    }
    fn handle(&self, id: JobId) -> JobHandle {
        JobHandle { id, core: self.clone() }
    }

    // ------------------------------------------------------------------ 添加

    pub fn add(&self, mut req: AddRequest) -> Result<Job> {
        // 跳转包装（YouTube /redirect?q=…）、B 站语言前缀、Dropbox 分享页、Commons 文件页：先理顺（detect::canonical）
        req.url = detect::canonical(&req.url);
        let d = detect::detect(&req.url).ok_or_else(|| anyhow!(tr!("链接好像不完整，检查一下是不是少复制了开头的 https://")))?;
        // 插件放在请求头里的 Cookie 搬进 cookies（请求头会原样拼进 yt-dlp / aria2 的命令行参数）；写不进文件的丢掉
        let moved = login::take_cookie_header(&mut req.headers, &req.url);
        req.cookies.extend(moved);
        req.cookies.retain(login::Cookie::writable);
        // 预设里的下载参数
        let preset = match &req.preset {
            Some(n) => Some(self.preset(n).ok_or_else(|| anyhow!(tr!("没有叫「{name}」的预设", name = n)))?),
            None => None,
        };
        if let Some(p) = &preset {
            p.apply_download(&mut req);
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let min_pri = self.jobs.lock().unwrap().values().map(|j| j.pri).min().unwrap_or(0);
        let max_pri = self.jobs.lock().unwrap().values().map(|j| j.pri).max().unwrap_or(0);
        let url = url::Url::parse(&req.url)?;
        let name = url.path_segments().and_then(|mut s| s.next_back().map(str::to_string)).filter(|s| !s.is_empty()).unwrap_or_else(|| d.host.clone());
        let name = url::form_urlencoded::parse(format!("x={name}").as_bytes()).next().map(|(_, v)| v.to_string()).unwrap_or(name);
        let fmt = match (d.kind, req.audio_only) {
            (Kind::Hls, _) => "MP4".to_string(),
            (_, true) => req.format.clone().unwrap_or_else(|| "MP3".into()),
            (Kind::File, false) => {
                let src = d.ext.clone().unwrap_or_default().to_uppercase();
                match req.format.as_deref() {
                    Some(f) if f != "保持原格式" && !f.eq_ignore_ascii_case(&src) && detect::media_kind_of_ext(&src.to_lowercase()) == "video" => format!("{src} → {}", f.to_uppercase()),
                    _ => src,
                }
            }
            (Kind::Page, false) => {
                let f = req.format.clone().filter(|f| f != "保持原格式").unwrap_or_else(|| "MP4".into());
                match req.quality.as_deref() {
                    Some(q) if q != "最高可用" => format!("{f} · {q}"),
                    _ => f,
                }
            }
        };
        let fmt = match &req.preset {
            // 界面按「 · 」拆这一串，分隔符留在外面
            Some(p) => format!("{fmt} · {}", tr!("预设「{name}」", name = p)),
            None => fmt,
        };
        let from_sniff = req.from_sniff;
        let origin = req.origin.clone().unwrap_or_else(|| "ui".into());
        // 只有用户在界面上亲手开始的才插队；插件、AI 助手、命令行加进来的按顺序排
        // 手机上点的「发到电脑」（captae-remote 加进来的，origin 是 `remote:<手机名>`）也是用户亲手开始的
        let remote = origin.strip_prefix("remote:").map(str::to_string);
        let manual = !from_sniff && (origin == "ui" || origin == "extension" || remote.is_some());
        let untitled = d.kind == Kind::Page && req.page_title.is_none();
        let mut job = Job {
            id,
            untitled,
            from: remote,
            title: match d.kind {
                Kind::Page => req.page_title.clone().unwrap_or_else(|| tr!("{host} · 正在读取标题…", host = d.host)),
                // M3U8 的文件名通常是 index / master 这种，没有意义，有网页标题就用网页标题
                _ => req.page_title.clone().filter(|_| d.kind == Kind::Hls).unwrap_or(name),
            },
            src: detect::short_src(&req.url),
            url: req.url.clone(),
            plat: d.plat.map(str::to_string),
            eng: d.eng,
            fmt,
            dur: String::new(),
            thumb: None,
            audio: req.audio_only,
            size: 0.0,
            got: 0.0,
            speed: 0.0,
            state: if manual { JobState::Run } else { JobState::Queue },
            phase: Phase::Probe,
            pri: if manual { min_pri - 1 } else { max_pri + 1 },
            boost: manual,
            err: None,
            fail: None,
            choose: false,
            entries: vec![],
            playlist: None,
            retry_at: None,
            auto_retries: 0,
            probe_args: vec![],
            needs_login: false,
            live: false,
            rec: None,
            when: None,
            probe_at: None,
            done_at: None,
            log: vec![],
            segs: vec![],
            tries: vec![],
            vars: vec![],
            v_idx: 0,
            pick_until: None,
            merge: 0.0,
            path: None,
            asset_id: None,
            tmp: None,
            req,
            meta: None,
        };
        // 队列状态由调度器决定，这里先放进排队
        job.state = JobState::Queue;
        job.log(match origin.as_str() {
            "extension" => tr!("来自浏览器插件：{url}", url = job.url),
            "clipboard" => tr!("来自剪贴板：{url}", url = job.url),
            o if o.starts_with("agent:") => tr!("来自 AI 助手（{client}）：{url}", client = &o[6..], url = job.url),
            o if o.starts_with("remote:") => tr!("来自{name}：{url}", name = &o[7..], url = job.url),
            _ => tr!("添加链接：{url}", url = job.url),
        });
        if !job.req.cookies.is_empty() {
            job.log(tr!("浏览器插件带来了这个网站的登录状态（只给这次下载用，不保存）"));
        }
        self.jobs.lock().unwrap().insert(id, job.clone());
        self.emit(Event::Job { job: Box::new(job.clone()) });
        self.persist_queue();
        self.wake.notify_one();
        Ok(job)
    }

    /// 等任务结束（完成或失败），给 agent 的同步调用用。
    pub async fn wait(&self, id: JobId) -> Result<Job> {
        let rx = {
            let j = self.job(id).ok_or_else(|| anyhow!(tr!("没有这个任务")))?;
            // 播放列表停下来等选条目时也返回（调用方据此让用户选）
            if !j.state.is_active() || j.choose {
                return Ok(j);
            }
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.done.lock().unwrap().entry(id).or_default().push(tx);
            rx
        };
        rx.await.map_err(|_| anyhow!(tr!("任务被取消了")))
    }

    // ------------------------------------------------------------------ 操作

    pub fn act(&self, id: JobId, a: JobAction) -> Result<()> {
        let h = self.handle(id);
        let st = self.job(id).ok_or_else(|| anyhow!(tr!("没有这个任务")))?.state;
        match a {
            JobAction::Toggle => return self.act(id, if st == JobState::Pause { JobAction::Resume } else { JobAction::Pause }),
            JobAction::Pause if st.is_active() => {
                // 录直播暂停没有意义：继续以后中间会缺一段（只给「停止并保存」和「取消」）
                if self.job(id).is_some_and(|j| j.recording() || j.rec.as_ref().is_some_and(|r| r.stopping)) {
                    return Err(anyhow!(tr!("录直播不能暂停：点「停止并保存」把录到的部分放进素材库，或者「取消」")));
                }
                self.pause_now(id, tr!("已暂停§任务记录"));
            }
            JobAction::Resume if st == JobState::Pause => {
                h.update(|j| {
                    j.state = JobState::Queue;
                    j.log(tr!("继续§任务记录"));
                });
            }
            JobAction::Now => {
                h.update(|j| {
                    j.boost = true;
                    if j.state == JobState::Pause {
                        j.state = JobState::Queue;
                    }
                    j.log(tr!("马上开始§任务记录"));
                });
            }
            JobAction::Up | JobAction::Down => {
                let mut jobs = self.jobs.lock().unwrap();
                let mut act: Vec<(i64, JobId)> = jobs.values().filter(|j| j.state.is_active()).map(|j| (j.pri, j.id)).collect();
                act.sort();
                if let Some(k) = act.iter().position(|(_, i)| *i == id) {
                    let other = if a == JobAction::Up { k.checked_sub(1) } else { Some(k + 1).filter(|x| *x < act.len()) };
                    if let Some(o) = other {
                        let (pa, pb) = (act[k].0, act[o].0);
                        jobs.get_mut(&id).unwrap().pri = pb;
                        jobs.get_mut(&act[o].1).unwrap().pri = pa;
                        let (x, y) = (jobs[&id].clone(), jobs[&act[o].1].clone());
                        drop(jobs);
                        self.emit(Event::Job { job: Box::new(x) });
                        self.emit(Event::Job { job: Box::new(y) });
                    }
                }
            }
            JobAction::Cancel | JobAction::Remove => {
                self.stop(id);
                let j = self.jobs.lock().unwrap().remove(&id);
                if let Some(j) = j {
                    if a == JobAction::Cancel {
                        self.cleanup_partial(&j);
                    }
                }
                self.done.lock().unwrap().remove(&id);
                self.emit(Event::JobRemoved { id });
            }
            JobAction::Retry if st == JobState::Failed => {
                h.update(|j| {
                    j.state = JobState::Queue;
                    j.err = None;
                    j.fail = None;
                    j.retry_at = None;
                    j.boost = true;
                    for (k, s) in j.segs.iter_mut().enumerate() {
                        if *s == SegState::Failed {
                            *s = SegState::Wait;
                            j.tries[k] = 0;
                        }
                    }
                    if j.phase != Phase::Pick {
                        j.phase = if j.meta.is_some() { Phase::Dl } else { Phase::Probe };
                    }
                    j.log(tr!("重试§任务记录"));
                });
            }
            JobAction::Segretry => {
                h.update(|j| {
                    for (k, s) in j.segs.iter_mut().enumerate() {
                        if *s == SegState::Failed {
                            *s = SegState::Wait;
                            j.tries[k] = 0;
                        }
                    }
                    if j.state == JobState::Failed {
                        j.state = JobState::Queue;
                        j.err = None;
                        j.fail = None;
                        j.retry_at = None;
                        j.boost = true;
                        j.phase = Phase::Dl;
                    }
                    j.log(tr!("再试一次§任务记录"));
                });
            }
            JobAction::Pickgo => {
                if let Some(r) = self.running.lock().unwrap().get(&id) {
                    r.pick.notify_one();
                }
            }
            JobAction::Stop => self.stop_recording(id, None)?,
            _ => {}
        }
        self.persist_queue();
        self.wake.notify_one();
        Ok(())
    }

    /// 录直播「停止并保存」：界面马上换成「正在保存录到的部分」（不等引擎反应过来），引擎收到以后让录制的进程收尾，
    /// 录到的部分照常整理、入库。`limit`：录满了设定的秒数、自动停下的（AI 助手、命令行 `--max-minutes`），任务记录里写明
    fn stop_recording(&self, id: JobId, limit: Option<u64>) -> Result<()> {
        let j = self.job(id).ok_or_else(|| anyhow!(tr!("没有这个任务")))?;
        // 已经在保存了（点了两次，或者刚好录满时又点了停止）
        if j.rec.as_ref().is_some_and(|r| r.stopping) {
            return Ok(());
        }
        if !j.recording() {
            // 插件按代码认出「刚好直播自己结束了」，不当成失败（content.js、Popup.tsx）
            return Err(crate::errors::coded("NOT_RECORDING", tr!("只有正在录制的直播才能「停止并保存」")));
        }
        self.handle(id).update(|j| {
            j.phase = Phase::Merge;
            j.merge = 0.0;
            j.speed = 0.0;
            if let Some(r) = j.rec.as_mut() {
                r.stopping = true;
                r.until.get_or_insert(now_ms());
                r.full = limit.is_some();
            }
            match limit {
                Some(s) => j.log(tr!("录满设定的 {limit}，自动停止，正在保存录到的部分", limit = limit_label(s))),
                None => j.log(tr!("停止录制，正在保存录到的部分")),
            }
        });
        if let Some(r) = self.running.lock().unwrap().get(&id) {
            r.finish.cancel();
        }
        Ok(())
    }

    pub fn set_variant(&self, id: JobId, idx: usize) -> Result<()> {
        self.handle(id).update(|j| {
            if idx < j.vars.len() && j.phase == Phase::Pick {
                j.v_idx = idx;
                j.size = j.vars[idx].mb;
            }
        });
        Ok(())
    }

    pub fn pause_all(&self) {
        let ids: Vec<JobId> = self.jobs().into_iter().filter(|j| matches!(j.state, JobState::Run | JobState::Queue)).map(|j| j.id).collect();
        for id in ids {
            let _ = self.act(id, JobAction::Pause);
        }
    }
    pub fn resume_all(&self) {
        let ids: Vec<JobId> = self.jobs().into_iter().filter(|j| j.state == JobState::Pause).map(|j| j.id).collect();
        for id in ids {
            let _ = self.act(id, JobAction::Resume);
        }
    }

    /// 停下一个正在下载 / 排队的任务，记成「暂停」（和用户点暂停一样：下到一半的文件留着，继续时接着下）
    fn pause_now(&self, id: JobId, log: String) {
        self.stop(id);
        self.handle(id).update(|j| {
            j.state = JobState::Pause;
            j.speed = 0.0;
            j.boost = false;
            for s in j.segs.iter_mut() {
                if matches!(s, SegState::Loading | SegState::Retrying) {
                    *s = SegState::Wait;
                }
            }
            if j.phase == Phase::Pick {
                j.pick_until = None;
            }
            j.log(log);
        });
    }

    /// 点「重新打开」装 Captae 的更新之前（桌面端 app_update.rs）：正在下载、排队的任务停下来记成「暂停」、做上记号，马上写进 queue.json；
    /// 新版打开后桌面端调 `release_held(true)` 把它们接着下。录直播的不停（重新打开后整理录到的部分），等选播放列表条目的不动。
    /// 不先停下来的话，退出前结束 aria2c、yt-dlp 会被当成下载出错：Windows 上更新插件先收拾（结束附带程序）再去运行安装程序，
    /// 中间这段时间里任务被记成「失败」写进 queue.json，新版打开后就不会接着下（Devin 的 Windows U4）。返回停下来的任务数
    pub fn hold_for_update(&self) -> usize {
        let ids: Vec<JobId> = self
            .jobs()
            .into_iter()
            .filter(|j| matches!(j.state, JobState::Run | JobState::Queue) && j.rec.is_none() && !j.choose)
            .map(|j| j.id)
            .collect();
        for &id in &ids {
            self.pause_now(id, tr!("重新打开 Captae 装更新：先停下，新版打开后接着下"));
        }
        self.held.lock().unwrap().extend(ids.iter().copied());
        self.persist_queue();
        ids.len()
    }

    /// 去掉 `hold_for_update` 做的记号。`resume`：其中还暂停着的接着下（新版打开后确认真的更新过了；或者没装成、留在这个进程里继续）。
    /// 不接着下的（版本没变的重新打开）照平时一样暂停着，补上恢复队列时为它省掉的那句「点继续接着下」。返回接着下的任务
    pub fn release_held(&self, resume: bool) -> Vec<JobId> {
        let ids: Vec<JobId> = self.held.lock().unwrap().drain().collect();
        let mut out = vec![];
        for id in ids {
            let paused = self.job(id).is_some_and(|j| j.state == JobState::Pause && !j.choose);
            if resume && paused && self.act(id, JobAction::Resume).is_ok() {
                out.push(id);
            } else if paused && self.restored.lock().unwrap().contains(&id) {
                self.handle(id).update(|j| j.log(tr!("Captae 重新打开，任务已恢复，点继续接着下")));
            }
        }
        self.restored.lock().unwrap().clear();
        self.persist_queue();
        out
    }

    fn stop(&self, id: JobId) {
        if let Some(r) = self.running.lock().unwrap().remove(&id) {
            r.cancel.cancel();
        }
    }

    fn cleanup_partial(&self, j: &Job) {
        for v in j.vars.iter().map(|v| v.uri.as_str()).chain(std::iter::once(j.url.as_str())) {
            let _ = std::fs::remove_dir_all(self.paths.work.join(format!("hls-{}", crate::util::url_key(v))));
        }
        // 取消录直播：录到的都不要了（M3U8 引擎的中间目录；yt-dlp 的在 remove_tmp 里删，电台流就是下面的 j.path）
        if let Some(raw) = j.rec.as_ref().and_then(|r| r.raw.as_ref()) {
            if raw.is_dir() && raw.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("hls-rec-")) {
                let _ = std::fs::remove_dir_all(raw);
            }
        }
        remove_tmp(j);
        // Captae 重新打开以后恢复的任务不知道自己的中间目录（tmp 不存盘）：按会用到的几个保存位置算出来，有就删
        if j.tmp.is_none() && j.eng == Engine::YtDlp {
            let s = self.settings();
            let dirs = [j.req.output_dir.clone(), Some(PathBuf::from(&s.save_dir)).filter(|p| !p.as_os_str().is_empty()), Some(self.paths.movies.clone()), Some(self.paths.music.clone()), Some(self.paths.pictures.clone())];
            for d in dirs.into_iter().flatten() {
                let t = ytdlp::temp_dir(&d, &j.url, j.req.playlist_item);
                if t.is_dir() {
                    let _ = std::fs::remove_dir_all(&t);
                    let _ = std::fs::remove_dir(d.join(".captae-tmp"));
                }
            }
        }
        let _ = std::fs::remove_file(info_json_path(&self.paths, &j.url, j.req.playlist_item));
        // 已经进了素材库的（下完了、正在执行预设里下载之后的步骤时被取消）：文件是用户的素材，不删，只停掉后面的步骤
        if j.state != JobState::Done && j.asset_id.is_none() {
            if let Some(p) = &j.path {
                let _ = std::fs::remove_file(p);
                let _ = std::fs::remove_file(format!("{}.aria2", p.display()));
            }
        }
    }

    // ------------------------------------------------------------------ 调度

    async fn scheduler(self) {
        loop {
            self.schedule_once();
            tokio::select! {
                _ = self.wake.notified() => {}
                _ = tokio::time::sleep(std::time::Duration::from_millis(1000)) => {}
            }
        }
    }

    fn schedule_once(&self) {
        // 网站要求放慢（rate_limited）的任务：到点自动重试
        let now = now_ms();
        let due: Vec<JobId> = self.jobs().into_iter().filter(|j| j.state == JobState::Failed && j.retry_at.is_some_and(|t| t <= now)).map(|j| j.id).collect();
        for id in due {
            self.handle(id).update(|j| {
                j.retry_at = None;
                j.auto_retries += 1;
                j.state = JobState::Queue;
                j.err = None;
                j.fail = None;
                if j.phase != Phase::Pick {
                    j.phase = if j.meta.is_some() { Phase::Dl } else { Phase::Probe };
                }
                j.log(tr!("自动重试（第 {n} 次）", n = j.auto_retries));
            });
        }
        let conc = self.settings.read().unwrap().concurrency.max(1) as usize;
        let mut order: Vec<Job> = self.jobs().into_iter().filter(|j| matches!(j.state, JobState::Run | JobState::Queue)).collect();
        order.sort_by_key(|j| j.pri);
        // 录直播设了最多录多久（AI 助手默认 10 分钟、命令行 --max-minutes）：录满了自动「停止并保存」。
        // 调度器至少每秒转一次，误差在一秒以内
        let full: Vec<(JobId, u64)> = order
            .iter()
            .filter_map(|j| {
                let max = j.rec.as_ref()?.max?;
                (j.recording() && now - j.rec.as_ref()?.since >= max as i64 * 1000).then_some((j.id, max))
            })
            .collect();
        for (id, max) in full {
            if self.stop_recording(id, Some(max)).is_ok() {
                self.persist_queue();
            }
        }
        let mut slots = conc;
        for j in order {
            let running = self.running.lock().unwrap().contains_key(&j.id);
            // 录着的直播不让位：停下来再接着录，中间会缺一段
            if j.boost || j.rec.is_some() {
                if !running {
                    self.start(j.id);
                }
                continue;
            }
            if slots > 0 {
                if !running {
                    self.start(j.id);
                }
                slots -= 1;
            } else if running {
                // 并发数调小了：让出位置，回到排队
                self.stop(j.id);
                self.handle(j.id).update(|j| {
                    j.state = JobState::Queue;
                    j.speed = 0.0;
                    for s in j.segs.iter_mut() {
                        if matches!(s, SegState::Loading | SegState::Retrying) {
                            *s = SegState::Wait;
                        }
                    }
                });
            }
        }
    }

    fn start(&self, id: JobId) {
        let cancel = CancellationToken::new();
        let pick = Arc::new(Notify::new());
        self.running.lock().unwrap().insert(id, Running { cancel: cancel.clone(), pick: pick.clone(), finish: CancellationToken::new() });
        let h = self.handle(id);
        h.update(|j| {
            j.state = JobState::Run;
            j.err = None;
            j.fail = None;
            j.retry_at = None;
            j.log(tr!("开始§任务记录"));
        });
        let core = self.clone();
        tokio::spawn(async move {
            let r = tokio::select! {
                r = core.pipeline(id, pick) => Some(r),
                _ = cancel.cancelled() => None,
            };
            let Some(r) = r else { return };
            // Captae 正在退出（`tools::kill_all` 已经开始结束附带程序）：aria2c、yt-dlp、ffmpeg 是被结束的，不是下载出错，
            // 不记成失败、不写 queue.json，留着开始下载时存的「下载中」，下次打开恢复成暂停（录直播的照旧整理录到的部分）。
            // Windows 上装更新时尤其要紧：更新插件先收拾再运行安装程序，这段时间里任务会被记成失败（Devin 的 Windows U4）
            if r.is_err() && crate::tools::quitting() {
                return;
            }
            core.running.lock().unwrap().remove(&id);
            let h = core.handle(id);
            match r {
                Ok(()) => {}
                Err(e) => {
                    let url = core.job(id).map(|j| j.url).unwrap_or_default();
                    let f = crate::errors::from_anyhow(&e, &url);
                    // 重试也接不上的失败（不是媒体、已删除、DRM、直播……）：.captae-tmp 里这个任务的中间文件删掉（FIXES 第 6 项）
                    use crate::errors::ErrorCode::*;
                    if matches!(f.code, NoMedia | Unavailable | Drm | Unsupported | Live | LiveUpcoming | NeedLogin) {
                        if let Some(j) = core.job(id) {
                            remove_tmp(&j);
                            // 直链（aria2）下到一半的文件直接写在保存位置：留着会被当成下好的文件（Dropbox 链接失效 410 时留下了半个 .MP4）
                            // 只删旁边有 .aria2 控制文件的（没下完、是这个任务自己的），同名的旧文件不碰
                            if j.eng == Engine::Aria2 {
                                if let Some(p) = &j.path {
                                    let ctl = PathBuf::from(format!("{}.aria2", p.display()));
                                    if ctl.exists() {
                                        let _ = std::fs::remove_file(p);
                                        let _ = std::fs::remove_file(&ctl);
                                    }
                                }
                            }
                        }
                    }
                    h.update(|j| fail_job(j, f));
                }
            }
            core.finish_waiters(id);
            core.persist_queue();
            core.wake.notify_one();
        });
    }

    fn finish_waiters(&self, id: JobId) {
        if let Some(j) = self.job(id) {
            if let Some(ws) = self.done.lock().unwrap().remove(&id) {
                for w in ws {
                    let _ = w.send(j.clone());
                }
            }
        }
    }

    fn ctx(&self, id: JobId) -> Ctx {
        let finish = self.running.lock().unwrap().get(&id).map(|r| r.finish.clone()).unwrap_or_default();
        Ctx { tools: self.toolbox(), http: self.http(), paths: self.paths.clone(), settings: self.settings(), h: self.handle(id), auth: login::Auth::default(), finish }
    }

    /// 播放列表 / 多视频帖子选好了条目：每个条目单独成一个下载任务，原来那个等选择的任务去掉。
    /// `ids` 是条目的 id（`entries[].id`）；传 `["all"]` 表示全部。返回新建的任务。
    pub fn choose(&self, id: JobId, ids: &[String]) -> Result<Vec<Job>> {
        let parent = self.job(id).ok_or_else(|| anyhow!(tr!("没有这个任务")))?;
        if !parent.choose || parent.entries.is_empty() {
            return Err(anyhow!(tr!("这个任务不是等选条目的播放列表")));
        }
        let all = ids.iter().any(|x| x == "all");
        let picked: Vec<&Entry> = parent.entries.iter().filter(|e| all || ids.iter().any(|x| x == &e.id || x == &e.n.to_string())).collect();
        if picked.is_empty() {
            return Err(anyhow!(tr!("没有选中任何条目")));
        }
        let mut out = vec![];
        for e in picked {
            let mut req = parent.req.clone();
            match e.item {
                Some(n) => {
                    req.url = parent.url.clone();
                    req.playlist_item = Some(n);
                }
                None => {
                    req.url = e.url.clone();
                    req.playlist_item = None;
                }
            }
            req.page_title = Some(e.title.clone()).filter(|t| !t.is_empty());
            out.push(self.add(req)?);
        }
        self.stop(id);
        self.jobs.lock().unwrap().remove(&id);
        self.done.lock().unwrap().remove(&id);
        self.emit(Event::JobRemoved { id });
        self.persist_queue();
        self.wake.notify_one();
        Ok(out)
    }

    /// 一个任务从读取信息到入库的完整流程。
    async fn pipeline(&self, id: JobId, pick: Arc<Notify>) -> Result<()> {
        let h = self.handle(id);
        let mut ctx = self.ctx(id);
        // Captae 上次在录直播时退出了：不重新录，把录到的部分整理好
        if h.snapshot().rec.as_ref().is_some_and(|r| r.interrupted) {
            return self.recover_recording(&h, &ctx).await;
        }
        // lnkd.in 这种要联网才拆得开的跳转：换成目标链接再往下走（FIXES 第 11 项）
        if let Some(t) = detect::unwrap_remote(&ctx.http, &h.snapshot().url).await {
            if let Some(d2) = detect::detect(&t) {
                h.update(|j| {
                    j.log(tr!("这是跳转链接，目标是 {target}", target = t));
                    j.url = t.clone();
                    j.req.url = t.clone();
                    j.src = detect::short_src(&t);
                    j.plat = d2.plat.map(str::to_string);
                    j.eng = d2.eng;
                });
            }
        }
        let job = h.snapshot();
        // 登录状态：这次带不带、带哪一份（见 login.rs）。YouTube 只有这个任务报过要登录 / 要验证以后才带
        ctx.auth = self.auth_for(&job, job.needs_login).await;
        self.note_auth(&h, &ctx.auth, &job);
        let req = job.req.clone();
        let preset = req.preset.as_deref().and_then(|n| self.preset(n));
        let no_watermark = preset.as_ref().map(|p| p.steps.iter().any(|s| matches!(Step::parse(s), Step::NoWatermark))).unwrap_or(false);
        let d = detect::detect(&job.url).ok_or_else(|| anyhow!(tr!("链接无效")))?;
        let mut eng = job.eng;
        let mut kind_hint = if req.audio_only { "audio" } else { "video" };

        // ---- 1. 读取信息 ----
        let info_json = info_json_path(&self.paths, &job.url, req.playlist_item);
        let mut aria_name: Option<String> = job.path.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().to_string());
        let mut hls_pick: Option<(String, Option<String>)> = None;
        if job.meta.is_none() || (eng == Engine::Hls && job.phase == Phase::Pick) || (eng == Engine::Hls && job.vars.is_empty() && job.segs.is_empty()) {
            h.update(|j| {
                j.phase = Phase::Probe;
                j.log(tr!("正在读取视频信息"));
                j.probe_at = Some(crate::util::now_ms());
            });
            // 直链先 HEAD：打开是网页（Dropbox 分享页、Commons 文件页）改走下载引擎（FIXES 第 9 项）；电台流按直播（第 13 项）
            let mut pre_head: Option<aria2::Head> = None;
            if eng == Engine::Aria2 {
                let head = aria2::head(&ctx.http, &job.url, &req.headers, &ctx.auth.jar()).await?;
                if head.is_html() {
                    eng = Engine::YtDlp;
                    h.update(|j| {
                        j.eng = Engine::YtDlp;
                        j.log(tr!("这个地址打开是网页，不是文件，改用下载引擎读取"));
                    });
                } else {
                    let m3u8 = head.content_type.as_deref().is_some_and(|c| c.contains("mpegurl"));
                    if !m3u8 && head.is_stream() && !req.record_live {
                        return Err(crate::errors::radio(format!("audio stream without length (icy={}): {}", head.icy, job.url)).into());
                    }
                    pre_head = Some(head);
                }
            }
            match eng {
                Engine::YtDlp => {
                    let nh = h.clone();
                    let note = move |s: String| {
                        nh.update(|j| j.warn(s));
                    };
                    let pr = ytdlp::ProbeReq { url: &job.url, headers: &req.headers, item: req.playlist_item, record_live: req.record_live, note: &note, auth: Some(&ctx.auth).filter(|a| !a.is_empty()), extra: vec![] };
                    let mut res = ytdlp::probe_url(&ctx.tools, &ctx.http, &pr).await;
                    // YouTube 要登录 / 要验证（年龄限制、会员、确认不是机器人）：有现成的登录状态就当场带上再读一次，不用用户再点
                    if let Err(f) = &res {
                        if matches!(f.code, crate::errors::ErrorCode::NeedLogin | crate::errors::ErrorCode::NeedVerify) && ctx.auth.is_empty() && !job.needs_login {
                            h.update(|j| j.needs_login = true);
                            let a = self.auth_for(&h.snapshot(), true).await;
                            if !a.is_empty() {
                                let msg = f.message.clone();
                                h.update(|j| j.warn(tr!("{reason}，带上登录状态再读一次", reason = msg)));
                                self.note_auth(&h, &a, &job);
                                ctx.auth = a;
                                let pr = ytdlp::ProbeReq { url: &job.url, headers: &req.headers, item: req.playlist_item, record_live: req.record_live, note: &note, auth: Some(&ctx.auth), extra: vec![] };
                                res = ytdlp::probe_url(&ctx.tools, &ctx.http, &pr).await;
                            }
                        }
                    }
                    // 读浏览器里的登录状态失败了（Windows 上 Chrome 的 app-bound 加密、浏览器开着锁住了数据库，Linux 上密钥环没解锁……）：
                    // yt-dlp 会让整个读取失败。说清楚原因，这次不带浏览器的登录状态再读一次（公开的视频照样能下）
                    if let (Err(f), Some(b)) = (&res, ctx.auth.browser.clone()) {
                        if let Some(hint) = login::browser_failure_hint(&format!("{}\n{}", f.raw, f.message), &b) {
                            h.update(|j| j.warn(tr!("{reason}。这次先不带它的登录状态再读一次", reason = hint)));
                            ctx.auth = login::Auth::default();
                            let pr = ytdlp::ProbeReq { url: &job.url, headers: &req.headers, item: req.playlist_item, record_live: req.record_live, note: &note, auth: None, extra: vec![] };
                            res = ytdlp::probe_url(&ctx.tools, &ctx.http, &pr).await;
                        }
                    }
                    match res? {
                        ytdlp::Probed::List { title, entries } => {
                            // 播放列表、多视频帖子：停下来等用户选（state=pause + phase=probe + choose）
                            let n = entries.len();
                            h.update(|j| {
                                j.state = JobState::Pause;
                                j.phase = Phase::Probe;
                                j.boost = false;
                                j.speed = 0.0;
                                j.choose = true;
                                j.thumb = entries.iter().find_map(|e| e.thumb.clone());
                                j.entries = entries;
                                j.title = title.filter(|t| !t.trim().is_empty()).unwrap_or_else(|| tr!("{host} 的 {n} 个视频", host = d.host, n = n));
                                j.untitled = false;
                                j.log(tr!("这里有 {n} 个视频，选好要下载的再开始", n = n));
                            });
                            return Ok(());
                        }
                        ytdlp::Probed::Video { info, mut raw, playlist, extra } => {
                            // 下载时用 --load-info-json，不再重新解析网页。存盘前去掉我们带进去的登录 cookie（下载时会重新带上）
                            ytdlp::strip_auth_cookies(&mut raw, &ctx.auth);
                            ytdlp::prepare_info(&mut raw);
                            // 单张图片（picsum、Truth Social 引用帖）：照原样存成图片（FIXES 第 8 项）
                            let image = ytdlp::Shape::of(Some(&info)).image;
                            if let Ok(b) = serde_json::to_vec(&raw) {
                                let _ = tokio::fs::write(&info_json, b).await;
                            }
                            let title = ytdlp::title_or(&info, &d.host);
                            h.update(|j| {
                                j.title = title;
                                j.untitled = false;
                                j.dur = ytdlp::dur_label(&info);
                                j.thumb = info.thumbnail.clone();
                                if let Some(sz) = info.filesize {
                                    j.size = mb(sz as f64);
                                }
                                if image {
                                    j.fmt = tr!("{ext} 图片", ext = info.ext.clone().unwrap_or_default().to_uppercase());
                                    j.log(tr!("这是一张图片，照原样保存"));
                                }
                                if let Some(p) = &playlist {
                                    if ytdlp::bilibili_set_of(&j.url).is_some() {
                                        j.log(match (p.index, p.count) {
                                            (Some(i), Some(n)) => tr!("这是分 P 视频里的第 {index} P（共 {n} P），只下载这一 P（全部分 P：{url}）", index = i, n = n, url = p.url),
                                            _ => tr!("这是分 P 视频里的一 P，只下载这一 P（全部分 P：{url}）", url = p.url),
                                        });
                                    } else {
                                        j.log(tr!("这是播放列表里的一个视频，只下载这一个（整个列表：{url}）", url = p.url));
                                    }
                                }
                                j.playlist = playlist;
                                j.probe_args = extra;
                                // 正在直播、要从现在开始录（ytdlp::probe_url 认出来的会标成 is_live）
                                j.live = j.req.record_live && info.live_status.as_deref() == Some("is_live");
                                j.meta = Some(info);
                            });
                        }
                    }
                }
                Engine::Aria2 => {
                    let head = match pre_head.take() {
                        Some(x) => x,
                        None => aria2::head(&ctx.http, &job.url, &req.headers, &ctx.auth.jar()).await?,
                    };
                    if head.content_type.as_deref().map(|c| c.contains("mpegurl")).unwrap_or(false) {
                        eng = Engine::Hls;
                        h.update(|j| j.eng = Engine::Hls);
                    } else {
                        kind_hint = detect::media_kind_of_ext(head.filename.rsplit('.').next().unwrap_or(""));
                        aria_name = Some(head.filename.clone());
                        let info = aria2::probe_info(&head);
                        // 电台流、要从现在开始录：存进「音乐」
                        let live = req.record_live && head.is_stream();
                        if live {
                            kind_hint = "audio";
                        }
                        h.update(|j| {
                            j.live = live;
                            if j.untitled || !j.title.contains(' ') || j.title == d.host {
                                j.title = head.filename.clone();
                                j.untitled = false;
                            }
                            if let Some(s) = head.size {
                                j.size = mb(s as f64);
                            }
                            j.meta = Some(info);
                        });
                    }
                }
                Engine::Hls => {}
            }
            if eng == Engine::Hls {
                let p = hls::probe_live(&ctx.http, &job.url, &req.headers, &ctx.auth.jar(), req.record_live).await?;
                let pick_secs = ctx.settings.pick_seconds as i64;
                h.update(|j| {
                    j.live = p.live;
                    j.dur = p.info.duration.map(clock).unwrap_or_default();
                    j.meta = Some(p.info.clone());
                    if !p.variants.is_empty() {
                        j.vars = p.variants.clone();
                        j.size = j.vars[j.v_idx.min(j.vars.len() - 1)].mb;
                        let list: Vec<&str> = j.vars.iter().map(|v| v.p.as_str()).collect();
                        j.log(tr!("有 {n} 种清晰度可选：{list}", n = j.vars.len(), list = crate::i18n::join(&list)));
                    }
                });
                // 已经指定了清晰度（插件里选好的「1080p」，或 AI 助手传的 quality）：直接用，不再等人选
                let wanted = req.quality.as_deref().filter(|q| !q.is_empty() && *q != "最高可用").and_then(|q| {
                    let vs = &p.variants;
                    vs.iter().position(|v| v.p == q).or_else(|| {
                        let h: u32 = q.trim_end_matches(['p', 'P']).parse().ok()?;
                        vs.iter().position(|v| v.p.trim_end_matches('p').parse::<u32>().map(|x| x <= h).unwrap_or(false)).or(Some(vs.len().saturating_sub(1)))
                    })
                });
                if let (Some(k), true) = (wanted, p.variants.len() > 1) {
                    let v = p.variants[k].clone();
                    h.update(|j| {
                        j.v_idx = k;
                        j.size = v.mb;
                        j.log(tr!("按指定的清晰度用 {quality}", quality = v.p));
                    });
                    hls_pick = Some((v.uri.clone(), v.audio.clone()));
                } else if p.variants.len() > 1 {
                    h.update(|j| {
                        j.phase = Phase::Pick;
                        j.pick_until = Some(now_ms() + pick_secs * 1000);
                    });
                    let picked = tokio::time::timeout(std::time::Duration::from_secs(pick_secs as u64), pick.notified()).await.is_ok();
                    let v = h.snapshot().vars[h.snapshot().v_idx].clone();
                    h.update(|j| {
                        j.pick_until = None;
                        j.log(if picked { tr!("选择了 {quality}", quality = v.p) } else { tr!("没有选择，自动用 {quality}", quality = v.p) });
                    });
                    hls_pick = Some((v.uri.clone(), v.audio.clone()));
                } else if let Some(v) = p.variants.first() {
                    hls_pick = Some((v.uri.clone(), v.audio.clone()));
                } else {
                    hls_pick = p.media_url.map(|u| (u, None));
                }
            }
        } else if eng == Engine::Hls {
            let j = h.snapshot();
            hls_pick = j.vars.get(j.v_idx).map(|v| (v.uri.clone(), v.audio.clone())).or(Some((j.url.clone(), None)));
        }

        // ---- 2. 下载 ----
        if eng == Engine::YtDlp && ytdlp::Shape::of(h.snapshot().meta.as_ref()).image && !req.audio_only {
            kind_hint = "image";
        }
        let out_dir = req
            .output_dir
            .clone()
            .or_else(|| Some(PathBuf::from(&ctx.settings.save_dir)).filter(|p| !p.as_os_str().is_empty()))
            .unwrap_or_else(|| self.paths.dir_for_kind(kind_hint).to_path_buf());
        tokio::fs::create_dir_all(&out_dir).await?;
        h.update(|j| {
            j.phase = Phase::Dl;
            if let (Engine::Hls, Some(v)) = (j.eng, j.vars.get(j.v_idx)) {
                j.fmt = format!("MP4 · {}", v.p);
            }
            if j.live {
                // 录直播：从这一刻算「已录多久」；录到哪由引擎记下（note_raw），马上存进 queue.json
                let max = j.req.record_max_secs.filter(|s| *s > 0);
                j.rec = Some(Rec { since: now_ms(), out: Some(out_dir.clone()), max, ..Default::default() });
                j.size = 0.0;
                j.got = 0.0;
                match max {
                    Some(s) => j.log(tr!("开始录直播，最多录 {limit}，录满自动停止并保存（也可以提前点「停止并保存」）", limit = limit_label(s))),
                    None => j.log(tr!("开始录直播，录到你点「停止并保存」为止")),
                }
            } else {
                j.log(tr!("开始下载§任务记录"));
            }
        });
        // 开始下载时存一次 queue.json：下载中退出，重新打开时照旧暂停、等用户点继续（还记着「读取信息」的话会自己重新开始）；
        // 录直播要马上记下开始录了
        self.persist_queue();
        let out = match eng {
            Engine::YtDlp => {
                let ij = info_json.exists().then_some(info_json.as_path()).filter(|p| {
                    // 信息里的直链一般几小时后过期，太旧就重新解析
                    std::fs::metadata(p).and_then(|m| m.modified()).map(|t| t.elapsed().map(|e| e.as_secs() < 1800).unwrap_or(false)).unwrap_or(false)
                });
                let tmp = ytdlp::temp_dir(&out_dir, &job.url, req.playlist_item);
                h.update(|j| j.tmp = Some(tmp));
                match ytdlp::download(&ctx, &job.url, ij, &out_dir, no_watermark).await {
                    // YouTube 下载时偶发 403：读取阶段有「换一组客户端」，下载阶段也来一次，还不行才说要验证（FIXES 第 17 项）
                    Err(e) if ytdlp::is_youtube(&job.url) && !h.snapshot().probe_args.iter().any(|a| a.contains("player_client")) && crate::errors::failure_in(&e).is_some_and(|f| f.code == crate::errors::ErrorCode::NeedVerify && f.raw.contains("403")) => {
                        h.update(|j| j.warn(tr!("YouTube 拒绝了下载（403），换一组客户端（{clients}）重新读取，再下一次", clients = ytdlp::YT_ALT_CLIENTS)));
                        let nh = h.clone();
                        let note = move |s: String| {
                            nh.update(|j| j.warn(s));
                        };
                        let pr = ytdlp::ProbeReq { url: &job.url, headers: &req.headers, item: req.playlist_item, record_live: req.record_live, note: &note, auth: Some(&ctx.auth).filter(|a| !a.is_empty()), extra: ytdlp::alt_clients() };
                        let ytdlp::Probed::Video { info, mut raw, extra, .. } = ytdlp::probe_url(&ctx.tools, &ctx.http, &pr).await? else {
                            return Err(e);
                        };
                        ytdlp::strip_auth_cookies(&mut raw, &ctx.auth);
                        ytdlp::prepare_info(&mut raw);
                        tokio::fs::write(&info_json, serde_json::to_vec(&raw).unwrap_or_default()).await?;
                        h.update(|j| {
                            j.probe_args = extra;
                            j.meta = Some(info);
                        });
                        ytdlp::download(&ctx, &job.url, Some(&info_json), &out_dir, no_watermark).await?
                    }
                    r => r?,
                }
            }
            Engine::Aria2 => {
                let name = aria_name.ok_or_else(|| anyhow!(tr!("没有拿到文件名")))?;
                h.update(|j| j.path = Some(out_dir.join(&name)));
                aria2::download(&ctx, &job.url, &out_dir, &name).await?
            }
            Engine::Hls => {
                let (media, audio) = hls_pick.ok_or_else(|| anyhow!(tr!("没有可用的清晰度")))?;
                let snap = h.snapshot();
                let base = snap.req.page_title.clone().or(snap.meta.as_ref().and_then(|m| m.title.clone())).unwrap_or_else(|| snap.title.trim_end_matches(".m3u8").to_string());
                let target = crate::util::unique_path(out_dir.join(format!("{}.mp4", crate::util::safe_name(&base))));
                if snap.live {
                    hls::record(&ctx, &media, audio.as_deref(), &target).await?
                } else {
                    hls::download(&ctx, &media, audio.as_deref(), &target).await?
                }
            }
        };
        let _ = tokio::fs::remove_file(&info_json).await;
        let mut path = out.path.clone();

        // ---- 3. 后处理：格式 ----
        // yt-dlp 只有在装不进 MP4、按原格式重下时才会 format_done = false（FIXES 第 16 项）：没选格式时默认就是 MP4，照样转
        let want = req.format.clone().or_else(|| (eng == Engine::YtDlp && !req.audio_only).then(|| "MP4".to_string())).filter(|f| f != "保持原格式");
        if let (false, Some(f)) = (out.format_done, want) {
            let cur = path.extension().map(|e| e.to_string_lossy().to_ascii_uppercase()).unwrap_or_default();
            let is_video = detect::media_kind_of_ext(&cur.to_lowercase()) == "video";
            if is_video && !f.eq_ignore_ascii_case(&cur) {
                h.update(|j| {
                    j.phase = Phase::Post;
                    j.merge = 0.0;
                    j.speed = 0.0;
                    j.log(tr!("正在转成 {format}", format = f));
                });
                let h2 = h.clone();
                let newp = crate::ffmpeg::convert(&ctx.tools, &path, &f, None, vec![], move |p| {
                    h2.update(|j| j.merge = p * 100.0);
                })
                .await?;
                let _ = tokio::fs::remove_file(&path).await;
                path = newp;
            }
        }

        // ---- 3.5 核对、4. 入库 ----
        let (asset_id, media) = self.check_and_ingest(&h, &ctx.tools, &path).await?;

        // ---- 5. 预设里下载之后的步骤 ----
        if let Some(p) = preset {
            h.update(|j| {
                j.phase = Phase::Post;
                j.log(tr!("正在执行预设「{name}」", name = p.name));
            });
            let res = crate::presets::run_post(self, &p, asset_id, Some(h.clone())).await?;
            if let Some(last) = res.last() {
                h.update(|j| j.path = Some(last.clone()));
            }
        }

        self.finish_done(&h, &path, asset_id, media.as_ref());
        Ok(())
    }

    /// 下载完（或者录完）的文件：核对下到的是不是媒体、是不是只有试看片段（FIXES 第 7、8、9 项），再入库，
    /// 任务上记下文件、素材编号和封面。返回素材编号和 ffprobe 读到的信息
    async fn check_and_ingest(&self, h: &JobHandle, tools: &Toolbox, path: &Path) -> Result<(i64, Option<crate::ffmpeg::Probe>)> {
        // 以前只看下载工具的退出码：网页、HTML 当成 .mp4 进了素材库，会员节目只下到 5 分钟试看也当成完整的
        let snap = h.snapshot();
        let media = match crate::ffmpeg::probe(tools, path).await {
            Ok(p) => Some(p),
            // ffprobe 本身不在、启动不了：没法核对，照旧入库
            Err(e) if crate::errors::from_anyhow(&e, &snap.url).code == crate::errors::ErrorCode::Tool => None,
            Err(e) => {
                let _ = tokio::fs::remove_file(&path).await;
                return Err(not_media(&tr!("ffprobe 读不了 {path}：{error}", path = path.display(), error = format!("{e:#}"))).into());
            }
        };
        if let Some(p) = &media {
            if !usable(p) {
                let _ = tokio::fs::remove_file(&path).await;
                return Err(not_media(&format!("ffprobe: kind={} format={} size={:?}x{:?} duration={:?} ({})", p.kind, p.format, p.width, p.height, p.duration, path.display())).into());
            }
            if let Some(f) = preview_after(&snap, p) {
                let _ = tokio::fs::remove_file(&path).await;
                return Err(f.into());
            }
        }

        // 入库
        let asset_id = self.ingest_probed(&snap, path, media.clone()).await?;
        let local_thumb = self.lib.get(asset_id).ok().flatten().and_then(|a| a.thumb).map(|t| t.display().to_string());
        h.update(|j| {
            j.path = Some(path.to_path_buf());
            j.asset_id = Some(asset_id);
            // 直链和 M3U8 没有网站封面，用入库时截的那一帧
            if j.thumb.is_none() {
                j.thumb = local_thumb;
            }
        });
        Ok((asset_id, media))
    }

    /// 任务完成：记下完成时间，通知界面（下载完成的提示、素材库刷新）。录直播的写上录了多久；
    /// 录制被中断、这次打开才整理好的，另外提示一次放在哪
    fn finish_done(&self, h: &JobHandle, path: &Path, asset_id: i64, media: Option<&crate::ffmpeg::Probe>) {
        let dir = self.paths.human_dir(path);
        // 手机上没有「影片 › Captae」这种文件夹（下完的进 App 的素材库，存相册是 App 的事）：不写位置，只说进了素材库。
        // Rust 这边分不出 iPhone 和 iPad，所以不写设备名
        let phone = self.runner.kind() != "desktop";
        let secs = media.and_then(|m| m.duration);
        let bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        let mut interrupted = false;
        h.update(|j| {
            j.state = JobState::Done;
            j.phase = Phase::Done;
            j.boost = false;
            j.speed = 0.0;
            j.got = j.size.max(j.got);
            j.when = Some(tr!("刚刚"));
            j.done_at = Some(now_ms());
            match j.rec.as_mut() {
                Some(r) => {
                    r.until.get_or_insert(now_ms());
                    r.stopping = false;
                    r.secs = secs;
                    interrupted = r.interrupted;
                    if bytes > 0 {
                        j.size = mb(bytes as f64);
                        j.got = j.size;
                    }
                    if let Some(s) = secs {
                        j.dur = clock(s);
                    }
                    // 没读到时长时原来写「一段」：各自是一整句，不拼片段
                    j.log(match (interrupted, secs) {
                        _ if phone => match (interrupted, secs) {
                            (true, Some(s)) => tr!("录制被中断（Captae 退出时还在录），录到的 {dur} 已经整理好，存进了素材库", dur = long_dur(s)),
                            (true, None) => tr!("录制被中断（Captae 退出时还在录），录到的 一段 已经整理好，存进了素材库"),
                            (false, Some(s)) => tr!("录了 {dur}，已存进素材库", dur = long_dur(s)),
                            (false, None) => tr!("录了 一段，已存进素材库"),
                        },
                        (true, Some(s)) => tr!("录制被中断（Captae 退出时还在录），录到的 {dur} 已经整理好，保存到 {dir}", dur = long_dur(s), dir = dir),
                        (true, None) => tr!("录制被中断（Captae 退出时还在录），录到的 一段 已经整理好，保存到 {dir}", dir = dir),
                        (false, Some(s)) => tr!("录了 {dur}，已保存到 {dir}", dur = long_dur(s), dir = dir),
                        (false, None) => tr!("录了 一段，已保存到 {dir}", dir = dir),
                    });
                }
                None if phone => j.log(tr!("完成，已存进素材库")),
                None => j.log(tr!("完成，已保存到 {dir}", dir = dir)),
            }
        });
        let title = h.snapshot().title;
        if interrupted {
            self.emit(Event::Notice {
                key: format!("rec_interrupted_{}", h.id),
                title: tr!("上次退出时还在录的直播已经保存好了"),
                text: match secs {
                    Some(s) if phone => tr!("「{title}」录到的 {dur} 整理好了，在素材库里", title = title, dur = long_dur(s)),
                    None if phone => tr!("「{title}」录到的 一段 整理好了，在素材库里", title = title),
                    Some(s) => tr!("「{title}」录到的 {dur} 整理好了，放在 {dir}，素材库里也有", title = title, dur = long_dur(s), dir = dir),
                    None => tr!("「{title}」录到的 一段 整理好了，放在 {dir}，素材库里也有", title = title, dir = dir),
                },
            });
        }
        self.emit(Event::Finished { id: h.id, title, path: path.to_path_buf(), asset_id: Some(asset_id) });
        self.emit(Event::Library);
    }

    /// Captae 上次在录直播时退出了（或者崩溃、被强制结束）：不重新录，把录到的部分整理成能播放的文件，放进保存位置和素材库，
    /// 任务记成完成、写明「录制被中断」。录制时写的都是 MPEG-TS、没有文件头的音频流这类边写边能读的格式，进程被直接结束也只丢最后一点
    async fn recover_recording(&self, h: &JobHandle, ctx: &Ctx) -> Result<()> {
        let job = h.snapshot();
        let rec = job.rec.clone().unwrap_or_default();
        let raw = rec.raw.clone().filter(|p| p.exists()).ok_or_else(|| anyhow!(tr!("录制被中断了，录到的部分没有找到（可能已经被删掉了）")))?;
        let out_dir = rec.out.clone().unwrap_or_else(|| self.paths.dir_for_kind(if job.audio { "audio" } else { "video" }).to_path_buf());
        let stem = if job.untitled { tr!("直播录像") } else { job.title.trim_end_matches(".m3u8").to_string() };
        let path = if raw.is_dir() {
            // M3U8 引擎录的（画面一路、可能还有单独一路声音），或者 yt-dlp 的中间目录（里面最大的那个就是录到的）
            let (v, a) = hls::recorded_tracks(&raw);
            let (v, a) = match v {
                Some(v) => (v, a),
                None => (crate::engine::largest_file(&raw).ok_or_else(|| anyhow!(tr!("录制被中断了，还没有录到内容")))?, None),
            };
            let p = crate::ffmpeg::tidy_recording(&ctx.tools, &v, a.as_deref(), &out_dir, &stem).await?;
            let _ = tokio::fs::remove_dir_all(&raw).await;
            if let Some(parent) = raw.parent().filter(|p| p.ends_with(".captae-tmp")) {
                let _ = tokio::fs::remove_dir(parent).await;
            }
            p
        } else {
            // 电台流：aria2 直接写在保存位置的那个文件
            let _ = tokio::fs::remove_file(format!("{}.aria2", raw.display())).await;
            aria2::tidy_stream(&ctx.tools, &raw).await?
        };
        let (asset_id, media) = self.check_and_ingest(h, &ctx.tools, &path).await?;
        self.finish_done(h, &path, asset_id, media.as_ref());
        Ok(())
    }

    /// 文件入库：ffprobe 取技术参数，生成缩略图，写入元数据。
    pub async fn ingest(&self, job: &Job, path: &Path) -> Result<i64> {
        let probe = crate::ffmpeg::probe(&self.toolbox(), path).await.ok();
        self.ingest_probed(job, path, probe).await
    }

    async fn ingest_probed(&self, job: &Job, path: &Path, probe: Option<crate::ffmpeg::Probe>) -> Result<i64> {
        let tools = self.toolbox();
        // 没有 ffprobe（手机上）：MP4 / M4A 自己读 moov，素材库里照样有类型、宽高、时长；格式按扩展名
        let no_ffprobe = !tools.has(crate::tools::Tool::Ffprobe);
        let probe = probe.or_else(|| no_ffprobe.then(|| crate::engine::remux::mp4_info(path)).flatten()).unwrap_or_default();
        let meta = job.meta.clone().unwrap_or_default();
        let kind = if probe.kind.is_empty() { detect::media_kind_of_ext(&path.extension().unwrap_or_default().to_string_lossy()).to_string() } else { probe.kind.clone() };
        let thumb = if kind == "video" || kind == "image" {
            let t = self.paths.thumbs.join(format!("{}.jpg", now_ms()));
            crate::ffmpeg::thumbnail(&tools, path, &kind, probe.duration, &t).await.ok().map(|_| t)
        } else {
            None
        };
        let size = tokio::fs::metadata(path).await.map(|m| m.len()).unwrap_or(0);
        let site = site_label(&job.url, job.req.page_url.as_deref(), meta.extractor.as_deref());
        let title = if job.untitled { path.file_stem().unwrap_or_default().to_string_lossy().to_string() } else { job.title.clone() };
        let a = NewAsset {
            kind,
            title,
            description: meta.description.clone().unwrap_or_default().chars().take(4000).collect(),
            source_url: job.url.clone(),
            page_url: job.req.page_url.clone().or(meta.webpage_url.clone()).unwrap_or_default(),
            site,
            uploader: meta.uploader.clone().unwrap_or_default(),
            path: path.to_path_buf(),
            format: if probe.format.is_empty() && no_ffprobe { path.extension().map(|e| e.to_string_lossy().to_uppercase()).unwrap_or_default() } else { probe.format.clone() },
            width: probe.width.or(meta.width),
            height: probe.height.or(meta.height),
            duration: probe.duration.or(meta.duration),
            size,
            thumb,
            tags: vec![],
            parent_id: None,
            // 录直播录下来的：插件、AI 助手按链接查「下载过没有」时不算（同一个直播链接下次录的是别的内容）
            live: job.rec.is_some(),
        };
        let id = self.lib.add(&a)?;
        Ok(id)
    }

    /// 把本地已有文件加入素材库（转换、剪辑的产物也走这里）。
    pub async fn import_file(&self, path: &Path, parent: Option<i64>, title: Option<String>) -> Result<i64> {
        let tools = self.toolbox();
        let probe = crate::ffmpeg::probe(&tools, path).await?;
        let parent_asset = parent.and_then(|p| self.lib.get(p).ok().flatten());
        let thumb = if probe.kind == "video" || probe.kind == "image" {
            let t = self.paths.thumbs.join(format!("{}.jpg", now_ms()));
            crate::ffmpeg::thumbnail(&tools, path, &probe.kind, probe.duration, &t).await.ok().map(|_| t)
        } else {
            None
        };
        let a = NewAsset {
            kind: probe.kind.clone(),
            title: title.unwrap_or_else(|| path.file_stem().unwrap_or_default().to_string_lossy().to_string()),
            description: parent_asset.as_ref().map(|p| p.desc.clone()).unwrap_or_default(),
            source_url: parent_asset.as_ref().map(|p| p.url.clone()).unwrap_or_default(),
            page_url: parent_asset.as_ref().map(|p| p.page.clone()).unwrap_or_default(),
            site: parent_asset.as_ref().map(|p| p.src.clone()).unwrap_or_else(|| "本地".into()),
            uploader: parent_asset.as_ref().map(|p| p.uploader.clone()).unwrap_or_default(),
            path: path.to_path_buf(),
            format: probe.format.clone(),
            width: probe.width,
            height: probe.height,
            duration: probe.duration,
            size: probe.size.max(tokio::fs::metadata(path).await.map(|m| m.len()).unwrap_or(0)),
            thumb,
            // 直播录像转换、剪辑出来的也带着同一个来源链接：一样不算「下载过了」
            live: parent_asset.as_ref().is_some_and(|p| p.live),
            tags: parent_asset.map(|p| p.tags).unwrap_or_default(),
            parent_id: parent,
        };
        let id = self.lib.add(&a)?;
        self.emit(Event::Library);
        Ok(id)
    }

    // ------------------------------------------------------------------ 登录（第二阶段）

    /// 桌面端启动时接上登录窗口的存储
    pub fn set_login_store(&self, s: Arc<dyn login::Store>) {
        *self.login_store.write().unwrap() = Some(s);
    }

    /// 在 Captae 里登录过 / 验证过的网站
    pub fn logins(&self) -> login::Logins {
        self.logins.lock().unwrap().clone()
    }

    /// 记下：这个网站在 Captae 里登录了 / 验证过了（只记网站和时间）
    pub fn login_mark(&self, key: &str, how: login::How, at: Option<i64>) {
        let mut l = self.logins.lock().unwrap();
        l.mark(key, how, at);
        let _ = l.save(&self.paths.logins());
    }

    /// 忘掉这个网站（退出、清除验证以后）
    pub fn login_forget(&self, key: &str) -> bool {
        let mut l = self.logins.lock().unwrap();
        let gone = l.forget(key);
        if gone {
            let _ = l.save(&self.paths.logins());
        }
        gone
    }

    /// 在等这个网站登录 / 验证的任务：失败了、错误码是 need_login / need_verify、链接或来源页面属于这个网站
    pub fn login_waiting(&self, key: &str) -> Vec<JobId> {
        use crate::errors::ErrorCode::{NeedLogin, NeedVerify};
        self.jobs()
            .into_iter()
            .filter(|j| j.state == JobState::Failed && j.fail.as_ref().is_some_and(|f| matches!(f.code, NeedLogin | NeedVerify)))
            .filter(|j| login::url_in_key(&j.url, key) || j.req.page_url.as_deref().is_some_and(|p| login::url_in_key(p, key)))
            .map(|j| j.id)
            .collect()
    }

    /// 登录好了 / 验证完了：这些任务带上登录状态重试（YouTube 也带）。返回重试了几个
    pub fn login_retry(&self, ids: &[JobId]) -> usize {
        let mut n = 0;
        for &id in ids {
            if self.job(id).is_some_and(|j| j.state == JobState::Failed) {
                self.handle(id).update(|j| j.needs_login = true);
                if self.act(id, JobAction::Retry).is_ok() {
                    n += 1;
                }
            }
        }
        n
    }

    /// 这个任务这次要带的登录状态（规则见 `login::plan`）。`needed`：报过要登录 / 要验证（YouTube 只有这时才带）
    pub async fn auth_for(&self, job: &Job, needed: bool) -> login::Auth {
        let req = &job.req;
        let browser = self.settings().cookies_from_browser;
        let plan = {
            let l = self.logins.lock().unwrap();
            login::plan(&job.url, req.page_url.as_deref(), !req.cookies.is_empty(), needed, &l, &browser)
        };
        let dir = login::auth_dir(&self.paths);
        let tools = self.toolbox();
        match plan {
            login::Plan::None => login::Auth::default(),
            login::Plan::Extension => login::extension_auth(req.cookies.clone(), req.user_agent.clone(), &tools, dir).await,
            login::Plan::Store(key) => {
                let store = self.login_store.read().unwrap().clone();
                let Some(store) = store else { return login::Auth::default() };
                let how = self.logins.lock().unwrap().get(&key).map(|r| r.how).unwrap_or(login::How::Login);
                match store.cookies(login::key_domains(&key)).await {
                    Ok(c) if !c.is_empty() => login::store_auth(&key, how, c, &tools, dir).await,
                    Ok(_) => login::Auth::default(),
                    Err(e) => {
                        tracing::warn!("读登录存储失败：{e:#}");
                        login::Auth::default()
                    }
                }
            }
            // 读浏览器只有 yt-dlp 会：直链、M3U8 用不上
            login::Plan::Browser(b) if job.eng == Engine::YtDlp => {
                login::Auth { browser: Some(b.into()), from: tr!("{browser} 里的登录状态", browser = login::browser_name(b)), dir, ..Default::default() }
            }
            login::Plan::Browser(_) => login::Auth::default(),
        }
    }

    /// 任务记录里写一句这次带了哪份登录状态（不写值）；第一次用 YouTube 账号时提示一次
    fn note_auth(&self, h: &JobHandle, auth: &login::Auth, job: &Job) {
        if auth.is_empty() {
            return;
        }
        let from = auth.from.clone();
        h.update(|j| j.log(tr!("带上了登录状态：{from}", from = from)));
        let youtube = login::is_youtube(&job.url) || job.req.page_url.as_deref().is_some_and(login::is_youtube);
        if !youtube {
            return;
        }
        let first = {
            let mut l = self.logins.lock().unwrap();
            let first = l.tell_once("youtube");
            if first {
                let _ = l.save(&self.paths.logins());
            }
            first
        };
        if first {
            self.emit(Event::Notice {
                key: "youtube_login".into(),
                title: tr!("这次带上了你的 YouTube 登录状态"),
                text: tr!("这个视频要登录才能看。只有遇到年龄限制、会员视频、要确认不是机器人时才带；用账号大量下载，账号可能被 YouTube 限制。"),
            });
        }
    }

    // ------------------------------------------------------------------ 预设

    pub fn presets(&self) -> Vec<Preset> {
        self.presets.read().unwrap().clone()
    }
    pub fn preset(&self, name: &str) -> Option<Preset> {
        self.presets.read().unwrap().iter().find(|p| p.name == name).cloned()
    }
    pub fn set_presets(&self, v: Vec<Preset>) -> Result<()> {
        crate::presets::save(&self.paths.presets(), &v)?;
        *self.presets.write().unwrap() = v;
        Ok(())
    }

    // ------------------------------------------------------------------ 嗅探

    pub fn push_sniffed(&self, s: Sniffed) {
        let mut v = self.sniffed.lock().unwrap();
        v.retain(|x| x.url != s.url);
        v.insert(0, s.clone());
        v.truncate(50);
        drop(v);
        self.emit(Event::Sniffed { item: s });
    }
    pub fn sniffed(&self) -> Vec<Sniffed> {
        self.sniffed.lock().unwrap().clone()
    }
    pub fn take_sniffed(&self, url: &str) -> Option<Sniffed> {
        let mut v = self.sniffed.lock().unwrap();
        let i = v.iter().position(|s| s.url == url)?;
        Some(v.remove(i))
    }

    // ------------------------------------------------------------------ 持久化

    fn queue_file(&self) -> PathBuf {
        self.paths.data.join("queue.json")
    }

    /// 只存未完成任务的请求，重启后恢复成暂停状态，不自动开跑。
    fn persist_queue(&self) {
        if !self.owns_queue || self.restoring.load(Ordering::SeqCst) {
            return;
        }
        let jobs = self.jobs();
        let held = self.held.lock().unwrap().clone();
        // 未完成的全部保留；已完成的只留最近 30 个，让「已完成」页签重开后不是空的
        let done: Vec<_> = jobs.iter().filter(|j| j.state == JobState::Done).rev().take(30).collect();
        let v: Vec<serde_json::Value> = jobs
            .iter()
            .filter(|j| j.state.is_active() || j.state == JobState::Failed)
            .chain(done.into_iter().rev())
            // 登录状态（插件带来的 cookies、Cookie 请求头）不写进 queue.json
            .map(|j| {
                let mut x = serde_json::json!({"req": j.req.without_secrets(), "title": j.title, "pri": j.pri, "err": j.err, "job": j});
                if held.contains(&j.id) {
                    x["held"] = true.into();
                }
                x
            })
            .collect();
        let _ = std::fs::write(self.queue_file(), serde_json::to_vec(&v).unwrap_or_default());
    }

    /// 恢复上次的队列。每一条先 `add` 再改回原来的状态：`add` 放进去时是「排队 / 读取」，
    /// 这期间不能写盘，不然最后恢复的那一条（最新完成的）会以「排队」留在 queue.json 里，下次打开又下一遍。
    /// 所以恢复期间 `persist_queue` 不写，全部恢复完再写一次（恢复到一半崩了，原来的 queue.json 也还在）。
    /// 调度器在这之后才启动（open_with），恢复期间没有任务会被开始
    fn restore_queue(&self) {
        self.restoring.store(true, Ordering::SeqCst);
        self.restore_queue_inner();
        self.restoring.store(false, Ordering::SeqCst);
        self.persist_queue();
    }

    fn restore_queue_inner(&self) {
        let Ok(b) = std::fs::read(self.queue_file()) else { return };
        let Ok(v) = serde_json::from_slice::<Vec<serde_json::Value>>(&b) else { return };
        for x in v {
            let Ok(mut req) = serde_json::from_value::<AddRequest>(x["req"].clone()) else { continue };
            req.from_sniff = true;
            if let Ok(j) = self.add(req) {
                let title = x["title"].as_str().map(str::to_string);
                let saved = &x["job"];
                // 点「重新打开」装更新之前停下来的（hold_for_update）：恢复成暂停，但不写「点继续接着下」，桌面端确认更新过以后马上接着下
                // （记录里接着就是「继续」，写了反倒前后矛盾）；没接着下的由 release_held 补上这一句
                let held = x["held"].as_bool() == Some(true);
                let st = saved["state"].as_str().unwrap_or("");
                self.handle(j.id).update(|j| {
                    j.boost = false;
                    if let Some(t) = title {
                        j.untitled = saved["untitled"].as_bool().unwrap_or_else(|| t.contains("正在读取"));
                        j.title = t;
                    }
                    // 下载记录原样放回去（暂停的任务也保留之前的记录）
                    let saved_log: Vec<crate::model::LogLine> = saved["log"]
                        .as_array()
                        .map(|a| {
                            a.iter()
                                .map(|l| {
                                    let lv = match l[2].as_str() {
                                        Some("warn") => "warn",
                                        Some("err") => "err",
                                        _ => "",
                                    };
                                    crate::model::LogLine(l[0].as_i64().unwrap_or(0), l[1].as_str().unwrap_or("").to_string(), lv)
                                })
                                .collect()
                        })
                        .unwrap_or_default();
                    if !saved_log.is_empty() {
                        j.log = saved_log;
                    }
                    if st == "done" || st == "failed" {
                        // 结束了的任务只是展示，原样放回去
                        let s = |k: &str| saved[k].as_str().map(str::to_string);
                        j.state = if st == "done" { JobState::Done } else { JobState::Failed };
                        j.phase = serde_json::from_value(saved["phase"].clone()).unwrap_or(Phase::Done);
                        j.size = saved["size"].as_f64().unwrap_or(0.0);
                        j.got = saved["got"].as_f64().unwrap_or(0.0);
                        j.fmt = s("fmt").unwrap_or_default();
                        j.dur = s("dur").unwrap_or_default();
                        j.thumb = s("thumb");
                        j.when = s("when");
                        j.err = s("err");
                        j.audio = saved["audio"].as_bool().unwrap_or(false);
                        j.path = s("path").map(PathBuf::from);
                        j.asset_id = saved["assetId"].as_i64();
                        j.done_at = saved["doneAt"].as_i64();
                        j.fail = serde_json::from_value(saved["fail"].clone()).ok();
                        j.needs_login = j.fail.as_ref().is_some_and(|f: &crate::errors::Failure| matches!(f.code, crate::errors::ErrorCode::NeedLogin | crate::errors::ErrorCode::NeedVerify));
                        j.retry_at = saved["retryAt"].as_i64();
                        j.playlist = serde_json::from_value(saved["playlist"].clone()).ok();
                    } else if let Some(rec) = serde_json::from_value::<Rec>(saved["rec"].clone()).ok().filter(|r| r.raw.is_some()) {
                        // 退出时还在录直播（或者正在保存录到的部分）：这次不重新录，开始以后把录到的部分整理好（recover_recording）
                        let raw = rec.raw.clone().unwrap_or_default();
                        let until = last_write(&raw).unwrap_or_else(now_ms);
                        j.state = JobState::Queue;
                        j.phase = Phase::Merge;
                        j.live = true;
                        j.fmt = saved["fmt"].as_str().unwrap_or_default().to_string();
                        j.thumb = saved["thumb"].as_str().map(str::to_string);
                        j.got = saved["got"].as_f64().unwrap_or(0.0);
                        j.rec = Some(Rec { until: Some(rec.until.unwrap_or(until)), stopping: true, interrupted: true, ..rec });
                        j.log(tr!("Captae 上次退出时还在录这个直播，正在把录到的部分整理好"));
                    } else if saved["choose"].as_bool() == Some(true) {
                        // 播放列表还在等选条目：原样放回去，不重新读取
                        j.state = JobState::Pause;
                        j.phase = Phase::Probe;
                        j.choose = true;
                        j.entries = serde_json::from_value(saved["entries"].clone()).unwrap_or_default();
                        j.thumb = saved["thumb"].as_str().map(str::to_string);
                    } else if st != "pause" && saved["phase"].as_str().unwrap_or("probe") == "probe" {
                        // 退出时还在读取视频信息：重新排队，自动再读一次，不留在暂停状态。
                        // 暂停着的不算：恢复出来的暂停任务 phase 还是 probe，不看 state 的话再重开一次它就自己开始下了
                        j.state = JobState::Queue;
                        j.log(tr!("Captae 重新打开，接着读取视频信息"));
                    } else {
                        j.state = JobState::Pause;
                        // 直链下到一半的文件（aria2 直接写在保存位置）：记回来，继续时接着下，取消时才知道删哪个
                        // （已下载多少不记回来：只在存盘那一刻记过，不准）
                        j.path = saved["path"].as_str().map(PathBuf::from).filter(|p| p.exists());
                        // 已经进了素材库的（退出时正在执行预设里下载之后的步骤）也记回来：取消时靠它知道 path 是素材、不能删（cleanup_partial）
                        j.asset_id = saved["assetId"].as_i64();
                        if !held {
                            j.log(tr!("Captae 重新打开，任务已恢复，点继续接着下"));
                        }
                    }
                });
                // 记号跟着新编号走（任务号每次打开都重新排）
                if held && self.job(j.id).is_some_and(|j| j.state == JobState::Pause) {
                    self.held.lock().unwrap().insert(j.id);
                    self.restored.lock().unwrap().insert(j.id);
                }
            }
        }
    }
}

/// yt-dlp 读到的信息存在这里，下载时 `--load-info-json` 用。多视频帖子的每个条目各存一份。
pub(crate) fn info_json_path(paths: &Paths, url: &str, item: Option<u32>) -> PathBuf {
    let key = match item {
        Some(n) => crate::util::url_key(&format!("{url}#item={n}")),
        None => crate::util::url_key(url),
    };
    paths.work.join(format!("{key}.info.json"))
}

/// 「12 分 34 秒」「1 小时 5 分」「45 秒」（任务记录里写录了多久）
pub(crate) fn long_dur(s: f64) -> String {
    let s = s.max(0.0).round() as u64;
    if s >= 3600 {
        tr!("{h} 小时 {m} 分", h = s / 3600, m = s % 3600 / 60)
    } else if s >= 60 {
        tr!("{m} 分 {s} 秒", m = s / 60, s = s % 60)
    } else {
        tr!("{n} 秒", n = s)
    }
}

/// 录直播最多录多久的说法：整分钟只写分钟（「10 分钟」「3 小时」「1 小时 30 分」），不是整分钟的照 `long_dur`（「45 秒」）
pub(crate) fn limit_label(s: u64) -> String {
    if s < 60 || s % 60 != 0 {
        return long_dur(s as f64);
    }
    let m = s / 60;
    match (m / 60, m % 60) {
        (0, m) => tr!("{n} 分钟", n = m),
        (h, 0) => tr!("{n} 小时", n = h),
        (h, m) => tr!("{h} 小时 {m} 分", h = h, m = m),
    }
}

/// 文件（或目录里的文件）最后一次写入的时刻（毫秒）：录制被中断时，大概就是 Captae 退出的时候
fn last_write(p: &Path) -> Option<i64> {
    let m = std::fs::metadata(p).ok()?;
    let t = if m.is_dir() {
        std::fs::read_dir(p).ok()?.flatten().filter_map(|e| e.metadata().ok()?.modified().ok()).max()?
    } else {
        m.modified().ok()?
    };
    Some(t.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as i64)
}

/// yt-dlp 这个任务的中间文件目录整个删掉（失败了重试也接不上、或者取消了）
fn remove_tmp(j: &Job) {
    if let Some(t) = &j.tmp {
        let _ = std::fs::remove_dir_all(t);
        if let Some(parent) = t.parent() {
            let _ = std::fs::remove_dir(parent);
        }
    }
}

/// 下到的不是视频或音频（网页、HTML 当成 .mp4，FIXES 第 8 项）
fn not_media(raw: &str) -> crate::errors::Failure {
    crate::errors::Failure::new(crate::errors::ErrorCode::NoMedia, tr!("下载到的文件不是视频或音频（可能是网页），没有保存"), raw)
}

/// 下到的文件能用：视频、图片要有画面（宽、高都大于 0），声音要有音轨
fn usable(p: &crate::ffmpeg::Probe) -> bool {
    match p.kind.as_str() {
        "video" | "image" => p.width.unwrap_or(0) > 0 && p.height.unwrap_or(0) > 0,
        "audio" => p.has_audio,
        _ => false,
    }
}

/// 下完以后用实际时长核对：比网页上说的短很多，就是只给了试看、试听片段（读取时没认出来的，芒果 TV、网易云 VIP 这类）。
/// 阈值：不到 60%、而且短了 30 秒以上（试看一般只有一两成；留余量给时长写得不准的）。直播回放、录直播、自己加了参数的不核对
fn preview_after(job: &Job, p: &crate::ffmpeg::Probe) -> Option<crate::errors::Failure> {
    if job.eng != Engine::YtDlp || job.req.record_live || job.req.raw_args.as_ref().is_some_and(|a| !a.is_empty()) {
        return None;
    }
    let meta = job.meta.as_ref()?;
    if meta.live_status.as_deref().is_some_and(|s| s != "not_live") {
        return None;
    }
    let want = meta.duration.filter(|d| *d >= 60.0)?;
    let got = p.duration.filter(|d| *d > 0.0)?;
    (got < want * 0.6 && want - got >= 30.0).then(|| ytdlp::preview_failure(&job.url, Some(got), p.kind == "audio", format!("downloaded file is {got:.1}s, the page says {want:.0}s")))
}

/// 任务失败：记下错误码、一句话和原文；网站要求放慢时安排自动重试（15 分钟、45 分钟后各一次）。
pub(crate) fn fail_job(j: &mut Job, mut f: crate::errors::Failure) {
    // 说法里用的网站名字一起交给界面（「登录芒果TV」按钮和上面那句话对得上）；生成说法时没带上的按链接补
    if f.site.is_none() {
        f.site = crate::errors::site_name(&j.url).or_else(|| j.req.page_url.as_deref().and_then(crate::errors::site_name)).map(str::to_string);
    }
    let msg = f.message.clone();
    j.state = JobState::Failed;
    j.boost = false;
    j.speed = 0.0;
    j.err = Some(msg.clone());
    j.error(tr!("下载没有完成：{reason}", reason = msg));
    if !f.raw.is_empty() && f.raw != msg {
        let raw: String = f.raw.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
        j.log(tr!("原始报错：{raw}", raw = raw));
    }
    j.retry_at = None;
    // 要登录 / 要验证：之后重试时 YouTube 也带登录状态
    if matches!(f.code, crate::errors::ErrorCode::NeedLogin | crate::errors::ErrorCode::NeedVerify) {
        j.needs_login = true;
    }
    if f.code == crate::errors::ErrorCode::RateLimited && j.auto_retries < 2 {
        let mins = if j.auto_retries == 0 { 15 } else { 45 };
        j.retry_at = Some(now_ms() + mins * 60_000);
        j.log(tr!("{n} 分钟后自动重试", n = mins));
    }
    j.fail = Some(f);
}

/// 来源站点的展示名：YouTube / 哔哩哔哩 / blender.org …
pub fn site_label(url: &str, page: Option<&str>, extractor: Option<&str>) -> String {
    let pick = page.unwrap_or(url);
    if let Some(d) = detect::detect(pick) {
        if let Some(p) = d.plat {
            return match p {
                "youtube" => "YouTube",
                "bilibili" => "哔哩哔哩",
                "tiktok" => if d.host.contains("douyin") { "抖音" } else { "TikTok" },
                "x" => "X",
                "instagram" => "Instagram",
                _ => p,
            }
            .to_string();
        }
        let parts: Vec<&str> = d.host.split('.').collect();
        if d.host.parse::<std::net::IpAddr>().is_err() && parts.len() >= 2 {
            return parts[parts.len() - 2..].join(".");
        }
        return d.host;
    }
    extractor.unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sites() {
        assert_eq!(site_label("https://www.youtube.com/watch?v=1", None, None), "YouTube");
        assert_eq!(site_label("https://download.blender.org/a.mp4", None, None), "blender.org");
        assert_eq!(site_label("https://cdn.x.com/a.m3u8", Some("https://www.bilibili.com/video/1"), None), "哔哩哔哩");
    }

    struct FakeStore(Vec<login::Cookie>);
    impl login::Store for FakeStore {
        fn cookies(&self, domains: Vec<String>) -> futures_util::future::BoxFuture<'static, Result<Vec<login::Cookie>>> {
            let v: Vec<login::Cookie> = self.0.iter().filter(|c| domains.iter().any(|d| c.related_to(d))).cloned().collect();
            Box::pin(async move { Ok(v) })
        }
    }

    fn ck(name: &str, value: &str, domain: &str) -> login::Cookie {
        login::Cookie { name: name.into(), value: value.into(), domain: domain.into(), path: "/".into(), secure: true, http_only: true, host_only: false, expires: None }
    }

    /// 什么时候带登录状态（Core 这一层）、queue.json 里没有 cookie、等登录的任务、登录好以后重试
    #[tokio::test]
    async fn login_state_for_jobs() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        paths.ensure().unwrap();
        let core = Core::open(Options { paths: paths.clone(), sidecar_dir: None, restore: true }).await.unwrap();
        // 不让测试真的跑 yt-dlp（调度器拿到空档会开始任务）
        core.patch_settings(&serde_json::json!({"ytdlpPath": "/nonexistent/captae-test/yt-dlp", "aria2Path": "/nonexistent/captae-test/aria2c"})).unwrap();
        core.set_login_store(Arc::new(FakeStore(vec![ck("SESSDATA", "sess-123456", ".bilibili.com"), ck("SAPISID", "yt-123456", ".youtube.com"), ck("SID", "google-123456", ".google.com")])));
        let add = |url: &str| {
            let j = core.add(AddRequest { url: url.into(), ..Default::default() }).unwrap();
            core.act(j.id, JobAction::Pause).unwrap();
            core.job(j.id).unwrap()
        };
        let bili = add("https://www.bilibili.com/video/BV121yjzQEWf");
        assert!(core.auth_for(&bili, false).await.is_empty(), "没在 Captae 里登录过：不带");
        core.login_mark("bilibili", login::How::Login, Some(1));
        let a = core.auth_for(&bili, false).await;
        assert_eq!(a.cookies.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["SESSDATA"]);
        assert!(a.user_agent.as_deref().is_some_and(|u| u.contains("Safari/605.1.15")));
        // YouTube 登录过也不带，报过要登录 / 要验证以后才带；Google 账号本身的 cookie 不带
        let yt = add("https://www.youtube.com/watch?v=mC73amqYFpQ");
        core.login_mark("youtube", login::How::Login, Some(1));
        assert!(core.auth_for(&yt, false).await.is_empty());
        let a = core.auth_for(&yt, true).await;
        assert_eq!(a.cookies.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), vec!["SAPISID"]);
        // 第一次用 YouTube 账号：提示一次
        let mut rx = core.subscribe();
        core.note_auth(&core.handle(yt.id), &a, &yt);
        core.note_auth(&core.handle(yt.id), &a, &yt);
        let mut notices = 0;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, Event::Notice { .. }) {
                notices += 1;
            }
        }
        assert_eq!(notices, 1, "只提示一次");
        assert!(core.job(yt.id).unwrap().log.iter().any(|l| l.1.contains("带上了登录状态") && !l.1.contains("yt-123456")));
        // 插件带来的 cookies（和放在请求头里的 Cookie）：给这次下载用，不写进 queue.json
        let chrome_ua = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36";
        let ext = core
            .add(AddRequest {
                url: "https://media.example.com/v.m3u8".into(),
                cookies: vec![ck("sid", "ext-secret-1", ".example.com")],
                headers: BTreeMap::from([("Cookie".to_string(), "tok=hdr-secret-2".to_string()), ("Referer".to_string(), "https://example.com/".to_string())]),
                user_agent: Some(chrome_ua.into()),
                origin: Some("extension".into()),
                ..Default::default()
            })
            .unwrap();
        core.act(ext.id, JobAction::Pause).unwrap();
        let ej = core.job(ext.id).unwrap();
        assert!(!ej.req.headers.keys().any(|k| k.eq_ignore_ascii_case("cookie")), "Cookie 请求头不留在请求头里（会进命令行参数）");
        let a = core.auth_for(&ej, false).await;
        assert_eq!(a.cookies.len(), 2);
        assert_eq!(a.user_agent.as_deref(), Some(chrome_ua));
        // 请求头里的 Cookie 只给原来那个主机；插件给的 .example.com 的子域名都给
        assert_eq!(a.jar().header_for("https://media.example.com/seg1.ts").as_deref(), Some("sid=ext-secret-1; tok=hdr-secret-2"));
        assert_eq!(a.jar().header_for("https://cdn.example.com/seg1.ts").as_deref(), Some("sid=ext-secret-1"));
        let q = std::fs::read_to_string(paths.data.join("queue.json")).unwrap();
        assert!(q.contains("media.example.com") && !q.contains("ext-secret-1") && !q.contains("hdr-secret-2"), "{q}");
        // 任务快照（给界面、插件、AI 助手的 JSON）里也没有
        let v = serde_json::to_string(&ej).unwrap();
        assert!(!v.contains("ext-secret-1") && !v.contains("hdr-secret-2"));
        // 等登录的任务：失败了、错误码是 need_login / need_verify、属于这个网站
        core.handle(yt.id).update(|j| fail_job(j, crate::errors::classify("ERROR: [youtube] mC73amqYFpQ: Sign in to confirm your age. This video may be inappropriate for some users.", &j.url)));
        core.handle(bili.id).update(|j| fail_job(j, crate::errors::classify("ERROR: [BiliBili] 1: Unable to download webpage: HTTP Error 500", &j.url)));
        assert_eq!(core.login_waiting("youtube"), vec![yt.id]);
        assert!(core.login_waiting("bilibili").is_empty(), "不是要登录的失败不算");
        assert!(core.job(yt.id).unwrap().needs_login);
        // 登录好了：重试，之后 YouTube 也带
        assert_eq!(core.login_retry(&[yt.id]), 1);
        let j = core.job(yt.id).unwrap();
        assert!(j.state != JobState::Failed && j.needs_login);
        for id in [yt.id, bili.id, ext.id] {
            core.act(id, JobAction::Remove).unwrap();
        }
        // 退出（忘掉这个网站）以后不再带
        assert!(core.login_forget("bilibili"));
        assert!(core.auth_for(&bili, false).await.is_empty());
        assert!(login::Logins::load(&paths.logins()).get("bilibili").is_none());
    }

    /// FIXES 第 7、8 项：入库前的核对
    #[test]
    fn checks_before_ingest() {
        use crate::ffmpeg::Probe;
        // giphy-1d28e6 下到的网页 ffprobe 读不了；Commons 的 .jpg 文件页按扩展名读出 0×0
        assert!(!usable(&Probe { kind: "image".into(), width: Some(0), height: Some(0), ..Default::default() }));
        assert!(!usable(&Probe { kind: "other".into(), ..Default::default() }));
        assert!(usable(&Probe { kind: "image".into(), width: Some(800), height: Some(600), ..Default::default() }));
        assert!(usable(&Probe { kind: "video".into(), width: Some(1280), height: Some(720), duration: Some(10.0), ..Default::default() }));
        assert!(usable(&Probe { kind: "audio".into(), has_audio: true, duration: Some(30.0), ..Default::default() }));
        assert_eq!(not_media("x").code, crate::errors::ErrorCode::NoMedia);
        // mgtv-55e8de：网页说 80 分钟，下到的只有 5 分钟
        let mut j = Job { id: 1, title: "我家那闺女2026".into(), src: String::new(), url: "https://www.mgtv.com/b/899395/24812225.html".into(), plat: None, eng: Engine::YtDlp, fmt: String::new(), dur: String::new(), thumb: None, audio: false, size: 0.0, got: 0.0, speed: 0.0, state: JobState::Run, phase: Phase::Dl, pri: 0, boost: false, err: None, fail: None, choose: false, entries: vec![], playlist: None, retry_at: None, auto_retries: 0, probe_args: vec![], needs_login: false, live: false, rec: None, when: None, untitled: false, probe_at: None, from: None, done_at: None, log: vec![], segs: vec![], tries: vec![], vars: vec![], v_idx: 0, pick_until: None, merge: 0.0, path: None, asset_id: None, tmp: None, req: AddRequest::default(), meta: Some(crate::engine::MediaInfo { duration: Some(4849.0), ..Default::default() }) };
        let got = Probe { kind: "video".into(), width: Some(1280), height: Some(720), duration: Some(300.0), ..Default::default() };
        let f = preview_after(&j, &got).unwrap();
        assert_eq!((f.code, f.message.as_str()), (crate::errors::ErrorCode::NeedLogin, "这是会员内容，没登录只能下到 5 分钟的试看片段，完整版要登录开了会员的芒果TV 账号"));
        // 差不多长的不算；直播回放、录直播不核对
        assert!(preview_after(&j, &Probe { duration: Some(4700.0), ..got.clone() }).is_none());
        j.meta.as_mut().unwrap().live_status = Some("was_live".into());
        assert!(preview_after(&j, &got).is_none());
        j.meta.as_mut().unwrap().live_status = None;
        j.req.record_live = true;
        assert!(preview_after(&j, &got).is_none());
    }

    /// FIXES 第 15 项：暂停一个 yt-dlp 下载，yt-dlp 自己起的子进程（相当于合并用的 ffmpeg）也要结束
    #[tokio::test]
    async fn pause_kills_ytdlp_children() {
        use crate::tools::testkit;
        let dir = tempfile::tempdir().unwrap();
        let pidf = dir.path().join("child.pid");
        let info = r#"{"id":"x","title":"t","duration":100,"ext":"mp4","width":640,"height":360,"formats":[{"format_id":"18","url":"https://example.invalid/x.mp4","ext":"mp4","vcodec":"avc1","acodec":"mp4a","width":640,"height":360}]}"#;
        let fake = testkit::fake(
            dir.path(),
            "yt-dlp",
            &format!("for a; do if [ \"$a\" = \"-J\" ]; then echo '{info}'; exit 0; fi; done\n{}", testkit::sh_wait_child(&pidf)),
            &format!("if ($args -contains '-J') {{ Write-Output '{info}'; exit 0 }}\n{}", testkit::ps_wait_child(&pidf)),
        );
        let paths = Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        let core = Core::open(Options { paths, sidecar_dir: None, restore: false }).await.unwrap();
        core.patch_settings(&serde_json::json!({"ytdlpPath": fake.display().to_string(), "ffmpegPath": "/nonexistent/captae-test/ffmpeg", "proxyMode": "none"})).unwrap();
        let j = core.add(AddRequest { url: "https://example.com/watch/1".into(), ..Default::default() }).unwrap();
        let pids = testkit::read_pids(&pidf).await;
        core.act(j.id, JobAction::Pause).unwrap();
        for p in pids {
            assert!(testkit::gone(p).await, "暂停以后 yt-dlp 起的子进程 {p} 还在");
        }
        assert_eq!(core.job(j.id).unwrap().state, JobState::Pause);
    }

    /// FIXES 第 17 项：YouTube 下载时 403（youtube-4c4598 第 2 轮的原文），先换一组客户端重新读、再下一次，还不行才说要验证
    #[cfg(unix)]
    #[tokio::test]
    async fn youtube_403_retries_with_other_clients() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let calls = dir.path().join("calls.txt");
        let out = dir.path().join("out");
        let file = out.join("zoo.mp4");
        let fake = dir.path().join("yt-dlp");
        let info = r#"{"id":"jNQXAC9IVRw","title":"Me at the zoo","duration":19,"ext":"mp4","extractor_key":"Youtube","width":320,"height":240,"formats":[{"format_id":"18","url":"https://rr1.googlevideo.com/videoplayback","ext":"mp4","vcodec":"avc1","acodec":"mp4a","width":320,"height":240}]}"#;
        // 假的 yt-dlp：读取照常给信息；下载时没带 player_client 就报 403，带了就「下好」一个文件
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\necho \"$*\" >> '{c}'\nfor a; do if [ \"$a\" = \"-J\" ]; then echo '{info}'; exit 0; fi; done\ncase \"$*\" in\n  *player_client=*) printf x > '{f}'; echo 'CAPTAE|F|{f}'; exit 0;;\nesac\necho 'ERROR: unable to download video data: HTTP Error 403: Forbidden' >&2\nexit 1\n",
                c = calls.display(),
                f = file.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        let paths = Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        let core = Core::open(Options { paths, sidecar_dir: None, restore: false }).await.unwrap();
        core.patch_settings(&serde_json::json!({"ytdlpPath": fake.display().to_string(), "ffmpegPath": "/nonexistent/captae-test/ffmpeg", "proxyMode": "none", "ytdlpUseAria2": false})).unwrap();
        let j = core.add(AddRequest { url: "https://www.youtube.com/watch?v=jNQXAC9IVRw".into(), output_dir: Some(out.clone()), ..Default::default() }).unwrap();
        let done = tokio::time::timeout(std::time::Duration::from_secs(30), core.wait(j.id)).await.expect("任务要在 30 秒内结束").unwrap();
        assert_eq!(done.state, JobState::Done, "{:?}", done.fail);
        assert!(done.log.iter().any(|l| l.1.contains("换一组客户端")), "任务记录里要写一句换了客户端");
        let calls = std::fs::read_to_string(&calls).unwrap();
        let lines: Vec<&str> = calls.lines().collect();
        // 读取 → 下载（403）→ 换客户端重新读取 → 下载
        assert_eq!(lines.len(), 4, "{calls}");
        assert!(lines[0].contains("-J") && !lines[0].contains("player_client"));
        assert!(!lines[1].contains("-J") && !lines[1].contains("player_client"));
        assert!(lines[2].contains("-J") && lines[2].contains("player_client"));
        assert!(!lines[3].contains("-J") && lines[3].contains("player_client"));
        assert!(file.exists());
    }

    /// 点「重新打开」装更新：正在下载、排队的停下来做记号，新版打开后按记号接着下。
    /// 任务编号每次打开都重新排（Devin 的 Linux D5：caf2129 记的是旧编号，新版里对不上，任务一直暂停着）
    #[tokio::test]
    async fn held_for_update_follows_jobs_across_restart() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        paths.ensure().unwrap();
        std::fs::write(paths.settings(), br#"{"autoUpdate": false, "ytdlpPath": "/nonexistent/captae-test/yt-dlp", "aria2Path": "/nonexistent/captae-test/aria2c"}"#).unwrap();
        let saved = serde_json::json!([
            {"req": {"url": "https://example.invalid/a.mp4"}, "title": "下载中", "job": {"state": "run", "phase": "dl", "log": []}},
            {"req": {"url": "https://example.invalid/b"}, "title": "排队", "job": {"state": "pause", "phase": "probe", "log": []}},
            {"req": {"url": "https://example.invalid/c.mp4"}, "title": "用户暂停的", "job": {"state": "pause", "phase": "dl", "log": []}}
        ]);
        std::fs::write(paths.data.join("queue.json"), serde_json::to_vec(&saved).unwrap()).unwrap();
        // 调度器要等这个测试让出线程才会跑：下面一直是同步调用，任务不会真的开始
        let core = Core::open(Options { paths: paths.clone(), sidecar_dir: None, restore: true }).await.unwrap();
        let by = |c: &Core, t: &str| c.jobs().into_iter().find(|j| j.title == t).unwrap();
        core.act(by(&core, "下载中").id, JobAction::Resume).unwrap();
        core.act(by(&core, "排队").id, JobAction::Resume).unwrap();
        assert_eq!(core.hold_for_update(), 2, "下载中、排队的停下来；用户自己暂停的不算");
        for t in ["下载中", "排队", "用户暂停的"] {
            assert_eq!(by(&core, t).state, JobState::Pause);
        }
        let q: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(paths.data.join("queue.json")).unwrap()).unwrap();
        let held: Vec<&str> = q.iter().filter(|x| x["held"] == true).filter_map(|x| x["title"].as_str()).collect();
        assert_eq!(held.len(), 2);
        assert!(held.contains(&"下载中") && held.contains(&"排队"));

        // 新版打开：前面多一条完成的任务，编号全都变了
        let dir2 = tempfile::tempdir().unwrap();
        let p2 = Paths::portable(dir2.path());
        p2.ensure().unwrap();
        std::fs::copy(paths.settings(), p2.settings()).unwrap();
        let mut q2 = vec![serde_json::json!({"req": {"url": "https://example.invalid/done.mp4"}, "title": "完成的", "job": {"state": "done", "phase": "done", "log": []}})];
        q2.extend(q);
        std::fs::write(p2.data.join("queue.json"), serde_json::to_vec(&q2).unwrap()).unwrap();
        let core2 = Core::open(Options { paths: p2.clone(), sidecar_dir: None, restore: true }).await.unwrap();
        assert_ne!(by(&core2, "下载中").id, by(&core, "下载中").id, "编号换过了");
        // 记录的最后一句：带记号的还是「先停下，新版打开后接着下」（恢复时不写「点继续接着下」，接着就是「继续」，写了前后矛盾）
        let last = |c: &Core, t: &str| by(c, t).log.last().map(|l| l.1.clone()).unwrap_or_default();
        let restore_line = tr!("Captae 重新打开，任务已恢复，点继续接着下");
        let hold_line = tr!("重新打开 Captae 装更新：先停下，新版打开后接着下");
        assert_eq!(last(&core2, "下载中"), hold_line);
        assert_eq!(last(&core2, "用户暂停的"), restore_line, "没记号的照平时写");
        let resumed = core2.release_held(true);
        assert_eq!(resumed.len(), 2);
        assert_eq!(last(&core2, "下载中"), tr!("继续§任务记录"));
        assert_eq!(by(&core2, "下载中").state, JobState::Queue);
        assert_eq!(by(&core2, "排队").state, JobState::Queue);
        assert_eq!(by(&core2, "用户暂停的").state, JobState::Pause, "用户自己暂停的照旧暂停着");
        assert!(core2.release_held(true).is_empty(), "记号只用一次");
        let q3: Vec<serde_json::Value> = serde_json::from_slice(&std::fs::read(p2.data.join("queue.json")).unwrap()).unwrap();
        assert!(q3.iter().all(|x| x.get("held").is_none()), "接着下以后 queue.json 里不再有记号");

        // 没更新成（版本没变）：去掉记号，照旧暂停着
        let dir3 = tempfile::tempdir().unwrap();
        let p3 = Paths::portable(dir3.path());
        p3.ensure().unwrap();
        std::fs::copy(paths.settings(), p3.settings()).unwrap();
        std::fs::write(p3.data.join("queue.json"), serde_json::to_vec(&q2).unwrap()).unwrap();
        let core3 = Core::open(Options { paths: p3.clone(), sidecar_dir: None, restore: true }).await.unwrap();
        assert!(core3.release_held(false).is_empty());
        assert_eq!(by(&core3, "下载中").state, JobState::Pause);
        assert_eq!(last(&core3, "下载中"), restore_line, "照旧暂停着的补上「点继续接着下」");
    }

    #[tokio::test]
    async fn restore_after_quit() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(dir.path());
        paths.ensure().unwrap();
        // 上次退出时：一个还在读取信息、一个在下载、一个等选条目、一个失败了；
        // 还有一个是再上一次恢复出来的暂停任务（暂停着、phase 还是 probe）：要接着暂停，不能自己开始下（最后一轮发现的）
        let saved = serde_json::json!([
            {"req": {"url": "https://example.invalid/a"}, "title": "读取中", "job": {"state": "run", "phase": "probe", "log": []}},
            {"req": {"url": "https://example.invalid/b.mp4"}, "title": "下载中", "job": {"state": "run", "phase": "dl", "log": []}},
            {"req": {"url": "https://example.invalid/p.mp4"}, "title": "恢复过的暂停", "job": {"state": "pause", "phase": "probe", "log": []}},
            {"req": {"url": "https://x.com/a/status/1"}, "title": "两个视频", "job": {"state": "pause", "phase": "probe", "choose": true, "log": [],
                "entries": [{"id": "1", "title": "#1", "dur": 3.0, "thumb": null, "url": "https://x.com/a/status/1", "n": 1, "item": 1}]}},
            {"req": {"url": "https://vimeo.com/1"}, "title": "失败", "job": {"state": "failed", "phase": "probe", "err": "连接被你的网络拦截了，检查代理设置", "log": [],
                "fail": {"code": "blocked", "message": "连接被你的网络拦截了，检查代理设置", "raw": "certificate verify failed", "action": "retry", "next": "检查代理"}}}
        ]);
        std::fs::write(paths.data.join("queue.json"), serde_json::to_vec(&saved).unwrap()).unwrap();
        // 调度器要等这个测试让出线程才会跑，所以下面看到的是刚恢复出来的状态
        let core = Core::open(Options { paths: paths.clone(), sidecar_dir: None, restore: true }).await.unwrap();
        let jobs = core.jobs();
        let by = |t: &str| jobs.iter().find(|j| j.title == t).cloned().unwrap();
        assert_eq!(by("读取中").state, JobState::Queue, "还在读取信息的自动重新读取");
        assert_eq!(by("下载中").state, JobState::Pause, "下载中的照旧暂停，等用户点继续");
        assert_eq!(by("恢复过的暂停").state, JobState::Pause, "暂停着的接着暂停，再重开几次都一样");
        let c = by("两个视频");
        assert!(c.choose && c.state == JobState::Pause && c.entries.len() == 1);
        let f = by("失败");
        assert_eq!((f.state, f.fail.as_ref().map(|x| x.code)), (JobState::Failed, Some(crate::errors::ErrorCode::Blocked)));
        let v = serde_json::to_value(&f).unwrap();
        assert_eq!((v["err"].as_str(), v["fail"]["action"].as_str()), (Some("连接被你的网络拦截了，检查代理设置"), Some("retry")));
        drop(core);

        // 下载中退出、重新打开（变成暂停）、再取消：直链下到一半的文件和 yt-dlp 的中间目录都要删掉（集成和收尾时查出来的）
        let dir3 = tempfile::tempdir().unwrap();
        let p3 = Paths::portable(dir3.path());
        p3.ensure().unwrap();
        std::fs::write(p3.settings(), br#"{"autoUpdate": false, "ytdlpPath": "/nonexistent/captae-test/yt-dlp", "aria2Path": "/nonexistent/captae-test/aria2c"}"#).unwrap();
        let half = p3.movies.join("half.mp4");
        std::fs::create_dir_all(&p3.movies).unwrap();
        std::fs::write(&half, b"partial").unwrap();
        std::fs::write(p3.movies.join("half.mp4.aria2"), b"ctl").unwrap();
        let page = "https://example.invalid/watch/9";
        let ytmp = crate::engine::ytdlp::temp_dir(&p3.movies, page, None);
        std::fs::create_dir_all(&ytmp).unwrap();
        std::fs::write(ytmp.join("x.mp4.part"), b"partial").unwrap();
        // 下完、进了素材库、正在执行预设里后面的步骤时退出：取消只停掉预设，素材文件不能删
        let kept = p3.movies.join("kept.mp4");
        std::fs::write(&kept, b"in library").unwrap();
        let saved = serde_json::json!([
            {"req": {"url": "https://example.invalid/half.mp4"}, "title": "直链下到一半", "job": {"state": "run", "phase": "dl", "path": half, "size": 10.0, "got": 4.0, "log": []}},
            {"req": {"url": page}, "title": "网页下到一半", "job": {"state": "run", "phase": "dl", "log": []}},
            {"req": {"url": "https://example.invalid/kept.mp4"}, "title": "预设做到一半", "job": {"state": "run", "phase": "post", "path": kept, "assetId": 1, "log": []}}
        ]);
        std::fs::write(p3.data.join("queue.json"), serde_json::to_vec(&saved).unwrap()).unwrap();
        let core = Core::open(Options { paths: p3.clone(), sidecar_dir: None, restore: true }).await.unwrap();
        let by = |t: &str| core.jobs().into_iter().find(|j| j.title == t).unwrap();
        let a = by("直链下到一半");
        assert_eq!((a.state, a.path.as_deref()), (JobState::Pause, Some(half.as_path())));
        core.act(a.id, JobAction::Cancel).unwrap();
        core.act(by("网页下到一半").id, JobAction::Cancel).unwrap();
        assert!(!half.exists() && !p3.movies.join("half.mp4.aria2").exists(), "直链下到一半的文件要删掉");
        assert!(!ytmp.exists() && !p3.movies.join(".captae-tmp").exists(), "yt-dlp 的中间目录要删掉");
        let k = by("预设做到一半");
        assert_eq!(k.asset_id, Some(1), "恢复时要记得它已经进了素材库");
        core.act(k.id, JobAction::Cancel).unwrap();
        assert!(kept.exists(), "已经进了素材库的文件，取消时不能删");

        // 命令行、MCP 临时起的 Core（不恢复队列）不写 queue.json
        let dir2 = tempfile::tempdir().unwrap();
        let p2 = Paths::portable(dir2.path());
        p2.ensure().unwrap();
        std::fs::write(p2.data.join("queue.json"), b"[]").unwrap();
        let tmp = Core::open(Options { paths: p2.clone(), sidecar_dir: None, restore: false }).await.unwrap();
        tmp.add(AddRequest { url: "https://example.invalid/c.mp4".into(), ..Default::default() }).unwrap();
        assert_eq!(std::fs::read(p2.data.join("queue.json")).unwrap(), b"[]");
    }

    // ------------------------------------------------------------------ 录直播「停止并保存」（集成和收尾）

    /// 假的 yt-dlp：读取时说「正在直播」；下载时往中间目录的 live.mp4.part 一直写，记下自己的进程号。
    /// `on_int` 是收到 SIGINT 时做的事（shell 的 trap）
    #[cfg(unix)]
    fn fake_live_ytdlp(dir: &Path, on_int: &str) -> (PathBuf, PathBuf) {
        use std::os::unix::fs::PermissionsExt;
        let pidf = dir.join("ytdlp.pid");
        let fake = dir.join("yt-dlp");
        let info = r#"{"id":"live1","title":"直播测试","live_status":"is_live","is_live":true,"ext":"mp4","protocol":"m3u8_native","url":"https://example.invalid/live.m3u8","formats":[{"format_id":"hls-1","url":"https://example.invalid/live.m3u8","ext":"mp4","protocol":"m3u8_native","vcodec":"avc1","acodec":"mp4a","width":640,"height":360}]}"#;
        let script = format!(
            "#!/bin/sh\nfor a; do if [ \"$a\" = \"-J\" ]; then echo '{info}'; exit 0; fi; done\nhome=\"\"; tmp=\"\"; prev=\"\"\nfor a; do\n  if [ \"$prev\" = \"-P\" ]; then case \"$a\" in temp:*) tmp=\"${{a#temp:}}\";; *) home=\"$a\";; esac; fi\n  prev=\"$a\"\ndone\nmkdir -p \"$tmp\"\necho $$ > '{pid}'\ntrap '{on_int}' INT\nwhile :; do printf '0123456789abcdef' >> \"$tmp/live.mp4.part\"; sleep 0.1; done\n",
            pid = pidf.display()
        );
        std::fs::write(&fake, script).unwrap();
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o755)).unwrap();
        (fake, pidf)
    }

    /// 下面几个录直播的测试用（假的 yt-dlp 是 sh 脚本，靠 trap 接 SIGINT，只在 unix 上跑）
    #[cfg(unix)]
    async fn until<F: Fn() -> bool>(secs: u64, f: F) -> bool {
        for _ in 0..secs * 20 {
            if f() {
                return true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        false
    }

    /// 「停止并保存」只对正在录制的任务有效，录制中不能暂停；停下以后 yt-dlp 自己收尾（收到 SIGINT 给出整理好的文件），
    /// 照常入库，任务完成，记录里写录了多久
    #[cfg(unix)]
    #[tokio::test]
    async fn stop_and_save_recording() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let marker = dir.path().join("got-sigint");
        let on_int = format!("mv \"$tmp/live.mp4.part\" \"$home/live.mp4\"; echo int > \"{}\"; echo \"CAPTAE|F|$home/live.mp4\"; exit 0", marker.display());
        let (fake, _pidf) = fake_live_ytdlp(dir.path(), &on_int);
        let paths = Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        let core = Core::open(Options { paths, sidecar_dir: None, restore: false }).await.unwrap();
        core.patch_settings(&serde_json::json!({"ytdlpPath": fake.display().to_string(), "ffmpegPath": "/nonexistent/captae-test/ffmpeg", "proxyMode": "none", "ytdlpUseAria2": false})).unwrap();
        // 不是录直播的任务：停不了
        let plain = core.add(AddRequest { url: "https://example.invalid/plain.mp4".into(), ..Default::default() }).unwrap();
        core.act(plain.id, JobAction::Pause).unwrap();
        assert!(core.act(plain.id, JobAction::Stop).is_err(), "不是录直播的任务不能「停止并保存」");
        let j = core.add(AddRequest { url: "https://example.com/live/1".into(), record_live: true, output_dir: Some(out.clone()), ..Default::default() }).unwrap();
        assert!(until(15, || core.job(j.id).is_some_and(|x| x.recording() && x.got > 0.0)).await, "没有开始录：{:?}", core.job(j.id).map(|x| x.log));
        let rec = core.job(j.id).unwrap();
        assert!(rec.live && rec.size == 0.0 && rec.rec.as_ref().is_some_and(|r| r.since > 0 && r.raw.as_ref().is_some_and(|p| p.starts_with(&out))), "{:?}", rec.rec);
        assert!(core.act(j.id, JobAction::Pause).is_err(), "录制中不能暂停");
        assert!(core.act(j.id, JobAction::Toggle).is_err(), "录制中不能暂停（切换）");
        core.pause_all();
        assert!(core.job(j.id).unwrap().recording(), "「全部暂停」不动录直播");
        core.act(j.id, JobAction::Stop).unwrap();
        // 点了马上就是「正在保存录到的部分」，不再算在录（不能再点一次停止、也不能暂停）
        let s = core.job(j.id).unwrap();
        assert!(s.phase == Phase::Merge && s.rec.as_ref().is_some_and(|r| r.stopping && r.until.is_some()) && !s.recording(), "{:?}", s.rec);
        assert!(core.act(j.id, JobAction::Pause).is_err(), "保存中也不能暂停");
        core.act(j.id, JobAction::Stop).unwrap();
        let done = tokio::time::timeout(std::time::Duration::from_secs(30), core.wait(j.id)).await.expect("停下以后要在 30 秒内完成").unwrap();
        assert_eq!(done.state, JobState::Done, "{:?}", done.fail);
        assert!(marker.exists(), "yt-dlp 要收到 SIGINT 自己收尾");
        let path = done.path.clone().unwrap();
        assert_eq!(path, out.join("live.mp4"));
        assert!(path.exists() && std::fs::metadata(&path).unwrap().len() > 0);
        let a = core.lib.get(done.asset_id.unwrap()).unwrap().unwrap();
        assert_eq!(a.path, path, "录到的放进了素材库");
        // 最后一轮第 2 项：标成直播录像，插件再查这个直播链接时不说「下载过了」
        assert!(a.live, "录直播录下来的要标成 live");
        // 插件宿主只在桌面端（desktop feature）；手机端这套配置跑测试时跳过这一段
        #[cfg(feature = "desktop")]
        {
            let chk = crate::native_host::check(&core.lib, &core.paths, &crate::native_host::CheckReq { urls: vec!["https://example.com/live/1".into()], page_url: Some("https://example.com/live/1".into()) }).unwrap();
            assert_eq!(chk["found"], false, "录过的直播链接不算下载过了");
        }
        let r = done.rec.clone().unwrap();
        assert!(r.until.is_some_and(|u| u >= r.since) && !r.stopping && !r.interrupted);
        assert!(done.log.iter().any(|l| l.1.contains("停止录制")) && done.log.iter().any(|l| l.1.starts_with("录了")), "{:?}", done.log);
        assert!(!out.join(".captae-tmp").exists(), "中间目录删掉了");
        // 录完的任务不再算在录
        assert!(core.act(j.id, JobAction::Stop).is_err());
    }

    /// 收尾修补第 2 项：设了最多录多久（record_max_secs），录满了自动「停止并保存」，记录里写明；没设的照旧一直录
    #[cfg(unix)]
    #[tokio::test]
    async fn full_recording_stops_itself() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let on_int = "mv \"$tmp/live.mp4.part\" \"$home/live.mp4\"; echo \"CAPTAE|F|$home/live.mp4\"; exit 0";
        let (fake, _pidf) = fake_live_ytdlp(dir.path(), on_int);
        let paths = Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        let core = Core::open(Options { paths, sidecar_dir: None, restore: false }).await.unwrap();
        core.patch_settings(&serde_json::json!({"ytdlpPath": fake.display().to_string(), "ffmpegPath": "/nonexistent/captae-test/ffmpeg", "proxyMode": "none", "ytdlpUseAria2": false})).unwrap();
        let j = core.add(AddRequest { url: "https://example.com/live/2".into(), record_live: true, record_max_secs: Some(2), output_dir: Some(out.clone()), ..Default::default() }).unwrap();
        assert!(until(15, || core.job(j.id).is_some_and(|x| x.recording())).await, "没有开始录：{:?}", core.job(j.id).map(|x| x.log));
        let rec = core.job(j.id).unwrap();
        assert_eq!(rec.rec.as_ref().and_then(|r| r.max), Some(2), "界面要知道最多录多久");
        assert_eq!(serde_json::to_value(&rec).unwrap()["rec"]["max"], 2);
        assert!(rec.log.iter().any(|l| l.1.contains("最多录 2 秒")), "{:?}", rec.log);
        let done = tokio::time::timeout(std::time::Duration::from_secs(30), core.wait(j.id)).await.expect("录满以后要自己停下、在 30 秒内完成").unwrap();
        assert_eq!(done.state, JobState::Done, "{:?}", done.fail);
        let r = done.rec.clone().unwrap();
        let took = r.until.unwrap() - r.since;
        assert!(r.full && (2000..4000).contains(&took), "录满 2 秒自动停（误差一秒以内）：{took}ms {r:?}");
        assert!(done.log.iter().any(|l| l.1.contains("录满设定的 2 秒，自动停止")), "{:?}", done.log);
        assert!(done.path.as_ref().is_some_and(|p| p.exists()) && done.asset_id.is_some(), "录到的照常整理、入库");
        // AI 助手拿到的结果里写着为什么停
        let v = crate::agent::job_result(&core, &done);
        assert!(v["recording"]["note"].as_str().is_some_and(|n| n.contains("录满设定的 2 秒")), "{v}");
        assert_eq!(limit_label(600), "10 分钟");
        assert_eq!(limit_label(10800), "3 小时");
        assert_eq!(limit_label(5400), "1 小时 30 分");
        assert_eq!(limit_label(6), "6 秒");
        assert_eq!(limit_label(90), "1 分 30 秒");
    }

    /// 取消录直播 = 不要了：录制的进程结束，录到的部分删掉，不进素材库
    #[cfg(unix)]
    #[tokio::test]
    async fn cancel_recording_deletes_it() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("out");
        let (fake, pidf) = fake_live_ytdlp(dir.path(), "exit 1");
        let paths = Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        let core = Core::open(Options { paths, sidecar_dir: None, restore: false }).await.unwrap();
        core.patch_settings(&serde_json::json!({"ytdlpPath": fake.display().to_string(), "ffmpegPath": "/nonexistent/captae-test/ffmpeg", "proxyMode": "none"})).unwrap();
        let j = core.add(AddRequest { url: "https://example.com/live/2".into(), record_live: true, output_dir: Some(out.clone()), ..Default::default() }).unwrap();
        assert!(until(15, || core.job(j.id).is_some_and(|x| x.recording() && x.got > 0.0)).await);
        let pid: u32 = std::fs::read_to_string(&pidf).unwrap().trim().parse().unwrap();
        let tmp = core.job(j.id).unwrap().rec.unwrap().raw.unwrap();
        assert!(tmp.join("live.mp4.part").exists());
        core.act(j.id, JobAction::Cancel).unwrap();
        assert!(core.job(j.id).is_none());
        assert!(until(5, || !crate::tools::test_alive(pid)).await, "取消以后录制的进程 {pid} 还在");
        assert!(!tmp.exists() && !out.join(".captae-tmp").exists(), "录到的部分要删掉");
        assert_eq!(core.lib.count().unwrap(), 0, "不进素材库");
    }

    /// 用真的 ffmpeg 做几秒测试画面（没有 ffmpeg 的机器上跳过这几个测试）
    #[cfg(unix)]
    fn make_media(args: &[&str], out: &Path) -> bool {
        let Some(ff) = crate::tools::which("ffmpeg") else { return false };
        std::process::Command::new(ff).args(["-hide_banner", "-loglevel", "error", "-y"]).args(args).arg(out).status().is_ok_and(|s| s.success()) && out.exists()
    }

    #[cfg(unix)]
    fn ts_args() -> Vec<&'static str> {
        vec!["-f", "lavfi", "-i", "testsrc2=size=320x240:rate=25", "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=48000", "-t", "4", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p", "-c:a", "aac", "-f", "mpegts"]
    }

    /// yt-dlp 没收好尾（收到 SIGINT 直接退出、没给文件）：中间目录里写了一半的就是录到的部分，Captae 自己整理成 MP4，照样入库
    #[cfg(unix)]
    #[tokio::test]
    async fn recording_leftover_is_tidied() {
        let dir = tempfile::tempdir().unwrap();
        let ts = dir.path().join("src.ts");
        if !make_media(&ts_args(), &ts) || crate::tools::which("ffprobe").is_none() {
            eprintln!("没有 ffmpeg（或者不带 libx264），跳过");
            return;
        }
        let out = dir.path().join("out");
        let (fake, _) = fake_live_ytdlp(dir.path(), "exit 1");
        // 下载时先把一段真的 MPEG-TS 放进中间目录（像 ffmpeg 录了 4 秒），再接着写
        let s = std::fs::read_to_string(&fake).unwrap().replace("echo $$ >", &format!("cp '{}' \"$tmp/live.mp4.part\"; echo $$ >", ts.display()));
        let s = s.replace("printf '0123456789abcdef' >> \"$tmp/live.mp4.part\"; ", "");
        std::fs::write(&fake, s).unwrap();
        let paths = Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        let core = Core::open(Options { paths, sidecar_dir: None, restore: false }).await.unwrap();
        let ff = crate::tools::which("ffmpeg").unwrap();
        core.patch_settings(&serde_json::json!({"ytdlpPath": fake.display().to_string(), "ffmpegPath": ff.display().to_string(), "proxyMode": "none"})).unwrap();
        let j = core.add(AddRequest { url: "https://example.com/live/3".into(), record_live: true, output_dir: Some(out.clone()), ..Default::default() }).unwrap();
        assert!(until(15, || core.job(j.id).is_some_and(|x| x.recording() && x.got > 0.0)).await);
        core.act(j.id, JobAction::Stop).unwrap();
        let done = tokio::time::timeout(std::time::Duration::from_secs(60), core.wait(j.id)).await.unwrap().unwrap();
        assert_eq!(done.state, JobState::Done, "{:?} {:?}", done.fail, done.log);
        let path = done.path.clone().unwrap();
        assert_eq!(path.extension().unwrap(), "mp4");
        let secs = done.rec.as_ref().and_then(|r| r.secs).unwrap();
        assert!((secs - 4.0).abs() < 0.6, "录到的时长 {secs}");
        assert!(done.log.iter().any(|l| l.1.contains("录了 4 秒") || l.1.contains("录了 3 秒") || l.1.contains("录了 5 秒")), "{:?}", done.log);
        assert!(!out.join(".captae-tmp").exists());
    }

    /// yt-dlp 收尾以后给出的 .mp4 里其实是 MPEG-TS（画面和声音分两路录时它不再转 MP4，2026-11-24 录 YouTube 直播实测）：整理成真正的 MP4，同名
    #[cfg(unix)]
    #[tokio::test]
    async fn stopped_ts_becomes_mp4() {
        let dir = tempfile::tempdir().unwrap();
        let ts = dir.path().join("src.ts");
        if !make_media(&ts_args(), &ts) || crate::tools::which("ffprobe").is_none() {
            eprintln!("没有 ffmpeg（或者不带 libx264），跳过");
            return;
        }
        let out = dir.path().join("out");
        let on_int = format!("cp '{}' \"$home/live.mp4\"; echo \"CAPTAE|F|$home/live.mp4\"; exit 0", ts.display());
        let (fake, _) = fake_live_ytdlp(dir.path(), &on_int);
        let paths = Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        let core = Core::open(Options { paths, sidecar_dir: None, restore: false }).await.unwrap();
        let ff = crate::tools::which("ffmpeg").unwrap();
        core.patch_settings(&serde_json::json!({"ytdlpPath": fake.display().to_string(), "ffmpegPath": ff.display().to_string(), "proxyMode": "none"})).unwrap();
        let j = core.add(AddRequest { url: "https://example.com/live/4".into(), record_live: true, output_dir: Some(out.clone()), ..Default::default() }).unwrap();
        assert!(until(15, || core.job(j.id).is_some_and(|x| x.recording() && x.got > 0.0)).await);
        core.act(j.id, JobAction::Stop).unwrap();
        let done = tokio::time::timeout(std::time::Duration::from_secs(60), core.wait(j.id)).await.unwrap().unwrap();
        assert_eq!(done.state, JobState::Done, "{:?} {:?}", done.fail, done.log);
        let path = done.path.clone().unwrap();
        assert_eq!(path, out.join("live.mp4"), "同名");
        let c = crate::ffmpeg::container(&core.toolbox(), &path).await.unwrap();
        assert!(c.contains("mp4") && !c.contains("mpegts"), "{c}");
        assert!(done.rec.as_ref().and_then(|r| r.secs).is_some_and(|s| (s - 4.0).abs() < 0.6));
        assert!(std::fs::read_dir(&out).unwrap().flatten().filter(|e| e.path().is_file()).count() == 1, "只剩整理好的那一个");
    }

    /// Captae 在录直播时退出了：下次打开不重新录，把录到的部分（M3U8 引擎的分段、电台流的文件）整理好放进保存位置和素材库，
    /// 任务完成、写明录制被中断，另外提示一次放在哪
    #[cfg(unix)]
    #[tokio::test]
    async fn interrupted_recording_is_saved_next_time() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::portable(&dir.path().join("home"));
        paths.ensure().unwrap();
        let out = dir.path().join("out");
        std::fs::create_dir_all(&out).unwrap();
        let hls_raw = paths.work.join("hls-rec-test-1");
        std::fs::create_dir_all(&hls_raw).unwrap();
        let radio = out.join("radio-stream.mp3");
        let mp3 = ["-f", "lavfi", "-i", "sine=frequency=330:sample_rate=44100", "-t", "4", "-c:a", "libmp3lame", "-b:a", "128k", "-f", "mp3"];
        if !make_media(&ts_args(), &hls_raw.join("video.ts")) || !make_media(&mp3, &radio) || crate::tools::which("ffprobe").is_none() {
            eprintln!("没有 ffmpeg（或者不带 libx264 / libmp3lame），跳过");
            return;
        }
        std::fs::write(out.join("radio-stream.mp3.aria2"), b"x").unwrap();
        let ff = crate::tools::which("ffmpeg").unwrap();
        std::fs::write(paths.settings(), serde_json::to_vec(&serde_json::json!({"ffmpegPath": ff.display().to_string(), "ytdlpPath": "/nonexistent/captae-test/yt-dlp", "aria2Path": "/nonexistent/captae-test/aria2c", "autoUpdate": false, "proxyMode": "none"})).unwrap()).unwrap();
        let since = now_ms() - 60_000;
        let saved = serde_json::json!([
            {"req": {"url": "https://example.invalid/live/index.m3u8", "record_live": true}, "title": "直播 A", "job": {"state": "run", "phase": "dl", "fmt": "MP4", "log": [], "live": true,
                "rec": {"since": since, "raw": hls_raw, "out": out}}},
            {"req": {"url": "https://radio.example.invalid/stream.mp3", "record_live": true}, "title": "电台 B", "job": {"state": "run", "phase": "dl", "fmt": "MP3", "log": [], "live": true,
                "rec": {"since": since, "raw": radio, "out": out}}}
        ]);
        std::fs::write(paths.data.join("queue.json"), serde_json::to_vec(&saved).unwrap()).unwrap();
        let core = Core::open(Options { paths: paths.clone(), sidecar_dir: None, restore: true }).await.unwrap();
        let mut rx = core.subscribe();
        let by = |t: &str| core.jobs().into_iter().find(|j| j.title == t);
        assert!(until(60, || ["直播 A", "电台 B"].iter().all(|t| by(t).is_some_and(|j| !j.state.is_active()))).await, "{:?}", core.jobs().iter().map(|j| (j.title.clone(), j.state, j.log.clone())).collect::<Vec<_>>());
        for t in ["直播 A", "电台 B"] {
            let j = by(t).unwrap();
            assert_eq!(j.state, JobState::Done, "{t}: {:?} {:?}", j.fail, j.log);
            let r = j.rec.clone().unwrap();
            assert!(r.interrupted && r.until.is_some() && r.secs.is_some_and(|s| (s - 4.0).abs() < 0.6), "{t}: {r:?}");
            assert!(j.log.iter().any(|l| l.1.contains("录制被中断")), "{t}: {:?}", j.log);
            assert!(j.asset_id.is_some() && j.path.as_ref().is_some_and(|p| p.starts_with(&out) && p.exists()));
        }
        assert_eq!(by("直播 A").unwrap().path.unwrap().extension().unwrap(), "mp4");
        assert_eq!(by("电台 B").unwrap().path.unwrap(), radio, "电台流整理好以后还是原来的名字");
        assert!(!hls_raw.exists() && !out.join("radio-stream.mp3.aria2").exists() && !out.join(".captae-tmp").exists());
        assert_eq!(core.lib.count().unwrap(), 2);
        // 最后一轮第 2 项：下次打开才整理好的录像也标成直播；从它转换、剪辑出来的跟着（来源链接一样）
        let a = core.lib.get(by("直播 A").unwrap().asset_id.unwrap()).unwrap().unwrap();
        assert!(a.live && core.lib.get(by("电台 B").unwrap().asset_id.unwrap()).unwrap().unwrap().live);
        let copy = out.join("直播 A 片段.mp4");
        std::fs::copy(&a.path, &copy).unwrap();
        let cid = core.import_file(&copy, Some(a.id), None).await.unwrap();
        let c = core.lib.get(cid).unwrap().unwrap();
        assert!(c.live && c.url == a.url, "直播录像转换出来的也算直播录像：{c:?}");
        assert!(core.lib.find_by_url("https://example.invalid/live/index.m3u8", true).unwrap().is_none(), "录过的直播链接不算下载过了");
        assert!(core.lib.find_by_url("https://radio.example.invalid/stream.mp3", true).unwrap().is_none());
        let mut notices = 0;
        while let Ok(ev) = rx.try_recv() {
            if matches!(ev, Event::Notice { ref key, .. } if key.starts_with("rec_interrupted")) {
                notices += 1;
            }
        }
        assert_eq!(notices, 2, "每个被中断的录制提示一次放在哪");
    }
}
