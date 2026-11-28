#!/usr/bin/env bash
# 打正式安装包（macOS arm64 的 dmg）：准备附带程序 → 编译出 Captae.app → 用 dmgbuild 做成 dmg → 检查（scripts/check-bundle.sh）。
#
# 和直接运行 `cargo tauri build` 的区别：
# 1. 编译时把打包这台电脑上的路径从程序里去掉。
#    Rust 默认会把源码路径写进程序（panic 信息、file!()），依赖的源码在 ~/.cargo/registry/src/<镜像名>/ 下，
#    里面有打包人的用户名和用的是哪个 crates.io 镜像。这里用 rustc 的 --remap-path-prefix 换成不带个人信息的路径，
#    cc 编的 C 代码（素材库用的 SQLite 这类）用 -ffile-prefix-map。check-bundle.sh 会检查程序里还有没有主目录的路径。
# 2. dmg 不用 Tauri 自带的打包脚本：它靠 AppleScript 让访达给窗口排版，要有人看着屏幕、不能锁屏，排出来也只是默认的白窗口。
#    这里用 dmgbuild 直接写 .DS_Store：背景图、窗口大小、图标位置都在 apps/desktop/src-tauri/dmg/（说明在 settings.py），
#    锁着屏、没人看着也能打，每次打出来都一样。dmgbuild 按 dmg/requirements.txt 写死的版本和校验值装进 target/dmgbuild-venv。
# 3. 钥匙串里有 Developer ID 证书时，用它重新签整个 app（scripts/sign-macos.sh：hardened runtime、时间戳、deno 的 JIT 授权），
#    交给 Apple 公证，把公证结果钉进 app 和 dmg：下载的人打开时系统只问一句「是从互联网下载的，要打开吗」。
#    没有证书时照旧只做本地签名（Tauri 签的 adhoc），不公证：下载的人第一次打开要到「系统设置 › 隐私与安全性」里点「仍要打开」。
#
# 用法：scripts/build-release.sh
# 需要：Rust、Node（apps/desktop 里 npm ci 过）、tauri-cli、Python 3.11 以上（dmgbuild 要求；用 PYTHON=<路径> 指定别的）。
# 签名和公证（都不设时自动找）：
#   CAPTAE_SIGN_IDENTITY   证书名，不设就用钥匙串里第一个「Developer ID Application: …」；设成 - 就只做本地签名、不公证
#   公证用 App Store Connect API 密钥：
#   CAPTAE_NOTARY_KEY      .p8 文件，不设就用 ~/.appstoreconnect/private_keys/ 里唯一的 AuthKey_*.p8
#   CAPTAE_NOTARY_KEY_ID   密钥 ID，不设就从文件名 AuthKey_<ID>.p8 里取
#   CAPTAE_NOTARY_ISSUER   Issuer ID（App Store Connect › 用户和访问 › 集成 › App Store Connect API，页面上方）
#   CAPTAE_NOTARY_PROFILE  或者用 `xcrun notarytool store-credentials <名字>` 存进钥匙串的凭据，设了就不用上面三个
#   这几个可以写进仓库里的 keys/notary.env（keys/ 不进仓库），每次打包自动读。
# 换了编译参数，第一次会把依赖全部重新编译一遍。路径里不能有空格（C 编译参数按空格分开）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CARGO_DIR="${CARGO_HOME:-$HOME/.cargo}"

# 多条规则都对得上时，rustc 和 clang 都用最后一条：先写最宽的（主目录），再写具体的
remaps=("$HOME=~")
for d in "$CARGO_DIR"/registry/src/*/; do
  [[ -d "$d" ]] && remaps+=("${d%/}=/cargo/registry")
done
[[ -d "$CARGO_DIR/git/checkouts" ]] && remaps+=("$CARGO_DIR/git/checkouts=/cargo/git")
remaps+=("$ROOT=/captae")

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

# ---------- 签名和公证用哪张证书、什么凭据（先定下来，免得编译了半天才发现不对） ----------
# shellcheck disable=SC1091
[[ -f "$ROOT/keys/notary.env" ]] && source "$ROOT/keys/notary.env"
identity="${CAPTAE_SIGN_IDENTITY:-}"
if [[ -z "$identity" ]]; then
  identity="$(security find-identity -v -p codesigning 2>/dev/null | sed -n 's/.*"\(Developer ID Application: [^"]*\)".*/\1/p' | head -n 1)"
fi
[[ -z "$identity" ]] && identity="-"
notary=()
if [[ "$identity" != "-" ]]; then
  if [[ -n "${CAPTAE_NOTARY_PROFILE:-}" ]]; then
    notary=(--keychain-profile "$CAPTAE_NOTARY_PROFILE")
  else
    key="${CAPTAE_NOTARY_KEY:-}"
    if [[ -z "$key" ]]; then
      keys=("$HOME"/.appstoreconnect/private_keys/AuthKey_*.p8)
      [[ ${#keys[@]} == 1 && -f "${keys[0]}" ]] && key="${keys[0]}"
    fi
    key_id="${CAPTAE_NOTARY_KEY_ID:-$(basename "${key:-x}" .p8 | sed 's/^AuthKey_//')}"
    if [[ -n "$key" && -n "${CAPTAE_NOTARY_ISSUER:-}" ]]; then
      notary=(--key "$key" --key-id "$key_id" --issuer "$CAPTAE_NOTARY_ISSUER")
    fi
  fi
  # 变量后面紧跟中文时要写成 ${变量}：macOS 自带的 bash 3.2 会把中文的字节当成变量名的一部分（set -u 下直接退出）
  echo "签名：${identity}；公证：$([[ ${#notary[@]} -gt 0 ]] && echo "做" || echo "不做（没有凭据：设 CAPTAE_NOTARY_ISSUER 或 CAPTAE_NOTARY_PROFILE，见开头的说明）")"
else
  echo "签名：只做本地签名（钥匙串里没有 Developer ID Application 证书），不公证。下载的人第一次打开会被系统拦"
fi

echo "编译时去掉本机路径：${#remaps[@]} 条规则"
"$ROOT/scripts/fetch-sidecars.sh"
(cd "$ROOT/apps/desktop/src-tauri" && cargo tauri build --config tauri.release.conf.json --bundles app)

DMG_SRC="$ROOT/apps/desktop/src-tauri/dmg"
app="$ROOT/target/release/bundle/macos/Captae.app"
[[ -d "$app" ]] || { echo "没找到编译出来的 $app" >&2; exit 1; }
work="$ROOT/target/release/notary"
mkdir -p "$work"

# 交给 Apple 公证，等结果（一般几分钟）。Apple 的记录（没过时的问题清单；过了也可能有提醒）存在 target/release/notary/<文件名>.log.json
notarize() {
  local out id status name issues
  name="$(basename "$1")"
  echo "$(date +%H:%M:%S) 交给 Apple 公证：$name"
  out="$(xcrun notarytool submit "$1" "${notary[@]}" --wait --timeout 1h --output-format json 2>"$work/submit.err")" || true
  id="$(python3 -I -c 'import json, sys; print(json.loads(sys.stdin.read()).get("id", ""))' <<<"$out" 2>/dev/null || true)"
  status="$(python3 -I -c 'import json, sys; print(json.loads(sys.stdin.read()).get("status", ""))' <<<"$out" 2>/dev/null || true)"
  rm -f "$work/$name.log.json"
  if [[ -n "$id" ]]; then
    xcrun notarytool log "${notary[@]}" "$id" "$work/$name.log.json" >/dev/null 2>&1 || true
  fi
  if [[ "$status" != "Accepted" ]]; then
    echo "公证没过：${name}（${status:-没有结果}）" >&2
    cat "$work/submit.err" >&2
    if [[ -f "$work/$name.log.json" ]]; then cat "$work/$name.log.json" >&2; fi
    exit 1
  fi
  issues="$(python3 -I -c 'import json, sys; print(len(json.load(open(sys.argv[1])).get("issues") or []))' "$work/$name.log.json" 2>/dev/null || echo "?")"
  echo "$(date +%H:%M:%S) 公证通过：${name}（${id}；Apple 的记录里有 ${issues} 条提醒，见 target/release/notary/$name.log.json）"
}

# 把公证结果钉在 app / dmg 上（没网时第一次打开也认）。刚公证完，Apple 那边偶尔还没同步好（Record not found），等一会儿再试
staple() {
  local i
  for i in 1 2 3 4 5; do
    if xcrun stapler staple "$1"; then return 0; fi
    echo "钉公证结果没成功（第 ${i} 次），30 秒后再试" >&2
    sleep 30
  done
  echo "钉不上公证结果：$1" >&2
  exit 1
}

# ---------- 签名、公证 app ----------
if [[ "$identity" != "-" ]]; then
  "$ROOT/scripts/sign-macos.sh" "$app" "$identity"
  if [[ ${#notary[@]} -gt 0 ]]; then
    rm -f "$work/Captae.zip"
    ditto -c -k --keepParent "$app" "$work/Captae.zip"
    notarize "$work/Captae.zip"
    staple "$app"
  fi
else
  # Tauri 已经本地签好；去掉可能带着的隔离属性（见 sign-macos.sh）
  xattr -dr com.apple.quarantine "$app" 2>/dev/null || true
fi

# ---------- dmg ----------
ver="$(plutil -extract version raw -o - "$ROOT/apps/desktop/src-tauri/tauri.conf.json")"
arch="$(uname -m)"
[[ "$arch" == arm64 ]] && arch=aarch64   # 和 Tauri 原来的命名一样：Captae_<版本>_aarch64.dmg
dmg="$ROOT/target/release/bundle/dmg/Captae_${ver}_${arch}.dmg"

# dmgbuild 装在 target/dmgbuild-venv；requirements.txt 变了（换版本）才重装
venv="$ROOT/target/dmgbuild-venv"
req_sum="$(shasum -a 256 "$DMG_SRC/requirements.txt" | awk '{print $1}')"
if [[ ! -x "$venv/bin/dmgbuild" || "$(cat "$venv/requirements.sha256" 2>/dev/null)" != "$req_sum" ]]; then
  rm -rf "$venv"
  "${PYTHON:-python3}" -m venv "$venv"
  "$venv/bin/pip" install --quiet --disable-pip-version-check --require-hashes --only-binary :all: --no-deps -r "$DMG_SRC/requirements.txt"
  echo "$req_sum" >"$venv/requirements.sha256"
fi

mkdir -p "$(dirname "$dmg")"
rm -f "$dmg"
# 卷名 Captae 就是打开 dmg 时窗口的标题
"$venv/bin/dmgbuild" -s "$DMG_SRC/settings.py" -D app="$app" -D dir="$DMG_SRC" Captae "$dmg"

# dmg 本身也签名、公证、钉上结果（下载的人打开 dmg 时系统先查它）
if [[ "$identity" != "-" ]]; then
  codesign --force --timestamp --sign "$identity" "$dmg"
  if [[ ${#notary[@]} -gt 0 ]]; then
    notarize "$dmg"
    staple "$dmg"
  fi
fi

# ---------- 应用内更新用的包（docs/DESIGN.md 第 15 节） ----------
# 已经重签、公证、钉好的 app 原样打成 tar.gz（Tauri 打包时自己生成的那份在重签之前，不能用），用 keys/updater.key 签名。
# Captae 的更新插件用 Rust 的 tar 包解开：符号链接（yt-dlp 的 Python.framework）照原样还原，硬链接解不开，所以先查有没有硬链接。
# COPYFILE_DISABLE、--no-mac-metadata：不打进 ._ 文件和扩展属性（签名、公证结果都在 app 里面的普通文件里，不靠扩展属性）。
# 没有更新签名私钥（别人从源码打包）时跳过这一步，照样能打出 dmg。
updater_dir="$ROOT/target/release/bundle/updater"
tgz="$updater_dir/Captae_${ver}_${arch}.app.tar.gz"
if [[ -f "${CAPTAE_UPDATER_KEY:-$ROOT/keys/updater.key}" ]]; then
  if [[ -n "$(find "$app" -type f -links +1 -print -quit)" ]]; then
    echo "Captae.app 里有硬链接，更新插件解不开：" >&2
    find "$app" -type f -links +1 >&2
    exit 1
  fi
  mkdir -p "$updater_dir"
  rm -f "$tgz" "$tgz.sig"
  (cd "$(dirname "$app")" && COPYFILE_DISABLE=1 tar --no-mac-metadata -czf "$tgz" "$(basename "$app")")
  "$ROOT/scripts/sign-update.sh" "$tgz" "$ver"
  echo "更新包：${tgz}（$(du -h "$tgz" | cut -f1)）"
else
  echo "没有更新签名私钥（keys/updater.key），不打应用内更新用的包"
fi

echo
echo "打好了：$dmg"
# 两项检查都跑完再看结果（一项没过也要看到另一项的结果）
rc=0
"$ROOT/scripts/check-bundle.sh" "$dmg" || rc=1
if [[ -f "$tgz" ]]; then
  "$ROOT/scripts/check-update-package.sh" "$tgz" "$ver" || rc=1
fi
exit $rc
