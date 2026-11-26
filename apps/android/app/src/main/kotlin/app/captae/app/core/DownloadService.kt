package app.captae.app.core

import android.Manifest
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import app.captae.app.MainActivity
import app.captae.app.R
import app.captae.app.model.speed
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.launch
import uniffi.captae_mobile.MobileJob
import uniffi.captae_mobile.MobileJobState

/**
 下载时的前台服务（PLAN 5.4「后台」：dataSync）：切到后台、锁屏照样下。
 - 有任务在跑或排队时由 [CaptaeRuntime] 开起来；通知栏一条进度（正在下几个、速度、当前那条的百分比），能「全部暂停」。
 - 全部暂停了：前台身份放掉、通知留着（能划掉），上面一个「继续」；都下完 / 都没了：通知收起、服务停掉。
 - Android 15 起 dataSync 每天只给 6 小时（系统调 onTimeout）：停掉服务，通知里说清楚「今天的后台时间用完了，打开 Captae 会接着下」；
   额度用完时再开前台服务会被系统拒绝，也是同一句话。打开 App（回到前台）额度就恢复，[MainActivity] 回到前台时再开一次。
 */
class DownloadService : Service() {
  private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)
  private var watcher: Job? = null
  private var foreground = false

  override fun onBind(intent: Intent?): IBinder? = null

  override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
    when (intent?.action) {
      ACTION_PAUSE -> CaptaeRuntime.pauseAll()
      ACTION_RESUME -> CaptaeRuntime.resumeAll()
    }
    // 先马上进前台（startForegroundService 之后 5 秒内必须调），内容等下面的循环刷新
    if (!foreground) {
      if (!goForeground(build(CaptaeRuntime.jobs.value.values.toList()))) {
        stopSelf()
        return START_NOT_STICKY
      }
    }
    if (watcher == null) watcher = scope.launch { watch() }
    return START_NOT_STICKY
  }

  private fun goForeground(n: Notification): Boolean = try {
    val type = if (Build.VERSION.SDK_INT >= 29) ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC else 0
    ServiceCompat.startForeground(this, NOTIFY_ID, n, type)
    foreground = true
    true
  } catch (e: Exception) {
    // Android 12+ 后台不许开前台服务、Android 15 dataSync 额度用完（ForegroundServiceStartNotAllowedException）
    Log.w(TAG, "开不了前台服务", e)
    quotaNotice(this)
    false
  }

  /** 跟着任务刷新通知：最快一秒一次（核心 250 毫秒推一次进度，通知不用这么勤） */
  private suspend fun watch() {
    CaptaeRuntime.jobs.collectLatest { m ->
      val list = m.values.toList()
      val active = list.any { it.state == MobileJobState.RUNNING || it.state == MobileJobState.QUEUED }
      val paused = list.count { it.state == MobileJobState.PAUSED }
      val nm = NotificationManagerCompat.from(this)
      when {
        active -> {
          if (!foreground) goForeground(build(list)) else notify(nm, build(list))
        }
        paused > 0 -> {
          // 全部暂停：放掉前台身份，通知留着，能划掉，点「继续」再开回来
          if (foreground) { ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_DETACH); foreground = false }
          notify(nm, buildPaused(paused))
          stopSelf()
        }
        else -> {
          ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
          foreground = false
          nm.cancel(NOTIFY_ID)
          stopSelf()
        }
      }
      delay(1000)
    }
  }

  private fun notify(nm: NotificationManagerCompat, n: Notification) {
    if (Build.VERSION.SDK_INT < 33 || ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) {
      nm.notify(NOTIFY_ID, n)
    }
  }

  private fun openApp(): PendingIntent = PendingIntent.getActivity(
    this, 0, Intent(this, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP),
    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
  )

  private fun build(list: List<MobileJob>): Notification {
    ensureChannel(this)
    val running = list.filter { it.state == MobileJobState.RUNNING && it.phase != "pick" }
    val withSize = running.filter { it.totalBytes > 0u }
    val got = withSize.sumOf { it.gotBytes.toDouble() }
    val total = withSize.sumOf { it.totalBytes.toDouble() }
    val pct = if (total > 0) (got / total * 100).toInt().coerceIn(0, 100) else 0
    val speedMb = running.sumOf { it.speed.toDouble() } / 1_000_000
    val cur = running.firstOrNull { it.phase == "dl" && it.totalBytes > 0u } ?: running.firstOrNull() ?: list.firstOrNull { it.state == MobileJobState.QUEUED }
    val title = if (running.isEmpty()) getString(R.string.notify_queued, list.count { it.state == MobileJobState.QUEUED })
    else getString(R.string.bar_running, running.size, speed(speedMb))
    val text = cur?.let { if (it.untitled) getString(R.string.job_reading_a11y) else it.title }.orEmpty()
    val pause = PendingIntent.getService(this, 1, Intent(this, DownloadService::class.java).setAction(ACTION_PAUSE), PendingIntent.FLAG_IMMUTABLE)
    return NotificationCompat.Builder(this, CHANNEL)
      .setSmallIcon(R.drawable.ic_launcher_monochrome)
      .setContentTitle(title)
      .setContentText(text)
      .setSubText(if (total > 0) "$pct%" else null)
      .setProgress(100, pct, total <= 0)
      .setOngoing(true)
      .setOnlyAlertOnce(true)
      .setSilent(true)
      .setCategory(NotificationCompat.CATEGORY_PROGRESS)
      .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
      .setContentIntent(openApp())
      .addAction(0, getString(R.string.pause_all), pause)
      .build()
  }

  private fun buildPaused(n: Int): Notification {
    ensureChannel(this)
    val resume = PendingIntent.getForegroundService(this, 2, Intent(this, DownloadService::class.java).setAction(ACTION_RESUME), PendingIntent.FLAG_IMMUTABLE)
    return NotificationCompat.Builder(this, CHANNEL)
      .setSmallIcon(R.drawable.ic_launcher_monochrome)
      .setContentTitle(getString(R.string.bar_paused, n))
      .setOngoing(false)
      .setSilent(true)
      .setContentIntent(openApp())
      .addAction(0, getString(R.string.resume_all), resume)
      .build()
  }

  /** Android 15：dataSync 一天 6 小时用完了。系统要求几秒内停掉 */
  override fun onTimeout(startId: Int, fgsType: Int) {
    Log.w(TAG, "dataSync 额度用完了")
    quotaNotice(this)
    ServiceCompat.stopForeground(this, ServiceCompat.STOP_FOREGROUND_REMOVE)
    foreground = false
    stopSelf()
  }

  override fun onDestroy() {
    scope.cancel()
    super.onDestroy()
  }

  companion object {
    private const val TAG = "CaptaeService"
    private const val CHANNEL = "downloads"
    private const val NOTIFY_ID = 1
    private const val QUOTA_ID = 2
    const val ACTION_PAUSE = "app.captae.PAUSE_ALL"
    const val ACTION_RESUME = "app.captae.RESUME_ALL"

    fun ensureChannel(c: Context) {
      val nm = c.getSystemService(NotificationManager::class.java)
      if (nm.getNotificationChannel(CHANNEL) == null) {
        nm.createNotificationChannel(NotificationChannel(CHANNEL, c.getString(R.string.notify_channel), NotificationManager.IMPORTANCE_LOW).apply { setShowBadge(false) })
      }
    }

    /** 开前台服务（App 在前台、或从通知的按钮点进来时才开得了；后台开会被拒，改成发一条说明） */
    fun start(c: Context) {
      try {
        ContextCompat.startForegroundService(c, Intent(c, DownloadService::class.java))
      } catch (e: Exception) {
        Log.w(TAG, "开不了前台服务", e)
        quotaNotice(c)
      }
    }

    /** 回到前台时：还有任务在跑就再开一次（被额度停掉以后，回到前台额度恢复） */
    fun startIfActive(c: Context) {
      val active = CaptaeRuntime.jobs.value.values.any { it.state == MobileJobState.RUNNING || it.state == MobileJobState.QUEUED }
      if (active) start(c)
      NotificationManagerCompat.from(c).cancel(QUOTA_ID)
    }

    /** 「今天的后台下载时间用完了」 */
    fun quotaNotice(c: Context) {
      ensureChannel(c)
      if (Build.VERSION.SDK_INT >= 33 && ContextCompat.checkSelfPermission(c, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) return
      val open = PendingIntent.getActivity(c, 3, Intent(c, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
      val n = NotificationCompat.Builder(c, CHANNEL)
        .setSmallIcon(R.drawable.ic_launcher_monochrome)
        .setContentTitle(c.getString(R.string.notify_quota_title))
        .setContentText(c.getString(R.string.notify_quota_text))
        .setStyle(NotificationCompat.BigTextStyle().bigText(c.getString(R.string.notify_quota_text)))
        .setContentIntent(open)
        .setAutoCancel(true)
        .build()
      NotificationManagerCompat.from(c).notify(QUOTA_ID, n)
    }
  }
}
