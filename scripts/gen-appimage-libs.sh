#!/usr/bin/env bash
# 列出 Linux AppImage 里随带的系统库（linuxdeploy 和它的 GTK 插件从打包机器拷进去的 GTK、WebKitGTK、GLib……），
# 生成 licenses/linux-appimage-libs.md（每个库属于哪个 Ubuntu 包、版本、源码包、许可证）和
# licenses/linux-appimage-copyright.txt（这些包的 /usr/share/doc/<包>/copyright 原文，加上它们引用的 LGPL / GPL 全文）。
#
# 要在打出这个 AppImage 的那台 Ubuntu 上运行（用 dpkg 查库文件属于哪个包）。
# 随带的库一般只在换打包机器的系统版本、换 Tauri 版本时才变；变了以后重新运行、把结果提交进仓库，再打一次包（licenses/ 会打进包里）。
# scripts/check-bundle-linux.sh 会核对 AppImage 里的库和 linux-appimage-libs.md 列的是不是同一批。
#
# 用法：scripts/gen-appimage-libs.sh <Captae_*.AppImage>
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APPIMAGE="${1:?用法：scripts/gen-appimage-libs.sh <AppImage>}"
APPIMAGE="$(cd "$(dirname "$APPIMAGE")" && pwd)/$(basename "$APPIMAGE")"
command -v dpkg >/dev/null || { echo "要在 Ubuntu / Debian 上运行（用 dpkg 查库属于哪个包）" >&2; exit 2; }
WORK="$(mktemp -d "${TMPDIR:-/tmp}/appimage-libs.XXXXXX")"
trap 'rm -rf "$WORK"' EXIT
(cd "$WORK" && "$APPIMAGE" --appimage-extract >/dev/null)
R="$WORK/squashfs-root"

# AppImage 里不是 Captae 自己的 ELF 文件（库、GTK / WebKit 的辅助程序和模块）
list_bundled() {
  (cd "$R" && find . -type f ! -path './usr/lib/Captae/*' ! -path './usr/bin/captae-desktop' ! -name 'AppRun*' -print0 |
    while IFS= read -r -d '' f; do
      # 用 if 不用 &&：最后一个文件不是 ELF 时 && 让循环以非 0 结束，set -e 加 pipefail 下整个脚本就退出了
      if head -c 4 "$f" 2>/dev/null | grep -q $'\x7fELF'; then echo "${f#./}"; fi
    done) | LC_ALL=C sort
}
list_bundled >"$WORK/files"

# 每个文件找回打包机器上的原件属于哪个包（按文件名找；找不到的列成「未知」）
: >"$WORK/map"
while read -r f; do
  name="$(basename "$f")"
  pkg="$(dpkg -S "/$name" 2>/dev/null | grep -v '^diversion' | head -1 | cut -d: -f1 || true)"
  if [[ -z "$pkg" ]]; then
    pkg="$(dpkg -S "*/$name" 2>/dev/null | grep -v '^diversion' | head -1 | cut -d: -f1 || true)"
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
  echo "把下面这些库和辅助程序从打包机器（${os}）拷进了 AppImage，原样、没有改动，都是动态链接。"
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
