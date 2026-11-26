package app.captae.glass.controls

import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.runtime.remember
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.Animatable
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.interaction.MutableInteractionSource
import android.os.Build
import androidx.compose.animation.core.animateDpAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.LocalContentColor
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospacedDigit
import app.captae.glass.theme.weight

// 内容层的部件：照桌面端（docs/design/README.md 第 2、4 节、apps/desktop/src/theme.css），不做玻璃。

/** 圆角（桌面端 --r-*）：卡片 14、浮卡和菜单 16、输入框和弹窗 20、小控件 8 / 10 */
object Radius {
  val sm = 8.dp
  val md = 10.dp
  val lg = 14.dp
  val xl = 16.dp
  val xxl = 20.dp
}

/**
 桌面端 --sh-card：`0 0 0 .5px rgba(0,0,0,.08), 0 1px 2px rgba(0,0,0,.04)`。
 一圈 0.5 的细边 + 往下 1、模糊 2 的淡影（API 28 起硬件画布才画得出模糊影；更低只画细边）。
 */
fun Modifier.cardShadow(radius: Dp = Radius.lg): Modifier = drawBehind {
  val r = radius.toPx()
  if (Build.VERSION.SDK_INT >= 28) {
    drawIntoCanvas { c ->
      val paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
        color = android.graphics.Color.TRANSPARENT
        setShadowLayer(2.dp.toPx(), 0f, 1.dp.toPx(), Color.Black.copy(alpha = 0.04f).toArgb())
      }
      c.nativeCanvas.drawRoundRect(0f, 0f, size.width, size.height, r, r, paint)
    }
  }
  val w = 0.5.dp.toPx()
  drawRoundRect(
    Color.Black.copy(alpha = 0.08f),
    topLeft = Offset(-w / 2, -w / 2),
    size = Size(size.width + w, size.height + w),
    cornerRadius = CornerRadius(r + w / 2),
    style = Stroke(w),
  )
}

/**
 按下缩一点（docs/MOTION.md 1.2：按压 160ms ease-out-quad；大块卡片 .98、小圆按钮 .95）。动效评审第 5 条：
 整张卡、实心圆钮不用 PressDim（整块变成 30% 不透明是无边框文字按钮的反馈，放在卡上就是一闪），按下是「感觉得到」的轻微缩放。
 系统开「移除动画」时 Compose 直接跳到终点（照系统走）。
 */
@Composable
fun Modifier.pressScale(source: MutableInteractionSource, pressed: Float): Modifier {
  val down by source.collectIsPressedAsState()
  val s = remember { Animatable(1f) }
  LaunchedEffect(down) { s.animateTo(if (down) pressed else 1f, tween(160, easing = CubicBezierEasing(0.25f, 0.46f, 0.45f, 0.94f))) }
  return graphicsLayer { scaleX = s.value; scaleY = s.value }
}

/** 白卡（桌面端 .cp-job / 设置里的白卡）：白底、圆角 14、--sh-card；能点的按下缩到 .98 */
@Composable
fun Card(modifier: Modifier = Modifier, onClick: (() -> Unit)? = null, padding: Dp = 12.dp, content: @Composable ColumnScope.() -> Unit) {
  val source = remember { MutableInteractionSource() }
  Column(
    modifier
      .then(if (onClick != null) Modifier.pressScale(source, 0.98f) else Modifier)
      .cardShadow()
      .clip(RoundedCornerShape(Radius.lg))
      .background(Palette.card)
      .then(if (onClick != null) Modifier.clickable(interactionSource = source, indication = null, role = Role.Button, onClick = onClick) else Modifier)
      .padding(padding),
    content = content,
  )
}

/**
 进度条（桌面端 .cp-progress）：4 高、全圆角、底 rgba(0,0,0,.07)、填充是强调色；暂停时填充变成 --ink-4。
 indeterminate（还不知道总量）：一段淡蓝的流光停在左边（设计稿是静态截图；接上数据时照 MOTION.md 的 1.5s 流光做）。
 */
@Composable
fun ProgressBar(progress: Float, modifier: Modifier = Modifier, paused: Boolean = false, indeterminate: Boolean = false, label: String? = null) {
  val p = progress.coerceIn(0f, 1f)
  Canvas(
    modifier
      .fillMaxWidth()
      .height(4.dp)
      .semantics {
        if (label != null) contentDescription = label
        progressBarRangeInfo = if (indeterminate) ProgressBarRangeInfo.Indeterminate else ProgressBarRangeInfo(p, 0f..1f)
      },
  ) {
    val r = CornerRadius(size.height / 2)
    drawRoundRect(Color.Black.copy(alpha = 0.07f), cornerRadius = r)
    if (indeterminate) {
      drawRoundRect(
        Brush.horizontalGradient(listOf(Color.Transparent, Palette.accent.copy(alpha = 0.5f), Color.Transparent), endX = size.width * 0.45f),
        size = Size(size.width * 0.45f, size.height),
        cornerRadius = r,
      )
    } else if (p > 0f) {
      drawRoundRect(if (paused) Palette.ink4 else Palette.accent, size = Size(size.width * p, size.height), cornerRadius = r)
    }
  }
}

/** 倒计时圆环（桌面端 Countdown）：14 的圆、底圈 rgba(0,0,0,.12)、走过的部分是强调色；fraction 是剩下的比例 */
@Composable
fun CountdownRing(fraction: Float, modifier: Modifier = Modifier, size: Dp = 14.dp) {
  Canvas(modifier.size(size)) {
    val stroke = 2.dp.toPx() * this.size.minDimension / 14.dp.toPx()
    val inset = stroke / 2 + this.size.minDimension / 14f
    val s = Size(this.size.width - inset * 2, this.size.height - inset * 2)
    drawArc(Color.Black.copy(alpha = 0.12f), 0f, 360f, false, topLeft = Offset(inset, inset), size = s, style = Stroke(stroke))
    drawArc(Palette.accent, -90f, 360f * fraction.coerceIn(0f, 1f), false, topLeft = Offset(inset, inset), size = s, style = Stroke(stroke, cap = StrokeCap.Round))
  }
}

/** 步骤时间线的一步（桌面端 parts/index.tsx 的 Step） */
@Immutable
data class Step(val label: String, val sub: String? = null, val state: State = State.Todo) {
  enum class State { Done, Now, Todo, Err }
}

/**
 步骤时间线（桌面端 .cp-steps）：18 的图标 + 1 的连线；完成是墨色圈加勾，进行中是蓝色圈加实心点，
 没到的是 --ink-4 空圈，出错是 --warn 圈加感叹号。字 13（手机上用元信息字号），进行中半粗。
 */
@Composable
fun StepTimeline(steps: List<Step>, modifier: Modifier = Modifier) {
  val line = Palette.line2
  Column(modifier) {
    steps.forEachIndexed { i, st ->
      val last = i == steps.lastIndex
      Row(
        Modifier
          .fillMaxWidth()
          .drawBehind {
            if (!last) {
              val x = 9.dp.toPx()
              drawLine(line, Offset(x, 22.dp.toPx()), Offset(x, size.height - 3.dp.toPx()), strokeWidth = 1.dp.toPx())
            }
          }
          .padding(bottom = if (last) 0.dp else 14.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
      ) {
        StepIcon(st.state, Modifier.padding(top = 1.dp))
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
          val active = st.state != Step.State.Todo
          Text(
            st.label,
            style = CaptaeType.subheadline.weight(if (st.state == Step.State.Now || st.state == Step.State.Err) FontWeight.SemiBold else FontWeight.Normal),
            color = if (active) Palette.ink else Palette.ink3,
          )
          if (st.sub != null) Text(st.sub, style = CaptaeType.footnote.monospacedDigit(), color = Palette.ink3)
        }
      }
    }
  }
}

@Composable
private fun StepIcon(state: Step.State, modifier: Modifier = Modifier) {
  val color = when (state) {
    Step.State.Done -> Palette.ink
    Step.State.Now -> Palette.accent
    Step.State.Err -> Palette.warn
    Step.State.Todo -> Palette.ink4
  }
  Canvas(modifier.size(18.dp)) {
    val u = size.minDimension / 18f
    val r = 7.25f * u
    val c = Offset(9f * u, 9f * u)
    val w = 1.5f * u
    when (state) {
      Step.State.Done -> {
        drawCircle(color, r, c, style = Stroke(w))
        val path = androidx.compose.ui.graphics.Path().apply {
          moveTo(5.9f * u, 9.2f * u)
          lineTo(8f * u, 11.3f * u)
          lineTo(12.1f * u, 6.9f * u)
        }
        drawPath(path, color, style = Stroke(1.7f * u, cap = StrokeCap.Round, join = androidx.compose.ui.graphics.StrokeJoin.Round))
      }
      Step.State.Now -> {
        drawCircle(color.copy(alpha = 0.35f), r, c, style = Stroke(w))
        drawCircle(color, 3.4f * u, c)
      }
      Step.State.Todo -> drawCircle(color, r, c, style = Stroke(w))
      Step.State.Err -> {
        drawCircle(color, r, c, style = Stroke(w))
        drawLine(color, Offset(9f * u, 5.6f * u), Offset(9f * u, 9.6f * u), strokeWidth = 1.7f * u, cap = StrokeCap.Round)
        drawCircle(color, 1f * u, Offset(9f * u, 12.3f * u))
      }
    }
  }
}

/**
 分段控件（桌面端顶栏的「进行中 / 已完成 / 失败」、素材库的「全部 / 视频 / 音频 / 图片」）：浅灰底，选中项一块白色小卡、半粗字。
 轨道在 #F7F7F7 的主区上用 5% 墨色（约 #EBEBEB，和桌面端截图一致；--well 的 #F3F3F3 压在主区上几乎看不出来）。
 */
@Composable
fun <T> Segmented(options: List<T>, selected: T, label: (T) -> String, onSelect: (T) -> Unit, modifier: Modifier = Modifier, enabled: Boolean = true) {
  val index = options.indexOf(selected).coerceAtLeast(0)
  BoxWithConstraints(
    modifier
      .fillMaxWidth()
      .heightIn(min = 36.dp)
      .clip(RoundedCornerShape(Radius.md))
      .background(Color.Black.copy(alpha = 0.05f))
      .padding(2.dp),
  ) {
    val cell = maxWidth / options.size
    // 动效评审第 26 条：滑块位置只在放置阶段读（offset 的 lambda），不每帧重组、重新排版
    val x = animateDpAsState(cell * index, spring(1f, 500f), label = "segment")
    // 选中块跟着这一排的高度走（字号放大时一起变高）：先让外层量出高度，里面再填满
    Box(Modifier.matchParentSize()) {
      Box(
        Modifier
          .offset { IntOffset(x.value.roundToPx(), 0) }
          .width(cell)
          .fillMaxHeight()
          .cardShadow(Radius.sm)
          .background(Palette.card, RoundedCornerShape(Radius.sm)),
      )
    }
    Row(Modifier.fillMaxWidth().heightIn(min = 32.dp), verticalAlignment = Alignment.CenterVertically) {
      options.forEachIndexed { i, o ->
        Box(
          Modifier
            .weight(1f)
            .heightIn(min = 32.dp)
            .clickable(enabled = enabled, interactionSource = null, indication = null) { onSelect(o) }
            .semantics { role = Role.Tab; this.selected = i == index }
            .padding(horizontal = 6.dp, vertical = 6.dp),
          contentAlignment = Alignment.Center,
        ) {
          Text(
            label(o),
            style = CaptaeType.footnote.weight(if (i == index) FontWeight.SemiBold else FontWeight.Normal),
            color = if (!enabled) Palette.ink4 else if (i == index) Palette.ink else Palette.ink2,
            maxLines = 1,
            textAlign = TextAlign.Center,
          )
        }
      }
    }
  }
}

/** 内容里的主按钮（桌面端：近黑胶囊、白字）。enabled=false 时变成浅灰底灰字 */
@Composable
fun PrimaryButton(title: String, onClick: () -> Unit, modifier: Modifier = Modifier, sf: String? = null, enabled: Boolean = true) {
  Row(
    modifier
      .heightIn(min = 48.dp)
      .clip(CircleShape)
      .background(if (enabled) Palette.ink else Palette.side)
      .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
      .padding(horizontal = 22.dp, vertical = 12.dp),
    horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.CenterHorizontally),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    val c = if (enabled) Color.White else Palette.ink4
    if (sf != null) Icon(sf, size = 18.dp, weight = FontWeight.SemiBold, tint = c)
    Text(title, style = CaptaeType.headline, color = c, maxLines = 1)
  }
}

/** 次要按钮（桌面端设置里的「更改」「登录」）：白色胶囊 + --sh-card，墨色字；danger 时字是 --err */
@Composable
fun SecondaryButton(title: String, onClick: () -> Unit, modifier: Modifier = Modifier, sf: String? = null, danger: Boolean = false, compact: Boolean = false) {
  val c = if (danger) Palette.err else Palette.ink
  Row(
    modifier
      .heightIn(min = if (compact) 32.dp else 44.dp)
      .cardShadow(if (compact) 16.dp else 22.dp)
      .clip(CircleShape)
      .background(Palette.card)
      .clickable(role = Role.Button, onClick = onClick)
      .padding(horizontal = if (compact) 12.dp else 18.dp, vertical = if (compact) 6.dp else 10.dp),
    horizontalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterHorizontally),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    if (sf != null) Icon(sf, size = if (compact) 16.dp else 18.dp, weight = FontWeight.Medium, tint = c)
    Text(title, style = (if (compact) CaptaeType.footnote else CaptaeType.subheadline).weight(FontWeight.SemiBold), color = c, maxLines = 1)
  }
}

/** 文字链接按钮（桌面端「马上开始」）：强调色文字（--accent-ink） */
@Composable
fun LinkButton(title: String, onClick: () -> Unit, modifier: Modifier = Modifier) {
  Text(
    title,
    style = CaptaeType.footnote.weight(FontWeight.Medium),
    color = Palette.accentInk,
    maxLines = 1,
    modifier = modifier.clickable(role = Role.Button, onClick = onClick).padding(vertical = 4.dp),
  )
}

/** 卡片里的小图标按钮（桌面端任务行右上的暂停、更多）：32 的点击区，图标 --ink-2 */
@Composable
fun PlainIconButton(sf: String, contentDescription: String, onClick: () -> Unit, modifier: Modifier = Modifier, size: Dp = 36.dp, iconSize: Dp = 20.dp, tint: Color = Palette.ink2) {
  Box(
    modifier
      .size(size)
      .clip(CircleShape)
      .clickable(role = Role.Button, onClick = onClick)
      .semantics { this.contentDescription = contentDescription },
    contentAlignment = Alignment.Center,
  ) {
    Icon(sf, size = iconSize, tint = tint)
  }
}

// ───────────────────────── 分组列表（设置） ─────────────────────────

/** 分组的尺寸：白卡左右 16、圆角 14（和桌面端卡片同一个）、行最矮 52、字离边 16 */
object GroupedMetrics {
  val sideInset = 16.dp
  val rowHeight = 52.dp
  val rowPadding = 16.dp
  val sectionGap = 24.dp
}

/**
 桌面端设置式的分组（screenshots/settings.png）：组标题是小灰字，下面一张白卡，一行一项，行间 --line 细线（左边缩 16）。
 footer 是白卡下面的说明小字。
 */
@Composable
fun GroupedSection(modifier: Modifier = Modifier, header: String? = null, footer: String? = null, content: @Composable ColumnScope.() -> Unit) {
  Column(modifier.fillMaxWidth().padding(horizontal = GroupedMetrics.sideInset)) {
    if (header != null) {
      Text(
        header,
        style = CaptaeType.footnote.weight(FontWeight.Medium),
        color = Palette.ink3,
        modifier = Modifier.padding(start = 4.dp, bottom = 8.dp).semantics { heading() },
      )
    }
    Column(
      Modifier
        .fillMaxWidth()
        .cardShadow()
        .clip(RoundedCornerShape(Radius.lg))
        .background(Palette.card),
    ) {
      CompositionLocalProvider(LocalContentColor provides Palette.ink) { content() }
    }
    if (footer != null) {
      Text(footer, style = CaptaeType.footnote, color = Palette.ink3, modifier = Modifier.padding(start = 4.dp, end = 4.dp, top = 8.dp))
    }
  }
}

/** 分组里两行之间的细线（左边缩 16，右边到头；桌面端设置白卡里的分隔） */
@Composable
fun GroupedDivider(start: Dp = GroupedMetrics.rowPadding) {
  Box(Modifier.padding(start = start).fillMaxWidth().height(0.5.dp).background(Palette.line2))
}

/** 一行：左边标题（可带图标和说明），右边值（灰字）和尾部件；chevron 画右边的小箭头（点进下一页） */
@Composable
fun GroupedRow(
  title: String,
  modifier: Modifier = Modifier,
  sf: String? = null,
  value: String? = null,
  subtitle: String? = null,
  chevron: Boolean = false,
  /** 删除这类：字和图标用 --err（只上字和小图标，不用红底） */
  danger: Boolean = false,
  onClick: (() -> Unit)? = null,
  trailing: (@Composable RowScope.() -> Unit)? = null,
) {
  Row(
    modifier
      .fillMaxWidth()
      .heightIn(min = GroupedMetrics.rowHeight)
      .then(if (onClick != null) Modifier.clickable(role = Role.Button, onClick = onClick) else Modifier)
      .padding(horizontal = GroupedMetrics.rowPadding, vertical = 12.dp),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(12.dp),
  ) {
    if (sf != null) Icon(sf, size = 22.dp, tint = if (danger) Palette.err else Palette.ink2)
    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
      Text(title, style = CaptaeType.body, color = if (danger) Palette.err else Palette.ink)
      if (subtitle != null) Text(subtitle, style = CaptaeType.footnote, color = Palette.ink3)
    }
    if (value != null) Text(value, style = CaptaeType.body, color = Palette.ink3, maxLines = 1, textAlign = TextAlign.End)
    trailing?.invoke(this)
    if (chevron) Icon(Sym.chevronRight, size = 18.dp, weight = FontWeight.SemiBold, tint = Palette.ink4)
  }
}

/** 单选点（选中的那一项是强调色实心圈；桌面端 --accent 只用在这类地方） */
@Composable
fun RadioDot(selected: Boolean, modifier: Modifier = Modifier) {
  Canvas(modifier.size(20.dp)) {
    val r = size.minDimension / 2
    if (selected) {
      drawCircle(Palette.accent, r)
      drawCircle(Color.White, r * 0.38f)
    } else {
      drawCircle(Palette.line2, r - 0.75.dp.toPx(), style = Stroke(1.5.dp.toPx()))
    }
  }
}

/** 小圆点（在线绿点、离线灰点、未读蓝点） */
@Composable
fun Dot(color: Color, modifier: Modifier = Modifier, size: Dp = 8.dp) {
  Box(modifier.size(size).background(color, CircleShape))
}
