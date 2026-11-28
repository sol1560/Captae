# Captae 设计说明

Captae 是 macOS 上的本地媒体获取和管理工具。它分成四块：桌面端负责下载、转换、整理；浏览器插件负责在网页上发现媒体、一键交给 Captae；
命令行 `captae` 同时是浏览器插件的宿主和 AI 助手用的 MCP 服务器；`skills/captae/SKILL.md` 是写给 AI 助手看的说明。
四块共用同一个 Rust 核心库，所以界面、插件、命令行、AI 助手做同一件事时走的是同一段代码。

界面的视觉规范在 `docs/design/README.md`，动效规范在 `docs/MOTION.md`，多语言的写法、词表和术语在 `docs/I18N.md`，许可证在 `docs/LICENSING.md` 和 `THIRD_PARTY_NOTICES.md`。

## 1. 目录

```
crates/captae-core        核心库：下载引擎、队列、素材库、转换、字幕、预设、登录状态、发给 AI、MCP、本地控制接口、插件宿主
crates/captae-cli         命令行 captae：MCP stdio、Native Messaging 宿主、probe / download / update / engine / status 等
apps/desktop/src-tauri    桌面端的 Rust 外壳（Tauri 2）：窗口、登录窗口、各页面用的命令、Captae 自己的更新（第 15 节）
apps/desktop/src          桌面端界面（React 19 + Vite），也是插件工具栏面板的源码（extension-popup/）
extension                 Chrome 插件（Manifest V3）：网页里的部件是原生 JS + CSS，工具栏面板是 popup/ 里的构建产物
skills/captae             给 AI 助手的 Skill
scripts                   打包和维护脚本：build-release.sh、fetch-sidecars.sh、check-bundle.sh、gen-licenses.py，以及语音识别脚本 captae_whisper.py
licenses                  附带程序和第三方组件的许可证、源码地址（打进安装包）
docs                      设计、动效、多语言、许可证说明；docs/design 是界面规范
screenshots               新界面的演示版截图
```

## 2. 技术选择

| 部分 | 选择 | 理由 |
|---|---|---|
| 桌面外壳 | Tauri 2 | 安装包小，常驻内存比 Electron 小一个数量级。下载工具要长期挂在后台，这点很重要 |
| 核心逻辑 | Rust | 下载队列、子进程管理、SQLite 都在一个进程里，没有 Node 或 Python 运行时要打包。命令行和 MCP 服务器复用同一个库，行为和界面完全一致 |
| 界面 | React 19 + Vite + TypeScript；组件用 affectUI 的免费组件（MIT），提示条 Sonner，图标 Hugeicons Free，动效 Motion | affectUI 每个组件是一个独立的 .tsx 文件、样式写在文件里，不依赖 Tailwind 和动画库；源码原样放进仓库，颜色和曲线通过它的 CSS 变量统一。付费库只参考，代码不进仓库 |
| 下载 | yt-dlp（附带 deno 解 YouTube 的 JS 验证）、aria2、自写的 M3U8 引擎 | yt-dlp 覆盖上千个网站；aria2 多线程下直链；M3U8 自己写是因为要分片进度格、清晰度倒计时、失败分片单独重试、录直播只在两段之间停，这些 yt-dlp 和 ffmpeg 都给不出来 |
| 转换 | ffmpeg / ffprobe | 事实标准，能只换封装时就不重新编码 |
| 字幕识别 | faster-whisper（Python 子进程） | 速度是原版 Whisper 的几倍，CPU 上也能跑；MIT。用户点「安装」时才装进数据目录里单独的 Python 环境 |
| 素材库 | SQLite + FTS5（trigram 分词） | 单文件，不用装数据库。trigram 对中文不需要分词器，标题、简介、标签、字幕文字一起搜；少于 3 个字时退回 LIKE |
| MCP | 自己实现的 stdio JSON-RPC | 协议面很小（initialize、tools/list、tools/call），自己写比引入 SDK 少依赖；支持 2025-06-18、2025-03-26、2024-11-05 三个版本 |
| 多语言 | 三处各一个小模块（Rust `i18n.rs`、界面 `src/i18n/`、插件 `lib/i18n.js`），代码里写中文原文、按原文查译文 | 三处共用一套原文和词表格式，自己写的几百行够用；不给插件注入网页的部件多加依赖。见第 13 节和 `docs/I18N.md` |
| 插件 | 网页部件原生 JS + CSS（放在 Shadow DOM 里）；工具栏面板 React + affectUI，构建产物提交进仓库 | 网页部件会注入用户打开的每一个网页，不给每个网页多加一个 React；面板和桌面端共用组件和样式，插件文件夹拿来就能加载，不用再构建 |

外部工具一律作为独立程序通过子进程调用，Captae 不链接它们的代码。许可证上的意义见 `LICENSING.md`。

## 3. 进程和通信

```
浏览器插件 ──Native Messaging──> captae native-host ──HTTP 127.0.0.1──> 桌面端（Core）
AI 助手   ──MCP stdio────────> captae mcp --stdio ──HTTP 127.0.0.1──> 桌面端（Core）
                                        └── 桌面端没开时，在自己进程里起一个 Core
```

桌面端启动时在随机端口监听 127.0.0.1，把端口和随机 token 写进 `<数据目录>/control.json`（权限 0600）。命令行、MCP 进程、插件宿主读这个文件找到桌面端，请求都带 token。
这样 AI 助手发起的下载会出现在桌面端的下载列表里，用户看得到，也能暂停和取消；桌面端没开时 MCP 照样能用。只有主窗口关掉（退出）时才删 `control.json`。

自己起的 Core（命令行、桌面端没开时的 MCP）不恢复也不写 `queue.json`：不然它结束时会用自己的几个任务把桌面端保存的未完成任务覆盖掉。
代价是桌面端关着时用命令行或 AI 助手下的东西，不会出现在之后打开的桌面端下载列表里（文件照样进素材库，AI 调用记录也在）。

Native Messaging 宿主名是 `app.captae.bridge`，消息是 4 字节长度加 JSON，收发逻辑在 `captae-core/src/native_host.rs`。插件发的消息类型：
`hello`、`sniffed`、`download`、`share`、`open`（可带目的地：某条素材、某个任务、粘贴框）、`jobs`、`task`、`job`（暂停 / 继续 / 取消）、`check`（这个链接下载过没有）、
`probe`（标题、时长、封面、各清晰度大小）、`task-cancel`（取消「发给 AI」）、`recopy`（再复制一次，含关键帧）。宿主每条消息单独处理，「发给 AI」这种要跑一会儿的请求不会挡住进度查询。
失败的回复除了一句话的 `error`，还带错误码 `code` 和完整的 `fail`（见 4.2）。

桌面端一时忙（几个下载同时收尾、入库截图）时不要误报「Captae 关掉了」：宿主发现 `control.json` 里的桌面端进程还活着、只是 800 毫秒没回话时，再等 2 秒、4 秒各问一次（`Client::connect_patient`），
真没开（没有 `control.json`、进程不在）马上返回；插件交给 Captae 的请求最多等 120 秒。网页卡片连着 3 次、超过 3 秒连不上才说「Captae 关掉了」，之后放慢到 3 秒一次接着问、连上了自己接上，
Captae 关掉又打开（任务编号变了）时说「Captae 重新打开了」；面板没交出去时那一行写清楚原因（「Captae 一直没回话，可能已经收下了」），不悄悄回到「可以下载」。轮询只更新这一轮问到的任务。

宿主由浏览器启动（包装脚本 `exec` captae），启动时看父进程是哪个 `.app`（`getppid` + `proc_pidpath`，往上最多找 4 层，只跳过 shell、env 这类启动器），
认出是 16 家浏览器里的哪一家，`hello` 里带给桌面端，插件页和上手引导显示得准（Arc、Aside 不会被说成 Chrome）。
16 家：Chrome、Chrome Beta / Dev / Canary、Chrome for Testing、Chromium、Edge、Edge Beta / Dev / Canary、Brave、Vivaldi、Opera、Opera GX、Arc、Aside。

桌面端每次启动都会检查宿主清单，装了的浏览器（用户数据目录里有 `Local State`，或者 `/Applications`、`~/Applications` 里有这个 .app）才写，缺了或者指向的 `captae` 不对就重新写；
Opera 另外写 Chrome 的目录，Chrome for Testing 另外写 Chromium 的目录，Arc 写 `Arc/User Data/NativeMessagingHosts`。也可以手动运行 `captae install-native-host`。不支持 Firefox。

## 4. 下载

### 4.1 流程

每个任务的流程固定是：读取信息 → 选清晰度（只有 M3U8）→ 下载 → 格式后处理 → 入库前核对 → 放进素材库 → 预设里的后续步骤。

- **链接识别和改写**（`detect.rs`）：决定走 yt-dlp、aria2 还是 M3U8 引擎，规则和前端 `lib/detect.ts` 一致（前端只拿它做即时提示）。
  读取前先把几种链接改成真正能下的：B 站带语言前缀的地址（`/zh-Hans/video/…`）去掉前缀、Dropbox 分享页 `?dl=0` 改成 `?dl=1`、Wikimedia Commons 文件页改成直接给文件的地址、
  YouTube `/redirect?q=`、lnkd.in 这类跳转包装展开。
- **yt-dlp**：先用 `-J` 取信息（标题、时长、封面、可选格式），信息存成文件，下载时用 `--load-info-json` 免得再解析一次网页（存之前去掉里面带的 cookie 值）。
  进度用 `--progress-template` 输出成固定格式，最终路径用 `--print after_move:` 拿。每个任务的中间文件在保存位置下的 `.captae-tmp/<链接算出的 12 位>/`，暂停后继续还是同一个目录。
  读不到公开的 YouTube 视频时（先用 oEmbed 确认是公开的），换一组 player_client（`tv_simply,web_embedded`）自动再试一次；下载时偶发的 403 也一样。
- **播放列表和多视频帖子**：读到列表时用 `--flat-playlist` 拿条目，任务停在「等你选」（`choose: true` + `entries`），用户选好后每个条目一个任务（`choose_entries` 命令）。
  YouTube `watch?v=…&list=…` 默认只下这一个，详情里可以改成下整个列表；B 站分 P 视频也让用户选，`?p=` 的链接显示「第 2 P，共 5 P」。
- **aria2**：先发 HEAD 拿文件名和大小，再多线程下载；每秒存一次进度（`--auto-save-interval=1`），暂停（结束进程）后能接着下。多路被网站拒绝（403 / 429）时换成一路接着下。
- **M3U8 引擎**：解析主列表和分段列表，8 路并发下分片，每片最多试 3 次，支持 AES-128 解密和带字节范围的初始化分片，拼接后用 ffmpeg `-c copy` 封装成 MP4。
  临时分片按分段列表地址的哈希命名，关掉 Captae 再打开，已经下好的分片会接着用。主列表有多个清晰度时进入「选清晰度」，默认 6 秒后自动选最高。认得出 Widevine 加密的流（报 DRM）。
- **队列**：设置里的「同时下载」控制后台任务。用户在界面上亲手开始的任务（粘贴链接、插件里点下载、点「马上开始」）不排队，临时多占一个名额；AI 助手、命令行、嗅探加进来的按顺序排。
  暂停、继续、上移、下移、取消都在 Core 里。未完成和最近完成的任务存在 `queue.json`（不存 cookie），重开后恢复：读取中的自动重读，下载中的变成暂停，等选条目的原样放回。

### 4.2 错误码（`errors.rs`）

yt-dlp、aria2、ffmpeg、HTTP 状态码、io、网络库的报错都先归类，任务里同时存原文（去掉终端颜色控制符，界面上能看、能复制）。失败详情 `fail` 的结构：
`code`、`message`（给人看的一句话，跟着界面语言）、`raw`（原文最后 12 行）、`action`（界面上的下一步按钮）、`next`（给 AI 看的下一步，只有英文）、`reason`（同一个错误码下的细分：`image_post` 图片帖、`radio` 电台流）、`brief`（去掉「可以用浏览器插件在网页里下」的短说法，插件里显示它）、`startsAt`（直播开始时间，只有 `live_upcoming` 有）、
`site`（说法里用的网站名字，「这个视频要登录芒果TV 才能看」和按钮「登录芒果TV」用同一个；认不出的网站不带，按钮退回主机名）。B 站在说法里也叫「哔哩哔哩」。

| code | 什么情况 | action |
|---|---|---|
| `need_login` | 年龄限制、会员、私密、付费内容只给试看片段 | `login` 登录这个网站 |
| `need_verify` | 机器人检测、Cloudflare 一类的验证、抖音要新 cookies | `verify` 在网页里确认 |
| `drm` | DRM 保护（含 YouTube 付费电影、Widevine 加密的 M3U8） | 无 |
| `geo` | 地区限制 | `retry` |
| `blocked` | 证书不对、被网络拦截、网站拒绝这个网络（418） | `retry` |
| `unavailable` | 已删除、不存在、分享链接失效、直播没留录像 | 无 |
| `unsupported` | 下载引擎没有这个网站、或者跟不上网站改版；图片帖 | `use_extension` 用浏览器插件在网页里下 |
| `no_media` | 网页里没有视频或音频 | 无 |
| `live_upcoming` / `live` | 直播还没开始 / 正在直播（含电台流） | `wait_live` / `record_live` 从现在开始录 |
| `rate_limited` | 429、请求太频繁 | `auto_retry`：15 分钟后、45 分钟后各自动重试一次 |
| `network` | 超时、断线、DNS | `retry` |
| `disk` | 磁盘满、没有写入权限 | `change_dir` |
| `tool` | 组件缺失或损坏 | `repair` |
| `unknown` | 其他 | `copy_error` |

插件、剪贴板提示、AI 助手（MCP）、命令行（`--json`）拿到的是同一套错误码。

### 4.3 附带的程序和自动更新

- **deno**：安装包附带官方的 deno（`Contents/MacOS/deno`），所有 yt-dlp 子进程都加 `--no-js-runtimes --js-runtimes deno:<路径>`。
  原因：从访达打开时 PATH 里只有系统目录，yt-dlp 找不到 JS 运行时，解不了 YouTube 的验证。找运行时的顺序：设置里指定的 → 安装包自带的 → PATH / Homebrew 的 deno、node、bun。
- **yt-dlp 用官方目录版**（`Contents/Resources/yt-dlp/`）：单文件版每次启动都把自己解压成新文件，macOS 每次都要重新做安全检查；目录版只在第一次。
  桌面端启动时在后台把各个外部工具先跑一次（预热），用户第一次下载不用等安全检查。yt-dlp 的查找顺序：设置里指定的 → 自动更新装的 → 安装包自带的 → PATH。
- **自动更新**（`updater.rs`，设置里可以关；这是 yt-dlp 的，Captae 自己的更新见第 15 节）：每天查一次 GitHub 官方 release（走 Captae 的代理设置）。有新版时：用编译进程序的 yt-dlp 官方公钥（钉死指纹）验证 `SHA2-256SUMS.sig` →
  核对 `yt-dlp_macos.zip` 的 SHA256 和签过名的清单一致 → 版本号只许数字和点 → 解压到 `<数据目录>/engines/yt-dlp/<版本>/` → 检查代码签名（没有就加本地签名）→
  自检（`--version` 对得上，再读一个固定的公开视频）→ 通过了才切换，保留上一个版本。新版在使用中出「组件 / 未知」错误时当场体检，确认是新版的问题就退回并记下坏版本（每日检查不再装它）。
  任何一步失败都只记进 `engines/yt-dlp/state.json`，继续用原来的版本。设置 › 关于里能看版本、上次检查时间，能手动「检查更新」。命令行 `captae update` / `captae engine` 也行。
- **根证书**：附带的 ffmpeg（静态构建，OpenSSL）找不到系统根证书，启动 ffmpeg、ffprobe、yt-dlp、aria2c 时设 `SSL_CERT_FILE=/etc/ssl/cert.pem`（macOS 自带的根证书包），照样校验证书。
- **aria2c 从 Homebrew 拷来**（没有官方 arm64 构建）：`fetch-sidecars.sh` 把它依赖的动态库放进 `Contents/Resources/aria2-libs/`、改写加载路径。
  它带的 libcrypto 还把两样东西写死在打包那台电脑的 Homebrew 目录里，没装 Homebrew（或者 Homebrew 的 OpenSSL 升级过）的 Mac 上都会出问题，2026-11-24 挡住 Homebrew 目录实测才查出来：
  - OpenSSL 的 legacy 模块：aria2 1.37 一启动就加载它，加载不了直接退出（`OSSL_PROVIDER_load 'legacy' failed`），直链下载、录电台全都用不了。
    模块随包带着（`aria2-libs/ossl-modules/legacy.dylib`，和带走的 libcrypto 同一个版本），启动安装包里的 aria2c 时用 `OPENSSL_MODULES` 指过去
    （`tools.rs` 的 `aria2_modules`；yt-dlp 把下载交给 aria2c 时也在 yt-dlp 上设，aria2c 继承）。Developer ID 签名的包开着库校验，也不许加载 Homebrew 签的模块，同一个团队签的随包模块可以。
  - 根证书：aria2 用 OpenSSL 校验 https，默认读 Homebrew 的 `cert.pem`，读不到就「unable to get local issuer certificate」。上一条的 `SSL_CERT_FILE` 也给 aria2c 设上。
- **aria2 的 User-Agent**：请求头里没有 User-Agent 时用和 Captae 自己的请求一样的 `net::UA`（像 Chrome）。读取信息（`aria2::head`）用的就是它，
  aria2 自己的 UA 有的网站直接 403，结果读取信息成功、下载却被拒（w3schools 的样片就是这样）。带了登录状态时用登录那边的 User-Agent。

### 4.4 代理（`net.rs`）

设置里的代理模式：跟随系统（默认）/ 不用代理 / 手动填（`127.0.0.1:7890`、`http://…`、`socks5://…`）。跟随系统时自己读 `scutil --proxy`（缓存 30 秒），算出实际要用的代理，
Captae 自己的 HTTP 请求、aria2c（`--all-proxy`）、ffmpeg 和 yt-dlp（`http_proxy` 等环境变量 / `--proxy`）共用一份，认「忽略这些主机」。
系统开了 PAC 自动代理时按直连处理；aria2 不支持 socks，手动填 socks5 时直链下载不走代理（`proxy_status` 会说）。代理地址不进日志。改了马上生效。

### 4.5 入库前核对和试看片段

- **入库前核对**：下完先用 ffprobe 核对：读不了、视频和图片没有宽高、声音没有音轨 → 报 `no_media`「下载到的文件不是视频或音频（可能是网页），没有保存」，删掉文件、不入库
  （以前 Giphy、Dropbox 分享页会把网页当成视频存下来）。yt-dlp 选中的是单张图片时照原样存进「图片」文件夹。直链先发 HEAD，回来是网页的改走 yt-dlp；
  不是媒体的直链（PDF、压缩包、JSON……）读文件开头 64 字节认出来，直接说清楚。ffprobe 不在时不拦，照旧入库，不会因为工具坏了删用户的文件。
- **试看片段**：会员 / 付费内容没登录时只给试看片段（芒果 TV、网易云 VIP、喜马拉雅付费书……）。读取时认得出的直接报 `need_login`（「完整版要登录开了会员的账号」），不下载；
  下完才发现的（只对 yt-dlp、跳过直播：网页说的时长 ≥ 60 秒，文件不到 60%、而且短了 30 秒以上）也算失败，试看片段不入库。

### 4.6 直播

读取时认出正在直播（含没有 `live_status` 的直播流和电台流）报 `live`，用户点「从现在开始录」才录。录制中不给暂停，点「停止并保存」把录到的部分整理成能播放的文件放进素材库：
- yt-dlp：录直播交给 ffmpeg，停的时候只给 yt-dlp 发 SIGINT，让它自己收尾（ffmpeg 写完文件、yt-dlp 照常整理）；120 秒没收好尾就结束进程树，Captae 自己用 `ffmpeg -c copy` 整理中间目录里录到的文件。
  画面和声音分两路时 yt-dlp 不做「TS 转 MP4」，录完看一下容器，是 TS 就整理成同名的 MP4。
- M3U8 引擎：每隔一段时长重读列表，新分段接在后面；只在两段之间停，写进文件的都是完整的分段。
- aria2（电台流）：一个连接、不用磁盘缓存，停的时候发 SIGINT，整理成带文件头、时长准确的 `.mp3` / `.m4a`。
- yt-dlp（ffmpeg）和 M3U8 引擎都从列表里最新的 3 段接起，文件比界面上的「已录」多两三段。录到东西以后网络断了，录到的照样整理入库；录制中 Captae 被退出，下次打开时整理好、提示一次。
- 「取消」会删掉录到的东西（要确认）。
- **三处都能「停止并保存」**：桌面端下载页；插件网页上的卡片（录制中写「正在录制 · 已录 12:34」，主按钮「停止并保存」，「取消录制」先在卡片里问一句）；工具栏面板（那一行同样，按钮在第二行）。
  从网页卡片开始录的直播（直播的 M3U8 本来不在嗅探列表里），面板里会补上一行。卡片上点「从现在开始录」时，先把 Captae 里那条「这是正在进行的直播」失败任务去掉（和桌面端一样，不留两条）。
- **最多录多久**：`AddRequest.record_max_secs`，录满了自动「停止并保存」（调度器每秒看一次），任务记录写「录满设定的 10 分钟，自动停止」，桌面端录制行的小字写「录满 10 分钟自动停止」（没有对应的设置）。
  AI 助手 `download` 的 `max_minutes`：不填按 10 分钟，最多 180 分钟（超过报 `BAD_ARGS`），结果里多一个 `recording: {seconds, max_seconds, note}`。命令行 `--max-minutes N` 不套用这两个数，不写就一直录。
- **命令行两段式 Ctrl-C**（`download --record-live`）：录制中第一次 Ctrl-C =「停止并保存」（本进程的 Core 直接停；交给了桌面端的，找到这次新加的、来自命令行、正在录的那条发 stop），保存完打印文件在哪、正常退出；
  第二次直接退出：结束外部程序，没整理的原始数据留在原处，打印在哪、说明 7 天后自动删。录制中被 SIGTERM、SIGHUP 结束也一样（留下、记下、说明）。
- **没整理的原始数据保留 7 天**（`recordings.rs`）：桌面端、命令行、桌面端没开时 MCP 自己起的 Core，启动时都清一次，删「最后一次写入」在 7 天以前的：`<数据目录>/work/hls-rec-*`，
  以及命令行退出时记进 `<数据目录>/work/recordings-left.json` 的 yt-dlp 中间目录（保存位置下的 `.captae-tmp/<…>`，保存位置可以是任何文件夹，所以要记下来）。
  不删：queue.json 里记着的（桌面端等下次打开整理的录制，不管多旧）；queue.json 读不懂时（可能正写到一半）这次什么都不删；电台流（aria2 直接写在保存位置、名字就是最后的文件名、本身能播放）不删也不记。
- **直播不算「下载过了」**：录下来的素材在素材库里标成直播录像（`assets.live = 1`，从它转换、剪辑出来的跟着），按链接查重（`Library::find_by_url`）时跳过：
  直播频道的链接每次录到的是不同的内容。插件按钮、清晰度菜单、工具栏面板、「发给 AI」找现成素材都走这一处；普通视频照旧。

### 4.7 结束进程树、取消和半成品

- 所有外部程序都经 `tools::spawn` / `tools::output` 启动，登记在一张表里（`tools::Proc`）。超时、暂停、取消时结束整棵进程树（`kill_tree`：先逐个 SIGSTOP 再 SIGKILL，
  跳过自己和 1 号进程，只在子进程还没被回收时动手，不会误杀重用的进程号），yt-dlp 启动的 ffmpeg 不会留在后台。桌面端退出（包括 ⌘Q，以及收到 SIGTERM、SIGHUP、SIGINT）时 `kill_all` 结束全部；
  命令行收到这几个信号时也一样。
- 后台任务（转换、剪辑、拼接、字幕、预设、发给 AI）可以取消（`task_cancel`）。写了一半的输出文件挂在 `tasks::Partial` 上，取消、出错、退出时删掉，做完才留下；
  数据目录 `work/` 里的中间文件（`subs-`、`share-subs-`、`whisper-`、`burn-`、`preset-` 开头的）做完、出错、取消都删，开始任务前再清一遍一天以前的残留。
- 取消下载任务时删掉下到一半的文件和中间目录；已经进了素材库的文件（任务后面还在跑预设步骤时）不删。下载的断点续传文件、录直播的原始数据在退出时不删，下次接着用。

## 5. 登录和人机验证

原则：用户自己登录、自己点验证，Captae 只是把登录状态交给下载引擎。不自动破解验证码，不绕过 DRM。登录状态有三个来源，按这个顺序用：

1. **浏览器插件带来的**：插件发下载、读取信息、发给 AI 时带上这个网站（标签页的主域名 + 媒体地址的主机名）的 cookies 和浏览器的 User-Agent。
   工具栏面板有「带上这个网站的登录状态」开关，按网站分别记，默认开。只给这一次用，不写进 `queue.json`、不进任务记录和错误原文、不给 AI 助手。
2. **Captae 里登录过的网站**：登录窗口里登录过 / 验证过的网站，读登录窗口自己的存储。
3. **读浏览器的登录状态**：设置里「也读取浏览器里的登录状态」，默认关（读 Chrome 会弹钥匙串授权，读 Safari 要「完全磁盘访问」）。用 yt-dlp `--cookies-from-browser`，
   只有 yt-dlp 会用（直链、M3U8 用不上）。可选 Chrome、Safari、Edge、Brave、Firefox、Vivaldi、Opera、Chromium；没有 Arc（yt-dlp 读不了 Arc 的 cookies）。

**YouTube 例外**（三个来源都一样）：第一次读取不带登录状态；报 `need_login` / `need_verify` 时，如果有可用的登录状态，当场带上再读一次（不用用户再点），这个任务之后的下载也带；
第一次用 YouTube 账号时提示一次（用账号大量下载，账号可能被 YouTube 限制）。

### 5.1 登录窗口（`apps/desktop/src-tauri/src/login.rs`）

- 任务报 `need_login` / `need_verify` 时任务行上有「登录 X」「在网页里确认」；设置里有「已登录的网站」（YouTube、哔哩哔哩、X、Instagram、抖音、TikTok 默认列出，可以登录、退出、清除，
  也能「登录其他网站」）；AI 助手页调用记录里要登录的失败也有「登录 X」。
- 单独的系统窗口（默认 1080×800，最小 560×520，系统标题栏「登录 YouTube」），同一时间只开一个，再要求登录别的网站时换内容。窗口里两块 webview：
  - `login-bar`：Captae 自己的界面（`index.html#/login-bar`，铺满窗口），上面一条工具条（锁 + 网站那块**当前的主机名** + 一句话 + 取消 / 完成），下面一行说明。
    让用户看得到自己在哪个网站输密码；没加密的网页换成提醒色和「别在这里输密码」。状态由 Rust 用 `eval` 推过去，不需要事件权限。
  - `login-site`：网站自己的登录页，盖在中间。单独的数据存储（`data_store_identifier`，macOS 14 起生效，更低的系统用默认存储），Safari 的 User-Agent；
    只许跳 http / https，新窗口在同一个窗口里打开。
- **命令白名单（默认拒绝）**：所有 Tauri 命令外面包一层：主窗口全部能调；工具条只能调 `login_bar_state`、`login_done`、`login_cancel`、`js_log`；其余一律拒绝。
  网站那块的来源是远程网址，又没有任何 capability，Tauri 本身也会拒绝它调用命令（实测 `invoke('boot')` 被拒）。
- **拦截规则**：网站那块挂一条 WebKit 内容拦截规则（WKContentRuleList），`asset:`、`ipc:`、`tauri:`、`file:` 和 `asset.localhost` 一类地址一律不许加载（挂不上就不打开网站），
  网站不能用 `<img src="asset://…">` 试探本机文件在不在。网站那块先用一个不存在的地址 `https://captae-login.invalid/` 创建、马上再跳到登录页，asset 协议的跨域头永远对不上真实网站。
- **登录存储**：不用 Tauri 的 cookie 接口（它会去掉域名前面的点，删不掉 `.youtube.com`、也拿不到子域名的 cookie），桌面端用 objc2 直接读写 `WKWebsiteDataStore(forIdentifier:)`。
  测试版（设了 `CAPTAE_HOME`）的存储标识由数据目录算出来，碰不到正式版的登录状态。`<数据目录>/logins.json` 只记登录过哪些网站和时间，没有 cookie。
- **判断登录成功**：每秒读一次，这个网站的登录 cookie（YouTube `SAPISID`、B 站 `SESSDATA`、X `auth_token`、Instagram `sessionid`、抖音 / TikTok `sid_tt`……）出现或变了 → 记下、重试等着的任务、关窗。
  打开前就登录着的不自动关，等用户点「完成」；人机验证没有固定的 cookie，也等「完成」。「取消」、红色关闭按钮 = 关窗，任务留在失败。
- **cookies 临时文件**：下载时把要用的 cookies 写成 Netscape 格式的临时文件（目录 700、文件建的时候就是 600），每个 yt-dlp / aria2 进程单独一份，进程结束就删；
  启动时删 10 分钟以前的残留。报错原文里的 cookie 值抹成 `***`。值里有制表符、换行的 cookie 跳过（yt-dlp 会把格式不对的整行打进报错）。
  登录窗口那条路给 yt-dlp 加 `--impersonate safari:macos`（这个 yt-dlp 支持时），插件那条路是 Chrome 一家的浏览器就只加 `--impersonate chrome:macos`、不另加 `--user-agent`（两个一起加 B 站会回空的格式列表）。
- Google 登录页在内嵌窗口里能打开、能输账号（用不存在的地址试过）；真账号在密码那一步会不会被拦，要等真人登录一次才知道。

## 6. 素材库、字幕、发给 AI

- 素材库表：`assets`（路径、标题、来源、时长、分辨率、封面、简介、文字稿、`live` 是不是录直播录的……）、`tags`、`assets_fts`（FTS5 trigram）、`agent_calls`（AI 助手调用记录）。同一个文件重复入库保留原来的 id。
  后来加的列（目前只有 `live`）在 `Library::migrate` 里补：第一版建的素材库打开时补上，新列都带默认值，退回旧版时旧版照样能读写（它写明列名插入、按列名读）。
  界面里能转换、剪辑（胶片条和波形 + 两个把手）、拼接、换音轨、生成字幕、打标签、在访达中显示、移到废纸篓（6 秒内可以撤销）。
- 字幕：先用 yt-dlp 取网站字幕（包括自动字幕，去掉 YouTube 自动字幕的重复行），没有再用 faster-whisper 本地识别。可以导出 SRT、VTT、TXT，可以烧录（附带的 ffmpeg 带 libass），
  可以翻译（走用户在设置里填的 OpenAI 兼容接口，本地 Ollama 也行，没填就不提供）。
- 发给 AI：一个视频变成 AI 能直接读的材料——720p 下载（已经在素材库里就直接用）、取文字稿（网站字幕优先，其次本地识别）、挑关键画面（每秒 4 帧扫一遍全片，按时长平均分段，每段取变化最大的那次换镜头稳定后的画面、没有换镜头取中间，避开黑场闪白；默认按时长 6–12 张，AI 助手最多可要 20 张，见 `ffmpeg::keyframes`），
  写出 `context.md`、`frames/`、`transcript.srt`、`meta.json`，再把文字、HTML 和图片文件放进剪贴板。插件里的「发给 AI」带浏览器的登录状态（同一个开关），AI 助手调 `share_video` 不带。

## 7. AI 助手（MCP）

工具：`download`、`convert`、`subtitle`、`trim`、`concat`、`library_query`、`library_add_tag`、`preset_run`、`preset_list`、`share_video`。
AI 访问关掉时 `tools/list` 返回空列表，单独关掉的工具不出现；设置变化时发 `notifications/tools/list_changed`。每个 MCP 进程把客户端名称和心跳写进 `<数据目录>/mcp_clients/`，
AI 助手页据此显示「已连接 · Claude Desktop · 2 分钟前用过」。工具名只用小写字母和下划线，界面上显示成 `captae.library.query` 这种样子。

- `download` 失败返回 `{"error": "DOWNLOAD_FAILED", "message", "code", "raw", "action", "next", "startsAt"?}`，AI 能据此告诉用户下一步。遇到列表、没给 `items` 时返回 `status: "choose"` 和 `entries`，
  AI 用同一个链接加 `items` 再调一次；`record_live: true` 从现在开始录直播。
- 基础模式下 AI 只能用这些参数，Captae 自己决定编码细节；高级模式打开后才接受 `ytdlp_args`、`aria2_args`、`ffmpeg_args`、`overwrite`。用户可以整体关掉 AI 访问，也可以单独关某个工具。
  每次调用（包括被拒绝的）都记录。AI 助手看不到任何 cookie；它发起的下载交给桌面端时，和其他任务一样按第 5 节的规则用登录状态。

## 8. 浏览器插件（`extension/`）

- 后台（`background.js`）只旁听网络请求的响应头（不拦截、不改写），按扩展名和 Content-Type 认出视频、声音、大于 120 KB 的图片和 M3U8，M3U8 会读列表拿清晰度。
  YouTube、B 站、抖音这类网站不列零碎的分段请求，整页交给 yt-dlp。每个标签页的发现存在 `storage.session`，工具栏图标上显示数量。网页本身打不开（证书错误、被拦截、4xx/5xx）时不算能下载。
- 网页部件（`content.js`、`lib/kit.js`、`lib/ui.css`，第二版设计）：鼠标停在视频上出现按钮组，清晰度菜单（分辨率、格式、大小、「推荐」、下载过了），右下角进度卡片（可以收起成小胶囊）、
  右键菜单。全部在封闭的 Shadow DOM 里、纯 CSS 动效，网页样式影响不到它。注入网页的字体叫 `Captae Geist`，不和网站自己的 Geist 混在一起。
- 网页里复制了视频链接：网页上什么都不弹（原来右下角会弹卡片问要不要下载，打扰人），后台判断是视频链接后经宿主（`copied`）记下什么链接、什么时候，
  1 分钟内打开 Captae 时由 Captae 来问（见 9.1）。
- 工具栏面板（源码 `apps/desktop/src/extension-popup/`，`npm run build:popup` 构建到 `extension/popup/`）：这一页找到的东西、每一条的进度、「下载过了」、全部下载，
  两个开关（在视频旁边显示按钮、带上这个网站的登录状态）。
- Captae 没打开或没装时，图标变灰加橙点，面板和卡片说清楚，并提供「打开 Captae」。插件 ID 由 manifest 里的 `key` 固定为 `docjnhebpmdchlahmhkjladmehlcfkcp`，和宿主清单里的一致。
- 录直播：卡片和面板里的「正在录制 · 已录」「停止并保存」「取消录制」见 4.6；和 Captae 之间连不上、一时没回话时怎么说见第 3 节。
- X 帖子做成图片（`lib/xcard.js`，2026-11-24 加）：鼠标停在 X 的帖子上，帖子右上角（X 自己的按钮左边）出「做成图片」，右键菜单里也有。
  数据取自 X 给网页嵌入帖子用的公开接口 `cdn.syndication.twimg.com/tweet-result`（不用登录，后台取，头像和配图也由后台取成 data URL）——X 网页 2026 年改版后没有稳定的标记可抓。
  卡片是一段内联样式的 HTML（样式都挂在 `.xc-root` 下，不会套到部件自己的元素上），放进 SVG `<foreignObject>` 按 3 倍画到 canvas，600 宽的卡片出约 2000 像素宽的 PNG。
  右下角卡片里预览、换浅色深色（默认跟网页）、复制图片、下载。超过 280 字的长帖接口只给前一段，从网页上的帖子补全（只要纯文字）。
- 文章存成 Markdown（`lib/article.js`、`lib/zip.js`）：公众号文章（`#js_content`）和 X 长文章（`.x-article-body`）停在正文上出「存成 Markdown」，别的网页从右键菜单存
  （取 `<article>`、`<main>` 里段落文字最多的一块）。读排好版的 DOM 转 Markdown（标题、加粗斜体、链接、列表、引用、代码块、表格、图片说明；藏起来的不要），文件头写 YAML（标题、作者、日期、来源）。
  有图片时后台把图片取下来（不带 cookie 和来源，微信的图片带了别的网站的来源会被换掉），和 Markdown 一起打成不压缩的 zip（`images/01.jpg`…）；没有图片就是一个 .md。
  完成卡片上还能「只下载 .md」（图片用网上的地址）和「复制 Markdown」。下载走网页上的 `<a download>`，存到浏览器的下载文件夹，不用 `downloads` 权限。
- 嗅探规则和界面都是独立写的，没有复制 cat-catch（GPL-3.0）的代码，所以可以用 MIT。

## 9. 桌面端界面（`apps/desktop/src/`）

```
main.tsx              入口：先装演示后端（只在带 ?mock 时），取启动快照，再画界面；#/login-bar 只画登录窗口的工具条
theme.css             颜色、圆角、阴影、曲线等变量（:root），affectUI 的主题桥（--aui-*），窗口骨架和各页面共用的样式
components/ui/        affectUI 免费组件，原样不改（AFFECTUI-SOURCES.json 记着来源）
parts/index.tsx       自己写的小部件：logo、进度条、步骤时间线、倒计时、状态互换、站点图标……
shell/                窗口骨架、侧栏、顶栏、登录窗口工具条、全局动作（事件分发、打开弹窗）
pages/                下载、素材库、AI 助手、浏览器插件四页，各页的子目录放这一页的部件和逻辑
dialogs/              设置、上手引导（onboarding/）
lib/                  和后端通信（bridge.ts：invoke + 事件）、状态（store.ts）、路由、类型、格式化
i18n/                 多语言：t()、tx()、词表（locales/<语言>/<分册>.json）、存下来的站点名（sites.ts），见第 13 节
extension-popup/      插件工具栏面板的源码
dev/                  演示后端（mock*.ts）和演示图片，不进正式构建
assets/               字体（Geist）、上手引导的剧照、站点图标
```

- **路由**：单页应用，用网址的 `#` 记当前页面（`#/downloads`、`#/library?asset=5`、`#/downloads?t=12`、`#/downloads?paste`）。
  后端（插件、命令行要求打开某个页面）通过 `window.captaeGo("/library?asset=5")` 跳过来；桌面端 `--page library` 直接打开某一页。旧界面的地址（`app-library.html#asset=5` 这类）后端也认。
- **和后端通信**：只用 `@tauri-apps/api` 的 `invoke` 和事件 `captae://event`（任务进度、设置、嗅探、剪贴板、登录……），`withGlobalTauri` 关着。
  `@tauri-apps/api` 的小版本要和 Rust 的 `tauri` 一致（现在都是 2.11：前端 `~2.11.1`，Rust 2.11.6），`cargo tauri build` 会核对，对不上就不打包。
- **asset 协议**（页面读本机的缩略图、预览视频）：`tauri.conf.json` 里范围是空的，`src-tauri/src/asset_scope.rs` 启动后按需放行：缩略图目录、各个保存位置（影片 / 音乐 / 图片下的 Captae、设置里选的），
  素材所在的那一层文件夹在素材库列表加载、打开某条素材、下载完、改保存位置时放行。数据目录本身不放行（里面有 `control.json` 的令牌）。
- **演示后端**：在浏览器里带 `?mock` 打开时代替 Captae 的后台，给截图和检查界面用，正式构建里没有（`lib/bridge.ts` 的 `setupMock` 只在开发服务器和演示构建里加载）。
  常用参数：`?mock`（下载页各种状态）、`?mock=empty`（第一次打开）、`&still`（停住计时，截图用）、`&open=settings|onboarding`、`&side=off`、`&clip`（打开时问一次剪贴板）、`&many`（插件找到一大堆）、`&wait`、`&notice`、`&browser=chrome`、
  `&rec`、`&fails`、`&bili=parts`；素材库 `&whisper`、`&fail`、`&ltask`；AI 助手 `&agent=new|idle`、`&calls=login`；插件页 `&ext=off|gone|fix|nohost|nohost-on`、`&extfrom=arc`；
  登录工具条 `#/login-bar&bar=verify|insecure|other`。完整的列在 `dev/mock.ts`、`mock-library.ts`、`mock-agent.ts`、`mock-extension.ts` 开头。
- **构建**（都在 `apps/desktop` 里，先 `npm ci`）：
  - `npm run build`：tsc 检查 + 正式构建到 `dist/`，Tauri 打包时嵌进程序（`tauri.conf.json` 的 `beforeBuildCommand` 会自动跑）。
  - `npm run build:mock`：演示构建到 `dist-mock/`，打成单个 HTML（vite-plugin-singlefile），用无界面浏览器从 `file://` 打开截图。几个人同时干活时用 `--outDir` 各用各的目录。
  - `npm run build:popup`：插件工具栏面板构建到 `extension/popup/`（文件名不带哈希，产物提交进仓库）；`--mode mock` 是面板的演示构建。
  - `npm run typecheck`：只做类型检查。`npm run affectui -- <名字>…`：从 affectUI 公开 registry 拉免费组件（付费的会返回 401，拉不到）。
- **界面检查**：维护者本机有两套 Playwright 脚本（不在仓库里），对演示构建跑：下载页 34 项、素材库 47 项（点按钮、看文字和状态、量动效时长）。
  改了界面以后先 `npx tsc --noEmit -p .`，再演示构建、跑这两套检查。
- **macOS 上要注意**：Tauri 默认会往 CSP 的 style-src 里加哈希，WebKit 看到哈希就不再认 `'unsafe-inline'`，页面里所有内联样式都会失效，
  所以 `tauri.conf.json` 里用 `dangerousDisableAssetCspModification: ["style-src"]` 关掉了这一改写。界面在 WKWebView（Safari 内核）里跑，玻璃、模糊和动效要在真窗口里看。

### 9.1 剪贴板提示（`crates/captae-core/src/clipboard.rs`、`apps/desktop/src-tauri/src/clip_ask.rs`）

复制了视频链接、**而且 1 分钟内把 Captae 拿到前面（或打开 Captae）**，才问一句要不要下载：下载页在输入框上方出一张提示卡，别的页面弹提示条。
复制的那一刻什么都不弹，插件在网页上也不弹（原来两边复制了就提示，复制链接多半是要发给别人，打扰）。设置里「复制链接时问我要不要下载」关掉就都不问。

- Captae 开着时：后台线程每半秒看一眼系统剪贴板的变更次数（`NSPasteboard.changeCount`，只看次数、不读内容），记下最近一次变化的时间、那时 Captae 在不在前台（`AskGate`）。
  主窗口拿到焦点时才读一次剪贴板，是视频链接（平台视频页、媒体直链、M3U8）才问。在 Captae 自己里面复制的（那时 Captae 在前台，登录窗口也算）不问；一次复制最多问一次。
  复制完马上切过来、后台还没看到这次变化的，拿到焦点那一刻先看一眼，算在切过来之前。
- Captae 没开时没人看剪贴板：插件在网页里看到复制了视频链接，经宿主写 `<数据目录>/copied-link.json`（链接和时间）。打开 Captae 时读走它，
  剪贴板里还是这个链接、又在 1 分钟内，就问。别的程序里复制、Captae 又没开的，不知道是什么时候复制的，不问。
- 刚打开 Captae 时，主窗口拿到焦点那一刻页面多半还没在听事件：页面挂好监听以后调 `clip_check`，由它来问这一次；之后每次拿到焦点由 `on_window_event` 来问。
- 侧栏「浏览器里发现的」只放最近 3 条视频和声音（开着很多网页时找到的东西很多，图片最多，全列出来太占地方），标题右边写「全部 N 个」，全部在「浏览器插件」页。

## 10. 动效

规矩和数值在 `docs/MOTION.md`（代码注释里的「配方 a」「配方 c5」指它的小节）。数值来自 Emil Kowalski《Animations on the Web》课程的曲线和时长，加上对几款参考应用录屏的逐帧实测；
仓库里只写我们自己的取值和理由，课程的练习和演示代码不进仓库。React 根上 `<MotionConfig reducedMotion="user">`，系统开了「减少动态效果」时每一处都只淡入淡出。

## 11. 测试和调试

- `cargo test -p captae-core`：单元测试 + 端到端（本机有 ffmpeg、aria2c、yt-dlp 时真的跑下载、合并、入库、转换、剪辑、拼接、换音轨、字幕、预设）。
  `cargo test -p captae-desktop`：命令白名单、登录窗口的地址和导航规则、旧地址的路由。
- **测试数据目录**：设 `CAPTAE_HOME=<目录>` 后所有数据放在这个目录下（`data/`、`Movies/`、`Music/`、`Pictures/`），测试版的登录存储也和正式版分开；
  这时桌面端不注册插件宿主（不然会把浏览器里的插件指到测试版上），要测插件时另设 `CAPTAE_REGISTER_HOST=1`。测试数据目录的 `data/settings.json` 写 `"autoUpdate": false`，免得每次都去下新版引擎。
- **开发版要换一个标识编译**：`TAURI_CONFIG='{"identifier":"app.captae.dev"}' cargo build -p captae-desktop`，再 `CAPTAE_HOME=<目录> target/debug/captae-desktop` 运行。
  原因：标识和正式版一样时，单实例插件会把新启动的这个直接交给正在运行的 Captae（测的其实是正式版），WebKit 的数据目录、登录存储也会和正式版共用。几个人同时测时各用各的标识。
  不设 `CAPTAE_HOME` 不要运行任何桌面端（会读写正式版的数据、改浏览器里插件宿主的登记）。
- **只在调试版里有的开关和钩子**（`cfg(debug_assertions)`，正式版不编译这些代码）：
  - `CAPTAE_TEST_ONTOP`：主窗口和登录窗口浮在最上面、被挡住也照常画。台前调度开着时，从终端启动的窗口抢不到前台、会被收进侧边条，WebKit 也不画。
    **别人正用着电脑时不要开**：窗口一旦被显示出来就盖在最上面。
  - `CAPTAE_TEST_NO_SHOW`：插件、命令行要求打开窗口时（「在 Captae 里查看」「打开 Captae」、打开某条素材）只让页面跳过去，不把窗口拿到前面。
  - `CAPTAE_TEST_NO_CLIPBOARD`：不写系统剪贴板（「发给 AI」、复制文字改成记一行日志）。实测「发给 AI」一律先打开它。
  - `CAPTAE_TEST_CLIPBOARD=<文件>`：用这个文件代替系统剪贴板（剪贴板提示、「粘贴」读它；改一次文件算复制一次），实测剪贴板提示不碰用户的剪贴板。
    `CAPTAE_TEST_CLIP_SECS=<秒>`：把剪贴板提示的「1 分钟内」改短。`<数据目录>/clip-test.json` 写 `{"focus": true}` / `{"focus": false}`：当作主窗口拿到 / 失去焦点（藏着的测试版拿不到真焦点）。
  - `<数据目录>/eval.js`：写进去的 JS 会在主窗口里执行一次（自动化走查界面用；结果用 `invoke('js_log', …)` 写进日志再读）。
  - `<数据目录>/login-test.json`：登录窗口的测试钩子（在网站那块或工具条里执行 JS、往存储里放假的登录 cookie、列出 cookie 名字、改窗口大小、关窗……）。
    `{"paint": true}` 只让 WebKit 不管窗口被不被挡住都照常画；**不要写 `{"top": true}`**，它会先藏再显示主窗口、浮在最上面。
- **在别人正用着的电脑上实测**（测试版的窗口要是浮到屏幕上，会被当成自己的 Captae 来用）：
  - 测试版放进自己的 .app 外壳（标识和编译时的一样），Info.plist 写 `LSUIElement`（不进程序坞，用户看不到也点不到）；入口脚本里设 `CAPTAE_HOME`、`CAPTAE_TEST_NO_SHOW`、`CAPTAE_TEST_NO_CLIPBOARD`，不设 `CAPTAE_TEST_ONTOP`。
    宿主在桌面端没开时会用 `open -a` 拉起这个外壳，那样启动拿不到调用方的环境变量，所以这些都写在入口脚本里。
  - 编译时让主窗口一开始就是隐藏的：`TAURI_CONFIG` 里除了标识，再把 `app.windows` 整个写一遍并加上 `"visible": false`（数组是整个替换的）。
    Tauri 先按配置建窗口（默认可见）、再跑我们的 setup，`--background` 要到 setup 最后才把窗口藏起来，中间会显示一下。正式版不受影响（macOS 上宿主拉起 Captae 用 `open -g -a`，本来就要显示窗口）。
  - 一律带 `--background` 启动（`open -n -g <外壳> --args --background`），核对一律用 `eval.js` 读页面、读日志和文件。窗口从没显示过时页面是「隐藏」状态：定时器会被限流，
    网格里 `loading="lazy"` 的缩略图不会加载，要核对图片和媒体能不能读，用 `new Image()`、`<video>` 直接按 asset 协议读。
  - 停测试版用 `osascript -e 'tell application id "<标识>" to quit'` 正常退出，停完 `pgrep -f <测试数据目录>` 确认没有残留。不要用 `kill -9`（退出流程一步都不走，yt-dlp、ffmpeg、aria2c 会留成孤儿）。
  - 重新装外壳时先把整个 .app 删掉再建：原地覆盖已经签过名的文件，内核还记着旧的签名，启动时会被当成「签名无效」直接结束。
- **退出**：⌘Q、关主窗口、收到 SIGTERM（`kill`、关机）/ SIGHUP（启动它的终端关了）/ SIGINT（终端里 Ctrl-C）都走同一个退出流程（`RunEvent::Exit`）：删 `control.json`、
  结束还在跑的 yt-dlp、ffmpeg、aria2c 连同子进程；录直播的原始数据留着，下次打开整理好。收到信号 5 秒还没退完、或者又来一个信号，就自己做完这两件事直接退出（`main.rs` 的 `quit_on_signals`）。
- 命令行：`captae probe <链接> [--json]`、`captae download <链接> [--sample <MB>] [--items 1,2|all] [--record-live [--max-minutes N]] [--json]`、`captae update [--force] [--json]`、
  `captae engine [--health] [--json]`、`captae status`。`probe` 和 `--sample` 在本进程里跑、不经过桌面端；`--json` 输出一行。退出码 0 成功、1 失败、2 用法不对、3 播放列表要选条目。
  样片默认放在 `<数据目录>/samples/`。几百个网站的批量实测就是用 `--sample` 跑的。录直播的 Ctrl-C 见 4.6。
- 插件宿主的排查日志：`CAPTAE_LOG=captae_core::native_host=debug` 每条消息记用了多久；超过 3 秒的、连不上要重新打开 Captae 的，info 级别就会记。

## 12. 打包

```
scripts/build-release.sh         打正式安装包：依次运行下面几步，编译时去掉本机路径
scripts/fetch-sidecars.sh        准备附带的 yt-dlp（目录版）、deno、ffmpeg / ffprobe、aria2c（含动态库和 OpenSSL 模块）、captae；已经有的跳过，下载的都要校验
cd apps/desktop/src-tauri && cargo tauri build --config tauri.release.conf.json --bundles app
scripts/sign-macos.sh            钥匙串里有 Developer ID 证书时：整个 app 重签、签完自检，然后交给 Apple 公证、钉上结果（见下面）
dmgbuild -s apps/desktop/src-tauri/dmg/settings.py …   把 Captae.app 做成 dmg（窗口的背景图、大小、图标位置；装在 target/dmgbuild-venv）；有证书时 dmg 也签名、公证、钉上
scripts/check-bundle.sh          检查打好的 dmg（只读挂载，不运行桌面端；附带程序在看不到 Homebrew 的环境里跑；程序里有本机主目录的路径也算不过；第 8 项核对安装包窗口）
scripts/render-dmg-background.sh 改了安装包窗口的背景页或 layout.json 以后重新截背景图（两张 PNG 提交进仓库）
python3 scripts/gen-licenses.py  依赖变了以后重新生成 licenses/rust-crates.md、npm-packages.md
```

**安装包窗口**（在访达里打开 dmg 看到的那一页）：不用 Tauri 自带的 dmg 脚本（它靠 AppleScript 让访达排版，要有人看着屏幕、不能锁屏，排出来也只是默认的白窗口），
用 dmgbuild（MIT，Python 3.11 以上；版本和校验值写死在 `src-tauri/dmg/requirements.txt`）直接写 `.DS_Store`，没人看着也能打，每次一样。
- 版式：标题「Drag Captae to Applications」和一句副标题在上面（安装包窗口只能有一种语言，用英文）；一条五格的开放电影剧照「胶片」横穿窗口，从一块轻玻璃（和界面里浮在内容上的玻璃同一套变量）后面经过，
  玻璃外面清楚、里面糊开；Captae 和「Applications」两个图标、中间的箭头在玻璃上。背景页源码 `apps/desktop/src/dmg-background/`（React，和界面同一套 theme.css、Hugeicons），
  `scripts/render-dmg-background.sh` 用无界面 Chrome 按 1 倍、2 倍各截一张，dmgbuild 合成带高清版的 TIFF。窗口大小、图标位置、玻璃的位置都在 `src-tauri/dmg/layout.json`，背景页和 `settings.py` 读同一份。
- `.DS_Store` 里的窗口大小是**连标题栏一起算**的（macOS 26 的访达实测：写 660×400，内容区只有 660×368，背景图下面被切掉 32 点），
  所以写进去的高度是内容区加 `titleBar`（32），背景图也多画这一截底色（标题栏矮的旧系统上露出来的是同色）。
- 窗口有背景图时，访达在深色模式下也按浅色画图标的名字（深色字），所以背景只做浅色这一版。指向「应用程序」文件夹的快捷方式叫 Applications，和标题一致。

`tauri.release.conf.json` 把附带程序（`externalBin`）、插件文件夹、`licenses/`、`THIRD_PARTY_NOTICES.md` 打进安装包，并且给整个 app 做本地签名：`bundle.macOS` 里
`"signingIdentity": "-"`、`"hardenedRuntime": false`。没有 Apple 的开发者证书时，不封好整个 app 的签名（只有链接器给主程序加的临时签名、Info.plist 和资源没封），
从网上下载后打开很可能被说成「已损坏」；hardened runtime 只在公证时有用，本地签名时开着它，aria2 加载不了自己带的动态库（没有 Team ID，过不了库校验），deno 也会因为没有 JIT 授权跑不了 JS。

**Developer ID 签名和公证**：钥匙串里有「Developer ID Application」证书时，`build-release.sh` 在 Tauri 本地签完以后用 `scripts/sign-macos.sh` 由里到外整个重签，
都开 hardened runtime、带时间戳：
- yt-dlp 目录版带的 `Python.framework`：官方 zip 打包时把符号链接展开成了完整的副本（`Versions/Current`、framework 顶层的 `Python` 和 `Resources`，还有 framework 外面的 `_internal/Python`），
  codesign 认不出这是什么 bundle（bundle format is ambiguous），签不了。先恢复成 PyInstaller 原来的符号链接（只换内容和版本目录里一模一样的），再整个当 framework 签。
- Resources 里其余的 Mach-O 一个个签（yt-dlp 的引导程序和 .so、aria2 的动态库和 OpenSSL 模块），然后 MacOS 里的附带程序，最后整个 app。
- 同一个团队签的库能过库校验（aria2 的动态库、随包的 OpenSSL 模块都能加载）；deno 带 `src-tauri/entitlements/deno.plist`（JIT），没有它开着 hardened runtime 跑不了 JS（实测一跑 JS 就崩）；
  yt-dlp 不用额外授权（实测 `--version`、`--list-impersonate-targets`、模拟 Chrome 读网址都正常）。
- 签完自检：在 hardened runtime 下、用 `sandbox-exec` 挡住 `/opt/homebrew` 和 `/usr/local`（模拟没装 Homebrew 的 Mac），把附带程序都跑一遍，有一个跑不了就停下，不交给 Apple。

签完先把 app 打成 zip 交给 Apple 公证、钉进 app，做好 dmg 以后 dmg 也签名、公证、钉上（刚公证完偶尔钉不上，会等 30 秒重试），
下载的人打开时系统只问一句「是从互联网下载的，要打开吗」。Apple 的公证记录存在 `target/release/notary/<文件名>.log.json`（过了也可能有提醒）。
公证用 App Store Connect API 密钥（`~/.appstoreconnect/private_keys/AuthKey_*.p8` 加 Issuer ID），Issuer ID 这些写进 `keys/notary.env`（`keys/` 不进仓库），变量名见 `build-release.sh` 开头。
`check-bundle.sh` 第 3 项核对：每个程序和库是同一个团队签的、可执行文件开着 hardened runtime、带时间戳、app 和 dmg 上都钉着公证结果、Gatekeeper 对 app 和 dmg 都认（Notarized Developer ID）。
**打包用的脚本跑在 macOS 自带的 bash 3.2 上**：字符串里变量后面紧跟中文时要写成 `${变量}`，不然 bash 会把中文的字节当成变量名的一部分（`set -u` 下直接退出）。
版本号在 `Cargo.toml` 的 `[workspace.package]`（命令行、MCP 报的版本也是它）、`tauri.conf.json`（安装包和 Info.plist）、`apps/desktop/package.json`、`extension/manifest.json` 四处，一起改。
附带程序的版本写在 `fetch-sidecars.sh` 里，
yt-dlp、deno 的版本另记在 `binaries/*.version`，改了版本号就重新下；yt-dlp 先用官方公钥验 `SHA2-256SUMS.sig`（装了 gpg 时）再核对 SHA256，deno 核对脚本里写死的 SHA256 和官方 `.sha256sum`。

## 13. 多语言

界面、提示、命令行输出支持 8 种语言：简体中文（原文）、English、繁體中文、日本語、한국어、Deutsch、Español、Français。
写法、词表、术语和检查脚本（`scripts/i18n.py`）见 `docs/I18N.md`，这里只记和架构有关的几件事。

- 三处各有一个小模块，写法相同（代码里写中文原文，按原文查译文）：核心库 `crates/captae-core/src/i18n.rs`（`tr!`，词表编译时放进程序）、
  界面 `apps/desktop/src/i18n/`（`t()`，词表按语言分块加载，插件工具栏面板只打进 `common`、`popup` 两个分册）、插件 `extension/lib/i18n.js`（`tr()`）。
- 设置里的 `language`（空 = 跟随系统）由桌面端、命令行、MCP、插件宿主共用；跟随系统时按系统首选语言里第一个支持的，一个都不支持时用英文。
  启动快照带 `systemLang`、`lang`；插件宿主的 `hello` 回复带 `lang`、`langSet`，插件跟 Captae 实际用的语言（插件里显示的错误说法、任务记录是 Captae 生成的），
  电脑上没有 Captae 时跟浏览器。插件在 `chrome://extensions` 里的简介按浏览器语言（`extension/_locales/`）。
- 后端生成的文字（任务记录、错误说法、通知、`fmt`、文件夹名）按生成那一刻的语言写，换语言以后已经生成的不变。
  界面、插件判断情况一律看固定字段，不看文字：`fail.code`、`fail.reason`、`fail.brief`、`job.untitled`（标题还是读取前的占位）、`job.probeAt`（这一次读取从什么时候开始）、
  固定代码的错误（`errors::coded`，比如「停止并保存」时任务已经不在录制的 `NOT_RECORDING`；经本机控制接口、插件宿主转发时带着 `code`）。
- 给 AI 看的固定说明只用英文：MCP 的 `instructions`、工具和参数的说明、错误里的 `next`、`skills/captae/SKILL.md`。工具的显示名字（MCP 的 `title`）跟界面语言；
  命令行打印给人看的「下一步」用 `ErrorCode::cli_hint`，跟界面语言。
- 存下来的值不翻：预设的步骤（`presets.rs` 按中文解析，显示时用 `step_label` / `stepLabel` 翻）、协议值（「保持原格式」「最高可用」）、站点名和素材的来源（存中文原文，显示时翻）。
  字幕翻译的目标语言跟界面语言（简体中文照旧传「中文」）。
- 换语言时界面整个按新语言重新挂一次（`main.tsx` 的 `Root`）。这一次挂载不播进场动画：Motion 的动画直接到终点，刚开始的 CSS 动画在画出来之前跳到结尾；
  设置弹窗还开着，焦点留在语言那一栏。macOS 菜单栏跟着重建（`src-tauri/src/menu.rs`）。
- 只能有一种语言的地方用英文：安装包窗口（标题、Applications 快捷方式）、打包配置的 `shortDescription`。`src-tauri/Info.plist` 的 `CFBundleLocalizations` 列出 8 种语言，
  系统的文件选择窗口、文本框右键菜单用对的语言，「系统设置 › 通用 › 语言与地区 › 应用程序」里也能单独给 Captae 选语言（Captae 选「跟随系统」时跟着它）。
- 字体按 `lang` 换中日韩的字形（`theme.css`、插件的 `ui.css`）：同一个汉字简体、繁体、日文写法不同。


## 14. Windows 和 Linux

2026-11-24 起有 Windows 10/11 x64 和 Linux x86_64（Ubuntu 22.04 及以上、Debian 12、Fedora 39 这一档）版，功能和 macOS 版一样。
ARM 的 Windows、Linux 没有安装包（代码能编，附带程序没准备）。下面只记和 macOS 不一样的地方。

### 14.1 位置

| | macOS | Windows | Linux |
|---|---|---|---|
| 数据目录 | `~/Library/Application Support/app.Captae.Captae` | `%LOCALAPPDATA%\Captae\data`（不放漫游的 `%APPDATA%`：引擎、语音模型几百 MB 起） | `$XDG_DATA_HOME/captae`（`~/.local/share/captae`） |
| 保存位置 | 影片 / 音乐 / 图片 › Captae | 视频 / 音乐 / 图片 › Captae | XDG 用户目录（没设退回 `~/Videos`） |
| 安装位置 | `/Applications/Captae.app` | `%LOCALAPPDATA%\Captae`（NSIS，装给当前用户，不要管理员） | deb：`/usr/bin/captae-desktop` + `/usr/lib/Captae/`；AppImage 单个文件 |
| 附带程序 | `Contents/MacOS/` + `Contents/Resources/yt-dlp/` | 安装目录（和 captae-desktop.exe 同一层）+ `yt-dlp\` | `/usr/lib/Captae/`（平铺，`tools::sidecar_dir_of`） |

Linux 的附带程序不放 `/usr/bin`：会和系统的 ffmpeg、aria2c 冲突，dpkg 拒装（`scripts/build-release-linux.sh` 的说明）。
桌面端、命令行、找插件宿主都用 `tools::sidecar_dir_of`：`<exe 目录>/../lib/Captae/captae` 在就用那里，不然用 exe 所在的目录。

### 14.2 进程（`tools.rs`，接 4.7）

- Windows：每个外部工具起来后放进自己的 Job Object（`KILL_ON_JOB_CLOSE`）。`kill_tree` 先结束整个 Job，再按进程快照找漏掉的子孙（比创建时间，防进程号复用）；
  Captae 崩了、被任务管理器结束时系统关掉句柄，工具也跟着结束。`alive` 用 `OpenProcess` + `GetExitCodeProcess`。
- 「停止并保存」在 Windows 上发 **Ctrl-C**（不是 Ctrl-Break：yt-dlp 里的 Python 收到 Ctrl-Break 直接退出，不收尾；aria2c 只认 Ctrl-C）。
  外部工具带 `CREATE_NO_WINDOW` 起，各有一个看不见的控制台，Ctrl-C 只能从连着这个控制台的进程发，所以交给一次性的辅助进程 `captae __ctrl-c <pid>`
  （命令行的隐藏子命令：连上那个控制台、发一次、退出），不在 Captae 自己的进程里换控制台。找不到 captae.exe 或发不出去时，马上结束进程树、整理录到的部分（原来的兜底）。
- 根证书：Linux 上按发行版找系统证书包设 `SSL_CERT_FILE`（附带的 ffmpeg、aria2c 用的 OpenSSL 编译时写的 `/etc/ssl`，Debian 系上没有 `cert.pem`）；
  Windows 不设（ffmpeg 用 schannel、aria2c 用 WinTLS，读系统证书库）。
- `which`：Linux 补 `~/.local/bin`、`/usr/local/bin`（从应用菜单打开时 PATH 里常没有）。不经过 `Toolbox::command` 起的程序也加 `no_window`，Windows 上不闪黑窗。
- 命令行在 Windows 上接 Ctrl-C、Ctrl-Break、关控制台、注销、关机（`tokio::signal::windows`），行为和 unix 的信号一样（录直播第一次 Ctrl-C 停止并保存、第二次直接退出）。

### 14.3 插件宿主（`native_host.rs`，接第 8 节）

- Windows：清单写在数据目录 `app.captae.bridge.json`，注册表 `HKCU\Software\{Google\Chrome,Chromium,Microsoft\Edge}\NativeMessagingHosts\app.captae.bridge` 的默认值指向它（用 winreg 写，`status()` 读回来核对）。
  Chrome 各渠道只读 Chrome 的键；Chromium、Brave、Vivaldi、Opera、Arc 先读 Chromium 的、再读 Chrome 的；Edge 先读自己的、再读这两个（Chromium 源码 `launch_context_win.cc`、微软文档）。
  清单直接指向 `captae.exe`（浏览器直接启动它，第一个参数是 `chrome-extension://…/`，命令行据此当成 `native-host`）；只有便携模式（设了 `CAPTAE_HOME`）写 .bat。
- Linux：各浏览器的 `~/.config/<浏览器>/NativeMessagingHosts/`，含 Beta / Dev 渠道；Flatpak 版浏览器写在 `~/.var/app/<id>/config/…`，清单指向沙盒里看得到的包装脚本
  （`flatpak-spawn --host`），要用户自己跑一次「浏览器插件」页给出的 `flatpak override --user --talk-name=org.freedesktop.Flatpak <id>`（放宽浏览器沙盒，Captae 不替用户开）；Snap 版 Chromium 照样写，多半连不上。
- AppImage：运行时挂在 `/tmp/.mount_xxx/`，退出就没了，清单不能指过去。AppImage 版启动时把 `captae` 拷到数据目录 `host/` 再登记（`ext_cmds.rs`）；
  宿主拉起桌面端用 `app_path` 里记的 .AppImage 文件（`$APPIMAGE`）。这份宿主在桌面端没开时自己读视频信息，用的是自动更新下载的 yt-dlp（附带的它够不着）。
- 认是哪家浏览器拉起的：Linux 读 `/proc/<父进程>/exe`，Windows 用进程快照 + `QueryFullProcessImageNameW`，按表里各浏览器程序路径的末尾几段比对。
- 桌面端在 Linux 上一启动就去掉值为 `disabled:` 的 `DBUS_SESSION_BUS_ADDRESS`（Chrome 给插件宿主设的，单实例插件连会话总线时会 panic），宿主拉起时也不传下去。
- 拉起桌面端：Windows 带 `DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_BREAKAWAY_FROM_JOB`，Linux `setsid`，浏览器关掉插件时桌面端不跟着结束。
  被拉起的桌面端的报错输出写进数据目录的 `launch.log`（每次覆盖，没设 `CAPTAE_LOG` 时按 info），宿主里有个线程等它、退出了收走。

### 14.4 网络和更新

- 系统代理（接 4.4）：Windows 读 `HKCU\…\Internet Settings`（`ProxyServer` 两种写法、`ProxyOverride`、`AutoConfigURL` 当 PAC 按直连）；
  Linux 先环境变量，再读 KDE 的 `kioslaverc` 或 GNOME 的 gsettings。
- yt-dlp 一律带 `--encoding utf-8`：它往管道打印时按 stdout 的编码 `encode(…, 'ignore')`，Windows 上全角的「？」「：」「｜」会被丢掉，
  打印出的最终路径和磁盘上的文件名对不上（PyInstaller 打包的 yt-dlp 不认下面这些环境变量）。
- 起 Python 程序（yt-dlp、语音识别脚本、pip）一律设 `PYTHONIOENCODING=utf-8`、`PYTHONUTF8=1`、`PYTHONUNBUFFERED=1`（`tools::python_utf8`）：
  Windows 上不设时按 ANSI 代码页往管道写，yt-dlp 打印的最终路径里全角的「？」「：」对不上磁盘上的文件。
- 语音识别安装：建环境时优先用 3.12、3.13、3.11、3.10（Windows 用 `py -3.12` 这样选，Linux 找 `python3.12`），都没有才用默认的 Python；
  组件没有这个 Python 的 wheel 时删掉这个环境，提示装 Python 3.12。建好虚拟环境先升级 pip（Ubuntu 22.04 的 venv 里是 pip 22，不认 `--progress-bar raw`，升不了就不报进度），只装现成的 wheel（`--only-binary :all:`，
  Python 太新、没有对应 wheel 时直接说要装 Python 3.12，不去编源码）。
- yt-dlp 自动更新（接 4.3）：三个平台都开，用官方目录版 `yt-dlp_win.zip` / `yt-dlp_linux.zip`；验签、SHA256、自检、退回一样。
  macOS 用 ditto 解压（保留签名要的符号链接和扩展属性），Windows、Linux 用 Rust 的 zip 包（挡 `../`，符号链接只许指向包里面）。
  Windows 上某个版本的 yt-dlp 正在跑时，那个目录挪不动、删不掉：挪不动报更新失败、继续用原来的，删不掉的下次再删。
- 读取信息时 yt-dlp 读浏览器的登录状态失败（Windows 上 Chrome / Edge 的 app-bound 加密、浏览器开着锁住了数据库，Linux 上密钥环没解锁）：
  任务记录里说明原因，这次不带浏览器的登录状态再读一次（`queue.rs`，`login::browser_failure_hint`）。设置里能选的浏览器按系统分：Safari 只在 macOS；Windows 上 Firefox 排第一，其余写明读不了。

### 14.5 登录窗口（接 5.1）

- 窗口在 Windows / Linux 上保留系统标题栏（`titlebar_height` 是 0）。网站那块用单独的数据目录 `<数据目录>/login-store`：
  Windows 上 wry 给每块 webview 建自己的 WebView2 environment，目录不同就是不同的浏览器进程；Linux 上 Tauri 按目录分 `WebContext`，cookie 是这个目录下的明文 `cookies` 文件，目录设成 700。
- 读、删 cookie 直接调 `ICoreWebView2CookieManager`（`login_win.rs`）、`WebKitCookieManager`（`login_linux.rs`），带点的域名（`.youtube.com`）也删得掉。
  登录窗口没开时建一个看不见的窗口 `login-store` 来读（同一个数据目录，什么命令都不能调），闲 2 分钟关掉。
- 拦截：Linux 用 WebKitGTK 的 `UserContentFilterStore`，和 macOS 同一份规则；Windows 在网站那块把 wry 挂的 `asset.` / `ipc.` / `tauri.` WebResourceRequested 过滤器摘掉
  （自己加过滤器回 403 不行：wry 的处理函数先拿到请求、回的内容会盖掉自己的）。命令白名单本来就不分平台。
- User-Agent：Windows 用 WebView2 自己的（和 Edge 的写法一样），下载时 `--impersonate chrome:windows`；Linux 照 macOS 装成 Safari。

### 14.6 桌面端外壳和界面（接第 9 节）

- 主窗口一开始就无边框、先藏着（`tauri.windows.conf.json`、`tauri.linux.conf.json` 覆盖 `app.windows`；Tauri 合并配置时数组整个替换，打包配置不要再覆盖 `app.windows`），
  按屏幕缩好再显示，`--background` 时不显示。
- 界面最上面一条标题条（`shell/WindowControls.tsx`）：右边最小化、最大化 / 还原、关闭，Windows 照 Windows 11（46×32、关闭悬停红底），Linux 照 GNOME（浅灰圆钮）；
  整条能拖，双击最大化。做不到的：Windows 11 悬停最大化按钮出贴靠布局（WebView2 盖满客户区，系统收不到 `WM_NCHITTEST`；Win+Z 照常能用）、
  标题条右键的系统菜单（Alt+Space 能出）、模态弹窗开着时标题条点不到；Linux 固定三个按钮在右边，不读 GNOME 的 `button-layout`。
- 说法按系统换（`lib/os.ts`）：文件资源管理器 / 文件管理器、回收站、Ctrl+K、开始菜单 / 应用列表；快捷键在 Windows / Linux 只认 Ctrl。演示后端加 `&os=win`、`&os=linux` 看两种样子。
- 剪贴板提示（接 9.1）：Windows 看 `GetClipboardSequenceNumber`，「Captae 在不在前台」不靠窗口的焦点事件（焦点在 WebView2 子窗口里，点回 Captae 时 tao 不发 `Focused(true)`），
  改成每半秒看 `GetForegroundWindow` 是不是主窗口、登录窗口；Linux 用 X11 XFixes 的剪贴板事件计数（不读内容），连不上 X11（纯 Wayland）时只在窗口拿到 / 失去焦点时各读一次。
- 退出：Windows 关机、注销时 tao 会让主窗口走正常的关闭流程，和关窗口一样收拾；调试版从控制台跑时接住 Ctrl-C、关控制台。Linux 的 SIGTERM / SIGHUP / SIGINT 和 macOS 一样。
- 没有托盘（三个平台都没有），关主窗口就是退出。Linux 上玻璃的底色调实了一些，WebKitGTK 画不出模糊时字也不被下面的内容干扰。
- 打开浏览器的扩展程序页面（`open_browser.rs`）：按浏览器的程序路径起它、带 `chrome://extensions`；没装那个浏览器时复制地址。Linux 在文件管理器里显示失败时退回打开所在文件夹。

### 14.7 打包（接第 12 节）

- Windows：`scripts/build-release-windows.ps1`（PowerShell 5.1 也能跑；三个 ps1 存成带 BOM 的 UTF-8）→ `fetch-sidecars-windows.ps1`（yt-dlp 目录版验签、deno、BtbN 的 GPL ffmpeg 9.0.2、aria2 官方 1.37.0，校验值写死）
  → `cargo tauri build --bundles nsis` → `check-bundle-windows.ps1`。NSIS：当前用户、WebView2 缺了静默下载、中英文；
  安装钩子 `src-tauri/windows/installer-hooks.nsh`：装和卸之前结束安装目录里的附带程序（浏览器拉起的 captae.exe 不是桌面端的子进程），卸载删三个注册表键，勾了「删除应用程序数据」再删数据目录。
  安装程序不签名（2026-11-24 定：这一版是测试版），第一次打开会被 SmartScreen 拦，点「仍要运行」。
- Linux：`scripts/build-release-linux.sh`（在 Ubuntu 22.04 上，glibc 2.35）→ deb 和 AppImage → `check-bundle-linux.sh`。aria2 官方没有 Linux 版，脚本从源码静态编（OpenSSL 3.5、c-ares、zlib、Expat、SQLite）；
  ffmpeg 用 BtbN 的静态版。素材库里播放靠 WebKitGTK 经 GStreamer 解码：deb 依赖 `gstreamer1.0-plugins-good`、`gstreamer1.0-libav`（Ubuntu 默认不装 libav，H.264 / AAC 播不了），
  AppImage 打开 `bundleMediaFramework` 带上打包机器上的 GStreamer 插件（随带哪些看打包机器装了什么，清单要在打包机器上重新生成）。
  WebKitGTK 放不了 asset 协议给的视频（一律 FormatError），素材库详情里的 `<video>`、`<audio>` 在 Linux 上改用桌面端起的本机小服务
  （`media_server.rs`：只听 127.0.0.1、地址带每次启动随机的令牌、只给 asset 协议放行范围里页面要过的文件、支持 Range），
  `tauri.linux.conf.json` 的 CSP 为它在 `media-src` 里加了 `http://127.0.0.1:*`（改主配置的 CSP 时这里要跟着改）。AppImage 分两步打：先让 Tauri / linuxdeploy 打不带附带程序的（linuxdeploy 处理不了 yt-dlp 目录版里的库），再解开、放进附带程序、用原来的 runtime 重新拼。
  AppImage 随带的系统库清单在 `licenses/linux-appimage-libs.md`，换了打包机器的系统或 Tauri 版本要在打包机上跑 `scripts/gen-appimage-libs.sh` 重新生成。
- 两边都和 macOS 一样去掉本机路径（`--remap-path-prefix`；Windows 另加 `/PDBALTPATH:%_PDB%`）。
- 许可证（`licenses/`）：aria2 在链接 OpenSSL 3 的 macOS、Linux 构建上按 GPL-3.0-or-later 分发（OpenSSL 3 是 Apache-2.0，和 GPLv2 不兼容），Windows 构建用 WinTLS 按 GPLv2+。
  `scripts/gen-licenses.py` 按三个平台汇总 Rust 依赖（「平台」一列），跑之前要 `cargo fetch --locked`。

### 14.8 测试

- 本机：Windows 用 `cargo check --target x86_64-pc-windows-gnu`（mingw）只查编译；Linux 用 Docker 里的 Ubuntu 22.04 跑测试（arm64 也行）。
  单元测试的假程序：unix 是 sh 脚本，Windows 是 `.cmd` 调同名 `.ps1`（`tools::testkit`）。录直播那几组、几个假 yt-dlp 写成大段 sh 的测试只在 unix 跑；Windows 的 Ctrl-C 送达由 `interrupt_lets_tool_finish` 测（要先 `cargo build -p captae-cli`）。
- 真机：在 Devin 的 Windows Server 2022、Ubuntu 22.04 虚拟机上打包、安装、逐项实测（脚本和记录只在维护者本机 `work/winlinux/`）。不在别人正用着的电脑上起测试版（第 11 节）的规矩一样适用。

## 15. Captae 自己的更新（`apps/desktop/src-tauri/src/app_update.rs`）

2026-11-28 加，0.3.0 起有（0.2.0 没有这个功能，0.2.0 的用户要手动下载一次新版）。下载引擎 yt-dlp 的每日更新是另一回事，见 4.3。
手机端不在这里：iOS 走 App Store，Android 的 APK 版发布时另做。

### 15.1 做法

- 用 Tauri 官方的更新插件 tauri-plugin-updater（2.14）：minisign 验签、macOS 原子地换掉整个 .app（失败时放回旧版）、没有写权限时弹系统的管理员密码框、
  Windows 运行 NSIS 安装程序、AppImage 换文件、deb 用 `pkexec dpkg -i`。它要 tauri 2.12 以上，2026-11-28 跟着把 tauri 从 2.11 升到 2.12（wry 0.57；
  Windows 上直接调 WebView2 的 `webview2-com` 跟着到 0.39、`windows-core` 到 0.62，前端 `@tauri-apps/api` 到 2.12）。
  打包要 tauri-cli 2.12 以上（给更新包签名的 `signer sign --app-version` 是 2.12 才有）：Windows、Linux 的打包脚本装 2.12.6，已经装了旧版也会换掉（GitHub 的机器上 rust-cache 可能留着旧的）。
- 插件只在 Rust 里用，页面不直接调它（没给 `updater:*` 权限）：页面调 `app_update_status` / `_check` / `_download` / `_restart` 四个命令，状态变了收 `{type: "app_update"}` 事件。
- 更新地址只有一个：`https://github.com/sol1560/Captae/releases/latest/download/latest.json`。草稿和预发布不算 latest，Release 点「发布」那一刻用户才查得到。
- 走 Captae 的代理设置（`net::effective`），连「不走代理的主机」一起：插件自己的 `.proxy()` 用 `Proxy::all`、不认例外，所以先 `no_proxy()` 清空，
  再用 `configure_client` 按算出来的代理加上（`app_update::with_proxy`，和 `net::build_with` 同一种写法）；读不出系统代理时什么都不设，交给插件自己读。
  插件用的是 reqwest 0.13，桌面端的 Cargo.toml 只为它打开 `socks` 特性（手动填 socks5 时也能用）。请求里没有任何标识用户的东西。
- latest.json 用 Tauri 的静态格式，四个键。插件先找「系统-架构-安装方式」，所以不写不带安装方式的 `linux-x86_64`，免得 deb 版拿到 AppImage：

| 安装方式 | 键 | 更新包 | 怎么装 |
|---|---|---|---|
| macOS | `darwin-aarch64-app` | `Captae_<版本>_aarch64.app.tar.gz` | 换掉 `/Applications/Captae.app`；没写权限时系统弹管理员密码框 |
| Windows | `windows-x86_64-nsis` | 安装程序本身 `Captae_<版本>_x64-setup.exe` | passive 模式运行（只有进度条，不用点）；装之前 `installer-hooks.nsh` 结束安装目录里的附带程序（浏览器拉起的 captae.exe 也算） |
| Linux AppImage | `linux-x86_64-appimage` | `.AppImage` 本身 | 换掉 `$APPIMAGE` 那个文件 |
| Linux deb | `linux-x86_64-deb` | `.deb` | `pkexec dpkg -i`，系统弹密码框 |

- 安装方式靠 Tauri 打包时写进程序的标记（`__TAURI_BUNDLE_TYPE_VAR_DEB` / `_APP` / `_NSS`），三个平台的检查脚本都核对。
  不是打包出来的程序（开发时直接运行的、macOS 上不在 .app 里的）不更新。
- Tauri 读 latest.json 时先整体校验：一个平台的条目不完整，所有平台都查不到。生成它的脚本（15.6）缺一个就停。

**签名钥匙**：minisign 私钥 `keys/updater.key`（`cargo tauri signer generate` 生成，`keys/` 不进仓库），私钥的密码在维护者的钥匙串里
（服务名「Captae updater signing key」，账户 captae）；公钥写在 `tauri.conf.json` 的 `plugins.updater.pubkey`。
签名里写上版本号（tauri-cli 2.12 起的 `--app-version`），Captae 开着 `requireSignedVersion`：latest.json 说的版本和签名里的对不上就不装，
别人拿旧版本的包配一个新版本号骗不过去。只在维护者这台 Mac 上签，GitHub 的机器上不放私钥：GitHub 账号或某个 workflow 出了问题，别人也发不出能被装上的更新。
**私钥丢了，已经装上的 Captae 再也收不到更新**（只能请用户手动下载），私钥和密码各在维护者自己的密码管理器里备份一份。

### 15.2 什么时候查、下、装

- 打开 1 分钟后查一次，之后每 10 分钟看一眼离上次检查满没满 24 小时（电脑睡眠醒来也赶得上）。调试版可以用 `CAPTAE_TEST_UPDATE_DELAY=<秒>` 改第一次的等待。
- 「自动更新 Captae」（设置里的 `appAutoUpdate`，默认开）开着：查到新版就下载，插件下完先验签，存进 `<数据目录>/app-update/Captae_<版本>.<扩展名>`，
  latest.json 原文记在 `app-update/state.json` 的 `ready` 里：重新打开 Captae 后用 `Updater::restore_update` 不联网也能装，装之前 `Update::install` 再验一次签。
  关着：照样每天查，只在侧栏和设置里说有新版，用户点「下载并更新」才下（下好接着走「重新打开」）。
- 装有两个时机：
  - 点「重新打开」（`app_update_restart`）：有正在下载、正在录的任务时界面先问一句（重新打开后任务接着下，录到的直播下次打开时整理好）；装好后 `app.restart()`。
    Captae 重新打开时，下到一半的任务平时恢复成「暂停」（4.1、`queue.rs` 的 restore），所以装之前把正在下载、排队的任务号记进 `state.json` 的 `resume`，
    新版打开、确认真的更新过以后把其中还在暂停的接着下（`resume_jobs`）；没装成就清掉。退出时装的不记，下次打开照平时一样暂停着。
    macOS 要输管理员密码时，插件把系统的密码框交给主线程弹，所以装的那一步放在 `spawn_blocking` 里，不占着主线程。
    Tauri 的 `restart` 在 macOS 上经 LaunchServices（`open -n -g`）重新打开，新进程拿不到启动它的进程的环境变量；设了 `CAPTAE_HOME`（测试数据目录）时改成自己重新打开
    （`relaunch_keeping_env`：一个 sh 等这个进程号没了再 exec，环境变量、参数原样，报错输出写进 `<数据目录>/relaunch.log`），不然测试版会去读写正式的数据、改浏览器里插件宿主的登记。
  - 正常退出时（`RunEvent::Exit`，退出时的收拾做完以后）装上，装完不打开，下次打开就是新版。这些情况退出时不装：收到结束信号（退出流程只有 5 秒）、
    Windows 正在关机或注销（`GetSystemMetrics(SM_SHUTTINGDOWN)`）、deb（要输密码）、macOS 要输管理员密码的（主线程正在退出，弹不了密码框）、同一个版本已经失败 3 次。
    macOS 算「要输管理员密码」的：「应用程序」文件夹不能写，或者 Captae.app 本身不是这个账号的、没有写权限（另一个管理员拖进去的；换掉 .app 要把它挪到别的父目录下，只有父目录能写不够）。
- Windows：插件运行安装程序后直接结束进程，结束之前 `on_before_exit` 做退出时的收拾（`main.rs` 的 `cleanup_on_exit`：删 control.json、关给手机的服务、结束附带程序）。
- 失败：记在 `state.json` 的 `error`（查 / 下载 / 装哪一步出的错）和 `fails`（每个版本连着失败几次），继续用现在的版本；同一个版本失败 3 次后不再自动下载、退出时不再自动装，等用户自己点。
  下载的包验签不对就删掉，下次重新下。
- 更新后第一次打开：`state.json` 里记的上一次运行的版本比现在的旧，界面提示一次「已更新到 x.y.z」，带「看看改了什么」（这一版的 Release 页）；失败记录清掉。
  主窗口拿到前面（`main.rs` setup 的最后，带 `--background` 的不动）：macOS 上 Tauri 的 restart 用 `open -n -g`（不到前台），旧窗口一关、新窗口却在别的窗口后面。
- 装不了（`blocked`）：`dmg`（在只读的磁盘映像里打开的）、`translocated`（从下载文件夹直接打开、被系统转移运行的）、`readonly`（AppImage 所在的文件夹不能写）、
  `no_pkexec`（deb 版没有 pkexec / zenity / kdialog）、`unsupported`、`test`（设了 `CAPTAE_HOME` 又用正式地址：测试版的标识不同，装上去就把测试版换成了正式版）。
  这些情况不下载，设置里说原因、给「去下载页」。

### 15.3 界面

- 设置 › 关于 Captae（`dialogs/Settings.tsx` 的 `AboutCaptae`）：版本和状态（已是最新 · 上次检查、正在下载和进度条、已下载好、有新版、装不了的原因、上次没更新成），
  右边一个按钮（检查更新 / 重新打开 / 下载并更新 / 再试一次 / 去下载页），下面「自动更新 Captae」开关（deb 版的说明写明要输密码）。
- 侧栏上手清单下面一行（`shell/AppUpdate.tsx`）：下好了是「更新到 x.y.z · 重新打开」，自动更新关着时是「有新版本 x.y.z · 更新」；装不了的不在这里出现。
  和上手清单同一种白底小卡片，出现和消失用 `Collapse`（高度 270ms / 200ms，减少动效时只淡入淡出，第一次挂载不播；一个版本只出现一次，属于偶尔出现的东西）。
- 重新打开前的确认用 affectUI 的 AlertDialog；更新后的提示用 Sonner。演示后端带 `&upd=idle|available|downloading|ready|failed|dmg|deb|updated` 看各种样子（`dev/mock-update.ts`）。

### 15.4 浏览器插件和宿主跟着换

插件是「加载已解压」装的，文件夹在安装包里。Captae 换成新版以后，文件夹里已经是新文件，但浏览器里跑的还是旧代码；浏览器先前拉起的宿主进程也还是旧程序。
- 宿主：桌面端的 `/v1/hello` 回复带上自己的版本，宿主的 `hello` 回复里给插件 `appVersion`。宿主发现桌面端比自己新时，回完手头的消息就退出（`native_host.rs` 的 `stale`），
  插件下次连接时浏览器起的是新程序。只在桌面端比宿主新时退出，不会来回重启。
- 插件（`background.js` 的 `followApp`）：Captae 比插件新、插件又是「加载已解压」装的（`management.getSelf().installType === "development"`）时，`chrome.runtime.reload()` 从文件夹重新读一次；
  同一个版本只试一次（`storage.local` 的 `reloadedFor`）。重新加载后还是旧的（用户是从别的文件夹装的），记下 `stale`，工具栏面板里说「插件比 Captae 旧，到扩展程序页面点「重新加载」」并能打开那个页面，
  桌面端「浏览器插件」页也这么说。网页里的旧部件在插件重新加载后自己拆掉（`content.js` 的 teardown，原来就有）。
- Linux 的 AppImage：给用户「加载已解压」的插件文件夹原来在 AppImage 的临时挂载点里（`/tmp/.mount_xxx/…`），Captae 一退出就没了。
  改成和宿主一样拷一份到数据目录的 `extension/`（`ext_cmds::stable_extension_dir`），内容变了才重拷，插件从这里重新加载自己。

### 15.5 yt-dlp 跟着换

打包时在自带的 yt-dlp 旁边写一个 `version.txt`（三个 fetch-sidecars 脚本）。自动更新装的 yt-dlp 比自带的旧时用自带的（`tools.rs` 的 `managed_ytdlp_in_use`）：
Captae 更新带来了更新的 yt-dlp、而 yt-dlp 的自动更新关着（或者还没来得及查）时，不再一直用旧的。老的安装包没有 `version.txt`，照旧用自动更新装的。

### 15.6 发版（接第 12 节、14.7）

- macOS：`scripts/build-release.sh` 在钉好公证结果以后，把 Captae.app 原样打成 `target/release/bundle/updater/Captae_<版本>_aarch64.app.tar.gz`
  （Tauri 打包时自己生成的那份在重签之前，不能用；先查没有硬链接，插件用 Rust 的 tar 包解不开硬链接；`COPYFILE_DISABLE`、`--no-mac-metadata` 不打进 ._ 文件和扩展属性），
  用 `scripts/sign-update.sh` 签名，`scripts/check-update-package.sh` 检查（签名；照插件的做法解开以后 codesign、Gatekeeper、公证结果、版本、符号链接数）。
- Windows、Linux：`release-winlinux.yml` 照旧在 GitHub 的机器上打包、检查、挂到草稿 Release，只多在日志里打印每个安装包的 SHA256（`CAPTAE-SHA256` 开头）。
- `scripts/release-sign-updates.sh <标签>`（维护者的 Mac 上）：下载草稿上的 Windows 安装程序、deb、AppImage，和 CI 日志里的 SHA256 对照，本机签名，
  生成 latest.json（四个键缺一个就停），上传 macOS 的更新包、四个 `.sig`、latest.json，再下载 Release 上其余的文件重新生成 `SHA256SUMS.txt`。只认草稿 Release。
- 发布前演练：`release-sign-updates.sh <标签> --no-upload --base-url http://127.0.0.1:<端口>` 生成一份指向本机的 latest.json，用本机测试服务和上一版的测试版更新过去。
- `scripts/check-bundle.sh`、`check-bundle-linux.sh`、`check-bundle-windows.ps1` 各加一项：安装方式标记、更新公钥和 tauri.conf.json 一致、更新地址是正式地址、自带 yt-dlp 的 `version.txt`。

### 15.7 测试

- 单元测试：`app_update.rs`（文件名、版本比较、装不了的判断、记录的读写）、`ext_cmds.rs`（AppImage 插件文件夹拷贝，只在 Linux 跑）、
  `tools.rs`（自带的和自动更新装的 yt-dlp 哪个新）、`native_host.rs`（hello 带上 Captae 的版本）。
- 实测用的程序：测试标识、`LSUIElement`、窗口一开始就藏着，更新地址在编译配置里换成本机的 http 测试服务（`dangerousInsecureTransportProtocol` 只在测试编译里开），
  维护者本机的脚本在 `work/updater/`。不在别人正用着的电脑上起会显示窗口的测试版（第 11 节）的规矩一样适用。
