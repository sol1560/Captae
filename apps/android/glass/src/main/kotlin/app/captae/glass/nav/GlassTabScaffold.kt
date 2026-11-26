package app.captae.glass.nav

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.size
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import app.captae.glass.icons.Icon
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.unit.lerp
import androidx.compose.ui.input.nestedscroll.NestedScrollConnection
import androidx.compose.ui.input.nestedscroll.NestedScrollSource
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.captae.glass.core.GlassBox
import app.captae.glass.core.GlassEffect
import app.captae.glass.core.GlassShapes
import app.captae.glass.core.glassEffect
import app.captae.glass.theme.Palette

/**
 底部被标签栏占掉的高度（从屏幕底边算起）。页面的 GlassScaffold 读它：内容底部留白、底部浮动件（输入框）放在标签栏上面。
 */
val LocalBottomBarInset = compositionLocalOf { 0.dp }

/** 页面滚动交给标签栏判断要不要收起（GlassScaffold 自动挂上） */
val LocalTabBarScrollConnection = staticCompositionLocalOf<NestedScrollConnection?> { null }

/**
 底部附件条（iOS `.tabViewBottomAccessory`）：展开时单独一条玻璃胶囊，和标签栏同宽、高 48、离标签栏 8（iOS 第二版截图量的）；
 大字号时两行字放不下会自己长高。收起时缩进标签栏那一行，48 高（和收起的标签栏小圆一样高）。
 */
object AccessoryMetrics {
  val minHeight = 48.dp
  val collapsedHeight = 48.dp
  val gapAboveBar = 8.dp
}

/**
 标签栏外壳（iOS TabView）：当前标签的内容铺满，底部悬浮玻璃标签栏从它取样。
 页面往下滚（内容往上走）超过一点就收成小圆，往回滚或点小圆就展开（`.tabBarMinimizeBehavior(.onScrollDown)`）。
 accessory：标签栏上方的一条玻璃胶囊（iOS 26 的 tabViewBottomAccessory，Captae 的下载条）。标签栏收起时它跟着缩进标签栏那一行，
 放在小圆右边（iOS 自带的做法）；collapsed 告诉里面换成只有一行的样子（iOS `tabViewBottomAccessoryPlacement == .inline`）。
 iconOnly：标签栏只有图标（Captae 第二版）。
 宽屏（Duo）不用它：竖放用 [GlassTabRail]，横放用 [GlassSidebar]，由外壳按窗口尺寸选。
 */
@Composable
fun GlassTabScaffold(
  tabs: List<GlassTab>,
  selectedIndex: Int,
  onSelect: (Int) -> Unit,
  modifier: Modifier = Modifier,
  minimizeOnScroll: Boolean = true,
  // false 时标签栏藏起来（iOS `.toolbar(.hidden, for: .tabBar)`：扫码页），页面底边不再让出它
  barVisible: Boolean = true,
  iconOnly: Boolean = false,
  accessory: (@Composable BoxScope.(collapsed: Boolean) -> Unit)? = null,
  // 换了这个值（比如当前标签的返回栈深度）也展开：推进来 / 退回去的页是新的滚动位置
  expandKey: Any? = null,
  // 起始就是收起的（设计稿截「滚过以后」那一张时用；正式页面由滚动决定）
  initiallyMinimized: Boolean = false,
  // 收起时单独放在右边的那个标签（iOS `Tab(role: .search)`：收起后搜索在最右单独一个圆）
  detachedTabIndex: Int? = null,
  content: @Composable BoxScope.() -> Unit,
) {
  var minimized by remember { mutableStateOf(initiallyMinimized) }
  // 收起只跟着当前这一页的滚动走：切标签（iOS 实测切过去是展开的）、换页都先展开；滚不动的页就一直展开
  var firstKey by remember { mutableStateOf(true) }
  LaunchedEffect(selectedIndex, expandKey) {
    if (firstKey) firstKey = false else minimized = false
  }
  // 动效评审第 23 条：滚动带出来的收起、隐藏都不回弹（不是手指拖着它）
  val hidden by animateFloatAsState(if (barVisible) 0f else 1f, spring(1f, 500f), label = "tabBarHidden")
  // 和标签栏收起用同一条弹簧，附件条跟着它一起走
  val collapse by animateFloatAsState(if (minimized) 1f else 0f, spring(1f, 380f), label = "accessoryCollapse")
  val density = LocalDensity.current
  val navBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
  val barBottom = tabBarBottomPadding(navBottom)
  val barHeight = TabBarMetrics.height(iconOnly)
  // 附件条展开时的实际高度（大字号时两行字会撑高）
  var accessoryHeightPx by remember { mutableIntStateOf(0) }
  val accessoryHeight = with(density) { maxOf(accessoryHeightPx.toDp(), AccessoryMetrics.minHeight) }
  // 有底部附件条时，页面底边要再让出它的高度（收起时不变，免得内容跳）
  val inset: Dp = if (!barVisible) 0.dp else barBottom + barHeight + if (accessory != null) accessoryHeight + AccessoryMetrics.gapAboveBar else 0.dp
  val threshold = with(density) { 24.dp.toPx() }
  val connection = remember(minimizeOnScroll) {
    object : NestedScrollConnection {
      private var travel = 0f
      override fun onPostScroll(consumed: Offset, available: Offset, source: NestedScrollSource): Offset {
        if (!minimizeOnScroll || source != NestedScrollSource.UserInput) return Offset.Zero
        // consumed.y < 0：内容往上走（手指往上推）= 往下看
        travel = if ((travel < 0) == (consumed.y < 0)) travel + consumed.y else consumed.y
        if (travel < -threshold && !minimized) minimized = true
        if (travel > threshold && minimized) minimized = false
        // 拉到顶以后继续下拉（available.y > 0）也展开
        if (available.y > 0 && minimized) minimized = false
        return Offset.Zero
      }
    }
  }
  GlassBox(
    modifier.fillMaxSize(),
    background = Palette.paper,
    content = {
      CompositionLocalProvider(LocalBottomBarInset provides inset, LocalTabBarScrollConnection provides connection) {
        Box(Modifier.fillMaxSize(), content = content)
      }
    },
  ) {
    // 收起时右边单独的那个圆（搜索）：小圆 48，离右边 20；它在时下载条右边让出 48 + 8
    val detached = detachedTabIndex?.takeIf { iconOnly && it != selectedIndex }
    val collapsedEnd = TabBarMetrics.sideMargin + if (detached != null) TabBarMetrics.minimizedSize + 8.dp else 0.dp
    if (accessory != null && hidden < 1f) {
      // 展开：标签栏上方 8、和标签栏同宽；收起：标签栏那一行里、左边小圆和右边搜索圆中间，48 高
      val collapsedStart = TabBarMetrics.sideMargin + TabBarMetrics.minimizedInset + TabBarMetrics.minimizedSize + 8.dp
      val start = lerp(TabBarMetrics.sideMargin, collapsedStart, collapse)
      val end = lerp(TabBarMetrics.sideMargin, collapsedEnd, collapse)
      val expandedBottom = barBottom + barHeight + AccessoryMetrics.gapAboveBar
      val collapsedBottom = barBottom + (barHeight - AccessoryMetrics.collapsedHeight) / 2
      val bottom = lerp(expandedBottom, collapsedBottom, collapse)
      val collapsedNow = collapse > 0.5f
      Box(
        Modifier
          .align(Alignment.BottomStart)
          .padding(start = start, end = end, bottom = bottom)
          .graphicsLayer { alpha = 1f - hidden; translationY = hidden * 160.dp.toPx() }
          .fillMaxWidth()
          .then(
            if (collapse > 0f) Modifier.height(lerp(accessoryHeight, AccessoryMetrics.collapsedHeight, collapse))
            else Modifier.heightIn(min = AccessoryMetrics.minHeight).onSizeChanged { accessoryHeightPx = it.height },
          )
          // 不加 interactive、不折射：条里不要按下的高光，边缘也不把下面封面的颜色折进来（用户定：下载条里不应该有光）；
          // 用厚一点的磨砂（Panel），下面列表的字不透上来和条里的字搅在一起
          .glassEffect(GlassEffect.Panel.refraction(false), GlassShapes.capsule),
        contentAlignment = Alignment.CenterStart,
      ) {
        // 第 24 条：两行 ↔ 一行在收起的过程中交叉淡化（前半段两行淡出、后半段一行淡入），不在中间那一帧一下换掉
        if (collapse <= 0f || collapse >= 1f) accessory(collapsedNow)
        else {
          Box(Modifier.matchParentSize().graphicsLayer { alpha = (1f - 2f * collapse).coerceAtLeast(0f) }, contentAlignment = Alignment.CenterStart) { accessory(false) }
          Box(Modifier.matchParentSize().graphicsLayer { alpha = (2f * collapse - 1f).coerceAtLeast(0f) }, contentAlignment = Alignment.CenterStart) { accessory(true) }
        }
      }
    }
    if (hidden < 1f) {
      Box(
        Modifier
          .align(Alignment.BottomCenter)
          .padding(bottom = barBottom)
          .graphicsLayer { alpha = 1f - hidden; translationY = hidden * (barHeight + barBottom).toPx() }
          .padding(horizontal = TabBarMetrics.sideMargin),
      ) {
        GlassTabBar(tabs, selectedIndex, onSelect, minimized = minimized, onExpand = { minimized = false }, iconOnly = iconOnly)
      }
    }
    if (detached != null && hidden < 1f && collapse > 0f) {
      val tab = tabs[detached]
      Box(
        Modifier
          .align(Alignment.BottomEnd)
          .padding(end = TabBarMetrics.sideMargin, bottom = barBottom + (barHeight - TabBarMetrics.minimizedSize) / 2)
          .graphicsLayer { alpha = collapse * (1f - hidden); scaleX = 0.9f + 0.1f * collapse; scaleY = 0.9f + 0.1f * collapse }
          .size(TabBarMetrics.minimizedSize)
          .glassEffect(GlassEffect.Regular.interactive(), GlassShapes.circle)
          .clickable(interactionSource = null, indication = null, role = Role.Tab) { onSelect(detached) }
          .semantics { contentDescription = tab.title },
        contentAlignment = Alignment.Center,
      ) {
        Icon(tab.sf, size = 26.dp, weight = FontWeight.Medium, tint = Palette.ink)
      }
    }
  }
}
