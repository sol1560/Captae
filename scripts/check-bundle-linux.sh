#!/usr/bin/env bash
# 检查打好的 Linux 安装包（.deb 和 / 或 .AppImage）。只解开到临时目录里看，不安装、不运行桌面端、不碰用户的数据。
# 项目照 macOS 的 scripts/check-bundle.sh 对应过来：
#
#   1. 安装包大小；deb 的包名、版本（和 tauri.conf.json 对照）、架构、依赖；AppImage 的 AppRun、.desktop、图标
#   2. 附带程序齐不齐：captae、yt-dlp（目录版）、deno、ffmpeg、ffprobe、aria2c 都在 usr/lib/Captae/ 下，
#      在干净的环境里（env -i，PATH 只有 /usr/bin:/bin，临时的 HOME 和 CAPTAE_HOME）都跑得出版本；版本和 fetch-sidecars-linux.sh 写的一致；
#      deno 能跑 JS；ffmpeg 带 libass；aria2c 只动态依赖 glibc
#   3. 不和系统的包抢文件：usr/bin 里只有桌面端主程序（附带程序要是装进 /usr/bin，会和系统的 ffmpeg、aria2 包冲突）；
#      deb 里的文件只在 /usr/bin、/usr/lib/Captae、/usr/share 下
#   4. 正式构建：嵌进桌面端的前端文件里没有演示数据，图片只有上手引导的剧照
#   5. 没有调试钩子（CAPTAE_TEST_、login-test.json、clip-test.json、eval.js），没有打包那台电脑的主目录路径
#   6. 插件文件夹在、popup/ 是构建产物、和仓库里的 extension/ 一样
#   7. licenses/、THIRD_PARTY_NOTICES.md 在、和仓库一样
#   8. .desktop 文件（desktop-file-validate）和 AppStream 元数据（appstreamcli validate）
#
# 用法：scripts/check-bundle-linux.sh [deb 或 AppImage 的路径 ...]
#   不给路径时检查 $CARGO_TARGET_DIR（默认 target）/release/bundle/{deb,appimage}/ 里最新的各一个。
#   AppImage 要能在这台机器上运行（同一种 CPU 架构）才解得开（用它自带的 --appimage-extract，不需要 FUSE）。
#   临时目录在 $CHECK_BUNDLE_TMP（默认 target/check-bundle）下，检查完删掉。
# 退出码：0 全过；1 有没过的项；2 用法或环境不对。
set -uo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"
if [[ "$(uname -s)" != Linux ]]; then
  echo "只能在 Linux 上检查" >&2
  exit 2
fi
PKGS=("$@")
if [[ ${#PKGS[@]} == 0 ]]; then
  d="$(ls -t "$TARGET_DIR"/release/bundle/deb/*.deb 2>/dev/null | head -1)"
  a="$(ls -t "$TARGET_DIR"/release/bundle/appimage/*.AppImage 2>/dev/null | head -1)"
  [[ -n "$d" ]] && PKGS+=("$d")
  [[ -n "$a" ]] && PKGS+=("$a")
fi
if [[ ${#PKGS[@]} == 0 ]]; then
  echo "找不到安装包（$TARGET_DIR/release/bundle/deb、appimage 里都没有）" >&2
  exit 2
fi

TMPBASE="${CHECK_BUNDLE_TMP:-$ROOT/target/check-bundle}"
mkdir -p "$TMPBASE"
WORK="$(mktemp -d "$TMPBASE/linux.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

FAILS=0
WARNS=0
pass() { printf '  ✓ %s\n' "$*"; }
fail() { printf '  ✗ %s\n' "$*"; FAILS=$((FAILS + 1)); }
warn() { printf '  ! %s\n' "$*"; WARNS=$((WARNS + 1)); }
section() { printf '\n%s\n' "$*"; }
mb() { awk -v b="$1" 'BEGIN { printf "%.1f MB", b / 1000000 }'; }

conf() { python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$ROOT/apps/desktop/src-tauri/tauri.conf.json" "$1" 2>/dev/null; }
want_ver="$(conf version)"
want_of() { grep -E "^$1=" "$ROOT/scripts/fetch-sidecars-linux.sh" 2>/dev/null | head -1 | sed -E 's/^[A-Z0-9_]+="?([^"]*)"?.*/\1/'; }
want_ytdlp="$(want_of YTDLP_VER)"
want_deno="$(want_of DENO_VER)"
want_ffmpeg="$(want_of FFMPEG_VER)"
want_aria2="$(want_of ARIA2_VER)"
want_openssl="$(want_of OPENSSL_VER)"
MAIN_BIN="captae-desktop"

# 检查解开后的目录树（deb 解到的根目录，或 AppImage 的 squashfs-root）：check_tree <根目录> <说明>
check_tree() {
  local R="$1" label="$2" L="$1/usr/lib/Captae" main="$1/usr/bin/$MAIN_BIN"
  local SANDBOX="$WORK/home-$label"
  mkdir -p "$SANDBOX"

  section "2. 附带程序（$label：干净的环境，PATH 只有 /usr/bin:/bin，HOME 和 CAPTAE_HOME 是临时目录）"
  run() { env -i PATH=/usr/bin:/bin HOME="$SANDBOX" CAPTAE_HOME="$SANDBOX/captae" LANG=C.UTF-8 "$@" 2>&1 | head -1; }
  check_tool() { # 名字 路径 参数 期望出现在版本行里的字符串
    local name="$1" path="$2" arg="$3" want="${4:-}" out
    if [[ ! -x "$path" ]]; then
      fail "${name}：没有（${path#"$R"}）"
      return
    fi
    out="$(run "$path" "$arg")"
    if [[ -z "$out" ]]; then
      fail "${name}：跑不出版本"
    elif [[ -n "$want" && "$out" != *"$want"* ]]; then
      fail "${name}：${out}（期望 ${want}）"
    else
      pass "${name}：${out}（${path#"$R"}）"
    fi
  }
  local ytexe
  ytexe="$(ls "$L"/yt-dlp/yt-dlp_linux* 2>/dev/null | head -1)"
  check_tool captae "$L/captae" --help "Captae $want_ver"
  check_tool yt-dlp "${ytexe:-$L/yt-dlp/yt-dlp_linux}" --version "$want_ytdlp"
  check_tool deno "$L/deno" --version "deno $want_deno"
  check_tool ffmpeg "$L/ffmpeg" -version "version n$want_ffmpeg"
  check_tool ffprobe "$L/ffprobe" -version "version n$want_ffmpeg"
  check_tool aria2c "$L/aria2c" --version "aria2 version $want_aria2"
  if [[ -x "$L/aria2c" ]]; then
    local av needed bad=""
    av="$(env -i PATH=/usr/bin:/bin HOME="$SANDBOX" "$L/aria2c" --version 2>&1)"
    # aria2 按 OpenSSL 1.x 的老格式解析版本号，3.5.9 显示成「OpenSSL/3.5.0i」，所以只比主次版本
    if grep -q "OpenSSL/${want_openssl%.*}\." <<<"$av"; then pass "aria2c 用的是静态链进去的 OpenSSL ${want_openssl%.*}（$(grep -o 'OpenSSL/[^ ]*' <<<"$av")；legacy 提供者编在里面，不用另带模块文件）"; else fail "aria2c 的 TLS 库不对：$(grep -i -E 'ssl|tls' <<<"$av" | head -2 | tr '\n' ' ')"; fi
    needed="$(LC_ALL=C readelf -d "$L/aria2c" 2>/dev/null | sed -n 's/.*Shared library: \[\(.*\)\]/\1/p' | tr '\n' ' ')"
    for lib in $needed; do
      case "$lib" in libc.so.* | libm.so.* | libpthread.so.* | libdl.so.* | librt.so.* | ld-linux*) ;; *) bad="$bad $lib" ;; esac
    done
    if [[ -z "$bad" ]]; then pass "aria2c 只动态依赖 glibc：$needed"; else fail "aria2c 动态依赖了 glibc 以外的库：$bad"; fi
  fi
  # 附带程序要求的 glibc 最高版本（打包机器是 Ubuntu 22.04 时应该不超过 2.35）
  local maxg=""
  for f in "$main" "$L"/{captae,ffmpeg,ffprobe,aria2c,deno}; do
    [[ -f "$f" ]] || continue
    g="$(LC_ALL=C objdump -T "$f" 2>/dev/null | grep -o -E 'GLIBC_[0-9]+\.[0-9]+' | sort -t. -k2 -n -u | tail -1)"
    [[ -n "$g" ]] && maxg="$(printf '%s\n%s\n' "$maxg" "$g" | sort -t. -k2 -n | tail -1)"
  done
  if [[ -z "$maxg" ]]; then warn "看不出要求的 glibc 版本（没有 objdump？）"
  elif [[ "${maxg#GLIBC_2.}" -le 35 ]]; then pass "主程序和附带程序最高要求 ${maxg}（Ubuntu 22.04 / Debian 12 / Fedora 39 都满足）"
  else fail "要求 ${maxg}：比 Ubuntu 22.04 的 glibc 2.35 新，老一点的发行版跑不起来（在 Ubuntu 22.04 上打包）"; fi
  if [[ -x "$L/deno" ]]; then
    local js
    js="$(env -i PATH=/usr/bin:/bin HOME="$SANDBOX" DENO_NO_UPDATE_CHECK=1 "$L/deno" eval 'console.log(6 * 7)' 2>&1 | tail -1)"
    if [[ "$js" == 42 ]]; then pass "deno 能跑 JS"; else fail "deno 跑不了 JS：${js:-没有输出}"; fi
  fi
  if [[ -x "$L/ffmpeg" ]]; then
    local filters
    filters="$("$L/ffmpeg" -hide_banner -filters 2>/dev/null)"
    if grep -q " subtitles " <<<"$filters"; then pass "ffmpeg 带 libass（能烧录字幕）"; else fail "ffmpeg 不支持字幕烧录（缺 libass）"; fi
  fi
  if [[ -x "$main" ]]; then pass "桌面端主程序 usr/bin/$MAIN_BIN 在（不运行它）"; else fail "桌面端主程序 usr/bin/$MAIN_BIN 不在"; fi

  section "3. 不和系统的包抢文件（$label）"
  local inbin
  inbin="$(ls "$R/usr/bin" 2>/dev/null | tr '\n' ' ')"
  if [[ "$label" == deb ]]; then
    if [[ "$inbin" == "$MAIN_BIN " ]]; then pass "usr/bin 里只有 $MAIN_BIN"; else fail "usr/bin 里不只有桌面端：${inbin}（装进 /usr/bin 的附带程序会和系统的 ffmpeg、aria2 包冲突）"; fi
    local outside
    outside="$(cd "$R" && find . -mindepth 1 \( -type f -o -type l \) | sed 's#^\./#/#' | grep -v -E '^/usr/(bin|lib/Captae|share)/' | head -5 | tr '\n' ' ')"
    if [[ -z "$outside" ]]; then pass "文件都在 /usr/bin、/usr/lib/Captae、/usr/share 下"; else fail "有文件装到别处：$outside"; fi
  else
    # AppImage 里 linuxdeploy 会放进 GTK 相关的库和程序，usr/bin 里有别的也不冲突（不装进系统）
    if [[ ! -e "$R/usr/bin/ffmpeg" && ! -e "$R/usr/bin/aria2c" ]]; then pass "附带程序不在 usr/bin（和 deb 同一种布局：usr/lib/Captae）"; else fail "usr/bin 里有附带程序：$inbin"; fi
  fi

  section "4. 正式构建，没有演示数据（$label）"
  if [[ -x "$main" ]]; then
    local embedded allowed stray="" base ok n
    embedded="$(strings -a "$main" | grep -o -E '/assets/[A-Za-z0-9_.-]+\.(js|css|jpg|jpeg|png|webp|gif|svg|woff2|mp4|webm)' | sort -u)"
    if [[ -z "$embedded" ]]; then
      fail "在桌面端里找不到嵌进去的前端文件名（/assets/…），没法核对"
    else
      pass "嵌进桌面端的前端文件 $(wc -l <<<"$embedded" | tr -d ' ') 个"
      allowed="$(ls "$ROOT/apps/desktop/src/assets/img/onboarding/" 2>/dev/null | sed -E 's/\.[a-z]+$//')"
      while read -r a; do
        [[ "$a" =~ \.(jpg|jpeg|png|webp|gif|mp4|webm)$ ]] || continue
        base="$(basename "$a")"
        ok=0
        for n in $allowed; do [[ "$base" == "$n"-* ]] && ok=1; done
        [[ $ok == 1 ]] || stray="$stray $base"
      done <<<"$embedded"
      if [[ -n "$stray" ]]; then fail "嵌进去的图片里有上手引导剧照以外的：$stray"; else pass "图片只有上手引导的剧照"; fi
      if grep -q -E '/assets/index-[A-Za-z0-9_-]+\.js' <<<"$embedded"; then pass "是正式构建（index.html + 分开的 js/css）"; else fail "没找到正式构建的 index-*.js"; fi
    fi
  fi
  local odd
  odd="$(find "$L" \( -iname '*mock*' -o -path '*/dev/img*' -o -name '*.map' -o -name '.DS_Store' \) 2>/dev/null | sed "s#^$L/##" | head -5 | tr '\n' ' ')"
  if [[ -n "$odd" ]]; then fail "usr/lib/Captae 里有不该有的文件：$odd"; else pass "usr/lib/Captae 里没有 .DS_Store、mock、dev/img、source map"; fi

  section "5. 调试钩子和本机路径（$label）"
  local bin hits
  for bin in "$main" "$L/captae"; do
    [[ -f "$bin" ]] || continue
    hits="$(strings -a "$bin" | grep -o -E 'CAPTAE_TEST_[A-Z_]+|login-test\.json|clip-test\.json|eval\.js' | sort -u | tr '\n' ' ')"
    if [[ -n "$hits" ]]; then fail "$(basename "$bin") 里有调试钩子：$hits"; else pass "$(basename "$bin") 里没有 CAPTAE_TEST_、login-test.json、clip-test.json、eval.js"; fi
    # 只数不打印，免得把用户名印出来。/root/.cargo、/root/.rustup 是在 Docker 里用 root 打包时的主目录
    n="$(strings -a "$bin" | grep -c -E '/home/[^/[:space:]]+/|/Users/[^/[:space:]]+/|/root/\.(cargo|rustup)/' || true)"
    if [[ "${n:-0}" == 0 ]]; then pass "$(basename "$bin") 里没有打包机器的主目录路径"; else fail "$(basename "$bin") 里有 $n 处主目录路径（里面有用户名）：用 scripts/build-release-linux.sh 打包，编译时会去掉"; fi
  done

  section "6. 浏览器插件文件夹（usr/lib/Captae/extension，$label）"
  local ext="$L/extension" src
  if [[ ! -f "$ext/manifest.json" ]]; then
    fail "没有插件文件夹或 manifest.json"
  else
    pass "插件文件夹在，manifest 版本 $(python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))["version"])' "$ext/manifest.json" 2>/dev/null)"
    if [[ -f "$ext/popup/index.html" && -f "$ext/popup/assets/popup.js" ]]; then pass "popup/ 有构建产物"; else fail "popup/ 里没有构建产物"; fi
    src="$(find "$ext" \( -name '*.tsx' -o -name '*.ts' -o -name 'node_modules' -o -name '*.map' -o -name 'package.json' \) 2>/dev/null | sed "s#^$ext/##" | tr '\n' ' ')"
    if [[ -n "$src" ]]; then fail "插件文件夹里有源码或开发文件：$src"; else pass "插件文件夹里没有源码、node_modules、source map"; fi
    if diff -rq "$ext" "$ROOT/extension" >/dev/null 2>&1; then pass "和仓库里的 extension/ 一样"; else warn "和仓库里的 extension/ 不一样（打包以后又改过插件？）：$(diff -rq "$ext" "$ROOT/extension" 2>&1 | head -3 | tr '\n' ' ')"; fi
  fi

  section "7. 许可证和第三方声明（$label）"
  local missing="" d
  if [[ -f "$L/THIRD_PARTY_NOTICES.md" ]]; then pass "THIRD_PARTY_NOTICES.md 在"; else fail "没有 THIRD_PARTY_NOTICES.md"; fi
  # Captae 自己是 AGPL-3.0：分发二进制要附许可证原文
  if cmp -s "$L/LICENSE" "$ROOT/LICENSE" 2>/dev/null; then pass "LICENSE（AGPL-3.0）在"; else fail "没有 LICENSE，或者和仓库里的不一样"; fi
  for d in yt-dlp ffmpeg aria2 deno; do [[ -d "$L/licenses/$d" ]] || missing="$missing $d"; done
  if [[ ! -f "$L/licenses/README.md" ]]; then fail "没有 licenses/README.md"; elif [[ -n "$missing" ]]; then fail "licenses/ 里缺：$missing"; else pass "licenses/ 在，附带程序各有自己的目录"; fi
  if [[ -f "$L/yt-dlp/_internal/THIRD_PARTY_LICENSES.txt" ]]; then pass "yt-dlp 目录版自带的 THIRD_PARTY_LICENSES.txt 在"; else fail "yt-dlp/_internal/THIRD_PARTY_LICENSES.txt 不在"; fi
  if [[ "$label" == appimage ]]; then
    # 随带的系统库要都列在 licenses/linux-appimage-libs.md 里（许可证和源码获取方式）
    local list="$L/licenses/linux-appimage-libs.md" unlisted="" nlib=0 f
    while IFS= read -r -d '' f; do
      head -c 4 "$R/$f" 2>/dev/null | grep -q $'\x7fELF' || continue
      nlib=$((nlib + 1))
      grep -q -F " $(basename "$f") " "$list" 2>/dev/null || unlisted="$unlisted $(basename "$f")"
    done < <(cd "$R" && find . -type f ! -path './usr/lib/Captae/*' ! -path './usr/bin/captae-desktop' ! -name 'AppRun*' -print0)
    if [[ ! -f "$list" ]]; then fail "没有 licenses/linux-appimage-libs.md（AppImage 随带了 $nlib 个系统库和辅助程序；在打包的 Ubuntu 上运行 scripts/gen-appimage-libs.sh 生成）"
    elif [[ -n "$unlisted" ]]; then fail "AppImage 随带的这些没有列在 licenses/linux-appimage-libs.md 里（重新运行 scripts/gen-appimage-libs.sh）：$(cut -c 1-300 <<<"$unlisted")"
    else pass "AppImage 随带的 $nlib 个系统库和辅助程序都列在 linux-appimage-libs.md 里"; fi
  fi
  if diff -rq "$L/licenses" "$ROOT/licenses" >/dev/null 2>&1 && diff -q "$L/THIRD_PARTY_NOTICES.md" "$ROOT/THIRD_PARTY_NOTICES.md" >/dev/null 2>&1; then
    pass "和仓库里的 licenses/、THIRD_PARTY_NOTICES.md 一样"
  else
    warn "和仓库里的 licenses/ 或 THIRD_PARTY_NOTICES.md 不一样（打包以后又改过？）"
  fi

  section "8. 桌面入口和软件中心元数据（$label）"
  local desk="$R/usr/share/applications/Captae.desktop" meta="$R/usr/share/metainfo/app.captae.desktop.metainfo.xml" out icon
  if [[ ! -f "$desk" ]]; then
    fail "没有 usr/share/applications/Captae.desktop"
  else
    grep -q "^Exec=$MAIN_BIN" "$desk" && grep -q "^Icon=$MAIN_BIN" "$desk" && pass "Captae.desktop：Exec、Icon 都是 $MAIN_BIN" || fail "Captae.desktop 的 Exec / Icon 不对：$(grep -E '^(Exec|Icon)=' "$desk" | tr '\n' ' ')"
    grep -q '^Name\[zh_CN\]\|^Comment\[zh_CN\]' "$desk" && pass "Captae.desktop 有中文说明" || warn "Captae.desktop 没有中文说明"
    if command -v desktop-file-validate >/dev/null; then
      if out="$(desktop-file-validate "$desk" 2>&1)" && [[ -z "$out" ]]; then pass "desktop-file-validate 通过"; else fail "desktop-file-validate：$(tr '\n' ' ' <<<"$out" | cut -c 1-300)"; fi
    else
      warn "没有 desktop-file-validate（apt install desktop-file-utils），跳过"
    fi
  fi
  icon="$(ls "$R"/usr/share/icons/hicolor/*/apps/$MAIN_BIN.png 2>/dev/null | sed -E 's#.*/hicolor/([^/]+)/.*#\1#' | sort -n | tr '\n' ' ')"
  if [[ "$icon" == *512x512* && "$icon" == *256x256* ]]; then pass "图标：$icon"; else fail "图标不齐（要有 256x256 和 512x512）：${icon:-没有}"; fi
  if [[ ! -f "$meta" ]]; then
    fail "没有 usr/share/metainfo/app.captae.desktop.metainfo.xml"
  elif command -v appstreamcli >/dev/null; then
    # --no-net：不去核对网址能不能打开。只有提示（I:）、注意（P:）算过
    out="$(appstreamcli validate --no-net --pedantic "$meta" 2>&1)"
    if grep -q -E '^(E|W):' <<<"$out"; then fail "appstreamcli validate：$(grep -E '^(E|W):' <<<"$out" | head -3 | tr '\n' ' ')"; else pass "appstreamcli validate 通过（$(tail -1 <<<"$out")）"; fi
  else
    warn "没有 appstreamcli（apt install appstream），跳过元数据检查"
  fi
}

for PKG in "${PKGS[@]}"; do
  if [[ ! -f "$PKG" ]]; then
    echo "找不到：$PKG" >&2
    exit 2
  fi
  bytes="$(stat -c %s "$PKG")"
  case "$PKG" in
    *.deb)
      echo "检查 $(basename "$PKG")"
      command -v dpkg-deb >/dev/null || { echo "没有 dpkg-deb" >&2; exit 2; }
      section "1. 安装包（deb）"
      R="$WORK/deb"
      mkdir -p "$R"
      dpkg-deb -x "$PKG" "$R"
      pass "安装包 $(mb "$bytes")（$bytes 字节）；装好以后约 $(mb $(($(du -sk "$R" | awk '{print $1}') * 1024)))"
      field() { dpkg-deb -f "$PKG" "$1"; }
      if [[ "$(field Package)" == captae ]]; then pass "包名 captae"; else warn "包名是 $(field Package)"; fi
      if [[ "$(field Version)" == "$want_ver" ]]; then pass "版本 $want_ver"; else fail "版本 $(field Version)（tauri.conf.json 是 ${want_ver}）"; fi
      pass "架构 $(field Architecture)"
      dep="$(field Depends)"
      miss=""
      for p in libwebkit2gtk-4.1-0 libgtk-3-0 xdg-utils ca-certificates gstreamer1.0-plugins-good gstreamer1.0-libav; do grep -q -w "$p" <<<"$dep" || miss="$miss $p"; done
      if [[ -z "$miss" ]]; then pass "依赖：$dep"; else fail "依赖里缺：${miss}（现在是：${dep}）"; fi
      check_tree "$R" deb
      ;;
    *.AppImage)
      echo
      echo "检查 $(basename "$PKG")"
      section "1. 安装包（AppImage）"
      pass "AppImage $(mb "$bytes")（$bytes 字节）"
      [[ -x "$PKG" ]] || chmod +x "$PKG" 2>/dev/null || true
      R="$WORK/appimage/squashfs-root"
      mkdir -p "$WORK/appimage"
      if ! (cd "$WORK/appimage" && "$PKG" --appimage-extract >/dev/null 2>&1); then
        fail "解不开 AppImage（--appimage-extract 失败；CPU 架构和这台机器不一样？）"
        continue
      fi
      if [[ -x "$R/AppRun" || -L "$R/AppRun" ]]; then pass "有 AppRun"; else fail "没有 AppRun"; fi
      if [[ -f "$R/Captae.desktop" || -n "$(ls "$R"/*.desktop 2>/dev/null)" ]]; then pass "根目录有 .desktop：$(cd "$R" && ls *.desktop | tr '\n' ' ')"; else fail "根目录没有 .desktop"; fi
      if [[ -e "$R/.DirIcon" ]]; then pass "有 .DirIcon（文件管理器里显示的图标）"; else warn "没有 .DirIcon"; fi
      check_tree "$R" appimage
      ;;
    *)
      echo "不认识的文件：$PKG（要 .deb 或 .AppImage）" >&2
      exit 2
      ;;
  esac
done

echo
if [[ $FAILS == 0 ]]; then
  echo "全部通过（提醒 $WARNS 条）"
  exit 0
fi
echo "没过 $FAILS 项（提醒 $WARNS 条）"
exit 1
