#!/usr/bin/env bash
# 给应用内更新的包签名（docs/DESIGN.md 第 15 节），输出 <文件>.sig，签完用 check-update-sig.py 核对一遍。
#
# 私钥：keys/updater.key（Tauri 生成的 minisign 私钥，keys/ 不进仓库；用 CAPTAE_UPDATER_KEY 指定别的位置）。
# 私钥的密码：钥匙串里服务名「Captae updater signing key」、账户 captae 的那一条；也可以用 TAURI_SIGNING_PRIVATE_KEY_PASSWORD 给。
# 签名里写上版本号（tauri-cli 2.12 起的 --app-version）：Captae 开着 requireSignedVersion，latest.json 说的版本和签名里的对不上就不装。
# 只在维护者这台 Mac 上签，GitHub 的机器上不放私钥（见 scripts/release-sign-updates.sh）。
#
# 用法：scripts/sign-update.sh <文件> <版本号>
# 用哪个 tauri-cli：CARGO_TAURI（默认 `cargo tauri`）
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
file="${1:?用法：sign-update.sh <文件> <版本号>}"
ver="${2:?用法：sign-update.sh <文件> <版本号>}"
key="${CAPTAE_UPDATER_KEY:-$ROOT/keys/updater.key}"
[[ -f "$file" ]] || { echo "没有这个文件：$file" >&2; exit 1; }
[[ -f "$key" ]] || { echo "没有更新签名私钥：${key}（见 docs/DESIGN.md 第 15 节）" >&2; exit 1; }

pw="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"
if [[ -z "$pw" ]]; then
  pw="$(security find-generic-password -a captae -s 'Captae updater signing key' -w 2>/dev/null)" || {
    echo "钥匙串里没有更新签名私钥的密码（服务名「Captae updater signing key」），也没有设 TAURI_SIGNING_PRIVATE_KEY_PASSWORD" >&2
    exit 1
  }
fi

read -r -a tauri <<<"${CARGO_TAURI:-cargo tauri}"
if ! "${tauri[@]}" signer sign --help 2>/dev/null | grep -q -- '--app-version'; then
  echo "这个 tauri-cli 太旧，签不了带版本号的签名：cargo install tauri-cli --version '^2.12' --locked" >&2
  exit 1
fi

rm -f "$file.sig"
# 密码走环境变量，不放在命令行参数里（ps 看得到）
TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$pw" "${tauri[@]}" signer sign -f "$key" --app-version "$ver" "$file" >/dev/null
[[ -s "$file.sig" ]] || { echo "没有签出 $file.sig" >&2; exit 1; }
python3 -I "$ROOT/scripts/check-update-sig.py" "$file" "$ver" "$ROOT/apps/desktop/src-tauri/tauri.conf.json"
