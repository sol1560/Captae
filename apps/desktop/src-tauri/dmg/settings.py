# dmgbuild 的设置：打开安装包（dmg）时访达窗口里的样子。dmgbuild 直接写 .DS_Store，不用访达、不用 AppleScript，
# 屏幕锁着、没人看着时也能打包，每次打出来都一样。scripts/build-release.sh 这样调用：
#
#   dmgbuild -s apps/desktop/src-tauri/dmg/settings.py -D app=<Captae.app> -D dir=<这个目录> Captae <输出的 .dmg>
#
# 窗口大小、两个图标的位置在同一目录的 layout.json：背景图那一页（apps/desktop/src/dmg-background/）也读它，图标才对得上背景。
#   window：标题栏下面的内容区（背景图按它设计）；titleBar：访达窗口标题栏的高度（macOS 26 起 32 点）；
#   app、applications：两个图标中心在内容区里的位置。
# 背景图是这个目录的 background.png 和 background@2x.png（scripts/render-dmg-background.sh 生成），dmgbuild 自动合成一张带高清版的 TIFF。
# dmgbuild 执行这个文件时，下面这些名字（format、files、symlinks……）就是它的设置项；defines 是 -D 传进来的值。
import json
import os.path

app = defines["app"]  # noqa: F821（dmgbuild 执行时传进来）
here = defines["dir"]  # noqa: F821

with open(os.path.join(here, "layout.json"), encoding="utf-8") as f:
    layout = json.load(f)

# 镜像：lzfse 压缩（macOS 10.11 起都能打开），HFS+
format = "ULFO"
filesystem = "HFS+"

files = [app]
# 指向「应用程序」文件夹的快捷方式。名字用英文 Applications，和背景上那句「Drag Captae to Applications」一致：
# 安装包窗口只有一种语言，用英文（docs/I18N.md 第 1 节）
symlinks = {"Applications": "/Applications"}

# 磁盘图标用应用图标（和 Tauri 原来打的 dmg 一样）
icon = os.path.join(app, "Contents", "Resources", "icon.icns")

background = os.path.join(here, "background.png")

# 窗口：没有工具栏、侧栏、状态栏和路径栏，只有背景和两个图标。
# .DS_Store 里的窗口大小是连标题栏一起算的（2026-11 在 macOS 26 的访达里实测：写 660×400，内容区只有 660×368，背景图下面被切掉 32 点），
# 所以高度加上 titleBar。背景图也比内容区多画这么高的一截底色：标题栏矮的旧系统上内容区高几点，露出来的是同色，不会多一条白边。
w = layout["window"]
window_rect = ((w["x"], w["y"]), (w["width"], w["height"] + layout["titleBar"]))
default_view = "icon-view"
show_toolbar = False
show_sidebar = False
show_status_bar = False
show_pathbar = False
show_tab_view = False
show_icon_preview = False
show_item_info = False
arrange_by = None
label_pos = "bottom"
icon_size = layout["iconSize"]
text_size = layout["textSize"]
icon_locations = {
    os.path.basename(app): (layout["app"]["x"], layout["app"]["y"]),
    "Applications": (layout["applications"]["x"], layout["applications"]["y"]),
}
