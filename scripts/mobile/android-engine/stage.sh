#!/usr/bin/env bash
# 编 yt-dlp 启动器（libytdlp.so），把各组件整理成 Gradle 模块 apps/android/engine 要的样子：
#
#   target/android-engine/stage/jniLibs/<ABI>/      原生库目录（安装时解压出来，能执行）
#     libytdlp.so              启动器（apps/android/engine/src/main/native/ytdlp_launcher.c），链接 libpython3.14.so
#     libpython3.14.so、libssl_python.so、libcrypto_python.so、libsqlite3_python.so   官方 Python 包原样
#     libpy_<模块>.so           官方包 lib-dynload 里的扩展模块改名（_ssl、zlib……；测试用的去掉），captae_boot.py 按这个名字加载
#     libqjs.so                QuickJS
#     libffmpeg.so、libffprobe.so、lib{avcodec,avformat,avfilter,avutil,swscale,swresample}_captae.so   FFmpeg（LGPL）
#   target/android-engine/stage/assets/captae-engine/
#     engine.zip               首次启动解压到 App 私有目录的数据：lib/python3.14/（标准库 + captae_boot.py）、yt-dlp（zipapp）
#     engine.json              版本、engine.zip 的 SHA256（App 据此判断要不要重新解压）
#
# 用法：scripts/mobile/android-engine/stage.sh   （先跑 fetch、build-python、build-quickjs、build-ffmpeg，或者直接跑 build.sh）
source "$(dirname "$0")/env.sh"

ENGINE_SRC="$ROOT/apps/android/engine/src/main"
SKIP_MODULES='^(_test.*|_xxtestfuzz|xxlimited.*|xxsubtype|_ctypes_test|_remote_debugging)$'

rm -rf "$STAGE"
mkdir -p "$STAGE/assets/captae-engine"

for abi in "${ABIS[@]}"; do
  use_abi "$abi"
  PY="$WORK/build/$abi/python/prefix"
  FF="$WORK/build/$abi/ffmpeg/prefix"
  QJS="$WORK/build/$abi/quickjs/qjs"
  J="$STAGE/jniLibs/$abi"
  mkdir -p "$J"
  for need in "$PY/lib/libpython$PY_MAJOR.so" "$FF/bin/ffmpeg" "$QJS"; do
    [[ -f "$need" ]] || { echo "缺少 ${need}：先跑 build.sh" >&2; exit 2; }
  done

  echo "启动器（${abi}）"
  # 直接链接 OpenSSL、SQLite：扩展模块（libpy__ssl.so 等）被 dlopen 时它们已经在进程里，不靠 LD_LIBRARY_PATH 找
  # shellcheck disable=SC2086
  "$CC" -O2 -fPIE -pie -Wall -Werror $PREFIX_MAP -I"$PY/include/python$PY_MAJOR" \
    -o "$J/libytdlp.so" "$ENGINE_SRC/native/ytdlp_launcher.c" \
    -L"$PY/lib" -Wl,--no-as-needed -lpython"$PY_MAJOR" -lssl_python -lcrypto_python -lsqlite3_python -Wl,--as-needed \
    $PAGE_LDFLAGS '-Wl,-rpath,$ORIGIN'
  "$STRIP" --strip-unneeded "$J/libytdlp.so"

  # Python
  for f in "libpython$PY_MAJOR.so" libssl_python.so libcrypto_python.so libsqlite3_python.so; do
    cp "$PY/lib/$f" "$J/$f"
  done
  n=0
  for m in "$PY/lib/python$PY_MAJOR/lib-dynload/"*.so; do
    name="$(basename "$m")"
    name="${name%%.*}"
    if [[ "$name" =~ $SKIP_MODULES ]]; then
      continue
    fi
    cp "$m" "$J/libpy_$name.so"
    n=$((n + 1))
  done
  echo "  扩展模块 $n 个"

  # QuickJS、FFmpeg
  cp "$QJS" "$J/libqjs.so"
  cp "$FF/bin/ffmpeg" "$J/libffmpeg.so"
  cp "$FF/bin/ffprobe" "$J/libffprobe.so"
  cp "$FF"/lib/lib*_captae.so "$J/"
  chmod 755 "$J"/*.so

  # 有没有这台电脑的路径
  for f in "$J"/*.so; do
    if LC_ALL=C grep -a -q -F -- "$HOME" "$f"; then
      echo "还有本机路径：$f" >&2
      LC_ALL=C strings -a "$f" | grep -F -- "$HOME" | head -3 >&2
      exit 3
    fi
  done
done

echo "数据包（标准库 + yt-dlp ${YTDLP_VERSION}）"
T="$WORK/build/engine-data"
rm -rf "$T"
mkdir -p "$T/lib"
cp -R "$WORK/build/stdlib/lib/python$PY_MAJOR" "$T/lib/"
cp "$ENGINE_SRC/python/captae_boot.py" "$T/lib/python$PY_MAJOR/"
cp "$WORK/dl/yt-dlp/yt-dlp" "$T/yt-dlp"
# 文件时间统一，同样的输入打出同样的 zip
find "$T" -exec touch -h -t 202601010000 {} +
(cd "$T" && find . -type f | LC_ALL=C sort | sed 's#^\./##' | zip -q -X -9 -@ "$STAGE/assets/captae-engine/engine.zip")
ZIP_SHA="$(sha256_of "$STAGE/assets/captae-engine/engine.zip")"
cat >"$STAGE/assets/captae-engine/engine.json" <<EOF
{
  "python": "$PY_VERSION",
  "ytdlp": "$YTDLP_VERSION",
  "quickjs": "$QJS_VERSION",
  "ffmpeg": "$FFMPEG_VERSION",
  "zipSha256": "$ZIP_SHA"
}
EOF

echo "整理好了：$STAGE"
for abi in "${ABIS[@]}"; do
  total=0
  for f in "$STAGE/jniLibs/$abi"/*.so; do total=$((total + $(size_of "$f"))); done
  echo "  jniLibs/${abi}：$(ls "$STAGE/jniLibs/$abi" | wc -l | tr -d ' ') 个文件，解压后 $(mb "$total")"
done
echo "  engine.zip：$(mb "$(size_of "$STAGE/assets/captae-engine/engine.zip")")"
