package app.captae.glass.theme

import androidx.compose.ui.text.PlatformTextStyle
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.LineHeightStyle
import androidx.compose.ui.unit.sp
import app.captae.glass.R

/**
 字：Geist（拉丁，OFL 1.1，res/font，许可在 assets/licenses/Geist-OFL.txt）+ 系统中文字体（Geist 里没有的字由系统回退到
 Noto Sans CJK 或厂商字体；苹方 / SF 只许在苹果设备上用，不打包）。
 字号跟平台走、跟随系统字号（用 sp）。BRIEF 第 1 节：大标题 34 粗、标题 17 半粗、正文 17、次要 15、元信息 13、小字 12；
 其余几档（title3、callout、caption2）是搬来的 medcxy 部件要用的，数值照 iOS 文字样式。
 lineHeight 写行距（iOS HIG leading），首末行裁掉（Trim.Both），单行文字就是字体的自然高。
 */
object CaptaeType {
  val geist = FontFamily(
    Font(R.font.geist_regular, FontWeight.Normal),
    Font(R.font.geist_medium, FontWeight.Medium),
    Font(R.font.geist_semibold, FontWeight.SemiBold),
    Font(R.font.geist_bold, FontWeight.Bold),
  )

  /** 等宽：网址、时长角标（桌面端 --mono） */
  val geistMono = FontFamily(
    Font(R.font.geist_mono_regular, FontWeight.Normal),
    Font(R.font.geist_mono_medium, FontWeight.Medium),
  )

  val largeTitle = style(34, 41, FontWeight.Bold)
  val title2 = style(22, 28, FontWeight.SemiBold)
  val title3 = style(20, 25, FontWeight.SemiBold)
  /** 标题（卡片标题、导航栏小标题） */
  val headline = style(17, 22, FontWeight.SemiBold)
  /** 正文 */
  val body = style(17, 22)
  val callout = style(16, 21)
  /** 次要 */
  val subheadline = style(15, 20)
  /** 元信息 */
  val footnote = style(13, 18)
  /** 小字 */
  val caption = style(12, 16)
  val caption2 = style(11, 13)

  private fun style(size: Int, leading: Int, weight: FontWeight = FontWeight.Normal) = TextStyle(
    fontFamily = geist,
    fontSize = size.sp,
    lineHeight = leading.sp,
    fontWeight = weight,
    // Tight：中文回退字体比行高高时不在首行上、末行下补空白
    lineHeightStyle = LineHeightStyle(LineHeightStyle.Alignment.Center, LineHeightStyle.Trim.Both, LineHeightStyle.Mode.Tight),
    platformStyle = PlatformTextStyle(includeFontPadding = false),
  )
}

/** iOS `.font(.body.weight(.semibold))` */
fun TextStyle.weight(weight: FontWeight): TextStyle = copy(fontWeight = weight)

/** 会变的数字（进度、速度、剩余时间、时长、大小）用等宽数字：Geist 的 tabular-nums（同桌面端 font-variant-numeric） */
fun TextStyle.monospacedDigit(): TextStyle = copy(fontFeatureSettings = "tnum")

/** Geist Mono（网址、时长角标） */
fun TextStyle.monospaced(): TextStyle = copy(fontFamily = CaptaeType.geistMono)
