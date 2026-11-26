// 凹面分段：浅灰凹面里一块白色选中块（桌面端 affectUI 分段控件的样子）。
// 用在卡片里面（选清晰度、素材类型），这些在内容层；顶部的「这台手机 / 电脑」切换用系统分段控件。
// 每段可以有第二行小字（清晰度后面的大小）。
// 选中块用手点时滑过去：弹簧 .28s、不回弹（MOTION d，和 Android、桌面端一样；审查第 27 条）。减少动态效果时直接换。
import SwiftUI

struct WellSegments<Value: Hashable>: View {
  struct Item: Identifiable {
    var value: Value
    var title: String
    var detail: String?
    var id: Value { value }
  }

  var items: [Item]
  @Binding var selection: Value
  var label: String

  @Environment(\.dynamicTypeSize) private var typeSize
  @Environment(\.accessibilityReduceMotion) private var reduce
  @Namespace private var ns

  var body: some View {
    // 辅助功能大字号：一行放不下四段，排成两列
    let columns = typeSize.isAccessibilitySize ? min(2, items.count) : items.count
    LazyVGrid(columns: Array(repeating: GridItem(.flexible(), spacing: 2), count: max(1, columns)), spacing: 2) {
      ForEach(items) { item in
        let on = item.value == selection
        Button {
          if reduce {
            selection = item.value
          } else {
            withAnimation(.spring(duration: 0.28, bounce: 0)) { selection = item.value }
          }
        } label: {
          VStack(spacing: 1) {
            Text(item.title)
              .font(on ? .cpMetaMedium : .cpMeta)
              .foregroundStyle(on ? Palette.ink : Palette.ink2)
            if let d = item.detail {
              Text(d)
                .font(.cpMono(11, relativeTo: .caption2, weight: on ? .monoMedium : .mono))
                .foregroundStyle(on ? Palette.ink2 : Palette.ink3)
            }
          }
          .lineLimit(1)
          .minimumScaleFactor(0.85)
          .frame(maxWidth: .infinity)
          .padding(.vertical, item.detail == nil ? 6 : 6)
          .background {
            if on {
              RoundedRectangle(cornerRadius: Radius.sm - 1, style: .continuous)
                .fill(Palette.card)
                .shadow(color: .black.opacity(0.08), radius: 1, y: 0.5)
                // 同一块选中块在各段之间移动；减少动效时换选中不包动画，直接到位
                .matchedGeometryEffect(id: "selected", in: ns, isSource: true)
            }
          }
          .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .accessibilityLabel([item.title, item.detail].compactMap(\.self).joined(separator: "，"))
        .accessibilityAddTraits(on ? .isSelected : [])
      }
    }
    .padding(2)
    .well(radius: Radius.sm + 1)
    .accessibilityElement(children: .contain)
    .accessibilityLabel(label)
  }
}
