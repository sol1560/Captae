#!/usr/bin/env bash
# 下载并校验 Android 下载引擎要用的官方包，放进 target/android-engine/dl/<名字>/（每样一个单独的目录）。
# 已经下过、校验值对得上的跳过。只下免费公开的官方包。
#
#   Python   python.org 官方 Android 嵌入包：SHA256（写死，来自发布页）；有 sigstore 时再验 .sigstore（身份 hugo@python.org）
#   yt-dlp   官方 release 的 zipapp：yt-dlp 官方公钥验 SHA2-256SUMS.sig → 清单里的 SHA256 和写死的一致 → 文件 SHA256 对上
#   QuickJS  bellard.org 源码包：SHA256（写死）
#   FFmpeg   ffmpeg.org 源码包：FFmpeg release signing key 验 .asc（指纹写死）→ SHA256（写死）
#   LAME     SourceForge 源码包：SHA256（写死，官方没有签名）
#   dav1d    VideoLAN 源码包：VideoLAN Release Signing Key 验 .asc（指纹写死）→ SHA256（写死）
#
# 用法：scripts/mobile/android-engine/fetch.sh
source "$(dirname "$0")/env.sh"

DL="$WORK/dl"
mkdir -p "$DL"

fetch_plain() {
  local url="$1" dst="$2"
  mkdir -p "$(dirname "$dst")"
  curl -fsSL --retry 3 --proto '=https' --tlsv1.2 -o "$dst.part" "$url"
  mv "$dst.part" "$dst"
}

# sigstore：PATH 里的，或者 python3 -m venv target/android-engine/venv-sigstore && …/pip install sigstore 装的
SIGSTORE="$(command -v sigstore || true)"
[[ -z "$SIGSTORE" && -x "$WORK/venv-sigstore/bin/sigstore" ]] && SIGSTORE="$WORK/venv-sigstore/bin/sigstore"

echo "Python $PY_VERSION"
for abi in "${ABIS[@]}"; do
  arch="$(py_arch_of "$abi")"
  f="python-$PY_VERSION-$arch-linux-android.tar.gz"
  var="PY_SHA256_$(echo "$abi" | tr '-' '_')"
  fetch "$PY_URL_BASE/$f" "$DL/python/$f" "${!var}"
  if [[ -n "$SIGSTORE" ]]; then
    [[ -f "$DL/python/$f.sigstore" ]] || fetch_plain "$PY_URL_BASE/$f.sigstore" "$DL/python/$f.sigstore"
    "$SIGSTORE" verify identity --bundle "$DL/python/$f.sigstore" --cert-identity "$PY_SIGSTORE_IDENTITY" \
      --cert-oidc-issuer "$PY_SIGSTORE_ISSUER" "$DL/python/$f" >/dev/null 2>&1 \
      && echo "  sigstore 签名通过：${f}（${PY_SIGSTORE_IDENTITY}）" \
      || { echo "sigstore 签名验证没通过：$f" >&2; exit 4; }
  else
    echo "  （没装 sigstore，只核对了 SHA256）"
  fi
done

echo "yt-dlp $YTDLP_VERSION"
d="$DL/yt-dlp"
mkdir -p "$d"
if [[ ! -f "$d/yt-dlp" || "$(sha256_of "$d/yt-dlp")" != "$YTDLP_SHA256" ]]; then
  fetch_plain "$YTDLP_URL_BASE/SHA2-256SUMS" "$d/SHA2-256SUMS"
  fetch_plain "$YTDLP_URL_BASE/SHA2-256SUMS.sig" "$d/SHA2-256SUMS.sig"
fi
gpg_verify "$YTDLP_KEY" "$YTDLP_KEY_FPR" "$d/SHA2-256SUMS.sig" "$d/SHA2-256SUMS"
listed="$(awk '$2=="yt-dlp"||$2=="*yt-dlp"{print $1}' "$d/SHA2-256SUMS")"
if [[ "$listed" != "$YTDLP_SHA256" ]]; then
  echo "签过名的清单里 yt-dlp 的 SHA256（${listed}）和写死的不一样" >&2
  exit 4
fi
fetch "$YTDLP_URL_BASE/yt-dlp" "$d/yt-dlp" "$YTDLP_SHA256"

echo "QuickJS $QJS_VERSION"
fetch "$QJS_URL" "$DL/quickjs/quickjs-$QJS_VERSION.tar.xz" "$QJS_SHA256"

echo "FFmpeg $FFMPEG_VERSION"
d="$DL/ffmpeg"
fetch "$FFMPEG_URL" "$d/ffmpeg-$FFMPEG_VERSION.tar.xz" "$FFMPEG_SHA256"
[[ -f "$d/ffmpeg-$FFMPEG_VERSION.tar.xz.asc" ]] || fetch_plain "$FFMPEG_URL.asc" "$d/ffmpeg-$FFMPEG_VERSION.tar.xz.asc"
[[ -f "$d/ffmpeg-devel.asc" ]] || fetch_plain "$FFMPEG_KEY_URL" "$d/ffmpeg-devel.asc"
gpg_verify "$d/ffmpeg-devel.asc" "$FFMPEG_KEY_FPR" "$d/ffmpeg-$FFMPEG_VERSION.tar.xz.asc" "$d/ffmpeg-$FFMPEG_VERSION.tar.xz"

echo "LAME $LAME_VERSION"
fetch "$LAME_URL" "$DL/lame/lame-$LAME_VERSION.tar.gz" "$LAME_SHA256"

echo "dav1d $DAV1D_VERSION"
d="$DL/dav1d"
fetch "$DAV1D_URL" "$d/dav1d-$DAV1D_VERSION.tar.xz" "$DAV1D_SHA256"
[[ -f "$d/dav1d-$DAV1D_VERSION.tar.xz.asc" ]] || fetch_plain "$DAV1D_URL.asc" "$d/dav1d-$DAV1D_VERSION.tar.xz.asc"
[[ -f "$d/videolan.asc" ]] || fetch_plain "$DAV1D_KEY_URL" "$d/videolan.asc"
gpg_verify "$d/videolan.asc" "$DAV1D_KEY_FPR" "$d/dav1d-$DAV1D_VERSION.tar.xz.asc" "$d/dav1d-$DAV1D_VERSION.tar.xz"

echo "下载和校验都完成：$DL"
