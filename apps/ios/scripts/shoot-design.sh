#!/bin/zsh
# 设计稿截图：编译、装到 Captae 专用模拟器，按 `-captaeDemo <编号>-<状态>` 逐个打开、截图。
# 用法：AD=<device_open 给的 agent-device 路径> scripts/shoot-design.sh [状态 ...]
#   不给状态时截全部 24 张；给了就只截这几张（如 01-list 02-pick）。
# 点按、截图都走设备面板的 agent-device（用户在面板里看着）；simctl 只用来安装。
set -euo pipefail
cd "${0:A:h}/.."
UDID=656022B7-F7E2-4289-A179-6108FFEB363F
OUT=../../work/mobile/design/ios
DD=../../target/ios-dd
: "${AD:?先用 device_open 打开 captae-iphone，把返回的 agent-device 路径放进 AD}"

xcodegen generate >/dev/null
xcodebuild -project Captae.xcodeproj -scheme Captae -destination "id=$UDID" -derivedDataPath $DD build -quiet
xcrun simctl install $UDID $DD/Build/Products/Debug-iphonesimulator/Captae.app

ALL=(01-empty 01-list 01-errors 02-running 02-pick 02-failed 03-empty 03-grid 03-offline 04-video 04-remote 04-saving
  05-none 05-online 05-offline 06-scan 06-code 06-failed 07-main 07-proxy 07-about 08-reading 08-ready 08-offline)
mkdir -p $OUT
for s in ${@:-$ALL}; do
  $AD open app.captae.mobile --relaunch --launch-args -captaeDemo --launch-args $s >/dev/null
  # 等 sheet、全屏页弹出来停稳
  sleep 3
  $AD screenshot $OUT/$s.png --pixel-density 3 | tail -1
done
