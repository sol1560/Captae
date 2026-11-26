package app.captae.app.ui.downloads

import app.captae.glass.controls.pressScale
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.foundation.Canvas
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.Animatable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.captae.app.R
import app.captae.app.model.DownloadItem
import app.captae.app.model.JobState
import app.captae.app.model.Phase
import app.captae.app.model.percent
import app.captae.app.model.speed
import app.captae.app.ui.parts.Thumb
import app.captae.app.ui.parts.str
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospacedDigit
import app.captae.glass.theme.weight

/**
 下载条现在是哪种样子（BRIEF 2.3），从所有设备上的任务算出来（它是全局的状态，像「音乐」的迷你播放条，不跟着标题菜单变）。
 - running：在下的个数（读取信息、下载中，不算等用户选清晰度的）、总速度、第一个有进度的那条、总进度；
 - paused：没有在下的、有暂停的；
 - 都没有：空闲，「粘贴链接下载」。
 三种样子右边都有「+」（[DownloadBar]）。
 */
class BarState(items: List<DownloadItem>) {
  private val active = items.filter { it.job.state == JobState.Run && it.job.phase != Phase.Pick }
  val running = active.size
  val speed = active.sumOf { it.job.speed }
  /** 有进度的第一条（封面小图、第二行的标题和百分比） */
  val current = active.firstOrNull { it.job.phase == Phase.Dl && it.job.size > 0 } ?: active.firstOrNull()
  val progress: Float = active.filter { it.job.size > 0 }.let { l -> if (l.isEmpty()) 0f else (l.sumOf { it.job.got } / l.sumOf { it.job.size }).toFloat() }
  val paused = items.count { it.job.state == JobState.Pause }
  val allPaused = running == 0 && paused > 0
  val firstPaused = items.firstOrNull { it.job.state == JobState.Pause }
}

/**
 下载条本体：放在 GlassTabScaffold 的 accessory 里（玻璃胶囊由外壳画，这里只画里面的字和按钮，玻璃里不再套玻璃）。
 collapsed：往下滚以后缩进标签栏那一行，只剩一行（「正在下 3 个 · 41%」和「+」都留着，同 iOS）。
 右边一直有一个近黑圆形「+」（BRIEF 2.3 末尾：开始下载的入口必须一直看得见，有没有任务都在）：onNew 打开输入卡，剪贴板里是链接就填好。
 没有任务时「+」左边还留着「粘贴」（onPaste，点了直接把剪贴板填进输入卡）。
 「全部暂停 / 全部继续」不放在条里，只在右上角「更多」菜单里。
 点条的其余地方：onOpen（有任务时跳到下载页 / 打开那一条的详情，空闲时打开输入卡）。
 */
@Composable
fun BoxScope.DownloadBar(
  state: BarState,
  collapsed: Boolean,
  onOpen: () -> Unit,
  onNew: () -> Unit,
  onPaste: () -> Unit,
) {
  val open = Modifier.matchParentSize().clip(CircleShape).clickable(role = Role.Button, onClick = onOpen)
  Box(open)
  Row(
    Modifier.fillMaxWidth().padding(start = if (state.running > 0 || state.allPaused) 8.dp else 16.dp, end = 7.dp, top = 5.dp, bottom = if (state.running > 0) 7.dp else 5.dp),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(8.dp),
  ) {
    when {
      state.running > 0 -> Running(state, collapsed)
      state.allPaused -> Paused(state, collapsed)
      else -> Idle(collapsed, onPaste)
    }
    NewButton(onNew)
  }
  if (state.running > 0 && !collapsed) ProgressLine(state.progress)
}

/** 有任务在跑：封面小图、「正在下 3 个 · 11.2 MB/s」+「Sintel · 完整版短片 45%」（底边一条 2 高的总进度由外面画） */
@Composable
private fun RowScope.Running(state: BarState, collapsed: Boolean) {
  val cur = state.current
  val line1 = if (collapsed) str(R.string.bar_running_short, state.running, (state.progress * 100).toInt()) else str(R.string.bar_running, state.running, speed(state.speed))
  val thumb: Dp = if (collapsed) 28.dp else 32.dp
  Thumb(cur?.job?.thumb, Modifier.size(thumb), plat = cur?.job?.plat, corner = 6.dp)
  Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(1.dp)) {
    Text(line1, style = CaptaeType.footnote.weight(FontWeight.SemiBold).monospacedDigit(), maxLines = 1)
    if (!collapsed && cur != null) {
      Text(str(R.string.bar_current, cur.job.title, cur.job.percent()), style = CaptaeType.caption.monospacedDigit(), color = Palette.ink3, maxLines = 1)
    }
  }
}

/** 全部暂停了：封面小图 +「全部暂停了 · 3 个」（继续在「更多」菜单里） */
@Composable
private fun RowScope.Paused(state: BarState, collapsed: Boolean) {
  val p = state.firstPaused
  Thumb(p?.job?.thumb, Modifier.size(if (collapsed) 28.dp else 32.dp), plat = p?.job?.plat, corner = 6.dp)
  Text(str(R.string.bar_paused, state.paused), style = CaptaeType.footnote.weight(FontWeight.SemiBold).monospacedDigit(), maxLines = 1, modifier = Modifier.weight(1f))
}

/** 没有任务：链接图标 +「粘贴链接下载」+ 近黑「粘贴」胶囊（同 iOS 系统的 PasteButton） */
@Composable
private fun RowScope.Idle(collapsed: Boolean, onPaste: () -> Unit) {
  Icon(Sym.link, size = 20.dp, tint = Palette.ink2)
  Text(str(R.string.bar_idle), style = (if (collapsed) CaptaeType.footnote else CaptaeType.subheadline).weight(FontWeight.Medium), maxLines = 1, modifier = Modifier.weight(1f))
  Box(
    Modifier
      .heightIn(min = 32.dp)
      .clip(CircleShape)
      .background(Palette.ink)
      .clickable(role = Role.Button, onClick = onPaste)
      .padding(horizontal = 14.dp, vertical = 6.dp),
    contentAlignment = Alignment.Center,
  ) {
    Text(str(R.string.bar_paste), style = CaptaeType.footnote.weight(FontWeight.SemiBold), color = Color.White, maxLines = 1)
  }
}

/**
 底边的总进度：2 高，强调色；左右让开胶囊的圆头。动效评审第 11 条：和任务卡的进度条一样 linear 走到新值（250ms，核心推进度的间隔），
 只在绘制阶段读值（不每次重新排版）；往回走直接跳。
 */
@Composable
private fun BoxScope.ProgressLine(p: Float) {
  val target = p.coerceIn(0f, 1f)
  val anim = remember { Animatable(target) }
  LaunchedEffect(target) { if (target < anim.value) anim.snapTo(target) else anim.animateTo(target, tween(250, easing = LinearEasing)) }
  val track = Palette.line
  val fill = Palette.accent
  Canvas(
    Modifier
      .align(Alignment.BottomCenter)
      .padding(start = 22.dp, end = 22.dp, bottom = 3.dp)
      .fillMaxWidth()
      .height(2.dp),
  ) {
    val r = CornerRadius(size.height / 2)
    drawRoundRect(track, cornerRadius = r)
    val w = size.width * anim.value
    if (w > 0f) drawRoundRect(fill, size = Size(w, size.height), cornerRadius = r)
  }
}

/** 右边的圆形「+」：近黑底白色加号（和输入卡的发送钮同一种分量，是这一条里最主要的按钮；普通按钮，不是玻璃） */
@Composable
private fun NewButton(onClick: () -> Unit) {
  val label = str(R.string.bar_new)
  // 按下缩到 .95（小圆按钮，MOTION 1.2；动效评审第 5 条：不再整块变透明）
  val source = remember { MutableInteractionSource() }
  Box(
    Modifier
      .pressScale(source, 0.95f)
      .size(32.dp)
      .clip(CircleShape)
      .background(Palette.ink)
      .clickable(interactionSource = source, indication = null, role = Role.Button, onClick = onClick)
      .semantics { contentDescription = label },
    contentAlignment = Alignment.Center,
  ) {
    Icon(Sym.plus, size = 18.dp, weight = FontWeight.SemiBold, tint = Color.White)
  }
}
