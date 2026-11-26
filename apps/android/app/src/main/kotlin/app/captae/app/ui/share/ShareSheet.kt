package app.captae.app.ui.share

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import app.captae.app.R
import app.captae.app.core.CaptaeRuntime
import app.captae.app.core.RemoteRuntime
import androidx.compose.runtime.rememberCoroutineScope
import kotlinx.coroutines.launch
import uniffi.captae_mobile.SendOptions
import app.captae.app.core.clock
import app.captae.app.demo.DemoData
import app.captae.app.demo.ShareMode
import app.captae.app.demo.Sheet
import app.captae.app.model.Computer
import app.captae.app.ui.addOptions
import app.captae.app.ui.downloads.FmtChoice
import app.captae.app.ui.downloads.platOf
import app.captae.app.ui.parts.LocalReducedMotion
import app.captae.app.ui.parts.SiteMark
import app.captae.app.ui.parts.StatusLine
import app.captae.app.ui.parts.Str
import app.captae.app.ui.parts.Thumb
import app.captae.app.ui.parts.Tone
import app.captae.app.ui.parts.dotJoin
import app.captae.app.ui.parts.rememberShimmerPhase
import app.captae.app.ui.parts.shimmer
import app.captae.app.ui.parts.str
import app.captae.glass.controls.GroupedRow
import app.captae.glass.controls.PrimaryButton
import app.captae.glass.controls.Radius
import app.captae.glass.controls.Segmented
import app.captae.glass.controls.cardShadow
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.overlay.GlassSheet
import app.captae.glass.overlay.MenuButton
import app.captae.glass.overlay.MenuOrder
import app.captae.glass.overlay.SheetDetent
import app.captae.glass.overlay.SheetHeader
import app.captae.glass.overlay.menuPicker
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospaced
import app.captae.glass.theme.weight
import uniffi.captae_mobile.CancelToken
import uniffi.captae_mobile.MobileException
import uniffi.captae_mobile.MobileProbe

/** 分享卡片上要显示的链接信息（演示数据和读取结果都转成这个） */
private data class LinkInfo(val title: String, val thumb: String?, val site: String, val meta: String, val plat: String?)

/**
 分享卡片（BRIEF 08）：在别的 App 里点分享、选了 Captae 以后弹出来的玻璃底部面板（同窗口的 sheet，只有中档）。标题统一「Captae」（照 iOS 08-*）。
 - 读取中：标题的位置是扫光文字「正在读取视频信息」（真数据时边读边显示，读完换成标题、封面、网站）；
 - 读好了：「电脑 / 手机」（配对过电脑才有）、清晰度、「下载」；
 - 选了电脑、电脑不在线：说明先存着，回到同一个 Wi-Fi 后自动发过去，按钮是「先存着」。
 真数据：sheet.url 是分享进来的链接。读取失败照样能点「下载」（核心自己再读一次，失败的说法按错误码显示在任务卡上）；
 加任务本身失败（链接不完整这类）时卡片下面一行说原因。
 */
@Composable
fun ShareSheet(sheet: Sheet.Share?, mac: Computer?, demo: Boolean, onDismiss: () -> Unit, onDone: () -> Unit) {
  var last by remember { mutableStateOf(sheet) }
  if (sheet != null) last = sheet
  GlassSheet(
    isPresented = sheet != null,
    onDismiss = onDismiss,
    detents = listOf(SheetDetent.Medium),
    header = { SheetHeader(title = str(R.string.share_title), onClose = onDismiss) },
  ) {
    val sh = last ?: return@GlassSheet
    val url = sh.url
    // 真数据：读取信息（换了链接重新读；sheet 收起时停掉）
    var probe by remember(url) { mutableStateOf<MobileProbe?>(null) }
    var probeDone by remember(url) { mutableStateOf(demo || url == null) }
    var error by remember(url) { mutableStateOf<String?>(null) }
    if (!demo && url != null) {
      val token = remember(url) { CancelToken() }
      DisposableEffect(token) { onDispose { token.cancel() } }
      LaunchedEffect(url) {
        try {
          probe = CaptaeRuntime.probe(url, token)
        } catch (_: MobileException.Failed) {
          // 读不到也让人能点下载（交给核心，任务卡上按错误码说清楚）
        } catch (_: Exception) {
        }
        probeDone = true
      }
    }
    val m = when {
      demo -> sh.mode
      !probeDone -> ShareMode.Reading
      else -> ShareMode.Ready
    }
    val scope = rememberCoroutineScope()
    var toMac by rememberSaveable { mutableStateOf(mac != null) }
    val offline = mac != null && toMac && (if (demo) sh.mode == ShareMode.Offline else !mac.online)
    var fmt by rememberSaveable { mutableStateOf(FmtChoice.Best) }
    val info = if (demo) DemoData.bbb.let { LinkInfo(it.title, it.thumb, "YouTube", it.dur, it.plat) } else probe?.let { p ->
      val site = p.site ?: url?.substringAfter("://")?.substringBefore('/')?.removePrefix("www.").orEmpty()
      LinkInfo(p.title ?: url.orEmpty(), p.thumbnail, site, clock(p.duration), platOf(url.orEmpty()))
    } ?: LinkInfo(url.orEmpty(), null, url?.substringAfter("://")?.substringBefore('/')?.removePrefix("www.").orEmpty(), "", platOf(url.orEmpty()))
    Column(
      Modifier
        .fillMaxSize()
        .verticalScroll(rememberScrollState())
        .padding(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 24.dp),
      verticalArrangement = Arrangement.spacedBy(14.dp),
    ) {
      LinkCard(info, url ?: "youtube.com/watch?v=aqz-KE-bpKQ", reading = m == ShareMode.Reading)
      if (mac != null) {
        Segmented(listOf(true, false), toMac, label = { Str.get(if (it) R.string.share_to_mac else R.string.share_to_phone) }, onSelect = { toMac = it }, enabled = m != ShareMode.Reading)
      }
      if (offline) {
        StatusLine(str(R.string.share_offline, mac.name), Tone.Warn, Sym.wifiSlash, Modifier.fillMaxWidth())
      } else {
        Box(Modifier.fillMaxWidth().cardShadow(Radius.lg).clip(RoundedCornerShape(Radius.lg)).background(Palette.card)) {
          MenuButton(
            listOf(menuPicker(FmtChoice.main, fmt, title = { Str.get(it.label) }) { fmt = it }, menuPicker(FmtChoice.more, fmt, title = { Str.get(it.label) }) { fmt = it }),
            modifier = Modifier.fillMaxWidth(),
            enabled = m != ShareMode.Reading,
            contentDescription = str(R.string.share_fmt_a11y),
            order = MenuOrder.Fixed,
          ) {
            GroupedRow(
              str(R.string.share_quality),
              value = str(fmt.label),
              trailing = { Icon(Sym.chevronUpChevronDown, size = 15.dp, weight = FontWeight.Medium, tint = Palette.ink4) },
            )
          }
        }
      }
      if (error != null) {
        Row(horizontalArrangement = Arrangement.spacedBy(6.dp), modifier = Modifier.padding(horizontal = 4.dp)) {
          Icon(Sym.exclamationmarkCircle, size = 14.dp, weight = FontWeight.Medium, tint = Palette.warn, modifier = Modifier.padding(top = 1.dp))
          Text(error!!, style = CaptaeType.footnote, color = Palette.warn)
        }
      }
      PrimaryButton(
        str(if (offline) R.string.share_later else R.string.share_download),
        onClick = {
          if (demo || url == null) { onDismiss(); return@PrimaryButton }
          val o = addOptions(url, fmt, probe?.kind?.let { it == "page" })
          if (mac != null && toMac) {
            // 发到电脑：在线就发，连不上进「等电脑上线」（RemoteHub.send）
            scope.launch {
              try {
                RemoteRuntime.send(mac.id, SendOptions(url = url, quality = o.quality, format = o.format, audioOnly = o.audioOnly, preset = null, title = probe?.title))
                onDone()
              } catch (e: MobileException.Failed) {
                error = e.detail.message
              }
            }
            return@PrimaryButton
          }
          try {
            CaptaeRuntime.add(o)
            onDone()
          } catch (e: MobileException.Failed) {
            error = e.detail.message
          }
        },
        enabled = m != ShareMode.Reading,
        modifier = Modifier.fillMaxWidth(),
      )
    }
  }
}

/** 链接卡：封面小图 + 标题 + 网站；读取中时标题位置是扫光文字、封面是灰底 */
@Composable
private fun LinkCard(info: LinkInfo, url: String, reading: Boolean) {
  Row(
    Modifier
      .fillMaxWidth()
      .cardShadow(Radius.lg)
      .clip(RoundedCornerShape(Radius.lg))
      .background(Palette.card)
      .padding(10.dp),
    horizontalArrangement = Arrangement.spacedBy(12.dp),
    verticalAlignment = Alignment.CenterVertically,
  ) {
    Thumb(if (reading) null else info.thumb, Modifier.width(96.dp).aspectRatio(16f / 9f), dur = if (reading) "" else info.meta, plat = info.plat)
    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(4.dp)) {
      if (reading) {
        // 扫光文字（配方 m）：2.7 秒一次，减少动效时不扫
        Text(str(R.string.job_reading_a11y), style = CaptaeType.subheadline.weight(FontWeight.SemiBold), color = Palette.ink3, maxLines = 1, modifier = Modifier.shimmer(LocalReducedMotion.current, rememberShimmerPhase()))
        Text(url.removePrefix("https://").removePrefix("http://").removePrefix("www."), style = CaptaeType.caption.monospaced(), color = Palette.ink3, maxLines = 1, overflow = TextOverflow.MiddleEllipsis)
      } else {
        Text(info.title, style = CaptaeType.subheadline.weight(FontWeight.SemiBold), maxLines = 2)
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(5.dp)) {
          SiteMark(info.plat, size = 13.dp)
          Text(dotJoin(info.site, info.meta), style = CaptaeType.footnote, color = Palette.ink3, maxLines = 1)
        }
      }
    }
  }
}
