package app.captae.app.model

import androidx.compose.runtime.Immutable

// 和 Rust 核心（crates/captae-core/src/model.rs、errors.rs、library.rs）对得上的数据类。
// 字段名、取值和 JSON 里的一样（wire 是序列化时的字符串）。真数据从 UniFFI 绑定转过来（core/Convert.kt），
// 演示数据（demo/DemoData.kt）只在 debug 包加 captaeDemo 启动参数时用。

/** 任务状态（model.rs JobState，serde 小写） */
enum class JobState(val wire: String) {
  /** 等待空位 */
  Queue("queue"),
  /** 正在跑 */
  Run("run"),
  /** 用户暂停 */
  Pause("pause"),
  Done("done"),
  Failed("failed");

  val isActive: Boolean get() = this == Queue || this == Run || this == Pause
}

/** 进行到哪一步（model.rs Phase） */
enum class Phase(val wire: String) {
  /** 读取视频信息 */
  Probe("probe"),
  /** 等用户选清晰度（M3U8 多码率时） */
  Pick("pick"),
  /** 下载中 */
  Dl("dl"),
  /** 合并分片 / 生成视频文件 */
  Merge("merge"),
  /** 下载后处理：转格式 */
  Post("post"),
  Done("done"),
}

/** 下载引擎（model.rs Engine） */
enum class Engine(val wire: String) { YtDlp("yt-dlp"), Aria2("aria2"), Hls("M3U8") }

/** 出错以后界面给的下一步（errors.rs ErrorCode::action） */
enum class FailAction(val wire: String) {
  Login("login"),
  Verify("verify"),
  None("none"),
  Retry("retry"),
  UseExtension("use_extension"),
  WaitLive("wait_live"),
  RecordLive("record_live"),
  AutoRetry("auto_retry"),
  ChangeDir("change_dir"),
  Repair("repair"),
  CopyError("copy_error"),
}

/** 15 个错误码（errors.rs ErrorCode / ALL_CODES），action 和 Rust 的 action() 一一对应 */
enum class ErrorCode(val wire: String, val action: FailAction) {
  /** 年龄限制、会员、私密、要登录 */
  NeedLogin("need_login", FailAction.Login),
  /** 机器人检测、Cloudflare、抖音要新 cookies */
  NeedVerify("need_verify", FailAction.Verify),
  /** DRM 保护 */
  Drm("drm", FailAction.None),
  /** 地区限制 */
  Geo("geo", FailAction.Retry),
  /** 证书不对、被网络拦截 */
  Blocked("blocked", FailAction.Retry),
  /** 已删除、不存在 */
  Unavailable("unavailable", FailAction.None),
  /** 网站不认识 */
  Unsupported("unsupported", FailAction.UseExtension),
  /** 页面里没有媒体 */
  NoMedia("no_media", FailAction.None),
  /** 直播还没开始 */
  LiveUpcoming("live_upcoming", FailAction.WaitLive),
  /** 正在直播 */
  Live("live", FailAction.RecordLive),
  /** 429、请求太频繁 */
  RateLimited("rate_limited", FailAction.AutoRetry),
  /** 超时、断线、DNS */
  Network("network", FailAction.Retry),
  /** 磁盘满、没有写入权限 */
  Disk("disk", FailAction.ChangeDir),
  /** 组件缺失或损坏 */
  Tool("tool", FailAction.Repair),
  /** 其他 */
  Unknown("unknown", FailAction.CopyError);

  companion object {
    fun parse(s: String): ErrorCode? = entries.firstOrNull { it.wire == s }
  }
}

/** 失败的说明（errors.rs Failure）：message 是给人看的那句话，site 是说法里用的网站名字（「哔哩哔哩」「YouTube」） */
@Immutable
data class Failure(
  val code: ErrorCode,
  val message: String,
  val raw: String = "",
  val startsAt: Long? = null,
  val site: String? = null,
)

/** M3U8 的一档清晰度（model.rs Variant）：r 分辨率、p 「1080p」、b 码率、mb 估算大小 */
@Immutable
data class Variant(val r: String, val p: String, val b: Double, val mb: Double, val uri: String, val audio: String? = null)

/** 下载记录的一行（model.rs LogLine）：时间戳（毫秒）、文字、级别 */
@Immutable
data class LogLine(val at: Long, val text: String, val level: String = "info")

/**
 一个下载任务（model.rs Job，只留界面要用的字段；大小、已下载以 MB 计，速度 MB/s，和桌面端界面一样）。
 thumb 是封面：真数据是文件路径；演示数据写成 `still:<名字>`（res/drawable-nodpi/still_<名字>.jpg）。
 */
@Immutable
data class Job(
  val id: Long,
  val title: String,
  /** 来源（主机名 + 路径，界面上显示的那一截） */
  val src: String,
  val url: String,
  /** 平台（youtube、bilibili……），认不出是空 */
  val plat: String? = null,
  val eng: Engine,
  /** 格式说明：「MP4 · 1080p」「MP4 文件」「M3U8 → MP4」 */
  val fmt: String,
  /** 时长「14:48」 */
  val dur: String = "",
  val thumb: String? = null,
  val audio: Boolean = false,
  val size: Double = 0.0,
  val got: Double = 0.0,
  val speed: Double = 0.0,
  val state: JobState,
  val phase: Phase,
  val pri: Long = 0,
  val boost: Boolean = false,
  val err: String? = null,
  val fail: Failure? = null,
  val choose: Boolean = false,
  val retryAt: Long? = null,
  val autoRetries: Int = 0,
  val needsLogin: Boolean = false,
  val live: Boolean = false,
  val vars: List<Variant> = emptyList(),
  val vIdx: Int = 0,
  /** 选清晰度倒计时结束的时刻（毫秒） */
  val pickUntil: Long? = null,
  /** 合并、转格式的进度（0–100） */
  val merge: Double = 0.0,
  val path: String? = null,
  val assetId: Long? = null,
  val log: List<LogLine> = emptyList(),
  /** 这一次读取信息从什么时候开始（毫秒，「正在读取视频信息… 6.3s」） */
  val probeAt: Long? = null,
  /** 核心说标题还是占位（真数据才有；演示数据按标题猜） */
  val untitledFlag: Boolean = false,
  /** 播放列表：等用户选的条目 */
  val entries: List<Entry> = emptyList(),
  /** 正在录直播（只能「停止并保存」，不能暂停） */
  val recording: Boolean = false,
) {
  val progress: Float get() = if (size > 0) (got / size).coerceIn(0.0, 1.0).toFloat() else 0f

  /** 还没有真标题：标题就是网址（桌面端 model.ts 的 untitled） */
  val untitled: Boolean get() = untitledFlag || title.isEmpty() || title == src || title == url || title.endsWith("正在读取标题…")
}

/** 播放列表里的一条（model.rs Entry） */
@Immutable
data class Entry(val id: String, val title: String, val dur: String, val thumb: String?, val index: Int)

/** 任务在哪儿跑：连着的电脑上（远程控制）还是这台手机上 */
enum class Where { Computer, Phone }

/**
 下载页上的一行：任务本身 + 手机端才有的说明。
 - note：状态行下面的一句补充（「网络有点不稳，正在自动重试」），warn 时用橙色小字；
 - queueAhead：排队时前面还有几个；
 - pickLeft：选清晰度倒计时还剩几秒（真数据从 pickUntil 算，演示数据直接给）。
 */
@Immutable
data class DownloadItem(
  val job: Job,
  val where: Where,
  val note: String? = null,
  val noteWarn: Boolean = false,
  val queueAhead: Int = 0,
  val pickLeft: Int = 0,
  val elapsed: String? = null,
  /** 电脑上的任务：哪台电脑（RemoteHub 的设备 id，证书指纹） */
  val device: String? = null,
  /** 「等电脑上线」队列里的一条（还没发到电脑）：值是队列里的 id */
  val pendingId: Long? = null,
)

/** 素材（library.rs Asset，只留界面要用的字段）；kind 是 video / audio / image */
@Immutable
data class Asset(
  val id: Long,
  val kind: String,
  val title: String,
  val desc: String = "",
  val url: String = "",
  val src: String,
  val plat: String? = null,
  val fmt: String,
  val width: Int? = null,
  val height: Int? = null,
  val dur: String = "",
  val size: String = "",
  val thumb: String? = null,
  val tags: List<String> = emptyList(),
  /** 「今天 06:55」（列表上的） */
  val `when`: String = "",
  /** 「2026年11月24日 06:55」（详情里的） */
  val downloadedAt: String = "",
  /** 保存在哪：「相册 › Captae」「影片 › Captae」 */
  val location: String = "",
  /** 文件名（真数据才有：打开、分享时用来猜类型） */
  val fileName: String = "",
  /** 电脑上的素材：哪台电脑（RemoteHub 的设备 id） */
  val device: String? = null,
) {
  val res: String get() = if (width != null && height != null) "$width×$height" else ""
}

/** 配对过的电脑 */
@Immutable
data class Computer(
  val name: String,
  val online: Boolean,
  /** 电脑上 Captae 的版本 */
  val version: String,
  /** 电脑上正在下的任务数 */
  val running: Int = 0,
  /** 上次在线：「今天 14:02」 */
  val lastSeen: String = "",
  /** RemoteHub 的设备 id（真数据才有） */
  val id: String = "",
  /** 被电脑撤销了、令牌没了：要重新配对 */
  val needsPairing: Boolean = false,
)
