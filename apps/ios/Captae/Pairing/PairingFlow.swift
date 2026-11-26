// 配对：扫码是全屏弹出（相机画面）；扫不了就输电脑上显示的 6 位数（计划 3.1、5.3）。
// 输入配对码不在这里：它在「管理电脑」sheet 里推进去，导航标题是电脑名（BRIEF 2.8 第 6 条），见 `PairCodeView`。
// 扫码用 VisionKit 的 DataScannerViewController（只认二维码）；用不了相机的设备（模拟器、没给相机权限）用「从相册选二维码图片」，
// 用 CoreImage 认图里的二维码。扫到的内容先交给 RemoteHub.parsePairLink，是 Captae 的二维码就问一句「要连这台电脑吗」，
// 确认以后 pairQr（AppModel+Remote.swift）。画面上不放玻璃，关闭按钮在系统工具栏里（玻璃由系统画）。
import CoreImage
import PhotosUI
import SwiftUI
import VisionKit

struct PairingFlow: View {
  /// 点了「输入 6 位数配对」：由弹出它的地方关掉扫码、推进输入配对码
  var useCode: () -> Void
  @Environment(AppModel.self) private var model
  @Environment(\.dismiss) private var dismiss

  var body: some View {
    NavigationStack {
      ScanStep(useCode: useCode)
        .toolbar {
          ToolbarItem(placement: .cancellationAction) {
            Button(role: .close) { dismiss() }
          }
        }
    }
    // 相机画面是深色的：状态栏、关闭按钮跟着用深色外观
    .preferredColorScheme(.dark)
    // 扫到配对二维码：先问一句连哪台（parsePairLink 不联网）
    .alert(
      Text(verbatim: cjk(String(localized: "连接"), model.pairLink?.name ?? "") + String(localized: "？")),
      isPresented: Binding(get: { model.pairLink != nil }, set: { if !$0 { model.pairLink = nil } }),
      presenting: model.pairLink
    ) { _ in
      Button("连接") { Task { await model.pairScanned() } }
      Button("取消", role: .cancel) { model.pairLinkText = nil }
    } message: { info in
      Text(info.known ? "这台电脑配对过，会换一个新的令牌" : "配好以后，链接可以直接发给这台电脑下，也能看它的素材库")
    }
    // 配上了：关掉扫码（AppModel.paired 会刷新电脑列表）
    .onChange(of: model.computers.map(\.id)) { old, new in
      if new.count > old.count || Set(new) != Set(old) { dismiss() }
    }
  }
}

// MARK: 扫码

private struct ScanStep: View {
  var useCode: () -> Void
  @Environment(AppModel.self) private var model
  @State private var photo: PhotosPickerItem?

  /// 这台设备能用相机扫码（模拟器不能；没给相机权限时 isAvailable 是 false）
  private var canScan: Bool { DataScannerViewController.isSupported && DataScannerViewController.isAvailable }

  var body: some View {
    ZStack {
      if canScan && !model.isDemo {
        QRScanner { model.scanned($0) }
          .ignoresSafeArea()
      } else {
        Palette.camera.ignoresSafeArea()
      }
      VStack(spacing: 0) {
        Spacer(minLength: 0)
        ViewfinderCorners(arm: 0.2, radius: 22)
          .stroke(.white, style: StrokeStyle(lineWidth: 4, lineCap: .round, lineJoin: .round))
          .frame(width: 250, height: 250)
          .accessibilityHidden(true)
        Text(canScan || model.isDemo ? "对准电脑上 Captae「设置 › 手机」里的二维码" : "这台设备用不了相机扫码。可以把电脑上的二维码截图，从相册里选")
          .font(.cpSecondary)
          .foregroundStyle(.white)
          .multilineTextAlignment(.center)
          .padding(.horizontal, 40)
          .padding(.top, 28)
        if model.pairingBusy {
          ProgressView().tint(.white).padding(.top, 16)
        }
        Spacer(minLength: 0)
        HStack(spacing: 10) {
          PhotosPicker(selection: $photo, matching: .images) {
            Label("从相册选", systemImage: "photo")
          }
          .buttonStyle(.secondaryLarge)
          Button("输入 6 位数配对", action: useCode)
            .buttonStyle(.secondaryLarge)
        }
        .padding(.bottom, 24)
      }
    }
    .navigationTitle("扫码配对")
    .navigationBarTitleDisplayMode(.inline)
    .onChange(of: photo) { _, item in
      guard let item else { return }
      Task {
        let data = try? await item.loadTransferable(type: Data.self)
        photo = nil
        if let text = data.flatMap(QRReader.read) {
          model.scanned(text)
        } else {
          model.notice = AppNotice(title: String(localized: "图里没认出二维码"), text: String(localized: "换一张清楚一点的截图，二维码要完整"))
        }
      }
    }
  }
}

/// 认图片里的二维码（相册里选的截图）
nonisolated enum QRReader {
  static func read(_ data: Data) -> String? {
    guard let image = CIImage(data: data),
      let detector = CIDetector(ofType: CIDetectorTypeQRCode, context: nil, options: [CIDetectorAccuracy: CIDetectorAccuracyHigh])
    else { return nil }
    return detector.features(in: image).compactMap { ($0 as? CIQRCodeFeature)?.messageString }.first
  }
}

/// 相机扫码（VisionKit）：认到一个二维码就交出来，同一个内容只交一次
private struct QRScanner: UIViewControllerRepresentable {
  var onFound: (String) -> Void

  func makeCoordinator() -> Coordinator { Coordinator(onFound: onFound) }

  func makeUIViewController(context: Context) -> DataScannerViewController {
    let vc = DataScannerViewController(
      recognizedDataTypes: [.barcode(symbologies: [.qr])], qualityLevel: .balanced, isHighlightingEnabled: true)
    vc.delegate = context.coordinator
    try? vc.startScanning()
    return vc
  }

  func updateUIViewController(_ vc: DataScannerViewController, context: Context) {}

  static func dismantleUIViewController(_ vc: DataScannerViewController, coordinator: Coordinator) {
    vc.stopScanning()
  }

  final class Coordinator: NSObject, DataScannerViewControllerDelegate {
    let onFound: (String) -> Void
    private var last: String?

    init(onFound: @escaping (String) -> Void) { self.onFound = onFound }

    func dataScanner(_ dataScanner: DataScannerViewController, didAdd addedItems: [RecognizedItem], allItems: [RecognizedItem]) {
      for case let .barcode(code) in addedItems {
        guard let text = code.payloadStringValue, text != last else { continue }
        last = text
        onFound(text)
      }
    }
  }
}

// MARK: 输 6 位数

/// 输入配对码：「管理电脑」sheet 里推进去的一页（BRIEF 2.8 第 6 条）。电脑图标、「输入配对码」、
/// 「在电脑上看到的 6 位数」、6 个格子、下面一行说明；配对码过期时换成状态行 +「重新扫码」
struct PairCodeView: View {
  var expired: Bool
  var rescan: () -> Void

  @Environment(AppModel.self) private var model
  @Environment(\.accessibilityReduceMotion) private var reduce
  @State private var code: String
  /// 配不上的原因（远程控制还没接上、配对码不对……）
  @State private var failure: String?
  @FocusState private var focused: Bool

  init(code: String, expired: Bool, rescan: @escaping () -> Void) {
    _code = State(initialValue: code)
    self.expired = expired
    self.rescan = rescan
  }

  var body: some View {
    ScrollView {
      VStack(spacing: 0) {
        DeviceIcon(size: 56)
          .padding(.bottom, 16)
        Text("输入配对码")
          .font(.cpHero)
          .foregroundStyle(Palette.ink)
        Text("在电脑上看到的 6 位数")
          .font(.cpSecondary)
          .foregroundStyle(Palette.ink3)
          .padding(.top, 6)

        digits
          .padding(.top, 28)

        if expired {
          StatusRow(tone: .err, icon: "exclamationmark.circle.fill", title: String(localized: "配对码过期了，在电脑上重新打开二维码"), hint: String(localized: "配对码 5 分钟有效，用一次就作废")) {
            Button("重新扫码", action: rescan)
              .buttonStyle(.secondary)
              .fixedSize()
          }
          .padding(.top, 22)
        } else if let failure {
          StatusRow(tone: .err, icon: "exclamationmark.circle.fill", title: failure, hint: nil) {}
            .padding(.top, 22)
            .transition(.statusSwap(reduce: reduce))
        } else {
          Text("在电脑上打开 Captae › 设置 › 手机，打开「让手机连接」就能看到")
            .font(.cpMeta)
            .foregroundStyle(Palette.ink3)
            .multilineTextAlignment(.center)
            .padding(.top, 20)
        }
      }
      .padding(.horizontal, Spacing.page + 8)
      .padding(.top, 40)
    }
    // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
    .scrollEdgeEffectStyle(.soft, for: .all)
    .pageBackground()
    // 导航标题是要配的那台电脑（同一个 Wi-Fi 里发现到的）
    .navigationTitle((model.pairTarget ?? model.nearby.first)?.name ?? String(localized: "输入配对码"))
    .navigationBarTitleDisplayMode(.inline)
    .onAppear { if !model.demo { focused = true } }
  }

  private func pair(_ code: String) {
    Task {
      if let why = await model.pairCode(code) {
        withAnimation(Motion.swap) { failure = why }
      }
    }
  }

  /// 6 个格子，背后是一个看不见的数字输入框
  private var digits: some View {
    let chars = Array(code.prefix(6))
    return ZStack {
      TextField("配对码", text: $code)
        .keyboardType(.numberPad)
        .textContentType(.oneTimeCode)
        .focused($focused)
        .foregroundStyle(.clear)
        .tint(.clear)
        .frame(width: 1, height: 1)
        .opacity(0.01)
        .onChange(of: code) { _, v in
          let only = String(v.filter(\.isNumber).prefix(6))
          if only != v { code = only }
          if only.count < 6 { failure = nil }
          if only.count == 6 && !model.isDemo { pair(only) }
        }
      HStack(spacing: 8) {
        ForEach(0..<6, id: \.self) { i in
          let filled = i < chars.count
          let current = i == chars.count && !expired
          Text(filled ? String(chars[i]) : "")
            .font(.cpMono(26, relativeTo: .title2, weight: .monoMedium))
            .foregroundStyle(expired ? Palette.ink3 : Palette.ink)
            .frame(maxWidth: 48)
            .frame(height: 58)
            .background {
              RoundedRectangle(cornerRadius: Radius.md, style: .continuous)
                .fill(Palette.card)
                .overlay {
                  RoundedRectangle(cornerRadius: Radius.md, style: .continuous)
                    .strokeBorder(current ? Palette.accent : Color.black.opacity(0.1), lineWidth: current ? 1.5 : 0.5)
                }
            }
        }
      }
      .allowsHitTesting(false)
    }
    .contentShape(.rect)
    .onTapGesture { focused = true }
    .accessibilityElement()
    .accessibilityLabel("配对码")
    .accessibilityValue(code.isEmpty ? "还没输入" : code.map(String.init).joined(separator: " "))
  }
}
