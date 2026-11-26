// 素材：和 Rust 核心 `crates/captae-core/src/library.rs` 的 `Asset` 对应（字段名照 serde 序列化后的名字）。
// 手机上的素材和电脑上的素材是同一个结构，`where` 区分在哪台设备上。
import Foundation

enum AssetKind: String, Codable, Sendable, CaseIterable {
  case video, audio, image
}

struct Asset: Identifiable, Codable, Hashable, Sendable {
  var id: Int64
  var kind: AssetKind
  var title: String
  var desc: String
  var url: String
  var page: String
  /// 来源站点（「YouTube」「blender.org」）
  var src: String
  var uploader: String
  var path: String
  var fmt: String
  var width: Int?
  var height: Int?
  /// 秒
  var duration: Double?
  var bytes: Int64
  /// 缩略图：本地路径或电脑上的缩略图地址；演示数据里是资源名
  var thumb: String?
  var tags: [String]
  /// 毫秒时间戳
  var createdAt: Int64
  // ---- 展示用派生字段（后端算好的） ----
  /// 「3840×2160」
  var res: String
  /// 「10:34」
  var dur: String
  /// 「640 MB」
  var size: String
  /// 「今天 06:55」
  var when: String

  enum CodingKeys: String, CodingKey {
    case id, title, desc, url, page, src, uploader, path, fmt, width, height, bytes, thumb, tags, res, dur, size, when
    case kind = "type"
    case duration = "durS"
    case createdAt = "created_at"
  }
}

/// 素材在哪台设备上
enum LibrarySource: Hashable, Sendable {
  case phone
  case computer
}
