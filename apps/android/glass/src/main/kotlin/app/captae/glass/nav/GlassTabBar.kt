/*
 * Copyright 2025 Kyant (https://github.com/Kyant0/AndroidLiquidGlass)
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 *
 * Medcxy 改过：从 AndroidLiquidGlass catalog（kmp 分支 65ab177e）components/LiquidBottomTabs.kt、LiquidBottomTab.kt 复制改写。
 * 改动：尺寸照 iOS 27 系统标签栏实测（胶囊 362×63、选中底 98×54、按下放大成 118×74 的透镜）；颜色用 Medcxy 的玻璃参数；
 * 选中底在静止时是一块暗一点的胶囊（不是蓝色强调色，App 的 tint 是墨色）；加了角标、往下滚收起成小圆、读屏语义、
 * 低版本退化（没有折射时拖动只移动选中底）。
 *
 * Captae 改过：从 medcxy-v7 apps/android/glass 搬来（那边的改动说明见上一段），包名改成 app.captae.glass；图标和字体换成 Captae 的（Hugeicons、Geist），标签字用 Captae 的字号。
 */
package app.captae.glass.nav

import androidx.compose.ui.res.stringResource
import app.captae.glass.R
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.EaseOut
import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.background
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.compose.ui.util.fastCoerceIn
import androidx.compose.ui.util.fastRoundToInt
import androidx.compose.ui.util.lerp
import com.kyant.backdrop.Backdrop
import com.kyant.backdrop.backdrops.emptyBackdrop
import com.kyant.backdrop.backdrops.layerBackdrop
import com.kyant.backdrop.backdrops.rememberCombinedBackdrop
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.lens
import com.kyant.backdrop.highlight.Highlight
import com.kyant.backdrop.shadow.InnerShadow
import com.kyant.backdrop.shadow.Shadow
import com.kyant.shapes.Capsule
import app.captae.glass.core.DampedDragAnimation
import app.captae.glass.core.GlassStyle
import app.captae.glass.core.GlassTier
import app.captae.glass.core.InteractiveHighlight
import app.captae.glass.core.LocalBackdrop
import app.captae.glass.core.LocalGlassTierOverride
import app.captae.glass.core.applyLook
import app.captae.glass.core.drawRim
import app.captae.glass.icons.Icon
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.isDark
import kotlinx.coroutines.flow.collectLatest
import kotlinx.coroutines.flow.drop
import kotlinx.coroutines.launch
import kotlin.math.abs
import kotlin.math.sign

/** 一个标签：名字、SF 图标名（标签栏里画实心的 .fill 版）、角标数（0 不显示） */
@Immutable
data class GlassTab(val title: String, val sf: String, val badge: Int = 0) {
  /** iOS 26 标签栏自动把图标换成实心版 */
  val filledSf: String get() = if (app.captae.glass.icons.CaptaeIcons.has("$sf.fill")) "$sf.fill" else sf
}

/** 标签栏尺寸（iOS 27 实测，pt = dp） */
object TabBarMetrics {
  val height = 63.dp
  val sideMargin = 20.dp
  /** 胶囊底边离屏幕底边（iOS 在 home 条区域里，离底 20） */
  val bottomMargin = 20.dp
  val pillWidth = 98.dp
  val pillHeight = 54.dp
  /** 选中底离胶囊边 */
  val pillInset = 4.5.dp
  /** 第一个 / 最后一个标签中心离胶囊边 53.8，标签间距 84.7 → 两头留白 11.45 */
  val itemsPadding = 11.45.dp
  val itemWidth = 84.7.dp
  val iconSize = 27.dp
  val minimizedSize = 48.dp
  val minimizedInset = 8.dp

  /**
   只有图标的标签栏（Captae 第二版，BRIEF 2.2）：没有字、铺满宽（左右离屏 20），图标平分。
   尺寸照 iOS 第二版截图量的（2026-11-24 用户对照 iOS 定）：高约 61，选中底是浅灰胶囊约 124×53，
   差不多占满一个标签格（标签格 = (胶囊宽 - 两头各 4) / 标签数，选中底就是一格宽），选中的图标是实心版。
   */
  val iconOnlyHeight = 61.dp
  val iconOnlyPillHeight = 53.dp
  val iconOnlyItemsPadding = 4.dp
  val iconOnlyIconSize = 26.dp

  fun height(iconOnly: Boolean) = if (iconOnly) iconOnlyHeight else height
}

/**
 底部悬浮玻璃标签栏（iOS TabView，iOS 26 起 Liquid Glass）：选中项下面一块暗一点的胶囊，按住拖动时变成放大的液态透镜跟手滑、
 松手吸附到最近的标签；minimized 时收成左边一个 48 的小圆（iOS `.tabBarMinimizeBehavior(.onScrollDown)`，点它展开）。
 放在页面的玻璃层（GlassBox 的 glass 里），底下的内容是它的背景源。
 */
@Composable
fun GlassTabBar(
  tabs: List<GlassTab>,
  selectedIndex: Int,
  onSelect: (Int) -> Unit,
  modifier: Modifier = Modifier,
  minimized: Boolean = false,
  onExpand: () -> Unit = {},
  backdrop: Backdrop? = LocalBackdrop.current,
  // 只有图标（Captae 第二版）：铺满宽、没有字；读屏照样念标签名
  iconOnly: Boolean = false,
) {
  // 标签少时胶囊收窄居中（iOS：每个标签约 84.7 宽，两头各留 11.45；四个正好 362）；只有图标时铺满（ChatGPT）
  BoxWithConstraints(modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
    val ideal = if (iconOnly) maxWidth else TabBarMetrics.itemWidth * tabs.size + TabBarMetrics.itemsPadding * 2
    GlassTabBarBody(tabs, selectedIndex, onSelect, Modifier.width(if (ideal < maxWidth) ideal else maxWidth), minimized, onExpand, backdrop, iconOnly)
  }
}

@Composable
private fun GlassTabBarBody(
  tabs: List<GlassTab>,
  selectedIndex: Int,
  onSelect: (Int) -> Unit,
  modifier: Modifier,
  minimized: Boolean,
  onExpand: () -> Unit,
  backdrop: Backdrop?,
  iconOnly: Boolean,
) {
  val barHeightDp = TabBarMetrics.height(iconOnly)
  val itemsPaddingDp = if (iconOnly) TabBarMetrics.iconOnlyItemsPadding else TabBarMetrics.itemsPadding
  val pillHeightDp = if (iconOnly) TabBarMetrics.iconOnlyPillHeight else TabBarMetrics.pillHeight
  val dark = isDark
  val look = GlassStyle.regular(dark)
  val tier = LocalGlassTierOverride.current ?: GlassTier.current
  val pageBackdrop = if (tier == GlassTier.Flat || backdrop == null) emptyBackdrop() else backdrop
  val flat = tier == GlassTier.Flat || backdrop == null
  val barSurface = when {
    flat -> look.surfaceFlat
    tier == GlassTier.BlurOnly -> look.surfaceBlurOnly
    else -> look.surface
  }
  // 选中底：浅色比胶囊暗约 6%（实测 #E3E1DE 叠在 #F8F6F3 上），深色明显更暗（#181712 叠在 #312F2C 上）
  val pillColor = if (dark) Color.Black.copy(alpha = 0.45f) else Color.Black.copy(alpha = 0.075f)
  val ink = Palette.ink
  val rim = look.rim
  val tabsCount = tabs.size
  val tabsBackdrop = rememberLayerBackdrop()
  val onSelectState by rememberUpdatedState(onSelect)
  val selectedState by rememberUpdatedState(selectedIndex)

  // 动效评审第 23 条：滚动带出来的收起不回弹
  val collapse by animateFloatAsState(if (minimized) 1f else 0f, spring(1f, 380f), label = "minimize")

  BoxWithConstraints(modifier.fillMaxWidth().height(barHeightDp), contentAlignment = Alignment.CenterStart) {
    val density = LocalDensity.current
    val fullWidth = constraints.maxWidth.toFloat()
    val itemsPadding = with(density) { itemsPaddingDp.toPx() }
    val tabWidth = (fullWidth - itemsPadding * 2) / tabsCount
    // 只有图标时选中底就是一格宽；带字的照 iOS 27 实测 98
    val pillWidth = if (iconOnly) tabWidth else with(density) { TabBarMetrics.pillWidth.toPx() }
    val pillWidthDp = with(density) { pillWidth.toDp() }
    val pillInset = with(density) { (if (iconOnly) TabBarMetrics.iconOnlyItemsPadding else TabBarMetrics.pillInset).toPx() }

    val offsetAnimation = remember { Animatable(0f) }
    val tapSlop = with(density) { 6.dp.toPx() }
    val panelOffset by remember(density) {
      derivedStateOf {
        val fraction = (offsetAnimation.value / fullWidth).fastCoerceIn(-1f, 1f)
        with(density) { 4.dp.toPx() * fraction.sign * EaseOut.transform(abs(fraction)) }
      }
    }

    val animationScope = rememberCoroutineScope()
    var currentIndex by remember { mutableIntStateOf(selectedIndex) }
    // Captae 改过（2026-11-24 用户在 captae-phone 上拖不动）：拖动手势挂在透镜上，透镜又跟着手指移动（offset），
    // 指针坐标是相对透镜自己的，它一走就把手指的位移抵掉一截，越拖越落后，松手时停在中间那格。
    // 现在每次拖动都把透镜自己移动的距离加回来；拖到哪儿自己累计（同一帧来好几个移动事件时，还在动画里的 targetValue 是旧的，会丢增量）。
    val track = remember { DragTrack() }
    track.tabWidth = tabWidth
    track.maxValue = (tabsCount - 1).toFloat()
    val dampedDragAnimation = remember(animationScope, tabsCount) {
      DampedDragAnimation(
        animationScope = animationScope,
        initialValue = selectedIndex.toFloat(),
        valueRange = 0f..(tabsCount - 1).toFloat(),
        visibilityThreshold = 0.001f,
        initialScale = 1f,
        // iOS：拖动时选中底变成约 1.2 倍大的透镜（98×54 → 118×74）
        pressedScale = 1.2f,
        onDragStarted = {
          track.value = value
          track.total = 0f
          track.lastNodeX = track.nodeX()
        },
        onDragStopped = {
          // 动效评审第 22 条：按位置和速度一起判断（往右快甩但没过半格也换过去），v 是格 / 秒
          val targetIndex = (track.value + releaseVelocity() * 0.15f).fastRoundToInt().fastCoerceIn(0, tabsCount - 1)
          // 选中底盖在当前标签上，点它（没拖动）就是「再点一下当前标签」：交给外面（退回根页）
          if (targetIndex == currentIndex && abs(track.total) < tapSlop) onSelectState(targetIndex)
          currentIndex = targetIndex
          animateToValue(targetIndex.toFloat())
          animationScope.launch { offsetAnimation.animateTo(0f, spring(1f, 300f, 0.5f)) }
        },
        onDrag = { _, dragAmount ->
          // 手指在屏幕上真正走了多少 = 相对透镜的位移 + 透镜自己这段时间走的
          val nodeX = track.nodeX()
          val dx = dragAmount.x + (nodeX - track.lastNodeX)
          track.lastNodeX = nodeX
          track.total += dx
          track.value = (track.value + dx / track.tabWidth).fastCoerceIn(0f, track.maxValue)
          updateValue(track.value)
          val total = track.total
          animationScope.launch { offsetAnimation.snapTo(total) }
        },
      )
    }
    LaunchedEffect(selectedIndex) {
      if (currentIndex != selectedIndex) currentIndex = selectedIndex
    }
    LaunchedEffect(dampedDragAnimation) {
      snapshotFlow { currentIndex }
        .drop(1)
        .collectLatest { index ->
          dampedDragAnimation.animateToValue(index.toFloat())
          // 外面改了选中项（深链切标签）只跟着动，不当成用户点的，免得回调里又把那个标签退回根页
          if (index != selectedState) onSelectState(index)
        }
    }

    // 选中底的左边：以标签中心为中心，夹在胶囊里面
    fun pillX(value: Float): Float =
      (itemsPadding + (value + 0.5f) * tabWidth - pillWidth / 2).fastCoerceIn(pillInset, fullWidth - pillInset - pillWidth)

    // 透镜现在的左边（拖动时用来把它自己的移动加回去）
    track.nodeX = { pillX(dampedDragAnimation.value) + panelOffset }

    val interactiveHighlight = remember(animationScope) {
      InteractiveHighlight(
        animationScope = animationScope,
        position = { size, _ -> Offset(pillX(dampedDragAnimation.value) + pillWidth / 2 + panelOffset, size.height / 2) },
      )
    }

    val minSize = with(density) { TabBarMetrics.minimizedSize.toPx() }
    val minInset = with(density) { TabBarMetrics.minimizedInset.toPx() }
    val barHeight = with(density) { barHeightDp.toPx() }

    // 胶囊（收起时变成左边的小圆）
    val barModifier = Modifier
      .graphicsLayer {
        translationX = panelOffset + lerp(0f, minInset, collapse)
      }
      .width(with(density) { lerp(fullWidth, minSize, collapse).toDp() })
      .height(with(density) { lerp(barHeight, minSize, collapse).toDp() })
      .drawBackdrop(
        backdrop = pageBackdrop,
        shape = { Capsule() },
        effects = { if (!flat) applyLook(look, tier) },
        highlight = { Highlight(width = look.highlightWidth, alpha = look.highlightAlpha) },
        shadow = { Shadow(radius = look.shadowRadius, offset = androidx.compose.ui.unit.DpOffset(0.dp, look.shadowOffset), color = look.shadowColor) },
        layerBlock = {
          val progress = dampedDragAnimation.pressProgress
          // 按住时整条每边宽约 7pt（实测 362 → 376）
          val scale = lerp(1f, 1f + 14.dp.toPx() / size.width, progress)
          scaleX = scale
          scaleY = scale
        },
        onDrawSurface = { drawRect(barSurface) },
        onDrawFront = { drawRim(Capsule(), rim, look.rimWidth.toPx()) },
      )

    Box(barModifier.then(interactiveHighlight.modifier)) {
      if (collapse > 0f) {
        // 收起后的小圆：只剩选中标签的图标，点了展开
        Box(
          Modifier
            .size(TabBarMetrics.minimizedSize)
            .alpha(collapse)
            .clickable(interactionSource = null, indication = null, role = Role.Button, onClick = onExpand)
            .semantics { contentDescription = tabs.getOrNull(selectedIndex)?.title ?: "" },
          contentAlignment = Alignment.Center,
        ) {
          tabs.getOrNull(selectedIndex)?.let { Icon(it.filledSf, size = 28.dp, tint = ink) }
        }
      }
    }

    if (collapse < 1f) {
      // 静止时的选中底：画在图标下面（只压暗玻璃，不压暗图标和字）
      Box(
        Modifier
          .offset { IntOffset((pillX(dampedDragAnimation.value) + panelOffset).fastRoundToInt(), 0) }
          .size(pillWidthDp, pillHeightDp)
          .graphicsLayer { alpha = (1f - collapse) * (1f - dampedDragAnimation.pressProgress) }
          .background(pillColor, CircleShape)
      )

      // 标签内容（看得见的那一排）
      Row(
        Modifier
          .alpha(1f - collapse)
          .graphicsLayer { translationX = panelOffset }
          .fillMaxWidth()
          .height(barHeightDp)
          .padding(horizontal = itemsPaddingDp),
        verticalAlignment = Alignment.CenterVertically,
      ) {
        tabs.forEachIndexed { i, tab ->
          TabItem(tab, selected = i == currentIndex, ink = ink, iconOnly = iconOnly, scale = { 1f }, onClick = { if (i == currentIndex) onSelectState(i) else currentIndex = i }, modifier = Modifier.weight(1f))
        }
      }

      // 一份看不见的副本，录进 tabsBackdrop：透镜从这里取图标来放大（选中底静止时也是从这里画出选中标签）。
      // 它盖在上面一排之上，点击也落在它这儿，所以点击要一样
      Row(
        Modifier
          .clearAndSetSemantics {}
          .alpha(0f)
          .layerBackdrop(tabsBackdrop)
          .graphicsLayer { translationX = panelOffset }
          .drawBackdrop(
            backdrop = pageBackdrop,
            shape = { Capsule() },
            effects = {
              if (!flat) applyLook(look, tier, lensScale = dampedDragAnimation.pressProgress)
            },
            highlight = { Highlight.Default.copy(alpha = dampedDragAnimation.pressProgress * look.highlightAlpha) },
            onDrawSurface = { drawRect(barSurface) },
          )
          .fillMaxWidth()
          .height(barHeightDp)
          .padding(horizontal = itemsPaddingDp),
        verticalAlignment = Alignment.CenterVertically,
      ) {
        tabs.forEachIndexed { i, tab ->
          TabItem(tab, selected = i == currentIndex, ink = ink, iconOnly = iconOnly, scale = { lerp(1f, 1.2f, dampedDragAnimation.pressProgress) }, onClick = { if (i == currentIndex) onSelectState(i) else currentIndex = i }, modifier = Modifier.weight(1f))
        }
      }

      // 拖动时的透镜（静止时看不见；手势也挂在它身上）
      Box(
        Modifier
          .alpha(1f - collapse)
          .offset { IntOffset((pillX(dampedDragAnimation.value) + panelOffset).fastRoundToInt(), 0) }
          .then(interactiveHighlight.gestureModifier)
          .then(dampedDragAnimation.modifier)
          .drawBackdrop(
            backdrop = rememberCombinedBackdrop(pageBackdrop, tabsBackdrop),
            shape = { Capsule() },
            effects = {
              val progress = dampedDragAnimation.pressProgress
              if (tier == GlassTier.Full && progress > 0f) {
                lens(10.dp.toPx() * progress, 14.dp.toPx() * progress, chromaticAberration = true)
              }
            },
            highlight = { Highlight.Default.copy(alpha = dampedDragAnimation.pressProgress) },
            shadow = { Shadow(alpha = dampedDragAnimation.pressProgress) },
            innerShadow = {
              val progress = dampedDragAnimation.pressProgress
              InnerShadow(radius = 8.dp * progress, alpha = progress)
            },
            layerBlock = {
              // 静止时透镜整个隐身（选中底由上面那块画），按下才出现
              alpha = (dampedDragAnimation.pressProgress * 3f).coerceAtMost(1f)
              scaleX = dampedDragAnimation.scaleX
              scaleY = dampedDragAnimation.scaleY
              val velocity = dampedDragAnimation.velocity / 10f
              scaleX /= 1f - (velocity * 0.75f).fastCoerceIn(-0.2f, 0.2f)
              scaleY *= 1f - (velocity * 0.25f).fastCoerceIn(-0.2f, 0.2f)
            },
            onDrawSurface = { drawRect(Color.Black.copy(alpha = 0.03f * dampedDragAnimation.pressProgress)) },
          )
          .size(pillWidthDp, pillHeightDp)
      )

      // 角标画在最上面（透镜下面也要看得见）
      Row(
        Modifier.alpha(1f - collapse).graphicsLayer { translationX = panelOffset }.fillMaxWidth().height(barHeightDp).padding(horizontal = itemsPaddingDp).clearAndSetSemantics {},
        verticalAlignment = Alignment.CenterVertically,
      ) {
        tabs.forEach { tab -> Box(Modifier.weight(1f).fillMaxHeight()) { if (tab.badge > 0) TabBadge(tab.badge) } }
      }
    }
  }
}

@Composable
private fun TabItem(tab: GlassTab, selected: Boolean, ink: Color, iconOnly: Boolean, scale: () -> Float, onClick: () -> Unit, modifier: Modifier) {
  val badgeLabel = stringResource(R.string.glass_tab_badge)
  Column(
    modifier
      .fillMaxHeight()
      .clickable(interactionSource = null, indication = null, onClick = onClick)
      .semantics {
        role = Role.Tab
        this.selected = selected
        contentDescription = if (tab.badge > 0) badgeLabel.format(tab.title, tab.badge) else tab.title
      }
      .graphicsLayer {
        val s = scale()
        scaleX = s
        scaleY = s
      },
    verticalArrangement = Arrangement.spacedBy(1.dp, Alignment.CenterVertically),
    horizontalAlignment = Alignment.CenterHorizontally,
  ) {
    if (iconOnly) {
      // 只有图标：选中的是实心版，其余是描边版（ChatGPT）；描边比正文图标粗一档，和实心版的分量接近
      Icon(if (selected) tab.filledSf else tab.sf, size = TabBarMetrics.iconOnlyIconSize, weight = FontWeight.Medium, tint = ink)
      return@Column
    }
    Icon(tab.filledSf, size = TabBarMetrics.iconSize, tint = ink)
    // iOS 标签字 10pt 半粗
    Text(tab.title, style = CaptaeType.caption2.copy(fontSize = 10.sp, lineHeight = 12.sp, fontWeight = FontWeight.SemiBold), color = ink, maxLines = 1, textAlign = TextAlign.Center)
  }
}

/** 标签栏角标：系统红圆（实测直径约 18，圆心在图标中心右 21、和图标顶齐） */
@Composable
private fun TabBadge(count: Int) {
  val text = if (count > 99) "99+" else count.toString()
  Box(Modifier.fillMaxWidth().fillMaxHeight()) {
    Box(
      Modifier
        .align(Alignment.TopCenter)
        .offset(x = 21.dp, y = 6.dp)
        .height(18.dp)
        .width(if (text.length > 1) (10 + 7 * text.length).dp else 18.dp)
        .background(Palette.systemRed, CircleShape),
      contentAlignment = Alignment.Center,
    ) {
      Text(text, style = CaptaeType.caption2.copy(fontSize = 12.sp, lineHeight = 14.sp), color = Color.White, maxLines = 1)
    }
  }
}

/** 标签栏放在屏幕底部时离底边多远：手势导航（导航条矮）时照 iOS 离底 20；三键导航时放在按键上面 */
fun tabBarBottomPadding(navigationBarHeight: Dp): Dp =
  if (navigationBarHeight > 32.dp) navigationBarHeight + 8.dp else TabBarMetrics.bottomMargin

/** 拖动时的记账（见 GlassTabBarBody 里的说明）：拖到的位置、总位移、透镜上一次的左边 */
private class DragTrack {
  var value = 0f
  var total = 0f
  var lastNodeX = 0f
  var tabWidth = 1f
  var maxValue = 0f
  var nodeX: () -> Float = { 0f }
}
