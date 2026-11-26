package app.captae.app.ui.parts

import android.content.res.Resources
import androidx.annotation.StringRes
import androidx.compose.runtime.Composable
import androidx.compose.runtime.ReadOnlyComposable
import androidx.compose.ui.res.stringResource

// 给人看的字从 res/values/strings.xml 取（以后加多语言）。界面里用 str()；不在界面里的逻辑（任务状态、出错说明、步骤）用 Str。

/** 名字占位（%1$s）前后：名字是英文、数字开头或结尾，紧挨着的是汉字时加一个空格（「登录 YouTube」「发到 Sol 的 MacBook Pro」「登录哔哩哔哩」） */
private val SPEC = Regex("%(\\d+)\\$([sd])|%%")

private fun cjkChar(c: Char?) = c != null && c in '一'..'鿿'
private fun asciiChar(c: Char?) = c != null && (c in 'A'..'Z' || c in 'a'..'z' || c in '0'..'9')

/** 照模板填值；%s 的名字自动处理中英文之间的空格（桌面端同一条规矩，BRIEF 第 1 节「文案」） */
fun cjkFormat(template: String, vararg args: Any): String {
  if (args.isEmpty()) return template
  val sb = StringBuilder()
  var last = 0
  for (m in SPEC.findAll(template)) {
    sb.append(template, last, m.range.first)
    if (m.value == "%%") {
      sb.append('%')
    } else {
      val v = args[m.groupValues[1].toInt() - 1].toString()
      val name = m.groupValues[2] == "s"
      if (name && cjkChar(template.getOrNull(m.range.first - 1)) && asciiChar(v.firstOrNull())) sb.append(' ')
      sb.append(v)
      if (name && cjkChar(template.getOrNull(m.range.last + 1)) && asciiChar(v.lastOrNull())) sb.append(' ')
    }
    last = m.range.last + 1
  }
  sb.append(template, last, template.length)
  return sb.toString()
}

/** 界面里取字（跟着系统语言、配置变了会重组） */
@Composable
@ReadOnlyComposable
fun str(@StringRes id: Int, vararg args: Any): String = cjkFormat(stringResource(id), *args)

/**
 不在界面里的逻辑取字。MainActivity 一启动就设好 res（配置变了 Activity 重建，跟着换）。
 */
object Str {
  lateinit var res: Resources
  /** 时间格式（12 / 24 小时制）要读系统设置 */
  lateinit var ctx: android.content.Context

  fun get(@StringRes id: Int): String = res.getString(id)

  fun fmt(@StringRes id: Int, vararg args: Any): String = cjkFormat(res.getString(id), *args)
}
