package app.captae.glass.overlay

import kotlinx.coroutines.launch
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.animation.core.tween
import androidx.compose.animation.core.CubicBezierEasing
import androidx.compose.animation.core.LinearEasing
import android.view.HapticFeedbackConstants
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.layer.GraphicsLayer
import androidx.compose.ui.graphics.layer.drawLayer
import androidx.compose.ui.graphics.rememberGraphicsLayer
import androidx.compose.ui.input.pointer.PointerEventPass
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.platform.LocalViewConfiguration
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.captae.glass.theme.Palette
import app.captae.glass.theme.isDark
import kotlinx.coroutines.withTimeoutOrNull

/*
 * Captae 改过：从 medcxy-v7 apps/android/glass 的 overlay/ContextMenu.kt 搬来，包名改成 app.captae.glass.overlay；
 * 背景照 Captae 的 --scrim 洗灰（不压黑，同 GlassSheet）；浮起来那一块的圆角改成参数（Captae 的卡片是 14）。
 */

/**
 iOS `.contextMenu { … }`：长按一张卡，背景洗灰（不模糊），这张卡浮起来，旁边弹出玻璃菜单（ChatGPT 长按的样子）。
 普通点击照常交给这张卡自己的 clickable（长按触发后这次松手不算点击）。cornerRadius 是浮起来那一块的圆角，和卡片一样。
 */
@Composable
fun Modifier.contextMenu(sections: List<MenuSection>, enabled: Boolean = true, cornerRadius: Dp = 14.dp): Modifier {
  if (!enabled) return this
  var open by remember { mutableStateOf(false) }
  var touchX by remember { mutableStateOf(0f) }
  val anchor = rememberAnchorState()
  val snapshot = rememberGraphicsLayer()
  val view = LocalView.current
  val longPress = LocalViewConfiguration.current.longPressTimeoutMillis
  // 动效评审第 17 条：按住的那段时间卡片 linear 慢慢缩到 .97（看得出「再按一会儿就有东西」），提前松手 150ms ease-out 回去
  val hold = remember { Animatable(1f) }
  val holdScope = rememberCoroutineScope()
  ContextMenuOverlay(open, { open = false }, { anchor.bounds }, snapshot, { anchor.bounds.left + touchX }, sections, cornerRadius)
  return this
    .anchor(anchor)
    .graphicsLayer { scaleX = hold.value; scaleY = hold.value }
    // 每次画的时候顺手录一份，长按时直接拿去浮起来
    .drawWithContent {
      snapshot.record { this@drawWithContent.drawContent() }
      drawLayer(snapshot)
    }
    .pointerInput(Unit) {
      awaitEachGesture {
        val down = awaitFirstDown(requireUnconsumed = false, pass = PointerEventPass.Initial)
        val shrink = holdScope.launch { hold.animateTo(0.97f, tween(longPress.toInt(), easing = LinearEasing)) }
        val up = withTimeoutOrNull(longPress) {
          while (true) {
            val e = awaitPointerEvent(PointerEventPass.Initial)
            val c = e.changes.firstOrNull { it.id == down.id } ?: return@withTimeoutOrNull true
            if (!c.pressed) return@withTimeoutOrNull true
            // 手指挪远了就是在滚动，不算长按
            if ((c.position - down.position).getDistance() > viewConfiguration.touchSlop) return@withTimeoutOrNull true
          }
        }
        shrink.cancel()
        // 到时间了：浮起来的那一份从 .97 接着长（见 ContextMenuOverlay），原来这张直接回到 1（被盖住）；提前松手：150ms 回去
        if (up == null) holdScope.launch { hold.snapTo(1f) } else holdScope.launch { hold.animateTo(1f, tween(150, easing = CubicBezierEasing(0.23f, 1f, 0.32f, 1f))) }
        if (up == null) {
          touchX = down.position.x
          open = true
          view.performHapticFeedback(HapticFeedbackConstants.LONG_PRESS)
          // 吃掉这次手势剩下的事件，松手不会再触发点击
          while (true) {
            val e = awaitPointerEvent(PointerEventPass.Initial)
            e.changes.forEach { it.consume() }
            if (e.changes.none { it.pressed }) break
          }
        }
      }
    }
}

@Composable
private fun ContextMenuOverlay(open: Boolean, onDismiss: () -> Unit, anchor: () -> Rect, snapshot: GraphicsLayer, centerX: () -> Float, sections: List<MenuSection>, cornerRadius: Dp) {
  val dismiss by rememberUpdatedState(onDismiss)
  OverlayLayer(open) { presence ->
    val progress = remember { Animatable(0f) }
    LaunchedEffect(presence.visible) {
      // 浮起不回弹（≈.31s）
      if (presence.visible) progress.animateTo(1f, spring(1f, 420f))
      else {
        progress.animateTo(0f, spring(1f, 700f))
        presence.exited()
      }
    }
    BackHandler(enabled = presence.visible) { dismiss() }
    val dark = isDark
    val density = LocalDensity.current
    // 背景只洗灰、不模糊：浅色用 Captae 的 --scrim（中灰洗白，docs/design/README.md 第 2 节「不是压黑」）；深色照 medcxy 压暗
    Box(
      Modifier
        .fillMaxSize()
        .graphicsLayer { alpha = progress.value.coerceIn(0f, 1f) }
        .background(if (dark) Color.Black.copy(alpha = 0.47f) else Palette.scrim)
        .clickable(interactionSource = null, indication = null) { dismiss() },
    )
    // 浮起来的那一行
    val a = anchor()
    Box(
      Modifier
        .graphicsLayer {
          // 从按住时缩到的 .97 接着浮到 1.02
          val s = 0.97f + 0.05f * progress.value
          scaleX = s
          scaleY = s
          shadowElevation = 12.dp.toPx() * progress.value
          shape = RoundedCornerShape(cornerRadius)
          clip = true
          translationX = a.left
          translationY = a.top
        }
        .drawWithContent { drawLayer(snapshot) }
        .then(with(density) { Modifier.requiredSize(a.width.toDp(), a.height.toDp()) }),
    )
    // 菜单：放在这一行下面（放不下就上面），水平以按的位置为中心
    GlassMenu(expanded = presence.visible, onDismiss = { dismiss() }, anchor = anchor, sections = sections, centerX = centerX, outside = true)
  }
}
