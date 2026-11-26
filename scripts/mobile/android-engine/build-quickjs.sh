#!/usr/bin/env bash
# 用 NDK 从源码编 QuickJS 的 qjs（yt-dlp 解 YouTube 的 JS 验证用，--js-runtimes quickjs:<路径>）。
# 产物：target/android-engine/build/<ABI>/quickjs/qjs（去符号），stage.sh 改名成 libqjs.so 放进 jniLibs。
#
# 做法：先用本机的 cc 编一个 qjsc，把 repl.js 编成 repl.c（qjs 里带的交互界面，字节码和架构无关、只看字长和字节序，
# 这里两边都是 64 位小端）；再用 NDK 的 clang 编 qjs。不用 Makefile 的交叉编译（它要 gcc 当本机编译器）。
#
# 用法：scripts/mobile/android-engine/build-quickjs.sh   已经编过（源码版本没变）的跳过；--force 重编
source "$(dirname "$0")/env.sh"

FORCE="${1:-}"
SRC="$WORK/src/quickjs-$QJS_VERSION"
if [[ ! -f "$SRC/quickjs.c" ]]; then
  mkdir -p "$WORK/src"
  tar -xJf "$WORK/dl/quickjs/quickjs-$QJS_VERSION.tar.xz" -C "$WORK/src"
fi

# 本机的 qjsc → repl.c
HOST="$WORK/build/host/quickjs"
mkdir -p "$HOST"
if [[ ! -f "$HOST/repl.c" || "$FORCE" == "--force" ]]; then
  echo "本机编 qjsc，生成 repl.c"
  (
    cd "$SRC" || exit 1
    cc -O2 -D_GNU_SOURCE -DCONFIG_VERSION="\"$QJS_VERSION\"" -DCONFIG_CC='"cc"' -DCONFIG_PREFIX='"/usr/local"' -fwrapv -w \
      -o "$HOST/qjsc" qjsc.c quickjs.c dtoa.c libregexp.c libunicode.c cutils.c quickjs-libc.c -lm -lpthread
    "$HOST/qjsc" -s -c -o "$HOST/repl.c" -m repl.js
  )
fi

for abi in "${ABIS[@]}"; do
  use_abi "$abi"
  OUT="$WORK/build/$abi/quickjs"
  if [[ -f "$OUT/qjs" && "$FORCE" != "--force" && "$(cat "$OUT/VERSION" 2>/dev/null)" == "$QJS_VERSION" ]]; then
    echo "QuickJS $abi 已经编过：$OUT/qjs"
    continue
  fi
  echo "编 QuickJS（${abi}）"
  mkdir -p "$OUT"
  # -O2：解 YouTube 的验证脚本时速度要紧（-Os 慢得多）；-fPIE / -pie：Android 只认位置无关的可执行文件
  # shellcheck disable=SC2086
  (
    cd "$SRC" || exit 1
    "$CC" -O2 -fwrapv -fPIE -pie -D_GNU_SOURCE -DCONFIG_VERSION="\"$QJS_VERSION\"" $PREFIX_MAP -w \
      -ffunction-sections -fdata-sections -Wl,--gc-sections $PAGE_LDFLAGS \
      -o "$OUT/qjs" qjs.c "$HOST/repl.c" quickjs.c dtoa.c libregexp.c libunicode.c cutils.c quickjs-libc.c -lm -ldl
  )
  "$STRIP" --strip-unneeded "$OUT/qjs"
  echo "$QJS_VERSION" >"$OUT/VERSION"
  echo "  $(mb "$(size_of "$OUT/qjs")")  $OUT/qjs"
done
