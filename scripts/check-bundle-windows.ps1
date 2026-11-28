# 检查打好的 Windows 安装程序（NSIS）。项目照 macOS 的 scripts/check-bundle.sh 对应过来：
#
#   1. 安装程序大小、版本（和 tauri.conf.json 对照）；装好以后的大小
#   2. 附带程序齐不齐：captae.exe、yt-dlp\yt-dlp.exe（目录版）、deno.exe、ffmpeg.exe、ffprobe.exe、aria2c.exe 都在安装目录里，
#      在干净的环境里（PATH 只有 Windows 的系统目录，CAPTAE_HOME 是临时目录）都跑得出版本；版本和 fetch-sidecars-windows.ps1 写的一致；
#      deno 能跑 JS；ffmpeg 带 libass；aria2c 的 HTTPS（Windows 的 SChannel）能用（要联网，连不上只提醒）
#   3. 签名：这一版不签名，只提醒一条（下载的人第一次打开会被 SmartScreen 拦）
#   4. 正式构建：嵌进桌面端的前端文件里没有演示数据，图片只有上手引导的剧照
#   5. 没有调试钩子（CAPTAE_TEST_、login-test.json、clip-test.json、eval.js），没有打包那台电脑的主目录路径和 PDB 路径
#   6. 插件文件夹在、popup/ 是构建产物、和仓库里的 extension/ 一样
#   7. licenses/、THIRD_PARTY_NOTICES.md 在、和仓库一样
#   8. 应用内更新：桌面端里记着的安装方式是 NSIS（更新插件据此在 latest.json 里找 windows-x86_64-nsis）、更新公钥和 tauri.conf.json 一致、
#      更新地址是 GitHub 上的正式地址；自带的 yt-dlp 旁边有 version.txt（docs/DESIGN.md 第 15 节）
#   9. 只在 -Install 时：卸载干净（安装目录、开始菜单、卸载登记都没了）
#
# 拿到安装目录的三种办法：
#   -InstallDir <目录>   检查已经装好的目录（例如 %LOCALAPPDATA%\Captae）
#   默认                 用 7-Zip（7z.exe）把安装程序解到临时目录，不安装、不改系统（choco install 7zip）
#   -Install             静默安装到临时目录（安装程序 /S /D=…）、检查、再静默卸载。会写开始菜单和卸载登记（检查完删掉）；
#                        这台电脑上已经装着 Captae 时拒绝（静默安装会把它换掉）
# 用法：powershell -ExecutionPolicy Bypass -File scripts\check-bundle-windows.ps1 [-Installer <setup.exe>] [-InstallDir <目录>] [-Install]
#   不给 -Installer 时检查 target\release\bundle\nsis\ 里最新的 *-setup.exe。临时目录在 target\check-bundle 下，检查完删掉。
# 退出码：0 全过；1 有没过的项；2 用法或环境不对。不运行桌面端主程序。
# 这个文件要存成带 BOM 的 UTF-8（Windows PowerShell 5.1 不认没有 BOM 的 UTF-8）。
param([string]$Installer, [string]$InstallDir, [switch]$Install)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = [Text.Encoding]::UTF8

$Root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
$Script:Fails = 0
$Script:Warns = 0
function Pass([string]$m) { Write-Host "  ✓ $m" }
function Fail([string]$m) { Write-Host "  ✗ $m"; $Script:Fails++ }
function Warn([string]$m) { Write-Host "  ! $m"; $Script:Warns++ }
function Section([string]$m) { Write-Host ''; Write-Host $m }
function MB([long]$b) { '{0:N1} MB' -f ($b / 1MB) }
function Capture([string]$Exe, [string[]]$Arguments) {
  $old = $ErrorActionPreference; $ErrorActionPreference = 'Continue'
  try { return @(& $Exe @Arguments 2>&1 | ForEach-Object { "$_" }) } finally { $ErrorActionPreference = $old }
}

$conf = Get-Content -Raw -LiteralPath (Join-Path $Root 'apps\desktop\src-tauri\tauri.conf.json') | ConvertFrom-Json
$wantVer = $conf.version
$fetch = Get-Content -Raw -LiteralPath (Join-Path $PSScriptRoot 'fetch-sidecars-windows.ps1')
function Want([string]$name) { ([regex]::Match($fetch, "(?m)^\`$$name = '([^']*)'")).Groups[1].Value }
$wantYtdlp = Want 'YtdlpVer'; $wantDeno = Want 'DenoVer'; $wantFfmpeg = Want 'FfmpegVer'; $wantAria2 = Want 'Aria2Ver'

if (-not $InstallDir -and -not $Installer) {
  $Installer = Get-ChildItem -ErrorAction SilentlyContinue (Join-Path $TargetDir 'release\bundle\nsis\*-setup.exe') | Sort-Object LastWriteTime -Descending | Select-Object -First 1 -ExpandProperty FullName
  if (-not $Installer) { Write-Host '找不到安装程序（target\release\bundle\nsis\ 里没有 *-setup.exe）'; exit 2 }
}

$tmpBase = if ($env:CHECK_BUNDLE_TMP) { $env:CHECK_BUNDLE_TMP } else { Join-Path $Root 'target\check-bundle' }
$Work = Join-Path $tmpBase ('win-' + [Guid]::NewGuid().ToString('N').Substring(0, 8))
New-Item -ItemType Directory -Force -Path $Work | Out-Null
$uninstallKey = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Captae'
# Tauri 的安装程序把安装目录记在这里（默认值），卸载时只有勾了「删除应用程序数据」才删；下次安装（含 /S）默认装回这个目录。
# -Install 装到临时目录，检查完要把它恢复成原来的样子，不然以后的静默安装会装进已经删掉的临时目录（2026-11-24 Devin 上碰到过）
$dirKey = 'HKCU:\Software\Captae contributors\Captae'
$dirKeyBefore = if (Test-Path $dirKey) { (Get-ItemProperty -Path $dirKey -ErrorAction SilentlyContinue).'(default)' } else { $null }
$dirKeyExisted = Test-Path $dirKey
$installedHere = $false
$touchedDirKey = $false

try {
  # ---------- 1. 安装程序 ----------
  Section '1. 安装程序'
  if ($Installer) {
    $item = Get-Item -LiteralPath $Installer
    Write-Host "检查 $($item.Name)"
    Pass "安装程序 $(MB $item.Length)（$($item.Length) 字节）"
    $pv = $item.VersionInfo.ProductVersion
    if ($pv -and $pv.StartsWith($wantVer)) { Pass "版本 $pv" } else { Fail "版本 $pv（tauri.conf.json 是 $wantVer）" }
  }
  if ($InstallDir) {
    $D = (Resolve-Path -LiteralPath $InstallDir).Path
    Pass "检查已经装好的目录 $D"
  } elseif ($Install) {
    if (Test-Path $uninstallKey) { Write-Host '这台电脑上已经装着 Captae，静默安装会把它换掉；改用 -InstallDir 检查装好的目录，或者先卸载'; exit 2 }
    $D = Join-Path $Work 'install'
    # /D= 要放在最后，路径不加引号（NSIS 的规矩）
    $touchedDirKey = $true
    $p = Start-Process -FilePath $Installer -ArgumentList @('/S', "/D=$D") -Wait -PassThru
    if ($p.ExitCode -ne 0) { Fail "静默安装失败（退出码 $($p.ExitCode)）"; exit 1 }
    $installedHere = $true
    Pass "静默安装到 $D"
  } else {
    $7z = (Get-Command 7z -ErrorAction SilentlyContinue).Source
    if (-not $7z -and (Test-Path "$env:ProgramFiles\7-Zip\7z.exe")) { $7z = "$env:ProgramFiles\7-Zip\7z.exe" }
    if (-not $7z) { Write-Host '没有 7-Zip（choco install 7zip），解不开安装程序；用 -Install（静默安装到临时目录）或 -InstallDir'; exit 2 }
    $x = Join-Path $Work 'extract'
    $null = Capture $7z @('x', '-y', "-o$x", $Installer)
    if ($LASTEXITCODE -ne 0) { Fail "7-Zip 解不开安装程序"; exit 1 }
    # 7-Zip 解出来的布局：安装目录的文件在根上（或 $INSTDIR 下），另有 $PLUGINSDIR 等 NSIS 自己的目录
    $main = Get-ChildItem -LiteralPath $x -Recurse -Filter 'captae-desktop.exe' | Select-Object -First 1
    if (-not $main) { Fail '解出来的文件里没有 captae-desktop.exe'; exit 1 }
    $D = $main.DirectoryName
    Pass "用 7-Zip 解到临时目录（$($D.Substring($x.Length).TrimStart('\')) 下是安装目录的内容）"
  }
  $size = (Get-ChildItem -LiteralPath $D -Recurse -File | Measure-Object -Sum Length).Sum
  Pass "装好以后约 $(MB $size)"

  # ---------- 2. 附带程序 ----------
  Section '2. 附带程序（PATH 只有 Windows 系统目录，CAPTAE_HOME 是临时目录）'
  $saved = @{ PATH = $env:PATH; CAPTAE_HOME = $env:CAPTAE_HOME; DENO_NO_UPDATE_CHECK = $env:DENO_NO_UPDATE_CHECK }
  $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot;$env:SystemRoot\System32\WindowsPowerShell\v1.0"
  $env:CAPTAE_HOME = Join-Path $Work 'captae-home'
  $env:DENO_NO_UPDATE_CHECK = '1'
  try {
    function Check-Tool([string]$name, [string]$path, [string]$arg, [string]$want) {
      if (-not (Test-Path -LiteralPath $path)) { Fail "${name}：没有（$($path.Substring($D.Length).TrimStart('\'))）"; return }
      $out = ((Capture $path @($arg)) | Where-Object { $_.Trim() } | Select-Object -First 1)
      if (-not $out) { Fail "${name}：跑不出版本" }
      elseif ($want -and $out -notlike "*$want*") { Fail "${name}：$out（期望 $want）" }
      else { Pass "${name}：$out" }
    }
    Check-Tool 'captae' (Join-Path $D 'captae.exe') '--help' "Captae $wantVer"
    Check-Tool 'yt-dlp' (Join-Path $D 'yt-dlp\yt-dlp.exe') '--version' $wantYtdlp
    Check-Tool 'deno' (Join-Path $D 'deno.exe') '--version' "deno $wantDeno"
    Check-Tool 'ffmpeg' (Join-Path $D 'ffmpeg.exe') '-version' "version n$wantFfmpeg"
    Check-Tool 'ffprobe' (Join-Path $D 'ffprobe.exe') '-version' "version n$wantFfmpeg"
    Check-Tool 'aria2c' (Join-Path $D 'aria2c.exe') '--version' "aria2 version $wantAria2"
    if (Test-Path -LiteralPath (Join-Path $D 'deno.exe')) {
      $js = (Capture (Join-Path $D 'deno.exe') @('eval', 'console.log(6 * 7)')) | Select-Object -Last 1
      if ($js -eq '42') { Pass 'deno 能跑 JS' } else { Fail "deno 跑不了 JS：$js" }
    }
    if (Test-Path -LiteralPath (Join-Path $D 'ffmpeg.exe')) {
      $f = (Capture (Join-Path $D 'ffmpeg.exe') @('-hide_banner', '-filters')) -join "`n"
      if ($f -match ' subtitles ') { Pass 'ffmpeg 带 libass（能烧录字幕）' } else { Fail 'ffmpeg 不支持字幕烧录（缺 libass）' }
    }
    if (Test-Path -LiteralPath (Join-Path $D 'aria2c.exe')) {
      $dl = Join-Path $Work 'aria2-test'
      $null = Capture (Join-Path $D 'aria2c.exe') @('-q', '--allow-overwrite=true', '-d', $dl, '-o', 'COPYING', 'https://raw.githubusercontent.com/aria2/aria2/master/COPYING')
      if ($LASTEXITCODE -eq 0 -and (Test-Path (Join-Path $dl 'COPYING'))) { Pass 'aria2c 能下载 HTTPS（Windows 的证书）' } else { Warn 'aria2c 下载 HTTPS 测试地址没成功（没联网？）' }
    }
  } finally {
    $env:PATH = $saved.PATH; $env:CAPTAE_HOME = $saved.CAPTAE_HOME; $env:DENO_NO_UPDATE_CHECK = $saved.DENO_NO_UPDATE_CHECK
  }
  $mainExe = Join-Path $D 'captae-desktop.exe'
  if (Test-Path -LiteralPath $mainExe) { Pass '桌面端主程序 captae-desktop.exe 在（不运行它）' } else { Fail '桌面端主程序 captae-desktop.exe 不在' }
  $extra = Get-ChildItem -LiteralPath $D -File -Filter '*.dll' | Select-Object -ExpandProperty Name
  if ($extra) { Warn "安装目录根上有 DLL：$($extra -join ' ')（附带程序都是静态链接的，不该有）" } else { Pass '安装目录根上没有多出来的 DLL' }

  # ---------- 3. 签名 ----------
  Section '3. 签名'
  foreach ($f in @($Installer, $mainExe) | Where-Object { $_ -and (Test-Path -LiteralPath $_) }) {
    $s = Get-AuthenticodeSignature -LiteralPath $f
    if ($s.Status -eq 'Valid') { Pass "$(Split-Path -Leaf $f) 签了名（$($s.SignerCertificate.Subject)）" }
    else { Warn "$(Split-Path -Leaf $f) 没签名（$($s.Status)）：这一版是测试版，下载的人第一次打开会被 SmartScreen 拦，要点「更多信息 › 仍要运行」" }
  }

  # ---------- 4. 正式构建 ----------
  Section '4. 正式构建，没有演示数据'
  $latin1 = [Text.Encoding]::GetEncoding(28591)
  function Bytes-Text([string]$p) { $latin1.GetString([IO.File]::ReadAllBytes($p)) }
  if (Test-Path -LiteralPath $mainExe) {
    $mainText = Bytes-Text $mainExe
    $embedded = @([regex]::Matches($mainText, '/assets/[A-Za-z0-9_.-]+\.(js|css|jpg|jpeg|png|webp|gif|svg|woff2|mp4|webm)') | ForEach-Object { $_.Value } | Sort-Object -Unique)
    if ($embedded.Count -eq 0) { Fail '在桌面端里找不到嵌进去的前端文件名（/assets/…），没法核对' }
    else {
      Pass "嵌进桌面端的前端文件 $($embedded.Count) 个"
      $allowed = Get-ChildItem -ErrorAction SilentlyContinue (Join-Path $Root 'apps\desktop\src\assets\img\onboarding') | ForEach-Object { $_.BaseName }
      $stray = @($embedded | Where-Object { $_ -match '\.(jpg|jpeg|png|webp|gif|mp4|webm)$' } | ForEach-Object { Split-Path -Leaf $_ } | Where-Object { $b = $_; -not ($allowed | Where-Object { $b -like "$_-*" }) })
      if ($stray.Count) { Fail "嵌进去的图片里有上手引导剧照以外的：$($stray -join ' ')" } else { Pass '图片只有上手引导的剧照' }
      if ($embedded | Where-Object { $_ -match '^/assets/index-[A-Za-z0-9_-]+\.js$' }) { Pass '是正式构建（index.html + 分开的 js/css）' } else { Fail '没找到正式构建的 index-*.js' }
    }
  }
  $odd = Get-ChildItem -LiteralPath $D -Recurse -File | Where-Object { $_.Name -like '*mock*' -or $_.Extension -eq '.map' -or $_.Name -eq '.DS_Store' -or $_.FullName -like '*\dev\img\*' } | Select-Object -First 5 -ExpandProperty Name
  if ($odd) { Fail "安装目录里有不该有的文件：$($odd -join ' ')" } else { Pass '安装目录里没有 .DS_Store、mock、dev\img、source map' }

  # ---------- 5. 调试钩子和本机路径 ----------
  Section '5. 调试钩子和本机路径'
  foreach ($b in @($mainExe, (Join-Path $D 'captae.exe'))) {
    if (-not (Test-Path -LiteralPath $b)) { continue }
    $t = Bytes-Text $b
    $name = Split-Path -Leaf $b
    $hits = @([regex]::Matches($t, 'CAPTAE_TEST_[A-Z_]+|login-test\.json|clip-test\.json|eval\.js') | ForEach-Object { $_.Value } | Sort-Object -Unique)
    if ($hits.Count) { Fail "$name 里有调试钩子：$($hits -join ' ')" } else { Pass "$name 里没有 CAPTAE_TEST_、login-test.json、clip-test.json、eval.js" }
    # 只数不打印，免得把用户名印出来（UTF-16 的路径也查）
    $n = [regex]::Matches($t, '(?i)[a-z]:[\\/]Users[\\/](?!Public[\\/]|Default[\\/])[^\\/\x00"]+[\\/]').Count
    $n += [regex]::Matches($t, '(?i)[a-z]\x00:\x00\\\x00U\x00s\x00e\x00r\x00s\x00\\\x00').Count
    if ($n -eq 0) { Pass "$name 里没有打包机器的主目录路径" } else { Fail "$name 里有 $n 处主目录路径（里面有用户名）：用 scripts\build-release-windows.ps1 打包，编译时会去掉" }
    $pdb = @([regex]::Matches($t, '(?i)[a-z]:\\[^\x00]{0,200}?\.pdb') | ForEach-Object { $_.Value })
    if ($pdb.Count) { Fail "$name 里写着 PDB 的完整路径（$($pdb.Count) 处）：链接时加 /PDBALTPATH:%_PDB%（build-release-windows.ps1 会加）" } else { Pass "$name 里没有 PDB 的完整路径" }
  }

  # ---------- 6. 插件文件夹 ----------
  Section '6. 浏览器插件文件夹（extension）'
  function Tree-Hash([string]$dir) {
    Get-ChildItem -LiteralPath $dir -Recurse -File | Sort-Object FullName | ForEach-Object { $_.FullName.Substring($dir.Length).Replace('\', '/') + ' ' + (Get-FileHash -Algorithm SHA256 -LiteralPath $_.FullName).Hash }
  }
  $ext = Join-Path $D 'extension'
  if (-not (Test-Path -LiteralPath (Join-Path $ext 'manifest.json'))) { Fail '没有插件文件夹或 manifest.json' }
  else {
    Pass "插件文件夹在，manifest 版本 $((Get-Content -Raw -LiteralPath (Join-Path $ext 'manifest.json') | ConvertFrom-Json).version)"
    if ((Test-Path (Join-Path $ext 'popup\index.html')) -and (Test-Path (Join-Path $ext 'popup\assets\popup.js'))) { Pass 'popup\ 有构建产物' } else { Fail 'popup\ 里没有构建产物' }
    $src = Get-ChildItem -LiteralPath $ext -Recurse | Where-Object { $_.Name -like '*.ts' -or $_.Name -like '*.tsx' -or $_.Name -eq 'node_modules' -or $_.Name -like '*.map' -or $_.Name -eq 'package.json' } | Select-Object -ExpandProperty Name
    if ($src) { Fail "插件文件夹里有源码或开发文件：$($src -join ' ')" } else { Pass '插件文件夹里没有源码、node_modules、source map' }
    if (Compare-Object @(Tree-Hash $ext) @(Tree-Hash (Join-Path $Root 'extension'))) { Warn '和仓库里的 extension\ 不一样（打包以后又改过插件？或者 git 换了行尾：core.autocrlf）' } else { Pass '和仓库里的 extension\ 一样' }
  }

  # ---------- 7. 许可证 ----------
  Section '7. 许可证和第三方声明'
  if (Test-Path -LiteralPath (Join-Path $D 'THIRD_PARTY_NOTICES.md')) { Pass 'THIRD_PARTY_NOTICES.md 在' } else { Fail '没有 THIRD_PARTY_NOTICES.md' }
  # Captae 自己是 AGPL-3.0：分发二进制要附许可证原文（比内容不比字节，git 可能换了行尾）
  $lic = Join-Path $D 'LICENSE'
  if ((Test-Path -LiteralPath $lic) -and ((Get-Content -LiteralPath $lic -Raw) -replace "`r", '') -eq ((Get-Content -LiteralPath (Join-Path $Root 'LICENSE') -Raw) -replace "`r", '')) { Pass 'LICENSE（AGPL-3.0）在' } else { Fail '没有 LICENSE，或者和仓库里的不一样' }
  $missing = @('yt-dlp', 'ffmpeg', 'aria2', 'deno') | Where-Object { -not (Test-Path -LiteralPath (Join-Path $D "licenses\$_")) }
  if (-not (Test-Path -LiteralPath (Join-Path $D 'licenses\README.md'))) { Fail '没有 licenses\README.md' } elseif ($missing) { Fail "licenses\ 里缺：$($missing -join ' ')" } else { Pass 'licenses\ 在，附带程序各有自己的目录' }
  if (Test-Path -LiteralPath (Join-Path $D 'yt-dlp\_internal\THIRD_PARTY_LICENSES.txt')) { Pass 'yt-dlp 目录版自带的 THIRD_PARTY_LICENSES.txt 在' } else { Fail 'yt-dlp\_internal\THIRD_PARTY_LICENSES.txt 不在' }
  if ((Test-Path -LiteralPath (Join-Path $D 'licenses')) -and -not (Compare-Object @(Tree-Hash (Join-Path $D 'licenses')) @(Tree-Hash (Join-Path $Root 'licenses'))) -and
      (Get-FileHash -LiteralPath (Join-Path $D 'THIRD_PARTY_NOTICES.md')).Hash -eq (Get-FileHash -LiteralPath (Join-Path $Root 'THIRD_PARTY_NOTICES.md')).Hash) {
    Pass '和仓库里的 licenses\、THIRD_PARTY_NOTICES.md 一样'
  } else { Warn '和仓库里的 licenses\ 或 THIRD_PARTY_NOTICES.md 不一样（打包以后又改过？或者 git 换了行尾）' }

  # ---------- 8. 应用内更新 ----------
  Section '8. 应用内更新'
  $t = Bytes-Text $mainExe
  $marks = @([regex]::Matches($t, '__TAURI_BUNDLE_TYPE_VAR_[A-Z]+') | ForEach-Object { $_.Value } | Sort-Object -Unique)
  if ($marks.Count -eq 1 -and $marks[0] -eq '__TAURI_BUNDLE_TYPE_VAR_NSS') { Pass '桌面端记着安装方式 NSIS，更新时找对的包' } else { Fail "桌面端里的安装方式标记是 $($marks -join ' ')，应该是 __TAURI_BUNDLE_TYPE_VAR_NSS（更新会找错包）" }
  $pub = (Get-Content -Raw -LiteralPath (Join-Path $Root 'apps\desktop\src-tauri\tauri.conf.json') | ConvertFrom-Json).plugins.updater.pubkey
  if ($pub -and $t.Contains($pub)) { Pass '桌面端里的更新公钥和 tauri.conf.json 一致' } else { Fail '桌面端里没有 tauri.conf.json 里的更新公钥' }
  if ($t.Contains('https://github.com/sol1560/Captae/releases/latest/download/latest.json')) { Pass '更新地址是 GitHub 上的正式地址' } else { Fail '桌面端里没有正式的更新地址' }
  $ytv = Join-Path $D 'yt-dlp\version.txt'
  if ((Test-Path -LiteralPath $ytv) -and (Get-Content -Raw -LiteralPath $ytv).Trim() -eq $wantYtdlp) { Pass "自带的 yt-dlp 旁边记着版本 $wantYtdlp（version.txt）" } else { Fail "自带的 yt-dlp 旁边没有 version.txt，或者版本不是 $wantYtdlp" }

  # ---------- 9. 卸载（只在 -Install 时） ----------
  if ($installedHere) {
    Section '9. 卸载'
    $un = Join-Path $D 'uninstall.exe'
    if (-not (Test-Path -LiteralPath $un)) { Fail '安装目录里没有 uninstall.exe' }
    else {
      # NSIS 的卸载程序会把自己拷到临时目录再运行，/S 时立刻返回：等安装目录里的文件删掉
      $p = Start-Process -FilePath $un -ArgumentList @('/S') -Wait -PassThru
      for ($i = 0; $i -lt 60 -and (Test-Path -LiteralPath $mainExe); $i++) { Start-Sleep -Seconds 1 }
      Start-Sleep -Seconds 2
      $left = @(Get-ChildItem -LiteralPath $D -Recurse -Force -ErrorAction SilentlyContinue | Select-Object -First 5 | ForEach-Object { $_.FullName.Substring($D.Length) })
      if (-not (Test-Path -LiteralPath $D) -or $left.Count -eq 0) { Pass '安装目录删干净了' } else { Fail "卸载后安装目录里还有：$($left -join ' ')" }
      if (Test-Path $uninstallKey) { Fail '卸载登记还在（HKCU\…\Uninstall\Captae）' } else { Pass '卸载登记删掉了' }
      $sm = Join-Path ([Environment]::GetFolderPath('Programs')) 'Captae'
      if (Test-Path -LiteralPath $sm) { Fail "开始菜单里的 Captae 还在（$sm）" } else { Pass '开始菜单里的 Captae 删掉了' }
      $installedHere = $false
    }
  }
} finally {
  if ($installedHere -and (Test-Path -LiteralPath (Join-Path $D 'uninstall.exe'))) {
    Start-Process -FilePath (Join-Path $D 'uninstall.exe') -ArgumentList @('/S') -Wait | Out-Null
    Start-Sleep -Seconds 5
  }
  if ($touchedDirKey) {
    if (-not $dirKeyExisted) { Remove-Item -Path $dirKey -Recurse -Force -ErrorAction SilentlyContinue }
    elseif ($null -ne $dirKeyBefore) { Set-ItemProperty -Path $dirKey -Name '(default)' -Value $dirKeyBefore -ErrorAction SilentlyContinue }
    else { Remove-ItemProperty -Path $dirKey -Name '(default)' -ErrorAction SilentlyContinue }
    $now = if (Test-Path $dirKey) { (Get-ItemProperty -Path $dirKey -ErrorAction SilentlyContinue).'(default)' } else { $null }
    if ($now -and $now.StartsWith($Work, [StringComparison]::OrdinalIgnoreCase)) { Fail "安装目录记录还指着临时目录（$dirKey = $now）" }
  }
  Remove-Item -Recurse -Force -LiteralPath $Work -ErrorAction SilentlyContinue
}

Write-Host ''
if ($Script:Fails -eq 0) { Write-Host "全部通过（提醒 $($Script:Warns) 条）"; exit 0 }
Write-Host "没过 $($Script:Fails) 项（提醒 $($Script:Warns) 条）"
exit 1
