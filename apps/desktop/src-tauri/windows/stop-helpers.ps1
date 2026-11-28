# Captae 安装、卸载之前：结束安装目录里正在运行的附带程序（installer-hooks.nsh 的 CAPTAE_STOP_HELPERS 调用）。
# 这个文件要存成带 BOM 的 UTF-8：Windows PowerShell 5.1 读不带 BOM 的脚本时按系统的 ANSI 编码，中文注释会读乱。
# 要能在 Windows PowerShell 5.1 上运行（Windows 10 自带的），不用 PowerShell 7 才有的写法。
#
# 要结束的：路径在安装目录下的 captae.exe（浏览器拉起的插件宿主，不是桌面端的子进程，桌面端关了它还在），
# 以及它们起的 yt-dlp、ffmpeg、ffprobe、aria2c、deno。它们开着时文件覆盖不了，安装程序报「Error opening file for writing」。
# 不结束的：
# - 桌面端 captae-desktop.exe 本身：留给 Tauri 模板接下来的 CheckIfAppIsRunning（问用户，passive 模式下直接结束）。
# - 正在运行的桌面端起的附带程序（祖先里有安装目录下的 captae-desktop.exe）：桌面端被结束时自己收拾（每个附带程序在自己的 Job 里）；
#   在这里先结束的话，桌面端会把正在下的任务记成下载失败（用户开着 Captae 手动运行新版安装程序时）。
#   点「重新打开」更新时桌面端在运行安装程序之前已经退出、收拾过了，这一条碰不上。
# - 别的地方的程序（用户自己装的 ffmpeg、yt-dlp）。
#
# 进程列表从 WMI（Win32_Process）取：NSIS 安装程序是 32 位的，32 位的 PowerShell 用 Get-Process 读不到 64 位进程的路径（Path 是空的），
# 原来的写法因此一个进程都没结束（Devin 的 Windows U5、U7）。钩子那边也改成运行 64 位的 PowerShell；WMI 两种都读得到。
# 结束了哪些写进 %TEMP%\captae-installer-hooks.log（passive 模式看不到安装程序的详细信息，出了问题靠它查）。
param([Parameter(Mandatory = $true)][string]$InstDir)

$ErrorActionPreference = 'SilentlyContinue'
# NSIS 的 $INSTDIR 本来就是完整路径，只统一斜杠、末尾加一个 \（免得 Captae 匹配上 CaptaeOld）
$dir = $InstDir.Replace('/', '\').TrimEnd('\') + '\'
$log = Join-Path $env:TEMP 'captae-installer-hooks.log'
function Say([string]$s) {
  Write-Output $s
  Add-Content -LiteralPath $log -Value ((Get-Date).ToString('yyyy-MM-dd HH:mm:ss') + ' ' + $s)
}

$all = @(Get-CimInstance -ClassName Win32_Process)
if ($all.Count -eq 0) { $all = @(Get-WmiObject -Class Win32_Process) }
$byId = @{}
foreach ($p in $all) { $byId[[int]$p.ProcessId] = $p }

function InDir($p) { [bool]($p.ExecutablePath -and $p.ExecutablePath.StartsWith($dir, [StringComparison]::OrdinalIgnoreCase)) }
function IsDesktop($p) { (InDir $p) -and ($p.Name -ieq 'captae-desktop.exe') }
# 祖先里有安装目录下的桌面端。进程号会复用：「父进程」比自己晚创建的不是真的父进程，到此为止
function UnderDesktop($p) {
  $seen = @{}
  $child = $p
  $q = $byId[[int]$p.ParentProcessId]
  while ($q -and -not $seen.ContainsKey([int]$q.ProcessId)) {
    if ($child.CreationDate -and $q.CreationDate -and $q.CreationDate -gt $child.CreationDate) { return $false }
    if (IsDesktop $q) { return $true }
    $seen[[int]$q.ProcessId] = $true
    $child = $q
    $q = $byId[[int]$q.ParentProcessId]
  }
  return $false
}

Say ('stop helpers in ' + $dir + ' (' + $all.Count + ' processes, ' + [IntPtr]::Size * 8 + '-bit PowerShell)')
$stopped = @()
if ($all.Count -gt 0) {
  foreach ($p in $all) {
    if ((InDir $p) -and -not (IsDesktop $p) -and -not (UnderDesktop $p)) {
      Say ('stop ' + $p.Name + ' ' + $p.ProcessId)
      Stop-Process -Id ([int]$p.ProcessId) -Force
      $stopped += [int]$p.ProcessId
    }
  }
} else {
  # WMI 读不出来（服务坏了）：退回 Get-Process（钩子运行的是 64 位的 PowerShell，读得到路径），只是认不出谁是桌面端起的
  foreach ($p in @(Get-Process)) {
    if ($p.Path -and $p.Path.StartsWith($dir, [StringComparison]::OrdinalIgnoreCase) -and $p.ProcessName -ne 'captae-desktop') {
      Say ('stop ' + $p.ProcessName + ' ' + $p.Id)
      Stop-Process -Id $p.Id -Force
      $stopped += $p.Id
    }
  }
}
# Stop-Process 不等进程真的退出：最多等 5 秒，文件才放得开
if ($stopped.Count -gt 0) { Wait-Process -Id $stopped -Timeout 5 }
exit 0
