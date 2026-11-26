// 演示路由：启动参数 `-captaeDemo <编号>-<状态>`（如 `-captaeDemo 01-list`）直接打开某一页的某个状态，
// 截设计稿用（BRIEF 第 4 节、5.1 节的表）。不带参数时打开 01-list。sheet 和全屏页打开后自动弹出来。
// 系统把 `-key value` 形式的启动参数放进 UserDefaults，这里从那里读。
// 只在 Debug 编进去；不带 `-captaeDemo` 时 App 走真数据（Rust 核心）。
// 要靠操作才出来的状态（09-menu 点标题、11-context 长按、01-list-scrolled 滑动、01-list-large 改字号）
// 数据和 01-list 一样，操作由截图脚本用设备命令做。
#if DEBUG
import Foundation

struct DemoRoute: Equatable {
  var page: String
  var state: String

  /// 设计稿的全部状态（编号 → 状态），和 BRIEF 5.1 一致
  static let all: [String: [String]] = [
    "01": ["empty", "list", "list-mac", "list-scrolled", "errors", "list-large"],
    "02": ["running", "pick", "failed"],
    "03": ["empty", "grid", "grid-mac", "offline"],
    "04": ["video", "remote", "saving"],
    "05": ["none", "online", "offline"],
    "06": ["scan", "code", "failed"],
    "07": ["main", "proxy", "about"],
    "08": ["reading", "ready", "offline"],
    "09": ["menu"],
    "10": ["composer", "composer-reading", "composer-redirect"],
    "11": ["context"],
    "12": ["search"],
  ]

  /// 从启动参数读；没带 `-captaeDemo` 时为空（走真数据），认不出的参数退回 01-list
  static func fromLaunchArguments(_ defaults: UserDefaults = .standard) -> DemoRoute? {
    guard let raw = defaults.string(forKey: "captaeDemo") else { return nil }
    let parts = raw.split(separator: "-", maxSplits: 1).map(String.init)
    guard parts.count == 2, all[parts[0]]?.contains(parts[1]) == true else { return DemoRoute(page: "01", state: "list") }
    return DemoRoute(page: parts[0], state: parts[1])
  }

  /// 把这一页这个状态的数据和导航填进 App 状态（弹出来的东西另见 `present`）
  func apply(to m: AppModel) {
    m.demo = true
    // 设计稿是按上架版截的（10-composer-redirect 另外打开），其余页面不受它影响
    m.appStoreBuild = false
    m.phoneName = "iPhone"
    m.computer = Demo.computer
    m.items = Demo.onComputer + Demo.onPhone
    m.phoneAssets = Demo.phoneAssets
    m.computerAssets = Demo.computerAssets
    m.computerAssetCount = Demo.computerAssetCount
    m.nearby = Demo.nearby
    // 配对过时默认「全部设备」（BRIEF 2.1）
    m.scope = .all

    switch (page, state) {
    case ("01", "empty"):
      m.computer = nil
      m.items = []
      m.scope = .phone
    case ("01", "list-mac"):
      m.scope = .computer
    case ("01", "errors"), ("02", "failed"):
      m.items = [Demo.sintelRunning] + Demo.failures
    case ("03", _):
      m.tab = .library
      switch state {
      case "empty":
        m.computer = nil
        m.phoneAssets = []
        m.items = []
        m.scope = .phone
      case "grid-mac":
        m.scope = .computer
      case "offline":
        m.computer = Demo.computerOffline
        m.items = Demo.onPhone + Demo.waiting
        m.scope = .computer
      default: break
      }
    case ("04", _):
      m.tab = .library
      if state == "video" {
        m.libraryPath = [.asset(Demo.bbb, .phone)]
      } else {
        m.libraryPath = [.asset(Demo.tearsFull, .computer)]
        if state == "saving" { m.saving[Demo.tearsFull.id] = SavingState(fraction: 0.62, speed: 12.5) }
      }
    case ("05", _):
      if state == "none" {
        m.computer = nil
        m.items = []
        m.scope = .phone
      }
      if state == "offline" {
        m.computer = Demo.computerOffline
        m.waiting = Demo.waiting
        m.items = Demo.onPhone + Demo.waiting
        m.scope = .computer
      }
    case ("06", _):
      m.computer = nil
      m.items = []
      m.scope = .phone
    case ("07", _):
      m.downloadsPath = state == "about" ? [.settings, .about] : [.settings]
      if state == "proxy" {
        m.settings.proxy = .manual
        m.settings.proxyURL = "http://127.0.0.1:7890"
      }
    case ("08", "offline"):
      m.computer = Demo.computerOffline
    case ("10", "composer"):
      m.demoFocusComposer = true
    case ("10", "composer-redirect"):
      m.appStoreBuild = true
      m.demoFocusComposer = true
    case ("12", _):
      m.tab = .search
      m.searchQuery = "Sintel"
    default:
      break
    }
  }

  /// 弹出来的东西：等界面出来以后再弹，和真的点开一样从下面升上来
  func present(on m: AppModel) {
    let youtube = "https://www.youtube.com/watch?v=aqz-KE-bpKQ"
    switch (page, state) {
    case ("02", "running"): m.detail = Demo.sintelRunning
    case ("02", "pick"): m.detail = Demo.onComputer[1]
    case ("02", "failed"): m.detail = Demo.needLogin
    case ("05", _): m.manageComputers = true
    case ("06", "scan"): m.pairing = PairingSession(start: .scan)
    // 输入配对码在「管理电脑」sheet 里推进去（BRIEF 2.8 第 6 条）
    case ("06", "code"):
      m.managePath = [.code(code: "482")]
      m.manageComputers = true
    case ("06", "failed"):
      m.managePath = [.code(code: "482915", expired: true)]
      m.manageComputers = true
    case ("08", _): m.share = ShareCardState(rawValue: state)
    case ("10", "composer"): m.composer = ComposerSession(text: youtube)
    case ("10", "composer-reading"): m.composer = ComposerSession(text: youtube, reading: true)
    // 上架版：选了这台 iPhone、填了 YouTube 链接 → 去向自动改到电脑，下面一行橙色小字
    case ("10", "composer-redirect"): m.composer = ComposerSession(text: youtube, target: .phone)
    default: break
    }
  }
}
#endif
