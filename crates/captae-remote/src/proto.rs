//! 电脑和手机两边共用的协议：配对链接（二维码内容）、证书指纹、6 位数配对（SPAKE2）的确认码、错误代码、请求的样子。
//! 接口一览见 crates/captae-remote/CLIENT.md。

use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64;
use base64::Engine;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 协议版本：路径前缀 `/v1`，Bonjour 的 TXT 里 `v=1`
pub const VERSION: u32 = 1;
/// Bonjour 服务类型
pub const SERVICE_TYPE: &str = "_captae._tcp.local.";
/// 默认端口：固定一个，手机下次在同一个地址上就能找到；被占了才换随机端口（换了会记下来）
pub const DEFAULT_PORT: u16 = 47863;
/// 配对码（二维码里的一次性码、6 位数）多久有效
pub const PAIR_TTL_MS: i64 = 5 * 60 * 1000;
/// 6 位数最多试几次
pub const PAIR_MAX_TRIES: u32 = 5;
/// WebSocket 关闭码：这台手机被撤销了（客户端据此不再重连）
pub const CLOSE_REVOKED: u16 = 4001;
/// WebSocket 关闭码：电脑关掉了「让手机连接」
pub const CLOSE_STOPPED: u16 = 4002;

/// 错误代码（回应里的 `code`）。按代码判断，不要按给人看的文字判断
pub mod code {
    /// 没带令牌、令牌不对、手机被撤销了（HTTP 401）
    pub const UNAUTHORIZED: &str = "UNAUTHORIZED";
    /// 白名单外的请求（HTTP 403）
    pub const NOT_ALLOWED: &str = "NOT_ALLOWED";
    /// 请求的格式不对、带了不许带的字段（HTTP 400）
    pub const BAD_REQUEST: &str = "BAD_REQUEST";
    /// 没有这个任务 / 素材 / 文件（HTTP 404）
    pub const NOT_FOUND: &str = "NOT_FOUND";
    /// 配对码过期了或者已经用过（HTTP 410）
    pub const PAIR_EXPIRED: &str = "PAIR_EXPIRED";
    /// 配对码不对（HTTP 403），回应里带 `left`：还能试几次
    pub const PAIR_WRONG: &str = "PAIR_WRONG";
    /// 6 位数错了 5 次，作废了（HTTP 410）
    pub const PAIR_VOID: &str = "PAIR_VOID";
    /// 电脑上现在没有在显示配对码（HTTP 409）
    pub const PAIR_CLOSED: &str = "PAIR_CLOSED";
    /// Range 超出文件大小（HTTP 416）
    pub const RANGE: &str = "RANGE";
    /// 下载、操作本身失败（HTTP 400），回应里可能带 `fail`（和桌面端同一套错误码，见 docs/DESIGN.md 4.2）
    pub const FAILED: &str = "FAILED";
}

/// 证书指纹：证书（DER）的 SHA-256，base64url 不带 `=`（43 个字符，二维码短一点）
pub fn fingerprint(cert_der: &[u8]) -> String {
    B64.encode(Sha256::digest(cert_der))
}

/// 令牌的哈希（电脑上只存这个）：SHA-256 的十六进制
pub fn token_hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// `n` 字节的随机数，base64url
pub fn random_b64(n: usize) -> String {
    use rand::RngCore;
    let mut b = vec![0u8; n];
    rand::rng().fill_bytes(&mut b);
    B64.encode(b)
}

/// 两个字符串是否相同，用时和内容无关（比较配对码）
pub fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

pub fn b64(b: &[u8]) -> String {
    B64.encode(b)
}

pub fn unb64(s: &str) -> Option<Vec<u8>> {
    B64.decode(s.trim()).ok()
}

// ------------------------------------------------------------------ 二维码

/// 二维码里的内容：`captae://pair?h=<地址,地址>&p=<端口>&fp=<指纹>&c=<一次性配对码>&n=<电脑名>`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairLink {
    /// 电脑的局域网地址（IPv4，或者不带方括号的 IPv6），按优先顺序
    pub hosts: Vec<String>,
    pub port: u16,
    /// 证书指纹（[`fingerprint`]）
    pub fp: String,
    /// 一次性配对码
    pub code: String,
    /// 电脑名（只用来显示，配对前就能告诉用户在连哪台）
    pub name: String,
}

impl PairLink {
    pub fn to_uri(&self) -> String {
        let mut q = url::form_urlencoded::Serializer::new(String::new());
        q.append_pair("h", &self.hosts.join(","));
        q.append_pair("p", &self.port.to_string());
        q.append_pair("fp", &self.fp);
        q.append_pair("c", &self.code);
        if !self.name.is_empty() {
            q.append_pair("n", &self.name);
        }
        format!("captae://pair?{}", q.finish())
    }

    /// 扫到的内容 → 配对链接。不是 Captae 的二维码、缺东西时 None
    pub fn parse(s: &str) -> Option<PairLink> {
        let u = url::Url::parse(s.trim()).ok()?;
        if u.scheme() != "captae" || u.host_str() != Some("pair") {
            return None;
        }
        let (mut hosts, mut port, mut fp, mut code, mut name) = (vec![], None, None, None, String::new());
        for (k, v) in u.query_pairs() {
            match &*k {
                "h" => hosts = v.split(',').map(str::trim).filter(|h| !h.is_empty()).map(str::to_string).collect(),
                "p" => port = v.parse::<u16>().ok(),
                "fp" => fp = Some(v.to_string()),
                "c" => code = Some(v.to_string()),
                "n" => name = v.to_string(),
                _ => {}
            }
        }
        let fp = fp.filter(|f| unb64(f).is_some_and(|b| b.len() == 32))?;
        let code = code.filter(|c| !c.is_empty())?;
        if hosts.is_empty() {
            return None;
        }
        Some(PairLink { hosts, port: port.filter(|p| *p != 0)?, fp, code, name })
    }
}

/// 地址 + 端口 → `https://…`（IPv6 加方括号）
pub fn base_url(host: &str, port: u16) -> String {
    if host.contains(':') && !host.starts_with('[') {
        format!("https://[{host}]:{port}")
    } else {
        format!("https://{host}:{port}")
    }
}

// ------------------------------------------------------------------ 6 位数配对（SPAKE2）

/// SPAKE2 两边的身份：A 是手机，B 是电脑
pub const SPAKE_ID_PHONE: &[u8] = b"captae phone";
pub const SPAKE_ID_DESKTOP: &[u8] = b"captae desktop";

/// 确认码：HMAC-SHA256(共享密钥, 角色 ‖ 电脑证书的指纹 ‖ 手机的消息 ‖ 电脑的消息)。
/// 证书指纹绑进来：中间有人冒充电脑（换了证书）时，手机看到的指纹和电脑算的不一样，确认码对不上；
/// 中间人自己不知道 6 位数，算不出共享密钥。`role` 是 `b"desktop"` 或 `b"phone"`
pub fn spake_confirm(key: &[u8], role: &[u8], fp: &str, msg_phone: &[u8], msg_desktop: &[u8]) -> Vec<u8> {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC 收任意长度的密钥");
    for part in [b"captae-pair-v1" as &[u8], role, fp.as_bytes(), msg_phone, msg_desktop] {
        m.update(&(part.len() as u32).to_be_bytes());
        m.update(part);
    }
    m.finalize().into_bytes().to_vec()
}

/// 校验确认码（用时和内容无关）
pub fn spake_check(key: &[u8], role: &[u8], fp: &str, msg_phone: &[u8], msg_desktop: &[u8], got: &[u8]) -> bool {
    let mut m = <Hmac<Sha256> as Mac>::new_from_slice(key).expect("HMAC 收任意长度的密钥");
    for part in [b"captae-pair-v1" as &[u8], role, fp.as_bytes(), msg_phone, msg_desktop] {
        m.update(&(part.len() as u32).to_be_bytes());
        m.update(part);
    }
    m.verify_slice(got).is_ok()
}

// ------------------------------------------------------------------ 请求

/// 配对请求里手机报的自己
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct PhoneHello {
    /// 手机名（iOS 的 `UIDevice.name` 在 iOS 16 起只给「iPhone」，Android 是设备名），电脑的下载列表上写「来自 iPhone」
    pub name: String,
    /// ios / android
    pub platform: String,
}

/// `POST /v1/pair`：扫二维码配对
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PairReq {
    pub code: String,
    #[serde(flatten)]
    pub phone: PhoneHello,
}

/// `POST /v1/pair/start`：6 位数配对第一步，`msg` 是手机的 SPAKE2 消息（base64url）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpakeStart {
    pub msg: String,
}

/// `POST /v1/pair/start` 的回应：`msg` 电脑的 SPAKE2 消息，`confirm` 电脑的确认码（都是 base64url）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpakeStarted {
    pub session: String,
    pub msg: String,
    pub confirm: String,
    pub name: String,
}

/// `POST /v1/pair/finish`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpakeFinish {
    pub session: String,
    pub confirm: String,
    #[serde(flatten)]
    pub phone: PhoneHello,
}

/// 配对成功：设备令牌（只这一次给，电脑上只存哈希）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Paired {
    pub token: String,
    /// 电脑给这台手机编的号，撤销用
    pub device: String,
    /// 电脑名
    pub name: String,
}

/// `POST /v1/jobs`：手机能填的只有这几项。保存位置、yt-dlp / ffmpeg 参数这类字段一律拒绝（deny_unknown_fields）
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AddJob {
    pub url: String,
    /// 「最高可用」/ 2160p / 1080p / 720p …（和桌面端同一套值）
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    /// MP4 / WebM / MKV / MP3 / AAC / FLAC / 保持原格式
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default)]
    pub audio_only: bool,
    /// 电脑上的预设名
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preset: Option<String>,
}

/// `GET /v1/library` 的查询参数
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LibQuery {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    /// video / audio / image
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub days: Option<i64>,
    /// 一个标签
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<u32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_link_roundtrip() {
        let l = PairLink { hosts: vec!["192.168.1.5".into(), "fe80::1".into()], port: 47863, fp: fingerprint(b"cert"), code: random_b64(16), name: "Sol 的 MacBook Pro".into() };
        let u = l.to_uri();
        assert!(u.starts_with("captae://pair?h="));
        assert_eq!(PairLink::parse(&u), Some(l));
        assert_eq!(PairLink::parse("https://pair?h=1&p=2"), None);
        // 指纹不是 32 字节的不认
        assert_eq!(PairLink::parse("captae://pair?h=1.2.3.4&p=1&fp=abc&c=x"), None);
        assert_eq!(base_url("fe80::1", 5), "https://[fe80::1]:5");
        assert_eq!(base_url("10.0.0.2", 5), "https://10.0.0.2:5");
    }

    #[test]
    fn spake_roundtrip() {
        use spake2::{Ed25519Group, Identity, Password, Spake2};
        let id = (Identity::new(SPAKE_ID_PHONE), Identity::new(SPAKE_ID_DESKTOP));
        let (a, ma) = Spake2::<Ed25519Group>::start_a(&Password::new(b"123456"), &id.0, &id.1);
        let (b, mb) = Spake2::<Ed25519Group>::start_b(&Password::new(b"123456"), &id.0, &id.1);
        let (ka, kb) = (a.finish(&mb).unwrap(), b.finish(&ma).unwrap());
        assert_eq!(ka, kb);
        let c = spake_confirm(&kb, b"desktop", "fp", &ma, &mb);
        assert!(spake_check(&ka, b"desktop", "fp", &ma, &mb, &c));
        // 指纹不同（中间人换了证书）对不上
        assert!(!spake_check(&ka, b"desktop", "other", &ma, &mb, &c));
        // 6 位数不对：密钥不同，对不上
        let (a2, ma2) = Spake2::<Ed25519Group>::start_a(&Password::new(b"000000"), &id.0, &id.1);
        let (b2, mb2) = Spake2::<Ed25519Group>::start_b(&Password::new(b"123456"), &id.0, &id.1);
        let (ka2, kb2) = (a2.finish(&mb2).unwrap(), b2.finish(&ma2).unwrap());
        let c2 = spake_confirm(&kb2, b"desktop", "fp", &ma2, &mb2);
        assert!(!spake_check(&ka2, b"desktop", "fp", &ma2, &mb2, &c2));
    }
}
