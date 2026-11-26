package app.captae.glass.overlay

import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.res.stringResource
import app.captae.glass.R
import app.captae.glass.controls.GlassIconButton
import app.captae.glass.icons.Sym
import android.os.Build
import android.view.RoundedCorner
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.Orientation
import androidx.compose.foundation.gestures.draggable
import androidx.compose.foundation.gestures.rememberDraggableState
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.Stable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.lerp
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.Velocity
import androidx.compose.ui.unit.dp
import androidx.compose.ui.util.lerp
import com.kyant.backdrop.backdrops.layerBackdrop
import com.kyant.backdrop.backdrops.rememberCombinedBackdrop
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.kyant.shapes.RoundedRectangularShape
import com.kyant.shapes.UnevenRoundedRectangle
import app.captae.glass.core.GlassEffect
import app.captae.glass.core.GlassStyle
import app.captae.glass.core.GlassTier
import app.captae.glass.core.LocalBackdrop
import app.captae.glass.core.LocalGlassTierOverride
import app.captae.glass.core.glassEffect
import app.captae.glass.core.glassSurface
import app.captae.glass.nav.ScrollEdge
import app.captae.glass.nav.ScrollEdgeEffect
import app.captae.glass.core.GlassShapes
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.isDark
import app.captae.glass.theme.weight
import kotlinx.coroutines.launch
import kotlin.math.abs

/** iOS `.presentationDetents`：半高 / 全高 */
enum class SheetDetent { Medium, Large }

/** sheet 里面能用的：当前档位、关掉 */
@Stable
class SheetScope internal constructor(private val close: () -> Unit) {
  var detent by mutableStateOf(SheetDetent.Medium)
    internal set

  /** 往全高拉的进度（0 半高，1 全高）；内容想跟着变（比如卡片半透明→不透明）时读 */
  var expansion by mutableStateOf(0f)
    internal set

  /** 不透明底（iOS `.presentationBackground(色)`）：半高时也不是玻璃，里面的白卡一直不透明 */
  var opaque by mutableStateOf(false)
    internal set

  /** 白卡现在的不透明度：玻璃 sheet 半高时半透明（iOS 26 半高 sheet 整块都是玻璃），拉到全高变回不透明 */
  val cardAlpha: Float get() = if (opaque) 1f else 0.55f + 0.45f * expansion

  fun dismiss() = close()

  /**
   拖着它换档 / 下拉关 sheet（iOS 拖 sheet 的标题栏）。sheet 自己的标题栏已经有；内容自己画标题栏时（sheet 里的返回栈，
   GlassScaffold 会自动挂在按钮行上）给那一条加上。
   */
  var dragHandle: Modifier by mutableStateOf(Modifier)
    internal set
}

val LocalSheetScope = staticCompositionLocalOf<SheetScope?> { null }

/** 抓手颜色（iOS 27 实测：纸色底上 浅色黑 24%，深色白 40%） */
private const val GRABBER_LIGHT = 0.24f
private const val GRABBER_DARK = 0.4f

/** 尺寸（iOS 27 实测）：半高时四周离屏 8、顶部圆角 38；全高顶到状态栏下 10、圆角 36；抓手 57×4、离顶 5.7（压在标题栏上面） */
object SheetMetrics {
  val floatingInset = 8.dp
  val mediumCorner = 38.dp
  val largeCorner = 36.dp
  /** 半高档的顶边在屏幕高度的比例（iOS：874 高的屏上顶在 414） */
  const val mediumTopFraction = 0.474f
  val largeTopGap = 10.dp
  val grabberWidth = 57.dp
  val grabberHeight = 4.dp
  val grabberTop = 5.7.dp
  val headerTop = 15.dp
  val headerHeight = 44.dp
  val headerSide = 15.dp
  /** 窗口宽过这个就按宽屏摆：限宽居中 */
  val regularMinWidth = 600.dp
  val regularMaxWidth = 580.dp
  val regularSide = 24.dp
}

/**
 iOS 的 sheet（`.sheet` + `.presentationDetents([.medium, .large])`）：同窗口覆盖层。
 - 半高：玻璃浮起，四周留 8 的边，背景压暗；全高：贴边、变成不透明的纸色。
 - 拖抓手 / 标题栏换档，往下拖过半高或快速下甩就关；里面的列表滚到顶后继续往下拉也会带着 sheet 走（嵌套滚动）。
 - 系统返回键关掉。
 detents 只有 Large 时没有抓手（iOS 账户页那种）；能换档时顶部正中一条抓手，压在标题栏上面。
 header 是标题栏（[SheetHeader]），画在内容上面一层；内容自动在它下面开始。
 不传 header 时内容自己画标题栏：sheet 里带返回栈的（账户），内容是一页页 GlassScaffold，按钮行自动摆在 sheet 标题栏的位置。
 background：不透明底（iOS `.presentationBackground(Palette.paper)`，收藏面板），半高时也不透明、白卡不透明；默认是玻璃。
 fitContent：高度跟着内容走的一档（iOS `.presentationDetents([.height(…)])`，Captae 的输入卡）：没有抓手、不能拉高，
 键盘弹起时整块浮在键盘上面 8。
 contentBackground：iOS 内容自己铺了 `.background(Palette.paper)`（不是 presentationBackground，来源面板）：整块 sheet 半高也是这个颜色，
 但白卡照玻璃 sheet 半高时半透明（iOS 27 实测深色来源面板：纸色底、行底 #2C2C2E 透一半）。
 */
@Composable
fun GlassSheet(
  isPresented: Boolean,
  onDismiss: () -> Unit,
  detents: List<SheetDetent> = listOf(SheetDetent.Medium, SheetDetent.Large),
  initialDetent: SheetDetent = detents.first(),
  header: (@Composable () -> Unit)? = null,
  background: Color? = null,
  contentBackground: Color? = null,
  fitContent: Boolean = false,
  content: @Composable SheetScope.() -> Unit,
) {
  val dismiss by rememberUpdatedState(onDismiss)
  OverlayLayer(isPresented) { presence ->
    val scope = remember { SheetScope { dismiss() } }
    scope.opaque = background != null
    val dark = isDark
    val density = LocalDensity.current
    val view = LocalView.current
    val tier = LocalGlassTierOverride.current ?: GlassTier.current
    val root = LocalRootBackdrop.current
    val statusTop = WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
    // 屏幕圆角：半高浮起时底部圆角和屏幕圆角同心（安卓 12+ 能读到真实圆角，读不到就用顶部那个）
    val screenCorner: Dp = remember(view) {
      val px = if (Build.VERSION.SDK_INT >= 31) view.rootWindowInsets?.getRoundedCorner(RoundedCorner.POSITION_BOTTOM_LEFT)?.radius ?: 0 else 0
      with(density) { px.toDp() }
    }
    val bottomCornerMedium = if (screenCorner > SheetMetrics.mediumCorner) screenCorner - SheetMetrics.floatingInset else SheetMetrics.mediumCorner

    BackHandler(enabled = presence.visible) { dismiss() }
    val closeLabel = stringResource(R.string.glass_close)
    val resizeLabel = stringResource(R.string.glass_resize)

    BoxWithConstraints(Modifier.fillMaxSize()) {
      val h = constraints.maxHeight.toFloat()
      val mediumTop = h * SheetMetrics.mediumTopFraction
      val largeTop = with(density) { (statusTop + SheetMetrics.largeTopGap).toPx() }
      // 跟着内容走的那一档：内容量出来的高（标题栏也算上），量出来之前先不弹
      var fitPx by remember { mutableIntStateOf(0) }
      val fitGap = with(density) { SheetMetrics.floatingInset.toPx() }
      val fitTop = h - fitGap - fitPx
      fun topOf(d: SheetDetent) = when {
        fitContent -> fitTop
        d == SheetDetent.Large || SheetDetent.Medium !in detents -> largeTop
        else -> mediumTop
      }
      val top = remember { Animatable(h) }
      val animScope = rememberCoroutineScope()
      // 动效评审第 18 条：按按钮打开不回弹；拖完松手吸附才带一点回弹（手指施了力）
      val openSpec = spring<Float>(1f, 420f)
      val spec = spring<Float>(0.86f, 420f)
      // 第 20 条：甩下去关掉时带上松手的速度，不先减速再加速（按按钮关时是 0）
      var exitVelocity by remember { mutableFloatStateOf(0f) }
      LaunchedEffect(presence.visible, fitPx) {
        if (presence.visible) {
          if (fitContent && fitPx == 0) return@LaunchedEffect
          top.animateTo(topOf(initialDetent), openSpec)
        } else {
          top.animateTo(h, spring(1f, 500f), initialVelocity = exitVelocity)
          exitVelocity = 0f
          presence.exited()
        }
      }

      val expansion = if (fitContent) 0f else if (SheetDetent.Medium in detents && SheetDetent.Large in detents) ((mediumTop - top.value) / (mediumTop - largeTop)).coerceIn(0f, 1f) else if (SheetDetent.Medium in detents) 0f else 1f
      scope.expansion = expansion
      scope.detent = if (expansion > 0.5f) SheetDetent.Large else SheetDetent.Medium
      val appear = ((h - top.value) / (h - topOf(initialDetent)).coerceAtLeast(1f)).coerceIn(0f, 1f)

      fun settle(velocity: Float) {
        val current = top.value
        val candidates = detents.map { topOf(it) }.distinct()
        val target = when {
          velocity > 2500f || current > (candidates.maxOrNull() ?: mediumTop) + (h - (candidates.maxOrNull() ?: mediumTop)) * 0.3f -> null
          velocity < -1500f -> candidates.minOrNull()
          velocity > 1500f -> candidates.filter { it > current }.minOrNull()
          else -> candidates.minByOrNull { abs(it - current) }
        }
        if (target == null) { exitVelocity = velocity.coerceAtLeast(0f); dismiss() } else animScope.launch { top.animateTo(target, spec, velocity) }
      }

      val dragState = rememberDraggableState { delta ->
        // 第 19 条：拖过全高以后有阻尼（多拖的按 0.2 走），不硬停；松手 settle() 回到全高
        animScope.launch {
          val next = top.value + delta
          top.snapTo(if (next < largeTop) top.value + (if (top.value < largeTop) delta * 0.2f else (largeTop - top.value) + (next - largeTop) * 0.2f) else next)
        }
      }
      val settleLatest = rememberUpdatedState<(Float) -> Unit> { v -> settle(v) }
      scope.dragHandle = remember(dragState) { Modifier.draggable(dragState, Orientation.Vertical, onDragStopped = { v -> settleLatest.value(v) }) }
      val nested = remember(detents) {
        object : NestedScrollConnection {
          override fun onPreScroll(available: Offset, source: NestedScrollSource): Offset {
            // 往上推：先把 sheet 拉到全高，再让列表滚
            // 只管手指拖：输入框获得焦点时的「滚到看得见」也走嵌套滚动，不能让它把 sheet 推走（还会打断打开的动画）
            if (available.y < -0.5f && top.value > largeTop + 0.5f && source == NestedScrollSource.UserInput && !top.isRunning) {
              val next = (top.value + available.y).coerceAtLeast(largeTop)
              val used = next - top.value
              animScope.launch { top.snapTo(next) }
              return Offset(0f, used)
            }
            return Offset.Zero
          }

          override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset {
            // 列表到顶了还往下拉：带着 sheet 往下走
            // 打开 / 换档的动画进行中不接（输入框聚焦时会冒出一点点滚动，会打断动画）；不到半个像素的也不算
            if (available.y > 0.5f && source == NestedScrollSource.UserInput && !top.isRunning) {
              animScope.launch { top.snapTo(top.value + available.y) }
              return Offset(0f, available.y)
            }
            return Offset.Zero
          }

          override suspend fun onPreFling(available: Velocity): Velocity {
            val atDetent = detents.any { abs(topOf(it) - top.value) < 1f }
            if (!atDetent) {
              settle(available.y)
              return available
            }
            return Velocity.Zero
          }
        }
      }

      // 背景遮罩。Captae 改过：浅色用 Captae 的 --scrim（rgba(140,140,140,.5)，中灰洗白，把 #F7F7F7 压到约 #C1C1C1），
      // 不用 medcxy 的压黑（黑 20%，亮度差不多，但照片会被压脏、偏冷）；docs/design/README.md 第 2 节「不是压黑」。
      // 深色那一档照 medcxy 留着（iOS 实测：半高 ×0.51，全高 ×0.57）
      val dimMedium = if (dark) 0.48f else 1f
      val dimLarge = if (dark) 0.43f else 1f
      val scrim = if (dark) Color.Black else Palette.scrim
      Box(
        Modifier
          .fillMaxSize()
          .graphicsLayer { alpha = appear }
          .background(scrim.copy(alpha = scrim.alpha * lerp(dimMedium, dimLarge, expansion)))
          .clickable(interactionSource = null, indication = null) { dismiss() }
          .semantics { contentDescription = closeLabel; onClick { dismiss(); true } },
      )

      // 宽屏（展开的折叠屏、平板）：像 iPad 的表单 sheet，居中、限宽、四角圆、底下一直留边，不贴满整个屏
      val regular = maxWidth >= SheetMetrics.regularMinWidth
      val inset = if (regular) 0.dp else with(density) { lerp(SheetMetrics.floatingInset.toPx(), 0f, expansion).toDp() }
      val navBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
      val bottomGap = if (regular) navBottom + SheetMetrics.floatingInset else inset
      val topCorner = with(density) { lerp(SheetMetrics.mediumCorner.toPx(), SheetMetrics.largeCorner.toPx(), expansion).toDp() }
      val bottomCorner = if (regular) topCorner else with(density) { lerp(bottomCornerMedium.toPx(), 0f, expansion).toDp() }
      val shape: RoundedRectangularShape = UnevenRoundedRectangle(topCorner, topCorner, bottomCorner, bottomCorner)
      // 键盘弹起时整块往上让（iOS sheet 跟着键盘走）：底边贴着键盘，顶边最多推到全高的位置
      val ime = WindowInsets.ime.getBottom(density).toFloat()
      val shownTop = if (ime > 0f) (top.value - ime).coerceAtLeast(largeTop) else top.value
      val shownBottom = h - ime
      val sheetHeight = with(density) {
        if (fitContent) {
          // 跟着内容走的那一档：高度就是内容的高，不跟着拖动变（进出场只是整块上下移），键盘弹起时浮在键盘上面
          (if (fitPx > 0) fitPx.toFloat() else h * 0.3f).toDp()
        } else {
          (shownBottom - shownTop - (if (ime > 0f) 0f else bottomGap.toPx())).coerceAtLeast(0f).toDp()
        }
      }
      val surface = rememberLayerBackdrop()
      val inner = rememberLayerBackdrop()
      val panelLook = GlassStyle.panel(dark)
      val paper = Palette.paper
      // 全高时的底色 / 不透明底：标题栏下的滚动边缘也渐隐成这个颜色
      val fill: () -> Color = { background ?: contentBackground ?: paper.copy(alpha = expansion) }
      Box(
        Modifier
          .align(Alignment.TopCenter)
          .offset { IntOffset(0, (if (fitContent && ime > 0f) minOf(top.value, h - ime - fitGap - fitPx) else shownTop).toInt()) }
          .padding(horizontal = inset)
          .then(if (regular) Modifier.width(minOf(maxWidth - SheetMetrics.regularSide * 2, SheetMetrics.regularMaxWidth)) else Modifier.fillMaxWidth())
          .height(sheetHeight)
          // 全高时变成不透明的纸色（画在玻璃表面里，标题栏的玻璃按钮取样时也是纸色）
          .glassSurface(panelLook, tier, shape, Color.Unspecified, root, null, exportedBackdrop = surface, overlay = fill)
          .nestedScroll(nested)
          .clickable(interactionSource = null, indication = null) {},
      ) {
        // 顶部拖动区（标题栏那一条）。没有 header 时内容自己画标题栏（sheet 里的返回栈），拖动区垫在内容下面，不挡返回钮这些
        val dragStrip = @Composable {
          Box(
            Modifier
              .fillMaxWidth()
              .height(SheetMetrics.headerTop + SheetMetrics.headerHeight)
              .draggable(dragState, Orientation.Vertical, onDragStopped = { v -> settle(v) }),
          )
        }
        if (header == null && !fitContent) dragStrip()
        CompositionLocalProvider(LocalSheetScope provides scope, LocalBackdrop provides null) {
          // 内容裁成 sheet 的形状：里面的页面自己铺纸色底时，圆角外不露出方角
          Box(Modifier.fillMaxSize().clip(shape).layerBackdrop(inner)) {
            val headerPad = if (header != null) SheetMetrics.headerTop + SheetMetrics.headerHeight + 8.dp else if (detents.size > 1 && !fitContent) 16.dp else 0.dp
            if (fitContent) {
              // 量内容的高：不受 sheet 当前高度限制（unbounded，超出的被裁掉），量完 sheet 正好包住它
              Column(
                Modifier
                  .fillMaxWidth()
                  .wrapContentHeight(unbounded = true, align = Alignment.Top)
                  .onSizeChanged { fitPx = it.height }
                  .padding(top = headerPad),
              ) { scope.content() }
            } else {
              Column(Modifier.fillMaxSize().padding(top = headerPad)) { scope.content() }
            }
          }
        }
        if (header != null && !fitContent) dragStrip()
        // 标题栏是内容的兄弟节点（玻璃按钮从 sheet 表面 + 内容取样，不能在内容的背景源里）
        if (header != null) {
          CompositionLocalProvider(LocalBackdrop provides rememberCombinedBackdrop(surface, inner), LocalSheetScope provides scope) {
            // 内容滚到标题栏下面时变糊、淡掉（iOS sheet 顶部也有滚动边缘效果）
            ScrollEdgeEffect(ScrollEdge.Top, SheetMetrics.headerTop + SheetMetrics.headerHeight + 12.dp, fill())
            header()
          }
        }
        // 抓手最后画：压在标题栏和滚动边缘上面（iOS 26 有标题栏时也看得见）
        if (detents.size > 1 && !fitContent) {
          Box(
            Modifier
              .align(Alignment.TopCenter)
              .padding(top = SheetMetrics.grabberTop)
              .size(SheetMetrics.grabberWidth, SheetMetrics.grabberHeight)
              .background(if (dark) Color.White.copy(alpha = GRABBER_DARK) else Color.Black.copy(alpha = GRABBER_LIGHT), RoundedCornerShape(2.dp))
              .semantics { contentDescription = resizeLabel },
          )
        }
      }
    }
  }
}

/**
 sheet 的标题栏（iOS NavigationStack 里 inline 标题 + toolbar 的取消 / 完成）：左右是玻璃胶囊按钮（约 64×44，离边 15），
 标题 17pt 半粗居中，整条离 sheet 顶 15。没有的那边传 null。
 onClose：左上角 44 的玻璃圆钮（xmark），iOS 26 系统的关闭位置（Captae 第二版的 sheet 都用它，BRIEF 5.1）；和 onCancel 二选一。
 */
@Composable
fun SheetHeader(
  title: String,
  onCancel: (() -> Unit)? = null,
  onConfirm: (() -> Unit)? = null,
  cancelLabel: String = stringResource(R.string.glass_cancel),
  confirmLabel: String = stringResource(R.string.glass_done),
  confirmEnabled: Boolean = true,
  onClose: (() -> Unit)? = null,
) {
  Box(
    Modifier
      .fillMaxWidth()
      .padding(top = SheetMetrics.headerTop)
      .height(SheetMetrics.headerHeight)
      .padding(horizontal = SheetMetrics.headerSide),
  ) {
    Text(
      title, style = CaptaeType.headline, maxLines = 1, textAlign = TextAlign.Center,
      modifier = Modifier.align(Alignment.Center).padding(horizontal = 84.dp).semantics { heading() },
    )
    if (onClose != null) {
      GlassIconButton(
        Sym.xmark, onClick = onClose, modifier = Modifier.align(Alignment.CenterStart), size = 44.dp,
        iconStyle = CaptaeType.body.weight(FontWeight.Medium), contentDescription = stringResource(R.string.glass_close),
      )
    }
    if (onCancel != null) SheetTextButton(cancelLabel, onCancel, Modifier.align(Alignment.CenterStart), bold = false)
    if (onConfirm != null) SheetTextButton(confirmLabel, onConfirm, Modifier.align(Alignment.CenterEnd), bold = true, enabled = confirmEnabled)
  }
}

/** sheet 标题栏的玻璃胶囊按钮（取消 / 完成） */
@Composable
fun SheetTextButton(label: String, onClick: () -> Unit, modifier: Modifier = Modifier, bold: Boolean = false, enabled: Boolean = true) {
  Row(
    modifier
      .height(44.dp)
      .glassEffect(GlassEffect.Regular.interactive(), GlassShapes.capsule, enabled = enabled)
      .clickable(enabled = enabled, interactionSource = null, indication = null, role = Role.Button, onClick = onClick)
      .padding(horizontal = 16.dp),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    Text(label, style = CaptaeType.body.weight(if (bold) FontWeight.SemiBold else FontWeight.Normal), color = if (enabled) Palette.ink else Palette.ink3)
  }
}

