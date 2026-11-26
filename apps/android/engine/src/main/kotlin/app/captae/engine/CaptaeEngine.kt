// Android 完整版下载引擎的 App 这一边（AGPL-3.0，Captae 自己写的）。
//
// 能执行的都在原生库目录（安装器从 APK 解压出来，Android 10 起只有这里的文件能 execve）：
//   libytdlp.so（yt-dlp 启动器）、libpython3.14.so、libpy_<模块>.so（扩展模块）、libqjs.so、libffmpeg.so、libffprobe.so ……
// captae-core 的 AndroidRunner 按 MobileCore 的 nativeLibDir 找它们。
//
// 数据（标准库的 .py、yt-dlp 的 zipapp）打在 assets/captae-engine/engine.zip 里，prepare() 第一次（或 App 更新换了引擎时）
// 解压到 noBackupFilesDir/captae-engine/，再用 Os.setenv 设好环境变量，之后 Rust 起的子进程都继承：
//   CAPTAE_ENGINE_HOME  上面那个目录（启动器在这里找标准库和 yt-dlp）
//   TMPDIR              cacheDir/captae-engine-tmp（yt-dlp 给 QuickJS 写临时脚本）
//   XDG_CACHE_HOME      cacheDir（yt-dlp 的缓存：YouTube 播放器脚本等，cacheDir/yt-dlp/）
//   HOME                没有时设成 filesDir
//   SSL_CERT_FILE       noBackupFilesDir/captae-engine-cacerts.pem：系统证书库（AndroidCAStore，含用户自己装的证书）导出的 PEM。
//                       Python（yt-dlp）和 ffmpeg 用的 OpenSSL 读它；系统的证书目录按旧式哈希命名，OpenSSL 3 直接读不了
package app.captae.engine

import android.content.Context
import android.system.Os
import android.util.Base64
import java.io.File
import java.io.FileOutputStream
import java.security.KeyStore
import java.security.cert.X509Certificate
import java.util.zip.ZipInputStream
import org.json.JSONObject

object CaptaeEngine {
  private const val ASSETS = "captae-engine"
  private const val STAMP = ".engine-sha256"

  /** 引擎各部分的版本和位置（排错、「设置 › 关于」用） */
  data class Info(
    val home: File,
    val nativeLibDir: String,
    val python: String,
    val ytdlp: String,
    val quickjs: String,
    val ffmpeg: String,
    /** 这次 prepare 有没有解压（第一次打开、App 更新换了引擎） */
    val extracted: Boolean,
    /**
     * 引擎要的环境变量（prepare 已经用 Os.setenv 设进进程，Rust 起的子进程会继承）。
     * Java 的 ProcessBuilder 用的是 JVM 启动时的环境快照、看不到后来设的，自己起程序时把这些加进 environment()
     */
    val env: Map<String, String>,
  )

  /** 原生库目录：建 MobileCore 时传给 InitOptions.nativeLibDir */
  fun nativeLibDir(context: Context): String = context.applicationInfo.nativeLibraryDir

  /** 解压出来的数据目录 */
  fun home(context: Context): File = File(context.applicationContext.noBackupFilesDir, "captae-engine")

  /** 原生库目录里某个程序的完整路径（libytdlp.so、libffmpeg.so、libqjs.so……） */
  fun program(context: Context, name: String): File = File(nativeLibDir(context), name)

  /**
   * 准备好引擎：需要时解压数据、设环境变量。在建 MobileCore 之前、后台线程里调（第一次解压约 12 MB，要一两秒）。
   * 重复调没关系，引擎没变就只设环境变量。
   */
  @Synchronized
  fun prepare(context: Context): Info {
    val ctx = context.applicationContext
    val meta = JSONObject(ctx.assets.open("$ASSETS/engine.json").bufferedReader().use { it.readText() })
    val want = meta.getString("zipSha256")
    val home = home(ctx)
    val stamp = File(home, STAMP)
    val ok = stamp.isFile && stamp.readText().trim() == want &&
      File(home, "lib/python3.14/os.py").isFile && File(home, "yt-dlp").isFile
    if (!ok) {
      extract(ctx, home, want)
    }
    val tmp = File(ctx.cacheDir, "captae-engine-tmp").apply { mkdirs() }
    val env = linkedMapOf(
      "CAPTAE_ENGINE_HOME" to home.absolutePath,
      "TMPDIR" to tmp.absolutePath,
      "XDG_CACHE_HOME" to ctx.cacheDir.absolutePath,
      "HOME" to (Os.getenv("HOME")?.takeIf { it.isNotEmpty() } ?: ctx.filesDir.absolutePath),
      // captae-core 的 AndroidRunner 起程序时也会设这个；ffmpeg 的库自己带 RUNPATH，不靠它
      "LD_LIBRARY_PATH" to nativeLibDir(ctx),
    )
    exportCerts(File(ctx.noBackupFilesDir, "captae-engine-cacerts.pem"))?.let { env["SSL_CERT_FILE"] = it.absolutePath }
    for ((k, v) in env) {
      // LD_LIBRARY_PATH 不设进 App 自己的进程（只给子进程），其余的设进去让 Rust 起的子进程继承
      if (k != "LD_LIBRARY_PATH") Os.setenv(k, v, true)
    }
    return Info(
      home = home,
      nativeLibDir = nativeLibDir(ctx),
      python = meta.getString("python"),
      ytdlp = meta.getString("ytdlp"),
      quickjs = meta.getString("quickjs"),
      ffmpeg = meta.getString("ffmpeg"),
      extracted = !ok,
      env = env,
    )
  }

  /** 系统证书库（系统自带 + 用户装的，用户在设置里停用的不在里面）导出成 PEM。失败返回 null（启动器会自己拼一份系统的） */
  private fun exportCerts(out: File): File? = try {
    val ks = KeyStore.getInstance("AndroidCAStore").apply { load(null) }
    val sb = StringBuilder()
    for (alias in ks.aliases()) {
      val c = ks.getCertificate(alias) as? X509Certificate ?: continue
      sb.append("-----BEGIN CERTIFICATE-----\n")
      sb.append(Base64.encodeToString(c.encoded, Base64.DEFAULT))
      sb.append("-----END CERTIFICATE-----\n")
    }
    if (sb.isEmpty()) {
      null
    } else {
      val tmp = File(out.parentFile, out.name + ".part")
      tmp.writeText(sb.toString())
      if (tmp.renameTo(out)) out else null
    }
  } catch (e: Exception) {
    null
  }

  /** 解到旁边的临时目录，全部写完再换掉旧的：中途被杀掉也不会留下半套引擎 */
  private fun extract(ctx: Context, home: File, sha: String) {
    val parent = home.parentFile!!
    val partial = File(parent, "captae-engine.partial")
    partial.deleteRecursively()
    partial.mkdirs()
    val root = partial.canonicalPath + File.separator
    ctx.assets.open("$ASSETS/engine.zip").use { raw ->
      ZipInputStream(raw.buffered(1 shl 16)).use { zip ->
        val buf = ByteArray(1 shl 16)
        while (true) {
          val e = zip.nextEntry ?: break
          val out = File(partial, e.name)
          // 防 zip 里的「../」写到目录外面
          if (!out.canonicalPath.startsWith(root)) throw SecurityException("engine.zip 里有不合法的路径：${e.name}")
          if (e.isDirectory) {
            out.mkdirs()
          } else {
            out.parentFile?.mkdirs()
            FileOutputStream(out).use { os ->
              while (true) {
                val n = zip.read(buf)
                if (n < 0) break
                os.write(buf, 0, n)
              }
            }
          }
        }
      }
    }
    File(partial, STAMP).writeText(sha)
    val old = File(parent, "captae-engine.old")
    old.deleteRecursively()
    if (home.exists() && !home.renameTo(old)) home.deleteRecursively()
    if (!partial.renameTo(home)) throw IllegalStateException("没法把引擎放到 $home")
    old.deleteRecursively()
  }
}
