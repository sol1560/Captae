// 设置：系统分组列表（Form），从下载页右上角「更多」进。对照桌面端 `dialogs/Settings.tsx`、`screenshots/settings.png`：
// 白卡里一行一项。底色换成和其他页一样的 #f7f7f7（系统列表的默认底色是偏蓝的灰）。
// 分组：下载、电脑、网络、关于（BRIEF 第 4 节 07）。代理只有「不用」和「手动填写」（手机上没有「跟随系统」，计划 1）。
import SwiftUI

struct SettingsView: View {
  @Environment(AppModel.self) private var model

  var body: some View {
    @Bindable var model = model
    Form {
      Section {
        Picker("存到", selection: $model.settings.saveTo) {
          ForEach(PhoneSettings.SaveTo.allCases) { Text($0.rawValue).tag($0) }
        }
        // 默认清晰度、格式只对网页链接有用（直链、M3U8 照原样下）
        Picker("默认清晰度", selection: $model.settings.quality) {
          ForEach(PhoneSettings.qualities, id: \.label) { Text(LocalizedStringKey($0.label)).tag($0.label) }
        }
        Picker("格式", selection: $model.settings.format) {
          ForEach(PhoneSettings.formats, id: \.label) { Text(LocalizedStringKey($0.label)).tag($0.label) }
        }
        Picker("同时下载", selection: $model.settings.concurrency) {
          ForEach(1...5, id: \.self) { Text("\($0) 个").tag($0) }
        }
        Picker("选清晰度等", selection: $model.settings.pickSeconds) {
          ForEach(Set([3, 6, 10, 15, 30, model.settings.pickSeconds]).sorted(), id: \.self) { Text("\($0) 秒").tag($0) }
        }
      } header: {
        Text("下载").font(.cpMeta)
      } footer: {
        Text(model.settings.saveTo == .photosAndFiles ? "视频和图片存进相册，声音存到「文件」› Captae" : "全部存到「文件」› Captae")
          .font(.cpMeta)
      }

      Section {
        // 打开「管理电脑」sheet（和标题菜单里的「管理电脑…」是同一张）
        Button {
          model.managePath = []
          model.manageComputers = true
        } label: {
          LabeledContent("已配对的电脑") {
            HStack(spacing: 6) {
              Text(model.computers.isEmpty ? "还没有" : "\(model.computers.count) 台")
              Image(systemName: "chevron.right").font(.system(size: 13, weight: .semibold)).foregroundStyle(Palette.ink4)
            }
          }
        }
        .foregroundStyle(Palette.ink)
      } header: {
        Text("电脑").font(.cpMeta)
      }

      Section {
        Picker("代理", selection: $model.settings.proxy) {
          ForEach(PhoneSettings.Proxy.allCases) { Text($0.rawValue).tag($0) }
        }
        if model.settings.proxy == .manual {
          TextField("代理地址", text: $model.settings.proxyURL, prompt: Text("http://127.0.0.1:7890"))
            .font(.cpMono(15, relativeTo: .body))
            .keyboardType(.URL)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
        }
      } header: {
        Text("网络").font(.cpMeta)
      } footer: {
        Text(verbatim: model.settings.proxy == .manual ? "要写成 协议://地址:端口，比如 http://127.0.0.1:7890 或 socks5://127.0.0.1:1080" : "不经过代理，直接连接。交给电脑下的不受这里影响")
          .font(.cpMeta)
      }

      Section {
        LabeledContent("版本", value: appVersion)
        NavigationLink("开源许可", value: Route.about)
        Link(destination: URL(string: "https://github.com/sol1560/Captae")!) {
          LabeledContent("源代码") {
            Text(verbatim: "github.com/sol1560/Captae").font(.cpMono(13, relativeTo: .footnote))
          }
        }
      } header: {
        Text("关于").font(.cpMeta)
      }
    }
    // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
    .scrollEdgeEffectStyle(.soft, for: .all)
    .headerProminence(.standard)
    .scrollContentBackground(.hidden)
    .pageBackground()
    .navigationTitle("设置")
    // 改了就存（核心的 settings.json + mobile.json，「存到」在 UserDefaults）。代理地址打字时不每个字存一次，离开这页时存
    .onChange(of: model.settings) { old, new in
      var a = old, b = new
      a.proxyURL = ""
      b.proxyURL = ""
      if a != b { model.saveSettings() }
    }
    .onDisappear { model.saveSettings() }
  }

  private var appVersion: String {
    Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
  }
}
