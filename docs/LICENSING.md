# 许可证和商业模式

## 结论

Captae 自己的代码（Rust 核心、桌面端、命令行、MCP 服务器、浏览器插件、Skill、手机端）用 AGPL-3.0-or-later（2026-11-26 维护者定的，原来是 MIT），
以后按 star 数逐步放宽到 MIT、再到 Unlicense，见下面「许可证路线图」。开源，完全免费，只收打赏，见下文。

## 依赖的许可证

| 项目 | 许可证 | Captae 怎么用 | 影响 |
|---|---|---|---|
| yt-dlp | 源码 Unlicense；官方 PyInstaller 打包的可执行文件是 GPLv3+（打包进了 GPL 组件） | 子进程 | 分发官方二进制要附 GPLv3 文本和源码获取方式；用 pip / 源码版就没有这个义务 |
| FFmpeg | LGPL 2.1+；带 x264、x265 等 GPL 组件编译时整体是 GPL | 子进程 | 分发时附对应许可证和源码获取方式。安装包里的都是 GPLv3 构建（macOS 用 martin-riedl.de 的，Windows、Linux 用 BtbN 的 gpl 版；BtbN 也有 lgpl 版） |
| aria2 | GPLv2+，带 OpenSSL 例外 | 子进程 | 同上，附 GPL 文本和源码获取方式。例外只针对旧版 OpenSSL 的许可证，链接 OpenSSL 3（Apache-2.0）的 macOS、Linux 构建按「或更高版本」整体用 GPLv3+ 分发；Windows 官方构建用系统的 SChannel，不涉及 OpenSSL |
| faster-whisper、CTranslate2、Whisper 代码和模型 | MIT | 子进程（Python） | 保留版权声明即可 |
| Buzz | MIT | 只参考了交互，没用代码 | 无 |
| cobalt | API 部分 AGPL-3.0，网页部分 CC-BY-NC-SA-4.0，名字和商标不在授权范围内 | 只参考思路，没用代码 | 不能拷它的代码，也不能用它的名字做宣传 |
| cat-catch（猫抓） | GPL-3.0（1.0 版曾是 MIT），README 要求衍生插件也开源 | 只参考思路，嗅探规则独立写 | 没有拷贝代码。GPL-3.0 和 AGPL-3.0 能合在一起用，但拷进来以后插件就换不成 MIT，所以照旧不拷 |
| Tauri | MIT / Apache-2.0 | 链接 | 保留声明 |
| GTK 3、WebKitGTK 4.1 等系统库（只在 Linux AppImage 里随带） | 大多 LGPL-2.1+ | 动态链接，不改动 | 附许可证和源码获取方式（`licenses/linux-appimage-libs.md`，指向 Ubuntu 的源码包）；.deb 用系统的库，不随带 |
| Geist 字体 | OFL 1.1 | 打包 | 附 OFL 文本（`licenses/geist/OFL.txt`） |
| deno | MIT | 子进程（yt-dlp 解 YouTube 的 JS 验证用） | 保留声明（`licenses/deno/`） |
| 界面的开源组件：affectUI 免费组件、React、Motion、Sonner、Hugeicons Free、@tauri-apps/api | MIT（tslib 是 0BSD） | 打包进界面和插件面板 | 保留版权声明和许可证全文（`licenses/affectui/`、`licenses/hugeicons/`、`licenses/npm-packages.md`）；付费的 UI 库只参考，代码不进仓库 |
| Rust 依赖（418 个 crate） | 大多是 MIT / Apache-2.0；另有 BSD、ISC、Zlib、MPL-2.0、Unicode-3.0 等 | 链接 | 保留声明（`licenses/rust-crates.md`）；MPL-2.0 的几个没有改动，源码地址列在清单里 |

## 子进程调用会不会「传染」GPL

GPL FAQ 对「聚合」和「衍生作品」的说法是：通过 fork/exec、管道、命令行参数通信的程序，通常算两个独立程序；共享地址空间（静态或动态链接）、交换复杂内部数据结构才算一个整体。Captae 对 yt-dlp、ffmpeg、aria2 全部是起子进程、传命令行参数、读标准输出，所以 Captae 本身不受 GPL 约束，可以用任何许可证。

但把这些 GPL 二进制放进 Captae 安装包里一起发，算「聚合分发」，GPL 对那几个二进制本身的义务还在：附许可证原文，提供对应版本源码或书面源码提供承诺。`THIRD_PARTY_NOTICES.md` 已经列好了，发布时照着做就行。想少一点麻烦可以：ffmpeg 用 BtbN 的 LGPL 版本；yt-dlp 走 Python 包而不是官方单文件版；或者安装包里不带这些工具，首次启动时从官方地址下载。

## 插件和 cat-catch

插件的风险点在于 GPL 的「衍生作品」看的是有没有复制受保护的表达。Captae 插件的嗅探逻辑（按 Content-Type 和扩展名分类、读 M3U8 列表）是通用做法，代码独立写，界面来自自己的设计稿，所以不构成衍生作品。在 AGPL 下贴进 cat-catch 的代码许可证上是允许的，但那一块以后就换不成 MIT、Unlicense，等于给下面的路线图添一块要重写的。代码评审时留意这一点。

## 许可证路线图（2026-11-26 定）

| 什么时候 | Captae 自己的代码 |
|---|---|
| 现在 | AGPL-3.0-or-later |
| GitHub 上达到 5000 个 star | 开始逐步切换到 MIT |
| GitHub 上达到 1 万个 star | 开始切换到 Unlicense |

现在用 AGPL，别人拿 Captae 的代码做产品（包括做成网络服务）就得把改过的源码也公开；项目长大以后再放宽，方便别人把核心库、MCP 服务器嵌进自己的工具。

这是长期投入，意味着要重写很多底层的东西。一块代码要换成更宽松的许可证，里面每一行都得能换：

- 维护者自己写的：直接换。
- 外部贡献者写的：贡献者第一次提 PR 时都要先同意贡献者协议（CLA，2026-11-26 定的，正文在 `CONTRIBUTING.md`），协议允许以后改用任何 OSI 认可的开源许可证或公有领域声明（但不能改成闭源），所以直接换。
  协议生效之前合进来的外部代码（现在没有），要作者同意，不同意就重写那一部分。
- 从别的 GPL / AGPL 项目拿来的：换不了，只能重写。
- 依赖的第三方库（MIT、Apache-2.0 这类）：不用动，照旧保留它们的声明。

附带的 yt-dlp、FFmpeg、aria2 是独立程序，各自保留自己的许可证，不在切换范围里。

为了让以后好换，现在就照着做：不往仓库里拷 GPL / AGPL 项目的代码（cat-catch、cobalt 的 API 部分都是），外部贡献的 PR 先确认作者同意了 CLA 再合并，git 历史里的作者要是真实可联系的账号。

### 原来为什么选 MIT（留作参考）

整个项目最有价值的东西是「下载、整理、交给 AI」这条链路和设计，技术壁垒其实不高，别人要抄 GPL 也拦不住。MIT 让别人更愿意把 Captae 的核心库或 MCP 服务器嵌进自己的工具，这对一个靠口碑传播的工具比较重要。

## 开源还是闭源

建议开源，原因主要是这类软件的处境。

分发渠道基本只能自己来。Apple 审核指南 5.2.3 不允许未经授权下载第三方网站媒体的应用，Downie 就一直不在 Mac App Store，而是官网和 Setapp 卖。Chrome 网上应用店明确禁止能下载 YouTube 视频的插件（违规代号 Blue Zinc），插件大概率只能让用户手动加载或走 Edge / Firefox 商店并去掉 YouTube 支持。没有商店背书，开源代码是让用户相信「这东西不偷数据」最便宜的办法。

法律风险是真实存在的。2020 年 10 月 23 日 RIAA 以 DMCA 规避条款让 GitHub 下架 youtube-dl，11 月 16 日才恢复。德国 Uberspace 案里，汉堡地方法院 2023 年 3 月 31 日判托管 youtube-dl 网站的服务商承担责任（310 O 316/21），汉堡高等法院 2024 年 11 月 21 日维持，联邦最高法院 2025 年 10 月驳回上诉。收钱的闭源下载器会被当成商业侵权工具看待，攻击面比开源项目大。开源项目被下架后，代码还在所有人手里。

Captae 的核心能力本身就站在开源工具肩上。yt-dlp 每隔几周就要跟着网站改版更新，靠社区维护比自己追着修要轻松得多。

## 已定：完全免费，只收打赏（2026-09-26）

维护者决定不卖安装包，全部免费，只挂 GitHub Sponsors、Ko-fi 或 Buy Me a Coffee 这类打赏入口。这样做和上面的法律分析最合拍：没有商业收入，更难被说成「靠侵权工具牟利」。要注意的是签名证书得自己出钱：macOS 公证需要 Apple 开发者账号（每年 99 美元），不签名的话用户会看到「无法验证开发者」；Windows 不签名会被 SmartScreen 拦，开源项目可以申请 SignPath 的免费签名。打赏入口放在「设置 › 关于」和 README 里就够了，不要弹窗求打赏。

下面是原先的收费分析，留作参考。

## 如果要收钱

不建议订阅。用户买的是一个本地工具，服务器成本几乎为零，订阅很难解释「每月付的钱花在哪」。同类产品的价格：Downie 19.99 美元买断；IDM 每年 11.95 美元或终身 24.95 美元；4K Video Downloader Plus 有 Lite 每年 15 美元、Personal 终身 25 美元、Pro 终身 45 美元。市场普遍接受买断，也有不少用户明确讨厌下载器订阅。

比较合适的做法是源码 MIT 公开，官网卖签好名、自动更新、自带 yt-dlp / ffmpeg 的安装包，价格 15 到 25 美元买断，一个大版本内免费升级，下个大版本老用户半价。愿意自己编译的人免费用，他们本来也不会付钱，还会帮忙提交修复。

可以另外收费的是真正有成本的东西：云端转写或翻译额度（按量付费），团队版的共享素材库。这些才适合按月收。

## 发布前要做的事

- 安装包里附上 `THIRD_PARTY_NOTICES.md` 和各 GPL 组件的许可证原文、源码地址。
- 不要在任何地方用 cobalt 的名字或图标。
- 官网写清楚用户要自己为下载内容的版权负责，并提供 DMCA 联系方式。
- 插件上架商店前去掉对 YouTube 的直接支持，或者只在自托管版本里提供。
