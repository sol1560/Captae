// 步骤时间线（「进行到哪一步」）：照桌面端 `StepTimeline`：做完的打勾、正在做的强调色圆点、
// 没做的空心圈、失败的红色、要登录或验证的橙色（BRIEF 2.8 第 4 条），左边一条细竖线连起来，每步下面可选一行小字。
import SwiftUI

struct Step: Hashable {
  /// `warn`：要登录、要人机验证（橙色，不是失败的红色）
  enum State { case done, now, todo, err, warn }
  var label: String
  var sub: String?
  var state: State
}

struct StepTimeline: View {
  var steps: [Step]

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      ForEach(Array(steps.enumerated()), id: \.offset) { i, step in
        HStack(alignment: .top, spacing: 12) {
          VStack(spacing: 0) {
            marker(step.state)
              .frame(width: 18, height: 20)
            if i < steps.count - 1 {
              Rectangle().fill(Palette.line2).frame(width: 1).frame(maxHeight: .infinity)
            }
          }
          VStack(alignment: .leading, spacing: 2) {
            Text(step.label)
              .font(step.state == .now ? .cpSecondaryMedium : .cpSecondary)
              .foregroundStyle(step.state == .todo ? Palette.ink3 : Palette.ink)
            if let sub = step.sub {
              Text(sub)
                .font(.cpMeta)
                .monospacedDigit()
                .foregroundStyle(step.state == .err ? Palette.err : step.state == .warn ? Palette.warn : Palette.ink3)
                .fixedSize(horizontal: false, vertical: true)
            }
          }
          .padding(.top, 1)
          .padding(.bottom, i < steps.count - 1 ? 14 : 0)
          Spacer(minLength: 0)
        }
        .accessibilityElement(children: .combine)
        .accessibilityValue(stateText(step.state))
      }
    }
  }

  @ViewBuilder
  private func marker(_ s: Step.State) -> some View {
    switch s {
    case .done:
      Image(systemName: "checkmark.circle.fill")
        .font(.system(size: 17))
        .foregroundStyle(Palette.ink)
    case .now:
      ZStack {
        Circle().strokeBorder(Palette.accent.opacity(0.35), lineWidth: 1.5)
        Circle().fill(Palette.accent).padding(4.5)
      }
      .frame(width: 17, height: 17)
    case .todo:
      Circle().strokeBorder(Palette.ink4, lineWidth: 1.5).frame(width: 17, height: 17)
    case .err:
      Image(systemName: "exclamationmark.circle.fill")
        .font(.system(size: 17))
        .foregroundStyle(Palette.err)
    case .warn:
      Image(systemName: "exclamationmark.circle.fill")
        .font(.system(size: 17))
        .foregroundStyle(Palette.warn)
    }
  }

  private func stateText(_ s: Step.State) -> String {
    switch s {
    case .done: "已完成"
    case .now: "正在进行"
    case .todo: "还没开始"
    case .err: "没有成功"
    case .warn: "要登录或验证"
    }
  }
}
