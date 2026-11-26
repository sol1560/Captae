// Captae 的 logo：黑色圆角方块（上 #2e2e30 到下 #0e0e0f）+ 白色取景框 + 中间的圆点。
// 照桌面端 `apps/desktop/src/parts/index.tsx` 的 `CaptaeLogo` 画成矢量：方块在 64×64 的格子里，
// 取景框在 24×24 的格子里，平移 14.6、放大 1.45 倍，线宽 2.35。
import SwiftUI

struct CaptaeLogo: View {
  var size: CGFloat = 22

  var body: some View {
    ZStack {
      LogoSquare()
        .fill(LinearGradient(colors: [Color(hex: 0x2E2E30), Color(hex: 0x0E0E0F)], startPoint: .top, endPoint: .bottom))
      LogoMark()
        .stroke(.white, style: StrokeStyle(lineWidth: size * 2.35 * 1.45 / 64, lineCap: .round, lineJoin: .round))
    }
    .frame(width: size, height: size)
    .accessibilityHidden(true)
  }
}

/// 圆角方块：`M0 18C0 5.4 5.4 0 18 0h28c12.6 0 18 5.4 18 18v28c0 12.6-5.4 18-18 18H18C5.4 64 0 58.6 0 46Z`
nonisolated private struct LogoSquare: Shape {
  func path(in rect: CGRect) -> Path {
    let k = rect.width / 64
    func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: rect.minX + x * k, y: rect.minY + y * k) }
    var path = Path()
    path.move(to: p(0, 18))
    path.addCurve(to: p(18, 0), control1: p(0, 5.4), control2: p(5.4, 0))
    path.addLine(to: p(46, 0))
    path.addCurve(to: p(64, 18), control1: p(58.6, 0), control2: p(64, 5.4))
    path.addLine(to: p(64, 46))
    path.addCurve(to: p(46, 64), control1: p(64, 58.6), control2: p(58.6, 64))
    path.addLine(to: p(18, 64))
    path.addCurve(to: p(0, 46), control1: p(5.4, 64), control2: p(0, 58.6))
    path.closeSubpath()
    return path
  }
}

/// 取景框的四个角 + 圆点：`M4 8V5a1 1 0 0 1 1-1h3 …`，`circle(12, 12, 2.6)`
nonisolated private struct LogoMark: Shape {
  func path(in rect: CGRect) -> Path {
    let k = rect.width / 64 * 1.45
    let o = rect.width / 64 * 14.6
    func p(_ x: CGFloat, _ y: CGFloat) -> CGPoint { CGPoint(x: rect.minX + o + x * k, y: rect.minY + o + y * k) }
    var path = Path()
    // 每个角：一条边、半径 1 的圆角、另一条边
    let corners: [(CGPoint, CGPoint, CGPoint)] = [
      (p(4, 8), p(4, 4), p(8, 4)),
      (p(16, 4), p(20, 4), p(20, 8)),
      (p(20, 16), p(20, 20), p(16, 20)),
      (p(8, 20), p(4, 20), p(4, 16)),
    ]
    for (a, corner, b) in corners {
      path.move(to: a)
      path.addArc(tangent1End: corner, tangent2End: b, radius: k)
      path.addLine(to: b)
    }
    let c = p(12, 12)
    path.addEllipse(in: CGRect(x: c.x - 2.6 * k, y: c.y - 2.6 * k, width: 5.2 * k, height: 5.2 * k))
    return path
  }
}

/// 取景框四个角（配对扫码页的取景框用同一个形状，不画圆点）
nonisolated struct ViewfinderCorners: Shape {
  /// 每个角的边长占整个框的比例
  var arm: CGFloat = 0.18
  var radius: CGFloat = 14

  func path(in rect: CGRect) -> Path {
    let lx = rect.width * arm, ly = rect.height * arm
    var path = Path()
    let corners: [(CGPoint, CGPoint, CGPoint)] = [
      (CGPoint(x: rect.minX, y: rect.minY + ly), CGPoint(x: rect.minX, y: rect.minY), CGPoint(x: rect.minX + lx, y: rect.minY)),
      (CGPoint(x: rect.maxX - lx, y: rect.minY), CGPoint(x: rect.maxX, y: rect.minY), CGPoint(x: rect.maxX, y: rect.minY + ly)),
      (CGPoint(x: rect.maxX, y: rect.maxY - ly), CGPoint(x: rect.maxX, y: rect.maxY), CGPoint(x: rect.maxX - lx, y: rect.maxY)),
      (CGPoint(x: rect.minX + lx, y: rect.maxY), CGPoint(x: rect.minX, y: rect.maxY), CGPoint(x: rect.minX, y: rect.maxY - ly)),
    ]
    for (a, corner, b) in corners {
      path.move(to: a)
      path.addArc(tangent1End: corner, tangent2End: b, radius: radius)
      path.addLine(to: b)
    }
    return path
  }
}

#Preview {
  HStack(spacing: 20) {
    CaptaeLogo(size: 22)
    CaptaeLogo(size: 64)
  }
  .padding()
}
