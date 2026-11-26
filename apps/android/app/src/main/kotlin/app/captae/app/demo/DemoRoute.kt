package app.captae.app.demo

import androidx.compose.runtime.Immutable
import app.captae.app.model.Asset
import app.captae.app.model.DownloadItem

/** 底部三个标签（BRIEF 2.2：下载 / 素材库 / 搜索；「电脑」标签去掉了，挪进「管理电脑」sheet） */
enum class Tab { Downloads, Library, Search }

/** 标题菜单选的设备（BRIEF 2.1）：全部设备 / 这台手机 / 配对的那台电脑 */
enum class DeviceScope { All, Phone, Mac }

enum class DownloadsMode { Empty, List, Errors }
enum class LibraryMode { Empty, Grid }
enum class ComputerMode { None, Online, Offline }

/** 推进来的页（每个标签自己的返回栈里） */
sealed interface Screen {
  /** 素材详情：手机上的、电脑上的、正在存到手机（progress 0–1） */
  data class AssetDetail(val asset: Asset, val remote: Boolean, val saving: Float? = null, val device: String? = null) : Screen
  data object Settings : Screen
  data object SettingsProxy : Screen
  data object SettingsAbout : Screen
  /** 扫码配对（全屏取景）。输 6 位数是 sheet，见 [Sheet.PairCode] */
  data object PairScan : Screen
}

/** 输入卡的几种样子（BRIEF 2.4、2.6）：填链接、点了发送正在读取（流光）、选了发到电脑但电脑连不上时的提示 */
enum class ComposerMode { Edit, Reading, Offline }

/** 弹出来的东西（同一窗口里的覆盖层） */
sealed interface Sheet {
  data class JobDetail(val item: DownloadItem) : Sheet
  /** 分享卡片：演示数据时 mode 定样子；真数据时 url 是分享进来的链接，样子跟着读取的进度走 */
  data class Share(val mode: ShareMode, val url: String? = null) : Sheet
  /**
   输入卡：url 是一打开就填好的链接（从下载条「+」点开时剪贴板里是链接就填好）。
   mode = Reading 时 startedAt 是开始读取的时刻（「已经用了几秒」）；error 是加任务失败时的一句话（留在输入卡里，链接不丢）。
   */
  data class Composer(val url: String = "", val mode: ComposerMode = ComposerMode.Edit, val startedAt: Long = 0, val error: String? = null) : Sheet
  /** 管理电脑（配对、电脑列表、等电脑上线） */
  data object Computers : Sheet
  /** 输入配对码（BRIEF 2.8 第 6 条：两边都是 sheet，从「管理电脑」、扫码页进来）；failed 是配对码过期了 */
  /** 真数据时 name / host / port 是附近发现的那台电脑（6 位数配对连它） */
  data class PairCode(val failed: Boolean = false, val name: String? = null, val host: String? = null, val port: Int = 0) : Sheet
}

enum class ShareMode { Reading, Ready, Offline }

/**
 一屏的起始状态：哪个标签、看哪台设备、各页是什么状态、推进来的页、弹出来的东西。
 启动参数 `captaeDemo=<编号>-<状态>`（BRIEF 5.1 的文件名）对应一个 Scene。
 要真的操作才能截的几张（01-list-scrolled 往下滚、09-menu 点开设备菜单、11-context 长按）用同一个起始状态，截图脚本再去滚、点、长按。
 */
@Immutable
data class Scene(
  val tab: Tab = Tab.Downloads,
  /** null：按有没有配对过定（配对过是全部设备，没配对是这台手机） */
  val device: DeviceScope? = null,
  val downloads: DownloadsMode = DownloadsMode.List,
  val library: LibraryMode = LibraryMode.Grid,
  val computer: ComputerMode = ComputerMode.Online,
  val push: Screen? = null,
  val sheet: Sheet? = null,
  /** 代理设置页：手动、填了地址 */
  val proxyManual: Boolean = false,
  /** 搜索页一打开就填好的词 */
  val query: String = "",
) {
  companion object {
    const val EXTRA = "captaeDemo"
    const val DEFAULT = "01-list"

    /** YouTube 的演示链接（输入卡、分享卡片） */
    const val YOUTUBE_URL = "https://www.youtube.com/watch?v=aqz-KE-bpKQ"

    /** 第二版的全部状态（截图脚本照这个顺序跑；BRIEF 5.1） */
    val ALL = listOf(
      "01-empty", "01-list", "01-list-mac", "01-list-scrolled", "01-errors", "01-list-large",
      "09-menu",
      "10-composer", "10-composer-reading", "10-composer-offline",
      "11-context",
      "03-grid", "03-grid-mac", "03-offline", "03-empty",
      "05-none", "05-online", "05-offline",
      "12-search",
      "02-running", "02-pick", "02-failed",
      "04-video", "04-remote", "04-saving",
      "06-scan", "06-code", "06-failed",
      "07-main", "07-proxy", "07-about",
      "08-reading", "08-ready", "08-offline",
    )

    /** 认不出的编号退回 01-list */
    fun of(id: String?): Scene = when (id ?: DEFAULT) {
      // 第一次打开、没配对：只有这台手机，空状态里内嵌输入卡
      "01-empty" -> Scene(downloads = DownloadsMode.Empty, computer = ComputerMode.None)
      // 01-list-scrolled、01-list-large、09-menu、11-context 都从 01-list 开始（滚、改字号、点、长按交给截图脚本）
      "01-list", "01-list-scrolled", "01-list-large", "09-menu", "11-context" -> Scene()
      "01-list-mac" -> Scene(device = DeviceScope.Mac)
      "01-errors" -> Scene(downloads = DownloadsMode.Errors)
      "10-composer" -> Scene(sheet = Sheet.Composer(YOUTUBE_URL))
      "10-composer-reading" -> Scene(sheet = Sheet.Composer(YOUTUBE_URL, ComposerMode.Reading))
      // iOS 上架版的 10-composer-redirect 在安卓没有（完整版手机上能下 YouTube）；同一个位置换成「选了发到电脑、电脑连不上」的提示（BRIEF 2.8 第 9 条）
      "10-composer-offline" -> Scene(computer = ComputerMode.Offline, sheet = Sheet.Composer(YOUTUBE_URL, ComposerMode.Offline))
      "02-running" -> Scene(sheet = Sheet.JobDetail(DemoData.sintelRunning))
      "02-pick" -> Scene(sheet = Sheet.JobDetail(DemoData.tosPicking))
      "02-failed" -> Scene(downloads = DownloadsMode.Errors, sheet = Sheet.JobDetail(DemoData.needLogin))
      // 第一次用、什么都没有：没配对、没任务（下载条是「粘贴链接下载」）
      "03-empty" -> Scene(tab = Tab.Library, library = LibraryMode.Empty, downloads = DownloadsMode.Empty, computer = ComputerMode.None)
      "03-grid" -> Scene(tab = Tab.Library)
      "03-grid-mac" -> Scene(tab = Tab.Library, device = DeviceScope.Mac)
      "03-offline" -> Scene(tab = Tab.Library, device = DeviceScope.Mac, computer = ComputerMode.Offline)
      "04-video" -> Scene(tab = Tab.Library, push = Screen.AssetDetail(DemoData.bbb, remote = false))
      "04-remote" -> Scene(tab = Tab.Library, device = DeviceScope.Mac, push = Screen.AssetDetail(DemoData.remoteAsset, remote = true))
      "04-saving" -> Scene(tab = Tab.Library, device = DeviceScope.Mac, push = Screen.AssetDetail(DemoData.remoteAsset, remote = true, saving = 0.62f))
      "05-none" -> Scene(downloads = DownloadsMode.Empty, computer = ComputerMode.None, sheet = Sheet.Computers)
      "05-online" -> Scene(sheet = Sheet.Computers)
      "05-offline" -> Scene(computer = ComputerMode.Offline, sheet = Sheet.Computers)
      "06-scan" -> Scene(downloads = DownloadsMode.Empty, computer = ComputerMode.None, push = Screen.PairScan)
      // 输入配对码是全高的 sheet，下面垫着没配对时的下载页
      "06-code" -> Scene(downloads = DownloadsMode.Empty, computer = ComputerMode.None, sheet = Sheet.PairCode())
      "06-failed" -> Scene(downloads = DownloadsMode.Empty, computer = ComputerMode.None, sheet = Sheet.PairCode(failed = true))
      "07-main" -> Scene(push = Screen.Settings)
      "07-proxy" -> Scene(push = Screen.SettingsProxy, proxyManual = true)
      "07-about" -> Scene(push = Screen.SettingsAbout)
      "08-reading" -> Scene(sheet = Sheet.Share(ShareMode.Reading))
      "08-ready" -> Scene(sheet = Sheet.Share(ShareMode.Ready))
      "08-offline" -> Scene(computer = ComputerMode.Offline, sheet = Sheet.Share(ShareMode.Offline))
      "12-search" -> Scene(tab = Tab.Search, query = "Sintel")
      else -> Scene()
    }
  }
}
