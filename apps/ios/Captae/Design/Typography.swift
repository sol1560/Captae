// 字：Geist（拉丁，OFL 1.1）+ 系统苹方（中文走字体回退）。会变的数字用 Geist Mono 或等宽数字。
// 字号照 BRIEF 第 1 节：大标题 34 粗、标题 17 半粗、正文 17、次要 15、元信息 13、小字 12；
// 都用 `Font.custom(_:size:relativeTo:)`，跟着系统字号（包括辅助功能大字号）缩放。
import SwiftUI

enum Typo {
  /// 字体文件里的 PostScript 名字（Resources/Fonts，Info.plist 的 UIAppFonts 登记）
  enum Face: String {
    case regular = "Geist-Regular"
    case medium = "Geist-Medium"
    case semibold = "Geist-SemiBold"
    case bold = "Geist-Bold"
    case mono = "GeistMono-Regular"
    case monoMedium = "GeistMono-Medium"
    case monoSemibold = "GeistMono-SemiBold"
  }

  static func font(_ face: Face, _ size: CGFloat, relativeTo style: Font.TextStyle) -> Font {
    .custom(face.rawValue, size: size, relativeTo: style)
  }
}

extension Font {
  /// 大标题 34 粗（导航栏的大标题由系统画，这个给页面里的大字用）
  static let cpLargeTitle = Typo.font(.bold, 34, relativeTo: .largeTitle)
  /// 空状态、引导里的标题 22 半粗
  static let cpHero = Typo.font(.semibold, 22, relativeTo: .title2)
  /// 标题 17 半粗
  static let cpTitle = Typo.font(.semibold, 17, relativeTo: .headline)
  /// 正文 17
  static let cpBody = Typo.font(.regular, 17, relativeTo: .body)
  /// 正文加重 17
  static let cpBodyMedium = Typo.font(.medium, 17, relativeTo: .body)
  /// 次要 15
  static let cpSecondary = Typo.font(.regular, 15, relativeTo: .subheadline)
  /// 次要加重 15（卡片标题）
  static let cpSecondaryMedium = Typo.font(.medium, 15, relativeTo: .subheadline)
  /// 元信息 13
  static let cpMeta = Typo.font(.regular, 13, relativeTo: .footnote)
  /// 元信息加重 13
  static let cpMetaMedium = Typo.font(.medium, 13, relativeTo: .footnote)
  /// 小字 12
  static let cpSmall = Typo.font(.regular, 12, relativeTo: .caption)
  /// 分组标题 13 半粗
  static let cpSection = Typo.font(.semibold, 13, relativeTo: .footnote)
  /// 等宽：网址、时长角标、配对码
  static func cpMono(_ size: CGFloat, relativeTo style: Font.TextStyle = .footnote, weight: Typo.Face = .mono) -> Font {
    Typo.font(weight, size, relativeTo: style)
  }
}
