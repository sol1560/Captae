; Captae 的 NSIS 安装 / 卸载钩子（tauri.release.windows.conf.json 的 bundle.windows.nsis.installerHooks）。
; Tauri 自带的安装程序模板（tauri-bundler 2.10 的 installer.nsi）在安装、卸载的开头和结尾各插一个钩子。
; 这个文件要存成带 BOM 的 UTF-8（NSIS 3 靠 BOM 认 UTF-8）。
;
; 1. 安装（含升级）、卸载之前：结束从安装目录里启动的附带程序（captae.exe 插件宿主、yt-dlp、ffmpeg、ffprobe、aria2c、deno）。
;    它们开着时文件删不掉、覆盖不了：浏览器拉起的插件宿主 captae.exe 不是桌面端的子进程，桌面端关了它还在。
;    怎么认、哪些不结束写在旁边的 stop-helpers.ps1 开头（打进安装程序，运行时解到 $PLUGINSDIR）。
;    点「重新打开」更新（passive 模式）和用户自己运行新版安装程序都走这里，不靠桌面端的更新代码。
;    安装程序是 32 位的：$SYSDIR 下的 powershell.exe 会被转到 32 位的那个，读不到 64 位进程的路径，原来的写法一个都没结束
;    （Devin 的 Windows U5、U7：Error opening file for writing: …\captae.exe）。现在 64 位系统上运行 Sysnative 下的 64 位 PowerShell，
;    脚本里的进程列表也改从 WMI 取（32、64 位都读得到）。
; 2. 安装时再把旧的 captae.exe 挪开（Windows 允许给正在运行的程序改名，不许覆盖、删除）。结束宿主以后到写新的 captae.exe 之间有几秒到十几秒
;    （模板先写桌面端和资源：yt-dlp 目录、插件文件夹，附带程序在最后），浏览器这时又拉起宿主的话（插件每分钟问一次 Captae，网页上发现视频时也问）
;    captae.exe 又被占住。挪开以后这段时间里浏览器拉不起宿主，插件显示连不上，装完下一次问的时候连上新的。
;    挪开的旧文件装完就删（还有旧宿主在用时删不掉，留到下次装、卸载时再删）；装到一半失败了（写不进文件点了「中止」、
;    「Captae 正在运行」时点了「取消」）在 .onInstFailed 里放回去，不留下一个没有 captae.exe 的安装目录。
; 3. 卸载（不是升级）时：删掉 Captae 在注册表里登记的浏览器插件宿主（各浏览器读的 NativeMessagingHosts\app.captae.bridge 键），
;    不然浏览器还会去找已经删掉的程序。登记由核心库 crates/captae-core/src/native_host.rs 写（Chrome、Chromium、Edge 三个键；
;    Brave、Vivaldi、Opera 在 Windows 上读 Chrome 的键），两边要一致。
; 4. 卸载时勾了「删除应用程序数据」（默认不勾）：Tauri 模板只删 %APPDATA%\app.captae.desktop 和 %LOCALAPPDATA%\app.captae.desktop
;    （WebView2 的缓存），Captae 自己的数据目录是 %LOCALAPPDATA%\Captae\data（设置、素材库数据库、自动更新的 yt-dlp、
;    语音识别环境、登录状态，crates/captae-core/src/config.rs 的 Paths::default_for_user），这里一起删。
;    默认安装目录也是 %LOCALAPPDATA%\Captae：不勾时 Tauri 模板只删自己装的文件、RMDir 不带 /r，data\ 留着，升级、重装后接着用。
;    下载的视频（「视频」文件夹里的 Captae）永远不删。数据目录要是改了，这里要跟着改。

; 这个文件所在的目录，在这里（被 !include 时）记下：宏是在插入它的地方展开的，宏里的 ${__FILEDIR__} 是 Tauri 生成的 installer.nsi 的目录
!define CAPTAE_HOOKS_DIR "${__FILEDIR__}"

!macro CAPTAE_STOP_HELPERS
  Push $0
  InitPluginsDir
  File "/oname=$PLUGINSDIR\captae-stop-helpers.ps1" "${CAPTAE_HOOKS_DIR}\stop-helpers.ps1"
  ; 64 位系统上运行 64 位的 PowerShell：32 位的程序要从 Sysnative 才看得到真正的 System32
  StrCpy $0 "$SYSDIR\WindowsPowerShell\v1.0\powershell.exe"
  ${If} ${RunningX64}
  ${AndIf} ${FileExists} "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
    StrCpy $0 "$WINDIR\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
  ${EndIf}
  nsExec::ExecToLog '"$0" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "$PLUGINSDIR\captae-stop-helpers.ps1" "$INSTDIR"'
  Pop $0 ; 退出码，不看（结束不了的，接下来写文件时安装程序会照常报错）
  Pop $0
!macroend

; 挪开的旧 captae.exe（见开头第 2 条）
!macro CAPTAE_DELETE_OLD_HOST
  Delete "$INSTDIR\captae.exe.old"
  Delete "$INSTDIR\captae.exe.old2"
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro CAPTAE_STOP_HELPERS
  !insertmacro CAPTAE_DELETE_OLD_HOST
  ${If} ${FileExists} "$INSTDIR\captae.exe"
    ClearErrors
    Rename "$INSTDIR\captae.exe" "$INSTDIR\captae.exe.old"
    ${If} ${Errors}
      ; 上一次挪开的还有旧宿主在用、删不掉：换个名字
      Rename "$INSTDIR\captae.exe" "$INSTDIR\captae.exe.old2"
    ${EndIf}
  ${EndIf}
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro CAPTAE_DELETE_OLD_HOST
!macroend

; 装到一半失败了：挪开的 captae.exe 放回去（新的已经写进去了就不动）。.old2 是后挪的，先看它
Function .onInstFailed
  ${IfNot} ${FileExists} "$INSTDIR\captae.exe"
    ${If} ${FileExists} "$INSTDIR\captae.exe.old2"
      Rename "$INSTDIR\captae.exe.old2" "$INSTDIR\captae.exe"
    ${ElseIf} ${FileExists} "$INSTDIR\captae.exe.old"
      Rename "$INSTDIR\captae.exe.old" "$INSTDIR\captae.exe"
    ${EndIf}
  ${EndIf}
FunctionEnd

!macro NSIS_HOOK_PREUNINSTALL
  !insertmacro CAPTAE_STOP_HELPERS
  !insertmacro CAPTAE_DELETE_OLD_HOST
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
