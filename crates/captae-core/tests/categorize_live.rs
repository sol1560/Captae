//! 用真的模型整理一份真的素材库（手动跑，默认跳过，会花模型服务的钱）。素材库先复制一份，原来的不动：
//!
//! CAPTAE_LIVE_LLM_URL=https://api.deepseek.com CAPTAE_LIVE_LLM_MODEL=deepseek-flash CAPTAE_LIVE_LLM_KEY=sk-… \
//! CAPTAE_LIVE_LIBRARY=<library.sqlite> cargo test -p captae-core --test categorize_live -- --ignored --nocapture
//!
//! 打印提议的分类、每条放进哪一类和 AI 标签、用了多少 token；`CAPTAE_LIVE_OUT=<文件>` 时另写一份 Markdown。

use captae_core::categorize;
use captae_core::{Core, Options, Paths};
use serde_json::json;
use std::fmt::Write as _;

#[tokio::test]
#[ignore]
async fn organize_real_library() {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let (Some(url), Some(model), Some(src)) = (env("CAPTAE_LIVE_LLM_URL"), env("CAPTAE_LIVE_LLM_MODEL"), env("CAPTAE_LIVE_LIBRARY")) else {
        eprintln!("没设 CAPTAE_LIVE_LLM_URL / CAPTAE_LIVE_LLM_MODEL / CAPTAE_LIVE_LIBRARY，跳过");
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::portable(dir.path());
    paths.ensure().unwrap();
    // 只读打开原来的素材库，整个复制一份（带上 WAL 里还没写回的）
    {
        let c = rusqlite::Connection::open_with_flags(&src, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        c.execute("VACUUM INTO ?1", [paths.db().display().to_string()]).unwrap();
    }
    let core = Core::open(Options { paths, sidecar_dir: None, restore: false }).await.unwrap();
    core.patch_settings(&json!({"llmBaseUrl": url, "llmModel": model, "llmApiKey": env("CAPTAE_LIVE_LLM_KEY").unwrap_or_default(), "autoUpdate": false}))
        .unwrap();

    let est = categorize::estimate(&core).unwrap();
    println!("估计：{} 条，附封面 {} 条，约 {} token", est.items, est.covers, est.tokens);

    let t0 = std::time::Instant::now();
    let c2 = core.clone();
    let p = core.run_task("categorize", "live".into(), None, |rep| async move { Ok(serde_json::to_value(categorize::propose(&c2, &rep).await?)?) }).await.unwrap();
    let secs = t0.elapsed().as_secs_f64();
    let mut md = String::new();
    writeln!(md, "# 真模型整理结果\n\n模型 {model}（{url}），{} 条素材，用时 {secs:.1} 秒，token 输入 {} / 输出 {}。估计是 {} token。\n", p["total"], p["usage"]["prompt"], p["usage"]["completion"], est.tokens).unwrap();
    writeln!(md, "## 提议的分类\n").unwrap();
    for c in p["categories"].as_array().unwrap() {
        writeln!(md, "- **{}**（{} 条）：{}", c["name"].as_str().unwrap(), c["count"], c["desc"].as_str().unwrap()).unwrap();
    }
    writeln!(md, "- 没把握、留在未分类：{} 条\n", p["none"]).unwrap();

    let r = categorize::apply(&core, &[]).unwrap();
    writeln!(md, "确认后：{} 个分类，放进去 {} 条，未分类 {} 条。\n\n## 每一条\n\n| 标题 | 来源 | 分类 | AI 标签 | 封面描述 |\n|---|---|---|---|---|", r.categories, r.filed, r.none).unwrap();
    let (cats, _) = core.lib.categories().unwrap();
    let (items, _) = core.lib.query(&captae_core::library::Query { limit: Some(1000), ..Default::default() }).unwrap();
    for a in &items {
        let cat = a.category_id.and_then(|id| cats.iter().find(|c| c.id == id)).map(|c| c.name.as_str()).unwrap_or("（未分类）");
        let cap = core.lib.cache_get(a.id, "ai_caption").unwrap().map(|x| x.0).unwrap_or_default();
        let title: String = a.title.chars().take(40).collect::<String>().replace('|', "/");
        writeln!(md, "| {title} | {} | {cat} | {} | {} |", a.src, a.ai_tags.join("、"), cap.replace('|', "/")).unwrap();
    }
    println!("{md}");
    if let Some(out) = env("CAPTAE_LIVE_OUT") {
        std::fs::write(out, &md).unwrap();
    }
    assert!(r.categories >= 2, "至少分出两类");
}
