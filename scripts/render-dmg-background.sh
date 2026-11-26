#!/usr/bin/env bash
# 重新生成安装包（dmg）窗口的背景图：apps/desktop/src-tauri/dmg/background.png（1 倍）和 background@2x.png（2 倍）。
#
# 背景图的源码是 apps/desktop/src/dmg-background/（React 页面，和界面同一套颜色、字和图标），窗口大小、图标位置在
# apps/desktop/src-tauri/dmg/layout.json。改了其中任何一个以后运行一次，把两张图一起提交；打包（scripts/build-release.sh）直接用提交的图。
#
# 用无界面 Chrome 按 1 倍、2 倍各截一次（不是把 2 倍缩小：1 倍单独排版，字更清楚）。用一个临时的浏览器数据目录，用完删掉，不碰平时用的 Chrome。
# 需要：apps/desktop 里装过依赖（npm ci）；Google Chrome，或者用 CHROME=<可执行文件> 指定别的 Chromium 内核浏览器。
# 用法：scripts/render-dmg-background.sh
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DMG_DIR="$ROOT/apps/desktop/src-tauri/dmg"
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
if [[ ! -x "$CHROME" ]]; then
  echo "找不到 Chrome：${CHROME}（用 CHROME=<路径> 指定）" >&2
  exit 2
fi

(cd "$ROOT/apps/desktop" && npm run --silent build:dmg-bg)
page="$ROOT/apps/desktop/dist-dmg/index.html"

# 图比窗口的内容区（window）多出标题栏那么高（titleBar）：多出来的一截在最下面，只有底色（为什么见 dmg/settings.py）
read -r W H < <(python3 -c 'import json, sys; l = json.load(open(sys.argv[1])); print(l["window"]["width"], l["window"]["height"] + l["titleBar"])' "$DMG_DIR/layout.json")

PROFILE="$(mktemp -d)"
trap 'rm -rf "$PROFILE"' EXIT

for scale in 1 2; do
  out="$DMG_DIR/background.png"
  [[ $scale == 2 ]] && out="$DMG_DIR/background@2x.png"
  # --force-color-profile=srgb：截出来的是 sRGB，不带这台电脑显示器的色彩配置
  "$CHROME" --headless=new --hide-scrollbars --force-color-profile=srgb --no-first-run --no-default-browser-check \
    --disable-extensions --user-data-dir="$PROFILE" --window-size="$W,$H" --force-device-scale-factor="$scale" \
    --virtual-time-budget=3000 --screenshot="$out" "file://$page" >/dev/null 2>&1
  # Chrome 写出来的文件带隔离属性（com.apple.quarantine），去掉，免得跟着进 dmg
  xattr -d com.apple.quarantine "$out" 2>/dev/null || true
  got="$(sips -g pixelWidth -g pixelHeight "$out" | awk '/pixel(Width|Height)/ { printf "%s ", $2 }')"
  want="$((W * scale)) $((H * scale)) "
  if [[ "$got" != "$want" ]]; then
    echo "截出来的大小不对：$out 是 ${got}，应该是 ${want}" >&2
    exit 1
  fi
  echo "${out}（${got% }）"
done
