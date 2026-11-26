package app.captae.glass.icons

import android.util.Log
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.drawWithContent
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.CompositingStrategy
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.addPathNodes
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.isSpecified
import app.captae.glass.theme.LocalContentColor

/** 一条路径（Hugeicons 的 24×24 格）：描边、实心，或两样都有 */
@Immutable
internal class HugePath(
  val d: String,
  val stroke: Boolean = true,
  val fill: Boolean = false,
  val evenOdd: Boolean = false,
  val alpha: Float = 1f,
  /** 实心版里要挖空的线（画面里的山、点）：不画进图标，画完图标再从上面擦掉 */
  val knockout: Boolean = false,
)

/** 一个图标：Hugeicons 的名字和它的几条路径（生成在 Symbols.kt） */
@Immutable
internal class HugeSpec(val name: String, val paths: List<HugePath>)

/**
 SF Symbol 名 → Hugeicons 图标（对照表在 scripts/gen-icons.mts，生成 Symbols.kt）。
 iOS 写 `Image(systemName: "arrow.down.circle")` 的地方，安卓写 `Icon(Sym.arrowDownCircle)`。
 */
object CaptaeIcons {
  /** 图标和字号的比例：Hugeicons 的字形占 24 格里约 18 格，和同字号的 SF Symbol 差不多大时外框约 1.2 倍字号 */
  const val FontScale = 1.2f

  /** 描边粗细（24 格里的单位）：桌面端 Icon 默认 1.75；字重越粗描边越粗，跟着按钮字走 */
  fun strokeFor(weight: FontWeight?): Float = when {
    weight == null -> 1.75f
    weight.weight >= 700 -> 2.2f
    weight.weight >= 600 -> 2f
    weight.weight >= 500 -> 1.9f
    else -> 1.75f
  }

  fun has(sf: String): Boolean = SymbolTable.specs.containsKey(sf)

  private val cache = HashMap<Pair<String, Float>, ImageVector>()
  private val knockCache = HashMap<Pair<String, Float>, ImageVector?>()

  /** 实心版要挖空的那几条线（没有就是 null） */
  internal fun knockout(sf: String, stroke: Float = 1.75f): ImageVector? = synchronized(knockCache) {
    knockCache.getOrPut(sf to stroke) {
      val spec = SymbolTable.specs[sf] ?: return null
      val knock = spec.paths.filter { it.knockout }
      if (knock.isEmpty()) null else build(HugeSpec(spec.name + "Knockout", knock), stroke)
    }
  }

  /** 这个 SF 名在某个描边粗细下的 ImageVector（同一个名字和粗细只建一次） */
  fun vector(sf: String, stroke: Float = 1.75f): ImageVector? = synchronized(cache) {
    cache.getOrPut(sf to stroke) {
      val spec = SymbolTable.specs[sf] ?: return null
      build(spec, stroke)
    }
  }

  private fun build(spec: HugeSpec, stroke: Float): ImageVector {
    val b = ImageVector.Builder(name = spec.name, defaultWidth = 24.dp, defaultHeight = 24.dp, viewportWidth = 24f, viewportHeight = 24f)
    for (p in spec.paths) {
      if (p.knockout && !spec.name.endsWith("Knockout")) continue
      b.addPath(
        pathData = addPathNodes(p.d),
        pathFillType = if (p.evenOdd) PathFillType.EvenOdd else PathFillType.NonZero,
        fill = if (p.fill) SolidColor(Color.Black) else null,
        fillAlpha = p.alpha,
        stroke = if (p.stroke) SolidColor(Color.Black) else null,
        strokeAlpha = p.alpha,
        strokeLineWidth = stroke,
        strokeLineCap = StrokeCap.Round,
        strokeLineJoin = StrokeJoin.Round,
      )
    }
    return b.build()
  }
}

/**
 画一个图标。size 不传时跟着 style 的字号走（iOS 的 Image 跟着 .font 走）；都不传是 20。
 tint 不传用当前的字色。contentDescription 为空时读屏跳过它（装饰图标）。
 */
@Composable
fun Icon(
  sf: String,
  modifier: Modifier = Modifier,
  size: Dp = Dp.Unspecified,
  style: TextStyle? = null,
  weight: FontWeight? = style?.fontWeight,
  tint: Color = Color.Unspecified,
  contentDescription: String? = null,
) {
  val density = LocalDensity.current
  val side = when {
    size.isSpecified -> size
    style != null && style.fontSize.isSpecified -> with(density) { style.fontSize.toDp() } * CaptaeIcons.FontScale
    else -> 20.dp
  }
  val color = if (tint != Color.Unspecified) tint else LocalContentColor.current
  val stroke = CaptaeIcons.strokeFor(weight)
  val vector = remember(sf, stroke) { CaptaeIcons.vector(sf, stroke) }
  val semantics = if (contentDescription != null) Modifier.semantics { this.contentDescription = contentDescription } else Modifier.clearAndSetSemantics {}
  if (vector == null) {
    // 表里没有：留出位置，日志里提醒补表（不崩）
    Log.w("CaptaeIcons", "图标表里没有 $sf：在 scripts/gen-icons.mts 的 TABLE 里加一行再跑")
    Box(modifier.size(side).then(semantics))
    return
  }
  val filter = remember(color) { ColorFilter.tint(color) }
  val knock = remember(sf, stroke) { CaptaeIcons.knockout(sf, stroke) }
  if (knock == null) {
    Image(vector, contentDescription = null, modifier = modifier.size(side).then(semantics), colorFilter = filter)
    return
  }
  // 实心版：先画图标，再把挖空的线用 DstOut 擦掉（离屏图层里擦，不会擦到底下的东西）
  val knockPainter = rememberVectorPainter(knock)
  Image(
    vector,
    contentDescription = null,
    colorFilter = filter,
    modifier = modifier
      .size(side)
      .then(semantics)
      .graphicsLayer { compositingStrategy = CompositingStrategy.Offscreen }
      .drawWithContent {
        drawContent()
        // this.size：函数参数 size（Dp）会遮住画布的尺寸
        val box = this.size
        drawIntoCanvas { c ->
          c.saveLayer(Rect(Offset.Zero, box), Paint().apply { blendMode = BlendMode.DstOut })
          with(knockPainter) { draw(box) }
          c.restore()
        }
      },
  )
}
