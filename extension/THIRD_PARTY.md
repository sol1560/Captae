# 插件里用到的第三方代码和字体

Captae 浏览器插件本身是 AGPL-3.0-or-later 许可（见仓库根目录 `LICENSE`）。插件文件夹里还带着下面这些开源的东西：

| 用在哪 | 名称和版本 | 许可 | 版权 | 来源 |
|---|---|---|---|---|
| 工具栏面板 `popup/assets/popup.js`（构建产物） | React、React DOM 19.3.1，scheduler 0.28.0 | MIT | Meta Platforms, Inc. and affiliates | https://github.com/facebook/react |
| 工具栏面板 | affectUI 免费组件（Button、Icon Button、Segmented Control、Dropdown Menu、Switch、Tooltip、Spinner、Empty State），源码原样放在 `apps/desktop/src/components/ui/` | MIT | affectUI | https://www.affectui.com |
| 工具栏面板，以及网页部件的图标 `lib/kit.js`、`lib/xcard.js` | Hugeicons Free（@hugeicons/core-free-icons 4.4.0、@hugeicons/react 1.1.10） | MIT | 2025 Hugeicons | https://hugeicons.com |
| 工具栏面板（和桌面端共用的一小段桥接代码） | @tauri-apps/api 2.12.5 | MIT 或 Apache-2.0 | 2017 - Present Tauri Apps Contributors | https://github.com/tauri-apps/tauri |
| 字体 `fonts/`、`popup/assets/*.woff2` | Geist、Geist Mono | SIL Open Font License 1.1（原文 `fonts/OFL.txt`） | The Geist Project Authors（Vercel） | https://github.com/vercel/geist-font |

面板的源码在 `apps/desktop/src/extension-popup/`，用 `npm run build:popup`（在 `apps/desktop` 里）构建到 `popup/`。

## MIT 许可原文（React、affectUI、Hugeicons、@tauri-apps/api 用的都是这一份，版权一行见上表）

```
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

Geist 字体的许可全文：https://github.com/vercel/geist-font/blob/main/OFL.txt
