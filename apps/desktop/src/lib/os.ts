// 按系统换的说法和快捷键（Windows、Linux 版加的，2026-11-24）。
// 系统自己的叫法照系统，不自己起名（docs/I18N.md 8.3）：
// - 文件管理器：macOS「访达」，Windows「文件资源管理器」（Windows 10、11 的叫法），Linux「文件管理器」（GNOME 叫「文件」、KDE 叫 Dolphin，统称文件管理器）
// - 回收站：macOS「废纸篓」（Trash），Windows「回收站」（Recycle Bin），Linux 的 GNOME、KDE 中文界面也叫「回收站」，英文是 Trash
// - 用户文件夹：macOS「影片」「文稿」，Windows、Linux（XDG 的英文目录名）「视频」「文档」
// - 快捷键：macOS ⌘K，Windows、Linux Ctrl+K
// 原文都用 N_ 标出来，显示时 t()（模块顶层不能直接调 t()，词表那时还没加载）
import { N_, type Key } from "@/i18n";
import { platform } from "./bridge";

export const isMac = platform === "mac";

/** 快捷键的写法：macOS「⌘K」「⇧⌘K」，Windows、Linux「Ctrl+K」「Ctrl+Shift+K」 */
export function shortcut(key: string, opts?: { shift?: boolean }): string {
  if (isMac) return `${opts?.shift ? "⇧" : ""}⌘${key}`;
  return `Ctrl+${opts?.shift ? "Shift+" : ""}${key}`;
}

/** 快捷键的修饰键按下了没有：macOS 认 ⌘（Ctrl 也认，和原来一样），Windows、Linux 只认 Ctrl（Win 键 / Super 键是系统的，不抢） */
export function modKey(e: { metaKey: boolean; ctrlKey: boolean }): boolean {
  return isMac ? e.metaKey || e.ctrlKey : e.ctrlKey && !e.metaKey;
}

/** 「在访达中显示」这一类（菜单项、按钮） */
export const REVEAL: Key = platform === "mac" ? N_("在访达中显示") : platform === "win" ? N_("在文件资源管理器中显示") : N_("在文件管理器中显示");

/** 废纸篓 / 回收站，放进句子用占位符：「移到{trash}」 */
export const TRASH: Key = platform === "mac" ? N_("废纸篓") : platform === "win" ? N_("回收站") : N_("回收站§GNOME、KDE 里的叫法（英文是 Trash）");

/** 默认的保存位置「影片 › Captae」：Windows、Linux 的用户文件夹叫「视频」 */
export const MOVIES_CAPTAE: Key = isMac ? N_("影片 › Captae") : N_("视频 › Captae");
