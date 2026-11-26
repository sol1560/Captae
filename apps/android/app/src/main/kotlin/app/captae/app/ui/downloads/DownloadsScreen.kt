@file:OptIn(ExperimentalLayoutApi::class)

package app.captae.app.ui.downloads

import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.animation.core.Animatable
import androidx.compose.runtime.key
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.captae.app.R
import app.captae.app.demo.DemoData
import app.captae.app.demo.DeviceScope
import app.captae.app.model.Computer
import app.captae.app.model.DownloadItem
import app.captae.app.model.Where
import app.captae.app.ui.parts.CaptaeLogo
import app.captae.app.ui.parts.SiteMark
import app.captae.app.ui.parts.str
import app.captae.app.ui.parts.Ease
import app.captae.app.ui.parts.LocalReducedMotion
import app.captae.app.ui.parts.springOf
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.animation.core.tween
import androidx.compose.ui.unit.IntOffset
import app.captae.app.ui.nav.macStatus
import app.captae.glass.controls.Dot
import app.captae.glass.icons.Icon
import app.captae.glass.nav.GlassScaffold
import app.captae.glass.nav.rememberScrollPosition
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.weight

/**
 下载页（BRIEF 01，第二版）。顶栏左边是设备菜单（navigation），右边是「更多」（trailing），都由外壳给。
 - 按选中的设备显示：「全部设备」时分两组（电脑名 / 这台手机），选了某台就只看它，不再分组；
 - 没有任务（第一次打开）：居中 logo + 一句话 + 内嵌的输入卡（白卡，不是玻璃）+「试试这些链接」；
 - 常驻的输入框去掉了：下载条在标签栏上方（外壳画），点它打开输入卡。
 */
@Composable
fun DownloadsScreen(
  items: List<DownloadItem>,
  scope: DeviceScope,
  macs: List<Computer>,
  /** 设备菜单选了哪台电脑（scope 是 Mac 时） */
  selected: String?,
  navigation: @Composable () -> Unit,
  trailing: @Composable RowScope.() -> Unit,
  onOpen: (DownloadItem) -> Unit,
  onConnect: () -> Unit,
  onSubmit: (url: String, macId: String?, FmtChoice) -> Unit = { _, _, _ -> },
  onTry: (url: String) -> Unit = {},
) {
  // 每台电脑一组（BRIEF 2.1）；演示数据的电脑任务没有设备 id，算第一台的
  val mac = macs.firstOrNull()
  fun devOf(i: DownloadItem) = i.device ?: mac?.id
  val sel = macs.firstOrNull { it.id == selected } ?: mac
  val shown = when (scope) {
    DeviceScope.All -> items
    DeviceScope.Mac -> items.filter { it.where == Where.Computer && devOf(it) == sel?.id }
    DeviceScope.Phone -> items.filter { it.where == Where.Phone }
  }
  if (shown.isEmpty()) {
    GlassScaffold(navigation = navigation, trailing = trailing, topEdgeEffect = false) { padding ->
      EmptyDownloads(macs, padding, onConnect, onSubmit, onTry)
    }
    return
  }
  val state = rememberLazyListState()
  // 新任务加在最上面：列表本来就在顶上时跟着回到顶（不然 LazyColumn 会钉住原来的第一条，新任务落在看不见的上方）
  // requestScrollToItem 在这一次测量里生效、不按原来的第一条锚定（放在 LaunchedEffect 里就晚了一帧，已经被锚走）
  val newest = shown.firstOrNull()?.job?.id
  var lastNewest by remember { mutableStateOf(newest) }
  if (newest != lastNewest) {
    if (state.firstVisibleItemIndex == 0 && state.firstVisibleItemScrollOffset < 40) state.requestScrollToItem(0)
    lastNewest = newest
  }
  GlassScaffold(scroll = rememberScrollPosition(state), navigation = navigation, trailing = trailing) { padding ->
    val onPhone = shown.filter { it.where == Where.Phone }
    val grouped = scope == DeviceScope.All
    // 配方 c1 / c3：新任务淡入（弹簧 0.4s），取消、移除的淡出 150ms，其余行同时让位；超过 40 行不做让位（主线程逐帧算）。
    // 减少动效：不让位（不动位置），只留淡入淡出
    val reduced = LocalReducedMotion.current
    val many = shown.size > 40 || reduced
    val enter = springOf<Float>(0.4f)
    val place = springOf(0.4f, IntOffset.VisibilityThreshold)
    val exit = tween<Float>(150, easing = Ease.OutQuint)
    // 动效评审第 8 条：新任务除了淡入还从上方 8dp 进来（M·c1）。这一屏一开始就在的、换了设备的不播
    val seen = remember(scope, selected) { mutableSetOf<String>().also { set -> shown.forEach { set += cardKey(it, devOf(it)) } } }
    // 第 7 条：换设备是换一张新列表（key），不让旧的全部淡出、新的全部淡入
    key(scope, selected) {
    LazyColumn(
      Modifier.fillMaxSize(),
      state = state,
      contentPadding = PaddingValues(top = padding.calculateTopPadding() + if (grouped) 0.dp else 10.dp, bottom = padding.calculateBottomPadding() + 12.dp),
    ) {
      for (m in macs) {
        val onMac = shown.filter { it.where == Where.Computer && devOf(it) == m.id }
        if (onMac.isEmpty()) continue
        if (grouped) item(key = "h-mac-" + m.id) { GroupHeader(m.name, status = macStatus(m), online = m.online) }
        items(onMac, key = { cardKey(it, m.id) }) { JobCard(it, m.name, onOpen = { onOpen(it) }, modifier = Modifier.animateItem(fadeInSpec = enter, placementSpec = if (many) null else place, fadeOutSpec = exit).enterFromAbove(seen, cardKey(it, m.id), reduced).padding(horizontal = 16.dp, vertical = 5.dp)) }
      }
      if (onPhone.isNotEmpty()) {
        if (grouped) item(key = "h-phone") { GroupHeader(str(R.string.device_phone), status = null, online = null) }
        items(onPhone, key = { cardKey(it, null) }) { JobCard(it, mac?.name, onOpen = { onOpen(it) }, modifier = Modifier.animateItem(fadeInSpec = enter, placementSpec = if (many) null else place, fadeOutSpec = exit).enterFromAbove(seen, cardKey(it, null), reduced).padding(horizontal = 16.dp, vertical = 5.dp)) }
      }
    }
    }
  }
}

/** 列表里一张卡的 key：电脑上的带电脑 id，手机上的 p 开头 */
private fun cardKey(i: DownloadItem, macId: String?): String = if (i.where == Where.Phone) "p" + i.job.id else "m" + macId + ":" + i.job.id

/** 第一次出现的卡从上方 8dp 落到位（弹簧 0.4s，和 animateItem 的淡入同一个时长）；见过的、减少动效时不动 */
@Composable
private fun Modifier.enterFromAbove(seen: MutableSet<String>, key: String, reduced: Boolean): Modifier {
  val fresh = remember(key) { seen.add(key) && !reduced }
  val dy = remember(key) { Animatable(if (fresh) -8f else 0f) }
  LaunchedEffect(key) { if (dy.value != 0f) dy.animateTo(0f, springOf(0.4f)) }
  return graphicsLayer { translationY = dy.value * density }
}

/**
 分组标题（全部设备时）：设备名（深一点的灰）+ 状态小字「在线 · 正在下 3 个」；电脑那一组前面一个在线（绿）/ 离线（灰）的小点。
 大字号时状态换到下一行，不截断。
 */
@Composable
private fun GroupHeader(title: String, status: String?, online: Boolean?) {
  Row(
    Modifier.fillMaxWidth().padding(start = 20.dp, end = 16.dp, top = 14.dp, bottom = 6.dp).semantics(mergeDescendants = true) { heading() },
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(7.dp),
  ) {
    if (online != null) Dot(if (online) Palette.ok else Palette.ink4, size = 7.dp)
    FlowRow(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
      Text(title, style = CaptaeType.footnote.weight(FontWeight.SemiBold), color = Palette.ink2)
      if (status != null) Text(status, style = CaptaeType.footnote, color = Palette.ink3)
    }
  }
}

/**
 第一次打开（BRIEF 01-empty，桌面端 screenshots/empty.png）：logo、「粘贴链接，开始下载」、一句说明、输入卡（在内容中间，不是浮着的，
 所以是白卡不是玻璃）、「试试这些链接」三个胶囊。
 */
@Composable
private fun EmptyDownloads(macs: List<Computer>, padding: PaddingValues, onConnect: () -> Unit, onSubmit: (String, String?, FmtChoice) -> Unit, onTry: (String) -> Unit) {
  Box(
    Modifier
      .fillMaxSize()
      .verticalScroll(rememberScrollState())
      .padding(top = padding.calculateTopPadding(), bottom = padding.calculateBottomPadding()),
    contentAlignment = Alignment.Center,
  ) {
    Column(
      Modifier.widthIn(max = 520.dp).fillMaxWidth().padding(horizontal = 16.dp, vertical = 24.dp),
      horizontalAlignment = Alignment.CenterHorizontally,
    ) {
      CaptaeLogo(size = 56.dp)
      Spacer(Modifier.height(20.dp))
      Text(str(R.string.empty_title), style = CaptaeType.title2, textAlign = TextAlign.Center)
      Spacer(Modifier.height(6.dp))
      Text(str(R.string.empty_sub), style = CaptaeType.subheadline, color = Palette.ink3, textAlign = TextAlign.Center)
      Spacer(Modifier.height(24.dp))
      ComposerCard(macs = macs, inSheet = false, onConnect = onConnect, onSubmit = onSubmit)
      Spacer(Modifier.height(16.dp))
      Text(str(R.string.empty_tries), style = CaptaeType.footnote, color = Palette.ink3)
      Spacer(Modifier.height(8.dp))
      FlowRow(
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
        verticalArrangement = Arrangement.spacedBy(8.dp),
      ) {
        for (t in DemoData.tries) TryChip(t) { onTry(t.url) }
      }
    }
  }
}

/** 「试试这些链接」的一个（桌面端 .cp-try、iOS 同款：灰色凹面胶囊、不描边不投影，站点图标或类型图标 + 名字） */
@Composable
private fun TryChip(t: DemoData.Try, onClick: () -> Unit) {
  Row(
    Modifier
      .heightIn(min = 36.dp)
      .clip(CircleShape)
      .background(Palette.well)
      .clickable(role = Role.Button, onClick = onClick)
      .padding(horizontal = 13.dp, vertical = 8.dp),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(6.dp),
  ) {
    if (t.site != null) SiteMark(t.site, size = 14.dp) else if (t.sf != null) Icon(t.sf, size = 15.dp, tint = Palette.ink2)
    Text(t.label, style = CaptaeType.footnote.weight(FontWeight.Medium), color = Palette.ink, maxLines = 1)
  }
}
