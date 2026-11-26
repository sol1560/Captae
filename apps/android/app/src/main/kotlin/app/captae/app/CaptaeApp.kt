package app.captae.app

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.RowScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshots.SnapshotStateList
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import app.captae.app.core.CaptaeRuntime
import app.captae.app.core.RemoteRuntime
import app.captae.app.core.computerItems
import app.captae.app.core.toComputer
import app.captae.app.core.phoneItems
import app.captae.app.core.toAsset
import app.captae.app.demo.ComposerMode
import app.captae.app.demo.ComputerMode
import app.captae.app.demo.DemoData
import app.captae.app.demo.DeviceScope
import app.captae.app.demo.DownloadsMode
import app.captae.app.demo.LibraryMode
import app.captae.app.demo.Scene
import app.captae.app.demo.Screen
import app.captae.app.demo.Sheet
import app.captae.app.demo.Tab
import app.captae.app.model.Asset
import app.captae.app.model.Computer
import app.captae.app.model.DownloadItem
import app.captae.app.model.Where
import app.captae.app.ui.AssetActions
import app.captae.app.ui.AssetOps
import app.captae.app.ui.LocalAssetActions
import app.captae.app.ui.LocalJobActions
import app.captae.app.ui.NoAssetActions
import app.captae.app.ui.NoJobActions
import app.captae.app.ui.AppJobActions
import app.captae.app.ui.parts.cjkFormat
import app.captae.app.ui.addOptions
import app.captae.app.ui.computer.ComputersSheet
import app.captae.app.ui.computer.PairCodeSheet
import app.captae.app.ui.computer.ScanScreen
import app.captae.app.ui.downloads.BarState
import app.captae.app.ui.downloads.ComposerSheet
import app.captae.app.ui.downloads.DownloadBar
import app.captae.app.ui.downloads.DownloadsScreen
import app.captae.app.ui.downloads.FmtChoice
import app.captae.app.ui.downloads.JobDetailSheet
import app.captae.app.ui.library.AssetDetailScreen
import app.captae.app.ui.library.Kind
import app.captae.app.ui.library.LibraryScreen
import app.captae.app.ui.library.MacSection
import androidx.compose.runtime.mutableStateMapOf
import app.captae.app.ui.nav.DeviceTitleMenu
import app.captae.app.ui.nav.MoreButton
import app.captae.app.ui.parts.ProvideReducedMotion
import app.captae.app.ui.parts.str
import app.captae.app.ui.search.SearchScreen
import app.captae.app.ui.settings.AboutScreen
import app.captae.app.ui.settings.ProxyScreen
import app.captae.app.ui.settings.SettingsScreen
import app.captae.app.ui.share.ShareSheet
import app.captae.glass.icons.Sym
import app.captae.glass.nav.GlassTab
import app.captae.glass.nav.GlassTabScaffold
import app.captae.glass.overlay.OverlayHost
import app.captae.glass.theme.CaptaeTheme
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import uniffi.captae_mobile.CancelToken
import uniffi.captae_mobile.MobileException
import uniffi.captae_mobile.MobileJob

/**
 整个 App（第二版导航，BRIEF 第 2 节）：
 - 三个只有图标的标签：下载 / 素材库 / 搜索，每个标签自己一条返回栈；
 - 主页面的标题是设备菜单（看哪台设备），右上角圆形「更多」；
 - 标签栏上方一条下载条（玻璃胶囊），往下滚时和标签栏一起收成一行；
 - sheet（任务详情、输入卡、管理电脑、分享卡片）和菜单画在同一个窗口的覆盖层里。
 数据来自 Rust 核心（[CaptaeRuntime]）；demo = true 时（debug 包加启动参数 captaeDemo）全是演示数据，scene 决定起始状态。
 */
@Composable
fun CaptaeApp(scene: Scene, demo: Boolean) {
  CaptaeTheme {
    ProvideReducedMotion {
      OverlayHost {
        // scene 换了（新的启动参数、分享进来的链接）整个状态重来
        val state = remember(scene, demo) { AppState(scene, demo) }
        Shell(state)
      }
    }
  }
}

/** 素材库一页几条（滚到底再读下一页） */
private const val LIB_PAGE = 60

/** 界面状态：当前标签、看哪台设备、各标签的返回栈、弹出来的东西；真数据时还有任务、素材库、输入卡的读取 */
private class AppState(scene: Scene, val demo: Boolean) {
  var tab by mutableStateOf(scene.tab)
  val stacks: Map<Tab, SnapshotStateList<Screen>> = Tab.entries.associateWith { mutableStateListOf() }
  var sheet by mutableStateOf<Sheet?>(null)
  /** 起始状态要自动弹出的 sheet（等第一帧画完再弹，进场动画照常走） */
  val pendingSheet: Sheet? = scene.sheet
  private val downloads = scene.downloads
  private val library = scene.library
  private val computer = if (demo) scene.computer else ComputerMode.None
  val proxyManual = scene.proxyManual
  val query = scene.query
  /** 没配对过默认「这台手机」，配对过默认「全部设备」（BRIEF 2.1） */
  /** 用户在设备菜单里自己选过（之后配对、删电脑不再自动改） */
  var scopeChosen = scene.device != null
  var scope by mutableStateOf(scene.device ?: if (computer == ComputerMode.None) DeviceScope.Phone else DeviceScope.All)

  // ---------------- 真数据 ----------------
  var jobsMap by mutableStateOf<Map<ULong, MobileJob>>(emptyMap())
  /** 素材库变了（下完存进系统、有了封面）就加一：任务卡的封面跟着换 */
  var mediaTick by mutableIntStateOf(0)
  /** 电脑上的任务和等它上线的（RemoteHub.mergedJobs） */
  var groups by mutableStateOf<List<uniffi.captae_mobile.JobGroup>>(emptyList())
  /** 配对过的电脑（RemoteHub.devices，不含这台手机） */
  var devices by mutableStateOf<List<uniffi.captae_mobile.MobileDevice>>(emptyList())
  /** 设备菜单选了哪台电脑（scope 是 Mac 时） */
  var selectedMac by mutableStateOf<String?>(null)
  private val realItems by derivedStateOf {
    mediaTick
    // 每台配对过的电脑一组（顺序跟 macs 一样），最后是这台手机
    val order = realMacs.map { it.id }
    groups.sortedBy { g -> order.indexOf(g.device.id).let { if (it < 0) Int.MAX_VALUE else it } }.flatMap { computerItems(it) } + phoneItems(jobsMap.values)
  }
  /** 每台电脑的素材库（最近一次读到的，电脑离线时变灰显示） */
  val macSecs = mutableStateMapOf<String, MacSection>()
  var libKind by mutableStateOf(Kind.All)
  val libItems = mutableStateListOf<Asset>()
  var libTotal by mutableIntStateOf(0)
  /** 配对过的电脑，最近用过的在前（输入卡、分享卡片默认发给第一台） */
  val realMacs: List<Computer> get() = devices.sortedByDescending { it.lastUsed ?: it.pairedAt ?: 0 }.map { it.toComputer() }
  val realMac: Computer? get() = realMacs.firstOrNull()
  private var probeToken: CancelToken? = null
  private var probeTask: Job? = null

  init {
    when (val p = scene.push) {
      null -> {}
      // 代理、关于是从设置里点进去的：栈里先放设置
      Screen.SettingsProxy, Screen.SettingsAbout -> stacks.getValue(scene.tab).addAll(listOf(Screen.Settings, p))
      else -> stacks.getValue(scene.tab).add(p)
    }
  }

  val mac: Computer? get() = if (!demo) realMac else when (computer) {
    ComputerMode.None -> null
    ComputerMode.Online -> DemoData.mac
    ComputerMode.Offline -> DemoData.macOffline
  }

  /** 配对过的每一台（演示数据只有一台） */
  val macs: List<Computer> get() = if (!demo) realMacs else listOfNotNull(mac)
  /** 设备菜单选了的那台（没选过就是第一台） */
  val selected: Computer? get() = macs.firstOrNull { it.id == selectedMac } ?: macs.firstOrNull()

  /** 素材库每台电脑一段 */
  val macSections: List<MacSection> get() = if (demo) listOfNotNull(mac?.let { MacSection(it, DemoData.macAssets, DemoData.MAC_ASSET_COUNT) })
    else realMacs.map { m -> macSecs[m.id]?.copy(mac = m) ?: MacSection(m, emptyList(), 0) }

  /** 所有设备上的任务（下载条、搜索用；下载页再按设备筛） */
  val items: List<DownloadItem> get() = if (!demo) realItems else when (downloads) {
    DownloadsMode.Empty -> emptyList()
    DownloadsMode.List -> DemoData.downloads
    DownloadsMode.Errors -> DemoData.errorDownloads
  }.let { l -> if (mac == null) l.filter { it.where == Where.Phone } else l }

  val phoneAssets: List<Asset> get() = if (!demo) libItems else if (library == LibraryMode.Empty) emptyList() else DemoData.phoneAssets.filter { libKind.wire == null || it.kind == libKind.wire }
  val phoneTotal: Int get() = if (!demo) libTotal else phoneAssets.size

  fun push(s: Screen) = stacks.getValue(tab).add(s)
  fun pop() = stacks.getValue(tab).removeLastOrNull()

  suspend fun reloadLibrary() {
    val p = CaptaeRuntime.library(libKind.wire, null, LIB_PAGE, 0)
    libItems.clear()
    libItems.addAll(p.items.map { it.toAsset() })
    libTotal = p.total.toInt()
  }

  /**
   读每台电脑的素材库（RemoteHub.mergedLibrary：几台同时查，每台第一页 60 条；全部设备时每段只显示最新 4 条）。
   离线 / 被撤销的那段带 error：留着上次读到的（变灰显示）。封面另外去拿，拿到一张换一张
   */
  suspend fun reloadMacLibrary() {
    val hub = RemoteRuntime.hub ?: return
    val ids = realMacs.map { it.id }.toSet()
    macSecs.keys.retainAll(ids)
    if (ids.isEmpty()) return
    val secs = runCatching { hub.mergedLibrary(uniffi.captae_mobile.LibraryQuery(kind = libKind.wire, search = null, limit = LIB_PAGE.toUInt(), offset = 0u), null) }.getOrNull() ?: return
    for (sec in secs) {
      if (sec.device.local || sec.error != null) continue
      val id = sec.device.id
      macSecs[id] = MacSection(sec.device.toComputer(), sec.items.map { it.asset.toAsset(remote = true).copy(device = id) }, sec.total.toInt())
    }
    for (sec in secs) {
      if (sec.device.local || sec.error != null) continue
      val id = sec.device.id
      for (da in sec.items.filter { it.asset.thumb == null }) {
        val t = runCatching { hub.remoteThumbnail(id, da.asset.id, false) }.getOrNull() ?: continue
        val cur = macSecs[id] ?: continue
        macSecs[id] = cur.copy(assets = cur.assets.map { if (it.id == da.asset.id) it.copy(thumb = t) else it })
      }
    }
  }

  suspend fun loadMoreLibrary() {
    if (libItems.size >= libTotal) return
    val p = CaptaeRuntime.library(libKind.wire, null, LIB_PAGE, libItems.size)
    val have = libItems.map { it.id }.toSet()
    libItems.addAll(p.items.map { it.toAsset() }.filter { it.id !in have })
    libTotal = p.total.toInt()
  }

  /**
   输入卡点了发送（BRIEF 2.4、2.6）：先读取视频信息（输入卡换成读取中的样子，能停），读完加任务、收起输入卡。
   - 停了（cancelled）：回到填链接的样子，链接还在；
   - 读取失败：照样加任务，核心自己再读一次，失败的说法按错误码显示在任务卡上（例如引擎还没装上时的 tool）；
   - 加任务本身失败（链接不完整这类）：留在输入卡里，下面一行说原因。
   发到电脑、电脑连不上时：存进「等电脑上线」。
   */
  fun submit(url: String, macId: String?, fmt: FmtChoice, scope: CoroutineScope) {
    if (demo) { sheet = null; return }
    val m = macs.firstOrNull { it.id == macId }
    if (m != null) {
      // 发到电脑：在线就发，连不上进「等电脑上线」（RemoteHub.send 自己判断）。清晰度、格式按网页链接的规矩带
      val o = addOptions(url, fmt, null)
      scope.launch {
        try {
          RemoteRuntime.send(m.id, uniffi.captae_mobile.SendOptions(url = url, quality = o.quality, format = o.format, audioOnly = o.audioOnly, preset = null, title = null))
          sheet = null
          tab = Tab.Downloads
          stacks.getValue(Tab.Downloads).clear()
        } catch (e: MobileException.Failed) {
          sheet = Sheet.Composer(url, error = e.detail.message)
        }
      }
      return
    }
    // 所有链接都先在输入卡里读（流光，BRIEF 2.6），读完再加任务。核心把读到的结果按链接记 10 分钟，
    // 加任务时同一条链接直接用，不读第二遍（work/mobile/M2b.md 9.2）
    val token = CancelToken()
    probeToken = token
    sheet = Sheet.Composer(url, ComposerMode.Reading, startedAt = System.currentTimeMillis())
    probeTask?.cancel()
    probeTask = scope.launch {
      var isPage: Boolean? = null
      try {
        val p = CaptaeRuntime.probe(url, token)
        isPage = p.kind == "page"
      } catch (e: MobileException.Failed) {
        if (e.detail.code == "cancelled") {
          if ((sheet as? Sheet.Composer)?.mode == ComposerMode.Reading) sheet = Sheet.Composer(url)
          return@launch
        }
      } catch (e: Exception) {
        // 其他意外：照样交给核心
      }
      if (token.isCancelled()) return@launch
      try {
        CaptaeRuntime.add(addOptions(url, fmt, isPage))
        if ((sheet as? Sheet.Composer)?.mode == ComposerMode.Reading) sheet = null
        // 加好以后回到下载页、看这台手机（新任务在这里）
        tab = Tab.Downloads
        stacks.getValue(Tab.Downloads).clear()
      } catch (e: MobileException.Failed) {
        sheet = Sheet.Composer(url, error = e.detail.message)
      }
    }
  }

  /** 读取中点了停止，或者把输入卡划掉了 */
  fun stopReading() {
    probeToken?.cancel()
  }
}

@Composable
private fun Shell(s: AppState) {
  val context = LocalContext.current
  val scope = rememberCoroutineScope()
  LaunchedEffect(s) {
    if (s.pendingSheet != null) {
      delay(250)
      s.sheet = s.pendingSheet
    }
  }
  if (!s.demo) {
    LaunchedEffect(s) { CaptaeRuntime.jobs.collect { s.jobsMap = it } }
    LaunchedEffect(s) {
      RemoteRuntime.computers.collect {
        // 配对过电脑：默认看「全部设备」（BRIEF 2.1）；用户自己选过就不动
        if (!s.scopeChosen && s.devices.isEmpty() && it.isNotEmpty()) s.scope = DeviceScope.All
        s.devices = it
      }
    }
    LaunchedEffect(s) { RemoteRuntime.groups.collect { s.groups = it } }
    val macTick by RemoteRuntime.libraryTick.collectAsState()
    // 哪几台在线变了（连上、断开）也重读
    val online = s.realMacs.filter { it.online }.map { it.id }.sorted().joinToString()
    LaunchedEffect(macTick, online, s.libKind) { s.reloadMacLibrary() }
    val libVersion by CaptaeRuntime.libraryVersion.collectAsState()
    LaunchedEffect(libVersion, s.libKind) { s.mediaTick = libVersion; s.reloadLibrary() }
  }
  val stack = s.stacks.getValue(s.tab)
  val top = stack.lastOrNull()
  BackHandler(enabled = top != null && s.sheet == null) { s.pop() }
  // 扫码是全屏取景，标签栏和下载条藏起来（iOS `.toolbar(.hidden, for: .tabBar)`）
  val barVisible = top != Screen.PairScan
  val tabs = listOf(
    GlassTab(str(R.string.tab_downloads), Sym.arrowDownCircle),
    GlassTab(str(R.string.tab_library), Sym.photoOnRectangleAngled),
    GlassTab(str(R.string.tab_search), Sym.magnifyingglass),
  )
  // 电脑连不上时它上面的任务不算（不知道它现在的状态），下载条只看这台手机上的
  val bar = BarState(s.items.filter { it.where == Where.Phone || s.macs.firstOrNull { m -> m.id == (it.device ?: s.mac?.id) }?.online == true })
  val clipboard = LocalClipboard.current
  val jobActions = remember(s) {
    if (s.demo) NoJobActions else AppJobActions(context, scope, onOpenSettings = { s.sheet = null; s.push(Screen.Settings) }, onOpenComputers = { s.sheet = Sheet.Computers }, computer = { s.realMac?.id })
  }
  val assetActions = remember(s) {
    if (s.demo) NoAssetActions else object : AssetActions {
      override fun open(a: Asset) { scope.launch { AssetOps.open(context, a.id) } }
      override fun share(a: Asset) { scope.launch { AssetOps.share(context, a.id) } }
      override fun showFiles(a: Asset) { scope.launch { AssetOps.open(context, a.id) } }
      override fun delete(a: Asset) {
        scope.launch {
          CaptaeRuntime.deleteAsset(a.id)
          // 在这条的详情页里删的：退回去
          val st = s.stacks.getValue(s.tab)
          if ((st.lastOrNull() as? Screen.AssetDetail)?.asset?.id == a.id) st.removeLastOrNull()
        }
      }
      override fun transfer(a: Asset, remote: Boolean) {
        val m = (if (remote) s.macs.firstOrNull { it.id == a.device } else null) ?: s.realMac ?: run { s.sheet = Sheet.Computers; return }
        if (remote) {
          // 文件名：标题本来就带扩展名（直链的标题是文件名）就不再加
          val ext = "." + a.fmt.lowercase()
          RemoteRuntime.saveToPhone(m.id, a.id, a.fileName.ifEmpty { if (a.title.lowercase().endsWith(ext)) a.title else a.title + ext })
        }
        else scope.launch {
          // 手机上的素材发到电脑：发的是它的下载链接（电脑自己下一份），不传文件
          val url = a.url.ifEmpty { return@launch }
          runCatching { RemoteRuntime.send(m.id, uniffi.captae_mobile.SendOptions(url = if (url.startsWith("http")) url else "https://$url", quality = null, format = null, audioOnly = a.kind == "audio", preset = null, title = a.title)) }
        }
      }
      override fun cancelSave(a: Asset) {
        val m = s.macs.firstOrNull { it.id == a.device } ?: s.realMac ?: return
        RemoteRuntime.saves.value[m.id to a.id]?.let { RemoteRuntime.cancelSave(it.id) }
      }
    }
  }
  CompositionLocalProvider(LocalJobActions provides jobActions, LocalAssetActions provides assetActions) {
    GlassTabScaffold(
      tabs = tabs,
      selectedIndex = s.tab.ordinal,
      onSelect = { i ->
        val t = Tab.entries[i]
        // 再点一下当前标签：退回这个标签的根页
        if (t == s.tab) s.stacks.getValue(t).clear() else s.tab = t
      },
      barVisible = barVisible,
      iconOnly = true,
      // 往下滚收起时，搜索在最右单独一个圆（iOS 的搜索标签就是这样）
      detachedTabIndex = Tab.Search.ordinal,
      // 下载条：搜索标签上不放（和 iOS 一样，搜索时底部只留标签栏）
      accessory = if (s.tab == Tab.Search) null else { collapsed ->
        DownloadBar(
          state = bar,
          collapsed = collapsed,
          onOpen = {
            when {
              bar.running == 0 && !bar.allPaused -> s.sheet = Sheet.Composer()
              // 已经在下载页就打开那一条的详情，不然跳到下载页
              s.tab == Tab.Downloads && top == null -> (bar.current ?: bar.firstPaused)?.let { s.sheet = Sheet.JobDetail(it) }
              else -> { s.stacks.getValue(Tab.Downloads).clear(); s.tab = Tab.Downloads }
            }
          },
          onNew = {
            // 「+」：打开输入卡。只在点它时读剪贴板（PLAN 1：不主动读），是链接就直接填好，不是链接就空着
            scope.launch {
              val text = clipboard.getClipEntry()?.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()?.trim().orEmpty()
              s.sheet = Sheet.Composer(if (looksLikeLink(text)) text else "")
            }
          },
          onPaste = {
            // 「粘贴」（没有任务时）：点了就把剪贴板里的字填进输入卡（是不是链接由输入卡认）
            scope.launch {
              val text = clipboard.getClipEntry()?.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()?.trim().orEmpty()
              s.sheet = Sheet.Composer(text)
            }
          },
        )
      },
      expandKey = stack.size,
    ) {
      when (top) {
        null -> TabRoot(s, bar.allPaused)
        is Screen.AssetDetail -> {
          // 电脑上的素材：存到手机的进度从 RemoteHub 来
          val saves by RemoteRuntime.saves.collectAsState()
          val sv = if (s.demo || !top.remote) null else (top.device ?: s.realMac?.id)?.let { saves[it to top.asset.id] }
          val saving = top.saving ?: sv?.takeIf { it.state == uniffi.captae_mobile.SaveState.RUNNING || it.state == uniffi.captae_mobile.SaveState.PAUSED }
            ?.let { if (it.totalBytes > 0u) (it.gotBytes.toDouble() / it.totalBytes.toDouble()).toFloat() else 0f }
          val saved = sv?.state == uniffi.captae_mobile.SaveState.DONE
          AssetDetailScreen(top.asset, top.remote, saving, s.macs.firstOrNull { it.id == top.device }?.name ?: s.mac?.name, onBack = { s.pop() }, saved = saved)
        }
        Screen.Settings -> SettingsScreen(
          onBack = { s.pop() },
          onProxy = { s.push(Screen.SettingsProxy) },
          onAbout = { s.push(Screen.SettingsAbout) },
          onComputers = { s.sheet = Sheet.Computers },
          proxyManual = s.proxyManual,
          demo = s.demo,
          pairedCount = s.macs.size,
        )
        Screen.SettingsProxy -> ProxyScreen(onBack = { s.pop() }, initialManual = s.proxyManual, demo = s.demo)
        Screen.SettingsAbout -> AboutScreen(onBack = { s.pop() })
        Screen.PairScan -> {
          var status by remember { mutableStateOf<String?>(null) }
          var busy by remember { mutableStateOf(false) }
          val notCaptae = str(R.string.pair_not_captae)
          val pairing = str(R.string.pair_pairing)
          ScanScreen(
            onBack = { s.pop() },
            onCode = { s.sheet = Sheet.PairCode(name = RemoteRuntime.nearby.value.firstOrNull()?.name, host = RemoteRuntime.nearby.value.firstOrNull()?.hosts?.firstOrNull(), port = RemoteRuntime.nearby.value.firstOrNull()?.port?.toInt() ?: 0) },
            demo = s.demo,
            status = status,
            busy = busy,
            onScanned = { text ->
              val hub = RemoteRuntime.hub
              val link = hub?.parsePairLink(text)
              if (hub == null || link == null) { status = notCaptae; return@ScanScreen }
              if (busy) return@ScanScreen
              busy = true
              status = cjkFormat(pairing, link.name)
              scope.launch {
                try {
                  hub.pairQr(text)
                  s.pop()
                  s.scope = DeviceScope.All
                  s.sheet = Sheet.Computers
                } catch (e: MobileException.Failed) {
                  status = e.detail.message
                } finally {
                  busy = false
                }
              }
            },
          )
        }
      }
    }
    val sheet = s.sheet
    val close = { s.sheet = null }
    // 详情 sheet 跟着任务实时变（快照里的那条换成最新的）；任务被取消、移除了就收起
    val detail = (sheet as? Sheet.JobDetail)?.item?.let { snap -> if (s.demo) snap else s.items.firstOrNull { it.job.id == snap.job.id && it.where == snap.where } }
    LaunchedEffect(sheet, detail == null) { if (sheet is Sheet.JobDetail && detail == null && !s.demo) s.sheet = null }
    JobDetailSheet(item = detail, macName = s.macs.firstOrNull { it.id == detail?.device }?.name ?: s.mac?.name, onDismiss = close)
    ShareSheet(
      sheet = sheet as? Sheet.Share,
      mac = s.mac,
      demo = s.demo,
      onDismiss = close,
      onDone = { s.sheet = null; s.tab = Tab.Downloads; s.stacks.getValue(Tab.Downloads).clear() },
    )
    ComposerSheet(
      sheet = sheet as? Sheet.Composer,
      macs = s.macs,
      onDismiss = { s.stopReading(); close() },
      onConnect = { s.sheet = Sheet.Computers },
      onSubmit = { url, dest, fmt -> s.submit(url, dest, fmt, scope) },
      onStop = { s.stopReading() },
    )
    ComputersSheet(
      visible = sheet == Sheet.Computers,
      macs = s.macs,
      demo = s.demo,
      onDismiss = close,
      onScan = { s.sheet = null; s.push(Screen.PairScan) },
      onCode = { n -> s.sheet = if (n == null) Sheet.PairCode() else Sheet.PairCode(name = n.name, host = n.hosts.firstOrNull(), port = n.port.toInt()) },
      onRename = { id, name -> RemoteRuntime.rename(id, name) },
      onRemove = { id ->
        RemoteRuntime.remove(id)
        // 正看着删掉的那台：退回全部设备（一台都不剩就是这台手机）
        if (s.scope == DeviceScope.Mac && s.selected?.id == id) { s.selectedMac = null; s.scope = if (s.macs.size > 1) DeviceScope.All else DeviceScope.Phone }
      },
    )
    PairCodeSheet(
      sheet = sheet as? Sheet.PairCode,
      onSubmit = if (s.demo) null else ({ p, digits ->
        val hub = RemoteRuntime.hub
        val host = p.host
        if (hub == null || host == null) context.getString(R.string.pair_no_nearby)
        else try {
          hub.pairCode(host, p.port.toUShort(), digits)
          s.scope = DeviceScope.All
          s.sheet = Sheet.Computers
          null
        } catch (e: MobileException.Failed) {
          e.detail.message
        }
      }),
      onDismiss = close,
      // 重新扫码：关掉 sheet，到扫码页（已经在扫码页就留在那儿）
      onRescan = { s.sheet = null; if (top != Screen.PairScan) s.push(Screen.PairScan) },
    )
  }
}

/** 剪贴板里的字像不像一条链接（http(s)://…，或者「域名/路径」这种没写协议的） */
private val LINK = Regex("""^(https?://\S+|(www\.)?[a-z0-9-]+(\.[a-z0-9-]+)+(/\S*)?)$""", RegexOption.IGNORE_CASE)

fun looksLikeLink(text: String): Boolean = text.length <= 4096 && LINK.matches(text)

/** 各标签的根页 */
@Composable
private fun TabRoot(s: AppState, allPaused: Boolean) {
  val scope = rememberCoroutineScope()
  // 主页面的顶栏：左边设备菜单，右边「更多」（BRIEF 2.1）
  val navigation: @Composable () -> Unit = {
    DeviceTitleMenu(
      scope = s.scope,
      macs = s.macs,
      selected = s.selected?.id,
      onSelect = { sc, id -> s.scope = sc; if (id != null) s.selectedMac = id; s.scopeChosen = true },
      onAddComputer = { s.push(Screen.PairScan) },
      onManageComputers = { s.sheet = Sheet.Computers },
    )
  }
  val trailing: @Composable RowScope.() -> Unit = {
    MoreButton(
      allPaused = allPaused,
      onPauseAll = { if (!s.demo) CaptaeRuntime.pauseAll() },
      onResumeAll = { if (!s.demo) CaptaeRuntime.resumeAll() },
      onClearDone = { if (!s.demo) CaptaeRuntime.clearDone() },
      onSettings = { s.push(Screen.Settings) },
    )
  }
  when (s.tab) {
    Tab.Downloads -> DownloadsScreen(
      items = s.items,
      scope = s.scope,
      macs = s.macs,
      selected = s.selected?.id,
      navigation = navigation,
      trailing = trailing,
      onOpen = { s.sheet = Sheet.JobDetail(it) },
      onConnect = { s.sheet = Sheet.Computers },
      onSubmit = { url, dest, fmt -> s.submit(url, dest, fmt, scope) },
      onTry = { url -> s.sheet = Sheet.Composer(url) },
    )
    Tab.Library -> LibraryScreen(
      scope = s.scope,
      phone = s.phoneAssets,
      phoneTotal = s.phoneTotal,
      kind = s.libKind,
      onKind = { s.libKind = it },
      onLoadMore = if (s.demo) null else ({ scope.launch { s.loadMoreLibrary() } }),
      macs = s.macSections,
      selected = s.selected?.id,
      navigation = navigation,
      trailing = trailing,
      onOpen = { a, remote, macId -> s.push(Screen.AssetDetail(if (remote && s.demo) DemoData.remoteAsset.takeIf { it.id == a.id } ?: a else a, remote, device = macId)) },
      onGoDownloads = { s.tab = Tab.Downloads },
      onShowMac = { id -> s.scope = DeviceScope.Mac; s.selectedMac = id; s.scopeChosen = true },
    )
    Tab.Search -> SearchScreen(
      initialQuery = s.query,
      items = s.items,
      phone = s.phoneAssets,
      mac = s.mac,
      macAssets = s.macSections.flatMap { it.assets },
      searchPhone = if (s.demo) null else ({ q -> CaptaeRuntime.library(null, q, 50, 0).items.map { it.toAsset() } }),
      onOpenJob = { s.sheet = Sheet.JobDetail(it) },
      onOpenAsset = { a, remote -> s.push(Screen.AssetDetail(a, remote, device = a.device)) },
    )
  }
}
