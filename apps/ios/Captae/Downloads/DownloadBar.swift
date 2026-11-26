// 下载条：标签栏上方常驻的一条，像「音乐」的迷你播放条（BRIEF 2.3）。
// 放在 `.tabViewBottomAccessory` 里：位置、玻璃、往下滚时缩进标签栏那一行都是系统做的，这里不加背景。
// 展开时两行（「正在下 3 个 · 11.2 MB/s」+「Sintel · 完整版短片 45%」，底边 2pt 总进度），
// 缩进标签栏那一行时（`tabViewBottomAccessoryPlacement == .inline`）只剩一行。
// 三种样子：有任务在跑、全部暂停了、没有任务（「粘贴链接下载」+ 系统的 PasteButton）。
// 开始下载的入口一直看得见（2.3 末尾，2026-11-24 补）：右边固定一个圆形「+」，三种样子、收起成一行时都在，点了打开输入卡。
// 「全部暂停 / 全部继续」不放在这里，只在右上角「更多」菜单里。
import SwiftUI

struct DownloadBar: View {
  @Environment(AppModel.self) private var model
  @Environment(\.tabViewBottomAccessoryPlacement) private var placement

  private var inline: Bool { placement == .inline }

  var body: some View {
    HStack(spacing: 8) {
      // 左边：进度或说明，点它去下载页（没有任务时打开输入卡）
      Group {
        if !model.runningItems.isEmpty {
          running
        } else if !model.pausedItems.isEmpty {
          paused
        } else {
          idle
        }
      }
      .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .leading)
      .contentShape(.rect)
      .onTapGesture(perform: open)
      .accessibilityElement(children: .combine)
      .accessibilityAddTraits(.isButton)
      // 没有任务时多一个系统的粘贴按钮：不弹「允许粘贴」提示，点了直接把链接填进输入卡
      if model.runningItems.isEmpty && model.pausedItems.isEmpty && !inline {
        PasteButton(payloadType: String.self) { strings in
          model.composer = ComposerSession(text: strings.first ?? "")
        }
        .labelStyle(.titleOnly)
        .buttonBorderShape(.capsule)
        .controlSize(.small)
        .tint(Palette.ink)
      }
      newButton
    }
    .padding(.leading, inline ? 12 : 10)
    .padding(.trailing, inline ? 4 : 6)
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    // 有任务时底边一条 2pt 的总进度（强调色），展开时才有
    .overlay(alignment: .bottom) {
      if !inline && !model.runningItems.isEmpty {
        ProgressBar(value: model.totalProgress, label: String(localized: "总进度"), height: 2)
          .padding(.leading, 14)
          .padding(.trailing, 16)
          .offset(y: 1)
      }
    }
    // 和系统的标签栏一样，辅助功能大字号时不继续放大（高度是系统定的），读屏照样读全
    .dynamicTypeSize(...DynamicTypeSize.xxLarge)
  }

  /// 新下载：近黑圆形「+」，一直在最右边（内容里的主按钮色，在玻璃条上一眼看得到）
  private var newButton: some View {
    Button {
      model.composer = ComposerSession()
    } label: {
      Image(systemName: "plus")
        .font(.system(size: inline ? 13 : 15, weight: .semibold))
        .foregroundStyle(.white)
        .frame(width: inline ? 28 : 32, height: inline ? 28 : 32)
        .background(Palette.ink, in: .circle)
        .contentShape(.circle)
    }
    // 小圆按钮按下缩到 .95（审查第 15 条）
    .buttonStyle(.circle)
    .accessibilityLabel("新下载")
  }

  /// 点它：有任务时跳到下载页（已经在下载页就打开那一条的详情）；没有任务时打开输入卡
  private func open() {
    guard let lead = model.leadItem ?? model.pausedItems.first else {
      model.composer = ComposerSession()
      return
    }
    if model.tab == .downloads && model.downloadsPath.isEmpty {
      model.detail = lead
    } else {
      model.tab = .downloads
      model.downloadsPath = []
    }
  }

  // MARK: 有任务在跑

  private var running: some View {
    let n = model.runningItems.count
    let speed = Fmt.speed(model.totalSpeed)
    return HStack(spacing: 10) {
      if let lead = model.leadItem {
        Thumb(name: lead.job.thumb, radius: 6, audio: lead.job.audio)
          .frame(width: inline ? 26 : 32, height: inline ? 26 : 32)
      }
      if inline {
        // 缩进标签栏那一行时只有一小段宽度：个数 + 总进度
        Text("正在下 \(n) 个 · \(Fmt.percent(model.totalProgress))")
          .font(.cpMetaMedium)
          .monospacedDigit()
          .foregroundStyle(Palette.ink)
          .lineLimit(1)
          .minimumScaleFactor(0.85)
      } else {
        VStack(alignment: .leading, spacing: 1) {
          Text("正在下 \(n) 个 · \(speed)")
            .font(.cpMetaMedium)
            .foregroundStyle(Palette.ink)
          if let lead = model.leadItem {
            Text("\(lead.job.untitled ? lead.job.siteName : lead.job.title) \(Fmt.percent(lead.job.progress))")
              .font(.cpSmall)
              .foregroundStyle(Palette.ink3)
          }
        }
        .monospacedDigit()
        .lineLimit(1)
      }
    }
  }

  // MARK: 全部暂停了

  private var paused: some View {
    HStack(spacing: 10) {
      Image(systemName: "pause.circle.fill")
        .font(.system(size: 18))
        .foregroundStyle(Palette.ink2)
        .frame(width: 32)
      // 「全部继续」在右上角「更多」菜单里，这里只说状态
      Text("全部暂停了 · \(model.pausedItems.count) 个")
        .font(.cpMetaMedium)
        .foregroundStyle(Palette.ink)
        .lineLimit(1)
    }
  }

  // MARK: 没有任务

  private var idle: some View {
    HStack(spacing: 10) {
      Image(systemName: "link")
        .font(.system(size: 15, weight: .medium))
        .foregroundStyle(Palette.ink2)
        .frame(width: inline ? 22 : 32)
      Text("粘贴链接下载")
        .font(.cpSecondaryMedium)
        .foregroundStyle(Palette.ink)
        .lineLimit(1)
    }
  }
}
