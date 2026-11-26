# shellcheck shell=bash
# build-rust-ios.sh、build-rust-android.sh 共用：编译时去掉这台电脑上的路径（和 scripts/build-release.sh 同一套规则）。
#
# Rust 默认把源码路径写进程序（panic 信息、file!()），依赖在 ~/.cargo/registry/src/<镜像名>/ 下，带着用户名和镜像名。
# 用 --remap-path-prefix 换掉；cc 编的 C 代码（SQLite、ring）用 -ffile-prefix-map。
# 只给手机的目标设（CARGO_TARGET_<目标>_RUSTFLAGS），电脑上编的绑定生成器不受影响，也不用重编。
# 注意：这两个脚本跑在 macOS 自带的 bash 3.2 上，字符串里变量后面紧跟中文时写成 ${变量}。

mobile_remaps() {
  local cargo_dir="${CARGO_HOME:-$HOME/.cargo}"
  # 多条规则都对得上时用最后一条：先写最宽的（主目录），再写具体的
  REMAPS=("$HOME=~")
  local d
  for d in "$cargo_dir"/registry/src/*/; do
    [[ -d "$d" ]] && REMAPS+=("${d%/}=/cargo/registry")
  done
  [[ -d "$cargo_dir/git/checkouts" ]] && REMAPS+=("$cargo_dir/git/checkouts=/cargo/git")
  REMAPS+=("$ROOT=/captae")
  local r
  for r in "${REMAPS[@]}"; do
    if [[ "$r" == *" "* ]]; then
      echo "路径里有空格，没法去掉：${r}" >&2
      exit 2
    fi
  done
}

# 给一个目标（aarch64-apple-ios 这种）设去路径的编译参数
mobile_remap_target() {
  local triple="$1"
  local var
  var="CARGO_TARGET_$(echo "$triple" | tr '[:lower:]-' '[:upper:]_')_RUSTFLAGS"
  local flags=() cflags=() r
  for r in "${REMAPS[@]}"; do
    flags+=("--remap-path-prefix=$r")
    cflags+=("-ffile-prefix-map=$r")
  done
  export "$var=${flags[*]}"
  # cc crate 认 CFLAGS_<目标>（横线换成下划线）
  local cvar
  cvar="CFLAGS_$(echo "$triple" | tr '-' '_')"
  export "$cvar=${cflags[*]}"
}

# 编好的库里还有没有这台电脑上的主目录路径（有就停下）
mobile_check_paths() {
  local f="$1" n
  n=$(LC_ALL=C grep -a -c -F -- "$HOME" "$f" || true)
  if [[ "$n" != "0" ]]; then
    echo "库里还有本机路径：${f}" >&2
    LC_ALL=C grep -a -o -F -- "$HOME" "$f" | head -1 >&2
    LC_ALL=C strings -a "$f" | grep -F -- "$HOME" | sort -u | head -5 >&2
    exit 3
  fi
}

# 文件大小（字节）和 MB
mobile_size() {
  local b
  b=$(stat -f %z "$1")
  echo "$b 字节（$(awk -v b="$b" 'BEGIN{printf "%.1f", b/1000000}') MB）  $1"
}
