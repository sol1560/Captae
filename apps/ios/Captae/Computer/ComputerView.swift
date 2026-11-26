// 「管理电脑」sheet（第二版把「电脑」标签挪到这里，BRIEF 2.1）：从标题的设备菜单和设置里进。
// 没配对时说明怎么配（插画是桌面端设置窗口的一块截图）+「扫码配对」+ 附近的电脑；
// 配对过时照 BRIEF 2.8 第 5 条（和 Android 同一个结构）：分组「已配对的电脑」（每台一行，点进去看详情、撤销），
// 「等电脑上线」（每条一行：标题 + 链接）+ 页脚 +「都改成在这台手机上下」，「添加电脑」，最后一行说这台手机在电脑上叫什么。
// 输入配对码、电脑详情都在这张 sheet 里推进去（2.8 第 6 条）。关闭按钮在左上角系统的取消位置（`.cancellationAction`）。
import SwiftUI

struct ManageComputersSheet: View {
  @Environment(AppModel.self) private var model
  @Environment(\.dismiss) private var dismiss
  /// 在这张 sheet 上面弹扫码（根视图上面已经有这张 sheet，不能再从根视图弹）
  @State private var scanning = false

  var body: some View {
    @Bindable var model = model
    NavigationStack(path: $model.managePath) {
      ComputerView(scanning: $scanning)
        .navigationTitle("管理电脑")
        .navigationBarTitleDisplayMode(.inline)
        .toolbar {
          ToolbarItem(placement: .cancellationAction) {
            Button(role: .close) { dismiss() }
          }
        }
        .navigationDestination(for: ManageRoute.self) { route in
          switch route {
          case let .computer(id):
            if let c = model.computers.first(where: { $0.id == id }) { ComputerDetailView(computer: c) }
          case .code(let code, let expired):
            PairCodeView(code: code, expired: expired) {
              // 重新扫码：回到「管理电脑」，弹出扫码
              model.managePath = []
              scanning = true
            }
          }
        }
    }
    .fullScreenCover(isPresented: $scanning) {
      // 扫不了：关掉扫码，在这张 sheet 里推进输入配对码
      PairingFlow {
        scanning = false
        model.managePath = [.code()]
      }
    }
  }
}

struct ComputerView: View {
  @Environment(AppModel.self) private var model
  /// 弹扫码：由「管理电脑」sheet 那一层弹
  @Binding var scanning: Bool

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 14) {
        if model.computers.isEmpty {
          unpaired
        } else {
          paired
        }
      }
      .padding(.horizontal, Spacing.page)
      .padding(.top, 4)
      .padding(.bottom, 24)
    }
    // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
    .scrollEdgeEffectStyle(.soft, for: .all)
    .pageBackground()
  }

  // MARK: 没配对

  private var unpaired: some View {
    VStack(alignment: .leading, spacing: 14) {
      VStack(alignment: .leading, spacing: 16) {
        Image("pair-settings")
          .resizable()
          .scaledToFit()
          .clipShape(.rect(cornerRadius: Radius.md, style: .continuous))
          .overlay {
            RoundedRectangle(cornerRadius: Radius.md, style: .continuous).strokeBorder(Palette.line, lineWidth: 0.5)
          }
          .accessibilityHidden(true)
        VStack(alignment: .leading, spacing: 4) {
          Text("和电脑配对")
            .font(.cpTitle)
            .foregroundStyle(Palette.ink)
          Text("配好以后，手机上的链接可以交给电脑下，也能看电脑上的素材库")
            .font(.cpMeta)
            .foregroundStyle(Palette.ink3)
            .fixedSize(horizontal: false, vertical: true)
        }
        VStack(alignment: .leading, spacing: 12) {
          StepLine(n: 1, text: "在电脑上打开 Captae › 设置 › 手机，打开「让手机连接」")
          StepLine(n: 2, text: "用这台手机扫电脑上的二维码")
          StepLine(n: 3, text: "好了。以后打开 Captae 会自动连上")
        }
        Button("扫码配对", systemImage: "qrcode.viewfinder") {
          scanning = true
        }
        .buttonStyle(.primaryWide)
        .padding(.top, 2)
      }
      .padding(14)
      .card()

      nearbySection
    }
  }

  // MARK: 附近的电脑（没配对、配对过都显示：可以再加一台）

  @ViewBuilder
  private var nearbySection: some View {
    if !model.nearby.isEmpty {
      SectionLabel(title: "附近的电脑", detail: String(localized: "发现到 \(model.nearby.count) 台"))
        .padding(.top, 8)
      VStack(spacing: 0) {
        ForEach(model.nearby) { c in
          NavigationLink(value: ManageRoute.code()) {
            HStack(spacing: 12) {
              DeviceIcon()
              VStack(alignment: .leading, spacing: 2) {
                Text(c.name).font(.cpSecondaryMedium).foregroundStyle(Palette.ink)
                Text("在同一个 Wi-Fi 里 · 输入电脑上的 6 位数配对").font(.cpMeta).foregroundStyle(Palette.ink3)
              }
              Spacer(minLength: 0)
              Chevron()
            }
            .padding(12)
            .contentShape(.rect)
          }
          .buttonStyle(.plain)
          // 点了哪一台，输 6 位数时就配它
          .simultaneousGesture(TapGesture().onEnded { model.pairTarget = c })
        }
      }
      .card()
    }
    #if DEBUG
      // 调试入口：测试版电脑端只听 127.0.0.1，NWBrowser 发现到的是局域网地址、连不上。
      // 这里往 RemoteHub.discovered 交一个 127.0.0.1 的假条目，好在界面上走 6 位数配对。只在 Debug 包里有
      Button("调试：加一台 127.0.0.1 的测试电脑") { model.addDebugPeer() }
        .font(.cpMeta)
        .foregroundStyle(Palette.accentInk)
        .padding(.top, 8)
    #endif
  }

  // MARK: 配对过

  /// 上架版只改得动直链和 M3U8 的那几条：YouTube 这类网站只能在电脑上下，留着等电脑（计划 2）
  private var movable: [DownloadItem] {
    model.appStoreBuild ? model.waiting.filter { !Site.computerOnly($0.job.siteName) } : model.waiting
  }

  private var paired: some View {
    VStack(alignment: .leading, spacing: 8) {
      SectionLabel(title: "已配对的电脑")
      ForEach(model.computers) { c in
      NavigationLink(value: ManageRoute.computer(c.id)) {
        HStack(spacing: 12) {
          DeviceIcon(size: 46)
          VStack(alignment: .leading, spacing: 3) {
            Text(verbatim: c.name).font(.cpBodyMedium).foregroundStyle(Palette.ink)
            ComputerStatusLine(computer: c)
            if !c.online && !c.needsPairing {
              // 离线时那台电脑下面一行：怎么让它回来
              Text("回到同一个 Wi-Fi，打开电脑上的 Captae")
                .font(.cpMeta)
                .foregroundStyle(Palette.ink3)
                .fixedSize(horizontal: false, vertical: true)
            }
          }
          Spacer(minLength: 0)
          Chevron()
        }
        .padding(14)
        .contentShape(.rect)
      }
      .buttonStyle(.plain)
      .card()
      }

      if !model.waiting.isEmpty {
        SectionLabel(title: "等电脑上线", detail: String(localized: "\(model.waiting.count) 个"))
          .padding(.top, 14)
        VStack(spacing: 0) {
          ForEach(Array(model.waiting.enumerated()), id: \.element.id) { i, item in
            WaitingRow(item: item)
            if i < model.waiting.count - 1 { RowDivider(leading: 50) }
          }
        }
        .card()
        Text(waitingFooter)
          .font(.cpMeta)
          .foregroundStyle(Palette.ink3)
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, 4)
          .padding(.top, 2)
        if !movable.isEmpty {
          Button {
            if model.isDemo { return }
            model.moveWaitingToPhone()
          } label: {
            Label(movable.count == model.waiting.count ? "都改成在这台手机上下" : "这 \(movable.count) 个改成在这台手机上下", systemImage: "iphone")
              .frame(maxWidth: .infinity)
          }
          .buttonStyle(.secondaryLarge)
          .padding(.top, 6)
        }
      }

      nearbySection
        .padding(.top, 6)

      Button {
        scanning = true
      } label: {
        Label("添加电脑", systemImage: "plus")
          .font(.cpSecondaryMedium)
          .foregroundStyle(Palette.accentInk)
          .frame(maxWidth: .infinity, alignment: .leading)
          .padding(14)
          .contentShape(.rect)
      }
      .buttonStyle(.plain)
      .card()
      .padding(.top, 14)

      Text("在电脑上，这台手机叫「\(model.phoneName)」，配对过的手机可以在电脑的「设置 › 手机」里撤销")
        .font(.cpMeta)
        .foregroundStyle(Palette.ink3)
        .fixedSize(horizontal: false, vertical: true)
        .padding(.horizontal, 4)
        .padding(.top, 2)
    }
  }

  /// 「等电脑上线」下面那句：上架版改不动的那几条另外说一句
  private var waitingFooter: String {
    let base = String(localized: "回到同一个 Wi-Fi 后自动发过去。急着要的话，可以改成在这台手机上下")
    guard movable.count < model.waiting.count else { return base }
    return base + String(localized: "；YouTube 这类网站的只能在电脑上下，会留着等电脑")
  }
}

/// 电脑的状态那一行：绿点「在线 · Captae 0.2.0 · 正在下 3 个」/ 灰点「离线 · 上次在线 今天 14:20」
private struct ComputerStatusLine: View {
  var computer: Computer

  /// 版本 RemoteHub 现在不给，没有时不写
  private var statusText: String {
    if computer.needsPairing { return String(localized: "要重新配对：在电脑上被撤销了，或者这台手机的钥匙串清掉了") }
    if computer.connecting { return String(localized: "正在连接…") }
    if computer.online {
      return computer.version.isEmpty
        ? String(localized: "在线 · 正在下 \(computer.running) 个")
        : String(localized: "在线 · Captae \(computer.version) · 正在下 \(computer.running) 个")
    }
    return computer.lastSeen.isEmpty ? String(localized: "离线") : String(localized: "离线 · 上次在线 \(computer.lastSeen)")
  }

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: 6) {
      Circle()
        .fill(computer.needsPairing ? Palette.warn : computer.online ? Palette.ok : Palette.ink4)
        .frame(width: 7, height: 7)
        .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 1 }
      Text(verbatim: statusText)
        .font(.cpMeta)
        .monospacedDigit()
        .foregroundStyle(Palette.ink3)
        .fixedSize(horizontal: false, vertical: true)
    }
    .accessibilityElement(children: .combine)
  }
}

/// 「等电脑上线」的一条：时钟图标 + 标题 + 链接（等宽，一行，截掉中间）
private struct WaitingRow: View {
  var item: DownloadItem

  var body: some View {
    HStack(spacing: 12) {
      Image(systemName: "clock")
        .font(.system(size: 17))
        .foregroundStyle(Palette.ink3)
        .frame(width: 24)
      VStack(alignment: .leading, spacing: 3) {
        Text(verbatim: item.job.untitled ? item.job.src : item.job.title)
          .font(.cpSecondaryMedium)
          .foregroundStyle(Palette.ink)
          .lineLimit(2)
        Text(verbatim: item.job.src)
          .font(.cpMono(12, relativeTo: .caption))
          .foregroundStyle(Palette.ink3)
          .lineLimit(1)
          .truncationMode(.middle)
      }
      Spacer(minLength: 0)
    }
    .padding(.horizontal, 14)
    .padding(.vertical, 12)
    .accessibilityElement(children: .combine)
  }
}

/// 白卡行右边的小箭头
private struct Chevron: View {
  var body: some View {
    Image(systemName: "chevron.right")
      .font(.system(size: 13, weight: .semibold))
      .foregroundStyle(Palette.ink4)
      .accessibilityHidden(true)
  }
}

// MARK: 电脑详情

/// 「已配对的电脑」点进去：状态、版本、正在下几个，撤销配对
struct ComputerDetailView: View {
  var computer: Computer
  @Environment(AppModel.self) private var model
  @State private var confirming = false

  var body: some View {
    ScrollView {
      VStack(alignment: .leading, spacing: 14) {
        VStack(spacing: 10) {
          DeviceIcon(size: 56)
          Text(verbatim: computer.name)
            .font(.cpTitle)
            .foregroundStyle(Palette.ink)
          ComputerStatusLine(computer: computer)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, 8)

        VStack(spacing: 0) {
          if !computer.version.isEmpty {
            InfoRow(label: String(localized: "Captae 版本"), value: computer.version, mono: true)
            RowDivider()
          }
          InfoRow(label: String(localized: "正在下"), value: String(localized: "\(computer.running) 个"))
          RowDivider()
          InfoRow(label: String(localized: "上次在线"), value: computer.online ? String(localized: "现在") : computer.lastSeen)
        }
        .padding(.horizontal, 14)
        .card()

        Button(role: .destructive) {
          confirming = true
        } label: {
          Text("撤销配对")
            .font(.cpSecondaryMedium)
            .foregroundStyle(Palette.err)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(14)
            .contentShape(.rect)
        }
        .buttonStyle(.plain)
        .card()
        .padding(.top, 6)

        Text("撤销以后，这台手机不能再把链接发给这台电脑，也看不到它的素材库，等它上线的任务一起去掉。要再用就重新配对。电脑那边「已配对的手机」里还有这台手机，到电脑的「设置 › 手机」里撤销")
          .font(.cpMeta)
          .foregroundStyle(Palette.ink3)
          .fixedSize(horizontal: false, vertical: true)
          .padding(.horizontal, 4)
      }
      .padding(.horizontal, Spacing.page)
      .padding(.top, 4)
      .padding(.bottom, 24)
    }
    // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
    .scrollEdgeEffectStyle(.soft, for: .all)
    .pageBackground()
    .navigationTitle(computer.name)
    .navigationBarTitleDisplayMode(.inline)
    .confirmationDialog(Text(verbatim: cjk("撤销和", computer.name, "的配对？")), isPresented: $confirming, titleVisibility: .visible) {
      Button("撤销配对", role: .destructive) { model.removeComputer(computer.id) }
    }
  }
}

/// 三步说明里的一步：圆圈数字 + 一句话
private struct StepLine: View {
  var n: Int
  var text: LocalizedStringKey

  var body: some View {
    HStack(alignment: .firstTextBaseline, spacing: 10) {
      Text("\(n)")
        .font(.cpMono(12, relativeTo: .caption, weight: .monoSemibold))
        .foregroundStyle(Palette.ink)
        .frame(width: 22, height: 22)
        .background(Palette.well, in: .circle)
        .alignmentGuide(.firstTextBaseline) { $0[.bottom] - 6 }
      Text(text)
        .font(.cpSecondary)
        .foregroundStyle(Palette.ink)
        .fixedSize(horizontal: false, vertical: true)
    }
    .accessibilityElement(children: .combine)
  }
}

/// 电脑图标：凹面方块里的 laptopcomputer
struct DeviceIcon: View {
  var size: CGFloat = 38

  var body: some View {
    Image(systemName: "laptopcomputer")
      .font(.system(size: size * 0.42))
      .foregroundStyle(Palette.ink)
      .frame(width: size, height: size)
      .well(radius: size * 0.26)
      .accessibilityHidden(true)
  }
}
