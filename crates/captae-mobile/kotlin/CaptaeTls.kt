// Captae 核心库（libcaptae_mobile.so）的 Android 初始化：让 Rust 用系统的证书校验
// （认用户装的证书、企业代理的根证书）。在建 MobileCore 之前调一次：
//
//     CaptaeTls.init(applicationContext)
//
// 没调也能用，只是证书校验退回编进程序的 Mozilla 根证书。
// 需要 rustls-platform-verifier 的 Kotlin 部分（Maven：org.rustls:rustls-platform-verifier，版本见 build-rust-android.sh 的输出，现在是 0.2.0）。
// 这个文件由 scripts/mobile/build-rust-android.sh 复制到 Kotlin 绑定旁边，源文件在 crates/captae-mobile/kotlin/。
package app.captae.core

import android.content.Context

object CaptaeTls {
    init {
        System.loadLibrary("captae_mobile")
    }

    /** 成功返回 true。重复调没关系（只有第一次生效） */
    @JvmStatic
    external fun init(context: Context): Boolean
}
