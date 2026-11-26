// AppModel 接 Rust 核心：手机本地下载的全部真数据（M3）。
// - 任务：核心推来的事件 → phoneJobs → 下载页的 items（新的在上面）；暂停、继续、取消、重试、选清晰度、选条目都走核心
// - 待发列表（App Group 的 outbox.json）：分享扩展存进来的、只能在电脑上下的、等电脑上线的，和手机任务排在一起显示
// - 素材库：分页查询、按类型、搜索、删除；下完的存相册 /「文件」（SaveService）
// - 设置：读写核心的设置（同时下载、倒计时、代理、默认清晰度和格式），「存到」记在 UserDefaults
// - 后台：有手机任务在下就申请 BGContinuedProcessingTask（BackgroundDownloads）；被系统停了，下次回到前台接着下
import CaptaeKit
import SwiftUI
import UIKit
import os

/// 弹一个系统提醒
struct AppNotice: Identifiable, Hashable {
  var id = UUID()
  var title: String
  var text: String
}

/// 输入卡发送以后的结果
enum SubmitOutcome {
  /// 交给核心开始下了
  case added
  /// 上架版：这个网站要在电脑上下，存进了待发列表
  case heldForComputer(site: String?)
  /// 去向是电脑：存进了待发列表，等电脑上线
  case waitingComputer
}

extension AppModel {
  private static let log = Logger(subsystem: "app.captae.mobile", category: "core")
  /// 一页素材几条（Debug 包可以用启动参数 `-captaePageSize <条数>` 调小，测翻页）
  static var libraryPage: UInt32 {
    #if DEBUG
      let n = UserDefaults.standard.integer(forKey: "captaePageSize")
      if n > 0 { return UInt32(n) }
    #endif
    return 60
  }
  /// 被系统停掉时暂停的任务（下次回到前台接着下）
  private static let interruptedKey = "interruptedJobs"

  // MARK: 打开

  /// 打开核心、读一遍任务、素材库、设置，取走分享扩展存的待发任务
  func startLocal() {
    guard core == nil else { return }
    do {
      let core = try LocalEngine.open()
      self.core = core
      core.setListener(listener: CoreEvents { [weak self] e in self?.apply(e) })
      let info = core.info()
      Self.log.info("核心 \(info.version, privacy: .public) 平台 \(info.platform, privacy: .public) 语言 \(info.language, privacy: .public) 证书 \(info.tls, privacy: .public)")
    } catch {
      coreError = error.localizedDescription
      Self.log.error("打开核心失败：\(error.localizedDescription, privacy: .public)")
      return
    }
    background.onExpire = { [weak self] in self?.pauseForInterruption() }
    loadSettings()
    startRemote()
    reloadJobs()
    reloadLibrary()
    becameActive()
  }

  /// App 回到前台：取走待发任务、接着下被系统停掉的、申请后台继续
  func becameActive() {
    active = true
    guard core != nil else { return }
    background.enteredForeground()
    setRemoteActive(true)
    takeOutbox()
    resumeInterrupted()
    updateBackground()
  }

  func resignedActive() {
    active = false
    setRemoteActive(false)
    background.enteredBackground(hasRunning: phoneJobs.values.contains { $0.state == .run || $0.state == .queue })
  }

  // MARK: 事件

  func apply(_ e: CoreEvent) {
    switch e {
    case let .job(m):
      let isNew = phoneJobs[m.id] == nil
      phoneJobs[m.id] = Job(m)
      rebuildItems(animated: isNew)
      updateBackground()
    case let .removed(id):
      phoneJobs[id] = nil
      rebuildItems(animated: true)
      updateBackground()
    case let .finished(id, _, _, assetId):
      if let assetId { Task { await saveFinished(jobID: id, assetID: assetId) } }
    case .library:
      reloadLibrary()
    case let .notice(_, title, text):
      notice = AppNotice(title: title, text: text)
    case .resync:
      reloadJobs()
    }
  }

  func reloadJobs() {
    guard let core else { return }
    phoneJobs = Dictionary(uniqueKeysWithValues: core.jobs().map { ($0.id, Job($0)) })
    rebuildItems(animated: false)
  }

  /// 下载页的行：待发列表里的 + 手机上的任务，新的在上面。增删时带动画（MOTION c1 / c3），进度变化不带
  func rebuildItems(animated: Bool) {
    let queued = phoneJobs.values.filter { $0.state == .queue }.map(\.id).sorted()
    var list: [DownloadItem] = remoteItems() + outbox.reversed().map(outboxItem)
    list += phoneJobs.values.sorted { $0.id > $1.id }.map { job in
      DownloadItem(job: job, placement: .phone, ahead: queued.firstIndex(of: job.id) ?? 0)
    }
    waiting = list.filter { $0.hold == .waitingComputer }
    // 行太多时不做增删动画（布局让位很贵，MOTION 1.6）
    if animated && list.count <= 40 && Set(list.map(\.id)) != Set(items.map(\.id)) {
      withAnimation(Motion.listChange) { items = list }
    } else {
      items = list
    }
  }

  /// 待发列表的一条 → 下载页的一行。配对过电脑时放进电脑那一组（等电脑上线），没配对时放在手机这一组，提示「在电脑上下」
  private func outboxItem(_ p: PendingDownload) -> DownloadItem {
    let id = outboxIDs[p.id] ?? {
      let next = JobID(1 << 62) + JobID(outboxIDs.count)
      outboxIDs[p.id] = next
      return next
    }()
    let job = Job(
      id: id, title: p.title ?? Self.shortLink(p.url), src: Self.shortLink(p.url),
      url: p.url, eng: .ytDlp, fmt: "", thumb: p.thumb, audio: p.audioOnly, state: .queue, phase: .probe,
      fail: p.site.map { Failure(code: .unsupportedHere, message: "", site: $0) })
    if computer != nil {
      return DownloadItem(job: job, placement: .computer, hold: .waitingComputer)
    }
    return DownloadItem(job: job, placement: .phone, hold: p.computerOnly || p.target == .computer ? .computerOnly : nil)
  }

  func pending(for item: DownloadItem) -> PendingDownload? {
    guard item.device == nil, let uuid = outboxIDs.first(where: { $0.value == item.job.id })?.key else { return nil }
    return outbox.first { $0.id == uuid }
  }

  // MARK: 待发列表

  /// 分享扩展存的：去向是手机的交给核心（上架版遇到只能在电脑上下的网站，标成只能在电脑上下、留着）；
  /// 去向是电脑的、只能在电脑上下的：配对过电脑就走 RemoteHub 的 send（离线进它的「等电脑上线」），没配对的留着
  func takeOutbox() {
    guard let core else { return }
    defer { sendOutboxToComputer() }
    var failures: [String] = []
    let left = Outbox.update { list in
      list = list.filter { p in
        guard p.target == .phone, !p.computerOnly else { return true }
        do {
          _ = try core.add(
            o: AddOptions(url: p.url, quality: p.quality, format: nil, audioOnly: p.audioOnly, pageUrl: p.pageURL, pageTitle: p.pageURL == nil ? nil : p.title))
          return false
        } catch MobileError.Failed(let f) where f.code == "unsupported_here" {
          return true
        } catch {
          failures.append(error.localizedDescription)
          return false
        }
      }
      for i in list.indices where list[i].target == .phone { list[i].computerOnly = true }
    }
    outbox = left
    rebuildItems(animated: true)
    if let first = failures.first {
      notice = AppNotice(title: String(localized: "有一条分享进来的链接没能开始"), text: first)
    }
  }

  /// 待发列表里要交给电脑的，配对过电脑就交给 RemoteHub（它负责在线就发、离线排队）
  func sendOutboxToComputer() {
    guard hub != nil, computer != nil else { return }
    let toSend = outbox.filter { $0.target == .computer || $0.computerOnly }
    guard !toSend.isEmpty else { return }
    Task {
      for p in toSend {
        do {
          try await sendToComputer(url: p.pageURL ?? p.url, title: p.title, quality: p.quality, audioOnly: p.audioOnly)
          outbox = Outbox.update { $0.removeAll { $0.id == p.id } }
        } catch {
          notice = AppNotice(title: String(localized: "没能发到电脑"), text: error.localizedDescription)
          break
        }
      }
      rebuildItems(animated: true)
    }
  }

  /// 存进待发列表（只能在电脑上下的、发到不在线的电脑的）
  func hold(_ p: PendingDownload) {
    outbox = Outbox.update { $0.append(p) }
    rebuildItems(animated: true)
  }

  func dropPending(_ item: DownloadItem) {
    guard let p = pending(for: item) else { return }
    outbox = Outbox.update { $0.removeAll { $0.id == p.id } }
    rebuildItems(animated: true)
  }

  // MARK: 加任务（输入卡）

  /// 读取信息（可以用 `token` 取消），读到了就加进核心。上架版遇到 YouTube 这类网站存进待发列表
  func submit(url raw: String, target: Placement, format: FormatChoice, token: CancelToken, onProbe: (MobileProbe) -> Void)
    async throws -> SubmitOutcome
  {
    let url = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    if target == .computer {
      // 配对过：交给 RemoteHub（在线就发，离线进「等电脑上线」）；没配对：存进待发列表，配对以后发过去
      if try await sendToComputer(url: url, quality: format.quality, format: format.format, audioOnly: format.audioOnly) {
        return .waitingComputer
      }
      hold(PendingDownload(url: url, target: .computer, quality: format.quality, audioOnly: format.audioOnly))
      return .waitingComputer
    }
    guard let core else {
      throw MobileError.Failed(detail: MobileFailure(code: "unknown", message: coreError ?? "", action: "none", raw: "", site: nil, reason: nil, brief: nil, startsAt: nil))
    }
    do {
      let probe = try await core.probe(url: url, cancel: token)
      onProbe(probe)
      let media = probe.resolvedUrl ?? url
      let job = try core.add(
        o: AddOptions(
          url: media, quality: format.quality, format: format.format, audioOnly: format.audioOnly,
          pageUrl: probe.resolvedUrl == nil ? nil : url, pageTitle: probe.resolvedUrl == nil ? nil : probe.title))
      apply(.job(job))
      return .added
    } catch MobileError.Failed(let f) where f.code == "unsupported_here" {
      // 上架版：这个网站要在电脑上下。配对过就直接发给电脑，没配对存着
      if try await sendToComputer(url: url, quality: format.quality, format: format.format, audioOnly: format.audioOnly) {
        return .heldForComputer(site: f.site)
      }
      hold(PendingDownload(url: url, site: f.site, target: .computer, quality: format.quality, audioOnly: format.audioOnly, computerOnly: true))
      return .heldForComputer(site: f.site)
    }
  }

  // MARK: 任务操作

  /// 任务卡、详情、长按菜单上的操作
  func perform(_ action: JobCardAction, on item: DownloadItem) {
    if case .open = action {
      detail = item
      return
    }
    if case .moveToComputer = action {
      // 手机上的交给电脑：发过去（离线进「等电脑上线」），手机上这一条取消掉
      Task {
        do {
          guard try await sendToComputer(url: item.job.url, title: item.job.untitled ? nil : item.job.title, quality: nil, audioOnly: item.job.audio) else { return }
          if item.hold == nil { try? core?.action(id: item.job.id, action: item.job.state.isActive ? .cancel : .remove) } else { dropPending(item) }
        } catch {
          notice = AppNotice(title: String(localized: "没能发到电脑"), text: error.localizedDescription)
        }
      }
      return
    }
    if item.placement == .computer && item.device != nil {
      performRemote(action, on: item)
      return
    }
    if item.hold != nil {
      switch action {
      case .cancel: dropPending(item)
      case .copyLink: UIPasteboard.general.string = item.job.url
      case .note: openPairing()
      default: break
      }
      return
    }
    guard let core, item.placement == .phone else { return }
    let j = item.job
    do {
      switch action {
      case .open:
        break
      case .toggle:
        try core.action(id: j.id, action: .toggle)
      case .cancel:
        try core.action(id: j.id, action: j.state.isActive ? .cancel : .remove)
      case let .pick(k):
        try core.setVariant(id: j.id, index: UInt32(k))
      case .pickNow:
        try core.action(id: j.id, action: .pickGo)
      case .startNow:
        try core.action(id: j.id, action: .now)
      case .stop:
        try core.action(id: j.id, action: .stop)
      case .moveToPhone, .moveToComputer:
        break
      case .copyLink:
        UIPasteboard.general.string = j.url
      case .note:
        switch j.failAction {
        case .login, .verify, .useComputer, .useExtension: openPairing()
        case .copyError: UIPasteboard.general.string = j.fail?.raw.isEmpty == false ? j.fail?.raw : j.fail?.message
        default: try core.action(id: j.id, action: .retry)
        }
      }
    } catch {
      notice = AppNotice(title: String(localized: "没能完成"), text: error.localizedDescription)
    }
  }

  /// 播放列表：选好的条目各成一个任务
  func choose(_ entryIDs: [String], in item: DownloadItem) {
    if let device = item.device, let hub {
      Task {
        do { _ = try await hub.remoteChooseEntries(device: device, id: item.job.id, ids: entryIDs) } catch {
          notice = AppNotice(title: String(localized: "没能完成"), text: error.localizedDescription)
        }
      }
      return
    }
    guard let core else { return }
    do {
      for j in try core.chooseEntries(id: item.job.id, ids: entryIDs) { apply(.job(j)) }
    } catch {
      notice = AppNotice(title: String(localized: "没能完成"), text: error.localizedDescription)
    }
  }

  func log(of item: DownloadItem) -> [MobileLogLine] {
    guard item.hold == nil else { return [] }
    if let device = item.device { return hub?.remoteJobLog(device: device, id: item.job.id) ?? [] }
    return item.placement == .phone ? core?.jobLog(id: item.job.id) ?? [] : []
  }

  func pauseAll() { core?.pauseAll() }
  func resumeAll() { core?.resumeAll() }

  /// 清空已完成：从列表里去掉下完的（素材库里的不动）
  func clearDone() {
    guard let core else { return }
    for j in phoneJobs.values where j.state == .done {
      try? core.action(id: j.id, action: .remove)
    }
  }

  func openPairing() {
    pairing = PairingSession(start: .scan)
  }

  // MARK: 后台

  func updateBackground() {
    background.update(running: phoneJobs.values.filter { $0.state == .run }.sorted { $0.id < $1.id }, active: active)
  }

  /// 系统停掉了后台继续：暂停在下的（下到一半的文件、分片都留着），记下来下次接着下
  private func pauseForInterruption() {
    guard let core else { return }
    let ids = phoneJobs.values.filter { $0.state == .run || $0.state == .queue }.map(\.id)
    for id in ids { try? core.action(id: id, action: .pause) }
    UserDefaults.standard.set(ids.map { NSNumber(value: $0) }, forKey: Self.interruptedKey)
  }

  /// 接着下上次被系统停掉的（用户自己暂停的不动）；网络断了失败的再试一次
  private func resumeInterrupted() {
    guard let core, let ids = UserDefaults.standard.array(forKey: Self.interruptedKey) as? [NSNumber] else { return }
    UserDefaults.standard.removeObject(forKey: Self.interruptedKey)
    for n in ids {
      guard let j = phoneJobs[n.uint64Value] else { continue }
      if j.state == .pause {
        try? core.action(id: j.id, action: .resume)
      } else if j.state == .failed, j.fail?.code == .network {
        try? core.action(id: j.id, action: .retry)
      }
    }
  }

  // MARK: 存相册 /「文件」

  private func saveFinished(jobID: JobID, assetID: Int64) async {
    guard let core, let m = try? core.asset(id: assetID) else { return }
    let asset = Asset(m)
    let photos = settings.saveTo == .photosAndFiles
    let place = await saver.save(asset, photos: photos)
    if place == nil {
      notice = AppNotice(title: String(localized: "没能存好"), text: String(localized: "文件在素材库里，可以在素材详情里分享出去"))
    } else if photos && asset.kind != .audio && place?.kind == .files {
      notice = AppNotice(
        title: String(localized: "存到了「文件」› Captae"), text: String(localized: "没能存进相册（没有给相册的添加权限，或者相册不认这种格式）"))
    }
    rebuildItems(animated: false)
  }

  /// 任务卡「已完成」后面那句：已存进相册 / 已存到「文件」
  func savedNote(for job: Job) -> String? {
    guard let id = job.assetId, let place = saver.places[id] else { return nil }
    return place.kind == .photos ? String(localized: "已存进相册") : String(localized: "已存到「文件」")
  }

  // MARK: 素材库

  /// 重新查第一页（素材库变了、换了类型）。已经往下翻过的条数照旧留着
  func reloadLibrary() {
    guard let core else { return }
    let limit = max(Self.libraryPage, UInt32(phoneAssets.count))
    let q = LibraryQuery(kind: libraryKind.coreKind, search: nil, limit: limit, offset: 0)
    guard let page = try? core.library(q: q) else { return }
    phoneAssets = page.items.map(Asset.init)
    phoneAssetTotal = Int(page.total)
  }

  /// 翻到底了：再读一页
  func loadMoreLibrary() {
    guard let core, phoneAssets.count < phoneAssetTotal else { return }
    let q = LibraryQuery(kind: libraryKind.coreKind, search: nil, limit: Self.libraryPage, offset: UInt32(phoneAssets.count))
    guard let page = try? core.library(q: q) else { return }
    let known = Set(phoneAssets.map(\.id))
    phoneAssets += page.items.map(Asset.init).filter { !known.contains($0.id) }
    phoneAssetTotal = Int(page.total)
  }

  func setLibraryKind(_ kind: KindFilter) {
    guard kind != libraryKind else { return }
    libraryKind = kind
    phoneAssets = []
    reloadLibrary()
    reloadComputerLibrary()
  }

  /// 搜索标签：在核心里搜手机上的素材（标题、简介、标签、来源）
  func searchLibrary(_ text: String) {
    guard let core else { return }
    let s = text.trimmingCharacters(in: .whitespaces)
    guard !s.isEmpty, let page = try? core.library(q: LibraryQuery(kind: nil, search: s, limit: 50, offset: 0)) else {
      searchHits = []
      return
    }
    searchHits = page.items.map(Asset.init)
  }

  /// 从素材库里删掉（文件和「文件」里那份一起删；相册里的不动）
  func deleteAsset(_ asset: Asset) {
    guard let core else { return }
    do {
      try core.deleteAsset(id: asset.id, deleteFile: true)
      saver.forget(asset.id)
      libraryPath.removeAll { if case let .asset(a, _) = $0 { a.id == asset.id } else { false } }
    } catch {
      notice = AppNotice(title: String(localized: "没能删掉"), text: error.localizedDescription)
    }
  }

  // MARK: 设置

  private static let saveToKey = "saveTo"

  func loadSettings() {
    guard let core else { return }
    let s = core.settings()
    var p = PhoneSettings()
    p.concurrency = Int(s.concurrency)
    p.pickSeconds = Int(s.pickSeconds)
    p.proxy = s.proxyMode == "manual" ? .manual : .none
    p.proxyURL = s.proxyUrl
    p.quality = PhoneSettings.qualities.first { $0.value == s.defaultQuality }?.label ?? "最清晰"
    p.format = PhoneSettings.formats.first { $0.value == s.defaultFormat }?.label ?? "MP4"
    p.saveTo = UserDefaults.standard.string(forKey: Self.saveToKey).flatMap(PhoneSettings.SaveTo.init) ?? .photosAndFiles
    settings = p
  }

  /// 设置页改了一项：整份写回核心（核心把超出范围的值收回范围里），读回来对齐
  func saveSettings() {
    UserDefaults.standard.set(settings.saveTo.rawValue, forKey: Self.saveToKey)
    guard let core else { return }
    var s = core.settings()
    s.concurrency = UInt32(settings.concurrency)
    s.pickSeconds = UInt32(settings.pickSeconds)
    s.proxyMode = settings.proxy == .manual ? "manual" : "none"
    s.proxyUrl = settings.proxyURL
    s.defaultQuality = PhoneSettings.qualities.first { $0.label == settings.quality }?.value ?? "最高可用"
    s.defaultFormat = PhoneSettings.formats.first { $0.label == settings.format }?.value ?? "MP4"
    do {
      _ = try core.setSettings(s: s)
    } catch {
      notice = AppNotice(title: String(localized: "设置没能保存"), text: error.localizedDescription)
    }
  }
}

extension KindFilter {
  /// 核心的素材类型（nil = 全部）
  var coreKind: String? {
    switch self {
    case .all: nil
    case .video: "video"
    case .audio: "audio"
    case .image: "image"
    }
  }
}

extension FormatChoice {
  /// 清晰度（协议值）。「最清晰」不传：M3U8 有几种清晰度时照桌面端出倒计时让人选
  var quality: String? {
    switch self {
    case .p1080: "1080p"
    case .p720: "720p"
    default: nil
    }
  }

  /// 格式（协议值，只对网页链接有用）
  var format: String? {
    switch self {
    case .mp3: "MP3"
    case .aac: "AAC"
    case .flac: "FLAC"
    case .webm: "WEBM"
    case .mkv: "MKV"
    case .keep: "保持原格式"
    default: nil
    }
  }

  var audioOnly: Bool { self == .mp3 || self == .aac || self == .flac }
}
