// 几个页面都会用到的操作：下载浏览器里发现的资源、打开设置、登录窗口……放在这里，页面之间不互相依赖。
import { useSyncExternalStore } from "react";
import { t, tAny } from "@/i18n";
import { copyText, errText, invoke } from "@/lib/bridge";
import { notify } from "@/lib/notify";
import { go } from "@/lib/route";
import { addJob, getState, isActive, onEvent, takeSniffed } from "@/lib/store";
import type { Job } from "@/lib/types";

/** 排在它前面、还没下完的任务有几个（手动开始的不排队，不算） */
export function queueAhead(j: Job, jobs: Record<number, Job> = getState().jobs): number {
  if (j.state !== "queue" || j.boost) return 0;
  return Object.values(jobs).filter((x) => x.id !== j.id && isActive(x) && !x.choose && !x.boost && x.state !== "pause" && x.pri < j.pri).length;
}

/** 侧栏「浏览器里发现的」点一条：交给下载，带上插件记下的来源页面和请求头 */
export async function downloadSniffed(url: string) {
  const s = takeSniffed(url);
  if (!s) return;
  try {
    const j = await addJob({ url: s.url, from_sniff: true, origin: "extension", page_url: s.page_url || null, page_title: s.page_title || null, headers: s.headers || {} });
    go("downloads", { t: j.id });
    const ahead = queueAhead(j);
    notify(t("已加入下载"), ahead ? t("前面还有{n}个，想先下它可以点「马上开始」", { n: ahead }) : s.t);
  } catch (e) {
    notify(t("没有开始下载"), errText(e));
  }
}

/** 弹窗：设置、上手引导。同一时间只开一个 */
export type DialogName = "settings" | "onboarding" | null;
let dialog: DialogName = null;
const subs = new Set<() => void>();
export const getDialog = () => dialog;
export function subscribeDialog(f: () => void) {
  subs.add(f);
  return () => {
    subs.delete(f);
  };
}
export function openDialog(d: DialogName) {
  dialog = d;
  subs.forEach((f) => f());
}

/** 进行中的任务，按优先级排 */
export function activeJobs() {
  return Object.values(getState().jobs)
    .filter(isActive)
    .sort((a, b) => a.pri - b.pri);
}

/** 复制，成功和失败都给一句提示 */
export function copyWithToast(text: string, title: string, desc?: string) {
  copyText(text)
    .then(() => notify(title, desc))
    .catch((e) => notify(t("没有复制"), errText(e)));
}

/* ---------------- 登录和人机验证（第二阶段，后端见 src-tauri/src/login.rs） ----------------
 *   login_open({ url, jobId, kind: "login" | "verify" })  打开 Captae 的登录窗口；登录成功或验证通过后，后端自己重试等着这个网站的任务
 *   login_open({ site }) / login_open({ url })            从设置里主动登录某个网站
 *   login_close({ jobId })                                  用户在主窗口点了「取消」
 * 后端发 {type: "login", state: open | done | closed, jobs}：开始等 / 登录好了 / 窗口关了。
 * 窗口开着的这段时间，等着的任务（同一个网站上要登录的都算）显示「正在等你在弹出的窗口里登录」。任务一离开 failed（重试了）就不再等。 */

let waiting: Record<number, "login" | "verify"> = {};
const waitSubs = new Set<() => void>();
const setWaiting = (w: Record<number, "login" | "verify">) => {
  waiting = w;
  waitSubs.forEach((f) => f());
};

export function useLoginWait() {
  return useSyncExternalStore(
    (f) => {
      waitSubs.add(f);
      return () => {
        waitSubs.delete(f);
      };
    },
    () => waiting,
  );
}

export function openLogin(job: Job, kind: "login" | "verify") {
  setWaiting({ ...waiting, [job.id]: kind });
  invoke("login_open", { url: job.url, jobId: job.id, kind }).catch((e) => {
    const { [job.id]: _gone, ...rest } = waiting;
    setWaiting(rest);
    notify(t("没有打开登录窗口"), errText(e));
  });
}

export function cancelLogin(id: number) {
  const { [id]: _gone, ...rest } = waiting;
  setWaiting(rest);
  invoke("login_close", { jobId: id }).catch(() => undefined);
}

/** 从设置里主动登录一个网站（site 是 login_sites 给的代号），或者打开一个网址登录 */
export function openSiteLogin(target: { site: string } | { url: string }): Promise<boolean> {
  return invoke("login_open", { ...target, kind: "login" }).then(
    () => true,
    (e) => {
      notify(t("没有打开登录窗口"), errText(e));
      return false;
    },
  );
}

onEvent("login", (ev) => {
  if (ev.state === "open") {
    const next = { ...waiting };
    for (const id of ev.jobs) next[id] = ev.kind;
    setWaiting(next);
    return;
  }
  const next = { ...waiting };
  for (const id of ev.jobs) delete next[id];
  setWaiting(next);
  if (ev.state === "done") {
    // ev.name 是后端存的网站名（中文原文），显示时翻译
    const what = ev.how === "login" ? t("已登录{site}", { site: tAny(ev.name) }) : t("验证好了");
    notify(what, ev.retried > 0 ? t("{n}个任务接着下载", { n: ev.retried }) : t("以后下载这个网站的视频会带上登录状态"));
  }
});

/** 只提示一次的事（比如第一次用 YouTube 账号下载），停留久一点 */
onEvent("notice", (ev) => {
  notify(ev.title, ev.text, undefined, { duration: 12000, id: ev.key });
});

onEvent("job", (ev) => {
  if (waiting[ev.job.id] && ev.job.state !== "failed") cancelWaitOnly(ev.job.id);
});
onEvent("job_removed", (ev) => {
  if (waiting[ev.id]) cancelWaitOnly(ev.id);
});
function cancelWaitOnly(id: number) {
  const { [id]: _gone, ...rest } = waiting;
  setWaiting(rest);
}

/* ---------------- 播放列表、多视频帖子（第一阶段） ---------------- */

/** 选好的条目各自生成一个下载任务（choose_entries：ids 是 entries[].id，["all"] 表示全部；原来那个等选择的任务会去掉） */
export function pickEntries(id: number, ids: string[]): Promise<void> {
  return invoke<unknown>("choose_entries", { id, ids }).then(() => undefined, (e) => void notify(t("没有开始下载"), errText(e)));
}
