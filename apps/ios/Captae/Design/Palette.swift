// 颜色：照桌面端 `docs/design/README.md` 第 2 节（`apps/desktop/src/theme.css` 的 `:root`）。
// 只做浅色（桌面端也没有深色规范）。玻璃不在这里：系统控件自带，自定义的只有底部输入框（`.glassEffect`）。
import SwiftUI

enum Palette {
  /// 主区底色 `--main`
  static let main = Color(hex: 0xF7F7F7)
  /// 卡片、输入框 `--card`
  static let card = Color.white
  /// 卡片里的凹面（状态行、分段控件的底）`--well`
  static let well = Color(hex: 0xF3F3F3)
  /// 正文、标题 `--ink`
  static let ink = Color(hex: 0x171717)
  /// 次要文字 `--ink-2`
  static let ink2 = Color(hex: 0x525252)
  /// 元信息 `--ink-3`
  static let ink3 = Color(hex: 0x8B8B8B)
  /// 占位、不可用 `--ink-4`
  static let ink4 = Color(hex: 0xB4B4B4)
  /// 分隔线 `--line`
  static let line = Color.black.opacity(0.07)
  /// 描边 `--line-2`
  static let line2 = Color.black.opacity(0.12)
  /// 唯一的强调色 `--accent`：只用在进度条、焦点、选中的单选点、未读点、链接
  static let accent = Color(hex: 0x007AFF)
  /// 强调色的文字 `--accent-ink`
  static let accentInk = Color(hex: 0x0066D6)
  /// 强调色的浅底
  static let accentSoft = Color(hex: 0x007AFF).opacity(0.1)
  /// 成功（小图标）`--ok`
  static let ok = Color(hex: 0x1F8F50)
  /// 要登录、要验证（小图标、小字）`--warn`
  static let warn = Color(hex: 0xC2660A)
  /// 失败（小图标）`--err`
  static let err = Color(hex: 0xD6382A)
  /// 没有封面时的占位底（桌面端 `.cp-thumb` 的底）
  static let thumb = Color(hex: 0xE6E6E6)
  /// 声音素材的封面底
  static let audioTile = Color(hex: 0x232325)
  /// 相机画面的占位（配对扫码）
  static let camera = Color(hex: 0x2A2A2C)
}

extension Color {
  /// 0xRRGGBB
  init(hex: UInt32, opacity: Double = 1) {
    self.init(
      .sRGB,
      red: Double((hex >> 16) & 0xFF) / 255,
      green: Double((hex >> 8) & 0xFF) / 255,
      blue: Double(hex & 0xFF) / 255,
      opacity: opacity
    )
  }
}

/// 圆角：桌面端 README 第 4 节
nonisolated enum Radius {
  /// 小控件
  static let sm: CGFloat = 8
  static let md: CGFloat = 10
  /// 卡片
  static let lg: CGFloat = 14
  /// 浮卡、菜单
  static let xl: CGFloat = 16
  /// 输入框、弹窗
  static let xxl: CGFloat = 20
}

/// 间距：页边距和卡片之间的空
nonisolated enum Spacing {
  /// 页面左右留白
  static let page: CGFloat = 16
  /// 卡片之间
  static let cards: CGFloat = 10
  /// 卡片里面
  static let inset: CGFloat = 12
}
