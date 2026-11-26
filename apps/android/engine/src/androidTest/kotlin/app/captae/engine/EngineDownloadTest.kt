// 下载引擎的仪器测试：在真设备 / 模拟器上，用 APK 里打包的 yt-dlp + Python + QuickJS + FFmpeg 真的读取、下载。
// 测试 APK 的包名是 app.captae.engine.test（不碰 App 本身 app.captae）。
//
// 跑法（只对一台设备，别用 connectedAndroidTest，它会装到所有连着的设备上）：
//   ./gradlew :engine:assembleDebugAndroidTest
//   adb -s <设备> install -r -t engine/build/outputs/apk/androidTest/debug/engine-debug-androidTest.apk
//   adb -s <设备> shell am instrument -w -r app.captae.engine.test/androidx.test.runner.AndroidJUnitRunner
//   adb -s <设备> logcat -d -s CaptaeEngineTest      # 每一步的输出
//
// 测试只下公开内容的一小段：YouTube 上 Blender 官方的开放电影 Big Buck Bunny（开头 12 秒），B 站一条公开的科普短片（360p）。
// 另外：yt-dlp 应用内更新（验签 → 自检 → 切换，再造一个坏版本看会不会退回）、只要声音出 MP3、AV1 视频出缩略图。
package app.captae.engine

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.FileOutputStream
import java.util.concurrent.TimeUnit
import java.util.zip.ZipEntry
import java.util.zip.ZipOutputStream
import kotlinx.coroutines.runBlocking
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.BeforeClass
import org.junit.FixMethodOrder
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import uniffi.captae_mobile.AddOptions
import uniffi.captae_mobile.CoreListener
import uniffi.captae_mobile.InitOptions
import uniffi.captae_mobile.MobileCore
import uniffi.captae_mobile.MobileJob
import uniffi.captae_mobile.MobileJobState
import uniffi.captae_mobile.Platform

@RunWith(AndroidJUnit4::class)
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class EngineDownloadTest {
  companion object {
    private const val TAG = "CaptaeEngineTest"
    // Blender 基金会官方频道的开放电影（CC BY 3.0）
    const val YOUTUBE = "https://www.youtube.com/watch?v=aqz-KE-bpKQ"
    // B 站公开视频（90 秒的科普动画）
    const val BILIBILI = "https://www.bilibili.com/video/BV1jT421a72z"

    lateinit var info: CaptaeEngine.Info
    lateinit var work: File

    @BeforeClass
    @JvmStatic
    fun setUp() {
      val ctx = InstrumentationRegistry.getInstrumentation().targetContext
      val t0 = System.nanoTime()
      info = CaptaeEngine.prepare(ctx)
      log("prepare：解压=${info.extracted}，${ms(t0)} 毫秒；引擎目录 ${info.home}；原生库目录 ${info.nativeLibDir}")
      log("版本：Python ${info.python}、yt-dlp ${info.ytdlp}、QuickJS ${info.quickjs}、FFmpeg ${info.ffmpeg}")
      log("SSL_CERT_FILE=${info.env["SSL_CERT_FILE"]}")
      work = File(ctx.cacheDir, "engine-test").apply {
        deleteRecursively()
        mkdirs()
      }
    }

    fun log(s: String) {
      // logcat 一行最多 4000 字左右，长的分开打
      s.chunked(3000).forEach { Log.i(TAG, it) }
    }

    fun ms(t0: Long) = (System.nanoTime() - t0) / 1_000_000

    fun lib(name: String) = File(info.nativeLibDir, name).absolutePath

    data class Out(val code: Int, val out: String, val err: String, val millis: Long)

    /** 起原生库目录里的程序（和 captae-core 的 AndroidRunner 一样：程序 = lib*.so，环境变量带上） */
    fun run(args: List<String>, timeoutSec: Long = 600): Out {
      val t0 = System.nanoTime()
      val pb = ProcessBuilder(args).directory(work)
      pb.environment().putAll(info.env)
      val p = pb.start()
      p.outputStream.close()
      var out = ""
      var err = ""
      val to = Thread { out = p.inputStream.bufferedReader().readText() }.apply { start() }
      val te = Thread { err = p.errorStream.bufferedReader().readText() }.apply { start() }
      if (!p.waitFor(timeoutSec, TimeUnit.SECONDS)) {
        p.destroyForcibly()
        throw AssertionError("超时：${args.joinToString(" ")}")
      }
      to.join()
      te.join()
      return Out(p.exitValue(), out, err, ms(t0))
    }

    /** yt-dlp 的参数前缀，和 captae-core（tools.rs prepare）给 yt-dlp 加的一样 */
    fun ytdlp(vararg a: String): List<String> = listOf(
      lib("libytdlp.so"), "--ignore-config",
      "--no-js-runtimes", "--js-runtimes", "quickjs:${lib("libqjs.so")}",
      "--ffmpeg-location", lib("libffmpeg.so"),
    ) + a

    /** ffprobe 读文件：返回 (视频编码, 音频编码, 时长秒) */
    fun probe(f: File): Triple<String?, String?, Double> {
      val r = run(listOf(lib("libffprobe.so"), "-v", "error", "-print_format", "json", "-show_streams", "-show_format", f.absolutePath))
      assertEquals("ffprobe 失败：${r.err}", 0, r.code)
      val j = JSONObject(r.out)
      val streams = j.getJSONArray("streams")
      var v: String? = null
      var a: String? = null
      for (i in 0 until streams.length()) {
        val s = streams.getJSONObject(i)
        when (s.optString("codec_type")) {
          "video" -> v = v ?: "${s.optString("codec_name")} ${s.optInt("width")}x${s.optInt("height")}"
          "audio" -> a = a ?: "${s.optString("codec_name")} ${s.optInt("sample_rate")}Hz"
        }
      }
      return Triple(v, a, j.getJSONObject("format").optString("duration", "0").toDouble())
    }

    fun openCore(name: String): MobileCore {
      val ctx = InstrumentationRegistry.getInstrumentation().targetContext
      val data = File(work, name).apply { mkdirs() }
      return MobileCore(InitOptions(data.absolutePath, File(data, "Downloads").absolutePath, Platform.ANDROID, "zh-CN", CaptaeEngine.nativeLibDir(ctx)))
    }

    /** 加任务、等它完成，返回 (任务, 文件) */
    fun download(core: MobileCore, opts: AddOptions, label: String): Pair<MobileJob, File> {
      var finished: String? = null
      core.setListener(object : CoreListener {
        override fun onJob(job: MobileJob) {}
        override fun onRemoved(id: ULong) {}
        override fun onFinished(id: ULong, title: String, path: String, assetId: Long?) {
          finished = path
        }
        override fun onLibrary() {}
        override fun onNotice(key: String, title: String, text: String) {}
        override fun onResync() {}
      })
      val t0 = System.nanoTime()
      val job = core.add(opts)
      var last: MobileJob? = null
      var phase = ""
      while (ms(t0) < 600_000) {
        val j = core.job(job.id) ?: break
        last = j
        if (j.phase != phase) {
          phase = j.phase
          log("$label：${ms(t0)} 毫秒 状态 ${j.state} 阶段 ${j.phase} ${j.gotBytes}/${j.totalBytes} 字节 ${j.title}")
        }
        if (j.state == MobileJobState.DONE || j.state == MobileJobState.FAILED) break
        Thread.sleep(500)
      }
      val j = last!!
      if (j.state != MobileJobState.DONE) {
        val f = j.failure
        log("任务记录：\n" + core.jobLog(job.id).joinToString("\n") { it.text })
        throw AssertionError("$label 没完成：${j.state} ${f?.code} ${f?.message}\n${f?.raw}")
      }
      Thread.sleep(500)
      return j to File(finished ?: error("没有收到 onFinished"))
    }

    /** 完整解码一遍，确认没有坏帧 */
    fun decodeAll(f: File) {
      val r = run(listOf(lib("libffmpeg.so"), "-v", "error", "-nostdin", "-i", f.absolutePath, "-f", "null", "-"))
      assertEquals("完整解码失败：${r.err}", 0, r.code)
      assertTrue("完整解码有错：${r.err}", r.err.isBlank())
    }
  }

  @Test
  fun a_programs() {
    val q = run(listOf(lib("libqjs.so"), "--help"))
    log("qjs --help（退出码 ${q.code}）：${q.out.lineSequence().first()}")
    assertTrue(q.out.startsWith("QuickJS version ${info.quickjs}"))

    val js = File(work, "t.js").apply { writeText("console.log([1, 2, 3].map(x => x * 2).join(','), 2n ** 64n)") }
    val qr = run(listOf(lib("libqjs.so"), "--script", js.absolutePath))
    log("qjs 跑脚本：${qr.out.trim()}")
    assertEquals("2,4,6 18446744073709551616n", qr.out.trim())

    val f = run(listOf(lib("libffmpeg.so"), "-hide_banner", "-version"))
    val first = f.out.lineSequence().first()
    val conf = f.out.lineSequence().first { it.startsWith("configuration:") }
    log("ffmpeg：$first")
    log("ffmpeg $conf")
    assertTrue(first.startsWith("ffmpeg version ${info.ffmpeg}"))
    assertTrue("不该带 GPL 组件", !conf.contains("--enable-gpl") && !conf.contains("--enable-nonfree") && conf.contains("--enable-version3"))
    val lic = run(listOf(lib("libffmpeg.so"), "-hide_banner", "-L")).out
    log("ffmpeg -L：${lic.lineSequence().filter { it.isNotBlank() }.take(2).joinToString(" / ")}")
    val bsf = run(listOf(lib("libffmpeg.so"), "-hide_banner", "-bsfs")).out
    assertTrue("要有 aac_adtstoasc", bsf.lines().any { it.trim() == "aac_adtstoasc" })
    val enc = run(listOf(lib("libffmpeg.so"), "-hide_banner", "-encoders")).out
    assertTrue("要有 AAC 编码器", enc.lines().any { it.trim().split(Regex("\\s+")).getOrNull(1) == "aac" })

    val y = run(ytdlp("--version"))
    log("yt-dlp --version：${y.out.trim()}（${y.millis} 毫秒，退出码 ${y.code}）${y.err.trim()}")
    assertEquals(0, y.code)
    assertEquals(info.ytdlp, y.out.trim())
    // 第二次（标准库已经写好 __pycache__）
    val y2 = run(ytdlp("--version"))
    log("yt-dlp --version 第二次：${y2.millis} 毫秒")
  }

  @Test
  fun b_youtube_probe_through_core() {
    val ctx = InstrumentationRegistry.getInstrumentation().targetContext
    val data = File(work, "core-probe").apply { mkdirs() }
    val core = MobileCore(InitOptions(data.absolutePath, File(data, "Downloads").absolutePath, Platform.ANDROID, "zh-CN", CaptaeEngine.nativeLibDir(ctx)))
    try {
      val i = core.info()
      log("MobileCore：平台 ${i.platform}，外部程序 ${i.runner}，证书校验 ${i.tls}")
      assertEquals("android", i.runner)
      val t0 = System.nanoTime()
      val p = runBlocking { core.probe(YOUTUBE, null) }
      log("captae-core 读 YouTube（${ms(t0)} 毫秒）：${p.title}，${p.duration} 秒，${p.width}x${p.height}，网站 ${p.site}；" +
        "清晰度 ${p.qualities.joinToString(" ") { it.label }}")
      assertTrue(p.title.orEmpty().contains("Big Buck Bunny"))
      assertTrue("要列出好几种清晰度（解了 JS 验证才有）", p.qualities.size >= 3)
    } finally {
      core.close()
    }
  }

  @Test
  fun c_youtube_sample() {
    // 和 captae-core 的样片（sample.rs：--download-sections "*0-N"，由 ffmpeg 截取）一样只下开头一段。
    // 格式选 360p 的 H.264 + AAC：画面、声音分两路下，再由 ffmpeg 合并
    val out = File(work, "yt").apply { mkdirs() }
    val r = run(ytdlp(
      "-v", "--no-playlist", "--newline",
      "-f", "bv*[height<=360][vcodec^=avc1]+ba[ext=m4a]/b[height<=360]",
      "--download-sections", "*0-12", "--merge-output-format", "mp4",
      "-o", "${out.absolutePath}/%(id)s.%(ext)s", "--print", "after_move:filepath", YOUTUBE,
    ), timeoutSec = 900)
    val jsLines = r.err.lines().filter { it.contains("jsc", ignoreCase = true) || it.contains("JS runtimes", ignoreCase = true) || it.contains("quickjs", ignoreCase = true) }
    log("yt-dlp YouTube 样片：退出码 ${r.code}，${r.millis} 毫秒")
    log("JS 相关输出：\n" + jsLines.joinToString("\n"))
    log("stderr 末尾：\n" + r.err.lines().takeLast(15).joinToString("\n"))
    assertEquals("yt-dlp 失败：${r.err.takeLast(2000)}", 0, r.code)
    assertTrue("yt-dlp 要用 QuickJS 解 JS 验证", jsLines.any { it.contains("quickjs", ignoreCase = true) })
    val f = File(r.out.lines().last { it.isNotBlank() }.trim())
    val (v, a, d) = probe(f)
    log("YouTube 样片：${f.name}，${f.length()} 字节；画面 $v，声音 $a，${"%.2f".format(d)} 秒")
    assertTrue(f.length() in 100_000..20_000_000)
    assertTrue(v != null && v.startsWith("h264") && a != null && a.startsWith("aac") && d in 8.0..20.0)
    decodeAll(f)
  }

  @Test
  fun d_ffmpeg_merge() {
    // 画面、声音分别下一小段，再用 ffmpeg 合并成 MP4（和 captae-core 合并时的参数一样：-c copy）
    val out = File(work, "merge").apply { mkdirs() }
    fun get(fmt: String, name: String): File {
      val r = run(ytdlp("--no-playlist", "-f", fmt, "--download-sections", "*0-8", "-o", "${out.absolutePath}/$name.%(ext)s",
        "--print", "after_move:filepath", YOUTUBE), timeoutSec = 900)
      assertEquals("下 $name 失败：${r.err.takeLast(1500)}", 0, r.code)
      return File(r.out.lines().last { it.isNotBlank() }.trim())
    }
    val v = get("bv*[height<=240][vcodec^=avc1]", "video")
    val a = get("ba[ext=m4a]", "audio")
    log("分开下的：${v.name} ${v.length()} 字节、${a.name} ${a.length()} 字节")
    val merged = File(out, "merged.mp4")
    val r = run(listOf(lib("libffmpeg.so"), "-hide_banner", "-nostdin", "-y", "-loglevel", "error",
      "-i", v.absolutePath, "-i", a.absolutePath, "-map", "0:v:0", "-map", "1:a:0", "-c", "copy", "-movflags", "+faststart", merged.absolutePath))
    assertEquals("ffmpeg 合并失败：${r.err}", 0, r.code)
    val (pv, pa, d) = probe(merged)
    log("ffmpeg 合并：${merged.length()} 字节，画面 $pv，声音 $pa，${"%.2f".format(d)} 秒（${r.millis} 毫秒）")
    assertTrue(pv != null && pa != null)
    decodeAll(merged)
  }

  @Test
  fun f_engine_update_and_rollback() {
    // 应用内更新 yt-dlp：用 GitHub 上当前的官方 release 走一遍（版本和随 App 带的一样，所以用 force = 重新安装）
    val core = openCore("core-update")
    try {
      val before = runBlocking { core.engineStatus() }
      log("更新前：版本 ${before.version}，来源 ${before.source}，随 App 带的 ${before.bundled}，自动更新装的 ${before.active}")
      assertEquals("bundled", before.source)
      val t0 = System.nanoTime()
      val r = runBlocking { core.checkEngineUpdate(true) }
      log("checkEngineUpdate(force)：${r.result}（${ms(t0)} 毫秒）${r.message}\n  " + r.steps.joinToString("\n  "))
      assertEquals("updated", r.result)
      assertTrue("要验签", r.steps.any { it.contains("签名验证通过") })
      assertTrue("要核对 SHA256", r.steps.any { it.contains("SHA256") })
      assertTrue("要自检", r.steps.any { it.contains("自检通过") })
      val after = runBlocking { core.engineStatus() }
      log("更新后：版本 ${after.version}，来源 ${after.source}，在用 ${after.active}，上一个 ${after.previous}")
      assertEquals("managed", after.source)
      assertEquals(r.latest, after.active)
      assertEquals(r.latest, after.version)
      val ver = after.active!!
      val zipapp = File(work, "core-update/engines/yt-dlp/$ver/yt-dlp")
      assertTrue("新版放在 $zipapp", zipapp.isFile)

      // 新版能正常读取（captae-core 起 yt-dlp 时由启动器跑新版的 zipapp）
      val p = runBlocking { core.probe(YOUTUBE, null) }
      log("用新版读 YouTube：${p.title}，${p.qualities.size} 种清晰度")

      // 造一个坏版本：能解开、但一运行就 import 失败（像新版把代码改坏了）
      ZipOutputStream(FileOutputStream(zipapp)).use { z ->
        z.putNextEntry(ZipEntry("__main__.py"))
        z.write("import yt_dlp_broken_by_update\n".toByteArray())
        z.closeEntry()
      }
      // 读一个这个 Core 还没读过的链接（读过的有缓存，不会起 yt-dlp）
      val t1 = System.nanoTime()
      val p2 = runBlocking { core.probe(BILIBILI, null) }
      val st = runBlocking { core.engineStatus() }
      log("坏版本：读取照样成功（${ms(t1)} 毫秒）：${p2.title}；退回后版本 ${st.version}，来源 ${st.source}，在用 ${st.active}，坏版本 ${st.bad}")
      log("更新记录：\n  " + st.log.joinToString("\n  ") { it.text })
      assertTrue(p2.title.orEmpty().contains("圆锥"))
      assertEquals("bundled", st.source)
      assertEquals(null, st.active)
      assertTrue(st.bad.contains(ver))
      assertTrue(st.log.any { it.text.contains("已退回到") })
      assertTrue("坏版本的目录删掉了", !zipapp.exists())
    } finally {
      core.close()
    }
  }

  @Test
  fun g_audio_only_mp3() {
    // 只要声音、格式 MP3：captae-core 交给 yt-dlp / ffmpeg 用 libmp3lame 转
    val core = openCore("core-mp3")
    try {
      val (j, f) = download(core, AddOptions(BILIBILI, null, "MP3", true, null, null), "B 站只要声音")
      val (v, a, d) = probe(f)
      log("只要声音：${j.format}；${f.name} ${f.length()} 字节；画面 $v，声音 $a，${"%.2f".format(d)} 秒")
      assertTrue(f.name.endsWith(".mp3"))
      assertTrue(v == null && a != null && a.startsWith("mp3") && d > 60)
      decodeAll(f)
    } finally {
      core.close()
    }
  }

  @Test
  fun h_av1_thumbnail() {
    // AV1 视频生成缩略图（dav1d 解码）：下 YouTube 的 AV1 画面开头几秒，取一帧缩成 JPEG（和 captae-core 生成缩略图的参数一样）
    val out = File(work, "av1").apply { mkdirs() }
    val r = run(ytdlp("--no-playlist", "-f", "bv*[vcodec^=av01][height<=360]", "--download-sections", "*0-6",
      "-o", "${out.absolutePath}/av1.%(ext)s", "--print", "after_move:filepath", YOUTUBE), timeoutSec = 900)
    assertEquals("下 AV1 失败：${r.err.takeLast(1500)}", 0, r.code)
    val f = File(r.out.lines().last { it.isNotBlank() }.trim())
    val (v, _, d) = probe(f)
    log("AV1 样片：${f.name} ${f.length()} 字节，画面 $v，${"%.2f".format(d)} 秒")
    assertTrue(v != null && v.startsWith("av1"))
    val jpg = File(out, "thumb.jpg")
    val t = run(listOf(lib("libffmpeg.so"), "-hide_banner", "-nostdin", "-y", "-loglevel", "error", "-ss", "2", "-i", f.absolutePath,
      "-frames:v", "1", "-vf", "scale=960:-2", "-q:v", "4", jpg.absolutePath))
    assertEquals("出缩略图失败：${t.err}", 0, t.code)
    val (tv, _, _) = probe(jpg)
    log("AV1 缩略图：${jpg.length()} 字节，$tv（${t.millis} 毫秒）")
    assertTrue(tv != null && tv.startsWith("mjpeg") && jpg.length() > 1000)
  }

  /**
   * 系统证书校验（CaptaeTls.init，M1 留下的待验证项）。放在最后：初始化是整个进程的、撤不回，
   * 而 rustls-platform-verifier-android 0.1.1 把没有 OCSP 地址的证书（2025 年起的 Let's Encrypt 证书）判成「已吊销」，
   * 会让前面走核心库 HTTP 的测试（应用内更新从 release-assets.githubusercontent.com 下文件）失败。见 M3-android-engine.md 第 11 节
   */
  @Test
  fun z_platform_tls() {
    val ctx = InstrumentationRegistry.getInstrumentation().targetContext
    val ok = app.captae.core.CaptaeTls.init(ctx)
    val core = openCore("core-tls")
    try {
      val i = core.info()
      log("CaptaeTls.init：$ok；证书校验 ${i.tls}")
      assertEquals("platform", i.tls)
      val p = runBlocking { core.probe(YOUTUBE, null) }
      log("系统证书校验下读 YouTube：${p.title}")
    } finally {
      core.close()
    }
  }

  @Test
  fun e_bilibili_through_core() {
    // 整个走 captae-core：加任务 → yt-dlp 读取、下载（画面、声音两路）→ ffmpeg 合并 → ffprobe 入库前核对 → 素材库
    val ctx = InstrumentationRegistry.getInstrumentation().targetContext
    val data = File(work, "core-bili").apply { mkdirs() }
    val core = MobileCore(InitOptions(data.absolutePath, File(data, "Downloads").absolutePath, Platform.ANDROID, "zh-CN", CaptaeEngine.nativeLibDir(ctx)))
    var finished: String? = null
    core.setListener(object : CoreListener {
      override fun onJob(job: MobileJob) {}
      override fun onRemoved(id: ULong) {}
      override fun onFinished(id: ULong, title: String, path: String, assetId: Long?) {
        finished = path
      }
      override fun onLibrary() {}
      override fun onNotice(key: String, title: String, text: String) {}
      override fun onResync() {}
    })
    try {
      val t0 = System.nanoTime()
      val job = core.add(AddOptions(BILIBILI, "360p", "MP4", false, null, null))
      var last: MobileJob? = null
      var phase = ""
      while (ms(t0) < 600_000) {
        val j = core.job(job.id) ?: break
        last = j
        if (j.phase != phase) {
          phase = j.phase
          log("B 站任务：${ms(t0)} 毫秒 状态 ${j.state} 阶段 ${j.phase} ${j.gotBytes}/${j.totalBytes} 字节 ${j.title}")
        }
        if (j.state == MobileJobState.DONE || j.state == MobileJobState.FAILED) break
        Thread.sleep(500)
      }
      val j = last!!
      if (j.state != MobileJobState.DONE) {
        val f = j.failure
        log("任务日志：\n" + core.jobLog(job.id).joinToString("\n") { it.toString() })
        throw AssertionError("B 站任务没完成：${j.state} ${f?.code} ${f?.message}\n${f?.raw}")
      }
      Thread.sleep(500)
      log("任务记录：\n" + core.jobLog(job.id).joinToString("\n") { it.toString() })
      val f = File(finished ?: error("没有收到 onFinished"))
      val (v, a, d) = probe(f)
      log("B 站（captae-core）：${j.title}；${j.format}；${f.name} ${f.length()} 字节；画面 $v，声音 $a，${"%.2f".format(d)} 秒；素材 ${j.assetId}；共 ${ms(t0)} 毫秒")
      assertTrue(v != null && a != null && d > 60)
      decodeAll(f)
    } finally {
      core.close()
    }
  }
}
