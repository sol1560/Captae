# 第三方组件声明

Captae 自己的代码用 AGPL-3.0-or-later 许可，见 `LICENSE`。安装包里还带着下面这些第三方的东西，各自保留原来的许可证。
许可证全文、版权行和源码地址都在 `licenses/`。安装包里：macOS 在 `Captae.app/Contents/Resources/`，Windows 在安装目录（默认 `%LOCALAPPDATA%\Captae`），
Linux 在 `/usr/lib/Captae/`（AppImage 里在 `usr/lib/Captae/`），`licenses/` 和这份声明都在那里。

## 1. 随安装包附带的独立程序

Captae 以子进程方式调用它们（传命令行参数、读输出），不链接它们的代码。三个平台的版本相同，构建来源见「构建」一列和详情目录里的 `SOURCE.md`。

| 程序 | 版本 | 许可证 | 构建 | 源码 | 详情 |
|---|---|---|---|---|---|
| yt-dlp（官方 PyInstaller 目录版） | 2026.10.26 | 源码 Unlicense；官方打包文件带 Python 等组件，整体按 GPL-3.0-or-later 分发 | 官方 `yt-dlp_macos.zip`、`yt-dlp_win.zip`、`yt-dlp_linux.zip` | https://github.com/yt-dlp/yt-dlp | `licenses/yt-dlp/` |
| FFmpeg / ffprobe（静态构建） | 9.0.2 | GPL-3.0-or-later（`--enable-gpl --enable-version3`，带 libass、x264、x265 等） | macOS：martin-riedl.de 的 arm64 构建；Windows、Linux：BtbN/FFmpeg-Builds 的 GPL 构建（n9.0.2-39-g7adfbe40dc） | https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz | `licenses/ffmpeg/` |
| aria2 | 1.37.0 | GPL-2.0-or-later，含 OpenSSL 例外；和 OpenSSL 3（Apache-2.0）链接的 macOS、Linux 构建整体按 GPL-3.0-or-later；所带的库各自的许可证见详情 | macOS：Homebrew 构建连同 OpenSSL、libssh2、c-ares、SQLite、libintl 动态库；Windows：官方构建（静态链接 GMP、Expat、SQLite、zlib、c-ares、libssh2）；Linux：`scripts/fetch-sidecars-linux.sh` 从源码编（静态链接 OpenSSL 3.5、c-ares、zlib、Expat、SQLite） | https://github.com/aria2/aria2 | `licenses/aria2/` |
| deno（yt-dlp 解 YouTube 的 JS 验证用） | 2.9.8 | MIT | 官方二进制 | https://github.com/denoland/deno | `licenses/deno/` |

GPL 程序的源码获取方式和书面承诺见 `licenses/README.md`。

## 2. 打包进 Captae 程序和浏览器插件的

| 组件 | 许可证 | 版权 | 用在哪 | 详情 |
|---|---|---|---|---|
| React、React DOM 19.3.1，scheduler 0.28.0 | MIT | Meta Platforms, Inc. and affiliates | 桌面端界面、插件工具栏面板 | `licenses/npm-packages.md` |
| Motion 14.14（motion、framer-motion、motion-dom、motion-utils） | MIT | Framer B.V.、Motion B.V. | 动效 | 同上 |
| Sonner 2.0.8 | MIT | Emil Kowalski | 提示条 | 同上 |
| @tauri-apps/api 2.11.1 | MIT 或 Apache-2.0（按 MIT） | Tauri Apps Contributors | 界面和 Rust 后端通信 | 同上 |
| tslib 2.9.1 | 0BSD | Microsoft Corporation | Motion 带进来的 | 同上 |
| Hugeicons Free（@hugeicons/core-free-icons 4.4.0、@hugeicons/react 1.1.10） | MIT | 2025 Hugeicons | 图标（界面、插件网页部件 `extension/lib/kit.js`、affectUI 组件里内嵌的，内嵌处留着署名注释） | `licenses/hugeicons/LICENSE.md` |
| affectUI 免费组件（36 个，来源 https://www.affectui.com ，源码原样放在 `apps/desktop/src/components/ui/`） | MIT（官网写明；文件里没有许可证头，附 MIT 全文） | affectUI | 按钮、菜单、弹窗、表单、分段控件、提示等 | `licenses/affectui/LICENSE.md` |
| Geist、Geist Mono 字体 | SIL Open Font License 1.1 | 2024 The Geist Project Authors | 桌面端、插件 | `licenses/geist/OFL.txt` |
| 开放电影剧照：*Big Buck Bunny*、*Sintel*、*Tears of Steel*、*Elephants Dream* | CC BY 3.0（*Elephants Dream* 是 CC BY 2.5） | Blender Foundation（*Elephants Dream* 另有 Netherlands Media Art Institute） | 上手引导插图、安装包窗口的背景图（原样，按卡片裁切显示） | `licenses/blender-stills/README.md` |
| 站点图标（YouTube、哔哩哔哩、抖音 / TikTok、X、Instagram） | CC0 1.0（Simple Icons）；标识归各自的所有者 | — | 指代来源网站 | `apps/desktop/src/assets/img/CREDITS.md` |
| Tauri 2.11 和其余 Rust 依赖（三个平台合在一张表里，注明只在哪个平台编进去：Windows 的 windows-sys、webview2-com，Linux 的 gtk、webkit2gtk 绑定等） | 大多是 MIT / Apache-2.0；另有 BSD-3-Clause、ISC、Zlib、MPL-2.0（cssparser、selectors、dtoa-short、option-ext，未改动）、Unicode-3.0、CDLA-Permissive-2.0 等 | 见清单 | 桌面端和命令行 `captae` | `licenses/rust-crates.md` |

`licenses/npm-packages.md`、`licenses/rust-crates.md` 由 `scripts/gen-licenses.py` 离线生成。

## 3. Linux AppImage 里随带的系统库

AppImage 要在不同的发行版上直接运行，打包时会把 GTK 3、WebKitGTK 4.1 和它们依赖的一部分系统库从打包机器（Ubuntu 22.04）拷进 AppImage。
这些库动态链接、没有改动，主要是 LGPL-2.1-or-later，其余是 MIT、BSD 这类宽松许可证；清单、许可证和源码（Ubuntu 的源码包）见 `licenses/linux-appimage-libs.md`。
.deb 不带这些库，用系统自己的。Windows 版用系统自带的 WebView2（没有时安装程序从微软下载），不随包分发。

## 4. 用到时才下载的（不在安装包里）

| 组件 | 许可证 | 什么时候下载 |
|---|---|---|
| faster-whisper、CTranslate2 和它们的 Python 依赖 | MIT 等 | 用户在设置或上手引导里点「安装语音识别」时，从 PyPI 装进数据目录里单独的 Python 环境 |
| Whisper 模型（faster-whisper 转换版） | MIT | 同上，从 Hugging Face 下载 |
| 新版 yt-dlp | 同第 1 节 | 自动更新：从 GitHub 官方发布页下载，先用随包附带的 yt-dlp 官方公钥验签、核对 SHA256 |

## 5. 只在开发和演示时用（不进安装包）

TypeScript、Vite、@vitejs/plugin-react、@types/react、@types/react-dom、vite-plugin-singlefile（只在演示构建里把页面打成单个 HTML）是开发依赖；
演示后端用的剧照在 `apps/desktop/src/dev/img/`（CC BY，出处见那里的 `CREDITS.md`）。这些都不进正式安装包。

## 5. 手机端

手机 App 里的 Rust 核心（`crates/captae-mobile`）编进去的 crate 见 `licenses/rust-crates-mobile.md`（`scripts/gen-licenses.py` 生成）。

### Android 完整版的下载引擎（`apps/android/engine`）

只在 Android 完整版（F-Droid、GitHub Releases）的 APK 里，iOS 和桌面端不带。都是独立的程序或库，Captae 以子进程方式调用 yt-dlp、ffmpeg、ffprobe、qjs，
不链接它们；文件改名成 `lib*.so` 放在原生库目录，只是因为 Android 10 起只有那里的文件能执行。编译和打包脚本在 `scripts/mobile/android-engine/`。

| 组件 | 版本 | 许可证 | 来源 | 详情 |
|---|---|---|---|---|
| Python（python.org 官方 Android 嵌入包，原样） | 3.14.10 | PSF License；带的 OpenSSL 3.5.9 是 Apache-2.0，SQLite 公有领域，bzip2、xz、zstd、libffi 是 BSD / 0BSD / MIT | https://www.python.org/downloads/release/python-31410/ | `licenses/python/` |
| yt-dlp（官方 zipapp，纯 Python） | 2026.11.23 | Unlicense（里面带的 meriyah 是 ISC、astring 是 MIT，许可证原文留在文件里） | https://github.com/yt-dlp/yt-dlp/releases/tag/2026.11.23 | `licenses/yt-dlp-android/` |
| QuickJS（从源码编译，没有改动） | 2026-06-04 | MIT | https://bellard.org/quickjs/ | `licenses/quickjs/` |
| FFmpeg / ffprobe（从源码编译的 LGPL 构建，没有改动，不带 GPL 组件） | 9.1 | LGPL-3.0-or-later（`--enable-version3`：链接了 Apache-2.0 的 OpenSSL 3，Apache-2.0 只和 LGPL 3.0 兼容） | https://ffmpeg.org/releases/ffmpeg-9.1.tar.xz | `licenses/ffmpeg-android/`（源码获取方式、编译脚本、书面承诺） |
| LAME（MP3 编码，静态链进 FFmpeg 的 libavcodec） | 3.100 | LGPL-2.0-or-later | https://lame.sourceforge.io | `licenses/ffmpeg-android/` |
| dav1d（AV1 解码，静态链进 FFmpeg 的 libavcodec） | 1.5.5 | BSD-2-Clause | https://code.videolan.org/videolan/dav1d | `licenses/ffmpeg-android/` |

yt-dlp 可以在 App 里更新：从 GitHub 官方发布页下载新的 zipapp，先用 yt-dlp 官方公钥验签、核对 SHA256，自检通过才换（许可证同上）。
