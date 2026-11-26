package app.captae.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.SystemBarStyle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.mutableStateOf
import androidx.core.content.ContextCompat
import androidx.core.content.edit
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.lifecycleScope
import androidx.lifecycle.repeatOnLifecycle
import app.captae.app.core.CaptaeRuntime
import app.captae.app.core.DownloadService
import app.captae.app.core.RemoteRuntime
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.flow.first
import app.captae.app.demo.Scene
import app.captae.app.demo.ShareMode
import app.captae.app.demo.Sheet
import app.captae.app.ui.parts.Str
import kotlinx.coroutines.launch
import uniffi.captae_mobile.MobileJobState

/**
 唯一的 Activity：铺满全屏（状态栏、导航条透明，内容和玻璃画到边上），整个界面是 Compose。
 - 平常接 Rust 核心的真数据（[CaptaeRuntime]）。
 - debug 包加启动参数 `captaeDemo=<编号>-<状态>`（如 `01-list`）：全是演示数据，跳到某一页的某个状态截设计稿，见 [Scene]。release 包不认这个参数。
 - 别的 App 分享进来的链接（ACTION_SEND text/plain）：弹分享卡片，读取这条链接。
 - 第一次有任务开始下载时（Android 13+）问一次通知权限：前台服务的进度通知要它（不给也照样下，只是通知栏看不到进度）。
 */
class MainActivity : ComponentActivity() {
  private val scene = mutableStateOf(Scene.of(Scene.DEFAULT))
  private val demo = mutableStateOf(false)
  private val askNotify = registerForActivityResult(ActivityResultContracts.RequestPermission()) {}

  override fun onCreate(savedInstanceState: Bundle?) {
    // 只做浅色（同桌面端）：状态栏、导航条用深色图标
    enableEdgeToEdge(
      statusBarStyle = SystemBarStyle.light(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT),
      navigationBarStyle = SystemBarStyle.light(android.graphics.Color.TRANSPARENT, android.graphics.Color.TRANSPARENT),
    )
    super.onCreate(savedInstanceState)
    // 配置变了 Activity 重建，取字的 res 跟着换
    Str.res = resources
    apply(intent)
    setContent { CaptaeApp(scene.value, demo.value) }
    lifecycleScope.launch {
      repeatOnLifecycle(Lifecycle.State.STARTED) {
        // 回到前台：被 Android 15 的额度停掉的前台服务再开一次（回到前台额度恢复）
        DownloadService.startIfActive(this@MainActivity)
        // 遥控电脑：前台连上配对过的电脑、开始找附近的；到后台断开（REMOTE.md 第 1 节）。核心第一次启动要等引擎准备好
        CaptaeRuntime.readyFlow.first { it }
        RemoteRuntime.foreground()
        try {
          awaitCancellation()
        } finally {
          RemoteRuntime.background()
        }
      }
    }
    lifecycleScope.launch {
      repeatOnLifecycle(Lifecycle.State.STARTED) {
        CaptaeRuntime.jobs.collect { m -> if (m.values.any { it.state == MobileJobState.RUNNING }) maybeAskNotify() }
      }
    }
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)
    setIntent(intent)
    apply(intent)
  }

  private fun apply(intent: Intent?) {
    val shared = if (intent?.action == Intent.ACTION_SEND && intent.type == "text/plain") sharedLink(intent) else null
    val demoId = if (BuildConfig.DEBUG) intent?.getStringExtra(Scene.EXTRA) else null
    demo.value = demoId != null
    scene.value = when {
      demoId != null -> Scene.of(demoId)
      intent?.action == Intent.ACTION_SEND -> Scene(sheet = Sheet.Share(ShareMode.Reading, shared.orEmpty()), downloads = app.captae.app.demo.DownloadsMode.List)
      else -> Scene(computer = app.captae.app.demo.ComputerMode.None)
    }
  }

  /** 分享进来的文字里找第一条链接（很多 App 分享的是「标题 + 链接」） */
  private fun sharedLink(intent: Intent): String? {
    val text = intent.getStringExtra(Intent.EXTRA_TEXT)?.trim() ?: return null
    return Regex("""https?://\S+""").find(text)?.value?.trimEnd('.', ',', '，', '。', ')', '）') ?: text.takeIf { looksLikeLink(it) }
  }

  private fun maybeAskNotify() {
    if (Build.VERSION.SDK_INT < 33) return
    if (ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) == PackageManager.PERMISSION_GRANTED) return
    val prefs = getSharedPreferences("ui", MODE_PRIVATE)
    if (prefs.getBoolean("askedNotify", false)) return
    prefs.edit { putBoolean("askedNotify", true) }
    askNotify.launch(Manifest.permission.POST_NOTIFICATIONS)
  }
}
