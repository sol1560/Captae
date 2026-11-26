// 内容里的按钮：主按钮是近黑胶囊、白字（BRIEF 第 1 节），次要按钮是白底胶囊。
// 这些在内容层，不是玻璃；工具栏上的按钮交给系统（自带 Liquid Glass）。按下的反馈见 Motion.swift 的 PressScale。
import SwiftUI

/// 近黑胶囊、白字（内容里的主按钮，一屏只放一个）
struct PrimaryButtonStyle: ButtonStyle {
  var fullWidth = false

  func makeBody(configuration: Configuration) -> some View {
    PrimaryLabel(configuration: configuration, fullWidth: fullWidth)
  }

  private struct PrimaryLabel: View {
    let configuration: ButtonStyleConfiguration
    let fullWidth: Bool
    @Environment(\.isEnabled) private var enabled

    var body: some View {
      configuration.label
        .font(.cpBodyMedium)
        .foregroundStyle(.white)
        .padding(.horizontal, 22)
        .padding(.vertical, 13)
        .frame(maxWidth: fullWidth ? .infinity : nil)
        .background(enabled ? Palette.ink : Palette.ink4, in: .capsule)
        .contentShape(.capsule)
        // 按压：缩到 .97（MOTION 1.2，课程 01/09「Scale your buttons」）
        .modifier(PressScale(pressed: configuration.isPressed))
    }
  }
}

/// 白底胶囊（状态行右边的下一步、次要操作）
struct SecondaryButtonStyle: ButtonStyle {
  var compact = true

  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .font(compact ? .cpMetaMedium : .cpBodyMedium)
      .foregroundStyle(Palette.ink)
      .lineLimit(1)
      .padding(.horizontal, compact ? 12 : 20)
      .padding(.vertical, compact ? 7 : 12)
      .background {
        Capsule()
          .fill(Palette.card)
          .shadow(color: .black.opacity(0.06), radius: 1.5, y: 1)
          .overlay { Capsule().strokeBorder(Color.black.opacity(0.1), lineWidth: 0.5) }
      }
      .contentShape(.capsule)
      .modifier(PressScale(pressed: configuration.isPressed))
  }
}

/// 链接样式的文字按钮（「马上开始」）
struct LinkButtonStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    configuration.label
      .font(.cpMetaMedium)
      .foregroundStyle(Palette.accentInk)
      .opacity(configuration.isPressed ? 0.6 : 1)
      .contentShape(.rect)
  }
}

extension ButtonStyle where Self == PrimaryButtonStyle {
  static var primary: PrimaryButtonStyle { PrimaryButtonStyle() }
  static var primaryWide: PrimaryButtonStyle { PrimaryButtonStyle(fullWidth: true) }
}

extension ButtonStyle where Self == SecondaryButtonStyle {
  static var secondary: SecondaryButtonStyle { SecondaryButtonStyle() }
  static var secondaryLarge: SecondaryButtonStyle { SecondaryButtonStyle(compact: false) }
}

extension ButtonStyle where Self == LinkButtonStyle {
  static var link: LinkButtonStyle { LinkButtonStyle() }
}
