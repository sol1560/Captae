package app.captae.glass.core

import androidx.compose.runtime.Immutable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.kyant.backdrop.isRenderEffectSupported
import com.kyant.backdrop.isRuntimeShaderSupported

/**
 一套玻璃参数（浅或深各一套）。数值来自 medcxy-v7：拿同一张背景图在 iOS 27 模拟器和安卓模拟器上并排截图、逐项对出来的。
 Captae 只改了表面色（见 [GlassStyle.RegularLight] 的注释）；模糊、折射、高光、暗边、投影照搬。改之前先重新截图对照 iOS 版。
 */
@Immutable
data class GlassLook(
  /** 背景模糊半径（backdrop 的 blur，安卓 RenderEffect 半径，约等于 1.7 倍高斯 sigma） */
  val blur: Dp,
  /** 边缘折射：从边缘往里多宽开始弯（lens refractionHeight） */
  val lensHeight: Dp,
  /** 折射量（lens refractionAmount） */
  val lensAmount: Dp,
  /** 背景饱和度放大（iOS 玻璃会把底下的颜色提鲜） */
  val saturation: Float,
  /** 背景亮度偏移（-1..1，0 不变） */
  val brightness: Float,
  /** 表面色（含不透明度）：盖在模糊背景上的那层奶白 / 深灰 */
  val surface: Color,
  /** 只有模糊没有折射（API 31–32）时的表面色：没有折射和高光，表面要厚一点才像 */
  val surfaceBlurOnly: Color,
  /** 什么效果都没有（API 26–30）时的表面色：几乎不透明的磨砂面 */
  val surfaceFlat: Color,
  /** 边缘高光（亮的一圈）的线宽和亮度 */
  val highlightWidth: Dp,
  val highlightAlpha: Float,
  /** 贴着边缘的一圈暗线（iOS 实测约 0.67pt，比背景暗 30–65 级） */
  val rim: Color,
  val rimWidth: Dp,
  /** 投影：iOS 很淡（约 1–1.5%），往下 8–10pt */
  val shadowRadius: Dp,
  val shadowOffset: Dp,
  val shadowColor: Color,
)

/** iOS 的几种玻璃：regular（绝大多数）、clear（更透，媒体上）、菜单 / 弹层（更厚的磨砂） */
object GlassStyle {
  /**
   Captae 改过：表面色从 medcxy 的暖白（#FBFAF8 / #FCFAF7 / #F9F7F4）换成中性白。Captae 规矩第 1 条是「中性近白，不加米黄」，
   暖白玻璃压在 #F7F7F7 的主区和白卡上会透出一层黄；透明度照旧（.6 / .72 / .96），只把颜色拉到 R=G=B。
   */
  val RegularLight = GlassLook(
    blur = 2.5.dp,
    lensHeight = 12.dp,
    lensAmount = 18.dp,
    saturation = 2.2f,
    brightness = 0f,
    surface = Color(0xFFFBFBFB).copy(alpha = 0.6f),
    surfaceBlurOnly = Color(0xFFFBFBFB).copy(alpha = 0.72f),
    surfaceFlat = Color(0xFFF9F9F9).copy(alpha = 0.96f),
    highlightWidth = 0.5.dp,
    highlightAlpha = 0.45f,
    rim = Color.Black.copy(alpha = 0.2f),
    rimWidth = 0.67.dp,
    shadowRadius = 10.dp,
    shadowOffset = 3.dp,
    shadowColor = Color.Black.copy(alpha = 0.06f),
  )

  /** 深色那一套照 medcxy 留着：手机端第一版只做浅色（同桌面端），以后两边一起加深色时再对照 iOS 校 */
  val RegularDark = GlassLook(
    blur = 2.5.dp,
    lensHeight = 12.dp,
    lensAmount = 18.dp,
    saturation = 1.8f,
    brightness = 0f,
    surface = Color(0xFF5C5B58).copy(alpha = 0.43f),
    surfaceBlurOnly = Color(0xFF3E3C38).copy(alpha = 0.78f),
    surfaceFlat = Color(0xFF302E2B).copy(alpha = 0.97f),
    highlightWidth = 0.5.dp,
    highlightAlpha = 0.35f,
    rim = Color.Black.copy(alpha = 0.35f),
    rimWidth = 0.67.dp,
    shadowRadius = 12.dp,
    shadowOffset = 4.dp,
    shadowColor = Color.Black.copy(alpha = 0.25f),
  )

  /** 透明玻璃（iOS .clear）：照片、视频之类的媒体上用，表面几乎没有颜色 */
  val ClearLight = RegularLight.copy(blur = 4.dp, surface = Color.White.copy(alpha = 0.12f), surfaceBlurOnly = Color.White.copy(alpha = 0.3f))
  val ClearDark = RegularDark.copy(blur = 4.dp, surface = Color.Black.copy(alpha = 0.12f), surfaceBlurOnly = Color.Black.copy(alpha = 0.3f))

  /**
   菜单、确认框、提醒框、半高 sheet：iOS 的弹层玻璃模糊更重。浅色更白更厚；深色几乎只剩模糊（iOS 真 App 深色菜单在 #11100D 上
   渲染成 #1C1C1E–#2D2D2F，比底只亮一点）。
   */
  // Captae 改过：表面色同上换成中性白（medcxy 是 #FCFAF7）
  val PanelLight = RegularLight.copy(blur = 28.dp, lensHeight = 20.dp, lensAmount = 28.dp, surface = Color(0xFFFCFCFC).copy(alpha = 0.66f), surfaceBlurOnly = Color(0xFFFCFCFC).copy(alpha = 0.8f))
  val PanelDark = RegularDark.copy(blur = 28.dp, lensHeight = 20.dp, lensAmount = 28.dp, surface = Color(0xFF6E6E70).copy(alpha = 0.18f), surfaceBlurOnly = Color(0xFF3A3A3C).copy(alpha = 0.72f))

  fun regular(dark: Boolean) = if (dark) RegularDark else RegularLight
  fun clear(dark: Boolean) = if (dark) ClearDark else ClearLight
  fun panel(dark: Boolean) = if (dark) PanelDark else PanelLight
}

/** 按系统版本退化：33+ 全效果；31–32 只有模糊和调色、没有折射；26–30 什么效果都没有，画成磨砂面 */
enum class GlassTier {
  Full, BlurOnly, Flat;

  companion object {
    val current: GlassTier by lazy {
      when {
        isRuntimeShaderSupported() -> Full
        isRenderEffectSupported() -> BlurOnly
        else -> Flat
      }
    }
  }
}
