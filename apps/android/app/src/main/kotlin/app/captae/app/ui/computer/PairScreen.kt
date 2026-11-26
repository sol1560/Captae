package app.captae.app.ui.computer

import app.captae.app.R
import app.captae.app.ui.parts.str
import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.PickVisualMediaRequest
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.input.KeyboardType
import androidx.core.content.ContextCompat
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBars
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import app.captae.app.demo.Sheet
import kotlinx.coroutines.launch
import app.captae.glass.overlay.GlassSheet
import app.captae.glass.overlay.SheetDetent
import app.captae.glass.overlay.SheetHeader
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.core.view.WindowCompat
import app.captae.app.demo.DemoData
import app.captae.app.ui.parts.StatusLine
import app.captae.app.ui.parts.Tone
import app.captae.glass.controls.Radius
import app.captae.glass.controls.cardShadow
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospacedDigit
import app.captae.glass.theme.weight

// 配对（BRIEF 06）：扫码是全屏取景（相机画面上不放玻璃）；输 6 位数和失败是 sheet（BRIEF 2.8 第 6 条，照 iOS）

/**
 06-scan：全屏取景框。真数据时是 CameraX 的取景（要相机权限，第一次进来问；不给就是深灰底 + 去设置里打开的说明），
 认出二维码交给 onScanned；也能「从图片识别」（相册里的截图，模拟器上也靠它）。demo 时是深灰占位。
 画面上的按钮都是实心的半透明白，不是玻璃（玻璃采不到相机的 SurfaceView，iOS 也不在画面上放玻璃）。
 status：配对中 / 配对失败的一句话（显示在说明下面）。
 */
@Composable
fun ScanScreen(onBack: () -> Unit, onCode: () -> Unit, demo: Boolean = true, onScanned: (String) -> Unit = {}, status: String? = null, busy: Boolean = false) {
  val back = str(R.string.pair_back)
  val context = LocalContext.current
  LightStatusIcons()
  var granted by remember { mutableStateOf(ContextCompat.checkSelfPermission(context, Manifest.permission.CAMERA) == PackageManager.PERMISSION_GRANTED) }
  val ask = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { granted = it }
  LaunchedEffect(demo) { if (!demo && !granted) ask.launch(Manifest.permission.CAMERA) }
  val pick = rememberLauncherForActivityResult(ActivityResultContracts.PickVisualMedia()) { uri ->
    if (uri != null) onScanned(decodeQrFromImage(context, uri) ?: "")
  }
  Box(Modifier.fillMaxSize().background(Color(0xFF1C1C1E))) {
    if (!demo && granted && !busy) QrCameraPreview(onText = onScanned, modifier = Modifier.fillMaxSize())
    Column(
      Modifier.fillMaxSize().windowInsetsPadding(WindowInsets.statusBars).windowInsetsPadding(WindowInsets.navigationBars),
      horizontalAlignment = Alignment.CenterHorizontally,
    ) {
      Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 2.dp).height(44.dp), verticalAlignment = Alignment.CenterVertically) {
        Box(
          Modifier
            .size(44.dp)
            .clip(CircleShape)
            .background(Color.White.copy(alpha = 0.16f))
            .clickable(role = Role.Button, onClick = onBack)
            .semantics { contentDescription = back },
          contentAlignment = Alignment.Center,
        ) {
          Icon(Sym.chevronLeft, size = 24.dp, weight = FontWeight.Medium, tint = Color.White)
        }
      }
      Spacer(Modifier.weight(1f))
      Viewfinder(Modifier.fillMaxWidth(0.68f).aspectRatio(1f))
      Spacer(Modifier.height(28.dp))
      Text(
        str(R.string.pair_scan_hint),
        style = CaptaeType.subheadline.weight(FontWeight.Medium),
        color = Color.White,
        textAlign = TextAlign.Center,
        modifier = Modifier.padding(horizontal = 32.dp),
      )
      Spacer(Modifier.height(6.dp))
      Text(str(R.string.pair_same_wifi), style = CaptaeType.footnote, color = Color.White.copy(alpha = 0.6f), textAlign = TextAlign.Center)
      if (!demo && !granted) {
        Spacer(Modifier.height(10.dp))
        Text(str(R.string.pair_no_camera), style = CaptaeType.footnote, color = Color.White.copy(alpha = 0.8f), textAlign = TextAlign.Center, modifier = Modifier.padding(horizontal = 32.dp))
      }
      if (status != null) {
        Spacer(Modifier.height(10.dp))
        Text(status, style = CaptaeType.subheadline.weight(FontWeight.Medium), color = Color.White, textAlign = TextAlign.Center, modifier = Modifier.padding(horizontal = 32.dp))
      }
      Spacer(Modifier.weight(1.3f))
      if (!demo) {
        Text(
          str(R.string.pair_from_image),
          style = CaptaeType.subheadline.weight(FontWeight.Medium),
          color = Color.White,
          modifier = Modifier
            .padding(bottom = 10.dp)
            .clip(CircleShape)
            .clickable(role = Role.Button) { pick.launch(PickVisualMediaRequest(ActivityResultContracts.PickVisualMedia.ImageOnly)) }
            .padding(horizontal = 16.dp, vertical = 10.dp),
        )
      }
      Row(
        Modifier
          .padding(bottom = 24.dp)
          .heightIn(min = 48.dp)
          .clip(CircleShape)
          .background(Color.White.copy(alpha = 0.16f))
          .clickable(role = Role.Button, onClick = onCode)
          .padding(horizontal = 22.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
      ) {
        Icon(Sym.keyboard, size = 18.dp, tint = Color.White)
        Text(str(R.string.pair_enter_code), style = CaptaeType.subheadline.weight(FontWeight.SemiBold), color = Color.White)
      }
    }
  }
}

/** 取景框：四个白色圆角的角（和 logo 的取景框同一个意思） */
@Composable
private fun Viewfinder(modifier: Modifier = Modifier) {
  Canvas(modifier) {
    val w = 4.dp.toPx()
    val len = size.minDimension * 0.18f
    val r = 18.dp.toPx()
    val c = Color.White
    val s = size
    fun corner(x: Float, y: Float, dx: Float, dy: Float) {
      // 一个角：横线 + 圆弧 + 竖线
      val path = androidx.compose.ui.graphics.Path().apply {
        moveTo(x, y + dy * len)
        lineTo(x, y + dy * r)
        quadraticTo(x, y, x + dx * r, y)
        lineTo(x + dx * len, y)
      }
      drawPath(path, c, style = androidx.compose.ui.graphics.drawscope.Stroke(w, cap = StrokeCap.Round))
    }
    val i = w / 2
    corner(i, i, 1f, 1f)
    corner(s.width - i, i, -1f, 1f)
    corner(i, s.height - i, 1f, -1f)
    corner(s.width - i, s.height - i, -1f, -1f)
  }
}

/** 深色画面上状态栏用白色图标，离开这一页恢复 */
@Composable
private fun LightStatusIcons() {
  val view = LocalView.current
  DisposableEffect(view) {
    val window = (view.context as? Activity)?.window
    val c = window?.let { WindowCompat.getInsetsController(it, view) }
    val before = c?.isAppearanceLightStatusBars
    c?.isAppearanceLightStatusBars = false
    c?.isAppearanceLightNavigationBars = false
    onDispose {
      if (before != null) {
        c.isAppearanceLightStatusBars = before
        c.isAppearanceLightNavigationBars = before
      }
    }
  }
}

/**
 06-code / 06-failed：输电脑上看到的 6 位数（BRIEF 2.8 第 6 条，两边一样）。全高的 sheet，左上关闭，导航标题是电脑名；
 内容照 iOS：电脑图标（浅灰圆角方块）、大字「输入配对码」、小字「在电脑上看到的 6 位数」、6 个格子（白卡，正在输的那一格强调色的框）、
 下面一行说明去哪儿看。失败时格子里的数变灰，下面一条状态行（红色图标、一句话 + 一行小字、右边「重新扫码」）。
 */
@Composable
fun PairCodeSheet(sheet: Sheet.PairCode?, onDismiss: () -> Unit, onRescan: () -> Unit, onSubmit: (suspend (Sheet.PairCode, String) -> String?)? = null) {
  // 关的时候 sheet 已经是 null，退场动画里还要画上一次的内容
  var last by remember { mutableStateOf(sheet) }
  if (sheet != null) last = sheet
  GlassSheet(
    isPresented = sheet != null,
    onDismiss = onDismiss,
    detents = listOf(SheetDetent.Large),
    header = { SheetHeader(title = last?.name ?: DemoData.mac.name, onClose = onDismiss) },
  ) {
    val cur = last
    // 真数据：输满 6 位就去配对（SPAKE2，REMOTE.md 第 4 节）；失败时那一句话显示在下面，格子清空重输
    var digits by remember(cur) { mutableStateOf("") }
    var error by remember(cur) { mutableStateOf<String?>(null) }
    var busy by remember(cur) { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val focus = remember { FocusRequester() }
    if (onSubmit != null && cur != null) {
      LaunchedEffect(cur) { kotlinx.coroutines.delay(350); runCatching { focus.requestFocus() } }
      BasicTextField(
        value = digits,
        onValueChange = { v ->
          val d = v.filter { it.isDigit() }.take(6)
          digits = d
          if (d.length == 6 && !busy) {
            busy = true
            scope.launch {
              error = onSubmit(cur, d)
              busy = false
              if (error != null) digits = ""
            }
          }
        },
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
        modifier = Modifier.size(1.dp).focusRequester(focus),
      )
    }
    val failed = cur?.failed == true || error != null
    Column(
      Modifier
        .fillMaxSize()
        .verticalScroll(rememberScrollState())
        .padding(top = 40.dp, bottom = 32.dp, start = 20.dp, end = 20.dp),
      horizontalAlignment = Alignment.CenterHorizontally,
    ) {
      Box(Modifier.size(56.dp).clip(RoundedCornerShape(Radius.lg)).background(Palette.well), contentAlignment = Alignment.Center) {
        Icon(Sym.laptopcomputer, size = 32.dp, weight = FontWeight.Medium, tint = Palette.ink)
      }
      Spacer(Modifier.height(18.dp))
      Text(str(R.string.pair_code_title), style = CaptaeType.title2.weight(FontWeight.Bold), textAlign = TextAlign.Center)
      Spacer(Modifier.height(4.dp))
      Text(str(R.string.pair_code_sub), style = CaptaeType.subheadline, color = Palette.ink3, textAlign = TextAlign.Center)
      Spacer(Modifier.height(28.dp))
      CodeBoxes(if (onSubmit != null) digits else if (failed) "482915" else "482", failed && onSubmit == null, Modifier.clickable(interactionSource = null, indication = null) { runCatching { focus.requestFocus() } })
      Spacer(Modifier.height(20.dp))
      if (failed) {
        StatusLine(
          error ?: str(R.string.pair_expired),
          Tone.Err,
          Sym.exclamationmarkCircle,
          Modifier.fillMaxWidth().widthIn(max = 480.dp),
          sub = if (error == null) str(R.string.pair_expired_sub) else null,
          action = str(R.string.pair_rescan),
          onAction = onRescan,
        )
      } else {
        Text(str(R.string.pair_code_where), style = CaptaeType.footnote, color = Palette.ink3, textAlign = TextAlign.Center, modifier = Modifier.widthIn(max = 360.dp))
      }
    }
  }
}

/** 六个格子一样宽、一样的间距（照 iOS），整排最宽 336，窄屏时跟着缩 */
@Composable
private fun CodeBoxes(digits: String, failed: Boolean, modifier: Modifier = Modifier) {
  val a11y = str(R.string.pair_code_a11y, digits.length)
  Row(
    modifier.widthIn(max = 336.dp).fillMaxWidth().semantics { contentDescription = a11y },
    horizontalArrangement = Arrangement.spacedBy(9.dp),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    for (i in 0 until 6) {
      val active = !failed && i == digits.length
      Box(
        Modifier
          .weight(1f)
          .height(58.dp)
          .cardShadow(Radius.md)
          .clip(RoundedCornerShape(Radius.md))
          .background(Palette.card)
          .then(if (active) Modifier.border(1.5.dp, Palette.accent, RoundedCornerShape(Radius.md)) else Modifier),
        contentAlignment = Alignment.Center,
      ) {
        val d = digits.getOrNull(i)
        if (d != null) {
          Text(d.toString(), style = CaptaeType.title2.monospacedDigit().copy(fontSize = 26.sp, lineHeight = 30.sp), color = if (failed) Palette.ink3 else Palette.ink)
        }
      }
    }
  }
}
