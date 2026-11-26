// 配对过的电脑和手机这边的下载列表行（手机端特有的一层，计划 3.1、3.2、5.3）。
// 附近发现的电脑用 CaptaeKit 的 `NearbyComputer`（Rust 的 RemoteHub 给的：名字、地址、端口）。
import CaptaeKit
import Foundation

/// 一台配对过的电脑（从 RemoteHub 的 `MobileDevice` 换算来，见 AppModel+Remote.swift）
struct Computer: Identifiable, Hashable, Sendable {
  /// 设备 id（证书指纹）
  var id: String
  /// 「Sol 的 MacBook Pro」
  var name: String
  var online: Bool
  /// 电脑上 Captae 的版本（RemoteHub 现在不给，空着时界面不写）
  var version: String
  /// 电脑上正在下的个数
  var running: Int
  /// 上次连上的时刻（显示用，「今天 14:20」）
  var lastSeen: String
  /// 在电脑上被撤销了 / 钥匙串里没有令牌了：要重新配对
  var needsPairing = false
  /// 正在连
  var connecting = false
}

extension NearbyComputer: @retroactive Identifiable {
  public var id: String { name }
}

/// 手机这边对一行任务的额外说明（不是 Rust 核心的失败，是手机端自己的规矩）
enum Hold: Hashable, Sendable {
  /// 上架版遇到只能在电脑上下的网站（计划 2：「这个网站在电脑上下」）
  case computerOnly
  /// 选了电脑、电脑不在线：存在手机上，回到同一个 Wi-Fi 后自动发过去（计划 3.2）
  case waitingComputer
}

/// 下载页的一行：任务本身 + 在哪里跑 + 手机端的额外说明
struct DownloadItem: Identifiable, Hashable, Sendable {
  var job: Job
  var placement: Placement
  var hold: Hold?
  /// 排在前面的任务数（排队时显示）
  var ahead: Int = 0
  /// 电脑上的任务、等它上线的任务：那台电脑的设备 id
  var device: String?

  /// 列表里的 id：电脑上的任务号和手机上的会重复，带上在哪台设备、是不是待发的
  var id: String {
    let where_ = device ?? (placement == .phone ? "phone" : "computer")
    return "\(where_)/\(hold == nil ? "j" : "p")\(job.id)"
  }
}
