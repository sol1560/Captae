package app.captae.glass.overlay

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.Stable
import androidx.compose.runtime.State
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.boundsInRoot
import androidx.compose.ui.layout.onGloballyPositioned
import com.kyant.backdrop.backdrops.LayerBackdrop
import com.kyant.backdrop.backdrops.layerBackdrop
import com.kyant.backdrop.backdrops.rememberLayerBackdrop
import app.captae.glass.core.LocalBackdrop

/**
 覆盖层（sheet、菜单、确认框、提醒框、长按菜单）都画在同一个窗口里：系统 Dialog / Popup / ModalBottomSheet 是独立窗口，
 玻璃采不到它下面的内容。App 根部挂一次 [OverlayHost]，各页直接用 GlassSheet / MenuButton / confirmationDialog / GlassAlert。
 */
@Stable
class OverlayState internal constructor() {
  internal val entries = mutableStateListOf<Entry>()

  internal class Entry(val content: State<@Composable () -> Unit>)
}

val LocalOverlay = staticCompositionLocalOf<OverlayState?> { null }

/** 整个 App 内容的背景源：覆盖层的玻璃从这里取样（包括页面上的玻璃栏） */
internal val LocalRootBackdrop = staticCompositionLocalOf<LayerBackdrop?> { null }

/**
 App 根部挂一次。content 是整个 App（四个标签、各页）；覆盖层画在它上面，和它是兄弟节点。
 */
@Composable
fun OverlayHost(modifier: Modifier = Modifier, content: @Composable () -> Unit) {
  val state = remember { OverlayState() }
  val root = rememberLayerBackdrop()
  CompositionLocalProvider(LocalOverlay provides state, LocalRootBackdrop provides root) {
    Box(modifier.fillMaxSize()) {
      Box(Modifier.fillMaxSize().layerBackdrop(root)) {
        CompositionLocalProvider(LocalBackdrop provides null) { content() }
      }
      CompositionLocalProvider(LocalBackdrop provides root) {
        for (entry in state.entries) key(entry) { entry.content.value() }
      }
    }
  }
}

/** 一个覆盖层的出场状态：visible 跟着调用方；退场动画走完调 [exited]，覆盖层才真正摘掉 */
@Stable
class OverlayPresence internal constructor(private val onExited: () -> Unit) {
  var visible by mutableStateOf(false)
    internal set

  fun exited() {
    if (!visible) onExited()
  }
}

/**
 把一块内容挂到覆盖层上：visible 期间以及退场动画没走完之前一直挂着（content 自己按 presence.visible 做进出场，
 退场完调 presence.exited()）。
 */
@Composable
internal fun OverlayLayer(visible: Boolean, content: @Composable (OverlayPresence) -> Unit) {
  val host = LocalOverlay.current ?: error("没挂 OverlayHost：App 根部要包一层 OverlayHost { … }")
  var mounted by remember { mutableStateOf(false) }
  val presence = remember { OverlayPresence { mounted = false } }
  if (visible && !mounted) mounted = true
  // 直接在组合里写：覆盖层的内容随后才组合，第一次就要读到对的值（不然会当成「刚关」立刻退场）
  presence.visible = visible
  val latest = rememberUpdatedState<@Composable () -> Unit> { content(presence) }
  if (mounted) {
    DisposableEffect(host) {
      val entry = OverlayState.Entry(latest)
      host.entries.add(entry)
      onDispose { host.entries.remove(entry) }
    }
  }
}

/** 记下一个部件在窗口里的位置（菜单、确认框锚定用） */
@Stable
class AnchorState {
  internal var coordinates: LayoutCoordinates? = null
  val bounds: Rect get() = coordinates?.takeIf { it.isAttached }?.boundsInRoot() ?: Rect.Zero
}

@Composable
fun rememberAnchorState(): AnchorState = remember { AnchorState() }

fun Modifier.anchor(state: AnchorState): Modifier = onGloballyPositioned { state.coordinates = it }
