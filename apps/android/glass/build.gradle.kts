plugins {
  alias(libs.plugins.android.library)
  alias(libs.plugins.kotlin.compose)
}

android {
  namespace = "app.captae.glass"
  // backdrop 2.0.1 的 AAR 写死 minCompileSdk=37
  compileSdk = 37
  defaultConfig {
    minSdk = 26
  }
  compileOptions {
    sourceCompatibility = JavaVersion.VERSION_17
    targetCompatibility = JavaVersion.VERSION_17
  }
  buildFeatures {
    compose = true
  }
}

dependencies {
  api(platform(libs.compose.bom))
  api(libs.compose.ui)
  api(libs.compose.ui.graphics)
  api(libs.compose.ui.text)
  api(libs.compose.foundation)
  api(libs.compose.animation)
  api(libs.backdrop)
  api(libs.shapes)
  implementation(libs.core.ktx)
  // 覆盖层接系统返回（BackHandler）
  implementation(libs.activity.compose)
  implementation(libs.kotlinx.coroutines.android)
  implementation(libs.compose.ui.tooling.preview)
  debugImplementation(libs.compose.ui.tooling)
}
