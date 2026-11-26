// 下载列表里的一张任务卡：上面封面 + 标题 + 来源，下面一块随状态变化（读取、选清晰度、进度、排队、要登录、失败……）。
// 照桌面端 `pages/downloads/JobRow.tsx`，手机竖屏窄，把状态块放到整张卡的宽度。
// 卡片在内容层：白卡、不透明、不做玻璃。进度、速度、剩余时间用等宽数字（数字不做动画，MOTION c6）。
// 动效：状态块换种类时（读取中 → 选清晰度 → 进度 → 完成 / 失败）新的从上方 8pt + 模糊进来、旧的往下出，弹簧 0.3s；
// 带进度条的几种（下载中、暂停、排队、处理中）之间不播互换（MOTION c「状态块」）。暂停 ↔ 继续图标 100ms 互换（c7）。
import SwiftUI

/// 任务卡上的操作（接上 Rust 以后映射到 `JobAction`）
enum JobCardAction {
  case open
  case toggle
  case cancel
  case pickNow
  case pick(Int)
  case startNow
  case note
  case copyLink
  /// 录直播：停止并保存
  case stop
  /// 等电脑上线的改成在这台手机上下
  case moveToPhone
  /// 手机上的改成交给电脑下
  case moveToComputer
}

struct JobCard: View {
  var item: DownloadItem
  /// 配对过的电脑名字（没配对时为空）：状态行按钮上用
  var computer: String?
  /// M3U8 选清晰度的倒计时一共几秒（设置里的值）
  var pickSeconds = 10
  /// 已完成后面那句：已存进相册 / 已存到「文件」
  var savedNote: String?
  /// 设计稿截图：读取中的秒数停在 6.3s
  var still = false
  var onAction: (JobCardAction) -> Void = { _ in }

  @State private var choice: Int
  @Environment(\.dynamicTypeSize) private var typeSize
  @Environment(\.accessibilityReduceMotion) private var reduce

  init(
    item: DownloadItem, computer: String?, pickSeconds: Int = 10, savedNote: String? = nil, still: Bool = false,
    onAction: @escaping (JobCardAction) -> Void = { _ in }
  ) {
    self.item = item
    self.computer = computer
    self.pickSeconds = pickSeconds
    self.savedNote = savedNote
    self.still = still
    self.onAction = onAction
    _choice = State(initialValue: item.job.vIdx)
  }

  /// 状态块属于哪一类：同一类里换内容不播互换（带进度条的几种算一类）
  private var blockKind: Int {
    switch view {
    case .probe: 0
    case .pick: 1
    case .dl, .pause, .queue, .merge: 2
    case .done: 3
    default: 4
    }
  }

  private var j: Job { item.job }
  private var view: JobView { item.view }

  var body: some View {
    // 整张卡是一个按钮：按下缩到 .98（审查第 15 条）；卡里的暂停、菜单、清晰度这些按钮照样各管各的
    Button { onAction(.open) } label: {
      content
    }
    .buttonStyle(.cardPress)
    .accessibilityAction(named: "看详情") { onAction(.open) }
  }

  private var content: some View {
    VStack(alignment: .leading, spacing: 12) {
      header
      block
        .id(blockKind)
        .transition(.statusSwap(reduce: reduce))
    }
    .animation(Motion.swap, value: blockKind)
    .onChange(of: j.vIdx) { _, v in choice = v }
    .padding(Spacing.inset)
    .frame(maxWidth: .infinity, alignment: .leading)
    .card()
    .contentShape(.rect(cornerRadius: Radius.lg))
  }

  // MARK: 封面、标题、来源

  /// 平常封面在左、标题在右；辅助功能大字号时封面单独一行，标题占满整张卡的宽度，不截断
  @ViewBuilder
  private var header: some View {
    if typeSize.isAccessibilitySize {
      VStack(alignment: .leading, spacing: 10) {
        HStack(alignment: .top) {
          Thumb(name: j.thumb, duration: j.dur, audio: j.audio)
            .frame(width: 136, height: 76)
          Spacer(minLength: 8)
          actions
        }
        titleBlock(large: true)
      }
    } else {
      HStack(alignment: .top, spacing: 12) {
        Thumb(name: j.thumb, duration: j.dur, audio: j.audio)
          .frame(width: 104, height: 58)
        titleBlock(large: false)
        actions
      }
    }
  }

  /// `large`：辅助功能大字号。长链接无论多大的字都最多两行、截掉中间；真标题大字号时最多三行；元信息可以换行
  private func titleBlock(large: Bool) -> some View {
    VStack(alignment: .leading, spacing: 4) {
      Text(verbatim: j.untitled ? j.src : j.title)
        .font(.cpSecondaryMedium)
        .foregroundStyle(Palette.ink)
        .lineLimit(j.untitled ? 2 : (large ? 3 : 2))
        .truncationMode(j.untitled ? .middle : .tail)
        .fixedSize(horizontal: false, vertical: true)
      HStack(alignment: .firstTextBaseline, spacing: 5) {
        SiteIcon(plat: j.plat, url: j.url)
        Text(verbatim: meta)
          .font(.cpMeta)
          .foregroundStyle(Palette.ink3)
          .lineLimit(large ? nil : 1)
          .fixedSize(horizontal: false, vertical: large)
      }
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .accessibilityElement(children: .combine)
  }

  /// 「哔哩哔哩 · MP4 · 1080p」：还没读到标题时只有网站名
  private var meta: String {
    j.untitled || j.fmt.isEmpty ? j.siteName : "\(j.siteName) · \(j.fmt)"
  }

  /// 大字号时进度下面那一行：「45% · 143 MB / 312 MB · 6.4 MB/s · 剩余 26 秒」
  private var statsLine: String {
    var parts = JobText.stats(for: item)
    if view == .dl, j.size > 0 { parts.insert(Fmt.percent(j.progress), at: 0) }
    return parts.joined(separator: " · ")
  }

  private var canPause: Bool {
    item.hold == nil && (j.state == .run || j.state == .queue || j.state == .pause)
  }

  private var actions: some View {
    // 两个 44pt 的点按区叠一点，图标之间的距离和原来 30pt 时一样
    HStack(spacing: -12) {
      if canPause {
        Button {
          onAction(.toggle)
        } label: {
          ZStack {
            Image(systemName: j.state == .pause ? "play.fill" : "pause.fill")
              .id(j.state == .pause)
              .transition(.iconSwap(reduce: reduce))
          }
          .font(.system(size: 13, weight: .semibold))
          .frame(width: 44, height: 44)
          .contentShape(.rect)
          .animation(Motion.iconSwap, value: j.state == .pause)
        }
        .buttonStyle(.plain)
        .foregroundStyle(Palette.ink2)
        .accessibilityLabel(j.state == .pause ? "继续" : "暂停")
      }
      Menu {
        if canPause {
          Button(j.state == .pause ? "继续" : "暂停", systemImage: j.state == .pause ? "play.fill" : "pause.fill") { onAction(.toggle) }
        }
        if j.recording {
          Button("停止并保存", systemImage: "stop.fill") { onAction(.stop) }
        }
        Button("复制链接", systemImage: "link") { onAction(.copyLink) }
        if j.state.isActive || item.hold != nil {
          Button(j.cancelLabel, systemImage: "xmark", role: .destructive) { onAction(.cancel) }
        } else {
          Button("从列表里去掉", systemImage: "xmark", role: .destructive) { onAction(.cancel) }
        }
      } label: {
        Image(systemName: "ellipsis")
          .font(.system(size: 14, weight: .semibold))
          .frame(width: 44, height: 44)
          .contentShape(.rect)
      }
      .foregroundStyle(Palette.ink2)
      .accessibilityLabel("这个下载的更多操作")
    }
    // 按钮点按区 44pt（课程 01/09「Appropriate target area」），画出来的位置和原来一样
    .padding(.top, -11)
    .padding(.bottom, -7)
    .padding(.trailing, -11)
  }

  // MARK: 状态块

  @ViewBuilder
  private var block: some View {
    switch view {
    case .probe:
      VStack(alignment: .leading, spacing: 8) {
        HStack(spacing: 6) {
          ShimmerText(text: "正在读取视频信息…", font: .cpMeta, still: still)
          Elapsed(since: j.probeAt, still: still)
        }
        ProgressBar(value: 0, indeterminate: true, label: "正在读取视频信息")
      }

    case .pick:
      VStack(alignment: .leading, spacing: 10) {
        WellSegments(
          items: j.vars.enumerated().map { k, v in .init(value: k, title: v.p, detail: Fmt.mb(v.mb)) },
          selection: Binding(get: { choice }, set: { choice = $0; onAction(.pick($0)) }),
          label: "清晰度"
        )
        HStack(spacing: 7) {
          PickCountdown(
            until: still ? Int64((Date.now.timeIntervalSince1970 + 2.4) * 1000) : j.pickUntil, total: still ? 4 : pickSeconds,
            variant: j.vars.indices.contains(choice) ? j.vars[choice].p : j.variantName)
          Spacer(minLength: 8)
          Button("马上开始") { onAction(.pickNow) }
            .buttonStyle(.link)
        }
      }

    case .dl, .pause, .queue, .merge:
      VStack(alignment: .leading, spacing: 8) {
        ProgressBar(
          value: view == .merge ? j.merge / 100 : j.progress,
          tone: view == .pause || view == .queue ? .paused : .accent,
          label: view == .merge ? "处理进度" : "下载进度"
        )
        if typeSize.isAccessibilitySize {
          // 大字号：拼成一段文字自然换行，不挤成一行缩小
          VStack(alignment: .leading, spacing: 6) {
            Text(verbatim: statsLine)
              .font(.cpMeta)
              .monospacedDigit()
              .foregroundStyle(Palette.ink3)
              .fixedSize(horizontal: false, vertical: true)
            if view == .queue {
              Button("马上开始") { onAction(.startNow) }
                .buttonStyle(.link)
            }
          }
        } else {
          HStack(alignment: .firstTextBaseline, spacing: 10) {
            if view == .dl, j.size > 0 {
              Text(verbatim: Fmt.percent(j.progress))
                .font(.cpMetaMedium)
                .foregroundStyle(Palette.ink)
            }
            ForEach(JobText.stats(for: item), id: \.self) { s in
              Text(verbatim: s).foregroundStyle(Palette.ink3)
            }
            Spacer(minLength: 0)
            if view == .queue {
              Button("马上开始") { onAction(.startNow) }
                .buttonStyle(.link)
            }
          }
          .font(.cpMeta)
          .monospacedDigit()
          .lineLimit(1)
          .minimumScaleFactor(0.9)
        }
        if view == .dl, j.segRetrying > 0 {
          Text("网络有点不稳，正在自动重试")
            .font(.cpMeta)
            .foregroundStyle(Palette.warn)
        }
      }

    case .login, .verify, .fail, .computerOnly, .waiting:
      if let note = JobText.note(for: item, computer: computer) {
        StatusRow(tone: note.tone, icon: note.icon, title: note.title, hint: note.hint, stacked: note.stacked) {
          if let label = note.label {
            Button(label) { onAction(.note) }
              .buttonStyle(.secondary)
              .fixedSize()
          }
        }
      }

    case .done:
      HStack(spacing: 10) {
        Label("已完成", systemImage: "checkmark.circle.fill")
          .labelStyle(.titleAndIcon)
          .foregroundStyle(Palette.ok)
        Text(Fmt.mb(j.size)).foregroundStyle(Palette.ink3)
        if let savedNote {
          Text(verbatim: savedNote).foregroundStyle(Palette.ink3)
        }
      }
      .font(.cpMeta)
      .monospacedDigit()
    }
  }
}

/// 读取了多久：「6.3s」，等宽数字，每 0.1 秒走一次（桌面端 format.ts `since`）
struct Elapsed: View {
  /// 从什么时候开始（毫秒时间戳）；没有时不显示
  var since: Int64?
  var still = false

  var body: some View {
    if still {
      label(6.3)
    } else if let since {
      let start = Date(timeIntervalSince1970: Double(since) / 1000)
      TimelineView(.periodic(from: start, by: 0.1)) { ctx in
        label(ctx.date.timeIntervalSince(start))
      }
    }
  }

  private func label(_ t: TimeInterval) -> some View {
    let s = max(0, t)
    let text = s < 60 ? String(format: "%.1fs", s) : String(format: "%dm %02ds", Int(s) / 60, Int(s) % 60)
    return Text(verbatim: text)
      .font(.cpMono(12, relativeTo: .caption))
      .foregroundStyle(Palette.ink4)
      .accessibilityLabel(Text("已经用了 \(Int(s)) 秒"))
  }
}
