// 网站名字和图标：照桌面端 `apps/desktop/src/lib/detect.ts`（`siteLabel`）和 `parts/index.tsx`（站点图标）。
import SwiftUI

enum Site {
  /// plat → 名字
  private static let names: [String: String] = [
    "youtube": "YouTube", "bilibili": "哔哩哔哩", "x": "X", "twitter": "X", "douyin": "抖音", "tiktok": "TikTok",
    "instagram": "Instagram",
  ]

  /// 网站名字：后端给的名字优先，再按 plat、主机名认；都认不出就用主机名
  static func label(plat: String?, url: String, named: String? = nil) -> String {
    if let named, !named.isEmpty { return named }
    if let plat, let n = names[plat] { return n }
    let h = host(url)
    if h.contains("youtube") || h == "youtu.be" { return "YouTube" }
    if h.contains("bilibili") || h == "b23.tv" { return "哔哩哔哩" }
    if h == "x.com" || h.hasSuffix(".x.com") || h.contains("twitter") { return "X" }
    if h.contains("douyin") { return "抖音" }
    if h.contains("tiktok") { return "TikTok" }
    if h.contains("instagram") { return "Instagram" }
    if h.contains("vimeo") { return "Vimeo" }
    return h
  }

  /// 站点图标的资源名（Simple Icons，CC0）；认不出的网站用地球图标
  static func iconAsset(plat: String?, url: String) -> String? {
    let key = (plat ?? "") + " " + host(url)
    if key.contains("youtube") || key.contains("youtu.be") { return "site-youtube" }
    if key.contains("bilibili") || key.contains("b23.tv") { return "site-bilibili" }
    if key.contains("douyin") || key.contains("tiktok") { return "site-tiktok" }
    if key.hasPrefix("x ") || key.hasSuffix(" x.com") || key.contains(".x.com") || key.contains("twitter") { return "site-x" }
    if key.contains("instagram") { return "site-instagram" }
    return nil
  }

  /// 只能交给电脑下的网站（上架版 iOS，计划 2）
  static func computerOnly(_ siteName: String) -> Bool {
    ["YouTube", "哔哩哔哩", "抖音", "TikTok", "Instagram", "X"].contains(siteName)
  }

  /// 读取时流光的颜色（BRIEF 2.6）：先用网站的颜色，认不出的用强调色蓝
  static func tint(url: String) -> Color {
    switch label(plat: nil, url: url) {
    case "YouTube": Color(hex: 0xFF0033)
    case "哔哩哔哩": Color(hex: 0xFB7299)
    case "X": Color(hex: 0x6B6B6B)
    case "抖音", "TikTok": Color(hex: 0x25F4EE)
    case "Instagram": Color(hex: 0xE1306C)
    default: Palette.accent
    }
  }

  /// 网址的主机名，去掉 www.
  static func host(_ u: String) -> String {
    let full = u.contains("://") ? u : "https://" + u
    let h = URLComponents(string: full)?.host ?? u.split(separator: "/").first.map(String.init) ?? u
    return h.hasPrefix("www.") ? String(h.dropFirst(4)) : h
  }
}

/// 中文里夹名字时，名字是英文、数字开头 / 结尾的那一边加空格：「登录 YouTube」「登录哔哩哔哩」（桌面端 format.ts `cjk`）
func cjk(_ a: String, _ name: String, _ b: String = "") -> String {
  func ascii(_ c: Character?) -> Bool { c.map { $0.isASCII && ($0.isLetter || $0.isNumber) } ?? false }
  let left = !a.isEmpty && ascii(name.first) ? " " : ""
  let right = !b.isEmpty && ascii(name.last) ? " " : ""
  return a + left + name + right + b
}
