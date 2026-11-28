# 打 Windows 安装包（x64 的 NSIS 安装程序，不签名，这一版是测试版）：
# 准备附带程序（scripts\fetch-sidecars-windows.ps1）→ cargo tauri build --bundles nsis（MSVC）→ 检查（scripts\check-bundle-windows.ps1）。
#
# 安装程序（tauri.release.windows.conf.json）：
#   - 装到当前用户（%LOCALAPPDATA%\Captae），不要管理员权限；开始菜单里有 Captae 文件夹
#   - 没有 WebView2 运行时时由安装程序下载微软的引导程序装上（downloadBootstrapper）
#   - 安装界面按系统语言用简体中文或英文
#   - 卸载时问要不要「删除应用程序数据」（默认不勾）；勾了的话连 Captae 的数据目录一起删（windows\installer-hooks.nsh）
#
# 和直接运行 `cargo tauri build` 的区别（照 macOS 的 build-release.sh）：编译时把打包这台电脑上的路径从程序里去掉——
#   rustc 的 --remap-path-prefix（主目录、cargo 缓存、仓库目录），MSVC 链接器写进 exe 的 PDB 路径只留文件名（/PDBALTPATH:%_PDB%），
#   cc 编的 C 代码用 cl 的 /d1trimfile 去掉主目录前缀。check-bundle-windows.ps1 检查程序里还有没有主目录的路径。
#
# 用法：powershell -ExecutionPolicy Bypass -File scripts\build-release-windows.ps1 [-InstallTauriCli]
#   -InstallTauriCli  没有 tauri-cli、或者版本低于 2.12 时用 cargo install 装 2.12.6（和 Mac 上一样的版本，编一次十来分钟）。
#                     要 2.12 以上：tauri 是 2.12；应用内更新的签名要写进版本号（signer sign --app-version，docs/DESIGN.md 第 15 节）
# 需要：Windows 10 / 11 / Server 2019 以上 x64；Rust 1.93 以上的 MSVC 工具链（rustup，stable-x86_64-pc-windows-msvc）；
#       Visual Studio 2022 生成工具（「使用 C++ 的桌面开发」：MSVC 和 Windows SDK）；Node 20.19 / 22.12 以上；Git for Windows（gpg）。
#       NSIS 不用自己装：tauri-cli 第一次打包时自己下载。
# 产物：$env:CARGO_TARGET_DIR（默认 target）\release\bundle\nsis\Captae_<版本>_x64-setup.exe
# 这个文件要存成带 BOM 的 UTF-8（Windows PowerShell 5.1 不认没有 BOM 的 UTF-8）。
param([switch]$InstallTauriCli)
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }

# 运行外部程序：Windows PowerShell 5.1 在 $ErrorActionPreference = 'Stop' 时会把外部程序的 stderr 当成错误抛出，调用期间临时改成 Continue
function Exec([string]$Exe, [string[]]$Arguments) {
  $old = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
  try { & $Exe @Arguments } finally { $ErrorActionPreference = $old }
}
function Capture([string]$Exe, [string[]]$Arguments) {
  $old = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
  try { return @(& $Exe @Arguments 2>&1 | ForEach-Object { "$_" }) } finally { $ErrorActionPreference = $old }
}

foreach ($c in 'cargo', 'rustc', 'node', 'npm') {
  if (-not (Get-Command $c -ErrorAction SilentlyContinue)) { throw "缺少 $c（见开头的「需要」）" }
}
$host_ = (Capture rustc @('-vV')) | Where-Object { $_ -like 'host:*' }
if ($host_ -notmatch 'x86_64-pc-windows-msvc') { throw "Rust 的默认工具链不是 x86_64-pc-windows-msvc（现在是 $host_）：rustup default stable-x86_64-pc-windows-msvc" }
$TauriCli = '2.12.6'
$tv = (Capture cargo @('tauri', '--version')) -join ' '
# 没装，或者是 2.12 以前的（GitHub 的机器上 rust-cache 可能留着旧的）
if ($LASTEXITCODE -ne 0 -or $tv -notmatch 'tauri-cli 2\.(1[2-9]|[2-9]\d)\.') {
  if (-not $InstallTauriCli) { throw "要 tauri-cli 2.12 以上（现在是 $tv）：cargo install tauri-cli --version $TauriCli --locked（或加 -InstallTauriCli）" }
  Exec cargo @('install', 'tauri-cli', '--version', $TauriCli, '--locked', '--force')
  if ($LASTEXITCODE -ne 0) { throw 'tauri-cli 安装失败' }
  $tv = (Capture cargo @('tauri', '--version')) -join ' '
}
Write-Host "$tv；$((Capture rustc @('--version')) -join ' ')；node $((Capture node @('--version')) -join ' ')"

# ---------- 去掉本机路径 ----------
# 多条规则都对得上时 rustc 用最后一条：先写最宽的（主目录），再写具体的
$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
$remaps = @("$env:USERPROFILE=~")
Get-ChildItem -Directory -ErrorAction SilentlyContinue (Join-Path $cargoHome 'registry\src') | ForEach-Object { $remaps += "$($_.FullName)=/cargo/registry" }
$git = Join-Path $cargoHome 'git\checkouts'
if (Test-Path -LiteralPath $git) { $remaps += "$git=/cargo/git" }
$remaps += "$Root=/captae"
if ($TargetDir -ne (Join-Path $Root 'target')) { $remaps += "$TargetDir=/captae/target" }
$rustFlags = @($remaps | ForEach-Object { "--remap-path-prefix=$_" })
# MSVC 链接器默认把 PDB 的完整路径（target\release\deps\…pdb）写进 exe，只留文件名
$rustFlags += '-Clink-arg=/PDBALTPATH:%_PDB%'
# CARGO_ENCODED_RUSTFLAGS 用 0x1f 分开每个参数；设了它，RUSTFLAGS 和 cargo 配置里的 rustflags 就不再起作用
$env:CARGO_ENCODED_RUSTFLAGS = $rustFlags -join [char]0x1f
# cl.exe 没有 -ffile-prefix-map，用 /d1trimfile 把 __FILE__ 里的主目录前缀去掉（cc 按空格切 CFLAGS，路径里有空格就不加）
if ($env:USERPROFILE -notmatch ' ') {
  $env:CFLAGS = ("$env:CFLAGS /d1trimfile:$env:USERPROFILE\").Trim()
  $env:CXXFLAGS = ("$env:CXXFLAGS /d1trimfile:$env:USERPROFILE\").Trim()
} else {
  Write-Host "提醒：主目录路径里有空格（$env:USERPROFILE），C 代码里的源码路径去不掉，检查时可能报主目录路径"
}
Write-Host "编译时去掉本机路径：$($remaps.Count) 条规则"

# ---------- 前端依赖 ----------
$desktop = Join-Path $Root 'apps\desktop'
if (-not (Test-Path -LiteralPath (Join-Path $desktop 'node_modules\.package-lock.json'))) {
  Push-Location $desktop
  try {
    Exec npm @('ci', '--no-audit', '--no-fund')
    if ($LASTEXITCODE -ne 0) { throw 'npm ci 失败' }
  } finally { Pop-Location }
}

# ---------- 附带程序 ----------
& (Join-Path $PSScriptRoot 'fetch-sidecars-windows.ps1')

# ---------- 编译、打包 ----------
Push-Location (Join-Path $desktop 'src-tauri')
try {
  Exec cargo @('tauri', 'build', '--config', 'tauri.release.windows.conf.json', '--bundles', 'nsis')
  if ($LASTEXITCODE -ne 0) { throw 'cargo tauri build 失败' }
} finally { Pop-Location }

$ver = (Get-Content -Raw -LiteralPath (Join-Path $desktop 'src-tauri\tauri.conf.json') | ConvertFrom-Json).version
$setup = Join-Path $TargetDir "release\bundle\nsis\Captae_${ver}_x64-setup.exe"
if (-not (Test-Path -LiteralPath $setup)) { throw "没找到打出来的 $setup" }
Write-Host ''
Write-Host "打好了：$setup（$([math]::Round((Get-Item -LiteralPath $setup).Length / 1MB, 1)) MB）"
& (Join-Path $PSScriptRoot 'check-bundle-windows.ps1') -Installer $setup
exit $LASTEXITCODE
