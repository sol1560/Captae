// 在同一个 Wi-Fi 里找电脑上的 Captae（`_captae._tcp`，REMOTE.md 第 3 节）。App 找，Rust 只收结果：
// NWBrowser 给的是服务不是地址，对每个结果开一个 NWConnection，`.ready` 以后从 remoteEndpoint 拿到 IP 和端口就关掉，
// 连同 TXT（v、fp、name）交给 RemoteHub.discovered；服务不见了交给 lost。
// 不用 Rust 自己的组播发现：那要 Apple 特批的 multicast 权限。
// 第一次开始找时系统问「本地网络」权限（Info.plist 的 NSLocalNetworkUsageDescription、NSBonjourServices）。
// 浏览和解析都放在主队列上：回调里要读写 App 的状态，闭包在主线程隔离的上下文里写，在主队列上调才对得上。
import CaptaeKit
import Network
import os

final class ComputerBrowser {
  private static let log = Logger(subsystem: "app.captae.mobile", category: "browse")
  private var browser: NWBrowser?
  private var resolving: [String: NWConnection] = [:]
  /// 解析好的服务交给谁（AppModel → RemoteHub.discovered）
  var onFound: (DiscoveredService) -> Void = { _ in }
  var onLost: (String) -> Void = { _ in }

  func start() {
    guard browser == nil else { return }
    let b = NWBrowser(for: .bonjourWithTXTRecord(type: "_captae._tcp", domain: nil), using: .tcp)
    // Network 的回调是不隔离的 @Sendable 闭包；队列是主队列，所以在里面断言回到主线程的上下文
    b.browseResultsChangedHandler = { [weak self] _, changes in
      MainActor.assumeIsolated {
        for change in changes {
          switch change {
          case let .added(r): self?.resolve(r)
          case let .changed(old: _, new: r, flags: _): self?.resolve(r)
          case let .removed(r): if let name = Self.name(r.endpoint) { self?.onLost(name) }
          default: break
          }
        }
      }
    }
    b.stateUpdateHandler = { state in
      MainActor.assumeIsolated {
        if case let .failed(e) = state { Self.log.error("找电脑失败：\(e.localizedDescription, privacy: .public)") }
      }
    }
    b.start(queue: .main)
    browser = b
  }

  func stop() {
    browser?.cancel()
    browser = nil
    for c in resolving.values { c.cancel() }
    resolving = [:]
  }

  private func resolve(_ r: NWBrowser.Result) {
    guard let name = Self.name(r.endpoint) else { return }
    var record: [String: String] = [:]
    if case let .bonjour(r) = r.metadata { record = r.dictionary }
    let txt = record
    resolving[name]?.cancel()
    let c = NWConnection(to: r.endpoint, using: .tcp)
    resolving[name] = c
    c.stateUpdateHandler = { [weak self] state in
      MainActor.assumeIsolated {
        switch state {
        case .ready:
          if case let .hostPort(host, port) = c.currentPath?.remoteEndpoint {
            let service = DiscoveredService(name: name, hosts: [Self.ip(host)], port: port.rawValue, txt: txt)
            Self.log.info("找到电脑：\(name, privacy: .public) \(Self.ip(host), privacy: .public):\(port.rawValue)")
            self?.onFound(service)
          }
          c.cancel()
          self?.resolving[name] = nil
        case .failed, .cancelled:
          self?.resolving[name] = nil
        default:
          break
        }
      }
    }
    c.start(queue: .main)
  }

  private static func name(_ e: NWEndpoint) -> String? {
    if case let .service(name, _, _, _) = e { return name }
    return nil
  }

  /// IP 地址的文字：IPv6 不带「%网卡」、不带方括号（REMOTE.md 第 3 节）
  private static func ip(_ host: NWEndpoint.Host) -> String {
    let s: String
    switch host {
    case let .ipv4(a): s = "\(a)"
    case let .ipv6(a): s = "\(a)"
    case let .name(n, _): s = n
    @unknown default: s = "\(host)"
    }
    return String(s.split(separator: "%").first ?? Substring(s))
  }
}
