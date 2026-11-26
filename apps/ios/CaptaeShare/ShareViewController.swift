// 分享扩展的入口（M4，计划 1 第 4 条）：在 Safari、YouTube、B 站这些 App 里点分享 › Captae。
// 从分享进来的东西里拿出第一个网页链接（直接是链接，或者「标题 + 链接」的一段文字），弹 BRIEF 08 的分享卡片（ShareRoot）。
// 系统给扩展的那一层设成透明，卡片自己是一张贴底的 sheet，和 App 里的输入卡一样大小。
import SwiftUI
import UIKit
import UniformTypeIdentifiers

final class ShareViewController: UIViewController {
  override func viewDidLoad() {
    super.viewDidLoad()
    view.backgroundColor = .clear
    let root = ShareRoot(link: { [weak self] in await self?.firstLink() }, finish: { [weak self] in self?.finish() })
    let host = UIHostingController(rootView: root)
    host.view.backgroundColor = .clear
    addChild(host)
    host.view.frame = view.bounds
    host.view.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    view.addSubview(host.view)
    host.didMove(toParent: self)
  }

  private func finish() {
    extensionContext?.completeRequest(returningItems: nil)
  }

  /// 分享进来的第一个 http(s) 链接
  private func firstLink() async -> String? {
    let providers = (extensionContext?.inputItems as? [NSExtensionItem] ?? []).flatMap { $0.attachments ?? [] }
    for p in providers where p.hasItemConformingToTypeIdentifier(UTType.url.identifier) {
      if let url = try? await p.loadItem(forTypeIdentifier: UTType.url.identifier) as? URL, url.scheme?.hasPrefix("http") == true {
        return url.absoluteString
      }
    }
    for p in providers where p.hasItemConformingToTypeIdentifier(UTType.plainText.identifier) {
      if let text = try? await p.loadItem(forTypeIdentifier: UTType.plainText.identifier) as? String, let link = Self.link(in: text) {
        return link
      }
    }
    return nil
  }

  /// 一段文字里的第一个网页链接（「【标题】https://b23.tv/xxxx」这类）
  static func link(in text: String) -> String? {
    let detector = try? NSDataDetector(types: NSTextCheckingResult.CheckingType.link.rawValue)
    let range = NSRange(text.startIndex..., in: text)
    return detector?.matches(in: text, range: range).lazy.compactMap(\.url).first { $0.scheme?.hasPrefix("http") == true }?.absoluteString
  }
}
