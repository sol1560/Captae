# yt-dlp 2026.11.23（Android 完整版）

Android 完整版带的是 yt-dlp 官方发布的 **zipapp**（release 里名字就叫 `yt-dlp` 的那个文件，纯 Python 源码打成的 zip），
由 APK 里的 Python 解释器运行。它不是 PyInstaller 打包的可执行文件，不含 GPL 组件。

- 许可证：Unlicense，全文见 `LICENSE.Unlicense`
- 来源：https://github.com/yt-dlp/yt-dlp/releases/tag/2026.11.23
  - `yt-dlp`，SHA256 `e8665ed64d6b9172773e812f891fb707a0cb1ee8d9ef4fc1acb00cabee4c5256`
  - 校验方式：先用 yt-dlp 官方公钥（Simon Sawicki (yt-dlp signing key)，指纹 `AC0CBBE6848D6A873464AF4E57CF65933B5A7581`，
    `crates/captae-core/assets/yt-dlp-public.key`）验 `SHA2-256SUMS.sig`，再核对清单里的 SHA256
- 里面带的 yt-dlp-ejs 0.8.0（解 YouTube JS 验证的脚本，Unlicense）打包了 meriyah 6.1.4（ISC）和 astring 1.9.0（MIT），
  它们的许可证全文写在脚本文件开头（`yt_dlp_ejs/yt/solver/lib.min.js`），原样保留
- 没有改动。App 里可以更新 yt-dlp（F-Droid 允许）：同样验官方签名和 SHA256、自检通过才换，见 `work/mobile/M3-android-engine.md`
