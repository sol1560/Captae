// 手机本地下载的引擎：打开 Rust 核心（crates/captae-mobile 的 MobileCore），把它的事件转到主线程交给 AppModel。
// 目录（都在 App 沙盒里）：
//   Application Support/Captae/data       素材库、设置、queue.json、中间文件
//   Application Support/Captae/Downloads  下完的文件（不在「文件」里显示；存相册、存「文件」由 SaveService 做，见 PLAN 5.4「保存」）
// 平台：上架版 iosStore（只下直链、M3U8、网页里直接写着的媒体地址）；以后的完整版用编译条件 CAPTAE_FULL 换成 iosFull。
import CaptaeKit
import Foundation

/// Rust 核心推来的事件（在主线程上交给 AppModel）
enum CoreEvent: Sendable {
  case job(MobileJob)
  case removed(UInt64)
  case finished(id: UInt64, title: String, path: String, assetId: Int64?)
  case library
  case notice(key: String, title: String, text: String)
  case resync
}

/// 事件回调：Rust 在自己的后台线程上调，这里只转一手到主线程（DispatchQueue.main 先进先出，事件顺序不乱）
nonisolated final class CoreEvents: CoreListener {
  private let sink: @MainActor @Sendable (CoreEvent) -> Void

  init(sink: @escaping @MainActor @Sendable (CoreEvent) -> Void) {
    self.sink = sink
  }

  private func send(_ e: CoreEvent) {
    let sink = sink
    DispatchQueue.main.async { MainActor.assumeIsolated { sink(e) } }
  }

  func onJob(job: MobileJob) { send(.job(job)) }
  func onRemoved(id: UInt64) { send(.removed(id)) }
  func onFinished(id: UInt64, title: String, path: String, assetId: Int64?) {
    send(.finished(id: id, title: title, path: path, assetId: assetId))
  }
  func onLibrary() { send(.library) }
  func onNotice(key: String, title: String, text: String) { send(.notice(key: key, title: title, text: text)) }
  func onResync() { send(.resync) }
}

enum LocalEngine {
  static var root: URL { URL.applicationSupportDirectory.appending(path: "Captae", directoryHint: .isDirectory) }
  static var dataDir: URL { root.appending(path: "data", directoryHint: .isDirectory) }
  static var downloadsDir: URL { root.appending(path: "Downloads", directoryHint: .isDirectory) }

  static var platform: Platform {
    #if CAPTAE_FULL
      .iosFull
    #else
      .iosStore
    #endif
  }

  /// 打开核心。数据目录不在就建；上次没下完的任务会恢复（App 被系统结束后再打开，接着下）
  static func open() throws -> MobileCore {
    let fm = FileManager.default
    try fm.createDirectory(at: dataDir, withIntermediateDirectories: true)
    try fm.createDirectory(at: downloadsDir, withIntermediateDirectories: true)
    return try MobileCore(
      o: InitOptions(
        dataDir: dataDir.path(percentEncoded: false), downloadsDir: downloadsDir.path(percentEncoded: false),
        platform: platform, language: systemLanguage(), nativeLibDir: nil))
  }

  /// 系统语言 → 核心库支持的语言代码（docs/I18N.md 第 1 节）。中文带 Hant / TW / HK / MO 的算繁体；都不支持时用英文
  static func systemLanguage(_ preferred: [String] = Locale.preferredLanguages) -> String {
    for tag in preferred {
      let l = tag.lowercased()
      if l.hasPrefix("zh") {
        return ["hant", "-tw", "-hk", "-mo"].contains { l.contains($0) } ? "zh-TW" : "zh-CN"
      }
      for code in ["en", "ja", "ko", "es", "fr", "de"] where l == code || l.hasPrefix(code + "-") {
        return code
      }
    }
    return "en"
  }
}
