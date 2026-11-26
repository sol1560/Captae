// 上手引导和侧栏上手清单共用的状态。存在界面小偏好里（lib/prefs.ts），值的形状和旧界面一样，老用户搬过来的不会丢：
//   getPref("onboarded")  引导看过了（走完、跳过、关掉都算）
//   getPref("ob")         清单 {items: {dl, convert, ext, agent}, hidden}
// 清单打勾只看真实结果：下载完成（finished 事件）、转换完成（task 事件 kind=convert、st=ok）、
// 插件连上（extension_status().connected）、AI 助手连上过（mcp_status 里有 lastSeen）。
import { useEffect, useSyncExternalStore } from "react";
import { N_, t, type Key } from "@/i18n";
import { errText, invoke } from "@/lib/bridge";
import { notify } from "@/lib/notify";
import { getPref, setPref } from "@/lib/prefs";
import { onEvent, useApp } from "@/lib/store";
import { getDialog, openDialog } from "@/shell/actions";

export type ItemKey = "dl" | "convert" | "ext" | "agent";
export type Ob = { items: Partial<Record<ItemKey, boolean>>; hidden: boolean };

/** 清单的四项，顺序和旧界面一样。label 是原文，显示时 t(label) */
export const ITEMS: { k: ItemKey; label: Key }[] = [
  { k: "dl", label: N_("下载第一个视频") },
  { k: "convert", label: N_("转换一次格式") },
  { k: "ext", label: N_("装好浏览器插件") },
  { k: "agent", label: N_("连接 AI 助手") },
];

/** 浏览器插件的连接状态（ext_cmds.rs 的 extension_status：tasks.rs 的 ExtStatus + host） */
export type ExtStatus = { connected: boolean; lastSeen?: number | null; version?: string | null; browser?: string | null; host?: unknown };

type ObState = {
  ob: Ob;
  /** 四项刚刚全部完成：清单先停一下，让最后一个勾画完，再提示、再收起 */
  finishing: boolean;
  /** extension_status 的结果；null = 还没查到 */
  ext: ExtStatus | null;
  /** 点了「安装插件」以后在等它连上 */
  extWait: boolean;
  /** 打开扩展程序页面没成功的原因 */
  extErr: string;
};

/** 旧值可能缺字段（或者被手动改坏了），读出来先整理成固定形状 */
function normalize(v: unknown): Ob {
  const o = (v && typeof v === "object" ? v : {}) as Partial<Ob>;
  const items = o.items && typeof o.items === "object" ? o.items : {};
  return { items: { ...items }, hidden: !!o.hidden };
}

let st: ObState = { ob: normalize(getPref("ob", null)), finishing: false, ext: null, extWait: false, extErr: "" };
const subs = new Set<() => void>();

function set(patch: Partial<ObState>) {
  st = { ...st, ...patch };
  subs.forEach((f) => f());
}
function setOb(ob: Ob) {
  setPref("ob", ob);
  set({ ob });
}
function subscribe(f: () => void) {
  subs.add(f);
  return () => {
    subs.delete(f);
  };
}

/** 选出状态里的一部分（选择函数返回状态里原有的值，不要在里面新建对象） */
export function useOb<T>(sel: (s: ObState) => T): T {
  return useSyncExternalStore(subscribe, () => sel(st));
}

export const doneCount = (ob: Ob) => ITEMS.filter((i) => ob.items[i.k]).length;

/** 清单打勾。四项都完成时：等最后一个勾画完再提示一次，清单随后收起 */
export function markDone(k: ItemKey) {
  if (st.ob.items[k]) return;
  const ob = { ...st.ob, items: { ...st.ob.items, [k]: true } };
  const all = doneCount(ob) === ITEMS.length;
  setOb(ob);
  if (!all) return;
  set({ finishing: true });
  // 画圈 250ms + 打勾 200ms（晚 150ms 开始）约 400ms 画完，再停一会儿让人看到「4/4」
  window.setTimeout(() => {
    set({ finishing: false });
    notify(t("上手清单全部完成"), t("清单已收起。想再看一遍，在设置的「帮助」里点「重新查看」"));
  }, 1200);
}

/** 关掉侧栏的清单（提示条里可以撤销） */
export function hideChecklist() {
  setOb({ ...st.ob, hidden: true });
  notify(t("已关闭上手清单"), t("想再看的话，在设置的「帮助」里点「重新查看」"), {
    label: t("撤销"),
    run: () => setOb({ ...st.ob, hidden: false }),
  });
}

/** 设置里的「重新查看上手引导」：清掉「看过了」和清单的「关掉了」，打开引导（会顺手关掉设置） */
export function restartOnboarding() {
  setPref("onboarded", false);
  setOb({ ...st.ob, hidden: false });
  openDialog("onboarding");
}

/** 引导走完、跳过、关掉都算看过了 */
export function markOnboarded() {
  setPref("onboarded", true);
}

/* ---------------- 浏览器插件 ---------------- */

/** 查一次插件连没连上；连上了顺手给清单打勾 */
export async function loadExt(): Promise<ExtStatus | null> {
  try {
    const x = await invoke<ExtStatus>("extension_status");
    set({ ext: x });
    if (x?.connected) markDone("ext");
    return x;
  } catch {
    return null;
  }
}

let waitTimer = 0;
let waitStart = 0;
function stopWait() {
  window.clearInterval(waitTimer);
  waitTimer = 0;
  set({ extWait: false });
}

/** 「安装插件」：插件还没上架商店，打开浏览器的扩展程序页面并在访达里显示插件文件夹，用户照着「加载已解压的扩展程序」装；
 *  之后每 2 秒看一次连上没有，最多等 10 分钟（和旧界面一样）。关掉引导以后也接着等，连上时提示一下 */
export function installExtension(browser: string) {
  set({ extErr: "", extWait: true });
  if (!waitTimer) {
    waitStart = Date.now();
    waitTimer = window.setInterval(async () => {
      const x = await loadExt();
      if (x?.connected) {
        stopWait();
        if (getDialog() !== "onboarding") notify(t("浏览器插件已连上"), t("在网页上看视频时，能下载的会出现在左边"));
      } else if (Date.now() - waitStart > 10 * 60_000) stopWait();
    }, 2000);
  }
  invoke("extension_install_help", { browser }).catch((e) => {
    stopWait();
    set({ extErr: errText(e) });
  });
}

/* ---------------- 自动打勾 ---------------- */

/** 侧栏清单挂着的时候一直听着（清单关掉了也照样记，重新打开时进度还在） */
export function useAutoTick() {
  const clients = useApp((s) => s.mcpClients);
  const extDone = useOb((s) => !!s.ob.items.ext);

  useEffect(() => onEvent("finished", () => markDone("dl")), []);
  useEffect(
    () =>
      onEvent("task", (ev) => {
        if (ev.task.kind === "convert" && ev.task.st === "ok") markDone("convert");
      }),
    [],
  );
  // AI 助手：store 已经 30 秒看一次 mcp_status，这里只看结果；连过一次（有 lastSeen）就算
  useEffect(() => {
    if (clients?.some((c) => c.connected || c.lastSeen)) markDone("agent");
  }, [clients]);
  // 插件：没打勾之前 15 秒看一次（窗口在后台时不看）
  useEffect(() => {
    if (extDone) return;
    loadExt();
    const timer = window.setInterval(() => !document.hidden && loadExt(), 15_000);
    return () => window.clearInterval(timer);
  }, [extDone]);
}
