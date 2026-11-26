#!/bin/zsh
# 把代码里的界面文字同步进 Captae/Resources/Localizable.xcstrings（源语言 zh-Hans）。
# 先编译一遍（SWIFT_EMIT_LOC_STRINGS 让编译器导出 .stringsdata），再用 xcstringstool 合并：
# 新的字符串加进来，代码里没有了的标成 stale，已有的翻译不动。
set -euo pipefail
cd "${0:A:h}/.."
DD=../../target/ios-dd
files=(${(f)"$(find $DD/Build/Intermediates.noindex/Captae.build -name '*.stringsdata' ! -name 'ExtractedAppShortcutsMetadata.stringsdata')"})
xcrun xcstringstool sync Captae/Resources/Localizable.xcstrings --stringsdata $files
xcrun xcstringstool print Captae/Resources/Localizable.xcstrings | wc -l | xargs echo "条目行数："
