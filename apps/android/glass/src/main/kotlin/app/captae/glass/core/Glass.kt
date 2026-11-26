package app.captae.glass.core

import android.os.Build
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.GraphicsLayerScope
import androidx.compose.ui.graphics.drawOutline
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.DpOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.util.fastCoerceAtMost
import androidx.compose.ui.util.lerp
import com.kyant.backdrop.Backdrop
import com.kyant.backdrop.BackdropEffectScope
import com.kyant.backdrop.backdrops.LayerBackdrop
import com.kyant.backdrop.backdrops.emptyBackdrop
import com.kyant.backdrop.backdrops.layerBackdrop
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.colorControls
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.highlight.Highlight
import com.kyant.backdrop.shadow.Shadow
import com.kyant.shapes.Capsule
import com.kyant.shapes.RoundedRectangle
import com.kyant.shapes.RoundedRectangularShape
import app.captae.glass.theme.isDark
import kotlin.math.abs
import kotlin.math.atan2
import kotlin.math.cos
import kotlin.math.sin
import kotlin.math.tanh

/**
 玻璃从哪儿取背景。每一页的内容层挂 layerBackdrop，玻璃件是它的兄弟节点、从这里取样（[GlassBox] 自动提供）。
 null 表示没有背景源（预览、单独测部件），玻璃退成磨砂面。
 */
val LocalBackdrop = staticCompositionLocalOf<Backdrop?> { null }

/** 样张页用：强制按某一档画（在 36 上看 31–32、26–30 的退化效果）；正式页面别设 */
val LocalGlassTierOverride = staticCompositionLocalOf<GlassTier?> { null }

/** iOS 玻璃的形状：胶囊、圆（正方形上的胶囊）、连续圆角矩形。只能是这几种，折射只认圆角矩形族 */
object GlassShapes {
  val capsule: RoundedRectangularShape = Capsule()
  val circle: RoundedRectangularShape = Capsule()
  fun rect(cornerRadius: androidx.compose.ui.unit.Dp): RoundedRectangularShape = RoundedRectangle(cornerRadius)
}

/**
 对应 SwiftUI 的 `Glass`：`.regular`、`.clear`、`.tint(色)`、`.interactive()`。
 - tint：着色玻璃（iOS 发送钮能发时 `.regular.tint(Palette.ink)`、`.glassProminent` 的红色结束钮），实测几乎不透明。
 - interactive：按下回弹放大、按下处有一团高光（iOS `.interactive()`）。
 */
@Immutable
data class GlassEffect(
  val kind: Kind = Kind.Regular,
  val tint: Color = Color.Unspecified,
  val interactive: Boolean = false,
  /** 边缘折射：关掉时只有模糊和调色（下载条：边缘不把下面封面的颜色折进来，条里不要有光） */
  val refraction: Boolean = true,
) {
  enum class Kind { Regular, Clear, Panel }

  fun tint(color: Color) = copy(tint = color)
  fun interactive(on: Boolean = true) = copy(interactive = on)
  fun refraction(on: Boolean) = copy(refraction = on)

  companion object {
    val Regular = GlassEffect()
    val Clear = GlassEffect(Kind.Clear)
    /** 菜单、确认框、提醒框、sheet 这些弹层用的厚玻璃 */
    val Panel = GlassEffect(Kind.Panel)
  }
}

/** 按下状态：外面的 clickable / 手势可以读它（比如按下时图标变灰） */
@Stable
class GlassInteraction internal constructor(internal val highlight: InteractiveHighlight) {
  val pressProgress: Float get() = highlight.pressProgress
}

/**
 iOS `.glassEffect(_:in:)`：把这块变成玻璃。背景从 [LocalBackdrop] 取（必须是兄弟节点的内容层，绝不能是自己的祖先）。
 点击自己加在后面（`.clickable(...)`）；interactive 只管视觉回弹，不吃点击。
 */
@Composable
fun Modifier.glassEffect(
  effect: GlassEffect = GlassEffect.Regular,
  shape: RoundedRectangularShape = GlassShapes.capsule,
  enabled: Boolean = true,
  backdrop: Backdrop? = LocalBackdrop.current,
  exportedBackdrop: LayerBackdrop? = null,
): Modifier {
  val dark = isDark
  val look = when (effect.kind) {
    GlassEffect.Kind.Regular -> GlassStyle.regular(dark)
    GlassEffect.Kind.Clear -> GlassStyle.clear(dark)
    GlassEffect.Kind.Panel -> GlassStyle.panel(dark)
  }
  val tier = LocalGlassTierOverride.current ?: GlassTier.current
  val interaction = if (effect.interactive) rememberGlassInteraction(enabled) else null
  return this.glassSurface(look, tier, shape, effect.tint, backdrop, interaction, exportedBackdrop, lens = effect.refraction)
}

@Composable
internal fun rememberGlassInteraction(enabled: Boolean): GlassInteraction {
  val scope = rememberCoroutineScope()
  val enabledState by rememberUpdatedState(enabled)
  return remember(scope) { GlassInteraction(InteractiveHighlight(scope, enabled = { enabledState })) }
}

/** 按下时放大：每边大约长 6dp（iOS 实测 48 的圆钮按下到 56–60，标签栏每边宽 7pt），拖动时跟手偏一点 */
internal fun GraphicsLayerScope.pressTransform(interaction: GlassInteraction) {
  val h = interaction.highlight
  val progress = h.pressProgress
  if (progress == 0f && h.offset == androidx.compose.ui.geometry.Offset.Zero) return
  val width = size.width
  val height = size.height
  val scale = lerp(1f, 1f + 12.dp.toPx() / maxOf(width, height), progress)
  val maxOffset = size.minDimension
  val offset = h.offset
  translationX = maxOffset * tanh(0.05f * offset.x / maxOffset)
  translationY = maxOffset * tanh(0.05f * offset.y / maxOffset)
  val maxDragScale = 4.dp.toPx() / height
  val angle = atan2(offset.y, offset.x)
  scaleX = scale + maxDragScale * abs(cos(angle) * offset.x / size.maxDimension) * (width / height).fastCoerceAtMost(1f)
  scaleY = scale + maxDragScale * abs(sin(angle) * offset.y / size.maxDimension) * (height / width).fastCoerceAtMost(1f)
}

/** 底层：按一套参数画玻璃。部件内部用；页面上用 [glassEffect] */
internal fun Modifier.glassSurface(
  look: GlassLook,
  tier: GlassTier,
  shape: RoundedRectangularShape,
  tint: Color,
  backdrop: Backdrop?,
  interaction: GlassInteraction?,
  exportedBackdrop: LayerBackdrop? = null,
  overlay: (() -> Color)? = null,
  lens: Boolean = true,
): Modifier {
  val flat = tier == GlassTier.Flat || backdrop == null
  val surface = when {
    tint != Color.Unspecified -> null
    flat -> look.surfaceFlat
    tier == GlassTier.BlurOnly -> look.surfaceBlurOnly
    else -> look.surface
  }
  // 26–27 的硬件画布不支持 BlurMaskFilter，投影会变成一圈硬边，干脆不画
  val shadow = if (Build.VERSION.SDK_INT >= 28) Shadow(radius = look.shadowRadius, offset = DpOffset(0.dp, look.shadowOffset), color = look.shadowColor) else null
  val highlight = Highlight(width = look.highlightWidth, alpha = look.highlightAlpha)
  val rim = look.rim
  val rimWidth = look.rimWidth
  val drawn = this.drawBackdrop(
    backdrop = if (flat) emptyBackdrop() else backdrop,
    shape = { shape },
    effects = { if (!flat) applyLook(look, tier, lensScale = if (lens) 1f else 0f) },
    highlight = { highlight },
    shadow = { shadow },
    layerBlock = interaction?.let { i -> { pressTransform(i) } },
    exportedBackdrop = exportedBackdrop,
    onDrawSurface = {
      if (tint != Color.Unspecified) {
        // 着色玻璃：iOS 实测几乎是实色（墨色钮 #1C1A18、红色结束钮 #AD160E），只透一点点底下的光
        drawRect(tint.copy(alpha = 0.94f))
      } else if (surface != null) {
        drawRect(surface)
      }
      overlay?.invoke()?.let { if (it.alpha > 0f) drawRect(it) }
    },
    onDrawFront = { drawRim(shape, rim, rimWidth.toPx()) },
  )
  return if (interaction != null) drawn.then(interaction.highlight.modifier).then(interaction.highlight.gestureModifier) else drawn
}

/** 一套参数对应的效果链：调色 → 模糊 → 折射（顺序不能换）。lensScale 给按下时变透镜的部件用 */
internal fun BackdropEffectScope.applyLook(look: GlassLook, tier: GlassTier, lensScale: Float = 1f, blurScale: Float = 1f) {
  if (tier == GlassTier.Flat) return
  colorControls(brightness = look.brightness, saturation = look.saturation)
  blur(look.blur.toPx() * blurScale)
  if (tier == GlassTier.Full && lensScale > 0f) lens(look.lensHeight.toPx() * lensScale, look.lensAmount.toPx() * lensScale)
}

internal fun DrawScope.drawRim(shape: RoundedRectangularShape, color: Color, width: Float) {
  if (width <= 0f || color.alpha == 0f) return
  val outline = shape.createOutline(size, layoutDirection, this)
  // 描边居中压在边上，外半圈被裁掉，看得见的正好是 width
  drawOutline(outline, color, style = Stroke(width * 2f), blendMode = BlendMode.SrcOver)
}

/**
 一块「内容层 + 玻璃层」：content 是被取样的内容（挂 layerBackdrop），glass 里的玻璃件从它取样。
 两层是兄弟节点，所以玻璃永远不会在自己的背景源里（那样 RenderThread 会崩）。
 background 画在内容层最底下（也进背景源），玻璃边缘折射到内容外面时取到的是底色而不是透明。
 */
@Composable
fun GlassBox(
  modifier: Modifier = Modifier,
  background: Color = Color.Transparent,
  content: @Composable BoxScope.() -> Unit,
  glass: @Composable BoxScope.() -> Unit,
) {
  val backdrop = rememberLayerBackdrop()
  Box(modifier) {
    // 内容层里不许取这个背景源（那是自己的祖先），也不许取更外面的（同样是祖先）：内容里的玻璃一律退成磨砂面
    Box(Modifier.layerBackdrop(backdrop).background(background)) {
      CompositionLocalProvider(LocalBackdrop provides null) { content() }
    }
    CompositionLocalProvider(LocalBackdrop provides backdrop) { glass() }
  }
}
