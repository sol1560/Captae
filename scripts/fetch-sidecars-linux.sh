#!/usr/bin/env bash
# 准备 Linux 安装包（AppImage、deb）附带的外部程序。照 scripts/fetch-sidecars.sh（macOS）的做法：版本写死、核对官方校验值和签名。
#
#   yt-dlp         官方 yt-dlp_linux.zip 2026.10.26（PyInstaller 目录版，自带 Python）。核对官方 SHA2-256SUMS，
#                  并用 yt-dlp 官方公钥（crates/captae-core/assets/yt-dlp-public.key）验 SHA2-256SUMS.sig（Linux 上必须有 gpg）。
#                  目录版比单文件版启动快：单文件版每次启动都要把自己解压到 /tmp
#   deno           官方 deno-<架构>-unknown-linux-gnu.zip 2.9.8（MIT），核对写死的 SHA256 和官方 .sha256sum
#   ffmpeg/ffprobe BtbN/FFmpeg-Builds 的 GPL 构建（带 libass），月末那次自动构建（BtbN 长期保留月末的构建，其余的只留几周），
#                  n9.0.2（和 macOS 版的 9.0.2 对齐）。只依赖 glibc 2.28 以上，核对写死的 SHA256 和 release 里的 checksums.sha256
#   aria2c         官方没有 Linux 二进制。在这台机器（Ubuntu 22.04）上从源码编：aria2 1.37.0，OpenSSL 3.5（LTS）、c-ares、
#                  zlib、expat、SQLite 都用官方源码包编成静态库链进去，只动态依赖 glibc。源码包都核对写死的校验值。
#                  OpenSSL 用 no-module 编：aria2 一启动就加载 legacy 提供者，no-module 时它编进 libcrypto，不用另带模块文件
#                  （macOS 版就是因为模块文件在 Homebrew 里，没装 Homebrew 的 Mac 上起不来）。
#                  OpenSSL 的配置目录设成 /etc/ssl（Debian / Ubuntu 的证书目录 /etc/ssl/certs 直接能用）；别的发行版由 Captae
#                  启动 aria2c 时用 SSL_CERT_FILE 指向系统的证书包（crates/captae-core/src/tools.rs 的 ca_bundle）
#   captae         本仓库的命令行（插件宿主、MCP 服务器）
#
# 放在 apps/desktop/src-tauri/binaries/linux/ 下，打包时作为 Tauri 的 resources 平铺进 /usr/lib/Captae/（Tauri 在 Linux 上的资源目录）：
#   bin/{captae,ffmpeg,ffprobe,aria2c,deno}   → /usr/lib/Captae/{captae,ffmpeg,ffprobe,aria2c,deno}
#   yt-dlp/{yt-dlp_linux,_internal/}          → /usr/lib/Captae/yt-dlp/
# 平铺是为了和核心库的查找规则对上：命令行 /usr/lib/Captae/captae 的 sidecar 目录就是 /usr/lib/Captae，
# 旁边的 ffmpeg、aria2c 和 yt-dlp/yt-dlp_linux 都能找到（crates/captae-core/src/tools.rs）。
# 不用 Tauri 的 externalBin：deb 会把 externalBin 装成 /usr/bin/ffmpeg、/usr/bin/aria2c……，和系统的 ffmpeg、aria2 包抢同一个文件，
# dpkg 直接拒绝安装（tauri-bundler 2.9.4 的 debian.rs：externalBin 一律复制到 usr/bin）。
#
# 用法：scripts/fetch-sidecars-linux.sh [--force]     （已下载过的会跳过；--force 全部重新准备）
# 下载的原始文件缓存在 target/sidecar-cache/（CAPTAE_SIDECAR_CACHE 可改），aria2 的编译结果也在那里，换版本才重编。
# 需要：curl、unzip、xz、gpg、python3、gcc/g++/make、perl、pkg-config（Ubuntu：apt install build-essential curl unzip xz-utils gnupg perl pkg-config python3）
# 只支持 x86_64（正式包）和 aarch64（只用来在 Apple Silicon 的 Docker 里验证脚本）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
OUT="$ROOT/apps/desktop/src-tauri/binaries/linux"
CACHE="${CAPTAE_SIDECAR_CACHE:-$ROOT/target/sidecar-cache}"
FORCE=0
[[ "${1:-}" == "--force" ]] && FORCE=1

if [[ "$(uname -s)" != Linux ]]; then
  echo "只能在 Linux 上准备 Linux 版的附带程序（在 Mac 上用 Docker：见 scripts/build-release-linux.sh 开头的说明）" >&2
  exit 1
fi
ARCH="$(uname -m)"

YTDLP_VER="2026.10.26"
YTDLP_BASE="https://github.com/yt-dlp/yt-dlp/releases/download/$YTDLP_VER"
DENO_VER="2.9.8"
DENO_BASE="https://github.com/denoland/deno/releases/download/v$DENO_VER"
FFMPEG_TAG="autobuild-2026-10-31-12-59"
FFMPEG_BUILD="n9.0.2-39-g7adfbe40dc"
FFMPEG_VER="9.0.2"
FFMPEG_BASE="https://github.com/BtbN/FFmpeg-Builds/releases/download/$FFMPEG_TAG"
ARIA2_VER="1.37.0"
# GitHub release 页面给的摘要（2026-11-24 核对）
case "$ARCH" in
  x86_64)
    YTDLP_ASSET="yt-dlp_linux.zip"
    DENO_ASSET="deno-x86_64-unknown-linux-gnu.zip"
    DENO_SHA256="e890975bc2549807bbcc0b584ffafda72cc7c93349dd99c80483dd784f6c4609"
    FFMPEG_ASSET="ffmpeg-$FFMPEG_BUILD-linux64-gpl-9.0.tar.xz"
    FFMPEG_SHA256="73e29a56974ae5a651133a04584c7483b21afe941e17dc91d79814a758b608d3"
    OPENSSL_TARGET="linux-x86_64"
    ;;
  aarch64)
    YTDLP_ASSET="yt-dlp_linux_aarch64.zip"
    DENO_ASSET="deno-aarch64-unknown-linux-gnu.zip"
    DENO_SHA256="d49596c20b9788cf8f5d8ddf644103d4bde56ed239753f0bdb92ba9361d51905"
    FFMPEG_ASSET="ffmpeg-$FFMPEG_BUILD-linuxarm64-gpl-9.0.tar.xz"
    FFMPEG_SHA256="6cad88ae5bcec356cfc5d7979a036cc4007128201928a0cf243df3db483276cc"
    OPENSSL_TARGET="linux-aarch64"
    ;;
  *)
    echo "不支持的 CPU 架构：$ARCH" >&2
    exit 1
    ;;
esac
# yt-dlp 目录版里主程序的名字（arm64 版的 zip 里也叫 yt-dlp_linux_aarch64）
YTDLP_EXE="${YTDLP_ASSET%.zip}"

# aria2 和它静态链进去的库：源码包地址和校验值（GitHub release 的摘要、官方 .sha256 文件、SQLite 下载页的 SHA3-256，2026-11-24 核对）
ARIA2_URL="https://github.com/aria2/aria2/releases/download/release-$ARIA2_VER/aria2-$ARIA2_VER.tar.xz"
ARIA2_SHA256="60a420ad7085eb616cb6e2bdf0a7206d68ff3d37fb5a956dc44242eb2f79b66b"
OPENSSL_VER="3.5.9"
OPENSSL_URL="https://github.com/openssl/openssl/releases/download/openssl-$OPENSSL_VER/openssl-$OPENSSL_VER.tar.gz"
OPENSSL_SHA256="603f5602e2eef00d77fbd429d34dcd5822bb301757a1bc9cdb24c670f1eb859a"
CARES_VER="1.34.9"
CARES_URL="https://github.com/c-ares/c-ares/releases/download/v$CARES_VER/c-ares-$CARES_VER.tar.gz"
CARES_SHA256="9364b70a12eb59a06830fb488d9357f90100dff0d98b23ea3593a183d43e6f2f"
ZLIB_VER="1.3.2"
ZLIB_URL="https://github.com/madler/zlib/releases/download/v$ZLIB_VER/zlib-$ZLIB_VER.tar.gz"
ZLIB_SHA256="bb329a0a2cd0274d05519d61c667c062e06990d72e125ee2dfa8de64f0119d16"
EXPAT_VER="2.11.0"
EXPAT_URL="https://github.com/libexpat/libexpat/releases/download/R_${EXPAT_VER//./_}/expat-$EXPAT_VER.tar.xz"
EXPAT_SHA256="61e7614b602201c28a189dafee78f5bceaee192bbde46a930752ff09cf5542fa"
SQLITE_VER="3.54.1"
SQLITE_DIR="sqlite-autoconf-3540100"
SQLITE_URL="https://www.sqlite.org/2026/$SQLITE_DIR.tar.gz"
SQLITE_SHA3="b6ced585c750916b1052667239a12c25a7ff34e7f06e1e6e01ecb0427657d9c4"

for c in curl unzip xz gpg python3 sha256sum; do
  command -v "$c" >/dev/null || { echo "缺少 $c（见开头的「需要」）" >&2; exit 1; }
done

mkdir -p "$OUT/bin" "$CACHE"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

sha256() { sha256sum "$1" | awk '{print $1}'; }
# 下载到缓存（已有且校验对就不再下载）：fetch <地址> <缓存文件名> <sha256，可空>
fetch() {
  local url="$1" name="$2" want="${3:-}" f="$CACHE/$2"
  if [[ -f "$f" && -n "$want" && "$(sha256 "$f")" == "$want" && $FORCE == 0 ]]; then
    return 0
  fi
  echo "  下载 $name …" >&2
  curl -fsSL --retry 3 -o "$f.part" "$url"
  mv "$f.part" "$f"
  if [[ -n "$want" && "$(sha256 "$f")" != "$want" ]]; then
    echo "$name 校验失败（期望 ${want}，实际 $(sha256 "$f")）" >&2
    rm -f "$f"
    exit 1
  fi
}
version_is() { [[ $FORCE == 0 && "$(cat "$1" 2>/dev/null)" == "$2" ]]; }

# ---------- yt-dlp（目录版） ----------
if ! version_is "$OUT/yt-dlp.version" "$YTDLP_VER" || [[ ! -x "$OUT/yt-dlp/$YTDLP_EXE" ]]; then
  echo "准备 yt-dlp $YTDLP_VER …"
  curl -fsSL --retry 3 -o "$TMP/SHA2-256SUMS" "$YTDLP_BASE/SHA2-256SUMS"
  curl -fsSL --retry 3 -o "$TMP/SHA2-256SUMS.sig" "$YTDLP_BASE/SHA2-256SUMS.sig"
  mkdir -m 700 "$TMP/gnupg"
  gpg --homedir "$TMP/gnupg" --quiet --import "$ROOT/crates/captae-core/assets/yt-dlp-public.key" 2>/dev/null
  if ! gpg --homedir "$TMP/gnupg" --quiet --verify "$TMP/SHA2-256SUMS.sig" "$TMP/SHA2-256SUMS" 2>/dev/null; then
    echo "SHA2-256SUMS 的签名验证没有通过" >&2
    exit 1
  fi
  echo "  SHA2-256SUMS 签名验证通过（yt-dlp 官方公钥）"
  want="$(awk -v a="$YTDLP_ASSET" '$2==a{print $1}' "$TMP/SHA2-256SUMS")"
  [[ -n "$want" ]] || { echo "SHA2-256SUMS 里没有 $YTDLP_ASSET" >&2; exit 1; }
  fetch "$YTDLP_BASE/$YTDLP_ASSET" "yt-dlp-$YTDLP_VER-$YTDLP_ASSET" "$want"
  rm -rf "$OUT/yt-dlp"
  mkdir -p "$OUT/yt-dlp"
  unzip -q -o "$CACHE/yt-dlp-$YTDLP_VER-$YTDLP_ASSET" -d "$OUT/yt-dlp"
  chmod 755 "$OUT/yt-dlp/$YTDLP_EXE"
  echo "$YTDLP_VER" >"$OUT/yt-dlp.version"
fi

# ---------- deno ----------
if ! version_is "$OUT/deno.version" "$DENO_VER" || [[ ! -x "$OUT/bin/deno" ]]; then
  echo "准备 deno $DENO_VER …"
  official="$(curl -fsSL --retry 3 "$DENO_BASE/$DENO_ASSET.sha256sum" | awk '{print $1}')"
  if [[ "$official" != "$DENO_SHA256" ]]; then
    echo "deno 官方 .sha256sum（${official}）和脚本里写的（${DENO_SHA256}）不一致" >&2
    exit 1
  fi
  fetch "$DENO_BASE/$DENO_ASSET" "deno-$DENO_VER-$DENO_ASSET" "$DENO_SHA256"
  unzip -q -o "$CACHE/deno-$DENO_VER-$DENO_ASSET" -d "$TMP/deno"
  install -m 755 "$TMP/deno/deno" "$OUT/bin/deno"
  echo "$DENO_VER" >"$OUT/deno.version"
fi

# ---------- ffmpeg / ffprobe ----------
if ! version_is "$OUT/ffmpeg.version" "$FFMPEG_BUILD" || [[ ! -x "$OUT/bin/ffmpeg" || ! -x "$OUT/bin/ffprobe" ]]; then
  echo "准备 ffmpeg $FFMPEG_BUILD（BtbN $FFMPEG_TAG）…"
  curl -fsSL --retry 3 -o "$TMP/checksums.sha256" "$FFMPEG_BASE/checksums.sha256"
  listed="$(awk -v a="$FFMPEG_ASSET" '$2==a || $2=="*"a {print $1}' "$TMP/checksums.sha256")"
  if [[ "$listed" != "$FFMPEG_SHA256" ]]; then
    echo "BtbN checksums.sha256 里 $FFMPEG_ASSET 的值（${listed:-没有}）和脚本里写的（${FFMPEG_SHA256}）不一致" >&2
    exit 1
  fi
  fetch "$FFMPEG_BASE/$FFMPEG_ASSET" "$FFMPEG_ASSET" "$FFMPEG_SHA256"
  tar -xJf "$CACHE/$FFMPEG_ASSET" -C "$TMP" --wildcards '*/bin/ffmpeg' '*/bin/ffprobe'
  install -m 755 "$TMP"/ffmpeg-*/bin/ffmpeg "$OUT/bin/ffmpeg"
  install -m 755 "$TMP"/ffmpeg-*/bin/ffprobe "$OUT/bin/ffprobe"
  echo "$FFMPEG_BUILD" >"$OUT/ffmpeg.version"
fi

# ---------- aria2c（从源码编） ----------
build_aria2() {
  local work="$CACHE/aria2-build-$ARCH" prefix jobs
  prefix="$work/prefix"
  jobs="$(nproc 2>/dev/null || echo 4)"
  for c in gcc g++ make perl pkg-config; do
    command -v "$c" >/dev/null || { echo "编 aria2 缺少 $c（apt install build-essential perl pkg-config）" >&2; exit 1; }
  done
  rm -rf "$work"
  mkdir -p "$work/src" "$prefix/lib/pkgconfig" "$prefix/include"
  echo "  下载源码包 …"
  fetch "$ZLIB_URL" "zlib-$ZLIB_VER.tar.gz" "$ZLIB_SHA256"
  fetch "$EXPAT_URL" "expat-$EXPAT_VER.tar.xz" "$EXPAT_SHA256"
  fetch "$CARES_URL" "c-ares-$CARES_VER.tar.gz" "$CARES_SHA256"
  fetch "$OPENSSL_URL" "openssl-$OPENSSL_VER.tar.gz" "$OPENSSL_SHA256"
  fetch "$ARIA2_URL" "aria2-$ARIA2_VER.tar.xz" "$ARIA2_SHA256"
  fetch "$SQLITE_URL" "sqlite-autoconf-$SQLITE_VER.tar.gz" ""
  got="$(python3 -I -c 'import hashlib, sys; print(hashlib.sha3_256(open(sys.argv[1], "rb").read()).hexdigest())' "$CACHE/sqlite-autoconf-$SQLITE_VER.tar.gz")"
  if [[ "$got" != "$SQLITE_SHA3" ]]; then
    echo "SQLite 源码包校验失败（SHA3-256 期望 ${SQLITE_SHA3}，实际 ${got}）" >&2
    rm -f "$CACHE/sqlite-autoconf-$SQLITE_VER.tar.gz"
    exit 1
  fi
  for f in "zlib-$ZLIB_VER.tar.gz" "expat-$EXPAT_VER.tar.xz" "c-ares-$CARES_VER.tar.gz" "openssl-$OPENSSL_VER.tar.gz" "aria2-$ARIA2_VER.tar.xz" "sqlite-autoconf-$SQLITE_VER.tar.gz"; do
    tar -xf "$CACHE/$f" -C "$work/src"
  done
  local log="$work/build.log"
  echo "  编译依赖库和 aria2（日志：${log}）…"
  export CFLAGS="${CFLAGS:-} -O2 -fPIC" CXXFLAGS="${CXXFLAGS:-} -O2 -fPIC"
  {
    set -x
    (cd "$work/src/zlib-$ZLIB_VER" && ./configure --static --prefix="$prefix" && make -j"$jobs" && make install)
    (cd "$work/src/expat-$EXPAT_VER" && ./configure --prefix="$prefix" --disable-shared --enable-static \
      --without-docbook --without-examples --without-tests --without-xmlwf && make -j"$jobs" && make install)
    (cd "$work/src/c-ares-$CARES_VER" && ./configure --prefix="$prefix" --disable-shared --enable-static --disable-tests && make -j"$jobs" && make install)
    # no-module：legacy 提供者编进 libcrypto；--openssldir=/etc/ssl：默认证书目录 /etc/ssl/certs（Debian / Ubuntu）
    (cd "$work/src/openssl-$OPENSSL_VER" && ./Configure "$OPENSSL_TARGET" no-shared no-module no-tests no-docs \
      --prefix="$prefix" --libdir=lib --openssldir=/etc/ssl && make -j"$jobs" build_sw && make install_sw)
    # SQLite：只要一个静态库，直接编合并版的 sqlite3.c，pkg-config 文件自己写
    (cd "$work/src/$SQLITE_DIR" && gcc $CFLAGS -DSQLITE_THREADSAFE=1 -DSQLITE_OMIT_LOAD_EXTENSION -c sqlite3.c -o sqlite3.o \
      && ar rcs "$prefix/lib/libsqlite3.a" sqlite3.o && cp sqlite3.h sqlite3ext.h "$prefix/include/")
    printf 'prefix=%s\nlibdir=${prefix}/lib\nincludedir=${prefix}/include\nName: SQLite\nDescription: SQL database engine\nVersion: %s\nLibs: -L${libdir} -lsqlite3\nLibs.private: -lm -lpthread\nCflags: -I${includedir}\n' \
      "$prefix" "$SQLITE_VER" >"$prefix/lib/pkgconfig/sqlite3.pc"
    # aria2：依赖都从 $prefix 拿（只有静态库）；libstdc++、libgcc 也静态链接，最后只动态依赖 glibc。
    # 不带 GnuTLS、libxml2（用 expat）、libssh2（SFTP 用不到）、nettle、gmp、gcrypt（OpenSSL 都有）
    (cd "$work/src/aria2-$ARIA2_VER" && PKG_CONFIG_PATH="$prefix/lib/pkgconfig" PKG_CONFIG_LIBDIR="$prefix/lib/pkgconfig" PKG_CONFIG="pkg-config --static" \
      ./configure --prefix=/usr --disable-nls --without-gnutls --with-openssl --without-libxml2 --with-libexpat \
        --with-sqlite3 --with-libcares --with-libz --without-libssh2 --without-libnettle --without-libgmp --without-libgcrypt \
        --without-libuv --disable-websocket \
        CPPFLAGS="-I$prefix/include" LDFLAGS="-L$prefix/lib -static-libstdc++ -static-libgcc" LIBS="-lpthread -ldl" \
      && make -j"$jobs")
    set +x
  } >"$log" 2>&1 || { echo "编译失败，最后 40 行：" >&2; tail -40 "$log" >&2; exit 1; }
  install -m 755 "$work/src/aria2-$ARIA2_VER/src/aria2c" "$work/aria2c"
  strip "$work/aria2c"
  echo "$ARIA2_VER openssl-$OPENSSL_VER c-ares-$CARES_VER zlib-$ZLIB_VER expat-$EXPAT_VER sqlite-$SQLITE_VER" >"$work/aria2c.version"
}
ARIA2_ID="$ARIA2_VER openssl-$OPENSSL_VER c-ares-$CARES_VER zlib-$ZLIB_VER expat-$EXPAT_VER sqlite-$SQLITE_VER"
if ! version_is "$OUT/aria2c.version" "$ARIA2_ID" || [[ ! -x "$OUT/bin/aria2c" ]]; then
  echo "准备 aria2c $ARIA2_VER …"
  if [[ $FORCE == 1 || "$(cat "$CACHE/aria2-build-$ARCH/aria2c.version" 2>/dev/null)" != "$ARIA2_ID" ]]; then
    build_aria2
  else
    echo "  用缓存里编好的（$CACHE/aria2-build-$ARCH）"
  fi
  install -m 755 "$CACHE/aria2-build-$ARCH/aria2c" "$OUT/bin/aria2c"
  echo "$ARIA2_ID" >"$OUT/aria2c.version"
fi

# ---------- captae 命令行 ----------
# 打正式包时由 build-release-linux.sh 设好去掉本机路径的编译参数（CARGO_ENCODED_RUSTFLAGS）再调用这里
echo "编译 captae 命令行 …"
(cd "$ROOT" && cargo build --release -p captae-cli --quiet)
install -m 755 "${CARGO_TARGET_DIR:-$ROOT/target}/release/captae" "$OUT/bin/captae"

# ---------- 检查：干净的 PATH 和环境下都跑得起来 ----------
echo "检查 …"
HOMETMP="$TMP/home"
mkdir -p "$HOMETMP"
for tool in yt-dlp deno ffmpeg ffprobe aria2c captae; do
  f="$OUT/bin/$tool"
  [[ $tool == yt-dlp ]] && f="$OUT/yt-dlp/$YTDLP_EXE"
  case $tool in
    ffmpeg | ffprobe) arg=-version ;;
    captae) arg=--help ;;
    *) arg=--version ;;
  esac
  if ! out="$(env -i PATH=/usr/bin:/bin HOME="$HOMETMP" CAPTAE_HOME="$HOMETMP/captae" "$f" $arg 2>&1 | head -1)" || [[ -z "$out" ]]; then
    echo "  $tool 运行失败：$out" >&2
    exit 1
  fi
  printf '  %-8s %s\n' "$tool" "$out"
done
filters="$("$OUT/bin/ffmpeg" -hide_banner -filters 2>/dev/null)"
grep -q " subtitles " <<<"$filters" || { echo "  ffmpeg 不支持字幕烧录（缺 libass）" >&2; exit 1; }
# aria2c 只能动态依赖 glibc 自己的库
needed="$(LC_ALL=C readelf -d "$OUT/bin/aria2c" | sed -n 's/.*Shared library: \[\(.*\)\]/\1/p' | tr '\n' ' ')"
for lib in $needed; do
  case "$lib" in
    libc.so.* | libm.so.* | libpthread.so.* | libdl.so.* | librt.so.* | ld-linux*) ;;
    *) echo "  aria2c 动态依赖了 glibc 以外的库：${needed}" >&2; exit 1 ;;
  esac
done
echo "  aria2c 只动态依赖：$needed"
echo "完成：$OUT"
