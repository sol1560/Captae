#!/usr/bin/env bash
# 发版时给 Windows、Linux 的安装包签上应用内更新的签名，生成 latest.json，和 macOS 的更新包一起挂到草稿 Release 上（docs/DESIGN.md 第 15 节）。
# 在 scripts/build-release.sh 打好 macOS 的包（target/release/bundle/updater/ 里有 .app.tar.gz 和 .sig）、
# release-winlinux.yml 把 Windows / Linux 的安装包挂到草稿 Release 以后运行。
#
#   1. 用 gh 下载草稿 Release 上的 Windows 安装程序、deb、AppImage（到 target/release/updater-<版本>/）
#   2. 和 release-winlinux.yml 运行日志里打印的 SHA256（CAPTAE-SHA256 开头的行）对照：下载到的就是 GitHub 的机器上打出来、检查过的那份
#   3. 本机签名（scripts/sign-update.sh；私钥只在维护者这台 Mac 上，GitHub 的机器上没有）
#   4. 生成 latest.json：四个键（darwin-aarch64-app、windows-x86_64-nsis、linux-x86_64-appimage、linux-x86_64-deb）一个都不能少，
#      Tauri 读 latest.json 时先整体校验，少一个、坏一个，所有平台都查不到更新
#   5. 上传 macOS 的更新包、四个 .sig、latest.json，下载 Release 上其余的文件，重新生成 SHA256SUMS.txt 一起上传
#
# 草稿和预发布不算 latest：Release 点「发布」那一刻，用户的 Captae 才查得到这个版本。
#
# 用法：scripts/release-sign-updates.sh <标签，比如 v0.3.0> [--run <release-winlinux.yml 的运行编号>] [--no-upload] [--base-url <地址>]
#   --run       用哪一次 release-winlinux.yml 的日志对照 SHA256（不给就从最近几次成功的里面找）
#   --no-upload 只下载、核对、签名、生成，不往 Release 上传（发版前的演练：把这个目录交给本机测试服务，用上一版的测试版更新过去）
#   --base-url  latest.json 里下载地址的前缀（演练时换成本机测试服务的地址；默认 GitHub 上这个标签的下载地址）
# 需要：gh（登录过、能看草稿 Release）、tauri-cli 2.12 以上、keys/updater.key 和钥匙串里它的密码。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
REPO="${CAPTAE_REPO:-sol1560/Captae}"
TAG="${1:?用法：release-sign-updates.sh <标签> [--run <编号>] [--no-upload] [--base-url <地址>]}"
shift
RUN=""
UPLOAD=1
BASE=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --run) RUN="${2:?--run 后面要写运行编号}"; shift 2 ;;
    --no-upload) UPLOAD=0; shift ;;
    --base-url) BASE="${2:?--base-url 后面要写地址}"; shift 2 ;;
    *) echo "不认识的参数：$1" >&2; exit 2 ;;
  esac
done

VER="${TAG#v}"
CONF_VER="$(plutil -extract version raw -o - "$ROOT/apps/desktop/src-tauri/tauri.conf.json")"
if [[ "$VER" != "$CONF_VER" ]]; then
  echo "标签 $TAG 和 tauri.conf.json 的版本 $CONF_VER 对不上" >&2
  exit 1
fi
[[ -n "$BASE" ]] || BASE="https://github.com/$REPO/releases/download/$TAG"
BASE="${BASE%/}"

MAC_PKG="Captae_${VER}_aarch64.app.tar.gz"
WIN_PKG="Captae_${VER}_x64-setup.exe"
DEB_PKG="Captae_${VER}_amd64.deb"
APPIMAGE_PKG="Captae_${VER}_amd64.AppImage"
DIR="$ROOT/target/release/updater-$VER"
mkdir -p "$DIR"

# ---------- 草稿 Release ----------
draft="$(gh release view "$TAG" --repo "$REPO" --json isDraft --jq .isDraft)"
if [[ "$draft" != "true" ]]; then
  echo "$TAG 不是草稿 Release（已经发布了的话，传上去的 latest.json 马上就对所有用户生效）。先在草稿上做完再发布" >&2
  exit 1
fi

# ---------- 1. macOS 的更新包（本机打的） ----------
mac_src="$ROOT/target/release/bundle/updater/$MAC_PKG"
[[ -f "$mac_src" && -f "$mac_src.sig" ]] || { echo "没有 ${mac_src}（和 .sig）：先运行 scripts/build-release.sh" >&2; exit 1; }
cp -f "$mac_src" "$mac_src.sig" "$DIR/"
python3 -I "$ROOT/scripts/check-update-sig.py" "$DIR/$MAC_PKG" "$VER"

# ---------- 2. 下载 Windows / Linux 的安装包，和 CI 日志里的 SHA256 对照 ----------
echo "下载草稿 Release 上的 Windows、Linux 安装包 …"
gh release download "$TAG" --repo "$REPO" --dir "$DIR" --clobber --pattern "$WIN_PKG" --pattern "$DEB_PKG" --pattern "$APPIMAGE_PKG"
for f in "$WIN_PKG" "$DEB_PKG" "$APPIMAGE_PKG"; do
  [[ -f "$DIR/$f" ]] || { echo "草稿 Release 上没有 ${f}：先手动运行 release-winlinux.yml（填标签、勾上传）" >&2; exit 1; }
done

if [[ -n "$RUN" ]]; then
  runs="$RUN"
else
  runs="$(gh run list --repo "$REPO" --workflow release-winlinux.yml --status success --limit 5 --json databaseId --jq '.[].databaseId')"
fi
ci="$DIR/ci-sha256.txt"
: >"$ci"
for r in $runs; do
  gh run view "$r" --repo "$REPO" --log 2>/dev/null | grep -o 'CAPTAE-SHA256 [0-9a-f]\{64\}  [^ ]*' >>"$ci" || true
  if grep -q "  $WIN_PKG\$" "$ci" && grep -q "  $DEB_PKG\$" "$ci" && grep -q "  $APPIMAGE_PKG\$" "$ci"; then
    echo "用 release-winlinux.yml 第 $r 次运行的日志对照"
    break
  fi
done
for f in "$WIN_PKG" "$DEB_PKG" "$APPIMAGE_PKG"; do
  want="$(grep "  $f\$" "$ci" | tail -1 | awk '{print $2}')"
  got="$(shasum -a 256 "$DIR/$f" | awk '{print $1}')"
  if [[ -z "$want" ]]; then
    echo "release-winlinux.yml 的日志里找不到 $f 的 SHA256（用 --run 指定那一次运行）" >&2
    exit 1
  fi
  if [[ "$want" != "$got" ]]; then
    echo "$f 和 GitHub 的机器上打出来的不一样（日志里 ${want}，下载到的 ${got}），不签" >&2
    exit 1
  fi
  echo "  ${f} 和 CI 打出来的一样（${got}）"
done

# ---------- 3. 签名 ----------
for f in "$WIN_PKG" "$DEB_PKG" "$APPIMAGE_PKG"; do
  "$ROOT/scripts/sign-update.sh" "$DIR/$f" "$VER"
done

# ---------- 4. latest.json ----------
python3 -I - "$DIR" "$VER" "$TAG" "$BASE" "$MAC_PKG" "$WIN_PKG" "$APPIMAGE_PKG" "$DEB_PKG" <<'PY'
import datetime, json, os, sys
d, ver, tag, base, mac, win, appimage, deb = sys.argv[1:]
plats = {
    "darwin-aarch64-app": mac,
    "windows-x86_64-nsis": win,
    "linux-x86_64-appimage": appimage,
    "linux-x86_64-deb": deb,
}
out = {
    "version": ver,
    "notes": f"Captae {ver}: https://github.com/sol1560/Captae/releases/tag/{tag}",
    "pub_date": datetime.datetime.now(datetime.timezone.utc).replace(microsecond=0).isoformat().replace("+00:00", "Z"),
    "platforms": {},
}
for key, name in plats.items():
    sig = open(os.path.join(d, name + ".sig"), encoding="utf-8").read().strip()
    if not sig:
        sys.exit(f"{name}.sig 是空的")
    out["platforms"][key] = {"url": f"{base}/{name}", "signature": sig}
# 四个键都要有、每个都有 url 和 signature（少一个 Tauri 会让所有平台都查不到更新）
assert sorted(out["platforms"]) == sorted(plats) and all(v["url"] and v["signature"] for v in out["platforms"].values())
json.dump(out, open(os.path.join(d, "latest.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=2)
print("latest.json：" + ", ".join(sorted(out["platforms"])))
PY

if [[ $UPLOAD == 0 ]]; then
  echo
  echo "没有上传（--no-upload）。文件在 $DIR"
  exit 0
fi

# ---------- 5. 上传 ----------
echo "上传更新包、签名、latest.json …"
gh release upload "$TAG" --repo "$REPO" --clobber \
  "$DIR/$MAC_PKG" "$DIR/$MAC_PKG.sig" "$DIR/$WIN_PKG.sig" "$DIR/$DEB_PKG.sig" "$DIR/$APPIMAGE_PKG.sig" "$DIR/latest.json"

# SHA256SUMS.txt：Release 上除它自己以外的每个文件（安装包、更新包、签名、latest.json、GPL 源码包）
echo "下载 Release 上其余的文件，重新生成 SHA256SUMS.txt …"
all="$DIR/all"
mkdir -p "$all"
gh release download "$TAG" --repo "$REPO" --dir "$all" --skip-existing
rm -f "$all/SHA256SUMS.txt"
(cd "$all" && shasum -a 256 -- * >"$DIR/SHA256SUMS.txt")
gh release upload "$TAG" --repo "$REPO" --clobber "$DIR/SHA256SUMS.txt"
echo
echo "好了：草稿 $TAG 上已经有 latest.json 和四个平台的更新包、签名。发布以后用户的 Captae 才查得到"
