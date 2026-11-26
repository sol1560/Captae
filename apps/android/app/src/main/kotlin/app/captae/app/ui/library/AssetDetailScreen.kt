@file:OptIn(ExperimentalLayoutApi::class)

package app.captae.app.ui.library

import app.captae.app.R
import app.captae.app.ui.LocalAssetActions
import app.captae.app.ui.parts.rememberCover
import androidx.compose.runtime.remember
import androidx.compose.ui.graphics.painter.BitmapPainter
import app.captae.app.ui.parts.str
import androidx.compose.foundation.background
import android.os.Build
import androidx.compose.foundation.Image
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.RowScope
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.wrapContentHeight
import androidx.compose.ui.draw.BlurredEdgeTreatment
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import app.captae.app.ui.parts.stillRes
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import app.captae.app.model.Asset
import app.captae.app.ui.parts.Thumb
import app.captae.app.ui.parts.WaveformCover
import app.captae.glass.controls.GroupedSection
import app.captae.glass.controls.PrimaryButton
import app.captae.glass.controls.ProgressBar
import app.captae.glass.controls.Radius
import app.captae.glass.controls.SecondaryButton
import app.captae.glass.controls.cardShadow
import app.captae.glass.icons.Icon
import app.captae.glass.icons.Sym
import app.captae.glass.nav.BackButton
import app.captae.glass.nav.GlassScaffold
import app.captae.glass.nav.TitleMode
import app.captae.glass.nav.rememberScrollPosition
import app.captae.glass.theme.CaptaeType
import app.captae.glass.theme.Palette
import app.captae.glass.theme.Text
import app.captae.glass.theme.monospaced
import app.captae.glass.theme.monospacedDigit
import app.captae.glass.theme.weight

/**
 素材详情（BRIEF 04，桌面端素材库右栏的手机版）：推进来的一页。版式照 iOS（BRIEF 2.8 第 8 条）：
 - 封面贴到顶：16:9 封面铺满宽，从导航栏下沿开始；导航栏和状态栏那一段是封面顶边往上翻过来、模糊的延伸
   （iOS `backgroundExtensionEffect()` 的做法，颜色来自封面本身，BRIEF 2.6）；返回是左上的圆形玻璃按钮。
   播放时换成系统播放器，画面上不放玻璃（播放钮是半透明深色圆，不是玻璃）。音频没有封面：波形占位，不加延伸。
 - 标题、简介；操作：
   - remote = false：手机上的，一排四个方块：分享 / 发到电脑 / 在「文件」中查看 / 删除（删除红字）；
   - remote = true：电脑上的，「存到手机」（主按钮）+ 分享链接；saving 不为空时主按钮变成进度。
 - 信息、标签。
 静止时顶部不做滚动边缘（会把封面的延伸糊掉），往上滚开始才渐渐出来（BRIEF 2.7）。
 */
@Composable
fun AssetDetailScreen(asset: Asset, remote: Boolean, saving: Float?, macName: String?, onBack: () -> Unit, saved: Boolean = false) {
  val scroll = rememberScrollState()
  val density = LocalDensity.current
  val edgeIn = with(density) { 60.dp.toPx() }
  val act = LocalAssetActions.current
  GlassScaffold(
    title = asset.title,
    titleMode = TitleMode.Large,
    scroll = rememberScrollPosition(scroll),
    navigation = { BackButton(onBack) },
    topEdgeAlpha = { (scroll.value / edgeIn).coerceIn(0f, 1f) },
  ) { padding ->
    Column(
      Modifier
        .fillMaxSize()
        .verticalScroll(scroll)
        .padding(bottom = padding.calculateBottomPadding() + 24.dp),
      verticalArrangement = Arrangement.spacedBy(18.dp),
    ) {
      // 导航栏那一段（状态栏 + 按钮行）和下面的封面连在一起，中间不留缝
      Column {
        val top = padding.calculateTopPadding()
        if (asset.kind == "audio") Spacer(Modifier.height(top)) else CoverExtension(asset.thumb, top)
        Preview(asset, onPlay = { act.open(asset) })
      }
      Column(Modifier.padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
        Text(asset.title, style = CaptaeType.title3)
        if (asset.desc.isNotEmpty()) Text(asset.desc, style = CaptaeType.subheadline, color = Palette.ink2)
      }
      if (remote) RemoteActions(asset, saving, saved, Modifier.padding(horizontal = 16.dp)) else LocalActions(asset, Modifier.padding(horizontal = 16.dp))
      Info(asset)
      if (asset.tags.isNotEmpty()) Tags(asset.tags)
    }
  }
}

/**
 封面往上的延伸（iOS `backgroundExtensionEffect()`）：把封面上下翻过来、底边对着封面的顶边，再模糊，铺满导航栏和状态栏那一段。
 这样接缝两边是同一排像素，往上越来越糊。安卓 12 以下没有模糊，只铺翻过来的图再盖一层淡淡的底色。
 */
@Composable
private fun CoverExtension(thumb: String?, height: Dp) {
  val res = stillRes(thumb)
  val cover = rememberCover(if (res == null) thumb else null).image
  Box(
    Modifier
      .fillMaxWidth()
      .height(height)
      .clipToBounds()
      .then(if (Build.VERSION.SDK_INT >= 31) Modifier.blur(24.dp, BlurredEdgeTreatment.Rectangle) else Modifier),
  ) {
    val painter = when {
      res != null -> painterResource(res)
      cover != null -> remember(cover) { BitmapPainter(cover) }
      else -> null
    }
    if (painter != null) {
      Image(
        painter,
        contentDescription = null,
        contentScale = ContentScale.Crop,
        modifier = Modifier
          .align(Alignment.BottomCenter)
          .fillMaxWidth()
          .wrapContentHeight(align = Alignment.Bottom, unbounded = true)
          .aspectRatio(16f / 9f)
          .graphicsLayer { scaleY = -1f },
      )
    }
    if (Build.VERSION.SDK_INT < 31) Box(Modifier.matchParentSize().background(Palette.paper.copy(alpha = 0.35f)))
  }
}

/** 预览：铺满宽的 16:9 封面（不圆角、不投影，贴着上面的延伸）+ 正中半透明深色圆里一个白色三角 + 时长角标 */
@Composable
private fun Preview(asset: Asset, onPlay: () -> Unit) {
  val playLabel = str(R.string.asset_play)
  val m = Modifier.fillMaxWidth().aspectRatio(16f / 9f)
  if (asset.kind == "audio") {
    WaveformCover(m.clickable(role = Role.Button, onClick = onPlay).semantics { contentDescription = playLabel }, dur = asset.dur, corner = 0.dp)
    return
  }
  Thumb(asset.thumb, m, dur = asset.dur, plat = asset.plat, corner = 0.dp) {
    Box(
      Modifier
        .align(Alignment.Center)
        .size(56.dp)
        .clip(CircleShape)
        .background(Color.Black.copy(alpha = 0.32f))
        .clickable(role = Role.Button, onClick = onPlay)
        .semantics { contentDescription = playLabel },
      contentAlignment = Alignment.Center,
    ) {
      Icon(Sym.playFill, size = 24.dp, tint = Color.White, modifier = Modifier.padding(start = 3.dp))
    }
  }
}

/** 手机上的素材：一排四个方块（白卡、图标在上、字在下；字长时自己换行，四块一样高） */
@Composable
private fun LocalActions(asset: Asset, modifier: Modifier = Modifier) {
  val act = LocalAssetActions.current
  Row(modifier.fillMaxWidth().height(IntrinsicSize.Min), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
    ActionTile(str(R.string.ctx_share), Sym.squareAndArrowUp) { act.share(asset) }
    ActionTile(str(R.string.ctx_send_mac), Sym.squareAndArrowDown) { act.transfer(asset, false) }
    ActionTile(str(R.string.ctx_show_files), Sym.folder) { act.showFiles(asset) }
    ActionTile(str(R.string.ctx_delete), Sym.trash, danger = true) { act.delete(asset) }
  }
}

@Composable
private fun RowScope.ActionTile(label: String, sf: String, danger: Boolean = false, onClick: () -> Unit = {}) {
  val c = if (danger) Palette.err else Palette.ink
  Column(
    Modifier
      .weight(1f)
      .fillMaxHeight()
      .heightIn(min = 76.dp)
      .cardShadow(Radius.lg)
      .clip(RoundedCornerShape(Radius.lg))
      .background(Palette.card)
      .clickable(role = Role.Button, onClick = onClick)
      .padding(horizontal = 4.dp, vertical = 12.dp),
    horizontalAlignment = Alignment.CenterHorizontally,
    verticalArrangement = Arrangement.spacedBy(6.dp, Alignment.CenterVertically),
  ) {
    Icon(sf, size = 24.dp, tint = c)
    Text(label, style = CaptaeType.footnote, color = if (danger) Palette.err else Palette.ink2, textAlign = TextAlign.Center)
  }
}

/** 电脑上的素材：「存到手机」主按钮（存的时候变成进度）+ 分享链接 */
@Composable
private fun RemoteActions(asset: Asset, saving: Float?, saved: Boolean, modifier: Modifier = Modifier) {
  val act = LocalAssetActions.current
  Column(modifier.fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(10.dp)) {
    when {
      // 存到手机：从电脑下载（RemoteHub.saveToPhone，断点续传），下完存进相册 / 音乐
      saved -> SecondaryButton(str(R.string.asset_saved_phone), onClick = {}, sf = Sym.checkmarkCircle, modifier = Modifier.fillMaxWidth())
      saving == null -> PrimaryButton(str(R.string.ctx_save_phone), onClick = { act.transfer(asset, true) }, sf = Sym.squareAndArrowDown, modifier = Modifier.fillMaxWidth())
      else -> SavingProgress(saving, onCancel = { act.cancelSave(asset) })
    }
    SecondaryButton(str(R.string.asset_share_link), onClick = {}, sf = Sym.link, modifier = Modifier.fillMaxWidth())
  }
}

/** 正在存到手机：主按钮的位置换成一条浅灰胶囊，里面一句话、百分比、进度条，右边「取消」 */
@Composable
private fun SavingProgress(p: Float, onCancel: () -> Unit = {}) {
  val savingLabel = str(R.string.asset_saving_a11y, (p * 100).toInt())
  Row(
    Modifier
      .fillMaxWidth()
      .heightIn(min = 48.dp)
      .clip(RoundedCornerShape(24.dp))
      .background(Palette.select)
      .padding(start = 18.dp, end = 8.dp, top = 8.dp, bottom = 8.dp)
      .semantics { contentDescription = savingLabel },
    verticalAlignment = Alignment.CenterVertically,
    horizontalArrangement = Arrangement.spacedBy(12.dp),
  ) {
    Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(6.dp)) {
      Row {
        Text(str(R.string.asset_saving), style = CaptaeType.subheadline.weight(FontWeight.SemiBold), modifier = Modifier.weight(1f))
        Text("${(p * 100).toInt()}%", style = CaptaeType.subheadline.monospacedDigit().weight(FontWeight.SemiBold))
      }
      ProgressBar(p)
    }
    Box(
      Modifier.clip(CircleShape).background(Palette.card).clickable(role = Role.Button, onClick = onCancel).padding(horizontal = 12.dp, vertical = 7.dp),
    ) {
      Text(str(R.string.job_cancel), style = CaptaeType.footnote.weight(FontWeight.SemiBold))
    }
  }
}

/** 信息（桌面端右栏「信息」：左边灰字标签、右边值；来源是蓝色等宽网址） */
@Composable
private fun Info(a: Asset) {
  val src = str(R.string.asset_src)
  val rows = listOfNotNull(
    src to (a.url.ifEmpty { a.src }),
    str(R.string.asset_fmt) to a.fmt,
    a.res.takeIf { it.isNotEmpty() }?.let { str(R.string.asset_res) to it },
    a.dur.takeIf { it.isNotEmpty() }?.let { str(R.string.asset_dur) to it },
    str(R.string.asset_size) to a.size,
    a.downloadedAt.takeIf { it.isNotEmpty() }?.let { str(R.string.asset_when) to it },
    a.location.takeIf { it.isNotEmpty() }?.let { str(R.string.asset_where) to it },
  )
  GroupedSection(header = str(R.string.asset_info)) {
    Column(Modifier.padding(vertical = 8.dp)) {
      for ((k, v) in rows) InfoRow(k, v, link = k == src && a.url.isNotEmpty())
    }
  }
}

@Composable
private fun InfoRow(label: String, value: String, link: Boolean, labelWidth: Dp = 64.dp) {
  Row(Modifier.fillMaxWidth().padding(horizontal = 16.dp, vertical = 6.dp), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
    Text(label, style = CaptaeType.subheadline, color = Palette.ink3, modifier = Modifier.width(labelWidth))
    Text(
      value,
      style = if (link) CaptaeType.footnote.monospaced() else CaptaeType.subheadline.monospacedDigit(),
      color = if (link) Palette.accentInk else Palette.ink,
      modifier = Modifier.weight(1f).padding(top = if (link) 1.dp else 0.dp),
    )
  }
}

/** 标签（桌面端「短片 ×」「动画 ×」：浅灰胶囊） */
@Composable
private fun Tags(tags: List<String>) {
  Column(Modifier.padding(horizontal = 16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
    Text(str(R.string.asset_tags), style = CaptaeType.footnote.weight(FontWeight.Medium), color = Palette.ink3, modifier = Modifier.padding(start = 4.dp))
    FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
      for (t in tags) {
        Box(Modifier.clip(CircleShape).background(Palette.select).padding(horizontal = 12.dp, vertical = 6.dp)) {
          Text(t, style = CaptaeType.footnote.weight(FontWeight.Medium), color = Palette.ink2)
        }
      }
      Row(
        Modifier.clip(CircleShape).padding(horizontal = 8.dp, vertical = 6.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(4.dp),
      ) {
        Icon(Sym.plus, size = 14.dp, tint = Palette.ink3)
        Text(str(R.string.asset_tags), style = CaptaeType.footnote, color = Palette.ink3)
      }
    }
  }
}
