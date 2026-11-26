// 切到后台以后接着下（计划 5.4「后台」）：手机上有 M3U8 或直链任务在下时，申请一个 BGContinuedProcessingTask（iOS 26 起）。
// 系统在灵动岛 / 锁屏显示「正在下 N 个」和总进度，用户可以在那里取消；资源紧张时系统可能提前停掉。
// - 只能在前台申请：任务开始时（加任务、App 回到前台而还有在下的）申请一次，所有手机任务共用这一个；全部下完就结束它。
// - 要一直报进度（NSProgressReporting），不报的会被当成卡住。总进度 = 在下的几个加起来。
// - 申请不到（模拟器上没有 BGTaskScheduler、系统现在不给）：这一次打开不再重试；切到后台时退回 UIApplication 的
//   beginBackgroundTask，系统给的那点时间（通常几十秒）里接着下。
// - 被系统停了（过期、用户在锁屏上取消、后台时间用完）：AppModel 把在下的手机任务暂停、记下来，下次回到前台接着下。
// SDK 核对（Xcode 27.2 / iOS 27.2 SDK 的 BackgroundTasks 头文件）：
//   BGContinuedProcessingTaskRequest(identifier:title:subtitle:)、.strategy（.fail / .queue）、
//   BGContinuedProcessingTask.updateTitle(_:subtitle:)、.progress；标识要用 Info.plist 里登记的通配前缀
//   `app.captae.mobile.download.*` 加上自己的后缀。处理函数按完整标识在提交前注册（论坛上通配注册在 26 正式版不可靠）。
import BackgroundTasks
import UIKit
import os

final class BackgroundDownloads {
  private static let log = Logger(subsystem: "app.captae.mobile", category: "background")
  private static let prefix = "app.captae.mobile.download."

  private var task: BGContinuedProcessingTask?
  /// 已经提交、系统还没开始的那一个
  private var pending: String?
  /// 这一次打开已经申请过、没申请到：回到前台前不再试（不然每个进度事件都试一次）
  private var refused = false
  /// 申请不到时退回的那点后台时间
  private var fallback: UIBackgroundTaskIdentifier = .invalid
  /// 系统停掉时：AppModel 暂停在下的任务、记下来
  var onExpire: () -> Void = {}

  /// 每次手机任务变化后调用。`running`：手机上正在下的；`active`：App 在前台
  func update(running: [Job], active: Bool) {
    guard !running.isEmpty else {
      finish(success: true)
      endFallback()
      return
    }
    let title = String(localized: "正在下 \(running.count) 个")
    let lead = running.first { $0.phase == .dl } ?? running[0]
    let subtitle = lead.untitled ? lead.src : lead.title
    if let task {
      let size = running.reduce(0) { $0 + $1.size }
      let got = running.reduce(0) { $0 + $1.got }
      task.progress.totalUnitCount = 1000
      task.progress.completedUnitCount = size > 0 ? Int64(min(1, got / size) * 1000) : 0
      if task.title != title || task.subtitle != subtitle {
        task.updateTitle(title, subtitle: subtitle)
      }
    } else if pending == nil && active && !refused {
      submit(title: title, subtitle: subtitle)
    }
  }

  /// 回到前台：上次没申请到的可以再试一次；退回用的后台时间不用了
  func enteredForeground() {
    refused = false
    endFallback()
  }

  /// 切到后台：有在下的、又没拿到继续运行的任务时，向系统要一点后台时间
  func enteredBackground(hasRunning: Bool) {
    guard hasRunning, task == nil, pending == nil, fallback == .invalid else { return }
    fallback = UIApplication.shared.beginBackgroundTask(withName: "captae.download") { [weak self] in
      // 时间用完（系统在主线程上调）：暂停、记下来，下次打开接着下
      MainActor.assumeIsolated {
        Self.log.info("后台时间用完，暂停手机上的任务")
        self?.onExpire()
        self?.endFallback()
      }
    }
    Self.log.info("没有继续运行的任务，向系统要了一点后台时间")
  }

  private func endFallback() {
    guard fallback != .invalid else { return }
    UIApplication.shared.endBackgroundTask(fallback)
    fallback = .invalid
  }

  private func submit(title: String, subtitle: String) {
    let id = Self.prefix + UUID().uuidString
    let ok = BGTaskScheduler.shared.register(forTaskWithIdentifier: id, using: .main) { [weak self] t in
      MainActor.assumeIsolated {
        guard let self, let t = t as? BGContinuedProcessingTask else {
          t.setTaskCompleted(success: false)
          return
        }
        self.began(t)
      }
    }
    guard ok else {
      refused = true
      Self.log.error("注册后台任务失败：\(id, privacy: .public)")
      return
    }
    let request = BGContinuedProcessingTaskRequest(identifier: id, title: title, subtitle: subtitle)
    // 系统现在给不了就算了（App 还在前台，照样在下），不排队等到以后才开始
    request.strategy = .fail
    do {
      try BGTaskScheduler.shared.submit(request)
      pending = id
      Self.log.info("申请后台继续下载：\(id, privacy: .public)")
    } catch {
      refused = true
      Self.log.error("申请后台继续下载失败（这次打开不再试）：\(error.localizedDescription, privacy: .public)")
    }
  }

  private func began(_ t: BGContinuedProcessingTask) {
    pending = nil
    task = t
    t.progress.totalUnitCount = 1000
    // 系统在后台线程上调过期处理：闭包本身不能是主线程隔离的（见 ExpiryHandler），里面再切回主线程
    t.expirationHandler = ExpiryHandler.make { [weak self] in
      Self.log.info("后台继续下载被系统停了")
      self?.onExpire()
      self?.finish(success: false)
    }
    Self.log.info("后台继续下载开始")
  }

  private func finish(success: Bool) {
    if let task {
      task.progress.completedUnitCount = task.progress.totalUnitCount
      task.setTaskCompleted(success: success)
      self.task = nil
    }
    if let pending {
      BGTaskScheduler.shared.cancel(taskRequestWithIdentifier: pending)
      self.pending = nil
    }
  }
}

/// 过期处理：系统在任意线程上调。在主线程隔离的上下文里直接写闭包会被推断成 @MainActor，
/// 在后台线程上一调就被运行时的隔离检查中止（存相册时实测踩过同一个坑），所以在这里造一个不隔离的闭包
nonisolated private enum ExpiryHandler {
  static func make(_ body: @escaping @MainActor @Sendable () -> Void) -> @Sendable () -> Void {
    { DispatchQueue.main.async { MainActor.assumeIsolated { body() } } }
  }
}
