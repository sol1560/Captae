#!/usr/bin/env bash
# 用 NDK 从源码编 FFmpeg 要链接的两个编解码库（静态库，-fPIC，链进 libavcodec_captae.so，不多出 .so）：
#   LAME 3.100   MP3 编码（「只要声音」出 MP3），LGPL-2.0-or-later
#   dav1d 1.5.5  AV1 解码（AV1 视频生成缩略图），BSD-2-Clause
# 两个都不是 GPL，FFmpeg 整体还是 LGPL（build-ffmpeg.sh 最后核对 configure 报的许可证）。
#
# 产物：target/android-engine/build/<ABI>/codecs/prefix/{include,lib/libmp3lame.a,lib/libdav1d.a,lib/pkgconfig/dav1d.pc}
# 需要：make、ninja、meson（没有时装进 target/android-engine/venv-tools，版本写死在 env.sh）
# 用法：scripts/mobile/android-engine/build-codecs.sh   编过、版本没变的跳过；--force 重编
source "$(dirname "$0")/env.sh"

FORCE="${1:-}"
mkdir -p "$WORK/src"
LAME_SRC="$WORK/src/lame-$LAME_VERSION"
DAV1D_SRC="$WORK/src/dav1d-$DAV1D_VERSION"
[[ -f "$LAME_SRC/configure" ]] || tar -xzf "$WORK/dl/lame/lame-$LAME_VERSION.tar.gz" -C "$WORK/src"
[[ -f "$DAV1D_SRC/meson.build" ]] || tar -xJf "$WORK/dl/dav1d/dav1d-$DAV1D_VERSION.tar.xz" -C "$WORK/src"

command -v ninja >/dev/null || { echo "缺少 ninja（brew install ninja）" >&2; exit 2; }
MESON="$(command -v meson || true)"
if [[ -z "$MESON" ]]; then
  MESON="$WORK/venv-tools/bin/meson"
  if [[ ! -x "$MESON" ]]; then
    echo "装 meson $MESON_VERSION 到 $WORK/venv-tools"
    python3 -m venv "$WORK/venv-tools"
    "$WORK/venv-tools/bin/pip" install -q "meson==$MESON_VERSION"
  fi
fi

JOBS="$(sysctl -n hw.ncpu 2>/dev/null || nproc)"

for abi in "${ABIS[@]}"; do
  use_abi "$abi"
  OUT="$WORK/build/$abi/codecs"
  P="$OUT/prefix"
  STAMP="lame-$LAME_VERSION dav1d-$DAV1D_VERSION api-$API"
  if [[ -f "$P/lib/libmp3lame.a" && -f "$P/lib/libdav1d.a" && "$FORCE" != "--force" && "$(cat "$OUT/STAMP" 2>/dev/null)" == "$STAMP" ]]; then
    echo "LAME、dav1d $abi 已经编过：$P"
    continue
  fi
  rm -rf "$OUT"
  mkdir -p "$OUT"

  echo "编 LAME ${LAME_VERSION}（${abi}）"
  mkdir -p "$OUT/lame"
  (
    cd "$OUT/lame" || exit 1
    # 只要库：不编命令行程序（--disable-frontend）和 MP3 解码（--disable-decoder，FFmpeg 自己会解）
    CC="$CC" AR="$AR" RANLIB="$RANLIB" CFLAGS="-O2 -fPIC $PREFIX_MAP" \
      "$LAME_SRC/configure" --host="$TRIPLE" --prefix="$P" --enable-static --disable-shared \
      --disable-frontend --disable-decoder --disable-gtktest >configure.log 2>&1 || { tail -30 configure.log; exit 1; }
    make -j"$JOBS" >make.log 2>&1 || { tail -40 make.log; exit 1; }
    make install >install.log 2>&1
  )

  echo "编 dav1d ${DAV1D_VERSION}（${abi}）"
  case "$abi" in
    arm64-v8a) CPU_FAMILY=aarch64; CPU=armv8; ASM=true ;;
    # x86_64 的汇编要 nasm；本机没有时关掉（x86_64 只给模拟器用）
    x86_64) CPU_FAMILY=x86_64; CPU=x86_64; if command -v nasm >/dev/null; then ASM=true; else ASM=false; fi ;;
  esac
  cat >"$OUT/cross.ini" <<EOF
[binaries]
c = '$CC'
ar = '$AR'
strip = '$STRIP'

[built-in options]
c_args = ['-fPIC', $(for f in $PREFIX_MAP; do printf "'%s', " "$f"; done)]

[host_machine]
system = 'android'
cpu_family = '$CPU_FAMILY'
cpu = '$CPU'
endian = 'little'
EOF
  "$MESON" setup "$OUT/dav1d" "$DAV1D_SRC" --cross-file "$OUT/cross.ini" --prefix "$P" --libdir lib \
    --buildtype release -Ddefault_library=static -Denable_tools=false -Denable_tests=false -Denable_examples=false \
    -Denable_asm="$ASM" -Dbitdepths=8,16 >"$OUT/dav1d-setup.log" 2>&1 || { tail -30 "$OUT/dav1d-setup.log"; exit 1; }
  ninja -C "$OUT/dav1d" -j "$JOBS" install >"$OUT/dav1d-build.log" 2>&1 || { tail -40 "$OUT/dav1d-build.log"; exit 1; }

  echo "$STAMP" >"$OUT/STAMP"
  echo "  libmp3lame.a $(mb "$(size_of "$P/lib/libmp3lame.a")")、libdav1d.a $(mb "$(size_of "$P/lib/libdav1d.a")")（静态库，链进 libavcodec 后会小得多）"
done
