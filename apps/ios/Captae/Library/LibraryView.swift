// 素材库：标题是当前看的设备（标题菜单切换，BRIEF 2.1），第一版顶上的「这台手机 / 电脑」分段控件去掉了。
// 「全部设备」时分两段（电脑、这台 iPhone），选了某台就只看它。类型分段「全部 / 视频 / 音频 / 图片」，两列卡片。
// 搜索挪到了搜索标签（搜所有设备）。电脑不在线时显示上次看到的内容（变灰）。
// 长按素材卡是系统的上下文菜单（2.5）。对照桌面端 `pages/Library.tsx`、`screenshots/library.jpg`。
import SwiftUI

enum KindFilter: String, CaseIterable, Identifiable {
  case all = "全部"
  case video = "视频"
  case audio = "音频"
  case image = "图片"
  var id: String { rawValue }

  func matches(_ a: Asset) -> Bool {
    switch self {
    case .all: true
    case .video: a.kind == .video
    case .audio: a.kind == .audio
    case .image: a.kind == .image
    }
  }
}

struct LibraryView: View {
  @Environment(AppModel.self) private var model
  @State private var kind: KindFilter = .all
  /// 长按菜单里点了删除：先问一次（删掉文件，不能撤销）
  @State private var deleting: Asset?

  private var scope: DeviceScope { model.effectiveScope }
  /// 要显示的电脑：全部设备时每台一段，选了某台时只有它
  private var shownComputers: [Computer] {
    switch scope {
    case .phone: []
    case .computer: model.computer.map { [$0] } ?? []
    case .all: model.computers
    }
  }
  private var showPhone: Bool { scope != .computer }
  /// 这台电脑现在查不了：离线，或者查的时候出错（显示上次看到的、变灰）
  private func offline(_ c: Computer) -> Bool { !c.online || model.shelves[c.id]?.error != nil }
  private var empty: Bool { scope == .phone && model.phoneAssets.isEmpty }

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 14) {
        if empty {
          EmptyLibrary { model.tab = .downloads }
        } else {
          WellSegments(
            items: KindFilter.allCases.map { .init(value: $0, title: String(localized: String.LocalizationValue($0.rawValue))) },
            selection: $kind,
            label: String(localized: "类型")
          )
          ForEach(shownComputers) { computerSection($0) }
          if showPhone {
            phoneSection
          }
        }
      }
      .padding(.horizontal, Spacing.page)
      .padding(.top, 6)
      .padding(.bottom, 20)
    }
    // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
    .scrollEdgeEffectStyle(.soft, for: .all)
    .pageBackground()
    .deviceTitle()
    // 手机上的素材按类型在核心里查（分页）；演示数据在这里筛
    .onChange(of: kind) { _, k in model.setLibraryKind(k) }
    // 电脑的素材库：每次看这一页时向电脑要一次（不联网的部分在 RemoteHub 里缓存着封面）
    .task(id: model.effectiveScope) { if !model.isDemo { model.reloadComputerLibrary() } }
    .confirmationDialog(
      "从这台手机上删掉？", isPresented: Binding(get: { deleting != nil }, set: { if !$0 { deleting = nil } }), titleVisibility: .visible,
      presenting: deleting
    ) { a in
      Button("删除", role: .destructive) { model.deleteAsset(a) }
    } message: { _ in
      Text("文件会从 Captae 里删掉。已经存进相册的不受影响")
    }
  }

  // MARK: 电脑上的

  private func computerSection(_ c: Computer) -> some View {
    let shelf = model.shelves[c.id] ?? ComputerShelf()
    let off = offline(c)
    return VStack(alignment: .leading, spacing: 8) {
      if scope == .all {
        SectionLabel(verbatim: c.name, detail: String(localized: "\(shelf.total) 项"))
      }
      if off {
        StatusRow(tone: .plain, icon: "wifi.slash", title: cjk("", c.name, String(localized: "现在连不上")), hint: String(localized: "回到同一个 Wi-Fi，打开电脑上的 Captae"))
      }
      // 全部设备时电脑这段只放最新的 4 个，下面一行「查看全部」切到电脑
      let all = shelf.assets.filter(kind.matches)
      grid(scope == .all ? Array(all.prefix(4)) : all, source: .computer, computerID: c.id)
        .opacity(off ? 0.45 : 1)
        .saturation(off ? 0 : 1)
        .allowsHitTesting(!off)
        .accessibilityHint(off ? "上次看到的内容，电脑连上以后刷新" : "")
      // 选了这台电脑、翻到底：再读一页
      if scope == .computer && !off && shelf.assets.count < shelf.total {
        ProgressView()
          .frame(maxWidth: .infinity)
          .padding(.vertical, 12)
          // 按已读条数触发：转圈一直在屏幕上时 onAppear 只来一次，读完一页要再读下一页
          .task(id: shelf.assets.count) { model.loadMoreComputerLibrary() }
      }
      if scope == .all && shelf.total > 0 {
        Button {
          model.computerID = c.id
          model.scope = .computer
          model.reloadComputerLibrary()
        } label: {
          HStack(spacing: 4) {
            Text("查看全部 \(shelf.total) 项")
            Image(systemName: "chevron.right").font(.system(size: 11, weight: .semibold))
          }
          .font(.cpMetaMedium)
          .foregroundStyle(Palette.accentInk)
          .padding(.horizontal, 4)
          .padding(.top, 2)
        }
        .buttonStyle(.plain)
      }
    }
  }

  // MARK: 这台手机上的

  private var phoneSection: some View {
    VStack(alignment: .leading, spacing: 8) {
      if scope == .all {
        SectionLabel(verbatim: model.phoneLabel, detail: String(localized: "\(model.phoneAssets.count) 项"))
          .padding(.top, 10)
      }
      if model.phoneAssets.isEmpty {
        Text("这台手机上还没有下载完成的")
          .font(.cpMeta)
          .foregroundStyle(Palette.ink3)
          .padding(.horizontal, 4)
      } else {
        grid(model.phoneAssets.filter(kind.matches), source: .phone)
        // 翻到底：再读一页
        if model.phoneAssets.count < model.phoneAssetTotal {
          ProgressView()
            .frame(maxWidth: .infinity)
            .padding(.vertical, 12)
            .task(id: model.phoneAssets.count) { model.loadMoreLibrary() }
        }
      }
    }
  }

  /// `computerID`：电脑那一段的素材属于哪台（点进去以后「存到手机」从它那里下）
  private func grid(_ assets: [Asset], source: LibrarySource, computerID: String? = nil) -> some View {
    LazyVGrid(columns: [GridItem(.flexible(), spacing: Spacing.cards), GridItem(.flexible(), spacing: Spacing.cards)], spacing: Spacing.cards) {
      ForEach(assets) { asset in
        NavigationLink(value: Route.asset(asset, source)) {
          AssetTile(asset: asset)
        }
        // 素材卡按下缩到 .98（MOTION j，审查第 15 条）
        .buttonStyle(.cardPress)
        .simultaneousGesture(TapGesture().onEnded { if let computerID { model.computerID = computerID } })
        .contextMenu {
          AssetMenuItems(asset: asset, source: source) { deleting = asset }
        } preview: {
          AssetTile(asset: asset)
            .frame(width: 180)
            .background(Palette.main)
        }
      }
    }
  }
}

/// 素材卡长按的菜单项：和左滑操作一致（BRIEF 2.5）
private struct AssetMenuItems: View {
  var asset: Asset
  var source: LibrarySource
  var delete: () -> Void
  @Environment(AppModel.self) private var model
  @Environment(\.openURL) private var openURL

  var body: some View {
    if source == .phone, !asset.path.isEmpty {
      ShareLink(item: URL(filePath: asset.path)) { Label("分享", systemImage: "square.and.arrow.up") }
    }
    if source == .computer {
      // 存到手机要远程控制接上以后才有（Engine/RemoteLink.swift）
      Button("存到手机", systemImage: "arrow.down.to.line") {}
    } else {
      Button("发到电脑", systemImage: "laptopcomputer.and.arrow.down") { model.openPairing() }
      if let url = model.saver.filesURL(asset.id) {
        Button("在「文件」中查看", systemImage: "folder") { openURL(url) }
      }
    }
    if source == .phone {
      Section {
        Button("删除", systemImage: "trash", role: .destructive, action: delete)
      }
    }
  }
}

/// 素材库还是空的
private struct EmptyLibrary: View {
  var goDownload: () -> Void

  var body: some View {
    VStack(spacing: 0) {
      Image(systemName: "photo.on.rectangle.angled")
        .font(.system(size: 34, weight: .light))
        .foregroundStyle(Palette.ink4)
        .padding(.bottom, 16)
      Text("素材库还是空的")
        .font(.cpHero)
        .foregroundStyle(Palette.ink)
      Text("下载完成的视频、音乐和图片会出现在这里")
        .font(.cpSecondary)
        .foregroundStyle(Palette.ink3)
        .multilineTextAlignment(.center)
        .padding(.top, 6)
      Button("去下载", action: goDownload)
        .buttonStyle(.primary)
        .padding(.top, 22)
    }
    .frame(maxWidth: .infinity)
    .padding(.top, 140)
  }
}
