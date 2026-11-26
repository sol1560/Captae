package app.captae.glass.overlay

import androidx.compose.ui.res.stringResource
import app.captae.glass.R
import androidx.activity.compose.BackHandler
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.spring
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.ime
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.GenericShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
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
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.Layout
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Constraints
import androidx.compose.ui.unit.dp
import com.kyant.backdrop.drawBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.colorControls
import app.captae.glass.core.GlassEffect
import app.captae.glass.core.GlassShapes
import app.captae.glass.core.GlassStyle
import app.captae.glass.core.GlassTier
import app.captae.glass.core.LocalBackdrop
import app.captae.glass.core.LocalGlassTierOverride
import app.captae.glass.core.glassEffect
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.isDark
import app.captae.glass.theme.weight

/** 确认框 / 提醒框里的一个按钮（iOS Button(role:)） */
@Immutable
class DialogAction(
  val label: String,
  val role: Role = Role.Default,
  val enabled: Boolean = true,
  val onClick: () -> Unit = {},
) {
  enum class Role { Default, Destructive, Cancel }
}

/** 弹层里的灰色胶囊按钮（iOS 26 确认框 / 提醒框的按钮）：浅色墨色 7%、深色白 12% 的底，48 高 */
@Composable
internal fun DialogButton(action: DialogAction, onClick: () -> Unit, modifier: Modifier = Modifier) {
  val interaction = remember { MutableInteractionSource() }
  val pressed by interaction.collectIsPressedAsState()
  val dark = isDark
  val base = if (dark) Color.White.copy(alpha = 0.12f) else Color(0xFF1F1D1A).copy(alpha = 0.07f)
  val down = if (dark) Color.White.copy(alpha = 0.2f) else Color(0xFF1F1D1A).copy(alpha = 0.14f)
  val color = when {
    !action.enabled -> Palette.ink3
    action.role == DialogAction.Role.Destructive -> Palette.systemRed
    else -> Palette.ink
  }
  Box(
    modifier
      .height(48.dp)
      .background(if (pressed) down else base, RoundedCornerShape(24.dp))
      .clickable(enabled = action.enabled, interactionSource = interaction, indication = null, role = androidx.compose.ui.semantics.Role.Button, onClick = onClick)
      .padding(horizontal = 16.dp),
    contentAlignment = Alignment.Center,
  ) {
    Text(action.label, style = CaptaeType.body.weight(if (action.role == DialogAction.Role.Cancel) FontWeight.Normal else FontWeight.Medium), color = color, maxLines = 1)
  }
}

/**
 iOS `.confirmationDialog(title, isPresented:, titleVisibility: .visible) { … } message: { … }`：iOS 26 起是锚在按钮旁边的玻璃气泡，
 箭头指着按钮，没有「取消」（点外面就是取消），所以 role 是 Cancel 的按钮不画。
 挂在触发它的那个部件上：`Modifier.confirmationDialog(...)`（位置就从它取）。气泡出在按钮上 / 下 / 右 / 左，规则见 [placePopover]。
 centered = true：iOS 把 confirmationDialog 挂在整张表单 / 整页上时（退出登录、清空记忆、删除声音），气泡在那块区域正中、没有箭头，
 没有说明文字时标题居中。这时把它挂在整块区域（整个列表）上。
 尺寸（实测）：宽 240、圆角 34、字离边 28、按钮 48 高离边 16、箭头 20×15。
 */
@Composable
fun Modifier.confirmationDialog(
  isPresented: Boolean,
  onDismiss: () -> Unit,
  title: String,
  titleVisible: Boolean = true,
  message: String? = null,
  actions: List<DialogAction>,
  centered: Boolean = false,
): Modifier {
  val anchor = rememberAnchorState()
  ConfirmationPopover(isPresented, onDismiss, { anchor.bounds }, if (titleVisible) title else null, message, actions, centered)
  return this.anchor(anchor)
}

@Composable
fun ConfirmationPopover(isPresented: Boolean, onDismiss: () -> Unit, anchor: () -> Rect, title: String?, message: String?, actions: List<DialogAction>, centered: Boolean = false) {
  val dismiss by rememberUpdatedState(onDismiss)
  OverlayLayer(isPresented) { presence ->
    val progress = remember { Animatable(0f) }
    LaunchedEffect(presence.visible) {
      // 动效评审第 3 条：不回弹（≈.25s），从 .95 起
      if (presence.visible) progress.animateTo(1f, spring(1f, 632f))
      else {
        progress.animateTo(0f, spring(1f, 900f))
        presence.exited()
      }
    }
    BackHandler(enabled = presence.visible) { dismiss() }
    val cancelLabel = stringResource(R.string.glass_cancel)
    val density = LocalDensity.current
    val safe = WindowInsets.safeDrawing.asPaddingValues()
    val top = with(density) { safe.calculateTopPadding().toPx() + 8.dp.toPx() }
    val arrowH = with(density) { 15.dp.toPx() }
    val arrowW = with(density) { 20.dp.toPx() }
    val corner = with(density) { 34.dp.toPx() }
    var side by remember { mutableStateOf(PopoverSide.Above) }
    Box(Modifier.fillMaxSize().clickable(interactionSource = null, indication = null) { dismiss() }.semantics { contentDescription = cancelLabel })
    val dark = isDark
    val look = GlassStyle.panel(dark)
    val tier = LocalGlassTierOverride.current ?: GlassTier.current
    val backdrop = LocalBackdrop.current
    Layout(
      content = {
        Column(
          Modifier
            .graphicsLayer {
              val p = progress.value
              alpha = p.coerceIn(0f, 1f)
              scaleX = 0.95f + 0.05f * p
              scaleY = 0.95f + 0.05f * p
              // 从箭头那边长出来
              transformOrigin = if (centered) {
                TransformOrigin.Center
              } else {
                when (side) {
                  PopoverSide.Above -> TransformOrigin(0.5f, 1f)
                  PopoverSide.Below -> TransformOrigin(0.5f, 0f)
                  PopoverSide.Right -> TransformOrigin(0f, 0.5f)
                  PopoverSide.Left -> TransformOrigin(1f, 0.5f)
                }
              }
            }
            .width(240.dp)
            .glassEffect(GlassEffect.Panel, GlassShapes.rect(34.dp))
            .clickable(interactionSource = null, indication = null) {}
            .padding(top = 24.dp, bottom = 13.dp),
        ) {
          if (title != null) {
            // 居中样式又没有说明时，标题居中（iOS 退出登录）
            val center = centered && message == null
            Text(
              title, style = CaptaeType.headline, textAlign = if (center) TextAlign.Center else null,
              modifier = Modifier.then(if (center) Modifier.fillMaxWidth() else Modifier).padding(horizontal = 28.dp).semantics { heading() },
            )
          }
          if (message != null) Text(message, style = CaptaeType.subheadline, color = Palette.ink2, modifier = Modifier.padding(start = 28.dp, end = 28.dp, top = 6.dp))
          Spacer(Modifier.height(16.dp))
          Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            for (a in actions.filter { it.role != DialogAction.Role.Cancel }) {
              DialogButton(a, onClick = { dismiss(); a.onClick() }, modifier = Modifier.fillMaxWidth())
            }
          }
        }
        // 箭头：一块小三角玻璃（折射只认圆角矩形，三角只有模糊和表面色）；居中样式没有箭头
        if (!centered) {
          val vertical = side == PopoverSide.Above || side == PopoverSide.Below
          Box(
            Modifier
              .graphicsLayer { alpha = progress.value.coerceIn(0f, 1f) }
              .size(if (vertical) 20.dp else 15.dp, if (vertical) 15.dp else 20.dp)
              .drawBackdrop(
                backdrop = backdrop ?: com.kyant.backdrop.backdrops.emptyBackdrop(),
                shape = {
                  when (side) {
                    PopoverSide.Above -> DownArrow
                    PopoverSide.Below -> UpArrow
                    PopoverSide.Right -> LeftArrow
                    PopoverSide.Left -> RightArrow
                  }
                },
                effects = {
                  if (tier != GlassTier.Flat) {
                    colorControls(saturation = look.saturation)
                    blur(look.blur.toPx())
                  }
                },
                highlight = null,
                shadow = null,
                onDrawSurface = { drawRect(if (tier == GlassTier.Flat) look.surfaceFlat else look.surface) },
              ),
          )
        }
      },
      modifier = Modifier.fillMaxSize(),
    ) { measurables, constraints ->
      val a = anchor()
      val panel = measurables[0].measure(Constraints(maxWidth = constraints.maxWidth))
      val w = constraints.maxWidth.toFloat()
      val h = constraints.maxHeight.toFloat()
      val margin = 8.dp.toPx()
      val bottom = h - with(density) { safe.calculateBottomPadding().toPx() } - margin
      val cx = (a.left + a.right) / 2
      val cy = (a.top + a.bottom) / 2
      if (centered) {
        // 那块区域正中，不画箭头（放不下就贴着屏幕边）
        val px = (cx - panel.width / 2f).coerceIn(margin, (w - margin - panel.width).coerceAtLeast(margin))
        val py = (cy - panel.height / 2f).coerceIn(top, (h - margin - panel.height).coerceAtLeast(top))
        return@Layout layout(constraints.maxWidth, constraints.maxHeight) { panel.place(px.toInt(), py.toInt()) }
      }
      val placed = placePopover(a, panel.width.toFloat(), panel.height.toFloat(), w, top, bottom, margin, arrowH)
      if (side != placed.side) side = placed.side
      val arrow = measurables.getOrNull(1)?.measure(Constraints())
      layout(constraints.maxWidth, constraints.maxHeight) {
        panel.place(placed.x.toInt(), placed.y.toInt())
        if (arrow != null) {
          // 箭头和面板重叠 1px，免得接缝；箭头对着锚点中间，但不出面板的圆角
          when (placed.side) {
            PopoverSide.Above, PopoverSide.Below -> {
              val ax = (cx - arrowW / 2).coerceIn(placed.x + corner * 0.6f, placed.x + panel.width - corner * 0.6f - arrowW)
              val ay = if (placed.side == PopoverSide.Above) placed.y + panel.height - 1 else placed.y - arrowH + 1
              arrow.place(ax.toInt(), ay.toInt())
            }
            PopoverSide.Right, PopoverSide.Left -> {
              val ay = (cy - arrowW / 2).coerceIn(placed.y + corner * 0.6f, placed.y + panel.height - corner * 0.6f - arrowW)
              val ax = if (placed.side == PopoverSide.Right) placed.x - arrowH + 1 else placed.x + panel.width - 1
              arrow.place(ax.toInt(), ay.toInt())
            }
          }
        }
      }
    }
  }
}

/** 气泡在锚点的哪一边（箭头朝锚点） */
internal enum class PopoverSide { Above, Below, Right, Left }

internal class PopoverPlacement(val side: PopoverSide, val x: Float, val y: Float)

/**
 气泡放哪（iOS 27 实测的挑法）：先看上面、再看下面——要能正对着锚点放（不用左右挪）；都不行就看右边、左边能不能整块放下
 （资料库文件页「删除」那种贴着左边的短按钮，iOS 出在右边、箭头朝左）；还不行就退回上 / 下（左右挪进屏幕）。
 */
internal fun placePopover(a: Rect, pw: Float, ph: Float, w: Float, top: Float, bottom: Float, margin: Float, arrow: Float): PopoverPlacement {
  val cx = (a.left + a.right) / 2
  val cy = (a.top + a.bottom) / 2
  val centeredX = cx - pw / 2
  val xFits = centeredX >= margin && centeredX + pw <= w - margin
  val aboveY = a.top - arrow - ph
  val belowY = a.bottom + arrow
  val fitsAbove = aboveY >= top
  val fitsBelow = belowY + ph <= bottom
  if (fitsAbove && xFits) return PopoverPlacement(PopoverSide.Above, centeredX, aboveY)
  if (fitsBelow && xFits) return PopoverPlacement(PopoverSide.Below, centeredX, belowY)
  val sideY = (cy - ph / 2).coerceIn(top, (bottom - ph).coerceAtLeast(top))
  if (ph <= bottom - top) {
    if (a.right + arrow + pw <= w - margin) return PopoverPlacement(PopoverSide.Right, a.right + arrow, sideY)
    if (a.left - arrow - pw >= margin) return PopoverPlacement(PopoverSide.Left, a.left - arrow - pw, sideY)
  }
  val x = centeredX.coerceIn(margin, (w - margin - pw).coerceAtLeast(margin))
  return if (fitsAbove || !fitsBelow && a.top - top > bottom - a.bottom) PopoverPlacement(PopoverSide.Above, x, aboveY) else PopoverPlacement(PopoverSide.Below, x, belowY)
}

private val DownArrow = GenericShape { size, _ ->
  moveTo(0f, 0f)
  lineTo(size.width, 0f)
  // 尖端稍微圆一点
  lineTo(size.width * 0.58f, size.height * 0.9f)
  quadraticTo(size.width * 0.5f, size.height, size.width * 0.42f, size.height * 0.9f)
  close()
}

private val UpArrow = GenericShape { size, _ ->
  moveTo(0f, size.height)
  lineTo(size.width, size.height)
  lineTo(size.width * 0.58f, size.height * 0.1f)
  quadraticTo(size.width * 0.5f, 0f, size.width * 0.42f, size.height * 0.1f)
  close()
}

/** 气泡在锚点右边时，箭头朝左 */
private val LeftArrow = GenericShape { size, _ ->
  moveTo(size.width, 0f)
  lineTo(size.width, size.height)
  lineTo(size.width * 0.1f, size.height * 0.58f)
  quadraticTo(0f, size.height * 0.5f, size.width * 0.1f, size.height * 0.42f)
  close()
}

private val RightArrow = GenericShape { size, _ ->
  moveTo(0f, 0f)
  lineTo(0f, size.height)
  lineTo(size.width * 0.9f, size.height * 0.58f)
  quadraticTo(size.width, size.height * 0.5f, size.width * 0.9f, size.height * 0.42f)
  close()
}

/** 提醒框里的输入框（iOS `.alert { TextField(...) }`） */
@Immutable
class AlertTextField(val value: String, val onValueChange: (String) -> Unit, val placeholder: String)

/**
 iOS `.alert(title, isPresented:) { 按钮 } message: { … }`：居中的玻璃卡片，宽 320、圆角 34，标题和正文靠左；
 一个按钮占满一行，两个并排，三个以上竖排；可选一个输入框（灰色胶囊，48 高），出来时自动聚焦、整块挪到键盘上方。
 点外面不关（和 iOS 一样），系统返回键当「取消」。
 */
@Composable
fun GlassAlert(
  isPresented: Boolean,
  onDismiss: () -> Unit,
  title: String,
  message: String? = null,
  buttons: List<DialogAction> = listOf(DialogAction(stringResource(R.string.glass_ok))),
  textField: AlertTextField? = null,
) {
  val dismiss by rememberUpdatedState(onDismiss)
  OverlayLayer(isPresented) { presence ->
    val progress = remember { Animatable(0f) }
    LaunchedEffect(presence.visible) {
      // 动效评审第 4 条：按按钮弹出来的提醒框不回弹（≈.28s）；1.12 → 1 的缩放留着（居中弹窗）
      if (presence.visible) progress.animateTo(1f, spring(1f, 500f))
      else {
        progress.animateTo(0f, spring(1f, 900f))
        presence.exited()
      }
    }
    BackHandler(enabled = presence.visible) {
      buttons.firstOrNull { it.role == DialogAction.Role.Cancel }?.onClick?.invoke()
      dismiss()
    }
    val dark = isDark
    // 背景压暗（实测：浅 ×0.80、深 ×0.52）
    Box(
      Modifier
        .fillMaxSize()
        .graphicsLayer { alpha = progress.value.coerceIn(0f, 1f) }
        .background(Color.Black.copy(alpha = if (dark) 0.48f else 0.2f))
        .clickable(interactionSource = null, indication = null) {},
    )
    val focus = remember { FocusRequester() }
    Box(Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.ime).padding(horizontal = 16.dp), contentAlignment = Alignment.Center) {
      Column(
        Modifier
          .graphicsLayer {
            val p = progress.value
            alpha = p.coerceIn(0f, 1f)
            val s = 1.12f - 0.12f * p
            scaleX = s
            scaleY = s
          }
          .width(320.dp)
          .glassEffect(GlassEffect.Panel, GlassShapes.rect(34.dp))
          .padding(top = 24.dp, bottom = 16.dp),
      ) {
        Text(title, style = CaptaeType.headline, modifier = Modifier.padding(horizontal = 28.dp).semantics { heading() })
        if (message != null) Text(message, style = CaptaeType.subheadline, color = Palette.ink2, modifier = Modifier.padding(start = 28.dp, end = 28.dp, top = 6.dp))
        if (textField != null) {
          val fill = if (dark) Color.White.copy(alpha = 0.12f) else Color(0xFF1F1D1A).copy(alpha = 0.07f)
          val ink = Palette.ink
          val ink3 = Palette.ink3
          BasicTextField(
            value = textField.value,
            onValueChange = textField.onValueChange,
            singleLine = true,
            textStyle = CaptaeType.body.copy(color = ink),
            cursorBrush = SolidColor(ink),
            modifier = Modifier.padding(start = 16.dp, end = 16.dp, top = 16.dp).fillMaxWidth().focusRequester(focus),
            decorationBox = { inner ->
              Box(Modifier.fillMaxWidth().height(48.dp).background(fill, RoundedCornerShape(24.dp)).padding(horizontal = 18.dp), contentAlignment = Alignment.CenterStart) {
                if (textField.value.isEmpty()) Text(textField.placeholder, color = ink3, maxLines = 1)
                inner()
              }
            },
          )
          LaunchedEffect(Unit) { runCatching { focus.requestFocus() } }
        }
        Spacer(Modifier.height(18.dp))
        val visible = buttons
        if (visible.size <= 2) {
          Row(Modifier.padding(horizontal = 16.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            for (b in visible) DialogButton(b, onClick = { dismiss(); b.onClick() }, modifier = Modifier.weight(1f))
          }
        } else {
          Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            for (b in visible) DialogButton(b, onClick = { dismiss(); b.onClick() }, modifier = Modifier.fillMaxWidth())
          }
        }
      }
    }
  }
}
