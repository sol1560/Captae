package app.captae.app.ui.search

import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import kotlinx.coroutines.delay

import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.captae.app.R
import app.captae.app.model.Asset
import app.captae.app.model.Computer
import app.captae.app.model.DownloadItem
import app.captae.app.model.Where
import app.captae.app.model.percent
import app.captae.app.ui.downloads.JobView
import app.captae.app.ui.downloads.siteLabel
import app.captae.app.ui.downloads.viewOf
import app.captae.app.ui.parts.Thumb
import app.captae.app.ui.parts.WaveformCover
import app.captae.app.ui.parts.dotJoin
import app.captae.app.ui.parts.str
import app.captae.glass.controls.Radius
import app.captae.glass.controls.cardShadow
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.nav.GlassScaffold
import app.captae.glass.nav.TitleMode
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospacedDigit
import app.captae.glass.theme.weight

/** 一条搜索结果：在哪台设备上（电脑 / 这台手机） */
private class Hit(val thumb: String?, val audio: Boolean, val title: String, val onMac: Boolean, val detail: String, val onClick: () -> Unit)

/**
 搜索标签（BRIEF 2.2、5.1 的 12）：搜所有设备上的任务和素材的标题、来源。结果分「下载」「素材库」两段（白卡），
 每条注明在哪台设备（图标 + 设备名）和状态 / 格式。搜索框在内容最上面（内容层的浅灰框，不是玻璃）。
 和 iOS 一样：素材先列这台手机上的，再列电脑上的；同名的两台都列（在哪台不一样）。
 */
@Composable
fun SearchScreen(
  initialQuery: String,
  items: List<DownloadItem>,
  phone: List<Asset>,
  mac: Computer?,
  macAssets: List<Asset>,
  /** 真数据：手机素材库按词查（核心搜标题、简介、标签、来源）；演示数据时为空，就在 phone 里筛 */
  searchPhone: (suspend (String) -> List<Asset>)? = null,
  onOpenJob: (DownloadItem) -> Unit,
  onOpenAsset: (Asset, remote: Boolean) -> Unit,
) {
  var query by rememberSaveable { mutableStateOf(initialQuery) }
  val q = query.trim()
  fun hit(vararg s: String) = s.any { it.contains(q, ignoreCase = true) }
  // 按词查手机素材库：打字停 200 毫秒再查（跟着打字变的东西不动画，MOTION 1.4）
  var found by remember { mutableStateOf<List<Asset>>(emptyList()) }
  if (searchPhone != null) LaunchedEffect(q) {
    if (q.isEmpty()) { found = emptyList(); return@LaunchedEffect }
    delay(200)
    found = searchPhone(q)
  }
  val phoneName = str(R.string.device_phone)
  val jobHits = if (q.isEmpty()) emptyList() else items.filter { hit(it.job.title, siteLabel(it.job), it.job.src) }.map { item ->
    Hit(item.job.thumb, item.job.audio, item.job.title, item.where == Where.Computer, jobState(item)) { onOpenJob(item) }
  }
  val assetHits = if (q.isEmpty()) emptyList() else
    (if (searchPhone != null) found else phone.filter { hit(it.title, it.src) }).map { a -> Hit(a.thumb, a.kind == "audio", a.title, false, dotJoin(a.fmt, a.dur)) { onOpenAsset(a, false) } } +
      (if (mac == null) emptyList() else macAssets.filter { hit(it.title, it.src) }.map { a -> Hit(a.thumb, a.kind == "audio", a.title, true, dotJoin(a.fmt, a.dur)) { onOpenAsset(a, true) } })
  val state = rememberLazyListState()
  GlassScaffold(title = str(R.string.search_title), titleMode = TitleMode.Inline) { padding ->
    LazyColumn(
      Modifier.fillMaxSize(),
      state = state,
      contentPadding = PaddingValues(start = 16.dp, end = 16.dp, top = padding.calculateTopPadding() + 6.dp, bottom = padding.calculateBottomPadding() + 20.dp),
    ) {
      item(key = "field") { SearchField(query, { query = it }) }
      when {
        q.isEmpty() -> item(key = "hint") {
          Text(str(R.string.search_hint), style = CaptaeType.subheadline, color = Palette.ink3, modifier = Modifier.padding(start = 4.dp, top = 14.dp))
        }
        jobHits.isEmpty() && assetHits.isEmpty() -> item(key = "none") {
          Column(Modifier.fillMaxWidth().padding(top = 80.dp), horizontalAlignment = Alignment.CenterHorizontally) {
            Icon(Sym.magnifyingglass, size = 36.dp, tint = Palette.ink4)
            Text(str(R.string.search_none, q), style = CaptaeType.headline, textAlign = TextAlign.Center, modifier = Modifier.padding(top = 12.dp))
          }
        }
        else -> {
          if (jobHits.isNotEmpty()) {
            item(key = "h-jobs") { SectionLabel(str(R.string.search_downloads), str(R.string.search_count_jobs, jobHits.size)) }
            item(key = "jobs") { ResultCard(jobHits, mac?.name ?: "", phoneName) }
          }
          if (assetHits.isNotEmpty()) {
            item(key = "h-assets") { SectionLabel(str(R.string.search_library), str(R.string.search_count_assets, assetHits.size)) }
            item(key = "assets") { ResultCard(assetHits, mac?.name ?: "", phoneName) }
          }
        }
      }
    }
  }
}

/** 任务在搜索结果里的一句状态（和 iOS 同一套说法） */
@Composable
private fun jobState(item: DownloadItem): String = when (viewOf(item.job)) {
  JobView.Dl, JobView.Merge -> item.job.percent()
  JobView.Pause -> str(R.string.job_paused)
  JobView.Queue, JobView.Starting -> str(R.string.job_queued)
  JobView.Pick -> str(R.string.search_state_pick)
  JobView.Choose -> str(R.string.search_state_choose)
  JobView.Probe -> str(R.string.search_state_probe)
  JobView.Done -> str(R.string.search_state_done)
  else -> str(R.string.search_state_failed)
}

/** 搜索框：浅灰圆角框（内容里的，不是玻璃），左边放大镜，有字时右边清除 */
@Composable
private fun SearchField(value: String, onChange: (String) -> Unit) {
  val placeholder = str(R.string.search_placeholder)
  val clear = str(R.string.search_clear)
  Row(
    Modifier
      .fillMaxWidth()
      .heightIn(min = 44.dp)
      .clip(RoundedCornerShape(22.dp))
      .background(Palette.select)
      .padding(start = 14.dp, end = 6.dp),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(8.dp),
  ) {
    Icon(Sym.magnifyingglass, size = 18.dp, tint = Palette.ink3)
    BasicTextField(
      value = value,
      onValueChange = onChange,
      singleLine = true,
      textStyle = CaptaeType.body.copy(color = Palette.ink),
      cursorBrush = SolidColor(Palette.accent),
      keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
      modifier = Modifier.weight(1f).padding(vertical = 10.dp).semantics { contentDescription = placeholder },
      decorationBox = { inner ->
        Box(contentAlignment = Alignment.CenterStart) {
          if (value.isEmpty()) Text(placeholder, style = CaptaeType.body, color = Palette.ink3, maxLines = 1)
          inner()
        }
      },
    )
    if (value.isNotEmpty()) {
      Box(
        Modifier.size(32.dp).clip(CircleShape).clickable(role = Role.Button) { onChange("") }.semantics { contentDescription = clear },
        contentAlignment = Alignment.Center,
      ) {
        Icon(Sym.xmarkCircle, size = 18.dp, tint = Palette.ink3)
      }
    }
  }
}

/** 一段的标题：「下载 · 2 个」 */
@Composable
private fun SectionLabel(title: String, detail: String) {
  Row(Modifier.padding(start = 4.dp, top = 18.dp, bottom = 8.dp).semantics(mergeDescendants = true) { heading() }, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
    Text(title, style = CaptaeType.footnote.weight(FontWeight.SemiBold), color = Palette.ink2)
    Text("·", style = CaptaeType.footnote, color = Palette.ink4)
    Text(detail, style = CaptaeType.footnote.monospacedDigit(), color = Palette.ink3)
  }
}

/** 一张白卡里的一组结果，行间细线 */
@Composable
private fun ResultCard(hits: List<Hit>, macName: String, phoneName: String) {
  Column(Modifier.fillMaxWidth().cardShadow().clip(RoundedCornerShape(Radius.lg)).background(Palette.card)) {
    hits.forEachIndexed { i, h ->
      if (i > 0) Box(Modifier.padding(start = 88.dp).fillMaxWidth().height(0.5.dp).background(Palette.line2))
      ResultRow(h, if (h.onMac) macName else phoneName)
    }
  }
}

/** 一条：封面小图 + 标题 +「设备图标 设备名 · 状态或格式」+ 右边小箭头 */
@Composable
private fun ResultRow(h: Hit, device: String) {
  Row(
    Modifier
      .fillMaxWidth()
      .clickable(role = Role.Button, onClick = h.onClick)
      .padding(horizontal = 12.dp, vertical = 10.dp)
      .semantics(mergeDescendants = true) {},
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(12.dp),
  ) {
    val cover = Modifier.width(64.dp).height(36.dp)
    if (h.audio) WaveformCover(cover, corner = 6.dp) else Thumb(h.thumb, cover, corner = 6.dp)
    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
      Text(h.title, style = CaptaeType.subheadline.weight(FontWeight.Medium), maxLines = 2)
      Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
        Icon(if (h.onMac) Sym.laptopcomputer else Sym.iphone, size = 13.dp, tint = Palette.ink3, modifier = Modifier.padding(top = 2.dp))
        Text(dotJoin(device, h.detail), style = CaptaeType.footnote.monospacedDigit(), color = Palette.ink3, maxLines = 2)
      }
    }
    Icon(Sym.chevronRight, size = 15.dp, weight = FontWeight.SemiBold, tint = Palette.ink4)
  }
}
