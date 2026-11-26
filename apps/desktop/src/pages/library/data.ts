// 素材库的数据：分页读取（library_query）、素材库变了就保持已加载的条数刷新一遍、按 id 记住见过的素材，
// 以及波形、胶片条、网站字幕这几样按素材缓存的东西。筛选和搜索都在后端做（SQLite + 全文检索）。
import { useCallback, useEffect, useRef, useState, useSyncExternalStore } from "react";
import { t } from "@/i18n";
import { errText, invoke } from "@/lib/bridge";
import { notify } from "@/lib/notify";
import { onEvent } from "@/lib/store";
import { PAGE, queryOf, type Asset, type Filters, type LibPage, type SubInfo } from "./model";

/* ---------------- 见过的素材（多选、详情会用到不在当前列表里的） ---------------- */

const known = new Map<number, Asset>();
export const remember = (a: Asset) => (known.set(a.id, a), a);
export const knownAsset = (id: number) => known.get(id);

/** 按 id 取一条（列表里没有时问后端），找不到返回 null */
export async function fetchAsset(id: number): Promise<Asset | null> {
  const a = await invoke<Asset | null>("library_get", { id }).catch(() => null);
  return a ? remember(a) : null;
}

/* ---------------- 刚移到废纸篓、还能撤销的（几秒后才真的删，见 act.ts） ---------------- */

let hidden = new Set<number>();
const hiddenSubs = new Set<() => void>();
export function setHidden(id: number, on: boolean) {
  const next = new Set(hidden);
  if (on) next.add(id);
  else next.delete(id);
  hidden = next;
  hiddenSubs.forEach((f) => f());
}
export function useHidden(): Set<number> {
  return useSyncExternalStore(
    (f) => {
      hiddenSubs.add(f);
      return () => {
        hiddenSubs.delete(f);
      };
    },
    () => hidden,
  );
}

/* ---------------- 分页列表 ---------------- */

export type LibState = {
  items: Asset[];
  total: number;
  all: number;
  sites: [string, number][];
  tags: [string, number][];
  /** 第一页回来了 */
  loaded: boolean;
  /** 正在要第一页（换了筛选条件） */
  resetting: boolean;
  /** 全部加载完了 */
  done: boolean;
};

const EMPTY: LibState = { items: [], total: 0, all: 0, sites: [], tags: [], loaded: false, resetting: true, done: false };

/** 素材库的一页一页：筛选条件变了从头要；滚到底附近 loadMore；素材库变了（library 事件）保持已加载的条数刷新 */
export function useLibrary(f: Filters) {
  const [st, setSt] = useState<LibState>(EMPTY);
  const live = useRef(st);
  live.current = st;
  const gen = useRef(0);
  const busy = useRef(false);
  const fRef = useRef(f);
  fRef.current = f;

  /** reset：从头要；keep：从头要，但条数和现在一样多（刷新用，不跳回第一页） */
  const load = useCallback(async (reset: boolean, keep = false) => {
    const cur = live.current;
    if (!reset && (busy.current || cur.done)) return;
    const my = reset ? ++gen.current : gen.current;
    busy.current = true;
    if (reset && !keep) setSt((s) => ({ ...s, resetting: true }));
    const offset = reset ? 0 : cur.items.length;
    const limit = reset ? Math.min(1000, Math.max(PAGE, keep ? cur.items.length : 0)) : PAGE;
    try {
      const r = await invoke<LibPage>("library_query", { q: queryOf(fRef.current, offset, limit) });
      if (my !== gen.current) return;
      r.items.forEach(remember);
      setSt((s) => {
        const items = reset ? r.items : s.items.concat(r.items.filter((a) => !s.items.some((b) => b.id === a.id)));
        return { items, total: r.total, all: r.all, sites: r.sites, tags: r.tags, loaded: true, resetting: false, done: items.length >= r.total || !r.items.length };
      });
    } catch (e) {
      if (my === gen.current) {
        setSt((s) => ({ ...s, loaded: true, resetting: false }));
        notify(t("素材库没有读出来"), errText(e));
      }
    } finally {
      if (my === gen.current) busy.current = false;
    }
  }, []);

  // 筛选条件变了：从头要
  const key = JSON.stringify(f);
  useEffect(() => {
    load(true);
  }, [key, load]);

  // 素材库变了（下载完成、转换好了、别处加了标签、AI 助手动过）：等 300ms 合并连续的变化，再保持条数刷新
  useEffect(() => {
    let t = 0;
    const off = onEvent("library", () => {
      clearTimeout(t);
      t = window.setTimeout(() => load(true, true), 300);
    });
    return () => {
      off();
      clearTimeout(t);
    };
  }, [load]);

  const loadMore = useCallback(() => load(false), [load]);
  const reload = useCallback(() => load(true, true), [load]);
  return { ...st, loadMore, reload };
}

/* ---------------- 按素材缓存：波形、胶片条、网站字幕 ---------------- */

type Cached<T> = T | "loading" | "err";
function cache<T>() {
  const m = new Map<string, Cached<T>>();
  const subs = new Set<() => void>();
  let ver = 0;
  return {
    get: (k: string) => m.get(k),
    set(k: string, v: Cached<T>) {
      m.set(k, v);
      ver++;
      subs.forEach((f) => f());
    },
    drop(prefix: string) {
      for (const k of [...m.keys()]) if (k.startsWith(prefix)) m.delete(k);
      ver++;
      subs.forEach((f) => f());
    },
    subscribe(f: () => void) {
      subs.add(f);
      return () => {
        subs.delete(f);
      };
    },
    version: () => ver,
  };
}

const waves = cache<number[]>();
const frames = cache<string[]>();
const subInfo = cache<SubInfo>();

/** 波形一条一条排队要（ffmpeg 读整段声音，同时跑很多会拖慢电脑），和旧界面一样 */
let waveQ: Promise<unknown> = Promise.resolve();

/** 一条声音的真实波形（n 个 0..1 的峰值）；还没拿到时是 null。文件不在了不要 */
export function useWave(a: Asset | null, n: number): number[] | null {
  useSyncExternalStore(waves.subscribe, waves.version);
  const k = a ? `${a.id}:${n}` : "";
  useEffect(() => {
    if (!a || !a.exists || waves.get(k)) return;
    waves.set(k, "loading");
    waveQ = waveQ
      .then(() => invoke<number[]>("library_waveform", { id: a.id, n }))
      .then((v) => waves.set(k, v))
      .catch(() => waves.set(k, "err"));
  }, [a, k, n]);
  const v = k ? waves.get(k) : undefined;
  return Array.isArray(v) ? v : null;
}

/** 视频的胶片条（均匀取的几帧小图，本地路径）；"loading" / "err" / 路径数组 */
export function useFrames(a: Asset | null, n = 10): Cached<string[]> | undefined {
  useSyncExternalStore(frames.subscribe, frames.version);
  const k = a ? `${a.id}:${n}` : "";
  useEffect(() => {
    if (!a || !a.exists || a.type !== "video" || frames.get(k)) return;
    frames.set(k, "loading");
    invoke<string[]>("library_frames", { id: a.id, n })
      .then((v) => frames.set(k, v))
      .catch(() => frames.set(k, "err"));
  }, [a, k, n]);
  return k ? frames.get(k) : undefined;
}

/** 字幕面板要知道的：网站上有哪些字幕、文件有没有声音、本地识别和嵌字幕能不能用 */
export function useSubInfo(a: Asset | null): Cached<SubInfo> | undefined {
  useSyncExternalStore(subInfo.subscribe, subInfo.version);
  const k = a ? String(a.id) : "";
  useEffect(() => {
    if (!a || subInfo.get(k)) return;
    subInfo.set(k, "loading");
    invoke<SubInfo>("subtitle_tracks", { id: a.id })
      .then((v) => subInfo.set(k, v))
      .catch((e) => subInfo.set(k, { tracks: [], err: errText(e), hasAudio: true, whisper: false, canBurn: false }));
  }, [a, k]);
  return k ? subInfo.get(k) : undefined;
}

/** 装好语音识别以后重新问一次（后端的检查结果缓存一分钟，界面先按「装好了」处理） */
export const forgetSubInfo = (id?: number) => subInfo.drop(id == null ? "" : String(id));
