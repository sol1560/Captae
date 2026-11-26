// 素材库的操作：打开、在访达中显示、复制来源链接、发给 AI、标签、移除和移到废纸篓（能撤销）、
// 转换、字幕、剪辑、拼接、批量转换、批量加标签。网格的右键菜单、详情的按钮、多选工具条都调这里，行为一致。
// 每个素材每种处理的进度记在 ops 里（后端的 task 事件按 assetId 更新），详情面板和卡片都从这里画，换素材再回来不会丢。
// 正在跑的处理能取消（task_cancel）：后端停下 ffmpeg、删掉没写完的文件，不进素材库；这边进度块收起，提示一句。
import { useSyncExternalStore } from "react";
import { toast } from "sonner";
import { joinList, N_, t, type Key } from "@/i18n";
import { copyText, errText, invoke, openUrl, reveal } from "@/lib/bridge";
import { REVEAL, TRASH } from "@/lib/os";
import { notify } from "@/lib/notify";
import { go } from "@/lib/route";
import { addJob, getState, onEvent, useApp } from "@/lib/store";
import type { TaskStatus } from "@/lib/types";
import { copyWithToast } from "@/shell/actions";
import { setHidden } from "./data";
import { fileName, sourceUrl, type Asset, type NewFile, type SubResult } from "./model";

/** 「在访达中显示」这一类的说法和「废纸篓 / 回收站」按系统换，定义在 lib/os.ts（下载页、AI 助手页、插件页也用），这里接着导出 */
export { REVEAL, TRASH };
/** 「移到废纸篓」（菜单项、按钮、确认弹窗都用这一句） */
export const moveToTrash = () => t("移到{trash}", { trash: t(TRASH) });

/* ---------------- 进度 ---------------- */

export type OpKind = "convert" | "subtitle" | "trim" | "share" | "concat";
export type Op = {
  st: "queued" | "run" | "ok" | "err";
  /** 0–100 */
  p: number;
  /** 转换：目标格式 */
  fmt?: string;
  /** 字幕、剪辑：步骤名（进度落在第几步） */
  steps?: string[];
  result?: unknown;
  err?: string;
  /** 开始的时刻：完成时只给最近这一次画勾 */
  at: number;
  /** 后端的任务编号（从 task 事件认出来的）：有了它才能取消 */
  task?: number;
  /** 别处发起的任务：后端的任务标题（「拼接 3 段视频」），进度块上照着显示 */
  label?: string;
  /** 这里发起的拼接：几段视频（进度块上写「正在拼接 3 段视频」） */
  count?: number;
  /** 点了「取消」、后端还没停下 */
  stopping?: boolean;
  /** 开始时已经在跑的同类任务（别处发起的）：认自己的任务编号时跳过 */
  skip?: number[];
};

/** 后端取消任务时的错误文字（core tasks.rs CANCELLED） */
const CANCELLED = "CANCELLED";
/** run() 被取消时的返回值 */
const STOPPED = Symbol("stopped");

let ops: Record<string, Op> = {};
const opSubs = new Set<() => void>();
const opKey = (id: number, k: OpKind) => `${id}:${k}`;
function setOp(id: number, k: OpKind, op: Op | null) {
  const next = { ...ops };
  if (op) next[opKey(id, k)] = op;
  else delete next[opKey(id, k)];
  ops = next;
  opSubs.forEach((f) => f());
}
/** 别处发起的任务「正在取消」变了：ops 换一个新对象，订阅的界面才会重画 */
function bump() {
  ops = { ...ops };
  opSubs.forEach((f) => f());
}
const patchOp = (id: number, k: OpKind, patch: Partial<Op>) => {
  const cur = ops[opKey(id, k)];
  if (cur) setOp(id, k, { ...cur, ...patch });
};
export const getOp = (id: number, k: OpKind): Op | undefined => ops[opKey(id, k)];
export const clearOp = (id: number, k: OpKind) => setOp(id, k, null);

export function useOps(): Record<string, Op> {
  return useSyncExternalStore(
    (f) => {
      opSubs.add(f);
      return () => {
        opSubs.delete(f);
      };
    },
    () => ops,
  );
}
export const opOf = (all: Record<string, Op>, id: number, k: OpKind): Op | undefined => all[opKey(id, k)];

/** 后端 task 事件的 kind → 这里的种类 */
const KIND: Record<string, OpKind> = { convert: "convert", subtitle: "subtitle", trim: "trim", share: "share", concat: "concat" };

/** 这一条素材这种处理的进度：这个页面发起的（ops），或者别处发起、还在跑的后台任务（AI 助手、预设、上次打开时开始的……）。
 *  别处发起的不知道步骤，只有百分比；转换的目标格式从任务标题「xx → ProRes 422」里取 */
export function useOp(id: number, k: OpKind): Op | undefined {
  const all = useOps();
  const tasks = useApp((s) => s.tasks);
  const own = all[opKey(id, k)];
  if (own && (own.st === "run" || own.st === "queued")) return own;
  let ext: TaskStatus | undefined;
  for (const task of Object.values(tasks)) if (task.st === "run" && task.assetId === id && KIND[task.kind] === k && (!ext || task.id > ext.id)) ext = task;
  if (ext)
    return {
      st: "run",
      p: ext.pct,
      at: -ext.id,
      task: ext.id,
      label: ext.title,
      stopping: stopping.has(ext.id),
      fmt: k === "convert" ? ext.title.split("→").pop()?.trim() : undefined,
    };
  return own;
}
onEvent("task", (ev) => {
  const task = ev.task;
  // 点过「取消」的任务停下了：这时后端已经删掉没写完的文件（core tasks::Partial），提示一句。
  // 没停成（点取消的前一刻刚好做完、或者出错了）就照常显示结果，不提示
  const told = task.st !== "run" ? stopping.get(task.id) : undefined;
  if (told) {
    stopping.delete(task.id);
    bump();
    if (task.st === "cancel") told();
  }
  const k = KIND[task.kind];
  if (!k || task.assetId == null) return;
  const op = getOp(task.assetId, k);
  if (!op || op.st !== "run" || task.st !== "run") return;
  // 认自己的任务编号：开始以后第一个新出现的同类任务（开始前就在跑的是别处发起的）
  if (op.task == null && !op.skip?.includes(task.id)) patchOp(task.assetId, k, { task: task.id, p: task.pct });
  else if (op.task === task.id) patchOp(task.assetId, k, { p: task.pct });
});

/** 跑一个处理：记进度，完成或失败都更新 op，失败时返回 null；被取消时清掉 op（进度块收起），返回 STOPPED */
async function run<T>(a: Asset, k: OpKind, cmd: string, args: Record<string, unknown>, extra: Partial<Op> = {}): Promise<T | null | typeof STOPPED> {
  const skip = Object.values(getState().tasks)
    .filter((task) => task.st === "run" && task.assetId === a.id && KIND[task.kind] === k)
    .map((task) => task.id);
  setOp(a.id, k, { st: "run", p: 0, at: Date.now(), skip, ...extra });
  try {
    const r = await invoke<T>(cmd, args);
    patchOp(a.id, k, { st: "ok", p: 100, result: r, stopping: false });
    return r;
  } catch (e) {
    const msg = errText(e);
    if (msg === CANCELLED) {
      setOp(a.id, k, null);
      return STOPPED;
    }
    patchOp(a.id, k, { st: "err", err: msg, stopping: false });
    return null;
  }
}

/* ---------------- 取消 ---------------- */

/** 点了「取消」、还没停下的任务 → 停下以后（task 事件 st = cancel）要提示的那一句。别处发起的任务没有 op，也记在这里 */
const stopping = new Map<number, () => void>();

/** 取消以后的那句提示：没写完的文件后端已经删了（core tasks::Partial）。原文，显示时 t() */
const STOP_NOTE: Record<OpKind, [Key, Key | null]> = {
  convert: [N_("已取消转换"), N_("没转完的文件已经删掉")],
  subtitle: [N_("已取消生成字幕"), null],
  trim: [N_("已取消导出片段"), N_("没导出完的文件已经删掉")],
  concat: [N_("已取消拼接"), N_("没拼完的文件已经删掉")],
  share: [N_("已取消"), null],
};

/** 取消一个处理：排着队的（批量转换）直接从队里拿掉；在跑的叫后端停下（task_cancel），
 *  停下来以后发起它的那次调用返回 CANCELLED，run() 清掉 op、进度块收起；task 事件 st = cancel 到了再提示一句。
 *  已经做完的取消不了，照常显示结果 */
export function cancelOp(a: Asset, k: OpKind, op: Op) {
  if (op.st === "queued") {
    setOp(a.id, k, null);
    notify(t("已从排队里拿掉"), a.title);
    return;
  }
  const id = op.task;
  if (op.st !== "run" || id == null || stopping.has(id)) return;
  stopping.set(id, () => {
    const [title, note] = STOP_NOTE[k];
    notify(t(title), note ? `${a.title} · ${t(note)}` : a.title);
  });
  const own = getOp(a.id, k)?.task === id;
  if (own) patchOp(a.id, k, { stopping: true });
  else bump();
  const undo = () => {
    stopping.delete(id);
    if (getOp(a.id, k)?.task === id) patchOp(a.id, k, { stopping: false });
    else bump();
  };
  invoke<boolean>("task_cancel", { id })
    .then((ok) => {
      // 已经结束了：不算取消，照常显示结果
      if (!ok) undo();
    })
    .catch((e) => {
      undo();
      notify(t("没能取消"), errText(e));
    });
}

/* ---------------- 打开、显示、复制 ---------------- */

export function openFile(a: Asset) {
  invoke("open_path", { path: a.path }).catch((e) => notify(t("打不开这个文件"), errText(e)));
}
export function revealFile(a: Asset) {
  reveal(a.path).catch((e) => notify(t("打不开文件夹"), errText(e)));
}
export function copyLink(a: Asset) {
  const u = sourceUrl(a);
  if (u) copyWithToast(u, t("来源链接已复制"));
}
export function openSource(a: Asset) {
  const u = sourceUrl(a);
  if (u) openUrl(u).catch((e) => notify(t("打不开网页"), errText(e)));
}
export function copyPath(a: Asset) {
  copyText(a.path)
    .then(() => notify(t("文件路径已复制"), fileName(a.path)))
    .catch((e) => notify(t("没有复制"), errText(e)));
}
/** 文件不在了：从来源再下载一次 */
export function redownload(a: Asset) {
  const u = sourceUrl(a);
  if (!u) return;
  addJob({ url: u, origin: "ui" })
    .then((j) => {
      notify(t("已加入下载"), a.title, { label: t("去看看"), run: () => go("downloads", { t: j.id }) });
    })
    .catch((e) => notify(t("没有开始下载"), errText(e)));
}

/** 去看新出来的素材（转换、剪辑、拼接的结果） */
export const showAsset = (id: number) => go("library", { asset: id });

/* ---------------- 发给 AI ---------------- */

export async function shareToAI(a: Asset) {
  if (getOp(a.id, "share")?.st === "run") return;
  const r = await run(a, "share", "library_share", { id: a.id });
  if (r === STOPPED) return;
  if (r) notify(t("已复制，可以直接粘贴给 AI"), t("文字稿和关键帧都在剪贴板里了"));
  else notify(t("没能发给 AI"), getOp(a.id, "share")?.err);
}

/* ---------------- 标签 ---------------- */

export function addTag(a: Asset, tag: string): Promise<string[] | null> {
  const name = tag.trim();
  if (!name || a.tags.includes(name)) return Promise.resolve(null);
  return invoke<string[]>("library_tags", { id: a.id, add: name }).catch((e) => {
    notify(t("标签没有加上"), errText(e));
    return null;
  });
}
export function removeTag(a: Asset, tag: string): Promise<string[] | null> {
  return invoke<string[]>("library_tags", { id: a.id, remove: tag }).catch((e) => {
    notify(t("标签没有去掉"), errText(e));
    return null;
  });
}

/** 多选后加同一个标签 */
export async function tagMany(L: Asset[], tag: string): Promise<boolean> {
  const name = tag.trim();
  if (!name) return false;
  const r = await Promise.all(L.filter((a) => !a.tags.includes(name)).map((a) => invoke("library_tags", { id: a.id, add: name }).then(() => true, () => false)));
  const bad = r.filter((x) => !x).length;
  if (bad) notify(t("有{n}项没加上标签", { n: bad }), t("「{tag}」", { tag: name }));
  else notify(t("已加上标签「{tag}」", { tag: name }), t("{n}项素材", { n: L.length }));
  return !bad;
}

/* ---------------- 移除、移到废纸篓：先藏起来，提示条上能撤销，6 秒后才真的做 ---------------- */
/* 后端没有「从废纸篓放回」的命令，所以撤销靠「晚一点再删」：提示条消失之前都能撤销；
 * 窗口在这期间关了，文件和记录都还在（宁可没删，也不误删） */

const UNDO_MS = 6000;
const waiting = new Map<number, { timer: number; doIt: () => Promise<unknown> }>();

function later(a: Asset, title: string, doIt: () => Promise<unknown>, failTitle: string, onUndo?: () => void) {
  setHidden(a.id, true);
  toast(title, {
    description: a.title,
    duration: UNDO_MS,
    action: {
      label: t("撤销"),
      onClick: () => {
        const w = waiting.get(a.id);
        if (w) clearTimeout(w.timer);
        waiting.delete(a.id);
        setHidden(a.id, false);
        onUndo?.();
      },
    },
  });
  const timer = window.setTimeout(() => {
    waiting.delete(a.id);
    doIt()
      .catch((e) => {
        setHidden(a.id, false);
        notify(failTitle, errText(e));
      })
      .finally(() => {
        // 后端会发 library 事件，列表刷新后这一条自己就没了；这里只是不再藏着
        window.setTimeout(() => setHidden(a.id, false), 1500);
      });
  }, UNDO_MS);
  waiting.set(a.id, { timer, doIt });
}

/** 移到废纸篓（确认之后调） */
export function trash(a: Asset, onUndo?: () => void) {
  const trashName = t(TRASH);
  later(a, t("已移到{trash}", { trash: trashName }), () => invoke("library_trash", { id: a.id }), t("没有移到{trash}", { trash: trashName }), onUndo);
}
/** 只从素材库里去掉记录，文件留在原处（文件已经不在了时也用它） */
export function removeFromLibrary(a: Asset, onUndo?: () => void) {
  later(a, t("已从素材库移除"), () => invoke("library_remove", { id: a.id }), t("没有移除"), onUndo);
}

/** 关窗口前：还在等的移除马上做掉（来得及的话） */
addEventListener("pagehide", () => {
  for (const w of waiting.values()) {
    clearTimeout(w.timer);
    w.doIt().catch(() => undefined);
  }
  waiting.clear();
});

/* ---------------- 转换 ---------------- */

export async function convert(a: Asset, fmt: string, height: number | null, quality: string | null) {
  const r = await run<NewFile>(a, "convert", "task_convert", { id: a.id, format: fmt, height, quality }, { fmt });
  if (r === STOPPED) return;
  if (r) notify(t("转换好了"), t("{title} → {fmt} · 已放进素材库", { title: a.title, fmt }), { label: t("查看"), run: () => showAsset(r.assetId) });
  else notify(t("转换没有完成"), getOp(a.id, "convert")?.err);
}

/** 多选后批量转成 MP4：一个接一个转（同时转很多会把电脑拖慢），卡片上显示排队和进度 */
export async function convertMany(L: Asset[], skipped: string) {
  if (!L.length) {
    notify(t("没有需要转换的视频"), skipped);
    return;
  }
  notify(t("开始转换{n}个视频", { n: L.length }), skipped || t("会一个接一个转完"));
  for (const a of L) setOp(a.id, "convert", { st: "queued", p: 0, at: Date.now(), fmt: "MP4" });
  let ok = 0;
  let stopped = 0;
  const bad: string[] = [];
  for (const a of L) {
    // 排队时被拿掉了（取消）：跳过
    if (getOp(a.id, "convert")?.st !== "queued") {
      stopped++;
      continue;
    }
    const r = await run<NewFile>(a, "convert", "task_convert", { id: a.id, format: "MP4" }, { fmt: "MP4" });
    if (r === STOPPED) stopped++;
    else if (r) ok++;
    else bad.push(a.title);
  }
  // 几种结果各写一整句（不拼「，取消了 n 个」这样的半句）
  if (bad.length)
    notify(
      stopped ? t("转好了{ok}个，{bad}个没有完成，取消了{stopped}个", { ok, bad: bad.length, stopped }) : t("转好了{ok}个，{bad}个没有完成", { ok, bad: bad.length }),
      joinList(bad.slice(0, 3)),
    );
  else if (!ok) notify(t("这批转换都取消了"), t("{n}个视频", { n: stopped }));
  else notify(stopped ? t("转好了{ok}个，取消了{stopped}个", { ok, stopped }) : t("都转成 MP4 了"), t("{n}个视频 · 已放进素材库", { n: ok }));
}

/* ---------------- 字幕 ---------------- */

export type SubOpts = { source: "platform" | "whisper"; language: string | null; format: string; translate_to: string | null };

export async function subtitle(a: Asset, opts: SubOpts, burn: boolean, steps: string[]) {
  const r = await run<SubResult>(a, "subtitle", "task_subtitle", { id: a.id, opts, burn }, { steps });
  if (r === STOPPED) return;
  if (r)
    notify(
      t("字幕导出好了"),
      r.burned ? t("{file} · 嵌好字幕的视频也放进素材库了", { file: fileName(r.path) }) : fileName(r.path),
      r.burnedId != null ? { label: t("查看"), run: () => showAsset(r.burnedId!) } : { label: t(REVEAL), run: () => reveal(r.path).catch(() => undefined) },
    );
  else notify(t("字幕没有生成"), getOp(a.id, "subtitle")?.err);
}

/* ---------------- 剪辑 ---------------- */

export type TrimOpts = { start: number; end: number; mute: boolean; precise: boolean; replace_audio: string | null };

export async function trim(a: Asset, opts: TrimOpts, steps: string[], note: string) {
  const r = await run<NewFile>(a, "trim", "task_trim", { id: a.id, opts }, { steps });
  if (r === STOPPED) return;
  if (r) notify(t("片段导出好了"), note, { label: t("查看"), run: () => showAsset(r.assetId) });
  else notify(t("片段没有导出"), getOp(a.id, "trim")?.err);
}

/* ---------------- 拼接 ---------------- */

export async function concat(L: Asset[]) {
  if (L.length < 2) return;
  notify(t("开始拼接"), t("{n}段视频 · 好了会放进素材库", { n: L.length }));
  const r = await run<NewFile>(L[0], "concat", "task_concat", { ids: L.map((a) => a.id) }, { count: L.length });
  if (r === STOPPED) return;
  if (r) notify(t("拼接好了"), t("已经放进素材库"), { label: t("查看"), run: () => showAsset(r.assetId) });
  else notify(t("拼接没有完成"), getOp(L[0].id, "concat")?.err);
}
