package app.captae.app.core

import android.text.format.DateFormat
import android.text.format.DateUtils
import app.captae.app.R
import app.captae.app.model.Asset
import app.captae.app.model.DownloadItem
import app.captae.app.model.Engine
import app.captae.app.model.Entry
import app.captae.app.model.ErrorCode
import app.captae.app.model.Failure
import app.captae.app.model.Job
import app.captae.app.model.JobState
import app.captae.app.model.Phase
import app.captae.app.model.Variant
import app.captae.app.model.Where
import app.captae.app.model.mb
import app.captae.app.ui.downloads.platOf
import app.captae.app.ui.parts.Str
import uniffi.captae_mobile.MobileAsset
import uniffi.captae_mobile.MobileFailure
import uniffi.captae_mobile.MobileJob
import uniffi.captae_mobile.MobileJobState
import java.io.File
import java.util.Calendar
import java.util.Date
import java.util.Locale
import kotlin.math.roundToInt

// Rust 核心的类型（UniFFI 生成）→ 界面用的数据类（model/Model.kt）。界面里大小按 MB、速度按 MB/s（和桌面端界面一样），核心给的是字节。

private const val MB = 1_000_000.0

fun MobileFailure.toFailure(): Failure =
  Failure(code = ErrorCode.parse(code) ?: ErrorCode.Unknown, message = message, raw = raw, startsAt = startsAt, site = site)

private fun phaseOf(s: String): Phase = Phase.entries.firstOrNull { it.wire == s } ?: Phase.Dl

private fun engineOf(s: String): Engine = when (s.lowercase()) {
  "yt-dlp" -> Engine.YtDlp
  "aria2" -> Engine.Aria2
  else -> Engine.Hls
}

/** 秒 → 「10:34」「1:02:03」 */
fun clock(seconds: Double?): String {
  val s = (seconds ?: return "").roundToInt().coerceAtLeast(0)
  return if (s >= 3600) "%d:%02d:%02d".format(Locale.ROOT, s / 3600, (s % 3600) / 60, s % 60) else "%d:%02d".format(Locale.ROOT, s / 60, s % 60)
}

/** local：这台手机上的任务（下完有自己取的封面）；电脑上的任务 thumb 是网上的封面地址 */
fun MobileJob.toJob(local: Boolean = true): Job = Job(
  id = id.toLong(),
  title = title,
  src = src,
  url = url,
  plat = plat,
  eng = engineOf(engine),
  fmt = format,
  dur = duration,
  // 下完以后素材有自己取的那一帧封面（核心在手机上没有 ffmpeg，读到的封面网址不一定有）
  thumb = (if (local) assetId?.let { MediaSaver.thumbFor(it) } else null) ?: thumb,
  audio = audioOnly,
  size = totalBytes.toDouble() / MB,
  got = gotBytes.toDouble() / MB,
  speed = speed.toDouble() / MB,
  state = when (state) {
    MobileJobState.QUEUED -> JobState.Queue
    MobileJobState.RUNNING -> JobState.Run
    MobileJobState.PAUSED -> JobState.Pause
    MobileJobState.DONE -> JobState.Done
    MobileJobState.FAILED -> JobState.Failed
  },
  phase = phaseOf(phase),
  fail = failure?.toFailure(),
  choose = choose,
  retryAt = retryAt,
  live = live,
  vars = variants.map { Variant(r = it.resolution, p = it.label, b = it.mbps, mb = it.bytes.toDouble() / MB, uri = "") },
  vIdx = variantIndex.toInt(),
  pickUntil = pickUntil,
  merge = mergePercent,
  path = path,
  assetId = assetId,
  probeAt = probeAt,
  untitledFlag = untitled,
  entries = entries.map { Entry(it.id, it.title, clock(it.duration), it.thumb, it.index.toInt()) },
  recording = recording,
)

/** 这台手机上的全部任务 → 下载页的行（新加的在最上面，同桌面端：新任务从上方进来，配方 c1） */
fun phoneItems(jobs: Collection<MobileJob>): List<DownloadItem> {
  // 排队的「前面还有几个」按加的先后算
  val ahead = jobs.filter { it.state == MobileJobState.QUEUED }.sortedBy { it.id }.withIndex().associate { (i, m) -> m.id to i }
  return jobs.sortedByDescending { it.id }.map { m ->
    val j = m.toJob()
    val ahead = ahead[m.id] ?: 0
    // 限流以后等着自动重试：状态行下面一行橙色小字（BRIEF 3 的「网络有点不稳，正在自动重试」）
    val retrying = m.retryAt != null && m.state != MobileJobState.FAILED && m.state != MobileJobState.DONE
    DownloadItem(j, Where.Phone, note = if (retrying) Str.get(R.string.job_auto_retry) else null, noteWarn = retrying, queueAhead = ahead)
  }
}

/** 「今天 06:55」「昨天 04:57」「11月22日」（素材卡上的） */
fun shortWhen(ms: Long): String {
  val now = System.currentTimeMillis()
  val time = DateFormat.getTimeFormat(Str.ctx).format(Date(ms))
  return when {
    DateUtils.isToday(ms) -> Str.fmt(R.string.time_today, time)
    DateUtils.isToday(ms + DateUtils.DAY_IN_MILLIS) -> Str.fmt(R.string.time_yesterday, time)
    else -> {
      val y = Calendar.getInstance().apply { timeInMillis = ms }.get(Calendar.YEAR) == Calendar.getInstance().apply { timeInMillis = now }.get(Calendar.YEAR)
      DateFormat.format(DateFormat.getBestDateTimePattern(Locale.getDefault(), if (y) "MMMd" else "yyyyMMMd"), ms).toString()
    }
  }
}

/** 「2026年11月24日 06:55」（素材详情里的） */
fun longWhen(ms: Long): String =
  DateFormat.format(DateFormat.getBestDateTimePattern(Locale.getDefault(), "yyyyMMMdHHmm"), ms).toString()

/** remote：电脑上的素材（id 是电脑那边的，手机这边存放的位置不适用） */
fun MobileAsset.toAsset(remote: Boolean = false): Asset {
  val place = if (remote) null else MediaSaver.placeOf(id)
  val location = when (place) {
    // Movies/Captae、Pictures/Captae 在相册里是同一个「Captae」相册
    MediaSaver.Place.Movies, MediaSaver.Place.Pictures -> Str.get(R.string.dir_photos)
    MediaSaver.Place.Music -> Str.get(R.string.dir_music)
    null -> ""
  }
  return Asset(
    id = id,
    kind = kind,
    title = title,
    desc = description,
    url = (pageUrl.ifEmpty { url }).removePrefix("https://").removePrefix("http://").removePrefix("www."),
    src = site,
    plat = platOf(pageUrl.ifEmpty { url }),
    fmt = format,
    width = width?.toInt(),
    height = height?.toInt(),
    dur = clock(duration),
    size = mb(bytes.toDouble() / MB),
    thumb = (if (remote) null else MediaSaver.thumbFor(id)) ?: thumb,
    tags = tags,
    `when` = shortWhen(createdAt),
    downloadedAt = longWhen(createdAt),
    location = location,
    fileName = File(path).name,
  )
}

// ------------------------------------------------------------------ 遥控电脑（RemoteHub）

/** 电脑 → 界面上的「电脑」 */
fun uniffi.captae_mobile.MobileDevice.toComputer(): app.captae.app.model.Computer = app.captae.app.model.Computer(
  name = name,
  // 正在连（刚回到前台、刚配好）也当在线显示：不然每次打开 App 都先闪一下「现在连不上」；真连不上时 RemoteHub 会改成 OFFLINE
  online = status == uniffi.captae_mobile.DeviceStatus.ONLINE || status == uniffi.captae_mobile.DeviceStatus.CONNECTING,
  version = "",
  running = running.toInt(),
  lastSeen = lastSeen?.let { shortWhen(it) }.orEmpty(),
  id = id,
  needsPairing = status == uniffi.captae_mobile.DeviceStatus.REVOKED || status == uniffi.captae_mobile.DeviceStatus.NEEDS_PAIRING,
)

/** 一台电脑上的任务 + 等它上线的 → 下载页的行（新的在上面；等上线的排在最上面，它们是这台手机刚交过去的） */
fun computerItems(g: uniffi.captae_mobile.JobGroup): List<DownloadItem> {
  val waiting = g.pending.sortedByDescending { it.id }.map { p ->
    val short = p.url.removePrefix("https://").removePrefix("http://").removePrefix("www.")
    DownloadItem(
      Job(
        id = -(p.id.toLong()) - 1, title = p.title ?: short, src = short, url = p.url, plat = platOf(p.url), eng = Engine.YtDlp,
        fmt = p.format.orEmpty(), state = JobState.Queue, phase = Phase.Probe, untitledFlag = p.title == null,
        fail = p.error?.toFailure(),
      ),
      Where.Computer,
      note = p.error?.message ?: Str.get(R.string.job_waiting_mac),
      noteWarn = p.error != null,
      device = g.device.id,
      pendingId = p.id.toLong(),
    )
  }
  val jobs = g.jobs.sortedByDescending { it.job.id }.map { dj ->
    val retrying = dj.job.retryAt != null && dj.job.state != MobileJobState.FAILED && dj.job.state != MobileJobState.DONE
    DownloadItem(dj.job.toJob(local = false), Where.Computer, note = if (retrying) Str.get(R.string.job_auto_retry) else null, noteWarn = retrying, device = g.device.id)
  }
  return waiting + jobs
}
