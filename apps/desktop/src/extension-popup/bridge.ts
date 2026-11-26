// 面板和插件后台（extension/background.js）说话：chrome.runtime.sendMessage，带上当前标签页号。
// 在普通浏览器里打开（演示构建带 ?mock）时用 mock.ts 的假后台，方便截图看样子；演示数据不进正式构建。
import { t } from "@/i18n";
import type { PopupError } from "./model";

type Msg = { t: string; [k: string]: unknown };
type Reply = { ok: true; v: unknown } | { ok: false; e?: string; code?: string; fail?: PopupError["fail"]; reason?: string };
type Mock = { send: (m: Msg & { tabId: number | null }) => Promise<unknown>; tell: (m: unknown) => void };

let mock: Mock | null = null;
/** 面板对着的标签页 */
export let tabId: number | null = null;

export const inExtension = typeof chrome !== "undefined" && !!chrome.runtime?.id;

export async function setup(): Promise<void> {
  if (!inExtension) {
    if (import.meta.env.MODE === "mock" || import.meta.env.DEV) {
      const m = await import("./mock");
      mock = m.createMock(new URLSearchParams(location.search));
      tabId = 1;
      return;
    }
    // 这时还没定界面语言，这句只进控制台（main.tsx 的 catch），不翻译
    throw new Error("请在浏览器插件里打开这个面板");
  }
  // ?tab=<标签页号>：在普通标签页里打开面板（调试、截图用），对着指定的标签页
  const forced = Number(new URLSearchParams(location.search).get("tab"));
  if (forced) {
    tabId = forced;
    return;
  }
  const [tab] = await chrome.tabs.query({ active: true, currentWindow: true });
  tabId = tab?.id ?? null;
}

/** 发给后台；失败时错误上带着 code（HOST_MISSING…，或桌面端的错误码）和 fail */
export function send<T>(m: Msg): Promise<T> {
  const msg = { tabId, ...m };
  if (mock) return mock.send(msg) as Promise<T>;
  return chrome.runtime.sendMessage<Reply>(msg).then((r) => {
    // 这两句会显示在面板里（出错的那一行、整个面板出错时）；r.e 是 Captae 按当时的语言写好的
    if (!r) throw new Error(t("没有回应"));
    if (!r.ok) throw Object.assign(new Error(r.e || t("没办成")), { code: r.code, fail: r.fail, reason: r.reason }) as PopupError;
    return r.v as T;
  });
}

/** 发给网页里的 Captae（content.js），比如让它在网页角落显示「发给 AI」的进度 */
export function tellPage(m: unknown): Promise<unknown> {
  if (mock) {
    mock.tell(m);
    return Promise.resolve();
  }
  // 调用的地方接住以后改交给 Captae，不显示，不翻译
  if (tabId == null) return Promise.reject(new Error("没有标签页"));
  return chrome.tabs.sendMessage(tabId, m);
}

/** 关掉面板（演示时不关） */
export function closePopup() {
  if (!mock) window.close();
}
