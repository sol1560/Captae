#!/usr/bin/env bash
# 检查 macOS 的应用内更新包（Captae_<版本>_aarch64.app.tar.gz，docs/DESIGN.md 第 15 节）。不运行桌面端，不碰用户的数据。
#
#   1. 签名：.sig 是 tauri.conf.json 里那把公钥签的，签名里的版本号和文件名对（scripts/check-update-sig.py）
#   2. 包里只有一个 Captae.app，没有硬链接（Captae 的更新插件用 Rust 的 tar 包解，硬链接解不开）、没有 ._ 文件
#   3. 照更新插件的做法解开（去掉最上面一层目录、保留符号链接）以后：
#      codesign --verify --deep --strict 通过；用 Developer ID 签的还要 Gatekeeper 认（Notarized Developer ID）、钉着公证结果；
#      Info.plist 的版本是要发的版本；符号链接（yt-dlp 的 Python.framework）一个不少
#
# 用法：scripts/check-update-package.sh <app.tar.gz> <版本号>
# 退出码：0 全过；1 有没过的项；2 用法不对。
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TGZ="${1:-}"
VER="${2:-}"
if [[ -z "$TGZ" || -z "$VER" || ! -f "$TGZ" ]]; then
  echo "用法：check-update-package.sh <app.tar.gz> <版本号>" >&2
  exit 2
fi

TMPBASE="${CHECK_BUNDLE_TMP:-$ROOT/target/check-bundle}"
mkdir -p "$TMPBASE"
WORK="$(mktemp -d "$TMPBASE/update.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

FAILS=0
pass() { printf '  ✓ %s\n' "$*"; }
fail() { printf '  ✗ %s\n' "$*"; FAILS=$((FAILS + 1)); }
section() { printf '\n%s\n' "$*"; }

echo "检查更新包：${TGZ}（$(du -h "$TGZ" | cut -f1)）"

section "1. 签名"
if out="$(python3 -I "$ROOT/scripts/check-update-sig.py" "$TGZ" "$VER" "$ROOT/apps/desktop/src-tauri/tauri.conf.json" 2>&1)"; then pass "$out"; else fail "$out"; fi

section "2. 包里的东西"
list="$(tar -tvzf "$TGZ" 2>&1)" || { fail "解不开：$list"; list=""; }
tops="$(tar -tzf "$TGZ" 2>/dev/null | cut -d/ -f1 | sort -u | tr '\n' ' ')"
if [[ "$tops" == "Captae.app " ]]; then pass "最上面一层只有 Captae.app"; else fail "最上面一层是：$tops"; fi
if grep -q ' link to ' <<<"$list"; then fail "有硬链接（更新插件解不开）：$(grep ' link to ' <<<"$list" | head -3 | tr '\n' ' ')"; else pass "没有硬链接"; fi
if grep -qE '(^|/)\._' <<<"$(tar -tzf "$TGZ" 2>/dev/null)"; then fail "有 ._ 开头的扩展属性文件（打包时要设 COPYFILE_DISABLE=1）"; else pass "没有 ._ 文件"; fi

section "3. 解开以后"
# 和更新插件一样：每一项去掉最上面一层目录，解到一个新目录里（这个目录就是新的 Captae.app）
APP="$WORK/Captae.app"
mkdir -p "$APP"
if tar -xzf "$TGZ" -C "$APP" --strip-components 1 2>"$WORK/x.err"; then
  pass "解开了"
else
  fail "解不开：$(head -3 "$WORK/x.err")"
fi
pv="$(plutil -extract CFBundleShortVersionString raw -o - "$APP/Contents/Info.plist" 2>/dev/null)"
if [[ "$pv" == "$VER" ]]; then pass "Info.plist 的版本 $pv"; else fail "Info.plist 的版本是 ${pv:-读不到}，要发的是 $VER"; fi
links="$(tar -tvzf "$TGZ" 2>/dev/null | grep -c '^l' || true)"
got="$(find "$APP" -type l | wc -l | tr -d ' ')"
if [[ "$links" == "$got" ]]; then pass "符号链接 $got 个，和包里一样"; else fail "包里有 $links 个符号链接，解开以后只有 $got 个"; fi
if out="$(codesign --verify --deep --strict "$APP" 2>&1)"; then
  info="$(codesign -dvv "$APP" 2>&1)"
  if grep -q '^Authority=Developer ID Application:' <<<"$info"; then
    pass "codesign --verify --deep --strict 通过（Developer ID）"
    if xcrun stapler validate "$APP" >/dev/null 2>&1; then pass "钉着公证结果"; else fail "没有钉公证结果"; fi
    gk="$(spctl -a -t exec -vv "$APP" 2>&1)"
    if grep -q "accepted" <<<"$gk" && grep -q "Notarized Developer ID" <<<"$gk"; then pass "Gatekeeper：accepted（Notarized Developer ID）"; else fail "Gatekeeper 不认：$(tr '\n' ' ' <<<"$gk")"; fi
  else
    pass "codesign --verify --deep --strict 通过（只是本地签名，没有公证）"
  fi
else
  fail "codesign --verify 不过：${out##*: }"
fi

echo
if [[ $FAILS == 0 ]]; then
  echo "更新包全部通过"
  exit 0
fi
echo "更新包没过 $FAILS 项"
exit 1
