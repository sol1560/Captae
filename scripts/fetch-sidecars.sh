#!/usr/bin/env bash
# 准备安装包里附带的外部程序（Tauri externalBin），目前只做 macOS arm64。
#
#   yt-dlp        官方发布的 yt-dlp_macos.zip 2026.10.26（目录版，自带 Python），放在 binaries/yt-dlp/，
#                 打包后在 Contents/Resources/yt-dlp/。不用单文件版：单文件版每次启动都把自己解压成新文件，
#                 macOS 每次都要重新做安全检查，一次启动要 10 秒；目录版只在第一次检查。
#                 核对官方 SHA2-256SUMS，装了 gpg 时再用 yt-dlp 官方公钥验证 SHA2-256SUMS.sig。
#                 装好后 Captae 会自动更新它（数据目录 engines/yt-dlp/），这里的版本是兜底用的
#   deno          官方发布的 deno-aarch64-apple-darwin.zip 2.9.8（MIT），yt-dlp 解 YouTube 的 JS 验证要用。
#                 核对脚本里写死的 SHA256 和官方 .sha256sum 文件。保留 Deno 官方的代码签名，不重签
#   ffmpeg/ffprobe ffmpeg.martin-riedl.de 的 arm64 静态构建（带 libass，字幕烧录要用），校验 sha256
#   aria2c        没有官方 arm64 构建：拷 Homebrew 的 aria2c，把它依赖的动态库一起放进
#                 binaries/aria2-libs（打包后在 Contents/Resources/aria2-libs），改写加载路径。
#                 OpenSSL 的 legacy 模块也带上（aria2-libs/ossl-modules/legacy.dylib）：aria2 一启动就加载它，
#                 libcrypto 却把模块目录写死在打包这台电脑的 Homebrew 里，Captae 启动 aria2c 时用 OPENSSL_MODULES 指过来
#   captae        本仓库的命令行（插件宿主、MCP 服务器）
#
# 所有文件放到 apps/desktop/src-tauri/binaries/，文件名带 Tauri 要求的目标后缀。
# 用法：scripts/fetch-sidecars.sh        （已下载过的会跳过，加 --force 重新下载）
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/apps/desktop/src-tauri/binaries"
TRIPLE="aarch64-apple-darwin"
FFMPEG_BASE="https://ffmpeg.martin-riedl.de/download/macos/arm64/1789931890_9.0.2"
YTDLP_VER="2026.10.26"
YTDLP_BASE="https://github.com/yt-dlp/yt-dlp/releases/download/$YTDLP_VER"
DENO_VER="2.9.8"
DENO_BASE="https://github.com/denoland/deno/releases/download/v$DENO_VER"
# GitHub release 页面给的摘要（2026-11-23 核对），和官方 .sha256sum 文件两边都要对上
DENO_SHA256="9f0bf660393da2eaff044cdc7444cb08c73f235849b61d224d825a194ce7c98e"
FORCE=0
[[ "${1:-}" == "--force" ]] && FORCE=1

if [[ "$(uname -s)" != "Darwin" || "$(uname -m)" != "arm64" ]]; then
  echo "目前只支持在 Apple Silicon 的 macOS 上准备附带程序" >&2
  exit 1
fi

mkdir -p "$BIN"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

need() { [[ $FORCE == 1 || ! -x "$BIN/$1-$TRIPLE" ]]; }

# ---------- ffmpeg / ffprobe ----------
for tool in ffmpeg ffprobe; do
  if need "$tool"; then
    echo "下载 $tool …"
    curl -fsSL "$FFMPEG_BASE/$tool.zip" -o "$TMP/$tool.zip"
    want="$(curl -fsSL "$FFMPEG_BASE/$tool.zip.sha256" | awk '{print $1}')"
    got="$(shasum -a 256 "$TMP/$tool.zip" | awk '{print $1}')"
    if [[ "$want" != "$got" ]]; then
      echo "$tool.zip 校验失败（期望 ${want}，实际 ${got}）" >&2
      exit 1
    fi
    unzip -q -o "$TMP/$tool.zip" -d "$TMP/$tool"
    install -m 755 "$TMP/$tool/$tool" "$BIN/$tool-$TRIPLE"
  fi
done

# ---------- yt-dlp（目录版） ----------
rm -f "$BIN/yt-dlp-$TRIPLE"   # 以前放的单文件版
# 版本记在 binaries/yt-dlp.version 里，换了 YTDLP_VER 就重新下（不跑 --version：第一次运行要等安全检查）
if [[ $FORCE == 1 || ! -x "$BIN/yt-dlp/yt-dlp_macos" || "$(cat "$BIN/yt-dlp.version" 2>/dev/null)" != "$YTDLP_VER" ]]; then
  echo "下载 yt-dlp $YTDLP_VER …"
  curl -fsSL "$YTDLP_BASE/yt-dlp_macos.zip" -o "$TMP/yt-dlp.zip"
  curl -fsSL "$YTDLP_BASE/SHA2-256SUMS" -o "$TMP/SHA2-256SUMS"
  curl -fsSL "$YTDLP_BASE/SHA2-256SUMS.sig" -o "$TMP/SHA2-256SUMS.sig"
  if command -v gpg >/dev/null; then
    mkdir -m 700 "$TMP/gnupg"
    gpg --homedir "$TMP/gnupg" --quiet --import "$ROOT/crates/captae-core/assets/yt-dlp-public.key" 2>/dev/null
    if ! gpg --homedir "$TMP/gnupg" --quiet --verify "$TMP/SHA2-256SUMS.sig" "$TMP/SHA2-256SUMS" 2>/dev/null; then
      echo "SHA2-256SUMS 的签名验证没有通过" >&2
      exit 1
    fi
    echo "  SHA2-256SUMS 签名验证通过（yt-dlp 官方公钥）"
  else
    echo "  没有 gpg，跳过签名验证（只核对 SHA256）"
  fi
  want="$(awk '$2=="yt-dlp_macos.zip"{print $1}' "$TMP/SHA2-256SUMS")"
  got="$(shasum -a 256 "$TMP/yt-dlp.zip" | awk '{print $1}')"
  if [[ -z "$want" || "$want" != "$got" ]]; then
    echo "yt-dlp_macos.zip 校验失败（期望 ${want}，实际 ${got}）" >&2
    exit 1
  fi
  rm -rf "$BIN/yt-dlp"
  mkdir -p "$BIN/yt-dlp"
  unzip -q -o "$TMP/yt-dlp.zip" -d "$BIN/yt-dlp"
  echo "$YTDLP_VER" > "$BIN/yt-dlp.version"
fi
# 安装包里也记一份版本（Contents/Resources/yt-dlp/version.txt）：Captae 不运行 yt-dlp 就知道自带的是哪一版，
# 比自动更新装的新时用自带的（tools.rs 的 bundled_ytdlp_version）
echo "$YTDLP_VER" > "$BIN/yt-dlp/version.txt"

# ---------- deno（JS 运行时） ----------
if [[ $FORCE == 1 || ! -x "$BIN/deno-$TRIPLE" || "$(cat "$BIN/deno.version" 2>/dev/null)" != "$DENO_VER" ]]; then
  echo "下载 deno $DENO_VER …"
  curl -fsSL "$DENO_BASE/deno-aarch64-apple-darwin.zip" -o "$TMP/deno.zip"
  official="$(curl -fsSL "$DENO_BASE/deno-aarch64-apple-darwin.zip.sha256sum" | awk '{print $1}')"
  got="$(shasum -a 256 "$TMP/deno.zip" | awk '{print $1}')"
  if [[ "$got" != "$DENO_SHA256" || "$official" != "$DENO_SHA256" ]]; then
    echo "deno 校验失败（脚本里写的 ${DENO_SHA256}，官方 .sha256sum ${official}，实际 ${got}）" >&2
    exit 1
  fi
  unzip -q -o "$TMP/deno.zip" -d "$TMP/deno"
  install -m 755 "$TMP/deno/deno" "$BIN/deno-$TRIPLE"
  echo "$DENO_VER" > "$BIN/deno.version"
fi

# ---------- aria2c（Homebrew 版 + 依赖库 + OpenSSL 的 legacy 模块） ----------
# 2026-11-24 以前打包的没带 OpenSSL 模块，也要重新打包
if need aria2c || [[ ! -f "$BIN/aria2-libs/ossl-modules/legacy.dylib" ]]; then
  SRC="$(brew --prefix 2>/dev/null)/bin/aria2c"
  if [[ ! -x "$SRC" ]]; then
    echo "没有找到 Homebrew 的 aria2c，先运行 brew install aria2" >&2
    exit 1
  fi
  echo "打包 aria2c 和它的依赖库 …"
  rm -rf "$BIN/aria2-libs"
  mkdir -p "$BIN/aria2-libs"
  install -m 755 "$(realpath "$SRC")" "$BIN/aria2c-$TRIPLE"
  python3 - "$BIN/aria2c-$TRIPLE" "$BIN/aria2-libs" <<'PY'
import os, shutil, subprocess, sys
exe, libdir = sys.argv[1], sys.argv[2]
# 打包后：可执行文件在 Contents/MacOS，库在 Contents/Resources/aria2-libs
REF = "@executable_path/../Resources/aria2-libs/"
def deps(p):
    out = subprocess.run(["otool", "-L", p], capture_output=True, text=True, check=True).stdout.splitlines()[1:]
    return [l.strip().split(" (")[0] for l in out]
def is_bundled(d):
    return d.startswith("/opt/homebrew/") or d.startswith("/usr/local/") or d.startswith("@rpath/")
done = {}
def resolve(d, owner):
    if d.startswith("@rpath/"):
        name = d[len("@rpath/"):]
        for base in ["/opt/homebrew/lib", os.path.dirname(owner)]:
            c = os.path.join(base, name)
            if os.path.exists(c):
                return c
        raise SystemExit(f"找不到 {d}")
    return d
queue = [exe]
while queue:
    f = queue.pop()
    for d in deps(f):
        if not is_bundled(d):
            continue
        real = os.path.realpath(resolve(d, f))
        name = os.path.basename(d)
        if name not in done:
            dst = os.path.join(libdir, name)
            shutil.copy2(real, dst)
            os.chmod(dst, 0o755)
            subprocess.run(["install_name_tool", "-id", REF + name, dst], check=True, capture_output=True)
            done[name] = dst
            queue.append(dst)
            if name.startswith("libcrypto."):
                # aria2 1.37 一启动就加载 OpenSSL 的 legacy 模块，加载不了直接退出（OSSL_PROVIDER_load 'legacy' failed）。
                # libcrypto 把模块目录写死成这台电脑上的 /opt/homebrew/Cellar/openssl@3/<版本>/lib/ossl-modules，别人的 Mac 上没有，
                # 所以模块随包带着，Captae 启动 aria2c 时用 OPENSSL_MODULES 指过来（crates/captae-core/src/tools.rs 的 aria2_modules）。
                # 要和带走的 libcrypto 是同一个版本：从它旁边的 ossl-modules 里拿。模块里对 libcrypto 的引用在下一轮循环里改写
                mod = os.path.join(os.path.dirname(real), "ossl-modules", "legacy.dylib")
                if not os.path.isfile(mod):
                    raise SystemExit(f"找不到 OpenSSL 的 legacy 模块：{mod}")
                mdst = os.path.join(libdir, "ossl-modules", "legacy.dylib")
                os.makedirs(os.path.dirname(mdst), exist_ok=True)
                shutil.copy2(mod, mdst)
                os.chmod(mdst, 0o755)
                done["ossl-modules/legacy.dylib"] = mdst
                queue.append(mdst)
        subprocess.run(["install_name_tool", "-change", d, REF + name, f], check=True, capture_output=True)
print("  已打包：" + "、".join(sorted(done)))
PY
  # 改过加载路径的文件签名失效，重新做本地签名（arm64 上没签名的程序会被系统直接杀掉）
  for f in "$BIN"/aria2-libs/*.dylib "$BIN"/aria2-libs/ossl-modules/*.dylib "$BIN/aria2c-$TRIPLE"; do
    codesign --force -s - "$f" 2>/dev/null
  done
fi

# ---------- captae 命令行 ----------
echo "编译 captae 命令行 …"
(cd "$ROOT" && cargo build --release -p captae-cli --quiet)
install -m 755 "$ROOT/target/release/captae" "$BIN/captae-$TRIPLE"

# ---------- 检查：在干净的 PATH 下、看不到 Homebrew 时都能跑 ----------
# 挡住 /opt/homebrew 和 /usr/local，模拟没装 Homebrew 的 Mac：附带程序不能悄悄用到打包这台电脑上 Homebrew 的东西
# （2026-11-24 查出来：aria2c 带的 libcrypto 把 OpenSSL 模块目录写死在 Homebrew 里，没装 Homebrew 的 Mac 上一启动就退出，原来这一步测不出来）
nobrew=()
if [[ -x /usr/bin/sandbox-exec ]]; then
  nobrew=(/usr/bin/sandbox-exec -p '(version 1)(allow default)(deny file-read* (subpath "/opt/homebrew") (subpath "/usr/local"))')
fi
echo "检查 …"
for tool in yt-dlp deno ffmpeg ffprobe aria2c captae; do
  f="$BIN/$tool-$TRIPLE"
  [[ $tool == yt-dlp ]] && f="$BIN/yt-dlp/yt-dlp_macos"
  # yt-dlp、deno 带着官方的签名，不动；其他的没有签名，加本地签名（Apple Silicon 上没签名的程序运行不了）
  if [[ $tool != yt-dlp && $tool != deno ]]; then
    codesign --force -s - "$f" 2>/dev/null || true
  elif ! codesign -v "$f" 2>/dev/null; then
    echo "  $tool 的代码签名无效" >&2
    exit 1
  fi
  case $tool in
    ffmpeg|ffprobe) arg=-version ;;
    captae) arg=--help ;;
    *) arg=--version ;;
  esac
  # aria2c 的依赖库路径是打包后的相对路径，这里用 DYLD_LIBRARY_PATH 模拟；OpenSSL 模块和 Captae 启动它时一样用 OPENSSL_MODULES 指过去
  extra=()
  [[ $tool == aria2c ]] && extra=(OPENSSL_MODULES="$BIN/aria2-libs/ossl-modules")
  if ! out="$(${nobrew[@]+"${nobrew[@]}"} env -i PATH=/usr/bin:/bin HOME="$HOME" DYLD_LIBRARY_PATH="$BIN/aria2-libs" ${extra[@]+"${extra[@]}"} "$f" $arg 2>&1 | head -1)"; then
    echo "  $tool 运行失败：$out" >&2
    exit 1
  fi
  printf '  %-8s %s\n' "$tool" "$out"
done
filters="$("$BIN/ffmpeg-$TRIPLE" -hide_banner -filters 2>/dev/null)"
grep -q " subtitles " <<<"$filters" || { echo "  ffmpeg 不支持字幕烧录（缺 libass）" >&2; exit 1; }
echo "完成：$BIN"
