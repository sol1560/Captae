# aria2 1.37.0

- 许可证：GPL-2.0-or-later，附带允许链接 OpenSSL 的例外条款；GPLv2 全文见 `COPYING.GPLv2`，例外条款和它针对的旧版 OpenSSL 许可证见 `LICENSE.OpenSSL`。
  那条例外只针对旧版 OpenSSL 的许可证；OpenSSL 3 改成了 Apache-2.0（和 GPLv2 不兼容、和 GPLv3 兼容），
  所以链接 OpenSSL 3 的两份构建（macOS、Linux）按 aria2 许可证里「或更高版本」的选择，整体按 **GPL-3.0-or-later** 分发，
  GPLv3 全文见 `../ffmpeg/COPYING.GPLv3`。Windows 构建不用 OpenSSL（TLS 用 Windows 自己的 SChannel），按 GPL-2.0-or-later 分发。
- 上游源码：https://github.com/aria2/aria2/releases/download/release-1.37.0/aria2-1.37.0.tar.xz
  （SHA256 `60a420ad7085eb616cb6e2bdf0a7206d68ff3d37fb5a956dc44242eb2f79b66b`）
- 静态链接或随附的库的许可证原文在 `libs/`（版权年份按各自最新的版本，条款相同）。SQLite 是公有领域，没有许可证文本。
- 用到的 libstdc++、libgcc 由 GCC 运行库例外（GCC Runtime Library Exception）覆盖，不附加义务。

## macOS（arm64）

`Captae.app/Contents/MacOS/aria2c`，来自 Homebrew 的 aria2 1.37.0 构建，它依赖的动态库一并放在
`Contents/Resources/aria2-libs/`（aria2 启动时要加载的 OpenSSL legacy 模块在其中的 `ossl-modules/`），由 `scripts/fetch-sidecars.sh` 准备：

| 库 | 版本 | 许可证 | 源码 |
|---|---|---|---|
| OpenSSL (libssl, libcrypto, legacy 模块) | 3.6.4 | Apache-2.0（`libs/OpenSSL-3-LICENSE.txt`） | https://github.com/openssl/openssl/releases/tag/openssl-3.6.4 |
| libssh2 | 1.11.1 | BSD-3-Clause（`libs/libssh2-COPYING.txt`） | https://libssh2.org/download/libssh2-1.11.1.tar.gz |
| c-ares | 1.34.6 | MIT（`libs/c-ares-LICENSE.md`） | https://github.com/c-ares/c-ares/releases/tag/v1.34.6 |
| SQLite | 3.53.2 | 公有领域 | https://www.sqlite.org/ |
| GNU gettext (libintl) | 1.0 | LGPL-2.1-or-later | https://ftpmirror.gnu.org/gettext/gettext-1.0.tar.gz |

libintl 以动态库形式单独提供，可以替换成用户自己编译的版本（LGPL 的要求）。

## Windows（x64）

安装目录里的 `aria2c.exe`：aria2 官方发布的 `aria2-1.37.0-win-64bit-build1.zip`
（https://github.com/aria2/aria2/releases/download/release-1.37.0/aria2-1.37.0-win-64bit-build1.zip ，
SHA256 `67d015301eef0b612191212d564c5bb0a14b5b9c4796b76454276a4d28d9b288`，原样，没有改动）。
官方用 mingw-w64 在 Ubuntu 上交叉编译，构建脚本是 aria2 源码包里的 `Dockerfile.mingw`。静态链接的库（官方 `README.mingw`）：

| 库 | 版本 | 许可证 | 源码 |
|---|---|---|---|
| GMP | 6.3.0 | LGPL-3.0-or-later 或 GPL-2.0-or-later 二选一（这里按 GPL-2.0-or-later，和 aria2 一致） | https://gmplib.org/download/gmp/gmp-6.3.0.tar.xz |
| Expat | 2.5.0 | MIT（`libs/expat-COPYING.txt`） | https://github.com/libexpat/libexpat/releases/download/R_2_5_0/expat-2.5.0.tar.bz2 |
| SQLite | 3.43.1 | 公有领域 | https://www.sqlite.org/2023/sqlite-autoconf-3430100.tar.gz |
| zlib | 1.3 | Zlib（`libs/zlib-LICENSE.txt`） | https://www.zlib.net/fossils/zlib-1.3.tar.gz |
| c-ares | 1.19.1 | MIT（`libs/c-ares-LICENSE.md`） | https://c-ares.org/download/c-ares-1.19.1.tar.gz |
| libssh2 | 1.11.0 | BSD-3-Clause（`libs/libssh2-COPYING.txt`） | https://libssh2.org/download/libssh2-1.11.0.tar.bz2 |

## Linux（x86_64）

`/usr/lib/Captae/aria2c`（AppImage 里在 `usr/lib/Captae/`）：官方没有 Linux 二进制，由 `scripts/fetch-sidecars-linux.sh`
在 Ubuntu 22.04 上从下面的官方源码包编译（脚本里写死了每个源码包的校验值和全部编译参数，照着运行就能得到同样的程序）。
依赖库都编成静态库链进 aria2c，只动态链接 glibc；aria2 本身没有改动。

| 库 | 版本 | 许可证 | 源码 |
|---|---|---|---|
| OpenSSL | 3.5.9（LTS，`no-shared no-module`，legacy 提供者编在里面） | Apache-2.0（`libs/OpenSSL-3-LICENSE.txt`） | https://github.com/openssl/openssl/releases/download/openssl-3.5.9/openssl-3.5.9.tar.gz |
| c-ares | 1.34.9 | MIT（`libs/c-ares-LICENSE.md`） | https://github.com/c-ares/c-ares/releases/download/v1.34.9/c-ares-1.34.9.tar.gz |
| zlib | 1.3.2 | Zlib（`libs/zlib-LICENSE.txt`） | https://github.com/madler/zlib/releases/download/v1.3.2/zlib-1.3.2.tar.gz |
| Expat | 2.11.0 | MIT（`libs/expat-COPYING.txt`） | https://github.com/libexpat/libexpat/releases/download/R_2_11_0/expat-2.11.0.tar.xz |
| SQLite | 3.54.1 | 公有领域 | https://www.sqlite.org/2026/sqlite-autoconf-3540100.tar.gz |
