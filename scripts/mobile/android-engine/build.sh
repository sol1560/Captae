#!/usr/bin/env bash
# Android 完整版下载引擎一条龙：下载校验 → Python → QuickJS → LAME、dav1d → FFmpeg → 整理成 apps/android/engine 要的样子。
# 每一步已经做过、输入没变的都跳过，所以可以随时重跑。
#
#   scripts/mobile/android-engine/build.sh
#
# 产物在 target/android-engine/stage/（jniLibs/<ABI>/*.so、assets/captae-engine/engine.zip、engine.json），
# apps/android/engine 的 Gradle 直接读那里。需要：curl、gpg、Android NDK（${NDK_VERSION}）、本机 cc、make、pkg-config、zip、rsync。
# 想同时验 Python 包的 sigstore 签名：python3 -m venv target/android-engine/venv-sigstore && target/android-engine/venv-sigstore/bin/pip install sigstore
set -euo pipefail
D="$(cd "$(dirname "$0")" && pwd)"
"$D/fetch.sh"
"$D/build-python.sh"
"$D/build-quickjs.sh"
"$D/build-codecs.sh"
"$D/build-ffmpeg.sh"
"$D/stage.sh"
