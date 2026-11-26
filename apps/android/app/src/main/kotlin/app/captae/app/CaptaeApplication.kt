package app.captae.app

import android.app.Application
import app.captae.app.core.CaptaeRuntime
import app.captae.app.ui.parts.Str

/** 进程一起来就建 Rust 核心（前台服务、分享进来的链接、界面都要用；App 被系统结束后再打开，核心会接着下没下完的） */
class CaptaeApplication : Application() {
  override fun onCreate() {
    super.onCreate()
    Str.res = resources
    Str.ctx = this
    CaptaeRuntime.init(this)
  }
}
