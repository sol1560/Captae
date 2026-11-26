package app.captae.glass.theme

import androidx.compose.ui.graphics.Color

/**
 Captae 的配色：和桌面端 `apps/desktop/src/theme.css` 开头 `:root` 的变量同名同值（docs/design/README.md 第 2 节）。
 中性近白，不加米黄；唯一的强调色是蓝，只用在进度条、焦点、选中的单选点、未读点、链接；
 状态色只上小图标和小字，不用彩色大底块。手机端第一版和桌面端一样只做浅色。
 下面「给搬来的部件用」那几个是 medcxy 玻璃部件里的叫法，指到同一套颜色上。
 */
object Palette {
  /** 主区底色（--main） */
  val main = Color(0xFFF7F7F7)
  /** 卡片、浮卡、输入框（--card） */
  val card = Color(0xFFFFFFFF)
  /** 卡片里的凹面：状态行、分段控件的底（--well） */
  val well = Color(0xFFF3F3F3)
  /** 侧栏（--side）：手机上用在相机画面以外的深一点的占位底 */
  val side = Color(0xFFEBEBEB)
  /** 按下、选中的底（--hover / --select） */
  val hover = Color(0x0B000000)
  val select = Color(0x11000000)
  /** 分隔线、描边（--line / --line-2） */
  val line = Color(0x12000000)
  val line2 = Color(0x1F000000)

  /** 正文、标题（--ink） */
  val ink = Color(0xFF171717)
  /** 次要文字（--ink-2） */
  val ink2 = Color(0xFF525252)
  /** 元信息（--ink-3） */
  val ink3 = Color(0xFF8B8B8B)
  /** 占位、不可用（--ink-4） */
  val ink4 = Color(0xFFB4B4B4)

  /** 唯一的强调色（--accent） */
  val accent = Color(0xFF007AFF)
  /** 强调色的文字（--accent-ink） */
  val accentInk = Color(0xFF0066D6)
  /** 强调色浅底（--accent-soft） */
  val accentSoft = Color(0x1A007AFF)

  /** 成功（--ok）、要登录 / 要验证（--warn）、失败（--err）：只上小图标和小字 */
  val ok = Color(0xFF1F8F50)
  val warn = Color(0xFFC2660A)
  val err = Color(0xFFD6382A)

  /** 弹窗遮罩：中灰洗白，不是压黑（--scrim） */
  val scrim = Color(0x808C8C8C)
  /** 深色提示条（--glass-dark） */
  val glassDark = Color(0xCC1A1A1C)

  /** 封面还没读到时的灰底（.cp-thumb 的 #e6e6e6） */
  val thumb = Color(0xFFE6E6E6)

  // ---- 给搬来的部件用（medcxy 的叫法） ----
  val paper get() = main
  val fill get() = well
  val separator get() = line
  val red get() = err
  /** 系统红（菜单里的「删除」）：Captae 只有一个红，就用 --err */
  val systemRed get() = err
}
