// 能传给 t() 的原文 = 英文词表各分册里的键合在一起。tsc 据此核对 t("…") 里的原文在不在词表里。
// 分册：common（窗口骨架、侧栏、lib、parts 这些各页共用的）、downloads、settings、library、extension（浏览器插件页）、agent（AI 助手页）、
// onboarding（上手引导）、popup（插件工具栏面板）。加分册时这里加一行，scripts/i18n.py 也会自动认得（按文件找）。
import type agent from "./locales/en/agent.json";
import type common from "./locales/en/common.json";
import type downloads from "./locales/en/downloads.json";
import type extension from "./locales/en/extension.json";
import type library from "./locales/en/library.json";
import type onboarding from "./locales/en/onboarding.json";
import type popup from "./locales/en/popup.json";
import type settings from "./locales/en/settings.json";

export type Key =
  | keyof typeof agent
  | keyof typeof common
  | keyof typeof downloads
  | keyof typeof extension
  | keyof typeof library
  | keyof typeof onboarding
  | keyof typeof popup
  | keyof typeof settings;
