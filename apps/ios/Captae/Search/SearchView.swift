// 搜索标签（BRIEF 2.2）：系统的搜索标签（`Tab(role: .search)`，系统放在最右、单独一个圆），
// 搜所有设备上的任务和素材的标题、来源、标签。结果分「下载」「素材库」两段，每条注明在哪台设备上。
// 搜索框用系统的 `.searchable`，不加背景。
import SwiftUI

struct SearchView: View {
  @Environment(AppModel.self) private var model

  private var query: String { model.searchQuery.trimmingCharacters(in: .whitespaces) }

  private func hit(_ s: String...) -> Bool { s.contains { $0.localizedCaseInsensitiveContains(query) } }

  private var jobs: [DownloadItem] {
    model.items.filter { hit($0.job.title, $0.job.siteName, $0.job.src) }
  }

  /// 素材：先电脑上的、再手机上的；同名的两台都列出来（在哪台设备上不一样）
  private var assets: [(Asset, LibrarySource)] {
    let c = model.computer == nil ? [] : model.computerAssets.filter { hit($0.title, $0.src) }.map { ($0, LibrarySource.computer) }
    // 手机上的在核心里搜（标题、简介、标签、来源，不受素材库分页影响）；演示数据在这里筛
    let phone = model.isDemo ? model.phoneAssets.filter { hit($0.title, $0.src) } : model.searchHits
    let p = phone.map { ($0, LibrarySource.phone) }
    return p + c
  }

  var body: some View {
    @Bindable var model = model
    ScrollView {
      VStack(alignment: .leading, spacing: 8) {
        if query.isEmpty {
          Text("搜所有设备上的下载和素材：标题、来源、标签")
            .font(.cpSecondary)
            .foregroundStyle(Palette.ink3)
            .padding(.horizontal, 4)
            .padding(.top, 8)
        } else if jobs.isEmpty && assets.isEmpty {
          ContentUnavailableView.search(text: query)
            .padding(.top, 60)
        } else {
          if !jobs.isEmpty {
            SectionLabel(title: "下载", detail: String(localized: "\(jobs.count) 个"))
              .padding(.top, 6)
            VStack(spacing: 0) {
              ForEach(Array(jobs.enumerated()), id: \.element.id) { i, item in
                Button {
                  model.detail = item
                } label: {
                  ResultRow(
                    thumb: item.job.thumb, audio: item.job.audio, title: item.job.title,
                    device: device(item.placement), detail: jobState(item))
                }
                .buttonStyle(.plain)
                if i < jobs.count - 1 { RowDivider(leading: 90) }
              }
            }
            .card()
          }
          if !assets.isEmpty {
            SectionLabel(title: "素材库", detail: String(localized: "\(assets.count) 项"))
              .padding(.top, 14)
            VStack(spacing: 0) {
              ForEach(Array(assets.enumerated()), id: \.offset) { i, pair in
                Button {
                  model.tab = .library
                  model.libraryPath = [.asset(pair.0, pair.1)]
                } label: {
                  ResultRow(
                    thumb: pair.0.thumb, audio: pair.0.kind == .audio, title: pair.0.title,
                    device: device(pair.1 == .computer ? .computer : .phone),
                    detail: [pair.0.fmt, pair.0.dur].filter { !$0.isEmpty }.joined(separator: " · "))
                }
                .buttonStyle(.plain)
                if i < assets.count - 1 { RowDivider(leading: 90) }
              }
            }
            .card()
          }
        }
      }
      .padding(.horizontal, Spacing.page)
      .padding(.bottom, 20)
    }
    // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
    .scrollEdgeEffectStyle(.soft, for: .all)
    .pageBackground()
    .navigationTitle("搜索")
    .navigationSubtitle(model.computer == nil ? model.phoneLabel : String(localized: "全部设备"))
    .navigationBarTitleDisplayMode(.inline)
    .searchable(text: $model.searchQuery, prompt: Text("搜标题、来源、标签"))
    .onChange(of: model.searchQuery, initial: true) { _, q in model.searchLibrary(q) }
  }

  private func device(_ p: Placement) -> String {
    p == .computer ? (model.computerName ?? "") : model.phoneLabel
  }

  private func jobState(_ item: DownloadItem) -> String {
    switch item.view {
    case .dl: Fmt.percent(item.job.progress)
    case .pause: String(localized: "已暂停")
    case .queue: String(localized: "排队中")
    case .pick: String(localized: "等你选清晰度")
    case .probe: String(localized: "正在读取")
    case .done: String(localized: "已完成")
    case .waiting: String(localized: "等电脑上线")
    default: String(localized: "没有成功")
    }
  }
}

/// 一条结果：封面小图 + 标题 + 「在哪台设备 · 状态或格式」
private struct ResultRow: View {
  var thumb: String?
  var audio: Bool
  var title: String
  var device: String
  var detail: String

  var body: some View {
    HStack(spacing: 12) {
      Thumb(name: thumb, radius: 6, audio: audio)
        .frame(width: 64, height: 36)
      VStack(alignment: .leading, spacing: 2) {
        Text(verbatim: title)
          .font(.cpSecondaryMedium)
          .foregroundStyle(Palette.ink)
          .lineLimit(2)
        HStack(spacing: 5) {
          Image(systemName: device.contains("iPhone") ? "iphone" : "laptopcomputer")
            .font(.system(size: 11))
          Text(verbatim: [device, detail].filter { !$0.isEmpty }.joined(separator: " · "))
            .lineLimit(1)
        }
        .font(.cpMeta)
        .monospacedDigit()
        .foregroundStyle(Palette.ink3)
      }
      Spacer(minLength: 0)
      Image(systemName: "chevron.right")
        .font(.system(size: 12, weight: .semibold))
        .foregroundStyle(Palette.ink4)
    }
    .padding(.horizontal, 12)
    .padding(.vertical, 10)
    .contentShape(.rect)
    .accessibilityElement(children: .combine)
  }
}
