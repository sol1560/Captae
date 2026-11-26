// 失败：错误码 + 给人看的一句话 + 原文。和 Rust 核心 `crates/captae-core/src/errors.rs` 对应：
// 15 个错误码、`Failure::to_json` 的 {code, message, raw, action, next, startsAt?, site?}。
// 下一步按钮由错误码决定（`ErrorCode.action`），界面不自己猜。
import Foundation

/// 15 个错误码（errors.rs `ErrorCode`，序列化成 snake_case）
enum ErrorCode: String, Codable, CaseIterable, Sendable {
  /// 年龄限制、会员、私密、要登录
  case needLogin = "need_login"
  /// 机器人检测、Cloudflare
  case needVerify = "need_verify"
  /// DRM 保护
  case drm
  /// 地区限制
  case geo
  /// 证书不对、被网络拦截
  case blocked
  /// 已删除、不存在
  case unavailable
  /// 网站不认识
  case unsupported
  /// 页面里没有媒体
  case noMedia = "no_media"
  /// 直播还没开始
  case liveUpcoming = "live_upcoming"
  /// 正在直播
  case live
  /// 429、请求太频繁
  case rateLimited = "rate_limited"
  /// 超时、断线、DNS
  case network
  /// 空间满、没有写入权限
  case disk
  /// 组件缺失或损坏
  case tool
  /// 其他
  case unknown
  // ---- 手机端自己的固定代码（crates/captae-mobile/src/types.rs `MobileFailure`） ----
  /// 上架版：这个网站要在电脑上下
  case unsupportedHere = "unsupported_here"
  /// 上架版：网页要先读取信息、找到里面的媒体地址再下
  case probeFirst = "probe_first"
  /// 读取信息被取消了
  case cancelled

  /// 下一步按钮（errors.rs `ErrorCode::action`）
  var action: FailAction {
    switch self {
    case .needLogin: .login
    case .needVerify: .verify
    case .drm, .unavailable, .noMedia: .none
    case .geo, .blocked, .network: .retry
    case .unsupported: .useExtension
    case .liveUpcoming: .waitLive
    case .live: .recordLive
    case .rateLimited: .autoRetry
    case .disk: .changeDir
    case .tool: .repair
    case .unknown: .copyError
    case .unsupportedHere: .useComputer
    case .probeFirst: .probe
    case .cancelled: .none
    }
  }
}

/// errors.rs `ErrorCode::action` 的取值
enum FailAction: String, Codable, Sendable {
  case login, verify, none, retry
  case useExtension = "use_extension"
  case waitLive = "wait_live"
  case recordLive = "record_live"
  case autoRetry = "auto_retry"
  case changeDir = "change_dir"
  case repair
  case copyError = "copy_error"
  /// 手机端：发到电脑
  case useComputer = "use_computer"
  /// 手机端：先读取信息
  case probe
}

/// 一次失败（errors.rs `Failure`）
struct Failure: Codable, Hashable, Sendable {
  var code: ErrorCode
  /// 给人看的一句话，后端已经按网站名字说好了（「这个视频要登录哔哩哔哩才能看」）
  var message: String
  /// 原始报错（英文原文的最后几行），能复制
  var raw: String
  /// 直播还没开始时的开始时间（毫秒时间戳）
  var startsAt: Int64?
  /// 说法里用的网站名字（「哔哩哔哩」「YouTube」），按钮上用同一个
  var site: String?
  /// 后端给的下一步按钮（为空时按错误码推）
  var action: FailAction?

  init(code: ErrorCode, message: String, raw: String = "", startsAt: Int64? = nil, site: String? = nil, action: FailAction? = nil) {
    self.code = code
    self.message = message
    self.raw = raw
    self.startsAt = startsAt
    self.site = site
    self.action = action
  }

  /// 下一步按钮：后端给的优先
  var act: FailAction { action ?? code.action }
}
