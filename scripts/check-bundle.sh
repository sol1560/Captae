#!/usr/bin/env bash
# 检查打好的 macOS 安装包（dmg）。只读挂载、看完就卸载；不运行桌面端，不安装，不碰用户的数据。
#
#   1. 安装包和 Captae.app 的大小；Info.plist 的标识、版本（和 tauri.conf.json 对照）
#   2. 附带程序齐不齐：captae、yt-dlp、deno、ffmpeg、ffprobe、aria2c（含 aria2 的动态库和 OpenSSL 模块），
#      都跑得出版本（干净的 PATH、临时的 HOME 和 CAPTAE_HOME，用 sandbox-exec 挡住 /opt/homebrew 和 /usr/local，模拟没装 Homebrew 的 Mac），
#      codesign -v 能过；版本和 fetch-sidecars.sh 写的一致；程序和库的加载路径里没有 Homebrew 的目录
#   3. 签名和公证：整个 Captae.app 的签名封好没有（没封好的话，从网上下载后打开很可能被系统说成「已损坏」）；
#      用 Developer ID 签的：每个程序和库同一个团队、hardened runtime、时间戳、公证结果钉在 app 和 dmg 上、Gatekeeper 认；只是本地签名的提醒一条
#   4. 正式构建：嵌进桌面端的前端文件里没有演示数据（src/dev/），图片只有上手引导的剧照；
#      apps/desktop/dist 和包里是同一次构建时，再在前端代码里搜演示后端的痕迹
#   5. 没有调试钩子：桌面端和命令行里搜 CAPTAE_TEST_、login-test.json、clip-test.json、eval.js（这些只在调试版里编译）；
#      也没有打包那台电脑上的主目录路径（里面有用户名；scripts/build-release.sh 编译时去掉）
#   6. 插件文件夹在不在、popup/ 是构建产物（没有源码、没有 node_modules）；和仓库里的 extension/ 对照
#   7. 许可证：LICENSE、licenses/ 和 THIRD_PARTY_NOTICES.md 在不在、每个附带程序有没有自己的目录；和仓库对照
#   8. 安装包窗口：卷名、根目录只有 Captae.app 和 Applications 快捷方式（指向「应用程序」文件夹，名字用英文）、背景图（1 倍 + 2 倍）、
#      .DS_Store 里的窗口大小和图标位置和 apps/desktop/src-tauri/dmg/layout.json 一致（读 .DS_Store 要 target/dmgbuild-venv）
#   9. 应用内更新：桌面端里的更新公钥和 tauri.conf.json 一致、更新地址是 GitHub 上的正式地址、没有允许 http 的测试配置；
#      自带的 yt-dlp 旁边有 version.txt（Captae 据此判断自带的和自动更新装的哪个新）。更新包本身由 scripts/check-update-package.sh 检查
#
# 用法：scripts/check-bundle.sh [dmg 路径]
#   不给路径时检查 target/release/bundle/dmg/ 里最新的 .dmg。
#   挂载点和临时目录放在 $CHECK_BUNDLE_TMP 下（默认 target/check-bundle），检查完删掉。
# 退出码：0 全过；1 有没过的项；2 用法或环境不对。
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DMG="${1:-}"
if [[ -z "$DMG" ]]; then
  DMG="$(ls -t "$ROOT"/target/release/bundle/dmg/*.dmg 2>/dev/null | head -1)"
fi
if [[ -z "$DMG" || ! -f "$DMG" ]]; then
  echo "找不到 dmg：${DMG:-（target/release/bundle/dmg/ 里没有）}" >&2
  exit 2
fi
if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "只能在 macOS 上检查" >&2
  exit 2
fi

TMPBASE="${CHECK_BUNDLE_TMP:-$ROOT/target/check-bundle}"
mkdir -p "$TMPBASE"
MNT="$(mktemp -d "$TMPBASE/mnt.XXXXXX")"
SANDBOX="$(mktemp -d "$TMPBASE/home.XXXXXX")"   # 跑附带程序时的 HOME 和 CAPTAE_HOME
MOUNTED=0
cleanup() {
  if [[ $MOUNTED == 1 ]]; then
    hdiutil detach "$MNT" -quiet 2>/dev/null || hdiutil detach "$MNT" -force -quiet 2>/dev/null
  fi
  rmdir "$MNT" 2>/dev/null
  rm -rf "$SANDBOX"
}
trap cleanup EXIT

FAILS=0
WARNS=0
pass() { printf '  ✓ %s\n' "$*"; }
fail() { printf '  ✗ %s\n' "$*"; FAILS=$((FAILS + 1)); }
warn() { printf '  ! %s\n' "$*"; WARNS=$((WARNS + 1)); }
section() { printf '\n%s\n' "$*"; }
mb() { awk -v b="$1" 'BEGIN { printf "%.1f MB", b / 1000000 }'; }

# hdiutil 新版会提示这种写法过时，提示走 stderr，不影响结果
if ! hdiutil attach -nobrowse -readonly -noautoopen -mountpoint "$MNT" "$DMG" >/dev/null 2>"$SANDBOX/attach.err"; then
  echo "挂载失败：$(cat "$SANDBOX/attach.err")" >&2
  exit 2
fi
MOUNTED=1
APP="$(ls -d "$MNT"/*.app 2>/dev/null | head -1)"
if [[ -z "$APP" ]]; then
  echo "dmg 里没有 .app" >&2
  exit 2
fi
C="$APP/Contents"
echo "检查 $(basename "$DMG")（$(basename "$APP")）"

# ---------- 1. 大小、Info.plist ----------
section "1. 大小和 Info.plist"
dmg_bytes="$(stat -f %z "$DMG")"
app_kb="$(du -sk "$APP" | awk '{print $1}')"
pass "安装包 $(mb "$dmg_bytes")（$dmg_bytes 字节）；装好以后 Captae.app 约 $(mb $((app_kb * 1024)))"
plist() { plutil -extract "$1" raw -o - "$C/Info.plist" 2>/dev/null; }
conf() { plutil -extract "$1" raw -o - "$ROOT/apps/desktop/src-tauri/tauri.conf.json" 2>/dev/null; }
id="$(plist CFBundleIdentifier)"
ver="$(plist CFBundleShortVersionString)"
build="$(plist CFBundleVersion)"
exe="$(plist CFBundleExecutable)"
minos="$(plist LSMinimumSystemVersion)"
want_id="$(conf identifier)"
want_ver="$(conf version)"
if [[ -n "$want_id" && "$id" == "$want_id" ]]; then pass "标识 $id"; else fail "标识 ${id}（tauri.conf.json 是 ${want_id:-读不到}）"; fi
if [[ -n "$want_ver" && "$ver" == "$want_ver" ]]; then pass "版本 ${ver}（构建号 ${build}）"; else fail "版本 ${ver}（tauri.conf.json 是 ${want_ver:-读不到}）"; fi
pass "主程序 ${exe}，最低系统版本 ${minos:-没写}"

# ---------- 2. 附带程序 ----------
section "2. 附带程序（干净的 PATH，HOME 和 CAPTAE_HOME 都是临时目录，看不到 Homebrew 的目录）"
# 挡住 /opt/homebrew 和 /usr/local，模拟没装 Homebrew 的 Mac：打包这台电脑上装着 Homebrew，附带程序要是悄悄用到那里的东西，只有这样才看得出来
# （2026-11-24 查出来：aria2c 带的 libcrypto 把 OpenSSL 模块目录、根证书都写死在 Homebrew 里，原来的检查在打包这台电脑上全过）
nobrew=()
if [[ -x /usr/bin/sandbox-exec ]]; then
  nobrew=(/usr/bin/sandbox-exec -p '(version 1)(allow default)(deny file-read* (subpath "/opt/homebrew") (subpath "/usr/local"))')
else
  warn "没有 sandbox-exec：附带程序照常在这台电脑上跑，看不出它们是不是用到了 Homebrew 的东西"
fi
MODS="$C/Resources/aria2-libs/ossl-modules"
run() { ${nobrew[@]+"${nobrew[@]}"} env -i PATH=/usr/bin:/bin HOME="$SANDBOX" CAPTAE_HOME="$SANDBOX/captae" "$@" 2>&1 | head -1; }
want_of() { grep -E "^$1=" "$ROOT/scripts/fetch-sidecars.sh" 2>/dev/null | head -1 | sed -E 's/^[A-Z_]+="?([^"]*)"?.*/\1/'; }
want_ytdlp="$(want_of YTDLP_VER)"
want_deno="$(want_of DENO_VER)"
want_ffmpeg="$(want_of FFMPEG_BASE | sed -E 's/.*_([0-9.]+)$/\1/')"
check_tool() { # 名字 路径 参数 期望出现在版本行里的字符串 [额外的环境变量，如 OPENSSL_MODULES=…]
  local name="$1" path="$2" arg="$3" want="${4:-}" extra="${5:-}"
  if [[ ! -x "$path" ]]; then
    fail "${name}：没有（${path#"$C"/}）"
    return
  fi
  local out sig=ok
  if [[ -n "$extra" ]]; then out="$(run "$extra" "$path" "$arg")"; else out="$(run "$path" "$arg")"; fi
  codesign -v "$path" 2>/dev/null || sig="签名验证不过"
  if [[ -z "$out" ]]; then
    fail "${name}：跑不出版本"
  elif [[ -n "$want" && "$out" != *"$want"* ]]; then
    fail "${name}：${out}（期望 ${want}）"
  elif [[ "$sig" != ok ]]; then
    fail "${name}：${out}；$sig"
  else
    pass "${name}：${out}；codesign -v 通过"
  fi
}
check_tool captae "$C/MacOS/captae" --help "Captae $ver"
check_tool yt-dlp "$C/Resources/yt-dlp/yt-dlp_macos" --version "$want_ytdlp"
check_tool deno "$C/MacOS/deno" --version "deno $want_deno"
check_tool ffmpeg "$C/MacOS/ffmpeg" -version "version $want_ffmpeg"
check_tool ffprobe "$C/MacOS/ffprobe" -version "version $want_ffmpeg"
# aria2c 和 Captae 启动它时一样指向随包带的 OpenSSL 模块（crates/captae-core/src/tools.rs 的 aria2_modules）
check_tool aria2c "$C/MacOS/aria2c" --version "aria2 version" "OPENSSL_MODULES=$MODS"
if [[ -f "$MODS/legacy.dylib" ]] && codesign -v "$MODS/legacy.dylib" 2>/dev/null; then
  pass "aria2 要加载的 OpenSSL legacy 模块随包带着（aria2-libs/ossl-modules），codesign -v 通过"
else
  fail "没有 aria2-libs/ossl-modules/legacy.dylib，或者签名验证不过：没有它，没装 Homebrew 的 Mac 上 aria2c 一启动就退出（scripts/fetch-sidecars.sh 会带上）"
fi
# 程序和库的加载路径里不能有 Homebrew 的目录（别人的 Mac 上没有）
brewrefs=""
nmach=0
while IFS= read -r -d '' f; do
  [[ "$(file -b "$f")" == *Mach-O* ]] || continue
  nmach=$((nmach + 1))
  refs="$(otool -L "$f" 2>/dev/null)"
  if grep -q -E '^[[:space:]]+/(opt/homebrew|usr/local)/' <<<"$refs"; then brewrefs="$brewrefs ${f#"$C"/}"; fi
done < <(find "$C/MacOS" "$C/Resources" -type f -print0)
if [[ -z "$brewrefs" ]]; then
  pass "${nmach} 个程序和库的加载路径里都没有 Homebrew 的目录"
else
  fail "这些的加载路径里有 Homebrew 的目录（别人的 Mac 上加载不了）：$(cut -c 1-300 <<<"$brewrefs")"
fi
libs_bad=""
for f in "$C"/Resources/aria2-libs/*.dylib; do
  [[ -e "$f" ]] || continue
  codesign -v "$f" 2>/dev/null || libs_bad="$libs_bad $(basename "$f")"
done
nlibs="$(ls "$C"/Resources/aria2-libs/*.dylib 2>/dev/null | wc -l | tr -d ' ')"
if [[ "$nlibs" == 0 ]]; then
  fail "aria2 的动态库：Resources/aria2-libs/ 是空的"
elif [[ -n "$libs_bad" ]]; then
  fail "aria2 的动态库签名验证不过：$libs_bad"
else
  pass "aria2 的动态库 $nlibs 个，codesign -v 都通过"
fi
# deno 要真的跑一段 JS（yt-dlp 解 YouTube 的验证靠它）：开了 hardened runtime 又没有 allow-jit 授权时，--version 能过、跑 JS 会崩
js="$(${nobrew[@]+"${nobrew[@]}"} env -i PATH=/usr/bin:/bin HOME="$SANDBOX" DENO_NO_UPDATE_CHECK=1 "$C/MacOS/deno" eval 'console.log(6 * 7)' 2>&1 | tail -1)"
if [[ "$js" == 42 ]]; then pass "deno 能跑 JS"; else fail "deno 跑不了 JS：${js:-没有输出}"; fi
# 先把输出存下来再找：直接接 grep -q 的话，grep 找到就退出，ffmpeg 收到 SIGPIPE，pipefail 会把整条当成失败
filters="$(${nobrew[@]+"${nobrew[@]}"} "$C/MacOS/ffmpeg" -hide_banner -filters 2>/dev/null)"
if grep -q " subtitles " <<<"$filters"; then
  pass "ffmpeg 带 libass（能烧录字幕）"
else
  fail "ffmpeg 不支持字幕烧录（缺 libass）"
fi
# 主程序是整个 app 的一部分，签名要连同整个 app 一起验（第 3 项）；这里只看它在不在，不运行它
if [[ -x "$C/MacOS/$exe" ]]; then
  pass "桌面端主程序 $exe 在（不运行它；签名见第 3 项）"
else
  fail "桌面端主程序 $exe 不在"
fi

# ---------- 3. 签名和公证 ----------
section "3. 签名和公证"
# codesign、spctl 的输出都先存下来再找（理由同上：接 grep -q 的话前面的命令可能收到 SIGPIPE，pipefail 会当成失败）
if out="$(codesign --verify --deep --strict "$APP" 2>&1)"; then
  info="$(codesign -dvv "$APP" 2>&1)"
  if grep -q '^Authority=Developer ID Application:' <<<"$info"; then
    team="$(sed -n 's/^TeamIdentifier=//p' <<<"$info")"
    pass "Captae.app 用 Developer ID 签名（团队 ${team}），codesign --verify --deep --strict 通过"
    if grep -q 'flags=.*runtime' <<<"$info"; then pass "开着 hardened runtime"; else fail "没开 hardened runtime（公证要求开）"; fi
    ts="$(sed -n 's/^Timestamp=//p' <<<"$info")"
    if [[ -n "$ts" ]]; then pass "签名带 Apple 的时间戳（${ts}）"; else fail "签名没带时间戳（公证要求带）"; fi
    # 公证要求包里每个程序和库都是同一个团队签的；可执行文件还要开 hardened runtime（库不要求）
    nbin=0
    other=""
    noruntime=""
    while IFS= read -r -d '' f; do
      kind="$(file -b "$f")"
      [[ "$kind" == *Mach-O* ]] || continue
      nbin=$((nbin + 1))
      d="$(codesign -dv "$f" 2>&1)"
      [[ "$(sed -n 's/^TeamIdentifier=//p' <<<"$d")" == "$team" ]] || other="$other ${f#"$C"/}"
      if [[ "$kind" == *executable* ]] && ! grep -q 'flags=.*runtime' <<<"$d"; then noruntime="$noruntime ${f#"$C"/}"; fi
    done < <(find "$C/MacOS" "$C/Resources" -type f -print0)
    if [[ -z "$other" ]]; then pass "包里 $nbin 个程序和库都是这个团队签的"; else fail "这些不是这个团队签的：$(cut -c 1-300 <<<"$other")"; fi
    if [[ -z "$noruntime" ]]; then pass "可执行文件都开着 hardened runtime"; else fail "这些可执行文件没开 hardened runtime：$(cut -c 1-300 <<<"$noruntime")"; fi
    if xcrun stapler validate "$APP" >/dev/null 2>&1; then pass "公证结果钉在 Captae.app 上（没网时第一次打开也认）"; else fail "Captae.app 上没有钉公证结果"; fi
    dinfo="$(codesign -dvv "$DMG" 2>&1)"
    if grep -q '^Authority=Developer ID Application:' <<<"$dinfo"; then pass "dmg 本身也用 Developer ID 签了名"; else fail "dmg 本身没签名"; fi
    if xcrun stapler validate "$DMG" >/dev/null 2>&1; then pass "公证结果钉在 dmg 上"; else fail "dmg 上没有钉公证结果"; fi
    # 系统的判断（Gatekeeper）：从网上下载的 dmg 打开时、里面的 app 第一次打开时就是这么查的
    gk="$(spctl -a -t exec -vv "$APP" 2>&1)"
    if grep -q "accepted" <<<"$gk" && grep -q "Notarized Developer ID" <<<"$gk"; then pass "Gatekeeper 对 Captae.app：accepted（Notarized Developer ID）"; else fail "Gatekeeper 不认 Captae.app：$(tr '\n' ' ' <<<"$gk")"; fi
    gk="$(spctl -a -t open --context context:primary-signature -vv "$DMG" 2>&1)"
    if grep -q "accepted" <<<"$gk" && grep -q "Notarized Developer ID" <<<"$gk"; then pass "Gatekeeper 对 dmg：accepted（Notarized Developer ID）"; else fail "Gatekeeper 不认 dmg：$(tr '\n' ' ' <<<"$gk")"; fi
  else
    pass "codesign --verify --deep --strict 通过（$(grep -E '^Signature=' <<<"$info" | head -n 1)）"
    warn "只是本地签名、没有公证：从网上下载后第一次打开会被系统拦，要到「隐私与安全性」里点「仍要打开」（钥匙串里有 Developer ID 证书时 scripts/build-release.sh 会自动签名、公证）"
  fi
else
  fail "codesign --verify Captae.app 不过：${out##*: }。只有链接器给主程序加的临时签名、资源没有封进签名时就是这样；从网上下载后打开很可能被说成「已损坏」。在 tauri.release.conf.json 里设 bundle.macOS.signingIdentity 为 \"-\"（本地签名）可以封好"
fi

# ---------- 4. 正式构建，没有演示数据 ----------
section "4. 正式构建，没有演示数据"
main="$C/MacOS/$exe"
embedded="$(strings -a "$main" | grep -o -E '/assets/[A-Za-z0-9_.-]+\.(js|css|jpg|jpeg|png|webp|gif|svg|woff2|mp4|webm)' | sort -u)"
if [[ -z "$embedded" ]]; then
  fail "在桌面端里找不到嵌进去的前端文件名（/assets/…），没法核对"
else
  pass "嵌进桌面端的前端文件 $(wc -l <<<"$embedded" | tr -d ' ') 个：$(tr '\n' ' ' <<<"$embedded" | sed 's#/assets/##g')"
  # 图片只许是上手引导的剧照（src/assets/img/onboarding/）
  allowed="$(ls "$ROOT/apps/desktop/src/assets/img/onboarding/" 2>/dev/null | sed -E 's/\.[a-z]+$//')"
  stray=""
  while read -r a; do
    [[ "$a" =~ \.(jpg|jpeg|png|webp|gif|mp4|webm)$ ]] || continue
    base="$(basename "$a")"
    ok=0
    for n in $allowed; do [[ "$base" == "$n"-* ]] && ok=1; done
    [[ $ok == 1 ]] || stray="$stray $base"
  done <<<"$embedded"
  if [[ -n "$stray" ]]; then fail "嵌进去的图片里有上手引导剧照以外的：${stray}（演示图片在 src/dev/img/，不该进正式包）"; else pass "图片只有上手引导的剧照"; fi
  if grep -q -E '/assets/index-[A-Za-z0-9_-]+\.js' <<<"$embedded"; then pass "是正式构建（index.html + 分开的 js/css），不是演示用的单文件构建"; else fail "没找到正式构建的 index-*.js"; fi
fi
dist="$ROOT/apps/desktop/dist"
if [[ -d "$dist/assets" && -n "$embedded" ]]; then
  local_list="$(cd "$dist" && ls assets | sed 's#^#/assets/#' | sort)"
  if [[ "$local_list" == "$embedded" ]]; then
    # 演示后端（src/dev/mock*.ts）里才有的字符串；正式构建里一个都不该有
    hits=""
    for s in "captae3:migrated" "Sintel_poster" "nohost-on"; do
      grep -q -F "$s" "$dist"/assets/*.js && hits="$hits $s"
    done
    if [[ -n "$hits" ]]; then fail "前端代码里有演示后端的字符串：$hits"; else pass "apps/desktop/dist 和包里是同一次构建，前端代码里没有演示后端的痕迹"; fi
  else
    warn "apps/desktop/dist 和包里的不是同一次构建，跳过前端代码的检查"
  fi
else
  warn "没有 apps/desktop/dist，跳过前端代码的检查"
fi
odd="$(find "$C" \( -name '.DS_Store' -o -iname '*mock*' -o -path '*/dev/img*' -o -name '*.map' \) 2>/dev/null | sed "s#^$C/##")"
if [[ -n "$odd" ]]; then fail "包里有不该有的文件：$(tr '\n' ' ' <<<"$odd")"; else pass "包里没有 .DS_Store、mock、dev/img、source map"; fi

# ---------- 5. 调试钩子 ----------
section "5. 调试钩子（只在调试版里编译）"
for bin in "$main" "$C/MacOS/captae"; do
  hits="$(strings -a "$bin" | grep -o -E 'CAPTAE_TEST_[A-Z_]+|login-test\.json|clip-test\.json|eval\.js' | sort -u | tr '\n' ' ')"
  if [[ -n "$hits" ]]; then fail "$(basename "$bin") 里有调试钩子：$hits"; else pass "$(basename "$bin") 里没有 CAPTAE_TEST_、login-test.json、clip-test.json、eval.js"; fi
done
# Rust 默认把源码路径写进程序（依赖在 ~/.cargo/registry/src/ 下），带着打包人的用户名。只数不打印，免得把用户名印出来
for bin in "$main" "$C/MacOS/captae"; do
  n="$(strings -a "$bin" | grep -E '/Users/[^/[:space:]]+/' | grep -c -v '/Users/Shared/' || true)"
  if [[ "${n:-0}" == 0 ]]; then pass "$(basename "$bin") 里没有本机主目录的路径"; else fail "$(basename "$bin") 里有 $n 处本机主目录的路径（里面有用户名）：用 scripts/build-release.sh 打包，编译时会去掉"; fi
done

# ---------- 6. 插件文件夹 ----------
section "6. 浏览器插件文件夹（Contents/Resources/extension）"
ext="$C/Resources/extension"
if [[ ! -f "$ext/manifest.json" ]]; then
  fail "没有插件文件夹或 manifest.json"
else
  pass "插件文件夹在，manifest 版本 $(plutil -extract version raw -o - "$ext/manifest.json" 2>/dev/null)"
  if [[ -f "$ext/popup/index.html" && -f "$ext/popup/assets/popup.js" ]]; then pass "popup/ 有构建产物（index.html、assets/popup.js）"; else fail "popup/ 里没有构建产物"; fi
  src="$(find "$ext" \( -name '*.tsx' -o -name '*.ts' -o -name 'node_modules' -o -name '*.map' -o -name 'package.json' \) 2>/dev/null | sed "s#^$ext/##")"
  if [[ -n "$src" ]]; then fail "插件文件夹里有源码或开发文件：$(tr '\n' ' ' <<<"$src")"; else pass "插件文件夹里没有源码、node_modules、source map"; fi
  if [[ -d "$ROOT/extension" ]]; then
    if diff -rq "$ext" "$ROOT/extension" >/dev/null 2>&1; then pass "和仓库里的 extension/ 一样"; else warn "和仓库里的 extension/ 不一样（打包以后又改过插件？）：$(diff -rq "$ext" "$ROOT/extension" 2>&1 | head -3 | tr '\n' ' ')"; fi
  fi
fi

# ---------- 7. 许可证 ----------
section "7. 许可证和第三方声明"
res="$C/Resources"
if [[ -f "$res/THIRD_PARTY_NOTICES.md" ]]; then pass "THIRD_PARTY_NOTICES.md 在"; else fail "没有 THIRD_PARTY_NOTICES.md"; fi
# Captae 自己是 AGPL-3.0：分发二进制要附许可证原文
if cmp -s "$res/LICENSE" "$ROOT/LICENSE" 2>/dev/null; then pass "LICENSE（AGPL-3.0）在"; else fail "没有 LICENSE，或者和仓库里的不一样"; fi
missing=""
for d in yt-dlp ffmpeg aria2 deno; do [[ -d "$res/licenses/$d" ]] || missing="$missing $d"; done
if [[ ! -f "$res/licenses/README.md" ]]; then fail "没有 licenses/README.md"; elif [[ -n "$missing" ]]; then fail "licenses/ 里缺：$missing"; else pass "licenses/ 在，附带程序各有自己的目录（$(ls "$res/licenses" | tr '\n' ' ')）"; fi
if [[ -d "$ROOT/licenses" ]]; then
  if diff -rq "$res/licenses" "$ROOT/licenses" >/dev/null 2>&1 && diff -q "$res/THIRD_PARTY_NOTICES.md" "$ROOT/THIRD_PARTY_NOTICES.md" >/dev/null 2>&1; then
    pass "和仓库里的 licenses/、THIRD_PARTY_NOTICES.md 一样"
  else
    warn "和仓库里的 licenses/ 或 THIRD_PARTY_NOTICES.md 不一样（打包以后又改过？）"
  fi
fi

# ---------- 8. 安装包窗口 ----------
section "8. 安装包窗口（在访达里打开 dmg 时看到的）"
DMG_SRC="$ROOT/apps/desktop/src-tauri/dmg"
lay() { plutil -extract "$1" raw -o - "$DMG_SRC/layout.json" 2>/dev/null; }
vol="$(diskutil info -plist "$MNT" 2>/dev/null | plutil -extract VolumeName raw -o - - 2>/dev/null)"
if [[ "$vol" == Captae ]]; then pass "卷名 Captae（窗口标题）"; else fail "卷名是「${vol}」，应该是 Captae"; fi
visible="$(ls "$MNT" | tr '\n' ' ')"
# 快捷方式的名字是英文 Applications（安装包窗口只有一种语言，用英文，docs/I18N.md 第 1 节）。ls 按名字排，它在 Captae.app 前面
if [[ "$visible" == "Applications $(basename "$APP") " ]]; then pass "窗口里只有 $(basename "$APP") 和 Applications 快捷方式"; else fail "窗口里的东西不对：$visible"; fi
if [[ "$(readlink "$MNT/Applications")" == /Applications ]]; then pass "Applications 是指向 /Applications（「应用程序」文件夹）的快捷方式"; else fail "Applications 没有指向 /Applications"; fi
# 背景图比内容区多出标题栏那么高（见 dmg/settings.py）
bw="$(lay window.width)"
bh=$(($(lay window.height) + $(lay titleBar)))
sizes="$(tiffutil -info "$MNT/.background.tiff" 2>/dev/null | awk '/Image Width/ { printf "%sx%s ", $3, $6 }')"
if [[ "$sizes" == "${bw}x${bh} $((bw * 2))x$((bh * 2)) " ]]; then
  pass "背景图 ${bw}×${bh}，带 2 倍图"
else
  fail "背景图的大小不对：${sizes:-没有 .background.tiff}（应该是 ${bw}x${bh} 和 2 倍）"
fi
py="$ROOT/target/dmgbuild-venv/bin/python"
if [[ -x "$py" ]]; then
  # 读 .DS_Store 的一小段 Python，每行输出一条：ok <说明> 或 bad <说明>。
  # 先读进变量再跑：macOS 自带的 bash 3.2 不认 <( ) 里面的 heredoc
  read -r -d '' DS_CHECK <<'PY'
import json, sys
from ds_store import DSStore

mnt, layout_path, app = sys.argv[1:4]
lay = json.load(open(layout_path, encoding="utf-8"))
w = lay["window"]
want_bounds = "{{%d, %d}, {%d, %d}}" % (w["x"], w["y"], w["width"], w["height"] + lay["titleBar"])
want_icons = {app: (lay["app"]["x"], lay["app"]["y"]), "Applications": (lay["applications"]["x"], lay["applications"]["y"])}
bwsp, icvp, icons = {}, {}, {}
with DSStore.open(mnt + "/.DS_Store", "r") as d:
    for e in d:
        if e.filename == "." and e.code == b"bwsp":
            bwsp = e.value
        elif e.filename == "." and e.code == b"icvp":
            icvp = e.value
        elif e.code == b"Iloc":
            icons[e.filename] = tuple(e.value)
bars = [k for k in ("ShowToolbar", "ShowSidebar", "ShowStatusBar", "ShowPathbar", "ShowTabView") if bwsp.get(k)]
print("ok" if bwsp.get("WindowBounds") == want_bounds else "bad", "窗口位置和大小 %s（连标题栏）" % bwsp.get("WindowBounds"))
print("ok" if not bars else "bad", "工具栏、侧栏、状态栏、路径栏都不显示" if not bars else "显示了：" + " ".join(bars))
print("ok" if icvp.get("backgroundType") == 2 and icvp.get("iconSize") == lay["iconSize"] else "bad",
      "图标视图：背景是图片（backgroundType %s），图标 %s 点" % (icvp.get("backgroundType"), icvp.get("iconSize")))
print("ok" if icons == want_icons else "bad", "图标位置 %s" % ", ".join("%s %s" % (k, v) for k, v in sorted(icons.items())))
PY
  # Python 出错时报错的每一行也会当成没过
  while read -r kind msg; do
    case "$kind" in
      ok) pass "$msg" ;;
      bad) fail "$msg" ;;
      *) fail "读 .DS_Store 出错：$kind $msg" ;;
    esac
  done < <("$py" -I -c "$DS_CHECK" "$MNT" "$DMG_SRC/layout.json" "$(basename "$APP")" 2>&1)
else
  warn "没有 target/dmgbuild-venv（scripts/build-release.sh 会装），跳过 .DS_Store 的核对"
fi

# ---------- 9. 应用内更新 ----------
section "9. 应用内更新（docs/DESIGN.md 第 15 节）"
want_pub="$(python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))["plugins"]["updater"]["pubkey"])' "$ROOT/apps/desktop/src-tauri/tauri.conf.json" 2>/dev/null)"
main_strs="$(strings -a "$main")"
if [[ -n "$want_pub" ]] && grep -qF -- "$want_pub" <<<"$main_strs"; then pass "桌面端里的更新公钥和 tauri.conf.json 一致"; else fail "桌面端里没有 tauri.conf.json 里的更新公钥"; fi
if grep -qF "https://github.com/sol1560/Captae/releases/latest/download/latest.json" <<<"$main_strs"; then pass "更新地址是 GitHub 上的正式地址"; else fail "桌面端里没有正式的更新地址"; fi
if grep -qE 'http://(127\.0\.0\.1|localhost)[^ ]*latest\.json' <<<"$main_strs"; then fail "测试用的本机更新地址混进了正式包"; else pass "没有测试用的本机更新地址"; fi
ytv="$C/Resources/yt-dlp/version.txt"
want_yt="$(sed -n 's/^YTDLP_VER="\{0,1\}\([0-9.]*\)"\{0,1\}.*/\1/p' "$ROOT/scripts/fetch-sidecars.sh" | head -1)"
if [[ -f "$ytv" && "$(tr -d '[:space:]' <"$ytv")" == "$want_yt" ]]; then pass "自带的 yt-dlp 旁边记着版本 ${want_yt}（version.txt）"; else fail "自带的 yt-dlp 旁边没有 version.txt，或者版本不是 ${want_yt:-读不到}"; fi

echo
if [[ $FAILS == 0 ]]; then
  echo "全部通过（提醒 $WARNS 条）"
  exit 0
fi
echo "没过 $FAILS 项（提醒 $WARNS 条）"
exit 1
