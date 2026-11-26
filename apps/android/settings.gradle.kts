pluginManagement {
  repositories {
    google {
      content {
        includeGroupByRegex("com\\.android.*")
        includeGroupByRegex("com\\.google.*")
        includeGroupByRegex("androidx.*")
      }
    }
    mavenCentral()
    gradlePluginPortal()
  }
}

dependencyResolutionManagement {
  repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
  repositories {
    google()
    mavenCentral()
    // Rust 核心的证书校验（rustls-platform-verifier 的 Kotlin 部分），scripts/mobile/build-rust-android.sh 生成，不入库
    maven { url = uri("../../target/mobile/maven") }
  }
}

rootProject.name = "captae-android"

include(":glass")
include(":app")
include(":engine")
