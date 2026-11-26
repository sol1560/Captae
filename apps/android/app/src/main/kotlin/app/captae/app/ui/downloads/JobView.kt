package app.captae.app.ui.downloads

import app.captae.app.model.DownloadItem
import app.captae.app.model.ErrorCode
import app.captae.app.model.FailAction
import app.captae.app.model.Job
import app.captae.app.model.JobState
import app.captae.app.model.Phase
import app.captae.app.model.Where
import app.captae.app.R
import app.captae.app.ui.parts.Str
import app.captae.app.model.mb
import app.captae.app.model.percent
import app.captae.app.model.speed
import app.captae.glass.controls.Step
import app.captae.glass.icons.Sym
import app.captae.app.ui.parts.Tone

// 下载页的纯逻辑：任务现在显示成哪种样子、步骤时间线、失败时给什么说明和按钮。
// 照桌面端 apps/desktop/src/pages/downloads/model.ts（viewOf、flowOf、stepsOf、failInfo），说法一个字不改；
// 手机上多出来的（「在电脑上下」）在这里加。界面（JobCard、JobDetailSheet）只管画。

/** 一行任务现在显示成哪种样子（桌面端 View，手机上用不到的录直播、选条目先不列） */
enum class JobView { Probe, Pick, Choose, Dl, Merge, Starting, Queue, Pause, Login, Verify, Fail, Done }

/** 同一组里的样子互换时不播状态块的进出（配方 c：带进度条的几种之间不播） */
fun JobView.group(): Int = when (this) {
  JobView.Dl, JobView.Merge, JobView.Pause, JobView.Queue, JobView.Starting -> 1
  JobView.Login, JobView.Verify, JobView.Fail -> 2
  else -> ordinal + 10
}

fun viewOf(j: Job): JobView = if (j.choose && j.state != JobState.Failed && j.state != JobState.Done) JobView.Choose else when (j.state) {
  JobState.Done -> JobView.Done
  JobState.Failed -> when (j.fail?.code?.action) {
    FailAction.Login -> JobView.Login
    FailAction.Verify -> JobView.Verify
    else -> JobView.Fail
  }
  JobState.Pause -> JobView.Pause
  JobState.Queue -> if (j.boost) JobView.Starting else JobView.Queue
  JobState.Run -> when (j.phase) {
    Phase.Probe -> JobView.Probe
    Phase.Pick -> JobView.Pick
    Phase.Merge, Phase.Post -> JobView.Merge
    else -> JobView.Dl
  }
}

/** 网站名字（桌面端 lib/detect.ts siteLabel 的手机版：认得的写名字，认不出写主机名） */
fun siteLabel(j: Job): String = when (j.plat?.lowercase()) {
  "youtube" -> "YouTube"
  "bilibili" -> Str.get(R.string.site_bilibili)
  "x" -> "X"
  "vimeo" -> "Vimeo"
  "douyin" -> Str.get(R.string.site_douyin)
  "tiktok" -> "TikTok"
  "instagram" -> "Instagram"
  else -> j.src.substringBefore('/').removePrefix("www.")
}

/** M3U8 清晰度说明：最高一档「最清晰」、最低一档「省流量」（桌面端 variantNote） */
fun variantNote(k: Int, n: Int): String = when {
  n <= 1 -> ""
  k == 0 -> Str.get(R.string.variant_best)
  k == n - 1 -> Str.get(R.string.variant_saver)
  k == 1 -> Str.get(R.string.variant_hd)
  else -> Str.get(R.string.variant_sd)
}

fun Job.variantName(): String = (vars.getOrNull(vIdx) ?: vars.firstOrNull())?.p ?: ""

/** 保存到哪（桌面端 saveDirOf 的手机版）：电脑上是「影片 › Captae」，手机上视频进相册、声音进 Music/Captae（MediaSaver） */
fun saveDirOf(item: DownloadItem): String = when (item.where) {
  Where.Computer -> Str.get(if (item.job.audio) R.string.dir_music else R.string.dir_movies)
  Where.Phone -> Str.get(if (item.job.audio) R.string.dir_music else R.string.dir_photos)
}

/** 下载下来是什么、最后存成什么（桌面端 formatPair）：M3U8 一律存成 MP4 */
fun formatTo(j: Job): String = when {
  j.eng == app.captae.app.model.Engine.Hls -> "MP4"
  "→" in j.fmt -> j.fmt.substringAfter("→").substringBefore(" · ").trim()
  else -> j.fmt.substringBefore(" · ").removeSuffix(" 文件").ifEmpty { "MP4" }
}

private class Flow(val names: List<String>, val subs: List<String?>, val at: Map<Phase, Int>)

private fun flowOf(item: DownloadItem): Flow {
  val j = item.job
  val dir = saveDirOf(item)
  return when (j.eng) {
    app.captae.app.model.Engine.Hls -> Flow(
      listOf(R.string.step_probe, R.string.step_pick_variant, R.string.step_download, R.string.step_merge_hls, R.string.step_library).map(Str::get),
      listOf(j.src.substringBefore('/'), j.variantName().ifEmpty { null }, null, Str.get(R.string.step_merge_hls_sub), dir),
      mapOf(Phase.Probe to 0, Phase.Pick to 1, Phase.Dl to 2, Phase.Merge to 3, Phase.Post to 3),
    )
    app.captae.app.model.Engine.YtDlp -> Flow(
      listOf(
        Str.get(R.string.step_probe),
        Str.get(R.string.step_pick_quality),
        Str.get(if (j.audio) R.string.step_download_audio else R.string.step_download_video),
        if (j.audio) Str.fmt(R.string.step_convert, formatTo(j)) else Str.get(R.string.step_merge_video),
        Str.get(R.string.step_library),
      ),
      listOf(siteLabel(j), if (j.untitled) null else j.fmt, null, null, dir),
      mapOf(Phase.Probe to 0, Phase.Pick to 1, Phase.Dl to 2, Phase.Merge to 3, Phase.Post to 3),
    )
    app.captae.app.model.Engine.Aria2 -> Flow(
      listOf(R.string.step_connect, R.string.step_download, R.string.step_verify, R.string.step_library).map(Str::get),
      listOf(j.src.substringBefore('/'), Str.get(R.string.step_resumable), null, dir),
      mapOf(Phase.Probe to 0, Phase.Pick to 0, Phase.Dl to 1, Phase.Merge to 2, Phase.Post to 2),
    )
  }
}

/** 步骤时间线（桌面端 stepsOf） */
fun stepsOf(item: DownloadItem, concurrency: Int = 3): List<Step> {
  val j = item.job
  val f = flowOf(item)
  val cur = if (j.state == JobState.Done) f.names.size else f.at[j.phase] ?: 0
  return f.names.mapIndexed { i, label ->
    var sub = f.subs[i]
    var st = when {
      i < cur -> Step.State.Done
      i == cur -> Step.State.Now
      else -> Step.State.Todo
    }
    if (i == cur) {
      when {
        j.state == JobState.Failed -> {
          st = Step.State.Err
          sub = j.fail?.message ?: j.err ?: Str.get(R.string.step_failed)
        }
        j.state == JobState.Queue -> sub = if (j.boost) Str.get(R.string.job_starting) else Str.fmt(R.string.step_queued, concurrency)
        j.state == JobState.Pause -> sub = Str.fmt(R.string.step_paused, mb(j.got))
        j.phase == Phase.Probe -> sub = Str.get(R.string.step_opening)
        j.phase == Phase.Pick -> sub = Str.fmt(R.string.step_pick_hint, j.variantName())
        j.phase == Phase.Merge || j.phase == Phase.Post -> sub = "${j.merge.toInt()}%"
        j.phase == Phase.Dl -> sub = if (j.size > 0) "${j.percent()} · ${speed(j.speed)}" else "${mb(j.got)} · ${speed(j.speed)}"
      }
    }
    Step(label, sub, st)
  }
}

/** 选清晰度的倒计时还剩几秒：真数据从 pickUntil 算（now 每秒变一次），演示数据直接给 */
fun pickLeftOf(item: DownloadItem, now: Long): Int =
  item.job.pickUntil?.let { ((it - now + 999) / 1000).toInt().coerceAtLeast(0) } ?: item.pickLeft

/** 「正在读取视频信息… 6.3s」的秒数：真数据从 probeAt 算 */
fun elapsedOf(item: DownloadItem, now: Long): String? =
  item.elapsed ?: item.job.probeAt?.let { String.format(java.util.Locale.ROOT, "%.1fs", (now - it).coerceAtLeast(0) / 1000.0) }

/** 出错时的说明和下一步（桌面端 failInfo）。手机上要登录的：电脑上有登录状态，下一步是「在电脑上下」（PLAN 1） */
data class FailInfo(val title: String, val hint: String?, val tone: Tone, val sf: String, val action: String?)

fun failInfo(item: DownloadItem, macName: String?): FailInfo {
  val j = item.job
  val f = j.fail
  val title = f?.message ?: j.err ?: Str.get(R.string.fail_generic)
  val site = f?.site ?: siteLabel(j)
  return when (f?.code?.action) {
    FailAction.Login, FailAction.Verify ->
      if (item.where == Where.Phone && macName != null) {
        FailInfo(title, Str.fmt(R.string.fail_login_on_mac_hint, site, macName), Tone.Warn, Sym.lock, Str.get(R.string.ctx_on_mac))
      } else {
        FailInfo(title, Str.fmt(R.string.fail_login_hint, site), Tone.Warn, Sym.lock, Str.fmt(R.string.fail_login_action, site))
      }
    FailAction.None -> FailInfo(title, if (f.code == ErrorCode.Drm) Str.get(R.string.fail_drm_hint) else null, Tone.Err, Sym.xmarkCircle, null)
    FailAction.Retry -> FailInfo(
      title,
      when (f.code) {
        ErrorCode.Geo -> Str.get(R.string.fail_geo_hint)
        ErrorCode.Network -> Str.get(R.string.fail_network_hint)
        else -> null
      },
      Tone.Err,
      if (f.code == ErrorCode.Network) Sym.wifiSlash else Sym.exclamationmarkCircle,
      Str.get(R.string.job_retry),
    )
    FailAction.AutoRetry -> FailInfo(title, Str.get(R.string.fail_rate_hint), Tone.Warn, Sym.clock, Str.get(R.string.job_retry))
    FailAction.ChangeDir -> FailInfo(title, Str.get(R.string.fail_disk_hint), Tone.Err, Sym.exclamationmarkCircle, Str.get(R.string.fail_disk_action))
    FailAction.Repair -> FailInfo(title, Str.get(R.string.fail_tool_hint), Tone.Err, Sym.exclamationmarkCircle, Str.get(R.string.fail_tool_action))
    else -> FailInfo(title, Str.get(R.string.fail_unknown_hint), Tone.Err, Sym.exclamationmarkCircle, Str.get(R.string.job_retry))
  }
}
