# Captae · 给 AI 助手的项目说明

先看 `HANDOFF.md`（交接文档）和 `docs/DESIGN.md`（设计和架构）。改界面前看 `docs/design/README.md`（视觉规范），加或改动效前看 `docs/MOTION.md`（动效规范），
加或改给人看的文字前看 `docs/I18N.md`（多语言的写法：代码里写中文原文，用 `t()` / `tr!` / `tr()` 包起来）。

## 测试和运行的规矩

- 开发版桌面端要换标识编译（`TAURI_CONFIG='{"identifier":"app.captae.dev"}' cargo build -p captae-desktop`），并且一定设 `CAPTAE_HOME=<测试目录>` 再运行，
  不然会被交给正在用的 Captae、读写正式的数据、改浏览器里插件宿主的登记。原因和调试开关见 `docs/DESIGN.md` 第 11 节。
- 测试数据目录的 `data/settings.json` 写 `"autoUpdate": false`；测「发给 AI」时设 `CAPTAE_TEST_NO_CLIPBOARD=1`，不要动系统剪贴板。
- 改了界面跑 `npx tsc --noEmit -p .` 和演示构建；改了插件工具栏面板要 `npm run build:popup`（产物 `extension/popup/` 提交进仓库）。
- 改了给人看的文字跑 `python3 scripts/i18n.py check`（英文词表必须齐）；发版前跑 `python3 scripts/i18n.py check --strict`（8 种语言都要齐）。
- README 有中文 `README.md` 和英文 `README.en.md` 两份，改一份就同步改另一份（界面上的名字照 `docs/I18N.md` 第 8 节的术语表）。
- Windows / Linux（`docs/DESIGN.md` 第 14 节）：改了 Rust 要过 `cargo check --target x86_64-pc-windows-gnu -p captae-core -p captae-cli -p captae-desktop --tests`（只查编译）和 Docker 里 Ubuntu 22.04 的 `cargo test`；
  真窗口、安装包、插件宿主这些只在真系统上看得出的，在 Devin 的 Windows / Linux 虚拟机上测，不在这台 Mac 上起测试版。打包：`scripts/build-release-windows.ps1`（Windows 上）、`scripts/build-release-linux.sh`（Ubuntu 22.04 上）。
- 依赖变了跑 `python3 scripts/gen-licenses.py` 更新 `licenses/`；打包用 `scripts/build-release.sh`（编译时去掉本机路径，最后自动运行 `scripts/check-bundle.sh`）。

## UI 库

<!-- 选型由用户决定；加库或换库先问用户。规则见 ~/.agents/skills/ui-libraries -->
Captae 开源（AGPL-3.0-or-later，许可证路线图见 `docs/LICENSING.md`），只用各库免费开源的部分。付费部分（affectUI Pro、HeroUI Pro、React Bits Pro、BoardUI Pro）只参考，代码不进仓库。
- affectUI 免费组件（MIT）：主力。按钮、菜单、弹窗、表单、分段控件、提示等
- HeroUI 开源版（`@heroui/react`，MIT）：affectUI 免费项里没有的基础件，用到时再加
- Sonner（MIT）：toast
- React Bits、BoardUI：只用开源免费的部分，用之前核对许可证
- 2026-11-23 用户选定（「都可以用，只要是开源的」）
- 手机端（`apps/ios`、`apps/android`，2026-11-24 用户选定原生 SwiftUI + Compose）：上面的 React 库用不上。iOS 只用系统控件和系统 Liquid Glass（`.glassEffect`、`.buttonStyle(.glass)`），
  不拼假玻璃；Android 用 backdrop（`io.github.kyant0:backdrop`，Apache-2.0）上自己写的玻璃组件，不用 Material 3 组件。计划在 `work/mobile/PLAN.md`
