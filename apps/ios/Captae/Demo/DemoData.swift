// 演示数据：设计稿用（BRIEF 第 3 节）。和桌面端演示后端 `apps/desktop/src/dev/mock.ts`、`mock-library.ts` 同一批：
// Blender 基金会的开放电影，剧照见 Resources/DEMO-CREDITS.md（CC BY）。接上 Rust 绑定以后这里只留给预览和截图用。
#if DEBUG
import CaptaeKit
import Foundation

enum Demo {
  // MARK: 电脑

  static let computer = Computer(id: "mbp", name: "Sol 的 MacBook Pro", online: true, version: "0.2.0", running: 3, lastSeen: "刚刚")
  static let computerOffline = Computer(id: "mbp", name: "Sol 的 MacBook Pro", online: false, version: "0.2.0", running: 0, lastSeen: "今天 14:20")
  static let nearby = [NearbyComputer(name: "Sol 的 MacBook Pro", hosts: ["192.168.1.5"], port: 47863, fingerprint: nil, seenAt: 0)]

  // MARK: 下载任务

  /// 「在电脑上」一组
  static let onComputer: [DownloadItem] = [
    DownloadItem(
      job: Job(
        id: 1, title: "youtube.com/watch?v=aqz-KE-bpKQ", src: "youtube.com/watch?v=aqz-KE-bpKQ",
        url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ", plat: "youtube", eng: .ytDlp, fmt: "", state: .run, phase: .probe),
      placement: .computer),
    DownloadItem(
      job: Job(
        id: 2, title: "Tears of Steel · 完整版", src: "mango.blender.org/tos/master.m3u8",
        url: "https://mango.blender.org/tos/master.m3u8", eng: .hls, fmt: "M3U8 → MP4", dur: "12:14", thumb: "tos-01",
        state: .run, phase: .pick,
        vars: [
          Variant(r: "3840×1600", p: "2160p", b: 15.2, mb: 1400),
          Variant(r: "1920×800", p: "1080p", b: 4.6, mb: 420),
          Variant(r: "1280×534", p: "720p", b: 2.3, mb: 210),
          Variant(r: "854×356", p: "480p", b: 1.2, mb: 112),
        ]),
      placement: .computer),
    sintelRunning,
    DownloadItem(
      job: Job(
        id: 4, title: "Elephants Dream", src: "cdn.example.org/ed/1080/index.m3u8",
        url: "https://cdn.example.org/ed/1080/index.m3u8", eng: .hls, fmt: "M3U8 → MP4", dur: "10:54", thumb: "ed-emo-proog",
        size: 286, got: 107, speed: 4.8, state: .run, phase: .dl,
        segs: Array(repeating: .done, count: 40) + [.retrying, .retrying] + Array(repeating: .wait, count: 70)),
      placement: .computer),
  ]

  /// 任务 3：详情 sheet 的「下载中」也用它
  static let sintelRunning = DownloadItem(
    job: Job(
      id: 3, title: "Sintel · 完整版短片", src: "bilibili.com/video/BV1qJ411m7Sg",
      url: "https://www.bilibili.com/video/BV1qJ411m7Sg", plat: "bilibili", eng: .ytDlp, fmt: "MP4 · 1080p", dur: "14:48",
      thumb: "sintel-1", size: 312, got: 143, speed: 6.4, state: .run, phase: .dl),
    placement: .computer)

  /// 「在手机上」一组
  static let onPhone: [DownloadItem] = [
    DownloadItem(
      job: Job(
        id: 5, title: "Tears of Steel · 预告片", src: "mango.blender.org/download/tears_of_steel_trailer.mp4",
        url: "https://mango.blender.org/download/tears_of_steel_trailer.mp4", eng: .aria2, fmt: "MP4 文件", dur: "1:04",
        thumb: "tos-05", size: 96, got: 62, state: .pause, phase: .dl),
      placement: .phone),
    DownloadItem(
      job: Job(
        id: 6, title: "Sintel · 预告片", src: "download.blender.org/durian/trailer/sintel_trailer-1080p.mp4",
        url: "https://download.blender.org/durian/trailer/sintel_trailer-1080p.mp4", eng: .aria2, fmt: "MP4 文件",
        dur: "0:52", thumb: "sintel-3", size: 48, state: .queue, phase: .probe),
      placement: .phone, ahead: 1),
  ]

  /// 出错状态用的任务（01-errors，都在手机上）
  static let needLogin = DownloadItem(
    job: Job(
      id: 11, title: "bilibili.com/video/BV1xx411c7mD", src: "bilibili.com/video/BV1xx411c7mD",
      url: "https://www.bilibili.com/video/BV1xx411c7mD", plat: "bilibili", eng: .ytDlp, fmt: "", state: .failed, phase: .probe,
      fail: Failure(
        code: .needLogin, message: "这个视频要登录哔哩哔哩才能看",
        raw: "ERROR: [BiliBili] BV1xx411c7mD: This video is only available for registered users.", site: "哔哩哔哩")),
    placement: .phone)

  static let failures: [DownloadItem] = [
    needLogin,
    DownloadItem(
      job: Job(
        id: 12, title: "youtube.com/watch?v=Qn7mG1pR2xE", src: "youtube.com/watch?v=Qn7mG1pR2xE",
        url: "https://www.youtube.com/watch?v=Qn7mG1pR2xE", plat: "youtube", eng: .ytDlp, fmt: "", state: .queue, phase: .probe),
      placement: .phone, hold: .computerOnly),
    DownloadItem(
      job: Job(
        id: 13, title: "vimeo.com/1084537", src: "vimeo.com/1084537", url: "https://vimeo.com/1084537", eng: .ytDlp, fmt: "",
        state: .failed, phase: .probe,
        fail: Failure(code: .network, message: "网络连不上，检查一下网络", raw: "ERROR: Unable to download webpage: <urlopen error [Errno 51] Network is unreachable>")),
      placement: .phone),
  ]

  /// 「等电脑上线」（电脑页离线状态、分享卡片存着的）
  static let waiting: [DownloadItem] = [
    DownloadItem(
      job: Job(
        id: 21, title: "Big Buck Bunny · 预告片", src: "youtube.com/watch?v=YE7VzlLtp-4",
        url: "https://www.youtube.com/watch?v=YE7VzlLtp-4", plat: "youtube", eng: .ytDlp, fmt: "MP4 · 最清晰", dur: "0:33",
        thumb: "bbb-frank", state: .queue, phase: .probe),
      placement: .computer, hold: .waitingComputer),
    DownloadItem(
      job: Job(
        id: 22, title: "Sintel · 原声", src: "bilibili.com/video/BV1qJ411m7Sg",
        url: "https://www.bilibili.com/video/BV1qJ411m7Sg", plat: "bilibili", eng: .ytDlp, fmt: "只要声音 · MP3", dur: "14:48",
        audio: true, state: .queue, phase: .probe),
      placement: .computer, hold: .waitingComputer),
  ]

  // MARK: 素材

  /// 手机上的素材
  static let phoneAssets: [Asset] = [
    { var a = bbb; a.when = "今天 06:55"; return a }(),
    asset(2, .video, "Sintel · 预告片", src: "blender.org", fmt: "MP4", w: 1920, h: 818, dur: 52, mb: 48, thumb: "sintel-3", when: "今天 06:16"),
    asset(3, .video, "Tears of Steel · 4K 片段", src: "blender.org", fmt: "MP4", w: 3840, h: 1714, dur: 192, mb: 412, thumb: "tos-05", when: "今天 03:57"),
    asset(4, .audio, "Elephants Dream · 原声", src: "YouTube", fmt: "MP3", dur: 654, mb: 11, when: "昨天 04:57"),
    asset(5, .video, "Sintel · 完整版短片", src: "哔哩哔哩", fmt: "MP4", w: 1920, h: 818, dur: 888, mb: 312, thumb: "sintel-1", when: "昨天 03:57"),
    asset(6, .video, "Elephants Dream · 片段 1", src: "X", fmt: "MP4", w: 1280, h: 720, dur: 30, mb: 8.4, thumb: "ed-platform", when: "11月22日"),
  ]

  /// 电脑上的素材库（一共 128 项，这里是最新的一页）
  static let computerAssets: [Asset] = [
    asset(128, .video, "Big Buck Bunny · 60fps 4K", src: "YouTube", fmt: "MP4", w: 3840, h: 2160, dur: 634, mb: 640, thumb: "bbb-frank", when: "今天 06:55"),
    asset(127, .video, "Sintel · 预告片", src: "blender.org", fmt: "MP4", w: 1920, h: 818, dur: 52, mb: 48, thumb: "sintel-3", when: "今天 06:16"),
    asset(126, .video, "Tears of Steel · 4K 片段", src: "blender.org", fmt: "MP4", w: 3840, h: 1714, dur: 192, mb: 412, thumb: "tos-05", when: "今天 03:57"),
    asset(125, .audio, "Elephants Dream · 原声", src: "YouTube", fmt: "MP3", dur: 654, mb: 11, when: "昨天 04:57"),
    asset(124, .video, "Sintel · 完整版短片", src: "哔哩哔哩", fmt: "MP4", w: 1920, h: 818, dur: 888, mb: 312, thumb: "sintel-1", when: "昨天 03:57"),
    asset(123, .audio, "Sintel · 原声", src: "哔哩哔哩", fmt: "MP3", dur: 888, mb: 34, when: "昨天 03:53"),
    tearsFull,
    asset(121, .video, "Elephants Dream · 片段 1", src: "X", fmt: "MP4", w: 1280, h: 720, dur: 30, mb: 8.4, thumb: "ed-platform", when: "11月22日"),
    asset(120, .video, "Elephants Dream · 片段 2", src: "X", fmt: "MP4", w: 1280, h: 720, dur: 78, mb: 21, thumb: "ed-emo-proog", when: "11月22日"),
  ]

  static let computerAssetCount = 128

  /// 素材详情：手机上的视频
  static let bbb = Asset(
    id: 1, kind: .video, title: "Big Buck Bunny · 60fps 4K", desc: "一只大兔子和三只捣蛋的啮齿动物。Blender 基金会 2008 年的开放电影。",
    url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ", page: "https://www.youtube.com/watch?v=aqz-KE-bpKQ", src: "YouTube",
    uploader: "Blender", path: "相册", fmt: "MP4", width: 3840, height: 2160, duration: 634, bytes: 640_000_000,
    thumb: "bbb-frank", tags: ["短片", "动画"], createdAt: 0, res: "3840×2160", dur: "10:34", size: "640 MB",
    when: "2026年11月24日 06:55")

  /// 素材详情：电脑上的视频
  static let tearsFull = Asset(
    id: 122, kind: .video, title: "Tears of Steel · 完整版", desc: "一群科学家和战士在阿姆斯特丹试图挽回过去。Blender 基金会 2012 年的开放电影。",
    url: "https://mango.blender.org/tos/master.m3u8", page: "https://mango.blender.org/", src: "blender.org", uploader: "Blender",
    path: "~/Movies/Captae/Tears of Steel · 完整版.mp4", fmt: "MP4", width: 1920, height: 800, duration: 734,
    bytes: 420_000_000, thumb: "tos-01", tags: ["短片"], createdAt: 0, res: "1920×800", dur: "12:14", size: "420 MB",
    when: "2026年11月23日 02:57")

  private static func asset(
    _ id: Int64, _ kind: AssetKind, _ title: String, src: String, fmt: String, w: Int? = nil, h: Int? = nil, dur: Double,
    mb: Double, thumb: String? = nil, when: String
  ) -> Asset {
    Asset(
      id: id, kind: kind, title: title, desc: "", url: "", page: "", src: src, uploader: "", path: "", fmt: fmt, width: w,
      height: h, duration: dur, bytes: Int64(mb * 1_000_000), thumb: thumb, tags: [], createdAt: 0,
      res: w.flatMap { w in h.map { "\(w)×\($0)" } } ?? "", dur: Fmt.clock(dur), size: Fmt.mb(mb), when: when)
  }

  // MARK: 分享卡片

  /// 从别的 App 分享进来的链接（读到以后）
  static let shared = Job(
    id: 31, title: "Tears of Steel · 预告片", src: "mango.blender.org/download/tears_of_steel_trailer.mp4",
    url: "https://mango.blender.org/download/tears_of_steel_trailer.mp4", eng: .aria2, fmt: "MP4 文件", dur: "1:04",
    thumb: "tos-05", size: 96, state: .queue, phase: .probe)
}
#endif
