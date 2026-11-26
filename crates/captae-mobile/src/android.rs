//! Android 上的 JNI 入口：初始化系统证书校验（rustls-platform-verifier 要拿到 App 的 Context 才能调 Android 的证书库）。
//!
//! Kotlin 那边（`kotlin/CaptaeTls.kt`，构建脚本复制到绑定旁边）在建 `MobileCore` 之前调一次：
//! ```kotlin
//! CaptaeTls.init(applicationContext)
//! ```
//! 没调的话核心库退回编进程序的 Mozilla 根证书（webpki-roots），不会崩。
//! 另外 App 要带上 rustls-platform-verifier 的 Kotlin 部分（Maven 包 `rustls:rustls-platform-verifier`），见 work/mobile/M1.md。

use jni::errors::LogErrorAndDefault;
use jni::objects::{JClass, JObject};
use jni::sys::jboolean;
use jni::EnvUnowned;

/// `app.captae.core.CaptaeTls.init(context)`：成功返回 true。失败只记日志、返回 false（不抛 Java 异常），核心库退回编进程序的根证书
#[no_mangle]
pub extern "system" fn Java_app_captae_core_CaptaeTls_init<'local>(mut env: EnvUnowned<'local>, _class: JClass<'local>, context: JObject<'local>) -> jboolean {
    let ok = env
        .with_env(|env| -> Result<bool, jni::errors::Error> {
            match rustls_platform_verifier::android::init_with_env(env, context) {
                Ok(()) => {
                    captae_core::net::set_platform_tls_ready();
                    Ok(true)
                }
                Err(e) => {
                    tracing::warn!("系统证书校验初始化失败，退回编进程序的根证书：{e}");
                    Ok(false)
                }
            }
        })
        .resolve::<LogErrorAndDefault>();
    ok
}
