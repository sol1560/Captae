#!/bin/zsh
# 设计稿截图（第二版，BRIEF 5.1）：每个状态用启动参数 captaeDemo=<编号>-<状态> 冷启动一次，截一张图。
# 用法：scripts/shoot-design.sh <agent-device 命令的完整路径> [状态 ...]
#   agent-device 的路径由设备面板的 device_open 给（绑定到 Captae 专用模拟器 captae-phone）；不写状态就截全部。
# 图存到 work/mobile/design/android-v2/<状态>.png（work/ 不入库）。第一版的图在 android/，这个脚本不碰。
# 几张要真的操作才截得到的：
#   01-list-scrolled  往下滚过一屏（标签栏和下载条缩成一行）
#   01-list-large     系统字号调大（agent-device 没有正好 1.3 倍的档，用 1.35 倍那一档，比 1.3 严；截完调回 1.0）
#   09-menu           点开左上角的设备菜单
#   11-context        长按正在下的「Sintel · 完整版短片」
# 坐标是 Pixel 9 尺寸（1080×2424 像素，密度 420）上的像素。
set -e
AD=$1; shift
# 只有 10-composer 要用 adb（切输入法，见下）；序列号是 captae-phone 的
ADB=${ANDROID_HOME:-$HOME/Library/Android/sdk}/platform-tools/adb
SERIAL=${CAPTAE_SERIAL:-emulator-5558}
OUT=${0:A:h}/../../../work/mobile/design/android-v2
mkdir -p $OUT
ALL=(01-empty 01-list 01-list-mac 01-list-scrolled 01-errors 01-list-large
  09-menu 10-composer 10-composer-reading 10-composer-offline 11-context
  03-grid 03-grid-mac 03-offline 03-empty 05-none 05-online 05-offline 12-search
  02-running 02-pick 02-failed 04-video 04-remote 04-saving
  06-scan 06-code 06-failed 07-main 07-proxy 07-about 08-reading 08-ready 08-offline)
LIST=(${@:-$ALL})

launch() {
  $AD open app.captae --relaunch --launch-args --es --launch-args captaeDemo --launch-args $1 > /dev/null
}

for s in $LIST; do
  case $s in
    01-list-large)
      # 字号改了要重启 App 才生效
      $AD settings text-size extra-extra-extra-large > /dev/null
      launch $s; sleep 2.5
      $AD screenshot $OUT/$s.png > /dev/null
      $AD settings text-size large > /dev/null
      ;;
    01-list-scrolled)
      launch $s; sleep 2
      # 慢一点往上推（往下看），标签栏收起
      $AD swipe 540 1700 540 900 > /dev/null; sleep 1.2
      $AD screenshot $OUT/$s.png > /dev/null
      ;;
    09-menu)
      launch $s; sleep 2
      $AD press 210 127 > /dev/null; sleep 1
      $AD screenshot $OUT/$s.png > /dev/null
      ;;
    11-context)
      launch $s; sleep 2
      $AD longpress 700 1300 700 > /dev/null; sleep 1.2
      $AD screenshot $OUT/$s.png > /dev/null
      ;;
    10-composer)
      # 输入卡在窗口拿到焦点 0.35 秒后自己聚焦、弹键盘。agent-device 平时把输入法换成它自己的无界面输入法（用来打字），
      # 屏幕上看不到键盘；它没有换输入法的命令，这一张用 adb 临时切到系统键盘，截完切回它的
      launch $s
      $ADB -s $SERIAL shell ime set com.android.inputmethod.latin/.LatinIME > /dev/null
      sleep 3
      $AD screenshot $OUT/$s.png > /dev/null
      $ADB -s $SERIAL shell ime set com.callstack.agentdevice.imehelper/.TestInputMethodService > /dev/null
      ;;
    *)
      # sheet 类状态晚 0.25 秒弹出，弹簧走完约 0.6 秒；其余等第一帧和图片解码
      launch $s; sleep 2
      $AD screenshot $OUT/$s.png > /dev/null
      ;;
  esac
  echo "$s → $OUT/$s.png"
done
