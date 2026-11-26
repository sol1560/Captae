package app.captae.app.model

import app.captae.app.R
import app.captae.app.ui.parts.Str
import java.util.Locale
import kotlin.math.roundToInt

// 数字、时间的显示格式：和桌面端 apps/desktop/src/lib/format.ts 同一套说法（单位 MB / GB / MB/s 不翻译；时间的说法在 strings.xml）。

/** 12 MB / 1.40 GB；10 MB 以下保留一位小数（format.ts mb） */
fun mb(n: Double): String = when {
  n.isNaN() || n <= 0 -> "0 MB"
  n >= 1000 -> String.format(Locale.ROOT, "%.2f GB", n / 1000)
  n < 10 -> String.format(Locale.ROOT, "%.1f MB", n)
  else -> "${n.roundToInt()} MB"
}

/** 剩余时间：26 秒 / 3 分 12 秒 / 1 小时 5 分（format.ts dur） */
fun dur(seconds: Double): String {
  val s = seconds.coerceAtLeast(0.0).roundToInt()
  return when {
    s >= 3600 -> Str.fmt(R.string.dur_hm, s / 3600, (s % 3600) / 60)
    s >= 60 -> Str.fmt(R.string.dur_ms, s / 60, s % 60)
    else -> Str.fmt(R.string.dur_s, s)
  }
}

/** 速度：6.4 MB/s（桌面端 toFixed(1)） */
fun speed(mbPerSecond: Double): String = String.format(Locale.ROOT, "%.1f MB/s", mbPerSecond)

/** 剩余时间（桌面端 model.ts etaOf）：速度为 0 时不显示 */
fun Job.eta(): String = if (speed <= 0 || size <= 0) "" else dur((size - got) / speed)

/** 百分比：45%（向下取整，和桌面端一样） */
fun Job.percent(): String = "${(progress * 100).toInt()}%"
