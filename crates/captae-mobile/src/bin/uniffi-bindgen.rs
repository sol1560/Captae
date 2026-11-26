//! 生成 Swift / Kotlin 绑定：`cargo run -p captae-mobile --features bindgen --bin uniffi-bindgen -- generate --library <库> --language swift --out-dir <目录>`
//! 只在电脑上跑（scripts/mobile/build-rust-*.sh 调它）。
fn main() {
    uniffi::uniffi_bindgen_main()
}
