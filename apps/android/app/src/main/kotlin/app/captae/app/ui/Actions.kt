package app.captae.app.ui

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.util.Log
import androidx.compose.runtime.staticCompositionLocalOf
import app.captae.app.R
import app.captae.app.core.CaptaeRuntime
import app.captae.app.core.RemoteRuntime
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.launch
import uniffi.captae_mobile.SendOptions
import app.captae.app.core.MediaSaver
import app.captae.app.model.DownloadItem
import app.captae.app.model.Where
import app.captae.app.ui.downloads.FmtChoice
import app.captae.app.ui.downloads.platOf
import uniffi.captae_mobile.AddOptions
import uniffi.captae_mobile.MobileAction

// 页面上的按钮要做的事。页面只管画，点了以后调这里（CompositionLocal 往下传，不用一层层传回调）。
// 演示数据（captaeDemo）时是什么都不做的那一份，截图时点了不会变。

/** 对一个任务的操作（和核心的 MobileAction 对应；电脑上的任务等远程控制接上以后走 JobAction） */
enum class JobOp { Pause, Resume, Cancel, Remove, Retry, Now, PickGo, Stop }

interface JobActions {
  fun act(item: DownloadItem, op: JobOp) {}
  fun pickVariant(item: DownloadItem, index: Int) {}
  fun chooseEntries(item: DownloadItem, ids: List<String>) {}
  fun copyLink(item: DownloadItem) {}
  /** 出错状态行右边的按钮（重试、打开设置、在电脑上下、复制错误信息） */
  fun failAction(item: DownloadItem) {}
  /** 长按菜单「在电脑上下 / 在手机上下」 */
  fun moveTo(item: DownloadItem) {}
}

object NoJobActions : JobActions

val LocalJobActions = staticCompositionLocalOf<JobActions> { NoJobActions }

/**
 下载页的按钮：这台手机上的任务交给本机核心，电脑上的交给 RemoteHub（REMOTE.md 第 6、7 节）。
 等电脑上线的（pendingId）：取消 = 从队列里去掉，「在手机上下」= moveToPhone。
 */
class AppJobActions(
  private val context: Context,
  private val scope: CoroutineScope,
  private val onOpenSettings: () -> Unit,
  private val onOpenComputers: () -> Unit,
  /** 发到哪台电脑（没配对时是空的） */
  private val computer: () -> String?,
) : JobActions {
  private fun core(op: JobOp) = when (op) {
    JobOp.Pause -> MobileAction.PAUSE
    JobOp.Resume -> MobileAction.RESUME
    JobOp.Cancel -> MobileAction.CANCEL
    JobOp.Remove -> MobileAction.REMOVE
    JobOp.Retry -> MobileAction.RETRY
    JobOp.Now -> MobileAction.NOW
    JobOp.PickGo -> MobileAction.PICK_GO
    JobOp.Stop -> MobileAction.STOP
  }

  private fun remote(block: suspend (hub: uniffi.captae_mobile.RemoteHub) -> Unit) {
    val hub = RemoteRuntime.hub ?: return
    scope.launch {
      try { block(hub) } catch (e: Exception) { Log.w("Captae", "遥控电脑失败", e) }
    }
  }

  override fun act(item: DownloadItem, op: JobOp) {
    val pid = item.pendingId
    if (pid != null) {
      if (op == JobOp.Cancel || op == JobOp.Remove) RemoteRuntime.hub?.removePending(pid.toULong())
      else if (op == JobOp.Retry) RemoteRuntime.hub?.retryPending(pid.toULong())
      return
    }
    val dev = item.device
    if (item.where == Where.Computer && dev != null) remote { it.remoteAction(dev, item.job.id.toULong(), core(op)) }
    else CaptaeRuntime.act(item.job.id.toULong(), core(op))
  }

  override fun pickVariant(item: DownloadItem, index: Int) {
    val dev = item.device
    if (item.where == Where.Computer && dev != null) remote { it.remoteSetVariant(dev, item.job.id.toULong(), index.toUInt()) }
    else CaptaeRuntime.setVariant(item.job.id.toULong(), index)
  }

  override fun chooseEntries(item: DownloadItem, ids: List<String>) {
    val dev = item.device
    if (item.where == Where.Computer && dev != null) remote { it.remoteChooseEntries(dev, item.job.id.toULong(), ids) }
    else CaptaeRuntime.chooseEntries(item.job.id.toULong(), ids)
  }

  override fun copyLink(item: DownloadItem) = copy(context, item.job.url)

  override fun failAction(item: DownloadItem) {
    val f = item.job.fail ?: return act(item, JobOp.Retry)
    when (f.code.action) {
      // 要登录、要验证：手机上没有登录状态，交给电脑下（没配对就去配一台）
      app.captae.app.model.FailAction.Login, app.captae.app.model.FailAction.Verify -> if (item.where == Where.Phone && computer() != null) moveTo(item) else onOpenComputers()
      app.captae.app.model.FailAction.Repair, app.captae.app.model.FailAction.ChangeDir -> onOpenSettings()
      app.captae.app.model.FailAction.CopyError -> copy(context, f.raw.ifEmpty { f.message })
      app.captae.app.model.FailAction.None -> {}
      else -> act(item, JobOp.Retry)
    }
  }

  /** 长按菜单「在电脑上下 / 在手机上下」：在另一边加上，这边的取消 */
  override fun moveTo(item: DownloadItem) {
    val j = item.job
    val pid = item.pendingId
    if (pid != null) { RemoteRuntime.hub?.moveToPhone(listOf(pid.toULong())); return }
    if (item.where == Where.Phone) {
      val dev = computer() ?: return onOpenComputers()
      remote { hub ->
        hub.send(dev, SendOptions(url = j.url, quality = null, format = null, audioOnly = j.audio, preset = null, title = if (j.untitled) null else j.title))
        CaptaeRuntime.act(j.id.toULong(), if (j.state == app.captae.app.model.JobState.Failed) MobileAction.REMOVE else MobileAction.CANCEL)
      }
    } else {
      val dev = item.device ?: return
      try {
        CaptaeRuntime.add(AddOptions(url = j.url, quality = null, format = null, audioOnly = j.audio, pageUrl = null, pageTitle = null))
        remote { it.remoteAction(dev, j.id.toULong(), if (j.state == app.captae.app.model.JobState.Failed) MobileAction.REMOVE else MobileAction.CANCEL) }
      } catch (e: Exception) {
        Log.w("Captae", "改到手机上下失败", e)
      }
    }
  }
}

fun copy(context: Context, text: String) {
  val cm = context.getSystemService(ClipboardManager::class.java)
  cm.setPrimaryClip(ClipData.newPlainText("Captae", text))
}

/**
 输入卡选的格式 → 加任务的参数。只对网页链接（yt-dlp 的网站）带清晰度和格式：直链、M3U8 套格式要转码，手机上没有 ffmpeg 会失败。
 isPage：读取信息说是网页（kind = page）；读取失败时按认不认得这个网站猜。
 */
fun addOptions(url: String, fmt: FmtChoice, isPage: Boolean?, pageTitle: String? = null): AddOptions {
  val page = isPage ?: (platOf(url) != null)
  val audio = fmt == FmtChoice.Mp3 || fmt == FmtChoice.Aac || fmt == FmtChoice.Flac
  val (quality, format) = when (fmt) {
    FmtChoice.Best -> "最高可用" to "MP4"
    FmtChoice.P1080 -> "1080p" to "MP4"
    FmtChoice.P720 -> "720p" to "MP4"
    FmtChoice.Mp3 -> null to "MP3"
    FmtChoice.Aac -> null to "AAC"
    FmtChoice.Flac -> null to "FLAC"
    FmtChoice.Webm -> "最高可用" to "WEBM"
    FmtChoice.Mkv -> "最高可用" to "MKV"
    FmtChoice.Keep -> "最高可用" to "保持原格式"
  }
  return AddOptions(
    url = url,
    quality = if (page) quality else null,
    format = if (page || audio) format else null,
    audioOnly = audio,
    pageUrl = null,
    pageTitle = pageTitle,
  )
}

/** 素材的打开、分享（手机上的素材存在 MediaStore 里，见 MediaSaver） */
object AssetOps {
  private suspend fun intentFor(context: Context, id: Long, action: String): Intent? {
    val a = CaptaeRuntime.asset(id) ?: return null
    val uri = MediaSaver.uriFor(a) ?: return null
    val mime = MediaSaver.mimeOf(a.path, a.kind)
    return Intent(action).apply {
      if (action == Intent.ACTION_SEND) { type = mime; putExtra(Intent.EXTRA_STREAM, uri); putExtra(Intent.EXTRA_TITLE, a.title) }
      else setDataAndType(uri, mime)
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
    }
  }

  suspend fun open(context: Context, id: Long) {
    val i = intentFor(context, id, Intent.ACTION_VIEW) ?: return
    runCatching { context.startActivity(Intent.createChooser(i, null).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }.onFailure { Log.w("Captae", "打不开", it) }
  }

  suspend fun share(context: Context, id: Long) {
    val i = intentFor(context, id, Intent.ACTION_SEND) ?: return
    runCatching { context.startActivity(Intent.createChooser(i, context.getString(R.string.ctx_share)).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)) }
  }
}

/** 素材卡长按菜单、素材详情里的按钮 */
interface AssetActions {
  fun open(a: app.captae.app.model.Asset) {}
  fun share(a: app.captae.app.model.Asset) {}
  fun showFiles(a: app.captae.app.model.Asset) {}
  fun delete(a: app.captae.app.model.Asset) {}
  /** 手机上的素材发到电脑 / 电脑上的素材存到手机（远程控制接上以后） */
  fun transfer(a: app.captae.app.model.Asset, remote: Boolean) {}
  /** 取消存到手机（删掉下了一半的） */
  fun cancelSave(a: app.captae.app.model.Asset) {}
}

object NoAssetActions : AssetActions

val LocalAssetActions = staticCompositionLocalOf<AssetActions> { NoAssetActions }
