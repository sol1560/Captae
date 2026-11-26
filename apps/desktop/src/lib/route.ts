// 页面切换：单页应用，用网址的 # 记当前页面，比如 #/library?asset=5、#/downloads?paste。
// 后端（浏览器插件、命令行要求打开某个页面）通过 window.captaeGo("/library?asset=5") 跳过来。
import { useSyncExternalStore } from "react";

export type Page = "downloads" | "library" | "agent" | "extension";
export const PAGES: Page[] = ["downloads", "library", "agent", "extension"];
export type Route = { page: Page; params: URLSearchParams };

function parse(): Route {
  const h = location.hash.replace(/^#\/?/, "");
  const [p, q] = h.split("?");
  const page = (PAGES as string[]).includes(p) ? (p as Page) : "downloads";
  return { page, params: new URLSearchParams(q ?? "") };
}

let current = parse();
const subs = new Set<() => void>();
addEventListener("hashchange", () => {
  current = parse();
  subs.forEach((f) => f());
});

export function useRoute(): Route {
  return useSyncExternalStore(
    (f) => {
      subs.add(f);
      return () => {
        subs.delete(f);
      };
    },
    () => current,
  );
}

/** 换页面。params 里值为 true 的写成没有值的参数（?paste） */
export function go(page: Page, params?: Record<string, string | number | boolean>) {
  const q = params
    ? Object.entries(params)
        .filter(([, v]) => v !== false)
        .map(([k, v]) => (v === true ? encodeURIComponent(k) : `${encodeURIComponent(k)}=${encodeURIComponent(String(v))}`))
        .join("&")
    : "";
  const next = `#/${page}${q ? "?" + q : ""}`;
  if (location.hash === next) {
    // 同一个地址再跳一次（比如插件连着要求「粘贴」两次）也要让页面知道
    current = parse();
    subs.forEach((f) => f());
  } else location.hash = next;
}

/** 页面用完一次性参数（paste、asset、t）后清掉，免得刷新时又执行一次 */
export function clearParams() {
  history.replaceState(null, "", `#/${current.page}`);
  current = { page: current.page, params: new URLSearchParams() };
  subs.forEach((f) => f());
}

declare global {
  interface Window {
    captaeGo?: (route: string) => void;
  }
}
window.captaeGo = (r: string) => {
  const [p, q] = r.replace(/^\/+/, "").split("?");
  const page = (PAGES as string[]).includes(p) ? (p as Page) : "downloads";
  const params: Record<string, string | boolean> = {};
  new URLSearchParams(q ?? "").forEach((v, k) => (params[k] = v === "" ? true : v));
  go(page, params);
};
