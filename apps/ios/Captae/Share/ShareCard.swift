// 分享卡片：在别的 App 里点分享、选 Captae 时出现的小卡（计划 1 第 4 条，BRIEF 08）。
// 同一个视图两处用：分享扩展（CaptaeShare/ShareRoot.swift，真数据）和 App 里的演示截图（DemoRoute 08-*）。
// 不依赖 AppModel（扩展里没有），要的东西都从参数进来。
// 读取中：标题位置是扫光文字；读到了：标题、封面小图、网站，「电脑 / 手机」（系统分段控件）、清晰度、「下载」主按钮；
// 电脑不在线：先存着，回到同一个 Wi-Fi 后自动发过去。上架版遇到 YouTube 这类网站：只能交给电脑，说清楚。
import SwiftUI

/// 卡片上显示的那条链接
struct ShareCardContent: Equatable {
  var url: String
  var title: String?
  var thumb: String?
  /// 「MP4 文件」「M3U8 → MP4」这类说明（读到以后才有）
  var fmt: String = ""
  var dur: String = ""
  /// 正在读取视频信息
  var reading = true
  /// 上架版：这个网站要在电脑上下（YouTube 这类），网站名
  var computerOnlySite: String?
  /// 读取失败的说法（和桌面端同一套）
  var failure: String?
}

/// 用户点了「下载 / 先存着」
struct ShareChoice {
  enum Target { case computer, phone }
  var target: Target
  /// 清晰度（协议值；nil = 最清晰）
  var quality: String?
}

struct ShareCard: View {
  var content: ShareCardContent
  /// 配对过的电脑（没配对时没有「电脑 / 手机」选择）
  var computerName: String?
  var computerOnline = false
  /// 发到电脑、现在发不了时说什么（为空时用「电脑现在连不上」那句；分享扩展里是「打开 Captae 以后发过去」）
  var holdNote: String? = nil
  var onCancel: () -> Void
  var onConfirm: (ShareChoice) -> Void

  @State private var target: ShareChoice.Target = .computer
  @State private var quality = Quality.best
  /// 卡片内容的高度：sheet 正好包住它（分享卡片是小卡，不留大片空白）
  @State private var height: CGFloat = 360

  enum Quality: String, CaseIterable, Identifiable {
    case best = "最清晰"
    case p1080 = "1080p"
    case p720 = "720p"
    var id: String { rawValue }
    var value: String? { self == .best ? nil : rawValue }
    var title: LocalizedStringKey { LocalizedStringKey(rawValue) }
  }

  private var reading: Bool { content.reading }
  /// 这条最后去哪：只能在电脑上下的网站一律电脑；没配对电脑时只有手机
  private var effectiveTarget: ShareChoice.Target {
    if content.computerOnlySite != nil { return .computer }
    return computerName == nil ? .phone : target
  }
  /// 现在发不过去，先存着（电脑不在线、没配对、还没接远程控制）
  private var holding: Bool { effectiveTarget == .computer && !computerOnline }

  var body: some View {
    NavigationStack {
      VStack(alignment: .leading, spacing: 16) {
        header
        if let site = content.computerOnlySite {
          StatusRow(
            tone: .info, icon: "laptopcomputer", title: cjk("", site, String(localized: "的视频在电脑上下")),
            hint: computerName == nil
              ? String(localized: "先存着。在 Captae 的「管理电脑」里配对一台电脑以后发过去")
              : holdNote ?? String(localized: "先存着，回到同一个 Wi-Fi 后自动发过去")) {}
        } else if let computer = computerName {
          Picker("下载到哪里", selection: $target) {
            Text("电脑").tag(ShareChoice.Target.computer)
            Text("手机").tag(ShareChoice.Target.phone)
          }
          .pickerStyle(.segmented)
          .disabled(reading)

          if holding {
            if let holdNote {
              StatusRow(tone: .plain, icon: "laptopcomputer", title: holdNote, hint: nil) {}
            } else {
              StatusRow(
                tone: .plain, icon: "wifi.slash", title: cjk("", computer, String(localized: "现在连不上")),
                hint: String(localized: "先存着，回到同一个 Wi-Fi 后自动发过去")) {}
            }
          }
        }
        if let failure = content.failure {
          StatusRow(tone: .err, icon: "exclamationmark.circle.fill", title: failure, hint: nil) {}
        } else {
          qualityRow
        }
        Button {
          onConfirm(ShareChoice(target: effectiveTarget, quality: quality.value))
        } label: {
          Text(holding ? "先存着" : "下载").frame(maxWidth: .infinity)
        }
        .buttonStyle(.primaryWide)
        .disabled(reading || content.failure != nil)
        .padding(.top, 6)
        if effectiveTarget == .phone && !reading && content.failure == nil {
          Text("打开 Captae 就开始下")
            .font(.cpMeta)
            .foregroundStyle(Palette.ink3)
            .frame(maxWidth: .infinity)
        }
      }
      .padding(.horizontal, Spacing.page + 4)
      .padding(.top, 6)
      .padding(.bottom, 12)
      .fixedSize(horizontal: false, vertical: true)
      .onGeometryChange(for: CGFloat.self) { $0.size.height } action: { height = $0 }
      .frame(maxHeight: .infinity, alignment: .top)
      .animation(Motion.swap, value: reading)
      .navigationTitle("Captae")
      .navigationBarTitleDisplayMode(.inline)
      .toolbar {
        ToolbarItem(placement: .cancellationAction) {
          Button(role: .cancel, action: onCancel)
        }
      }
    }
    // 滚动边缘：柔和的渐隐模糊（BRIEF 2.7）。这张卡现在不滚动，加在根上，以后有滚动的内容也照这个
    .scrollEdgeEffectStyle(.soft, for: .all)
    // 内容高度 + 导航栏
    .presentationDetents([.height(height + 72)])
    .presentationDragIndicator(.hidden)
  }

  // MARK: 链接是什么

  private var header: some View {
    HStack(alignment: .top, spacing: 12) {
      Thumb(name: reading ? nil : content.thumb, duration: reading ? "" : content.dur, radius: Radius.md)
        .frame(width: 96, height: 54)
      VStack(alignment: .leading, spacing: 5) {
        if reading {
          ShimmerText(text: "正在读取视频信息", font: .cpSecondaryMedium)
        } else {
          Text(verbatim: content.title ?? Site.host(content.url))
            .font(.cpSecondaryMedium)
            .foregroundStyle(Palette.ink)
            .lineLimit(2)
        }
        HStack(spacing: 5) {
          SiteIcon(plat: nil, url: content.url, size: 12)
          Text(verbatim: reading || content.fmt.isEmpty ? Site.host(content.url) : "\(Site.host(content.url)) · \(content.fmt)")
            .font(.cpMeta)
            .foregroundStyle(Palette.ink3)
            .lineLimit(1)
        }
      }
      .frame(maxWidth: .infinity, alignment: .leading)
    }
    .padding(12)
    .card()
    .accessibilityElement(children: .combine)
  }

  private var qualityRow: some View {
    HStack {
      Text("清晰度")
        .font(.cpSecondary)
        .foregroundStyle(Palette.ink)
      Spacer(minLength: 8)
      Menu {
        Picker("清晰度", selection: $quality) {
          ForEach(Quality.allCases) { Text($0.title).tag($0) }
        }
      } label: {
        HStack(spacing: 4) {
          if reading {
            Text("读到以后再定")
          } else {
            Text(quality.title)
          }
          Image(systemName: "chevron.up.chevron.down").font(.system(size: 10, weight: .semibold))
        }
        .font(.cpSecondary)
        .foregroundStyle(Palette.ink2)
      }
      .disabled(reading)
    }
    .padding(.horizontal, 14)
    .padding(.vertical, 12)
    .card()
  }
}
