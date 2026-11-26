// 各模块自己声明插件，这里只把版本钉住（版本都在 gradle/libs.versions.toml）
plugins {
  alias(libs.plugins.android.application) apply false
  alias(libs.plugins.android.library) apply false
  alias(libs.plugins.kotlin.compose) apply false
}
