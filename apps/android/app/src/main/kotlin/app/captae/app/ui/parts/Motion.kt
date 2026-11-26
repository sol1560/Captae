package app.captae.app.ui.parts

import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.BlurEffect
import androidx.compose.animation.EnterExitState
import androidx.compose.animation.AnimatedVisibilityScope
import android.os.Build
import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.SpringSpec
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.RepeatMode
import androidx.compose.animation.core.Spring
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.keyframes
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.spring
import androidx.compose.animation.core.tween
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.captae.glass.theme.Palette
import kotlinx.coroutines.delay
import kotlin.math.PI
import kotlin.math.ceil
import kotlin.math.max

// 动效的数值（docs/MOTION.md 第 1 节，换成 Compose 的写法）。代码注释里的「配方 c5」指 MOTION.md 第 3 节对应的小节。
// 只动 graphicsLayer（位移、缩放、透明度）和绘制阶段读的值，不在每一帧重组（课程 Performance：只让合成那一步干活）。

object Ease {
  /** 默认：进场、退场、弹层、提示（--ease-out-quint） */
  val OutQuint = CubicBezierEasing(0.23f, 1f, 0.32f, 1f)
  /** 浮在内容上的小面板（--ease-out-quart） */
  val OutQuart = CubicBezierEasing(0.165f, 0.84f, 0.44f, 1f)
  /** 按压缩放（--ease-out-quad） */
  val OutQuad = CubicBezierEasing(0.25f, 0.46f, 0.45f, 0.94f)
  /** 已经在屏幕上的往返：描线、呼吸（--ease-in-out-cubic） */
  val InOutCubic = CubicBezierEasing(0.645f, 0.045f, 0.355f, 1f)
  /** 高度变化（--ease-height） */
  val Height = CubicBezierEasing(0.25f, 1f, 0.5f, 1f)
}

/**
 不回弹的弹簧，按「感觉上的时长」给（MOTION 1.3 `{type: "spring", duration, bounce: 0}`；Apple / Motion 同一套换算：
 质量 1、阻尼比 1，刚度 = (2π / 时长)²）。0.3 秒 ≈ 439，0.4 秒 ≈ 247，0.5 秒 ≈ 158。
 */
fun <T> springOf(seconds: Float, visibilityThreshold: T? = null): SpringSpec<T> {
  val k = (2 * PI / seconds).let { it * it }.toFloat()
  return spring(dampingRatio = Spring.DampingRatioNoBouncy, stiffness = k, visibilityThreshold = visibilityThreshold)
}

/**
 系统的「减少动态效果」：安卓上是「移除动画」（无障碍）和开发者选项的动画时长缩放，都写进 ANIMATOR_DURATION_SCALE = 0。
 开着时：去掉位移和缩放，只留透明度和颜色；循环的流光停在代表帧；倒计时按秒跳（MOTION 1.5）。
 */
val LocalReducedMotion = staticCompositionLocalOf { false }

@Composable
fun ProvideReducedMotion(content: @Composable () -> Unit) {
  val context = LocalContext.current
  val resolver = context.contentResolver
  fun read() = Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
  var reduced by remember { mutableStateOf(read()) }
  DisposableEffect(resolver) {
    val obs = object : ContentObserver(Handler(Looper.getMainLooper())) {
      override fun onChange(selfChange: Boolean) { reduced = read() }
    }
    resolver.registerContentObserver(Settings.Global.getUriFor(Settings.Global.ANIMATOR_DURATION_SCALE), false, obs)
    onDispose { resolver.unregisterContentObserver(obs) }
  }
  CompositionLocalProvider(LocalReducedMotion provides reduced, content = content)
}

/**
 状态块互换时的那 4dp 模糊（MOTION 配方 c：新的从上方 8 + 4px 模糊进来、旧的往下出；和 iOS 对齐，动效评审「两端不一致」表）。
 在 AnimatedContent 的内容里用：进场从 4dp 清晰到 0，退场从 0 糊到 4dp，跟着同一条弹簧。只在 graphicsLayer 里读值；
 Android 12 以下没有 RenderEffect，就只有位移和淡入淡出。
 */
@Composable
fun AnimatedVisibilityScope.swapBlur(millis: Int? = null): Modifier {
  if (Build.VERSION.SDK_INT < 31) return Modifier
  // millis：跟着用 tween 的互换（暂停↔继续图标 100ms）；不给就是状态块的弹簧 0.3s
  val b = transition.animateFloat(transitionSpec = { if (millis != null) tween(millis, easing = Ease.OutQuint) else springOf(0.3f) }, label = "swapBlur") { if (it == EnterExitState.Visible) 0f else 4f }
  return Modifier.graphicsLayer {
    val r = b.value.dp.toPx()
    renderEffect = if (r > 0.05f) BlurEffect(r, r, TileMode.Decal) else null
  }
}

/** 进度条往前走的时长 = 核心推进度的间隔（MOTION c5，动效评审第 9 条）：yt-dlp 250ms、M3U8 400ms、直链（aria2 / Rust 直链）1s */
fun progressStepMillis(eng: app.captae.app.model.Engine): Int = when (eng) {
  app.captae.app.model.Engine.YtDlp -> 250
  app.captae.app.model.Engine.Hls -> 400
  app.captae.app.model.Engine.Aria2 -> 1000
}

/** 现在的时间，每 period 毫秒变一次（读取了几秒、倒计时还剩几秒） */
@Composable
fun rememberNow(period: Long = 1000): Long {
  var now by remember { mutableLongStateOf(System.currentTimeMillis()) }
  LaunchedEffect(period) {
    while (true) {
      // 对齐到整 period，几个计时的数字同一刻跳
      delay(period - System.currentTimeMillis() % period)
      now = System.currentTimeMillis()
    }
  }
  return now
}

/**
 进度条（配方 c5，桌面端 .cp-progress 的样子：4 高、全圆角、底 rgba(0,0,0,.07)、强调色填充，暂停时填充变 --ink-4）。
 - 新的进度用 linear 走过去，时长等于核心推进度的间隔（250 毫秒），看起来是匀速往前；刚出现时直接画当前值，不从 0 重播。
 - 不知道总量：一道流光（1.1 秒移动 + 0.4 秒停，linear）；减少动效时是一段停着的淡蓝。
 - 减少动效：直接跳到新值。
 进度值只在绘制阶段读，进度更新不触发重组以外的布局。
 */
@Composable
fun LiveProgressBar(progress: Float, modifier: Modifier = Modifier, paused: Boolean = false, indeterminate: Boolean = false, label: String? = null, stepMillis: Int = 250) {
  val reduced = LocalReducedMotion.current
  val target = progress.coerceIn(0f, 1f)
  val anim = remember { Animatable(target) }
  LaunchedEffect(target, reduced) {
    if (reduced || target < anim.value) anim.snapTo(target) else anim.animateTo(target, tween(stepMillis, easing = LinearEasing))
  }
  val sweep = if (indeterminate && !reduced) {
    val t = rememberInfiniteTransition(label = "sheen")
    t.animateFloat(0f, 1f, infiniteRepeatable(keyframes { durationMillis = 1500; 0f at 0 using LinearEasing; 1f at 1100 }), label = "sheenX")
  } else null
  val accent = Palette.accent
  val pausedColor = Palette.ink4
  Canvas(
    modifier
      .fillMaxWidth()
      .height(4.dp)
      .semantics {
        if (label != null) contentDescription = label
        progressBarRangeInfo = if (indeterminate) ProgressBarRangeInfo.Indeterminate else ProgressBarRangeInfo(target, 0f..1f)
      },
  ) {
    val r = CornerRadius(size.height / 2)
    drawRoundRect(Color.Black.copy(alpha = 0.07f), cornerRadius = r)
    if (indeterminate) {
      val w = size.width * 0.45f
      // 从左边外面扫到右边外面
      val x = sweep?.let { -w + (size.width + w) * it.value } ?: 0f
      drawRoundRect(
        Brush.horizontalGradient(listOf(Color.Transparent, accent.copy(alpha = 0.5f), Color.Transparent), startX = x, endX = x + w),
        topLeft = Offset(x.coerceAtLeast(0f), 0f),
        size = Size((x + w).coerceAtMost(size.width) - x.coerceAtLeast(0f), size.height).let { if (it.width < 0) Size.Zero else it },
        cornerRadius = r,
      )
    } else {
      val p = anim.value
      if (p > 0f) drawRoundRect(if (paused) pausedColor else accent, size = Size(size.width * p, size.height), cornerRadius = r)
    }
  }
}

/**
 倒计时圆环（配方 d）：底圈 rgba(0,0,0,.12)，剩下的部分是强调色，linear 走完剩下的秒数；减少动效时按秒跳，和旁边的数字同一刻变。
 until：倒计时结束的时刻（毫秒）；total：整个倒计时有几秒（算比例）。
 */
@Composable
fun LiveCountdownRing(until: Long, totalSeconds: Int, modifier: Modifier = Modifier, size: Dp = 14.dp) {
  val reduced = LocalReducedMotion.current
  val totalMs = max(1, totalSeconds) * 1000f
  fun frac() = ((until - System.currentTimeMillis()) / totalMs).coerceIn(0f, 1f)
  val anim = remember(until) { Animatable(frac()) }
  val now = rememberNow(1000)
  LaunchedEffect(until, reduced) {
    if (reduced) return@LaunchedEffect
    val left = (until - System.currentTimeMillis()).coerceAtLeast(0)
    anim.snapTo(frac())
    anim.animateTo(0f, tween(left.toInt(), easing = LinearEasing))
  }
  // 减少动效：剩下的整秒数 / 总秒数
  val stepped = (ceil((until - now) / 1000.0).coerceAtLeast(0.0) * 1000 / totalMs).toFloat().coerceIn(0f, 1f)
  val accent = Palette.accent
  Canvas(modifier.size(size)) {
    val f = if (reduced) stepped else anim.value
    val stroke = 2.dp.toPx() * this.size.minDimension / 14.dp.toPx()
    val inset = stroke / 2 + this.size.minDimension / 14f
    val s = Size(this.size.width - inset * 2, this.size.height - inset * 2)
    drawArc(Color.Black.copy(alpha = 0.12f), 0f, 360f, false, topLeft = Offset(inset, inset), size = s, style = Stroke(stroke))
    drawArc(accent, -90f, 360f * f, false, topLeft = Offset(inset, inset), size = s, style = Stroke(stroke, cap = StrokeCap.Round))
  }
}

/**
 扫光文字（配方 m，桌面端 cp-wait）：一次 2.7 秒，前 21% 一道亮光从左扫到右、其余停住。字本身是 --ink-3，光是叠在字上的一段提亮
 （只画在字形里面：SrcAtop）。减少动效时不扫，就是普通的灰字。
 */
fun Modifier.shimmer(reduced: Boolean, phase: () -> Float): Modifier =
  if (reduced) this else this
    .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
    .drawWithContent {
      drawContent()
      val p = phase()
      if (p in 0f..1f) {
        val w = size.width * 0.5f
        val x = -w + (size.width + w) * p
        drawRect(Brush.horizontalGradient(listOf(Color.Transparent, Color.White.copy(alpha = 0.75f), Color.Transparent), startX = x, endX = x + w), blendMode = BlendMode.SrcAtop)
      }
    }

/** 扫光的相位：0–1 在扫，大于 1 是停着（2.7 秒一次，扫的部分占 21%） */
@Composable
fun rememberShimmerPhase(): () -> Float {
  val t = rememberInfiniteTransition(label = "wait")
  val v = t.animateFloat(
    0f, 1f / 0.21f,
    infiniteRepeatable(tween(2700, easing = LinearEasing), RepeatMode.Restart),
    label = "waitPhase",
  )
  return { v.value }
}
