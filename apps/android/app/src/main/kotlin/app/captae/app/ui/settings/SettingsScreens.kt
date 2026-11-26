package app.captae.app.ui.settings

import androidx.annotation.StringRes
import app.captae.app.R
import app.captae.app.core.CaptaeRuntime
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.rememberUpdatedState
import app.captae.glass.icons.Icon
import app.captae.glass.overlay.MenuButton
import app.captae.glass.overlay.MenuOrder
import app.captae.glass.overlay.menuPicker
import app.captae.app.ui.parts.str
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import app.captae.app.BuildConfig
import app.captae.app.ui.parts.CaptaeLogo
import app.captae.glass.controls.GroupedDivider
import app.captae.glass.controls.GroupedRow
import app.captae.glass.controls.GroupedSection
import app.captae.glass.controls.RadioDot
import app.captae.glass.controls.Radius
import app.captae.glass.icons.Sym
import app.captae.glass.nav.BackButton
import app.captae.glass.nav.GlassScaffold
import app.captae.glass.nav.TitleMode
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospaced

// 设置（BRIEF 07，桌面端 screenshots/settings.png）：白卡分组，一行一项。推进来的页，左上返回。

/** 一页设置的外壳：居中小标题 + 返回 + 可以滚的分组 */
@Composable
private fun SettingsPage(title: String, onBack: () -> Unit, content: @Composable ColumnScope.() -> Unit) {
  GlassScaffold(title = title, titleMode = TitleMode.Inline, navigation = { BackButton(onBack) }) { padding ->
    Column(
      Modifier
        .fillMaxSize()
        .verticalScroll(rememberScrollState())
        .padding(top = padding.calculateTopPadding() + 16.dp, bottom = padding.calculateBottomPadding() + 24.dp),
      verticalArrangement = Arrangement.spacedBy(24.dp),
      content = content,
    )
  }
}

/** 默认清晰度的几档（协议值，不翻译；「最高可用」显示成「最清晰」） */
private val QUALITIES = listOf("最高可用", "2160p", "1080p", "720p", "480p")
/** 网页链接的默认格式（协议值） */
private val FORMATS = listOf("MP4", "WEBM", "保持原格式")

@Composable
private fun qualityLabel(q: String) = if (q == "最高可用") str(R.string.variant_best) else q

@Composable
private fun formatLabel(f: String) = if (f == "保持原格式") str(R.string.fmt_keep) else f

/** 一行设置，点了从这一行长出菜单（iOS Form 里的菜单式 Picker） */
@Composable
private fun <T> PickerRow(title: String, value: String, options: List<T>, selected: T, label: (T) -> String, enabled: Boolean, onSelect: (T) -> Unit) {
  MenuButton(listOf(menuPicker(options, selected, title = label) { onSelect(it) }), modifier = Modifier.fillMaxWidth(), enabled = enabled, contentDescription = title, order = MenuOrder.Fixed) {
    GroupedRow(title, value = value, trailing = { Icon(Sym.chevronUpChevronDown, size = 15.dp, weight = FontWeight.Medium, tint = Palette.ink4) })
  }
}

/**
 07-main。真数据时清晰度、格式、同时下载读写核心的设置（[CaptaeRuntime.saveSettings]）；demo 时是设计稿的固定值。
 pairedCount：配对过几台电脑（远程控制接上以前是 0）。
 */
@Composable
fun SettingsScreen(onBack: () -> Unit, onProxy: () -> Unit, onAbout: () -> Unit, onComputers: () -> Unit, proxyManual: Boolean, demo: Boolean = true, pairedCount: Int = 1) {
  val live by CaptaeRuntime.settings.collectAsState()
  val st = if (demo) null else live
  val quality = st?.defaultQuality ?: "最高可用"
  val format = st?.defaultFormat ?: "MP4"
  val conc = st?.concurrency?.toInt() ?: 3
  val manual = if (st != null) st.proxyMode == "manual" && st.proxyUrl.isNotBlank() else proxyManual
  val qLabels = QUALITIES.associateWith { qualityLabel(it) }
  val fLabels = FORMATS.associateWith { formatLabel(it) }
  val cLabels = (1..5).associateWith { str(R.string.set_concurrency_value, it) }
  SettingsPage(str(R.string.settings), onBack) {
    GroupedSection(header = str(R.string.set_downloads), footer = str(R.string.set_downloads_footer)) {
      GroupedRow(str(R.string.set_save_to), value = str(R.string.set_save_to_value))
      GroupedDivider()
      PickerRow(str(R.string.set_quality), qLabels[quality] ?: quality, QUALITIES, quality, { qLabels.getValue(it) }, enabled = st != null) { q ->
        st?.let { CaptaeRuntime.saveSettings(it.copy(defaultQuality = q)) }
      }
      GroupedDivider()
      PickerRow(str(R.string.set_format), fLabels[format] ?: format, FORMATS, format, { fLabels.getValue(it) }, enabled = st != null) { f ->
        st?.let { CaptaeRuntime.saveSettings(it.copy(defaultFormat = f)) }
      }
      GroupedDivider()
      PickerRow(str(R.string.set_concurrency), str(R.string.set_concurrency_value, conc), (1..5).toList(), conc, { cLabels.getValue(it) }, enabled = st != null) { n ->
        st?.let { CaptaeRuntime.saveSettings(it.copy(concurrency = n.toUInt())) }
      }
    }
    GroupedSection(header = str(R.string.set_computers)) {
      GroupedRow(str(R.string.set_paired), value = if (pairedCount > 0) str(R.string.set_paired_value, pairedCount) else str(R.string.set_paired_none), chevron = true, onClick = onComputers)
    }
    GroupedSection(header = str(R.string.set_network)) {
      GroupedRow(str(R.string.set_proxy), value = str(if (manual) R.string.set_proxy_manual else R.string.set_proxy_off), chevron = true, onClick = onProxy)
    }
    GroupedSection(header = str(R.string.set_about), footer = if (!demo && !CaptaeRuntime.engineInstalled()) str(R.string.set_engine_missing_footer) else null) {
      GroupedRow(str(R.string.set_version), value = BuildConfig.VERSION_NAME)
      GroupedDivider()
      // 安卓是完整版：手机上也有下载引擎（PLAN 1「完整版加上 yt-dlp 版本」）。引擎打包（:engine）接上以前写「还没装上」
      GroupedRow(str(R.string.set_engine), value = if (demo) "yt-dlp 2026.11.18" else CaptaeRuntime.engine?.let { "yt-dlp ${it.ytdlp}" } ?: if (CaptaeRuntime.engineInstalled()) "yt-dlp" else str(R.string.set_engine_missing))
      GroupedDivider()
      GroupedRow(str(R.string.set_licenses), chevron = true, onClick = onAbout)
      GroupedDivider()
      GroupedRow(str(R.string.set_source), value = "GitHub", chevron = true, onClick = {})
    }
  }
}

/** 07-proxy：不用代理 / 手动填写（手机上没有「跟随系统」：安卓的系统代理 App 读不到也用不上，PLAN 1 只做手动）。离开这页时存 */
@Composable
fun ProxyScreen(onBack: () -> Unit, initialManual: Boolean, demo: Boolean = true) {
  val st = if (demo) null else CaptaeRuntime.settings.value
  var manual by rememberSaveable { mutableStateOf(st?.let { it.proxyMode == "manual" } ?: initialManual) }
  var addr by rememberSaveable { mutableStateOf(st?.proxyUrl ?: if (initialManual) "127.0.0.1:7890" else "") }
  val addrLabel = str(R.string.proxy_addr)
  if (st != null) {
    // 离开这页（返回、切标签）时存：手动但没填地址就当不用
    val latest by rememberUpdatedState(manual to addr)
    DisposableEffect(Unit) {
      onDispose {
        val (m, a) = latest
        val cur = CaptaeRuntime.settings.value ?: return@onDispose
        val mode = if (m && a.isNotBlank()) "manual" else "none"
        if (cur.proxyMode != mode || cur.proxyUrl != a.trim()) CaptaeRuntime.saveSettings(cur.copy(proxyMode = mode, proxyUrl = a.trim()))
      }
    }
  }
  SettingsPage(str(R.string.set_proxy), onBack) {
    GroupedSection(footer = str(if (manual) R.string.proxy_manual_footer else R.string.proxy_none_footer)) {
      ChoiceRow(str(R.string.proxy_none), selected = !manual) { manual = false }
      GroupedDivider(52.dp)
      ChoiceRow(str(R.string.set_proxy_manual), selected = manual) { manual = true }
    }
    if (manual) {
      GroupedSection(header = str(R.string.proxy_addr), footer = str(R.string.proxy_addr_footer)) {
        Row(Modifier.fillMaxWidth().heightIn(min = 52.dp).padding(horizontal = 16.dp), verticalAlignment = Alignment.CenterVertically) {
          BasicTextField(
            value = addr,
            onValueChange = { addr = it },
            singleLine = true,
            textStyle = CaptaeType.body.monospaced().copy(color = Palette.ink),
            cursorBrush = SolidColor(Palette.accent),
            keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Uri),
            modifier = Modifier.fillMaxWidth().semantics { contentDescription = addrLabel },
            decorationBox = { inner ->
              Box(contentAlignment = Alignment.CenterStart) {
                if (addr.isEmpty()) Text("127.0.0.1:7890", style = CaptaeType.body.monospaced(), color = Palette.ink4)
                inner()
              }
            },
          )
        }
      }
    }
  }
}

/** 单选的一行：左边单选点（选中的是强调色），右边名字 */
@Composable
private fun ChoiceRow(title: String, selected: Boolean, onClick: () -> Unit) {
  Row(
    Modifier.fillMaxWidth().heightIn(min = 52.dp).clickable(role = Role.RadioButton, onClick = onClick).padding(horizontal = 16.dp),
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(14.dp),
  ) {
    RadioDot(selected)
    Text(title, style = CaptaeType.body)
  }
}

/** 一个开源组件：名字、许可证、用在哪 */
private data class License(val name: String, val license: String, @StringRes val use: Int)

private val LICENSES = listOf(
  License("Captae", "AGPL-3.0", R.string.lic_use_app),
  License("yt-dlp", "Unlicense", R.string.lic_use_ytdlp),
  License("FFmpeg", "LGPL 2.1", R.string.lic_use_ffmpeg),
  License("Python", "PSF", R.string.lic_use_python),
  License("QuickJS", "MIT", R.string.lic_use_quickjs),
  License("backdrop, shapes", "Apache 2.0", R.string.lic_use_backdrop),
  License("AndroidLiquidGlass", "Apache 2.0", R.string.lic_use_liquidglass),
  License("Jetpack Compose", "Apache 2.0", R.string.lic_use_compose),
  License("Geist, Geist Mono", "OFL 1.1", R.string.lic_use_fonts),
  License("Hugeicons Free", "MIT", R.string.lic_use_icons),
  License("Simple Icons", "CC0", R.string.lic_use_sites),
)

/** 07-about：logo、版本、许可证列表 */
@Composable
fun AboutScreen(onBack: () -> Unit) {
  SettingsPage(str(R.string.set_about), onBack) {
    Column(Modifier.fillMaxWidth().padding(top = 8.dp), horizontalAlignment = Alignment.CenterHorizontally) {
      CaptaeLogo(size = 72.dp)
      Spacer(Modifier.height(14.dp))
      Text("Captae", style = CaptaeType.title2.copy(fontWeight = FontWeight.Bold))
      Spacer(Modifier.height(4.dp))
      Text(str(R.string.about_version, BuildConfig.VERSION_NAME), style = CaptaeType.subheadline, color = Palette.ink3)
      Spacer(Modifier.height(10.dp))
      Text(str(R.string.about_tagline), style = CaptaeType.footnote, color = Palette.ink3, textAlign = TextAlign.Center)
    }
    GroupedSection {
      GroupedRow(str(R.string.set_source), sf = Sym.chevronLeftForwardslashChevronRight, value = "GitHub", chevron = true, onClick = {})
    }
    GroupedSection(header = str(R.string.set_licenses), footer = str(R.string.about_stills_footer)) {
      LICENSES.forEachIndexed { i, l ->
        if (i > 0) GroupedDivider()
        GroupedRow(l.name, subtitle = str(l.use), value = l.license, chevron = true, onClick = {})
      }
    }
  }
}
