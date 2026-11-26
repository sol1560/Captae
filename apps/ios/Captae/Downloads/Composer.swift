// 输入卡（Perplexity 式两行，BRIEF 2.4）：第一行输入链接（认出网站时左边出网站图标），
// 第二行左边「粘贴」，中间两个胶囊菜单（去向、格式），右边圆形发送（近黑，没输入时灰）。
// 两种放法：从下载条点开的 sheet（系统玻璃 sheet，卡本身不加背景）；下载页空状态里内嵌（白卡）。
// 不再有自己加的玻璃：第一版的 `.glassEffect` 输入框去掉了。
// 上架版：认出 YouTube 这类网站、去向又是这台 iPhone 时，去向自动改到电脑，下面一行橙色小字说明。
import CaptaeKit
import SwiftUI

/// 保存成什么（桌面端 model.ts 的 FMT）
enum FormatChoice: String, CaseIterable, Identifiable {
  case best = "MP4 · 最清晰"
  case p1080 = "MP4 · 1080p"
  case p720 = "MP4 · 720p"
  case mp3 = "只要声音 · MP3"
  case webm = "WebM · 最清晰"
  case mkv = "MKV · 最清晰"
  case keep = "保持原来的格式"
  case aac = "只要声音 · AAC"
  case flac = "只要声音 · FLAC"

  var id: String { rawValue }
  static let main: [FormatChoice] = [.best, .p1080, .p720, .mp3]
  static let more: [FormatChoice] = [.webm, .mkv, .keep, .aac, .flac]

  /// 显示用的名字（进本地化）
  var title: LocalizedStringKey { LocalizedStringKey(rawValue) }
}

struct ComposerCard: View {
  enum Style {
    /// 在 sheet 里：系统玻璃 sheet 是底，卡本身不加背景
    case sheet
    /// 下载页空状态里：白卡
    case embedded
  }

  var style: Style
  @Binding var text: String
  @Binding var target: Placement
  @Binding var format: FormatChoice
  var focus: FocusState<Bool>.Binding
  var onSend: () -> Void

  @Environment(AppModel.self) private var model
  /// 上架版自动把去向改到了电脑（下面一行橙色小字）
  @State private var redirectedSite: String?

  private var canSend: Bool { !text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty }
  private var siteIcon: String? { canSend ? Site.iconAsset(plat: nil, url: text) : nil }

  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      HStack(alignment: .firstTextBaseline, spacing: 8) {
        if siteIcon != nil {
          SiteIcon(plat: nil, url: text, size: 16)
            .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 2 }
        }
        TextField(
          "链接", text: $text, prompt: Text("粘贴视频、音乐或图片的链接").foregroundStyle(Palette.ink3), axis: .vertical
        )
        .font(.cpBody)
        .foregroundStyle(Palette.ink)
        .lineLimit(1...3)
        .keyboardType(.URL)
        .textInputAutocapitalization(.never)
        .autocorrectionDisabled()
        .submitLabel(.go)
        .focused(focus)
      }
      .padding(.horizontal, 4)
      .padding(.top, 2)

      toolRow

      if let site = redirectedSite {
        Label {
          Text(verbatim: cjk("", site, String(localized: "的视频在电脑上下")))
        } icon: {
          Image(systemName: "laptopcomputer")
        }
        .font(.cpSmall)
        .foregroundStyle(Palette.warn)
        .padding(.leading, 4)
        .accessibilityLabel(model.computer == nil ? Text("\(site) 的视频在电脑上下") : Text("\(site) 的视频在电脑上下，已经改成发到电脑"))
      }
    }
    .padding(.horizontal, style == .embedded ? 12 : 0)
    .padding(.top, style == .embedded ? 12 : 0)
    .padding(.bottom, style == .embedded ? 10 : 0)
    .background {
      if style == .embedded {
        RoundedRectangle(cornerRadius: Radius.xxl, style: .continuous)
          .fill(Palette.card)
          .shadow(color: .black.opacity(0.04), radius: 1, y: 1)
          .overlay {
            RoundedRectangle(cornerRadius: Radius.xxl, style: .continuous)
              .strokeBorder(Color.black.opacity(0.08), lineWidth: 0.5)
          }
      }
    }
    .onChange(of: text, initial: true) { _, _ in redirectIfNeeded() }
  }

  /// 第二行：粘贴、去向、格式、发送
  private var toolRow: some View {
    HStack(spacing: 4) {
      PasteButton(payloadType: String.self) { strings in
        if let s = strings.first { text = s }
      }
      .labelStyle(.iconOnly)
      .buttonBorderShape(.circle)
      .controlSize(.small)
      .tint(Palette.ink2)

      // 去向的名字太长时截掉，但先于 Spacer 拿宽度；格式和发送按钮不让位
      targetMenu
        .layoutPriority(0.5)
      formatMenu
        .layoutPriority(1)
      Spacer(minLength: 0)
      sendButton
    }
  }

  /// 上架版：只能在电脑上下的网站、去向又是手机 → 改到电脑，下面一行橙色小字。
  /// 没配对电脑时只出这行字（发送以后存进待发列表，配对了电脑再发过去）。准不准以核心为准：发送时核心说 unsupported_here 也照这样处理
  private func redirectIfNeeded() {
    guard model.appStoreBuild, canSend else {
      redirectedSite = nil
      return
    }
    let site = Site.label(plat: nil, url: text)
    guard Site.computerOnly(site) else {
      redirectedSite = nil
      return
    }
    if target == .phone && model.computer != nil { target = .computer }
    redirectedSite = site
  }

  /// 去向：「发到 Sol 的 MacBook Pro」/「在这台 iPhone 上下」
  private var targetMenu: some View {
    Menu {
      if let computer = model.computer {
        Button {
          target = .computer
        } label: {
          Label(cjk("发到", computer.name), systemImage: target == .computer ? "checkmark" : "laptopcomputer")
          if !computer.online { Text("现在连不上，回到同一个 Wi-Fi 后自动发过去") }
        }
      }
      Button {
        target = .phone
        redirectedSite = nil
      } label: {
        Label(cjk("在", model.phoneLabel, "上下"), systemImage: target == .phone ? "checkmark" : "iphone")
      }
    } label: {
      // 放得下就写全「发到 Sol 的 MacBook Pro」；放不下换成电脑图标 + 名字；再不行截掉名字末尾
      ViewThatFits(in: .horizontal) {
        Chip(icon: nil, text: targetText)
        Chip(icon: target == .computer ? "laptopcomputer" : "iphone", text: targetName)
        Chip(icon: target == .computer ? "laptopcomputer" : "iphone", text: targetName, truncate: true)
      }
    }
    .accessibilityLabel("下载到哪里")
  }

  private var targetText: String {
    target == .computer ? (model.computer.map { cjk("发到", $0.name) } ?? "") : cjk("在", model.phoneLabel, "上下")
  }

  private var targetName: String {
    target == .computer ? (model.computerName ?? "") : model.phoneLabel
  }

  /// 上架版手机上只下直链和 M3U8，没有 ffmpeg 转格式、抽声音，只给清晰度（计划 5.4）
  private var choices: [FormatChoice] { model.appStoreBuild ? [.best, .p1080, .p720] : FormatChoice.main }

  private var formatMenu: some View {
    Menu {
      Picker("保存成", selection: $format) {
        ForEach(choices) { Text($0.title).tag($0) }
      }
      if !model.appStoreBuild {
        Menu("更多格式") {
          Picker("更多格式", selection: $format) {
            ForEach(FormatChoice.more) { Text($0.title).tag($0) }
          }
        }
      }
    } label: {
      Chip(icon: nil, text: format.rawValue)
        .fixedSize()
    }
    .accessibilityLabel(Text("保存成 \(format.rawValue)"))
  }

  private var sendButton: some View {
    Button(action: onSend) {
      Image(systemName: "arrow.up")
        .font(.system(size: 15, weight: .semibold))
        .foregroundStyle(.white)
        .frame(width: 34, height: 34)
        .background(canSend ? Palette.ink : Color(hex: 0xD4D4D4), in: .circle)
        .animation(Motion.tint, value: canSend)
        .contentShape(.circle.inset(by: -5))
    }
    .buttonStyle(.circle)
    .disabled(!canSend)
    .accessibilityLabel(target == .computer ? "发到电脑" : "开始下载")
  }
}

/// 胶囊菜单按钮：浅灰凹面胶囊里的图标 + 文字 + 小箭头（内容层的面，不是玻璃）
private struct Chip: View {
  var icon: String?
  var text: String
  /// 放不下时截掉末尾；默认不截（交给外面的 ViewThatFits 挑能放下的那种）
  var truncate = false
  @Environment(\.dynamicTypeSize) private var typeSize

  /// 字大到放不下名字时只留图标（名字在读屏标签里）
  private var compact: Bool { icon != nil && typeSize >= .xxLarge }

  var body: some View {
    HStack(spacing: 4) {
      if let icon {
        Image(systemName: icon).font(.system(size: 11, weight: .medium))
      }
      if !compact {
        Text(verbatim: text)
          .font(.cpSmall)
          .lineLimit(1)
          .fixedSize(horizontal: !truncate, vertical: false)
      }
      Image(systemName: "chevron.down").font(.system(size: 9, weight: .semibold))
    }
    .foregroundStyle(Palette.ink2)
    .padding(.horizontal, 9)
    .padding(.vertical, 7)
    .background(Palette.well, in: .capsule)
    .contentShape(.capsule)
  }
}

// MARK: - 从下载条点开的输入卡 sheet

/// 输入卡走到哪了
private enum ComposerPhase: Equatable {
  case editing
  /// 点了发送，正在读取视频信息
  case reading
  /// 读取或加任务失败（按错误码说，和桌面端同一套说法）
  case failed(Failure)

  /// 互换动画看的种类
  var kind: Int {
    switch self {
    case .editing: 0
    case .reading: 1
    case .failed: 2
    }
  }
}

/// 贴着键盘的小 sheet（系统玻璃）。点了发送以后进入读取中：输入卡后面一层很淡的流光（BRIEF 2.6），右边停止按钮取消这次读取。
/// 读到了就交给核心开始下，sheet 收起；上架版遇到 YouTube 这类网站存进待发列表（下载页那一行写「在电脑上下」）。
struct ComposerSheet: View {
  var session: ComposerSession

  @Environment(AppModel.self) private var model
  @Environment(\.accessibilityReduceMotion) private var reduce
  @State private var text: String
  @State private var target: Placement
  @State private var format: FormatChoice
  @State private var phase: ComposerPhase = .editing
  @State private var token: CancelToken?
  /// 读到封面以后流光换成封面的颜色
  @State private var cover: Color?
  @State private var height: CGFloat = 140
  /// 编辑时输入卡的高度：读取中的卡片不比它矮，换过去时 sheet 高度不变（内容和 sheet 不会一前一后动两次，审查第 29 条）
  @State private var editHeight: CGFloat = 0
  @FocusState private var focused: Bool

  /// `fallback`：没指定去向时的默认（计划 3.2：配对过且在线 → 电脑；没配对 → 手机）
  init(session: ComposerSession, fallback: Placement) {
    self.session = session
    _text = State(initialValue: session.text)
    _target = State(initialValue: session.target ?? fallback)
    _format = State(initialValue: session.format)
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 12) {
      Group {
        if phase == .reading {
          ReadingCard(text: text, target: target, tint: cover ?? Site.tint(url: text), still: model.isDemo, onStop: stop)
            .frame(minHeight: editHeight, alignment: .top)
        } else {
          ComposerCard(style: .sheet, text: $text, target: $target, format: $format, focus: $focused, onSend: send)
            .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { editHeight = $0 }
        }
      }
      .id(phase == .reading)
      .transition(.statusSwap(reduce: reduce))
      if case let .failed(f) = phase {
        StatusRow(tone: .err, icon: "exclamationmark.circle.fill", title: f.message, hint: nil) {
          Button("重试", action: send)
            .buttonStyle(.secondary)
            .fixedSize()
        }
        .transition(.statusSwap(reduce: reduce))
      }
    }
    .animation(Motion.swap, value: phase.kind)
    .padding(.horizontal, 12)
    .padding(.top, 22)
    .padding(.bottom, 14)
    .fixedSize(horizontal: false, vertical: true)
    .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height = $0 }
    .frame(maxHeight: .infinity, alignment: .top)
    // 滚动边缘：柔和的渐隐模糊（BRIEF 2.7）。输入卡不滚动，加在 sheet 的根上
    .scrollEdgeEffectStyle(.soft, for: .all)
    .presentationDetents([.height(height)])
    .presentationDragIndicator(.visible)
    .onAppear {
      if session.reading {
        // 下载页空状态里点的发送：sheet 一出来就在读取
        if model.isDemo { phase = .reading } else { send() }
      } else if !model.demo || model.demoFocusComposer {
        focused = true
      }
    }
    // 往下拖掉 sheet：正在读的一起停
    .onDisappear { token?.cancel() }
  }

  private func send() {
    let url = text.trimmingCharacters(in: .whitespacesAndNewlines)
    guard !url.isEmpty else { return }
    focused = false
    cover = nil
    let token = CancelToken()
    self.token = token
    phase = .reading
    Task {
      do {
        _ = try await model.submit(url: url, target: target, format: format, token: token) { probe in
          guard let thumb = probe.thumbnail else { return }
          Task {
            if let c = await ImageStore.shared.averageColor(thumb) {
              // 颜色过渡由 FlowLight 自己的 .animation(value: tint) 管（审查第 35 条：不再写两处）
              cover = c
            }
          }
        }
        model.composer = nil
      } catch MobileError.Failed(let f) where f.code == "cancelled" {
        // 停止：回到能改链接的输入卡
        phase = .editing
        focused = true
      } catch MobileError.Failed(let f) {
        phase = .failed(Failure(f))
      } catch {
        phase = .failed(Failure(code: .unknown, message: error.localizedDescription))
      }
    }
  }

  private func stop() {
    token?.cancel()
  }
}

/// 读取中（BRIEF 2.8 第 7 条）：第一行网站图标 + 链接（一行，截掉中间）；第二行左边扫光的「正在读取视频信息…」
/// 和已经用了几秒，右边去向；底下一条不确定进度；最右边一个停止按钮（取消这次读取，和发送按钮同一个位置）。
/// 后面一层流光（2.6）：先是网站的颜色，读到封面后换成封面的颜色，读完就收起；减少动态效果时是一层静止的淡色
private struct ReadingCard: View {
  var text: String
  var target: Placement
  var tint: Color
  /// 设计稿截图：秒数停在 1.8s、流光是静止帧
  var still: Bool
  var onStop: () -> Void
  @Environment(AppModel.self) private var model
  @State private var started = Date.now

  /// 显示用的链接：去掉协议和 www
  private var shownURL: String {
    Site.host(text) + (URL(string: text)?.path ?? "") + (URL(string: text)?.query.map { "?" + $0 } ?? "")
  }

  var body: some View {
    HStack(alignment: .center, spacing: 12) {
      VStack(alignment: .leading, spacing: 10) {
        HStack(alignment: .firstTextBaseline, spacing: 8) {
          SiteIcon(plat: nil, url: text, size: 16)
            .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 2 }
          Text(verbatim: shownURL)
            .font(.cpBody)
            .foregroundStyle(Palette.ink)
            .lineLimit(1)
            .truncationMode(.middle)
        }
        HStack(alignment: .firstTextBaseline, spacing: 6) {
          ShimmerText(text: "正在读取视频信息…", font: .cpMeta, still: still)
            .layoutPriority(1)
          elapsed
          Spacer(minLength: 8)
          Text(verbatim: target == .computer ? cjk("发到", model.computerName ?? "") : cjk("在", model.phoneLabel, "上下"))
            .font(.cpSmall)
            .foregroundStyle(Palette.ink3)
            .lineLimit(1)
            .truncationMode(.middle)
        }
        ProgressBar(value: 0, indeterminate: true, label: String(localized: "正在读取"))
      }
      stopButton
    }
    .padding(.horizontal, 4)
    .padding(.vertical, 6)
    .background { FlowLight(tint: tint, still: still).padding(-28) }
  }

  /// 已经用了几秒：「1.8s」，等宽数字，每 0.1 秒走一次；演示时停在 1.8s，截图稳定
  @ViewBuilder
  private var elapsed: some View {
    if still {
      seconds(1.8)
    } else {
      TimelineView(.periodic(from: started, by: 0.1)) { ctx in
        seconds(ctx.date.timeIntervalSince(started))
      }
    }
  }

  private func seconds(_ t: TimeInterval) -> some View {
    Text(verbatim: String(format: "%.1fs", t))
      .font(.cpMono(12, relativeTo: .caption))
      .foregroundStyle(Palette.ink4)
      .accessibilityLabel(Text("已经用了 \(Int(t)) 秒"))
  }

  /// 停止：近黑圆 + 白色实心方块，和发送按钮同样大小、同一个位置
  private var stopButton: some View {
    Button(action: onStop) {
      Image(systemName: "stop.fill")
        .font(.system(size: 12, weight: .semibold))
        .foregroundStyle(.white)
        .frame(width: 34, height: 34)
        .background(Palette.ink, in: .circle)
        .contentShape(.circle.inset(by: -5))
    }
    .buttonStyle(.circle)
    .accessibilityLabel("停止读取")
  }
}

/// 流光（BRIEF 2.6）：两团很淡的颜色，从中间向外淡出（渐变本身就是柔边，不另加模糊），读取的那几秒里各绕一个小圈慢慢游动
/// （3.7s、2.9s，故意不同步，不像在呼吸）。每团光是一个固定的径向渐变圆，偏离中心放着，外面整体旋转：
/// 动的只是旋转，交给系统按帧插值，不每帧重画渐变（审查第 28 条）。
/// 只在读取中出现，常驻页面上不放；减少动态效果时停成一层静止的淡色（MOTION 1.5、课程 04/03：循环动画停在代表帧）
struct FlowLight: View {
  var tint: Color
  var still = false
  @Environment(\.accessibilityReduceMotion) private var reduce
  @State private var shown = false
  @State private var turning = false

  var body: some View {
    GeometryReader { geo in
      let w = geo.size.width, h = geo.size.height
      ZStack {
        blob(opacity: 0.22, radius: w * 0.32, period: 3.7)
          .position(x: w * 0.28, y: h * 0.55)
        blob(opacity: 0.14, radius: w * 0.26, period: 2.9, reverse: true)
          .position(x: w * 0.76, y: h * 0.36)
      }
    }
    .opacity(shown || still ? 1 : 0)
    .onAppear {
      // 出现：300ms ease-out-quint 淡入（MOTION 1.2）
      withAnimation(Motion.reveal) { shown = true }
      turning = true
    }
    .animation(.easeInOut(duration: 0.4), value: tint)
    .allowsHitTesting(false)
    .accessibilityHidden(true)
  }

  /// 一团光：离中心一小段放着，绕中心转一圈（看起来是光在游动）
  private func blob(opacity: Double, radius: CGFloat, period: Double, reverse: Bool = false) -> some View {
    let moving = !(still || reduce)
    return RadialGradient(colors: [tint.opacity(opacity), tint.opacity(0)], center: .center, startRadius: 0, endRadius: radius)
      .frame(width: radius * 2, height: radius * 2)
      .offset(x: radius * 0.18)
      .rotationEffect(.degrees(moving && turning ? (reverse ? -360 : 360) : 0))
      .animation(moving ? .linear(duration: period).repeatForever(autoreverses: false) : nil, value: turning)
  }
}
