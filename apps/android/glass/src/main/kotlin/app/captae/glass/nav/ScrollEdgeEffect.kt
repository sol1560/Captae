package app.captae.glass.nav

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.RectangleShape
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.kyant.backdrop.Backdrop
import com.kyant.backdrop.drawPlainBackdrop
import com.kyant.backdrop.effects.blur
import com.kyant.backdrop.effects.runtimeShaderEffect
import app.captae.glass.core.GlassTier
import app.captae.glass.core.LocalBackdrop
import app.captae.glass.core.LocalGlassTierOverride

enum class ScrollEdge { Top, Bottom }

/**
 滚动边缘（BRIEF 2.7，iOS 26 的 soft 滚动边缘效果）：内容滚到顶栏 / 标签栏 / 下载条 / sheet 标题栏下面时，靠边那一段越靠边越糊、
 越往里越清楚，并且淡进底色；没有硬分界线、没有发丝线（只有这一种，不提供 hard 的写法）。
 33+ 叠两层渐变遮罩的模糊：外层大半径（只在最靠边的一段），内层小半径（一直过渡到清晰）；更低的版本只铺一层底色渐变。
 */
@Composable
fun ScrollEdgeEffect(edge: ScrollEdge, height: Dp, color: Color, modifier: Modifier = Modifier, backdrop: Backdrop? = LocalBackdrop.current) {
  val tier = LocalGlassTierOverride.current ?: GlassTier.current
  val top = edge == ScrollEdge.Top
  if (tier == GlassTier.Full && backdrop != null) {
    // 两层的叠法：(模糊半径, 这一层清晰到哪儿（占高度的比例）, 叠多少底色)
    for ((radius, reach, tint) in listOf(Triple(4.dp, 1f, 0.55f), Triple(14.dp, 0.6f, 0.45f))) {
      Box(
        modifier
          .fillMaxWidth()
          .height(height)
          .drawPlainBackdrop(
            backdrop = backdrop,
            shape = { RectangleShape },
            effects = {
              blur(radius.toPx())
              runtimeShaderEffect("CaptaeScrollEdge", EdgeShader, "content") {
                setFloatUniform("size", size.width, size.height)
                setFloatUniform("top", if (top) 1f else 0f)
                setFloatUniform("reach", reach)
                setFloatUniform("tintAmount", tint)
                setColorUniform("tint", color)
              }
            },
          ),
      )
    }
  } else {
    val stops = if (top) listOf(color.copy(alpha = 0.92f), color.copy(alpha = 0.6f), color.copy(alpha = 0f)) else listOf(color.copy(alpha = 0f), color.copy(alpha = 0.6f), color.copy(alpha = 0.92f))
    Box(modifier.fillMaxWidth().height(height).background(Brush.verticalGradient(stops)))
  }
}

// 靠边一端：模糊 + 底色占满；往里用平滑曲线逐渐变回透明（露出清晰的内容），到 reach 处完全透明，中间没有台阶
private const val EdgeShader = """
uniform shader content;
uniform float2 size;
uniform float top;
uniform float reach;
uniform float tintAmount;
layout(color) uniform half4 tint;

half4 main(float2 coord) {
  float y = top > 0.5 ? coord.y : size.y - coord.y;
  float t = clamp(y / (size.y * reach), 0.0, 1.0);
  float blurAlpha = 1.0 - smoothstep(0.0, 1.0, t);
  float tintAlpha = (1.0 - smoothstep(0.0, 0.85, t)) * tintAmount;
  half4 c = content.eval(coord) * blurAlpha;
  return mix(c, tint * blurAlpha, tintAlpha);
}"""
