// 下载页：标题是当前看的设备（标题菜单切换，BRIEF 2.1）。「全部设备」时分「Sol 的 MacBook Pro」「这台 iPhone」两组，
// 选了某台就只看它。空的时候是居中的 logo、一句话、内嵌的输入卡（白卡）和几个能直接试的链接。
// 长按任务卡是系统的上下文菜单（2.5）。对照桌面端 `pages/Downloads.tsx`、`screenshots/downloads.png`。
import SwiftUI

struct DownloadsView: View {
  @Environment(AppModel.self) private var model
  @Environment(\.accessibilityReduceMotion) private var reduce
  @State private var cardWidth: CGFloat = 370

  /// 要显示的电脑组：全部设备时每台电脑一组（空的组不画），选了某台时只有那一台
  private var computerGroups: [(Computer, [DownloadItem])] {
    switch model.effectiveScope {
    case .phone: return []
    case .computer: return model.computer.map { [($0, model.items(of: $0))] } ?? []
    case .all: return model.computers.map { ($0, model.items(of: $0)) }.filter { !$0.1.isEmpty }
    }
  }
  private var shownComputer: [DownloadItem] { computerGroups.flatMap(\.1) }
  private var shownPhone: [DownloadItem] {
    model.effectiveScope == .computer ? [] : model.phoneItems
  }

  var body: some View {
    Group {
      if shownComputer.isEmpty && shownPhone.isEmpty {
        EmptyDownloads()
      } else {
        list
      }
    }
    .pageBackground()
    .deviceTitle()
  }

  private var list: some View {
    ScrollView {
      LazyVStack(alignment: .leading, spacing: Spacing.cards) {
        let grouped = model.effectiveScope == .all
        ForEach(computerGroups, id: \.0.id) { computer, list in
          if grouped {
            SectionLabel(verbatim: computer.name, detail: model.computerStatus(computer))
              .padding(.top, computer.id == computerGroups.first?.0.id ? 6 : 14)
          }
          ForEach(list) { card($0) }
        }
        if !shownPhone.isEmpty {
          if grouped {
            SectionLabel(verbatim: model.phoneLabel)
              .padding(.top, shownComputer.isEmpty ? 6 : 14)
          }
          ForEach(shownPhone) { card($0) }
        }
      }
      .padding(.horizontal, Spacing.page)
      .padding(.top, model.effectiveScope == .all ? 0 : 6)
      .padding(.bottom, 16)
      .onGeometryChange(for: CGFloat.self) { $0.size.width - Spacing.page * 2 } action: { cardWidth = $0 }
    }
    // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
    .scrollEdgeEffectStyle(.soft, for: .all)
  }

  /// 状态行按钮上用的电脑名：电脑上的任务用它那台，手机上的用现在看的那台
  private func computerName(_ item: DownloadItem) -> String? {
    item.device.flatMap { id in model.computers.first { $0.id == id }?.name } ?? model.computerName
  }

  private func card(_ item: DownloadItem) -> some View {
    JobCard(
      item: item, computer: computerName(item), pickSeconds: model.settings.pickSeconds, savedNote: item.placement == .phone ? model.savedNote(for: item.job) : nil,
      still: model.isDemo
    ) { model.perform($0, on: item) }
    // 新任务从上方 8pt 淡入、取消的缩到 .95 淡出（MOTION c1、c3；动画由 AppModel.rebuildItems 在增删时给）
    .transition(.listItem(reduce: reduce))
    // 长按菜单：菜单项和预览只看不随进度变的字段（.equatable()），菜单开着时进度刷新不会重建它，点得中（第二轮实测发现的）
    .contextMenu {
      JobMenuItems(item: item, computer: computerName(item)) { model.perform($0, on: item) }
        .equatable()
    } preview: {
      FrozenJobCard(item: item, computer: computerName(item), pickSeconds: model.settings.pickSeconds)
        .equatable()
        .frame(width: cardWidth)
        .background(Palette.main)
    }
  }
}

/// 任务卡长按的菜单项：和左滑操作一致（BRIEF 2.5）
private struct JobMenuItems: View, Equatable {
  var item: DownloadItem
  var computer: String?
  var act: (JobCardAction) -> Void

  /// 菜单项只取决于这些（状态、去向、能不能录、链接），进度、速度变了不算变
  static func == (a: Self, b: Self) -> Bool {
    a.item.id == b.item.id && a.item.job.state == b.item.job.state && a.item.hold == b.item.hold
      && a.item.placement == b.item.placement && a.item.job.recording == b.item.job.recording && a.item.job.live == b.item.job.live
      && a.item.job.url == b.item.job.url && a.computer == b.computer
  }

  var body: some View {
    let j = item.job
    if item.hold == nil && j.state.isActive {
      if j.state == .pause {
        Button("继续", systemImage: "play.fill") { act(.toggle) }
      } else {
        Button("暂停", systemImage: "pause.fill") { act(.toggle) }
      }
    }
    // 换设备：配对过电脑才有（远程控制接上以后才会有配对的电脑，Engine/RemoteLink.swift；现在只在演示数据里出现）
    // 换设备：等电脑上线的可以改成在手机上下；手机上还没下完的可以交给电脑。电脑上已经在跑的不换
    if computer != nil {
      if item.hold == .waitingComputer {
        Button("在手机上下", systemImage: "iphone") { act(.moveToPhone) }
      } else if item.placement == .phone && (j.state.isActive || j.state == .failed) {
        Button("在电脑上下", systemImage: "laptopcomputer") { act(.moveToComputer) }
      }
    }
    Button("复制链接", systemImage: "link") { act(.copyLink) }
    Section {
      // 「取消下载」，录直播时「取消录制」（BRIEF 2.8 第 3 条，和桌面端一样）；下完、失败的是「从列表里去掉」
      if j.state.isActive || item.hold != nil {
        Button(j.cancelLabel, systemImage: "xmark", role: .destructive) { act(.cancel) }
      } else {
        Button("从列表里去掉", systemImage: "xmark", role: .destructive) { act(.cancel) }
      }
    }
  }
}

/// 第一次打开、还没有任务：居中 logo +「粘贴链接，开始下载」+ 一句能下什么 + 内嵌输入卡 +「试试这些链接」胶囊
/// （BRIEF 2.8 第 1 条：胶囊照桌面端空状态 `.cp-try`，网站图标 + 名字，不用列表）
private struct EmptyDownloads: View {
  @Environment(AppModel.self) private var model
  @State private var text = ""
  @State private var target: Placement = .phone
  @State private var format: FormatChoice = .best
  @FocusState private var focused: Bool

  /// 都是 Blender 基金会的开放电影（CC BY），公开、能下。顺序、名字照桌面端 Downloads.tsx 的 TRIES
  private let tries: [(label: LocalizedStringKey, icon: String?, site: String?, url: String)] = [
    ("Big Buck Bunny", nil, "youtube", "https://www.youtube.com/watch?v=aqz-KE-bpKQ"),
    ("Sintel 预告片", "film", nil, "https://download.blender.org/durian/trailer/sintel_trailer-1080p.mp4"),
    ("M3U8 测试流", "list.and.film", nil, "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8"),
  ]

  /// 副标题照桌面端写法，按版本说清楚能下什么：上架版 YouTube 这类网站只能交给电脑（计划 2）
  private var subtitle: LocalizedStringKey {
    model.appStoreBuild
      ? "直链和 M3U8 在这台手机上下，YouTube、B 站这类网站交给电脑下"
      : "YouTube、B 站、X、抖音等上千个网站都可以，M3U8 和直链也行"
  }

  var body: some View {
    ScrollView {
      VStack(spacing: 0) {
        CaptaeLogo(size: 56)
          .padding(.bottom, 18)
        Text("粘贴链接，开始下载")
          .font(.cpHero)
          .foregroundStyle(Palette.ink)
          .multilineTextAlignment(.center)
        Text(subtitle)
          .font(.cpSecondary)
          .foregroundStyle(Palette.ink3)
          .multilineTextAlignment(.center)
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, 12)
          .padding(.top, 6)

        ComposerCard(style: .embedded, text: $text, target: $target, format: $format, focus: $focused) {
          focused = false
          model.composer = ComposerSession(text: text, target: target, reading: true, format: format)
          text = ""
        }
        .padding(.top, 26)

        VStack(spacing: 10) {
          Text("试试这些链接")
            .font(.cpMeta)
            .foregroundStyle(Palette.ink3)
            .accessibilityAddTraits(.isHeader)
          CenteredFlow(spacing: 8, lineSpacing: 8) {
            ForEach(Array(tries.enumerated()), id: \.offset) { _, t in
              Button {
                text = t.url
              } label: {
                TryChip(label: t.label, icon: t.icon, site: t.site, url: t.url)
              }
              // 这一轮不做动效（BRIEF 第 1 节），按下的反馈用系统 plain 样式自带的
              .buttonStyle(.plain)
              .accessibilityHint("填进输入框")
            }
          }
        }
        .padding(.top, 24)
      }
      .padding(.horizontal, Spacing.page)
      .padding(.top, 28)
      .padding(.bottom, 24)
    }
    // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
    .scrollEdgeEffectStyle(.soft, for: .all)
    .scrollBounceBehavior(.basedOnSize)
    .onAppear {
      target = model.computer != nil ? .computer : .phone
    }
  }
}

/// 「试试这些链接」的一颗胶囊：网站图标 + 名字（桌面端 `.cp-try`：浅灰底、无描边、次要文字色）
private struct TryChip: View {
  var label: LocalizedStringKey
  var icon: String?
  var site: String?
  var url: String

  var body: some View {
    HStack(spacing: 7) {
      Group {
        if let site {
          SiteIcon(plat: site, url: url, size: 15)
        } else if let icon {
          Image(systemName: icon).font(.system(size: 13, weight: .medium))
        }
      }
      .opacity(0.85)
      Text(label)
        .font(.cpMetaMedium)
        .lineLimit(1)
    }
    .foregroundStyle(Palette.ink2)
    .padding(.leading, 12)
    .padding(.trailing, 14)
    .padding(.vertical, 9)
    .background(Color.black.opacity(0.045), in: .capsule)
    .contentShape(.capsule)
  }
}

/// 一行放不下就换行、每行居中（「试试这些链接」的胶囊，桌面端 `.cp-tries` 的 flex-wrap + justify-center）
struct CenteredFlow: Layout {
  var spacing: CGFloat = 8
  var lineSpacing: CGFloat = 8

  private func lines(_ sizes: [CGSize], width: CGFloat) -> [[Int]] {
    var out: [[Int]] = [[]]
    var x: CGFloat = 0
    for (i, s) in sizes.enumerated() {
      let need = out[out.count - 1].isEmpty ? s.width : x + spacing + s.width
      if need > width, !out[out.count - 1].isEmpty {
        out.append([i])
        x = s.width
      } else {
        out[out.count - 1].append(i)
        x = need
      }
    }
    return out
  }

  func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
    let width = proposal.width ?? .infinity
    let sizes = subviews.map { $0.sizeThatFits(.unspecified) }
    let rows = lines(sizes, width: width)
    let height = rows.reduce(CGFloat(0)) { h, row in h + (row.map { sizes[$0].height }.max() ?? 0) } + lineSpacing * CGFloat(max(rows.count - 1, 0))
    let widest = rows.map { row in row.reduce(CGFloat(0)) { $0 + sizes[$1].width } + spacing * CGFloat(max(row.count - 1, 0)) }.max() ?? 0
    return CGSize(width: proposal.width ?? widest, height: height)
  }

  func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
    let sizes = subviews.map { $0.sizeThatFits(.unspecified) }
    var y = bounds.minY
    for row in lines(sizes, width: bounds.width) {
      let rowWidth = row.reduce(CGFloat(0)) { $0 + sizes[$1].width } + spacing * CGFloat(max(row.count - 1, 0))
      let rowHeight = row.map { sizes[$0].height }.max() ?? 0
      var x = bounds.minX + max(0, (bounds.width - rowWidth) / 2)
      for i in row {
        subviews[i].place(at: CGPoint(x: x, y: y + (rowHeight - sizes[i].height) / 2), proposal: ProposedViewSize(sizes[i]))
        x += sizes[i].width + spacing
      }
      y += rowHeight + lineSpacing
    }
  }
}

/// 长按时浮起来的卡片：按下那一刻的样子，菜单开着时进度不再跟着刷新（只在状态换了时才换）
private struct FrozenJobCard: View, Equatable {
  var item: DownloadItem
  var computer: String?
  var pickSeconds: Int

  static func == (a: Self, b: Self) -> Bool {
    a.item.id == b.item.id && a.item.job.state == b.item.job.state && a.item.job.phase == b.item.job.phase && a.item.hold == b.item.hold
  }

  var body: some View {
    JobCard(item: item, computer: computer, pickSeconds: pickSeconds, still: true)
  }
}
