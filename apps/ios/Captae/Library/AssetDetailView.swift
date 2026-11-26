// 素材详情：推入新页。预览（16:9 封面 + 播放按钮）、标题、简介、操作、信息。照桌面端素材库右栏 `pages/library/Detail.tsx`。
// 手机上的素材：分享 / 发到电脑 / 在「文件」中查看 / 删除；电脑上的素材：「存到手机」（主按钮）+ 分享链接，存的时候按钮变成进度。
// 画面上不放玻璃：播放按钮是半透明黑底圆，不是玻璃。接真数据时预览换成系统播放器（AVKit），控件交给系统。
// 顶部的光（BRIEF 2.6）：预览整宽贴边，`backgroundExtensionEffect()` 把封面延伸、模糊到导航栏和状态栏下面；声音没有封面时不加。
import AVKit
import SwiftUI

struct AssetDetailView: View {
  var asset: Asset
  var source: LibrarySource

  @Environment(AppModel.self) private var model
  @Environment(\.openURL) private var openURL
  /// 点了播放：封面换成系统播放器（画面上不放自定义玻璃，控件交给系统）
  @State private var player: AVPlayer?
  @State private var confirmDelete = false

  private var saving: SavingState? { model.saving[asset.id] }
  /// 本机文件还在（演示数据里没有真文件）
  private var playable: Bool { source == .phone && asset.kind != .image && FileManager.default.fileExists(atPath: asset.path) }

  var body: some View {
    // 预览固定在顶部，作为滚动视图的顶栏（`safeAreaBar`）：贴着导航栏，`backgroundExtensionEffect()` 才能把它
    // 延伸到导航栏和状态栏下面（2.6）；下面的内容从它下面滚过时，系统按顶栏给柔和的渐隐边缘，不切出硬线（2.7）
    ScrollView {
      VStack(alignment: .leading, spacing: 18) {
        VStack(alignment: .leading, spacing: 6) {
          Text(asset.title)
            .font(Typo.font(.semibold, 20, relativeTo: .title3))
            .foregroundStyle(Palette.ink)
          if !asset.desc.isEmpty {
            Text(asset.desc)
              .font(.cpSecondary)
              .foregroundStyle(Palette.ink2)
              .fixedSize(horizontal: false, vertical: true)
          }
        }
        if source == .phone {
          phoneActions
        } else {
          remoteActions
        }
        VStack(alignment: .leading, spacing: 8) {
          SectionLabel(title: "信息")
          info
        }
      }
      .padding(.horizontal, Spacing.page)
      .padding(.top, 16)
      .padding(.bottom, 28)
    }
    // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
    .scrollEdgeEffectStyle(.soft, for: .all)
    .safeAreaBar(edge: .top, spacing: 0) { preview }
    .pageBackground()
    .navigationTitle("")
    .navigationBarTitleDisplayMode(.inline)
    .confirmationDialog("从这台手机上删掉？", isPresented: $confirmDelete, titleVisibility: .visible) {
      Button("删除", role: .destructive) { model.deleteAsset(asset) }
    } message: {
      Text("文件会从 Captae 里删掉。已经存进相册的不受影响")
    }
    .onDisappear { player?.pause() }
  }

  // MARK: 预览

  @ViewBuilder
  private var preview: some View {
    if let player {
      VideoPlayer(player: player)
        .aspectRatio(16 / 9, contentMode: .fit)
        .frame(maxWidth: .infinity)
        .background(.black)
    } else {
      Thumb(name: asset.thumb, duration: asset.dur, radius: 0, audio: asset.kind == .audio)
        .aspectRatio(16 / 9, contentMode: .fit)
        .frame(maxWidth: .infinity)
        .backgroundExtensionEffect(isEnabled: asset.kind != .audio && asset.thumb != nil)
        .overlay {
          if asset.kind != .image {
            Button {
              guard playable else { return }
              let p = AVPlayer(url: URL(filePath: asset.path))
              player = p
              p.play()
            } label: {
              Image(systemName: "play.fill")
                .font(.system(size: 20, weight: .semibold))
                .foregroundStyle(.white)
                .frame(width: 54, height: 54)
                .background(Color.black.opacity(0.45), in: .circle)
                .contentShape(.circle)
            }
            .buttonStyle(.circle)
            .accessibilityLabel("播放")
          }
        }
    }
  }

  // MARK: 操作

  private var phoneActions: some View {
    HStack(spacing: 8) {
      if playable || FileManager.default.fileExists(atPath: asset.path) {
        ShareLink(item: URL(filePath: asset.path)) {
          ActionTileLabel(title: "分享", icon: "square.and.arrow.up")
        }
        .buttonStyle(.tile)
      } else {
        ActionTile(title: "分享", icon: "square.and.arrow.up") {}
      }
      ActionTile(title: "发到电脑", icon: "laptopcomputer.and.arrow.down") { model.openPairing() }
      ActionTile(title: "在「文件」中查看", icon: "folder") {
        if let url = model.saver.filesURL(asset.id) { openURL(url) }
      }
      .disabled(model.saver.filesURL(asset.id) == nil && !model.isDemo)
      ActionTile(title: "删除", icon: "trash", tint: Palette.err) { confirmDelete = true }
    }
  }

  @ViewBuilder
  private var remoteActions: some View {
    VStack(spacing: 10) {
      if let p = saving {
        SavingProgress(progress: p, total: Double(asset.bytes) / 1_000_000) { model.cancelSave(asset.id) }
      } else if model.isSavedToPhone(asset.id) {
        // 7 天内存到过手机（RemoteHub 的记录）：再存一次的按钮留着，但先说清楚已经存过
        Label("已存到手机", systemImage: "checkmark.circle.fill")
          .font(.cpSecondaryMedium)
          .foregroundStyle(Palette.ok)
          .frame(maxWidth: .infinity)
          .padding(.vertical, 12)
          .well(radius: Radius.lg)
      } else {
        Button {
          if model.isDemo {
            model.saving[asset.id] = SavingState(fraction: 0, speed: 0)
          } else {
            model.saveToPhone(asset)
          }
        } label: {
          Label("存到手机", systemImage: "arrow.down.to.line")
        }
        .buttonStyle(.primaryWide)
        .disabled(model.computer?.online == false)
      }
      if let link = URL(string: asset.page.isEmpty ? asset.url : asset.page), !link.absoluteString.isEmpty {
        ShareLink(item: link) {
          Label("分享链接", systemImage: "link")
            .frame(maxWidth: .infinity)
        }
        .buttonStyle(.secondaryLarge)
      }
    }
  }

  // MARK: 信息

  private var info: some View {
    VStack(spacing: 0) {
      let rows = infoRows
      ForEach(Array(rows.enumerated()), id: \.offset) { i, row in
        InfoRow(label: row.0, value: row.1, mono: row.2, link: row.2)
        if i < rows.count - 1 { RowDivider() }
      }
    }
    .padding(.horizontal, 14)
    .card()
  }

  private var infoRows: [(String, String, Bool)] {
    var rows: [(String, String, Bool)] = []
    if !asset.page.isEmpty { rows.append((String(localized: "来源"), bare(asset.page), true)) }
    rows.append((String(localized: "格式"), asset.fmt, false))
    if !asset.res.isEmpty { rows.append((String(localized: "分辨率"), asset.res, false)) }
    if !asset.dur.isEmpty { rows.append((String(localized: "时长"), asset.dur, false)) }
    rows.append((String(localized: "大小"), asset.size, false))
    rows.append((String(localized: "下载于"), asset.createdAt > 0 ? Fmt.whenFull(asset.createdAt) : asset.when, false))
    rows.append((String(localized: "位置"), source == .computer ? String(localized: "\(model.computerName ?? String(localized: "电脑")) · 影片 › Captae") : place, false))
    return rows
  }

  /// 手机上的素材存到了哪：相册 /「文件」› Captae（还没存好时是 Captae 里）
  private var place: String {
    switch model.saver.places[asset.id]?.kind {
    case .photos: String(localized: "相册")
    case .files: String(localized: "「文件」› Captae")
    case nil: model.isDemo ? asset.path : String(localized: "Captae 里")
    }
  }

  /// 去掉协议和 www.，显示用
  private func bare(_ u: String) -> String {
    var s = u
    for p in ["https://", "http://"] where s.hasPrefix(p) { s.removeFirst(p.count) }
    if s.hasPrefix("www.") { s.removeFirst(4) }
    if s.hasSuffix("/") { s.removeLast() }
    return s
  }
}

/// 一个操作：图标在上、字在下（白卡）
private struct ActionTile: View {
  var title: LocalizedStringKey
  var icon: String
  var tint: Color = Palette.ink
  var action: () -> Void

  var body: some View {
    Button(action: action) {
      ActionTileLabel(title: title, icon: icon, tint: tint)
    }
    .buttonStyle(.tile)
  }
}

/// 操作方块的样子（ShareLink 也用它）
private struct ActionTileLabel: View {
  var title: LocalizedStringKey
  var icon: String
  var tint: Color = Palette.ink
  @Environment(\.isEnabled) private var enabled

  var body: some View {
    VStack(spacing: 6) {
      Image(systemName: icon)
        .font(.system(size: 18, weight: .regular))
        .foregroundStyle(tint)
        .frame(height: 22)
      Text(title)
        .font(.cpSmall)
        .foregroundStyle(Palette.ink2)
        .multilineTextAlignment(.center)
        .lineLimit(2)
        .fixedSize(horizontal: false, vertical: true)
    }
    .frame(maxWidth: .infinity, minHeight: 74)
    .padding(.horizontal, 4)
    .card()
    .contentShape(.rect)
    .opacity(enabled ? 1 : 0.45)
  }
}

/// 操作方块按下：缩到 .98（大块卡片，MOTION 1.2）
private struct TileButtonStyle: ButtonStyle {
  func makeBody(configuration: Configuration) -> some View {
    configuration.label.modifier(PressScale(pressed: configuration.isPressed, scale: 0.98))
  }
}

extension ButtonStyle where Self == TileButtonStyle {
  fileprivate static var tile: TileButtonStyle { TileButtonStyle() }
}

/// 正在存到手机：主按钮的位置变成进度
private struct SavingProgress: View {
  var progress: SavingState
  /// MB
  var total: Double
  var cancel: () -> Void = {}

  private var remaining: String {
    progress.speed > 0 ? " · 剩余 \(Fmt.dur(total * (1 - progress.fraction) / progress.speed))" : ""
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 9) {
      HStack(alignment: .firstTextBaseline) {
        Text("正在存到手机")
          .font(.cpBodyMedium)
          .foregroundStyle(Palette.ink)
        Spacer(minLength: 8)
        Text(Fmt.percent(progress.fraction))
          .font(.cpBodyMedium)
          .monospacedDigit()
          .foregroundStyle(Palette.ink)
      }
      ProgressBar(value: progress.fraction, label: "存到手机的进度")
      HStack {
        Text("\(Fmt.mb(total * progress.fraction)) / \(Fmt.mb(total))\(remaining)")
          .font(.cpMeta)
          .monospacedDigit()
          .foregroundStyle(Palette.ink3)
        Spacer(minLength: 8)
        Button("取消", action: cancel)
          .buttonStyle(.link)
      }
    }
    .padding(14)
    .well(radius: Radius.lg)
  }
}
