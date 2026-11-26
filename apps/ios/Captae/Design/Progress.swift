// 进度条、倒计时圆环、扫光文字：照桌面端 `theme.css` 的 `.cp-progress`（高 4、底 rgba(0,0,0,.07)、填充强调色）、
// `parts/index.tsx` 的 `Countdown`（圆环、底 rgba(0,0,0,.12)、强调色描边）和 affectUI `ShimmerText` + `cp-wait`。
// 动效（docs/MOTION.md，2026-11-25 按 work/mobile/motion-review.md 改过）：
//   进度条 c5：整条宽的填充做平移（只动位移，不每帧重新排版），外面裁成胶囊；匀速往前，时长等于实际收到两次进度的间隔
//             （250–1000ms：手机上 250ms 一次，电脑经远程推来的可能更慢）；往回走（下载完进「处理中」从 0 开始）直接跳；
//             刚出现时不从 0 重播。不知道总量时是一道两头柔和的渐变段（45% 宽，1.1s 走过去 + 停 0.4s，和 Android 一样）
//   倒计时 d：圆环交给系统按剩下的秒数匀速走完（不每帧重算）；旁边的数字每整秒换一次，和圆环同一刻；减少动效时圆环也按秒跳
//   扫光 m：一次 2.7s，0.567s 扫过、其余停住；高光是一块固定的渐变，系统推着它走（不每帧重算）；减少动效时不扫
// 减少动态效果时：进度条直接跳到新值、流光换成静止的淡色、文字不扫光（MOTION 1.5）。
import SwiftUI

struct ProgressBar: View {
  enum Tone { case accent, paused }

  /// 0…1
  var value: Double
  var tone: Tone = .accent
  /// 不知道总量（正在读取）：一道流光
  var indeterminate = false
  var label = "下载进度"
  /// 条的高度：任务卡里 4，下载条底边 2
  var height: CGFloat = 4

  @Environment(\.accessibilityReduceMotion) private var reduce
  /// 画出来的值：新值来了以后按进度间隔匀速走过去
  @State private var shown: Double?
  @State private var lastChange = Date.now

  var body: some View {
    GeometryReader { geo in
      let w = geo.size.width
      let v = min(1, max(0, shown ?? value))
      ZStack(alignment: .leading) {
        Capsule().fill(Color.black.opacity(0.07))
        if indeterminate {
          if reduce {
            Capsule().fill(Palette.accent.opacity(0.35))
          } else {
            IndeterminateSweep(width: w)
          }
        } else {
          // 整条宽的填充往左推出去 (1 - v) 的宽度：动的只是位移（审查第 1 条）
          Capsule()
            .fill(tone == .paused ? Palette.ink4 : Palette.accent)
            .frame(width: w)
            .offset(x: -w * (1 - v))
        }
      }
      .clipShape(.capsule)
    }
    .frame(height: height)
    .onChange(of: value) { old, new in
      let now = Date.now
      let gap = min(1, max(0.25, now.timeIntervalSince(lastChange)))
      lastChange = now
      if reduce || new < old {
        // 往回走、减少动态效果：直接到位
        var t = Transaction()
        t.disablesAnimations = true
        withTransaction(t) { shown = new }
      } else {
        withAnimation(.linear(duration: gap)) { shown = new }
      }
    }
    .accessibilityElement()
    .accessibilityLabel(label)
    .accessibilityValue(indeterminate ? "正在读取" : Fmt.percent(value))
  }
}

/// 不确定进度：一段两头柔和的强调色渐变（45% 宽）从左走到右（1.1s 匀速），停 0.4s，再来
private struct IndeterminateSweep: View {
  var width: CGFloat
  private static let move = 1.1
  private static let rest = 0.4

  var body: some View {
    let seg = max(width * 0.45, 24)
    TimelineView(.animation) { ctx in
      let t = ctx.date.timeIntervalSinceReferenceDate.truncatingRemainder(dividingBy: Self.move + Self.rest)
      let p = min(1, t / Self.move)
      LinearGradient(
        colors: [Palette.accent.opacity(0), Palette.accent.opacity(0.5), Palette.accent.opacity(0)],
        startPoint: .leading, endPoint: .trailing
      )
      .frame(width: seg)
      .offset(x: -seg + (width + seg) * p)
      .opacity(t < Self.move ? 1 : 0)
    }
  }
}

/// 倒计时圆环：剩下的比例用强调色描边
struct CountdownRing: View {
  /// 剩下的比例 0…1
  var remaining: Double
  var size: CGFloat = 16

  var body: some View {
    ZStack {
      Circle().stroke(Color.black.opacity(0.12), lineWidth: size / 8)
      Circle()
        .trim(from: 0, to: min(1, max(0, remaining)))
        .stroke(Palette.accent, style: StrokeStyle(lineWidth: size / 8, lineCap: .round))
        .rotationEffect(.degrees(-90))
    }
    .frame(width: size, height: size)
    .accessibilityHidden(true)
  }
}

/// 跟着时间走的倒计时：圆环匀速（交给系统动画，减少动效时按秒跳）+ 「3 秒后自动用 1080p」（每整秒换一次）
struct PickCountdown: View {
  /// 倒计时到哪一刻（毫秒时间戳）；没有时不显示圆环和秒数
  var until: Int64?
  /// 一共几秒（设置里的「选清晰度等几秒」）
  var total: Int
  var variant: String
  var ringSize: CGFloat = 14
  var font: Font = .cpMeta

  @Environment(\.accessibilityReduceMotion) private var reduce
  @State private var ring: Double = 1

  private var end: Date? { until.map { Date(timeIntervalSince1970: Double($0) / 1000) } }

  var body: some View {
    HStack(spacing: 7) {
      if let end {
        // 数字和按秒跳的圆环：从结束时刻往回对齐到整秒，和数字同一刻变
        let start = end.addingTimeInterval(-ceil(max(0, end.timeIntervalSinceNow)))
        TimelineView(.periodic(from: start, by: 1)) { ctx in
          let secs = Int(max(0, end.timeIntervalSince(ctx.date)).rounded(.up))
          HStack(spacing: 7) {
            CountdownRing(remaining: reduce ? Double(secs) / Double(max(total, 1)) : ring, size: ringSize)
            Text("\(secs) 秒后自动用 \(variant)")
          }
        }
      } else {
        Text("选一个清晰度")
      }
    }
    .font(font)
    .monospacedDigit()
    .foregroundStyle(Palette.ink2)
    .task(id: until) { startRing() }
  }

  /// 圆环：从现在剩下的比例，用剩下的秒数匀速走到 0（系统按帧插值，不重算这个视图）
  private func startRing() {
    guard let end, !reduce else { return }
    let left = max(0, end.timeIntervalSinceNow)
    var t = Transaction()
    t.disablesAnimations = true
    withTransaction(t) { ring = left / Double(max(total, 1)) }
    withAnimation(.linear(duration: left)) { ring = 0 }
  }
}

/// 扫光文字（「正在读取视频信息」）：有状态文字、没有进度数字的等待，不放转圈，让文字本身扫光（MOTION m）
struct ShimmerText: View {
  var text: LocalizedStringKey
  var font: Font = .cpSecondary
  /// 设计稿的静止帧：高光停在文字中间
  var still = false

  @Environment(\.accessibilityReduceMotion) private var reduce
  @State private var swept = false
  /// 一次 2.7s：停 2.133s，0.567s 扫过（前 21%）
  private static let sweep = 0.567
  private static let rest = 2.133

  var body: some View {
    Text(text)
      .font(font)
      .foregroundStyle(Palette.ink3)
      .overlay {
        if !reduce {
          GeometryReader { geo in
            let w = geo.size.width
            highlight
              .frame(width: w * 0.5)
              // 静止帧停在中间；动的时候从左边外面推到右边外面
              .offset(x: still ? w * 0.25 : (swept ? w * 1.1 : -w * 0.6))
          }
          .mask(Text(text).font(font))
          .allowsHitTesting(false)
          .onAppear {
            guard !still else { return }
            withAnimation(.linear(duration: Self.sweep).delay(Self.rest).repeatForever(autoreverses: false)) { swept = true }
          }
        }
      }
  }

  /// 一块固定的高光：中间亮、两头透明
  private var highlight: some View {
    LinearGradient(
      colors: [.clear, .white.opacity(0.65), .clear],
      startPoint: .leading, endPoint: .trailing)
  }
}
