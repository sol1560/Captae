// Android 完整版的下载引擎：Python 3.14（python.org 官方包）+ yt-dlp + QuickJS + FFmpeg（LGPL 构建）。
//
// 能执行的（yt-dlp 启动器、libpython、扩展模块、qjs、ffmpeg、ffprobe）都改名成 lib*.so 放进 jniLibs，
// 安装时解压到原生库目录（Android 10 起只有那里的文件能 execve）；标准库和 yt-dlp 的 zipapp 是数据，
// 打成 assets/captae-engine/engine.zip，首次启动由 CaptaeEngine.prepare 解压到 App 私有目录。
//
// 这些东西由 scripts/mobile/android-engine/build.sh 生成到仓库根的 target/android-engine/stage/，不入库。
//
// 用这个模块的 App 要：
//   implementation(project(":engine"))
//   android { packaging { jniLibs { useLegacyPackaging = true } } }   // = extractNativeLibs=true，不然 .so 不解压、没法执行
//   启动时 CaptaeEngine.prepare(context)，建 MobileCore 时 nativeLibDir = CaptaeEngine.nativeLibDir(context)
plugins {
  alias(libs.plugins.android.library)
}

val engineStage = rootProject.layout.projectDirectory.dir("../../target/android-engine/stage")
// 仪器测试里通过 captae-core 跑一遍（MobileCore）：要 Rust 核心的 .so 和 Kotlin 绑定（scripts/mobile/build-rust-android.sh）
val mobileOut = rootProject.layout.projectDirectory.dir("../../target/mobile")

android {
  namespace = "app.captae.engine"
  compileSdk = 37
  defaultConfig {
    minSdk = 26
    testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
  }
  testOptions {
    // 测试 APK 的目标版本：和 App 一样（W^X 限制按 targetSdk ≥ 29 生效，测出来的才和 App 一样）
    targetSdk = 36
  }
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }
  sourceSets {
    getByName("main") {
      jniLibs.srcDir(engineStage.dir("jniLibs"))
      assets.srcDir(engineStage.dir("assets"))
    }
    getByName("androidTest") {
      kotlin.srcDir(mobileOut.dir("kotlin"))
      jniLibs.srcDir(mobileOut.dir("jniLibs"))
    }
  }
  packaging {
    // extractNativeLibs=true：安装时把 .so 解压到原生库目录（测试 APK 也照这个打）
    jniLibs { useLegacyPackaging = true }
  }
  androidResources {
    // engine.zip 自己已经压缩过，APK 里原样存，省得解压时再解一层
    noCompress += "zip"
  }
}

// 编译前检查引擎在不在；缺了就停下，告诉人跑哪个脚本
val checkEngine by tasks.registering {
  val files = listOf(
    engineStage.file("jniLibs/arm64-v8a/libytdlp.so").asFile,
    engineStage.file("jniLibs/x86_64/libytdlp.so").asFile,
    engineStage.file("assets/captae-engine/engine.zip").asFile,
    engineStage.file("assets/captae-engine/engine.json").asFile,
  )
  doLast {
    val missing = files.filterNot { it.exists() }
    if (missing.isNotEmpty()) {
      throw GradleException(
        "缺少下载引擎：\n" + missing.joinToString("\n") { "  $it" } +
          "\n先在仓库根目录跑 scripts/mobile/android-engine/build.sh（生成 target/android-engine/stage/）",
      )
    }
  }
}
tasks.named("preBuild") { dependsOn(checkEngine) }

dependencies {
  androidTestImplementation("androidx.test:runner:1.7.0")
  androidTestImplementation("androidx.test.ext:junit:1.3.0")
  androidTestImplementation("junit:junit:4.13.2")
  androidTestImplementation(libs.kotlinx.coroutines.android)
  // UniFFI 生成的绑定要 JNA（和 app 模块用同一个版本）
  androidTestImplementation("net.java.dev.jna:jna:5.17.0@aar")
  // CaptaeTls.init（Rust 用系统证书校验）要的 Kotlin 部分，本地 Maven 仓库见 settings.gradle.kts
  androidTestImplementation("org.rustls:rustls-platform-verifier:0.2.0")
}
