# 第三方许可证和源码地址

这个目录会原样复制进安装包：macOS 在 `Captae.app/Contents/Resources/licenses/`，Windows 在安装目录（默认 `%LOCALAPPDATA%\Captae`）的 `licenses\`，
Linux 在 `/usr/lib/Captae/licenses/`（AppImage 里在 `usr/lib/Captae/licenses/`）。总览见仓库根目录的 `THIRD_PARTY_NOTICES.md`。

## 随安装包附带的独立程序

下面这些程序作为**独立的可执行文件**随安装包分发，Captae 只以子进程方式调用它们，不链接它们的代码。
每个程序保留自己的许可证，许可证全文和源码获取方式在对应的子目录里（`SOURCE.md` 按平台分节写明用的是哪一份构建）。

| 程序 | 版本 | 许可证 | 各平台的构建 | 目录 |
|---|---|---|---|---|
| FFmpeg / ffprobe | 9.0.2 | GPL-3.0-or-later（构建启用了 GPL 组件，带 libass） | macOS：martin-riedl.de 的静态构建；Windows、Linux：BtbN/FFmpeg-Builds 的 GPL 静态构建 | `ffmpeg/` |
| aria2 | 1.37.0 | GPL-2.0-or-later（含 OpenSSL 例外）；和 OpenSSL 3 链接的 macOS、Linux 构建按 GPL-3.0-or-later | macOS：Homebrew 构建连同 OpenSSL、libssh2、c-ares、SQLite、libintl 动态库；Windows：官方构建（静态链接 GMP、Expat、SQLite、zlib、c-ares、libssh2）；Linux：本仓库脚本从源码编（静态链接 OpenSSL、c-ares、zlib、Expat、SQLite） | `aria2/` |
| yt-dlp | 2026.10.26 | 源码 Unlicense；官方打包的目录版含 GPL-3.0-or-later 组件 | 官方 `yt-dlp_macos.zip`、`yt-dlp_win.zip`、`yt-dlp_linux.zip` | `yt-dlp/` |
| deno | 2.9.8 | MIT | 官方二进制 | `deno/` |

源码：每个版本的 GitHub Release 页（https://github.com/sol1560/Captae/releases ）和安装包放在一起，附上 FFmpeg、aria2、yt-dlp 对应版本的源码包；
FFmpeg 静态链接的各个库的源码地址见 `ffmpeg/SOURCE.md` 里的构建脚本。

书面承诺：自本版本发布起三年内，任何人都可以通过项目的 GitHub Issues 索取上面 GPL 程序
对应版本的完整源码（包括 FFmpeg 静态链接的各个库），我们会免费提供（或提供各子目录里列出的原始源码包地址的存档副本）。

## 打包进 Captae 程序和插件里的

| 什么 | 许可证 | 文件 |
|---|---|---|
| affectUI 免费组件（36 个，源码原样在 `apps/desktop/src/components/ui/`） | MIT | `affectui/LICENSE.md` |
| Hugeicons Free 图标（界面、插件网页部件、affectUI 组件里内嵌的） | MIT | `hugeicons/LICENSE.md` |
| Geist、Geist Mono 字体（桌面端、插件） | SIL Open Font License 1.1 | `geist/OFL.txt` |
| 上手引导和安装包窗口用的 Blender 开放电影剧照 | CC BY 3.0 / CC BY 2.5 | `blender-stills/README.md` |
| npm 依赖（React、Motion、Sonner、@tauri-apps/api……共 12 个包） | MIT、0BSD | `npm-packages.md` |
| Rust 依赖（Tauri 和其余 crate，三个平台合在一张表里，注明只在哪个平台编进去的） | 大多是 MIT / Apache-2.0，另有 BSD、ISC、Zlib、MPL-2.0、Unicode-3.0、CDLA-Permissive-2.0 等 | `rust-crates.md` |

`npm-packages.md` 和 `rust-crates.md` 由 `scripts/gen-licenses.py` 生成（离线读 `cargo tree`、`cargo metadata`、`package-lock.json`），依赖变了以后重新跑。

## Android 完整版的下载引擎

只在 Android 完整版的 APK 里（详见 `THIRD_PARTY_NOTICES.md` 第 5 节）：

| 程序 | 版本 | 许可证 | 目录 |
|---|---|---|---|
| Python（官方 Android 嵌入包，含 OpenSSL、SQLite 等） | 3.14.10 | PSF License；OpenSSL 是 Apache-2.0 | `python/` |
| yt-dlp（官方 zipapp） | 2026.11.23 | Unlicense | `yt-dlp-android/` |
| QuickJS | 2026-06-04 | MIT | `quickjs/` |
| FFmpeg / ffprobe（LGPL 构建，链进 LAME 3.100、dav1d 1.5.5） | 9.1 | LGPL-3.0-or-later（链接 OpenSSL 3 的缘故）；LAME 是 LGPL-2.0-or-later，dav1d 是 BSD-2-Clause。对应源码和编译方法见目录里的 `SOURCE.md` | `ffmpeg-android/` |

## 手机 App 的第三方库

| 是什么 | 许可证 | 文件 |
|---|---|---|
| Android App 打进 APK 的 Maven 依赖（Jetpack Compose、CameraX、ZXing core、JNA、backdrop、rustls-platform-verifier 的 Kotlin 部分……） | 大多 Apache-2.0；JNA 按 Apache-2.0 用；CameraX core 另含 BSD 许可的 libyuv | `android-libraries.md`（`scripts/gen-licenses.py` 生成） |
| 手机端的 Rust 核心（iOS、Android 共用） | 见文件 | `rust-crates-mobile.md` |
| iOS App | 不用第三方 Swift 包，只用系统框架 | — |

## Linux AppImage 里随带的系统库

AppImage 要在不同的发行版上直接运行，打包时（linuxdeploy 和它的 GTK 插件）会把 Captae 用到的一部分系统库从打包机器
（Ubuntu 22.04）拷进 AppImage 的 `usr/lib/`：GTK 3、WebKitGTK 4.1 和它们依赖的 GLib、libsoup、HarfBuzz 等。
这些库都是动态链接、没有改动，主要是 LGPL-2.1-or-later（GTK、GLib、WebKitGTK、libsoup 等），其余是 MIT、BSD、ISC 这类宽松许可证；
清单、许可证和源码获取方式见 `linux-appimage-libs.md`。.deb 不带这些库，用系统自己的（deb 的依赖里写着）。
