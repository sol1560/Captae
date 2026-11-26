// 状态行：浅灰凹面 + 上了色的小图标 + 一句话（下面可选一行小字）+ 右边一个下一步按钮。
// 照桌面端 `.cp-note`：不铺彩色底，只有小图标上色（BRIEF 第 1 节、桌面端 README 第 1 节第 4 条）。
// 按钮上的字长、或者辅助功能大字号时，按钮换到文字下面。
import SwiftUI

/// 状态行小图标的颜色
enum NoteTone {
  /// 要登录、要验证
  case warn
  /// 失败
  case err
  /// 等待、提示（强调色）
  case info
  /// 没有颜色的说明
  case plain

  var color: Color {
    switch self {
    case .warn: Palette.warn
    case .err: Palette.err
    case .info: Palette.accent
    case .plain: Palette.ink3
    }
  }
}

struct StatusRow<Action: View>: View {
  var tone: NoteTone
  var icon: String
  var title: String
  var hint: String?
  /// 按钮放到文字下面（按钮上的字长时，或者辅助功能大字号）
  var stacked = false
  @ViewBuilder var action: () -> Action
  @Environment(\.dynamicTypeSize) private var typeSize

  var body: some View {
    Group {
      if stacked || typeSize.isAccessibilitySize {
        VStack(alignment: .leading, spacing: 10) {
          message
          HStack {
            Spacer(minLength: 0)
            action()
          }
        }
      } else {
        HStack(spacing: 10) {
          message
          action()
        }
      }
    }
    .padding(.leading, 11)
    .padding(.trailing, 8)
    .padding(.vertical, 9)
    .frame(maxWidth: .infinity, alignment: .leading)
    .well()
  }

  private var message: some View {
    HStack(alignment: .firstTextBaseline, spacing: 9) {
      Image(systemName: icon)
        .font(.system(size: 14, weight: .medium))
        .foregroundStyle(tone.color)
        .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 2 }
        .accessibilityHidden(true)
      VStack(alignment: .leading, spacing: 2) {
        Text(title)
          .font(.cpMeta)
          .foregroundStyle(Palette.ink)
          .fixedSize(horizontal: false, vertical: true)
        if let hint {
          Text(hint)
            .font(.cpSmall)
            .foregroundStyle(Palette.ink3)
            .fixedSize(horizontal: false, vertical: true)
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    }
    .accessibilityElement(children: .combine)
  }
}

extension StatusRow where Action == EmptyView {
  init(tone: NoteTone, icon: String, title: String, hint: String? = nil) {
    self.init(tone: tone, icon: icon, title: title, hint: hint, stacked: false) { EmptyView() }
  }
}
