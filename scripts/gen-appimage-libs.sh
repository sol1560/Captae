#!/usr/bin/env bash
# 列出 Linux AppImage 里随带的系统库（linuxdeploy 和它的 GTK 插件从打包机器拷进去的 GTK、WebKitGTK、GLib……），
# 生成 licenses/linux-appimage-libs.md（每个库属于哪个 Ubuntu 包、版本、源码包、许可证）和
# licenses/linux-appimage-copyright.txt（这些包的 /usr/share/doc/<包>/copyright 原文，加上它们引用的 LGPL / GPL 全文）。
#
# 要在 Ubuntu 22.04 上运行（用 dpkg 查库文件属于哪个包，AppImage 里的库在这台机器上也要装着）。
# 随带的库一般只在换打包机器的系统版本、换 Tauri 版本时才变；变了以后重新运行、把结果提交进仓库，再打一次包（licenses/ 会打进包里）。
# scripts/check-bundle-linux.sh 会核对 AppImage 里的库都列在 linux-appimage-libs.md 里。
#
# 可以一次给几个 AppImage，列的是它们随带的库的并集：随带哪些看打包机器上装了什么（GTK 插件把整个 GIO 模块目录拷进去，
# GStreamer 插件看装了哪几个插件包），发版的包在 GitHub 的机器上打，和本机打的不一定一样。0.3.0 起把本机打的和上一版 Release 上的
# AppImage 一起给（上一版是 GitHub 的机器打的；0.2.0 那份带着 libwayland-client.so.0，Devin 那台 Ubuntu 打的没有，
# 那台打的又多 5 个 GIO 模块），两边都盖住，发版时 GitHub 上的检查才过得去。多列了几个包里没有的库不要紧（多给了许可证）。
#
# 用法：scripts/gen-appimage-libs.sh <Captae_*.AppImage> [<另一个 AppImage> …]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
[[ $# -ge 1 ]] || { echo "用法：scripts/gen-appimage-libs.sh <AppImage> [<另一个 AppImage> …]" >&2; exit 2; }
command -v dpkg >/dev/null || { echo "要在 Ubuntu / Debian 上运行（用 dpkg 查库属于哪个包）" >&2; exit 2; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/appimage-libs.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT

# AppImage 里不是 Captae 自己的 ELF 文件（库、GTK / WebKit 的辅助程序和模块），路径相对 AppImage 的根
list_bundled() {
  (cd "$1" && find . -type f ! -path './usr/lib/Captae/*' ! -path './usr/bin/captae-desktop' ! -name 'AppRun*' -print0 |
    while IFS= read -r -d '' f; do
      # 用 if 不用 &&：最后一个文件不是 ELF 时 && 让循环以非 0 结束，set -e 加 pipefail 下整个脚本就退出了
      if head -c 4 "$f" 2>/dev/null | grep -q $'\x7fELF'; then echo "${f#./}"; fi
    done)
}
n=0
for a in "$@"; do
  n=$((n + 1))
  a="$(cd "$(dirname "$a")" && pwd)/$(basename "$a")"
  [[ -x "$a" ]] || chmod +x "$a"
  mkdir -p "$WORK/x$n"
  (cd "$WORK/x$n" && "$a" --appimage-extract >/dev/null)
  list_bundled "$WORK/x$n/squashfs-root" >"$WORK/files$n"
  echo "$(basename "$a")：随带 $(wc -l <"$WORK/files$n") 个"
  rm -rf "$WORK/x$n"
done
cat "$WORK"/files[0-9]* | LC_ALL=C sort -u >"$WORK/files"

# 每个文件找回打包机器上的原件属于哪个包（按文件名找；找不到的列成「未知」）。
# 不算 Captae 自己的 deb：打包机器上装过 Captae 时，yt-dlp 目录版自带的 libcrypto.so.3、libzstd.so.1 也叫这个名字，
# 原来会排在前面被当成「captae 包里的库」（0.2.0 的清单第一行就是这样）
: >"$WORK/map"
while read -r f; do
  name="$(basename "$f")"
  pkg="$(dpkg -S "/$name" 2>/dev/null | grep -v -e '^diversion' -e '^captae[:,]' | head -1 | cut -d: -f1 || true)"
  if [[ -z "$pkg" ]]; then
    pkg="$(dpkg -S "*/$name" 2>/dev/null | grep -v -e '^diversion' -e '^captae[:,]' | head -1 | cut -d: -f1 || true)"
  fi
  printf '%s\t%s\n' "${pkg:-未知}" "$f" >>"$WORK/map"
done <"$WORK/files"

out="$ROOT/licenses/linux-appimage-libs.md"
txt="$ROOT/licenses/linux-appimage-copyright.txt"
os="$(. /etc/os-release && echo "$PRETTY_NAME")"
{
  echo "# Linux AppImage 里随带的系统库"
  echo
  echo "由 \`scripts/gen-appimage-libs.sh\` 生成（不要手改）。AppImage 要在不同的发行版上直接运行，打包时 linuxdeploy 和它的 GTK 插件"
  echo "把下面这些库和辅助程序从打包机器（${os}）拷进了 AppImage，原样、没有改动，都是动态链接。随带哪些看打包机器上装了什么，"
  echo "表里是几台打包机器打的 AppImage 合起来的（发版的包不一定每个都带）；「版本」是生成这份清单的机器上装的版本，发版的打包机器上可能差一个小版本（同一个 Ubuntu 发行版）。"
  echo ".deb 不带这些，用系统自己的。"
  echo
  echo "AppImage 的启动部分不在表里：开头的 runtime 和根目录的 AppRun / AppRun.wrapped 来自 AppImage 项目（MIT，"
  echo "https://github.com/AppImage/type2-runtime 、https://github.com/AppImage/AppImageKit ，Tauri 打包时下载的构建）。"
  echo
  echo "源码：每个包对应的 Ubuntu 源码包（表里「源码」一列的链接，或在 Ubuntu 上 \`apt-get source <源码包>=<版本>\`）。"
  echo "LGPL 的库可以换成用户自己编译的版本：把同名的 .so 放进解开的 AppImage（\`--appimage-extract\`）的 \`usr/lib/\` 即可。"
  echo "各包的版权声明和许可证原文（Debian 的 copyright 文件，及其中引用的 LGPL / GPL 全文）见 \`linux-appimage-copyright.txt\`。"
  echo
  echo "| 包 | 版本 | 源码 | 许可证（copyright 文件里列出的） | AppImage 里的文件 |"
  echo "|---|---|---|---|---|"
  cut -f1 "$WORK/map" | LC_ALL=C sort -u | while read -r pkg; do
    files="$(awk -F'\t' -v p="$pkg" '$1==p {print $2}' "$WORK/map" | sed 's#.*/##' | LC_ALL=C sort -u | tr '\n' ' ')"
    if [[ "$pkg" == 未知 ]]; then
      echo "| （不属于已装的包） | — | — | — | $files |"
      continue
    fi
    ver="$(dpkg-query -W -f='${Version}' "$pkg" 2>/dev/null || echo '?')"
    src="$(dpkg-query -W -f='${source:Package}' "$pkg" 2>/dev/null || echo "$pkg")"
    srcver="$(dpkg-query -W -f='${source:Version}' "$pkg" 2>/dev/null || echo "$ver")"
    cr="/usr/share/doc/${pkg%%:*}/copyright"
    lic="$(grep -h -E '^License:' "$cr" 2>/dev/null | sed 's/^License:[[:space:]]*//' | sort -u | tr '\n' ';' | sed 's/;$//; s/;/；/g' || true)"
    echo "| $pkg | $ver | [$src $srcver](https://launchpad.net/ubuntu/+source/$src/$srcver) | ${lic:-见 copyright 文件} | $files |"
  done
} >"$out"

{
  echo "Linux AppImage 里随带的系统库的版权声明和许可证（Debian / Ubuntu 包的 copyright 文件原文）。由 scripts/gen-appimage-libs.sh 生成。"
  cut -f1 "$WORK/map" | LC_ALL=C sort -u | grep -v '^未知$' | while read -r pkg; do
    cr="/usr/share/doc/${pkg%%:*}/copyright"
    echo
    echo "================================================================================"
    echo "$pkg（$(dpkg-query -W -f='${Version}' "$pkg" 2>/dev/null)）"
    echo "================================================================================"
    if [[ -f "$cr" ]]; then cat "$cr"; else echo "（这个包没有 copyright 文件）"; fi
  done
  # copyright 文件里只写「见 /usr/share/common-licenses/xxx」的，把全文附上
  for l in LGPL-2 LGPL-2.1 LGPL-3 GPL-2 GPL-3 MPL-1.1 MPL-2.0 Apache-2.0; do
    if grep -q "/usr/share/common-licenses/$l\b" /usr/share/doc/*/copyright 2>/dev/null && cut -f1 "$WORK/map" | grep -v '^未知$' | while read -r pkg; do grep -q "common-licenses/$l\b" "/usr/share/doc/${pkg%%:*}/copyright" 2>/dev/null && echo y; done | grep -q y; then
      echo
      echo "================================================================================"
      echo "/usr/share/common-licenses/$l"
      echo "================================================================================"
      cat "/usr/share/common-licenses/$l"
    fi
  done
} >"$txt"

echo "licenses/linux-appimage-libs.md：$(cut -f1 "$WORK/map" | sort -u | wc -l) 个包、$(wc -l <"$WORK/files") 个文件"
echo "licenses/linux-appimage-copyright.txt：$(wc -c <"$txt") 字节"
if grep -q '^未知' "$WORK/map"; then
  echo "提醒：这些文件找不到属于哪个包：$(grep '^未知' "$WORK/map" | cut -f2 | tr '\n' ' ')" >&2
fi
