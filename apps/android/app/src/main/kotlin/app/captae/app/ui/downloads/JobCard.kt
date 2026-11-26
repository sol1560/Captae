@file:OptIn(ExperimentalLayoutApi::class)

package app.captae.app.ui.downloads

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.offset
import androidx.compose.ui.layout.layout
import app.captae.app.R
import app.captae.app.model.DownloadItem
import app.captae.app.model.Where
import app.captae.app.ui.parts.str
import app.captae.app.ui.JobOp
import app.captae.app.ui.LocalJobActions
import app.captae.app.ui.parts.Ease
import app.captae.app.ui.parts.LiveCountdownRing
import app.captae.app.ui.parts.LiveProgressBar
import app.captae.app.ui.parts.LocalReducedMotion
import app.captae.app.ui.parts.rememberNow
import app.captae.app.ui.parts.rememberShimmerPhase
import app.captae.app.ui.parts.shimmer
import app.captae.app.ui.parts.springOf
import app.captae.app.ui.parts.swapBlur
import app.captae.app.ui.parts.progressStepMillis
import app.captae.app.core.CaptaeRuntime
import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.animation.core.snap
import androidx.compose.animation.core.tween
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.scaleIn
import androidx.compose.animation.scaleOut
import androidx.compose.animation.slideIn
import androidx.compose.animation.slideOut
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.clickable
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.IntSize
import app.captae.glass.icons.Icon
import app.captae.glass.overlay.MenuAction
import app.captae.glass.overlay.MenuButton
import app.captae.glass.overlay.MenuSection
import app.captae.glass.overlay.contextMenu
import app.captae.glass.theme.monospaced
import app.captae.app.model.eta
import app.captae.app.model.mb
import app.captae.app.model.percent
import app.captae.app.model.speed
import app.captae.app.ui.parts.MetaFlow
import app.captae.app.ui.parts.SiteMark
import app.captae.app.ui.parts.StatusLine
import app.captae.app.ui.parts.Thumb
import app.captae.app.ui.parts.dotJoin
import app.captae.glass.controls.Card
import app.captae.glass.controls.CountdownRing
import app.captae.glass.controls.LinkButton
import app.captae.glass.controls.PlainIconButton
import app.captae.glass.controls.Radius
import app.captae.glass.icons.Sym
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospacedDigit
import app.captae.glass.theme.weight

/**
 下载页的一张任务卡（桌面端 pages/downloads/JobRow.tsx 的手机版）：左边封面、右边标题和来源，下面一整行是这个状态该有的东西
 （进度条和数字、清晰度选择、状态行）。点卡片打开任务详情；长按是上下文菜单（BRIEF 2.5，卡片浮起、其余洗灰）。
 会变的数字都用等宽数字，不做动画。还没读到标题时标题是网址：最多两行，用等宽字（大字号时不挤坏）；元信息可以换行。
 */
@Composable
fun JobCard(item: DownloadItem, macName: String?, onOpen: () -> Unit, modifier: Modifier = Modifier) {
  val j = item.job
  val view = viewOf(j)
  val menu = jobMenu(item, view)
  val actions = LocalJobActions.current
  Card(modifier.fillMaxWidth().contextMenu(menu), onClick = onOpen, padding = 12.dp) {
    Row {
      Thumb(j.thumb, Modifier.width(112.dp).aspectRatio(16f / 9f), dur = j.dur, plat = j.plat)
      Spacer(Modifier.width(12.dp))
      Column(Modifier.weight(1f).padding(top = 1.dp), verticalArrangement = Arrangement.spacedBy(3.dp)) {
        Text(
          j.title,
          style = if (j.untitled) CaptaeType.footnote.monospaced().weight(FontWeight.Medium) else CaptaeType.subheadline.weight(FontWeight.SemiBold),
          maxLines = 2,
        )
        Row(verticalAlignment = Alignment.Top, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
          SiteMark(j.plat, size = 13.dp, modifier = Modifier.padding(top = 2.dp))
          MetaFlow(listOfNotNull(siteLabel(j), if (j.untitled) null else j.fmt))
        }
      }
      Spacer(Modifier.width(4.dp))
      // 右上两个钮（BRIEF 2.8 第 2 条，同桌面端任务行）：暂停 / 继续（实心图标，只有能暂停的状态才有）+「…」（和长按同一份菜单）。
      // 按钮的点按区往卡片内边距里伸出 6，图标和卡片右边的距离跟 iOS 差不多，标题那一列多留点宽
      Row(Modifier.overhangEnd(6.dp)) {
        // 暂停↔继续（配方 c7）：图标先出后进，scale .5 + 透明度，100ms；减少动效时只换透明度
        val canToggle = item.pendingId == null && !j.recording && view in setOf(JobView.Dl, JobView.Probe, JobView.Pick, JobView.Queue, JobView.Merge, JobView.Pause, JobView.Choose)
        if (canToggle) {
          val reduced = LocalReducedMotion.current
          AnimatedContent(
            targetState = view == JobView.Pause,
            transitionSpec = {
              val t = tween<Float>(100, easing = Ease.OutQuint)
              if (reduced) fadeIn(t) togetherWith fadeOut(t)
              else (fadeIn(t) + scaleIn(t, initialScale = 0.5f)) togetherWith (fadeOut(t) + scaleOut(t, targetScale = 0.5f))
            },
            label = "toggle",
          ) { paused ->
            // 和状态块一样带 4dp 模糊（两端对齐）；减少动效时不糊
            Box(if (reduced) Modifier else swapBlur(100)) {
              if (paused) PlainIconButton(Sym.playFill, str(R.string.job_resume), onClick = { actions.act(item, JobOp.Resume) }, size = 30.dp, iconSize = 15.dp, tint = Palette.ink)
              else PlainIconButton(Sym.pauseFill, str(R.string.job_pause), onClick = { actions.act(item, JobOp.Pause) }, size = 30.dp, iconSize = 15.dp, tint = Palette.ink)
            }
          }
        }
        MenuButton(menu, modifier = Modifier.size(30.dp).clip(CircleShape), contentDescription = str(R.string.job_more)) {
          Icon(Sym.ellipsis, size = 18.dp, weight = FontWeight.SemiBold, tint = Palette.ink)
        }
      }
    }
    Spacer(Modifier.height(10.dp))
    // 状态块换样子（读取中 → 选清晰度 → 进度 → 完成 / 失败）：新的从上方 8 进来、旧的往下出，弹簧 0.3s，卡片高度跟着走；
    // 带进度条的几种之间（下载中、暂停、排队、处理中）不播。减少动效时只交叉淡化，高度直接到位
    val reduced = LocalReducedMotion.current
    val shift = with(LocalDensity.current) { 8.dp.roundToPx() }
    AnimatedContent(
      targetState = view,
      contentKey = { it.group() },
      transitionSpec = {
        val fade = springOf<Float>(0.3f)
        val move = springOf(0.3f, IntOffset.VisibilityThreshold)
        if (reduced) (fadeIn(fade) togetherWith fadeOut(fade)).using(SizeTransform(clip = false) { _, _ -> snap() })
        else ((fadeIn(fade) + slideIn(move) { IntOffset(0, -shift) }) togetherWith (fadeOut(fade) + slideOut(move) { IntOffset(0, shift) }))
          .using(SizeTransform(clip = false) { _, _ -> springOf(0.3f, IntSize.VisibilityThreshold) })
      },
      label = "jobBlock",
    ) { v ->
      // AnimatedContent 的内容放在 Box 里：状态块自己是一列，要包一层 Column，不然几行叠在一起
      Column(Modifier.fillMaxWidth().then(if (reduced) Modifier else swapBlur())) { JobBlock(item, if (v.group() == view.group()) view else v, macName, onOpen) }
    }
  }
}

/** 卡片下半截：按状态换内容 */
@Composable
private fun JobBlock(item: DownloadItem, view: JobView, macName: String?, onOpen: () -> Unit) {
  val j = item.job
  val num = CaptaeType.footnote.monospacedDigit()
  val actions = LocalJobActions.current
  when (view) {
    JobView.Probe -> {
      val now = rememberNow(100)
      LiveProgressBar(0f, indeterminate = true, label = str(R.string.job_reading_a11y))
      Spacer(Modifier.height(8.dp))
      Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        // 这一行的状态字一开始就在，扫光照配方 m
        Text(str(R.string.job_reading), style = CaptaeType.footnote, color = Palette.ink2, modifier = Modifier.shimmer(LocalReducedMotion.current, rememberShimmerPhase()))
        elapsedOf(item, now)?.let { Text(it, style = num, color = Palette.ink4) }
      }
    }
    JobView.Pick -> {
      val now = rememberNow(1000)
      val left = pickLeftOf(item, now)
      VariantStrip(item, onPick = { actions.pickVariant(item, it) })
      Spacer(Modifier.height(10.dp))
      Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        val until = j.pickUntil
        if (until != null) LiveCountdownRing(until, pickSecondsTotal()) else CountdownRing(item.pickLeft / 5f)
        Text(str(R.string.job_auto_pick, left, j.variantName()), style = num, color = Palette.ink2, modifier = Modifier.weight(1f, fill = false))
        Spacer(Modifier.width(4.dp))
        LinkButton(str(R.string.job_start_now), onClick = { actions.act(item, JobOp.PickGo) })
      }
    }
    JobView.Choose -> {
      Row(verticalAlignment = Alignment.CenterVertically) {
        Text(str(R.string.job_choose, j.entries.size), style = CaptaeType.footnote, color = Palette.ink2, modifier = Modifier.weight(1f))
        LinkButton(str(R.string.job_choose_action), onClick = onOpen)
      }
    }
    JobView.Dl, JobView.Merge -> {
      LiveProgressBar(if (view == JobView.Merge) (j.merge / 100).toFloat() else j.progress, indeterminate = view == JobView.Dl && j.size <= 0, label = str(R.string.job_downloaded_a11y, j.percent()), stepMillis = progressStepMillis(j.eng))
      Spacer(Modifier.height(8.dp))
      FlowRow(horizontalArrangement = Arrangement.spacedBy(10.dp), verticalArrangement = Arrangement.spacedBy(2.dp)) {
        if (view == JobView.Merge) Text(str(R.string.job_merging, j.merge.toInt()), style = num, color = Palette.ink2)
        else if (j.size > 0) {
          Text(j.percent(), style = num.weight(FontWeight.SemiBold), color = Palette.ink)
          Text("${mb(j.got)} / ${mb(j.size)}", style = num, color = Palette.ink3)
        } else Text(mb(j.got), style = num.weight(FontWeight.SemiBold), color = Palette.ink)
        if (view == JobView.Dl) Text(speed(j.speed), style = num, color = Palette.ink3)
        if (view == JobView.Dl) j.eta().takeIf { it.isNotEmpty() }?.let { Text(str(R.string.job_eta, it), style = num, color = Palette.ink3) }
      }
      if (item.note != null) {
        Text(item.note, style = CaptaeType.footnote, color = if (item.noteWarn) Palette.warn else Palette.ink3, modifier = Modifier.padding(top = 3.dp))
      }
    }
    JobView.Pause -> {
      LiveProgressBar(j.progress, paused = true, label = str(R.string.job_paused))
      Spacer(Modifier.height(8.dp))
      Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        Text(str(R.string.job_paused), style = CaptaeType.footnote, color = Palette.ink2)
        Text("${mb(j.got)} / ${mb(j.size)}", style = num, color = Palette.ink3)
      }
    }
    JobView.Queue, JobView.Starting -> if (item.pendingId != null) {
      // 等电脑上线（还在手机上存着）：没有进度条，一行说明（出错时橙色）
      Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Icon(if (item.noteWarn) Sym.exclamationmarkCircle else Sym.clock, size = 14.dp, weight = FontWeight.Medium, tint = if (item.noteWarn) Palette.warn else Palette.ink3)
        Text(item.note.orEmpty(), style = CaptaeType.footnote, color = if (item.noteWarn) Palette.warn else Palette.ink2)
      }
    } else {
      LiveProgressBar(0f, label = str(R.string.job_queued))
      Spacer(Modifier.height(4.dp))
      Row(verticalAlignment = Alignment.CenterVertically) {
        Text(
          if (view == JobView.Starting) str(R.string.job_starting) else if (item.queueAhead > 0) str(R.string.job_queued_ahead, item.queueAhead) else str(R.string.job_queued),
          style = num, color = Palette.ink2, modifier = Modifier.weight(1f),
        )
        if (view == JobView.Queue) LinkButton(str(R.string.job_start_now), onClick = { actions.act(item, JobOp.Now) })
      }
    }
    JobView.Login, JobView.Verify, JobView.Fail -> {
      val info = failInfo(item, macName)
      StatusLine(info.title, info.tone, info.sf, Modifier.fillMaxWidth(), action = info.action, onAction = { actions.failAction(item) })
    }
    JobView.Done -> Text(str(R.string.job_done, mb(j.size)), style = num, color = Palette.ink3)
  }
}

/**
 长按菜单和右上「…」（BRIEF 2.5，和左滑操作一致）：暂停 / 继续、在电脑上下 / 在手机上下、复制链接、取消（红字）。
 取消写全「取消下载」，录直播时「取消录制」（BRIEF 2.8 第 3 条，同桌面端）。出错的任务没有暂停，「取消」换成「移除」。
 */
@Composable
private fun jobMenu(item: DownloadItem, view: JobView): List<MenuSection> {
  val actions = LocalJobActions.current
  val first = buildList {
    if (item.pendingId != null) {} else when (view) {
      JobView.Pause -> add(MenuAction(str(R.string.job_resume), Sym.playFill) { actions.act(item, JobOp.Resume) })
      JobView.Login, JobView.Verify, JobView.Fail -> add(MenuAction(str(R.string.job_retry), Sym.arrowClockwise) { actions.act(item, JobOp.Retry) })
      JobView.Done -> {}
      else -> if (item.job.recording) add(MenuAction(str(R.string.job_stop_record), Sym.stop) { actions.act(item, JobOp.Stop) })
      else add(MenuAction(str(R.string.job_pause), Sym.pauseFill) { actions.act(item, JobOp.Pause) })
    }
    if (item.where == Where.Phone) add(MenuAction(str(R.string.ctx_on_mac), Sym.laptopcomputer) { actions.moveTo(item) })
    else add(MenuAction(str(R.string.ctx_on_phone), Sym.iphone) { actions.moveTo(item) })
    add(MenuAction(str(R.string.ctx_copy_link), Sym.link) { actions.copyLink(item) })
  }
  val failed = view == JobView.Login || view == JobView.Verify || view == JobView.Fail || view == JobView.Done
  return listOf(
    MenuSection(first),
    MenuSection(
      listOf(
        MenuAction(
          str(
            when {
              failed -> R.string.job_remove
              item.job.live -> R.string.ctx_cancel_record
              else -> R.string.ctx_cancel_download
            },
          ),
          Sym.xmark,
          destructive = true,
        ) { actions.act(item, if (failed) JobOp.Remove else JobOp.Cancel) },
      ),
    ),
  )
}

/**
 选清晰度（桌面端任务行里那一排）：浅灰凹面，四档一排，选中的那一档是白色小卡；每档上面分辨率、下面大小。
 */
@Composable
fun VariantStrip(item: DownloadItem, modifier: Modifier = Modifier, onPick: (Int) -> Unit = {}) {
  val j = item.job
  Row(
    modifier
      .fillMaxWidth()
      .clip(RoundedCornerShape(Radius.md))
      .background(Palette.well)
      .padding(3.dp),
    horizontalArrangement = Arrangement.spacedBy(2.dp),
  ) {
    j.vars.forEachIndexed { i, v ->
      val on = i == j.vIdx
      Box(
        Modifier
          .weight(1f)
          .heightIn(min = 44.dp)
          .clip(RoundedCornerShape(Radius.sm))
          .then(if (on) Modifier.background(Palette.card) else Modifier)
          .clickable(role = Role.RadioButton, onClick = { onPick(i) })
          .padding(vertical = 5.dp),
        contentAlignment = Alignment.Center,
      ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
          Text(v.p, style = CaptaeType.footnote.weight(if (on) FontWeight.SemiBold else FontWeight.Normal), color = if (on) Palette.ink else Palette.ink2, maxLines = 1, textAlign = TextAlign.Center)
          Text(mb(v.mb), style = CaptaeType.caption.monospacedDigit(), color = Palette.ink3, maxLines = 1, textAlign = TextAlign.Center)
        }
      }
    }
  }
}

/** 往右伸出 by：量出来照常，占位少报 by（伸进父布局的内边距里） */
private fun Modifier.overhangEnd(by: Dp): Modifier = layout { measurable, constraints ->
  val extra = by.roundToPx()
  val p = measurable.measure(constraints.offset(horizontal = extra))
  layout((p.width - extra).coerceAtLeast(0), p.height) { p.place(0, 0) }
}

/** 倒计时一共几秒（设置里的「几秒后自动选最高」，默认 5） */
fun pickSecondsTotal(): Int = CaptaeRuntime.settings.value?.pickSeconds?.toInt()?.takeIf { it > 0 } ?: 5
