// 底部标签：下载、素材库，加系统的搜索标签（BRIEF 2.2）。系统标签栏（iOS 26 起是 Liquid Glass），往下滚时收起。
// 标签只画图标（2026-11-24 用户定）：`Tab` 的 label 只给一个 Image，名字写进无障碍标签；选中的用填充图标。
// 标签栏上方是下载条（`tabViewBottomAccessory`，位置和玻璃都是系统的，不加背景），滚动时系统把它缩进标签栏那一行。
// 每个标签一个 NavigationStack。不给标签栏、导航栏加任何背景。
import SwiftUI

struct RootTabs: View {
  @Environment(AppModel.self) private var model

  var body: some View {
    @Bindable var model = model
    TabView(selection: $model.tab) {
      Tab(value: AppTab.downloads) {
        NavigationStack(path: $model.downloadsPath) {
          DownloadsView().routes()
        }
      } label: {
        TabIcon(name: "arrow.down.circle", title: "下载", selected: model.tab == .downloads)
      }
      Tab(value: AppTab.library) {
        NavigationStack(path: $model.libraryPath) {
          LibraryView().routes()
        }
      } label: {
        TabIcon(name: "photo.on.rectangle.angled", title: "素材库", selected: model.tab == .library)
      }
      // 搜索标签（role: .search）。label 同样只给图标：系统默认的 label 会带「搜索」两个字
      Tab(value: AppTab.search, role: .search) {
        NavigationStack {
          SearchView()
        }
      } label: {
        TabIcon(name: "magnifyingglass", title: "搜索", selected: false)
      }
    }
    .tabBarMinimizeBehavior(.onScrollDown)
    // 下载条：下载页、素材库都有；搜索标签打开时系统的搜索框占了这一行，不放
    .modifier(DownloadBarAccessory(enabled: model.tab != .search))
    .sheet(item: $model.detail) { item in
      JobDetailSheet(item: item)
    }
    .sheet(item: $model.share) { state in
      shareDemo(state)
    }
    .sheet(item: $model.composer) { session in
      ComposerSheet(session: session, fallback: model.computer?.online == true ? .computer : .phone)
    }
    .sheet(isPresented: $model.manageComputers) {
      ManageComputersSheet()
    }
    .fullScreenCover(item: $model.pairing) { _ in
      PairingFlow { model.enterPairingCode() }
    }
    // 只提示一次的事（核心的 notice、没能存进相册……）：系统提醒框
    .alert(
      model.notice?.title ?? "", isPresented: Binding(get: { model.notice != nil }, set: { if !$0 { model.notice = nil } }),
      presenting: model.notice
    ) { _ in
      Button("好") {}
    } message: { n in
      Text(verbatim: n.text)
    }
  }

  /// 分享卡片：App 里只有设计稿截图用（真的分享卡片在分享扩展里，CaptaeShare/）
  @ViewBuilder
  private func shareDemo(_ state: ShareCardState) -> some View {
    #if DEBUG
      let j = Demo.shared
      ShareCard(
        content: ShareCardContent(url: j.url, title: j.title, thumb: j.thumb, fmt: j.fmt, dur: j.dur, reading: state == .reading),
        computerName: model.computerName, computerOnline: model.computer?.online == true,
        onCancel: { model.share = nil }, onConfirm: { _ in model.share = nil })
    #endif
  }
}

/// 下载条放进 `tabViewBottomAccessory`。按标签开关要 iOS 26.1（`isEnabled:`），26.0 上一直显示
private struct DownloadBarAccessory: ViewModifier {
  var enabled: Bool

  func body(content: Content) -> some View {
    if #available(iOS 26.1, *) {
      content.tabViewBottomAccessory(isEnabled: enabled) { DownloadBar() }
    } else {
      content.tabViewBottomAccessory { DownloadBar() }
    }
  }
}

/// 标签图标：只有图标，选中的用填充版；名字给读屏和长按显示用
private struct TabIcon: View {
  var name: String
  var title: LocalizedStringKey
  var selected: Bool

  var body: some View {
    Image(systemName: selected ? "\(name).fill" : name)
      .accessibilityLabel(Text(title))
  }
}

extension View {
  /// 各标签共用的推入页面
  func routes() -> some View {
    navigationDestination(for: Route.self) { route in
      switch route {
      case .settings: SettingsView()
      case .about: AboutView()
      case let .asset(asset, source): AssetDetailView(asset: asset, source: source)
      }
    }
  }

  /// 主页面的顶栏：标题是当前看的设备，点开是系统的标题菜单（BRIEF 2.1）；右上角「更多」
  func deviceTitle() -> some View {
    modifier(DeviceTitle())
  }
}

/// 标题即设备菜单：`.navigationTitle` + `.navigationSubtitle` + `.toolbarTitleMenu`，inline 显示
private struct DeviceTitle: ViewModifier {
  @Environment(AppModel.self) private var model

  func body(content: Content) -> some View {
    content
      .navigationTitle(model.scopeTitle)
      .navigationSubtitle(model.scopeSubtitle)
      .navigationBarTitleDisplayMode(.inline)
      .toolbarTitleMenu { DeviceMenu() }
      .toolbar {
        ToolbarItem(placement: .topBarTrailing) { MoreMenu() }
      }
  }
}

/// 设备菜单：全部设备 / 这台 iPhone / 电脑（小字「在线」）/ 分隔 / 添加电脑… / 管理电脑…
private struct DeviceMenu: View {
  @Environment(AppModel.self) private var model

  var body: some View {
    @Bindable var model = model
    // 每台设备一个勾选项：选中的打勾；电脑下面一行小字是在不在线（菜单项里 Label 后面再跟一个 Text = 副标题）
    Section {
      if model.computer != nil {
        ScopeToggle(scope: .all, title: String(localized: "全部设备"), icon: "square.stack", subtitle: nil)
      }
      ScopeToggle(scope: .phone, title: model.phoneLabel, icon: "iphone", subtitle: nil)
      ForEach(model.computers) { computer in
        ScopeToggle(
          scope: .computer, title: computer.name, icon: "laptopcomputer", subtitle: model.computerStatus(computer),
          computerID: computer.id)
      }
    }
    Section {
      Button("添加电脑…", systemImage: "plus") { model.pairing = PairingSession(start: .scan) }
      Button("管理电脑…", systemImage: "laptopcomputer.and.iphone") {
        model.managePath = []
        model.manageComputers = true
      }
    }
  }
}

/// 设备菜单里的一项：系统菜单的勾选项（Toggle），点了切到这台
private struct ScopeToggle: View {
  var scope: DeviceScope
  var title: String
  var icon: String
  var subtitle: String?
  /// 选的是哪台电脑（scope 是 .computer 时）
  var computerID: String? = nil
  @Environment(AppModel.self) private var model

  private var isOn: Bool {
    model.effectiveScope == scope && (computerID == nil || model.computer?.id == computerID)
  }

  var body: some View {
    Toggle(
      isOn: Binding(
        get: { isOn },
        set: {
          guard $0 else { return }
          if let computerID { model.computerID = computerID }
          model.scope = scope
          model.reloadComputerLibrary()
        })
    ) {
      Label(title, systemImage: icon)
      if let subtitle { Text(verbatim: subtitle) }
    }
  }
}

/// 右上角「更多」：全部暂停 / 全部继续、清空已完成、设置
private struct MoreMenu: View {
  @Environment(AppModel.self) private var model

  var body: some View {
    Menu("更多", systemImage: "ellipsis") {
      if !model.items.isEmpty {
        if model.runningItems.isEmpty {
          Button("全部继续", systemImage: "play.fill") { model.resumeAll() }
        } else {
          Button("全部暂停", systemImage: "pause.fill") { model.pauseAll() }
        }
        Button("清空已完成", systemImage: "checkmark.circle") { model.clearDone() }
      }
      Section {
        Button("设置", systemImage: "gearshape") {
          model.tab = .downloads
          model.downloadsPath.append(.settings)
        }
      }
    }
  }
}
