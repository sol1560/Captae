package app.captae.app.core

import android.content.Context
import android.os.Build
import android.util.Log
import app.captae.core.CaptaeTls
import app.captae.engine.CaptaeEngine
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.captae_mobile.AddOptions
import uniffi.captae_mobile.CancelToken
import uniffi.captae_mobile.CoreListener
import uniffi.captae_mobile.InitOptions
import uniffi.captae_mobile.LibraryPage
import uniffi.captae_mobile.LibraryQuery
import uniffi.captae_mobile.MobileAction
import uniffi.captae_mobile.MobileAsset
import uniffi.captae_mobile.MobileCore
import uniffi.captae_mobile.MobileException
import uniffi.captae_mobile.MobileFailure
import uniffi.captae_mobile.MobileInfo
import uniffi.captae_mobile.MobileJob
import uniffi.captae_mobile.MobileJobState
import uniffi.captae_mobile.MobileProbe
import uniffi.captae_mobile.MobileSettings
import uniffi.captae_mobile.Platform
import java.io.File
import java.util.Locale

/**
 App 里唯一的 Rust 核心（captae-mobile 的 MobileCore）和它的状态。Application 一启动就建（[init]），界面、前台服务、分享面板都从这里拿。
 - 启动顺序（work/mobile/M1.md 第 3 节）：先 `CaptaeTls.init`（Rust 用系统证书校验），再建 MobileCore（平台 android、系统语言、原生库目录）。
 - 任务：`jobs` 是全部任务的快照（Rust 的回调在后台线程上来，这里只改 StateFlow，界面自己收集）。
 - 下完：把文件存进 MediaStore（[MediaSaver]），存好再通知素材库刷新。
 - 有任务在跑或排队时开前台服务（[DownloadService]），都停了它自己收起。
 */
object CaptaeRuntime {
  private const val TAG = "Captae"
  /** Rust 用系统证书校验（CaptaeTls.init）。2026-11-25 核心升级到 rustls-platform-verifier 0.7.1（Android 部分 0.2.0）后打开，见 start() 里的说明 */
  private const val USE_PLATFORM_TLS = true

  private lateinit var app: Context
  private var core: MobileCore? = null
  /** 给遥控（存到手机下完以后存进系统）用 */
  internal val coreOrNull: MobileCore? get() = core

  /** 素材库从外面变了（存到手机下完、存进系统）：让界面重查 */
  fun notifyLibrary() { _libraryVersion.update { it + 1 } }

  /** 建核心失败时的原因（界面上能看到，不闪退） */
  var initError: String? = null
    private set

  val scope = CoroutineScope(SupervisorJob() + Dispatchers.Default)

  private val _jobs = MutableStateFlow<Map<ULong, MobileJob>>(emptyMap())
  /** 全部任务，按 id */
  val jobs: StateFlow<Map<ULong, MobileJob>> = _jobs.asStateFlow()

  private val _libraryVersion = MutableStateFlow(0)
  /** 素材库变了就加一（界面据此重新查） */
  val libraryVersion: StateFlow<Int> = _libraryVersion.asStateFlow()

  private val _settings = MutableStateFlow<MobileSettings?>(null)
  val settings: StateFlow<MobileSettings?> = _settings.asStateFlow()

  /** 只提示一次的事（标题、说法），界面拿去弹一下就清掉 */
  private val _notice = MutableStateFlow<Pair<String, String>?>(null)
  val notice: StateFlow<Pair<String, String>?> = _notice.asStateFlow()
  fun clearNotice() { _notice.value = null }

  val ready: Boolean get() = core != null

  private val _ready = MutableStateFlow(false)
  /** 引擎准备好、核心建好了（第一次打开要解压引擎，约一两秒） */
  val readyFlow: StateFlow<Boolean> = _ready.asStateFlow()

  /** 引擎的版本（设置 › 关于） */
  var engine: CaptaeEngine.Info? = null
    private set

  /** 进程起来就调：后台线程里依次准备引擎 → 证书校验 → 建核心（M3-android-engine.md 第 9 节） */
  fun init(context: Context) {
    if (::app.isInitialized) return
    app = context.applicationContext
    MediaSaver.init(app)
    scope.launch(Dispatchers.IO) { start() }
  }

  private fun start() {
    try {
      // 引擎：第一次打开（或 App 更新换了引擎）解压数据、设好子进程要的环境变量。失败也接着建核心：直链、M3U8 不靠它
      engine = runCatching { CaptaeEngine.prepare(app) }.onFailure { Log.e(TAG, "引擎准备失败", it) }.getOrNull()
      // 证书校验要在建 MobileCore 之前初始化（HTTP 客户端是建 Core 时建的）：系统证书校验，认用户装的证书。
      // 2026-11-24 用 0.1.1 时 Let's Encrypt 这类只给 CRL 的证书都被判成吊销（Android 禁明文，取不到 CRL），关了一阵；
      // 0.2.0 的 aar 自带网络安全配置，只给 CA 的 CRL 域名放开明文，取不到时按「不知道」放行（work/mobile/M2b.md 9.1）。
      val tlsOk = if (USE_PLATFORM_TLS) runCatching { CaptaeTls.init(app) }.getOrDefault(false) else false
      val data = File(app.filesDir, "captae").apply { mkdirs() }
      // 下到一半、刚下完的文件先放在 App 自己的目录，下完再挪进 MediaStore（不要存储权限）
      val downloads = File(app.filesDir, "downloads").apply { mkdirs() }
      val c = MobileCore(
        InitOptions(
          dataDir = data.path,
          downloadsDir = downloads.path,
          platform = Platform.ANDROID,
          language = systemLanguage(),
          nativeLibDir = CaptaeEngine.nativeLibDir(app),
        ),
      )
      core = c
      _settings.value = c.settings()
      _jobs.value = c.jobs().associateBy { it.id }
      c.setListener(Listener)
      Log.i(TAG, "核心已启动：${c.info()} tls 初始化=$tlsOk 引擎=${engine?.let { "yt-dlp ${it.ytdlp}, 解压=${it.extracted}" }}")
      // 遥控电脑（REMOTE.md）：建在本机核心上
      RemoteRuntime.start(app, c)
      _ready.value = true
      // 素材库这时才查得到：让界面重查一次
      _libraryVersion.update { it + 1 }
      // 上次没存进 MediaStore 的（存的过程中 App 被结束了）补存
      scope.launch { MediaSaver.catchUp(c) }
      watchService()
    } catch (e: Throwable) {
      initError = e.message ?: e.toString()
      Log.e(TAG, "核心启动失败", e)
    }
  }

  /** 系统语言 → 核心库认的代码（docs/I18N.md：zh-CN / zh-TW / en / ja / ko / es / fr / de；空 = 跟随系统） */
  private fun systemLanguage(): String {
    val l = Locale.getDefault()
    return when (l.language) {
      "zh" -> if (l.script == "Hant" || l.country in setOf("TW", "HK", "MO")) "zh-TW" else "zh-CN"
      "en", "ja", "ko", "es", "fr", "de" -> l.language
      else -> ""
    }
  }

  // ------------------------------------------------------------------ 回调（Rust 后台线程）

  private object Listener : CoreListener {
    override fun onJob(job: MobileJob) {
      _jobs.update { it + (job.id to job) }
    }

    override fun onRemoved(id: ULong) {
      _jobs.update { it - id }
    }

    override fun onFinished(id: ULong, title: String, path: String, assetId: Long?) {
      // 存进 MediaStore（复制几百 MB 要几秒，不在回调线程上做）
      scope.launch {
        val c = core ?: return@launch
        if (assetId != null) MediaSaver.save(c, assetId, _jobs.value[id]?.thumb)
        // 封面、存放位置变了：任务卡（封面）和素材库都要重画
        _libraryVersion.update { it + 1 }
      }
    }

    override fun onLibrary() {
      _libraryVersion.update { it + 1 }
    }

    override fun onNotice(key: String, title: String, text: String) {
      _notice.value = title to text
    }

    override fun onResync() {
      val c = core ?: return
      _jobs.value = c.jobs().associateBy { it.id }
    }
  }

  // ------------------------------------------------------------------ 前台服务

  /** 有任务在跑 / 排队时开前台服务；服务自己看任务都停了就收起 */
  private fun watchService() {
    scope.launch {
      var wasActive = false
      jobs.collect { m ->
        val active = m.values.any { it.state == MobileJobState.RUNNING || it.state == MobileJobState.QUEUED }
        if (active && !wasActive) withContext(Dispatchers.Main) { DownloadService.start(app) }
        wasActive = active
      }
    }
  }

  // ------------------------------------------------------------------ 任务

  /** 加任务。失败（链接不对、组件缺失……）抛 [MobileException.Failed]，带错误码 */
  fun add(o: AddOptions): MobileJob {
    val c = core ?: throw notReady()
    val j = c.add(o)
    _jobs.update { it + (j.id to j) }
    return j
  }

  fun act(id: ULong, a: MobileAction) {
    val c = core ?: return
    try {
      c.action(id, a)
    } catch (e: MobileException) {
      Log.w(TAG, "任务操作失败 $id $a", e)
    }
    // 取消、移除时核心会发 onRemoved；这里不抢先改，等事件
  }

  fun setVariant(id: ULong, index: Int) {
    runCatching { core?.setVariant(id, index.toUInt()) }.onFailure { Log.w(TAG, "选清晰度失败", it) }
  }

  fun chooseEntries(id: ULong, ids: List<String>) {
    runCatching { core?.chooseEntries(id, ids) }.onFailure { Log.w(TAG, "选条目失败", it) }
  }

  fun pauseAll() { core?.pauseAll() }
  fun resumeAll() { core?.resumeAll() }

  /** 清空已完成：完成的任务从列表里去掉（文件和素材都留着） */
  fun clearDone() {
    val c = core ?: return
    for (j in _jobs.value.values) if (j.state == MobileJobState.DONE) runCatching { c.action(j.id, MobileAction.REMOVE) }
  }

  fun jobLog(id: ULong) = core?.jobLog(id).orEmpty()

  /** 读取信息（可取消：调 token.cancel()，马上返回 code = cancelled） */
  suspend fun probe(url: String, token: CancelToken): MobileProbe {
    val c = core ?: throw notReady()
    return c.probe(url, token)
  }

  // ------------------------------------------------------------------ 素材库

  suspend fun library(kind: String?, search: String?, limit: Int, offset: Int): LibraryPage = withContext(Dispatchers.IO) {
    val c = core ?: return@withContext LibraryPage(emptyList(), 0u)
    c.library(LibraryQuery(kind = kind, search = search, limit = limit.toUInt(), offset = offset.toUInt()))
  }

  suspend fun asset(id: Long): MobileAsset? = withContext(Dispatchers.IO) { runCatching { core?.asset(id) }.getOrNull() }

  /** 删素材：素材库里去掉、核心那份文件删掉、MediaStore 里那份也删掉 */
  suspend fun deleteAsset(id: Long) = withContext(Dispatchers.IO) {
    val c = core ?: return@withContext
    MediaSaver.delete(id)
    runCatching { c.deleteAsset(id, true) }.onFailure { Log.w(TAG, "删素材失败", it) }
  }

  // ------------------------------------------------------------------ 设置

  fun saveSettings(s: MobileSettings) {
    val c = core ?: return
    try {
      _settings.value = c.setSettings(s)
    } catch (e: MobileException) {
      Log.w(TAG, "存设置失败", e)
    }
  }

  fun info(): MobileInfo? = runCatching { core?.info() }.getOrNull()

  /** 下载引擎（yt-dlp）装没装上：:engine 模块把它放进原生库目录（lib*.so） */
  fun engineInstalled(): Boolean = File(CaptaeEngine.nativeLibDir(app), "libytdlp.so").exists()

  private fun notReady() = MobileException.Failed(
    MobileFailure(code = "tool", message = initError ?: "Captae core not ready", action = "repair", raw = initError.orEmpty(), site = null, reason = null, brief = null, startsAt = null),
  )

  val sdk: Int get() = Build.VERSION.SDK_INT
}
