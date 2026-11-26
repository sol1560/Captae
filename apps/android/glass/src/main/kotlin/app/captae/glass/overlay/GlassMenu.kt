package app.captae.glass.overlay

import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Column
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
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.safeDrawing
import app.captae.glass.core.GlassEffect
import app.captae.glass.core.GlassShapes
import app.captae.glass.core.glassEffect
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.isDark
import app.captae.glass.theme.weight

/** 菜单里的一项（iOS Menu 里的 Button / Picker 选项） */
@Immutable
class MenuAction(
  val title: String,
  val sf: String? = null,
  val subtitle: String? = null,
  val destructive: Boolean = false,
  val checked: Boolean = false,
  val enabled: Boolean = true,
  val onClick: () -> Unit,
)

/** 一组（iOS Menu 里的 Section / 分隔线隔开的一段） */
@Immutable
class MenuSection(val items: List<MenuAction>)

/**
 iOS `Menu { Picker(...) }`：一组带勾的选项。iOS 26 菜单里的 Picker 不显示说明文字，所以这里也不收。
 */
fun <T> menuPicker(options: List<T>, selected: T, title: (T) -> String, sf: (T) -> String? = { null }, enabled: (T) -> Boolean = { true }, onSelect: (T) -> Unit): MenuSection =
  MenuSection(options.map { o -> MenuAction(title(o), sf(o), checked = o == selected, enabled = enabled(o)) { onSelect(o) } })

/**
 往上弹时选项的顺序（iOS 27 实测）：
 - Automatic：按钮菜单（`Menu { … }`，输入框的「+」、模式钮）往上弹时倒过来，第一项离按钮最近；
 - Fixed：表单里的菜单式选择器（Form 里的 `Picker`，外观、记忆类别）不管往哪边弹都按原顺序。
 */
enum class MenuOrder { Automatic, Fixed }

/** 尺寸（iOS 27 实测）：宽 250、行距 42、上下内边距 11、圆角 32；有勾的菜单 勾 +27 / 图标 +55 / 字 +79，没勾的 图标 +40 / 字 +66 */
object MenuMetrics {
  val width = 250.dp
  val rowHeight = 42.dp
  val verticalPadding = 11.dp
  val corner = 32.dp
}

/**
 iOS `Menu { … } label: { … }`：点一下从按钮里长出一块玻璃菜单（锚定按钮：按钮在左半边就左对齐往右长，在右半边就右对齐往左长；
 按钮在屏幕上半截往下长、下半截往上长，往上长时顺序倒过来，离按钮最近的是第一项）。modifier 照常给按钮自己的样子（比如玻璃圆钮）。
 */
@Composable
fun MenuButton(
  sections: List<MenuSection>,
  modifier: Modifier = Modifier,
  enabled: Boolean = true,
  contentDescription: String? = null,
  order: MenuOrder = MenuOrder.Automatic,
  label: @Composable BoxScope.() -> Unit,
) {
  var open by remember { mutableStateOf(false) }
  val anchor = rememberAnchorState()
  Box(
    modifier
      .anchor(anchor)
      .clickable(enabled = enabled, interactionSource = null, indication = null, role = Role.Button) { open = true }
      .then(if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier),
    contentAlignment = Alignment.Center,
    content = label,
  )
  GlassMenu(open, onDismiss = { open = false }, anchor = { anchor.bounds }, sections = sections, order = order)
}

/**
 菜单本体（锚在某块区域上）。一般用 [MenuButton]；长按菜单、自己管开关时直接用。
 */
@Composable
fun GlassMenu(
  expanded: Boolean,
  onDismiss: () -> Unit,
  anchor: () -> Rect,
  sections: List<MenuSection>,
  centerX: (() -> Float)? = null,
  // 长按菜单：放在锚点（那一行）外面，下面放不下就放上面，不盖住它；往上放时也不倒序
  outside: Boolean = false,
  order: MenuOrder = MenuOrder.Automatic,
) {
  val dismiss by rememberUpdatedState(onDismiss)
  OverlayLayer(expanded) { presence ->
    val progress = remember { Animatable(0f) }
    LaunchedEffect(presence.visible) {
      // 动效评审第 2 条：不回弹（≈.25s），从 .95 长出来（不是 .35：菜单里的字不缩成一点再冒出来）
      if (presence.visible) progress.animateTo(1f, spring(1f, 632f))
      else {
        progress.animateTo(0f, spring(1f, 900f))
        presence.exited()
      }
    }
    BackHandler(enabled = presence.visible) { dismiss() }
    val density = LocalDensity.current
    val safe = WindowInsets.safeDrawing.asPaddingValues()
    val margin = with(density) { 8.dp.toPx() }
    val top = with(density) { safe.calculateTopPadding().toPx() } + margin
    val bottomInset = with(density) { safe.calculateBottomPadding().toPx() } + margin
    // 点外面关（菜单不压暗背景）
    Box(Modifier.fillMaxSize().clickable(interactionSource = null, indication = null) { dismiss() })
    var origin by remember { mutableStateOf(TransformOrigin.Center) }
    var upward by remember { mutableStateOf(false) }
    val ordered = if (upward && !outside && order == MenuOrder.Automatic) sections.reversed().map { MenuSection(it.items.reversed()) } else sections
    Layout(
      content = {
        MenuPanel(ordered, onPick = { action -> dismiss(); action.onClick() }, modifier = Modifier.graphicsLayer {
          val p = progress.value
          alpha = p.coerceIn(0f, 1f)
          scaleX = 0.95f + 0.05f * p
          scaleY = 0.95f + 0.05f * p
          transformOrigin = origin
        })
      },
      modifier = Modifier.fillMaxSize(),
    ) { measurables, constraints ->
      val a = anchor()
      val maxH = (constraints.maxHeight - top - bottomInset).toInt().coerceAtLeast(0)
      val p = measurables.first().measure(Constraints(maxWidth = constraints.maxWidth, maxHeight = maxH))
      val w = constraints.maxWidth.toFloat()
      val h = constraints.maxHeight.toFloat()
      val cx = centerX?.invoke()
      val x = when {
        cx != null -> cx - p.width / 2f
        a.center.x < w / 2 -> a.left - 2.dp.toPx()
        else -> a.right + 1.dp.toPx() - p.width
      }.coerceIn(margin, (w - margin - p.width).coerceAtLeast(margin))
      val gap = 8.dp.toPx()
      // iOS：按钮在屏幕上半截就往下长，下半截往上长（放不下再换方向）
      val fitsDown = if (outside) a.bottom + gap + p.height <= h - bottomInset else a.top - 1.dp.toPx() + p.height <= h - bottomInset
      val fitsUp = if (outside) a.top - gap - p.height >= top else a.bottom + 1.dp.toPx() - p.height >= top
      val down = if (a.center.y < h / 2) fitsDown || !fitsUp else !fitsUp && fitsDown
      val y = when {
        outside && down -> a.bottom + gap
        outside -> (a.top - gap - p.height).coerceAtLeast(top)
        down -> (a.top - 1.dp.toPx()).coerceAtLeast(top)
        else -> (a.bottom + 1.dp.toPx() - p.height).coerceAtLeast(top)
      }
      // 从按钮那个角长出来
      val ox = ((a.center.x - x) / p.width).coerceIn(0f, 1f)
      val oy = if (down) 0f else 1f
      if (origin.pivotFractionX != ox || origin.pivotFractionY != oy) origin = TransformOrigin(ox, oy)
      if (upward != !down) upward = !down
      layout(constraints.maxWidth, constraints.maxHeight) { p.place(x.toInt(), y.toInt()) }
    }
  }
}

/** 菜单面板：厚玻璃、圆角 32；多段之间一条细线 */
@Composable
internal fun MenuPanel(sections: List<MenuSection>, onPick: (MenuAction) -> Unit, modifier: Modifier = Modifier) {
  val hasCheck = sections.any { s -> s.items.any { it.checked } }
  val hasIcon = sections.any { s -> s.items.any { it.sf != null } }
  val sep = if (isDark) Color.White.copy(alpha = 0.12f) else Color.Black.copy(alpha = 0.1f)
  Column(
    modifier
      .width(MenuMetrics.width)
      .glassEffect(GlassEffect.Panel, GlassShapes.rect(MenuMetrics.corner))
      .verticalScroll(rememberScrollState())
      .padding(vertical = MenuMetrics.verticalPadding),
  ) {
    sections.forEachIndexed { si, section ->
      if (si > 0) Box(Modifier.padding(horizontal = 18.dp, vertical = 5.dp).fillMaxWidth().height(0.5.dp).background(sep))
      for (item in section.items) MenuRow(item, hasCheck, hasIcon, onPick)
    }
  }
}

@Composable
private fun MenuRow(item: MenuAction, hasCheck: Boolean, hasIcon: Boolean, onPick: (MenuAction) -> Unit) {
  val interaction = remember { MutableInteractionSource() }
  val pressed by interaction.collectIsPressedAsState()
  val color = when {
    !item.enabled -> Palette.ink3
    item.destructive -> Palette.systemRed
    else -> Palette.ink
  }
  val pressFill = if (isDark) Color.White.copy(alpha = 0.1f) else Color.Black.copy(alpha = 0.07f)
  // 勾、图标、字各自的位置（中心 / 起点，离面板左边）
  val checkCenter = 27.dp
  val iconCenter = if (hasCheck) 55.dp else 40.dp
  // 没有勾这一列时，iOS 不把各行的字对齐：带图标的行字在 +66，不带图标的行字在 +27（倍速菜单里只有当前那行带 checkmark 图标）
  val textStart = when {
    hasCheck && hasIcon -> 79.dp
    hasCheck -> 55.dp
    item.sf != null -> 66.dp
    else -> 27.dp
  }
  Box(
    Modifier
      .fillMaxWidth()
      .heightIn(min = MenuMetrics.rowHeight)
      .padding(horizontal = 8.dp)
      .background(if (pressed) pressFill else Color.Transparent, RoundedCornerShape(14.dp))
      .clickable(enabled = item.enabled, interactionSource = interaction, indication = null, role = Role.Button) { onPick(item) }
      .semantics { if (hasCheck) { selected = item.checked; role = Role.RadioButton } },
  ) {
    if (item.checked) {
      Box(Modifier.align(Alignment.CenterStart).padding(start = checkCenter - 8.dp - 9.dp).width(18.dp), contentAlignment = Alignment.Center) {
        Icon(Sym.checkmark, size = 20.dp, weight = FontWeight.SemiBold, tint = color)
      }
    }
    if (item.sf != null) {
      Box(Modifier.align(Alignment.CenterStart).padding(start = iconCenter - 8.dp - 12.dp).width(24.dp), contentAlignment = Alignment.Center) {
        Icon(item.sf, size = 24.dp, tint = color)
      }
    }
    Column(Modifier.align(Alignment.CenterStart).padding(start = textStart - 8.dp, end = 16.dp, top = 9.dp, bottom = 9.dp)) {
      Text(item.title, style = CaptaeType.body, color = color, maxLines = 2)
      if (item.subtitle != null) Text(item.subtitle, style = CaptaeType.footnote, color = Palette.ink2, maxLines = 3)
    }
  }
}

