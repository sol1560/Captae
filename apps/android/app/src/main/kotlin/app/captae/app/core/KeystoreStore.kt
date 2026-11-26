package app.captae.app.core

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import androidx.core.content.edit
import uniffi.captae_mobile.SecureStore
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 遥控电脑的设备令牌放在这里（REMOTE.md 第 2 节）：Android Keystore 里一把 AES-GCM 密钥（不离开安全硬件、不跟着备份走），
 值加密以后存进 SharedPreferences（「iv + 密文」的 Base64）。Rust 不把令牌写进任何文件。
 回调可能在任何线程上来：读写都加锁；Keystore 本身线程安全。
 */
class KeystoreStore(context: Context) : SecureStore {
  private val prefs = context.applicationContext.getSharedPreferences("remote-secure", Context.MODE_PRIVATE)
  private val lock = Any()

  private fun key(): SecretKey {
    val ks = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
    (ks.getKey(ALIAS, null) as? SecretKey)?.let { return it }
    val gen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
    gen.init(
      KeyGenParameterSpec.Builder(ALIAS, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setKeySize(256)
        .setUserAuthenticationRequired(false)
        .build(),
    )
    return gen.generateKey()
  }

  override fun read(key: String): String? = synchronized(lock) {
    val stored = prefs.getString(key, null) ?: return null
    try {
      val raw = Base64.decode(stored, Base64.NO_WRAP)
      val c = Cipher.getInstance(TRANSFORM)
      c.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, raw, 0, IV))
      String(c.doFinal(raw, IV, raw.size - IV), Charsets.UTF_8)
    } catch (e: Exception) {
      // 密钥没了（清过数据、换了手机）：当成没有令牌，那台电脑显示要重新配对
      Log.w(TAG, "读令牌失败", e)
      null
    }
  }

  override fun write(key: String, value: String): Boolean = synchronized(lock) {
    try {
      val c = Cipher.getInstance(TRANSFORM)
      c.init(Cipher.ENCRYPT_MODE, key())
      val iv = c.iv
      val out = iv + c.doFinal(value.toByteArray(Charsets.UTF_8))
      // commit：配对时要确定写进去了才算成功
      prefs.edit(commit = true) { putString(key, Base64.encodeToString(out, Base64.NO_WRAP)) }
      true
    } catch (e: Exception) {
      Log.e(TAG, "写令牌失败", e)
      false
    }
  }

  override fun delete(key: String) = synchronized(lock) {
    prefs.edit(commit = true) { remove(key) }
  }

  private companion object {
    const val TAG = "CaptaeRemote"
    const val ALIAS = "captae.remote.tokens"
    const val TRANSFORM = "AES/GCM/NoPadding"
    const val IV = 12
  }
}
