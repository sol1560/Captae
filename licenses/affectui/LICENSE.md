# affectUI 免费组件

`apps/desktop/src/components/ui/` 里的 36 个组件来自 affectUI 的公开 registry（`https://www.affectui.com/r/<名字>.json`，不带密钥就能取到的免费项），
源码原样没改，取的时间和来源记在那个目录里的 `AFFECTUI-SOURCES.json`：

avatar、badge、button、card、checkbox、code-block、context-menu、dialog、dropdown-menu、empty-state、filter-menu、form-field、hover-card、icon-button、kbd、notification-stack、number-input、number-ticker、popover、progress-tracker、scroll-area、segmented-control、select、separator、shimmer-loader、skeleton、spinner、switch、tag-input、text-area、text-field、thinking-loader、toggle-group、tool-call-card、tooltip、tree-view

affectUI 官网写明免费组件是 MIT 许可（「74 items are free and open source (MIT)」，见 https://www.affectui.com/llms.txt ）。
组件文件里没有许可证头，下面附上 MIT 全文。桌面端界面和插件的工具栏面板（`extension/popup/`）都用到这些组件。
组件里内嵌的图标来自 Hugeicons Free（MIT，© 2025 Hugeicons），文件里有署名注释，许可证见 `../hugeicons/LICENSE.md`。

affectUI Pro（付费部分）的代码不在仓库里，也不在安装包里。

```
MIT License

Copyright (c) affectUI (https://www.affectui.com)

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
