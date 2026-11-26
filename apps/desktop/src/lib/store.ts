// 全局状态：启动时从后端取一次快照（boot），之后靠后端事件更新。
// 下载进度事件很密（每个任务每秒好几次），这里把同一帧里的变化合并成一次通知，列表不会被反复重画。
import { useSyncExternalStore } from "react";
import { loadLang, parseLang, setLang, loadAndSetLang, t, type Lang } from "@/i18n";
import { errText, invoke, listenEvents } from "./bridge";
import type { AddRequest, Boot, CallRecord, CaptaeEvent, DepStatus, Job, McpClient, Preset, Settings, Sniffed, TaskStatus } from "./types";
import { notify } from "./notify";

export type AppState = {
  /** 外部工具（yt-dlp、ffmpeg、aria2、语音识别……）装没装、什么版本；null = 还没查 */
  deps: DepStatus[] | null;
  /** 正在安装语音转文字（从点「安装」到装完，包括第一条进度到达之前） */
  setupBusy: boolean;
  /** 启动快照拿到了 */
  ready: boolean;
  /** 后端没有响应（启动失败） */
  offline: string | null;
  settings: Settings | null;
  jobs: Record<number, Job>;
  sniffed: Sniffed[];
  presets: Preset[];
  libraryCount: number;
  tools: Boot["tools"];
  calls: CallRecord[];
  mcpCommand: unknown;
  /** null = 还不知道 */
  mcpClients: McpClient[] | null;
  /** 后台任务（转换、字幕、安装语音识别……），按 id */
  tasks: Record<number, TaskStatus>;
  /** 界面语言（词表已经加载好的）。变了以后整个界面重新挂一次（main.tsx） */
  lang: Lang;
};

let state: AppState = {
  deps: null,
  setupBusy: false,
  ready: false,
  offline: null,
  settings: null,
  jobs: {},
  sniffed: [],
  presets: [],
  libraryCount: 0,
  tools: [],
  calls: [],
  mcpCommand: null,
  mcpClients: null,
  tasks: {},
  lang: "zh-CN",
};

const subs = new Set<() => void>();
let pending: { raf: number; timer: number } | null = null;

function flush() {
  if (!pending) return;
  cancelAnimationFrame(pending.raf);
  clearTimeout(pending.timer);
  pending = null;
  subs.forEach((f) => f());
}

/** 改状态。默认等到下一帧再通知（合并密集的进度事件）；用户操作要立刻看到结果时传 now=true */
export function setState(patch: Partial<AppState> | ((s: AppState) => Partial<AppState>), now = false) {
  state = { ...state, ...(typeof patch === "function" ? patch(state) : patch) };
  if (now) {
    pending ??= { raf: 0, timer: 0 };
    flush();
    return;
  }
  if (pending) return;
  // 窗口被挡住时 WebKit 会暂停 requestAnimationFrame，所以同时设一个 200ms 的定时器，谁先到用谁
  pending = { raf: requestAnimationFrame(flush), timer: window.setTimeout(flush, 200) };
}

export const getState = () => state;

function subscribe(f: () => void) {
  subs.add(f);
  return () => {
    subs.delete(f);
  };
}

/** 选出状态里的一部分。选择函数要返回状态里原有的对象（不要在里面 new 数组），派生数据用 useMemo 算 */
export function useApp<T>(sel: (s: AppState) => T): T {
  return useSyncExternalStore(subscribe, () => sel(state));
}

/* ---------------- 一次性事件（剪贴板、下载完成……） ---------------- */

type Handler = (ev: CaptaeEvent) => void;
const bus = new Map<string, Set<Handler>>();

export function onEvent<K extends CaptaeEvent["type"]>(type: K, fn: (ev: Extract<CaptaeEvent, { type: K }>) => void): () => void {
  const set = bus.get(type) ?? new Set<Handler>();
  bus.set(type, set);
  set.add(fn as Handler);
  return () => {
    set.delete(fn as Handler);
  };
}

function handle(ev: CaptaeEvent) {
  switch (ev.type) {
    case "job":
      setState((s) => ({ jobs: { ...s.jobs, [ev.job.id]: ev.job } }));
      break;
    case "job_removed":
      setState((s) => {
        const jobs = { ...s.jobs };
        delete jobs[ev.id];
        return { jobs };
      });
      break;
    case "settings":
      setState({ settings: ev.settings });
      switchLang(effectiveLang(ev.settings));
      break;
    case "sniffed":
      setState((s) => ({ sniffed: [ev.item, ...s.sniffed.filter((x) => x.url !== ev.item.url)].slice(0, 50) }));
      break;
    case "library":
      invoke<number>("library_count")
        .then((n) => setState({ libraryCount: n }))
        .catch(() => undefined);
      break;
    case "agent_call":
      // 同一次调用的后续（进行中 → 完成）原地更新、时间留开始的那一刻（后端每次推送都带当时的时间；和旧界面一样）
      setState((s) => {
        const k = s.calls.findIndex((c) => c.id === ev.call.id);
        if (k < 0) return { calls: [ev.call, ...s.calls].slice(0, 200) };
        const calls = s.calls.slice();
        calls[k] = { ...ev.call, at: s.calls[k].at };
        return { calls };
      });
      break;
    case "task":
      setState((s) => ({ tasks: { ...s.tasks, [ev.task.id]: ev.task } }));
      break;
  }
  bus.get(ev.type)?.forEach((f) => {
    try {
      f(ev);
    } catch (e) {
      console.error(e);
    }
  });
}

/* ---------------- 界面语言 ---------------- */

/** 后端告诉的系统语言（设置里选「跟随系统」时用） */
let systemLang: Lang | null = null;

/** 系统语言（设置里「跟随系统」那一项后面写着它） */
export const getSystemLang = (): Lang => systemLang ?? parseLang(navigator.language) ?? "en";

/** 设置里的语言，空 = 跟随系统；系统语言不支持时英文 */
export function effectiveLang(s: Settings | null): Lang {
  return parseLang(s?.language) ?? systemLang ?? parseLang(navigator.language) ?? "en";
}

let langSeq = 0;
/** 换语言：先加载词表，加载好了再换、再改 state.lang（界面跟着重画）。连着换几次时只认最后一次 */
function switchLang(l: Lang) {
  const seq = ++langSeq;
  if (l === state.lang) return;
  loadLang(l)
    .then(() => {
      if (seq !== langSeq) return;
      setLang(l);
      setState({ lang: l }, true);
    })
    .catch((e) => console.error(e));
}

/* ---------------- 启动 ---------------- */

export async function boot() {
  listenEvents(handle);
  try {
    const b = await invoke<Boot>("boot");
    systemLang = parseLang(b.systemLang);
    // 先把词表加载好再画第一屏，不先闪一下中文
    const l = parseLang(b.lang) ?? effectiveLang(b.settings);
    await loadAndSetLang(l).catch((e) => console.error(e));
    state = { ...state, lang: l };
    const jobs: Record<number, Job> = {};
    for (const j of b.jobs) jobs[j.id] = j;
    setState(
      (s) => ({
        ready: true,
        offline: null,
        settings: b.settings,
        // 启动快照到之前事件可能已经先到了，以事件里的为准
        jobs: { ...jobs, ...s.jobs },
        sniffed: b.sniffed,
        presets: b.presets,
        libraryCount: b.libraryCount,
        tools: b.tools,
        calls: b.calls,
        mcpCommand: b.mcpCommand,
      }),
      true,
    );
  } catch (e) {
    const l = effectiveLang(null);
    await loadAndSetLang(l).catch(() => undefined);
    setState({ ready: true, offline: errText(e), lang: l }, true);
  }
  pollMcp();
  setInterval(() => {
    if (!document.hidden) pollMcp();
  }, 30000);
}

/** AI 助手的连接状态：MCP 进程写的状态文件，30 秒看一次 */
export function pollMcp() {
  invoke<{ clients?: McpClient[] }>("mcp_status")
    .then((r) => {
      const next = r.clients ?? [];
      const prev = state.mcpClients;
      const key = (L: McpClient[] | null) => JSON.stringify((L ?? []).map((c) => [c.name, c.connected]));
      if (!prev || key(prev) !== key(next)) setState({ mcpClients: next });
    })
    .catch(() => undefined);
}

/** 重新查一次外部工具（打开设置、装完语音识别之后） */
export function loadDeps(): Promise<void> {
  return invoke<DepStatus[]>("deps_status")
    .then((d) => setState({ deps: d }, true))
    .catch(() => undefined);
}

/* ---------------- 常用操作 ---------------- */

/** 只发改动的字段，后端合并后返回完整设置 */
export function setSettings(patch: Partial<Settings>): Promise<void> {
  if (state.settings) setState({ settings: { ...state.settings, ...patch } as Settings }, true);
  if (patch.language !== undefined) switchLang(effectiveLang(state.settings));
  return invoke<Settings>("set_settings", { patch })
    .then((s) => setState({ settings: s }, true))
    .catch((e) => void notify(t("设置没有保存"), errText(e)));
}

export function setPresets(presets: Preset[]): Promise<void> {
  setState({ presets }, true);
  return invoke<void>("set_presets", { presets }).catch((e) => void notify(t("预设没有保存"), errText(e)));
}

/** 加一个下载任务。后端一加进来就开始跑，事件常常比返回值先到：收到过事件就以事件为准 */
export async function addJob(req: AddRequest): Promise<Job> {
  const j = await invoke<Job>("add_job", { req: { origin: "ui", ...req } });
  setState((s) => (s.jobs[j.id] ? {} : { jobs: { ...s.jobs, [j.id]: j } }), true);
  return state.jobs[j.id] ?? j;
}

export type JobAction = "toggle" | "pause" | "resume" | "now" | "up" | "down" | "cancel" | "remove" | "retry" | "segretry" | "pickgo";

/** 成功返回 true；失败时已经提示过了，返回 false */
export function jobAction(id: number, action: JobAction): Promise<boolean> {
  return invoke<void>("job_action", { id, action })
    .then(() => true)
    .catch((e) => {
      notify(t("没有完成"), errText(e));
      return false;
    });
}

export function setVariant(id: number, idx: number): Promise<void> {
  return invoke<void>("set_variant", { id, idx }).catch((e) => void notify(t("没有换成这个清晰度"), errText(e)));
}

/** 从「浏览器里发现的」拿走一条（侧栏里点了它，交给下载） */
export function takeSniffed(url: string): Sniffed | undefined {
  const s = state.sniffed.find((x) => x.url === url);
  setState((st) => ({ sniffed: st.sniffed.filter((x) => x.url !== url) }), true);
  invoke("take_sniffed", { url }).catch(() => undefined);
  return s;
}

export const isActive = (j: Job) => j.state === "run" || j.state === "queue" || j.state === "pause";
