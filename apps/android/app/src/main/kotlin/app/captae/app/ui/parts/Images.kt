package app.captae.app.ui.parts

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asImageBitmap
import androidx.compose.ui.platform.LocalContext
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Semaphore
import kotlinx.coroutines.sync.withPermit
import kotlinx.coroutines.withContext
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import kotlin.math.max

/**
 封面、缩略图：本地文件（素材的那一帧）或网址（读取信息拿到的封面）。不引图片库，自己读：
 - 网址先下到缓存目录（cacheDir/covers，按网址的 SHA-1 起名），再解码；
 - 解码时按最长边 720 缩（列表、网格、详情顶部都够了）；
 - 内存里留最近用过的（约 24 MB）。这次打开以来读到过的直接显示，不再淡入一遍（MOTION 1.2「封面读到」）。
 */
object Covers {
  private val mem = object : LruCache<String, ImageBitmap>(24 * 1024 * 1024) {
    override fun sizeOf(key: String, value: ImageBitmap) = value.width * value.height * 4
  }
  private val net = Semaphore(4)

  fun cached(key: String): ImageBitmap? = mem.get(key)

  suspend fun load(context: Context, key: String): ImageBitmap? = withContext(Dispatchers.IO) {
    mem.get(key)?.let { return@withContext it }
    val file = if (key.startsWith("http://") || key.startsWith("https://")) download(context, key) else File(key).takeIf { it.isFile }
    val bmp = file?.let { decode(it) } ?: return@withContext null
    bmp.asImageBitmap().also { mem.put(key, it) }
  }

  private fun decode(f: File, maxSide: Int = 720): Bitmap? = runCatching {
    val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeFile(f.path, o)
    if (o.outWidth <= 0) return null
    val s = Integer.highestOneBit(max(1, max(o.outWidth, o.outHeight) / maxSide))
    BitmapFactory.decodeFile(f.path, BitmapFactory.Options().apply { inSampleSize = s })
  }.getOrNull()

  private suspend fun download(context: Context, url: String): File? {
    val dir = File(context.cacheDir, "covers").apply { mkdirs() }
    val name = MessageDigest.getInstance("SHA-1").digest(url.toByteArray()).joinToString("") { "%02x".format(it) }
    val out = File(dir, name)
    if (out.isFile && out.length() > 0) return out
    return net.withPermit {
      runCatching {
        val c = URL(url).openConnection() as HttpURLConnection
        c.connectTimeout = 10_000
        c.readTimeout = 15_000
        c.setRequestProperty("User-Agent", "Mozilla/5.0 (Linux; Android) Captae")
        c.inputStream.use { input ->
          val tmp = File(dir, "$name.part")
          tmp.outputStream().use { input.copyTo(it) }
          tmp.renameTo(out)
        }
        c.disconnect()
        out.takeIf { it.isFile }
      }.getOrNull()
    }
  }
}

/** 一张封面的读取状态：图（读到了的话）、是不是刚刚才读到（要淡入） */
class CoverState(val image: ImageBitmap?, val fresh: Boolean)

/** 读封面：thumb 是文件路径或网址；演示剧照（still:）和空的不读 */
@Composable
fun rememberCover(thumb: String?): CoverState {
  val key = thumb?.takeIf { it.isNotEmpty() && !it.startsWith("still:") }
  val context = LocalContext.current
  // 内存里已经有的直接给（不淡入）
  val initial = remember(key) { key?.let { Covers.cached(it) } }
  var state by remember(key) { mutableStateOf(CoverState(initial, fresh = false)) }
  LaunchedEffect(key) {
    if (key != null && state.image == null) {
      val img = Covers.load(context, key)
      if (img != null) state = CoverState(img, fresh = true)
    }
  }
  return state
}
