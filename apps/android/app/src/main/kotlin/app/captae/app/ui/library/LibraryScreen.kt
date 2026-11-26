package app.captae.app.ui.library

import androidx.annotation.StringRes
import androidx.compose.foundation.clickable
import androidx.compose.ui.semantics.Role
import androidx.compose.foundation.layout.RowScope
import app.captae.app.R
import app.captae.app.ui.LocalAssetActions
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import app.captae.app.demo.DeviceScope
import app.captae.app.ui.parts.Str
import app.captae.app.ui.parts.str
import app.captae.glass.controls.Dot
import app.captae.glass.overlay.MenuAction
import app.captae.glass.overlay.MenuSection
import app.captae.glass.overlay.contextMenu
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.GridItemSpan
import androidx.compose.foundation.lazy.grid.LazyGridState
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.lazy.grid.rememberLazyGridState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.captae.app.demo.DemoData
import app.captae.app.model.Asset
import app.captae.app.model.Computer
import app.captae.app.ui.parts.SiteMark
import app.captae.app.ui.parts.Thumb
import app.captae.app.ui.parts.WaveformCover
import app.captae.app.ui.parts.dotJoin
import app.captae.glass.controls.Card
import app.captae.glass.controls.PrimaryButton
import app.captae.glass.controls.Radius
import app.captae.glass.controls.Segmented
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.nav.GlassScaffold
import app.captae.glass.nav.ScrollPosition
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.weight

/** 素材类型分段（桌面端素材库顶栏） */
enum class Kind(@StringRes val label: Int, val wire: String?) { All(R.string.kind_all, null), Video(R.string.kind_video, "video"), Audio(R.string.kind_audio, "audio"), Image(R.string.kind_image, "image") }

/**
 素材库（BRIEF 03，第二版）：按标题菜单选的设备显示，原来顶上的「这台手机 / 电脑」分段控件去掉了。
 - 全部设备：分两段（这台手机 · 6 项 / 电脑名 · 128 项）；选了某台就只看它；
 - 这台手机上没有素材、也没有电脑（03-empty）：一句话 + 去下载页；
 - 选了电脑、电脑不在线（03-offline）：上面一张说明卡，下面是上次看到的内容，变灰、点不了。
 搜索挪到了「搜索」标签（搜所有设备），这里不再放搜索框。
 */
@Composable
fun LibraryScreen(
  scope: DeviceScope,
  phone: List<Asset>,
  phoneTotal: Int,
  kind: Kind,
  onKind: (Kind) -> Unit,
  onLoadMore: (() -> Unit)?,
  /** 配对过的每台电脑一段（最近一次读到的内容） */
  macs: List<MacSection>,
  /** scope 是 Mac 时选的是哪台 */
  selected: String?,
  navigation: @Composable () -> Unit,
  trailing: @Composable RowScope.() -> Unit,
  onOpen: (Asset, remote: Boolean, macId: String?) -> Unit,
  onGoDownloads: () -> Unit,
  onShowMac: (String) -> Unit,
) {
  val state = rememberLazyGridState()
  // 分页（配方 j：不动画）：滚到离底部 8 张以内读下一页
  if (onLoadMore != null) {
    val nearEnd by remember(state) { derivedStateOf { state.layoutInfo.totalItemsCount > 0 && (state.layoutInfo.visibleItemsInfo.lastOrNull()?.index ?: 0) >= state.layoutInfo.totalItemsCount - 8 } }
    LaunchedEffect(nearEnd, phone.size) { if (nearEnd && phone.size < phoneTotal) onLoadMore() }
  }
  val sel = macs.firstOrNull { it.mac.id == selected } ?: macs.firstOrNull()
  val shownMacs = when (scope) {
    DeviceScope.Phone -> emptyList()
    DeviceScope.Mac -> listOfNotNull(sel)
    DeviceScope.All -> macs
  }
  val showPhone = scope != DeviceScope.Mac || sel == null
  val showMac = shownMacs.isNotEmpty()
  // 只看一台、它又不在线：上面一张说明卡，类型分段点不了
  val singleOffline = scope == DeviceScope.Mac && sel != null && !sel.mac.online
  fun filter(l: List<Asset>) = l.filter { kind.wire == null || it.kind == kind.wire }
  GlassScaffold(scroll = rememberGridScroll(state), navigation = navigation, trailing = trailing) { padding ->
    LazyVerticalGrid(
      columns = GridCells.Fixed(2),
      state = state,
      modifier = Modifier.fillMaxSize(),
      contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = padding.calculateTopPadding() + 8.dp, bottom = padding.calculateBottomPadding() + 16.dp),
      horizontalArrangement = Arrangement.spacedBy(10.dp),
      verticalArrangement = Arrangement.spacedBy(10.dp),
    ) {
      val empty = showPhone && !showMac && phone.isEmpty() && kind == Kind.All
      if (empty) {
        full("empty") { EmptyLibrary(onGoDownloads) }
        return@LazyVerticalGrid
      }
      if (singleOffline) full("offline") { OfflineNotice(sel.mac) }
      full("kind") { Segmented(Kind.entries, kind, label = { Str.get(it.label) }, onSelect = onKind, enabled = !singleOffline) }
      val grouped = showPhone && showMac
      // 电脑那段在前（和下载页、iOS 一样）；全部设备时只放最新的 4 个，下面「查看全部 128 项 ›」切到电脑
      for (sec in shownMacs) {
        val mac = sec.mac
        val offline = !mac.online
        full("h-mac-" + mac.id) {
          val count = sec.total
          SectionHeader(
            when {
              offline && sec.total > 0 -> if (grouped) dotJoinOf(mac.name, str(R.string.library_last_seen, count, mac.lastSeen)) else str(R.string.library_last_seen, count, mac.lastSeen)
              grouped -> str(R.string.library_section, mac.name, count)
              else -> str(R.string.library_count, count)
            },
            online = if (grouped) mac.online else null,
          )
        }
        val list = filter(sec.assets).let { if (grouped) it.take(4) else it }
        items(list, key = { "m" + mac.id + ":" + it.id }) { a ->
          AssetTile(a, remote = true, macName = mac.name, dim = offline, onClick = if (offline) null else ({ onOpen(a, true, mac.id) }))
        }
        if (grouped && sec.total > 4) full("see-all-" + mac.id) { SeeAll(sec.total) { onShowMac(mac.id) } }
      }
      if (showPhone) {
        full("h-phone") {
          SectionHeader(if (grouped) str(R.string.library_section, str(R.string.device_phone), phoneTotal) else str(R.string.library_count, phoneTotal), online = null)
        }
        items(phone, key = { "p" + it.id }) { a -> AssetTile(a, remote = false, macName = macs.firstOrNull()?.mac?.name, onClick = { onOpen(a, false, null) }) }
        if (phone.isEmpty() && kind != Kind.All) full("none") {
          Text(str(R.string.library_none_kind), style = CaptaeType.subheadline, color = Palette.ink3, modifier = Modifier.padding(start = 4.dp, top = 8.dp))
        }
      }
    }
  }
}

/** 一台电脑的素材库：最近一次读到的那一页和总数 */
data class MacSection(val mac: Computer, val assets: List<Asset>, val total: Int)

private fun dotJoinOf(a: String, b: String) = "$a · $b"

/** 「查看全部 128 项 ›」：强调色小字链接（桌面端链接色），点了切到电脑 */
@Composable
private fun SeeAll(count: Int, onClick: () -> Unit) {
  Row(
    Modifier.padding(start = 4.dp, bottom = 8.dp).clip(RoundedCornerShape(6.dp)).clickable(role = Role.Button, onClick = onClick).padding(vertical = 4.dp),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(3.dp),
  ) {
    Text(str(R.string.library_see_all, count), style = CaptaeType.footnote.weight(FontWeight.Medium), color = Palette.accentInk)
    Icon(Sym.chevronRight, size = 13.dp, weight = FontWeight.SemiBold, tint = Palette.accentInk)
  }
}

/** 一段的标题：小灰字（全部设备时前面一个电脑的在线点） */
@Composable
private fun SectionHeader(title: String, online: Boolean?) {
  Row(
    Modifier.fillMaxWidth().padding(start = 4.dp, top = 6.dp).semantics { heading() },
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(7.dp),
  ) {
    if (online != null) Dot(if (online) Palette.ok else Palette.ink4, size = 7.dp)
    Text(title, style = CaptaeType.footnote.weight(FontWeight.Medium), color = Palette.ink3, maxLines = 2)
  }
}

/** 网格里占满一整行的一项 */
private fun androidx.compose.foundation.lazy.grid.LazyGridScope.full(key: String, content: @Composable () -> Unit) {
  item(key = key, span = { GridItemSpan(maxLineSpan) }) { content() }
}

/** 网格的滚动位置（大标题收起用；GlassScaffold 自带的只认列表和普通滚动） */
@Composable
private fun rememberGridScroll(state: LazyGridState): ScrollPosition =
  remember(state) { ScrollPosition { if (state.firstVisibleItemIndex > 0) Int.MAX_VALUE / 2 else state.firstVisibleItemScrollOffset } }

/**
 素材卡（桌面端素材库网格）：白卡里封面（16:9、圆角 10、时长角标）、标题、来源，再一行格式和时间。
 长按是上下文菜单（BRIEF 2.5）：分享、存到手机 / 发到电脑、在「文件」中查看、删除（红字）。
 */
@Composable
fun AssetTile(a: Asset, remote: Boolean, macName: String?, dim: Boolean = false, onClick: (() -> Unit)?) {
  Card(Modifier.fillMaxWidth().alpha(if (dim) 0.55f else 1f).contextMenu(assetMenu(a, remote), enabled = !dim), onClick = onClick, padding = 6.dp) {
    val cover = Modifier.fillMaxWidth().aspectRatio(16f / 9f)
    if (a.kind == "audio") WaveformCover(cover, dur = a.dur, corner = Radius.md, dim = dim)
    else Thumb(a.thumb, cover, dur = a.dur, plat = a.plat, corner = Radius.md, dim = dim)
    Column(Modifier.padding(start = 4.dp, end = 4.dp, top = 8.dp, bottom = 4.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
      Text(a.title, style = CaptaeType.subheadline.weight(FontWeight.SemiBold), maxLines = 1)
      Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        SiteMark(a.plat, size = 12.dp)
        Text(dotJoin(a.src, a.fmt), style = CaptaeType.caption, color = Palette.ink3, maxLines = 1)
      }
      Text(a.`when`, style = CaptaeType.caption, color = Palette.ink3, maxLines = 1)
    }
  }
}

/** 素材的长按菜单：手机上的「分享、发到电脑、在「文件」中查看、删除」；电脑上的「分享、存到手机、删除」 */
@Composable
private fun assetMenu(a: Asset, remote: Boolean): List<MenuSection> {
  val act = LocalAssetActions.current
  val first = buildList {
    add(MenuAction(str(R.string.ctx_share), Sym.squareAndArrowUp) { act.share(a) })
    if (remote) add(MenuAction(str(R.string.ctx_save_phone), Sym.squareAndArrowDown) { act.transfer(a, true) })
    else add(MenuAction(str(R.string.ctx_send_mac), Sym.laptopcomputer) { act.transfer(a, false) })
    if (!remote) add(MenuAction(str(R.string.ctx_show_files), Sym.folder) { act.showFiles(a) })
  }
  return listOf(MenuSection(first), MenuSection(listOf(MenuAction(str(R.string.ctx_delete), Sym.trash, destructive = true) { act.delete(a) })))
}

/** 03-empty：一句话 + 去下载页 */
@Composable
private fun EmptyLibrary(onGoDownloads: () -> Unit) {
  Column(
    Modifier.fillMaxWidth().padding(top = 96.dp, bottom = 48.dp),
    horizontalAlignment = Alignment.CenterHorizontally,
  ) {
    Icon(Sym.photoOnRectangleAngled, size = 40.dp, tint = Palette.ink4)
    Spacer(Modifier.height(16.dp))
    Text(str(R.string.library_empty_title), style = CaptaeType.headline, textAlign = TextAlign.Center)
    Spacer(Modifier.height(6.dp))
    Text(str(R.string.library_empty_sub), style = CaptaeType.subheadline, color = Palette.ink3, textAlign = TextAlign.Center)
    Spacer(Modifier.height(24.dp))
    PrimaryButton(str(R.string.library_go_downloads), onClick = onGoDownloads, sf = Sym.arrowDownCircle)
  }
}

/** 03-offline：电脑连不上。白卡里灰色的断网小图标 + 一句话 + 怎么办（不用彩色大底块） */
@Composable
private fun OfflineNotice(mac: Computer) {
  Card(Modifier.fillMaxWidth(), padding = 14.dp) {
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
      Icon(Sym.wifiSlash, size = 22.dp, tint = Palette.ink3, modifier = Modifier.padding(top = 1.dp))
      Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Text(str(R.string.mac_unreachable, mac.name), style = CaptaeType.subheadline.weight(FontWeight.SemiBold))
        Text(str(R.string.mac_unreachable_hint), style = CaptaeType.footnote, color = Palette.ink2)
      }
    }
  }
}
