# Captae 交接文档

2026-11-24 第三轮交接（含收尾修补和最后一轮，版本 0.2.0），2026-11-26 补了公开发布前的改动。下面是真实状态：做了什么、怎么构建和测试、测到什么程度、还有什么没做。
设计和架构看 `docs/DESIGN.md`，界面规范看 `docs/design/README.md`，动效看 `docs/MOTION.md`，多语言看 `docs/I18N.md`，许可证看 `docs/LICENSING.md` 和 `THIRD_PARTY_NOTICES.md`。

## 现在是什么

macOS、Windows、Linux 上的开源下载器（AGPL-3.0-or-later），四块共用一个 Rust 核心库：

- 桌面端（Tauri 2 + React 新界面）：下载、素材库、AI 助手、浏览器插件四页，设置，上手引导，登录窗口。
- 浏览器插件（Chrome 系，MV3）：网页上的下载按钮和清晰度菜单、进度卡片、右键菜单、工具栏面板，可以带上浏览器里的登录状态。
- 命令行 `captae`：浏览器插件的 Native Messaging 宿主、AI 助手用的 MCP 服务器，以及 probe / download / update / engine / status 等命令。
- `skills/captae/SKILL.md`：写给 AI 助手的工具说明。

## 第三轮（2026-11-23 — 11-24）做了什么

- **下载可靠性**：失败都归到 15 个错误码，界面、插件、AI 助手都按错误码给原因和下一步按钮，不再只有一句「读取视频信息失败」；安装包附带 deno（从访达打开时 yt-dlp 找不到 JS 运行时，YouTube 读不了，这是第二轮最主要的失败原因）；
  yt-dlp 换成官方目录版并每天自动更新（官方公钥验签、核对 SHA256、自检通过才切换、坏了自动退回）；跟随系统代理或手动代理；播放列表和多视频帖子让用户选；限流时自动重试；
  读不到公开 YouTube 视频时换一组客户端再试；附带的 ffmpeg 读 https 用系统根证书包；aria2 暂停后能续传；入库前核对（网页、单张图片不再当成视频入库）；付费内容只拿到试看片段算失败；
  认出直播和电台流，「从现在开始录」和「停止并保存」；结束整棵进程树（不再留下孤儿 ffmpeg）；转换、字幕、剪辑、拼接、预设、发给 AI 都能取消，半成品和中间文件会删掉；几种链接先改写成能下的地址。
- **登录和人机验证**：Captae 里的登录窗口（两块 webview、命令白名单、拦截规则、单独的登录存储，做法见 DESIGN 第 5 节）；下载时带 cookies（临时文件，用完就删，YouTube 只在需要时才带）；
  设置里「已登录的网站」和默认关的「也读取浏览器里的登录状态」；插件带浏览器的登录状态（下载、读取信息、发给 AI）。
- **几百个网站实测**：682 条链接、251 个网站，用发布版命令行在模拟安装包、模拟从访达打开的条件下跑，按结果修了 19 类问题（见下面「测试状态」）。
- **新界面**：桌面端从原生 HTML 改成 React 19 + Vite + affectUI 免费组件（原样放在 `apps/desktop/src/components/ui/`）+ Motion + Sonner + Hugeicons Free，第二版设计「轻玻璃」（中性近白、唯一强调色、只给浮在内容上的东西用玻璃，规范在 `docs/design/README.md`）。
  第二轮的原生 HTML 界面（`apps/desktop/ui/`）已删，git 历史里有。演示后端（`?mock`）能在普通浏览器里看全部界面，截图在 `screenshots/`。
- **插件改版**：网页部件和工具栏面板按第二版重做；面板改成 React（源码 `apps/desktop/src/extension-popup/`，构建产物在 `extension/popup/`）；带登录状态的开关按网站记；
  宿主看父进程认出是 16 家浏览器里的哪一家；计划里的 11 项逐项实测通过，B 站开着登录状态真的下完。
- **动效**：全应用按课程和录屏实测评审过一遍，规矩和数值写进 `docs/MOTION.md`，系统「减少动态效果」时每一处都只淡入淡出。
- **其他**：插件或命令行要求打开某条素材 / 某个任务时直接跳过去；AI 调用记录里要登录的失败有「登录 X」；「发给 AI」测试用的不写剪贴板开关；文档、第三方声明、开源检查、打包检查脚本。
- **录直播收尾**：桌面端、插件网页卡片、工具栏面板三处都能「停止并保存」（卡片和面板里「取消录制」先问一句）；最多录多久（AI 助手 `max_minutes` 默认 10 分钟、最多 180，命令行 `--max-minutes`，桌面端不加设置）；
  命令行录制中第一次 Ctrl-C 停止并保存、第二次直接退出，没整理的原始数据保留 7 天、之后启动时自动删（`recordings.rs`，桌面端等着整理的不删）；
  **录过的直播不算「下载过了」**（素材库新加 `live` 列，查重跳过它，插件按钮、菜单、面板都一样，普通视频照旧）。
- **剪贴板提示改成「复制了、1 分钟内打开 Captae 才问」**（11-24 用户提的）：复制那一刻桌面端和插件都不弹；Captae 只看剪贴板的变更次数、拿到焦点时才读一次；
  Captae 没开时插件经宿主记下复制的链接和时间，打开时用；插件网页上的复制卡片和面板里「复制视频链接时问我」开关删了。侧栏「浏览器里发现的」只放最近 3 条视频和声音。做法见 DESIGN 9.1。
  侧栏底部「AI 助手」那一行：原来写连着的 AI 助手的名字，名字很长（`local-agent-mode-captae 等 2 个`）时把「AI 助手」挤没、压到设置按钮上；
  改成只写状态（已连接 / 已连接 N 个 / 未连接 / 已关闭），名字在鼠标停上去的提示里，AI 助手页和上手引导第 4 页照旧写全名。
- **更稳**：失败信息带网站名字（`site`，「登录芒果TV」）；asset 协议从 `$HOME/**` 收窄成按需放行；插件和 Captae 之间一时没回话时多等一会儿，卡片不再误报「Captae 关掉了」、面板「全部下载」不再悄悄漏一条；
  桌面端收到 SIGTERM、SIGHUP、SIGINT 时和 ⌘Q 一样收拾（不再留下孤儿 yt-dlp、ffmpeg、aria2c，不再留下 `control.json`）；暂停着的任务重开几次都还是暂停（以前第二次重开会自己开始下）；
  插件卡片「从现在开始录」不再在 Captae 的下载列表里留一条「这是正在进行的直播」失败任务。
- **打包**：版本 0.2.0；用 Developer ID 证书签名、经过 Apple 公证（app 和 dmg 上都钉着公证结果）；附带的 aria2c 改成在没装 Homebrew 的 Mac 上也能用（见下面「打包」）；
  前端 `@tauri-apps/api` 和 Rust 的 tauri 对齐到 2.11；用 9 月的旧版造数据核对过升级（见「测试状态」）。

前两轮（2026-09-26 交接）做了第一版功能和逐个控件的补齐：三个下载引擎、队列、SQLite 素材库、转换、剪辑、拼接、字幕、预设、「发给 AI」、本地控制接口、MCP 服务器、插件、Skill，打出了第一个 dmg。

## 多语言（2026-11-24 晚，第三轮之后）

- 界面、提示、命令行输出支持 8 种语言：简体中文（原文）、English、繁體中文、日本語、한국어、Deutsch、Español、Français。设置最上面「通用 › 语言」，默认跟随系统（系统语言都不支持时用英文），换了马上生效，不用重开。
  桌面端、命令行、MCP、插件宿主读同一项设置；浏览器插件跟 Captae 实际用的语言，电脑上没有 Captae 时跟浏览器。写法、词表、术语、检查脚本见 `docs/I18N.md`，架构见 DESIGN 第 13 节。
- 只能写一种语言的地方用英文（用户定的）：安装包窗口（标题「Drag Captae to Applications」，快捷方式叫 Applications）、给 AI 看的说明（MCP 的工具说明、错误里的 `next`、`skills/captae/SKILL.md`）、打包配置的 `shortDescription`、各 crate 的 `description`。
  macOS 菜单栏换成自己建的、跟着语言换的菜单（`src-tauri/src/menu.rs`）。
- 词表一共约 2180 句原文（界面 1247、核心库和命令行 701、插件网页部件 232），7 种语言都齐（`python3 scripts/i18n.py check --strict` 通过）。
  英文和其他 6 种语言都是 AI 按 `docs/I18N.md` 的术语表和风格翻译、几种语言之间互相对齐过的，还没请母语的人看过。
- 界面和插件不再拿文字做判断（文字跟着语言变），后端加了几个固定字段：`fail.reason`（`image_post` / `radio`）、`fail.brief`、`job.untitled`、`job.probeAt`，固定代码的错误（`NOT_RECORDING`）。
  `ErrorCode::next_step` 只有英文（给 AI 看），命令行给人看的下一步用 `cli_hint`。
- 另外修了：网页里右键「用 Captae 下载」报错、卡片出不来（`shortUrl` 没有定义，第三轮删复制卡片时连定义一起删了）；字幕翻译的目标改成界面语言（原来写死「中文」，判断「已经是这种语言」也跟着改）。
- 以后加给人看的文字：代码里写中文原文，包上 `t()` / `tr!` / `tr()`，英文词表里加一条（`check` 会查）；其他 6 种语言可以攒一批再翻（`i18n.py export <语言>` 导出、翻好 `import`，再 `tidy`、`quotes`）。没翻的显示英文（繁体中文显示简体原文）。

## Windows / Linux 版（2026-11-24 — 11-25）

- 新增 Windows 10/11 x64（NSIS 安装程序，装给当前用户、不签名）和 Linux x86_64（deb、AppImage，Ubuntu 22.04 上打）版，功能和 macOS 一样，含登录窗口和浏览器插件。
  和 macOS 不一样的地方、为什么这样做，写在 `docs/DESIGN.md` 第 14 节；安装说明在 README；检查规矩在 AGENTS.md。
- 打包：`scripts/build-release-windows.ps1`（在 Windows 上）、`scripts/build-release-linux.sh`（在 Ubuntu 22.04 上），各自最后跑 `check-bundle-windows.ps1` / `check-bundle-linux.sh`。
  本机只能交叉检查 Windows 编译（`cargo check --target x86_64-pc-windows-gnu`）、在 Docker 的 Ubuntu 22.04 里跑 Linux 测试。
- 实测：在 Devin 的 Windows Server 2022、Ubuntu 22.04 虚拟机上打包、安装、逐项实测了 6 轮，按结果修了二十多处（yt-dlp 在 Windows 上丢全角字符、WebKitGTK 放不了 asset 协议的视频、
  Chrome 给插件宿主的 `DBUS_SESSION_BUS_ADDRESS=disabled:`、登录后读到正在关的 webview、识别脚本写死 `/dev/null`、PyAV 19 不兼容……）。最后一轮全部通过：
  下载（直链、下载引擎、全角文件名）、暂停 / 取消 / 退出不留孤儿进程、录直播停止并保存、素材库播放 / 转换 / 剪辑 / 字幕、语音识别安装和转文字、剪贴板提示、
  Chrome / Edge 插件（宿主登记、网页按钮、工具栏面板、插件拉起 Captae）、登录窗口 10 项（含 Google 登录页不被拦）、系统代理、yt-dlp 自动更新、MCP、安装和卸载。
  截图、录屏和每轮记录在维护者本机 `work/winlinux/runs/func/`。
- 已知做不到 / 没测的：Windows 11 悬停最大化按钮出贴靠布局（Win+Z 能用）；标题条右键没有系统菜单；Linux 固定三个窗口按钮在右边；
  Windows 上 Chrome / Edge 的登录状态读不了（app-bound 加密，设置里写明，可选 Firefox）；Flatpak / Snap 版浏览器的插件要用户放宽沙盒；
  YouTube、B 站在 Devin 的机器上被人机验证和地区限制拦，没在 Windows / Linux 上真的下过；ARM 版没有安装包。
- 主目录新加的 `crates/captae-core/src/engine/remux.rs` 有三个测试把结果和本机 ffmpeg 精确比对（起点差 0.2 毫秒也算不过）：
  Mac 上的 ffmpeg 8.0 能过，Linux 容器里的 ffmpeg 4.4、9.0 和 GitHub 的 macOS 机器上各有不过的；2026-11-26 `ts_h264_aac` 改成起点允许差 2 毫秒（CI 上差 0.22 毫秒），另外两个照旧，在别的 ffmpeg 版本上不过时照这样改。

## 目录

```
crates/captae-core        核心库
crates/captae-cli         命令行 captae
apps/desktop/src-tauri    桌面端 Rust 外壳（Tauri 2）
apps/desktop/src          桌面端界面（React）和插件工具栏面板的源码
extension                 Chrome 插件（工具栏面板是构建产物，提交进仓库，不用构建就能加载）
skills/captae             AI 助手的 Skill
scripts                   build-release.sh（打正式安装包）、fetch-sidecars.sh（准备附带程序）、check-bundle.sh（检查安装包）、gen-licenses.py（汇总依赖许可证）、captae_whisper.py（语音识别）
licenses                  附带程序和第三方组件的许可证、源码地址（打进安装包）
docs                      DESIGN.md、MOTION.md、LICENSING.md，design/（界面规范）
screenshots               新界面的演示版截图
keys                      插件 ID 私钥（不进仓库，见下文）
```

`work/`（计划、实测记录、测试脚本）只在维护者本机，不进仓库。

## 构建和运行

需要 Rust（1.93.1 编译通过）、Node（26 编译通过，构建界面用），跑端到端测试还要本机有 ffmpeg、aria2c、yt-dlp（`brew install ffmpeg aria2 yt-dlp`）。

```
cd apps/desktop && npm ci                     # 第一次：装界面的依赖
cargo test -p captae-core                     # 核心库单元测试 + 端到端
cargo test -p captae-desktop                  # 桌面端：命令白名单、登录窗口规则、路由
cd apps/desktop && npx tsc --noEmit -p .      # 界面类型检查
cd apps/desktop && npm run build:mock         # 演示构建：dist-mock/index.html，浏览器里带 ?mock 打开
cargo run -p captae-cli -- mcp --stdio        # MCP 服务器
```

开发版桌面端**一定要换标识、设测试数据目录**再跑（原因见 DESIGN 第 11 节）：

```
TAURI_CONFIG='{"identifier":"app.captae.dev"}' cargo build -p captae-desktop
CAPTAE_HOME=<测试目录> target/debug/captae-desktop
```

测试数据目录的 `data/settings.json` 写 `{"autoUpdate": false}`。实测「发给 AI」时另设 `CAPTAE_TEST_NO_CLIPBOARD=1`（调试版才有，不写系统剪贴板）；台前调度开着时设 `CAPTAE_TEST_ONTOP=1`。
**在别人正用着的电脑上实测**（测试版的窗口要是浮到屏幕上，会被当成自己的 Captae 来用，第三轮出过一次）：测试版放进 Info.plist 写了 `LSUIElement` 的外壳、编译时让主窗口一开始就隐藏、
一律 `--background` 启动、设 `CAPTAE_TEST_NO_SHOW=1`、不设 `CAPTAE_TEST_ONTOP`、不往 `login-test.json` 写 `{"top": true}`，核对用 `eval.js` 读页面；停的时候用 osascript 正常退出。细节见 DESIGN 第 11 节。

插件：在 `chrome://extensions` 打开开发者模式，「加载已解压的扩展程序」选 `extension/`。插件 ID 由 manifest 里的 `key` 固定为 `docjnhebpmdchlahmhkjladmehlcfkcp`，和宿主清单里的一致。
对应的私钥在 `keys/extension-key.pem`，只有打 .crx 包时才需要；`keys/`、`*.pem` 在 `.gitignore` 里，不要提交。

## 打包

```
cargo install tauri-cli --version "^2" --locked   # 只需一次
scripts/build-release.sh                      # fetch-sidecars.sh → cargo tauri build（.app）→ 有证书时 sign-macos.sh 重签、交给 Apple 公证 → dmgbuild 做 dmg（有证书时也签名、公证）→ check-bundle.sh，编译时去掉本机路径
scripts/render-dmg-background.sh              # 改了安装包窗口的背景页或 layout.json 以后重新截背景图
```

产物在 `target/release/bundle/dmg/`。2026-11-24 18:51 打的最终安装包 `Captae_0.2.0_aarch64.dmg`（Developer ID 签名、Apple 公证）：

- 安装包 153.2 MB，装好约 355 MB（yt-dlp 带的 Python.framework 恢复成符号链接，少了 3 份 14 MB 的副本）；附带 yt-dlp 2026.10.26、deno 2.9.8、ffmpeg / ffprobe 9.0.2（带 libass）、aria2 1.37.0、captae 0.2.0，
  在干净的 PATH、看不到 Homebrew 目录的环境里都跑得出版本，deno 能跑 JS，117 个程序和库的加载路径里没有 Homebrew 的目录。安装包里没有演示数据（前端只嵌了上手引导的四张剧照）、没有调试钩子；
  插件文件夹、`licenses/`、`THIRD_PARTY_NOTICES.md` 和仓库一致。用 `scripts/build-release.sh` 打的，桌面端和命令行里都没有本机主目录的路径（原来各有 1063、744 处）。
- 签名和公证：整个 app 和里面 117 个程序和库都是同一个团队的 Developer ID 签的，开着 hardened runtime、带时间戳；app（打成 zip）和 dmg 各交给 Apple 公证一次，都是 Accepted、0 条提醒，
  公证结果钉在 app 和 dmg 上。`spctl` 对 app 和 dmg 都是 accepted（Notarized Developer ID），Apple 的 `syspolicy_check distribution` 说可以发布。`scripts/check-bundle.sh` 全部通过、提醒 0 条。
- 用包里的命令行（复制出来、设测试数据目录、看不到 Homebrew 的目录）下 YouTube、B 站、直链、本机 M3U8 各一段样片，都成功，用的是包里的 yt-dlp 和 deno；
  另外完整下载两条直链（本机 http、w3schools 的 https 样片），走的是包里的 aria2c，也都成功。
- **11-24 之前打的 0.2.0（09:27 用 Tauri 打的、17:48 本地签名的）有个问题**：附带的 aria2c 在没装 Homebrew 的 Mac 上一启动就退出（它带的 libcrypto 把 OpenSSL 模块目录写死在打包那台电脑的 Homebrew 里），
  https 直链也校验不了证书，直链下载、录电台都用不了。维护者的电脑上装着 Homebrew，原来的检查一直测不出来。18:51 这份修好了，做法见 DESIGN 4.3；检查脚本、回归现在都在看不到 Homebrew 的环境里跑附带程序。
- 打包时 `cargo tauri build` 会核对前端 `@tauri-apps/api` 和 Rust `tauri` 的小版本（现在都是 2.11），不用再加 `--ignore-version-mismatches`。
- 要用 `scripts/build-release.sh` 打包：Rust 默认把源码路径写进程序（依赖在 `~/.cargo/registry/src/<镜像>/` 下），直接 `cargo tauri build` 打出来的程序带着打包人的用户名和用的 crates.io 镜像；
  脚本编译时用 `--remap-path-prefix`（C 代码用 `-ffile-prefix-map`）去掉，`check-bundle.sh` 会检查程序里还有没有主目录的路径。
- 本地签名靠 `tauri.release.conf.json` 的 `bundle.macOS`：`"signingIdentity": "-"` 加 `"hardenedRuntime": false`（理由见 DESIGN 第 12 节）。
- 钥匙串里有「Developer ID Application」证书时，`build-release.sh` 自动用 `scripts/sign-macos.sh` 整个重签（签完在 hardened runtime 下把附带程序都跑一遍）、交给 Apple 公证、钉上结果。
  公证凭据是 `~/.appstoreconnect/private_keys/AuthKey_*.p8` 加 `keys/notary.env` 里的 Issuer ID（`keys/` 不进仓库）。没有证书就照旧只做本地签名，下载的人第一次打开会被系统拦。做法见 DESIGN 第 12 节。
- 安装包窗口（11-24 用户提的）：不再用 Tauri 的 dmg 脚本（AppleScript 排版、要人看着屏幕，0.2.0 第一次打的是默认白窗口），改用 dmgbuild 直接写 `.DS_Store`；
  窗口是轻玻璃加一条开放电影剧照的「胶片」，做法见 DESIGN 第 12 节，`check-bundle.sh` 第 8 项核对。
- 版本号在四处，一起改：`Cargo.toml`（`[workspace.package]`，三个 crate 跟着走，命令行和 MCP 报的版本也是它）、`apps/desktop/src-tauri/tauri.conf.json`（安装包和 Info.plist 的版本）、
  `apps/desktop/package.json`（`package-lock.json` 跟着 npm 更新）、`extension/manifest.json`。

## 测试状态（2026-11-24，macOS / Apple Silicon）

- `cargo test -p captae-core`：208 个单元测试 + 1 个端到端，全过（端到端在装了 ffmpeg、aria2c、yt-dlp 的机器上真的下载、转换、剪辑、拼接、嵌字幕）；`cargo test -p captae-desktop`：8 个全过；`cargo build -p captae-desktop` 没有警告（数字是多语言合并以后的）。
- 界面：`tsc` 没有错；演示构建上下载页检查 34 项、素材库 47 项全过（维护者本机的 Playwright 脚本）。
- 最后一轮实测（测试版都藏着窗口，本机测试网站）：
  - 收到 SIGTERM：直链下载中 0.13 秒退出，aria2c 跟着结束、`control.json` 删了；录本机直播（M3U8 引擎）、yt-dlp 起 ffmpeg 录直播时发 SIGTERM，整棵进程树都结束，下次打开把录到的部分整理好进了素材库。
  - 直播不算「下载过了」：插件在本机直播上录一次以后，按钮、清晰度菜单、工具栏面板、复制链接的提示都不说「下载过了」；同一套测试里普通视频下过以后四处照旧说「下载过了」。
  - 命令行两次 Ctrl-C：M3U8 引擎和 yt-dlp 两条路都打印「原始数据留在 …，7 天后会自动删掉」；时间改到 8 天前以后启动时删了；queue.json 里还等着整理的那份 8 天前的也没删，桌面端打开后整理好了。
  - 旧数据升级：用 9 月的旧版命令行造数据（3 个文件分在影片、音乐、自选文件夹，标签，改过的设置，一个下到一半的任务），新版打开：素材、标签、设置都在，缩略图和文件都能显示，没下完的任务接着下完。
    新版写过以后旧版命令行照样能读、能加标签、能下载；但旧版命令行一下载就会把 `queue.json` 换成它自己的任务（新版暂停着的任务从列表里没了，下到一半的文件还在）。
    旧版在设置里改任何一项会按旧的字段重写 `settings.json`，新版才有的几项（自动更新、代理、读浏览器登录状态）回到默认（读旧版代码得出，旧版桌面端和正式版标识相同，没运行）。
  - 插件回归（Chrome for Testing 无界面 + 测试版）：带登录状态 10/10、录直播 11/11、下载 6/8、面板 9/10；没过的三项都是「窗口从隐藏变成显示」——测试版故意不显示窗口，按接口核对过跳到哪一页、选中哪一条。
- 几百个网站实测（第一轮 682 条链接、251 个网站，一条海外的移动网络）：「公开、无 DRM、Captae 声称支持」的 204 条，第一次就成功 157 条（77.0%）；
  修完 19 类问题后按同样算法估算 165 条（80.9%），失败里「未知」从 20.1% 降到 0.2%，YouTube 公开链接 19/19（修完只跑了一轮）。
  还失败的 39 条：附带的 yt-dlp 跟不上网站改版 21 条（界面引导用浏览器插件）、要人机验证 9 条、这个网络和网站当时的状态 9 条（限流、地区限制、海外限速）；Captae 自己的问题在这 204 条里没有剩下的。
  离计划的验收标准（第一次就成功 ≥ 95%、YouTube 连测 3 轮 ≥ 98%）还差，见下面「等决定的事」。
- 多语言（演示构建，无界面 Chrome）：中文界面和改之前逐页对比（38 个页面和状态），除了新加的「通用 › 语言」一行、插件面板「拖进 Applications」那句，一字不差；
  8 种语言都没有页面报错，比中文多出来的截断逐个修了（剩下的是读屏用的隐藏文字和本来就省略的长网址）。换语言时设置弹窗不重播进场动画、焦点留在语言那一栏、倒计时圆环按剩下的时间接着走（逐帧核对过）。
  插件网页部件在假 chrome 接口上中英文各走了 33 个状态；命令行用 `CAPTAE_LANG=en` 和中文各跑过一遍。
- 插件：计划里的 11 项在 Chrome for Testing + 测试宿主上逐项实测通过，集成那一轮又回归了 9 轮（B 站开着登录状态真的下完 480p）；后台逻辑另有用假 chrome 接口跑真代码的测试。
- 登录窗口：在开发版真窗口里测过 B 站、X、YouTube、本机测试网站（用假的登录 cookie，没有登录任何真账号）：判断登录成功、关窗、重试、退出、清除、网站那块调不了 Captae 的命令、拦截规则。
- 录直播：本机的直播 HLS、yt-dlp 能认的直播流、电台流三条路都录过、停止并保存过，YouTube 真直播录过一次。

## 没做完、做不了的

- **真窗口里的检查没做完**：最后几轮测试时屏幕一直锁着，真窗口截图没拍成。磨砂玻璃在 WKWebView 里的样子（菜单、右键菜单、小弹层、多选工具条，底下有内容在动时）、
  状态互换 4px 模糊在 120Hz 屏上的观感、素材库用真文件时缩略图淡入、提示条浮在设置弹窗上，要在真窗口里看。无界面浏览器（Playwright 的 WebKit）的截图不画 `backdrop-filter`，代替不了。
- **给真人测试版之前要请人做的**：在登录窗口里登录 YouTube、B 站、X 各一次，下一个要登录的视频（看 Google 在密码那一步拦不拦内嵌窗口）；设置里打开「也读取浏览器里的登录状态」试 Chrome、Safari；
  在自己的浏览器里装插件走一遍。出测试版前想好怎么不碰正在用的 Captae 的数据（备份数据目录，或者用单独的标识）。
- **网站实测**：第二轮全量回归和 YouTube 连测 3 轮还没跑（等定在什么网络下测）。
- **多语言没在真窗口里看过**：换语言在 WKWebView 里的样子、macOS 菜单栏跟着换（只有单元测试）、`CFBundleLocalizations` 让文件选择窗口用对语言，要装上新版才看得到。
- 多语言的小缺口：后端已经生成的文字（下载记录、错误说法、任务标题、通知）按生成那一刻的语言，换语言以后旧的不变；换语言时正显示的提示条会消失（界面整个重新挂一次）；
  macOS 菜单栏里系统自己加的几项（开始听写、表情与符号）跟系统给 Captae 选的语言，在 Captae 里另选语言时不跟着换。手机端（`apps/ios`、`apps/android`）的界面文字不在这次范围里。
- 自己从源码打、没有 Developer ID 证书的包没有公证：第一次打开要到「系统设置 › 隐私与安全性」点「仍要打开」。正式发布的包签名、公证见上面「打包」。
- macOS 只打了 Apple Silicon 的包，Intel Mac 没做。Windows、Linux 见上面「Windows / Linux 版」。Firefox 不做。
- 附带的 aria2c 来自 Homebrew，`scripts/fetch-sidecars.sh` 需要本机先 `brew install aria2`。ffmpeg 来自 ffmpeg.martin-riedl.de（带 libass；Homebrew 的 ffmpeg 没有 libass）。
- GPL 源码（2026-11-26 定）：每个版本的 Release 页附上 FFmpeg、aria2、yt-dlp 对应版本的源码包，`licenses/README.md` 里「三年内可索取」的书面承诺也保留，兜底 FFmpeg 静态链接的各个库。
- 打赏入口没加。CI 在 `.github/workflows/`：`ci.yml`（核心库测试、界面类型检查、词表、插件面板产物）、`release-winlinux.yml`（手动运行，打 Windows / Linux 包并挂到 Release）、`cla.yml`（贡献者协议）。
- 直播还没开始时（`live_upcoming`）只给出开始时间，没有「开始后自动下载」。
- 微信视频号暂不支持（yt-dlp 没有，视频还有加密）；图片帖（小红书图文、抖音图集、Flickr……）桌面端不下，提示用插件在网页里下。Archive.org 借阅制条目、X 敏感内容推文报「没有视频」（分不清）。
- 插件里「电脑上还没有 Captae」的卡片和面板有「下载 Captae」按钮，打开 GitHub 上最新的 Release。插件没上架商店（Chrome 网上应用店不允许能下载 YouTube 的插件）。
- 系统开了 PAC 自动代理时按直连处理（三个系统都是）。
- 直链下载在服务器不回 Content-Length 时，队列里的大小先显示 0，下起来后才更新。素材库「移到废纸篓」是 6 秒后才真的移（这 6 秒里退出 Captae，文件不动）。
- AI 助手看不到 cookie，但它发起的下载交给桌面端时，和其他任务一样会用 Captae 里登录过的网站的登录状态（设置和上手引导里也是这么写的）。
- 录直播的原始数据保留 7 天：AI 助手在桌面端没开时让 MCP 自己录、录到一半 MCP 进程被关掉，yt-dlp 的中间目录（保存位置下的 `.captae-tmp/…`）没有人记下，不会被自动删（M3U8 引擎的在数据目录里，按名字照样会删）。
  第三轮测试版录的直播在素材库里没有 `live` 标记，照旧算「下载过了」（9 月版不能录直播，用户的数据里没有这种）。
- 命令行把下载交给桌面端、桌面端中途退出时，命令行报的是网络库的原文（`error sending request for url …`），还没换成一句中文说法。
- 旧界面存在 WebKit 本地存储里的小偏好（上手引导看过了、上手清单、侧栏收起），新界面第一次打开时会搬过来（`lib/prefs.ts`）。键名对过：9 月的 0.1.0（提交 ba4b70e）存的是
  `captae2:obSeen`、`captae2:ob`、`captae2:sideOff`，和迁移的一致；真的搬没搬过来要装上新版才看得到（测试版的标识不同、存储也不同）。

## 等决定的事

- 附带的 yt-dlp 已经读不了的网站算不算进「声称支持」的分母（不算的话维护一个「已知读不了」的名单，读取前直接引导用插件）。
  2026-11-26 定：第一次就成功 95% 不拦公开发布，作为以后的目标。
- 图片帖要不要加下载引擎支持（gallery-dl 是 GPL，要单独核对能不能随包分发），还是一直靠插件。
- 下一轮网站回归在什么网络下测（这一轮用的海外移动网络，Vimeo 等被运营商劫持、不少网站有地区限制）。
- 安全扫描：2026-10 那次扫的是 9 月版代码（ba4b70e），现在的代码还没重扫（维护者说以后再跑）。

## 公开发布（2026-11-26 定）

- 原来的私有仓库改名 `sol1560/Captae-archive` 原样保留（含 `devin/*`、`wip/*` 分支和两个 PR）；公开的 `sol1560/Captae` 只有一个干净的首个提交（旧提交的 LICENSE 是 MIT，不公开）。
- 三个平台一起发：macOS 本机打包、签名、公证；Windows、Linux 用 `release-winlinux.yml` 在 GitHub 的机器上打，检查脚本过了才挂到 Release，安装包不签名（README 写明）。
- 翻译先发 AI 翻的版本，README 写明欢迎纠正。
- 用真账号在登录窗口里登录 YouTube、B 站、X 只能维护者自己做，还没做（见上面「给真人测试版之前要请人做的」）。

## 许可证和商业模式（已定）

- Captae 自己的代码用 AGPL-3.0-or-later，见 `LICENSE`（2026-11-26 从 MIT 改过来）。路线图：GitHub 上达到 5000 个 star 开始逐步切换到 MIT，达到 1 万个开始切换到 Unlicense；
  这是长期投入，要重写很多底层的东西（从 GPL / AGPL 项目拿来的只能重写；外部贡献者提 PR 前要先同意 `CONTRIBUTING.md` 里的贡献者协议，允许以后改用更宽松的开源许可证），做法见 `docs/LICENSING.md`「许可证路线图」。界面只用开源的部分：affectUI 免费组件（MIT）、HeroUI 开源版（用到时再加）、Sonner、React Bits 和 BoardUI 的免费部分；付费库只参考，代码不进仓库。
  课程（《Animations on the Web》）只用数值，练习和演示代码不进仓库。
- yt-dlp、ffmpeg、aria2 都只作为子进程调用，GPL 不会传染到 Captae 自己的代码上；安装包里附带它们的二进制，所以附上许可证原文和源码获取方式（`licenses/`、`THIRD_PARTY_NOTICES.md`）。
- 插件没有复制猫抓（cat-catch，GPL-3.0）的代码，以后也不拷（拷进来那一块就换不成 MIT）。cobalt 的代码和名字都没有用。
- 2026-09-26 定的：开源，完全免费，只挂打赏（Buy Me a Coffee 之类）。依据和引用在 `docs/LICENSING.md`。
