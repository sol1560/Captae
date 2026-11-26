// AppModel 接遥控电脑（Rust 的 RemoteHub，crates/captae-mobile/REMOTE.md）：
// - 设备：`devices()` → 设备菜单、管理电脑；最近用过的那台排在前面（发到电脑默认发给它）
// - 发现：NWBrowser 找到的 `_captae._tcp` 交给 `discovered` / `lost`
// - 配对：扫码 `pairQr`（扫到先 `parsePairLink` 问一句）、6 位数 `pairCode`；令牌在钥匙串（KeychainStore）
// - 下载页：电脑上的任务（事件 onJob / onRemoved / onResync）+「等电脑上线」（`pending()`），按电脑分组；操作走 `remoteAction` 这些
// - 发到电脑：`send`（离线自动进等电脑上线）；分享扩展存下的、上架版只能在电脑上下的，也在 App 里走 `send`
// - 素材库：`mergedLibrary`（全部设备）/ `remoteLibrary`（某一台）；封面 `remoteThumbnail`
// - 存到手机：`saveToPhone` → onSave 进度 → 下完存相册 /「文件」（SaveService）
// 回调在 Rust 的后台线程上，RemoteEvents 转到主线程（和本机的 CoreEvents 一样）。
import CaptaeKit
import SwiftUI
import UIKit
import os

/// RemoteHub 推来的事件
enum RemoteEvent: Sendable {
  case job(DeviceJob)
  case removed(device: String, id: UInt64)
  case finished(device: String)
  case library(device: String)
  case resync(device: String)
  case device(MobileDevice)
  case devices
  case pending
  case pendingSent(task: PendingTask, job: DeviceJob)
  case save(CaptaeKit.SaveProgress)
  /// App 重开以后，上次没存完的「存到手机」记录（Rust 存盘的，会自动接着下）
  case savesRestored([CaptaeKit.SaveProgress])
}

nonisolated final class RemoteEvents: RemoteListener {
  private let sink: @MainActor @Sendable (RemoteEvent) -> Void

  init(sink: @escaping @MainActor @Sendable (RemoteEvent) -> Void) {
    self.sink = sink
  }

  private func send(_ e: RemoteEvent) {
    let sink = sink
    DispatchQueue.main.async { MainActor.assumeIsolated { sink(e) } }
  }

  func onJob(job: DeviceJob) { send(.job(job)) }
  func onRemoved(device: String, id: UInt64) { send(.removed(device: device, id: id)) }
  func onFinished(device: String, id: UInt64, title: String, assetId: Int64?) { send(.finished(device: device)) }
  func onLibrary(device: String) { send(.library(device: device)) }
  func onResync(device: String) { send(.resync(device: device)) }
  func onDevice(device: MobileDevice) { send(.device(device)) }
  func onDevices() { send(.devices) }
  func onPending() { send(.pending) }
  func onPendingSent(task: PendingTask, job: DeviceJob) { send(.pendingSent(task: task, job: job)) }
  func onSave(save: CaptaeKit.SaveProgress) { send(.save(save)) }
  func onSavesRestored(saves: [CaptaeKit.SaveProgress]) { send(.savesRestored(saves)) }
}

extension Computer {
  init(_ d: MobileDevice) {
    self.init(
      id: d.id, name: d.name, online: d.status == .online, version: "", running: Int(d.running),
      lastSeen: d.lastSeen.map { Fmt.when(Date(timeIntervalSince1970: Double($0) / 1000)) } ?? "",
      needsPairing: d.status == .revoked || d.status == .needsPairing, connecting: d.status == .connecting)
  }
}

extension AppModel {
  private static let remoteLog = Logger(subsystem: "app.captae.mobile", category: "remote")

  // MARK: 打开

  func startRemote() {
    guard let core, hub == nil else { return }
    do {
      let hub = try RemoteHub(
        core: core, options: RemoteOptions(phoneName: phoneName, cacheDir: URL.cachesDirectory.path(percentEncoded: false)),
        secure: KeychainStore())
      self.hub = hub
      hub.setListener(listener: RemoteEvents { [weak self] e in self?.applyRemote(e) })
    } catch {
      Self.remoteLog.error("打开遥控电脑失败：\(error.localizedDescription, privacy: .public)")
      return
    }
    browser.onFound = { [weak self] s in
      guard let self, let hub = self.hub else { return }
      _ = hub.discovered(service: s)
      self.nearby = hub.nearby()
    }
    browser.onLost = { [weak self] name in
      self?.hub?.lost(name: name)
      self?.nearby = self?.hub?.nearby() ?? []
    }
    reloadDevices()
    reloadSavedToPhone()
    // 配对过电脑：默认看「全部设备」（BRIEF 2.1）
    scope = computers.isEmpty ? .phone : .all
    reloadRemoteJobs()
  }

  /// 前台：连上所有电脑、开始找；后台：全部断开（iOS 在后台不找电脑）
  func setRemoteActive(_ on: Bool) {
    hub?.setActive(active: on)
    if on { browser.start() } else { browser.stop() }
  }

  // MARK: 事件

  func applyRemote(_ e: RemoteEvent) {
    switch e {
    case let .job(dj):
      let isNew = remoteJobs[dj.device]?[dj.job.id] == nil
      remoteJobs[dj.device, default: [:]][dj.job.id] = Job(dj.job)
      rebuildItems(animated: isNew)
    case let .removed(device, id):
      remoteJobs[device]?[id] = nil
      rebuildItems(animated: true)
    case let .finished(device), let .library(device):
      if shelves[device] != nil || computer?.id == device { reloadComputerLibrary() }
    case .resync:
      reloadRemoteJobs()
    case .device, .devices:
      reloadDevices()
    case .pending:
      reloadRemoteJobs()
    case let .pendingSent(_, job):
      remoteJobs[job.device, default: [:]][job.job.id] = Job(job.job)
      reloadRemoteJobs()
    case let .save(p):
      applySave(p)
    case let .savesRestored(list):
      for p in list { applySave(p) }
    }
  }

  func reloadDevices() {
    guard let hub else { return }
    let list = hub.devices().filter { !$0.local }
    // 最近用过的排前面：没选过电脑时，「发到…」默认发给它（REMOTE.md 第 4 节）
    let sorted = list.sorted { ($0.lastUsed ?? 0, $0.pairedAt ?? 0) > ($1.lastUsed ?? 0, $1.pairedAt ?? 0) }
    computers = sorted.map(Computer.init)
    if let id = computerID, !computers.contains(where: { $0.id == id }) { computerID = nil }
    nearby = hub.nearby()
    if computers.isEmpty { scope = .phone }
    // 分享扩展要知道发到哪台（它不开 RemoteHub，见 CaptaeShare/ShareRoot.swift）
    SharedStore.defaults.set(computer?.name, forKey: SharedStore.computerNameKey)
  }

  /// 电脑上的任务和等它上线的，整份重新拿（不联网，RemoteHub 存着最近一次收到的）
  func reloadRemoteJobs() {
    guard let hub else { return }
    var jobs: [String: [JobID: Job]] = [:]
    var pend: [String: [PendingTask]] = [:]
    for g in hub.mergedJobs(device: nil, search: nil) where !g.device.local {
      jobs[g.device.id] = Dictionary(g.jobs.map { ($0.job.id, Job($0.job)) }, uniquingKeysWith: { $1 })
      pend[g.device.id] = g.pending
    }
    remoteJobs = jobs
    pendingTasks = pend
    rebuildItems(animated: true)
  }

  /// 下载页里电脑那几组的行：电脑上的任务（新的在上面）+ 等它上线的
  func remoteItems() -> [DownloadItem] {
    var list: [DownloadItem] = []
    for c in computers {
      for p in (pendingTasks[c.id] ?? []).reversed() {
        let job = Job(
          id: p.id, title: p.title ?? Self.shortLink(p.url), src: Self.shortLink(p.url), url: p.url, eng: .ytDlp,
          fmt: [p.format, p.quality].compactMap { $0 }.joined(separator: " · "), audio: p.audioOnly, state: .queue, phase: .probe,
          fail: p.error.map(Failure.init))
        list.append(DownloadItem(job: job, placement: .computer, hold: .waitingComputer, device: c.id))
      }
      for job in (remoteJobs[c.id] ?? [:]).values.sorted(by: { $0.id > $1.id }) {
        list.append(DownloadItem(job: job, placement: .computer, device: c.id))
      }
    }
    return list
  }

  /// 显示用的链接：主机 + 路径 + 查询（去掉协议和 www）
  static func shortLink(_ url: String) -> String {
    guard let u = URL(string: url) else { return url }
    return Site.host(url) + u.path + (u.query.map { "?" + $0 } ?? "")
  }

  // MARK: 配对

  /// 扫到的内容：是 Captae 的配对二维码就先问一句「要连这台电脑吗」；不是就说清楚
  func scanned(_ text: String) {
    guard let hub else { return }
    if let info = hub.parsePairLink(text: text) {
      pairLink = info
      pairLinkText = text
    } else {
      notice = AppNotice(title: String(localized: "这不是 Captae 的配对二维码"), text: String(localized: "在电脑上打开 Captae › 设置 › 手机，扫那里的二维码"))
    }
  }

  /// 确认连扫到的那台
  func pairScanned() async {
    guard let hub, let text = pairLinkText else { return }
    pairingBusy = true
    defer { pairingBusy = false }
    do {
      let d = try await hub.pairQr(text: text)
      paired(d)
    } catch {
      notice = AppNotice(title: String(localized: "没能配对"), text: error.localizedDescription)
    }
    pairLink = nil
    pairLinkText = nil
  }

  /// 6 位数配对：配「管理电脑」里点的那台，没点过就配附近找到的第一台。成功返回 nil，失败返回给人看的原因
  func pairCode(_ digits: String) async -> String? {
    guard let hub else { return String(localized: "这个版本还不能连电脑") }
    guard let target = pairTarget ?? nearby.first, let host = target.hosts.first else {
      return String(localized: "还没在同一个 Wi-Fi 里找到电脑。先在电脑上打开「让手机连接」")
    }
    pairingBusy = true
    defer { pairingBusy = false }
    do {
      let d = try await hub.pairCode(host: host, port: target.port, digits: digits)
      paired(d)
      return nil
    } catch {
      return error.localizedDescription
    }
  }

  private func paired(_ d: MobileDevice) {
    reloadDevices()
    computerID = d.id
    scope = .all
    pairing = nil
    managePath = []
    reloadRemoteJobs()
    reloadComputerLibrary()
    // 没配对时存下的（分享进来的、只能在电脑上下的）现在发过去
    sendOutboxToComputer()
    notice = AppNotice(title: cjk(String(localized: "连上了"), d.name), text: String(localized: "以后打开 Captae 会自动连上。链接可以直接发给这台电脑下"))
  }

  /// 删掉一台电脑（断开、删令牌、去掉等它上线的任务）
  func removeComputer(_ id: String) {
    do {
      try hub?.remove(id: id)
      managePath = []
      shelves[id] = nil
      reloadDevices()
      reloadRemoteJobs()
    } catch {
      notice = AppNotice(title: String(localized: "没能完成"), text: error.localizedDescription)
    }
  }

  // MARK: 发到电脑

  /// 发到电脑：在线就发，离线进「等电脑上线」（RemoteHub 自己处理）。没配对时返回 false（调用的存进待发列表）
  @discardableResult
  func sendToComputer(url: String, title: String? = nil, quality: String?, format: String? = nil, audioOnly: Bool) async throws -> Bool {
    guard let hub, let c = computer else { return false }
    let r = try await hub.send(
      device: c.id, options: SendOptions(url: url, quality: quality, format: format, audioOnly: audioOnly, preset: nil, title: title))
    if case let .sent(job) = r {
      remoteJobs[c.id, default: [:]][job.id] = Job(job)
    }
    reloadRemoteJobs()
    return true
  }

  // MARK: 电脑上的任务操作

  func performRemote(_ action: JobCardAction, on item: DownloadItem) {
    guard let hub, let device = item.device else { return }
    let j = item.job
    if item.hold == .waitingComputer {
      switch action {
      case .cancel: try? hub.removePending(id: j.id)
      case .copyLink: UIPasteboard.general.string = j.url
      case .note: try? hub.retryPending(id: j.id)
      case .moveToPhone:
        let r = hub.moveToPhone(ids: [j.id])
        for m in r.moved { apply(.job(m)) }
        if !r.kept.isEmpty {
          notice = AppNotice(title: cjk("", j.siteName, String(localized: "的视频在电脑上下")), text: String(localized: "这台手机只下直链和 M3U8，这一条留着等电脑"))
        }
      default: break
      }
      reloadRemoteJobs()
      return
    }
    let mapped: MobileAction?
    switch action {
    case .toggle: mapped = .toggle
    case .cancel: mapped = j.state.isActive ? .cancel : .remove
    case .pickNow: mapped = .pickGo
    case .startNow: mapped = .now
    case .stop: mapped = .stop
    case .note: mapped = j.failAction == .retry || j.failAction == .autoRetry ? .retry : nil
    case .copyLink:
      UIPasteboard.general.string = j.url
      mapped = nil
    default: mapped = nil
    }
    Task {
      do {
        if case let .pick(k) = action {
          try await hub.remoteSetVariant(device: device, id: j.id, index: UInt32(k))
        } else if let mapped {
          try await hub.remoteAction(device: device, id: j.id, action: mapped)
        }
      } catch {
        notice = AppNotice(title: String(localized: "没能完成"), text: error.localizedDescription)
      }
    }
  }

  /// 等电脑上线的改成在这台手机上下（`ids` 空 = 全部；上架版只改得动直链和 M3U8）
  func moveWaitingToPhone(_ ids: [UInt64] = []) {
    guard let hub else { return }
    let r = hub.moveToPhone(ids: ids)
    for j in r.moved { apply(.job(j)) }
    reloadRemoteJobs()
  }

  // MARK: 电脑的素材库

  /// 选「全部设备」时每台电脑查一页（mergedLibrary，几台同时查）；选了某台时查那一台
  func reloadComputerLibrary() {
    guard let hub, !computers.isEmpty else { return }
    let query = LibraryQuery(kind: libraryKind.coreKind, search: nil, limit: Self.libraryPage, offset: 0)
    let only = effectiveScope == .computer ? computer?.id : nil
    Task {
      let sections = await hub.mergedLibrary(query: query, device: only)
      for s in sections where !s.device.local {
        shelves[s.device.id] = ComputerShelf(
          assets: s.items.map { Asset($0.asset) }, total: Int(s.total), error: s.error?.message)
        await loadThumbs(device: s.device.id)
      }
    }
  }

  /// 选了某台电脑、翻到底：再向它要一页（RemoteHub.remoteLibrary 支持 offset）
  func loadMoreComputerLibrary() {
    guard let hub, let c = computer, var shelf = shelves[c.id], shelf.assets.count < shelf.total, !loadingMoreShelf else { return }
    loadingMoreShelf = true
    let query = LibraryQuery(kind: libraryKind.coreKind, search: nil, limit: Self.libraryPage, offset: UInt32(shelf.assets.count))
    Task {
      do {
        let page = try await hub.remoteLibrary(device: c.id, query: query)
        let known = Set(shelf.assets.map(\.id))
        shelf.assets += page.items.map(Asset.init).filter { !known.contains($0.id) }
        shelf.total = Int(page.total)
        // 先复位再放数据：条数一变，素材库的转圈马上要下一页（task(id:)），不能被这个标记挡住
        loadingMoreShelf = false
        shelves[c.id] = shelf
        await loadThumbs(device: c.id)
      } catch {
        loadingMoreShelf = false
        shelves[c.id]?.error = error.localizedDescription
      }
    }
  }

  #if DEBUG
    /// 调试：往 RemoteHub 交一个 127.0.0.1 的「发现到的电脑」（测试版电脑端只听 127.0.0.1、不广播 Bonjour）。
    /// 地址和端口取启动参数 `-captaeDebugPeer 127.0.0.1:<端口>`，没给时用测试版默认的 47863。
    /// 再给 `-captaeDebugFp <证书指纹>` 时，RemoteHub 按指纹认出是配对过的那台、记下新端口（测试版换了端口时用）；不给就进「附近的电脑」
    func addDebugPeer() {
      guard let hub else { return }
      let raw = UserDefaults.standard.string(forKey: "captaeDebugPeer") ?? "127.0.0.1:47863"
      let parts = raw.split(separator: ":")
      guard parts.count == 2, let port = UInt16(parts[1]) else { return }
      let name = "测试版电脑（\(raw)）"
      var txt = ["v": "1", "name": name]
      if let fp = UserDefaults.standard.string(forKey: "captaeDebugFp") { txt["fp"] = fp }
      _ = hub.discovered(service: DiscoveredService(name: name, hosts: [String(parts[0])], port: port, txt: txt))
      nearby = hub.nearby()
      reloadDevices()
    }
  #endif

  /// 没缓存封面的，向电脑要（一条一条要，不同时开太多连接）
  private func loadThumbs(device: String) async {
    guard let hub else { return }
    for a in shelves[device]?.assets ?? [] where a.thumb == nil && a.kind != .audio {
      guard let path = try? await hub.remoteThumbnail(device: device, id: a.id, refresh: false) else { continue }
      if let i = shelves[device]?.assets.firstIndex(where: { $0.id == a.id }) {
        shelves[device]?.assets[i].thumb = path
      }
    }
  }

  // MARK: 存到手机

  static var fromComputerDir: URL { LocalEngine.root.appending(path: "FromComputer", directoryHint: .isDirectory) }

  /// 把电脑上的一条素材下到手机（断点续传），下完存相册 /「文件」
  func saveToPhone(_ asset: Asset) {
    guard let hub, let device = computer?.id else { return }
    try? FileManager.default.createDirectory(at: Self.fromComputerDir, withIntermediateDirectories: true)
    let ext = asset.fmt.isEmpty ? "mp4" : asset.fmt.lowercased()
    // 标题本身带着扩展名（直链的标题常是文件名）时不再加一层
    var safe = String(asset.title.replacingOccurrences(of: "/", with: "-").prefix(80))
    if safe.lowercased().hasSuffix("." + ext) { safe = String(safe.dropLast(ext.count + 1)) }
    let path = Self.fromComputerDir.appending(path: "\(safe).\(ext)").path(percentEncoded: false)
    do {
      let p = try hub.saveToPhone(device: device, assetId: asset.id, path: path)
      var a = asset
      a.path = path
      savingAssets[p.id] = a
      applySave(p)
    } catch {
      notice = AppNotice(title: String(localized: "没能开始存到手机"), text: error.localizedDescription)
    }
  }

  /// 7 天内存到过手机的（打开时读一次，之后每下完一条加一条）
  func reloadSavedToPhone() {
    savedToPhone = Set((hub?.saves() ?? []).filter { $0.state == .done }.map { "\($0.device)/\($0.assetId)" })
  }

  func isSavedToPhone(_ assetID: Int64) -> Bool {
    guard let id = computer?.id else { return false }
    return savedToPhone.contains("\(id)/\(assetID)")
  }

  func cancelSave(_ assetID: Int64) {
    guard let id = saving[assetID]?.id else { return }
    _ = try? hub?.cancelSave(id: id)
    saving[assetID] = nil
  }

  private func applySave(_ p: CaptaeKit.SaveProgress) {
    let fraction = p.totalBytes > 0 ? Double(p.gotBytes) / Double(p.totalBytes) : 0
    switch p.state {
    case .running, .paused:
      saving[p.assetId] = SavingState(fraction: fraction, speed: Double(p.speed) / 1_000_000, id: p.id, paused: p.state == .paused)
    case .done:
      // 下完的记录 RemoteHub 留 7 天（素材详情写「已存到手机」），这里不删
      saving[p.assetId] = nil
      savedToPhone.insert("\(p.device)/\(p.assetId)")
      let known = savingAssets.removeValue(forKey: p.id)
      let photos = settings.saveTo == .photosAndFiles
      Task {
        // 核心存完会自己把文件导入手机素材库（importFile，只发「素材库变了」，不发「下完」，所以本机那条存相册的逻辑不会再跑一次）。
        // 相册 /「文件」只在这里存一次；导入成功时按手机素材库里那条记下存到了哪（详情页的「位置」「在『文件』中查看」用）
        if let local = p.localAssetId, let m = try? core?.asset(id: local) {
          var a = Asset(m)
          a.path = p.path
          let place = await saver.save(a, photos: photos, record: true)
          let title = place?.kind == .photos ? String(localized: "已存进相册") : String(localized: "存到了「文件」› Captae")
          notice = AppNotice(title: title, text: a.title)
          return
        }
        // 重开 App 以后接着存完的那几条，这次打开没记着素材的样子：向电脑要一次（要不到就按文件扩展名猜类型）
        var a: Asset
        if let known {
          a = known
        } else if let m = try? await hub?.remoteAsset(device: p.device, id: p.assetId) {
          a = Asset(m)
        } else {
          a = Asset(fileAt: p.path, id: p.assetId)
        }
        a.path = p.path
        let place = await saver.save(a, photos: photos, record: false)
        let title = place?.kind == .photos ? String(localized: "已存进相册") : String(localized: "存到了「文件」› Captae")
        notice = AppNotice(title: title, text: a.title)
      }
    case .failed:
      saving[p.assetId] = nil
      notice = AppNotice(title: String(localized: "没能存到手机"), text: p.error?.message ?? "")
    case .cancelled:
      saving[p.assetId] = nil
      savingAssets[p.id] = nil
    }
  }
}

extension Asset {
  /// 只知道文件在哪时（存到手机的记录是上次打开 App 时留下的）：按扩展名认类型
  init(fileAt path: String, id: Int64) {
    let ext = URL(filePath: path).pathExtension.lowercased()
    let kind: AssetKind = ["mp3", "m4a", "aac", "flac", "wav", "opus", "ogg"].contains(ext) ? .audio : ["jpg", "jpeg", "png", "webp", "heic", "gif"].contains(ext) ? .image : .video
    let title = URL(filePath: path).deletingPathExtension().lastPathComponent
    self.init(
      id: id, kind: kind, title: title, desc: "", url: "", page: "", src: "", uploader: "", path: path, fmt: ext.uppercased(), width: nil,
      height: nil, duration: nil, bytes: 0, thumb: nil, tags: [], createdAt: 0, res: "", dur: "", size: "", when: "")
  }
}
