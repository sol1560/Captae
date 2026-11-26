package app.captae.app.ui.computer

import androidx.compose.foundation.Image
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import app.captae.app.R
import app.captae.app.core.CaptaeRuntime
import app.captae.app.core.RemoteRuntime
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import app.captae.glass.overlay.DialogAction
import app.captae.glass.overlay.GlassAlert
import app.captae.glass.overlay.AlertTextField
import uniffi.captae_mobile.NearbyComputer
import androidx.compose.foundation.border
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import app.captae.app.demo.DemoData
import app.captae.app.model.Computer
import app.captae.app.ui.parts.dotJoin
import app.captae.glass.controls.Dot
import app.captae.glass.controls.GroupedDivider
import app.captae.glass.controls.GroupedRow
import app.captae.glass.controls.GroupedSection
import app.captae.glass.controls.PrimaryButton
import app.captae.glass.controls.Radius
import app.captae.glass.controls.SecondaryButton
import app.captae.glass.controls.cardShadow
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.overlay.GlassSheet
import app.captae.glass.overlay.SheetDetent
import app.captae.glass.overlay.SheetHeader
import app.captae.app.ui.parts.str
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospaced
import app.captae.glass.theme.monospacedDigit
import app.captae.glass.theme.weight

/**
 管理电脑（BRIEF 2.1，第二版：原来的「电脑」标签挪进来）：同一窗口里的玻璃 sheet，中档打开、能拉到全高，左上角关闭。
 从设备菜单的「添加电脑…」「管理电脑…」和设置里的「已配对的电脑」进来。
 - mac 为空（05-none）：怎么配对（桌面端设置的截图 + 三步）、「扫码配对」、附近的电脑；
 - 在线（05-online）：电脑名、在线绿点、Captae 版本、正在下几个；「添加电脑」；
 - 离线（05-offline）：灰点、上次在线；「等电脑上线」的任务，回到同一个 Wi-Fi 后自动发过去。
 */
@Composable
fun ComputersSheet(
  visible: Boolean,
  /** 配对过的每一台（按最近用过排） */
  macs: List<Computer>,
  demo: Boolean = true,
  onDismiss: () -> Unit,
  onScan: () -> Unit,
  onCode: (NearbyComputer?) -> Unit,
  onRemove: (String) -> Unit = {},
  onRename: (String, String) -> Unit = { _, _ -> },
) {
  // 真数据：附近发现的电脑（NSD → RemoteHub.nearby）、等电脑上线的链接（RemoteHub.pending）
  val nearbyLive by RemoteRuntime.nearby.collectAsState()
  val pendingLive by RemoteRuntime.pending.collectAsState()
  val nearby: List<Pair<String, NearbyComputer?>> = if (demo) listOf(DemoData.mac.name to null) else nearbyLive.map { it.name to it }
  val short = { u: String -> u.removePrefix("https://").removePrefix("http://").removePrefix("www.") }
  val waiting = if (demo) DemoData.waitingForMac else pendingLive.map { (it.title ?: short(it.url)) to short(it.url) }
  // 点了哪一台：改个在手机上显示的名字（几台电脑同名时分得清；空 = 用电脑自己报的名字，REMOTE.md 第 4 节），或者删掉它
  var confirmRemove by remember { mutableStateOf<Computer?>(null) }
  confirmRemove?.let { mac ->
    var name by remember(mac.id) { mutableStateOf(mac.name) }
    GlassAlert(
      isPresented = true,
      onDismiss = { confirmRemove = null },
      title = mac.name,
      message = str(R.string.computers_remove_text),
      textField = AlertTextField(name, { name = it }, str(R.string.computers_name_placeholder)),
      buttons = listOf(
        DialogAction(str(R.string.computers_rename)) { confirmRemove = null; onRename(mac.id, name.trim()) },
        DialogAction(str(R.string.computers_remove), DialogAction.Role.Destructive) { confirmRemove = null; onRemove(mac.id) },
        DialogAction(str(R.string.job_cancel), DialogAction.Role.Cancel) { confirmRemove = null },
      ),
    )
  }
  GlassSheet(
    isPresented = visible,
    onDismiss = onDismiss,
    detents = listOf(SheetDetent.Medium, SheetDetent.Large),
    header = { SheetHeader(title = str(R.string.computers_title), onClose = onDismiss) },
  ) {
    Column(
      Modifier
        .fillMaxSize()
        .verticalScroll(rememberScrollState())
        .padding(top = 4.dp, bottom = 32.dp),
      verticalArrangement = Arrangement.spacedBy(20.dp),
    ) {
      // 「都改成在这台手机上下」：RemoteHub.moveToPhone（空 = 全部）调本机的 add
      val allToPhone = { if (!demo) RemoteRuntime.hub?.moveToPhone(emptyList()) }
      if (macs.isEmpty()) NotPaired(nearby, onScan, onCode) else Paired(macs, waiting, onScan, onAllToPhone = { allToPhone() }, onOpen = { if (!demo) confirmRemove = it })
    }
  }
}

/**
 没配对（05-none，照 iOS）：一张白卡里是桌面端设置的截图、「和电脑配对」、一句说明、三步、「扫码配对」；
 下面「附近的电脑 · 发现到 1 台」，每台一行（点了输 6 位数）。
 */
@Composable
private fun NotPaired(nearby: List<Pair<String, NearbyComputer?>>, onScan: () -> Unit, onCode: (NearbyComputer?) -> Unit) {
  Column(
    Modifier.padding(horizontal = 16.dp).fillMaxWidth().cardShadow(Radius.xl).clip(RoundedCornerShape(Radius.xl)).background(Palette.card).padding(16.dp),
    verticalArrangement = Arrangement.spacedBy(16.dp),
  ) {
    // 插画：桌面端设置的真实画面（screenshots/settings.png 裁的一块），不画装饰图
    Image(
      painterResource(R.drawable.illus_desktop_settings),
      contentDescription = str(R.string.computers_illus_a11y),
      contentScale = ContentScale.FillWidth,
      modifier = Modifier.fillMaxWidth().wrapContentHeight().clip(RoundedCornerShape(Radius.md)).border(0.5.dp, Palette.line, RoundedCornerShape(Radius.md)),
    )
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
      Text(str(R.string.computers_connect), style = CaptaeType.title3)
      Text(str(R.string.computers_connect_sub), style = CaptaeType.subheadline, color = Palette.ink3)
    }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
      NumberedStep(1, str(R.string.computers_step1))
      NumberedStep(2, str(R.string.computers_step2))
      NumberedStep(3, str(R.string.computers_step3))
    }
    PrimaryButton(str(R.string.computers_scan), onClick = onScan, sf = Sym.qrcodeViewfinder, modifier = Modifier.fillMaxWidth())
  }
  GroupedSection(header = dotJoin(str(R.string.computers_nearby), if (nearby.isEmpty()) str(R.string.computers_nearby_searching) else str(R.string.computers_nearby_found, nearby.size))) {
    if (nearby.isEmpty()) {
      // 还没发现：说一句为什么可能找不到，还能直接输 6 位数
      ComputerRow(name = str(R.string.computers_nearby_none), sub = str(R.string.computers_nearby_footer), online = null, onClick = { onCode(null) })
    } else {
      nearby.forEachIndexed { i, (name, n) ->
        if (i > 0) GroupedDivider(72.dp)
        ComputerRow(name = name, sub = str(R.string.computers_enter_code), online = null, onClick = { onCode(n) })
      }
    }
  }
}

/** 编号的一步：灰色小圆里一个数字 + 一句话 */
@Composable
private fun NumberedStep(n: Int, text: String) {
  Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
    Box(Modifier.size(22.dp).clip(CircleShape).background(Palette.select), contentAlignment = Alignment.Center) {
      Text("$n", style = CaptaeType.footnote.weight(FontWeight.SemiBold).monospacedDigit(), color = Palette.ink2)
    }
    Text(text, style = CaptaeType.subheadline, modifier = Modifier.weight(1f).padding(top = 1.dp))
  }
}

@Composable
private fun Paired(macs: List<Computer>, waiting: List<Pair<String, String>>, onScan: () -> Unit, onAllToPhone: () -> Unit, onOpen: (Computer) -> Unit) {
  GroupedSection(header = str(R.string.computers_paired)) {
    macs.forEachIndexed { i, mac ->
      if (i > 0) GroupedDivider(72.dp)
      val sub = if (mac.online) dotJoin(str(R.string.device_online), mac.version.takeIf { it.isNotEmpty() }?.let { str(R.string.computers_version, it) }, if (mac.running > 0) str(R.string.device_running, mac.running) else null)
      else dotJoin(str(R.string.device_offline), str(R.string.device_last_seen, mac.lastSeen))
      // 离线时多一行怎么办（BRIEF 2.8 第 5 条）
      // 被电脑撤销了 / 令牌没了：一行说要重新配对（REMOTE.md 第 10 节 unauthorized / needs_pairing）
      val hint = when {
        mac.needsPairing -> str(R.string.computers_needs_pairing)
        mac.online -> null
        else -> str(R.string.mac_unreachable_hint)
      }
      ComputerRow(mac.name, sub, online = mac.online, hint = hint, onClick = { onOpen(mac) })
    }
  }
  if (waiting.isNotEmpty()) Waiting(waiting, onAllToPhone)
  GroupedSection(footer = str(R.string.computers_lan_footer, phoneName())) {
    GroupedRow(str(R.string.computers_add), sf = Sym.plus, onClick = onScan)
  }
}

/** 「等电脑上线」（BRIEF 2.8 第 5 条）：每条一行（标题 + 链接），页脚说明，「都改成在这台手机上下」 */
@Composable
private fun Waiting(waiting: List<Pair<String, String>>, onAllToPhone: () -> Unit) {
  GroupedSection(header = dotJoin(str(R.string.computers_waiting), str(R.string.computers_waiting_count, waiting.size)), footer = str(R.string.computers_waiting_footer)) {
    waiting.forEachIndexed { i, (title, url) ->
      if (i > 0) GroupedDivider(54.dp)
      Row(
        Modifier.fillMaxWidth().heightIn(min = 60.dp).padding(horizontal = 16.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(12.dp),
      ) {
        Icon(Sym.clock, size = 22.dp, tint = Palette.ink3)
        Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
          Text(title, style = CaptaeType.body, maxLines = 2)
          Text(url, style = CaptaeType.caption.monospaced(), color = Palette.ink3, maxLines = 1)
        }
      }
    }
  }
  SecondaryButton(str(R.string.computers_all_to_phone), onClick = onAllToPhone, sf = Sym.iphone, modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp))
}

/** 一台电脑：灰色方块里一个笔记本图标、名字、状态（在线绿点 / 离线灰点）、离线时再一行怎么办、右边小箭头 */
@Composable
private fun ComputerRow(name: String, sub: String, online: Boolean?, onClick: () -> Unit, hint: String? = null) {
  Row(
    Modifier
      .fillMaxWidth()
      .heightIn(min = 68.dp)
      .clickable(role = Role.Button, onClick = onClick)
      .padding(horizontal = 16.dp, vertical = 12.dp),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(12.dp),
  ) {
    Box(Modifier.size(44.dp).clip(RoundedCornerShape(Radius.md)).background(Palette.well), contentAlignment = Alignment.Center) {
      Icon(Sym.laptopcomputer, size = 26.dp, tint = Palette.ink)
    }
    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(3.dp)) {
      Text(name, style = CaptaeType.headline, maxLines = 2)
      Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        if (online != null) Dot(if (online) Palette.ok else Palette.ink4, size = 7.dp)
        Text(sub, style = CaptaeType.footnote.monospacedDigit(), color = Palette.ink3)
      }
      if (hint != null) Text(hint, style = CaptaeType.footnote, color = Palette.ink3)
    }
    Icon(Sym.chevronRight, size = 18.dp, weight = FontWeight.SemiBold, tint = Palette.ink4)
  }
}

/** 这台手机在电脑上显示的名字（系统设置里的设备名，没有就用型号） */
@Composable
private fun phoneName(): String {
  val ctx = androidx.compose.ui.platform.LocalContext.current
  return androidx.compose.runtime.remember {
    android.provider.Settings.Global.getString(ctx.contentResolver, android.provider.Settings.Global.DEVICE_NAME)?.takeIf { it.isNotBlank() } ?: android.os.Build.MODEL
  }
}
