// AI 助手页的数据和操作：agent_info（只取一次）、AI 助手的连接状态（这页开着时 10 秒看一次）、
// 总开关、高级模式、单个工具开关、导出 Skill，预设的保存、步骤检查、选文件夹、试运行。
// 试运行的进度按任务编号记（preset_step 事件可能比 preset_trial 的返回值先到），页面换来换去也不丢；试运行能取消（task_cancel）。
// 调用记录里要登录、要验证的失败：打开登录窗口（和下载页一样）。
import { useEffect, useState, useSyncExternalStore } from "react";
import { t } from "@/i18n";
import { errText, invoke } from "@/lib/bridge";
import { notify } from "@/lib/notify";
import { go } from "@/lib/route";
import { getState, onEvent, setPresets, setSettings, setState } from "@/lib/store";
import type { McpClient, Preset, TaskStatus } from "@/lib/types";
import { openSiteLogin } from "@/shell/actions";
import { normTool, shortPath, STEP_OPTS, type AgentInfo, type AssetLite, type Client, type StepResult } from "./model";

/** 一个小的外部状态：值 + 订阅，给 useSyncExternalStore 用 */
function cell<T>(v: T) {
  const subs = new Set<() => void>();
  return {
    get: () => v,
    set(next: T) {
      v = next;
      subs.forEach((f) => f());
    },
    sub(f: () => void) {
      subs.add(f);
      return () => {
        subs.delete(f);
      };
    },
  };
}

/* ---------------- agent_info ---------------- */

const infoCell = cell<AgentInfo | null>(null);
let infoLoading = false;

/** 页面要用的静态信息：拿到以前是 null，界面先用 boot 里的 mcpCommand 顶上 */
export function useAgentInfo(): AgentInfo | null {
  const v = useSyncExternalStore(infoCell.sub, infoCell.get);
  useEffect(() => {
    if (infoCell.get() || infoLoading) return;
    infoLoading = true;
    invoke<AgentInfo>("agent_info")
      .then((i) => infoCell.set(i ?? {}))
      .catch((e) => notify(t("读不到 AI 助手的设置"), errText(e)))
      .finally(() => (infoLoading = false));
  }, []);
  return v;
}

/** MCP 命令：agent_info 的为准，没拿到时用启动快照里的 */
export function mcpCommandOf(info: AgentInfo | null): string {
  const boot = getState().mcpCommand;
  return info?.mcpCommand || (typeof boot === "string" && boot ? boot : "captae");
}

/* ---------------- 连接状态 ---------------- */

const connKey = (L: McpClient[] | Client[] | null) => JSON.stringify((L ?? []).map((c) => [c.name, c.connected]));

/** 这页开着时 10 秒看一次 mcp_status（侧栏是 30 秒），AI 调用了工具 400ms 后再看一次，「几分钟前用过」才是新的。
 *  连没连上变了就顺手更新全局状态，侧栏和这页说的一样 */
export function useClients(): Client[] | null {
  const [clients, setClients] = useState<Client[] | null>(() => (getState().mcpClients as Client[] | null) ?? null);
  useEffect(() => {
    let alive = true;
    const load = () =>
      invoke<{ clients?: Client[] }>("mcp_status")
        .then((r) => {
          if (!alive) return;
          const next = r?.clients ?? [];
          setClients(next);
          if (connKey(getState().mcpClients) !== connKey(next)) setState({ mcpClients: next as McpClient[] });
        })
        .catch(() => undefined);
    load();
    const timer = setInterval(() => !document.hidden && load(), 10_000);
    let k = 0;
    const off = onEvent("agent_call", () => {
      clearTimeout(k);
      k = window.setTimeout(load, 400);
    });
    return () => {
      alive = false;
      clearInterval(timer);
      clearTimeout(k);
      off();
    };
  }, []);
  return clients;
}

/* ---------------- 设置里的几个开关 ---------------- */

export function setMcpOn(on: boolean) {
  setSettings({ mcpOn: on });
  if (on) notify(t("AI 助手可以使用 Captae 了"), t("已经连着的 AI 助手会自动刷新功能列表"));
  else notify(t("已关闭 AI 助手"), t("AI 助手现在用不了 Captae"));
}

export function setAdv(on: boolean) {
  setSettings({ adv: on });
  if (on) notify(t("高级模式已开启"), t("随时可以在这里关掉"));
  else notify(t("已回到普通模式"), t("AI 只能用安全的默认设置"));
}

/** 单独关掉一个工具：toolsOff 里存界面上的名字（captae.download），旧写法也认 */
export function setToolOn(name: string, on: boolean) {
  const off = (getState().settings?.toolsOff ?? []).filter((x) => normTool(x) !== normTool(name));
  if (!on) off.push(name);
  setSettings({ toolsOff: off });
}

export function exportSkill(info: AgentInfo | null) {
  invoke<string | null>("export_skill")
    .then((p) => p && notify(t("已导出 Skill"), shortPath(p, info)))
    .catch((e) => notify(t("没有导出"), errText(e)));
}

/* ---------------- 预设：步骤 Captae 看不看得懂 ---------------- */

/** preset_check 的结果缓存：列表里的选项都认识，别的问一次后端 */
const known = new Map<string, boolean>(STEP_OPTS.map((s) => [s, true]));
const asking = new Set<string>();
const knownCell = cell(0);

/** 返回一个函数：这一步认不认识（还没问到时是 undefined）；顺便把没问过的拿去问 */
export function useKnownSteps(steps: string[]): (s: string) => boolean | undefined {
  useSyncExternalStore(knownCell.sub, knownCell.get);
  const key = steps.join("\n");
  useEffect(() => {
    const todo = [...new Set(key.split("\n").map((s) => s.trim()))].filter((s) => s && !known.has(s) && !asking.has(s));
    if (!todo.length) return;
    todo.forEach((s) => asking.add(s));
    invoke<boolean[]>("preset_check", { preset: { name: "_", steps: todo, agent: false } })
      .then((ok) => {
        todo.forEach((s, i) => known.set(s, !!ok?.[i]));
        knownCell.set(knownCell.get() + 1);
      })
      .catch(() => undefined)
      .finally(() => todo.forEach((s) => asking.delete(s)));
  }, [key]);
  return (s: string) => known.get(s.trim());
}

/* ---------------- 预设：增删改 ---------------- */

export function savePresets(list: Preset[]) {
  return setPresets(list);
}

/** 删掉一个预设，提示条上可以撤销（放回原来的位置） */
export function removePreset(name: string) {
  const list = getState().presets;
  const k = list.findIndex((p) => p.name === name);
  if (k < 0) return;
  const gone = list[k];
  savePresets(list.filter((_, i) => i !== k));
  dropTrial(name);
  notify(t("已删除预设「{name}」", { name }), undefined, {
    label: t("撤销"),
    run: () => {
      const now = getState().presets;
      if (now.some((p) => p.name === name)) return;
      const next = now.slice();
      next.splice(Math.min(k, next.length), 0, gone);
      savePresets(next);
    },
  });
}

export function pickFolder(title: string): Promise<string | null> {
  return invoke<string | null>("pick_folder", { title }).catch((e) => {
    notify(t("没有选好文件夹"), errText(e));
    return null;
  });
}

/* ---------------- 预设：试运行 ---------------- */

/** 一个预设的试运行：task 是后端的任务编号（返回之前是 null）；stopping：点了「取消」、还没停下 */
export type Trial = { task: number | null; st: "run" | "ok" | "err"; err?: string; outputs?: number; assetId?: number | null; asset?: string; stopping?: boolean };

const trialCell = cell<Record<string, Trial>>({});
/** preset_step 事件按任务编号记 */
const stepCell = cell<Record<number, Record<number, StepResult>>>({});
/** 试运行结束的任务（返回值还没到时先记下） */
const ended = new Map<number, TaskStatus>();

export const useTrials = () => useSyncExternalStore(trialCell.sub, trialCell.get);
export const useTrialSteps = () => useSyncExternalStore(stepCell.sub, stepCell.get);

function patchTrial(name: string, p: Partial<Trial> | null) {
  const all = { ...trialCell.get() };
  if (p === null) delete all[name];
  else if (all[name]) all[name] = { ...all[name], ...p };
  else return;
  trialCell.set(all);
}

export function dropTrial(name: string) {
  patchTrial(name, null);
}

/** 预设改了名字：试运行的结果跟着走 */
export function renameTrial(from: string, to: string) {
  const all = { ...trialCell.get() };
  if (!all[from] || from === to) return;
  all[to] = all[from];
  delete all[from];
  trialCell.set(all);
}

export function startTrial(p: Preset, asset: AssetLite) {
  trialCell.set({ ...trialCell.get(), [p.name]: { task: null, st: "run", asset: asset.title } });
  invoke<{ task: number; assetId: number; title: string }>("preset_trial", { preset: p, assetId: asset.id })
    .then((r) => {
      patchTrial(p.name, { task: r.task });
      const end = ended.get(r.task);
      if (end) finish(p.name, end);
    })
    .catch((e) => {
      dropTrial(p.name);
      notify(t("试运行没有开始"), errText(e));
    });
}

/** 取消试运行：后端停下正在跑的那一步（没写完的文件删掉），task 事件 st = cancel 回来时收起结果那一行、提示一句 */
export function cancelTrial(name: string) {
  const trial = trialCell.get()[name];
  if (!trial || trial.st !== "run" || trial.task == null || trial.stopping) return;
  patchTrial(name, { stopping: true });
  invoke<boolean>("task_cancel", { id: trial.task })
    .then((ok) => {
      // 已经结束了：等 task 事件照常显示结果
      if (!ok) patchTrial(name, { stopping: false });
    })
    .catch((e) => {
      patchTrial(name, { stopping: false });
      notify(t("没能取消"), errText(e));
    });
}

function finish(name: string, task: TaskStatus) {
  ended.delete(task.id);
  const r = (task.result ?? {}) as { outputs?: unknown[]; assetId?: number };
  const n = Array.isArray(r.outputs) ? r.outputs.length : 0;
  if (task.st === "cancel") {
    dropTrial(name);
    notify(t("已取消试运行「{name}」", { name }), t("做到一半的文件已经删掉"));
    return;
  }
  if (task.st === "ok") {
    patchTrial(name, { st: "ok", outputs: n, assetId: r.assetId ?? null });
    notify(
      t("「{name}」试运行完成", { name }),
      n ? t("生成了 {n} 个文件", { n }) : t("每一步都走完了"),
      n && r.assetId != null ? { label: t("在素材库里看"), run: () => go("library", { asset: r.assetId! }) } : undefined,
    );
  } else {
    // 存下来的说法也按当时的语言（换语言以后已经显示的不变，和后端生成的文字一样）
    patchTrial(name, { st: "err", err: task.err || t("出了点问题") });
    notify(t("「{name}」试运行没有走完", { name }), task.err || t("出了点问题"));
  }
}

onEvent("preset_step", (ev) => {
  const all = stepCell.get();
  stepCell.set({ ...all, [ev.task]: { ...all[ev.task], [ev.step.index]: ev.step } });
});

onEvent("task", (ev) => {
  const task = ev.task;
  if (task.kind !== "preset" || task.st === "run") return;
  const hit = Object.entries(trialCell.get()).find(([, x]) => x.task === task.id);
  if (hit) finish(hit[0], task);
  else ended.set(task.id, task);
});

/* ---------------- 调用记录：去登录 ---------------- */

/** AI 的下载要登录 / 要验证：打开登录窗口。登录用第二阶段的 openSiteLogin；验证要打开视频那一页
 * （login_open 的 kind = verify，和下载页「去验证」一样），openSiteLogin 只发 login，这里自己发 */
export function loginForCall(a: { kind: "login" | "verify"; url: string }) {
  if (a.kind === "login") {
    void openSiteLogin({ url: a.url });
    return;
  }
  invoke("login_open", { url: a.url, kind: "verify" }).catch((e) => notify(t("没有打开登录窗口"), errText(e)));
}

/** 试运行用的素材：素材库里还在的视频和音频，最近的 40 个 */
export function loadTrialAssets(): Promise<AssetLite[]> {
  return invoke<{ items?: AssetLite[] }>("library_query", { q: { limit: 40 } }).then((r) => (r?.items ?? []).filter((a) => a.exists && a.type !== "image"));
}
