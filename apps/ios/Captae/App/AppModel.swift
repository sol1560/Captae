// App 的状态：现在在哪个标签、看的是哪台设备、每个标签推进去了哪几页、弹着什么，以及各页要的数据。
// 数据来自 Rust 核心（AppModel+Local.swift：任务、素材库、设置、事件）；Debug 版带 `-captaeDemo` 启动参数时
// 改用演示数据（DemoRoute 填进来，不打开核心），截设计稿用。
// 第二版导航（BRIEF 第 2 节）：设备是第一层（标题即菜单），标签只剩「下载 / 素材库 / 搜索」，
// 「电脑」标签挪进「管理电脑」sheet，常驻输入框换成下载条 + 输入卡。
import CaptaeKit
import Observation
import SwiftUI

enum AppTab: Hashable {
  case downloads, library, search
}

/// 标题菜单里选的「看哪台设备」（BRIEF 2.1）
enum DeviceScope: Hashable {
  /// 全部设备：下载页、素材库分组显示
  case all
  /// 这台手机
  case phone
  /// 配对的那台电脑
  case computer
}

/// 标签里推进去的页面
enum Route: Hashable {
  case settings
  case about
  case asset(Asset, LibrarySource)
}

/// 分享卡片（Share Extension 的界面）现在是哪种样子
enum ShareCardState: String, Identifiable {
  /// 正在读取视频信息
  case reading
  /// 读到了，选去向和清晰度
  case ready
  /// 电脑不在线：先存着
  case offline

  var id: String { rawValue }
}

/// 配对流程走到哪一步
enum PairingStep: Hashable {
  case scan
  case code
}

/// 配对页弹出来时从哪一步开始
struct PairingSession: Identifiable, Hashable {
  var id = UUID()
  var start: PairingStep
  /// 6 位数已经填了的部分
  var code: String = ""
  /// 配对码过期了
  var expired = false
}

/// 「管理电脑」sheet 里推进去的页面（BRIEF 2.8 第 5、6 条）
enum ManageRoute: Hashable {
  /// 一台配对过的电脑：看详情、撤销
  case computer(String)
  /// 输入配对码。`code` 是已经填了的部分，`expired` 是配对码过期了
  case code(code: String = "", expired: Bool = false)
}

/// 输入卡 sheet（BRIEF 2.4）：打开时带的链接、去向，点了发送以后读取中
struct ComposerSession: Identifiable, Hashable {
  var id = UUID()
  var text = ""
  /// 打开时选的去向；为空时按默认规则（电脑在线 → 电脑，否则手机）
  var target: Placement?
  /// 点了发送、正在读取视频信息（2.6 的流光）
  var reading = false
  /// 保存成什么（下载页空状态里内嵌的输入卡选好了带过来）
  var format: FormatChoice = .best
}

/// 把电脑上的素材存到手机：走到哪了（计划 3.3，局域网下载，支持断点续传）。
/// 叫 SavingState 不叫 SaveProgress：CaptaeKit 里 RemoteHub 的记录叫 SaveProgress
struct SavingState: Hashable {
  /// 0…1
  var fraction: Double
  /// MB/s
  var speed: Double
  /// RemoteHub 里这条记录的 id（演示数据里没有）
  var id: UInt64? = nil
  var paused = false
}

/// 设置里的选项（手机端）
struct PhoneSettings: Hashable {
  enum SaveTo: String, CaseIterable, Identifiable {
    case photosAndFiles = "相册和文件"
    case files = "都存到文件"
    var id: String { rawValue }
  }

  /// 默认清晰度（只对网页链接有用）。显示用的名字 → 核心的协议值（「最高可用」不翻译，docs/I18N.md 2.4）
  static let qualities: [(label: String, value: String)] = [("最清晰", "最高可用"), ("1080p", "1080p"), ("720p", "720p"), ("480p", "480p")]
  /// 默认格式：MP4 /「保持原格式」（手机上没有 ffmpeg，直链不转格式）
  static let formats: [(label: String, value: String)] = [("MP4", "MP4"), ("保持原来的格式", "保持原格式")]

  enum Proxy: String, CaseIterable, Identifiable {
    case none = "不用"
    case manual = "手动填写"
    var id: String { rawValue }
  }

  var saveTo: SaveTo = .photosAndFiles
  var quality = "最清晰"
  var format = "MP4"
  var concurrency = 3
  var proxy: Proxy = .none
  var proxyURL = ""
  /// M3U8 不选清晰度时几秒后自动用最高
  var pickSeconds = 10
}

/// 一台电脑的素材库（查到的这一页）
struct ComputerShelf: Hashable {
  var assets: [Asset] = []
  var total = 0
  /// 查不了的原因（电脑离线、被撤销……）；为空时是好的
  var error: String?
}

@Observable
final class AppModel {
  var tab: AppTab = .downloads
  var scope: DeviceScope = .all
  var downloadsPath: [Route] = []
  var libraryPath: [Route] = []

  /// 配对过的电脑（RemoteHub 的设备列表里除了这台手机的那些）
  var computers: [Computer] = []
  /// 标题菜单里选的那台电脑（选「全部设备」时也记着，发到电脑默认发给它）
  var computerID: String?
  /// 现在看的那台电脑：选过的，没选过就是第一台（AppModel+Remote.swift 按最近用过的排在前面）。没配对时为空
  var computer: Computer? {
    get { computers.first { $0.id == computerID } ?? computers.first }
    set {
      computers = newValue.map { [$0] } ?? []
      computerID = newValue?.id
    }
  }
  /// 同一个 Wi-Fi 里发现到、还没配对的电脑
  var nearby: [NearbyComputer] = []
  /// 「管理电脑」里点了附近的哪一台（输 6 位数时配它）
  var pairTarget: NearbyComputer?
  /// 下载列表：电脑上的和手机上的
  var items: [DownloadItem] = []
  /// 选了电脑、电脑不在线，存在手机上等着发过去的
  var waiting: [DownloadItem] = []

  var phoneAssets: [Asset] = []
  /// 各台电脑的素材库（按设备 id）：查到的这一页、一共几条、查不了的原因（离线）
  var shelves: [String: ComputerShelf] = [:]
  /// 现在看的那台电脑的素材
  var computerAssets: [Asset] {
    get { computer.flatMap { shelves[$0.id]?.assets } ?? [] }
    set { if let id = computer?.id { shelves[id, default: ComputerShelf()].assets = newValue } }
  }
  var computerAssetCount: Int {
    get { computer.flatMap { shelves[$0.id]?.total } ?? 0 }
    set { if let id = computer?.id { shelves[id, default: ComputerShelf()].total = newValue } }
  }
  /// 正在存到手机的电脑素材（按电脑上的素材 id）
  var saving: [Int64: SavingState] = [:]

  var settings = PhoneSettings()
  /// 上架版：YouTube 这类网站只能交给电脑下（计划 2）。完整版（编译条件 CAPTAE_FULL）为 false
  #if CAPTAE_FULL
    var appStoreBuild = false
  #else
    var appStoreBuild = true
  #endif

  // ---- 真数据（AppModel+Local.swift） ----
  /// Rust 核心；演示模式下没有
  var core: MobileCore?
  /// 核心打不开时的原因（数据目录写不进去之类，极少见）
  var coreError: String?
  /// 手机上的任务（按 id）
  var phoneJobs: [JobID: Job] = [:]
  /// 待发列表里的（等电脑上线、只能在电脑上下的），和它们在列表里用的 id
  var outbox: [PendingDownload] = []
  var outboxIDs: [UUID: JobID] = [:]
  /// 素材库：这台手机上一共几条（分页用）、现在看的类型
  var phoneAssetTotal = 0
  var libraryKind: KindFilter = .all
  /// 搜索标签里手机素材的结果（在核心里搜，不受分页影响）
  var searchHits: [Asset] = []
  /// 下完以后存到了哪（相册 /「文件」）
  let saver = SaveService()
  let background = BackgroundDownloads()
  /// App 在前台
  var active = true
  /// 只提示一次的事（核心的 notice、存相册失败改存「文件」……），弹一个系统提醒
  var notice: AppNotice?

  // ---- 遥控电脑（AppModel+Remote.swift） ----
  /// Rust 的 RemoteHub：配对、电脑上的任务、等电脑上线、电脑的素材库、存到手机
  var hub: RemoteHub?
  /// 发现同一个 Wi-Fi 里的电脑（NWBrowser）
  let browser = ComputerBrowser()
  /// 电脑上的任务（按设备 id、任务 id）和等它上线的
  var remoteJobs: [String: [JobID: Job]] = [:]
  var pendingTasks: [String: [PendingTask]] = [:]
  /// 存到手机时那条素材（下完存相册 /「文件」要用）
  var savingAssets: [UInt64: Asset] = [:]
  /// 7 天内存到过手机的电脑素材（「设备 id/素材 id」，RemoteHub 的 saves() 里下完的记录），素材详情写「已存到手机」
  var savedToPhone: Set<String> = []
  /// 扫到的配对二维码：先问一句「要连这台电脑吗」
  var pairLink: PairLinkInfo?
  var pairLinkText: String?
  /// 正在配对（转圈、按钮不能点）
  var pairingBusy = false
  /// 电脑素材库正在读下一页
  var loadingMoreShelf = false

  // 弹出来的东西
  var detail: DownloadItem?
  var share: ShareCardState?
  var pairing: PairingSession?
  var composer: ComposerSession?
  var manageComputers = false
  /// 「管理电脑」里推进去了哪几页（输入配对码、电脑详情）
  var managePath: [ManageRoute] = []

  /// 演示：不自动聚焦输入框（截图时不弹键盘）
  var demo = false
  /// 演示：输入卡打开时弹键盘（10-composer）
  var demoFocusComposer = false
  /// 演示：搜索标签里预先填的词（12-search）
  var searchQuery = ""
  /// 这台手机的名字（电脑上「已配对的手机」里显示）。iOS 16 起没有特别授权时系统给的就是「iPhone」
  var phoneName = UIDevice.current.name

  var computerName: String? { computer?.name }

  /// 扫码页点了「输入 6 位数配对」：关掉扫码，在「管理电脑」sheet 里推进输入配对码（BRIEF 2.8 第 6 条）
  func enterPairingCode() {
    pairing = nil
    managePath = [.code()]
    manageComputers = true
  }

  /// 演示模式（设计稿截图）：不打开核心，数据是 DemoRoute 填的
  var isDemo: Bool { core == nil && demo }

  /// 「这台 iPhone」
  var phoneLabel: String { cjk("这台", phoneName) }

  var computerItems: [DownloadItem] { items.filter { $0.placement == .computer } }
  /// 一台电脑上的任务和等它上线的（演示数据、没配对时存下的待发任务没有设备 id，算在现在看的那台上）
  func items(of c: Computer) -> [DownloadItem] {
    computerItems.filter { $0.device == c.id || ($0.device == nil && c.id == computer?.id) }
  }
  var phoneItems: [DownloadItem] { items.filter { $0.placement == .phone } }

  /// 没配对时只有手机这一台，范围固定是手机
  var effectiveScope: DeviceScope { computer == nil ? .phone : scope }

  // MARK: 标题和副标题（BRIEF 2.1）

  /// 标题：「全部设备」「这台 iPhone」「Sol 的 MacBook Pro」
  var scopeTitle: String {
    switch effectiveScope {
    case .all: String(localized: "全部设备")
    case .phone: phoneLabel
    case .computer: computerName ?? phoneLabel
    }
  }

  /// 副标题：「2 台设备」「在线 · 正在下 3 个」「离线 · 上次在线 今天 14:20」
  var scopeSubtitle: String {
    switch effectiveScope {
    case .all: return String(localized: "\(1 + computers.count) 台设备")
    case .computer: return computer.map(computerStatus) ?? ""
    case .phone:
      let n = running(phoneItems).count
      if n > 0 { return String(localized: "正在下 \(n) 个") }
      return phoneItems.isEmpty ? "" : String(localized: "\(phoneItems.count) 个任务")
    }
  }

  func computerStatus(_ c: Computer) -> String {
    if c.needsPairing { return String(localized: "要重新配对") }
    if c.connecting { return String(localized: "正在连接…") }
    return c.online
      ? String(localized: "在线 · 正在下 \(running(items(of: c)).count) 个")
      : c.lastSeen.isEmpty ? String(localized: "离线") : String(localized: "离线 · 上次在线 \(c.lastSeen)")
  }

  // MARK: 下载条（BRIEF 2.3）

  /// 正在下的：在跑、不在等人选清晰度、不是等电脑上线
  func running(_ list: [DownloadItem]) -> [DownloadItem] {
    list.filter { $0.hold == nil && $0.job.state == .run && $0.job.phase != .pick }
  }

  var runningItems: [DownloadItem] { running(items) }
  var pausedItems: [DownloadItem] { items.filter { $0.hold == nil && $0.job.state == .pause } }
  var totalSpeed: Double { runningItems.reduce(0) { $0 + $1.job.speed } }
  /// 下载条上显示的那一条：第一个真的在下（有进度）的
  var leadItem: DownloadItem? { runningItems.first { $0.job.phase == .dl } ?? runningItems.first }
  /// 总进度：在下的几个加起来
  var totalProgress: Double {
    let dl = runningItems.filter { $0.job.size > 0 }
    let size = dl.reduce(0) { $0 + $1.job.size }
    return size > 0 ? dl.reduce(0) { $0 + $1.job.got } / size : 0
  }
}
