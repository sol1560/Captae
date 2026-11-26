// 下完以后存到哪（计划 5.4「保存」）：
// - 默认：视频、图片存进相册（PhotoKit 只要「添加」权限），声音存到「文件」› Captae；
// - 设置里改成「都存到文件」：全部放「文件」› Captae。
// 「文件」› Captae 就是 App 的 Documents（Info.plist 打开了 UIFileSharingEnabled + LSSupportsOpeningDocumentsInPlace）。
// 下完的原文件在 Application Support/Captae/Downloads（素材库认这个路径），放进「文件」用硬链接：不多占空间，
// 在「文件」里删掉不影响素材库，反过来也一样。存进相册是相册自己的一份（素材库里那份留着，App 里能播）。
// 存到哪记在 UserDefaults（素材详情的「位置」「在「文件」中查看」用）。
import Foundation
import Photos

/// 一条素材存到了哪里
struct SavedPlace: Codable, Hashable, Sendable {
  enum Kind: String, Codable, Sendable {
    case photos
    case files
  }

  var kind: Kind
  /// 存到「文件」时，在 Documents 里的文件名
  var fileName: String?
}

@Observable
final class SaveService {
  private static let key = "savedPlaces"
  private(set) var places: [Int64: SavedPlace] = [:]

  init() {
    if let data = UserDefaults.standard.data(forKey: Self.key),
      let decoded = try? JSONDecoder().decode([Int64: SavedPlace].self, from: data)
    {
      places = decoded
    }
  }

  static var filesDir: URL { URL.documentsDirectory }

  /// 存一条下完的素材。相册存不进去（没给权限、格式相册不认）时改存「文件」
  /// `record`：记下存到了哪（手机素材库里的才记；从电脑存过来的素材 id 是电脑上的，和手机的会重复，不记）
  func save(_ asset: Asset, photos: Bool, record: Bool = true) async -> SavedPlace? {
    let src = URL(filePath: asset.path)
    guard FileManager.default.fileExists(atPath: src.path(percentEncoded: false)) else { return nil }
    var place: SavedPlace?
    if photos && asset.kind != .audio, await Self.addToPhotos(src, image: asset.kind == .image) {
      place = SavedPlace(kind: .photos)
    } else if let name = Self.linkIntoFiles(src) {
      place = SavedPlace(kind: .files, fileName: name)
    }
    if let place, record {
      places[asset.id] = place
      persist()
    }
    return place
  }

  /// 素材从素材库里删掉时：「文件」里那一份一起删（相册里的不动，那是用户自己的相册）
  func forget(_ assetId: Int64) {
    if let name = places[assetId]?.fileName {
      try? FileManager.default.removeItem(at: Self.filesDir.appending(path: name))
    }
    places[assetId] = nil
    persist()
  }

  /// 「文件」App 里定位到这个文件
  func filesURL(_ assetId: Int64) -> URL? {
    guard let name = places[assetId]?.fileName else { return nil }
    let path = Self.filesDir.appending(path: name).path(percentEncoded: false)
    return URL(string: "shareddocuments://" + (path.addingPercentEncoding(withAllowedCharacters: .urlPathAllowed) ?? path))
  }

  private func persist() {
    if let data = try? JSONEncoder().encode(places) {
      UserDefaults.standard.set(data, forKey: Self.key)
    }
  }

  private static func addToPhotos(_ url: URL, image: Bool) async -> Bool {
    let status = await PHPhotoLibrary.requestAuthorization(for: .addOnly)
    guard status == .authorized || status == .limited else { return false }
    do {
      try await PhotoWrite.add(url, image: image)
      return true
    } catch {
      return false
    }
  }

  /// 在 Documents 里放一个硬链接（同名的加「 2」「 3」）；链接不了就复制
  private static func linkIntoFiles(_ src: URL) -> String? {
    let fm = FileManager.default
    let base = src.deletingPathExtension().lastPathComponent
    let ext = src.pathExtension
    var name = src.lastPathComponent
    var n = 2
    while fm.fileExists(atPath: filesDir.appending(path: name).path(percentEncoded: false)) {
      name = ext.isEmpty ? "\(base) \(n)" : "\(base) \(n).\(ext)"
      n += 1
    }
    let dst = filesDir.appending(path: name)
    do {
      try fm.linkItem(at: src, to: dst)
    } catch {
      do { try fm.copyItem(at: src, to: dst) } catch { return nil }
    }
    return name
  }
}

/// 写相册：Photos 在自己的后台队列上调这个闭包，所以闭包不能是主线程隔离的（App 模块默认在主线程，
/// 在主线程的上下文里写闭包会被推断成 @MainActor，Photos 一调就被运行时的隔离检查中止，M3 实测踩过）
nonisolated private enum PhotoWrite {
  static func add(_ url: URL, image: Bool) async throws {
    try await PHPhotoLibrary.shared().performChanges {
      let request = PHAssetCreationRequest.forAsset()
      request.addResource(with: image ? .photo : .video, fileURL: url, options: nil)
    }
  }
}
