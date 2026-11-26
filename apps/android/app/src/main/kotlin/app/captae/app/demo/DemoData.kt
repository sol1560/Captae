package app.captae.app.demo

import app.captae.app.model.Asset
import app.captae.app.model.Computer
import app.captae.app.model.DownloadItem
import app.captae.app.model.Engine
import app.captae.app.model.ErrorCode
import app.captae.app.model.Failure
import app.captae.app.model.Job
import app.captae.app.model.JobState
import app.captae.app.model.Phase
import app.captae.app.model.Variant
import app.captae.app.model.Where

/**
 演示数据：照 work/mobile/design/BRIEF.md 第 3 节，和 iOS 版用同一套标题、数字和说法。
 接上 Rust 核心以后这里只给设计稿和调试用。
 */
object DemoData {
  const val PHONE_NAME = "Pixel 9"

  val mac = Computer(name = "Sol 的 MacBook Pro", online = true, version = "0.2.0", running = 3, lastSeen = "刚刚")
  val macOffline = mac.copy(online = false, running = 0, lastSeen = "今天 14:02")

  // ---------------- 下载任务 ----------------

  private val probing = Job(
    id = 1, title = "youtube.com/watch?v=aqz-KE-bpKQ", src = "youtube.com/watch?v=aqz-KE-bpKQ",
    url = "https://www.youtube.com/watch?v=aqz-KE-bpKQ", plat = "youtube", eng = Engine.YtDlp, fmt = "MP4",
    state = JobState.Run, phase = Phase.Probe,
  )

  private val tosVariants = listOf(
    Variant(r = "3840×1714", p = "2160p", b = 15_000.0, mb = 1400.0, uri = "2160/index.m3u8"),
    Variant(r = "1920×856", p = "1080p", b = 4_500.0, mb = 420.0, uri = "1080/index.m3u8"),
    Variant(r = "1280×534", p = "720p", b = 2_250.0, mb = 210.0, uri = "720/index.m3u8"),
    Variant(r = "854×356", p = "480p", b = 1_200.0, mb = 112.0, uri = "480/index.m3u8"),
  )

  private val picking = Job(
    id = 2, title = "Tears of Steel · 完整版", src = "mango.blender.org/tos/master.m3u8",
    url = "https://mango.blender.org/tos/master.m3u8", eng = Engine.Hls, fmt = "M3U8 → MP4", dur = "12:14",
    thumb = "still:tos_01", state = JobState.Run, phase = Phase.Pick, vars = tosVariants, vIdx = 0,
  )

  private val sintel = Job(
    id = 3, title = "Sintel · 完整版短片", src = "bilibili.com/video/BV1qJ411m7Sg",
    url = "https://www.bilibili.com/video/BV1qJ411m7Sg", plat = "bilibili", eng = Engine.YtDlp, fmt = "MP4 · 1080p", dur = "14:48",
    thumb = "still:sintel_1", size = 312.0, got = 143.0, speed = 6.4, state = JobState.Run, phase = Phase.Dl,
  )

  private val elephants = Job(
    id = 4, title = "Elephants Dream", src = "cdn.example.org/ed/1080/index.m3u8",
    url = "https://cdn.example.org/ed/1080/index.m3u8", eng = Engine.Hls, fmt = "M3U8 → MP4", dur = "10:54",
    thumb = "still:ed_emo_proog", size = 286.0, got = 107.0, speed = 4.8, state = JobState.Run, phase = Phase.Dl,
  )

  private val tosTrailer = Job(
    id = 5, title = "Tears of Steel · 预告片", src = "mango.blender.org/download/tears_of_steel_trailer.mp4",
    url = "https://mango.blender.org/download/tears_of_steel_trailer.mp4", eng = Engine.Aria2, fmt = "MP4 文件", dur = "1:04",
    thumb = "still:tos_05", size = 96.0, got = 62.0, state = JobState.Pause, phase = Phase.Dl,
  )

  private val sintelTrailer = Job(
    id = 6, title = "Sintel · 预告片", src = "download.blender.org/durian/trailer/sintel_trailer-1080p.mp4",
    url = "https://download.blender.org/durian/trailer/sintel_trailer-1080p.mp4", eng = Engine.Aria2, fmt = "MP4 文件", dur = "0:52",
    thumb = "still:sintel_3", state = JobState.Queue, phase = Phase.Probe,
  )

  /** 01-list：「在电脑上」4 个、「在手机上」2 个 */
  val downloads: List<DownloadItem> = listOf(
    DownloadItem(probing, Where.Computer, elapsed = "6.3s"),
    DownloadItem(picking, Where.Computer, pickLeft = 3),
    DownloadItem(sintel, Where.Computer),
    DownloadItem(elephants, Where.Computer, note = "网络有点不稳，正在自动重试", noteWarn = true),
    DownloadItem(tosTrailer, Where.Phone),
    DownloadItem(sintelTrailer, Where.Phone, queueAhead = 1),
  )

  /** 要登录哔哩哔哩的那条（01-errors、02-failed） */
  val needLogin = DownloadItem(
    Job(
      id = 11, title = "bilibili.com/video/BV1xx411c7mD", src = "bilibili.com/video/BV1xx411c7mD",
      url = "https://www.bilibili.com/video/BV1xx411c7mD", plat = "bilibili", eng = Engine.YtDlp, fmt = "MP4",
      state = JobState.Failed, phase = Phase.Probe,
      fail = Failure(ErrorCode.NeedLogin, "这个视频要登录哔哩哔哩才能看", site = "哔哩哔哩"),
    ),
    Where.Phone,
  )

  /**
   受版权保护、下不了的那条（01-errors）。
   iOS 那边这个位置是「YouTube 的视频在电脑上下」（上架版手机上不下 YouTube）；安卓是完整版，手机上能下 YouTube，
   这条说法不成立，换成同样「没有下一步按钮」的 DRM（errors.rs 的 Drm 说法），三条出错状态各是一种样子。
   */
  val drm = DownloadItem(
    Job(
      id = 12, title = "tv.example.com/watch/2041", src = "tv.example.com/watch/2041",
      url = "https://tv.example.com/watch/2041", eng = Engine.YtDlp, fmt = "MP4",
      state = JobState.Failed, phase = Phase.Probe,
      fail = Failure(ErrorCode.Drm, "这个视频受版权保护（DRM），Captae 不能下载"),
    ),
    Where.Phone,
  )

  /** 网络连不上的那条（01-errors）：手机自己没网，说法照 BRIEF */
  val offlineNet = DownloadItem(
    Job(
      id = 13, title = "vimeo.com/1084537", src = "vimeo.com/1084537", url = "https://vimeo.com/1084537", plat = "vimeo",
      eng = Engine.YtDlp, fmt = "MP4", state = JobState.Failed, phase = Phase.Probe,
      fail = Failure(ErrorCode.Network, "网络连不上，检查一下网络"),
    ),
    Where.Phone,
  )

  /** 01-errors：3 条出错 + 1 条正常 */
  val errorDownloads: List<DownloadItem> = listOf(
    DownloadItem(sintel, Where.Computer),
    needLogin,
    drm,
    offlineNet,
  )

  val sintelRunning: DownloadItem get() = downloads[2]
  val tosPicking: DownloadItem get() = downloads[1]

  // ---------------- 素材 ----------------

  val bbb = Asset(
    id = 101, kind = "video", title = "Big Buck Bunny · 60fps 4K",
    desc = "一只大兔子和三只捣蛋的啮齿动物。Blender 基金会 2008 年的开放电影。",
    url = "youtube.com/watch?v=aqz-KE-bpKQ", src = "YouTube", plat = "youtube", fmt = "MP4", width = 3840, height = 2160,
    dur = "10:34", size = "640 MB", thumb = "still:bbb_frank", tags = listOf("短片", "动画"),
    `when` = "今天 06:55", downloadedAt = "2026年11月24日 06:55", location = "相册 › Captae",
  )

  /** 手机上的素材（03-grid） */
  val phoneAssets: List<Asset> = listOf(
    bbb,
    Asset(id = 102, kind = "video", title = "Sintel · 预告片", src = "blender.org", fmt = "MP4", width = 1920, height = 818, dur = "0:52", size = "38 MB", thumb = "still:sintel_3", `when` = "今天 06:16"),
    Asset(id = 103, kind = "video", title = "Tears of Steel · 4K 片段", src = "blender.org", fmt = "MP4", width = 3840, height = 1714, dur = "3:12", size = "512 MB", thumb = "still:tos_05", `when` = "今天 03:57"),
    Asset(id = 104, kind = "audio", title = "Elephants Dream · 原声", src = "YouTube", plat = "youtube", fmt = "MP3", dur = "10:54", size = "15 MB", `when` = "昨天 04:57"),
    Asset(id = 105, kind = "video", title = "Sintel · 完整版短片", src = "哔哩哔哩", plat = "bilibili", fmt = "MP4", width = 1920, height = 818, dur = "14:48", size = "312 MB", thumb = "still:sintel_1", `when` = "昨天 03:57"),
    Asset(id = 106, kind = "video", title = "Elephants Dream · 片段 1", src = "X", plat = "x", fmt = "MP4", width = 1280, height = 720, dur = "0:30", size = "9.6 MB", thumb = "still:ed_platform", `when` = "11月22日"),
  )

  /** 电脑上的素材库（03-offline 变灰的那一屏；标题照 apps/desktop/src/dev/mock-library.ts） */
  val macAssets: List<Asset> = listOf(
    bbb.copy(location = "影片 › Captae"),
    phoneAssets[1],
    phoneAssets[2],
    phoneAssets[3],
    phoneAssets[4],
    Asset(id = 107, kind = "audio", title = "Sintel · 原声", src = "哔哩哔哩", plat = "bilibili", fmt = "MP3", dur = "14:48", size = "21 MB", `when` = "昨天 03:53"),
    Asset(id = 108, kind = "video", title = "Tears of Steel · 完整版", src = "blender.org", fmt = "MP4", width = 3840, height = 1714, dur = "12:14", size = "1.40 GB", thumb = "still:tos_01", `when` = "昨天 02:57"),
    phoneAssets[5],
  )

  const val MAC_ASSET_COUNT = 128

  /** 04-remote / 04-saving：电脑上的那一条 */
  val remoteAsset = macAssets[6].copy(
    desc = "Blender 基金会 2012 年的开放电影，真人和特效合成的科幻短片。",
    url = "mango.blender.org/tos/master.m3u8", downloadedAt = "2026年11月23日 02:57", location = "影片 › Captae · Sol 的 MacBook Pro",
  )

  // ---------------- 电脑 ----------------

  /** 05-offline：等电脑上线、回到同一个 Wi-Fi 后自动发过去的两条 */
  val waitingForMac: List<Pair<String, String>> = listOf(
    "Big Buck Bunny · 60fps 4K" to "youtube.com/watch?v=aqz-KE-bpKQ",
    "Sintel · 完整版短片" to "bilibili.com/video/BV1qJ411m7Sg",
  )

  // ---------------- 空页面「试试这些链接」（同桌面端 pages/Downloads.tsx 的 TRIES：Blender 开放电影） ----------------

  data class Try(val label: String, val url: String, val sf: String?, val site: String?)

  val tries = listOf(
    Try("Big Buck Bunny", "https://www.youtube.com/watch?v=aqz-KE-bpKQ", null, "youtube"),
    Try("Sintel 预告片", "https://download.blender.org/durian/trailer/sintel_trailer-1080p.mp4", "film", null),
    Try("M3U8 测试流", "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8", "list.and.film", null),
  )
}
