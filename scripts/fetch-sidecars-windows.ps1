# 准备 Windows 安装包（NSIS）附带的外部程序。照 scripts/fetch-sidecars.sh（macOS）的做法：版本写死、核对官方校验值和签名。
#
#   yt-dlp         官方 yt-dlp_win.zip 2026.10.26（PyInstaller 目录版，自带 Python）。核对官方 SHA2-256SUMS，
#                  并用 yt-dlp 官方公钥（crates/captae-core/assets/yt-dlp-public.key）验 SHA2-256SUMS.sig。
#                  gpg 用 PATH 里的，没有就用 Git for Windows 自带的（C:\Program Files\Git\usr\bin\gpg.exe）。
#                  放在 binaries\windows\yt-dlp\，打包后在安装目录的 yt-dlp\yt-dlp.exe
#   deno           官方 deno-x86_64-pc-windows-msvc.zip 2.9.8（MIT），核对写死的 SHA256 和官方 .sha256sum
#   ffmpeg/ffprobe BtbN/FFmpeg-Builds 的 win64-gpl 静态构建（带 libass），月末那次自动构建（BtbN 长期保留月末的构建），
#                  n9.0.2（和 macOS 版的 9.0.2 对齐），核对写死的 SHA256 和 release 里的 checksums.sha256
#   aria2c         aria2 官方的 aria2-1.37.0-win-64bit-build1.zip（静态链接，TLS 用 Windows 自己的 SChannel，不用另带证书）。
#                  官方没有给校验值，核对的是 2026-11-24 第一次下载时算出的 SHA256（脚本里写死）
#   captae         本仓库的命令行（插件宿主、MCP 服务器），cargo build --release -p captae-cli
#
# exe 放在 apps\desktop\src-tauri\binaries\，文件名带 Tauri externalBin 要求的目标后缀（ffmpeg-x86_64-pc-windows-msvc.exe），
# 打包后和桌面端主程序在同一个目录（安装目录）。
#
# 用法：powershell -ExecutionPolicy Bypass -File scripts\fetch-sidecars-windows.ps1 [-Force]
#   已经准备好的会跳过；-Force 全部重新准备。下载的原始文件缓存在 target\sidecar-cache\（环境变量 CAPTAE_SIDECAR_CACHE 可改）。
# 需要：Windows 10 / 11 / Server 2019 以上 x64、Windows PowerShell 5.1 或 PowerShell 7、Rust（MSVC 工具链）、Git for Windows（gpg）。
# 这个文件要存成带 BOM 的 UTF-8：Windows PowerShell 5.1 不认没有 BOM 的 UTF-8，中文会读乱。
param([switch]$Force)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Windows PowerShell 5.1 的下载进度条会让下载慢几十倍
[Console]::OutputEncoding = [Text.Encoding]::UTF8
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
Add-Type -AssemblyName System.IO.Compression.FileSystem

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$Bin = Join-Path $Root 'apps\desktop\src-tauri\binaries'
$Win = Join-Path $Bin 'windows'
$Triple = 'x86_64-pc-windows-msvc'
$Cache = if ($env:CAPTAE_SIDECAR_CACHE) { $env:CAPTAE_SIDECAR_CACHE } else { Join-Path $Root 'target\sidecar-cache' }

$YtdlpVer = '2026.10.26'
$YtdlpBase = "https://github.com/yt-dlp/yt-dlp/releases/download/$YtdlpVer"
$YtdlpAsset = 'yt-dlp_win.zip'
$DenoVer = '2.9.8'
$DenoBase = "https://github.com/denoland/deno/releases/download/v$DenoVer"
$DenoAsset = 'deno-x86_64-pc-windows-msvc.zip'
$DenoSha256 = 'cba84b26e0df5ae7f74fc66b86a95b866872a86292ebd84fa1e959927edd93bf'   # GitHub release 页面的摘要（2026-11-24 核对）
$FfmpegTag = 'autobuild-2026-10-31-12-59'
$FfmpegBuild = 'n9.0.2-39-g7adfbe40dc'
$FfmpegVer = '9.0.2'
$FfmpegBase = "https://github.com/BtbN/FFmpeg-Builds/releases/download/$FfmpegTag"
$FfmpegAsset = "ffmpeg-$FfmpegBuild-win64-gpl-9.0.zip"
$FfmpegSha256 = '693f3e224bc184b40524a62ca47526cfebaf2504414021b92fe75f677ad868b9'
$Aria2Ver = '1.37.0'
$Aria2Asset = "aria2-$Aria2Ver-win-64bit-build1.zip"
$Aria2Url = "https://github.com/aria2/aria2/releases/download/release-$Aria2Ver/$Aria2Asset"
$Aria2Sha256 = '67d015301eef0b612191212d564c5bb0a14b5b9c4796b76454276a4d28d9b288'

if (-not [Environment]::Is64BitOperatingSystem) { throw '只支持 64 位 Windows' }
New-Item -ItemType Directory -Force -Path $Bin, $Win, $Cache | Out-Null
$Tmp = Join-Path $Cache ('tmp-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force -Path $Tmp | Out-Null

# 运行外部程序。Windows PowerShell 5.1 在 $ErrorActionPreference = 'Stop' 时，会把外部程序（重定向了的）stderr 输出当成错误抛出，
# 所以调用期间临时改成 Continue；成没成功看 $LASTEXITCODE。Capture 收集 stdout + stderr 成字符串数组
function Exec([string]$Exe, [string[]]$Arguments) {
  $old = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
  try { & $Exe @Arguments } finally { $ErrorActionPreference = $old }
}
function Capture([string]$Exe, [string[]]$Arguments) {
  $old = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
  try { return @(& $Exe @Arguments 2>&1 | ForEach-Object { "$_" }) } finally { $ErrorActionPreference = $old }
}

function Sha256([string]$Path) { (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant() }

# 下载到缓存（已有且校验对就不再下载）
function Fetch([string]$Url, [string]$Name, [string]$Want) {
  $f = Join-Path $Cache $Name
  if ((Test-Path -LiteralPath $f) -and $Want -and ((Sha256 $f) -eq $Want) -and -not $Force) { return $f }
  Write-Host "  下载 $Name …"
  Invoke-WebRequest -UseBasicParsing -Uri $Url -OutFile "$f.part"
  Move-Item -Force -LiteralPath "$f.part" -Destination $f
  if ($Want -and ((Sha256 $f) -ne $Want)) {
    $got = Sha256 $f
    Remove-Item -Force -LiteralPath $f
    throw "$Name 校验失败（期望 $Want，实际 $got）"
  }
  return $f
}

# 取一个网址的文本内容。Windows PowerShell 5.1 遇到 application/octet-stream（GitHub release 里的 .sha256sum、checksums.sha256 都是）
# 时 .Content 是字节数组，不是字符串，直接拿去匹配会得到空串（2026-11-24 在 Devin 的 Windows Server 2022 上实测），这里按 UTF-8 解码
function Get-Text([string]$Url) {
  $c = (Invoke-WebRequest -UseBasicParsing -Uri $Url).Content
  if ($c -is [byte[]]) { return [Text.Encoding]::UTF8.GetString($c) }
  return [string]$c
}

# 从 zip 里只解出指定的几个文件（按文件名匹配，不管在 zip 里的哪一层）
function Expand-Some([string]$Zip, [string[]]$Names, [string]$Dest) {
  $z = [IO.Compression.ZipFile]::OpenRead($Zip)
  try {
    foreach ($n in $Names) {
      $e = $z.Entries | Where-Object { $_.Name -eq $n } | Select-Object -First 1
      if (-not $e) { throw "$Zip 里没有 $n" }
      [IO.Compression.ZipFileExtensions]::ExtractToFile($e, (Join-Path $Dest $n), $true)
    }
  } finally { $z.Dispose() }
}

function Version-Is([string]$File, [string]$Want) {
  (-not $Force) -and (Test-Path -LiteralPath $File) -and ((Get-Content -LiteralPath $File -Raw).Trim() -eq $Want)
}

function Find-Gpg {
  $c = Get-Command gpg -ErrorAction SilentlyContinue
  if ($c) { return $c.Source }
  foreach ($p in @("$env:ProgramFiles\Git\usr\bin\gpg.exe", "${env:ProgramFiles(x86)}\Git\usr\bin\gpg.exe", "$env:ProgramFiles\GnuPG\bin\gpg.exe")) {
    if ($p -and (Test-Path -LiteralPath $p)) { return $p }
  }
  return $null
}

try {
  # ---------- yt-dlp（目录版） ----------
  $ytExe = Join-Path $Win 'yt-dlp\yt-dlp.exe'
  if (-not (Version-Is (Join-Path $Win 'yt-dlp.version') $YtdlpVer) -or -not (Test-Path -LiteralPath $ytExe)) {
    Write-Host "准备 yt-dlp $YtdlpVer …"
    $sums = Join-Path $Tmp 'SHA2-256SUMS'
    Invoke-WebRequest -UseBasicParsing -Uri "$YtdlpBase/SHA2-256SUMS" -OutFile $sums
    Invoke-WebRequest -UseBasicParsing -Uri "$YtdlpBase/SHA2-256SUMS.sig" -OutFile "$sums.sig"
    $gpg = Find-Gpg
    if (-not $gpg) { throw '没有 gpg（装 Git for Windows 就有），验不了 yt-dlp 的签名' }
    New-Item -ItemType Directory -Force -Path (Join-Path $Tmp 'gnupg') | Out-Null
    Copy-Item -LiteralPath (Join-Path $Root 'crates\captae-core\assets\yt-dlp-public.key') -Destination (Join-Path $Tmp 'yt-dlp-public.key')
    # 在临时目录里只传相对路径：Git 带的 gpg 是 MSYS2 编的，有的机器上（GitHub 的 Windows 机器）会把 D:/… 当成相对路径拼到当前目录后面
    Push-Location -LiteralPath $Tmp
    try {
      $null = Capture $gpg @('--homedir', 'gnupg', '--batch', '--quiet', '--import', 'yt-dlp-public.key')
      $v = Capture $gpg @('--homedir', 'gnupg', '--batch', '--verify', 'SHA2-256SUMS.sig', 'SHA2-256SUMS')
      $rc = $LASTEXITCODE
    } finally { Pop-Location }
    if ($rc -ne 0) { throw "SHA2-256SUMS 的签名验证没有通过：$($v -join ' ')" }
    Write-Host '  SHA2-256SUMS 签名验证通过（yt-dlp 官方公钥）'
    $want = (Get-Content -LiteralPath $sums | Where-Object { $_ -match "^([0-9a-f]{64})\s+\*?$([regex]::Escape($YtdlpAsset))$" } | ForEach-Object { $Matches[1] } | Select-Object -First 1)
    if (-not $want) { throw "SHA2-256SUMS 里没有 $YtdlpAsset" }
    $zip = Fetch "$YtdlpBase/$YtdlpAsset" "yt-dlp-$YtdlpVer-$YtdlpAsset" $want
    $dst = Join-Path $Win 'yt-dlp'
    if (Test-Path -LiteralPath $dst) { Remove-Item -Recurse -Force -LiteralPath $dst }
    [IO.Compression.ZipFile]::ExtractToDirectory($zip, $dst)
    Set-Content -LiteralPath (Join-Path $Win 'yt-dlp.version') -Value $YtdlpVer -Encoding ASCII
  }
  # 安装包里也记一份版本（安装目录的 yt-dlp\version.txt），见 tools.rs 的 bundled_ytdlp_version
  Set-Content -LiteralPath (Join-Path $Win 'yt-dlp\version.txt') -Value $YtdlpVer -Encoding ASCII

  # ---------- deno ----------
  $denoExe = Join-Path $Bin "deno-$Triple.exe"
  if (-not (Version-Is (Join-Path $Win 'deno.version') $DenoVer) -or -not (Test-Path -LiteralPath $denoExe)) {
    Write-Host "准备 deno $DenoVer …"
    $official = ([regex]::Match((Get-Text "$DenoBase/$DenoAsset.sha256sum"), '[0-9a-fA-F]{64}')).Value.ToLowerInvariant()
    if ($official -ne $DenoSha256) { throw "deno 官方 .sha256sum（$official）和脚本里写的（$DenoSha256）不一致" }
    $zip = Fetch "$DenoBase/$DenoAsset" "deno-$DenoVer-$DenoAsset" $DenoSha256
    Expand-Some $zip @('deno.exe') $Tmp
    Copy-Item -Force -LiteralPath (Join-Path $Tmp 'deno.exe') -Destination $denoExe
    Set-Content -LiteralPath (Join-Path $Win 'deno.version') -Value $DenoVer -Encoding ASCII
  }

  # ---------- ffmpeg / ffprobe ----------
  $ffExe = Join-Path $Bin "ffmpeg-$Triple.exe"
  $fpExe = Join-Path $Bin "ffprobe-$Triple.exe"
  if (-not (Version-Is (Join-Path $Win 'ffmpeg.version') $FfmpegBuild) -or -not (Test-Path -LiteralPath $ffExe) -or -not (Test-Path -LiteralPath $fpExe)) {
    Write-Host "准备 ffmpeg $FfmpegBuild（BtbN $FfmpegTag）…"
    $sums = Get-Text "$FfmpegBase/checksums.sha256"
    $listed = ([regex]::Match($sums, "([0-9a-f]{64})\s+\*?$([regex]::Escape($FfmpegAsset))")).Groups[1].Value
    if ($listed -ne $FfmpegSha256) { throw "BtbN checksums.sha256 里 $FfmpegAsset 的值（$listed）和脚本里写的（$FfmpegSha256）不一致" }
    $zip = Fetch "$FfmpegBase/$FfmpegAsset" $FfmpegAsset $FfmpegSha256
    Expand-Some $zip @('ffmpeg.exe', 'ffprobe.exe') $Tmp
    Copy-Item -Force -LiteralPath (Join-Path $Tmp 'ffmpeg.exe') -Destination $ffExe
    Copy-Item -Force -LiteralPath (Join-Path $Tmp 'ffprobe.exe') -Destination $fpExe
    Set-Content -LiteralPath (Join-Path $Win 'ffmpeg.version') -Value $FfmpegBuild -Encoding ASCII
  }

  # ---------- aria2c ----------
  $a2Exe = Join-Path $Bin "aria2c-$Triple.exe"
  if (-not (Version-Is (Join-Path $Win 'aria2c.version') $Aria2Ver) -or -not (Test-Path -LiteralPath $a2Exe)) {
    Write-Host "准备 aria2c $Aria2Ver …"
    $zip = Fetch $Aria2Url $Aria2Asset $Aria2Sha256
    Expand-Some $zip @('aria2c.exe') $Tmp
    Copy-Item -Force -LiteralPath (Join-Path $Tmp 'aria2c.exe') -Destination $a2Exe
    Set-Content -LiteralPath (Join-Path $Win 'aria2c.version') -Value $Aria2Ver -Encoding ASCII
  }

  # ---------- captae 命令行 ----------
  # 打正式包时由 build-release-windows.ps1 设好去掉本机路径的编译参数（CARGO_ENCODED_RUSTFLAGS）再调用这里
  Write-Host '编译 captae 命令行 …'
  Push-Location $Root
  try {
    Exec cargo @('build', '--release', '-p', 'captae-cli', '--quiet')
    if ($LASTEXITCODE -ne 0) { throw 'captae 命令行编译失败' }
  } finally { Pop-Location }
  $target = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
  Copy-Item -Force -LiteralPath (Join-Path $target 'release\captae.exe') -Destination (Join-Path $Bin "captae-$Triple.exe")

  # ---------- 检查：PATH 里只有系统目录、HOME 和 CAPTAE_HOME 是临时目录时都能跑 ----------
  Write-Host '检查 …'
  $saved = @{ PATH = $env:PATH; CAPTAE_HOME = $env:CAPTAE_HOME }
  $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
  $env:CAPTAE_HOME = Join-Path $Tmp 'captae-home'
  try {
    $tools = @(
      @{ n = 'yt-dlp'; p = $ytExe; a = '--version' },
      @{ n = 'deno'; p = $denoExe; a = '--version' },
      @{ n = 'ffmpeg'; p = $ffExe; a = '-version' },
      @{ n = 'ffprobe'; p = $fpExe; a = '-version' },
      @{ n = 'aria2c'; p = $a2Exe; a = '--version' },
      @{ n = 'captae'; p = (Join-Path $Bin "captae-$Triple.exe"); a = '--help' }
    )
    foreach ($t in $tools) {
      $out = ((Capture $t.p @($t.a)) | Select-Object -First 1 | Out-String).Trim()
      if ($LASTEXITCODE -ne 0 -or -not $out) { throw "  $($t.n) 运行失败：$out" }
      Write-Host ('  {0,-8} {1}' -f $t.n, $out)
    }
    $filters = (Capture $ffExe @('-hide_banner', '-filters')) -join "`n"
    if ($filters -notmatch ' subtitles ') { throw '  ffmpeg 不支持字幕烧录（缺 libass）' }
  } finally {
    $env:PATH = $saved.PATH
    $env:CAPTAE_HOME = $saved.CAPTAE_HOME
  }
  Write-Host "完成：$Bin"
} finally {
  Remove-Item -Recurse -Force -LiteralPath $Tmp -ErrorAction SilentlyContinue
}
