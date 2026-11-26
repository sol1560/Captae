// 和后端说话的唯一入口：调用 Tauri 命令、收事件、把本地文件变成能在 <img> 里用的地址。
// 在浏览器里打开（不是 Captae 里）时，只有网址带 ?mock 才会用演示数据，方便截图和检查界面；
// 演示数据在 src/dev/，Captae 里永远不会加载它。
import { convertFileSrc, invoke as tauriInvoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { t } from "@/i18n";
import type { CaptaeEvent } from "./types";

export const native = isTauri();
export type Platform = "mac" | "win" | "linux";
const detected: Platform = /Mac/.test(navigator.platform) ? "mac" : /Win/.test(navigator.platform) ? "win" : "linux";
/** 在浏览器里看演示数据时可以用 `&os=win` / `&os=linux` / `&os=mac` 换成别的系统（窗口按钮、说法、快捷键写法），截图核对用；Captae 里永远按真实的系统 */
const forced = native ? null : new URLSearchParams(location.search).get("os");
export const platform: Platform = forced === "mac" || forced === "win" || forced === "linux" ? forced : detected;
// 样式里按系统分的地方（窗口按钮、标题条）认 <html data-os>
document.documentElement.dataset.os = platform;

type Mock = {
  invoke: (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
  listen: (fn: (ev: CaptaeEvent) => void) => () => void;
};
let mock: Mock | null = null;

/** 浏览器里带 ?mock 打开时装上演示后端。
 *  只有 vite 开发服务器和 `npm run build:mock` 的产物里有演示后端；正式构建时这段整个被去掉，src/dev/ 不会打进安装包 */
export async function setupMock(): Promise<boolean> {
  if (!(import.meta.env.DEV || import.meta.env.MODE === "mock")) return false;
  if (native || !new URLSearchParams(location.search).has("mock")) return false;
  const m = await import("../dev/mock");
  mock = m.createMock();
  return true;
}

export function invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (native) return tauriInvoke<T>(cmd, args);
  if (mock) return mock.invoke(cmd, args) as Promise<T>;
  return Promise.reject(new Error(t("请在 Captae 里打开这个页面")));
}

/** 后端广播的事件（任务进度、设置、嗅探、剪贴板……），返回取消订阅的函数 */
export function listenEvents(fn: (ev: CaptaeEvent) => void): () => void {
  if (native) {
    let off: (() => void) | null = null;
    let dead = false;
    listen<CaptaeEvent>("captae://event", (e) => fn(e.payload)).then((u) => {
      if (dead) u();
      else off = u;
    });
    return () => {
      dead = true;
      off?.();
    };
  }
  if (mock) return mock.listen(fn);
  return () => undefined;
}

/** 本地文件路径 → <img>/<video> 能用的地址；网址原样返回 */
export function fileSrc(p?: string | null): string {
  if (!p) return "";
  if (/^(https?:|data:|blob:)/.test(p)) return p;
  return native ? convertFileSrc(p) : p;
}

/** 页面里出错时写进 Captae 的日志，方便排查用户反馈 */
export function reportError(msg: string) {
  if (native) tauriInvoke("js_log", { level: "error", msg: msg.slice(0, 2000) }).catch(() => undefined);
  else console.error(msg);
}

if (native) {
  addEventListener("error", (e) => reportError(`${e.message} @ ${(e.filename || "").split("/").pop()}:${e.lineno}`));
  addEventListener("unhandledrejection", (e) => reportError("promise: " + ((e.reason && (e.reason.stack || e.reason)) ?? "")));
}

/** 复制文字到系统剪贴板 */
export function copyText(text: string): Promise<void> {
  if (native) return tauriInvoke("copy_text", { text });
  return navigator.clipboard ? navigator.clipboard.writeText(text) : Promise.reject(new Error(t("剪贴板不可用")));
}

/** 用系统浏览器打开网址 */
export function openUrl(url: string): Promise<void> {
  if (native) return tauriInvoke("open_url", { url });
  window.open(url, "_blank", "noopener");
  return Promise.resolve();
}

/** 在文件管理器（访达、文件资源管理器）里显示文件 */
export function reveal(path?: string | null): Promise<void> {
  if (!path) return Promise.reject(new Error(t("文件还没有生成")));
  return invoke("reveal", { path });
}

/** 关闭、最小化、最大化 / 还原窗口（Windows / Linux 的主窗口没有系统标题栏，窗口按钮由界面画，见 shell/WindowControls.tsx） */
export function windowCtl(action: "close" | "min" | "max") {
  invoke("window_ctl", { action }).catch(() => undefined);
}

/** 错误转成一句话 */
export function errText(e: unknown): string {
  if (e instanceof Error) return e.message;
  return String(e);
}
