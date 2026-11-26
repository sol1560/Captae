//! 电脑推来的 JSON（桌面端 `Job`、素材、读取信息的样子，见 apps/desktop/src/lib/types.ts）→ 手机这边的类型；
//! captae-remote 客户端的错误 → `MobileError`（固定代码 + 当前语言的说法）。

use crate::types::{bytes, MobileFailure};
use crate::{MobileAsset, MobileEntry, MobileError, MobileJob, MobileJobState, MobileLogLine, MobilePlaylist, MobileProbe, MobileQuality, MobileVariant};
use captae_core::errors::{self, Failure};
use captae_core::{i18n, tr};
use captae_remote::client::{self, RemoteError};
use captae_remote::proto;
use serde_json::Value;

fn s(v: &Value, k: &str) -> String {
    v[k].as_str().unwrap_or_default().to_string()
}

fn os(v: &Value, k: &str) -> Option<String> {
    v[k].as_str().filter(|x| !x.is_empty()).map(str::to_string)
}

fn b(v: &Value, k: &str) -> bool {
    v[k].as_bool().unwrap_or(false)
}

fn f(v: &Value, k: &str) -> f64 {
    v[k].as_f64().unwrap_or(0.0)
}

fn oi(v: &Value, k: &str) -> Option<i64> {
    v[k].as_i64()
}

fn ou32(v: &Value, k: &str) -> Option<u32> {
    v[k].as_u64().map(|n| n.min(u32::MAX as u64) as u32)
}

fn entry(e: &Value) -> MobileEntry {
    MobileEntry { id: s(e, "id"), title: s(e, "title"), duration: e["dur"].as_f64(), thumb: os(e, "thumb"), url: s(e, "url"), index: ou32(e, "n").unwrap_or(0) }
}

fn playlist(p: &Value) -> Option<MobilePlaylist> {
    p.is_object().then(|| MobilePlaylist { url: s(p, "url"), count: ou32(p, "count"), index: ou32(p, "index") })
}

/// 任务里的 `fail`（`Failure::to_json` 的样子）→ 失败详情。网站名按手机当前的语言翻译；说法是电脑那边生成的（电脑的语言）
pub fn failure(v: &Value) -> Option<MobileFailure> {
    if !v.is_object() {
        return None;
    }
    serde_json::from_value::<Failure>(v.clone()).ok().map(|f| MobileFailure::from_failure(&f))
}

/// 电脑上的一个任务 → `MobileJob`（电脑上的路径本来就不给手机，`path` 恒为空）。认不出的返回 None
pub fn job(v: &Value) -> Option<MobileJob> {
    let id = v["id"].as_u64()?;
    let state = match v["state"].as_str()? {
        "queue" => MobileJobState::Queued,
        "run" => MobileJobState::Running,
        "pause" => MobileJobState::Paused,
        "done" => MobileJobState::Done,
        "failed" => MobileJobState::Failed,
        _ => return None,
    };
    let phase = s(v, "phase");
    let rec = &v["rec"];
    // 和 `Job::recording` 一样：在跑、在下、开始录了还没停
    let recording = state == MobileJobState::Running && phase == "dl" && rec.is_object() && rec["until"].is_null() && !b(rec, "stopping");
    let fail = failure(&v["fail"]).or_else(|| {
        os(v, "err").map(|e| MobileFailure::from_failure(&Failure::new(errors::ErrorCode::Unknown, e.clone(), e)))
    });
    Some(MobileJob {
        id,
        title: s(v, "title"),
        untitled: b(v, "untitled"),
        src: s(v, "src"),
        url: s(v, "url"),
        plat: os(v, "plat"),
        engine: s(v, "eng"),
        format: s(v, "fmt"),
        duration: s(v, "dur"),
        thumb: os(v, "thumb"),
        audio_only: b(v, "audio"),
        state,
        phase,
        total_bytes: bytes(f(v, "size")),
        got_bytes: bytes(f(v, "got")),
        speed: bytes(f(v, "speed")),
        merge_percent: f(v, "merge"),
        choose: b(v, "choose"),
        entries: v["entries"].as_array().map(|a| a.iter().map(entry).collect()).unwrap_or_default(),
        playlist: playlist(&v["playlist"]),
        variants: v["vars"]
            .as_array()
            .map(|a| a.iter().map(|x| MobileVariant { resolution: s(x, "r"), label: s(x, "p"), mbps: f(x, "b"), bytes: bytes(f(x, "mb")) }).collect())
            .unwrap_or_default(),
        variant_index: ou32(v, "vIdx").unwrap_or(0),
        pick_until: oi(v, "pickUntil"),
        segments: v["segs"].as_array().map(|a| a.iter().map(|x| x.as_u64().unwrap_or(0).min(4) as u8).collect()).unwrap_or_default(),
        live: b(v, "live"),
        recording,
        record_since: rec["since"].as_i64(),
        record_max_secs: rec["max"].as_u64(),
        probe_at: oi(v, "probeAt"),
        done_at: oi(v, "doneAt"),
        retry_at: oi(v, "retryAt"),
        path: None,
        asset_id: oi(v, "assetId"),
        failure: fail,
    })
}

/// 任务是从哪台手机加的
pub fn job_from(v: &Value) -> Option<String> {
    os(v, "from")
}

/// 电脑上任务的记录（`log`：`[时间, 文字, 级别]`）
pub fn job_log(v: &Value) -> Vec<MobileLogLine> {
    v["log"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|l| {
                    let l = l.as_array()?;
                    Some(MobileLogLine { at: l.first()?.as_i64()?, text: l.get(1)?.as_str()?.to_string(), level: l.get(2).and_then(Value::as_str).unwrap_or_default().to_string() })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// 电脑上的一条素材（`path` 去掉了，封面只说有没有：`hasThumb`）。`thumb` 填手机上缓存好的封面（有的话）
pub fn asset(v: &Value, thumb: Option<String>) -> Option<MobileAsset> {
    Some(MobileAsset {
        id: v["id"].as_i64()?,
        kind: s(v, "type"),
        title: s(v, "title"),
        description: s(v, "desc"),
        url: s(v, "url"),
        page_url: s(v, "page"),
        site: i18n::dynamic(v["src"].as_str().unwrap_or_default()),
        uploader: s(v, "uploader"),
        path: String::new(),
        format: s(v, "fmt"),
        width: ou32(v, "width"),
        height: ou32(v, "height"),
        duration: v["durS"].as_f64(),
        bytes: v["bytes"].as_u64().unwrap_or(0),
        thumb,
        tags: v["tags"].as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        created_at: v["created_at"].as_i64().unwrap_or(0),
        parent_id: v["parent_id"].as_i64(),
        live: b(v, "live"),
        exists: v["exists"].as_bool().unwrap_or(true),
    })
}

/// 电脑上的一条素材 → 导入手机素材库时带的信息。来源存原文（`src`），显示时再翻译
pub fn import_info(v: &Value, thumb: Option<String>) -> crate::ImportInfo {
    crate::ImportInfo {
        title: os(v, "title"),
        kind: os(v, "type"),
        description: s(v, "desc"),
        source_url: s(v, "url"),
        page_url: s(v, "page"),
        site: s(v, "src"),
        uploader: s(v, "uploader"),
        format: os(v, "fmt"),
        width: ou32(v, "width"),
        height: ou32(v, "height"),
        duration: v["durS"].as_f64(),
        tags: v["tags"].as_array().map(|a| a.iter().filter_map(|t| t.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        thumb_path: thumb,
        live: b(v, "live"),
    }
}

/// 电脑读取信息的结果（`ProbeInfo` 的样子）
pub fn probe(v: &Value) -> MobileProbe {
    MobileProbe {
        kind: s(v, "kind"),
        title: os(v, "title"),
        duration: v["duration"].as_f64(),
        thumbnail: os(v, "thumbnail"),
        site: os(v, "site"),
        width: ou32(v, "width"),
        height: ou32(v, "height"),
        qualities: v["qualities"]
            .as_array()
            .map(|a| a.iter().map(|q| MobileQuality { label: s(q, "label"), height: ou32(q, "height"), bytes: q["bytes"].as_u64() }).collect())
            .unwrap_or_default(),
        audio_bytes: v["audio"].as_u64(),
        size: v["size"].as_u64(),
        choose: b(v, "choose"),
        entries: v["entries"].as_array().map(|a| a.iter().map(entry).collect()).unwrap_or_default(),
        playlist: playlist(&v["playlist"]),
        is_live: b(v, "isLive"),
        resolved_url: None,
        media_urls: vec![],
    }
}

// ------------------------------------------------------------------ 错误

/// 手机端遥控电脑时的固定代码（小写，和本机的 `unsupported_here` 一个写法）。见 crates/captae-mobile/REMOTE.md
pub mod code {
    /// 连不上电脑（不在同一个 Wi-Fi、电脑没开 Captae、没打开「让手机连接」）
    pub const UNREACHABLE: &str = "unreachable";
    /// 连上的不是配对过的那台（或者电脑重新生成了证书）：要重新配对
    pub const CERT_MISMATCH: &str = "cert_mismatch";
    /// 6 位数配对：确认码对不上（输错了，或者有人冒充电脑）
    pub const PAIR_MISMATCH: &str = "pair_mismatch";
    /// 二维码不是 Captae 的
    pub const BAD_LINK: &str = "bad_link";
    /// 被撤销了（或者令牌不对）：令牌已经删掉，要重新配对
    pub const UNAUTHORIZED: &str = "unauthorized";
    /// 电脑不让手机做这件事（白名单外）
    pub const NOT_ALLOWED: &str = "not_allowed";
    pub const BAD_REQUEST: &str = "bad_request";
    pub const NOT_FOUND: &str = "not_found";
    pub const PAIR_EXPIRED: &str = "pair_expired";
    /// 二维码的配对码不对
    pub const PAIR_WRONG: &str = "pair_wrong";
    pub const PAIR_VOID: &str = "pair_void";
    pub const PAIR_CLOSED: &str = "pair_closed";
    /// 续传时电脑上的文件变了
    pub const RANGE: &str = "range";
    /// 没有这台电脑（id 不对、已经删了）
    pub const NO_DEVICE: &str = "no_device";
    /// 安全存储里没有这台电脑的令牌 / 写不进去
    pub const NEEDS_PAIRING: &str = "needs_pairing";
    /// 没有这个「存到手机」/ 队列里没有这条
    pub const NO_TASK: &str = "no_task";
    /// 和电脑通信出错（别的原因，`raw` 里有原文）
    pub const FAILED: &str = "remote_failed";
}

/// 固定代码 → 失败详情（`errors::coded` 生成，`types::to_mobile` 转换），按代码配上下一步按钮
pub fn coded(code: &str, message: String) -> MobileError {
    let mut e = crate::types::to_mobile(&errors::coded(code, message), "");
    let MobileError::Failed { detail } = &mut e;
    detail.action = match code {
        code::UNAUTHORIZED | code::CERT_MISMATCH | code::NEEDS_PAIRING => "pair",
        code::UNREACHABLE | code::FAILED => "retry",
        _ => "none",
    }
    .into();
    e
}

/// captae-remote 客户端的错误 → `MobileError`。下载失败带 `fail` 时用和桌面端同一套错误码
pub fn remote(e: &anyhow::Error) -> MobileError {
    let Some(r) = client::remote_error(e) else {
        let mut m = coded(code::FAILED, tr!("和电脑通信出错了"));
        let MobileError::Failed { detail } = &mut m;
        detail.raw = format!("{e:#}");
        return m;
    };
    if let Some(f) = r.fail.as_ref().and_then(failure) {
        return MobileError::Failed { detail: f };
    }
    let mut m = from_remote(r);
    let MobileError::Failed { detail } = &mut m;
    detail.raw = r.message.clone();
    m
}

/// 错误代码（小写的手机端代码），判断用
pub fn code_of(e: &MobileError) -> &str {
    &e.detail().code
}

fn from_remote(r: &RemoteError) -> MobileError {
    let server_msg = || (!r.message.is_empty() && r.status != 0).then(|| r.message.clone());
    match r.code.as_str() {
        client::code::UNREACHABLE => coded(code::UNREACHABLE, tr!("连不上电脑：手机和电脑要在同一个 Wi-Fi，电脑上的 Captae 要开着")),
        client::code::CERT_MISMATCH => coded(code::CERT_MISMATCH, tr!("连上的不是配对过的那台电脑，或者电脑重新生成过证书，要重新配对")),
        client::code::PAIR_MISMATCH => coded(code::PAIR_MISMATCH, tr!("配对码不对，再看一眼电脑上的 6 位数")),
        client::code::BAD_LINK => coded(code::BAD_LINK, tr!("这不是 Captae 的配对二维码")),
        proto::code::UNAUTHORIZED => coded(code::UNAUTHORIZED, tr!("这台手机在电脑上被撤销了，要重新配对")),
        proto::code::NOT_ALLOWED => coded(code::NOT_ALLOWED, tr!("电脑不允许手机做这件事")),
        proto::code::NOT_FOUND => coded(code::NOT_FOUND, tr!("电脑上没有这个任务或素材了")),
        proto::code::PAIR_EXPIRED => coded(code::PAIR_EXPIRED, tr!("配对码过期了，在电脑上换一个新的")),
        proto::code::PAIR_WRONG => coded(code::PAIR_WRONG, tr!("配对码不对，还能试 {n} 次", n = r.left.unwrap_or(0))),
        proto::code::PAIR_VOID => coded(code::PAIR_VOID, tr!("配对码错了太多次，已经作废，在电脑上换一个新的")),
        proto::code::PAIR_CLOSED => coded(code::PAIR_CLOSED, tr!("电脑上现在没有在显示配对码：在电脑上打开 Captae › 设置 › 手机，打开「让手机连接这台电脑」")),
        proto::code::RANGE => coded(code::RANGE, tr!("电脑上的文件变了，要从头再存一次")),
        // 电脑上生成的说法（链接不完整这类）已经是给人看的句子（电脑的语言），照用
        proto::code::BAD_REQUEST => coded(code::BAD_REQUEST, server_msg().unwrap_or_else(|| tr!("电脑没有接受这个请求"))),
        _ => coded(code::FAILED, server_msg().unwrap_or_else(|| tr!("和电脑通信出错了"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn job_json_roundtrip() {
        let v = json!({
            "id": 7, "title": "Sintel", "src": "example.com/a.mp4", "url": "https://example.com/a.mp4", "eng": "aria2", "fmt": "MP4", "dur": "0:52",
            "audio": false, "size": 4.5, "got": 1.25, "speed": 2.0, "state": "run", "phase": "dl", "pri": 0, "boost": false,
            "vars": [{"r": "1920×1080", "p": "1080p", "b": 5.0, "mb": 120.5}], "vIdx": 0, "merge": 0.0, "segs": [2, 1, 0],
            "from": "iPhone", "log": [[1700000000000i64, "来自 iPhone：https://example.com/a.mp4", ""]],
            "fail": {"code": "network", "message": "网络断了", "raw": "x", "action": "retry", "next": "…", "site": "哔哩哔哩"}
        });
        let j = job(&v).unwrap();
        assert_eq!((j.id, j.state, j.total_bytes, j.got_bytes, j.speed), (7, MobileJobState::Running, 4_500_000, 1_250_000, 2_000_000));
        assert_eq!(j.variants[0].bytes, 120_500_000);
        assert_eq!(j.segments, vec![2, 1, 0]);
        assert!(j.path.is_none() && !j.recording);
        let f = j.failure.unwrap();
        assert_eq!((f.code.as_str(), f.action.as_str(), f.site.as_deref()), ("network", "retry", Some("哔哩哔哩")));
        assert_eq!(job_from(&v).as_deref(), Some("iPhone"));
        assert_eq!(job_log(&v)[0].text, "来自 iPhone：https://example.com/a.mp4");
        assert!(job(&json!({"id": 1, "state": "什么"})).is_none());
        // 录直播：在跑、在下、没停
        let rec = json!({"id": 2, "state": "run", "phase": "dl", "rec": {"since": 5, "max": 600}});
        let j = job(&rec).unwrap();
        assert!(j.recording);
        assert_eq!((j.record_since, j.record_max_secs), (Some(5), Some(600)));
    }

    #[test]
    fn remote_errors_have_codes() {
        let e = anyhow::Error::new(RemoteError { status: 403, code: proto::code::PAIR_WRONG.into(), message: "wrong".into(), fail: None, left: Some(3) });
        let m = remote(&e);
        assert_eq!(code_of(&m), code::PAIR_WRONG);
        assert!(m.detail().message.contains('3'), "{}", m.detail().message);
        let e = anyhow::Error::new(RemoteError { status: 401, code: proto::code::UNAUTHORIZED.into(), message: "x".into(), fail: None, left: None });
        assert_eq!((code_of(&remote(&e)), remote(&e).detail().action.as_str()), (code::UNAUTHORIZED, "pair"));
        // 下载失败带 fail：用桌面端同一套错误码
        let e = anyhow::Error::new(RemoteError { status: 400, code: proto::code::FAILED.into(), message: "x".into(), fail: Some(json!({"code": "need_login", "message": "要登录", "raw": ""})), left: None });
        assert_eq!(code_of(&remote(&e)), "need_login");
        assert_eq!(code_of(&remote(&anyhow::anyhow!("别的"))), code::FAILED);
    }
}
