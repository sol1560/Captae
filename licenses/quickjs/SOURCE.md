# QuickJS 2026-06-04（Android 完整版）

yt-dlp 解 YouTube 的 JS 验证要一个 JS 运行时。Android 完整版带自己从源码编译的 QuickJS 命令行程序 `qjs`（APK 里叫 `libqjs.so`）。只在 Android 完整版里。

- 作者：Fabrice Bellard、Charlie Gordon
- 许可证：MIT，全文见 `LICENSE`（源码包里的原文）
- 源码：https://bellard.org/quickjs/quickjs-2026-06-04.tar.xz ，SHA256 `b376e839b322978313d929fd20663b11ba58b75df5a46c126dd19ea2fa70ad2a`
- 没有改动源码。编译方法：`scripts/mobile/android-engine/build-quickjs.sh`（Android NDK 28.2.13676358，`-O2`，API 26）
