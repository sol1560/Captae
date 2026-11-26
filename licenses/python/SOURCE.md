# Python 3.14.10（Android 完整版）

Android 完整版的 APK 里带 python.org 官方的 Android 嵌入包（Android embeddable package），用来运行 yt-dlp。只在 Android 完整版里，桌面端和 iOS 不带。

- 来源：https://www.python.org/downloads/release/python-31410/
  - `python-3.14.10-aarch64-linux-android.tar.gz`，SHA256 `423c80b957330cf0ebb92b9b20f2294c292144a441bdb0ceb8c69d14928e569b`
  - `python-3.14.10-x86_64-linux-android.tar.gz`，SHA256 `6ebda1fc3f28efc5861cad397254e8ce5d81ba4f83a3cb6a0c124c3826a0e14c`
  - 两个包都另用官方 sigstore 签名验过（发布者 `hugo@python.org`）
- 没有改动：`libpython3.14.so`、扩展模块（改名成 `libpy_<模块>.so`）、标准库原样；去掉了用不上的 `test`、`idlelib`、`tkinter`、`ensurepip` 等目录
  （脚本 `scripts/mobile/android-engine/build-python.sh`）
- 许可证：Python Software Foundation License，全文见 `LICENSE.txt`（官方包里的原文）

## 官方包里一起带的库

| 库 | 版本 | 许可证 | 在哪 |
|---|---|---|---|
| OpenSSL | 3.5.9 | Apache-2.0，全文见 `LICENSE.OpenSSL.txt`（https://github.com/openssl/openssl/blob/openssl-3.5.9/LICENSE.txt） | `libssl_python.so`、`libcrypto_python.so`（FFmpeg 的 https 也用这一份） |
| SQLite | 3.53.4 | 公有领域（https://sqlite.org/copyright.html） | `libsqlite3_python.so` |
| bzip2 | 1.0.8 | bzip2 许可证（BSD 类，https://sourceware.org/bzip2/） | 编进 `libpy__bz2.so` |
| xz / liblzma | 5.4.6 | 0BSD / 公有领域（https://tukaani.org/xz/） | 编进 `libpy__lzma.so` |
| zstd | 1.5.7 | BSD-3-Clause（https://github.com/facebook/zstd/blob/dev/LICENSE） | 编进 `libpy__zstd.so` |
| libffi | 官方包自带的版本 | MIT（https://github.com/libffi/libffi/blob/master/LICENSE） | 编进 `libpy__ctypes.so` |
| mpdecimal、expat、HACL* 等 CPython 源码里自带的 | 随 CPython 3.14.10 | 见 https://docs.python.org/3.14/license.html | 编进 `libpython3.14.so` 和扩展模块 |

这些库由 CPython 官方的构建脚本（`Android/android.py`，依赖来自 https://github.com/beeware/cpython-android-source-deps ）编译，
Captae 没有重新编译它们。源码：https://www.python.org/ftp/python/3.14.10/Python-3.14.10.tar.xz
