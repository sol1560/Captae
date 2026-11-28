#!/usr/bin/env bash
# 打 Linux 安装包（x86_64 的 AppImage 和 .deb）：准备附带程序（scripts/fetch-sidecars-linux.sh）→ cargo tauri build → 检查（scripts/check-bundle-linux.sh）。
#
# 在 Ubuntu 22.04（glibc 2.35）上打：程序只能在 glibc 不比打包机器旧的系统上跑，22.04 打出来的包 Ubuntu 22.04+、Debian 12、Fedora 39 都能用。
# 更新的系统上打的包，老一点的发行版会报「GLIBC_2.xx not found」，所以在别的系统上打时会提醒。
#
# 和直接运行 `cargo tauri build` 的区别（照 macOS 的 build-release.sh）：
# 1. 编译时把打包这台电脑上的路径从程序里去掉（rustc 的 --remap-path-prefix、C 代码的 -ffile-prefix-map），
#    check-bundle-linux.sh 检查程序里还有没有主目录的路径。
# 2. 用 tauri.release.linux.conf.json：附带程序不走 externalBin，作为 resources 平铺进 /usr/lib/Captae/（原因见 fetch-sidecars-linux.sh 开头），
#    带上插件文件夹、licenses/、THIRD_PARTY_NOTICES.md、AppStream 元数据，自定义 .desktop 模板，deb 的依赖。
# 3. AppImage 分两步打：Tauri 打 AppImage 时用 linuxdeploy 处理 AppDir 里所有现成的 ELF 文件——找它们的依赖拷进来、用 patchelf 改 rpath。
#    附带程序不能让它动：yt-dlp 目录版里 PyInstaller 带的 libreadline 依赖系统没有的 libtinfo.so.5，linuxdeploy 直接失败；
#    就算不失败，改 PyInstaller 程序和 deno、ffmpeg 的 rpath 也没有必要、有风险。所以先让 Tauri 打一份不带附带程序的
#    （再叠一层 tauri.release.linux-appimage.conf.json，把那两项 resources 去掉），linuxdeploy 只处理桌面端和 GTK、WebKitGTK；
#    再解开这个 AppImage，把附带程序放进 usr/lib/Captae/（和 deb 同一个位置），用它原来的 runtime 和压缩方式重新打包。
#
# 用法：scripts/build-release-linux.sh [--install-deps]
#   --install-deps  先用 apt-get 装编译要的系统包（要 root 或 sudo；Ubuntu / Debian），没有 tauri-cli、或者版本低于 2.12 时用 cargo install 装 2.12.6
# 需要：Rust（rustup，1.93 以上）、Node 20.19 / 22.12 以上、tauri-cli 2.12 以上（cargo install tauri-cli --version 2.12.6 --locked；tauri 是 2.12，应用内更新的签名要 signer sign --app-version），
#       Tauri 的系统依赖（libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev），
#       附带程序要的（build-essential curl unzip xz-utils gnupg perl pkg-config python3 file），
#       AppImage 要的 xdg-utils（Tauri 把 /usr/bin/xdg-open 拷进 AppImage）、squashfs-tools（重新打包），
#       检查要的（desktop-file-utils appstream，没有时那两项跳过）。
#       AppImage 带上 GStreamer 的插件（bundleMediaFramework）：素材库里播放视频要 WebKitGTK 经 GStreamer 解 H.264 / AAC，
#       打包机器上要装 gstreamer1.0-plugins-base / -good / gstreamer1.0-libav（deb 版靠依赖从系统装，见 tauri.release.linux.conf.json）。
# 在 Mac 上：只能在 Docker 里验证脚本（Apple Silicon 上是 arm64，打出来的不是正式包），见 work/winlinux/C/NOTES.md。
# 产物：$CARGO_TARGET_DIR（默认 target）/release/bundle/deb/Captae_<版本>_amd64.deb、bundle/appimage/Captae_<版本>_amd64.AppImage
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CARGO_DIR="${CARGO_HOME:-$HOME/.cargo}"
TARGET_DIR="${CARGO_TARGET_DIR:-$ROOT/target}"

if [[ "$(uname -s)" != Linux ]]; then
  echo "只能在 Linux 上打 Linux 包" >&2
  exit 2
fi

# tauri-cli 要 2.12 以上（见开头的「需要」）。GitHub 的机器上 rust-cache 可能留着旧的，所以不光看装没装，还看版本
TAURI_CLI=2.12.6
tauri_cli_ok() { cargo tauri --version 2>/dev/null | grep -qE 'tauri-cli 2\.(1[2-9]|[2-9][0-9])\.'; }

if [[ "${1:-}" == "--install-deps" ]]; then
  sudo=()
  [[ "$(id -u)" != 0 ]] && sudo=(sudo)
  "${sudo[@]}" apt-get update
  "${sudo[@]}" env DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
    build-essential curl ca-certificates pkg-config file git unzip xz-utils gnupg perl python3 xdg-utils squashfs-tools \
    libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libssl-dev libxdo-dev \
    desktop-file-utils appstream \
    gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-libav
  if ! tauri_cli_ok; then
    cargo install tauri-cli --version "$TAURI_CLI" --locked --force
  fi
fi

for c in cargo node npm xdg-open mksquashfs unsquashfs; do
  command -v "$c" >/dev/null || { echo "缺少 ${c}（见开头的「需要」）" >&2; exit 2; }
done
tv="$(cargo tauri --version 2>/dev/null)"
if ! tauri_cli_ok; then
  echo "要 tauri-cli 2.12 以上（现在是 ${tv:-没装}）：cargo install tauri-cli --version ${TAURI_CLI} --locked（或加 --install-deps）" >&2
  exit 2
fi
echo "$tv；$(rustc --version)；node $(node --version)"

glibc="$(ldd --version 2>/dev/null | head -1 | grep -o -E '[0-9]+\.[0-9]+$' || true)"
if [[ -n "$glibc" && "$glibc" != 2.35 ]]; then
  echo "提醒：这台机器的 glibc 是 ${glibc}，不是 Ubuntu 22.04 的 2.35；打出来的包在 glibc 比 ${glibc} 旧的系统上跑不起来" >&2
fi

# ---------- 去掉本机路径（同 build-release.sh） ----------
# 多条规则都对得上时，rustc 和 gcc 都用最后一条：先写最宽的（主目录），再写具体的
remaps=("$HOME=~")
for d in "$CARGO_DIR"/registry/src/*/; do
  [[ -d "$d" ]] && remaps+=("${d%/}=/cargo/registry")
done
[[ -d "$CARGO_DIR/git/checkouts" ]] && remaps+=("$CARGO_DIR/git/checkouts=/cargo/git")
remaps+=("$ROOT=/captae")
[[ "$TARGET_DIR" != "$ROOT/target" ]] && remaps+=("$TARGET_DIR=/captae/target")
for r in "${remaps[@]}"; do
  if [[ "$r" == *" "* ]]; then
    echo "路径里有空格，没法去掉：$r" >&2
    exit 2
  fi
done
rust_flags=()
c_flags=()
for r in "${remaps[@]}"; do
  rust_flags+=("--remap-path-prefix=$r")
  c_flags+=("-ffile-prefix-map=$r")
done
# CARGO_ENCODED_RUSTFLAGS 用 0x1f 分开每个参数；设了它，RUSTFLAGS 和 cargo 配置里的 rustflags 就不再起作用
CARGO_ENCODED_RUSTFLAGS="$(IFS=$'\x1f'; echo "${rust_flags[*]}")"
export CARGO_ENCODED_RUSTFLAGS
export CFLAGS="${CFLAGS:-} ${c_flags[*]}"
export CXXFLAGS="${CXXFLAGS:-} ${c_flags[*]}"
echo "编译时去掉本机路径：${#remaps[@]} 条规则"

# ---------- 前端依赖 ----------
# 看 npm 装完才写的 .package-lock.json（Docker 里给 node_modules 单独挂一个空卷时，目录在、里面是空的）
if [[ ! -f "$ROOT/apps/desktop/node_modules/.package-lock.json" ]]; then
  (cd "$ROOT/apps/desktop" && npm ci --no-audit --no-fund)
fi

# ---------- 附带程序 ----------
# 附带程序自己的编译（aria2 和它的依赖库）不带上面的 C 编译参数：那些是官方源码包，路径在 target/sidecar-cache 下，
# 去掉也没有个人信息；带上反而让缓存失效
( unset CFLAGS CXXFLAGS; "$ROOT/scripts/fetch-sidecars-linux.sh" )

# ---------- 编译、打 deb ----------
TAURI_DIR="$ROOT/apps/desktop/src-tauri"
(cd "$TAURI_DIR" && cargo tauri build --config tauri.release.linux.conf.json --bundles deb)

ver="$(python3 -I -c 'import json, sys; print(json.load(open(sys.argv[1]))["version"])' "$TAURI_DIR/tauri.conf.json")"
case "$(uname -m)" in x86_64) darch=amd64 aarch=amd64 ;; aarch64) darch=arm64 aarch=aarch64 ;; *) darch="$(uname -m)" aarch="$(uname -m)" ;; esac
deb="$TARGET_DIR/release/bundle/deb/Captae_${ver}_${darch}.deb"
appimage="$TARGET_DIR/release/bundle/appimage/Captae_${ver}_${aarch}.AppImage"

# ---------- AppImage：先打不带附带程序的，再放进去重新打包（原因见开头第 3 条） ----------
# Tauri 不清空打 AppImage 用的中间目录 bundle/appimage_deb，上一次放进去的附带程序会留在里面，先删掉
rm -rf "$TARGET_DIR/release/bundle/appimage_deb"
(cd "$TAURI_DIR" && cargo tauri bundle --config tauri.release.linux.conf.json --config tauri.release.linux-appimage.conf.json --bundles appimage)
[[ -f "$appimage" ]] || { echo "没找到打出来的 $appimage" >&2; exit 1; }
echo "把附带程序放进 AppImage …"
work="$TARGET_DIR/release/bundle/appimage/repack"
rm -rf "$work"
mkdir -p "$work"
# AppImage 的 runtime 自己会报出 squashfs 从哪里开始（--appimage-offset）、会自己解开（--appimage-extract），都不需要 FUSE
offset="$("$appimage" --appimage-offset)"
comp="$(unsquashfs -s -o "$offset" "$appimage" | sed -n 's/^Compression \([a-z0-9]*\).*/\1/p')"
[[ -n "$offset" && -n "$comp" ]] || { echo "读不出 AppImage 的 squashfs 位置或压缩方式" >&2; exit 1; }
(cd "$work" && "$appimage" --appimage-extract >/dev/null)
mkdir -p "$work/squashfs-root/usr/lib/Captae"
cp -a "$TAURI_DIR"/binaries/linux/bin/* "$TAURI_DIR/binaries/linux/yt-dlp" "$work/squashfs-root/usr/lib/Captae/"
head -c "$offset" "$appimage" >"$work/runtime"
mksquashfs "$work/squashfs-root" "$work/payload.squashfs" -root-owned -noappend -no-progress -quiet -comp "$comp"
cat "$work/runtime" "$work/payload.squashfs" >"$appimage.new"
chmod 755 "$appimage.new"
mv -f "$appimage.new" "$appimage"
rm -rf "$work"
echo "  重新打包好了（runtime ${offset} 字节，squashfs 用 ${comp} 压缩）"

for f in "$deb" "$appimage"; do
  [[ -f "$f" ]] || { echo "没找到打出来的 $f（bundle 目录里有：$(ls "$TARGET_DIR"/release/bundle/*/ 2>/dev/null | tr '\n' ' ')）" >&2; exit 1; }
done
echo
echo "打好了："
ls -l "$deb" "$appimage"
"$ROOT/scripts/check-bundle-linux.sh" "$deb" "$appimage"
