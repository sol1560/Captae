# FFmpeg 9.1，LGPL 构建（Android 完整版）

Android 完整版带的 `ffmpeg`、`ffprobe` 和它们的动态库是 Captae 自己从 FFmpeg 官方源码编译的 **LGPL 构建**：
没有 `--enable-gpl`、没有 `--enable-nonfree`，不带 x264、x265 等 GPL 库。整个构建按 **GNU LGPL 3.0 或更新版本（LGPL-3.0-or-later）** 授权，
`ffmpeg -L` 也这样显示。许可证全文：`COPYING.LGPLv3`，以及它引用的 `COPYING.GPLv3`。

**为什么是 LGPL 3.0 而不是 2.1**：ffmpeg 的 https 要 TLS 库，这里链接的是 OpenSSL 3（Apache-2.0，Python 官方包里那份）。
Apache-2.0 和 GPL / LGPL 2.x 不兼容，只和 3.0 版兼容，所以照 FFmpeg 的规定开了 `--enable-version3`（不开的话 configure 拒绝链接 OpenSSL 3）。
这份构建只用在 Android 完整版（F-Droid、GitHub Releases）；iOS 版不带 ffmpeg。

APK 里的文件：`libffmpeg.so`（= ffmpeg 程序）、`libffprobe.so`（= ffprobe 程序）、
`libavcodec_captae.so`、`libavformat_captae.so`、`libavfilter_captae.so`、`libavutil_captae.so`、`libswscale_captae.so`、`libswresample_captae.so`（动态库）。
`libavcodec_captae.so` 里静态链接了 LAME 和 dav1d（见下一节）。
Captae 只以子进程方式调用 ffmpeg、ffprobe，不链接这些库。

## 对应源码和编译方法（LGPL 第 4、6 条）

- 源码：https://ffmpeg.org/releases/ffmpeg-9.1.tar.xz
  - SHA256 `1a4d394ae63d190a526cd052eb4415c156b5c5e2df37efab41a7de0b8fde2433`
  - 官方签名 `ffmpeg-9.1.tar.xz.asc`，FFmpeg release signing key（指纹 `FCF986EA15E6E293A5644F10B4322F04D67658D8`）
- 没有改动源码
- 编译脚本：Captae 仓库的 `scripts/mobile/android-engine/build-ffmpeg.sh`（版本、校验值在同目录 `env.sh`），用 Android NDK 28.2.13676358、API 26。
  `ffmpeg -version` 打印的 `configuration:` 就是完整的 configure 参数
- 链接的其它库：OpenSSL 3.5.9（Apache-2.0，就是 Python 官方包里那份，见 `../python/`）、系统的 zlib（Android 自带的 `libz.so`，不打包）
- 动态库是独立的 `.so` 文件，换成自己编译的同名库就能替换（库名带 `_captae` 后缀，用上面的脚本或同样的 configure 参数编出来就是同样的名字）。
  想换 LAME 或 dav1d：改 `build-codecs.sh` 用自己的源码，重跑 `build-codecs.sh` 和 `build-ffmpeg.sh`，换掉 `libavcodec_captae.so`

## 链进 libavcodec 的编解码库

| 库 | 版本 | 许可证 | 源码 | 用来 |
|---|---|---|---|---|
| LAME | 3.100 | LGPL-2.0-or-later，全文见 `COPYING.LAME`（项目主页 https://lame.sourceforge.io ） | https://downloads.sourceforge.net/project/lame/lame/3.100/lame-3.100.tar.gz ，SHA256 `ddfe36cab873794038ae2c1210557ad34857a4b6bdc515785d1da9e175b1da1e`（官方没有签名） | MP3 编码（「只要声音」出 MP3） |
| dav1d | 1.5.5 | BSD-2-Clause，全文见 `COPYING.dav1d`（Copyright © 2018-2025, VideoLAN and dav1d authors） | https://download.videolan.org/pub/videolan/dav1d/1.5.5/dav1d-1.5.5.tar.xz ，SHA256 `ca96b1d0ba3f01f55de7231d775892b64994ba061aaeeaaa4f662a390b3abf9e`，VideoLAN Release Signing Key（`65F7C6B4206BD057A7EB73787180713BE58D1ADC`）签名验过 | AV1 解码（AV1 视频生成缩略图） |

两个都没有改动源码，编译脚本是 `scripts/mobile/android-engine/build-codecs.sh`（静态库、`-fPIC`）。都不是 GPL，加上它们以后 configure 报的许可证还是 LGPL 3.0 或更新版本（脚本会核对）。
LAME 静态链进的 libavcodec 本身是 LGPL、源码和编译方法都在这里，所以满足 LGPL「能换上改过的库重新链接」的要求。

书面承诺：自含有这份构建的版本发布起三年内，任何人都可以通过项目的 GitHub Issues 索取上面源码包（FFmpeg、LAME、dav1d）的副本，我们免费提供。

## 打开了什么

只开手机上用得到的（`build-ffmpeg.sh` 里的清单）：

- 协议：file、pipe、http、https、httpproxy、tls、tcp、crypto、data、concat、subfile
- 解封装：mov（MP4 / M4A）、matroska（MKV / WebM）、mpegts、hls、flv、aac、mp3、ogg、wav、flac、webvtt、srt、ass、concat 等
- 封装：mp4、mov、ipod、matroska、webm、mpegts、adts、mp3、ogg、opus、flac、wav、webvtt、srt、ass、image2、mjpeg 等
- 解码：H.264、HEVC、VP8、VP9、AV1（libdav1d）、MPEG-4、MPEG-2、MJPEG、PNG、WebP、GIF、AAC、MP3、Opus、Vorbis、FLAC、ALAC、AC-3、E-AC-3、PCM、字幕
- 编码：AAC、MP3（libmp3lame）、MJPEG、PNG、PCM、FLAC、字幕、wrapped_avframe（`-f null` 用）。没有视频编码器
- 全部比特流过滤器（含 `aac_adtstoasc`）、缩略图和波形用的滤镜（scale、fps、select、signalstats、aresample 等）
- DASH 解封装要 libxml2，没开（yt-dlp 自己下 DASH，用不到 ffmpeg 的）
