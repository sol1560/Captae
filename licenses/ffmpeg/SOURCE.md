# FFmpeg 9.0.2

- 许可证：GPL-3.0-or-later（三个平台的构建参数都含 `--enable-gpl --enable-version3`），全文见 `COPYING.GPLv3`
- 上游源码：https://ffmpeg.org/releases/ffmpeg-9.0.2.tar.xz
- 三个平台的二进制都是静态构建（依赖库都链在 ffmpeg / ffprobe 里），带 libass（字幕烧录要用）、x264、x265 等 GPL 组件，整体按 GPL-3.0-or-later 分发。
  Captae 只用子进程调用它（转换、剪辑、字幕烧录、读媒体信息），不链接它的代码。

## macOS（arm64）

- `Captae.app/Contents/MacOS/ffmpeg`、`ffprobe`：https://ffmpeg.martin-riedl.de/ 的 arm64 静态构建
  （构建编号 1789931890_9.0.2，下载时校验了 sha256，`scripts/fetch-sidecars.sh`）
- 构建参数和所含依赖库的版本清单：
  https://ffmpeg.martin-riedl.de/download/macos/arm64/1789931890_9.0.2/versions.txt
- 构建脚本（含所有依赖库的源码地址）：https://git.martin-riedl.de/ffmpeg/build-script
- 这份构建启用的库（`ffmpeg -version` 的 configuration）：libass、freetype、fontconfig、harfbuzz、libxml2、libbluray、snappy、srt、vmaf、
  libklvanc、zimg、zvbi、aom、dav1d、openh264、openjpeg、rav1e、svt-av1、libvpx、vvenc、libwebp、x264、x265、lame（mp3）、opus、vorbis、theora、OpenSSL

## Windows（x64）和 Linux（x86_64）

- Windows 安装目录里的 `ffmpeg.exe`、`ffprobe.exe`，Linux 的 `/usr/lib/Captae/ffmpeg`、`ffprobe`：
  BtbN/FFmpeg-Builds 的 GPL 静态构建，n9.0 分支的 `n9.0.2-39-g7adfbe40dc`（9.0.2 之后 release/9.0 分支上的 39 个修复提交），
  2026-10-31 的月度自动构建（release 标签 `autobuild-2026-10-31-12-59`，对应构建脚本仓库的提交 `5031c6f1082414e53688d39e521849905071070d`）：
  - Windows：`ffmpeg-n9.0.2-39-g7adfbe40dc-win64-gpl-9.0.zip`，SHA256 `693f3e224bc184b40524a62ca47526cfebaf2504414021b92fe75f677ad868b9`
  - Linux：`ffmpeg-n9.0.2-39-g7adfbe40dc-linux64-gpl-9.0.tar.xz`，SHA256 `73e29a56974ae5a651133a04584c7483b21afe941e17dc91d79814a758b608d3`
  - 下载页：https://github.com/BtbN/FFmpeg-Builds/releases/tag/autobuild-2026-10-31-12-59 （`scripts/fetch-sidecars-windows.ps1`、`scripts/fetch-sidecars-linux.sh` 核对上面的值和 release 里的 checksums.sha256）
- FFmpeg 源码（这个版本）：https://github.com/FFmpeg/FFmpeg/tree/7adfbe40dc
- 构建脚本（MIT，含每个依赖库的源码地址和版本）：https://github.com/BtbN/FFmpeg-Builds/tree/5031c6f1082414e53688d39e521849905071070d
- 这份构建启用的库（`ffmpeg -buildconf`）：libass、freetype、fontconfig、fribidi、harfbuzz、libxml2、libbluray、libdvdnav、libdvdread、
  aom、dav1d、kvazaar、openh264、openjpeg、rav1e、svt-av1、libvpx、vvenc、uavs3d、oapv、lcevc-dec、x264、x265、xvid、libwebp、libjxl、
  lame（mp3）、opus、vorbis、theora、twolame、opencore-amr、libgme、libopenmpt、soxr、rubberband、vidstab、zimg、zvbi、aribb24、aribcaption、
  chromaprint、frei0r、lv2、lcms2、librsvg、snappy、srt、rist、libssh、zmq、vmaf、libplacebo、Vulkan、OpenCL、AMF、NVENC / CUDA（ffnvcodec）、
  VAAPI、libdrm、xcb、xlib、PulseAudio、OpenAL、SDL2、GMP、OpenSSL、zlib、lzma、iconv。
  安装包只带 ffmpeg 和 ffprobe，不带 ffplay。
