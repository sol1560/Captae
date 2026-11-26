// 素材库的一张卡：封面（时长角标）、标题、来源 · 格式 · 时间。照桌面端 `pages/library/Grid.tsx`。
// 两列放不下一行元信息，分成两行：来源一行，格式和时间一行。
import SwiftUI

struct AssetTile: View {
  var asset: Asset

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Thumb(name: asset.thumb, duration: asset.dur, radius: Radius.md, audio: asset.kind == .audio)
        .aspectRatio(16 / 9, contentMode: .fit)
        .padding(5)
      VStack(alignment: .leading, spacing: 4) {
        Text(asset.title)
          .font(.cpMetaMedium)
          .foregroundStyle(Palette.ink)
          .lineLimit(1)
        HStack(spacing: 4) {
          SiteIcon(plat: nil, url: siteURL, size: 12)
          Text(asset.src)
        }
        .lineLimit(1)
        Text([asset.fmt, asset.when].filter { !$0.isEmpty }.joined(separator: " · "))
          .lineLimit(1)
      }
      .font(.cpSmall)
      .foregroundStyle(Palette.ink3)
      .padding(.horizontal, 9)
      .padding(.top, 3)
      .padding(.bottom, 10)
    }
    .frame(maxWidth: .infinity, alignment: .leading)
    .card()
    .contentShape(.rect(cornerRadius: Radius.lg))
    .accessibilityElement(children: .combine)
  }

  /// 站点图标按来源名字认（「YouTube」「哔哩哔哩」「X」），认不出用地球
  private var siteURL: String {
    switch asset.src {
    case "YouTube": "youtube.com"
    case "哔哩哔哩": "bilibili.com"
    case "X": "x.com"
    case "抖音", "TikTok": "tiktok.com"
    case "Instagram": "instagram.com"
    default: asset.src
    }
  }
}
