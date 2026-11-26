# 截图

新界面（第二版设计）的演示版截图：界面代码和正式版是同一份，数据来自演示后端（`apps/desktop/src/dev/mock*.ts`），不是真实的下载记录。
2026-11-24 拍的，窗口 1440×900（插件面板 372 宽），2 倍图，按「减少动效」渲染（截下来的是动效走完的样子）。

| 文件 | 画面 | 演示版地址 |
|---|---|---|
| `downloads.png` | 下载页：读取中、选清晰度带倒计时、下载中、排队、暂停，右边是详情浮卡 | `index.html?mock&still#/downloads` |
| `empty.png` | 第一次打开：居中的 logo、输入框和几个可以直接试的链接 | `index.html?mock=empty&still` |
| `library.jpg` | 素材库：网格、类型分段、搜索，右边是素材详情和处理（转换、字幕、剪辑） | `index.html?mock&still#/library` |
| `settings.png` | 设置：下载、已登录的网站 | `index.html?mock&still&open=settings` |
| `onboarding.jpg` | 上手引导第 1 页 | `index.html?mock&still&open=onboarding` |
| `agent.png` | AI 助手页：连接状态、一键写入配置、调用记录、预设 | `index.html?mock&still#/agent` |
| `extension.png` | 浏览器插件页：各家浏览器的登记和连接状态 | `index.html?mock&still#/extension` |
| `popup.png` | 浏览器插件的工具栏面板 | 插件面板的演示版 `index.html?mock&still&scene=design` |

怎么重拍：在 `apps/desktop` 里 `npx vite build --mode mock --outDir <目录>`（插件面板是 `npx vite build -c vite.popup.config.ts --mode mock`），
用无界面的 Chrome 打开上面的地址截图（`--force-prefers-reduced-motion`，等 3–4 秒让演示数据就位）。网址参数见 `apps/desktop/src/dev/mock.ts` 开头。

画面里的视频封面是 Blender Foundation 开放电影的剧照（*Big Buck Bunny*、*Sintel*、*Tears of Steel*、*Elephants Dream*），
许可 CC BY 3.0（*Elephants Dream* 是 CC BY 2.5），出处见 `apps/desktop/src/dev/img/CREDITS.md`。
