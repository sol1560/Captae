// 任务详情：系统 sheet（中、大两档，玻璃由系统画），点任务卡打开。
// 照桌面端右侧详情浮卡 `pages/downloads/Detail.tsx`：封面、标题、来源，操作，分节「进行到哪一步」「清晰度」「会保存为」。
// 分节内容放白卡（内容层），不给 sheet 加自定义背景。
// 有封面的任务：顶部一张整宽的封面，`backgroundExtensionEffect()` 把它延伸、模糊成一层光铺到导航栏下面（BRIEF 2.6），
// 颜色来自这条视频自己；没封面、声音没有封面时不加。关闭按钮在左上角系统的取消位置。
import CaptaeKit
import SwiftUI

struct JobDetailSheet: View {
  /// 打开时的那一条；显示的是它在列表里的最新样子（核心的事件一直在更新，见 `item`）
  var opened: DownloadItem

  @Environment(AppModel.self) private var model
  @Environment(\.dismiss) private var dismiss
  @Environment(\.accessibilityReduceMotion) private var reduce
  @State private var choice: Int
  @State private var picked: Set<String> = []

  init(item: DownloadItem) {
    self.opened = item
    _choice = State(initialValue: item.job.vIdx)
  }

  /// 最新的样子；任务已经从列表里去掉了就停在最后看到的样子
  private var item: DownloadItem { model.items.first { $0.id == opened.id } ?? opened }
  private var j: Job { item.job }
  private func act(_ a: JobCardAction) { model.perform(a, on: item) }

  private var blockKind: Int {
    if j.choose && !j.entries.isEmpty { return 3 }
    switch item.view {
    case .pick: return 0
    case .login, .verify, .fail, .computerOnly, .waiting: return 1
    default: return 2
    }
  }

  /// 「放进素材库」下面那行：电脑上的任务存在电脑的「影片 › Captae」，手机上的存相册或「文件」
  private var saveTo: String {
    if item.placement == .computer { return "\(j.audio ? "音乐" : "影片") › Captae" }
    return j.audio || model.settings.saveTo == .files ? "「文件」› Captae" : "相册"
  }

  var body: some View {
    NavigationStack {
      // 封面固定在顶部，作为滚动视图的顶栏（`safeAreaBar`）：贴着标题栏，`backgroundExtensionEffect()` 把它
      // 延伸、模糊到标题栏下面（2.6）；下面的内容从它下面滚过时是柔和的渐隐边缘，不切出硬线（2.7）
      ScrollView {
        VStack(alignment: .leading, spacing: 18) {
          if hasCover { titleBlock } else { header }
          Group {
            if j.choose && !j.entries.isEmpty {
              entriesSection
            } else {
              switch item.view {
              case .pick: pickSection
              case .login, .verify, .fail, .computerOnly, .waiting: note
              default: actions
              }
            }
          }
          .id(blockKind)
          .transition(.statusSwap(reduce: reduce))
          section("进行到哪一步") {
            StepTimeline(steps: JobText.steps(for: item, saveTo: saveTo))
              .padding(14)
              .frame(maxWidth: .infinity, alignment: .leading)
              .card()
          }
          if j.state != .failed {
            section("会保存为") { saveAs }
          }
          let log = model.log(of: item)
          if !log.isEmpty {
            section("下载记录") { logSection(log) }
          }
        }
        .animation(Motion.swap, value: blockKind)
        .padding(.horizontal, Spacing.page + 4)
        .padding(.top, hasCover ? 14 : 4)
        .padding(.bottom, 24)
      }
      // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
      .scrollEdgeEffectStyle(.soft, for: .all)
      .safeAreaBar(edge: .top, spacing: 0) {
        if hasCover { hero }
      }
      // 标题「下载详情」；网站和设备写在大标题下面一行（BRIEF 2.8 第 4 条）
      .navigationTitle("下载详情")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) {
          Button(role: .close) { dismiss() }
        }
      }
    }
    .presentationDetents([.medium, .large])
    .presentationDragIndicator(.visible)
  }

  // MARK: 封面、标题、来源

  private var hasCover: Bool { j.thumb != nil && !j.audio }

  /// 整宽封面：系统把它延伸、模糊到导航栏下面，成为顶部的一层光
  private var hero: some View {
    Thumb(name: j.thumb, duration: j.dur, radius: 0)
      .frame(height: 150)
      .frame(maxWidth: .infinity)
      .backgroundExtensionEffect()
  }

  /// 标题：没读到标题时写「还没读到标题」（桌面端 Detail.tsx 的说法，BRIEF 2.8 第 4 条）
  private var titleText: String {
    j.untitled ? String(localized: "还没读到标题") : j.title
  }

  /// 标题下面一行：网站 · 在哪台设备（「哔哩哔哩 · 这台 iPhone」「YouTube · Sol 的 MacBook Pro」）
  private var whereLine: String {
    let device = item.placement == .computer ? (model.computerName ?? String(localized: "电脑")) : model.phoneLabel
    return "\(j.siteName) · \(device)"
  }

  /// 标题 + 网站和设备 + 链接（链接一行，截掉中间）
  private var titleLines: some View {
    VStack(alignment: .leading, spacing: 5) {
      Text(verbatim: titleText)
        .font(.cpTitle)
        .foregroundStyle(j.untitled ? Palette.ink2 : Palette.ink)
        .fixedSize(horizontal: false, vertical: true)
      HStack(alignment: .firstTextBaseline, spacing: 5) {
        SiteIcon(plat: j.plat, url: j.url)
        Text(verbatim: whereLine)
          .font(.cpMeta)
          .foregroundStyle(Palette.ink3)
          .lineLimit(2)
      }
      Text(verbatim: j.src)
        .font(.cpMono(12, relativeTo: .caption))
        .foregroundStyle(Palette.accentInk)
        .lineLimit(1)
        .truncationMode(.middle)
    }
  }

  private var titleBlock: some View {
    titleLines
      .accessibilityElement(children: .combine)
  }

  private var header: some View {
    HStack(alignment: .top, spacing: 14) {
      Thumb(name: j.thumb, duration: j.dur, radius: Radius.md, audio: j.audio)
        .frame(width: 120, height: 68)
      titleLines
    }
    .accessibilityElement(children: .combine)
  }

  // MARK: 操作

  private var actions: some View {
    HStack(spacing: 10) {
      if j.state == .run || j.state == .queue || j.state == .pause {
        if j.recording {
          Button("停止并保存", systemImage: "stop.fill") { act(.stop) }
            .buttonStyle(.secondaryLarge)
        } else {
          Button(j.state == .pause ? "继续" : "暂停", systemImage: j.state == .pause ? "play.fill" : "pause.fill") { act(.toggle) }
            .buttonStyle(.secondaryLarge)
        }
        // 「取消下载」，录直播时「取消录制」（2.8 第 3 条）
        Button(j.cancelLabel, systemImage: "xmark") {
          act(.cancel)
          dismiss()
        }
        .buttonStyle(.secondaryLarge)
      } else if j.state == .done, let path = j.path {
        ShareLink(item: URL(filePath: path)) {
          Label("分享", systemImage: "square.and.arrow.up")
        }
        .buttonStyle(.secondaryLarge)
      }
      Spacer(minLength: 0)
    }
    .labelStyle(.titleAndIcon)
  }

  @ViewBuilder
  private var note: some View {
    if let info = JobText.note(for: item, computer: model.computerName) {
      StatusRow(tone: info.tone, icon: info.icon, title: info.title, hint: info.hint, stacked: info.stacked) {
        if let label = info.label {
          Button(label) { act(.note) }
            .buttonStyle(.secondary)
            .fixedSize()
        }
      }
    }
  }

  // MARK: 选清晰度

  private var pickSection: some View {
    VStack(alignment: .leading, spacing: 14) {
      HStack(spacing: 8) {
        PickCountdown(
          until: model.isDemo ? Int64((Date.now.timeIntervalSince1970 + 2.4) * 1000) : j.pickUntil,
          total: model.isDemo ? 4 : model.settings.pickSeconds,
          variant: j.vars.indices.contains(choice) ? j.vars[choice].p : j.variantName, ringSize: 18, font: .cpSecondary)
        Spacer(minLength: 8)
        Button("马上开始") { act(.pickNow) }
          .buttonStyle(.primary)
      }
      .padding(.leading, 4)
      section("清晰度") {
        VStack(spacing: 0) {
          ForEach(Array(j.vars.enumerated()), id: \.offset) { k, v in
            Button {
              choice = k
              act(.pick(k))
            } label: {
              HStack(spacing: 12) {
                Radio(on: k == choice)
                VStack(alignment: .leading, spacing: 1) {
                  Text(v.p).font(.cpSecondaryMedium).foregroundStyle(Palette.ink)
                  Text([JobText.variantNote(k, of: j.vars.count), v.r].filter { !$0.isEmpty }.joined(separator: " · "))
                    .font(.cpMeta)
                    .foregroundStyle(Palette.ink3)
                }
                Spacer(minLength: 8)
                Text(Fmt.mb(v.mb))
                  .font(.cpMono(13, relativeTo: .footnote))
                  .foregroundStyle(Palette.ink2)
              }
              .padding(.horizontal, 14)
              .padding(.vertical, 10)
              .contentShape(.rect)
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(k == choice ? .isSelected : [])
            if k < j.vars.count - 1 { RowDivider(leading: 46) }
          }
        }
        .card()
      }
    }
  }

  // MARK: 播放列表选条目

  /// 播放列表：勾选要下的条目，每条一个新任务（核心 choose_entries）
  private var entriesSection: some View {
    VStack(alignment: .leading, spacing: 12) {
      HStack {
        Text("这是一个播放列表，选要下的")
          .font(.cpSecondary)
          .foregroundStyle(Palette.ink2)
        Spacer(minLength: 8)
        Button(picked.count == j.entries.count ? "全不选" : "全选") {
          picked = picked.count == j.entries.count ? [] : Set(j.entries.map(\.id))
        }
        .buttonStyle(.link)
      }
      VStack(spacing: 0) {
        ForEach(j.entries) { e in
          Button {
            if picked.contains(e.id) { picked.remove(e.id) } else { picked.insert(e.id) }
          } label: {
            HStack(spacing: 12) {
              Image(systemName: picked.contains(e.id) ? "checkmark.circle.fill" : "circle")
                .font(.system(size: 20))
                .foregroundStyle(picked.contains(e.id) ? Palette.accent : Palette.ink4)
              Text(verbatim: "\(e.index). \(e.title)")
                .font(.cpSecondary)
                .foregroundStyle(Palette.ink)
                .lineLimit(2)
              Spacer(minLength: 8)
              Text(verbatim: Fmt.clock(e.duration))
                .font(.cpMono(13, relativeTo: .footnote))
                .foregroundStyle(Palette.ink3)
            }
            .padding(.horizontal, 14)
            .padding(.vertical, 10)
            .contentShape(.rect)
          }
          .buttonStyle(.plain)
          .accessibilityAddTraits(picked.contains(e.id) ? .isSelected : [])
          if e.id != j.entries.last?.id { RowDivider(leading: 46) }
        }
      }
      .card()
      Button("下载选中的 \(picked.count) 个") {
        model.choose(Array(picked), in: item)
        dismiss()
      }
      .buttonStyle(.primaryWide)
      .disabled(picked.isEmpty)
    }
  }

  // MARK: 下载记录

  private func logSection(_ lines: [MobileLogLine]) -> some View {
    VStack(alignment: .leading, spacing: 6) {
      ForEach(Array(lines.suffix(30).enumerated()), id: \.offset) { _, l in
        HStack(alignment: .firstTextBaseline, spacing: 8) {
          Text(verbatim: Fmt.hm(Date(timeIntervalSince1970: Double(l.at) / 1000), seconds: true))
            .font(.cpMono(11, relativeTo: .caption2))
            .foregroundStyle(Palette.ink4)
          Text(verbatim: l.text)
            .font(.cpMeta)
            .foregroundStyle(l.level == "err" ? Palette.err : l.level == "warn" ? Palette.warn : Palette.ink2)
            .fixedSize(horizontal: false, vertical: true)
        }
      }
    }
    .padding(14)
    .frame(maxWidth: .infinity, alignment: .leading)
    .card()
  }

  // MARK: 会保存为

  private var saveAs: some View {
    HStack(spacing: 12) {
      FileBadge(format: formatTo)
      VStack(alignment: .leading, spacing: 2) {
        Text(j.untitled ? "\(formatTo) · 读到以后再定" : "\(formatTo) · \(quality)")
          .font(.cpSecondaryMedium)
          .foregroundStyle(Palette.ink)
        Text(item.placement == .computer ? "\(model.computerName ?? "电脑") · \(saveTo)" : saveTo)
          .font(.cpMeta)
          .foregroundStyle(Palette.ink3)
      }
      Spacer(minLength: 0)
    }
    .padding(14)
    .card()
  }

  /// 最后存成什么：M3U8 一律 MP4，「MP4 · 1080p」取前一半
  private var formatTo: String {
    if j.isHLS { return "MP4" }
    let f = j.fmt.components(separatedBy: " · ").first ?? ""
    if let arrow = f.range(of: "→") { return f[arrow.upperBound...].trimmingCharacters(in: .whitespaces) }
    return f.replacingOccurrences(of: " 文件", with: "").isEmpty ? "MP4" : f.replacingOccurrences(of: " 文件", with: "")
  }

  private var quality: String {
    if j.isHLS { return j.vars.indices.contains(choice) ? j.vars[choice].p : "最清晰" }
    let parts = j.fmt.components(separatedBy: " · ")
    return parts.count > 1 ? parts[1] : "原画质"
  }

  private func section<Content: View>(_ title: LocalizedStringKey, @ViewBuilder content: () -> Content) -> some View {
    VStack(alignment: .leading, spacing: 8) {
      SectionLabel(title: title)
      content()
    }
  }
}

/// 单选点：选中是强调色实心点（BRIEF：强调色只用在选中的单选点等处）
struct Radio: View {
  var on: Bool
  var body: some View {
    ZStack {
      Circle().strokeBorder(on ? Palette.accent : Palette.ink4, lineWidth: 1.5)
      if on { Circle().fill(Palette.accent).padding(4.5) }
    }
    .frame(width: 20, height: 20)
    .accessibilityHidden(true)
  }
}

/// 文件图标：白色纸片 + 左下角格式小标（桌面端 `FileIcon`）
struct FileBadge: View {
  var format: String

  var body: some View {
    ZStack(alignment: .bottomLeading) {
      RoundedRectangle(cornerRadius: 6, style: .continuous)
        .fill(Palette.card)
        .overlay { RoundedRectangle(cornerRadius: 6, style: .continuous).strokeBorder(Palette.line2, lineWidth: 0.5) }
        .shadow(color: .black.opacity(0.06), radius: 2, y: 1)
        .frame(width: 34, height: 42)
        .padding(.leading, 6)
      Text(format)
        .font(.cpMono(9, relativeTo: .caption2, weight: .monoSemibold))
        .foregroundStyle(.white)
        .padding(.horizontal, 4)
        .padding(.vertical, 2)
        .background(Palette.accent, in: .rect(cornerRadius: 3))
        .padding(.bottom, 5)
    }
    .frame(width: 44, height: 44, alignment: .leading)
    .accessibilityHidden(true)
  }
}
