// 动效的数值：照 docs/MOTION.md（桌面端的规范）换成 SwiftUI 写法。弹簧一律 `bounce: 0`（MOTION 1.3），
// 曲线照 1.1 的 cubic-bezier，时长照 1.2。系统控件（sheet、菜单、标签栏、导航）自带的动效不改（PLAN 4.4）。
// 减少动态效果（MOTION 1.5）：位移、缩放、模糊都去掉，只留透明度；时长照旧。各处用 `@Environment(\.accessibilityReduceMotion)` 挑。
// 课程依据：animations.dev 01/04 Spring animations（弹簧可打断、默认不回弹）、01/05 Timing and purpose（高频的不动、UI 动画 < 300ms）、
// 01/09 Practical tips（按压 .97、不从 scale(0) 起、进退场 ease-out、模糊遮住两态的缝）、04/03 Accessibility（减少动效：去位移留透明度）。
import SwiftUI

enum Motion {
  /// 状态互换（读取中 → 选清晰度 → 进度 → 完成 / 失败、输入卡 ↔ 读取中）：弹簧 0.3s（MOTION 1.2「状态互换」）
  static let swap = Animation.spring(duration: 0.3, bounce: 0)
  /// 新任务进列表、其余行让位：弹簧 0.4s（MOTION c1）
  static let listChange = Animation.spring(duration: 0.4, bounce: 0)
  /// 按压：160ms ease-out-quad（MOTION 1.2）
  static let press = Animation.timingCurve(0.25, 0.46, 0.45, 0.94, duration: 0.16)
  /// 图标互换（暂停 ↔ 继续）：100ms（MOTION c7）
  static let iconSwap = Animation.timingCurve(0.23, 1, 0.32, 1, duration: 0.1)
  /// 封面、缩略图读到：300ms ease-out-quint（MOTION 1.2）
  static let reveal = Animation.timingCurve(0.23, 1, 0.32, 1, duration: 0.3)
  /// 颜色、透明度的小变化：120ms ease（MOTION 1.1、1.2；CSS 的 ease 曲线，不用对称的 easeInOut，审查第 16 条）
  static let tint = Animation.timingCurve(0.25, 0.1, 0.25, 1, duration: 0.12)
  /// 取消、移除：150ms ease-out-quint（MOTION c3；退场比进场短，审查第 6 条）
  static let exit = Animation.timingCurve(0.23, 1, 0.32, 1, duration: 0.15)
  /// 进度条：匀速，时长等于核心推进度的间隔（手机上每 250ms 一次，MOTION c5），看起来一直在往前走
  static let progress = Animation.linear(duration: 0.25)
}

/// 模糊（给转场用：两态之间加一点模糊，遮住交叉淡化的缝，课程 01/09「Use blur when nothing else works」）
private struct Blur: ViewModifier {
  var radius: CGFloat
  func body(content: Content) -> some View { content.blur(radius: radius) }
}

extension AnyTransition {
  /// 状态块互换（MOTION c「状态块」）：新的从上方 8pt + 4pt 模糊进来，旧的往下出。减少动效时只淡入淡出
  static func statusSwap(reduce: Bool) -> AnyTransition {
    if reduce { return .opacity }
    return .asymmetric(
      insertion: .opacity.combined(with: .offset(y: -8)).combined(with: .modifier(active: Blur(radius: 4), identity: Blur(radius: 0))),
      removal: .opacity.combined(with: .offset(y: 8)).combined(with: .modifier(active: Blur(radius: 4), identity: Blur(radius: 0))))
  }

  /// 任务卡进出列表：进 opacity + 从上方 8pt，跟外面的 .4s 弹簧（c1）；出 opacity + 缩到 .95，自己用 150ms（c3），
  /// 不跟外面那个 .4s 弹簧。减少动效时只淡入淡出，退场同样 150ms
  static func listItem(reduce: Bool) -> AnyTransition {
    if reduce { return .asymmetric(insertion: .opacity, removal: AnyTransition.opacity.animation(Motion.exit)) }
    return .asymmetric(
      insertion: .opacity.combined(with: .offset(y: -8)),
      removal: AnyTransition.opacity.combined(with: .scale(scale: 0.95)).animation(Motion.exit))
  }

  /// 图标互换（c7）：先出后进，缩到 .5 + 4pt 模糊。减少动效时只淡入淡出
  static func iconSwap(reduce: Bool) -> AnyTransition {
    if reduce { return .opacity }
    return .opacity.combined(with: .scale(scale: 0.5)).combined(with: .modifier(active: Blur(radius: 4), identity: Blur(radius: 0)))
  }
}

/// 按压反馈：按下缩到 .97（小圆按钮 .95），160ms ease-out-quad；减少动效时不缩，只稍微变淡
struct PressScale: ViewModifier {
  var pressed: Bool
  var scale: CGFloat = 0.97
  @Environment(\.accessibilityReduceMotion) private var reduce

  func body(content: Content) -> some View {
    content
      .scaleEffect(pressed && !reduce ? scale : 1)
      .opacity(pressed && reduce ? 0.75 : 1)
      .animation(Motion.press, value: pressed)
  }
}

/// 小圆按钮（输入卡的发送 / 停止、下载条的「+」）：按下缩到 .95
struct CircleButtonStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    configuration.label.modifier(PressScale(pressed: configuration.isPressed, scale: 0.95))
  }
}

extension ButtonStyle where Self == CircleButtonStyle {
  static var circle: CircleButtonStyle { CircleButtonStyle() }
}

/// 大块卡片（任务卡、素材卡）：按下缩到 .98（MOTION 1.2、j；审查第 15 条）。不改颜色、不变暗
struct CardPressStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    configuration.label.modifier(PressScale(pressed: configuration.isPressed, scale: 0.98))
  }
}

extension ButtonStyle where Self == CardPressStyle {
  static var cardPress: CardPressStyle { CardPressStyle() }
}
