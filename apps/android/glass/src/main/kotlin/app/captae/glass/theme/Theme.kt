package app.captae.glass.theme

import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.foundation.IndicationNodeFactory
import androidx.compose.foundation.LocalIndication
import androidx.compose.foundation.interaction.InteractionSource
import androidx.compose.foundation.interaction.PressInteraction
import androidx.compose.foundation.text.BasicText
import androidx.compose.foundation.text.InlineTextContent
import androidx.compose.foundation.text.selection.LocalTextSelectionColors
import androidx.compose.foundation.text.selection.TextSelectionColors
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.drawscope.ContentDrawScope
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.node.DelegatableNode
import androidx.compose.ui.node.DrawModifierNode
import androidx.compose.ui.node.invalidateDraw
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.TextLayoutResult
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import kotlinx.coroutines.launch

/** 是不是深色。手机端第一版和桌面端一样只做浅色，一直是 false；玻璃参数里保留了深色那一套，以后两边一起加深色时用 */
val LocalDarkTheme = staticCompositionLocalOf { false }

/** 字和图标的默认颜色（iOS foregroundStyle 往下传的那个） */
val LocalContentColor = compositionLocalOf { Palette.ink }

/** 默认字体样式（iOS .font 往下传的那个） */
val LocalTextStyle = compositionLocalOf { CaptaeType.body }

/**
 整个 App 的主题：浅色、默认字体 Geist、按下变淡（不是水波纹）。
 不用 Material：所有部件都在 :glass 里自己画。
 */
@Composable
fun CaptaeTheme(content: @Composable () -> Unit) {
  CompositionLocalProvider(
    LocalDarkTheme provides false,
    LocalContentColor provides Palette.ink,
    LocalTextStyle provides CaptaeType.body,
    LocalIndication provides PressDim,
    LocalTextSelectionColors provides SelectionColors,
    content = content,
  )
}

private val SelectionColors = TextSelectionColors(handleColor = Palette.accent, backgroundColor = Palette.accent.copy(alpha = 0.25f))

/** 现在是不是深色（Composable 里读） */
val isDark: Boolean @Composable @ReadOnlyComposable get() = LocalDarkTheme.current

/** 文字：BasicText 加上主题默认样式和颜色（iOS Text） */
@Composable
fun Text(
  text: String,
  modifier: Modifier = Modifier,
  style: TextStyle = LocalTextStyle.current,
  color: Color = Color.Unspecified,
  textAlign: TextAlign? = null,
  maxLines: Int = Int.MAX_VALUE,
  minLines: Int = 1,
  overflow: TextOverflow = TextOverflow.Ellipsis,
  softWrap: Boolean = true,
  onTextLayout: ((TextLayoutResult) -> Unit)? = null,
) {
  BasicText(
    text = text,
    modifier = modifier,
    style = resolve(style, color, textAlign),
    maxLines = maxLines,
    minLines = minLines,
    overflow = overflow,
    softWrap = softWrap,
    onTextLayout = onTextLayout,
  )
}

/** 带 AnnotatedString 的版本（一句话里一段换颜色；inlineContent：跟在字里的小图标） */
@Composable
fun Text(
  text: AnnotatedString,
  modifier: Modifier = Modifier,
  style: TextStyle = LocalTextStyle.current,
  color: Color = Color.Unspecified,
  textAlign: TextAlign? = null,
  maxLines: Int = Int.MAX_VALUE,
  overflow: TextOverflow = TextOverflow.Ellipsis,
  onTextLayout: ((TextLayoutResult) -> Unit)? = null,
  inlineContent: Map<String, InlineTextContent> = emptyMap(),
) {
  BasicText(
    text = text,
    modifier = modifier,
    style = resolve(style, color, textAlign),
    maxLines = maxLines,
    overflow = overflow,
    onTextLayout = onTextLayout,
    inlineContent = inlineContent,
  )
}

@Composable
@ReadOnlyComposable
private fun resolve(style: TextStyle, color: Color, textAlign: TextAlign?): TextStyle {
  val c = when {
    color != Color.Unspecified -> color
    style.color != Color.Unspecified -> style.color
    else -> LocalContentColor.current
  }
  return style.merge(color = c, textAlign = textAlign ?: style.textAlign)
}

/** iOS `.font(x).foregroundStyle(y)` 往下传 */
@Composable
fun ProvideContentStyle(color: Color = LocalContentColor.current, style: TextStyle = LocalTextStyle.current, content: @Composable () -> Unit) {
  CompositionLocalProvider(LocalContentColor provides color, LocalTextStyle provides style, content = content)
}

/**
 按下变淡：无边框按钮按下时整块内容降到约 0.3 不透明度，松手 0.2 秒淡回（iOS 普通按钮的反馈；按下立刻变、松开才过渡）。
 */
object PressDim : IndicationNodeFactory {
  override fun create(interactionSource: InteractionSource): DelegatableNode = PressDimNode(interactionSource)
  override fun hashCode(): Int = -1
  override fun equals(other: Any?) = other === this
}

private class PressDimNode(private val source: InteractionSource) : Modifier.Node(), DrawModifierNode {
  private val alpha = Animatable(1f)
  private val paint = Paint()

  override fun onAttach() {
    coroutineScope.launch {
      var pressed = 0
      source.interactions.collect {
        when (it) {
          is PressInteraction.Press -> pressed++
          is PressInteraction.Release, is PressInteraction.Cancel -> pressed = (pressed - 1).coerceAtLeast(0)
        }
        launch {
          if (pressed > 0) alpha.snapTo(0.3f) else alpha.animateTo(1f, tween(200))
          invalidateDraw()
        }
      }
    }
  }

  override fun ContentDrawScope.draw() {
    val a = alpha.value
    if (a >= 1f) return drawContent()
    drawIntoCanvas { canvas ->
      paint.alpha = a
      canvas.saveLayer(Rect(0f, 0f, size.width, size.height), paint)
      drawContent()
      canvas.restore()
    }
  }
}
