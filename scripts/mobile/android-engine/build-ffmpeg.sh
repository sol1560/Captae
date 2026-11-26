#!/usr/bin/env bash
# 用 NDK 从源码编 FFmpeg 的 LGPL 构建（ffmpeg、ffprobe 和它们的动态库），给 Android 完整版用。
#
# 许可证：不开 --enable-gpl、--enable-nonfree，不带 x264、x265 等 GPL 库。https 用 OpenSSL 3（Apache-2.0），
# 和 Python 包里的是同一份（libssl_python.so、libcrypto_python.so，不重复打包）。Apache-2.0 和 LGPL 2.1 能不能放在一起有争议，
# 所以加 --enable-version3，整个构建按 LGPL 3.0 或更新版本授权（configure 最后会打印 License: LGPL version 3 or later，脚本会核对）。
# 另外链接两个非 GPL 的库（build-codecs.sh 编的静态库，链进 libavcodec）：LAME（MP3 编码，LGPL-2.0+）、dav1d（AV1 解码，BSD-2）。
# 源码地址和这个脚本就是 LGPL 要求提供的「对应源码和编译方法」，见 licenses/ffmpeg-android/。
#
# 只开手机上用得到的：
#   合并、转封装（-c copy）：MP4 / MOV / M4A、MKV / WebM、MPEG-TS、HLS、FLV、MP3、AAC、Ogg / Opus、FLAC、WAV、字幕（VTT、SRT、ASS、mov_text）
#   读取信息（ffprobe）、缩略图（解码 H.264 / HEVC / VP8 / VP9 / MPEG-4 / MJPEG / PNG / WebP 一帧、缩放、存 JPEG）
#   编码：AAC（声音要转成 AAC 时）、MP3（libmp3lame，「只要声音」出 MP3）、JPEG / PNG（缩略图）、PCM（波形）、字幕格式互转、wrapped_avframe（-f null 用）
#   AV1 解码（libdav1d）：AV1 视频也能生成缩略图
#   aac_adtstoasc 等全部比特流过滤器（都很小）
# 不开：视频编码器（转换、剪辑手机上不做）、MediaCodec（要 JVM，子进程里用不了）。
#
# 产物：target/android-engine/build/<ABI>/ffmpeg/prefix/{bin/ffmpeg,bin/ffprobe,lib/lib*_captae.so}
# 用法：scripts/mobile/android-engine/build-ffmpeg.sh   编过、配置没变的跳过；--force 重编
source "$(dirname "$0")/env.sh"

FORCE="${1:-}"
SRC="$WORK/src/ffmpeg-$FFMPEG_VERSION"
if [[ ! -f "$SRC/configure" ]]; then
  mkdir -p "$WORK/src"
  tar -xJf "$WORK/dl/ffmpeg/ffmpeg-$FFMPEG_VERSION.tar.xz" -C "$WORK/src"
fi

join() { local IFS=,; echo "$*"; }

# 把文件里这台电脑上的路径换成占位（NDK → <ndk>，工作目录 → /build，仓库 → /captae，主目录 → ~）
sanitize() {
  local f="$1"
  LC_ALL=C sed -i '' -e "s#$TOOLCHAIN/bin/#<ndk>/#g" -e "s#$WORK#/build#g" -e "s#$ROOT#/captae#g" -e "s#$HOME#~#g" "$f" 2>/dev/null ||
    LC_ALL=C sed -i -e "s#$TOOLCHAIN/bin/#<ndk>/#g" -e "s#$WORK#/build#g" -e "s#$ROOT#/captae#g" -e "s#$HOME#~#g" "$f"
}

PROTOCOLS=(file pipe http https httpproxy tls tcp crypto data concat subfile)
DEMUXERS=(mov matroska mpegts hls dash flv aac mp3 ogg wav flac webvtt srt ass concat ffmetadata image2 image_jpeg_pipe image_png_pipe
  image_webp_pipe mjpeg h264 hevc m4v mpegvideo ac3 eac3 data)
MUXERS=(mp4 mov ipod matroska webm mpegts adts mp3 ogg opus flac wav webvtt srt ass ffmetadata image2 mjpeg null s16le f32le)
DECODERS=(h264 hevc vp8 vp9 mpeg4 mpeg2video mjpeg png webp gif aac aac_latm mp3float mp3 opus vorbis flac alac ac3 eac3
  pcm_s16le pcm_s16be pcm_s24le pcm_f32le pcm_u8 webvtt subrip ass ssa mov_text libdav1d)
# wrapped_avframe：-f null 要它（captae-core 扫场景挑缩略图、完整解码检查都用 -f null -）
ENCODERS=(aac libmp3lame mjpeg png pcm_s16le pcm_f32le flac webvtt subrip ass ssa mov_text wrapped_avframe)
PARSERS=(h264 hevc aac aac_latm mpegaudio opus vorbis vp8 vp9 av1 mjpeg png webp flac ac3 mpeg4video mpegvideo)
FILTERS=(scale format fps select signalstats metadata null anull aresample aformat apad atrim trim setpts asetpts concat anullsrc
  crop pad transpose hflip vflip rotate copy acopy)

CONF=(
  --target-os=android --enable-cross-compile
  --enable-shared --disable-static --build-suffix=_captae
  --enable-version3
  --disable-autodetect --enable-zlib --enable-openssl --enable-libmp3lame --enable-libdav1d
  --disable-doc --disable-ffplay --disable-avdevice --disable-debug
  --disable-everything
  --enable-protocol="$(join "${PROTOCOLS[@]}")"
  --enable-demuxer="$(join "${DEMUXERS[@]}")"
  --enable-muxer="$(join "${MUXERS[@]}")"
  --enable-decoder="$(join "${DECODERS[@]}")"
  --enable-encoder="$(join "${ENCODERS[@]}")"
  --enable-parser="$(join "${PARSERS[@]}")"
  --enable-filter="$(join "${FILTERS[@]}")"
  --enable-bsfs
  --optflags=-O2
)

for abi in "${ABIS[@]}"; do
  use_abi "$abi"
  OUT="$WORK/build/$abi/ffmpeg"
  PY="$WORK/build/$abi/python/prefix"
  if [[ ! -f "$PY/lib/libssl_python.so" ]]; then
    echo "先跑 build-python.sh（ffmpeg 要链接 Python 包里的 OpenSSL）" >&2
    exit 2
  fi
  CODECS="$WORK/build/$abi/codecs/prefix"
  if [[ ! -f "$CODECS/lib/libmp3lame.a" || ! -f "$CODECS/lib/libdav1d.a" ]]; then
    echo "先跑 build-codecs.sh（LAME、dav1d）" >&2
    exit 2
  fi
  case "$abi" in
    arm64-v8a) ARCH=(--arch=aarch64 --cpu=armv8-a) ;;
    # 本机没装 nasm 时 x86_64 不用汇编优化（只给模拟器用）
    x86_64) ARCH=(--arch=x86_64); command -v nasm >/dev/null || ARCH+=(--disable-x86asm) ;;
  esac
  # OpenSSL 用 Python 包里那份：写一个 pkg-config 文件指过去
  PC="$OUT/pkgconfig"
  mkdir -p "$PC"
  cat >"$PC/openssl.pc" <<EOF
Name: openssl
Description: OpenSSL from the python.org Android package
Version: 3.5.9
Libs: -L$PY/lib -lssl_python -lcrypto_python
Cflags: -I$PY/include
EOF
  # 程序和动态库都带 RUNPATH ${ORIGIN}：互相依赖的库（libavformat → libssl_python.so）在同一个目录里就能找到，
  # 不靠 LD_LIBRARY_PATH（yt-dlp 起 ffmpeg 时一般会继承，但不保证）
  # configure 会吃掉参数里的 $，所以 rpath 不经过它：configure 完往 config.mak 末尾追加一行（见下面）
  LDFLAGS_EXTRA="$PAGE_LDFLAGS -Wl,--hash-style=gnu"
  # 程序的链接命令展开一次（$$ → $），库的链接命令在 define + eval 里展开两次（$$$$ → $），所以分开写
  RPATH_LINE='LDEXEFLAGS += -Wl,-rpath,\$$ORIGIN
LDSOFLAGS += -Wl,-rpath,\$$$$ORIGIN'
  STAMP="$(printf '%s\n' "$FFMPEG_VERSION" "$LAME_VERSION" "$DAV1D_VERSION" "${CONF[@]}" "${ARCH[@]}" "$LDFLAGS_EXTRA" "$RPATH_LINE" | shasum -a 256 | cut -c1-16)"
  if [[ -f "$OUT/prefix/bin/ffmpeg" && "$FORCE" != "--force" && "$(cat "$OUT/STAMP" 2>/dev/null)" == "$STAMP" ]]; then
    echo "FFmpeg $abi 已经编过：$OUT/prefix"
    continue
  fi
  echo "编 FFmpeg ${FFMPEG_VERSION}（${abi}）"
  rm -rf "$OUT/build" "$OUT/prefix"
  mkdir -p "$OUT/build"
  (
    cd "$OUT/build" || exit 1
    PKG_CONFIG_PATH="$PC:$CODECS/lib/pkgconfig" PKG_CONFIG_LIBDIR="$PC:$CODECS/lib/pkgconfig" "$SRC/configure" --prefix="$OUT/prefix" \
      --cc="$CC" --cxx="$CXX" --ar="$AR" --nm="$NM" --ranlib="$RANLIB" --strip="$STRIP" --pkg-config=pkg-config \
      "${ARCH[@]}" "${CONF[@]}" \
      --extra-cflags="$PREFIX_MAP -I$CODECS/include" \
      --extra-ldflags="$LDFLAGS_EXTRA -L$CODECS/lib" >configure.log 2>&1 || { tail -30 configure.log; exit 1; }
    # make 展开掉多余的 $，shell 再把 \$ 变成字面的 $：链接器拿到的是 -rpath,$ORIGIN
    echo "$RPATH_LINE" >>ffbuild/config.mak
    # ffmpeg -version 会打印 configure 的参数：把里面这台电脑的路径换掉
    sanitize config.h
    grep -q "License: LGPL version 3 or later" configure.log || { echo "configure 报的许可证不是 LGPL 3：" >&2; grep License configure.log >&2; exit 5; }
    make -j"$(sysctl -n hw.ncpu 2>/dev/null || nproc)" >make.log 2>&1 || { tail -40 make.log; exit 1; }
    make install >install.log 2>&1
  )
  "$STRIP" --strip-unneeded "$OUT"/prefix/bin/ffmpeg "$OUT"/prefix/bin/ffprobe "$OUT"/prefix/lib/*.so
  for f in "$OUT"/prefix/bin/ffmpeg "$OUT"/prefix/lib/libavformat_captae.so; do
    if ! "$READELF" -d "$f" | grep -q 'RUNPATH.*\[\$ORIGIN\]'; then
      echo "$(basename "$f") 的 RUNPATH 不是 \${ORIGIN}：" >&2
      "$READELF" -d "$f" | grep -E 'RUNPATH|RPATH' >&2
      exit 6
    fi
  done
  for f in "$OUT"/prefix/bin/ffmpeg "$OUT"/prefix/bin/ffprobe "$OUT"/prefix/lib/*.so; do
    if LC_ALL=C grep -a -q -F -- "$HOME" "$f"; then
      echo "编出来的文件里还有本机路径：$f" >&2
      LC_ALL=C strings -a "$f" | grep -F -- "$HOME" | head -3 >&2
      exit 3
    fi
  done
  echo "$STAMP" >"$OUT/STAMP"
  for f in "$OUT"/prefix/bin/ffmpeg "$OUT"/prefix/bin/ffprobe "$OUT"/prefix/lib/*.so; do
    echo "  $(mb "$(size_of "$f")")  $(basename "$f")"
  done
done
