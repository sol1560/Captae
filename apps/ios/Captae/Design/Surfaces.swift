// 内容层的面：白卡、凹面、分组标题。都不透明，不做玻璃（BRIEF 第 1 节：玻璃只给浮在内容上的东西）。
import SwiftUI

extension View {
  /// 白卡：圆角 14，投影 `0 0 0 .5px rgba(0,0,0,.08), 0 1px 2px rgba(0,0,0,.04)`（桌面端 `--sh-card`）
  func card(radius: CGFloat = Radius.lg) -> some View {
    background {
      RoundedRectangle(cornerRadius: radius, style: .continuous)
        .fill(Palette.card)
        .shadow(color: .black.opacity(0.04), radius: 1, y: 1)
        .overlay {
          RoundedRectangle(cornerRadius: radius, style: .continuous)
            .strokeBorder(Color.black.opacity(0.08), lineWidth: 0.5)
        }
    }
  }

  /// 凹面：卡片里的浅灰底（状态行、分段控件的底）
  func well(radius: CGFloat = Radius.md) -> some View {
    background(Palette.well, in: .rect(cornerRadius: radius, style: .continuous))
  }

  /// 页面底色 #f7f7f7，铺满到屏幕边缘（导航栏、标签栏下面也是这个色，系统玻璃透出来的就是它）
  func pageBackground() -> some View {
    background(Palette.main.ignoresSafeArea())
  }
}

/// 分组标题：卡片上方的一行小字（「在手机上」「进行到哪一步」）
struct SectionLabel: View {
  var title: Text
  /// 后半句（个数、在线状态）：调用处已经是本地化好的字符串
  var detail: String?

  /// 界面文字：字面量会进 Localizable.xcstrings
  init(title: LocalizedStringKey, detail: String? = nil) {
    self.title = Text(title)
    self.detail = detail
  }

  /// 数据（设备名）：原样显示，不进本地化
  init(verbatim title: String, detail: String? = nil) {
    self.title = Text(verbatim: title)
    self.detail = detail
  }

  var body: some View {
    // 拼成一段文字：大字号时整句自然换行，不会把后半截挤到右边一列
    Text("\(title.font(.cpSection).foregroundStyle(Palette.ink2))\(Text(verbatim: detail.map { "  ·  \($0)" } ?? "").font(.cpMeta).foregroundStyle(Palette.ink3))")
      .frame(maxWidth: .infinity, alignment: .leading)
      .padding(.horizontal, 4)
    .accessibilityElement(children: .combine)
    .accessibilityAddTraits(.isHeader)
  }
}

/// 白卡里一行「名字 … 值」（素材信息、关于页）
struct InfoRow: View {
  var label: String
  var value: String
  var mono = false
  var link = false

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: 12) {
      Text(label)
        .font(.cpSecondary)
        .foregroundStyle(Palette.ink3)
        .frame(minWidth: 64, alignment: .leading)
      Text(value)
        .font(mono ? .cpMono(14, relativeTo: .subheadline) : .cpSecondary)
        .foregroundStyle(link ? Palette.accentInk : Palette.ink)
        .frame(maxWidth: .infinity, alignment: .leading)
        .multilineTextAlignment(.leading)
    }
    .padding(.vertical, 11)
    .accessibilityElement(children: .combine)
  }
}

/// 白卡里行与行之间的分隔线（左边对齐文字）
struct RowDivider: View {
  var leading: CGFloat = 0
  var body: some View {
    Rectangle().fill(Palette.line).frame(height: 0.5).padding(.leading, leading)
  }
}
