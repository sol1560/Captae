// 浏览器插件页的纯逻辑：后端返回的结构、各家浏览器的扩展程序页地址、连接状态的说法、测试结果。
// 和旧界面（ui/js/page-extension.js）逐条对照；浏览器列表以 core native_host.rs 的 browsers() 为准（第一阶段加到 16 个）。
import { FileVideoIcon, Image01Icon, MusicNote01Icon, PlayListIcon } from "@hugeicons/core-free-icons";
import { joinList, t } from "@/i18n";
import { whenLabel } from "@/lib/format";
import type { Sniffed } from "@/lib/types";
import type { HugeIcon, Step } from "@/parts";

/* ---------------- 后端返回的结构 ---------------- */

/** 插件最近一次打招呼（core tasks.rs ExtStatus）：两分钟内见过就算连着。
 *  browser 是 16 家浏览器表里的 id：宿主看父进程认出来的（native_host.rs detect_browser），认不出时是插件自己报的 */
export type ExtSeen = { connected: boolean; lastSeen: number | null; version: string | null; browser: string | null };

/** 一个浏览器的宿主清单（core native_host.rs BrowserStatus） */
export type BrowserStatus = { id: string; name: string; installed: boolean; registered: boolean; manifest: string; also?: string[] };

/** 宿主注册情况（native_host.rs Status）；找不到 captae 命令行程序时只有 {ok:false, missing:true} */
export type HostStatus = { ok: boolean; missing?: boolean; exe?: string; wrapper?: string; wrapperOk?: boolean; browsers?: BrowserStatus[] };

/** extension_status：插件的招呼 + 宿主 */
export type ExtStatus = ExtSeen & { host?: HostStatus };

/** extension_test：像浏览器一样启动宿主发一条 hello（reply 是宿主的回复：Captae 开着没有、版本） */
export type ExtTest = { host: HostStatus; reply: { app: boolean; version?: string; clipboard?: boolean } | null; error: string | null; extension?: ExtSeen };

/* ---------------- 各家浏览器 ---------------- */

export type BrowserInfo = { id: string; name: string; page: string; help: boolean };

/** 扩展程序页的地址；help：extension_install_help 认识它（能直接用这个浏览器打开扩展程序页）。
 *  后端和插件宿主共用一张浏览器表，这 16 家都认；打不开（没装、浏览器不让外面打开内部页面）时退回复制地址 + 在访达里显示插件文件夹 */
export const BROWSERS: BrowserInfo[] = [
  { id: "chrome", name: "Chrome", page: "chrome://extensions", help: true },
  { id: "edge", name: "Edge", page: "edge://extensions", help: true },
  { id: "brave", name: "Brave", page: "brave://extensions", help: true },
  { id: "arc", name: "Arc", page: "arc://extensions", help: true },
  { id: "vivaldi", name: "Vivaldi", page: "vivaldi://extensions", help: true },
  { id: "opera", name: "Opera", page: "opera://extensions", help: true },
  { id: "opera-gx", name: "Opera GX", page: "opera://extensions", help: true },
  { id: "aside", name: "Aside", page: "chrome://extensions", help: true },
  { id: "chromium", name: "Chromium", page: "chrome://extensions", help: true },
  { id: "chrome-beta", name: "Chrome Beta", page: "chrome://extensions", help: true },
  { id: "chrome-dev", name: "Chrome Dev", page: "chrome://extensions", help: true },
  { id: "chrome-canary", name: "Chrome Canary", page: "chrome://extensions", help: true },
  { id: "chrome-for-testing", name: "Chrome for Testing", page: "chrome://extensions", help: true },
  { id: "edge-beta", name: "Edge Beta", page: "edge://extensions", help: true },
  { id: "edge-dev", name: "Edge Dev", page: "edge://extensions", help: true },
  { id: "edge-canary", name: "Edge Canary", page: "edge://extensions", help: true },
];

export const browserOf = (id?: string | null): BrowserInfo => BROWSERS.find((b) => b.id === id) ?? { id: id ?? "chrome", name: id || "Chrome", page: "chrome://extensions", help: true };

/** 插件是从哪家浏览器连上来的（ExtSeen.browser，宿主认出来的 id：arc / aside / chrome-beta……）→ 名字；上手引导也用它 */
export const seenBrowser = (b?: string | null) => (b ? browserOf(b).name : "");

/** 选浏览器的下拉：装了的排前面 */
export function browserOptions(host: HostStatus | null): { value: string; label: string; description?: string }[] {
  const inst = new Set((host?.browsers ?? []).filter((b) => b.installed).map((b) => b.id));
  const list = BROWSERS.slice().sort((a, b) => Number(inst.has(b.id)) - Number(inst.has(a.id)));
  return list.map((b) => ({ value: b.id, label: b.name, description: inst.has(b.id) ? t("这台电脑上有") : undefined }));
}

/* ---------------- 连接状态 ---------------- */
// 「上次见到是 3 分钟前」「刚刚打过招呼」「今天 09:47 打过招呼」：时间（lib/format 的 whenLabel）放进整句模板，
// 时间和汉字之间的空格由自动空格补上（数字开头、结尾时空一格）

/** 顶栏那一句 */
export function extLine(st: ExtStatus | null): { text: string; live: boolean } {
  if (!st) return { text: t("正在看看插件在不在…"), live: false };
  if (st.connected) return { text: st.browser ? t("已连上 · {browser}", { browser: seenBrowser(st.browser) }) : t("已连上"), live: true };
  if (st.lastSeen) return { text: t("没连上 · 上次见到是{when}", { when: whenLabel(st.lastSeen) }), live: false };
  return { text: t("还没连上"), live: false };
}

/** 状态卡片的标题和说明 */
export function extCard(st: ExtStatus | null): { title: string; desc: string } {
  if (!st) return { title: t("正在看看插件在不在…"), desc: "" };
  if (st.connected) {
    return {
      title: t("插件已连上"),
      desc: [seenBrowser(st.browser), st.version ? t("版本 {version}", { version: st.version }) : "", t("{when}打过招呼", { when: whenLabel(st.lastSeen) })].filter(Boolean).join(" · "),
    };
  }
  if (st.lastSeen) return { title: t("插件还没连上"), desc: t("上次见到是{when}，浏览器关着或者插件被停用了", { when: whenLabel(st.lastSeen) }) };
  return { title: t("插件还没连上"), desc: t("装好插件、打开浏览器以后，这里会变成「已连上」") };
}

/** 测试结果：三步，哪一步没通过就说哪一步。fix：第一步没通过、又能修的时候放「修复」按钮。
 *  后端只有宿主都登记好了才会真的叫一次 Captae（ext_cmds.rs extension_test），所以第一步没过时第二步算「还没试」，不算失败 */
export function testSteps(test: ExtTest, st: ExtStatus | null): { steps: Step[]; fix: boolean } {
  const host = test.host ?? { ok: false };
  const ext = test.extension ?? st;
  const inst = (host.browsers ?? []).filter((b) => b.installed);
  const reg = (host.browsers ?? []).filter((b) => b.registered).map((b) => b.name);
  const miss = inst.filter((b) => !b.registered).map((b) => b.name);
  let first: Step;
  if (host.ok) first = { label: t("已经告诉浏览器怎么找到 Captae"), sub: joinList(reg) || t("浏览器§测试结果：登记好的浏览器说不出名字时"), s: "done", fresh: true };
  else if (host.missing) first = { label: t("浏览器还不知道怎么找到 Captae"), sub: t("没找到 Captae 自带的 captae 程序，重新安装 Captae 试试"), s: "err" };
  else if (host.wrapperOk === false) first = { label: t("登记的不是现在这个 Captae"), sub: t("点「修复」重新登记一次"), s: "err" };
  else if (reg.length && miss.length) first = { label: t("有的浏览器还没登记"), sub: t("还没登记：{missing}；{done} 已经登记了", { missing: joinList(miss), done: joinList(reg) }), s: "err" };
  else first = { label: t("浏览器还不知道怎么找到 Captae"), sub: t("点「修复」登记一次"), s: "err" };
  const second: Step = !host.ok
    ? { label: t("叫一次 Captae"), sub: t("登记好以后再测一次"), s: "todo" }
    : test.reply?.app
      ? { label: t("插件能叫到 Captae"), sub: test.reply.version ? t("Captae 版本 {version}", { version: test.reply.version }) : undefined, s: "done", fresh: true }
      : { label: t("插件叫不到 Captae"), sub: test.error || t("宿主没有回应"), s: "err" };
  const third: Step = ext?.connected
    ? { label: t("插件在线"), sub: t("{when}打过招呼", { when: whenLabel(ext.lastSeen) }), s: "done", fresh: true }
    : { label: t("还没收到插件的招呼"), sub: t("按下面的步骤装好插件，浏览器开着时一分钟内就会连上"), s: "todo" };
  return { steps: [first, second, third], fix: !host.ok && !host.missing };
}

/** 测试完弹的那句 */
export function testToast(test: ExtTest): [string, string] {
  const on = !!test.extension?.connected;
  if (test.host?.ok && test.reply?.app) return on ? [t("一切正常"), t("插件和 Captae 已经连上了")] : [t("Captae 这边准备好了"), t("装好插件、打开浏览器，一分钟内就会连上")];
  if (on && !test.host?.missing) return [t("有浏览器还没登记"), t("点「修复」登记一下，别的浏览器里的插件也能用")];
  return [t("还没连上"), t("看看上面哪一步没通过")];
}

/* ---------------- 浏览器里发现的 ---------------- */

export type Kind = "all" | "video" | "audio" | "image";

/** 嗅探条目的种类：film / list（M3U8）算视频，music 算声音，image 算图片 */
export const kindOf = (s: Sniffed): Exclude<Kind, "all"> => (s.i === "music" ? "audio" : s.i === "image" ? "image" : "video");

const SNIFF_ICON: Record<string, HugeIcon> = { list: PlayListIcon, film: FileVideoIcon, music: MusicNote01Icon, image: Image01Icon };
export const sniffIcon = (s: Sniffed): HugeIcon => SNIFF_ICON[s.i] ?? FileVideoIcon;
