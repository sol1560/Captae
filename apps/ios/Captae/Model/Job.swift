// 下载任务的数据模型：和 Rust 核心 `crates/captae-core/src/model.rs` 的 `Job` 一一对应。
// 字段名照 serde 序列化后的名字（id / title / src / eng / fmt / size / got / speed / state / phase …），
// 以后 UniFFI 绑定或电脑发来的 JSON 直接解进来，不再转一层。这一轮只接演示数据（Demo/DemoData.swift）。
import Foundation
import SwiftUI

typealias JobID = UInt64

/// 任务状态（model.rs `JobState`）
enum JobState: String, Codable, Sendable {
  /// 等待空位
  case queue
  /// 正在跑
  case run
  /// 用户暂停
  case pause
  case done
  case failed

  var isActive: Bool { self == .queue || self == .run || self == .pause }
}

/// 进行到哪一步（model.rs `Phase`）
enum Phase: String, Codable, Sendable {
  /// 读取视频信息
  case probe
  /// 等用户选清晰度（M3U8 多码率时）
  case pick
  /// 下载中
  case dl
  /// 合并分片、生成视频文件
  case merge
  /// 下载后处理
  case post
  case done
}

/// 下载引擎（model.rs `Engine`）
enum Engine: String, Codable, Sendable {
  case ytDlp = "yt-dlp"
  case aria2
  case hls = "M3U8"
}

/// M3U8 的一档清晰度（model.rs `Variant`）
struct Variant: Codable, Hashable, Sendable {
  /// 1920×1080
  var r: String
  /// 1080p
  var p: String
  /// 码率 Mbps
  var b: Double
  /// 预估大小 MB
  var mb: Double
}

/// M3U8 一段的状态（model.rs `SegState`，序列化成数字）
enum SegState: Int, Codable, Sendable {
  case wait = 0, loading, done, retrying, failed
}

/// 对任务的操作（model.rs `JobAction`）。电脑上的任务从手机发过去也是这一套（计划 5.3 白名单）
enum JobAction: String, Codable, Sendable {
  case toggle, pause, resume, now, up, down, cancel, remove, retry, segretry, pickgo, stop
}

/// 任务在哪里跑：手机端多出来的一层（计划 3.2「下载去哪」）
enum Placement: Hashable, Sendable {
  /// 电脑上的任务，来自电脑的实时事件
  case computer
  /// 在这台手机上下
  case phone
}

/// 一个下载任务。字段对应 model.rs `Job` 序列化后的样子（只取界面用得到的）
struct Job: Identifiable, Codable, Hashable, Sendable {
  var id: JobID
  var title: String
  /// host + path，展示用
  var src: String
  var url: String
  var plat: String?
  var eng: Engine
  var fmt: String
  /// 时长「12:14」
  var dur: String
  /// 缩略图：远程 URL 或本地文件路径；演示数据里是资源名
  var thumb: String?
  var audio: Bool
  /// MB
  var size: Double
  /// MB
  var got: Double
  /// MB/s
  var speed: Double
  var state: JobState
  var phase: Phase
  var fail: Failure?
  var vars: [Variant]
  var vIdx: Int
  /// 选清晰度的倒计时到哪一刻（毫秒时间戳）
  var pickUntil: Int64?
  /// 合并进度（0–100）
  var merge: Double
  /// 网站要求放慢时什么时候自动重试（毫秒时间戳）
  var retryAt: Int64?
  var assetId: Int64?
  var doneAt: Int64?
  /// M3U8 每一段的状态（model.rs `SegState`）
  var segs: [SegState]
  /// 读取时认出来是正在进行的直播、从现在开始录（model.rs `live`）：取消写「取消录制」
  var live: Bool
  // ---- 下面几项来自 Rust 核心的 MobileJob，不参与编解码 ----
  /// 后端说标题还是占位（I18N 2.4：不拿文字判断）；演示数据里为空，按老办法认
  var untitledFlag: Bool?
  /// 这一次读取信息从什么时候开始（毫秒时间戳）：读取中那行的「6.3s」
  var probeAt: Int64?
  /// 播放列表：等用户选条目
  var choose = false
  var entries: [Entry] = []
  /// 正在录直播（只有这时「停止并保存」有效）
  var recording = false
  /// 文件在哪（下完以后是成品）
  var path: String?

  enum CodingKeys: String, CodingKey {
    case id, title, src, url, plat, eng, fmt, dur, thumb, audio, size, got, speed, state, phase, fail, vars, merge, segs, live
    case vIdx, pickUntil, retryAt, assetId, doneAt
  }

  init(
    id: JobID, title: String, src: String, url: String, plat: String? = nil, eng: Engine, fmt: String, dur: String = "",
    thumb: String? = nil, audio: Bool = false, size: Double = 0, got: Double = 0, speed: Double = 0,
    state: JobState, phase: Phase, fail: Failure? = nil, vars: [Variant] = [], vIdx: Int = 0, pickUntil: Int64? = nil,
    merge: Double = 0, retryAt: Int64? = nil, assetId: Int64? = nil, doneAt: Int64? = nil, segs: [SegState] = [],
    live: Bool = false
  ) {
    self.id = id
    self.title = title
    self.src = src
    self.url = url
    self.plat = plat
    self.eng = eng
    self.fmt = fmt
    self.dur = dur
    self.thumb = thumb
    self.audio = audio
    self.size = size
    self.got = got
    self.speed = speed
    self.state = state
    self.phase = phase
    self.fail = fail
    self.vars = vars
    self.vIdx = vIdx
    self.pickUntil = pickUntil
    self.merge = merge
    self.retryAt = retryAt
    self.assetId = assetId
    self.doneAt = doneAt
    self.segs = segs
    self.live = live
  }
}

/// 播放列表里的一条（model.rs `Entry`）
struct Entry: Hashable, Sendable, Identifiable {
  var id: String
  var title: String
  var duration: Double?
  var thumb: String?
  var url: String
  /// 第几个（从 1 开始）
  var index: Int
}

extension Job {
  /// 还没有真标题：后端给的标记优先；演示数据按老办法认（标题是网址，或者占位「… · 正在读取标题…」）
  var untitled: Bool {
    untitledFlag ?? (title.isEmpty || title == src || title == url || title.hasSuffix("正在读取标题…"))
  }

  var isHLS: Bool { eng == .hls }

  /// 正在重新下载的段数：大于 0 时进度下面写「网络有点不稳，正在自动重试」
  var segRetrying: Int { segs.lazy.filter { $0 == .retrying }.count }

  /// 取消按钮上的字：录直播时「取消录制」，平常「取消下载」（桌面端 act.ts）
  var cancelLabel: LocalizedStringKey { live ? "取消录制" : "取消下载" }

  /// 0…1
  var progress: Double { size > 0 ? min(1, got / size) : 0 }

  /// 剩余秒数；速度为 0 时没有
  var etaSeconds: Double? {
    guard speed > 0, size > 0 else { return nil }
    return (size - got) / speed
  }

  var variantName: String { (vars.indices.contains(vIdx) ? vars[vIdx] : vars.first)?.p ?? "" }

  /// 网站名字：失败里带的名字优先，再按 plat、主机名认（桌面端 detect.ts `siteLabel`）
  var siteName: String { Site.label(plat: plat, url: url, named: fail?.site) }

  /// 失败的下一步按钮（后端给的优先）
  var failAction: FailAction? { fail?.act }
}
