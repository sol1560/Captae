package app.captae.app.ui.nav

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import app.captae.app.R
import app.captae.app.demo.DeviceScope
import app.captae.app.model.Computer
import app.captae.app.ui.parts.dotJoin
import app.captae.app.ui.parts.str
import app.captae.glass.controls.Dot
import app.captae.glass.controls.NavIconSize
import app.captae.glass.core.GlassEffect
import app.captae.glass.core.GlassShapes
import app.captae.glass.core.glassEffect
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.overlay.MenuAction
import app.captae.glass.overlay.MenuButton
import app.captae.glass.overlay.MenuOrder
import app.captae.glass.overlay.MenuSection
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text

// 主页面的顶栏（BRIEF 2.1）：左边「标题即设备菜单」的玻璃胶囊，右边一个圆形「更多」。两块都是浮在内容上的玻璃（第 1 节允许的位置）。

/** 设备的名字：全部设备 / 这台手机 / 电脑名 */
@Composable
fun deviceName(scope: DeviceScope, mac: Computer?): String = when (scope) {
  DeviceScope.All -> str(R.string.device_all)
  DeviceScope.Phone -> str(R.string.device_phone)
  DeviceScope.Mac -> mac?.name ?: str(R.string.device_phone)
}

/** 电脑的状态说明：「在线 · 正在下 3 个」「离线 · 上次在线 今天 14:02」 */
@Composable
fun macStatus(mac: Computer): String =
  if (mac.online) dotJoin(str(R.string.device_online), if (mac.running > 0) str(R.string.device_running, mac.running) else null)
  else dotJoin(str(R.string.device_offline), str(R.string.device_last_seen, mac.lastSeen))

/** 菜单里电脑那一项的小字（BRIEF 2.1）：「在线」/「离线 · 上次在线 今天 14:02」 */
@Composable
fun macMenuStatus(mac: Computer): String =
  if (mac.online) str(R.string.device_online) else dotJoin(str(R.string.device_offline), str(R.string.device_last_seen, mac.lastSeen))

/**
 标题即设备菜单（Gemini「Carbon 高 ⌄」的做法）：玻璃胶囊里 状态点 + 设备名 + ⌄，下面不放第二行（状态写进菜单项的小字）。
 点开是 :glass 的玻璃菜单：每项一行小字说明、选中的打勾；分隔线下面是「添加电脑…」「管理电脑…」。
 状态点只在看的东西里有电脑时画（在线绿、离线灰）；只看这台手机时没有点。
 */
@Composable
fun DeviceTitleMenu(
  scope: DeviceScope,
  macs: List<Computer>,
  /** scope 是 Mac 时选的是哪台 */
  selected: String?,
  onSelect: (DeviceScope, String?) -> Unit,
  onAddComputer: () -> Unit,
  onManageComputers: () -> Unit,
  modifier: Modifier = Modifier,
) {
  // 选中的那台；全部设备时标题前的点看「有没有一台在线」
  val mac = macs.firstOrNull { it.id == selected } ?: macs.firstOrNull()
  val name = deviceName(scope, mac)
  val statuses = macs.associate { it.id to macMenuStatus(it) }
  val devices = buildList {
    if (macs.isNotEmpty()) add(MenuAction(str(R.string.device_all), Sym.squareStack, subtitle = str(R.string.device_count, macs.size + 1), checked = scope == DeviceScope.All) { onSelect(DeviceScope.All, null) })
    add(MenuAction(str(R.string.device_phone), Sym.iphone, checked = scope == DeviceScope.Phone) { onSelect(DeviceScope.Phone, null) })
    for (m in macs) add(MenuAction(m.name, Sym.laptopcomputer, subtitle = statuses.getValue(m.id), checked = scope == DeviceScope.Mac && m.id == mac?.id) { onSelect(DeviceScope.Mac, m.id) })
  }
  val dotOnline = if (scope == DeviceScope.All) macs.any { it.online } else mac?.online == true
  val actions = listOf(
    MenuAction(str(R.string.device_add), Sym.plus, onClick = onAddComputer),
    MenuAction(str(R.string.device_manage), Sym.laptopcomputerAndIphone, onClick = onManageComputers),
  )
  MenuButton(
    listOf(MenuSection(devices), MenuSection(actions)),
    modifier = modifier,
    contentDescription = str(R.string.device_menu_a11y, name),
    order = MenuOrder.Fixed,
  ) {
    Row(
      Modifier
        .heightIn(min = 44.dp)
        .widthIn(max = 264.dp)
        .glassEffect(GlassEffect.Regular.interactive(), GlassShapes.capsule)
        .padding(start = if (scope == DeviceScope.Phone || mac == null) 16.dp else 14.dp, end = 12.dp, top = 6.dp, bottom = 6.dp)
        .clearAndSetSemantics {},
      verticalAlignment = Alignment.CenterVertically,
      horizontalArrangement = Arrangement.spacedBy(7.dp),
    ) {
      if (scope != DeviceScope.Phone && mac != null) Dot(if (dotOnline) Palette.ok else Palette.ink4, size = 8.dp)
      Text(name, style = CaptaeType.headline, maxLines = 1, modifier = Modifier.weight(1f, fill = false))
      Icon(Sym.chevronDown, size = 15.dp, weight = FontWeight.SemiBold, tint = Palette.ink2)
    }
  }
}

/**
 右上角的圆形「更多」（BRIEF 2.1）：全部暂停 / 全部继续、清空已完成；分隔线下面是设置。
 allPaused：已经全部暂停了，第一项换成「全部继续」。
 */
@Composable
fun MoreButton(allPaused: Boolean, onPauseAll: () -> Unit, onResumeAll: () -> Unit, onClearDone: () -> Unit, onSettings: () -> Unit) {
  MenuButton(
    listOf(
      MenuSection(
        listOf(
          if (allPaused) MenuAction(str(R.string.resume_all), Sym.playFill, onClick = onResumeAll) else MenuAction(str(R.string.pause_all), Sym.pauseFill, onClick = onPauseAll),
          MenuAction(str(R.string.clear_done), Sym.checkmarkCircle, onClick = onClearDone),
        ),
      ),
      MenuSection(listOf(MenuAction(str(R.string.settings), Sym.gearshape, onClick = onSettings))),
    ),
    modifier = Modifier.size(44.dp).glassEffect(GlassEffect.Regular.interactive(), GlassShapes.circle),
    contentDescription = str(R.string.more),
  ) {
    Icon(Sym.ellipsis, size = NavIconSize)
  }
}
