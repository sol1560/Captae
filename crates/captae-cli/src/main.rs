//! `captae` 命令行。
//!
//!   captae mcp --stdio                       MCP Server，给 Claude Desktop / Cursor / Pi Agent 用
//!   captae native-host                       浏览器插件的 Native Messaging 宿主（浏览器自动启动）
//!   captae install-native-host               给 Chromium 系浏览器注册宿主（桌面端启动时也会自动注册装了的）
//!   captae probe <链接>                       读取视频信息：标题、时长、各清晰度；播放列表列出条目
//!   captae download <链接> [选项]             下载（桌面端在运行就交给它，否则在本进程里下）
//!   captae download <链接> --sample <MB>      只下开头几 MB，用 ffprobe 检查能不能读（批量测试用，不进队列、不入素材库）
//!   captae update [--force]                  检查下载引擎（yt-dlp）有没有新版，有就下载、验签、自检后换上
//!   captae engine [--health]                 下载引擎的版本、来源和自动更新状态；--health 体检正在用的新版
//!   captae query [关键词]                     检索素材库
//!   captae status                            看看桌面端和工具是否就绪
//!
//! probe、download、update、engine 加 `--json` 输出一行机器读的 JSON。
//! 退出码：0 成功；1 失败（JSON 里有错误码 `code`）；2 用法不对；3 是播放列表或多视频帖子，要用 `--items` 选条目。

use anyhow::Result;
use captae_core::api::Client;
use captae_core::errors::{self, Failure};
use captae_core::mcp::{Backend, CallFut};
use captae_core::tools::Toolbox;
use captae_core::tr;
use captae_core::{Core, Options, Paths, Settings};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

const OK: i32 = 0;
const FAILED: i32 = 1;
const USAGE: i32 = 2;
const CHOOSE: i32 = 3;

fn paths() -> Result<Paths> {
    match std::env::var_os("CAPTAE_HOME") {
        Some(h) => Ok(Paths::portable(&PathBuf::from(h))),
        None => Paths::default_for_user(),
    }
}

fn sidecar_dir() -> Option<PathBuf> {
    std::env::current_exe().ok().and_then(|p| captae_core::tools::sidecar_dir_of(&p))
}

async fn local_core(p: Paths) -> Result<Core> {
    Core::open(Options { paths: p, sidecar_dir: sidecar_dir(), restore: false }).await
}

/// 本进程用的工具和 HTTP 客户端（读同一份设置：自动更新的引擎、JS 运行时、代理）。
fn local_tools(p: &Paths) -> (Settings, Toolbox, captae_core::net::HttpClient) {
    let s = Settings::load(&p.settings());
    let tools = Toolbox::from_settings(&s, sidecar_dir(), Some(p.data.clone()));
    let http = captae_core::net::build_client(&s);
    (s, tools, http)
}

struct Remote(Client, Paths);
impl Backend for Remote {
    fn call(&self, client: String, name: String, args: Value) -> CallFut {
        let c = self.0.clone();
        Box::pin(async move { c.call(&client, &name, args).await })
    }
    fn settings(&self) -> Settings {
        Settings::load(&self.1.settings())
    }
}

struct Local(Core);
impl Backend for Local {
    fn call(&self, client: String, name: String, args: Value) -> CallFut {
        let c = self.0.clone();
        Box::pin(async move { captae_core::agent::call(&c, &client, &name, args).await })
    }
    fn settings(&self) -> Settings {
        self.0.reload_settings();
        self.0.settings()
    }
}

/// 桌面端在运行就转发给它；否则每次调用时在本进程里懒加载一个 Core。
struct Auto {
    paths: Paths,
    local: Arc<tokio::sync::OnceCell<Core>>,
}
impl Backend for Auto {
    fn call(&self, client: String, name: String, args: Value) -> CallFut {
        let paths = self.paths.clone();
        let cell = self.local.clone();
        Box::pin(async move {
            if let Some(c) = Client::connect(&paths.control()).await {
                return c.call(&client, &name, args).await;
            }
            let core = cell.get_or_try_init(|| local_core(paths.clone())).await?.clone();
            captae_core::agent::call(&core, &client, &name, args).await
        })
    }
    fn settings(&self) -> Settings {
        Settings::load(&self.paths.settings())
    }
}

// ------------------------------------------------------------------ 参数

/// 用法不对（退出码 2）。
#[derive(Debug)]
struct Usage(String);
impl std::fmt::Display for Usage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Usage {}

fn usage(s: impl Into<String>) -> anyhow::Error {
    anyhow::Error::new(Usage(s.into()))
}

fn arg_val(args: &[String], k: &str) -> Option<String> {
    args.iter().position(|a| a == k).and_then(|i| args.get(i + 1).cloned())
}

fn has(args: &[String], k: &str) -> bool {
    args.iter().any(|a| a == k)
}

/// `--header 'Referer: https://…'`，可以给多个。
fn headers(args: &[String]) -> Result<BTreeMap<String, String>> {
    let mut h = BTreeMap::new();
    for (i, a) in args.iter().enumerate() {
        if a == "--header" {
            let v = args.get(i + 1).ok_or_else(|| usage(tr!("--header 后面要写「名字: 值」")))?;
            let (k, v) = v.split_once(':').ok_or_else(|| usage(tr!("--header 的格式是「名字: 值」")))?;
            h.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    Ok(h)
}

/// 第一个参数后面的链接；不像链接就算用法不对。
fn url_arg(args: &[String], help: &str) -> Result<String> {
    let u = args.get(1).filter(|a| !a.starts_with("--")).ok_or_else(|| usage(help))?;
    if captae_core::detect::detect(u).is_none() {
        return Err(usage(format!("{}\n{help}", tr!("「{url}」不像一个完整的链接（要以 http:// 或 https:// 开头）", url = u))));
    }
    Ok(u.clone())
}

fn print_json(v: &Value) {
    println!("{}", serde_json::to_string(v).unwrap_or_default());
}

/// `{"ok": …, "url": …, "ms": …}` 后面接上 `rest` 里的字段。
fn with_head(ok: bool, url: &str, ms: u128, rest: Value) -> Value {
    let mut m = serde_json::Map::new();
    m.insert("ok".into(), json!(ok));
    m.insert("url".into(), json!(url));
    m.insert("ms".into(), json!(ms));
    if let Value::Object(o) = rest {
        m.extend(o);
    }
    Value::Object(m)
}

fn fail_json(url: &str, ms: u128, f: &Failure) -> Value {
    with_head(false, url, ms, json!({"code": f.code, "fail": f.to_json()}))
}

fn print_fail(f: &Failure) {
    eprintln!("{}", tr!("失败（{code}）：{message}", code = f.code, message = f.message));
    if !f.raw.is_empty() && f.raw != f.message {
        eprintln!("{}", tr!("原始报错：\n{raw}", raw = f.raw));
    }
    eprintln!("{}", tr!("下一步：{next}", next = f.code.cli_hint()));
}

fn secs(d: Option<f64>) -> String {
    d.map(captae_core::util::clock).filter(|s| !s.is_empty()).unwrap_or_else(|| "?".into())
}

fn mb(b: Option<u64>) -> String {
    b.map(|b| format!("{} MB", captae_core::i18n::decimal(b as f64 / 1e6, 1))).unwrap_or_else(|| tr!("大小未知"))
}

/// 设置项开着还是关着（engine、status 的输出）
fn on_off(on: bool) -> String {
    if on {
        tr!("开§命令行输出里设置项的状态")
    } else {
        tr!("关§命令行输出里设置项的状态")
    }
}

// ------------------------------------------------------------------ 命令

fn help_probe() -> String {
    tr!("用法：captae probe <链接> [--json] [--header '名字: 值']")
}

fn help_download() -> String {
    tr!("用法：captae download <链接> [--audio] [--format MP4] [--resolution 1080p] [--preset 名字] [--out 目录] [--items 1,3|all] [--json]\n      captae download <链接> --record-live [--max-minutes N] [--out 目录] [--json]\n      captae download <链接> --sample <MB> [--out 目录] [--header '名字: 值'] [--json]\n\n--record-live：正在直播时从现在开始录。--max-minutes N：最多录 N 分钟（可以写小数），录满自动停止并保存；\n不写就一直录，按一次 Ctrl-C 停止并保存（录到的部分整理成能播放的文件，放进素材库），再按一次直接退出")
}

async fn probe(args: &[String]) -> Result<i32> {
    let url = url_arg(args, &help_probe())?;
    let hs = headers(args)?;
    let json = has(args, "--json");
    let p = paths()?;
    p.ensure()?;
    let (_, tools, http) = local_tools(&p);
    let t0 = Instant::now();
    let r = captae_core::probe::probe(&tools, &http, &url, &hs).await;
    let ms = t0.elapsed().as_millis();
    let info = match r {
        Ok(i) => i,
        Err(e) => {
            let f = errors::from_anyhow(&e, &url);
            if json {
                print_json(&fail_json(&url, ms, &f));
            } else {
                print_fail(&f);
            }
            return Ok(FAILED);
        }
    };
    if json {
        print_json(&with_head(true, &url, ms, serde_json::to_value(&info)?));
        return Ok(OK);
    }
    println!("{}", info.title.clone().unwrap_or_else(|| tr!("（没有标题）")));
    if info.choose {
        println!("{}", tr!("这里有 {n} 个视频：", n = info.entries.len()));
        for e in &info.entries {
            println!("  {:>3}. {}  {}", e.n, e.title, secs(e.dur));
        }
        println!("{}", tr!("下载其中几个：captae download {url} --items 1,2（或 --items all）", url = url));
        return Ok(OK);
    }
    println!("{}", tr!("类型：{kind}  时长：{dur}  来源：{site}", kind = info.kind, dur = secs(info.duration), site = info.site.as_deref().unwrap_or("-")));
    for q in &info.qualities {
        println!("  {:<10} {}", q.label, mb(q.bytes));
    }
    if let Some(s) = info.size {
        println!("  {}", tr!("文件大小 {size}", size = mb(Some(s))));
    }
    if let Some(pl) = &info.playlist {
        println!("{}", tr!("这是播放列表里的一个视频（整个列表：{url}）", url = pl.url));
    }
    eprintln!("{}", tr!("用了 {sec} 秒", sec = captae_core::i18n::decimal(ms as f64 / 1000.0, 1)));
    Ok(OK)
}

async fn sample(args: &[String], url: String, mb_s: String) -> Result<i32> {
    let mbs: f64 = mb_s.parse().ok().filter(|x: &f64| x.is_finite() && *x > 0.0).ok_or_else(|| usage(tr!("--sample 后面写要下多少 MB，比如 --sample 5")))?;
    let hs = headers(args)?;
    let json = has(args, "--json");
    let p = paths()?;
    p.ensure()?;
    let (_, tools, http) = local_tools(&p);
    let dir = arg_val(args, "--out").map(PathBuf::from).unwrap_or_else(|| p.data.join("samples"));
    let o = captae_core::sample::Opts { url: url.clone(), mb: mbs, dir, headers: hs };
    let t0 = Instant::now();
    let r = captae_core::sample::run(&tools, &http, &o).await;
    let ms = t0.elapsed().as_millis();
    match r {
        Ok(s) => {
            if json {
                print_json(&with_head(true, &url, ms, serde_json::to_value(&s)?));
            } else {
                for n in &s.notes {
                    eprintln!("· {n}");
                }
                let m = &s.media;
                println!("{}", s.path.display());
                println!(
                    "{}",
                    tr!(
                        "{bytes} 字节（{method}），ffprobe：{kind} {format} {dur} {size}{codecs}，用了 {sec} 秒",
                        bytes = s.bytes,
                        method = s.method,
                        kind = m.kind,
                        format = m.format,
                        dur = secs(m.duration),
                        size = m.width.zip(m.height).map(|(w, h)| format!("{w}×{h} ")).unwrap_or_default(),
                        codecs = [m.vcodec.clone(), m.acodec.clone()].into_iter().flatten().collect::<Vec<_>>().join("/"),
                        sec = captae_core::i18n::decimal(ms as f64 / 1000.0, 1)
                    )
                );
            }
            Ok(OK)
        }
        Err(f) => {
            if json {
                print_json(&fail_json(&url, ms, &f));
            } else {
                print_fail(&f);
            }
            Ok(FAILED)
        }
    }
}

async fn download(args: &[String]) -> Result<i32> {
    let url = url_arg(args, &help_download())?;
    if has(args, "--sample") {
        let v = arg_val(args, "--sample").ok_or_else(|| usage(tr!("--sample 后面写要下多少 MB，比如 --sample 5")))?;
        return sample(args, url, v).await;
    }
    let json = has(args, "--json");
    let live = has(args, "--record-live");
    let mut a = json!({"url": url, "audio_only": has(args, "--audio"), "record_live": live});
    for (flag, key) in [("--format", "format"), ("--resolution", "resolution"), ("--preset", "preset"), ("--out", "output_dir"), ("--items", "items")] {
        if let Some(v) = arg_val(args, flag) {
            a[key] = json!(v);
        }
    }
    let mut max: Option<f64> = None;
    // 最多录几分钟：不写就一直录到 Ctrl-C（命令行不套用给 AI 助手的 10 分钟默认值和 180 分钟上限，见 agent::live_limit）
    if has(args, "--max-minutes") {
        if !live {
            return Err(usage(tr!("--max-minutes 要和 --record-live 一起用（录直播时最多录几分钟）")));
        }
        let v = arg_val(args, "--max-minutes").ok_or_else(|| usage(tr!("--max-minutes 后面写分钟数，比如 --max-minutes 30")))?;
        let m: f64 = v.trim().parse().ok().filter(|m: &f64| m.is_finite() && *m > 0.0).ok_or_else(|| usage(tr!("--max-minutes 后面写大于 0 的分钟数，比如 --max-minutes 30（可以写小数）")))?;
        a["max_minutes"] = json!(m);
        max = Some(m);
    }
    let paths = paths()?;
    let b = Auto { paths: paths.clone(), local: Default::default() };
    if live {
        // 录制中按 Ctrl-C：先让录制「停止并保存」（见 on_signals）
        let before = match Client::connect(&paths.control()).await {
            Some(c) => c.jobs().await.ok().and_then(|v| v.as_array().map(|a| a.iter().filter_map(|j| j["id"].as_u64()).collect())).unwrap_or_default(),
            None => vec![],
        };
        let _ = SAVER.set(Saver { paths: paths.clone(), local: b.local.clone(), before, url: captae_core::detect::canonical(&url) });
    }
    if !json {
        eprintln!(
            "{}",
            match (live, max) {
                (true, Some(m)) => tr!("正在录直播，最多录 {n} 分钟，录满自动停止并保存（按 Ctrl-C 提前停止并保存）…", n = if m.fract() == 0.0 { format!("{m:.0}") } else { format!("{m}") }),
                (true, None) => tr!("正在录直播，按 Ctrl-C 停止并保存…"),
                _ => tr!("正在下载…"),
            }
        );
    }
    let t0 = Instant::now();
    let r = b.call(captae_core::agent::CLI_CLIENT.into(), "download".into(), a).await;
    let ms = t0.elapsed().as_millis();
    // 录直播：录到的文件在哪（JSON 里也有 path，这里给人看）
    if let (true, false, Ok(v)) = (live, json, &r) {
        if let Some(p) = v["path"].as_str() {
            let rec = &v["recording"];
            // 录满设定的时长、自动停的：看 full（固定字段）。没有这一项的（旧版桌面端）照旧看说法是不是「录满」开头：说法跟着界面语言变
            let full = rec["full"].as_bool().unwrap_or_else(|| rec["note"].as_str().is_some_and(|n| n.starts_with("录满")));
            let how = if CTRL_C_SAVED.load(std::sync::atomic::Ordering::SeqCst) {
                tr!("按了 Ctrl-C 停止")
            } else if full {
                rec["note"].as_str().unwrap_or_default().to_string()
            } else {
                tr!("直播结束了，或者在 Captae 里点了「停止并保存」")
            };
            let done = match rec["seconds"].as_f64() {
                Some(s) => tr!("录到的部分已经整理好，放进了素材库：{path}（{how}，文件 {len}）", path = p, how = how, len = captae_core::util::clock(s)),
                None => tr!("录到的部分已经整理好，放进了素材库：{path}（{how}）", path = p, how = how),
            };
            eprintln!("{done}");
        }
    }
    match r {
        Ok(v) if v["status"] == "choose" => {
            if json {
                print_json(&with_head(false, &url, ms, v));
            } else {
                println!("{}", v["message"].as_str().unwrap_or(""));
                for e in v["entries"].as_array().into_iter().flatten() {
                    println!("  {:>3}. {}  {}", e["n"], e["title"].as_str().unwrap_or(""), secs(e["dur"].as_f64()));
                }
                println!("{}", tr!("再运行一次并加上 --items，比如：captae download {url} --items 1,2（或 --items all）", url = url));
            }
            Ok(CHOOSE)
        }
        Ok(v) => {
            let failed = v["failed"].as_u64().unwrap_or(0);
            if json {
                print_json(&with_head(failed == 0, &url, ms, v));
            } else {
                println!("{}", serde_json::to_string_pretty(&v)?);
            }
            Ok(if failed == 0 { OK } else { FAILED })
        }
        Err(e) => {
            let v = captae_core::agent::error_json(&e);
            if json {
                print_json(&with_head(false, &url, ms, v));
            } else {
                match errors::failure_in(&e) {
                    Some(f) => print_fail(f),
                    None => eprintln!("{}", tr!("下载失败：{err}", err = format!("{e:#}"))),
                }
            }
            Ok(FAILED)
        }
    }
}

async fn update(args: &[String]) -> Result<i32> {
    use captae_core::updater::{check, How};
    let p = paths()?;
    p.ensure()?;
    let (_, tools, http) = local_tools(&p);
    let how = if has(args, "--force") { How::Reinstall } else { How::Manual };
    if !has(args, "--json") {
        eprintln!("{}", tr!("正在检查下载引擎（yt-dlp）的新版…"));
    }
    let r = check(&tools, &http, &p.data, how).await;
    if has(args, "--json") {
        print_json(&serde_json::to_value(&r)?);
    } else {
        for s in &r.steps {
            eprintln!("· {s}");
        }
        println!("{}", r.message);
    }
    Ok(if matches!(r.result, "updated" | "latest" | "skipped") { OK } else { FAILED })
}

async fn engine(args: &[String]) -> Result<i32> {
    let p = paths()?;
    p.ensure()?;
    let (s, tools, _) = local_tools(&p);
    if has(args, "--health") {
        let back = captae_core::updater::health_check(&tools, &p.data).await;
        eprintln!("{}", if back { tr!("正在用的新版有问题，已经退回上一个版本") } else { tr!("体检完成，不用退回") });
    }
    let v = captae_core::updater::status(&tools, &p.data, s.auto_update).await;
    if has(args, "--json") {
        print_json(&v);
        return Ok(OK);
    }
    let st = |k: &str| v[k].as_str().map(str::to_string).unwrap_or_else(|| "-".into());
    println!("{}", tr!("yt-dlp {version}（{source}）  {path}", version = st("version"), source = st("source"), path = st("path")));
    println!(
        "{}",
        tr!(
            "安装包自带：{bundled}  自动更新：{auto}  正在用的自动更新版本：{active}  上一个：{previous}",
            bundled = st("bundled"),
            auto = on_off(s.auto_update),
            active = st("active"),
            previous = st("previous")
        )
    );
    println!("{}", tr!("上次检查：{when}", when = v["lastCheck"].as_i64().map(captae_core::util::when).unwrap_or_else(|| tr!("还没检查过"))));
    if let Some(r) = v["lastResult"].as_str() {
        println!("{}", tr!("结果：{result}", result = r));
    }
    if let Some(js) = v["jsRuntime"].as_object() {
        println!("{}", tr!("JS 运行时：{kind} {path}（{source}）", kind = js["kind"].as_str().unwrap_or(""), path = js["path"].as_str().unwrap_or(""), source = js["source"].as_str().unwrap_or("")));
    } else {
        println!("{}", tr!("JS 运行时：没有找到（YouTube 可能读不到）"));
    }
    Ok(OK)
}

async fn run(args: &[String]) -> Result<i32> {
    let cmd = args.first().map(String::as_str).unwrap_or("help");
    match cmd {
        "mcp" => {
            let p = paths()?;
            p.ensure()?;
            // 数据目录：MCP 进程在这里盯 settings.json、写连接状态
            let data = p.data.clone();
            let backend: Arc<dyn Backend> = if has(args, "--local") {
                Arc::new(Local(local_core(p).await?))
            } else if let Some(c) = Client::connect(&p.control()).await.filter(|_| has(args, "--remote")) {
                Arc::new(Remote(c, p))
            } else {
                Arc::new(Auto { paths: p, local: Default::default() })
            };
            captae_core::mcp::serve_stdio(backend, Some(data)).await?;
            Ok(OK)
        }
        // Windows 上浏览器直接启动 captae.exe 当宿主，第一个参数是 chrome-extension://<插件 id>/（见 native_host::Install::wrapper）
        "native-host" => {
            captae_core::native_host::run(paths()?, sidecar_dir()).await?;
            Ok(OK)
        }
        origin if captae_core::native_host::is_browser_origin(origin) => {
            captae_core::native_host::run(paths()?, sidecar_dir()).await?;
            Ok(OK)
        }
        "install-native-host" => {
            let o = captae_core::native_host::Install {
                ext_id: arg_val(args, "--extension-id"),
                // 命令行是手动注册：所有浏览器都写上，之后再装的浏览器也能直接用
                all: true,
                ..captae_core::native_host::Install::new(std::env::current_exe()?, paths()?.data)
            };
            for f in captae_core::native_host::install(&o)? {
                println!("{}", tr!("已写入 {path}", path = f.display()));
            }
            Ok(OK)
        }
        "probe" => probe(args).await,
        "download" => download(args).await,
        "update" => update(args).await,
        "engine" => engine(args).await,
        "query" => {
            let b = Auto { paths: paths()?, local: Default::default() };
            let r = b.call(captae_core::agent::CLI_CLIENT.into(), "library_query".into(), json!({"q": args.get(1), "limit": 50})).await?;
            for it in r["items"].as_array().cloned().unwrap_or_default() {
                // 来源是存进素材库的名字（「本地」、站点名），显示时翻译
                let src = captae_core::i18n::dynamic(it["source"].as_str().unwrap_or(""));
                println!("{:>5}  {:<6} {:<10} {}", it["asset_id"], it["type"].as_str().unwrap_or(""), src, it["title"].as_str().unwrap_or(""));
            }
            eprintln!("{}", tr!("共 {n} 项", n = r["count"]));
            Ok(OK)
        }
        "status" => {
            let p = paths()?;
            let app = Client::connect(&p.control()).await.is_some();
            println!("{}", tr!("桌面端：{state}", state = if app { tr!("运行中§命令行 status 里桌面端的状态") } else { tr!("没有运行§命令行 status 里桌面端的状态") }));
            println!("{}", tr!("数据目录：{path}", path = p.data.display()));
            let (s, tb, _) = local_tools(&p);
            println!("{}", tr!("AI 助手访问：{mcp}  高级模式：{adv}  自动更新下载引擎：{auto}", mcp = on_off(s.mcp_on), adv = on_off(s.adv), auto = on_off(s.auto_update)));
            for (n, v) in tb.versions().await {
                println!("{n:<8} {}", v.unwrap_or_else(|| tr!("未找到§命令行 status 里没装的工具")));
            }
            println!("{}", tr!("yt-dlp 来自：{source}", source = tb.ytdlp_source()));
            match tb.js_runtime() {
                Some(js) => println!("{}", tr!("JS 运行时：{kind} {path}（{source}）", kind = js.kind, path = js.path.display(), source = js.source)),
                None => println!("{}", tr!("JS 运行时：没有找到（YouTube 可能读不到）")),
            }
            let px = captae_core::net::status(&s);
            let direct = tr!("直连§命令行 status 里没有用代理");
            let via = px["https"].as_str().or(px["http"].as_str()).unwrap_or(&direct);
            println!("{}", tr!("代理：{via}（{mode}）", via = via, mode = px["mode"].as_str().unwrap_or("")));
            Ok(OK)
        }
        "help" | "--help" | "-h" => {
            println!("{}", tr!("Captae {version}\n\n  captae mcp --stdio\n  captae native-host\n  captae install-native-host [--extension-id ID]\n  captae probe <链接> [--json]\n  captae download <链接> [--audio] [--format F] [--resolution 1080p] [--preset 名字] [--out 目录] [--items 1,3|all] [--json]\n  captae download <链接> --record-live [--max-minutes N] [--json]   录直播：录满 N 分钟自动停止并保存；不写就按 Ctrl-C 停止并保存，再按一次直接退出\n  captae download <链接> --sample <MB> [--json]\n  captae update [--force] [--json]\n  captae engine [--health] [--json]\n  captae query [关键词]\n  captae status\n\n退出码：0 成功，1 失败，2 用法不对，3 要选条目（播放列表、多视频帖子）", version = env!("CARGO_PKG_VERSION")));
            Ok(OK)
        }
        other => Err(usage(tr!("不认识的命令「{cmd}」，运行 captae help 看用法", cmd = other))),
    }
}

/// `download --record-live` 正在录制时，第一次 Ctrl-C 交给它：让这次的录制「停止并保存」（录到的部分整理好、放进素材库），
/// download 照常返回、打印文件在哪以后正常退出。录制可能在本进程自己起的 Core 里，也可能交给了开着的桌面端
struct Saver {
    paths: Paths,
    /// 本进程自己起的 Core（桌面端没开时 Auto 懒加载的那个）
    local: Arc<tokio::sync::OnceCell<Core>>,
    /// 交给桌面端之前桌面端已经有的任务：按 Ctrl-C 时只停这次新加的
    before: Vec<u64>,
    /// 这次要录的链接（理顺过的，和任务里的 url 一样）：认桌面端里哪个任务是这次加的
    url: String,
}

static SAVER: std::sync::OnceLock<Saver> = std::sync::OnceLock::new();
/// 这次的录制是按 Ctrl-C 停下的（最后那句「按了 Ctrl-C 停止」）
static CTRL_C_SAVED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

impl Saver {
    /// 有正在录制的就让它停止并保存，返回 true；还没开始录（在读取信息）、已经在保存了，返回 false（照旧直接退出）
    async fn save(&self) -> bool {
        let any = self.stop_recording().await;
        if any {
            CTRL_C_SAVED.store(true, std::sync::atomic::Ordering::SeqCst);
        }
        any
    }

    /// 第二次 Ctrl-C 直接退出时：本进程自己录的、还没整理的原始数据在哪（交给桌面端的由桌面端接着保存）
    fn leftovers(&self) -> Vec<PathBuf> {
        let Some(core) = self.local.get() else { return vec![] };
        core.jobs().into_iter().filter(|j| j.state != captae_core::model::JobState::Done).filter_map(|j| j.rec.and_then(|r| r.raw)).filter(|p| p.exists()).collect()
    }

    /// 没整理就退出了：原始数据留在原处，保留 7 天，之后 Captae 或命令行启动时自动删掉（recordings.rs）。
    /// 保存位置里 yt-dlp 的中间目录要先记下来，启动时才找得到；电台流是 aria2 直接写在保存位置的文件，本身能播放，不删
    fn left_behind(&self, left: &[PathBuf]) {
        use captae_core::recordings::{remember, swept, KEEP};
        remember(&self.paths, left);
        for p in left {
            if swept(&self.paths, p) {
                // 保留几天按 KEEP 算（7 天），英文等要分单复数
                eprintln!("\n{}", tr!("没整理完就退出了，录到的原始数据留在：{path}，{n} 天后会自动删掉（想留着就先挪到别处）", path = p.display(), n = KEEP.as_secs() / 86_400));
            } else {
                // aria2 的控制文件没用了（录制的 aria2c 已经结束）
                let _ = std::fs::remove_file(format!("{}.aria2", p.display()));
                eprintln!("\n{}", tr!("没整理完就退出了，录到的部分留在：{path}（没有整理过，有的播放器拖不了进度条；不要了可以直接删掉）", path = p.display()));
            }
        }
    }

    async fn stop_recording(&self) -> bool {
        use captae_core::model::JobAction;
        if let Some(core) = self.local.get() {
            let ids: Vec<u64> = core.jobs().into_iter().filter(|j| j.recording()).map(|j| j.id).collect();
            return ids.into_iter().fold(false, |any, id| core.act(id, JobAction::Stop).is_ok() || any);
        }
        let Some(c) = Client::connect(&self.paths.control()).await else { return false };
        let Ok(all) = c.jobs().await else { return false };
        let mine = all.as_array().into_iter().flatten().filter(|j| {
            let id = j["id"].as_u64().unwrap_or(0);
            // 这次命令行加的：链接就是这次要录的（固定字段）。任务记录第一行「来自 AI 助手（命令行）」跟着界面语言变，只当旧版桌面端的退路
            let from_cli = j["url"].as_str() == Some(self.url.as_str())
                || j["log"][0][1].as_str().is_some_and(|l| l.starts_with(&format!("来自 AI 助手（{}）", captae_core::agent::CLI_CLIENT)));
            let r = &j["rec"];
            let recording = j["state"] == "run" && j["phase"] == "dl" && r.is_object() && r["until"].is_null() && r["stopping"].as_bool() != Some(true);
            !self.before.contains(&id) && from_cli && recording
        });
        let mut any = false;
        for j in mine {
            if let Some(id) = j["id"].as_u64() {
                any |= c.job_action(id, JobAction::Stop).await.is_ok();
            }
        }
        any
    }
}

/// 被 SIGTERM / SIGINT / SIGHUP 结束时（终端 Ctrl-C、`kill`、浏览器关掉插件宿主）：先把正在跑的 yt-dlp、ffmpeg、aria2c
/// 连同它们的子进程结束再退出。进程直接退出时下载的 future 不会被丢掉，不这样做它们会留成孤儿（第三阶段 FIXES 第 15 项）。
/// 例外：`download --record-live` 正在录制时，第一次 Ctrl-C 是「停止并保存」（`Saver`），第二次才直接退出；SIGTERM、SIGHUP 照旧。
/// 直接退出时本进程录到一半、没整理的原始数据留在原处，保留 7 天（`Saver::left_behind`）
#[cfg(unix)]
fn on_signals() {
    use tokio::signal::unix::{signal, SignalKind};
    for (kind, code) in [(SignalKind::terminate(), 143), (SignalKind::interrupt(), 130), (SignalKind::hangup(), 129)] {
        if let Ok(mut s) = signal(kind) {
            let ctrl_c = code == 130;
            tokio::spawn(async move {
                let mut saving = false;
                while s.recv().await.is_some() {
                    on_stop(ctrl_c, &mut saving, code).await;
                }
            });
        }
    }
}

/// Windows：Ctrl-C 和 unix 的 SIGINT 一样（录直播时第一次停止并保存、第二次直接退出）；Ctrl-Break、关掉控制台窗口、
/// 注销、关机和 SIGTERM 一样直接收拾好退出。关控制台时系统只给 5 秒左右，结束进程树（Job）是一下子的事，来得及。
/// 外部工具各有自己看不见的控制台（CREATE_NO_WINDOW），收不到这里的 Ctrl-C，全靠这里转告
#[cfg(windows)]
fn on_signals() {
    use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close, ctrl_logoff, ctrl_shutdown};
    if let Ok(mut s) = ctrl_c() {
        tokio::spawn(async move {
            let mut saving = false;
            while s.recv().await.is_some() {
                on_stop(true, &mut saving, 130).await;
            }
        });
    }
    macro_rules! quit_on {
        ($make:expr, $code:expr) => {
            if let Ok(mut s) = $make {
                tokio::spawn(async move {
                    let mut saving = false;
                    while s.recv().await.is_some() {
                        on_stop(false, &mut saving, $code).await;
                    }
                });
            }
        };
    }
    quit_on!(ctrl_break(), 131);
    quit_on!(ctrl_close(), 129);
    quit_on!(ctrl_logoff(), 143);
    quit_on!(ctrl_shutdown(), 143);
}

#[cfg(not(any(unix, windows)))]
fn on_signals() {}

/// 收到一次结束信号。`ctrl_c`：是 Ctrl-C（录直播时第一次是停止并保存，这时返回、接着等）；别的情况收拾好退出
async fn on_stop(ctrl_c: bool, saving: &mut bool, code: i32) {
    if ctrl_c && !*saving {
        if let Some(sv) = SAVER.get() {
            if sv.save().await {
                *saving = true;
                eprintln!("\n{}", tr!("正在保存录到的部分，再按一次 Ctrl-C 直接退出"));
                return;
            }
        }
    }
    // 本进程自己录的、还没整理的原始数据（第二次 Ctrl-C，或者录制中被 kill、终端关了）：留在原处，记下来、说清楚
    let left = SAVER.get().map(Saver::leftovers).unwrap_or_default();
    captae_core::tools::kill_all();
    if let Some(sv) = SAVER.get() {
        sv.left_behind(&left);
    }
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Windows 上「停止并保存」的辅助进程：`captae __ctrl-c <pid>` 往那个进程的控制台发一次 Ctrl-C 就退出（见 tools::interrupt）。
    // 要在接 Ctrl-C 之前处理：它自己也会连到那个控制台上
    if args.first().map(String::as_str) == Some(captae_core::tools::CTRL_C_ARG) {
        let ok = args.get(1).and_then(|p| p.parse().ok()).is_some_and(captae_core::tools::send_ctrl_c);
        std::process::exit(if ok { 0 } else { 1 });
    }
    // MCP 和 native-host 的 stdout 是协议通道，日志只能写 stderr
    tracing_subscriber::fmt().with_writer(std::io::stderr).with_env_filter(tracing_subscriber::EnvFilter::from_env("CAPTAE_LOG")).init();
    on_signals();
    // 界面语言：按设置（空 = 跟随系统），提示、报错、帮助都用它；数据目录读不到时跟随系统
    captae_core::i18n::apply(&paths().map(|p| captae_core::Settings::load(&p.settings()).language).unwrap_or_default());
    let code = match run(&args).await {
        Ok(c) => c,
        Err(e) if e.downcast_ref::<Usage>().is_some() => {
            eprintln!("{e}");
            USAGE
        }
        Err(e) => {
            eprintln!("{e:#}");
            FAILED
        }
    };
    // MCP 的客户端断开、插件宿主被浏览器关掉时 run 也会返回，这时可能还有下载在跑：
    // process::exit 不会丢掉那些 future，先把还在跑的 yt-dlp、ffmpeg、aria2c 连同子进程结束
    captae_core::tools::kill_all();
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}
