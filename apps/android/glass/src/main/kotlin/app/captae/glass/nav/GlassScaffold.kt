package app.captae.glass.nav

import androidx.compose.ui.res.stringResource
import app.captae.glass.R
import androidx.compose.foundation.ScrollState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Stable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.captae.glass.controls.GlassNavButton
import app.captae.glass.core.GlassBox
import app.captae.glass.icons.Sym
import app.captae.glass.overlay.LocalSheetScope
import app.captae.glass.overlay.SheetMetrics
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.weight

/** 内容滚了多少（像素）。大标题收起、滚动边缘渐隐都看它 */
@Stable
fun interface ScrollPosition {
  /** 离顶部多远；Lazy 列表滚过第一项就当作很远 */
  fun offset(): Int
}

@Composable
fun rememberScrollPosition(state: LazyListState): ScrollPosition =
  remember(state) { ScrollPosition { if (state.firstVisibleItemIndex > 0) Int.MAX_VALUE / 2 else state.firstVisibleItemScrollOffset } }

@Composable
fun rememberScrollPosition(state: ScrollState): ScrollPosition = remember(state) { ScrollPosition { state.value } }

/** iOS `.navigationBarTitleDisplayMode`：大标题（滚上去收成导航栏里的小标题）/ 一直是居中小标题 */
enum class TitleMode { Large, Inline }

/**
 导航栏尺寸（iOS 27 实测）：按钮行 44 高，离状态栏 2；整条导航栏 54 高（内容从状态栏下 54 开始）；大标题 34 粗体，左边 16。
 在 sheet 里（sheet 自带的返回栈）：按钮行离 sheet 顶 15、左右 15（和 sheet 标题栏一样），内容从 sheet 顶 70 开始。
 */
object NavBarMetrics {
  val rowHeight = 44.dp
  val rowTopGap = 2.dp
  val barHeight = 54.dp
  val sidePadding = 16.dp
  val largeTitleHeight = 52.dp
  val sheetBarHeight = 70.dp
}

/**
 一页：内容层（暖灰底 + 内容，挂背景源）+ 玻璃层（导航栏按钮、底部浮动件、滚动边缘渐隐）。对应 iOS 一个 NavigationStack 页面：
 - title / titleMode：大标题时，内容第一项放 [LargeTitle]，滚上去后标题以 17pt 半粗淡入按钮行正中（iOS 27 实测居中）；
   Inline 时标题居中常显（研究页「研究」）。
 - 导航栏整条高 [NavBarMetrics.barHeight]（54，iOS 实测）：content 拿到的 top 就是导航栏底；
   推进来的页（Inline）第一组照 iOS 再空 [app.captae.glass.controls.GroupedMetrics.listTop]（35）。
 - navigation：左上（一般是 [BackButton]）；trailing：右上按钮（[app.captae.glass.controls.GlassButtonGroup]、账户钮）。
 - bottom：底部浮动的玻璃件（输入框、停止条、回放条），bottomHeight 是给内容留的底边距（不含系统导航条）。
 content 拿到的 PaddingValues 已经算上状态栏、按钮行、底部件和导航条（在 [GlassTabScaffold] 里时是标签栏），直接给 LazyColumn 的 contentPadding。
 */
@Composable
fun GlassScaffold(
  modifier: Modifier = Modifier,
  title: String = "",
  titleMode: TitleMode = TitleMode.Large,
  scroll: ScrollPosition? = null,
  navigation: (@Composable () -> Unit)? = null,
  trailing: @Composable RowScope.() -> Unit = {},
  bottomHeight: Dp = 0.dp,
  bottom: (@Composable BoxScope.() -> Unit)? = null,
  background: Color = Palette.paper,
  topEdgeEffect: Boolean = true,
  // 顶部滚动边缘的浓淡（0–1，绘制阶段读）：封面贴到顶的页（素材详情）静止时不要，滚起来才出来，免得封面的延伸被糊掉
  topEdgeAlpha: () -> Float = { 1f },
  // 有底部浮动件、或者在标签栏外壳里（标签栏 + 下载条浮在内容上）时，底边也做滚动边缘效果：玻璃下面的内容先糊掉、淡进底色
  bottomEdgeEffect: Boolean = bottom != null || LocalBottomBarInset.current > 0.dp,
  content: @Composable (PaddingValues) -> Unit,
) {
  val density = LocalDensity.current
  // 在 sheet 里推进来的页：没有状态栏，按钮行照 sheet 标题栏的位置
  val sheet = LocalSheetScope.current
  val inSheet = sheet != null
  val statusTop = if (inSheet) 0.dp else WindowInsets.statusBars.asPaddingValues().calculateTopPadding()
  val rowTop = statusTop + if (inSheet) SheetMetrics.headerTop else NavBarMetrics.rowTopGap
  val side = if (inSheet) SheetMetrics.headerSide else NavBarMetrics.sidePadding
  // 有标签栏时，底部从标签栏上沿算（底部浮动件放在标签栏上面，内容留出标签栏的高度）
  val navBottom = maxOf(WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding(), LocalBottomBarInset.current)
  val tabScroll = LocalTabBarScrollConnection.current
  val barBottom = statusTop + if (inSheet) NavBarMetrics.sheetBarHeight else NavBarMetrics.barHeight
  val padding = PaddingValues(top = barBottom, bottom = bottomHeight + navBottom)
  val largeTitlePx = with(density) { NavBarMetrics.largeTitleHeight.toPx() }
  GlassBox(
    modifier.fillMaxSize(),
    background = background,
    content = { Box(Modifier.fillMaxSize().then(if (tabScroll != null) Modifier.nestedScroll(tabScroll) else Modifier)) { content(padding) } },
  ) {
    if (topEdgeEffect) ScrollEdgeEffect(ScrollEdge.Top, height = barBottom + 8.dp, color = background, modifier = Modifier.align(Alignment.TopCenter).graphicsLayer { alpha = topEdgeAlpha() })
    if (bottomEdgeEffect) ScrollEdgeEffect(ScrollEdge.Bottom, height = bottomHeight + navBottom + 16.dp, color = background, modifier = Modifier.align(Alignment.BottomCenter))
    // 按钮行
    Box(
      Modifier
        .align(Alignment.TopStart)
        .fillMaxWidth()
        .padding(top = rowTop)
        .height(NavBarMetrics.rowHeight)
        // 在 sheet 里：拖这一条换档 / 关 sheet（和 sheet 自己的标题栏一样）
        .then(sheet?.dragHandle ?: Modifier)
        .padding(horizontal = side),
    ) {
      // 在绘制阶段读滚动位置，滚动时不重组
      val inlineAlpha: () -> Float = {
        when (titleMode) {
          TitleMode.Inline -> 1f
          TitleMode.Large -> {
            val off = scroll?.offset() ?: 0
            ((off - largeTitlePx * 0.6f) / (largeTitlePx * 0.4f)).coerceIn(0f, 1f)
          }
        }
      }
      // 小标题居中（iOS 27 实测：大标题收起后、inline 都是居中）；大标题模式下滚过大标题才淡入
      if (title.isNotEmpty()) {
        Text(
          title, style = CaptaeType.headline, maxLines = 1, textAlign = TextAlign.Center,
          modifier = Modifier
            .align(Alignment.Center)
            .padding(horizontal = 110.dp)
            .then(
              if (titleMode == TitleMode.Large) {
                Modifier.graphicsLayer {
                  val a = inlineAlpha()
                  alpha = a
                  translationY = (1f - a) * 6.dp.toPx()
                }
              } else {
                Modifier.semantics { heading() }
              },
            ),
        )
      }
      Row(Modifier.fillMaxSize(), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        navigation?.invoke()
        Spacer(Modifier.weight(1f))
        trailing()
      }
    }
    if (bottom != null) {
      Box(Modifier.align(Alignment.BottomCenter).fillMaxWidth().padding(bottom = navBottom), content = bottom)
    }
  }
}

/** 大标题（放在内容最上面，跟着内容一起滚；iOS 34 粗体，左边 16；标题中心离按钮行底 34） */
@Composable
fun LargeTitle(title: String, modifier: Modifier = Modifier) {
  Box(modifier.fillMaxWidth().height(NavBarMetrics.largeTitleHeight).padding(horizontal = NavBarMetrics.sidePadding), contentAlignment = Alignment.CenterStart) {
    Text(title, style = CaptaeType.largeTitle, maxLines = 1, modifier = Modifier.semantics { heading() })
  }
}

/** 导航栏左上的返回钮（iOS 系统画的 44 玻璃圆，chevron.left） */
@Composable
fun BackButton(onClick: () -> Unit) {
  GlassNavButton(Sym.chevronLeft, contentDescription = stringResource(R.string.glass_back), onClick = onClick, iconWeight = FontWeight.Medium)
}

/** 小标题（Inline 模式之外，自己摆的时候用） */
@Composable
fun NavTitle(title: String, modifier: Modifier = Modifier) {
  Text(title, style = CaptaeType.body.weight(FontWeight.SemiBold), maxLines = 1, modifier = modifier.semantics { heading() })
}
