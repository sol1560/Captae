package app.captae.app.core

import android.content.Context
import android.net.nsd.NsdManager
import android.net.nsd.NsdServiceInfo
import android.net.wifi.WifiManager
import android.os.Build
import android.provider.Settings
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.captae_mobile.DeviceJob
import uniffi.captae_mobile.DeviceStatus
import uniffi.captae_mobile.DiscoveredService
import uniffi.captae_mobile.JobGroup
import uniffi.captae_mobile.MobileCore
import uniffi.captae_mobile.MobileDevice
import uniffi.captae_mobile.NearbyComputer
import uniffi.captae_mobile.PendingTask
import uniffi.captae_mobile.RemoteHub
import uniffi.captae_mobile.RemoteListener
import uniffi.captae_mobile.RemoteOptions
import uniffi.captae_mobile.SaveProgress
import uniffi.captae_mobile.SaveState
import uniffi.captae_mobile.SendOptions
import uniffi.captae_mobile.SendResult
import java.io.File
import java.util.concurrent.Executors

/**
 遥控电脑（crates/captae-mobile/REMOTE.md）：captae-mobile 的 RemoteHub 和 App 这边要做的事。
 - 令牌存 Android Keystore（[KeystoreStore]）；
 - App 在前台时 `setActive(true)` 并用 NSD 找 `_captae._tcp`（解析好交给 `discovered`，丢了 `lost`），到后台 `setActive(false)`、停止发现；
 - 事件进来以后重新取 `devices` / `mergedJobs` / `nearby` / `pending` / `saves`，放进 StateFlow 给界面（任务进度 250 毫秒一次，合并刷新）；
 - 存到手机下完：交给 [MediaSaver] 存进相册 / 音乐。
 */
object RemoteRuntime {
  private const val TAG = "CaptaeRemote"
  private const val SERVICE = "_captae._tcp."

  private lateinit var app: Context
  var hub: RemoteHub? = null
    private set
  private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

  private val _devices = MutableStateFlow<List<MobileDevice>>(emptyList())
  /** 配对过的电脑（不含这台手机） */
  val computers: StateFlow<List<MobileDevice>> = _devices.asStateFlow()

  private val _groups = MutableStateFlow<List<JobGroup>>(emptyList())
  /** 每台电脑上的任务 + 等它上线的（mergedJobs，不含这台手机那一组：手机的任务走本机核心的事件） */
  val groups: StateFlow<List<JobGroup>> = _groups.asStateFlow()

  private val _nearby = MutableStateFlow<List<NearbyComputer>>(emptyList())
  val nearby: StateFlow<List<NearbyComputer>> = _nearby.asStateFlow()

  private val _pending = MutableStateFlow<List<PendingTask>>(emptyList())
  val pending: StateFlow<List<PendingTask>> = _pending.asStateFlow()

  private val _saves = MutableStateFlow<Map<Pair<String, Long>, SaveProgress>>(emptyMap())
  /** 存到手机：按（电脑、素材 id） */
  val saves: StateFlow<Map<Pair<String, Long>, SaveProgress>> = _saves.asStateFlow()

  private val _libraryTick = MutableStateFlow(0)
  /** 电脑的素材库变了 */
  val libraryTick: StateFlow<Int> = _libraryTick.asStateFlow()

  /** 一条只提示一次的话（「已发到 Sol 的 MacBook Pro」） */
  val toast = MutableStateFlow<String?>(null)

  private val refresh = Channel<Unit>(Channel.CONFLATED)

  fun start(context: Context, core: MobileCore) {
    app = context.applicationContext
    try {
      val h = RemoteHub(core, RemoteOptions(phoneName = phoneName(app), cacheDir = app.cacheDir.path), KeystoreStore(app))
      h.setListener(Listener)
      hub = h
      reloadAll()
      // 事件很密（进度每 250 毫秒），合并成最多 4 次 / 秒的刷新
      scope.launch {
        for (u in refresh) {
          reloadJobs()
          delay(250)
        }
      }
      Log.i(TAG, "RemoteHub 已建，配对过 ${_devices.value.size} 台")
    } catch (e: Throwable) {
      Log.e(TAG, "RemoteHub 建不起来", e)
    }
  }

  /** 这台手机在电脑上显示的名字：系统设置里的设备名，没有就用型号 */
  fun phoneName(c: Context): String =
    Settings.Global.getString(c.contentResolver, Settings.Global.DEVICE_NAME)?.takeIf { it.isNotBlank() } ?: Build.MODEL

  private fun reloadAll() {
    val h = hub ?: return
    _devices.value = h.devices().filter { !it.local }
    _nearby.value = h.nearby()
    _pending.value = h.pending()
    _saves.value = h.saves().associateBy { it.device to it.assetId }
    reloadJobs()
  }

  private fun reloadJobs() {
    val h = hub ?: return
    _groups.value = runCatching { h.mergedJobs(null, null).filter { !it.device.local } }.getOrDefault(emptyList())
  }

  private object Listener : RemoteListener {
    override fun onJob(job: DeviceJob) { refresh.trySend(Unit) }
    override fun onRemoved(device: String, id: ULong) { refresh.trySend(Unit) }
    override fun onFinished(device: String, id: ULong, title: String, assetId: Long?) { refresh.trySend(Unit); _libraryTick.update { it + 1 } }
    override fun onLibrary(device: String) { _libraryTick.update { it + 1 } }
    override fun onResync(device: String) { refresh.trySend(Unit) }
    override fun onDevice(device: MobileDevice) {
      _devices.update { l -> if (l.any { it.id == device.id }) l.map { if (it.id == device.id) device else it } else l }
      refresh.trySend(Unit)
    }
    override fun onDevices() { reloadAll() }
    override fun onPending() { _pending.value = hub?.pending().orEmpty(); refresh.trySend(Unit) }
    override fun onPendingSent(task: PendingTask, job: DeviceJob) {
      _pending.value = hub?.pending().orEmpty()
      refresh.trySend(Unit)
    }
    override fun onSave(save: SaveProgress) {
      _saves.update { it + ((save.device to save.assetId) to save) }
      // 下完：核心已经把文件导入了手机素材库（localAssetId，M2b.md 9.3），照本机下完的做法存进相册 / 音乐、取封面；
      // 没导入成（localAssetId 为空）就只存进系统
      if (save.state == SaveState.DONE) scope.launch {
        val local = save.localAssetId
        val c = CaptaeRuntime.coreOrNull
        if (local != null && c != null) MediaSaver.save(c, local, null, remoteKey = "remote:${save.device}:${save.assetId}") else MediaSaver.saveRemote(save)
        CaptaeRuntime.notifyLibrary()
      }
    }
    override fun onSavesRestored(saves: List<SaveProgress>) {
      // App 重开后找回上次没存完的：进度条显示回去，电脑连上会自动接着下（REMOTE.md 第 8 节）
      _saves.update { m -> m + saves.associateBy { it.device to it.assetId } }
    }
  }

  // ------------------------------------------------------------------ 前台 / 后台

  private var nsd: NsdManager? = null
  private var discovery: NsdManager.DiscoveryListener? = null
  private var lock: WifiManager.MulticastLock? = null
  private val resolver = Executors.newSingleThreadExecutor()

  /** App 回到前台：连上配对过的电脑、开始找附近的电脑 */
  fun foreground() {
    val h = hub ?: return
    scope.launch { h.setActive(true) }
    startDiscovery()
  }

  /** 到后台：全部断开（没有服务器就没有推送），停止找 */
  fun background() {
    val h = hub ?: return
    scope.launch { h.setActive(false) }
    stopDiscovery()
  }

  private fun startDiscovery() {
    if (discovery != null) return
    val m = app.getSystemService(NsdManager::class.java) ?: return
    nsd = m
    // 有的机型不拿 MulticastLock 收不到组播
    lock = app.getSystemService(WifiManager::class.java)?.createMulticastLock("captae-nsd")?.apply { setReferenceCounted(false); runCatching { acquire() } }
    val l = object : NsdManager.DiscoveryListener {
      override fun onDiscoveryStarted(serviceType: String) {}
      override fun onDiscoveryStopped(serviceType: String) {}
      override fun onStartDiscoveryFailed(serviceType: String, errorCode: Int) { Log.w(TAG, "NSD 开始失败 $errorCode"); discovery = null }
      override fun onStopDiscoveryFailed(serviceType: String, errorCode: Int) {}
      override fun onServiceFound(info: NsdServiceInfo) { resolve(m, info) }
      override fun onServiceLost(info: NsdServiceInfo) {
        scope.launch { hub?.lost(info.serviceName); _nearby.value = hub?.nearby().orEmpty() }
      }
    }
    discovery = l
    runCatching { m.discoverServices(SERVICE, NsdManager.PROTOCOL_DNS_SD, l) }.onFailure { Log.w(TAG, "NSD 不可用", it); discovery = null }
  }

  private fun resolve(m: NsdManager, info: NsdServiceInfo) {
    val done = { r: NsdServiceInfo ->
      val hosts = if (Build.VERSION.SDK_INT >= 34) r.hostAddresses.mapNotNull { it.hostAddress?.substringBefore('%') } else listOfNotNull(@Suppress("DEPRECATION") r.host?.hostAddress?.substringBefore('%'))
      val txt = r.attributes.mapValues { (_, v) -> v?.toString(Charsets.UTF_8).orEmpty() }
      scope.launch {
        hub?.discovered(DiscoveredService(name = r.serviceName, hosts = hosts, port = r.port.toUShort(), txt = txt))
        _nearby.value = hub?.nearby().orEmpty()
      }
    }
    if (Build.VERSION.SDK_INT >= 34) {
      runCatching {
        m.registerServiceInfoCallback(info, resolver, object : NsdManager.ServiceInfoCallback {
          override fun onServiceInfoCallbackRegistrationFailed(errorCode: Int) {}
          override fun onServiceUpdated(serviceInfo: NsdServiceInfo) {
            done(serviceInfo)
            runCatching { m.unregisterServiceInfoCallback(this) }
          }
          override fun onServiceLost() {}
          override fun onServiceInfoCallbackUnregistered() {}
        })
      }
    } else {
      @Suppress("DEPRECATION")
      m.resolveService(info, object : NsdManager.ResolveListener {
        override fun onResolveFailed(serviceInfo: NsdServiceInfo, errorCode: Int) {}
        override fun onServiceResolved(serviceInfo: NsdServiceInfo) { done(serviceInfo) }
      })
    }
  }

  private fun stopDiscovery() {
    discovery?.let { l -> runCatching { nsd?.stopServiceDiscovery(l) } }
    discovery = null
    runCatching { lock?.release() }
  }

  // ------------------------------------------------------------------ 操作

  /** 发到电脑：在线就发，连不上进「等电脑上线」。返回给人看的一句话（失败时）；成功返回 null */
  suspend fun send(device: String, o: SendOptions): SendResult = hub!!.send(device, o)

  /** 存到手机：下到 App 自己的目录（重开后还在，能续传），下完 [MediaSaver.saveRemote] 存进系统 */
  fun saveToPhone(device: String, assetId: Long, fileName: String): SaveProgress? {
    val h = hub ?: return null
    // 每条素材一个目录：文件名保持电脑上的原名（存进相册时就是这个名字）
    val dir = File(app.filesDir, "remote-saves/${device.take(16)}/$assetId").apply { mkdirs() }
    val safe = fileName.replace(Regex("""[\\/:*?"<>|]"""), "_").ifBlank { "$assetId" }
    return runCatching { h.saveToPhone(device, assetId, File(dir, safe).path) }
      .onFailure { Log.w(TAG, "存到手机失败", it) }
      .getOrNull()?.also { s -> _saves.update { it + ((s.device to s.assetId) to s) } }
  }

  fun cancelSave(id: ULong) { runCatching { hub?.cancelSave(id) }.getOrNull()?.let { s -> _saves.update { it + ((s.device to s.assetId) to s) } } }

  /** 在手机上改电脑的名字（空 = 用电脑自己报的） */
  fun rename(device: String, name: String) { runCatching { hub?.rename(device, name) }; reloadAll() }

  fun remove(device: String) { runCatching { hub?.remove(device) }; reloadAll() }

  fun online(d: MobileDevice) = d.status == DeviceStatus.ONLINE
}
