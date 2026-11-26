//! M2 端到端实测：用 captae-remote 的 Rust 客户端遥控一个正在运行的桌面端（测试版）。步骤见 work/mobile/M2.md。
//!
//! ```text
//! remote-e2e pair  <二维码内容 captae://pair?…> <配对信息.json>      扫码配对，配对信息存进文件（0600）
//! remote-e2e run   <配对信息.json> <直链> <文件的 SHA-256> <存到哪>   加任务 → 等进度 → 暂停 → 继续 → 下完 → 查素材库 → Range 下载并核对哈希
//! remote-e2e check <配对信息.json>                                   撤销以后：请求和实时事件都应该被拒（被拒时退出码 0）
//! ```
//! 每一步打印一行 `[步骤] …`，最后打印一行 JSON 结果。

use anyhow::{anyhow, bail, Context, Result};
use captae_remote::client::{error_code, pair_qr, Client, Credentials, RemoteEvent, Subscription};
use captae_remote::proto::{code, AddJob, LibQuery, PairLink, PhoneHello};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn step(s: impl AsRef<str>) {
    println!("[{:>6.1}s] {}", START.elapsed().as_secs_f64(), s.as_ref());
}

static START: std::sync::LazyLock<Instant> = std::sync::LazyLock::new(Instant::now);

fn load(p: &str) -> Result<Credentials> {
    Ok(serde_json::from_slice(&std::fs::read(p).with_context(|| format!("读 {p}"))?)?)
}

/// 等一条满足条件的事件
async fn wait_for(sub: &mut Subscription, secs: u64, what: &str, mut f: impl FnMut(&RemoteEvent) -> bool) -> Result<RemoteEvent> {
    let r = tokio::time::timeout(Duration::from_secs(secs), async {
        while let Some(e) = sub.recv().await {
            if let RemoteEvent::Disconnected { reason, .. } = &e {
                step(format!("实时事件断开了：{reason}（会自动重连）"));
            }
            if f(&e) {
                return Some(e);
            }
        }
        None
    })
    .await;
    match r {
        Ok(Some(e)) => Ok(e),
        Ok(None) => bail!("等「{what}」时订阅结束了"),
        Err(_) => bail!("{secs} 秒内没等到「{what}」"),
    }
}

fn job_of(e: &RemoteEvent, id: u64) -> Option<&Value> {
    match e {
        RemoteEvent::Job(j) if j["id"] == id => Some(j),
        _ => None,
    }
}

async fn run(creds: Credentials, url: &str, want: &str, out: &str) -> Result<Value> {
    let c = Arc::new(Client::connect(creds).await?);
    let me = c.me().await?;
    step(format!("连上了：{}（这台手机叫 {}）", me["computer"], me["name"]));
    let mut sub = c.subscribe();
    wait_for(&mut sub, 10, "连上实时事件", |e| matches!(e, RemoteEvent::Connected)).await?;
    let snap = wait_for(&mut sub, 10, "任务快照", |e| matches!(e, RemoteEvent::Jobs(_))).await?;
    if let RemoteEvent::Jobs(v) = &snap {
        step(format!("收到任务快照：{} 个任务", v.len()));
    }

    // 1. 加任务
    let j = c.add(&AddJob { url: url.into(), ..Default::default() }).await?;
    let id = j["id"].as_u64().ok_or_else(|| anyhow!("没有任务号：{j}"))?;
    step(format!("加了任务 {id}：来源 from={}，标题「{}」", j["from"], j["title"]));
    if j["from"] != json!("iPhone") {
        bail!("任务来源不对：{}", j["from"]);
    }

    // 2. 收到进度
    let mut events = 0u32;
    let e = wait_for(&mut sub, 60, "下载进度", |e| {
        events += matches!(e, RemoteEvent::Job(_)) as u32;
        job_of(e, id).is_some_and(|j| j["state"] == "run" && j["phase"] == "dl" && j["got"].as_f64().unwrap_or(0.0) > 0.5)
    })
    .await?;
    let j = job_of(&e, id).unwrap();
    step(format!("收到进度：{:.1} / {:.1} MB，{:.1} MB/s", j["got"].as_f64().unwrap_or(0.0), j["size"].as_f64().unwrap_or(0.0), j["speed"].as_f64().unwrap_or(0.0)));

    // 3. 暂停
    c.action(id, "pause").await?;
    let e = wait_for(&mut sub, 15, "暂停", |e| job_of(e, id).is_some_and(|j| j["state"] == "pause")).await?;
    let paused_at = job_of(&e, id).unwrap()["got"].as_f64().unwrap_or(0.0);
    step(format!("已暂停，停在 {paused_at:.1} MB"));
    tokio::time::sleep(Duration::from_secs(2)).await;
    let still = c.jobs().await?.into_iter().find(|x| x["id"] == id).ok_or_else(|| anyhow!("任务没了"))?;
    step(format!("2 秒后任务列表里还是 {}（{:.1} MB）", still["state"], still["got"].as_f64().unwrap_or(0.0)));
    if still["state"] != "pause" {
        bail!("暂停以后又动了：{}", still["state"]);
    }

    // 4. 继续、下完
    c.action(id, "resume").await?;
    wait_for(&mut sub, 15, "继续", |e| job_of(e, id).is_some_and(|j| j["state"] == "run" || j["state"] == "queue")).await?;
    step("继续了");
    let t = Instant::now();
    let fin = wait_for(&mut sub, 180, "下完", |e| {
        events += matches!(e, RemoteEvent::Job(_)) as u32;
        if let Some(j) = job_of(e, id) {
            if j["state"] == "failed" {
                return true;
            }
        }
        matches!(e, RemoteEvent::Finished { id: x, .. } if *x == id)
    })
    .await?;
    let RemoteEvent::Finished { asset_id, title, .. } = fin else { bail!("下载失败了：{fin:?}") };
    let asset_id = asset_id.ok_or_else(|| anyhow!("下完了但没有进素材库"))?;
    step(format!("下完了（继续以后 {:.1}s）：「{title}」→ 素材 {asset_id}；下载过程中收到 {events} 条任务事件", t.elapsed().as_secs_f64()));
    wait_for(&mut sub, 10, "素材库变化", |e| matches!(e, RemoteEvent::Library)).await.ok();

    // 5. 素材库
    let lib = c.library(&LibQuery { limit: Some(20), ..Default::default() }).await?;
    let item = lib["items"].as_array().and_then(|v| v.iter().find(|a| a["id"] == asset_id)).cloned().ok_or_else(|| anyhow!("素材库里找不到 {asset_id}：{lib}"))?;
    step(format!("素材库：共 {} 条，找到「{}」{} · {} · 有封面={}，没有电脑上的路径={}", lib["total"], item["title"], item["fmt"], item["size"], item["hasThumb"], item.get("path").is_none()));
    let bytes = item["bytes"].as_u64().unwrap_or(0);

    // 6. Range：先取开头 1 KB 和最后 1 KB，再整个下到手机（断点续传的下载函数），核对哈希
    let (head, total) = c.file_range(asset_id, 0, 1023).await?;
    let (tail, _) = c.file_range(asset_id, total - 1024, total - 1).await?;
    step(format!("Range：开头 {} 字节、结尾 {} 字节，文件共 {total} 字节（素材库记的 {bytes}）", head.len(), tail.len()));
    let _ = std::fs::remove_file(out);
    let n = c.download_to(asset_id, std::path::Path::new(out), |_, _| {}).await?;
    let got = hex::encode(Sha256::digest(std::fs::read(out)?));
    step(format!("整个下到 {out}：{n} 字节，SHA-256 {got}"));
    let same = got == want.to_ascii_lowercase();
    step(if same { "哈希和服务器上的原文件一致".to_string() } else { format!("哈希对不上，原文件是 {want}") });
    if !same {
        bail!("哈希对不上");
    }
    if c.thumb(asset_id).await.is_ok() {
        step("封面也取到了");
    }
    Ok(json!({"ok": true, "job": id, "asset": asset_id, "bytes": n, "sha256": got, "pausedAtMb": paused_at, "jobEvents": events}))
}

async fn check(creds: Credentials) -> Result<Value> {
    let c = Arc::new(Client::new(creds)?);
    let e = c.me().await.err().ok_or_else(|| anyhow!("撤销以后请求还能用"))?;
    step(format!("GET /v1/me 被拒：{}", error_code(&e).unwrap_or("?")));
    let e2 = c.jobs().await.err().ok_or_else(|| anyhow!("撤销以后还能读任务列表"))?;
    step(format!("GET /v1/jobs 被拒：{}", error_code(&e2).unwrap_or("?")));
    let e3 = c.add(&AddJob { url: "http://127.0.0.1:9/x.mp4".into(), ..Default::default() }).await.err().ok_or_else(|| anyhow!("撤销以后还能加任务"))?;
    step(format!("POST /v1/jobs 被拒：{}", error_code(&e3).unwrap_or("?")));
    let mut sub = c.subscribe();
    let ev = wait_for(&mut sub, 15, "实时事件被拒", |e| matches!(e, RemoteEvent::Revoked | RemoteEvent::Connected)).await?;
    step(format!("实时事件：{ev:?}"));
    let ok = [&e, &e2, &e3].iter().all(|e| error_code(e) == Some(code::UNAUTHORIZED)) && ev == RemoteEvent::Revoked;
    if !ok {
        bail!("撤销以后没有全部被拒");
    }
    Ok(json!({"ok": true, "rejected": code::UNAUTHORIZED}))
}

#[tokio::main]
async fn main() {
    let _ = START.elapsed();
    let a: Vec<String> = std::env::args().skip(1).collect();
    let r = match a.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["pair", uri, file] => async {
            let link = PairLink::parse(uri).ok_or_else(|| anyhow!("不是 Captae 的二维码内容"))?;
            step(format!("二维码：电脑「{}」，地址 {:?}，端口 {}，指纹 {}", link.name, link.hosts, link.port, link.fp));
            let creds = pair_qr(&link, &PhoneHello { name: "iPhone".into(), platform: "ios".into() }).await?;
            step(format!("配对成功：设备号 {}，令牌 {} 位", creds.device, captae_remote::proto::unb64(&creds.token).map(|b| b.len() * 8).unwrap_or(0)));
            let b = serde_json::to_vec_pretty(&creds)?;
            std::fs::write(file, b)?;
            // 令牌是秘密：只有自己能读（手机上存钥匙串 / Keystore）
            #[cfg(unix)]
            std::fs::set_permissions(file, std::os::unix::fs::PermissionsExt::from_mode(0o600))?;
            Ok(json!({"ok": true, "device": creds.device}))
        }
        .await,
        ["run", file, url, sha, out] => match load(file) {
            Ok(c) => run(c, url, sha, out).await,
            Err(e) => Err(e),
        },
        ["check", file] => match load(file) {
            Ok(c) => check(c).await,
            Err(e) => Err(e),
        },
        _ => Err(anyhow!("用法见文件开头")),
    };
    match r {
        Ok(v) => println!("{v}"),
        Err(e) => {
            println!("{}", json!({"ok": false, "error": format!("{e:#}"), "code": error_code(&e)}));
            std::process::exit(1);
        }
    }
}
