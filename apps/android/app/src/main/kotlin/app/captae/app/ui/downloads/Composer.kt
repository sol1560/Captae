package app.captae.app.ui.downloads

import app.captae.app.ui.parts.swapBlur
import app.captae.app.ui.parts.springOf
import androidx.compose.ui.unit.IntSize
import androidx.compose.ui.unit.IntOffset
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.animation.togetherWith
import androidx.compose.animation.slideOut
import androidx.compose.animation.slideIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.fadeIn
import androidx.compose.animation.core.VisibilityThreshold
import androidx.compose.animation.animateColorAsState
import androidx.compose.animation.SizeTransform
import androidx.compose.animation.AnimatedContent
import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.platform.LocalClipboard
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.captae.app.R
import app.captae.app.demo.ComposerMode
import app.captae.app.demo.Sheet
import app.captae.app.model.Computer
import app.captae.app.ui.parts.SiteMark
import app.captae.app.ui.parts.Str
import app.captae.app.ui.parts.str
import app.captae.app.ui.parts.Ease
import app.captae.app.ui.parts.LiveProgressBar
import app.captae.app.ui.parts.LocalReducedMotion
import app.captae.app.ui.parts.rememberNow
import app.captae.app.ui.parts.rememberShimmerPhase
import app.captae.app.ui.parts.shimmer
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.LinearEasing
import androidx.compose.animation.core.animateFloat
import androidx.compose.animation.core.infiniteRepeatable
import androidx.compose.animation.core.rememberInfiniteTransition
import androidx.compose.animation.core.tween
import java.util.Locale
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin
import app.captae.glass.controls.Radius
import app.captae.glass.controls.cardShadow
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.overlay.GlassSheet
import app.captae.glass.overlay.MenuAction
import app.captae.glass.overlay.MenuButton
import app.captae.glass.overlay.MenuOrder
import app.captae.glass.overlay.MenuSection
import app.captae.glass.overlay.menuPicker
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospaced
import app.captae.glass.theme.monospacedDigit
import app.captae.glass.theme.weight
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.platform.LocalWindowInfo
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch

/** 保存格式（桌面端 pages/downloads/model.ts 的 FMT，同一套说法） */
enum class FmtChoice(@StringRes val label: Int) {
  Best(R.string.fmt_best),
  P1080(R.string.fmt_1080),
  P720(R.string.fmt_720),
  Mp3(R.string.fmt_mp3),
  Webm(R.string.fmt_webm),
  Mkv(R.string.fmt_mkv),
  Keep(R.string.fmt_keep),
  Aac(R.string.fmt_aac),
  Flac(R.string.fmt_flac);

  companion object {
    val main = listOf(Best, P1080, P720, Mp3)
    val more = listOf(Webm, Mkv, Keep, Aac, Flac)
  }
}

/** 下载去哪（PLAN 3.2）：发到电脑 / 在这台手机上下 */
enum class Destination { Computer, Phone }

/** 链接是哪个网站（认出来的才在输入框左边画网站图标；同桌面端 lib/detect.ts 的简化版） */
fun platOf(url: String): String? {
  val u = url.lowercase()
  return when {
    "youtube.com" in u || "youtu.be" in u -> "youtube"
    "bilibili.com" in u || "b23.tv" in u -> "bilibili"
    "x.com/" in u || "twitter.com" in u -> "x"
    "tiktok.com" in u || "douyin.com" in u -> "tiktok"
    "instagram.com" in u -> "instagram"
    else -> null
  }
}

/** 读取时流光用的网站颜色（BRIEF 2.6）：YouTube 红、哔哩哔哩粉、X 灰，认不出的用强调色蓝 */
private fun glowOf(plat: String?): Color = when (plat) {
  "youtube" -> Color(0xFFFF0033)
  "bilibili" -> Color(0xFFFB7299)
  "x" -> Color(0xFF8B8B8B)
  else -> Palette.accent
}

/**
 输入卡 sheet（BRIEF 2.4）：从下载条点开，同一窗口里的玻璃面板，高度跟着内容走，键盘弹起时浮在键盘上面。
 sheet 本身是玻璃，里面的输入卡是普通文字和按钮（玻璃不叠玻璃），没有标题栏：点外面或返回键关。
 */
@Composable
fun ComposerSheet(
  sheet: Sheet.Composer?,
  macs: List<Computer>,
  onDismiss: () -> Unit,
  onConnect: () -> Unit,
  onSubmit: (url: String, macId: String?, FmtChoice) -> Unit = { _, _, _ -> onDismiss() },
  onStop: () -> Unit = {},
) {
  // 关的时候 sheet 已经是 null，退场动画里还要画上一次的内容
  var last by remember { mutableStateOf(sheet) }
  if (sheet != null) last = sheet
  GlassSheet(isPresented = sheet != null, onDismiss = onDismiss, fitContent = true) {
    val s = last ?: return@GlassSheet
    ComposerCard(
      macs = macs,
      inSheet = true,
      initialUrl = s.url,
      mode = s.mode,
      startedAt = s.startedAt,
      error = s.error,
      autoFocus = s.mode == ComposerMode.Edit,
      onConnect = onConnect,
      onSubmit = onSubmit,
      onStop = onStop,
    )
  }
}

/**
 输入卡（Perplexity 式两行，BRIEF 2.4）。
 - 第一行：输入框「粘贴视频、音乐或图片的链接」，认出网站时左边出网站图标；
 - 第二行：左边「粘贴」，中间两个胶囊菜单「发到 Sol 的 MacBook Pro ⌄」（去向）和「MP4 · 最清晰 ⌄」（格式），右边圆形发送（近黑，没输入时灰）。
 inSheet = false 时在下载页空状态里直接内嵌：白卡（在内容中间，不是浮着的，不做玻璃）。
 mode：Reading 是点了发送、正在读取视频信息（版式照 iOS，见 [ReadingContent]；后面一层很淡的网站颜色的光，BRIEF 2.6；
 设计稿是静止帧，正式做动效时照全局规定先读动效课程）；
 Offline 是选了发到电脑、电脑连不上时，去向胶囊下面一行橙色小字（BRIEF 2.8 第 9 条；iOS 上架版这个位置是「改到电脑上下」的提示）。
 */
@Composable
fun ComposerCard(
  macs: List<Computer>,
  inSheet: Boolean,
  modifier: Modifier = Modifier,
  initialUrl: String = "",
  mode: ComposerMode = ComposerMode.Edit,
  autoFocus: Boolean = false,
  startedAt: Long = 0,
  error: String? = null,
  onConnect: () -> Unit = {},
  onSubmit: (url: String, macId: String?, FmtChoice) -> Unit = { _, _, _ -> },
  onStop: () -> Unit = {},
) {
  // 去向：哪台电脑（id），空 = 这台手机。默认最近用过的那台（macs 按最近用过排）
  val mac = macs.firstOrNull()
  var value by remember(initialUrl) { mutableStateOf(TextFieldValue(initialUrl, TextRange(initialUrl.length))) }
  var destId by remember(mac?.id) { mutableStateOf(mac?.id) }
  val destMac = macs.firstOrNull { it.id == destId }
  var fmt by remember { mutableStateOf(FmtChoice.Best) }
  val clipboard = LocalClipboard.current
  val context = LocalContext.current
  val scope = rememberCoroutineScope()
  val focus = remember { FocusRequester() }
  val keyboard = LocalSoftwareKeyboardController.current
  val reading = mode == ComposerMode.Reading
  val plat = platOf(value.text)
  val windowInfo = LocalWindowInfo.current
  LaunchedEffect(autoFocus) {
    if (autoFocus) {
      // 等窗口拿到焦点（冷启动直接打开输入卡时窗口还没拿到，这时请求的焦点不弹键盘），再等 sheet 弹出来（不然键盘先顶上来，sheet 的进场被打断）
      snapshotFlow { windowInfo.isWindowFocused }.first { it }
      delay(350)
      focus.requestFocus()
      keyboard?.show()
    }
  }
  val shape = RoundedCornerShape(Radius.xxl)
  val surface = if (inSheet) Modifier else Modifier.cardShadow(Radius.xxl).clip(shape).background(Palette.card)
  val glow = glowOf(plat)
  val reduced = LocalReducedMotion.current
  // 读取时的流光（BRIEF 2.6）：两团很淡的网站颜色从卡片左下和右上透出来，慢慢绕着走；进来时 300ms 淡入（--ease-out-quint）。
  // 动效评审第 13 条：两团各走各的（3.7 秒、2.9 秒一圈，故意不同步，不像机器转），颜色换的时候 400ms 过渡（和 iOS 一样）。
  // 只在读取的这几秒出现；减少动效时停在代表帧（左下、右上）
  val glowColor by animateColorAsState(glow, tween(400), label = "glowColor")
  val spin = if (reading && !reduced) rememberInfiniteTransition(label = "glow") else null
  val drift1 = spin?.animateFloat(0f, 1f, infiniteRepeatable(tween(3700, easing = LinearEasing)), label = "glowDrift1")
  val drift2 = spin?.animateFloat(0f, 1f, infiniteRepeatable(tween(2900, easing = LinearEasing)), label = "glowDrift2")
  val glowIn = remember { Animatable(0f) }
  LaunchedEffect(reading) { if (reading) glowIn.animateTo(1f, tween(300, easing = Ease.OutQuint)) else glowIn.snapTo(0f) }
  Column(
    modifier
      .fillMaxWidth()
      .then(surface)
      .then(if (reading) Modifier.drawBehind {
        val a = glowIn.value
        val t1 = (drift1?.value ?: 0f) * 2f * PI.toFloat()
        val t2 = (drift2?.value ?: 0f) * 2f * PI.toFloat()
        // 两团光沿着小椭圆反向绕，幅度是卡片宽的 6%、高的 25%
        val c1 = Offset(size.width * (0.18f + 0.06f * cos(t1)), size.height * (1.05f - 0.25f * sin(t1)))
        val c2 = Offset(size.width * (0.86f - 0.06f * cos(t2)), size.height * (0.1f + 0.25f * sin(t2)))
        drawRect(Brush.radialGradient(listOf(glowColor.copy(alpha = 0.13f * a), glowColor.copy(alpha = 0f)), center = c1, radius = size.width * 0.62f))
        drawRect(Brush.radialGradient(listOf(glowColor.copy(alpha = 0.07f * a), glowColor.copy(alpha = 0f)), center = c2, radius = size.width * 0.5f))
      } else Modifier)
      .padding(top = if (inSheet) 6.dp else 0.dp, bottom = if (inSheet) 6.dp else 0.dp),
  ) {
    // 动效评审第 12 条：编辑 ↔ 读取中是状态互换（同任务卡的状态块，MOTION 配方 c）：新的从上方 8dp + 4dp 模糊进来、旧的往下出，
    // 弹簧 0.3s，卡片高度跟着走；减少动效时照系统走（「移除动画」时直接到位）
    val shift = with(LocalDensity.current) { 8.dp.roundToPx() }
    AnimatedContent(
      targetState = reading,
      transitionSpec = {
        val fade = springOf<Float>(0.3f)
        val move = springOf(0.3f, IntOffset.VisibilityThreshold)
        ((fadeIn(fade) + slideIn(move) { IntOffset(0, -shift) }) togetherWith (fadeOut(fade) + slideOut(move) { IntOffset(0, shift) }))
          .using(SizeTransform(clip = false) { _, _ -> springOf(0.3f, IntSize.VisibilityThreshold) })
      },
      label = "composerReading",
    ) { r ->
    Column(Modifier.fillMaxWidth().then(if (reduced) Modifier else swapBlur())) {
    if (r) {
      ReadingContent(value.text, plat, if (destMac != null) str(R.string.composer_to_mac, destMac.name) else str(R.string.composer_to_phone), startedAt, onStop)
    } else {
      // 第一行：网站图标 + 输入框
      Row(
        Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, top = 14.dp, bottom = 6.dp),
        verticalAlignment = Alignment.Top,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
      ) {
        if (plat != null) {
          SiteMark(plat, size = 18.dp, modifier = Modifier.padding(top = 2.dp))
        }
        val placeholder = str(R.string.composer_placeholder)
        BasicTextField(
          value = value,
          onValueChange = { value = it },
          // 长链接最多两行（大字号时不把卡片撑得太高）
          maxLines = 2,
          textStyle = (if (value.text.isEmpty()) CaptaeType.body else CaptaeType.subheadline.monospaced()).copy(color = Palette.ink),
          cursorBrush = SolidColor(Palette.accent),
          keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri, imeAction = ImeAction.Go),
          modifier = Modifier.weight(1f).focusRequester(focus).semantics { contentDescription = placeholder },
          decorationBox = { inner ->
            Box(contentAlignment = Alignment.CenterStart) {
              if (value.text.isEmpty()) Text(placeholder, style = CaptaeType.body, color = Palette.ink3, maxLines = 1)
              inner()
            }
          },
        )
      }
      // 第二行：粘贴、去向、格式、发送
      Row(
        Modifier.fillMaxWidth().padding(start = 6.dp, end = 10.dp, top = 2.dp, bottom = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
      ) {
        // 粘贴：只在点它时读剪贴板（PLAN 1：不主动读）。只放图标：Pixel 9 宽度上两个胶囊菜单已经很挤，读屏照样念「粘贴」
        IconButton(Sym.docOnClipboard, str(R.string.composer_paste)) {
          scope.launch {
            val text = clipboard.getClipEntry()?.clipData?.takeIf { it.itemCount > 0 }?.getItemAt(0)?.coerceToText(context)?.toString()?.trim()
            if (!text.isNullOrEmpty()) value = TextFieldValue(text, TextRange(text.length))
          }
        }
        val toPhone = str(R.string.composer_to_phone)
        val offlineSub = str(R.string.composer_mac_offline_sub)
        val toMacLabels = macs.associate { it.id to str(R.string.composer_to_mac, it.name) }
        val destSections = listOf(
          MenuSection(
            macs.map { m -> MenuAction(toMacLabels.getValue(m.id), Sym.laptopcomputer, subtitle = if (m.online) null else offlineSub, checked = destId == m.id) { destId = m.id } } +
              MenuAction(toPhone, Sym.iphone, checked = destMac == null) { destId = null },
          ),
        ) + if (mac == null) listOf(MenuSection(listOf(MenuAction(str(R.string.composer_connect), Sym.plus, onClick = onConnect)))) else emptyList()
        val fmtLabel = str(fmt.label)
        // 去向胶囊让格式胶囊先量好，剩下的宽度都给它（太长时它截断，格式和发送钮不动）
        Row(Modifier.weight(1f).padding(start = 2.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
          MenuButton(destSections, modifier = Modifier.weight(1f, fill = false), contentDescription = str(R.string.composer_dest_a11y), order = MenuOrder.Fixed) {
            Chip(if (destMac != null) toMacLabels.getValue(destMac.id) else toPhone)
          }
          MenuButton(
            listOf(
              menuPicker(FmtChoice.main, fmt, title = { label(it.label) }) { fmt = it },
              menuPicker(FmtChoice.more, fmt, title = { label(it.label) }) { fmt = it },
            ),
            contentDescription = str(R.string.composer_fmt_a11y),
            order = MenuOrder.Fixed,
          ) {
            Chip(fmtLabel)
          }
        }
        SendButton(enabled = value.text.isNotBlank(), label = str(R.string.composer_send), onClick = { keyboard?.hide(); onSubmit(value.text.trim(), destMac?.id, fmt) })
      }
      if (error != null) {
        // 加任务失败（链接不完整这类）：链接留着，下面一行说原因（橙色小字，和电脑连不上的提示同一个位置）
        Row(Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp), horizontalArrangement = Arrangement.spacedBy(6.dp)) {
          Icon(Sym.exclamationmarkCircle, size = 14.dp, weight = FontWeight.Medium, tint = Palette.warn, modifier = Modifier.padding(top = 1.dp))
          Text(error, style = CaptaeType.footnote, color = Palette.warn)
        }
      }
      if (mode == ComposerMode.Offline && destMac != null && !destMac.online) {
        // 去向胶囊下面一行橙色小字（BRIEF 2.4 的位置）
        Row(
          Modifier.fillMaxWidth().padding(start = 16.dp, end = 16.dp, bottom = 12.dp),
          horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
          Icon(Sym.wifiSlash, size = 14.dp, weight = FontWeight.Medium, tint = Palette.warn, modifier = Modifier.padding(top = 1.dp))
          Text(str(R.string.composer_mac_offline_hint, destMac.name), style = CaptaeType.footnote, color = Palette.warn)
        }
      }
    }
    }
    }
  }
}

/** menuPicker 的标题不在 Composable 里取，用 Str */
private fun label(@StringRes id: Int): String = Str.get(id)

/**
 读取中（BRIEF 2.8 第 7 条，照 iOS 10-composer-reading）：左边三行，右边一个停止钮（取消这次读取），竖着居中。
 - 第一行：网站图标 + 链接（去掉 https://www.，只占一行，放不下时中间省略）；
 - 第二行：左边扫光字「正在读取视频信息…」+ 已经用了几秒，右边去向「发到 Sol 的 MacBook Pro」/「在这台手机上下」；
 - 底下一条不确定进度。
 */
@Composable
private fun ReadingContent(url: String, plat: String?, dest: String, startedAt: Long, onStop: () -> Unit) {
  val stop = str(R.string.composer_stop)
  val reduced = LocalReducedMotion.current
  // 已经用了几秒：每 100 毫秒一跳（等宽数字，不做动画；配方 c6）
  val now = rememberNow(100)
  val elapsed = if (startedAt > 0) String.format(Locale.ROOT, "%.1fs", ((now - startedAt).coerceAtLeast(0)) / 1000.0) else "1.8s"
  val phase = rememberShimmerPhase()
  Row(
    Modifier.fillMaxWidth().padding(start = 16.dp, end = 12.dp, top = 14.dp, bottom = 16.dp),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(12.dp),
  ) {
    Column(Modifier.weight(1f)) {
      Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        if (plat != null) SiteMark(plat, size = 18.dp)
        Text(
          url.removePrefix("https://").removePrefix("http://").removePrefix("www."),
          style = CaptaeType.body,
          maxLines = 1,
          overflow = TextOverflow.MiddleEllipsis,
          modifier = Modifier.weight(1f),
        )
      }
      Row(
        Modifier.fillMaxWidth().padding(top = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
      ) {
        // 扫光文字（配方 m，同桌面端 cp-wait、Gemini「正在搜索网络」）：2.7 秒一次，减少动效时不扫
        Text(str(R.string.composer_reading) + "…", style = CaptaeType.footnote, color = Palette.ink3, maxLines = 1, modifier = Modifier.shimmer(reduced, phase))
        Text(elapsed, style = CaptaeType.footnote.monospacedDigit(), color = Palette.ink4, maxLines = 1)
        // 去向靠右：先量左边两段，剩下的宽度给它，太长时它自己截断
        Spacer(Modifier.weight(1f))
        Text(dest, style = CaptaeType.footnote, color = Palette.ink3, maxLines = 1, overflow = TextOverflow.Ellipsis)
      }
      LiveProgressBar(0f, indeterminate = true, label = str(R.string.job_reading_a11y), modifier = Modifier.padding(top = 12.dp))
    }
    Box(
      Modifier
        .size(40.dp)
        .clip(CircleShape)
        .background(Palette.ink)
        .clickable(role = Role.Button, onClick = onStop)
        .semantics { contentDescription = stop },
      contentAlignment = Alignment.Center,
    ) {
      Box(Modifier.size(12.dp).clip(RoundedCornerShape(2.dp)).background(Color.White))
    }
  }
}

/** 胶囊菜单的样子：浅灰胶囊「发到 Sol 的 MacBook Pro ⌄」（字太长时截断，箭头一直在；Perplexity 的「模型」胶囊） */
@Composable
private fun Chip(title: String) {
  Row(
    Modifier
      .heightIn(min = 34.dp)
      .clip(CircleShape)
      .background(Palette.select)
      .padding(start = 11.dp, end = 9.dp, top = 6.dp, bottom = 6.dp),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(3.dp),
  ) {
    Text(title, style = CaptaeType.footnote.weight(FontWeight.Medium), color = Palette.ink, maxLines = 1, modifier = Modifier.weight(1f, fill = false))
    Icon(Sym.chevronDown, size = 13.dp, weight = FontWeight.SemiBold, tint = Palette.ink3)
  }
}

/** 36 的图标钮（粘贴） */
@Composable
private fun IconButton(sf: String, label: String, onClick: () -> Unit) {
  Box(
    Modifier
      .size(36.dp)
      .clip(CircleShape)
      .clickable(role = Role.Button, onClick = onClick)
      .semantics { contentDescription = label },
    contentAlignment = Alignment.Center,
  ) {
    Icon(sf, size = 19.dp, tint = Palette.ink2)
  }
}

/** 圆形发送钮：近黑、白色向上箭头；没输入时灰（桌面端输入框右下角） */
@Composable
private fun SendButton(enabled: Boolean, label: String, onClick: () -> Unit) {
  Box(
    Modifier
      .padding(start = 6.dp)
      .size(34.dp)
      .clip(CircleShape)
      .background(if (enabled) Palette.ink else Color(0xFFD4D4D4))
      .clickable(enabled = enabled, role = Role.Button, onClick = onClick)
      .semantics { contentDescription = label },
    contentAlignment = Alignment.Center,
  ) {
    Icon(Sym.arrowUp, size = 18.dp, weight = FontWeight.SemiBold, tint = Color.White)
  }
}
