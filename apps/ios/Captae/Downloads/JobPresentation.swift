// 下载页的纯逻辑：一行任务现在显示成哪种样子、步骤时间线、失败时说什么、按钮是什么。
// 照桌面端 `apps/desktop/src/pages/downloads/model.ts`（viewOf、stepsOf、failInfo），再加手机端自己的两种：
// 「这个网站在电脑上下」（上架版）、「等电脑上线」（计划 2、3.2）。界面只管画，判断都在这里。
import Foundation

/// 一行任务现在显示成哪种样子
enum JobView: Hashable {
  /// 正在读取视频信息
  case probe
  /// M3U8：选清晰度，倒计时结束自动用最清晰的
  case pick
  /// 下载中
  case dl
  /// 合并分段、转格式
  case merge
  case queue
  case pause
  /// 要登录
  case login
  /// 要人机验证
  case verify
  case fail
  case done
  /// 手机端：这个网站在电脑上下
  case computerOnly
  /// 手机端：等电脑上线
  case waiting
}

extension DownloadItem {
  var view: JobView {
    if let hold {
      return hold == .computerOnly ? .computerOnly : .waiting
    }
    switch job.state {
    case .done: return .done
    case .failed:
      switch job.fail?.act {
      case .login: return .login
      case .verify: return .verify
      default: return .fail
      }
    case .pause: return .pause
    case .queue: return .queue
    case .run:
      switch job.phase {
      case .probe: return .probe
      case .pick: return .pick
      case .merge, .post: return .merge
      default: return .dl
      }
    }
  }
}

/// 失败、提示时状态行上写什么、按钮是什么
struct NoteInfo {
  var tone: NoteTone
  var icon: String
  var title: String
  var hint: String?
  /// 按钮上的字；没有按钮时为空
  var label: String?

  /// 按钮上的字长（「发到 Sol 的 MacBook Pro」），放到文字下面，不挤正文
  var stacked: Bool { (label?.count ?? 0) > 6 }
}

enum JobText {
  /// 状态行：失败（按错误码）和手机端的两种提示。`computer` 是配对过的电脑名字（没配对时为空）
  static func note(for item: DownloadItem, computer: String?) -> NoteInfo? {
    let j = item.job
    switch item.view {
    case .computerOnly:
      return NoteInfo(
        tone: .info, icon: "laptopcomputer", title: cjk("", j.siteName, String(localized: "的视频在电脑上下")),
        hint: computer == nil ? String(localized: "先在「管理电脑」里配对一台电脑") : nil,
        label: computer.map { cjk(String(localized: "发到"), $0) } ?? String(localized: "去配对"))
    case .waiting:
      return NoteInfo(tone: .plain, icon: "clock", title: String(localized: "等电脑上线"), hint: String(localized: "回到同一个 Wi-Fi 后自动发过去"), label: nil)
    case .login, .verify, .fail:
      return failNote(j, computer: computer)
    default:
      return nil
    }
  }

  /// 后端 errors.rs 给的 action → 手机上的按钮（桌面端 failInfo 的手机版）
  private static func failNote(_ j: Job, computer: String?) -> NoteInfo {
    let f = j.fail
    let title = f?.message ?? String(localized: "下载失败")
    let site = j.siteName
    switch f?.act {
    case .login, .verify:
      // 手机上第一版不做登录窗口：交给电脑下，电脑上有登录状态（计划 1「不做」）
      if computer != nil {
        return NoteInfo(tone: .warn, icon: "lock.fill", title: title, hint: String(localized: "电脑上有登录状态，交给电脑下"), label: String(localized: "在电脑上下"))
      }
      return NoteInfo(tone: .warn, icon: "lock.fill", title: title, hint: cjk(String(localized: "配对一台电脑，在电脑上登录"), site, String(localized: "以后就能下")), label: String(localized: "去配对"))
    case .some(.none):
      return NoteInfo(tone: .err, icon: "exclamationmark.circle.fill", title: title, hint: f?.code == .drm ? String(localized: "受版权保护的视频，所有下载工具都下不了") : nil, label: nil)
    case .retry:
      let hint: String? =
        switch f?.code {
        case .geo: String(localized: "换个网络或者打开代理再试")
        case .network: String(localized: "网络恢复以后重试，会从停下的地方继续")
        default: nil
        }
      return NoteInfo(tone: .err, icon: "exclamationmark.circle.fill", title: title, hint: hint, label: String(localized: "重试"))
    case .autoRetry:
      return NoteInfo(tone: .warn, icon: "clock", title: title, hint: String(localized: "过几分钟会自动再试，也可以现在重试"), label: String(localized: "重试"))
    case .changeDir:
      return NoteInfo(tone: .err, icon: "exclamationmark.circle.fill", title: title, hint: String(localized: "清出一些空间再试"), label: String(localized: "重试"))
    case .useExtension:
      return NoteInfo(tone: .err, icon: "exclamationmark.circle.fill", title: title, hint: computer == nil ? nil : String(localized: "在电脑上的浏览器里打开，用 Captae 插件下"), label: computer == nil ? nil : String(localized: "在电脑上下"))
    default:
      return NoteInfo(tone: .err, icon: "exclamationmark.circle.fill", title: title, hint: String(localized: "可以复制错误信息，再试一次"), label: String(localized: "重试"))
    }
  }

  /// 进度下面那一行（下载中、暂停、排队、合并）
  static func stats(for item: DownloadItem) -> [String] {
    let j = item.job
    switch item.view {
    case .dl:
      var parts = [j.size > 0 ? "\(Fmt.mb(j.got)) / \(Fmt.mb(j.size))" : String(localized: "已下载 \(Fmt.mb(j.got))"), Fmt.speed(j.speed)]
      if let eta = j.etaSeconds { parts.append(String(localized: "剩余 \(Fmt.dur(eta))")) }
      return parts
    case .pause:
      return [String(localized: "已暂停"), j.isHLS ? String(localized: "已下载的部分会保留") : j.size > 0 ? "\(Fmt.mb(j.got)) / \(Fmt.mb(j.size))" : ""].filter { !$0.isEmpty }
    case .queue:
      return [String(localized: "排队中 · 前面还有 \(item.ahead) 个")]
    case .merge:
      return [j.isHLS ? String(localized: "快好了，正在生成视频文件") : String(localized: "下载好了，正在整理成视频文件"), "\(Int(j.merge.rounded()))%"]
    default:
      return []
    }
  }

  /// 清晰度说明：最高一档叫「最清晰」，最低一档叫「省流量」（桌面端 variantNote）
  static func variantNote(_ k: Int, of n: Int) -> String {
    guard n > 1 else { return "" }
    if k == 0 { return String(localized: "最清晰") }
    if k == n - 1 { return String(localized: "省流量") }
    return k == 1 ? String(localized: "高清") : String(localized: "标清")
  }

  /// 步骤时间线（桌面端 flowOf + stepsOf）。`saveTo` 是「放进素材库」下面那行（手机上是「相册」或「文件」）
  static func steps(for item: DownloadItem, saveTo: String) -> [Step] {
    let j = item.job
    let names: [String]
    var subs: [String?]
    let at: Int
    if j.isHLS {
      names = [String(localized: "读取视频信息"), String(localized: "选择清晰度"), String(localized: "下载"), String(localized: "生成视频文件"), String(localized: "放进素材库")]
      subs = [Site.host(j.url), j.variantName.isEmpty ? nil : j.variantName, nil, String(localized: "存成一个 MP4"), saveTo]
      at = phaseIndex(j.phase, [.probe: 0, .pick: 1, .dl: 2, .merge: 3, .post: 3])
    } else if j.eng == .ytDlp {
      names = [String(localized: "读取视频信息"), String(localized: "选择画质"), j.audio ? String(localized: "下载声音") : String(localized: "下载视频"), String(localized: "整理成视频文件"), String(localized: "放进素材库")]
      subs = [j.siteName, j.untitled ? nil : j.fmt, nil, nil, saveTo]
      at = phaseIndex(j.phase, [.probe: 0, .pick: 1, .dl: 2, .merge: 3, .post: 3])
    } else {
      names = [String(localized: "连接服务器"), String(localized: "下载"), String(localized: "检查文件是否完整"), String(localized: "放进素材库")]
      subs = [Site.host(j.url), String(localized: "暂停后可以继续"), nil, saveTo]
      at = phaseIndex(j.phase, [.probe: 0, .pick: 0, .dl: 1, .merge: 2, .post: 2])
    }
    let cur = j.state == .done ? names.count : at
    return names.enumerated().map { i, label in
      var state: Step.State = i < cur ? .done : i == cur ? .now : .todo
      var sub = subs[i]
      if i == cur {
        switch j.state {
        case .failed:
          // 要登录、要验证是橙色，其余失败是红色（2.8 第 4 条）
          let action = j.fail?.act
          state = action == .login || action == .verify ? .warn : .err
          sub = j.fail?.message ?? String(localized: "没有成功")
        case .queue: sub = String(localized: "排队中 · 最多同时下载 3 个")
        case .pause: sub = String(localized: "已暂停 · 已下载的 \(Fmt.mb(j.got)) 会保留")
        default:
          switch j.phase {
          case .probe: sub = String(localized: "正在打开网页")
          case .pick: sub = String(localized: "选一个清晰度，倒计时结束自动用 \(j.variantName)")
          case .merge, .post: sub = "\(Int(j.merge.rounded()))%"
          case .dl: sub = j.size > 0 ? "\(Fmt.percent(j.progress)) · \(Fmt.speed(j.speed))" : "\(Fmt.mb(j.got)) · \(Fmt.speed(j.speed))"
          case .done: break
          }
        }
      }
      return Step(label: label, sub: sub, state: state)
    }
  }

  private static func phaseIndex(_ p: Phase, _ map: [Phase: Int]) -> Int { map[p] ?? 0 }
}
