// 封面、时长角标、站点图标、声音波形：照桌面端 `.cp-thumb`、`.dur`、`SiteIcon`。
// 时长角标在桌面端是半透明黑底 + 背后模糊；iOS 这里不自己加模糊（只留半透明黑底），玻璃和模糊只给系统控件。
import SwiftUI

/// 封面：有图就铺满裁切，没有就灰底 + 播放图标占位；右下角时长角标
struct Thumb: View {
  /// 演示数据里是资源名；接真数据时换成本地文件或电脑上的缩略图地址
  var name: String?
  var duration: String = ""
  var radius: CGFloat = Radius.sm
  var audio = false

  var body: some View {
    Color.clear
      .overlay {
        if audio {
          GeometryReader { geo in
              Waveform()
                .padding(.horizontal, geo.size.width * 0.12)
                .padding(.vertical, geo.size.height * 0.24)
            }
            .background(Palette.audioTile)
        } else if let name, Self.isStored(name) {
          StoredImage(key: name) { placeholder }
        } else if let name {
          Image(name).resizable().scaledToFill()
        } else {
          placeholder
        }
      }
      .clipShape(.rect(cornerRadius: radius, style: .continuous))
      .overlay(alignment: .bottomTrailing) {
        if !duration.isEmpty {
          DurationBadge(text: duration).padding(5)
        }
      }
      .accessibilityHidden(true)
  }

  private var placeholder: some View {
    ZStack {
      Palette.well
      Image(systemName: "play.rectangle.fill")
        .font(.system(size: 18))
        .foregroundStyle(Palette.ink4)
    }
  }

  /// 要读的图（网上的、本地文件、视频取帧）；其余是资源目录里的图名（RemoteImage.swift）
  static func isStored(_ name: String) -> Bool {
    name.hasPrefix("http://") || name.hasPrefix("https://") || name.hasPrefix("/") || name.hasPrefix("media:")
  }
}

/// 时长角标：等宽数字，白字，半透明黑底
struct DurationBadge: View {
  var text: String

  var body: some View {
    Text(text)
      .font(.cpMono(10.5, relativeTo: .caption2, weight: .monoMedium))
      .monospacedDigit()
      .foregroundStyle(.white)
      .padding(.horizontal, 5)
      .padding(.vertical, 3)
      .background(Color.black.opacity(0.55), in: .rect(cornerRadius: 5, style: .continuous))
      // 角标压在封面上，字太大会盖住画面
      .dynamicTypeSize(...DynamicTypeSize.xLarge)
  }
}

/// 站点图标：认得的网站用 Simple Icons，认不出的用地球
struct SiteIcon: View {
  var plat: String?
  var url: String
  var size: CGFloat = 13

  var body: some View {
    Group {
      if let asset = Site.iconAsset(plat: plat, url: url) {
        Image(asset).resizable().scaledToFit()
      } else {
        Image(systemName: "globe").resizable().scaledToFit().foregroundStyle(Palette.ink3)
      }
    }
    .frame(width: size, height: size)
    .accessibilityHidden(true)
  }
}

/// 声音素材的波形占位：桌面端素材库同款（深底 + 白色竖条）
struct Waveform: View {
  private static let bars: [CGFloat] = [
    0.32, 0.5, 0.42, 0.7, 0.55, 0.8, 0.6, 0.9, 0.5, 0.74, 0.62, 0.95, 0.7, 0.48, 0.85, 0.66, 0.9, 0.58, 0.76, 0.5, 0.88,
    0.64, 0.8, 0.46, 0.7, 0.55, 0.62, 0.4, 0.5, 0.3,
  ]

  var body: some View {
    GeometryReader { geo in
      let n = CGFloat(Self.bars.count)
      let w = geo.size.width / (n * 2 - 1)
      HStack(alignment: .center, spacing: w) {
        ForEach(Self.bars.indices, id: \.self) { i in
          Capsule().fill(.white.opacity(0.9)).frame(width: w, height: geo.size.height * Self.bars[i])
        }
      }
      .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
  }
}
