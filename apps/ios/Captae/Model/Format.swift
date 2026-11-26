// 数字和时间的显示格式：和桌面端 `apps/desktop/src/lib/format.ts` 同一套说法。
import Foundation

enum Fmt {
  /// 12 MB / 1.40 GB；10 MB 以下保留一位小数
  static func mb(_ n: Double) -> String {
    guard n.isFinite, n > 0 else { return "0 MB" }
    if n >= 1000 { return String(format: "%.2f GB", n / 1000) }
    if n < 10 { return String(format: "%.1f MB", n) }
    return "\(Int(n.rounded())) MB"
  }

  /// 剩余时间：26 秒 / 3 分 12 秒 / 1 小时 5 分
  static func dur(_ seconds: Double) -> String {
    let s = max(0, Int(seconds.rounded()))
    if s >= 3600 { return "\(s / 3600) 小时 \((s % 3600) / 60) 分" }
    if s >= 60 { return "\(s / 60) 分 \(s % 60) 秒" }
    return "\(s) 秒"
  }

  /// 视频时长：0:52 / 14:48 / 1:02:03
  static func clock(_ seconds: Double?) -> String {
    guard let seconds, seconds.isFinite else { return "" }
    let s = max(0, Int(seconds.rounded()))
    let h = s / 3600, m = (s % 3600) / 60, r = s % 60
    return h > 0 ? String(format: "%d:%02d:%02d", h, m, r) : String(format: "%d:%02d", m, r)
  }

  /// 速度：6.4 MB/s
  static func speed(_ mbps: Double) -> String { String(format: "%.1f MB/s", mbps) }

  /// 百分比，向下取整：45%
  static func percent(_ p: Double) -> String { "\(Int((p * 100).rounded(.down)))%" }
}
