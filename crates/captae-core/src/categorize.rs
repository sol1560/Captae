//! 素材库 AI 分类：用户自己的模型（llm.rs，BYOK）按内容把素材分成几类，再给每条两三个标签。
//!
//! 流程（调研和实验见维护者本机 work/library-categories/RESEARCH.md）：
//! 1. 第一次整理（`propose`，后台任务）：标题、简介看不出内容的先看封面写一句描述（存成 asset_cache 的 ai_caption，也进全文检索）→
//!    让模型提 4 到 8 个分类（开低档思考）→ 逐批归类（关思考）。结果先放在内存里，界面给用户看、改名、合并、删掉。
//! 2. 用户确认（`apply`）才写进素材库：建分类、放素材、加 AI 标签。同样的请求问两次，模型给的分类会不一样，所以分类这一套必须用户确认后固定下来。
//! 3. 之后新下载的（`on_added`）往固定的分类里放；转换、剪辑出来的跟着原素材。「给未分类的归类」（`file_items`）同样只往已有的分类里放。
//! 4. 用户亲手定过分类的素材（category_by = user）AI 不再动，还会当例子带进提示词；用户建的、放着用户素材的分类重新整理时保留。
//!
//! 发给模型的只有标题、简介、来源、上传者、时长、用户加的标签、AI 写的封面描述，看不出内容时附上封面（设置里能关）；
//! 文件路径（里面有用户名）、cookie 一律不发。

use crate::library::{Asset, Category, Pick};
use crate::llm::{self, Chat, Llm, Part, Usage};
use crate::queue::Core;
use crate::tasks::Reporter;
use crate::tr;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// 一批归类多少条：实验里 45 条一次也没问题，40 条一批出错时重来的少
const ASSIGN_BATCH: usize = 40;
/// 一批看几张封面
const CAPTION_BATCH: usize = 6;
/// 提分类时最多给模型看多少条
const SAMPLE: usize = 400;
/// 置信度低于这个的放进未分类（实验里看不出内容的模型自己给 0.3）
const MIN_CONF: f64 = 0.35;
/// 自动归类时一次最多补多少条（启动时补上次没分成的）
const SWEEP_MAX: usize = 200;

#[derive(Default)]
pub struct State {
    /// 整理出来、还等用户确认的结果
    pending: Mutex<Option<Pending>>,
    /// 自动归类一条一条来，不同时发一堆请求
    auto: tokio::sync::Mutex<()>,
}

struct Pending {
    proposal: Proposal,
    /// 素材 id → (分类 key，没分成是 None；AI 标签)
    assign: HashMap<i64, (Option<String>, Vec<String>)>,
}

/// 提议里的一个分类
#[derive(Debug, Clone, Serialize)]
pub struct PCat {
    pub key: String,
    pub name: String,
    pub desc: String,
    /// 会放进去几条
    pub count: u64,
    /// 几张封面（界面上看这一类大概是什么）
    pub samples: Vec<Sample>,
    /// 已有的分类（重新整理时）
    pub id: Option<i64>,
    /// 用户建的、或者放着用户亲手放的素材：这次整理不动它
    pub fixed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Sample {
    pub id: i64,
    pub title: String,
    pub thumb: Option<std::path::PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Proposal {
    pub categories: Vec<PCat>,
    /// 这次整理了几条
    pub total: u64,
    /// 其中没分成、会留在未分类的
    pub none: u64,
    pub usage: Usage,
}

/// 用户在确认前对一个分类的改动
#[derive(Debug, Clone, Deserialize)]
pub struct Edit {
    pub key: String,
    pub name: Option<String>,
    pub desc: Option<String>,
    /// 合并进另一个分类（key）
    pub into: Option<String>,
    /// 不要这个分类，里面的放回未分类
    #[serde(default)]
    pub drop: bool,
}

/// 开始整理前给用户看的：会发多少、发给谁
#[derive(Debug, Clone, Serialize)]
pub struct Estimate {
    #[serde(rename = "hasModel")]
    pub has_model: bool,
    pub host: String,
    pub model: String,
    /// yes / no / 空（不知道）
    pub vision: String,
    /// 要整理的素材
    pub items: u64,
    /// 其中会附上封面的
    pub covers: u64,
    /// 大约多少 token（输入加输出）
    pub tokens: u64,
    /// 已经有几个分类（有就是「重新整理」）
    pub existing: u64,
    /// 用户亲手放过分类的素材（这次不动）
    #[serde(rename = "userFiled")]
    pub user_filed: u64,
}

/// 标题、简介看不出内容的：简介很短，或者标题只是网页名（「主页 / X」「Flo (@flohoeller) / X」）
pub fn thin(a: &Asset) -> bool {
    let t = a.title.trim();
    a.desc.trim().chars().count() < 40 || t.ends_with("/ X") || t.ends_with("/X")
}

fn want_covers(s: &crate::Settings) -> bool {
    s.ai_send_covers && s.llm_vision != "no"
}

/// 发给模型的一条：只有分类用得上的字段，空的不发
fn brief(a: &Asset, caption: Option<&str>, speech: &str) -> Value {
    let mut v = json!({"id": a.id, "type": a.kind, "title": a.title});
    let o = v.as_object_mut().unwrap();
    let mut put = |k: &str, x: Value| {
        o.insert(k.into(), x);
    };
    if !a.src.is_empty() {
        put("site", json!(a.src));
    }
    if !a.uploader.is_empty() {
        put("uploader", json!(a.uploader));
    }
    let page = if a.page.is_empty() { &a.url } else { &a.page };
    if !page.is_empty() {
        put("page", json!(page.chars().take(120).collect::<String>()));
    }
    if let Some(d) = a.duration.filter(|d| *d > 0.0) {
        put("minutes", json!((d / 6.0).round() / 10.0));
    }
    let desc: String = a.desc.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(400).collect();
    if !desc.is_empty() {
        put("desc", json!(desc));
    }
    let user_tags: Vec<&String> = a.tags.iter().filter(|t| !a.ai_tags.contains(t)).collect();
    if !user_tags.is_empty() {
        put("tags", json!(user_tags));
    }
    if let Some(c) = caption.filter(|c| !c.is_empty()) {
        put("cover", json!(c));
    }
    if !speech.is_empty() {
        put("speech", json!(speech));
    }
    v
}

/// 字幕文字的开头一段：简介看不出内容的声音、视频靠它
fn speech_of(core: &Core, a: &Asset) -> String {
    if !a.has_transcript || !thin(a) {
        return String::new();
    }
    core.lib.transcript(a.id).unwrap_or_default().split_whitespace().collect::<Vec<_>>().join(" ").chars().take(300).collect()
}

fn caption_of(core: &Core, id: i64) -> Option<String> {
    core.lib.cache_get(id, "ai_caption").ok().flatten().map(|(v, _)| v)
}

fn lang() -> &'static str {
    llm::lang_name(crate::i18n::current())
}

fn need_model(core: &Core) -> Result<Llm> {
    Llm::from_settings(&core.settings()).ok_or_else(|| anyhow!("NO_MODEL: {}", tr!("还没有设置 AI 模型：在设置 › AI 模型里填上地址和模型名")))
}

/* ---------------- 看封面 ---------------- */

/// 看不出内容、有封面、还没看过的，按批看封面写一句描述。模型看不了图时记下（llm_vision = no），这次不再看
async fn captions(core: &Core, llm: &Llm, items: &[Asset], usage: &mut Usage, mut on: impl FnMut(usize, usize)) -> Result<()> {
    if !want_covers(&core.settings()) {
        return Ok(());
    }
    let todo: Vec<&Asset> = items.iter().filter(|a| thin(a) && a.thumb.as_ref().is_some_and(|t| t.exists()) && caption_of(core, a.id).is_none()).collect();
    let system = format!(
        "You describe covers of items in a person's media library. For each item you get its metadata and its cover image (a video frame, album art or the image itself). \
Write one short sentence in {} saying what the item is about: the subject, what is shown, and any visible text that tells the topic. Don't guess beyond what you can see and read. \
Reply with JSON only: {{\"items\":[{{\"id\":0,\"caption\":\"\"}}]}}",
        lang()
    );
    let http = core.http();
    let mut done = 0;
    for chunk in todo.chunks(CAPTION_BATCH) {
        on(done, todo.len());
        let mut parts = vec![Part::Text("Items follow. Each item's JSON is followed by its cover image.".into())];
        for a in chunk {
            parts.push(Part::Text(brief(a, None, "").to_string()));
            parts.push(Part::Image(a.thumb.clone().unwrap()));
        }
        let r = llm::chat_json(&http, llm, &Chat::new(system.clone(), parts).json()).await;
        let (v, u) = match r {
            Ok(x) => x,
            // 400：多半是这个模型不收图片。记下来，之后只看文字
            Err(e) if e.downcast_ref::<llm::HttpError>().is_some_and(|h| h.status == 400) => {
                tracing::info!("模型不收图片，这次只看文字：{e}");
                let _ = core.patch_settings(&json!({"llmVision": "no"}));
                return Ok(());
            }
            Err(e) => return Err(e),
        };
        usage.add(&u);
        let ids: HashSet<i64> = chunk.iter().map(|a| a.id).collect();
        for it in v["items"].as_array().into_iter().flatten() {
            let (Some(id), Some(c)) = (it["id"].as_i64(), it["caption"].as_str()) else { continue };
            let c: String = c.trim().chars().take(200).collect();
            if ids.contains(&id) && !c.is_empty() {
                core.lib.cache_set_indexed(id, "ai_caption", &c)?;
            }
        }
        done += chunk.len();
    }
    on(done, todo.len());
    Ok(())
}

/* ---------------- 提分类 ---------------- */

/// 提分类时给模型看的：新的 250 条，再从更早的里面均匀挑一些
fn sample(items: &[Asset]) -> Vec<&Asset> {
    if items.len() <= SAMPLE {
        return items.iter().collect();
    }
    let head = 250;
    let rest = &items[head..];
    let step = rest.len() as f64 / (SAMPLE - head) as f64;
    let mut v: Vec<&Asset> = items[..head].iter().collect();
    v.extend((0..SAMPLE - head).map(|i| &rest[((i as f64 * step) as usize).min(rest.len() - 1)]));
    v
}

/// 已有分类里哪些这次不动：用户建的，或者放着用户亲手放的素材
fn fixed_ids(core: &Core, cats: &[Category]) -> HashSet<i64> {
    let held: HashSet<i64> = core.lib.user_filed().unwrap_or_default().into_iter().filter_map(|(_, c)| c).collect();
    cats.iter().filter(|c| c.by == "user" || held.contains(&c.id)).map(|c| c.id).collect()
}

/// 提几类：跟着素材数走，平均每类七八条以上。45 条最多 6 类，60 条以上最多 8 类，很少的库 2 到 3 类。
/// （真模型实测：45 条时给「4 到 8 类」，分出 8 类，有几类只有两三条）
fn category_range(n: usize) -> (usize, usize) {
    let hi = (n / 7).clamp(3, 8);
    (hi.min(4).min(if n < 14 { 2 } else { 3 }), hi)
}

async fn taxonomy(core: &Core, llm: &Llm, items: &[Asset], cats: &[Category], fixed: &HashSet<i64>, usage: &mut Usage) -> Result<Vec<PCat>> {
    let (lo, hi) = category_range(items.len());
    let mut system = format!(
        "You organize a person's media library: videos, audio and images they downloaded from websites.\n\
Design a small set of categories ({lo} to {hi} in total) that this person would use like folders to browse the library.\n\
Rules:\n\
- Prefer fewer, broader categories. Each category should hold several items; don't make a category for just one or two items.\n\
- Categories describe what the content is (topic, genre or purpose). Don't make categories by website, file type or language: the app already filters by those.\n\
- Name each category in {lang}, short (at most 6 Chinese characters, or 3 words in other languages). Give each a one-sentence description in {lang}.\n\
- Avoid a catch-all category such as \"Other\": items that fit nowhere can stay uncategorized.\n\
- New categories get the keys n1, n2, n3 and so on.\n",
        lang = lang(),
        lo = lo,
        hi = hi
    );
    if !cats.is_empty() {
        system.push_str("The person already has these categories:\n");
        for c in cats {
            let how = if fixed.contains(&c.id) { "fixed: keep it exactly as it is" } else { "replaceable: keep it (same key, you may improve the name or description) or drop it" };
            system.push_str(&format!("- key e{}: {} ({}) [{how}]\n", c.id, c.name, c.desc));
        }
    }
    system.push_str("Reply with JSON only: {\"categories\":[{\"key\":\"\",\"name\":\"\",\"desc\":\"\"}]}");
    let lines: String = sample(items).iter().map(|a| brief(a, caption_of(core, a.id).as_deref(), &speech_of(core, a)).to_string() + "\n").collect();
    let (v, u) = llm::chat_json(&core.http(), llm, &Chat::new(system, vec![Part::Text(lines)]).json().think(true)).await?;
    usage.add(&u);
    let mut out: Vec<PCat> = vec![];
    let mut names = HashSet::new();
    for c in v["categories"].as_array().into_iter().flatten() {
        let key = c["key"].as_str().unwrap_or("").trim().to_string();
        let name: String = c["name"].as_str().unwrap_or("").trim().chars().take(24).collect();
        if key.is_empty() || name.is_empty() || !names.insert(name.to_lowercase()) || out.iter().any(|x| x.key == key) {
            continue;
        }
        let id = key.strip_prefix('e').and_then(|n| n.parse::<i64>().ok()).filter(|id| cats.iter().any(|c| c.id == *id));
        // 不动的分类照原样（模型改了名也不算）
        let (name, desc) = match id.and_then(|id| cats.iter().find(|c| c.id == id && fixed.contains(&c.id))) {
            Some(c) => (c.name.clone(), c.desc.clone()),
            None => (name, c["desc"].as_str().unwrap_or("").trim().chars().take(120).collect()),
        };
        out.push(PCat { key, name, desc, count: 0, samples: vec![], id, fixed: id.is_some_and(|i| fixed.contains(&i)) });
        if out.len() >= 12 {
            break;
        }
    }
    // 模型漏掉的不动的分类加回去
    for c in cats.iter().filter(|c| fixed.contains(&c.id)) {
        if !out.iter().any(|x| x.id == Some(c.id)) {
            out.push(PCat { key: format!("e{}", c.id), name: c.name.clone(), desc: c.desc.clone(), count: 0, samples: vec![], id: Some(c.id), fixed: true });
        }
    }
    if out.is_empty() {
        bail!(tr!("AI 模型没有给出分类，再试一次"));
    }
    Ok(out)
}

/* ---------------- 归类 ---------------- */

type Assigned = HashMap<i64, (Option<String>, Vec<String>)>;

/// 一批一批放进给定的分类。返回 素材 id → (key 或 None, 标签)
async fn assign(core: &Core, llm: &Llm, items: &[Asset], cats: &[(String, String, String)], usage: &mut Usage, mut on: impl FnMut(usize, usize)) -> Result<Assigned> {
    let mut system = String::from("You file items of a person's media library into categories.\nCategories:\n");
    for (k, n, d) in cats {
        system.push_str(&format!("- {k}: {n}{}\n", if d.is_empty() { String::new() } else { format!(" ({d})") }));
    }
    system.push_str(&format!(
        "Put each item into exactly one category by its key. If none fits at all, use \"\" as the key.\n\
Also give up to 3 short tags in {} that help find the item later: specific names of brands, people, products, places or events. Don't repeat the category name.\n\
And a confidence from 0 to 1.\n",
        lang()
    ));
    let examples = core.lib.user_examples(12).unwrap_or_default();
    if !examples.is_empty() {
        system.push_str("How the person filed some items themselves (follow their way of sorting):\n");
        for (a, cat) in &examples {
            system.push_str(&format!("- \"{}\" → {cat}\n", a.title.chars().take(80).collect::<String>()));
        }
    }
    system.push_str("Reply with JSON only: {\"items\":[{\"id\":0,\"cat\":\"\",\"tags\":[],\"conf\":0.0}]}");
    let keys: HashMap<&str, &str> = cats.iter().map(|(k, n, _)| (k.as_str(), n.as_str())).collect();
    let http = core.http();
    let mut out = HashMap::new();
    let mut done = 0;
    for chunk in items.chunks(ASSIGN_BATCH) {
        on(done, items.len());
        let lines: String = chunk.iter().map(|a| brief(a, caption_of(core, a.id).as_deref(), &speech_of(core, a)).to_string() + "\n").collect();
        let (v, u) = llm::chat_json(&http, llm, &Chat::new(system.clone(), vec![Part::Text(lines)]).json()).await?;
        usage.add(&u);
        let ids: HashSet<i64> = chunk.iter().map(|a| a.id).collect();
        for it in v["items"].as_array().into_iter().flatten() {
            let Some(id) = it["id"].as_i64().filter(|id| ids.contains(id)) else { continue };
            let key = it["cat"].as_str().unwrap_or("").trim();
            let conf = it["conf"].as_f64().unwrap_or(1.0);
            let cat_name = keys.get(key).copied();
            let key = cat_name.filter(|_| conf >= MIN_CONF).map(|_| key.to_string());
            let mut tags: Vec<String> = vec![];
            for t in it["tags"].as_array().into_iter().flatten().filter_map(|t| t.as_str()) {
                let t = t.trim().trim_start_matches('#').to_string();
                let same_as_cat = cat_name.is_some_and(|n| n.eq_ignore_ascii_case(&t));
                if !t.is_empty() && t.chars().count() <= 24 && !same_as_cat && !tags.iter().any(|x| x.eq_ignore_ascii_case(&t)) {
                    tags.push(t);
                }
            }
            tags.truncate(3);
            out.insert(id, (key, tags));
        }
        done += chunk.len();
    }
    on(done, items.len());
    Ok(out)
}

/* ---------------- 第一次整理 / 重新整理 ---------------- */

pub fn estimate(core: &Core) -> Result<Estimate> {
    let s = core.settings();
    let llm = Llm::from_settings(&s);
    let items = core.lib.pick(&Pick::NotByUser)?;
    let covers = if want_covers(&s) { items.iter().filter(|a| thin(a) && a.thumb.as_ref().is_some_and(|t| t.exists()) && caption_of(core, a.id).is_none()).count() } else { 0 };
    let (cats, _) = core.lib.categories()?;
    let n = items.len() as u64;
    // 实验：每条文字约 150 token 输入、46 输出；提分类时样本再发一遍；一张封面约 290 token
    let tokens = n * 200 + (n.min(SAMPLE as u64)) * 150 + covers as u64 * 350 + 2000;
    let user_filed = core.lib.user_filed()?.len() as u64;
    Ok(Estimate {
        has_model: llm.is_some(),
        host: llm.as_ref().map(|l| l.host()).unwrap_or_default(),
        model: llm.as_ref().map(|l| l.model.clone()).unwrap_or_default(),
        vision: s.llm_vision.clone(),
        items: n,
        covers: covers as u64,
        tokens,
        existing: cats.len() as u64,
        user_filed,
    })
}

/// 整理：看封面 → 提分类 → 归类。结果等用户确认（`apply`），这一步不改素材库（封面描述除外，它本来就是缓存）
pub async fn propose(core: &Core, rep: &Reporter) -> Result<Proposal> {
    let llm = need_model(core)?;
    let items = core.lib.pick(&Pick::NotByUser)?;
    if items.is_empty() {
        bail!(tr!("没有要整理的素材"));
    }
    let (cats, _) = core.lib.categories()?;
    let fixed = fixed_ids(core, &cats);
    let mut usage = Usage::default();

    // 看封面：占进度的前 30%
    captions(core, &llm, &items, &mut usage, |d, n| {
        if n > 0 {
            rep.progress(0.3 * d as f64 / n as f64, tr!("正在看封面 {done} / {total}", done = d, total = n), None);
        }
    })
    .await?;
    // 看过封面以后重新读一遍（全文检索、描述都更新了）
    let items = core.lib.pick(&Pick::NotByUser)?;

    rep.progress(0.3, tr!("正在想怎么分"), None);
    let pcats = taxonomy(core, &llm, &items, &cats, &fixed, &mut usage).await?;

    let list: Vec<(String, String, String)> = pcats.iter().map(|c| (c.key.clone(), c.name.clone(), c.desc.clone())).collect();
    let assigned = assign(core, &llm, &items, &list, &mut usage, |d, n| {
        rep.progress(0.4 + 0.6 * d as f64 / n.max(1) as f64, tr!("正在归类 {done} / {total}", done = d, total = n), None);
    })
    .await?;

    let mut pcats = pcats;
    for c in &mut pcats {
        let mine: Vec<&Asset> = items.iter().filter(|a| assigned.get(&a.id).and_then(|x| x.0.as_deref()) == Some(c.key.as_str())).collect();
        c.count = mine.len() as u64;
        // 封面挑有图的
        let mut with_thumb: Vec<&&Asset> = mine.iter().filter(|a| a.thumb.as_ref().is_some_and(|t| t.exists())).collect();
        with_thumb.truncate(4);
        c.samples = with_thumb.into_iter().map(|a| Sample { id: a.id, title: a.title.clone(), thumb: a.thumb.clone() }).collect();
        // 不动的分类里本来就有的（用户放的）也算上
        if let Some(id) = c.id {
            c.count += core.lib.user_filed()?.into_iter().filter(|(_, cat)| *cat == Some(id)).count() as u64;
        }
    }
    let none = items.iter().filter(|a| assigned.get(&a.id).and_then(|x| x.0.as_ref()).is_none()).count() as u64;
    let proposal = Proposal { categories: pcats, total: items.len() as u64, none, usage };
    // 没分成的也记着（确认时放回未分类、标成 AI 整理过）
    let mut all: Assigned = assigned;
    for a in &items {
        all.entry(a.id).or_insert((None, vec![]));
    }
    *core.categorize.pending.lock().unwrap() = Some(Pending { proposal: proposal.clone(), assign: all });
    Ok(proposal)
}

#[derive(Debug, Clone, Serialize)]
pub struct Applied {
    pub categories: u64,
    pub filed: u64,
    pub none: u64,
}

/// 用户确认了（可能改过名、合并、删掉几个）：写进素材库
pub fn apply(core: &Core, edits: &[Edit]) -> Result<Applied> {
    let Some(p) = core.categorize.pending.lock().unwrap().take() else {
        bail!(tr!("整理的结果已经不在了，再整理一次"));
    };
    let edit = |k: &str| edits.iter().find(|e| e.key == k);
    // 合并到哪：顺着 into 找到最后留下来的那个（防止绕圈）
    let resolve = |k: &str| -> Option<String> {
        let mut cur = k.to_string();
        for _ in 0..16 {
            match edit(&cur) {
                Some(e) if e.drop => return None,
                Some(Edit { into: Some(t), .. }) if t != &cur => cur = t.clone(),
                _ => return p.proposal.categories.iter().any(|c| c.key == cur).then_some(cur),
            }
        }
        None
    };
    let (old, _) = core.lib.categories()?;
    let mut ids: HashMap<String, i64> = HashMap::new();
    let mut order = vec![];
    // 按用户排好的顺序（edits 的顺序），没出现在 edits 里的照提议的顺序排在后面
    let mut keys: Vec<String> = edits.iter().map(|e| e.key.clone()).filter(|k| p.proposal.categories.iter().any(|c| &c.key == k)).collect();
    for c in &p.proposal.categories {
        if !keys.contains(&c.key) {
            keys.push(c.key.clone());
        }
    }
    for k in &keys {
        if resolve(k).as_deref() != Some(k.as_str()) {
            continue; // 删掉的、合并走的
        }
        let c = p.proposal.categories.iter().find(|c| &c.key == k).unwrap();
        let e = edit(k);
        let name = e.and_then(|e| e.name.clone()).map(|n| n.trim().to_string()).filter(|n| !n.is_empty()).unwrap_or_else(|| c.name.clone());
        let desc = e.and_then(|e| e.desc.clone()).unwrap_or_else(|| c.desc.clone());
        // 用户改过名的算用户的（以后重新整理也留着）
        let renamed = name != c.name;
        let id = match c.id {
            Some(id) => {
                let cur = old.iter().find(|o| o.id == id);
                let name_change = cur.map(|o| o.name != name).unwrap_or(true);
                core.lib.update_category(id, name_change.then_some(name.as_str()), Some(&desc), renamed.then_some("user"))?;
                id
            }
            None => core.lib.add_category(&name, &desc, if renamed { "user" } else { "ai" })?,
        };
        ids.insert(k.clone(), id);
        order.push(id);
    }
    // 原来 AI 建的、这次没留下来的分类删掉（里面的素材下面重新放）
    for o in &old {
        if o.by == "ai" && !order.contains(&o.id) && !p.proposal.categories.iter().any(|c| c.fixed && c.id == Some(o.id)) {
            core.lib.delete_category(o.id)?;
        }
    }
    let mut filed = 0;
    let mut none = 0;
    for (aid, (key, tags)) in &p.assign {
        let cat = key.as_deref().and_then(resolve).and_then(|k| ids.get(&k).copied());
        if cat.is_some() {
            filed += 1;
        } else {
            none += 1;
        }
        // 这段时间里用户亲手改过的不动
        if core.lib.get(*aid)?.is_some_and(|a| a.category_by == "user") {
            continue;
        }
        core.lib.set_category(&[*aid], cat, "ai")?;
        core.lib.set_ai_tags(*aid, tags)?;
    }
    // 排序：这次留下的在前，原来其余的（用户的）跟在后面
    let (now, _) = core.lib.categories()?;
    for c in now {
        if !order.contains(&c.id) {
            order.push(c.id);
        }
    }
    core.lib.reorder_categories(&order)?;
    core.emit_library();
    Ok(Applied { categories: ids.len() as u64, filed, none })
}

/// 不要这次整理的结果了
pub fn discard(core: &Core) {
    core.categorize.pending.lock().unwrap().take();
}

/* ---------------- 往已有的分类里放 ---------------- */

/// 往已有的分类里放（「给未分类的归类」、新下载的、启动时补）。直接写进素材库，标成 AI 放的。返回 (放进分类的, 没分成的)
pub async fn file_items(core: &Core, pick: &Pick, rep: Option<&Reporter>) -> Result<(u64, u64)> {
    let llm = need_model(core)?;
    let (cats, _) = core.lib.categories()?;
    if cats.is_empty() {
        bail!(tr!("还没有分类，先用 AI 整理一次"));
    }
    let mut items = core.lib.pick(pick)?;
    items.retain(|a| a.category_by != "user");
    if let Pick::Never = pick {
        items.truncate(SWEEP_MAX);
    }
    if items.is_empty() {
        return Ok((0, 0));
    }
    let mut usage = Usage::default();
    captions(core, &llm, &items, &mut usage, |d, n| {
        if let (Some(r), true) = (rep, n > 0) {
            r.progress(0.3 * d as f64 / n as f64, tr!("正在看封面 {done} / {total}", done = d, total = n), None);
        }
    })
    .await?;
    let ids: Vec<i64> = items.iter().map(|a| a.id).collect();
    let items = core.lib.pick(&Pick::Ids(ids))?;
    let list: Vec<(String, String, String)> = cats.iter().map(|c| (format!("c{}", c.id), c.name.clone(), c.desc.clone())).collect();
    let assigned = assign(core, &llm, &items, &list, &mut usage, |d, n| {
        if let Some(r) = rep {
            r.progress(0.3 + 0.7 * d as f64 / n.max(1) as f64, tr!("正在归类 {done} / {total}", done = d, total = n), None);
        }
    })
    .await?;
    let (mut filed, mut none) = (0, 0);
    for a in &items {
        // 等回复的这段时间里用户亲手改过的不动
        if core.lib.get(a.id)?.is_some_and(|x| x.category_by == "user") {
            continue;
        }
        let (key, tags) = assigned.get(&a.id).cloned().unwrap_or((None, vec![]));
        let cat = key.and_then(|k| k.strip_prefix('c').and_then(|n| n.parse::<i64>().ok()));
        if cat.is_some() {
            filed += 1;
        } else {
            none += 1;
        }
        core.lib.set_category(&[a.id], cat, "ai")?;
        core.lib.set_ai_tags(a.id, &tags)?;
    }
    tracing::info!("归类了 {} 条（{} 条没分成），用了 {} + {} token", filed + none, none, usage.prompt, usage.completion);
    Ok((filed, none))
}

/// 自动归类开着、模型和分类都有
fn auto_ready(core: &Core) -> bool {
    let s = core.settings();
    s.ai_auto_categorize && Llm::from_settings(&s).is_some() && core.lib.categories().map(|(c, _)| !c.is_empty()).unwrap_or(false)
}

/// 新入库的一条：转换、剪辑出来的跟着原素材；下载的在后台往已有的分类里放（自动归类开着时）。失败了留着，下次启动时补
pub fn on_added(core: &Core, id: i64) {
    let core = core.clone();
    tokio::spawn(async move {
        let Ok(Some(a)) = core.lib.get(id) else { return };
        if let Some(p) = a.parent_id.and_then(|p| core.lib.get(p).ok().flatten()) {
            if !p.category_by.is_empty() && core.lib.set_category(&[id], p.category_id, &p.category_by).is_ok() {
                core.emit_library();
            }
            return;
        }
        if !auto_ready(&core) {
            return;
        }
        let _g = core.categorize.auto.lock().await;
        match file_items(&core, &Pick::Ids(vec![id]), None).await {
            Ok(_) => core.emit_library(),
            Err(e) => tracing::info!("新下载的这条没有自动归类（下次启动时再试）：{e}"),
        }
    });
}

/// 桌面端启动后：补上还没整理过的（上次没分成、或者 Captae 关着时命令行下的）
pub async fn sweep(core: &Core) {
    if !auto_ready(core) {
        return;
    }
    let _g = core.categorize.auto.lock().await;
    match file_items(core, &Pick::Never, None).await {
        Ok((f, n)) if f + n > 0 => core.emit_library(),
        Ok(_) => {}
        Err(e) => tracing::info!("启动时补归类没有完成：{e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::NewAsset;
    use crate::llm::testkit;
    use crate::{Options, Paths};
    use std::sync::Arc;

    async fn core(dir: &std::path::Path) -> Core {
        Core::open(Options { paths: Paths::portable(dir), sidecar_dir: None, restore: false }).await.unwrap()
    }

    fn add(core: &Core, title: &str, desc: &str, thumb: Option<std::path::PathBuf>) -> i64 {
        let path = core.paths.movies.join(format!("{}.mp4", title.replace('/', "_")));
        std::fs::write(&path, b"x").unwrap();
        core.lib.add(&NewAsset { kind: "video".into(), title: title.into(), description: desc.into(), site: "YouTube".into(), path, thumb, ..Default::default() }).unwrap()
    }

    /// 假模型：按系统提示词认出是哪一步，回固定的结果
    fn fake_model() -> testkit::Handler {
        Arc::new(|v: &Value| {
            let sys = v["messages"][0]["content"].as_str().unwrap_or("");
            let user: String = match &v["messages"][1]["content"] {
                Value::String(s) => s.clone(),
                Value::Array(a) => a.iter().filter_map(|p| p["text"].as_str()).collect::<Vec<_>>().join("\n"),
                _ => String::new(),
            };
            let items: Vec<Value> = user.lines().filter_map(|l| serde_json::from_str::<Value>(l).ok()).filter(|x| x.get("id").is_some()).collect();
            let ids: Vec<i64> = items.iter().filter_map(|x| x["id"].as_i64()).collect();
            if sys.starts_with("You describe covers") {
                let out: Vec<Value> = ids.iter().map(|id| json!({"id": id, "caption": "一个人在介绍日程管理软件"})).collect();
                return (200, json!({"items": out}).to_string());
            }
            if sys.starts_with("You organize") {
                return (200, json!({"categories": [{"key": "n1", "name": "科技数码", "desc": "软件和硬件"}, {"key": "n2", "name": "音乐演出", "desc": "MV 和演奏"}]}).to_string());
            }
            // 归类：标题以 Music 开头的进音乐，其余进科技；这一批里 id 最大的那条置信度很低
            let re = regex::Regex::new(r"^- ([a-z]\d+): (.+)$").unwrap();
            let key_of = |word: &str| sys.lines().filter_map(|l| re.captures(l)).find(|c| c[2].contains(word)).map(|c| c[1].to_string()).unwrap_or_default();
            let (tech, music) = (key_of("科技"), key_of("音乐"));
            let max = ids.iter().copied().max().unwrap_or(0);
            let out: Vec<Value> = items
                .iter()
                .map(|x| {
                    let id = x["id"].as_i64().unwrap();
                    let m = x["title"].as_str().unwrap_or("").starts_with("Music");
                    json!({"id": id, "cat": if m { &music } else { &tech }, "tags": ["微软", "科技数码", "Copilot"], "conf": if id == max { 0.2 } else { 0.9 }})
                })
                .collect();
            (200, json!({"items": out}).to_string())
        })
    }

    /// 整理一次：提议不改素材库，确认后才写；改名的分类算用户的；没把握的进未分类；AI 标签不重复分类名；
    /// 之后新加的往已有分类里放，转换出来的跟着原素材
    #[tokio::test]
    async fn propose_apply_and_file_new() {
        let dir = tempfile::tempdir().unwrap();
        let core = core(dir.path()).await;
        let (url, seen) = testkit::serve(fake_model()).await;
        core.patch_settings(&json!({"llmBaseUrl": url, "llmModel": "fake", "llmApiKey": "sk-x", "llmVision": "yes"})).unwrap();
        let thumb = dir.path().join("t.jpg");
        std::fs::write(&thumb, b"\xff\xd8\xff").unwrap();
        let a = add(&core, "Meet Copilot", &"Copilot is your AI companion for work and life. ".repeat(3), None);
        let b = add(&core, "Music video Not The End", &"Official music video by the class of 2025, recorded live. ".repeat(2), None);
        let c = add(&core, "主页 / X", "", Some(thumb.clone()));

        let est = estimate(&core).unwrap();
        assert_eq!((est.has_model, est.items, est.covers, est.existing), (true, 3, 1, 0));

        let id = core.new_task_id();
        let c2 = core.clone();
        let v = core.run_task_as(id, "categorize", "t".into(), None, |rep| async move { Ok(serde_json::to_value(propose(&c2, &rep).await?)?) }).await.unwrap();
        let p: Value = v;
        assert_eq!(p["categories"].as_array().unwrap().len(), 2);
        assert_eq!(p["total"], 3);
        assert_eq!(core.lib.categories().unwrap().0.len(), 0, "确认前不写素材库");
        // 看过封面：描述存下来了，也能搜到
        assert!(caption_of(&core, c).is_some());
        // 提分类那次开了思考（只对 DeepSeek 发档位，这里只看有没有 temperature）
        assert!(seen.lock().unwrap().iter().any(|r| r["messages"][0]["content"].as_str().unwrap().starts_with("You organize") && r.get("temperature").is_none()));

        // 用户把「科技数码」改名为「科技」，确认
        let r = apply(&core, &[Edit { key: "n1".into(), name: Some("科技".into()), desc: None, into: None, drop: false }]).unwrap();
        assert_eq!(r.categories, 2);
        let (cats, none) = core.lib.categories().unwrap();
        let tech = cats.iter().find(|c| c.name == "科技").unwrap();
        assert_eq!(tech.by, "user", "改过名的算用户的");
        let music = cats.iter().find(|c| c.name == "音乐演出").unwrap();
        assert_eq!(music.by, "ai");
        let get = |id| core.lib.get(id).unwrap().unwrap();
        assert_eq!((get(a).category_id, get(a).category_by.as_str()), (Some(tech.id), "ai"));
        assert_eq!(get(b).category_id, Some(music.id));
        // c 的 id 最大，假模型给了 0.2：进未分类，但算整理过
        assert_eq!((get(c).category_id, get(c).category_by.as_str(), none), (None, "ai", 1));
        assert_eq!(get(a).ai_tags, vec!["微软", "Copilot"], "标签不重复分类名（「科技数码」是提议时的名字，这里过滤的是归类时的分类名）");
        assert!(apply(&core, &[]).is_err(), "确认过的结果不能再用一次");

        // 新下载的：自动往已有分类里放
        let d = add(&core, "Copilot on Windows", &"The new Copilot app on Windows brings AI to your desktop and files. ".repeat(2), None);
        let e = add(&core, "Introducing Windows 365", &"Windows 365 puts your Windows PC in the cloud for any device you use. ".repeat(2), None);
        on_added(&core, d);
        let t0 = std::time::Instant::now();
        while get(d).category_by.is_empty() {
            assert!(t0.elapsed() < std::time::Duration::from_secs(10), "自动归类没有完成");
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!(get(d).category_id, None, "单独一条时它就是 id 最大的，假模型给 0.2");
        // 「给未分类的归类」：e 还没整理过；d、c 是 AI 没分成的，也再试一次
        let (filed, _) = file_items(&core, &Pick::Uncategorized, None).await.unwrap();
        assert!(filed >= 1);
        assert_eq!(get(c).category_id, Some(tech.id));
        let _ = e;
        // 用户亲手放的 AI 不动
        core.lib.set_category(&[a], Some(music.id), "user").unwrap();
        file_items(&core, &Pick::Ids(vec![a]), None).await.unwrap();
        assert_eq!(get(a).category_id, Some(music.id));

        // 转换出来的跟着原素材，不问模型
        let n_before = seen.lock().unwrap().len();
        let out = core.paths.movies.join("Meet Copilot (转换).mov");
        std::fs::write(&out, b"x").unwrap();
        let child = core.lib.add(&NewAsset { kind: "video".into(), title: "Meet Copilot (转换)".into(), path: out, parent_id: Some(a), ..Default::default() }).unwrap();
        on_added(&core, child);
        let t0 = std::time::Instant::now();
        while get(child).category_by.is_empty() {
            assert!(t0.elapsed() < std::time::Duration::from_secs(5));
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        assert_eq!((get(child).category_id, get(child).category_by.as_str()), (Some(music.id), "user"));
        assert_eq!(seen.lock().unwrap().len(), n_before);
    }

    /// 重新整理：用户建的分类原样留着；合并、删掉按用户说的来；原来 AI 建的、这次没留下的删掉
    #[tokio::test]
    async fn reorganize_keeps_user_categories() {
        let dir = tempfile::tempdir().unwrap();
        let core = core(dir.path()).await;
        let (url, seen) = testkit::serve(fake_model()).await;
        core.patch_settings(&json!({"llmBaseUrl": url, "llmModel": "fake"})).unwrap();
        let mine = core.lib.add_category("我的收藏", "", "user").unwrap();
        let old_ai = core.lib.add_category("旧的 AI 分类", "", "ai").unwrap();
        for i in 0..3 {
            add(&core, &format!("Copilot demo {i}"), &"A product demo of the new Copilot for work and life. ".repeat(2), None);
        }
        let x = add(&core, "Music live", &"Live music performance by a school orchestra with soloists. ".repeat(2), None);
        core.lib.set_category(&[x], Some(old_ai), "ai").unwrap();
        let rep_core = core.clone();
        let p = core.run_task("categorize", "t".into(), None, |rep| async move { Ok(serde_json::to_value(propose(&rep_core, &rep).await?)?) }).await.unwrap();
        let keys: Vec<&str> = p["categories"].as_array().unwrap().iter().map(|c| c["key"].as_str().unwrap()).collect();
        assert!(keys.contains(&format!("e{mine}").as_str()), "用户建的分类加回来了：{keys:?}");
        let sys = seen.lock().unwrap().iter().find(|r| r["messages"][0]["content"].as_str().unwrap().starts_with("You organize")).unwrap()["messages"][0]["content"].as_str().unwrap().to_string();
        assert!(sys.contains(&format!("key e{mine}: 我的收藏")) && sys.contains("fixed"), "{sys}");
        // 把音乐合并进科技
        apply(&core, &[Edit { key: "n2".into(), name: None, desc: None, into: Some("n1".into()), drop: false }]).unwrap();
        let (cats, _) = core.lib.categories().unwrap();
        let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
        assert!(names.contains(&"我的收藏") && names.contains(&"科技数码") && !names.contains(&"旧的 AI 分类") && !names.contains(&"音乐演出"), "{names:?}");
        assert_eq!(names[0], "科技数码", "这次留下的排在前面");
    }

    /// 没填模型：说清楚；模型回 400 看不了图：记下 llmVision = no，这次只看文字
    #[test]
    fn range_follows_library_size() {
        assert_eq!(category_range(8), (2, 3));
        assert_eq!(category_range(20), (3, 3));
        assert_eq!(category_range(45), (3, 6));
        assert_eq!(category_range(128), (3, 8));
        assert_eq!(category_range(5000), (3, 8));
    }

    #[tokio::test]
    async fn no_model_and_no_vision() {
        let dir = tempfile::tempdir().unwrap();
        let core = core(dir.path()).await;
        add(&core, "a", "", None);
        let c2 = core.clone();
        let e = core.run_task("categorize", "t".into(), None, |rep| async move { Ok(serde_json::to_value(propose(&c2, &rep).await?)?) }).await.unwrap_err();
        assert!(e.to_string().starts_with("NO_MODEL"), "{e}");

        let (url, _) = testkit::serve(Arc::new(|v: &Value| {
            if v["messages"][1]["content"].is_array() {
                (400, "{\"error\": \"image input not supported\"}".into())
            } else {
                (fake_model())(v)
            }
        }))
        .await;
        core.patch_settings(&json!({"llmBaseUrl": url, "llmModel": "text-only"})).unwrap();
        let thumb = dir.path().join("t.jpg");
        std::fs::write(&thumb, b"\xff\xd8\xff").unwrap();
        add(&core, "主页 / X", "", Some(thumb));
        let c2 = core.clone();
        core.run_task("categorize", "t".into(), None, |rep| async move { Ok(serde_json::to_value(propose(&c2, &rep).await?)?) }).await.unwrap();
        assert_eq!(core.settings().llm_vision, "no");
        assert_eq!(estimate(&core).unwrap().covers, 0, "看不了图以后不再算封面");
    }
}
