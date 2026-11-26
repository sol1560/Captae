// 关于：logo、版本、许可证列表。Captae 自己是 AGPL-3.0；随 App 发的第三方东西逐条列出来（字体、网站图标）。
// 接上 Rust 核心以后，Rust 依赖的许可证从 `licenses/`（scripts/gen-licenses.py 生成）读进来接在后面。
import SwiftUI

struct AboutView: View {
  private struct License: Identifiable {
    var name: String
    var detail: String
    var license: String
    var id: String { name }
  }

  private let licenses: [License] = [
    License(name: "Captae", detail: "github.com/sol1560/Captae", license: "AGPL-3.0"),
    License(name: "Geist、Geist Mono", detail: "字体 · The Geist Project Authors", license: "OFL 1.1"),
    License(name: "Simple Icons", detail: "YouTube、哔哩哔哩、X 等网站的图标", license: "CC0 1.0"),
    License(name: "Blender 开放电影剧照", detail: "Big Buck Bunny、Sintel、Tears of Steel、Elephants Dream", license: "CC BY"),
  ]

  var body: some View {
    ScrollView {
      VStack(spacing: 0) {
        CaptaeLogo(size: 72)
        Text("Captae")
          .font(Typo.font(.bold, 24, relativeTo: .title2))
          .foregroundStyle(Palette.ink)
          .padding(.top, 14)
        Text("版本 \(version)")
          .font(.cpSecondary)
          .monospacedDigit()
          .foregroundStyle(Palette.ink3)
          .padding(.top, 4)
        Text("开源、免费的视频下载工具")
          .font(.cpMeta)
          .foregroundStyle(Palette.ink3)
          .padding(.top, 2)

        VStack(alignment: .leading, spacing: 8) {
          SectionLabel(title: "开源许可")
          VStack(spacing: 0) {
            ForEach(Array(licenses.enumerated()), id: \.offset) { i, l in
              HStack(alignment: .firstTextBaseline, spacing: 12) {
                VStack(alignment: .leading, spacing: 2) {
                  Text(l.name).font(.cpSecondaryMedium).foregroundStyle(Palette.ink)
                  Text(l.detail).font(.cpMeta).foregroundStyle(Palette.ink3).fixedSize(horizontal: false, vertical: true)
                }
                Spacer(minLength: 8)
                Text(l.license)
                  .font(.cpMono(12, relativeTo: .caption, weight: .monoMedium))
                  .foregroundStyle(Palette.ink2)
                  .padding(.horizontal, 7)
                  .padding(.vertical, 3)
                  .well(radius: 6)
              }
              .padding(.horizontal, 14)
              .padding(.vertical, 11)
              .accessibilityElement(children: .combine)
              if i < licenses.count - 1 { RowDivider(leading: 14) }
            }
          }
          .card()
        }
        .padding(.top, 32)

        Text("下载用户自己有权保存的内容。网站的标识归各自的所有者，界面里只用来指代来源网站。")
          .font(.cpSmall)
          .foregroundStyle(Palette.ink3)
          .multilineTextAlignment(.center)
          .padding(.top, 16)
          .padding(.horizontal, 8)
      }
      .padding(.horizontal, Spacing.page)
      .padding(.top, 28)
      .padding(.bottom, 32)
    }
    // 滚动边缘：柔和的渐隐模糊，不要硬分界线（BRIEF 2.7）
    .scrollEdgeEffectStyle(.soft, for: .all)
    .pageBackground()
    .navigationTitle("关于")
    .navigationBarTitleDisplayMode(.inline)
  }

  private var version: String {
    Bundle.main.object(forInfoDictionaryKey: "CFBundleShortVersionString") as? String ?? ""
  }
}
