#!/usr/bin/env bash
# 把 Rust 核心（crates/captae-mobile，里面是不开 desktop feature 的 captae-core）编成 Android 的 .so，并生成 Kotlin 绑定。
#
# 产物：
#   target/mobile/jniLibs/{arm64-v8a,x86_64}/libcaptae_mobile.so
#   target/mobile/kotlin/uniffi/captae_mobile/captae_mobile.kt   UniFFI 生成的 Kotlin 绑定（运行时要 JNA：net.java.dev.jna:jna，aar）
#   target/mobile/kotlin/app/captae/core/CaptaeTls.kt           初始化系统证书校验（源文件 crates/captae-mobile/kotlin/），App 启动时
#                                                                在建 MobileCore 之前调 `CaptaeTls.init(applicationContext)`
#   target/mobile/maven/                                         rustls-platform-verifier 的 Kotlin 部分（本地 Maven 仓库，
#                                                                Gradle 里加这个仓库和依赖 org.rustls:rustls-platform-verifier:<版本>，版本见脚本输出）
#
# 用法：scripts/mobile/build-rust-android.sh [--debug]
#   默认用 Cargo.toml 里的 mobile 配置（LTO、去符号、按大小优化），编译时去掉本机路径（common.sh）
# 需要：
#   rustup target add aarch64-linux-android x86_64-linux-android
#   cargo install cargo-ndk --locked
#   Android NDK：ANDROID_NDK_HOME 指定；不设时用 Android SDK（ANDROID_HOME，默认 ~/Library/Android/sdk）里的 ndk/$NDK_VERSION
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
# shellcheck source=common.sh
source "$ROOT/scripts/mobile/common.sh"

PROFILE=mobile
if [[ "${1:-}" == "--debug" ]]; then
  PROFILE=dev
fi

NDK_VERSION="${NDK_VERSION:-28.2.13676358}"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Library/Android/sdk}}"
export ANDROID_NDK_HOME="${ANDROID_NDK_HOME:-$SDK/ndk/$NDK_VERSION}"
if [[ ! -d "$ANDROID_NDK_HOME" ]]; then
  echo "找不到 Android NDK：${ANDROID_NDK_HOME}（用 ANDROID_NDK_HOME 指定）" >&2
  exit 2
fi
if ! command -v cargo-ndk >/dev/null; then
  echo "缺少 cargo-ndk，先运行：cargo install cargo-ndk --locked" >&2
  exit 2
fi
for t in aarch64-linux-android x86_64-linux-android; do
  if ! rustup target list --installed | grep -qx "$t"; then
    echo "缺少 Rust 目标 ${t}，先运行：rustup target add $t" >&2
    exit 2
  fi
done

OUT="$ROOT/target/mobile"
# minSdk 和 App 一致（work/mobile/PLAN.md 5.5）
API="${ANDROID_MIN_SDK:-26}"

mobile_remaps
mobile_remap_target aarch64-linux-android
mobile_remap_target x86_64-linux-android

echo "==> 编 arm64-v8a、x86_64（${PROFILE}，API ${API}）"
rm -rf "$OUT/jniLibs"
cargo ndk -t arm64-v8a -t x86_64 --platform "$API" -o "$OUT/jniLibs" \
  build -p captae-mobile --lib --profile "$PROFILE"
if [[ "$PROFILE" != dev ]]; then
  for abi in arm64-v8a x86_64; do
    mobile_check_paths "$OUT/jniLibs/$abi/libcaptae_mobile.so"
  done
fi

echo "==> 生成 Kotlin 绑定"
rm -rf "$OUT/kotlin"
mkdir -p "$OUT/kotlin/app/captae/core"
# 绑定从库里读 UniFFI 的元数据（library 模式）。mobile 配置去了符号，.so 里读不到元数据，
# 所以从这台电脑上编的同一份库（没去符号）里读：给 Swift / Kotlin 的接口和平台无关（android.rs 的 JNI 入口不经过 UniFFI）
cargo build -q -p captae-mobile --lib
cargo run -q -p captae-mobile --features bindgen --bin uniffi-bindgen -- \
  generate --library "$ROOT/target/debug/libcaptae_mobile.dylib" --language kotlin --out-dir "$OUT/kotlin" --no-format
cp "$ROOT/crates/captae-mobile/kotlin/CaptaeTls.kt" "$OUT/kotlin/app/captae/core/"

echo "==> rustls-platform-verifier 的 Kotlin 部分"
# 0.7.1 起（Android 部分 0.2.0）crate 里不再带 Maven 仓库，官方放在 GitHub 的 maven-archive 分支上（见它的 README「Gradle setup」）。
# 按 Cargo.lock 里 rustls-platform-verifier-android 的版本取 aar 和 pom，放进 target/mobile/maven（App 的 settings.gradle.kts 用这个本地仓库）。
# 核对两次：仓库自己给的 .sha1，和下面写死的 SHA-256（换版本时先人工核对再加一行）。对不上就停下
RPV_VER="$(awk '/^name = "rustls-platform-verifier-android"$/ {getline; gsub(/version = |"/, ""); print; exit}' "$ROOT/Cargo.lock")"
case "$RPV_VER" in
  0.2.0) RPV_AAR_SHA256=aa021794230fbc2f0be355999e2cf67398dd563de066a2e17001ffbd0b69101b
         RPV_POM_SHA256=78dcb69a1a20e91e72017de9a67104bba1874a7ecc8e6dcdfb2fe1a85de6b362 ;;
  *) echo "没核对过 rustls-platform-verifier-android ${RPV_VER} 的 SHA-256，先人工核对再加进脚本" >&2; exit 2 ;;
esac
RPV_DIR="$OUT/maven/org/rustls/rustls-platform-verifier/$RPV_VER"
RPV_URL="https://raw.githubusercontent.com/rustls/rustls-platform-verifier/maven-archive/android-release-support/maven/org/rustls/rustls-platform-verifier/$RPV_VER"
rm -rf "$OUT/maven"
mkdir -p "$RPV_DIR"
for f in "rustls-platform-verifier-$RPV_VER.aar" "rustls-platform-verifier-$RPV_VER.pom"; do
  curl -sSfL --retry 3 -o "$RPV_DIR/$f" "$RPV_URL/$f"
  want_sha1="$(curl -sSfL --retry 3 "$RPV_URL/$f.sha1" | awk '{print $1}')"
  got_sha1="$(shasum -a 1 "$RPV_DIR/$f" | awk '{print $1}')"
  case "$f" in *.aar) want256="$RPV_AAR_SHA256" ;; *) want256="$RPV_POM_SHA256" ;; esac
  got256="$(shasum -a 256 "$RPV_DIR/$f" | awk '{print $1}')"
  if [[ "$want_sha1" != "$got_sha1" || "$want256" != "$got256" ]]; then
    echo "$f 校验不对（sha1 $got_sha1 / 应为 $want_sha1；sha256 $got256 / 应为 $want256）" >&2
    rm -rf "$OUT/maven"
    exit 1
  fi
  echo "$got_sha1" > "$RPV_DIR/$f.sha1"
done
echo "  org.rustls:rustls-platform-verifier:${RPV_VER}（App 的 Gradle 依赖要写这个版本）"

echo "完成："
find "$OUT/jniLibs" "$OUT/kotlin" -type f | sed 's|^|  |'
echo "  $OUT/maven/（$(find "$OUT/maven" -name '*.aar' | sed "s|$OUT/maven/||" | head -1)）"
echo "大小："
for abi in arm64-v8a x86_64; do
  mobile_size "$OUT/jniLibs/$abi/libcaptae_mobile.so"
done
