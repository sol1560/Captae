#!/usr/bin/env bash
# 解开 python.org 官方 Android 嵌入包，整理出打包要用的两样：
#   build/<ABI>/python/prefix/       官方包原样（libpython3.14.so、OpenSSL、SQLite、lib-dynload 的扩展模块、头文件）
#   build/stdlib/lib/python3.14/     纯 Python 的标准库（两个 ABI 共用，打进 APK 的资源、首次启动解压），去掉用不上的部分
#
# 去掉的（yt-dlp 和启动脚本都用不到，合计约 42 MB）：test（37 MB）、idlelib、tkinter、turtle*、ensurepip、venv、pydoc_data、
# _pyrepl（交互界面）、config-3.14-*（编译扩展用的）、lib-dynload（扩展模块另外放进原生库目录，见 stage.sh）、site-packages、__phello__。
# 两个 ABI 的 _sysconfigdata 都留着（各几十 KB）。
#
# 用法：scripts/mobile/android-engine/build-python.sh   已经整理过的跳过；--force 重做
source "$(dirname "$0")/env.sh"

FORCE="${1:-}"
STD="$WORK/build/stdlib/lib/python$PY_MAJOR"

for abi in "${ABIS[@]}"; do
  arch="$(py_arch_of "$abi")"
  OUT="$WORK/build/$abi/python"
  if [[ -f "$OUT/prefix/lib/libpython$PY_MAJOR.so" && "$FORCE" != "--force" && "$(cat "$OUT/VERSION" 2>/dev/null)" == "$PY_VERSION" ]]; then
    echo "Python $abi 已经解开：$OUT/prefix"
    continue
  fi
  echo "解开 Python ${PY_VERSION}（${abi}）"
  rm -rf "$OUT"
  mkdir -p "$OUT"
  tar -xzf "$WORK/dl/python/python-$PY_VERSION-$arch-linux-android.tar.gz" -C "$OUT" prefix README.md
  echo "$PY_VERSION" >"$OUT/VERSION"
done

if [[ -f "$STD/os.py" && "$FORCE" != "--force" && "$(cat "$WORK/build/stdlib/VERSION" 2>/dev/null)" == "$PY_VERSION" ]]; then
  echo "标准库已经整理过：$STD"
  exit 0
fi
echo "整理标准库"
rm -rf "$WORK/build/stdlib"
mkdir -p "$STD"
# 纯 Python 部分两个 ABI 一样，从 arm64 那份拷
SRC="$WORK/build/arm64-v8a/python/prefix/lib/python$PY_MAJOR"
rsync -a \
  --exclude '/test/' --exclude '/idlelib/' --exclude '/tkinter/' --exclude '/turtledemo/' --exclude '/turtle.py' \
  --exclude '/ensurepip/' --exclude '/venv/' --exclude '/pydoc_data/' --exclude '/_pyrepl/' --exclude '/config-*/' \
  --exclude '/lib-dynload/' --exclude '/site-packages/' --exclude '/__phello__/' --exclude '__pycache__/' \
  "$SRC/" "$STD/"
# 另一个 ABI 的 sysconfigdata
for abi in "${ABIS[@]}"; do
  cp "$WORK/build/$abi/python/prefix/lib/python$PY_MAJOR/"_sysconfig*android* "$STD/"
done
echo "$PY_VERSION" >"$WORK/build/stdlib/VERSION"
echo "  标准库 $(du -sh "$STD" | awk '{print $1}')（$(find "$STD" -type f | wc -l | tr -d ' ') 个文件）"
