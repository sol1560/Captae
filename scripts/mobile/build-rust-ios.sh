#!/usr/bin/env bash
# 把 Rust 核心（crates/captae-mobile，里面是不开 desktop feature 的 captae-core）编成 iOS 用的 xcframework，并生成 Swift 绑定。
#
# 产物：
#   target/mobile/CaptaeCore.xcframework   真机（aarch64-apple-ios）和 Apple 芯片模拟器（aarch64-apple-ios-sim）两份静态库 + C 头文件和 modulemap
#   target/mobile/swift/captae_mobile.swift  UniFFI 生成的 Swift 绑定，放进 Xcode 工程一起编
#
# 用法：scripts/mobile/build-rust-ios.sh [--debug]
#   默认用 Cargo.toml 里的 mobile 配置（LTO、去符号、按大小优化），编译时去掉本机路径（common.sh）
# 需要：Xcode（xcodebuild）、rustup target add aarch64-apple-ios aarch64-apple-ios-sim
# 没做的：Intel 模拟器（x86_64-apple-ios）
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"
# shellcheck source=common.sh
source "$ROOT/scripts/mobile/common.sh"

PROFILE=mobile
if [[ "${1:-}" == "--debug" ]]; then
  PROFILE=dev
fi
DIR="$PROFILE"
[[ "$PROFILE" == dev ]] && DIR=debug

OUT="$ROOT/target/mobile"
TARGETS=(aarch64-apple-ios aarch64-apple-ios-sim)
# 最低 iOS 版本和 App 一致（work/mobile/PLAN.md 5.6）；cc 编 SQLite 时也用它
export IPHONEOS_DEPLOYMENT_TARGET="${IPHONEOS_DEPLOYMENT_TARGET:-26.0}"

mobile_remaps
for t in "${TARGETS[@]}"; do
  if ! rustup target list --installed | grep -qx "$t"; then
    echo "缺少 Rust 目标 ${t}，先运行：rustup target add $t" >&2
    exit 2
  fi
  mobile_remap_target "$t"
  echo "==> 编 ${t}（${PROFILE}）"
  cargo build -p captae-mobile --lib --target "$t" --profile "$PROFILE"
done

LIB_DEVICE="$ROOT/target/aarch64-apple-ios/$DIR/libcaptae_mobile.a"
LIB_SIM="$ROOT/target/aarch64-apple-ios-sim/$DIR/libcaptae_mobile.a"
if [[ "$PROFILE" != dev ]]; then
  mobile_check_paths "$LIB_DEVICE"
  mobile_check_paths "$LIB_SIM"
fi

echo "==> 生成 Swift 绑定"
rm -rf "$OUT/swift" "$OUT/ios-headers"
mkdir -p "$OUT/swift" "$OUT/ios-headers"
# 绑定从编好的库里读 UniFFI 的元数据（library 模式），生成器本身在这台电脑上编
cargo run -q -p captae-mobile --features bindgen --bin uniffi-bindgen -- \
  generate --library "$LIB_SIM" --language swift --out-dir "$OUT/swift"

# xcframework 的头文件目录：C 头文件 + 名字必须叫 module.modulemap 的模块说明
mv "$OUT/swift/"*.h "$OUT/ios-headers/"
mv "$OUT/swift/"*.modulemap "$OUT/ios-headers/module.modulemap"

echo "==> 打包 CaptaeCore.xcframework"
rm -rf "$OUT/CaptaeCore.xcframework"
xcodebuild -create-xcframework \
  -library "$LIB_DEVICE" -headers "$OUT/ios-headers" \
  -library "$LIB_SIM" -headers "$OUT/ios-headers" \
  -output "$OUT/CaptaeCore.xcframework" >/dev/null

echo "完成："
echo "  $OUT/CaptaeCore.xcframework"
ls "$OUT/swift/" | sed "s|^|  $OUT/swift/|"
echo "静态库大小（链接进 App 时没用到的会被去掉，App 里实际占的小得多）："
mobile_size "$LIB_DEVICE"
mobile_size "$LIB_SIM"
