// Rust 核心用 Android 系统证书校验（CaptaeTls.init，rustls-platform-verifier 0.7.1 / Kotlin 部分 0.2.0）的仪器测试。
// 0.1.1 的 Kotlin 部分把取不到吊销信息的证书（Let's Encrypt 2025 年停了 OCSP，只剩 http:// 的 CRL，Android 默认不许明文请求）一律判成「已吊销」；
// 0.2.0 随 aar 带了网络安全配置，给 CA 的 CRL 域名放开明文。见 work/mobile/M2b.md 第 9 节。
//
// 初始化是整个进程的、撤不回，所以单独一个类，单独跑（不要和 EngineDownloadTest 一起跑）：
//   adb -s <设备> shell am instrument -w -r -e class app.captae.engine.PlatformTlsTest app.captae.engine.test/androidx.test.runner.AndroidJUnitRunner
//   adb -s <设备> logcat -d -s CaptaePlatformTls
//
// 直链都用 .mp4 结尾的地址：这样读取信息走 Rust 自己的 HTTP 客户端（证书校验就是要测的这一套），不经过 yt-dlp（它用 Python 自己的证书）
package app.captae.engine

import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.BeforeClass
import org.junit.FixMethodOrder
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.MethodSorters
import uniffi.captae_mobile.AddOptions
import uniffi.captae_mobile.InitOptions
import uniffi.captae_mobile.MobileCore
import uniffi.captae_mobile.MobileException
import uniffi.captae_mobile.MobileJobState
import uniffi.captae_mobile.Platform

@RunWith(AndroidJUnit4::class)
@FixMethodOrder(MethodSorters.NAME_ASCENDING)
class PlatformTlsTest {
  companion object {
    private const val TAG = "CaptaePlatformTls"
    // Blender 基金会的公开文件（Sintel 预告片 480p，4,372,373 字节，CC BY 3.0）
    const val BLENDER = "https://download.blender.org/durian/trailer/sintel_trailer-480p.mp4"
    const val YOUTUBE = "https://www.youtube.com/watch?v=aqz-KE-bpKQ"
    // Let's Encrypt 官方的测试站：证书有效 / 已经吊销（https://letsencrypt.org/docs/certificate-compatibility/ 列的测试站点）
    const val LE_VALID = "https://valid-isrgrootx1.letsencrypt.org/captae-tls-check.mp4"
    const val LE_REVOKED = "https://revoked-isrgrootx1.letsencrypt.org/captae-tls-check.mp4"
    // 自签证书
    const val SELF_SIGNED = "https://self-signed.badssl.com/captae-tls-check.mp4"

    lateinit var core: MobileCore
    var initOk = false

    fun log(s: String) {
      Log.i(TAG, s)
    }

    @BeforeClass
    @JvmStatic
    fun setUp() {
      val ctx = InstrumentationRegistry.getInstrumentation().targetContext
      CaptaeEngine.prepare(ctx)
      // 和 App 一样：先初始化证书校验，再建核心（HTTP 客户端是建核心时建的）
      initOk = app.captae.core.CaptaeTls.init(ctx)
      val data = File(ctx.cacheDir, "platform-tls").apply {
        deleteRecursively()
        mkdirs()
      }
      core = MobileCore(InitOptions(data.absolutePath, File(data, "Downloads").absolutePath, Platform.ANDROID, "zh-CN", CaptaeEngine.nativeLibDir(ctx)))
    }

    /** 读取信息，返回 (成功了没有, 失败时的 code / message / raw) */
    fun probe(url: String): Pair<Boolean, String> = try {
      runBlocking { core.probe(url, null) }
      true to ""
    } catch (e: MobileException.Failed) {
      false to "${e.detail.code} | ${e.detail.message} | ${e.detail.raw}"
    }

    /** 失败的原因是证书（被吊销、不认识的签发者、自签……） */
    fun certError(why: String): Boolean {
      val w = why.lowercase()
      return listOf("certificate", "revoked", "unknownissuer", "unknown issuer", "self signed", "self-signed", "invalidcertificate").any { w.contains(it) }
    }
  }

  @Test
  fun a_platform_verifier_is_on() {
    val i = core.info()
    log("CaptaeTls.init：$initOk；证书校验 ${i.tls}")
    assertTrue(initOk)
    assertEquals("platform", i.tls)
  }

  @Test
  fun b_blender_direct_download() {
    val t0 = System.nanoTime()
    val job = core.add(AddOptions(BLENDER, null, null, false, null, null))
    var j = core.job(job.id)!!
    while ((System.nanoTime() - t0) / 1_000_000_000 < 300) {
      j = core.job(job.id)!!
      if (j.state == MobileJobState.DONE || j.state == MobileJobState.FAILED) break
      Thread.sleep(300)
    }
    val f = j.path?.let { File(it) }
    log("Blender 直链：${j.state}，${f?.length()} 字节，${(System.nanoTime() - t0) / 1_000_000} 毫秒；失败：${j.failure?.code} ${j.failure?.raw}")
    assertEquals(MobileJobState.DONE, j.state)
    assertEquals(4_372_373L, f!!.length())
  }

  @Test
  fun c_youtube_probe() {
    val p = runBlocking { core.probe(YOUTUBE, null) }
    log("YouTube 读取：${p.title}，${p.qualities.size} 种清晰度")
    assertTrue(!p.title.isNullOrEmpty())
  }

  @Test
  fun d_letsencrypt_valid_passes_tls() {
    // 文件不存在（404），但证书这一关要过：失败原因不能是证书
    val (ok, why) = probe(LE_VALID)
    log("Let's Encrypt 有效证书：成功 $ok；$why")
    assertTrue("证书有效的站被拒了：$why", ok || !certError(why))
  }

  @Test
  fun e_letsencrypt_revoked_rejected() {
    val (ok, why) = probe(LE_REVOKED)
    log("Let's Encrypt 已吊销证书：成功 $ok；$why")
    assertTrue("已吊销的证书没被拒", !ok && certError(why))
  }

  @Test
  fun f_self_signed_rejected() {
    val (ok, why) = probe(SELF_SIGNED)
    log("自签证书：成功 $ok；$why")
    assertTrue("自签证书没被拒", !ok && certError(why))
  }
}
