// 封面、缩略图从哪来、怎么读（Thumb 用）。名字的写法：
//   http(s)://…       网上的封面（读取信息拿到的），下载后缩小，存进 Caches/thumbs
//   /…                本地图片文件
//   media:/…          本地的视频或图片文件：手机上没有 ffmpeg，核心不生成缩略图，这里用 AVFoundation 取一帧（视频）或缩小（图片）
//   其他              资源目录里的图（演示数据的剧照、网站图标）
// 读到时淡入 300ms（MOTION 1.2「封面、缩略图读到」，网格里图多，只动透明度）；这次打开以来读到过的直接显示，不重播。
import AVFoundation
import CryptoKit
import ImageIO
import SwiftUI
import UIKit

final class ImageStore {
  static let shared = ImageStore()
  private let memory = NSCache<NSString, UIImage>()
  private var inflight: [String: Task<UIImage?, Never>] = [:]

  func cached(_ key: String) -> UIImage? {
    memory.object(forKey: key as NSString)
  }

  /// 读一张图（同一个名字同时只读一次）
  func load(_ key: String, maxPixel: CGFloat = 640) async -> UIImage? {
    if let image = cached(key) { return image }
    if let task = inflight[key] { return await task.value }
    let task = Task { await Self.produce(key, maxPixel: maxPixel) }
    inflight[key] = task
    let image = await task.value
    inflight[key] = nil
    if let image { memory.setObject(image, forKey: key as NSString) }
    return image
  }

  /// 图的平均色（读取时的流光换成封面的颜色，BRIEF 2.6）
  func averageColor(_ key: String) async -> Color? {
    guard let image = await load(key), let cg = image.cgImage else { return nil }
    var px = [UInt8](repeating: 0, count: 4)
    let drew = px.withUnsafeMutableBytes { buf -> Bool in
      guard
        let ctx = CGContext(
          data: buf.baseAddress, width: 1, height: 1, bitsPerComponent: 8, bytesPerRow: 4, space: CGColorSpaceCreateDeviceRGB(),
          bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)
      else { return false }
      ctx.interpolationQuality = .medium
      ctx.draw(cg, in: CGRect(x: 0, y: 0, width: 1, height: 1))
      return true
    }
    guard drew else { return nil }
    return Color(red: Double(px[0]) / 255, green: Double(px[1]) / 255, blue: Double(px[2]) / 255)
  }

  @concurrent
  private static func produce(_ key: String, maxPixel: CGFloat) async -> UIImage? {
    await ImageWork.produce(key, maxPixel: maxPixel)
  }
}

/// 读图（在后台线程上，不碰主线程的状态）
nonisolated private enum ImageWork {
  static var diskDir: URL {
    URL.cachesDirectory.appending(path: "thumbs-2", directoryHint: .isDirectory)
  }

  static func produce(_ key: String, maxPixel: CGFloat) async -> UIImage? {
    if key.hasPrefix("/") {
      return downsample(url: URL(filePath: key), maxPixel: maxPixel)
    }
    let isRemote = key.hasPrefix("http://") || key.hasPrefix("https://")
    let isMedia = key.hasPrefix("media:")
    guard isRemote || isMedia else { return nil }
    // 磁盘缓存：网上的封面、视频取的帧，存成 JPEG（文件名是名字的摘要）
    let digest = Insecure.MD5.hash(data: Data(key.utf8)).map { String(format: "%02x", $0) }.joined()
    let cacheFile = diskDir.appending(path: digest + ".jpg")
    if let data = try? Data(contentsOf: cacheFile), let image = UIImage(data: data) { return image }
    var image: UIImage?
    if isRemote, let url = URL(string: key) {
      if let (data, _) = try? await URLSession.shared.data(from: url) { image = downsample(data: data, maxPixel: maxPixel) }
    } else {
      let path = String(key.dropFirst("media:".count))
      let url = URL(filePath: path)
      image = downsample(url: url, maxPixel: maxPixel)
      if image == nil { image = await videoFrame(url, maxPixel: maxPixel) }
    }
    if let image, let jpeg = image.jpegData(compressionQuality: 0.82) {
      try? FileManager.default.createDirectory(at: diskDir, withIntermediateDirectories: true)
      try? jpeg.write(to: cacheFile, options: .atomic)
    }
    return image
  }

  static func downsample(url: URL, maxPixel: CGFloat) -> UIImage? {
    guard let src = CGImageSourceCreateWithURL(url as CFURL, nil) else { return nil }
    return thumbnail(src, maxPixel: maxPixel)
  }

  static func downsample(data: Data, maxPixel: CGFloat) -> UIImage? {
    guard let src = CGImageSourceCreateWithData(data as CFData, nil) else { return nil }
    return thumbnail(src, maxPixel: maxPixel)
  }

  static func thumbnail(_ src: CGImageSource, maxPixel: CGFloat) -> UIImage? {
    let opts: [CFString: Any] = [
      kCGImageSourceCreateThumbnailFromImageAlways: true,
      kCGImageSourceCreateThumbnailWithTransform: true,
      kCGImageSourceThumbnailMaxPixelSize: maxPixel,
    ]
    guard let cg = CGImageSourceCreateThumbnailAtIndex(src, 0, opts as CFDictionary) else { return nil }
    return UIImage(cgImage: cg)
  }

  /// 视频取一帧：时长的四分之一处，最多第 10 秒（片头常是黑场，第 1 秒取出来一片黑），取不到再取开头
  static func videoFrame(_ url: URL, maxPixel: CGFloat) async -> UIImage? {
    let asset = AVURLAsset(url: url)
    let gen = AVAssetImageGenerator(asset: asset)
    gen.appliesPreferredTrackTransform = true
    gen.maximumSize = CGSize(width: maxPixel, height: maxPixel)
    let dur = (try? await asset.load(.duration)).map(CMTimeGetSeconds) ?? 0
    let at = dur.isFinite && dur > 0 ? min(dur / 4, 10) : 1
    for t in [CMTime(seconds: at, preferredTimescale: 600), .zero] {
      if let (cg, _) = try? await gen.image(at: t) { return UIImage(cgImage: cg) }
    }
    return nil
  }
}

/// 一张要读的图：读到前是占位，读到后淡入
struct StoredImage<Placeholder: View>: View {
  var key: String
  @ViewBuilder var placeholder: () -> Placeholder
  @State private var image: UIImage?

  init(key: String, @ViewBuilder placeholder: @escaping () -> Placeholder) {
    self.key = key
    self.placeholder = placeholder
    _image = State(initialValue: ImageStore.shared.cached(key))
  }

  var body: some View {
    ZStack {
      if let image {
        Image(uiImage: image).resizable().scaledToFill()
          .transition(.opacity)
      } else {
        placeholder()
      }
    }
    .task(id: key) {
      if let hit = ImageStore.shared.cached(key) {
        image = hit
        return
      }
      let loaded = await ImageStore.shared.load(key)
      withAnimation(Motion.reveal) { image = loaded }
    }
  }
}
