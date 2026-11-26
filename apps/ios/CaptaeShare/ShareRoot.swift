// 分享卡片的数据：拿到链接 → 用 Rust 核心读取信息（标题、封面、时长；上架版认出 YouTube 这类网站就说「在电脑上下」）→
// 用户点「下载 / 先存着」→ 写进 App Group 的待发列表（CaptaeKit/Outbox.swift），App 下次回到前台时取走：
// 去向是手机的开始下，去向是电脑的显示成「等电脑上线」（远程控制接好以后连上就发过去）。
// 扩展里的核心用自己的临时目录（只读取信息，不下载），不碰 App 的数据目录（两个进程同时写 queue.json、素材库会打架）。
import CaptaeKit
import SwiftUI

struct ShareRoot: View {
  var link: () async -> String?
  var finish: () -> Void

  @State private var content: ShareCardContent?
  /// 读到的媒体地址（网页里找到的）、原网页
  @State private var mediaURL: String?
  @State private var pageURL: String?
  @State private var noLink = false
  @State private var token = CancelToken()
  /// App 记下的、现在发到哪台电脑
  private let computer = SharedStore.defaults.string(forKey: SharedStore.computerNameKey)

  var body: some View {
    // 系统把扩展放在它自己的一张 sheet 里，卡片直接画在里面（再弹一张 sheet 会叠成两层）
    Group {
      if let content {
        // 扩展不开 RemoteHub（REMOTE.md 第 11 节：同一时间只让一边开）：发到电脑的存进待发列表，
        // App 回到前台时用 RemoteHub.send 发出去（电脑离线就进它的「等电脑上线」）
        ShareCard(
          content: content, computerName: computer, computerOnline: false,
          holdNote: computer.map { cjk(String(localized: "打开 Captae 以后发到"), $0) },
          onCancel: close,
          onConfirm: save)
      } else if noLink {
        ShareCard(
          content: ShareCardContent(url: "", reading: false, failure: String(localized: "分享进来的东西里没有网页链接")),
          onCancel: close, onConfirm: { _ in close() })
      } else {
        ShareCard(content: ShareCardContent(url: ""), onCancel: close, onConfirm: { _ in })
      }
    }
    .font(.cpBody)
    .tint(Palette.ink)
    .preferredColorScheme(.light)
    .background(Palette.main.ignoresSafeArea())
    .task { await load() }
  }

  private func close() {
    token.cancel()
    finish()
  }

  private func load() async {
    guard let url = await link() else {
      noLink = true
      return
    }
    content = ShareCardContent(url: url)
    do {
      let core = try Self.openCore()
      let p = try await core.probe(url: url, cancel: token)
      mediaURL = p.resolvedUrl
      pageURL = p.resolvedUrl == nil ? nil : url
      content = ShareCardContent(
        url: url, title: p.title, thumb: p.thumbnail, fmt: Self.format(p.kind, url: p.resolvedUrl ?? url), dur: Self.clock(p.duration), reading: false)
    } catch MobileError.Failed(let f) where f.code == "unsupported_here" {
      content = ShareCardContent(url: url, reading: false, computerOnlySite: f.site ?? Site.label(plat: nil, url: url))
    } catch MobileError.Failed(let f) where f.code == "cancelled" {
      return
    } catch MobileError.Failed(let f) {
      content = ShareCardContent(url: url, reading: false, failure: f.message)
    } catch {
      content = ShareCardContent(url: url, reading: false, failure: error.localizedDescription)
    }
  }

  private func save(_ choice: ShareChoice) {
    guard let c = content else { return }
    Outbox.append(
      PendingDownload(
        url: mediaURL ?? c.url, title: c.title, thumb: c.thumb, site: c.computerOnlySite,
        target: choice.target == .computer ? .computer : .phone, quality: choice.quality, pageURL: pageURL,
        computerOnly: c.computerOnlySite != nil))
    finish()
  }

  /// 扩展自己的核心：临时目录、上架版规则、系统语言
  private static func openCore() throws -> MobileCore {
    let root = URL.temporaryDirectory.appending(path: "captae-share", directoryHint: .isDirectory)
    let data = root.appending(path: "data"), dl = root.appending(path: "Downloads")
    try FileManager.default.createDirectory(at: data, withIntermediateDirectories: true)
    try FileManager.default.createDirectory(at: dl, withIntermediateDirectories: true)
    return try MobileCore(
      o: InitOptions(
        dataDir: data.path(percentEncoded: false), downloadsDir: dl.path(percentEncoded: false), platform: .iosStore,
        language: Locale.preferredLanguages.first.map(languageCode) ?? "en", nativeLibDir: nil))
  }

  private static func languageCode(_ tag: String) -> String {
    let l = tag.lowercased()
    if l.hasPrefix("zh") { return ["hant", "-tw", "-hk", "-mo"].contains { l.contains($0) } ? "zh-TW" : "zh-CN" }
    return ["en", "ja", "ko", "es", "fr", "de"].first { l == $0 || l.hasPrefix($0 + "-") } ?? "en"
  }

  /// 读取结果的类型 → 卡片上的说明（和任务卡上的说法一样）
  private static func format(_ kind: String, url: String) -> String {
    switch kind {
    case "hls": return "M3U8 → MP4"
    case "file":
      let ext = URL(string: url)?.pathExtension.uppercased() ?? ""
      return ext.isEmpty ? "" : String(localized: "\(ext) 文件")
    default: return ""
    }
  }

  private static func clock(_ s: Double?) -> String { Fmt.clock(s) }
}
