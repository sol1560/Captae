// 演示数据：Captae 自己的更新（结构照 src-tauri/src/app_update.rs 的 Status）。
// 网址带 &upd=<状态> 看各种样子：idle（默认，已是最新）/ available（有新版、自动更新关着）/ downloading / ready / failed（上次没装成）/
// dmg（在磁盘映像里打开的）/ deb（Linux 的 deb 版，要输密码）/ updated（刚更新完的第一次打开）
import type { AppUpdateStatus } from "@/lib/types";
import type { MockHandler } from "./mock";

const q = new URLSearchParams(location.search);
const MODE = q.get("upd") ?? "idle";
const NEXT = "0.3.0";
const MIN = 60_000;

function initial(auto: boolean): AppUpdateStatus {
  const base: AppUpdateStatus = {
    current: MODE === "updated" ? NEXT : "0.2.0",
    state: "idle",
    latest: null,
    progress: null,
    lastCheck: Date.now() - 47 * MIN,
    error: null,
    blocked: null,
    kind: MODE === "deb" ? "deb" : "app",
    updatedFrom: MODE === "updated" ? "0.2.0" : null,
    installOnQuit: MODE !== "deb",
    auto,
  };
  switch (MODE) {
    case "available":
      return { ...base, state: "available", latest: NEXT, auto: false };
    case "downloading":
      return { ...base, state: "downloading", latest: NEXT, progress: 0.45 };
    case "ready":
    case "deb":
      return { ...base, state: "ready", latest: NEXT };
    case "failed":
      return { ...base, state: "ready", latest: NEXT, error: { step: "install", message: "没有权限写入「应用程序」文件夹", at: Date.now() - 5 * MIN } };
    case "dmg":
      return { ...base, state: "available", latest: NEXT, blocked: "dmg" };
    default:
      return base;
  }
}

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

export const updateHandlers: Record<string, MockHandler> = (() => {
  let st: AppUpdateStatus | null = null;
  const cur = (auto: boolean) => (st ??= initial(auto));
  const set = (patch: Partial<AppUpdateStatus>, emit: (ev: { type: "app_update"; status: AppUpdateStatus }) => void) => {
    st = { ...st!, ...patch };
    emit({ type: "app_update", status: st });
    return st;
  };
  // 下载：每 250 毫秒推一次进度，2 秒下完
  const download = async (emit: (ev: { type: "app_update"; status: AppUpdateStatus }) => void, still: boolean) => {
    set({ state: "downloading", latest: NEXT, progress: 0 }, emit);
    for (let i = 1; i <= 8 && !still; i++) {
      await sleep(250);
      set({ progress: i / 8 }, emit);
    }
    return set({ state: "ready", progress: null, error: null }, emit);
  };
  return {
    app_update_status: (_a, ctx) => ({ ...cur(ctx.state.settings.appAutoUpdate ?? true), auto: ctx.state.settings.appAutoUpdate ?? true }),
    app_update_check: async (_a, ctx) => {
      const s = cur(ctx.state.settings.appAutoUpdate ?? true);
      const emit = ctx.emit as never;
      set({ state: "checking" }, emit);
      await sleep(ctx.still ? 0 : 900);
      if (s.blocked) return set({ state: "available", latest: NEXT, lastCheck: Date.now() }, emit);
      if (MODE === "idle" || MODE === "updated") return set({ state: "idle", latest: null, lastCheck: Date.now() }, emit);
      if (!(ctx.state.settings.appAutoUpdate ?? true)) return set({ state: "available", latest: NEXT, lastCheck: Date.now() }, emit);
      return download(emit, ctx.still);
    },
    app_update_download: async (_a, ctx) => {
      cur(ctx.state.settings.appAutoUpdate ?? true);
      return download(ctx.emit as never, ctx.still);
    },
    app_update_restart: async () => {
      await sleep(800);
      throw new Error("演示版不能重新打开");
    },
  };
})();
