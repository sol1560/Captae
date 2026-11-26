# shellcheck shell=bash disable=SC2034
# Android 完整版下载引擎（Python + yt-dlp + QuickJS + ffmpeg）构建脚本共用的版本、校验值和函数。
# 各脚本开头 source 这个文件。跑在 macOS 自带的 bash 3.2 上：变量后面紧跟中文时写成 ${变量}。
#
# 版本和校验值写死在这里；换版本时改这里，并照 work/mobile/M3-android-engine.md 第 2 节重新核对来源和签名。

set -euo pipefail

ENGINE_SCRIPTS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$ENGINE_SCRIPTS/../../.." && pwd)"
# 下载、源码、编译、打包前的产物都放这里（target/ 不进 git）
WORK="${CAPTAE_ENGINE_WORK:-$ROOT/target/android-engine}"
export STAGE="$WORK/stage"

# ---------------------------------------------------------------- 版本和校验值

# Python：python.org 官方 Android 嵌入包。SHA256 来自 https://www.python.org/downloads/release/python-31410/ 的表格，
# 另有 .sigstore 签名（fetch.sh 装了 sigstore 时顺带验）
PY_VERSION="3.14.10"
PY_MAJOR="3.14"
PY_URL_BASE="https://www.python.org/ftp/python/$PY_VERSION"
PY_SHA256_arm64_v8a="423c80b957330cf0ebb92b9b20f2294c292144a441bdb0ceb8c69d14928e569b"
PY_SHA256_x86_64="6ebda1fc3f28efc5861cad397254e8ce5d81ba4f83a3cb6a0c124c3826a0e14c"
# 3.14 的发布经理（sigstore 证书里的身份）
PY_SIGSTORE_IDENTITY="hugo@python.org"
PY_SIGSTORE_ISSUER="https://github.com/login/oauth"

# yt-dlp：官方 release 的 zipapp（文件名就叫 yt-dlp，纯 Python，Unlicense）。
# 先用 crates/captae-core/assets/yt-dlp-public.key 验 SHA2-256SUMS.sig，再核对清单里的 SHA256，两个都要对上
YTDLP_VERSION="2026.11.23"
YTDLP_URL_BASE="https://github.com/yt-dlp/yt-dlp/releases/download/$YTDLP_VERSION"
YTDLP_SHA256="e8665ed64d6b9172773e812f891fb707a0cb1ee8d9ef4fc1acb00cabee4c5256"
YTDLP_KEY="$ROOT/crates/captae-core/assets/yt-dlp-public.key"
YTDLP_KEY_FPR="AC0CBBE6848D6A873464AF4E57CF65933B5A7581"

# QuickJS（Fabrice Bellard，MIT）。官网没有签名和校验值，这里写死第一次下载时算的 SHA256
QJS_VERSION="2026-06-04"
QJS_URL="https://bellard.org/quickjs/quickjs-$QJS_VERSION.tar.xz"
QJS_SHA256="b376e839b322978313d929fd20663b11ba58b75df5a46c126dd19ea2fa70ad2a"

# FFmpeg 源码（LGPL）。官方 .asc 签名用 FFmpeg release signing key 验（指纹写死），再核对 SHA256
FFMPEG_VERSION="9.1"
FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-$FFMPEG_VERSION.tar.xz"
FFMPEG_SHA256="1a4d394ae63d190a526cd052eb4415c156b5c5e2df37efab41a7de0b8fde2433"
FFMPEG_KEY_URL="https://ffmpeg.org/ffmpeg-devel.asc"
FFMPEG_KEY_FPR="FCF986EA15E6E293A5644F10B4322F04D67658D8"

# LAME（MP3 编码，LGPL-2.0-or-later）。只在 SourceForge 发布、没有签名，写死 SHA256（和各大发行版打包用的一致）
LAME_VERSION="3.100"
LAME_URL="https://downloads.sourceforge.net/project/lame/lame/$LAME_VERSION/lame-$LAME_VERSION.tar.gz"
LAME_SHA256="ddfe36cab873794038ae2c1210557ad34857a4b6bdc515785d1da9e175b1da1e"

# dav1d（AV1 解码，BSD-2-Clause）。VideoLAN Release Signing Key 验 .asc（指纹写死），再核对 SHA256
DAV1D_VERSION="1.5.5"
DAV1D_URL="https://download.videolan.org/pub/videolan/dav1d/$DAV1D_VERSION/dav1d-$DAV1D_VERSION.tar.xz"
DAV1D_SHA256="ca96b1d0ba3f01f55de7231d775892b64994ba061aaeeaaa4f662a390b3abf9e"
DAV1D_KEY_URL="https://download.videolan.org/pub/keys/7180713BE58D1ADC.asc"
DAV1D_KEY_FPR="65F7C6B4206BD057A7EB73787180713BE58D1ADC"
# 编 dav1d 要 meson（本机没有时 build-codecs.sh 装进 target/android-engine/venv-tools）
MESON_VERSION="1.12.1"

# ---------------------------------------------------------------- NDK 和目标

ABIS=(arm64-v8a x86_64)
# 和 App 的 minSdk 一样（PLAN 5.5）
API=26
NDK_VERSION="${NDK_VERSION:-28.2.13676358}"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
NDK="${ANDROID_NDK_HOME:-$SDK/ndk/$NDK_VERSION}"
case "$(uname -s)" in
  Darwin) NDK_HOST=darwin-x86_64 ;;
  Linux) NDK_HOST=linux-x86_64 ;;
  *) echo "只支持在 macOS、Linux 上编译" >&2; exit 2 ;;
esac
TOOLCHAIN="$NDK/toolchains/llvm/prebuilt/$NDK_HOST"

need_ndk() {
  if [[ ! -x "$TOOLCHAIN/bin/clang" ]]; then
    echo "找不到 Android NDK：${NDK}（用 ANDROID_NDK_HOME 指定，版本 ${NDK_VERSION}）" >&2
    exit 2
  fi
}

# ABI → clang 的目标三元组 / Python 包里的架构名
triple_of() {
  case "$1" in
    arm64-v8a) echo "aarch64-linux-android" ;;
    x86_64) echo "x86_64-linux-android" ;;
    *) echo "不认识的 ABI：$1" >&2; exit 2 ;;
  esac
}
py_arch_of() {
  case "$1" in
    arm64-v8a) echo "aarch64" ;;
    x86_64) echo "x86_64" ;;
  esac
}

# 设好某个 ABI 的交叉编译器：CC、AR、STRIP 等
use_abi() {
  need_ndk
  local t
  t="$(triple_of "$1")"
  TRIPLE="$t"
  CC="$TOOLCHAIN/bin/${t}${API}-clang"
  CXX="$TOOLCHAIN/bin/${t}${API}-clang++"
  AR="$TOOLCHAIN/bin/llvm-ar"
  NM="$TOOLCHAIN/bin/llvm-nm"
  RANLIB="$TOOLCHAIN/bin/llvm-ranlib"
  STRIP="$TOOLCHAIN/bin/llvm-strip"
  READELF="$TOOLCHAIN/bin/llvm-readelf"
  # 去掉本机路径（和 scripts/mobile/common.sh 同一套）；16 KB 页对齐（Android 15 起的设备要求）
  PREFIX_MAP="-ffile-prefix-map=$WORK=/build -ffile-prefix-map=$ROOT=/captae -ffile-prefix-map=$HOME=~"
  PAGE_LDFLAGS="-Wl,-z,max-page-size=16384 -Wl,-z,common-page-size=16384"
}

# ---------------------------------------------------------------- 下载和校验

sha256_of() {
  shasum -a 256 "$1" | awk '{print $1}'
}

# fetch <地址> <放到哪个文件> <SHA256>：已经有、校验值对得上就跳过；下到 .part 再改名
fetch() {
  local url="$1" dst="$2" want="$3"
  if [[ -f "$dst" && "$(sha256_of "$dst")" == "$want" ]]; then
    echo "  已有：$(basename "$dst")"
    return
  fi
  mkdir -p "$(dirname "$dst")"
  echo "  下载：$url"
  curl -fL --retry 3 --proto '=https' --tlsv1.2 -o "$dst.part" "$url"
  local got
  got="$(sha256_of "$dst.part")"
  if [[ "$got" != "$want" ]]; then
    rm -f "$dst.part"
    echo "SHA256 对不上：$(basename "$dst")（应该是 ${want}，实际 ${got}）" >&2
    exit 4
  fi
  mv "$dst.part" "$dst"
}

# 用一个临时的 GnuPG 目录导入公钥、核对指纹、验分离签名（不碰本机的钥匙串）
gpg_verify() {
  local key="$1" fpr="$2" sig="$3" data="$4" g
  command -v gpg >/dev/null || { echo "缺少 gpg（brew install gnupg）" >&2; exit 2; }
  g="$(mktemp -d)"
  gpg --homedir "$g" --batch --quiet --import "$key" 2>/dev/null
  if ! gpg --homedir "$g" --batch --with-colons --list-keys 2>/dev/null | grep -q "^fpr:::::::::$fpr:"; then
    rm -rf "$g"
    echo "公钥指纹不对：${key}（应该是 ${fpr}）" >&2
    exit 4
  fi
  if ! gpg --homedir "$g" --batch --status-fd 1 --verify "$sig" "$data" 2>/dev/null | grep -q "VALIDSIG $fpr\|VALIDSIG .* $fpr\$"; then
    # 子钥签的时候 VALIDSIG 第一个字段是子钥、最后一个是主钥指纹
    if ! gpg --homedir "$g" --batch --status-fd 1 --verify "$sig" "$data" 2>/dev/null | grep "VALIDSIG" | grep -q "$fpr"; then
      rm -rf "$g"
      echo "签名验证没通过：$(basename "$sig")" >&2
      exit 4
    fi
  fi
  rm -rf "$g"
  echo "  签名通过：$(basename "$sig")（${fpr}）"
}

size_of() {
  if stat -f %z "$1" >/dev/null 2>&1; then stat -f %z "$1"; else stat -c %s "$1"; fi
}

mb() {
  awk -v b="$1" 'BEGIN{printf "%.2f MB", b/1000000}'
}
