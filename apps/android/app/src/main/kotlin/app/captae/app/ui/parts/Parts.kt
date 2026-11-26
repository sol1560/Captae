package app.captae.app.ui.parts

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.Build
import androidx.compose.foundation.Canvas
import androidx.compose.animation.core.Animatable
import androidx.compose.animation.core.tween
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.FilterQuality
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.IntSize
import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.BoxScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.drawscope.translate
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.captae.app.R
import app.captae.glass.controls.Radius
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospaced
import app.captae.glass.theme.monospacedDigit
import app.captae.glass.theme.weight

// 页面公用的小部件：照桌面端 apps/desktop/src/parts/index.tsx（logo、封面、站点图标、纸张格式图标）和 theme.css 的数值。

/** 取景框四角（和桌面端 CaptaeLogo 的 MARK 是同一条路径，24 格） */
private const val MARK = "M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"
/** 圆角方块（64 格）：桌面端 logo 的外形，不是普通圆角矩形，是 18 的「超椭圆」角 */
private const val SQUIRCLE = "M0 18C0 5.4 5.4 0 18 0h28c12.6 0 18 5.4 18 18v28c0 12.6-5.4 18-18 18H18C5.4 64 0 58.6 0 46Z"

/** Captae 的 logo：黑色圆角方块（上 #2E2E30 到下 #0E0E0F）+ 白色取景框和圆点（桌面端 CaptaeLogo，同一个图形） */
@Composable
fun CaptaeLogo(size: Dp = 44.dp, modifier: Modifier = Modifier) {
  val square = remember { PathParser().parsePathString(SQUIRCLE).toPath() }
  val mark = remember { PathParser().parsePathString(MARK).toPath() }
  Canvas(modifier.size(size)) {
    val k = this.size.minDimension / 64f
    scale(k, pivot = Offset.Zero) {
      drawPath(square, Brush.verticalGradient(listOf(Color(0xFF2E2E30), Color(0xFF0E0E0F)), startY = 0f, endY = 64f))
      // 桌面端：translate(14.6 14.6) scale(1.45)，描边 2.35
      translate(14.6f, 14.6f) {
        scale(1.45f, pivot = Offset.Zero) {
          val stroke = Stroke(2.35f, cap = StrokeCap.Round, join = StrokeJoin.Round)
          drawPath(mark, Color.White, style = stroke)
          drawCircle(Color.White, radius = 2.6f, center = Offset(12f, 12f), style = stroke)
        }
      }
    }
  }
}

/** 演示剧照：`still:<名字>` → res/drawable-nodpi/still_<名字>.jpg。真数据（文件路径、网址）走 [rememberCover] */
fun stillRes(thumb: String?): Int? = when (thumb) {
  "still:bbb_frank" -> R.drawable.still_bbb_frank
  "still:ed_emo_proog" -> R.drawable.still_ed_emo_proog
  "still:ed_platform" -> R.drawable.still_ed_platform
  "still:sintel_1" -> R.drawable.still_sintel_1
  "still:sintel_3" -> R.drawable.still_sintel_3
  "still:tos_01" -> R.drawable.still_tos_01
  "still:tos_05" -> R.drawable.still_tos_05
  else -> null
}

/** 网站图标（Simple Icons，CC0；gen-icons.mts 从桌面端转的 VectorDrawable）：按 plat 找，找不到返回空（同桌面端 siteIconOf） */
fun siteRes(plat: String?): Int? {
  val k = plat?.lowercase() ?: return null
  return when {
    "youtube" in k || "youtu.be" in k -> R.drawable.site_youtube
    "bilibili" in k || "b23.tv" in k -> R.drawable.site_bilibili
    "douyin" in k || "tiktok" in k -> R.drawable.site_tiktok
    k == "x" || "x.com" in k || "twitter" in k -> R.drawable.site_x
    "instagram" in k -> R.drawable.site_instagram
    else -> null
  }
}

/** 站点小标：认得的网站画它的图标，不认得的画一个地球（桌面端 SiteIcon） */
@Composable
fun SiteMark(plat: String?, size: Dp = 13.dp, modifier: Modifier = Modifier, tint: Color = Palette.ink3) {
  val res = siteRes(plat)
  if (res != null) {
    Image(painterResource(res), contentDescription = null, modifier = modifier.size(size))
  } else {
    Icon(Sym.globe, size = size, tint = tint, modifier = modifier)
  }
}

/** 灰度（封面还没读到时，站点图标画成灰的） */
private val Grayscale = ColorFilter.colorMatrix(ColorMatrix().apply { setToSaturation(0f) })

/**
 封面（桌面端 .cp-thumb）：圆角 8，读不到图时是 --well 灰底中间一个灰色的站点图标或播放图标；右下角时长角标。
 时长角标是 rgba(0,0,0,.55) 底、白色 Geist Mono 10.5（桌面端还加了背后模糊；画面上不放玻璃，这里不模糊）。
 thumb：演示剧照 `still:<名字>`，或者真数据的文件路径 / 网址（[rememberCover] 读）。
 第一次读到时淡入 300ms（--ease-out-quint，MOTION 1.2「封面读到」：图多，只动透明度）；这次打开以来读到过的直接显示。
 */
@Composable
fun Thumb(
  thumb: String?,
  modifier: Modifier = Modifier,
  dur: String = "",
  plat: String? = null,
  corner: Dp = Radius.sm,
  dim: Boolean = false,
  overlay: (@Composable BoxScope.() -> Unit)? = null,
) {
  val res = stillRes(thumb)
  val cover = rememberCover(if (res == null) thumb else null)
  val has = res != null || cover.image != null
  Box(modifier.clip(RoundedCornerShape(corner)).background(if (has) Palette.thumb else Palette.well)) {
    if (!has) {
      val site = siteRes(plat)
      Box(Modifier.align(Alignment.Center)) {
        if (site != null) {
          Image(painterResource(site), contentDescription = null, colorFilter = Grayscale, alpha = 0.28f, modifier = Modifier.size(20.dp))
        } else {
          Icon(Sym.play, size = 20.dp, tint = Palette.ink4)
        }
      }
    } else {
      val alpha = remember(cover.image) { Animatable(if (cover.fresh) 0f else 1f) }
      LaunchedEffect(cover.image) { if (alpha.value < 1f) alpha.animateTo(1f, tween(300, easing = Ease.OutQuint)) }
      val m = Modifier.fillMaxSize().graphicsLayer { this.alpha = alpha.value }
      if (res != null) {
        Image(painterResource(res), contentDescription = null, contentScale = ContentScale.Crop, colorFilter = if (dim) Grayscale else null, modifier = m)
      } else {
        Image(cover.image!!, contentDescription = null, contentScale = ContentScale.Crop, colorFilter = if (dim) Grayscale else null, modifier = m)
      }
      if (dur.isNotEmpty()) DurationBadge(dur, Modifier.align(Alignment.BottomEnd).padding(5.dp))
    }
    overlay?.invoke(this)
  }
}

/** 时长角标（桌面端 .dur：右下 5、内边距 3×5、圆角 5、rgba(0,0,0,.55)、白色 Geist Mono 500 10.5） */
@Composable
fun DurationBadge(dur: String, modifier: Modifier = Modifier) {
  Box(modifier.clip(RoundedCornerShape(5.dp)).background(Color.Black.copy(alpha = 0.55f)).padding(horizontal = 5.dp, vertical = 3.dp)) {
    Text(dur, style = CaptaeType.caption2.monospaced().copy(fontSize = 10.5.sp, lineHeight = 11.sp, fontWeight = FontWeight.Medium).monospacedDigit(), color = Color.White, maxLines = 1)
  }
}

/** 音频的封面（桌面端素材库：深灰底 + 白色竖条波形） */
@Composable
fun WaveformCover(modifier: Modifier = Modifier, dur: String = "", corner: Dp = Radius.sm, dim: Boolean = false) {
  Box(modifier.clip(RoundedCornerShape(corner)).background(Color(0xFF262626))) {
    // 内边距按比例（大封面约 24 / 18，小图上竖条也不会被挤没）
    Canvas(Modifier.fillMaxSize()) {
      val padX = size.width * 0.14f
      val padY = size.height * 0.2f
      translate(padX, padY) {
        val area = androidx.compose.ui.geometry.Size(this.size.width - padX * 2, this.size.height - padY * 2)
        // 一组固定的高度（不随机，截图每次一样），左右对称
        val heights = floatArrayOf(.28f, .45f, .62f, .4f, .78f, .55f, .9f, .66f, .48f, .82f, .6f, .95f, .7f, .52f, .86f, .44f, .72f, .58f, .36f, .64f, .5f, .3f)
        val n = heights.size
        val gap = area.width / (n * 2 - 1)
        val w = gap
        for (i in 0 until n) {
          val h = area.height * heights[i]
          drawRoundRect(
            Color.White.copy(alpha = if (dim) 0.5f else 0.9f),
            topLeft = Offset(i * gap * 2, (area.height - h) / 2),
            size = Size(w, h),
            cornerRadius = CornerRadius(w / 2),
          )
        }
      }
    }
    if (dur.isNotEmpty()) DurationBadge(dur, Modifier.align(Alignment.BottomEnd).padding(5.dp))
  }
}

/** 状态行的语气：要登录（橙）、失败（红）、成功（绿） */
enum class Tone(val color: Color) { Warn(Palette.warn), Err(Palette.err), Ok(Palette.ok) }

/**
 状态行（桌面端规矩第 4 条）：浅灰凹面 + 上了色的小图标 + 一句话 + 右边一个下一步按钮。不用彩色大底块。
 sub：一句话下面再一行灰色小字（配对码过期时「配对码 5 分钟有效，用一次就作废」）；有它时图标和第一行对齐。
 */
@Composable
fun StatusLine(text: String, tone: Tone, sf: String, modifier: Modifier = Modifier, sub: String? = null, action: String? = null, onAction: () -> Unit = {}) {
  Row(
    modifier
      .clip(RoundedCornerShape(Radius.md))
      .background(Palette.well)
      .heightIn(min = 44.dp)
      .padding(start = 12.dp, end = if (action != null) 6.dp else 12.dp, top = if (sub != null) 10.dp else 6.dp, bottom = if (sub != null) 10.dp else 6.dp),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(8.dp),
  ) {
    if (sub == null) {
      Icon(sf, size = 16.dp, weight = FontWeight.Medium, tint = tone.color)
      Text(text, style = CaptaeType.footnote, color = Palette.ink, modifier = Modifier.weight(1f))
    } else {
      Row(Modifier.weight(1f), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        Icon(sf, size = 16.dp, weight = FontWeight.Medium, tint = tone.color, modifier = Modifier.padding(top = 1.dp))
        Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
          Text(text, style = CaptaeType.footnote, color = Palette.ink)
          Text(sub, style = CaptaeType.caption, color = Palette.ink3)
        }
      }
    }
    if (action != null) {
      Box(
        Modifier
          .clip(CircleShape)
          .background(Palette.card)
          .clickable(role = Role.Button, onClick = onAction)
          .padding(horizontal = 12.dp, vertical = 7.dp),
      ) {
        Text(action, style = CaptaeType.footnote.weight(FontWeight.SemiBold), color = Palette.ink, maxLines = 1)
      }
    }
  }
}

/**
 纸张格式图标（桌面端 FileIcon，brand-spec v2 的 file3d）：白纸 + 左下角一个上了色的格式小标。
 颜色照桌面端 fmtTone：视频蓝、音频橙、图片绿、流灰。
 */
@Composable
fun FileBadge(fmt: String, modifier: Modifier = Modifier) {
  val tone = when (fmt.uppercase()) {
    "MP3", "AAC", "FLAC", "WAV", "M4A", "OPUS" -> Color(0xFFE8730C)
    "PNG", "JPG", "JPEG", "WEBP", "AVIF", "GIF" -> Color(0xFF1F8F50)
    "M3U8", "HLS" -> Color(0xFF6B6B6B)
    "WEBM", "MKV" -> Color(0xFF7A4CC2)
    else -> Palette.accent
  }
  Box(modifier.size(width = 36.dp, height = 44.dp)) {
    Box(
      Modifier
        .padding(start = 4.dp)
        .size(width = 32.dp, height = 42.dp)
        .clip(RoundedCornerShape(5.dp))
        .background(Palette.card)
        .background(Brush.verticalGradient(listOf(Color.White, Color(0xFFF1F1F1)))),
    )
    Canvas(Modifier.padding(start = 4.dp).size(width = 32.dp, height = 42.dp)) {
      drawRoundRect(Color.Black.copy(alpha = 0.1f), cornerRadius = CornerRadius(5.dp.toPx()), style = Stroke(0.5.dp.toPx()))
    }
    Box(
      Modifier
        .align(Alignment.BottomStart)
        .padding(bottom = 6.dp)
        .clip(RoundedCornerShape(3.dp))
        .background(tone)
        .padding(horizontal = 3.dp, vertical = 1.dp),
    ) {
      Text(fmt.uppercase(), style = CaptaeType.caption2.monospaced().copy(fontSize = 8.sp, lineHeight = 9.sp, fontWeight = FontWeight.Medium), color = Color.White, maxLines = 1)
    }
  }
}

/** 一行里「图标 + 灰字」的元信息（站点 · 格式） */
@Composable
fun MetaLine(text: String, plat: String?, modifier: Modifier = Modifier, showSite: Boolean = true) {
  Row(modifier, verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
    if (showSite) SiteMark(plat, size = 13.dp)
    Text(text, style = CaptaeType.footnote, color = Palette.ink3, maxLines = 1)
  }
}

/** 中圆点分隔（桌面端「 · 」） */
fun dotJoin(vararg parts: String?): String = parts.filterNot { it.isNullOrEmpty() }.joinToString(" · ")


/**
 封面的光（BRIEF 2.6）：把这条视频的封面缩成很小一张（24×14），再拉大铺满一块（双线性放大本身就是模糊；31+ 再叠一层模糊更匀），
 往下渐渐淡掉。颜色来自真实内容，不是挑的装饰色；没有封面（音频、读取中）就什么都不画。
 放在页面 / sheet 最上面、内容下面一层，跟着内容滚。strength 是最上面的不透明度。
 */
@Composable
fun CoverGlow(thumb: String?, modifier: Modifier = Modifier, height: Dp = 320.dp, strength: Float = 0.5f) {
  val res = stillRes(thumb)
  val cover = rememberCover(if (res == null) thumb else null)
  val context = LocalContext.current
  val tiny = remember(res, cover.image) {
    val big: Bitmap? = when {
      res != null -> BitmapFactory.decodeResource(context.resources, res, BitmapFactory.Options().apply { inSampleSize = 16 })
      cover.image != null -> cover.image.asAndroidBitmap()
      else -> null
    }
    big?.let { Bitmap.createScaledBitmap(it, 24, 14, true).asImageBitmap() }
  } ?: return
  // 读到封面时光是慢慢亮起来的（和封面淡入同一个时长），不突然铺上一层颜色
  val alpha = remember(tiny) { Animatable(if (cover.fresh) 0f else 1f) }
  LaunchedEffect(tiny) { if (alpha.value < 1f) alpha.animateTo(1f, tween(300, easing = Ease.OutQuint)) }
  Canvas(
    modifier
      .fillMaxWidth()
      .height(height)
      .then(if (Build.VERSION.SDK_INT >= 31) Modifier.blur(28.dp, BlurredEdgeTreatment.Unbounded) else Modifier)
      .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen; this.alpha = alpha.value },
  ) {
    drawImage(tiny, dstSize = IntSize(size.width.toInt(), size.height.toInt()), alpha = strength, filterQuality = FilterQuality.High)
    // 往下淡掉：上面 35% 是满的，到底变透明（DstIn 遮罩）
    drawRect(
      Brush.verticalGradient(0f to Color.Black, 0.35f to Color.Black.copy(alpha = 0.8f), 1f to Color.Transparent),
      blendMode = BlendMode.DstIn,
    )
  }
}

/**
 元信息「哔哩哔哩 · MP4 · 1080p」：放不下时只在「 · 」处换行，每一段自己不断开（不会断成「MP4 文 / 件」）；大字号时可以换行（BRIEF 第 6 节）。
 */
@OptIn(androidx.compose.foundation.layout.ExperimentalLayoutApi::class)
@Composable
fun MetaFlow(parts: List<String>, modifier: Modifier = Modifier, style: androidx.compose.ui.text.TextStyle = CaptaeType.footnote, color: Color = Palette.ink3) {
  androidx.compose.foundation.layout.FlowRow(modifier) {
    // 分隔点挂在上一段末尾：换行时是「mango.blender.org ·」换行「M3U8 → MP4」，下一行不以点开头
    val shown = parts.filter { it.isNotEmpty() }
    shown.forEachIndexed { i, p ->
      Text(if (i < shown.size - 1) "$p · " else p, style = style, color = color, maxLines = 1, softWrap = false, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
    }
  }
}
