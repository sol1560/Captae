plugins {
  alias(libs.plugins.android.application)
  alias(libs.plugins.kotlin.compose)
}

// Rust 核心（crates/captae-mobile）编出来的东西：各 ABI 的 libcaptae_mobile.so 和 UniFFI 生成的 Kotlin 绑定。
// 由 scripts/mobile/build-rust-android.sh 生成到仓库根的 target/mobile/，不入库。
val mobileOut = rootProject.layout.projectDirectory.dir("../../target/mobile")
val coreJniLibs = mobileOut.dir("jniLibs")
val coreKotlin = mobileOut.dir("kotlin")

android {
  namespace = "app.captae.app"
  compileSdk = 37
  defaultConfig {
    applicationId = "app.captae"
    minSdk = 26
    targetSdk = 36
    versionCode = 1
    versionName = "0.3.0"
  }
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }
  buildFeatures {
    compose = true
    // 关于页显示版本号
    buildConfig = true
  }
  // Rust 核心和下载引擎都只编了 arm64-v8a、x86_64（M1.md 第 7 节、M3-android-engine.md 第 9 节）：按 ABI 分包，
  // 别的 ABI（JNA 自带的那些）不打进去，那些手机装上就崩
  splits {
    abi {
      isEnable = true
      reset()
      include("arm64-v8a", "x86_64")
      isUniversalApk = false
    }
  }
  packaging {
    // = extractNativeLibs=true：引擎的 yt-dlp、ffmpeg 等是 lib*.so，要解压到原生库目录才能执行
    jniLibs { useLegacyPackaging = true }
  }
  sourceSets {
    getByName("main") {
      kotlin.directories.add(coreKotlin.asFile.path)
      jniLibs.directories.add(coreJniLibs.asFile.path)
    }
  }
}

// 编译前检查 Rust 核心和绑定在不在；缺了就停下，告诉人跑哪个脚本
val checkCaptaeCore = tasks.register("checkCaptaeCore") {
  val so = coreJniLibs.file("arm64-v8a/libcaptae_mobile.so").asFile
  val so86 = coreJniLibs.file("x86_64/libcaptae_mobile.so").asFile
  val bindings = coreKotlin.file("uniffi/captae_mobile/captae_mobile.kt").asFile
  val tls = coreKotlin.file("app/captae/core/CaptaeTls.kt").asFile
  val maven = mobileOut.dir("maven/org/rustls/rustls-platform-verifier").asFile
  inputs.files(so, so86, bindings, tls).optional()
  doLast {
    val missing = listOf(so, so86, bindings, tls, maven).filterNot { it.exists() }
    if (missing.isNotEmpty()) {
      throw GradleException(
        "缺少 Rust 核心或 Kotlin 绑定：\n" + missing.joinToString("\n") { "  $it" } +
          "\n先在仓库根目录跑 scripts/mobile/build-rust-android.sh（生成 target/mobile/jniLibs、kotlin、maven）",
      )
    }
  }
}
tasks.named("preBuild") { dependsOn(checkCaptaeCore) }

dependencies {
  implementation(project(":glass"))
  // 下载引擎（Python + yt-dlp + QuickJS + FFmpeg），同事负责的模块
  implementation(project(":engine"))
  implementation(libs.activity.compose)
  implementation(libs.core.ktx)
  implementation(libs.kotlinx.coroutines.android)
  implementation(libs.compose.ui.tooling.preview)
  debugImplementation(libs.compose.ui.tooling)
  // UniFFI 生成的 Kotlin 绑定通过 JNA 调 .so（Apache-2.0 / LGPL-2.1 双许可，用 Apache-2.0）
  implementation("net.java.dev.jna:jna:5.17.0@aar")
  // 扫配对二维码：CameraX 取景（Apache-2.0）+ ZXing 解码（Apache-2.0，只用 core，纯 Java、不联网）
  implementation("androidx.camera:camera-camera2:1.5.0")
  implementation("androidx.camera:camera-lifecycle:1.5.0")
  implementation("androidx.camera:camera-view:1.5.0")
  implementation("com.google.zxing:core:3.5.3")
  // Rust 用系统证书校验的 Kotlin 部分（MIT / Apache-2.0），本地 Maven 仓库见 settings.gradle.kts
  implementation("org.rustls:rustls-platform-verifier:0.2.0")
}
