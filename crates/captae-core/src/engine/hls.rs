//! M3U8 流媒体下载。
//!
//! 1. 读主播放列表，列出清晰度（界面上的「有 4 种清晰度可选」）
//! 2. 读选中清晰度的分片列表，8 路并发下载，每片最多重试 3 次（分片条 s0..s4 的状态来源）
//! 3. AES-128 加密的分片在内存里解密后落盘
//! 4. 按顺序拼接（TS 直接拼；fMP4 先写 init 段），再用 ffmpeg 无损封装成 MP4（没有 ffmpeg 时用 `remux.rs`）
//!
//! 已经下好的分片留在工作目录里，暂停、重试、崩溃重启后都不会重下。

use super::{Ctx, MediaInfo, Output, SpeedMeter};
use crate::errors::{self, ErrorCode, Failure};
use crate::login::Jar;
use crate::model::{SegState, Variant};
use crate::tr;
use crate::tools::Tool;
use crate::util::{clock, mb};
use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use anyhow::{anyhow, bail, Result};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use url::Url;

type Aes128CbcDec = cbc::Decryptor<aes::Aes128>;

/// 默认并发分片数；实际按设置里的「下载加速」（4 / 8 / 16）走，见 `parallel()`。
pub const PARALLEL: usize = 8;

/// 设置里的线程数 → 同时下几个分片。
pub fn parallel(threads: u32) -> usize {
    (threads as usize).clamp(2, 16)
}
pub const MAX_TRIES: u8 = 3;

#[derive(Debug, Clone, PartialEq)]
pub struct Key {
    pub method: String,
    pub uri: Option<String>,
    pub iv: Option<[u8; 16]>,
    /// KEYFORMAT（Widevine / FairPlay / PlayReady 的 DRM 会写在这里）
    pub keyformat: Option<String>,
}

/// fMP4 的初始化分片（`#EXT-X-MAP`）。`range` 是 (长度, 偏移)：Apple 的示例流整个视频是一个 main.mp4，
/// 初始化分片只是开头几百字节（`BYTERANGE="719@0"`），不带 Range 去下就会把整个文件当初始化分片下一遍
#[derive(Debug, Clone, PartialEq)]
pub struct Init {
    pub uri: String,
    pub range: Option<(u64, u64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub uri: String,
    pub dur: f64,
    pub seq: u64,
    pub key: Option<Key>,
    /// (长度, 偏移)
    pub range: Option<(u64, Option<u64>)>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Media {
    pub segments: Vec<Segment>,
    pub map: Option<Init>,
    pub endlist: bool,
    /// `#EXT-X-TARGETDURATION`：每段最长几秒（录直播时据此决定多久重新读一次列表）
    pub target: Option<f64>,
}

impl Media {
    pub fn duration(&self) -> f64 {
        self.segments.iter().map(|s| s.dur).sum()
    }
}

fn join(base: &Url, u: &str) -> String {
    base.join(u).map(|x| x.to_string()).unwrap_or_else(|_| u.to_string())
}

/// 解析 `KEY="value",KEY2=value` 形式的属性列表。
pub fn attrs(s: &str) -> HashMap<String, String> {
    let mut m = HashMap::new();
    let mut rest = s;
    while !rest.is_empty() {
        let Some(eq) = rest.find('=') else { break };
        let k = rest[..eq].trim().trim_start_matches(',').trim().to_string();
        rest = &rest[eq + 1..];
        let v;
        if let Some(r) = rest.strip_prefix('"') {
            let end = r.find('"').unwrap_or(r.len());
            v = r[..end].to_string();
            rest = r.get(end + 1..).unwrap_or("");
        } else {
            let end = rest.find(',').unwrap_or(rest.len());
            v = rest[..end].to_string();
            rest = &rest[end..];
        }
        rest = rest.trim_start_matches(',');
        m.insert(k, v);
    }
    m
}

pub fn is_master(text: &str) -> bool {
    text.contains("#EXT-X-STREAM-INF")
}

/// 一种清晰度的预估大小（MB，两位小数）：码率（bit/s）× 时长
pub(crate) fn size_mb(bps: f64, secs: f64) -> f64 {
    (bps * secs / 8.0 / 1e6 * 100.0).round() / 100.0
}

pub fn parse_master(text: &str, base: &Url) -> Vec<Variant> {
    let mut audio_groups: HashMap<String, String> = HashMap::new();
    for l in text.lines() {
        if let Some(a) = l.strip_prefix("#EXT-X-MEDIA:") {
            let a = attrs(a);
            if a.get("TYPE").map(String::as_str) == Some("AUDIO") {
                if let (Some(g), Some(u)) = (a.get("GROUP-ID"), a.get("URI")) {
                    let is_default = a.get("DEFAULT").map(String::as_str) == Some("YES");
                    if is_default || !audio_groups.contains_key(g) {
                        audio_groups.insert(g.clone(), join(base, u));
                    }
                }
            }
        }
    }
    let mut out = vec![];
    let mut pending: Option<HashMap<String, String>> = None;
    for l in text.lines().map(str::trim) {
        if let Some(a) = l.strip_prefix("#EXT-X-STREAM-INF:") {
            pending = Some(attrs(a));
        } else if !l.is_empty() && !l.starts_with('#') {
            if let Some(a) = pending.take() {
                let bw: f64 = a.get("AVERAGE-BANDWIDTH").or(a.get("BANDWIDTH")).and_then(|b| b.parse().ok()).unwrap_or(0.0);
                let res = a.get("RESOLUTION").cloned().unwrap_or_default();
                // 清晰度按短边叫：竖屏的 1080×1920 是 1080p，不是 1920p
                let mut wh = res.split('x').map(|x| x.trim().parse::<u32>().ok());
                let (w, h) = (wh.next().flatten(), wh.next().flatten());
                let h = match (w, h) {
                    (Some(w), Some(h)) => Some(w.min(h)),
                    (_, h) => h,
                };
                out.push(Variant {
                    r: res.replace('x', "×"),
                    p: h.map(|h| format!("{h}p")).unwrap_or_else(|| format!("{} kbps", (bw / 1000.0).round())),
                    b: (bw / 1e6 * 10.0).round() / 10.0,
                    mb: 0.0,
                    bps: bw,
                    uri: join(base, l),
                    audio: a.get("AUDIO").and_then(|g| audio_groups.get(g).cloned()),
                });
            }
        }
    }
    // 从高到低；同分辨率保留码率高的
    out.sort_by(|a, b| b.b.partial_cmp(&a.b).unwrap_or(std::cmp::Ordering::Equal));
    let mut seen = std::collections::HashSet::new();
    out.retain(|v| seen.insert(v.p.clone()));
    out
}

pub fn parse_media(text: &str, base: &Url) -> Result<Media> {
    if !text.trim_start().starts_with("#EXTM3U") {
        let head: String = text.trim_start().chars().take(200).collect();
        return Err(Failure::new(ErrorCode::NoMedia, tr!("这个地址不是有效的 M3U8 列表"), format!("not an M3U8 playlist ({base}): {head}")).into());
    }
    let mut m = Media::default();
    let mut seq: u64 = 0;
    let mut key: Option<Key> = None;
    let mut dur: Option<f64> = None;
    let mut range: Option<(u64, Option<u64>)> = None;
    // BYTERANGE 不写偏移时，接着同一个文件上一段的末尾
    let mut next_off: HashMap<String, u64> = HashMap::new();
    for l in text.lines().map(str::trim) {
        if let Some(v) = l.strip_prefix("#EXT-X-MEDIA-SEQUENCE:") {
            seq = v.trim().parse().unwrap_or(0);
        } else if let Some(v) = l.strip_prefix("#EXT-X-TARGETDURATION:") {
            m.target = v.trim().parse().ok();
        } else if let Some(v) = l.strip_prefix("#EXT-X-KEY:") {
            let a = attrs(v);
            let method = a.get("METHOD").cloned().unwrap_or_default();
            key = if method == "NONE" {
                None
            } else {
                let iv = a.get("IV").and_then(|s| {
                    let h = s.trim_start_matches("0x").trim_start_matches("0X");
                    let b = hex::decode(format!("{h:0>32}")).ok()?;
                    b.try_into().ok()
                });
                Some(Key { method, uri: a.get("URI").map(|u| join(base, u)), iv, keyformat: a.get("KEYFORMAT").cloned() })
            };
        } else if let Some(v) = l.strip_prefix("#EXT-X-MAP:") {
            let a = attrs(v);
            m.map = a.get("URI").map(|u| Init {
                uri: join(base, u),
                range: a.get("BYTERANGE").and_then(|r| {
                    let mut p = r.split('@');
                    let len = p.next()?.trim().parse().ok()?;
                    let off = p.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
                    Some((len, off))
                }),
            });
        } else if let Some(v) = l.strip_prefix("#EXTINF:") {
            dur = v.split(',').next().and_then(|d| d.trim().parse().ok());
        } else if let Some(v) = l.strip_prefix("#EXT-X-BYTERANGE:") {
            let mut p = v.split('@');
            let len = p.next().and_then(|x| x.trim().parse().ok()).unwrap_or(0);
            let off = p.next().and_then(|x| x.trim().parse().ok());
            range = Some((len, off));
        } else if l == "#EXT-X-ENDLIST" {
            m.endlist = true;
        } else if !l.is_empty() && !l.starts_with('#') {
            let uri = join(base, l);
            let range = range.take().map(|(len, off)| {
                let o = off.unwrap_or_else(|| next_off.get(&uri).copied().unwrap_or(0));
                next_off.insert(uri.clone(), o + len);
                (len, Some(o))
            });
            m.segments.push(Segment { uri, dur: dur.take().unwrap_or(0.0), seq, key: key.clone(), range });
            seq += 1;
        }
    }
    Ok(m)
}

/// 发请求的地方都带上 `jar`：插件或登录存储给的 cookies，按每个请求的地址挑（不会把网站的 cookie 发给别的 CDN）
async fn fetch_text(http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, jar: &Jar) -> Result<String> {
    let mut r = http.get(url);
    for (k, v) in headers {
        r = r.header(k, v);
    }
    r = jar.apply(r, url, headers);
    let resp = r.send().await.map_err(|e| errors::from_reqwest(&e, url))?;
    if !resp.status().is_success() {
        return Err(errors::http_status(resp.status().as_u16(), url).into());
    }
    Ok(resp.text().await.map_err(|e| errors::from_reqwest(&e, url))?)
}

fn live_failure(url: &str) -> Failure {
    Failure::new(ErrorCode::Live, tr!("这是正在进行的直播"), format!("M3U8 has no #EXT-X-ENDLIST: {url}"))
}

/// 加密方式、KEYFORMAT 是不是 DRM（Widevine、PlayReady、FairPlay、CENC 的 SAMPLE-AES）。AES-128 不算（自己能解）
pub fn drm_key(method: &str, keyformat: Option<&str>) -> bool {
    let m = method.to_ascii_uppercase();
    let kf = keyformat.unwrap_or("").to_ascii_lowercase();
    matches!(m.as_str(), "SAMPLE-AES" | "SAMPLE-AES-CTR" | "SAMPLE-AES-CENC" | "ISO-23001-7")
        || kf.contains("edef8ba9-79d6-4ace-a3c8-27dcd51d21ed")
        || kf.contains("9a04f079-9840-4286-ab92-e65be0885f95")
        || kf.contains("com.microsoft.playready")
        || kf.contains("com.apple.streamingkeydelivery")
        || kf.contains("com.widevine")
}

/// 分段列表或主列表里有 DRM 的密钥：读取时就说清楚，免得用户选了清晰度、点了下载才发现（FIXES 第 19 项）
fn drm_in(master: Option<&str>, media: &Media) -> Option<String> {
    for l in master.unwrap_or("").lines().map(str::trim) {
        if let Some(v) = l.strip_prefix("#EXT-X-SESSION-KEY:") {
            let a = attrs(v);
            if drm_key(a.get("METHOD").map(String::as_str).unwrap_or(""), a.get("KEYFORMAT").map(String::as_str)) {
                return Some(format!("EXT-X-SESSION-KEY: {v}"));
            }
        }
    }
    media.segments.iter().filter_map(|s| s.key.as_ref()).find(|k| drm_key(&k.method, k.keyformat.as_deref())).map(|k| format!("EXT-X-KEY: METHOD={} KEYFORMAT={}", k.method, k.keyformat.as_deref().unwrap_or("")))
}

fn drm_failure(url: &str, why: String) -> Failure {
    Failure::new(ErrorCode::Drm, tr!("这个视频受版权保护（DRM），Captae 不能下载"), format!("{why} ({url})"))
}

/// 读一个列表：是主列表就接着读第一路（码率最高的），返回分段列表。给直播判断、试看片段判断用
pub(crate) async fn media_of(http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>) -> Result<Media> {
    let jar = Jar::default();
    let text = fetch_text(http, url, headers, &jar).await?;
    if !is_master(&text) {
        return parse_media(&text, &Url::parse(url)?);
    }
    let vars = parse_master(&text, &Url::parse(url)?);
    let v = vars.first().ok_or_else(|| anyhow!("master playlist has no variants"))?;
    parse_media(&fetch_text(http, &v.uri, headers, &jar).await?, &Url::parse(&v.uri)?)
}

pub struct Probe {
    pub variants: Vec<Variant>,
    /// 只有一个清晰度时直接给出分片列表地址
    pub media_url: Option<String>,
    pub info: MediaInfo,
    /// 正在直播（分段列表没有结尾）。只有 `probe_live` 的 `record_live` 为真时才会读到
    pub live: bool,
}

pub async fn probe(http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, jar: &Jar) -> Result<Probe> {
    probe_live(http, url, headers, jar, false).await
}

/// 同 `probe`；`record_live` 为真时，正在直播的列表不报错，返回 `live: true`（从现在开始录）
pub async fn probe_live(http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, jar: &Jar, record_live: bool) -> Result<Probe> {
    let base = Url::parse(url)?;
    let text = fetch_text(http, url, headers, jar).await?;
    if !is_master(&text) {
        let m = parse_media(&text, &base)?;
        if let Some(why) = drm_in(None, &m) {
            return Err(drm_failure(url, why).into());
        }
        if !m.endlist && !record_live {
            return Err(live_failure(url).into());
        }
        let live = !m.endlist;
        let d = (!live).then(|| m.duration());
        return Ok(Probe { variants: vec![], media_url: Some(url.to_string()), info: MediaInfo { duration: d, live_status: live.then(|| "is_live".into()), ..Default::default() }, live });
    }
    let mut vars = parse_master(&text, &base);
    if vars.is_empty() {
        return Err(Failure::new(ErrorCode::NoMedia, tr!("没有找到可下载的清晰度"), format!("master playlist has no variants: {url}")).into());
    }
    // 读最高清晰度的分片列表来得到时长，用于估算各清晰度大小
    let top = fetch_text(http, &vars[0].uri, headers, jar).await?;
    let m = parse_media(&top, &Url::parse(&vars[0].uri)?)?;
    if let Some(why) = drm_in(Some(&text), &m) {
        return Err(drm_failure(url, why).into());
    }
    if !m.endlist && !record_live {
        return Err(live_failure(url).into());
    }
    let h = vars[0].p.trim_end_matches('p').parse().ok();
    if !m.endlist {
        // 直播没有总时长，各清晰度的大小估不出来（显示成 0）
        return Ok(Probe { variants: vars, media_url: None, info: MediaInfo { height: h, live_status: Some("is_live".into()), ..Default::default() }, live: true });
    }
    let d = m.duration();
    // 按原始码率算、留两位小数：原来先把码率舍到 0.1 Mbps、结果再取整到 MB，短片段两档会算成一样大（0.94 / 0.43 Mbps 的 10 秒都成了 1 MB）
    for v in &mut vars {
        v.mb = size_mb(v.bps, d);
    }
    Ok(Probe { variants: vars, media_url: None, info: MediaInfo { duration: Some(d), height: h, ..Default::default() }, live: false })
}

fn decrypt(data: &[u8], key: &[u8; 16], iv: &[u8; 16]) -> Result<Vec<u8>> {
    let mut buf = data.to_vec();
    let n = Aes128CbcDec::new(key.into(), iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|_| anyhow!(tr!("分片解密失败")))?
        .len();
    buf.truncate(n);
    Ok(buf)
}

fn seq_iv(seq: u64) -> [u8; 16] {
    let mut iv = [0u8; 16];
    iv[8..].copy_from_slice(&seq.to_be_bytes());
    iv
}

struct Shared {
    keys: Mutex<HashMap<String, [u8; 16]>>,
    bytes: Mutex<f64>,
    /// 最后一个一直没下好的分片的原因（错误分类用）
    last_err: Mutex<Option<String>>,
}

async fn fetch_segment(http: &reqwest::Client, seg: &Segment, headers: &BTreeMap<String, String>, jar: &Jar, sh: &Shared) -> Result<Vec<u8>> {
    let mut r = http.get(&seg.uri);
    for (k, v) in headers {
        r = r.header(k, v);
    }
    r = jar.apply(r, &seg.uri, headers);
    if let Some((len, off)) = seg.range {
        let o = off.unwrap_or(0);
        r = r.header("Range", format!("bytes={}-{}", o, o + len.saturating_sub(1)));
    }
    // 只限制「连上并拿到响应头」和「两次收到数据的间隔」，不限制整段下载的总时长：
    // 大分片在多路并发、网速一般时要下很久，总超时会把正常下载当成失败反复重试。
    const IDLE: std::time::Duration = std::time::Duration::from_secs(30);
    let resp = tokio::time::timeout(IDLE, r.send()).await.map_err(|_| anyhow!(tr!("连接超时")))??;
    if !resp.status().is_success() {
        bail!("HTTP {}", resp.status().as_u16());
    }
    let mut body = Vec::new();
    let mut stream = resp.bytes_stream();
    use futures_util::StreamExt;
    let mut counted = 0.0;
    let res: Result<()> = async {
        while let Some(c) = tokio::time::timeout(IDLE, stream.next()).await.map_err(|_| anyhow!(tr!("30 秒没有收到数据")))? {
            let c = c?;
            counted += c.len() as f64;
            *sh.bytes.lock().await += c.len() as f64;
            body.extend_from_slice(&c);
        }
        Ok(())
    }
    .await;
    if let Err(e) = res {
        // 这次没下完，已经算进进度里的字节退回去，重试时再算
        *sh.bytes.lock().await -= counted;
        return Err(e);
    }
    if let Some(k) = &seg.key {
        if k.method != "AES-128" {
            // 分片的报错最后按文字归类（seg_failure）：中文认「可能是 DRM」，各种语言都认得说法里的 SAMPLE-AES。
            // DRM 的密钥在读取信息时就拦下了（drm_in），走到这里的很少
            bail!("{}", tr!("不支持的加密方式 {method}（可能是 DRM 保护的视频）", method = k.method));
        }
        let ku = k.uri.clone().ok_or_else(|| anyhow!(tr!("缺少密钥地址")))?;
        let key = {
            let mut cache = sh.keys.lock().await;
            if let Some(k) = cache.get(&ku) {
                *k
            } else {
                let mut r = http.get(&ku);
                for (hk, hv) in headers {
                    r = r.header(hk, hv);
                }
                r = jar.apply(r, &ku, headers);
                let b = r.send().await?.bytes().await?;
                let arr: [u8; 16] = b.as_ref().try_into().map_err(|_| anyhow!(tr!("密钥长度不对")))?;
                cache.insert(ku.clone(), arr);
                arr
            }
        };
        body = decrypt(&body, &key, &k.iv.unwrap_or_else(|| seq_iv(seg.seq)))?;
    }
    Ok(body)
}

/// 下载一个分片列表里的所有分片，更新 job.segs[offset..]。返回每个分片的本地路径。
/// 第二个返回值：最后一个一直没下好的分片的原因（都下好了就是 None）。
async fn download_segments(ctx: &Ctx, media: &Media, dir: &Path, offset: usize, headers: &BTreeMap<String, String>, jar: &Jar) -> Result<(Vec<PathBuf>, Option<String>)> {
    tokio::fs::create_dir_all(dir).await?;
    let n = media.segments.len();
    let paths: Vec<PathBuf> = (0..n).map(|i| dir.join(format!("{i:05}.seg"))).collect();
    // 已有的分片直接算完成
    let mut todo = vec![];
    ctx.h.update(|j| {
        for (i, p) in paths.iter().enumerate() {
            if p.exists() {
                j.segs[offset + i] = SegState::Done;
            } else if j.segs[offset + i] != SegState::Failed {
                j.segs[offset + i] = SegState::Wait;
            }
        }
    });
    let snap = ctx.h.snapshot();
    for (i, p) in paths.iter().enumerate() {
        if !p.exists() && snap.segs[offset + i] != SegState::Failed {
            todo.push(i);
        }
    }
    let sh = Arc::new(Shared { keys: Mutex::new(HashMap::new()), bytes: Mutex::new(0.0), last_err: Mutex::new(None) });
    let sem = Arc::new(tokio::sync::Semaphore::new(parallel(ctx.settings.threads)));
    let mut set = tokio::task::JoinSet::new();
    for i in todo {
        let seg = media.segments[i].clone();
        let path = paths[i].clone();
        let (http, h, headers, jar, sh, sem) = (ctx.http.clone(), ctx.h.clone(), headers.clone(), jar.clone(), sh.clone(), sem.clone());
        set.spawn(async move {
            let _permit = sem.acquire_owned().await;
            h.update(|j| j.segs[offset + i] = SegState::Loading);
            let mut tries = h.snapshot().tries.get(offset + i).copied().unwrap_or(0);
            let mut last_err = String::new();
            loop {
                match fetch_segment(&http, &seg, &headers, &jar, &sh).await {
                    Ok(data) => {
                        let tmp = path.with_extension("part");
                        let ok = async {
                            let mut f = tokio::fs::File::create(&tmp).await?;
                            f.write_all(&data).await?;
                            f.flush().await?;
                            tokio::fs::rename(&tmp, &path).await
                        }
                        .await;
                        if ok.is_ok() {
                            h.update(|j| {
                                j.segs[offset + i] = SegState::Done;
                                if tries > 0 {
                                    j.log(tr!("重新下载成功"));
                                }
                            });
                            return;
                        }
                    }
                    Err(e) => {
                        tracing::debug!("segment {i} failed: {e}");
                        last_err = e.to_string();
                    }
                }
                tries += 1;
                if tries >= MAX_TRIES {
                    h.update(|j| {
                        j.segs[offset + i] = SegState::Failed;
                        j.tries[offset + i] = tries;
                        j.error(tr!("第 {index} 段一直下载失败（{reason}）", index = offset + i + 1, reason = last_err));
                    });
                    *sh.last_err.lock().await = Some(format!("{} → {last_err}", seg.uri));
                    return;
                }
                h.update(|j| {
                    j.segs[offset + i] = SegState::Retrying;
                    j.tries[offset + i] = tries;
                    j.warn(tr!("网络不稳，第 {index} 段没下好，正在自动重新下载（{reason}）", index = offset + i + 1, reason = last_err));
                });
                tokio::time::sleep(std::time::Duration::from_millis(800 * tries as u64)).await;
                h.update(|j| j.segs[offset + i] = SegState::Loading);
            }
        });
    }
    // 进度汇报
    let total_segs = ctx.h.snapshot().segs.len().max(1);
    let est_total = ctx.h.snapshot().size;
    let mut meter = SpeedMeter::default();
    let mut last_pct_logged = 0usize;
    let done0 = ctx.h.snapshot().segs.iter().filter(|s| **s == SegState::Done).count();
    loop {
        tokio::select! {
            r = set.join_next() => { if r.is_none() { break; } }
            _ = tokio::time::sleep(std::time::Duration::from_millis(400)) => {}
        }
        let b = *sh.bytes.lock().await;
        let sp = meter.push(b);
        ctx.h.update(|j| {
            let done = j.segs.iter().filter(|s| **s == SegState::Done).count();
            // 分片大的时候按分片数算太跳，把正在下的字节也算进来，但不超过估计大小
            let by_segs = est_total * done as f64 / total_segs as f64;
            let by_bytes = (est_total * done0 as f64 / total_segs as f64 + mb(b)).min(est_total * 0.99);
            j.got = if done == total_segs { est_total } else { by_segs.max(by_bytes) };
            j.speed = mb(sp);
            let pct = done * 100 / total_segs;
            if pct / 25 > last_pct_logged / 25 && pct < 100 {
                j.log(tr!("已下载 {percent}%", percent = pct / 25 * 25));
            }
            last_pct_logged = pct;
        });
    }
    let why = sh.last_err.lock().await.clone();
    Ok((paths, why))
}

/// 分片一直没下好 → 错误码：带 HTTP 状态码的（403、404……）按状态码，其他算网络问题（界面上有「再试一次」）。
fn seg_failure(why: &str, url: &str) -> Failure {
    static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| regex::Regex::new(r"HTTP (\d{3})").unwrap());
    if let Some(st) = RE.captures(why).and_then(|c| c[1].parse::<u16>().ok()) {
        let mut f = errors::http_status(st, url);
        f.raw = why.to_string();
        return f;
    }
    let f = errors::classify(why, url);
    match f.code {
        ErrorCode::Unknown | ErrorCode::Network => Failure::new(ErrorCode::Network, tr!("有一小部分一直没下载成功，可以再试一次"), why),
        _ => f,
    }
}

/// 分片按顺序写成一个文件。
async fn concat(paths: &[PathBuf], init: Option<&[u8]>, out: &Path) -> Result<()> {
    let mut f = tokio::fs::File::create(out).await?;
    if let Some(i) = init {
        f.write_all(i).await?;
    }
    for p in paths {
        let b = tokio::fs::read(p).await?;
        f.write_all(&b).await?;
    }
    f.flush().await?;
    Ok(())
}

/// 下载选中的清晰度并生成 MP4。
pub async fn download(ctx: &Ctx, media_url: &str, audio_url: Option<&str>, out: &Path) -> Result<Output> {
    let req = ctx.h.snapshot().req;
    let headers = req.headers.clone();
    let jar = ctx.auth.jar();
    let media = parse_media(&fetch_text(&ctx.http, media_url, &headers, &jar).await?, &Url::parse(media_url)?)?;
    if media.segments.is_empty() {
        return Err(Failure::new(ErrorCode::NoMedia, tr!("分片列表是空的"), format!("no segments: {media_url}")).into());
    }
    let audio = match audio_url {
        Some(u) => Some(parse_media(&fetch_text(&ctx.http, u, &headers, &jar).await?, &Url::parse(u)?)?),
        None => None,
    };
    let nv = media.segments.len();
    let na = audio.as_ref().map(|a| a.segments.len()).unwrap_or(0);
    ctx.h.update(|j| {
        if j.segs.len() != nv + na {
            j.segs = vec![SegState::Wait; nv + na];
            j.tries = vec![0; nv + na];
        }
        if j.dur.is_empty() {
            j.dur = clock(media.duration());
        }
    });
    let work = ctx.paths.work.join(format!("hls-{}", crate::util::url_key(media_url)));
    let (vpaths, verr) = download_segments(ctx, &media, &work.join("v"), 0, &headers, &jar).await?;
    let (apaths, aerr) = match &audio {
        Some(a) => {
            let (p, e) = download_segments(ctx, a, &work.join("a"), nv, &headers, &jar).await?;
            (Some(p), e)
        }
        None => (None, None),
    };
    let failed = ctx.h.snapshot().segs.iter().filter(|s| **s == SegState::Failed).count();
    if failed > 0 {
        return Err(seg_failure(&aerr.or(verr).unwrap_or_default(), media_url).into());
    }

    // 合并
    ctx.h.update(|j| {
        j.phase = crate::model::Phase::Merge;
        j.merge = 0.0;
        j.speed = 0.0;
        j.log(tr!("下载完成，正在生成视频文件"));
    });
    let init = match &media.map {
        Some(u) => Some(fetch_init(&ctx.http, u, &headers, &jar).await?),
        None => None,
    };
    let joined_v = work.join(if media.map.is_some() { "video.mp4" } else { "video.ts" });
    concat(&vpaths, init.as_deref(), &joined_v).await?;
    let joined_a = if let (Some(a), Some(p)) = (&audio, &apaths) {
        let init = match &a.map {
            Some(u) => Some(fetch_init(&ctx.http, u, &headers, &jar).await?),
            None => None,
        };
        let jp = work.join(if a.map.is_some() { "audio.mp4" } else { "audio.ts" });
        concat(p, init.as_deref(), &jp).await?;
        Some(jp)
    } else {
        None
    };
    ctx.h.update(|j| j.merge = 30.0);

    // 没有 ffmpeg（手机上，看 Runner）：用 Rust 封装（engine/remux.rs），不转码
    if !ctx.tools.has(Tool::Ffmpeg) {
        let h = ctx.h.clone();
        super::remux::remux_async(joined_v.clone(), joined_a.clone(), out.to_path_buf(), work.join("remux"), move |p| {
            h.update(|j| j.merge = 30.0 + p * 70.0);
        })
        .await?;
        ctx.h.update(|j| j.merge = 100.0);
        let _ = tokio::fs::remove_dir_all(&work).await;
        return Ok(Output { path: out.to_path_buf(), format_done: true });
    }
    let total = media.duration();
    let mut c = ctx.tools.command(Tool::Ffmpeg)?;
    c.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-progress", "pipe:1", "-i"]).arg(&joined_v);
    if let Some(a) = &joined_a {
        c.arg("-i").arg(a).args(["-map", "0:v:0", "-map", "1:a:0"]);
    }
    c.args(["-c", "copy", "-bsf:a", "aac_adtstoasc", "-movflags", "+faststart"]).arg(out);
    c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped());
    let mut child = crate::tools::spawn(&mut c)?;
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    while let Some(l) = lines.next_line().await? {
        if let Some(t) = crate::ffmpeg::parse_progress_time(&l) {
            let pct = if total > 0.0 { (t / total).min(1.0) } else { 0.5 };
            ctx.h.update(|j| j.merge = 30.0 + pct * 70.0);
        }
    }
    let st = child.wait().await?;
    if !st.success() {
        let mut e = String::new();
        use tokio::io::AsyncReadExt;
        if let Some(mut s) = child.stderr.take() {
            let _ = s.read_to_string(&mut e).await;
        }
        // 有的流 aac_adtstoasc 不适用，去掉再试一次
        let mut c = ctx.tools.command(Tool::Ffmpeg)?;
        c.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-i"]).arg(&joined_v);
        if let Some(a) = &joined_a {
            c.arg("-i").arg(a).args(["-map", "0:v:0", "-map", "1:a:0"]);
        }
        c.args(["-c", "copy", "-movflags", "+faststart"]).arg(out);
        let r = crate::tools::run_full(c, std::time::Duration::from_secs(3600)).await?;
        if !r.ok {
            return Err(errors::classify_ffmpeg(&format!("{e}\n{}", r.stderr), media_url).into());
        }
    }
    ctx.h.update(|j| j.merge = 100.0);
    let _ = tokio::fs::remove_dir_all(&work).await;
    Ok(Output { path: out.to_path_buf(), format_done: true })
}

// ------------------------------------------------------------------ 录直播（插件嗅探到的直播列表、粘贴的直播 M3U8）

/// 直播的一路（画面，或者单独一路的声音）：接到第几段了、写到哪个文件
struct LiveTrack {
    url: String,
    /// `<目录>/video` 或 `<目录>/audio`，第一次读到列表时按有没有初始化分段补上扩展名（.mp4 / .ts）
    stem: PathBuf,
    file: Option<PathBuf>,
    /// 下一段的序号（`#EXT-X-MEDIA-SEQUENCE` 起算）；还没开始时是 None
    next: Option<u64>,
    bytes: u64,
    /// 列表出现了 `#EXT-X-ENDLIST`：直播结束了
    ended: bool,
    /// 每段最长几秒
    target: f64,
    /// 连着几段没下到（网络断了、地址过期了）
    misses: u32,
}

impl LiveTrack {
    fn new(url: &str, stem: PathBuf) -> LiveTrack {
        LiveTrack { url: url.to_string(), stem, file: None, next: None, bytes: 0, ended: false, target: 6.0, misses: 0 }
    }

    /// 读一次列表，把新出现的分段按顺序接在文件后面。返回这次接上了几段。
    /// 点了「停止并保存」（`ctx.finish`）：正在下的那一段不要了，已经接上的都是完整的分段（不会在写到一半时被打断）
    async fn poll(&mut self, ctx: &Ctx, headers: &BTreeMap<String, String>, jar: &Jar, sh: &Shared) -> Result<usize> {
        let text = tokio::select! {
            r = fetch_text(&ctx.http, &self.url, headers, jar) => r?,
            _ = ctx.finish.cancelled() => return Ok(0),
        };
        let m = parse_media(&text, &Url::parse(&self.url)?)?;
        self.target = m.target.or_else(|| m.segments.last().map(|s| s.dur)).filter(|t| *t > 0.0).unwrap_or(6.0);
        self.ended = m.endlist;
        let (Some(first), Some(last)) = (m.segments.first().map(|s| s.seq), m.segments.last().map(|s| s.seq)) else { return Ok(0) };
        let file = match &self.file {
            Some(f) => f.clone(),
            None => {
                let f = self.stem.with_extension(if m.map.is_some() { "mp4" } else { "ts" });
                let mut out = tokio::fs::File::create(&f).await?;
                // fMP4：先写一次初始化分段，后面的分段直接接上就是一个能播放的分段式 MP4
                if let Some(init) = &m.map {
                    out.write_all(&fetch_init(&ctx.http, init, headers, jar).await?).await?;
                    out.flush().await?;
                }
                self.file = Some(f.clone());
                f
            }
        };
        // 第一次从最新的 3 段接起（和 ffmpeg、yt-dlp 录直播一样，留一点余量，不会一开始就等）；
        // 直播重新开始、序号从头算了（新列表的最后一段比要接的还早很多）：也从最新的 3 段重新接
        let start = match self.next {
            Some(n) if n <= last + 1 || n - last <= 30 => n,
            _ => m.segments[m.segments.len().saturating_sub(3)].seq,
        };
        if self.next.is_some() && start < first {
            let lost = first - start;
            ctx.h.update(|j| j.warn(tr!("网络慢了一点，直播往前走了，中间有 {n} 段没来得及录", n = lost)));
        }
        let mut n = 0;
        let mut f = tokio::fs::OpenOptions::new().append(true).open(&file).await?;
        for seg in m.segments.iter().filter(|s| s.seq >= start) {
            let got = tokio::select! {
                r = fetch_retry(&ctx.http, seg, headers, jar, sh) => r,
                _ = ctx.finish.cancelled() => break,
            };
            match got {
                Ok(data) => {
                    f.write_all(&data).await?;
                    self.bytes += data.len() as u64;
                    self.misses = 0;
                    n += 1;
                }
                Err(e) => {
                    self.misses += 1;
                    let why = e.to_string();
                    ctx.h.update(|j| j.warn(tr!("直播的一段一直没下到，跳过（{reason}）", reason = why)));
                    if self.misses >= 10 {
                        return Err(seg_failure(&format!("{} → {why}", seg.uri), &self.url).into());
                    }
                }
            }
            self.next = Some(seg.seq + 1);
        }
        f.flush().await?;
        Ok(n)
    }
}

/// 录直播：每隔一会儿重新读分段列表，新出现的分段按顺序接在后面，直到点了「停止并保存」（`ctx.finish`）或者直播结束（列表出现 ENDLIST）。
/// 接起来的原始数据在 `<数据目录>/work/hls-rec-…/`（Captae 中途退出时，下次打开从这里整理），最后原样封装成 `out` 旁边一个能播放的文件
/// （扩展名按内容定，见 `ffmpeg::tidy_recording`）。网络断了、地址过期了：录到了东西就照样保存，什么都没录到才算失败
pub async fn record(ctx: &Ctx, media_url: &str, audio_url: Option<&str>, out: &Path) -> Result<Output> {
    let req = ctx.h.snapshot().req;
    let headers = req.headers.clone();
    let jar = ctx.auth.jar();
    let work = ctx.paths.work.join(format!("hls-rec-{}-{}", crate::util::url_key(media_url), crate::util::now_ms()));
    tokio::fs::create_dir_all(&work).await?;
    ctx.note_raw(&work);
    let sh = Shared { keys: Mutex::new(HashMap::new()), bytes: Mutex::new(0.0), last_err: Mutex::new(None) };
    let mut v = LiveTrack::new(media_url, work.join("video"));
    let mut a = audio_url.map(|u| LiveTrack::new(u, work.join("audio")));
    let mut meter = SpeedMeter::default();
    let mut fails = 0u32;
    let mut broke: Option<anyhow::Error> = None;
    let mut stopped = false;
    loop {
        let step = async {
            let mut n = v.poll(ctx, &headers, &jar, &sh).await?;
            if let Some(a) = a.as_mut() {
                n += a.poll(ctx, &headers, &jar, &sh).await?;
            }
            anyhow::Ok(n)
        };
        // 不在这里打断：poll 自己在两段之间看「停止」，写进文件的都是完整的分段
        let got = step.await;
        if ctx.finish.is_cancelled() {
            stopped = true;
            break;
        }
        let n = match got {
            Ok(n) => {
                fails = 0;
                n
            }
            Err(e) => {
                // 列表一时读不到（网络抖动）：过一会儿再读，连着 5 次读不到才算断了
                fails += 1;
                if fails >= 5 {
                    broke = Some(e);
                    break;
                }
                0
            }
        };
        let total = v.bytes + a.as_ref().map(|a| a.bytes).unwrap_or(0);
        let sp = meter.push(total as f64);
        ctx.h.update(|j| {
            j.got = mb(total as f64);
            j.speed = mb(sp);
        });
        if v.ended && a.as_ref().is_none_or(|a| a.ended) {
            ctx.h.update(|j| j.log(tr!("直播结束了")));
            break;
        }
        // 有新分段：过一段的时长再读；没有：过半段再读（HLS 规范的建议），最少 1 秒、最多 10 秒
        let wait = if n > 0 { v.target } else { v.target / 2.0 }.clamp(1.0, 10.0);
        tokio::select! {
            _ = tokio::time::sleep(std::time::Duration::from_secs_f64(wait)) => {}
            _ = ctx.finish.cancelled() => { stopped = true; break; }
        }
    }
    let total = v.bytes + a.as_ref().map(|a| a.bytes).unwrap_or(0);
    if total == 0 {
        let _ = tokio::fs::remove_dir_all(&work).await;
        return Err(broke.unwrap_or_else(|| anyhow!(tr!("还没有录到内容"))));
    }
    if let Some(e) = &broke {
        let f = errors::from_anyhow(e, media_url);
        ctx.h.update(|j| j.warn(tr!("录制中断了（{reason}），录到的部分照样保存", reason = f.message)));
    }
    // 点了停止的话 act(Stop) 已经记过；直播自己结束、网络断了的这里记上（会重复记，没关系）
    ctx.note_stopping();
    if !stopped && broke.is_none() {
        tracing::debug!("直播的列表出现了结尾，录制结束");
    }
    ctx.h.update(|j| j.log(tr!("正在把录到的部分整理成能播放的文件")));
    let dir = out.parent().unwrap_or(Path::new("."));
    let stem = out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let vfile = v.file.clone().ok_or_else(|| anyhow!(tr!("还没有录到内容")))?;
    let afile = a.as_ref().and_then(|a| a.file.clone()).filter(|_| a.as_ref().is_some_and(|a| a.bytes > 0));
    let path = crate::ffmpeg::tidy_recording(&ctx.tools, &vfile, afile.as_deref(), dir, &stem).await?;
    let _ = tokio::fs::remove_dir_all(&work).await;
    Ok(Output { path, format_done: true })
}

/// 录直播的中间目录里录到的部分（Captae 在录制中途退出、下次打开时整理用）：画面那一路和单独一路的声音
pub fn recorded_tracks(dir: &Path) -> (Option<PathBuf>, Option<PathBuf>) {
    let pick = |stem: &str| ["mp4", "ts"].iter().map(|e| dir.join(format!("{stem}.{e}"))).find(|p| std::fs::metadata(p).is_ok_and(|m| m.len() > 0));
    (pick("video"), pick("audio"))
}

// ------------------------------------------------------------------ 只下一小段（命令行 --sample）

async fn fetch_retry(http: &reqwest::Client, seg: &Segment, headers: &BTreeMap<String, String>, jar: &Jar, sh: &Shared) -> Result<Vec<u8>> {
    let mut tries = 0u8;
    loop {
        match fetch_segment(http, seg, headers, jar, sh).await {
            Ok(b) => return Ok(b),
            Err(e) => {
                tries += 1;
                if tries >= MAX_TRIES {
                    return Err(e);
                }
                tokio::time::sleep(std::time::Duration::from_millis(800 * tries as u64)).await;
            }
        }
    }
}

async fn fetch_init(http: &reqwest::Client, init: &Init, headers: &BTreeMap<String, String>, jar: &Jar) -> Result<Vec<u8>> {
    let url = init.uri.as_str();
    let mut r = http.get(url);
    for (k, v) in headers {
        r = r.header(k, v);
    }
    r = jar.apply(r, url, headers);
    if let Some((len, off)) = init.range {
        r = r.header("Range", format!("bytes={}-{}", off, off + len.saturating_sub(1)));
    }
    let resp = r.send().await.map_err(|e| errors::from_reqwest(&e, url))?;
    if !resp.status().is_success() {
        return Err(errors::http_status(resp.status().as_u16(), url).into());
    }
    let mut b = resp.bytes().await?.to_vec();
    // 服务器不认 Range、回了整个文件（200）：只留要的那一段
    if let Some((len, off)) = init.range {
        if b.len() as u64 > len {
            let start = (off as usize).min(b.len());
            let end = (start + len as usize).min(b.len());
            b = b[start..end].to_vec();
        }
    }
    Ok(b)
}

/// 下前面的分片，攒够 `max_bytes`（或下完时长 `max_secs`）就停；返回下了多少秒。
async fn sample_track(http: &reqwest::Client, url: &str, headers: &BTreeMap<String, String>, jar: &Jar, max_bytes: u64, max_secs: Option<f64>, out: &Path) -> Result<f64> {
    let media = parse_media(&fetch_text(http, url, headers, jar).await?, &Url::parse(url)?)?;
    if media.segments.is_empty() {
        return Err(Failure::new(ErrorCode::NoMedia, tr!("分片列表是空的"), format!("no segments: {url}")).into());
    }
    let sh = Shared { keys: Mutex::new(HashMap::new()), bytes: Mutex::new(0.0), last_err: Mutex::new(None) };
    let mut f = tokio::fs::File::create(out).await?;
    if let Some(m) = &media.map {
        f.write_all(&fetch_init(http, m, headers, jar).await?).await?;
    }
    let (mut got, mut secs) = (0u64, 0.0);
    for seg in &media.segments {
        let b = fetch_retry(http, seg, headers, jar, &sh).await.map_err(|e| seg_failure(&format!("{} → {e:#}", seg.uri), url))?;
        got += b.len() as u64;
        secs += seg.dur;
        f.write_all(&b).await?;
        let enough = match max_secs {
            Some(t) => secs >= t,
            None => got >= max_bytes,
        };
        if enough {
            break;
        }
    }
    f.flush().await?;
    Ok(secs)
}

/// 只下开头一小段，封装成 MP4（命令行 `captae download --sample`，批量测试用）。
/// 视频按大小截（够 `max_bytes` 就停），单独的声音列表按视频的时长截。
pub async fn sample(http: &reqwest::Client, tools: &crate::tools::Toolbox, media_url: &str, audio_url: Option<&str>, headers: &BTreeMap<String, String>, max_bytes: u64, work: &Path, out: &Path) -> Result<()> {
    tokio::fs::create_dir_all(work).await?;
    // 命令行的样片：没有登录状态
    let jar = &Jar::default();
    let probe_media = parse_media(&fetch_text(http, media_url, headers, jar).await?, &Url::parse(media_url)?)?;
    let ext = if probe_media.map.is_some() { "mp4" } else { "ts" };
    let v = work.join(format!("video.{ext}"));
    let secs = sample_track(http, media_url, headers, jar, max_bytes, None, &v).await?;
    let a = match audio_url {
        Some(u) => {
            let p = work.join("audio.part");
            sample_track(http, u, headers, jar, u64::MAX, Some(secs), &p).await?;
            Some(p)
        }
        None => None,
    };
    let run = |bsf: bool| {
        let mut c = tools.command(Tool::Ffmpeg)?;
        c.args(["-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-i"]).arg(&v);
        if let Some(a) = &a {
            c.arg("-i").arg(a).args(["-map", "0:v:0", "-map", "1:a:0"]);
        }
        c.args(["-c", "copy"]);
        if bsf {
            c.args(["-bsf:a", "aac_adtstoasc"]);
        }
        c.args(["-movflags", "+faststart"]).arg(out);
        anyhow::Ok(c)
    };
    let r = crate::tools::run_full(run(true)?, std::time::Duration::from_secs(600)).await?;
    if !r.ok {
        let r2 = crate::tools::run_full(run(false)?, std::time::Duration::from_secs(600)).await?;
        if !r2.ok {
            return Err(errors::classify_ffmpeg(&format!("{}\n{}", r.stderr, r2.stderr), media_url).into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_sizes_follow_bitrate() {
        // iOS 实测的主列表：10 秒，两档 0.94 / 0.43 Mbps。原来都估成 1 MB
        let t = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=998656,AVERAGE-BANDWIDTH=939248,RESOLUTION=640x360\n0.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=469248,AVERAGE-BANDWIDTH=434656,RESOLUTION=320x180\n1.m3u8\n";
        let v = parse_master(t, &Url::parse("http://h/m.m3u8").unwrap());
        let (a, b) = (size_mb(v[0].bps, 10.0), size_mb(v[1].bps, 10.0));
        assert_eq!((a, b), (1.17, 0.54));
        assert!(a > b);
        // 给人看的码率照旧一位小数
        assert_eq!((v[0].b, v[1].b), (0.9, 0.4));
    }

    #[test]
    fn segment_failures() {
        let f = seg_failure("https://cdn.a.com/s1.ts → HTTP 403", "https://cdn.a.com/index.m3u8");
        assert_eq!(f.code, ErrorCode::NeedVerify);
        assert!(f.raw.contains("s1.ts"));
        assert_eq!(seg_failure("https://cdn.a.com/s1.ts → HTTP 404", "https://cdn.a.com/index.m3u8").code, ErrorCode::Unavailable);
        let f = seg_failure("https://cdn.a.com/s1.ts → 30 秒没有收到数据", "https://cdn.a.com/index.m3u8");
        assert_eq!((f.code, f.message.as_str()), (ErrorCode::Network, "有一小部分一直没下载成功，可以再试一次"));
        assert_eq!(seg_failure("不支持的加密方式 SAMPLE-AES（可能是 DRM 保护的视频）", "https://a/x.m3u8").code, ErrorCode::Drm);
        let base = Url::parse("https://a.com/x.m3u8").unwrap();
        let e = parse_media("<html>login</html>", &base).unwrap_err();
        assert_eq!(errors::failure_in(&e).unwrap().code, ErrorCode::NoMedia);
    }

    const MASTER: &str = "#EXTM3U
#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"aud\",NAME=\"en\",DEFAULT=YES,URI=\"audio/en.m3u8\"
#EXT-X-STREAM-INF:BANDWIDTH=4500000,RESOLUTION=1920x1080,AUDIO=\"aud\"
1080/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=800000,RESOLUTION=640x360
360/index.m3u8
#EXT-X-STREAM-INF:BANDWIDTH=2500000,RESOLUTION=1280x720,CODECS=\"avc1.4d401f,mp4a.40.2\"
http://cdn.example.com/720/index.m3u8
";
    #[test]
    fn master() {
        let base = Url::parse("https://a.com/x/master.m3u8").unwrap();
        let v = parse_master(MASTER, &base);
        assert_eq!(v.iter().map(|v| v.p.as_str()).collect::<Vec<_>>(), ["1080p", "720p", "360p"]);
        assert_eq!(v[0].uri, "https://a.com/x/1080/index.m3u8");
        assert_eq!(v[0].audio.as_deref(), Some("https://a.com/x/audio/en.m3u8"));
        assert_eq!(v[0].r, "1920×1080");
        assert_eq!(v[1].uri, "http://cdn.example.com/720/index.m3u8");
        assert_eq!(v[0].b, 4.5);
    }
    #[test]
    fn media() {
        let t = "#EXTM3U
#EXT-X-TARGETDURATION:10
#EXT-X-MEDIA-SEQUENCE:7
#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\",IV=0x0000000000000000000000000000000A
#EXTINF:9.009,
seg7.ts
#EXTINF:3.5,
seg8.ts
#EXT-X-KEY:METHOD=NONE
#EXTINF:1,
seg9.ts
#EXT-X-ENDLIST
";
        let m = parse_media(t, &Url::parse("https://a.com/p/index.m3u8").unwrap()).unwrap();
        assert_eq!(m.segments.len(), 3);
        assert!(m.endlist);
        assert_eq!(m.segments[0].seq, 7);
        assert_eq!(m.segments[1].uri, "https://a.com/p/seg8.ts");
        let k = m.segments[0].key.as_ref().unwrap();
        assert_eq!(k.uri.as_deref(), Some("https://a.com/p/key.bin"));
        assert_eq!(k.iv.unwrap()[15], 10);
        assert!(m.segments[2].key.is_none());
        assert!((m.duration() - 13.509).abs() < 1e-9);
    }
    #[test]
    fn aes_roundtrip() {
        use aes::cipher::BlockEncryptMut;
        type Enc = cbc::Encryptor<aes::Aes128>;
        let key = [7u8; 16];
        let iv = seq_iv(3);
        let plain = b"hello captae segment data".to_vec();
        let mut buf = plain.clone();
        buf.resize(32, 0);
        let ct = Enc::new(&key.into(), &iv.into()).encrypt_padded_mut::<Pkcs7>(&mut buf, plain.len()).unwrap().to_vec();
        assert_eq!(decrypt(&ct, &key, &iv).unwrap(), plain);
    }
    /// apple-da3434（bipbop_adv_example_hevc 的 v2/prog_index.m3u8 开头，2026-11-24 抓的）：整个视频是一个 main.mp4，
    /// 初始化分片和每一段都是字节范围
    #[test]
    fn byterange_init() {
        let t = "#EXTM3U
#EXT-X-TARGETDURATION:8
#EXT-X-VERSION:7
#EXT-X-MEDIA-SEQUENCE:1
#EXT-X-PLAYLIST-TYPE:VOD
#EXT-X-INDEPENDENT-SEGMENTS
#EXT-X-MAP:URI=\"main.mp4\",BYTERANGE=\"1118@0\"
#EXTINF:7.13333,\t
#EXT-X-BYTERANGE:283270@1118
main.mp4
#EXTINF:8.00000,\t
#EXT-X-BYTERANGE:315608@284388
main.mp4
#EXTINF:8.00000,\t
#EXT-X-BYTERANGE:312406
main.mp4
#EXT-X-ENDLIST
";
        let m = parse_media(t, &Url::parse("https://devstreaming-cdn.apple.com/videos/streaming/examples/bipbop_adv_example_hevc/v2/prog_index.m3u8").unwrap()).unwrap();
        let init = m.map.clone().unwrap();
        assert_eq!(init.uri, "https://devstreaming-cdn.apple.com/videos/streaming/examples/bipbop_adv_example_hevc/v2/main.mp4");
        assert_eq!(init.range, Some((1118, 0)), "初始化分片只是开头 1118 字节，不是整个文件");
        assert_eq!(m.segments[0].range, Some((283270, Some(1118))));
        // 不写偏移：接着同一个文件上一段的末尾（284388 + 315608）
        assert_eq!(m.segments[2].range, Some((312406, Some(599996))));
        assert!((m.duration() - 23.13333).abs() < 1e-6);
        // 没有 BYTERANGE 的 MAP 照旧
        let m = parse_media("#EXTM3U\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:4,\ns1.m4s\n#EXT-X-ENDLIST\n", &Url::parse("https://a.com/x/i.m3u8").unwrap()).unwrap();
        assert_eq!(m.map, Some(Init { uri: "https://a.com/x/init.mp4".into(), range: None }));
    }

    /// googleapis-30c1e9（Shaka 的 angel-one-widevine-hls，2026-11-24 抓的）：前两段不加密，后面是 Widevine
    #[test]
    fn widevine_is_drm() {
        let t = "#EXTM3U
#EXT-X-VERSION:6
#EXT-X-TARGETDURATION:5
#EXT-X-PLAYLIST-TYPE:VOD
#EXT-X-MAP:URI=\"v-0576p-1400k-libx264-init.mp4\"
#EXTINF:4.000,
v-0576p-1400k-libx264-s1.mp4
#EXTINF:4.000,
v-0576p-1400k-libx264-s2.mp4
#EXT-X-DISCONTINUITY
#EXT-X-KEY:METHOD=SAMPLE-AES-CTR,URI=\"data:text/plain;base64,AAAAPnBzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAAB4iFnNoYWthX2NlYzJmNjRhYTc4OTBhMTFI49yVmwY=\",KEYID=0x800AACAA522958AE888062B5695DB6BF,KEYFORMATVERSIONS=\"1\",KEYFORMAT=\"urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed\"
#EXTINF:4.000,
v-0576p-1400k-libx264-s3.mp4
#EXT-X-ENDLIST
";
        let m = parse_media(t, &Url::parse("https://storage.googleapis.com/shaka-demo-assets/angel-one-widevine-hls/playlist_v-0576p-1400k-libx264.mp4.m3u8").unwrap()).unwrap();
        assert!(drm_in(None, &m).is_some_and(|w| w.contains("SAMPLE-AES-CTR")));
        // AES-128 自己能解，不算 DRM
        let aes = parse_media("#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"k.bin\"\n#EXTINF:4,\ns1.ts\n#EXT-X-ENDLIST\n", &Url::parse("https://a.com/x.m3u8").unwrap()).unwrap();
        assert!(drm_in(None, &aes).is_none());
        // 主列表里的 SESSION-KEY（FairPlay）
        let master = "#EXTM3U\n#EXT-X-SESSION-KEY:METHOD=SAMPLE-AES,URI=\"skd://x\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n#EXT-X-STREAM-INF:BANDWIDTH=1\nv.m3u8\n";
        assert!(drm_in(Some(master), &Media::default()).is_some());
        assert!(drm_key("SAMPLE-AES", Some("identity")) && !drm_key("AES-128", Some("identity")) && !drm_key("NONE", None));
        assert_eq!(drm_failure("https://a/x.m3u8", "x".into()).code, ErrorCode::Drm);
    }

    /// 竖屏的清晰度按短边叫（插件发现：B 站竖屏列成「1920p、854p、640p」）
    #[test]
    fn vertical_labels() {
        let t = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=4000000,RESOLUTION=1080x1920\nhi.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=1000000,RESOLUTION=480x854\nmid.m3u8\n#EXT-X-STREAM-INF:BANDWIDTH=500000,RESOLUTION=360x640\nlo.m3u8\n";
        let v = parse_master(t, &Url::parse("https://a.com/m.m3u8").unwrap());
        assert_eq!(v.iter().map(|v| v.p.as_str()).collect::<Vec<_>>(), ["1080p", "480p", "360p"]);
        assert_eq!(v[0].r, "1080×1920");
    }

    #[test]
    fn attr_parsing() {
        let a = attrs("BANDWIDTH=1,CODECS=\"a,b\",RESOLUTION=1x2");
        assert_eq!(a["CODECS"], "a,b");
        assert_eq!(a["RESOLUTION"], "1x2");
    }
}
