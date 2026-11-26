#!/bin/zsh
# 第二版设计稿截图（BRIEF 5.1）：编译、装到 Captae 专用模拟器，按 `-captaeDemo <编号>-<状态>` 逐个打开、截图。
# 用法：AD=<device_open 给的 agent-device 路径> scripts/shoot-design-v2.sh [状态 ...]
#   不给状态时截全部；给了就只截这几张（如 01-list 09-menu）。SKIP_BUILD=1 跳过编译安装。
# 点按、滑动、长按、改字号、截图都走设备面板的 agent-device（用户在面板里看着）；simctl 只用来安装。
# 要靠操作才出来的状态：01-list-scrolled 往上滑、01-list-large 改成辅助功能 XL 字号、09-menu 点标题、11-context 长按任务卡。
set -euo pipefail
cd "${0:A:h}/.."
UDID=656022B7-F7E2-4289-A179-6108FFEB363F
OUT=../../work/mobile/design/ios-v2
DD=../../target/ios-dd
: "${AD:?先用 device_open 打开 captae-iphone，把返回的 agent-device 路径放进 AD}"

if [[ -z "${SKIP_BUILD:-}" ]]; then
  xcodegen generate >/dev/null
  xcodebuild -project Captae.xcodeproj -scheme Captae -destination "id=$UDID" -derivedDataPath $DD build -quiet
  xcrun simctl install $UDID $DD/Build/Products/Debug-iphonesimulator/Captae.app
fi

ALL=(01-empty 01-list 01-list-mac 01-list-scrolled 01-errors 01-list-large
  02-running 02-pick 02-failed 03-grid 03-grid-mac 03-offline 03-empty 04-video 04-remote 04-saving
  05-none 05-online 05-offline 06-scan 06-code 06-failed 07-main 07-proxy 07-about 08-reading 08-ready 08-offline
  09-menu 10-composer 10-composer-reading 10-composer-redirect 11-context 12-search)
mkdir -p $OUT
# 不往剪贴板里写东西：这台模拟器开着和 Mac 同步剪贴板，写进去会改掉 Mac 上的剪贴板（AGENTS.md：不动系统剪贴板）。
# 系统的「粘贴」按钮在剪贴板为空时是灰的，截出来就是灰的

launch() {
  $AD open app.captae.mobile --relaunch --launch-args -captaeDemo --launch-args $1 >/dev/null
  # 等 sheet、全屏页弹出来停稳
  sleep 3
}

for s in ${@:-$ALL}; do
  case $s in
    01-list-scrolled)
      launch 01-list
      # 往下滚：标签栏和下载条缩成一行
      $AD swipe 200 640 200 300 >/dev/null
      sleep 2
      ;;
    01-list-large)
      $AD settings text-size accessibility-extra-large >/dev/null
      launch 01-list
      ;;
    09-menu)
      launch 01-list
      $AD press 'label="全部设备, 操作菜单"' >/dev/null
      sleep 2
      ;;
    11-context)
      launch 01-list
      # 长按第三张卡（Sintel · 完整版短片，正在下）
      $AD longpress 'label="Sintel · 完整版短片、哔哩哔哩 · MP4 · 1080p"' 900 >/dev/null
      sleep 2
      ;;
    *)
      launch $s
      ;;
  esac
  $AD screenshot $OUT/$s.png --pixel-density 3 | tail -1
  if [[ $s == 01-list-large ]]; then
    $AD settings text-size large >/dev/null
  fi
done
