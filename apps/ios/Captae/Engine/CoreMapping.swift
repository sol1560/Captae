// Rust 核心（UniFFI 绑定）的类型 → 界面用的模型（Model/ 里那几个，和桌面端 JSON 同名的字段）。
// 界面一直按 Job / Asset / Failure 画，接真数据只多这一层换算：字节 → MB，毫秒时间戳 → 「今天 06:55」。
import CaptaeKit
import Foundation

extension Failure {
  init(_ f: MobileFailure) {
    self.init(
      code: ErrorCode(rawValue: f.code) ?? .unknown, message: f.message, raw: f.raw, startsAt: f.startsAt, site: f.site,
      action: FailAction(rawValue: f.action))
  }
}

extension Job {
  /// MobileJob 的大小是字节、速度是字节 / 秒；界面按桌面端的写法用 MB（10^6 字节）
  init(_ m: MobileJob) {
    let mb = { (b: UInt64) in Double(b) / 1_000_000 }
    self.init(
      id: m.id, title: m.title, src: m.src, url: m.url, plat: m.plat, eng: Engine(rawValue: m.engine) ?? .aria2,
      fmt: m.format, dur: m.duration, thumb: m.thumb, audio: m.audioOnly, size: mb(m.totalBytes), got: mb(m.gotBytes),
      speed: mb(m.speed), state: JobState(m.state), phase: Phase(rawValue: m.phase) ?? .dl, fail: m.failure.map(Failure.init),
      vars: m.variants.map { Variant(r: $0.resolution, p: $0.label, b: $0.mbps, mb: mb($0.bytes)) }, vIdx: Int(m.variantIndex),
      pickUntil: m.pickUntil, merge: m.mergePercent, retryAt: m.retryAt, assetId: m.assetId, doneAt: m.doneAt,
      segs: m.segments.map { SegState(rawValue: Int($0)) ?? .wait }, live: m.live)
    untitledFlag = m.untitled
    probeAt = m.probeAt
    choose = m.choose
    entries = m.entries.map {
      Entry(id: $0.id, title: $0.title, duration: $0.duration, thumb: $0.thumb, url: $0.url, index: Int($0.index))
    }
    recording = m.recording
    path = m.path
    // 直链、M3U8 读不到封面：下完以后用文件本身取一帧（RemoteImage.swift 的 media: 前缀）
    if thumb == nil, m.state == .done, !m.audioOnly, let p = m.path {
      thumb = "media:" + p
    }
  }
}

extension JobState {
  init(_ s: MobileJobState) {
    switch s {
    case .queued: self = .queue
    case .running: self = .run
    case .paused: self = .pause
    case .done: self = .done
    case .failed: self = .failed
    }
  }
}

extension Asset {
  init(_ a: MobileAsset) {
    let created = Date(timeIntervalSince1970: Double(a.createdAt) / 1000)
    let res = a.width.flatMap { w in a.height.map { "\(w)×\($0)" } } ?? ""
    // 手机上没有 ffmpeg，核心不生成缩略图：视频和图片用文件本身，由 App 生成（RemoteImage.swift 的 media: 前缀）
    let thumb = a.thumb ?? ((a.kind == "video" || a.kind == "image") && a.exists && !a.path.isEmpty ? "media:" + a.path : nil)
    self.init(
      id: a.id, kind: AssetKind(rawValue: a.kind) ?? .video, title: a.title, desc: a.description, url: a.url,
      page: a.pageUrl, src: a.site, uploader: a.uploader, path: a.path, fmt: a.format,
      width: a.width.map(Int.init), height: a.height.map(Int.init), duration: a.duration, bytes: Int64(a.bytes), thumb: thumb,
      tags: a.tags, createdAt: a.createdAt, res: res, dur: Fmt.clock(a.duration), size: Fmt.mb(Double(a.bytes) / 1_000_000),
      when: Fmt.when(created))
  }
}

extension Fmt {
  /// 下载时间：今天 06:55 / 昨天 04:57 / 11月22日（桌面端 format.ts `whenLabel` 的短写法，素材卡上用）
  static func when(_ d: Date, now: Date = .now) -> String {
    let cal = Calendar.current
    let time = hm(d)
    if cal.isDate(d, inSameDayAs: now) { return String(localized: "今天 \(time)") }
    if let y = cal.date(byAdding: .day, value: -1, to: now), cal.isDate(d, inSameDayAs: y) {
      return String(localized: "昨天 \(time)")
    }
    return d.formatted(.dateTime.month(.abbreviated).day())
  }

  /// 完整的下载时间：2026年11月24日 06:55（素材详情「下载于」）
  static func whenFull(_ ms: Int64) -> String {
    let d = Date(timeIntervalSince1970: Double(ms) / 1000)
    return d.formatted(.dateTime.year().month(.abbreviated).day()) + " " + hm(d)
  }

  /// 时:分，一律 24 小时制（docs/I18N.md 2.5）；系统的日期格式在中文、跟随 12 小时制时会写成 10:54
  static func hm(_ d: Date, seconds: Bool = false) -> String {
    let c = Calendar.current.dateComponents([.hour, .minute, .second], from: d)
    let base = String(format: "%02d:%02d", c.hour ?? 0, c.minute ?? 0)
    return seconds ? base + String(format: ":%02d", c.second ?? 0) : base
  }
}
