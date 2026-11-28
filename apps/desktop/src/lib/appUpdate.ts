// Captae 自己的更新：界面这边的操作（后端 src-tauri/src/app_update.rs，做法见 docs/DESIGN.md 第 15 节）。
// 状态在 store 的 appUpdate 里（启动时取一次，之后靠 app_update 事件）。
// 「重新打开」：有正在下载、正在录的任务时先弹窗问一句（shell/AppUpdate.tsx 的 RestartDialog），没有就直接装、重新打开。
import { useSyncExternalStore } from "react";
import { t } from "@/i18n";
import { errText, invoke, openUrl } from "./bridge";
import { notify } from "./notify";
import { getState, isActive, setState } from "./store";
import type { AppUpdateStatus, Job } from "./types";

const RELEASES = "https://github.com/sol1560/Captae/releases";

/** 「0.10.0」比「0.9.2」新：按点分开逐段比数字（和插件 background.js 的 newerVersion 一样） */
export function newerVersion(a: string, b: string): boolean {
  const x = a.split(".").map((n) => parseInt(n, 10) || 0);
  const y = b.split(".").map((n) => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) > (y[i] ?? 0);
  return false;
}

/** 这一版的 Release 页（「看看改了什么」「去下载页」）；不知道版本时打开最新的 */
export function releaseUrl(version?: string | null): string {
  return version ? `${RELEASES}/tag/v${version}` : `${RELEASES}/latest`;
}

export function openRelease(version?: string | null) {
  openUrl(releaseUrl(version)).catch((e) => notify(t("没有打开网页"), errText(e)));
}

/** 装不了的原因（和后端 app_update::blocked_text 一致） */
export function blockedText(b: AppUpdateStatus["blocked"]): string {
  switch (b) {
    case "dmg":
      return t("在磁盘映像里打开的 Captae 不能自动更新，先把它拖进「应用程序」文件夹");
    case "translocated":
      return t("先把 Captae 拖进「应用程序」文件夹再打开，才能自动更新");
    case "readonly":
      return t("Captae 所在的文件夹不能写入，不能自动更新");
    case "no_pkexec":
      return t("系统里没有输入密码用的 pkexec，不能自动更新");
    case "test":
      return t("测试数据目录下不自动更新");
    default:
      return t("这个版本的 Captae 不能自动更新");
  }
}

function apply(st: AppUpdateStatus) {
  setState({ appUpdate: st }, true);
  return st;
}

/** 「检查更新」：结果用提示条说一句（有新版、自动更新开着时后台接着下载，进度在设置里） */
export async function checkUpdate() {
  try {
    const st = apply(await invoke<AppUpdateStatus>("app_update_check"));
    if (st.error?.step === "check" && st.state !== "downloading") notify(t("没有检查成"), st.error.message);
    else if (!st.latest && st.state === "idle") notify(t("已经是最新的"), `Captae ${st.current}`);
    else if (st.state === "available") notify(t("有新版本 {version}", { version: st.latest ?? "" }), st.blocked ? blockedText(st.blocked) : undefined);
  } catch (e) {
    notify(t("没有检查成"), errText(e));
  }
}

/** 下载好以后要不要马上走「重新打开」（点了「下载并更新」的那一次） */
let restartWhenReady = false;

/** 「下载并更新」：自动更新关着、或者自动下载连着失败了以后，用户自己点 */
export async function downloadUpdate() {
  restartWhenReady = true;
  try {
    const st = apply(await invoke<AppUpdateStatus>("app_update_download"));
    // 下载完成时的 app_update 事件可能已经先触发过 onUpdateStatus（那时 restartWhenReady 已经清掉），不要再问一次
    if (st.state === "ready") onUpdateStatus(st);
    else if (st.state === "available" && st.error) {
      restartWhenReady = false;
      notify(t("没有下载成"), st.error.message);
    } else if (st.state === "idle") {
      restartWhenReady = false;
      notify(t("已经是最新的"), `Captae ${st.current}`);
    }
  } catch (e) {
    restartWhenReady = false;
    notify(t("没有下载成"), errText(e));
  }
}

/** 下载状态变成 ready 时由 store 的事件处理调：点过「下载并更新」就接着重新打开 */
export function onUpdateStatus(st: AppUpdateStatus) {
  if (st.state === "ready" && restartWhenReady) {
    restartWhenReady = false;
    requestRestart();
  }
}

/* ---------------- 重新打开前的确认 ---------------- */

/** 重新打开会打断的任务：正在下载 / 排着队的、正在录的直播（暂停着的不算，重新打开后照样暂停着） */
export function interrupted(jobs: Record<number, Job> = getState().jobs) {
  let downloads = 0;
  let recordings = 0;
  for (const j of Object.values(jobs)) {
    if (!isActive(j) || j.state === "pause") continue;
    const rec = j.state === "run" && j.phase === "dl" && !!j.rec && j.rec.until == null && !j.rec.stopping;
    if (rec) recordings++;
    else downloads++;
  }
  return { downloads, recordings };
}

let asking = false;
const subs = new Set<() => void>();
export const useRestartAsk = () =>
  useSyncExternalStore(
    (f) => {
      subs.add(f);
      return () => {
        subs.delete(f);
      };
    },
    () => asking,
  );
export function setRestartAsk(v: boolean) {
  asking = v;
  subs.forEach((f) => f());
}

/** 点「重新打开」：有会被打断的任务先问一句，没有就直接装 */
export function requestRestart() {
  const { downloads, recordings } = interrupted();
  if (downloads || recordings) setRestartAsk(true);
  else void restartNow();
}

/** 装上新版、重新打开。装成了窗口会直接关掉；没装成留在原地说原因 */
export async function restartNow(): Promise<void> {
  try {
    await invoke("app_update_restart");
  } catch (e) {
    notify(t("没有更新成"), errText(e));
    throw e;
  }
}

/** 更新后第一次打开：提示一次「已更新到 x.y.z」 */
let announced = false;
export function announceUpdated(st: AppUpdateStatus | null) {
  if (announced || !st?.updatedFrom) return;
  announced = true;
  notify(t("已更新到 {version}", { version: st.current }), undefined, { label: t("看看改了什么"), run: () => openRelease(st.current) }, { duration: 8000 });
}
