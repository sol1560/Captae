package app.captae.glass.controls

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.captae.glass.core.GlassEffect
import app.captae.glass.core.GlassShapes
import app.captae.glass.core.glassEffect
import app.captae.glass.icons.Icon
import app.captae.glass.theme.LocalContentColor
import app.captae.glass.theme.LocalTextStyle
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.weight

/** iOS `.buttonBorderShape(...)` */
enum class GlassButtonShape { Circle, Capsule }

/**
 iOS `.buttonStyle(.glass)` / `.buttonStyle(.glassProminent)`：系统在标签外面每边加 7pt 再套玻璃
 （实测：标签 44 → 58 的圆，32 → 46，76 → 90）。照搬 iOS 时标签尺寸照抄（`frame(width: 44)` 就写 44），外圈这里加。
 - prominent + tint：`.glassProminent` 配 `.tint(Palette.red)`（录音页的红色结束钮）。
 */
@Composable
fun GlassButton(
  onClick: () -> Unit,
  modifier: Modifier = Modifier,
  prominent: Boolean = false,
  tint: Color = Palette.ink,
  shape: GlassButtonShape = GlassButtonShape.Circle,
  enabled: Boolean = true,
  contentDescription: String? = null,
  content: @Composable BoxScope.() -> Unit,
) {
  val glassShape = GlassShapes.capsule
  val effect = (if (prominent) GlassEffect.Regular.tint(tint) else GlassEffect.Regular).interactive()
  val pad = if (shape == GlassButtonShape.Circle) Modifier.padding(7.dp) else Modifier.padding(horizontal = 14.dp, vertical = 7.dp)
  Box(
    modifier
      .glassEffect(effect, glassShape, enabled = enabled)
      .clickable(enabled = enabled, interactionSource = null, indication = null, role = Role.Button, onClick = onClick)
      .then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier)
      .then(pad),
    contentAlignment = Alignment.Center,
  ) {
    CompositionLocalProvider(LocalContentColor provides if (prominent) Color.White else LocalContentColor.current, content = { content() })
  }
}

/**
 一个图标的玻璃圆钮（iOS `Image(systemName:).font(.title3).frame(48, 48).glassEffect(.regular.interactive(), in: .circle)`：
 首页「+」、模式钮、追问的模式钮）。size 是玻璃圆的直径（iOS frame 的值，不加外圈）。
 */
@Composable
fun GlassIconButton(
  sf: String,
  onClick: () -> Unit,
  modifier: Modifier = Modifier,
  size: Dp = 48.dp,
  effect: GlassEffect = GlassEffect.Regular.interactive(),
  iconStyle: TextStyle = CaptaeType.title3,
  iconTint: Color = Color.Unspecified,
  enabled: Boolean = true,
  contentDescription: String? = null,
  content: (@Composable BoxScope.() -> Unit)? = null,
) {
  Box(
    modifier
      .size(size)
      .glassEffect(effect, GlassShapes.circle, enabled = enabled)
      .clickable(enabled = enabled, interactionSource = null, indication = null, role = Role.Button, onClick = onClick)
      .then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier),
    contentAlignment = Alignment.Center,
  ) {
    if (content != null) content() else Icon(sf, style = iconStyle, tint = iconTint)
  }
}

/**
 iOS `Label(...).padding(.horizontal, 18).frame(height: 48).glassEffect(.regular.interactive(), in: .capsule)`：
 研究页「停止研究」这种文字胶囊。onClick 为空就是不可点的提示胶囊（「跑完后可以接着追问」）。
 */
@Composable
fun GlassCapsule(
  modifier: Modifier = Modifier,
  onClick: (() -> Unit)? = null,
  height: Dp = 48.dp,
  horizontalPadding: Dp = 18.dp,
  effect: GlassEffect = if (onClick != null) GlassEffect.Regular.interactive() else GlassEffect.Regular,
  enabled: Boolean = true,
  content: @Composable RowScope.() -> Unit,
) {
  Row(
    modifier
      .height(height)
      .glassEffect(effect, GlassShapes.capsule, enabled = enabled)
      .then(if (onClick != null) Modifier.clickable(enabled = enabled, interactionSource = null, indication = null, role = Role.Button, onClick = onClick) else Modifier)
      .padding(horizontal = horizontalPadding),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(6.dp),
    content = content,
  )
}

/** 导航栏按钮组里的一项 */
class NavBarItem(val sf: String, val contentDescription: String, val enabled: Boolean = true, val onClick: () -> Unit)

/**
 导航栏右上的按钮组：iOS 26 系统把相邻的 ToolbarItem 并成一块玻璃胶囊（收藏 + 分享）。
 实测：高 44，每项 44 宽、项间 12（两项的胶囊 100 宽），图标约 22pt 高。
 */
@Composable
fun GlassButtonGroup(items: List<NavBarItem>, modifier: Modifier = Modifier) {
  Row(
    modifier.height(44.dp).glassEffect(GlassEffect.Regular.interactive(), GlassShapes.capsule),
    horizontalArrangement = Arrangement.spacedBy(12.dp),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    for (item in items) {
      Box(
        Modifier
          .width(44.dp)
          .height(44.dp)
          .clickable(enabled = item.enabled, interactionSource = null, indication = null, role = Role.Button, onClick = item.onClick)
          .semantics { contentDescription = item.contentDescription },
        contentAlignment = Alignment.Center,
      ) {
        Icon(item.sf, size = NavIconSize, tint = if (item.enabled) LocalContentColor.current else Palette.ink3)
      }
    }
  }
}

/** 导航栏图标的外框（Hugeicons 24 格的边长）：iOS 工具栏图标比正文同字号大一圈（medcxy 实测约 22pt 高的字形）；Hugeicons 字形比 Lucide 略满，24 就够 */
val NavIconSize = 24.dp

/** 导航栏单个玻璃圆钮（返回 44 的圆；系统画的，不是 .buttonStyle(.glass)，所以没有外圈） */
@Composable
fun GlassNavButton(sf: String, contentDescription: String, onClick: () -> Unit, modifier: Modifier = Modifier, iconWeight: FontWeight? = FontWeight.Medium) {
  Box(
    modifier
      .size(44.dp)
      .glassEffect(GlassEffect.Regular.interactive(), GlassShapes.circle)
      .clickable(interactionSource = null, indication = null, role = Role.Button, onClick = onClick)
      .semantics { this.contentDescription = contentDescription },
    contentAlignment = Alignment.Center,
  ) {
    Icon(sf, size = NavIconSize, weight = iconWeight)
  }
}

/** 给 Icon 用当前字体（iOS 的 Image 跟着 .font 走），粗细取字重 */
@Composable
fun SymbolLabel(sf: String, text: String, style: TextStyle = LocalTextStyle.current, color: Color = Color.Unspecified) {
  Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
    Icon(sf, style = style, tint = color)
    app.captae.glass.theme.Text(text, style = style, color = color)
  }
}

/** iOS `.font(.body.weight(.semibold))` 的简写 */
val BodySemibold: TextStyle get() = CaptaeType.body.weight(FontWeight.SemiBold)
