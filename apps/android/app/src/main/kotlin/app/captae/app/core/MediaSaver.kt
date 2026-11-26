package app.captae.app.core

import android.content.ContentValues
import android.content.Context
import android.content.SharedPreferences
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.util.Log
import android.webkit.MimeTypeMap
import androidx.core.content.FileProvider
import androidx.core.content.edit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import uniffi.captae_mobile.LibraryQuery
import uniffi.captae_mobile.MobileAsset
import uniffi.captae_mobile.MobileCore
import java.io.File
import kotlin.math.max

/**
 下完的文件存到系统里（PLAN 5.4「保存」），不要存储权限：
 - Android 10 起用 MediaStore：视频 `Movies/Captae`、图片 `Pictures/Captae`、声音 `Music/Captae`，相册和「文件」里都看得到；
   存好以后删掉 App 目录里那份（不占两份空间），素材库里这条记住它在 MediaStore 里的地址。
 - Android 8–9 没有 RELATIVE_PATH，写公共目录要存储权限：挪到 App 自己的外部目录（`Android/data/app.captae/files/Movies/Captae`），
   用 FileProvider 分享、打开。
 - 存之前生成一张封面小图（核心库手机上没有 ffmpeg，不出缩略图）：视频取一帧，图片缩小。
 每条素材存了什么记在 SharedPreferences（素材 id → 地址、类型、封面）。
 */
object MediaSaver {
  private const val TAG = "CaptaeSave"
  private lateinit var app: Context
  private lateinit var prefs: SharedPreferences
  private val lock = Mutex()

  /** 存到了哪一类目录（界面上写「影片 › Captae」这类说法） */
  enum class Place { Movies, Pictures, Music }

  fun init(context: Context) {
    app = context.applicationContext
    prefs = app.getSharedPreferences("media", Context.MODE_PRIVATE)
  }

  private val thumbs: File get() = File(app.filesDir, "captae/app-thumbs").apply { mkdirs() }

  fun thumbFor(id: Long): String? = File(thumbs, "$id.jpg").takeIf { it.isFile }?.path

  fun placeOf(id: Long): Place? = prefs.getString("place:$id", null)?.let { runCatching { Place.valueOf(it) }.getOrNull() }

  /** 打开、分享用的地址（MediaStore 的 content://，或 FileProvider 给的） */
  fun uriFor(a: MobileAsset): Uri? {
    prefs.getString("uri:${a.id}", null)?.let { return Uri.parse(it) }
    val f = prefs.getString("file:${a.id}", null)?.let(::File) ?: File(a.path).takeIf { it.isFile } ?: return null
    return runCatching { FileProvider.getUriForFile(app, "${app.packageName}.files", f) }.getOrNull()
  }

  fun mimeOf(name: String, kind: String): String =
    MimeTypeMap.getSingleton().getMimeTypeFromExtension(name.substringAfterLast('.', "").lowercase())
      ?: when (kind) { "audio" -> "audio/*"; "image" -> "image/*"; else -> "video/*" }

  /** 存好了没有（存好的素材，核心那份文件已经删了，exists 是 false，界面不当成「文件不见了」） */
  fun saved(id: Long): Boolean = prefs.contains("uri:$id") || prefs.contains("file:$id")

  /** 把素材 id 这条存进系统（已经存过就跳过） */
  suspend fun save(core: MobileCore, id: Long, jobThumb: String?, remoteKey: String? = null) = withContext(Dispatchers.IO) {
    lock.withLock {
      if (saved(id)) return@withLock
      val a = runCatching { core.asset(id) }.getOrNull() ?: return@withLock
      val src = File(a.path)
      if (!src.isFile) return@withLock
      makeThumb(a, src)
      val place = when (a.kind) { "audio" -> Place.Music; "image" -> Place.Pictures; else -> Place.Movies }
      try {
        if (Build.VERSION.SDK_INT >= 29) {
          val uri = insert(src, a, place) ?: return@withLock
          prefs.edit {
            putString("uri:$id", uri.toString()); putString("place:$id", place.name)
            // 从电脑存过来的：素材详情里「已存到手机」也认这条
            if (remoteKey != null) putString(remoteKey, uri.toString())
          }
        } else {
          val dirType = when (place) { Place.Music -> Environment.DIRECTORY_MUSIC; Place.Pictures -> Environment.DIRECTORY_PICTURES; Place.Movies -> Environment.DIRECTORY_MOVIES }
          val dir = File(app.getExternalFilesDir(dirType), "Captae").apply { mkdirs() }
          val dst = unique(dir, src.name)
          if (!src.renameTo(dst)) { src.copyTo(dst); src.delete() }
          prefs.edit { putString("file:$id", dst.path); putString("place:$id", place.name); if (remoteKey != null) putString(remoteKey, Uri.fromFile(dst).toString()) }
        }
        // 存好了：App 目录里那份删掉（不占两份空间）
        src.delete()
        Log.i(TAG, "已存到 $place：${a.title}")
      } catch (e: Exception) {
        Log.e(TAG, "存到系统失败：${a.path}", e)
      }
    }
  }

  /** Android 10+：写进 MediaStore（先标成「还没写完」，写完再放开，相册里不会出现半个文件） */
  private fun insert(src: File, a: MobileAsset, place: Place): Uri? {
    val resolver = app.contentResolver
    val collection = when (place) {
      Place.Movies -> MediaStore.Video.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
      Place.Pictures -> MediaStore.Images.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
      Place.Music -> MediaStore.Audio.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
    }
    val rel = when (place) { Place.Movies -> Environment.DIRECTORY_MOVIES; Place.Pictures -> Environment.DIRECTORY_PICTURES; Place.Music -> Environment.DIRECTORY_MUSIC } + "/Captae"
    val values = ContentValues().apply {
      put(MediaStore.MediaColumns.DISPLAY_NAME, src.name)
      put(MediaStore.MediaColumns.MIME_TYPE, mimeOf(src.name, a.kind))
      put(MediaStore.MediaColumns.RELATIVE_PATH, rel)
      put(MediaStore.MediaColumns.TITLE, a.title)
      put(MediaStore.MediaColumns.IS_PENDING, 1)
    }
    val uri = resolver.insert(collection, values) ?: return null
    try {
      resolver.openOutputStream(uri)?.use { out -> src.inputStream().use { it.copyTo(out, 1 shl 16) } } ?: error("打不开 $uri")
      resolver.update(uri, ContentValues().apply { put(MediaStore.MediaColumns.IS_PENDING, 0) }, null, null)
      return uri
    } catch (e: Exception) {
      resolver.delete(uri, null, null)
      throw e
    }
  }

  private fun unique(dir: File, name: String): File {
    var f = File(dir, name)
    var n = 1
    while (f.exists()) {
      f = File(dir, "${name.substringBeforeLast('.')} ($n).${name.substringAfterLast('.', "")}".removeSuffix("."))
      n++
    }
    return f
  }

  /** 封面小图：视频取四分之一处的一帧，图片缩小；最长边 640 */
  private fun makeThumb(a: MobileAsset, src: File) {
    val out = File(thumbs, "${a.id}.jpg")
    if (out.isFile) return
    val bmp: Bitmap? = try {
      when (a.kind) {
        "video" -> MediaMetadataRetriever().run {
          try {
            setDataSource(src.path)
            // 取四分之一处的一帧（片头常是黑场），最多到第 15 秒；时长不知道时取第 3 秒
            val at = a.duration?.let { (it * 1_000_000 * 0.25).toLong().coerceIn(0, 15_000_000) } ?: 3_000_000
            if (Build.VERSION.SDK_INT >= 27) getScaledFrameAtTime(at, MediaMetadataRetriever.OPTION_CLOSEST_SYNC, 640, 640) else getFrameAtTime(at)
          } finally { release() }
        }
        "image" -> {
          val o = BitmapFactory.Options().apply { inJustDecodeBounds = true }
          BitmapFactory.decodeFile(src.path, o)
          val s = max(1, max(o.outWidth, o.outHeight) / 640)
          BitmapFactory.decodeFile(src.path, BitmapFactory.Options().apply { inSampleSize = Integer.highestOneBit(s) })
        }
        else -> null
      }
    } catch (e: Exception) {
      Log.w(TAG, "生成封面失败：${src.name}", e)
      null
    }
    bmp?.let { b -> out.outputStream().use { b.compress(Bitmap.CompressFormat.JPEG, 82, it) } }
  }

  /**
   电脑上的素材存到手机，下完了（RemoteHub 的 onSave DONE）：存进 MediaStore（按扩展名分相册 / 音乐），删掉 App 目录里那份。
   记下「电脑、素材 id → 地址」，素材详情里显示「已存到手机」、能打开。手机素材库（核心的）现在没有导入接口，这份不进手机素材库。
   */
  suspend fun saveRemote(p: uniffi.captae_mobile.SaveProgress) = withContext(Dispatchers.IO) {
    lock.withLock {
      val key = "remote:${p.device}:${p.assetId}"
      if (prefs.contains(key)) return@withLock
      val src = File(p.path)
      if (!src.isFile) return@withLock
      // 文件名去掉存的时候加的「素材 id-」前缀
      val name = src.name.substringAfter('-', src.name)
      val mime = mimeOf(name, "video")
      val kind = when { mime.startsWith("audio") -> "audio"; mime.startsWith("image") -> "image"; else -> "video" }
      val place = when (kind) { "audio" -> Place.Music; "image" -> Place.Pictures; else -> Place.Movies }
      try {
        val fake = uniffi.captae_mobile.MobileAsset(
          id = -1, kind = kind, title = name.substringBeforeLast('.'), description = "", url = "", pageUrl = "", site = "", uploader = "", path = src.path,
          format = name.substringAfterLast('.', "").uppercase(), width = null, height = null, duration = null, bytes = src.length().toULong(), thumb = null,
          tags = emptyList(), createdAt = System.currentTimeMillis(), parentId = null, live = false, exists = true,
        )
        if (Build.VERSION.SDK_INT >= 29) {
          val renamed = File(src.parentFile, name).also { if (!src.renameTo(it)) src.copyTo(it, overwrite = true) }
          val uri = insert(renamed, fake, place) ?: return@withLock
          prefs.edit { putString(key, uri.toString()) }
          renamed.delete()
        } else {
          val dirType = when (place) { Place.Music -> Environment.DIRECTORY_MUSIC; Place.Pictures -> Environment.DIRECTORY_PICTURES; Place.Movies -> Environment.DIRECTORY_MOVIES }
          val dst = unique(File(app.getExternalFilesDir(dirType), "Captae").apply { mkdirs() }, name)
          if (!src.renameTo(dst)) { src.copyTo(dst); src.delete() }
          prefs.edit { putString(key, Uri.fromFile(dst).toString()) }
        }
        src.delete()
        Log.i(TAG, "电脑上的素材已存到 $place：$name")
      } catch (e: Exception) {
        Log.e(TAG, "存电脑上的素材失败：${p.path}", e)
      }
    }
  }

  /** 电脑上的这条素材存到手机了没有（存好的地址） */
  fun remoteSaved(device: String, assetId: Long): Uri? = prefs.getString("remote:$device:$assetId", null)?.let(Uri::parse)

  /** 删素材时一起删掉系统里那份和封面 */
  fun delete(id: Long) {
    prefs.getString("uri:$id", null)?.let { u -> runCatching { app.contentResolver.delete(Uri.parse(u), null, null) }.onFailure { Log.w(TAG, "删 MediaStore 失败", it) } }
    prefs.getString("file:$id", null)?.let { File(it).delete() }
    File(thumbs, "$id.jpg").delete()
    prefs.edit { remove("uri:$id"); remove("file:$id"); remove("place:$id") }
  }

  /** 启动时补存：素材库里文件还在 App 目录、没存进系统的（上次存到一半 App 被结束了） */
  suspend fun catchUp(core: MobileCore) = withContext(Dispatchers.IO) {
    val page = runCatching { core.library(LibraryQuery(kind = null, search = null, limit = 1000u, offset = 0u)) }.getOrNull() ?: return@withContext
    for (a in page.items) {
      val f = File(a.path)
      if (!f.isFile) continue
      // 已经存进系统的：App 目录里不该还有一份（存的过程中被结束、或者别的原因留下的），删掉不占两份空间
      if (saved(a.id)) f.delete() else save(core, a.id, null)
    }
  }
}
