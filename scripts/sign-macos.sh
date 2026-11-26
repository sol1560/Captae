#!/usr/bin/env bash
# 用 Developer ID 给打好的 Captae.app 重新签名。公证要求每个可执行文件和库都用 Developer ID 签名、开 hardened runtime、带时间戳，
# Tauri 只签 Contents/MacOS 里的程序，Resources 里 yt-dlp 目录版的 Python 库、aria2 的动态库它不管，所以这里由里到外全部重签：
#   1. Resources 里的 .framework（yt-dlp 带的 Python.framework）先恢复成正常的 framework 结构（见 restore_framework），
#      再整个当 framework 签
#   2. Resources 里其余的 Mach-O 文件（yt-dlp 的引导程序和 .so、aria2 的动态库……），一个个签
#   3. Contents/MacOS 里的附带程序（captae、deno、ffmpeg、ffprobe、aria2c）；apps/desktop/src-tauri/entitlements/<文件名>.plist 有的就带上
#      （deno 要 JIT，不然开着 hardened runtime 跑不了 JS）
#   4. 整个 Captae.app（主程序，封好资源）
#   5. 签完自检：在 hardened runtime 下、看不到 Homebrew 时把附带程序都跑一遍，跑不了就失败（不交给 Apple 公证）
# 同一个团队签的库能过 hardened runtime 的库校验，所以 aria2c（连同随包带的 OpenSSL 模块）、yt-dlp 不用额外的授权。
# 用法：scripts/sign-macos.sh <Captae.app> <证书名或 SHA-1>
#   证书写 - 是本地签名（不带时间戳），只用来试这个脚本的结构；本地签名的程序开着 hardened runtime 时，库校验会失败（没有 Team ID），别拿去用。
set -euo pipefail

APP="${1:?用法：sign-macos.sh <Captae.app> <证书>}"
ID="${2:?用法：sign-macos.sh <Captae.app> <证书>}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
ENT="$ROOT/apps/desktop/src-tauri/entitlements"
C="$APP/Contents"
main="$(plutil -extract CFBundleExecutable raw -o - "$C/Info.plist")"

opts=(--force --options runtime --sign "$ID")
if [[ "$ID" != "-" ]]; then opts+=(--timestamp); fi

sign() { # 文件或 bundle [授权文件]；失败时把 codesign 的原话打出来
  local out
  if [[ -n "${2:-}" ]]; then
    out="$(codesign "${opts[@]}" --entitlements "$2" "$1" 2>&1)" || { echo "签名失败：${1#"$C"/}：$out" >&2; exit 1; }
  else
    out="$(codesign "${opts[@]}" "$1" 2>&1)" || { echo "签名失败：${1#"$C"/}：$out" >&2; exit 1; }
  fi
}

# yt-dlp 目录版（PyInstaller）带的 Python.framework：官方 zip 打包时把符号链接展开成了完整的副本
# （Versions/Current、framework 顶层的 Python 和 Resources，外加 framework 外面的 _internal/Python），
# codesign 认不出这是什么 bundle（bundle format is ambiguous），签不了。恢复成 PyInstaller 原来的样子：
# 和版本目录里那份内容一模一样的，才换成指过去的符号链接（结构不认识、内容对不上的不动，后面签名会报错）。顺带少了 3 份 14 MB 的副本
restore_framework() { # <X.framework>
  local fw="$1" name ver outer e
  name="$(basename "$fw" .framework)"
  [[ -d "$fw/Versions" ]] || return 0
  ver="$(ls "$fw/Versions" | grep -v '^Current$' | head -n 1 || true)"
  [[ -n "$ver" && -f "$fw/Versions/$ver/$name" ]] || return 0
  if [[ -d "$fw/Versions/Current" && ! -L "$fw/Versions/Current" ]] && diff -rq "$fw/Versions/Current" "$fw/Versions/$ver" >/dev/null 2>&1; then
    rm -rf "${fw:?}/Versions/Current"
    ln -s "$ver" "$fw/Versions/Current"
  fi
  for e in "$name" Resources; do
    if [[ -e "$fw/$e" && ! -L "$fw/$e" ]] && diff -rq "$fw/$e" "$fw/Versions/$ver/$e" >/dev/null 2>&1; then
      rm -rf "${fw:?}/$e"
      ln -s "Versions/Current/$e" "$fw/$e"
    fi
  done
  outer="$(dirname "$fw")/$name"
  if [[ -f "$outer" && ! -L "$outer" ]] && cmp -s "$outer" "$fw/Versions/$ver/$name"; then
    rm -f "$outer"
    ln -s "$name.framework/Versions/$ver/$name" "$outer"
  fi
}

# 签之前去掉隔离属性：在这台机器上解压、复制出来的文件可能带着，打进 dmg 以后在别人那里跑附带程序时会被当成下载来的再查一遍
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

frameworks=()
while IFS= read -r -d '' fw; do
  restore_framework "$fw"
  frameworks+=("$fw")
done < <(find "$C/Resources" -type d -name "*.framework" -prune -print0)

n=0
while IFS= read -r -d '' f; do
  if [[ "$(file -b "$f")" == *Mach-O* ]]; then
    sign "$f"
    n=$((n + 1))
  fi
done < <(find "$C/Resources" -type f -not -path "*.framework/*" -print0)
echo "Resources 里的库和程序：$n 个"

for fw in ${frameworks[@]+"${frameworks[@]}"}; do
  sign "$fw"
  echo "${fw#"$C"/}（整个 framework）"
done

for f in "$C"/MacOS/*; do
  name="$(basename "$f")"
  [[ "$name" == "$main" || ! -f "$f" ]] && continue
  e=""
  [[ -f "$ENT/$name.plist" ]] && e="$ENT/$name.plist"
  sign "$f" "$e"
  echo "MacOS/${name}${e:+（授权 $(basename "$e")）}"
done

e=""
[[ -f "$ENT/app.plist" ]] && e="$ENT/app.plist"
sign "$APP" "$e"
codesign --verify --deep --strict "$APP"
echo "整个 $(basename "$APP")：$(codesign -dvv "$APP" 2>&1 | grep -E '^(Authority|Signature)=' | head -n 1)，codesign --verify --deep --strict 通过"

# ---------- 签完自检 ----------
# 在 hardened runtime 下、看不到 Homebrew 的目录时（模拟别人的 Mac），把附带程序都跑一遍：库校验（aria2 的 OpenSSL 模块、yt-dlp 的 Python）、
# JIT（deno）出了问题会在这里露出来，免得交给 Apple 公证以后才发现跑不了。本地签名（证书写 -）时库校验本来就过不了，不做
if [[ "$ID" != "-" ]]; then
  nobrew=()
  if [[ -x /usr/bin/sandbox-exec ]]; then
    nobrew=(/usr/bin/sandbox-exec -p '(version 1)(allow default)(deny file-read* (subpath "/opt/homebrew") (subpath "/usr/local"))')
  fi
  home="$(mktemp -d)"
  trap 'rm -rf "$home"' EXIT
  bad=0
  try() { # 名字 输出要对上的正则 [环境变量=值 …] 程序 参数…
    local name="$1" want="$2" out
    shift 2
    out="$(${nobrew[@]+"${nobrew[@]}"} env -i PATH=/usr/bin:/bin HOME="$home" CAPTAE_HOME="$home/captae" DENO_NO_UPDATE_CHECK=1 "$@" 2>&1)" || true
    if [[ "$out" =~ $want ]]; then
      echo "  ✓ $name"
    else
      echo "  ✗ ${name}：$(head -c 300 <<<"$out")" >&2
      bad=1
    fi
  }
  echo "签完自检（hardened runtime，看不到 Homebrew 的目录）："
  try "captae" "Captae [0-9]" "$C/MacOS/captae" --help
  try "yt-dlp" "[0-9]{4}\.[0-9]{2}\.[0-9]{2}" "$C/Resources/yt-dlp/yt-dlp_macos" --version
  try "yt-dlp 的 curl_cffi（模拟浏览器）" "Chrome-[0-9]+ +[A-Za-z0-9.-]+ +curl_cffi" "$C/Resources/yt-dlp/yt-dlp_macos" --list-impersonate-targets
  try "deno 跑 JS" "^42$" "$C/MacOS/deno" eval 'console.log(6 * 7)'
  try "ffmpeg" "^ffmpeg version" "$C/MacOS/ffmpeg" -version
  try "ffprobe" "^ffprobe version" "$C/MacOS/ffprobe" -version
  # 和 Captae 启动它时一样指过去（crates/captae-core/src/tools.rs 的 aria2_modules）
  try "aria2c（带上随包的 OpenSSL 模块）" "^aria2 version" OPENSSL_MODULES="$C/Resources/aria2-libs/ossl-modules" "$C/MacOS/aria2c" --version
  if [[ $bad != 0 ]]; then
    echo "签完自检没过，不往下做" >&2
    exit 1
  fi
fi
