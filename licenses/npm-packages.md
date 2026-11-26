# 打进界面的 npm 依赖

由 `scripts/gen-licenses.py` 生成（不要手改）：`apps/desktop/package-lock.json` 里不是开发依赖的包，
也就是 `package.json` 的 `dependencies` 连同它们带进来的。桌面端界面（嵌在 Captae 程序里）和插件的工具栏面板
（`extension/popup/`）都用这些。开发依赖（TypeScript、Vite、vite-plugin-singlefile 等）只在构建时用，不进安装包，不列。
许可证是「A OR B」的按 MIT 用。

| 包 | 版本 | 许可证 | 按 | 来源 | 版权 |
|---|---|---|---|---|---|
| @hugeicons/core-free-icons | 4.4.0 | MIT | MIT | https://github.com/hugeicons/hugeicons | Copyright (c) 2025 Hugeicons |
| @hugeicons/react | 1.1.10 | MIT | MIT | https://github.com/hugeicons/hugeicons | Copyright (c) 2025 Hugeicons |
| @tauri-apps/api | 2.11.1 | Apache-2.0 OR MIT | MIT | https://github.com/tauri-apps/tauri | Copyright (c) 2017 - Present Tauri Apps Contributors |
| framer-motion | 14.14.0 | MIT | MIT | https://github.com/motiondivision/motion/ | Copyright (c) 2018 Framer B.V. |
| lenis | 1.3.17 | MIT | MIT | https://github.com/darkroomengineering/lenis | Copyright (c) 2024 darkroom.engineering |
| motion | 14.14.0 | MIT | MIT | https://github.com/motiondivision/motion | Copyright (c) 2024 [Motion](https://motion.dev) B.V. |
| motion-dom | 14.14.0 | MIT | MIT | https://github.com/motiondivision/motion | Copyright (c) 2024 [Motion](https://motion.dev) B.V. |
| motion-utils | 14.7.0 | MIT | MIT | https://github.com/motiondivision/motion | Copyright (c) 2024 [Motion](https://motion.dev) B.V. |
| react | 19.3.1 | MIT | MIT | https://github.com/react/react | Copyright (c) Meta Platforms, Inc. and affiliates. |
| react-dom | 19.3.1 | MIT | MIT | https://github.com/react/react | Copyright (c) Meta Platforms, Inc. and affiliates. |
| scheduler | 0.28.0 | MIT | MIT | https://github.com/react/react | Copyright (c) Meta Platforms, Inc. and affiliates. |
| sonner | 2.0.8 | MIT | MIT | https://github.com/emilkowalski/sonner | Copyright (c) 2023 Emil Kowalski |
| tslib | 2.9.1 | 0BSD | 0BSD | https://github.com/Microsoft/tslib | Copyright (c) Microsoft Corporation. |
| uqr | 0.1.3 | MIT | MIT | https://github.com/unjs/uqr | Copyright (c) Project Nayuki；Copyright (c) 2023 Anthony Fu <https://github.com/antfu> |

## MIT 许可原文

（版权行见上表）
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

## 0BSD 许可原文


```
Copyright (c) Microsoft Corporation.

Permission to use, copy, modify, and/or distribute this software for any
purpose with or without fee is hereby granted.

THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH
REGARD TO THIS SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY
AND FITNESS. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT,
INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY DAMAGES WHATSOEVER RESULTING FROM
LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF CONTRACT, NEGLIGENCE OR
OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE OR
PERFORMANCE OF THIS SOFTWARE.
```
