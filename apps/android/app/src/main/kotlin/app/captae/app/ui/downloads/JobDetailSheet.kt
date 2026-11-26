package app.captae.app.ui.downloads

import app.captae.app.ui.parts.LocalReducedMotion
import app.captae.app.ui.parts.swapBlur
import app.captae.app.ui.parts.springOf
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.animation.togetherWith
import androidx.compose.animation.slideOut
import androidx.compose.animation.slideIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.fadeIn
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.AnimatedContent
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.layout
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.captae.app.R
import app.captae.app.model.DownloadItem
import app.captae.app.model.JobState
import app.captae.app.model.Where
import app.captae.app.model.mb
import app.captae.app.ui.JobOp
import app.captae.app.ui.LocalAssetActions
import app.captae.app.ui.LocalJobActions
import app.captae.app.ui.parts.CoverGlow
import app.captae.app.ui.parts.FileBadge
import app.captae.app.ui.parts.LiveCountdownRing
import app.captae.app.ui.parts.SiteMark
import app.captae.app.ui.parts.StatusLine
import app.captae.app.ui.parts.Thumb
import app.captae.app.ui.parts.dotJoin
import app.captae.app.ui.parts.rememberNow
import app.captae.app.ui.parts.str
import app.captae.app.ui.parts.stillRes
import app.captae.app.ui.parts.rememberCover
import app.captae.glass.controls.CountdownRing
import app.captae.glass.controls.PrimaryButton
import app.captae.glass.controls.RadioDot
import app.captae.glass.controls.Radius
import app.captae.glass.controls.SecondaryButton
import app.captae.glass.controls.StepTimeline
import app.captae.glass.controls.cardShadow
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.overlay.GlassSheet
import app.captae.glass.overlay.SheetDetent
import app.captae.glass.overlay.SheetHeader
import app.captae.glass.overlay.SheetMetrics
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospaced
import app.captae.glass.theme.monospacedDigit
import app.captae.glass.theme.weight

/**
 下载详情（BRIEF 02，桌面端右侧浮卡的手机版）：同窗口的玻璃 sheet，中档打开、能拉到全高。版式照 iOS 02-running / 02-pick：
 - 顶上是这条视频的封面大图，贴到 sheet 顶边（标题栏压在封面上，滚动边缘把封面顶上糊掉，BRIEF 2.6 封面延伸的光）；
   没有封面（读取中、出错的网址）时不放大图，只在顶上铺一层很淡的光（有封面时也没有）；
 - 标题、「网站 · 在哪台设备」、链接（蓝色等宽）；
 - 一排操作：暂停 / 继续 + 取消下载（出错时重试 + 移除，下完时打开 + 移除）；选清晰度时这一排换成倒计时 +「马上开始」；
 - 要用户做决定的（清晰度、条目、出错的下一步）在操作下面，然后是「进行到哪一步」「会保存为」。
 sheet 是玻璃，里面的白卡是内容，不再套玻璃。
 */
@Composable
fun JobDetailSheet(item: DownloadItem?, macName: String?, onDismiss: () -> Unit) {
  // 关的时候 item 已经是 null，退场动画里还要画上一次的内容
  var last by remember { mutableStateOf(item) }
  if (item != null) last = item
  GlassSheet(
    isPresented = item != null,
    onDismiss = onDismiss,
    detents = listOf(SheetDetent.Medium, SheetDetent.Large),
    header = { SheetHeader(title = str(R.string.detail_title), onClose = onDismiss) },
  ) {
    val shown = last ?: return@GlassSheet
    val j = shown.job
    val hasCover = stillRes(j.thumb) != null || rememberCover(j.thumb).image != null
    val view = viewOf(j)
    Box(Modifier.fillMaxSize()) {
      Column(
        Modifier
          .fillMaxSize()
          .verticalScroll(rememberScrollState())
          .padding(bottom = 32.dp),
        verticalArrangement = Arrangement.spacedBy(14.dp),
      ) {
        if (hasCover && !j.audio) CoverHero(shown) else Spacer(Modifier.height(4.dp))
        Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(14.dp)) {
          DetailHead(shown, macName, withThumb = !hasCover || j.audio)
          // 动效评审「两端不一致」：任务详情里随状态变的那一块（操作那一排、清晰度、条目、出错）也做状态互换，和任务卡同一套
          // （从上方 8dp + 4dp 模糊进来、旧的往下出，弹簧 0.3s，高度跟着走）；带进度条的几种之间不播
          val reduced = LocalReducedMotion.current
          val shift = with(LocalDensity.current) { 8.dp.roundToPx() }
          AnimatedContent(
            targetState = view,
            contentKey = { it.group() },
            transitionSpec = {
              val fade = springOf<Float>(0.3f)
              val move = springOf(0.3f, IntOffset.VisibilityThreshold)
              ((fadeIn(fade) + slideIn(move) { IntOffset(0, -shift) }) togetherWith (fadeOut(fade) + slideOut(move) { IntOffset(0, shift) }))
                .using(SizeTransform(clip = false) { _, _ -> springOf(0.3f, IntSize.VisibilityThreshold) })
            },
            label = "detailBlock",
          ) { v0 ->
            val v = if (v0.group() == view.group()) view else v0
            Column(Modifier.fillMaxWidth().then(if (reduced) Modifier else swapBlur()), verticalArrangement = Arrangement.spacedBy(14.dp)) {
              if (v == JobView.Pick) PickBar(shown) else ActionRow(shown, v)
              when (v) {
                JobView.Pick -> PickSection(shown)
                JobView.Choose -> EntriesSection(shown)
                JobView.Login, JobView.Verify, JobView.Fail -> FailSection(shown, macName)
                else -> {}
              }
            }
          }
          Section(str(R.string.detail_steps)) { StepTimeline(stepsOf(shown, concurrency())) }
          Section(str(R.string.detail_save_as)) { SaveAs(shown, macName) }
        }
      }
    }
  }
}

private fun concurrency(): Int = app.captae.app.core.CaptaeRuntime.settings.value?.concurrency?.toInt() ?: 3

/** 封面大图：16:9 铺满 sheet 的宽，往上伸到 sheet 顶边（伸进标题栏那一段，不占内容的高度） */
@Composable
private fun CoverHero(item: DownloadItem) {
  val pad = SheetMetrics.headerTop + SheetMetrics.headerHeight + 8.dp
  Thumb(
    item.job.thumb,
    Modifier
      .fillMaxWidth()
      .layout { m, c ->
        val up = pad.roundToPx()
        val w = c.maxWidth
        val h = w * 9 / 16
        val p = m.measure(c.copy(minHeight = h, maxHeight = h))
        layout(w, (h - up).coerceAtLeast(0)) { p.place(0, -up) }
      },
    dur = item.job.dur,
    plat = item.job.plat,
    corner = 0.dp,
  )
}

/** 头：（没有大图时左边一张小封面）标题、「网站 · 设备」、链接 */
@Composable
private fun DetailHead(item: DownloadItem, macName: String?, withThumb: Boolean) {
  val j = item.job
  Box {
    // 没有大图：顶上一层很淡的光（有封面才有颜色；读取中没封面就什么都不画）
    if (withThumb) CoverGlow(j.thumb, Modifier.padding(horizontal = 0.dp), height = 200.dp, strength = 0.35f)
    Row(horizontalArrangement = Arrangement.spacedBy(12.dp), verticalAlignment = Alignment.CenterVertically) {
      if (withThumb) Thumb(j.thumb, Modifier.width(96.dp).height(54.dp), dur = j.dur, plat = j.plat)
      Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Text(if (j.untitled) str(R.string.detail_untitled) else j.title, style = CaptaeType.title3, maxLines = 3)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
          SiteMark(j.plat, size = 15.dp)
          // 网站 · 在哪台设备（BRIEF 2.8 第 4 条：「哔哩哔哩 · 这台手机」，电脑上的写电脑名）
          Text(dotJoin(siteLabel(j), if (item.where == Where.Computer) macName ?: str(R.string.detail_on_mac) else str(R.string.device_phone)), style = CaptaeType.subheadline, color = Palette.ink3, maxLines = 2)
        }
        Text(
          j.url.removePrefix("https://").removePrefix("http://").removePrefix("www."),
          style = CaptaeType.footnote.monospaced(),
          color = Palette.accentInk,
          maxLines = 1,
          overflow = TextOverflow.MiddleEllipsis,
        )
      }
    }
  }
}

/** 一排操作（iOS 02-running：白色胶囊，不铺满宽） */
@Composable
private fun ActionRow(item: DownloadItem, view: JobView) {
  val j = item.job
  val actions = LocalJobActions.current
  val assets = LocalAssetActions.current
  Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
    when {
      j.recording -> {
        SecondaryButton(str(R.string.job_stop_record), onClick = { actions.act(item, JobOp.Stop) }, sf = Sym.stop)
        SecondaryButton(str(R.string.ctx_cancel_record), onClick = { actions.act(item, JobOp.Cancel) }, sf = Sym.xmark)
      }
      j.state == JobState.Done -> {
        val id = j.assetId
        if (id != null && item.where == Where.Phone) SecondaryButton(str(R.string.asset_play), onClick = { assets.open(app.captae.app.model.Asset(id = id, kind = if (j.audio) "audio" else "video", title = j.title, src = "", fmt = "")) }, sf = Sym.playFill)
        SecondaryButton(str(R.string.job_remove), onClick = { actions.act(item, JobOp.Remove) }, sf = Sym.xmark)
      }
      view == JobView.Login || view == JobView.Verify || view == JobView.Fail -> {
        SecondaryButton(str(R.string.job_retry), onClick = { actions.act(item, JobOp.Retry) }, sf = Sym.arrowClockwise)
        SecondaryButton(str(R.string.job_remove), onClick = { actions.act(item, JobOp.Remove) }, sf = Sym.xmark)
      }
      else -> {
        if (j.state == JobState.Pause) SecondaryButton(str(R.string.job_resume), onClick = { actions.act(item, JobOp.Resume) }, sf = Sym.playFill)
        else SecondaryButton(str(R.string.job_pause), onClick = { actions.act(item, JobOp.Pause) }, sf = Sym.pauseFill)
        SecondaryButton(str(R.string.ctx_cancel_download), onClick = { actions.act(item, JobOp.Cancel) }, sf = Sym.xmark)
      }
    }
  }
}

/** 一节：小标题 + 白卡 */
@Composable
private fun Section(title: String, content: @Composable ColumnScope.() -> Unit) {
  Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
    Text(title, style = CaptaeType.footnote.weight(FontWeight.Medium), color = Palette.ink3, modifier = Modifier.padding(start = 4.dp).semantics { heading() })
    Column(
      Modifier.fillMaxWidth().cardShadow().clip(RoundedCornerShape(Radius.lg)).background(Palette.card).padding(14.dp),
      content = content,
    )
  }
}

/** 选清晰度时操作那一排（iOS 02-pick）：倒计时圆环 +「3 秒后自动用 2160p」+ 右边近黑「马上开始」 */
@Composable
private fun PickBar(item: DownloadItem) {
  val j = item.job
  val actions = LocalJobActions.current
  val now = rememberNow(1000)
  Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
    val until = j.pickUntil
    if (until != null) LiveCountdownRing(until, pickSecondsTotal(), size = 22.dp) else CountdownRing(item.pickLeft / 5f, size = 22.dp)
    Text(str(R.string.job_auto_pick, pickLeftOf(item, now), j.variantName()), style = CaptaeType.subheadline.monospacedDigit(), color = Palette.ink2, modifier = Modifier.weight(1f))
    PrimaryButton(str(R.string.job_start_now), onClick = { actions.act(item, JobOp.PickGo) })
  }
}

/** 清晰度（02-pick）：单选的几行：分辨率名、「最清晰 · 3840×1600」、右边大小 */
@Composable
private fun PickSection(item: DownloadItem) {
  val j = item.job
  val actions = LocalJobActions.current
  Section(str(R.string.detail_quality)) {
    j.vars.forEachIndexed { i, v ->
      if (i > 0) Box(Modifier.fillMaxWidth().height(0.5.dp).background(Palette.line2))
      Row(
        Modifier.fillMaxWidth().heightIn(min = 56.dp).clickable(role = Role.RadioButton, onClick = { actions.pickVariant(item, i) }),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
      ) {
        RadioDot(selected = i == j.vIdx)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
          Text(v.p, style = CaptaeType.body.weight(if (i == j.vIdx) FontWeight.SemiBold else FontWeight.Normal))
          dotJoin(variantNote(i, j.vars.size), v.r).takeIf { it.isNotEmpty() }?.let { Text(it, style = CaptaeType.footnote.monospacedDigit(), color = Palette.ink3) }
        }
        if (v.mb > 0) Text(mb(v.mb), style = CaptaeType.subheadline.monospacedDigit(), color = Palette.ink2, modifier = Modifier.width(72.dp), textAlign = TextAlign.End)
      }
    }
  }
}

/** 播放列表：勾选要下的条目，「下载选中的 3 个」（每条一个新任务） */
@Composable
private fun EntriesSection(item: DownloadItem) {
  val j = item.job
  val actions = LocalJobActions.current
  var picked by remember(j.id) { mutableStateOf(j.entries.map { it.id }.toSet()) }
  Section(str(R.string.detail_entries, j.entries.size)) {
    j.entries.forEachIndexed { i, e ->
      if (i > 0) Box(Modifier.fillMaxWidth().height(0.5.dp).background(Palette.line2))
      val on = e.id in picked
      Row(
        Modifier.fillMaxWidth().heightIn(min = 52.dp).clickable(role = Role.Checkbox, onClick = { picked = if (on) picked - e.id else picked + e.id }),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
      ) {
        Icon(if (on) Sym.checkmarkCircle else Sym.xmarkCircle, size = 20.dp, tint = if (on) Palette.accent else Palette.ink4)
        Text("${e.index}. ${e.title}", style = CaptaeType.subheadline, maxLines = 2, modifier = Modifier.weight(1f))
        if (e.dur.isNotEmpty()) Text(e.dur, style = CaptaeType.footnote.monospacedDigit(), color = Palette.ink3)
      }
    }
  }
  PrimaryButton(str(R.string.detail_entries_go, picked.size), onClick = { actions.chooseEntries(item, j.entries.map { it.id }.filter { it in picked }) }, enabled = picked.isNotEmpty(), modifier = Modifier.fillMaxWidth())
}

/** 出错（02-failed）：状态行（浅灰凹面 + 上了色的小图标 + 一句话 + 右边下一步）+ 一句补充 */
@Composable
private fun FailSection(item: DownloadItem, macName: String?) {
  val info = failInfo(item, macName)
  val actions = LocalJobActions.current
  Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
    StatusLine(info.title, info.tone, info.sf, Modifier.fillMaxWidth(), action = info.action, onAction = { actions.failAction(item) })
    if (info.hint != null) Text(info.hint, style = CaptaeType.footnote, color = Palette.ink3, modifier = Modifier.padding(horizontal = 4.dp))
  }
}

/** 会保存为：纸张格式图标 + 格式 + 位置（桌面端「会保存为」） */
@Composable
private fun SaveAs(item: DownloadItem, macName: String?) {
  val j = item.job
  val to = formatTo(j)
  Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
    FileBadge(to)
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
      val quality = when {
        j.untitled -> str(R.string.detail_decide_later)
        j.vars.isNotEmpty() -> j.variantName()
        " · " in j.fmt -> j.fmt.substringAfter(" · ")
        else -> null
      }
      Text(dotJoin(to, quality), style = CaptaeType.subheadline.weight(FontWeight.SemiBold))
      val where = if (item.where == Where.Computer && macName != null) "${saveDirOf(item)} · $macName" else saveDirOf(item)
      Text(where, style = CaptaeType.footnote, color = Palette.ink3)
    }
  }
}
