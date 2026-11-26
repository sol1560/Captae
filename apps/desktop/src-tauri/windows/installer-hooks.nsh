; Captae 的 NSIS 安装 / 卸载钩子（tauri.release.windows.conf.json 的 bundle.windows.nsis.installerHooks）。
; Tauri 自带的安装程序模板（tauri-bundler 2.9.4 的 installer.nsi）在安装、卸载的开头和结尾各插一个钩子。
; 这个文件要存成带 BOM 的 UTF-8（NSIS 3 靠 BOM 认 UTF-8）。
;
; 1. 安装（含升级）、卸载之前：结束从安装目录里启动的附带程序（captae.exe 插件宿主、yt-dlp、ffmpeg、ffprobe、aria2c、deno）。
;    它们开着时文件删不掉、覆盖不了：浏览器拉起的插件宿主 captae.exe 不是桌面端的子进程，桌面端关了它还在。
;    桌面端主程序（captae-desktop.exe）不在这里结束，留给 Tauri 模板接下来的 CheckIfAppIsRunning 问用户。
;    只认路径在安装目录下的进程，不碰用户自己装的 ffmpeg、yt-dlp。
; 2. 卸载（不是升级）时：删掉 Captae 在注册表里登记的浏览器插件宿主（各浏览器读的 NativeMessagingHosts\app.captae.bridge 键），
;    不然浏览器还会去找已经删掉的程序。登记由核心库 crates/captae-core/src/native_host.rs 写（Chrome、Chromium、Edge 三个键；
;    Brave、Vivaldi、Opera 在 Windows 上读 Chrome 的键），两边要一致。
; 3. 卸载时勾了「删除应用程序数据」（默认不勾）：Tauri 模板只删 %APPDATA%\app.captae.desktop 和 %LOCALAPPDATA%\app.captae.desktop
;    （WebView2 的缓存），Captae 自己的数据目录是 %LOCALAPPDATA%\Captae\data（设置、素材库数据库、自动更新的 yt-dlp、
;    语音识别环境、登录状态，crates/captae-core/src/config.rs 的 Paths::default_for_user），这里一起删。
;    默认安装目录也是 %LOCALAPPDATA%\Captae：不勾时 Tauri 模板只删自己装的文件、RMDir 不带 /r，data\ 留着，升级、重装后接着用。
;    下载的视频（「视频」文件夹里的 Captae）永远不删。数据目录要是改了，这里要跟着改。

!macro CAPTAE_STOP_HELPERS
  ; 安装目录通过环境变量交给 PowerShell，免得路径里的引号、空格把命令弄坏
  System::Call 'kernel32::SetEnvironmentVariable(t "CAPTAE_INSTDIR", t "$INSTDIR")'
  nsExec::ExecToLog `"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "$$d = [IO.Path]::GetFullPath($$env:CAPTAE_INSTDIR).TrimEnd('\') + '\'; Get-Process | Where-Object { $$_.Path -and $$_.Path.StartsWith($$d, [StringComparison]::OrdinalIgnoreCase) -and $$_.ProcessName -ne 'captae-desktop' } | ForEach-Object { Write-Output ('stop ' + $$_.ProcessName + ' ' + $$_.Id); Stop-Process -Id $$_.Id -Force -ErrorAction SilentlyContinue }"`
  Pop $0
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro CAPTAE_STOP_HELPERS
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro CAPTAE_STOP_HELPERS
  ${If} $UpdateMode <> 1
    DeleteRegKey HKCU "Software\Google\Chrome\NativeMessagingHosts\app.captae.bridge"
    DeleteRegKey HKCU "Software\Chromium\NativeMessagingHosts\app.captae.bridge"
    DeleteRegKey HKCU "Software\Microsoft\Edge\NativeMessagingHosts\app.captae.bridge"
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $DeleteAppDataCheckboxState = 1
  ${AndIf} $UpdateMode <> 1
    SetShellVarContext current
    RmDir /r "$LOCALAPPDATA\Captae\data"
    RmDir "$LOCALAPPDATA\Captae"
  ${EndIf}
!macroend
