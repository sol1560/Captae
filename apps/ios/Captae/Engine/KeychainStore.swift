// 电脑发给这台手机的设备令牌只放在钥匙串里（crates/captae-mobile/REMOTE.md 第 2 节，PLAN 5.3）。
// Rust 的 RemoteHub 通过这个回调读写；Rust 不把令牌写进任何文件。
// - kSecClassGenericPassword，service 是 App 的标识 + 「.remote」，account 是 Rust 给的键（captae.remote.token.<证书指纹>）
// - kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly：不跟着备份到别的手机（令牌是电脑发给这台手机的）
// - 回调可能在任何线程上调（App 的线程、Rust 的后台线程）：类型不隔离，钥匙串本身是线程安全的
import CaptaeKit
import Foundation
import Security

nonisolated final class KeychainStore: SecureStore {
  private let service = "app.captae.mobile.remote"

  private func query(_ key: String) -> [String: Any] {
    [kSecClass as String: kSecClassGenericPassword, kSecAttrService as String: service, kSecAttrAccount as String: key]
  }

  func read(key: String) -> String? {
    var q = query(key)
    q[kSecReturnData as String] = true
    q[kSecMatchLimit as String] = kSecMatchLimitOne
    var out: CFTypeRef?
    guard SecItemCopyMatching(q as CFDictionary, &out) == errSecSuccess, let data = out as? Data else { return nil }
    return String(data: data, encoding: .utf8)
  }

  /// 写不进去返回 false：Rust 那边配对算失败，不留下没有令牌的电脑
  func write(key: String, value: String) -> Bool {
    let data = Data(value.utf8)
    let update: [String: Any] = [
      kSecValueData as String: data, kSecAttrAccessible as String: kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly,
    ]
    let status = SecItemUpdate(query(key) as CFDictionary, update as CFDictionary)
    if status == errSecSuccess { return true }
    guard status == errSecItemNotFound else { return false }
    var add = query(key)
    add.merge(update) { $1 }
    return SecItemAdd(add as CFDictionary, nil) == errSecSuccess
  }

  func delete(key: String) {
    SecItemDelete(query(key) as CFDictionary)
  }
}
