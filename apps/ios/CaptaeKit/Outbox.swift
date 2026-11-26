// App 和分享扩展共用的「待发列表」（计划 5.6：App Group 共用待发任务）。
// 分享扩展把选好的链接写进来；App 回到前台时取走：去向是这台手机的交给 Rust 核心开始下，
// 去向是电脑的留在这里，显示成「等电脑上线」（远程控制接好以后连上电脑就发过去，PLAN 3.2、5.3「离线队列」）。
// App 和扩展是两个进程，读写都经过 NSFileCoordinator，文件是 App Group 目录里的 outbox.json。
import Foundation

/// App Group 的目录
public enum SharedStore {
  public static let groupID = "group.app.captae.mobile"

  /// App Group 的目录；拿不到（签名里没有 App Group）时退回自己的 Application Support，App 自己照样能用
  public static var container: URL {
    if let url = FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: groupID) {
      return url
    }
    let url = URL.applicationSupportDirectory.appending(path: "Captae", directoryHint: .isDirectory)
    try? FileManager.default.createDirectory(at: url, withIntermediateDirectories: true)
    return url
  }

  /// App 写、分享扩展读：现在发到哪台电脑（名字；没配对时没有）
  public static let computerNameKey = "computerName"

  /// 两边都能读的设置（分享卡片要知道默认清晰度、电脑的名字）
  public static var defaults: UserDefaults {
    UserDefaults(suiteName: groupID) ?? .standard
  }
}

/// 一条还没交出去的下载
public struct PendingDownload: Codable, Hashable, Sendable, Identifiable {
  public enum Target: String, Codable, Sendable {
    case computer
    case phone
  }

  public var id: UUID
  /// 要下的地址（上架版读网页时是网页里找到的媒体地址）
  public var url: String
  /// 读到的标题、封面、网站名（分享卡片读到了才有）
  public var title: String?
  public var thumb: String?
  public var site: String?
  public var target: Target
  /// 清晰度、只要声音（协议值，和 AddOptions 一样）
  public var quality: String?
  public var audioOnly: Bool
  /// `url` 是从哪个网页里找到的（入库时记成来源）
  public var pageURL: String?
  /// 上架版：这个网站只能在电脑上下（YouTube 这类），改不成在手机上下
  public var computerOnly: Bool
  public var createdAt: Date

  public init(
    id: UUID = UUID(), url: String, title: String? = nil, thumb: String? = nil, site: String? = nil, target: Target,
    quality: String? = nil, audioOnly: Bool = false, pageURL: String? = nil, computerOnly: Bool = false,
    createdAt: Date = .now
  ) {
    self.id = id
    self.url = url
    self.title = title
    self.thumb = thumb
    self.site = site
    self.target = target
    self.quality = quality
    self.audioOnly = audioOnly
    self.pageURL = pageURL
    self.computerOnly = computerOnly
    self.createdAt = createdAt
  }
}

/// 待发列表的读写
public enum Outbox {
  static var file: URL { SharedStore.container.appending(path: "outbox.json") }

  public static func load() -> [PendingDownload] {
    var result: [PendingDownload] = []
    coordinate(writing: false) { url in
      result = read(url)
    }
    return result
  }

  /// 读出来、改、写回去（整个过程在同一次协调里，另一个进程不会插进来）
  @discardableResult
  public static func update(_ change: (inout [PendingDownload]) -> Void) -> [PendingDownload] {
    var result: [PendingDownload] = []
    coordinate(writing: true) { url in
      var list = read(url)
      change(&list)
      result = list
      if let data = try? JSONEncoder().encode(list) {
        try? data.write(to: url, options: .atomic)
      }
    }
    return result
  }

  public static func append(_ item: PendingDownload) {
    update { $0.append(item) }
  }

  private static func read(_ url: URL) -> [PendingDownload] {
    guard let data = try? Data(contentsOf: url) else { return [] }
    return (try? JSONDecoder().decode([PendingDownload].self, from: data)) ?? []
  }

  private static func coordinate(writing: Bool, _ body: (URL) -> Void) {
    let coordinator = NSFileCoordinator()
    var error: NSError?
    if writing {
      coordinator.coordinate(writingItemAt: file, options: .forMerging, error: &error, byAccessor: body)
    } else {
      coordinator.coordinate(readingItemAt: file, options: [], error: &error, byAccessor: body)
    }
    // 协调失败（极少见）时直接读写，丢一次并发保护好过丢数据
    if error != nil { body(file) }
  }
}
