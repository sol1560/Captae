// 工具栏面板的数据和纯逻辑：后台（extension/background.js）回的结构、每一行的清晰度选项、每一行现在是什么状态。
// 这里不碰界面，也不发消息，方便单独看懂。给人看的字（清晰度的名字、每一行的状态）在调用时按当前语言翻译。

import { N_, t, type Key } from "@/i18n";
import { bytes, clock } from "@/lib/format";

/** 和 Captae 的连接：ok 连着 / off 装了没开 / nohost 没装（找不到宿主）/ broken 装了但连接程序起不来 */
export type Conn = "ok" | "off" | "nohost" | "broken";

export type Variant = { p: string; h: number; uri: string; bytes: number; audio: boolean };
export type Quality = { label: string; height?: number | null; bytes?: number | null };
/** 下载引擎读到的信息（宿主的 probe） */
export type Probe = {
  kind?: string;
  title?: string | null;
  duration?: number | null;
  thumbnail?: string | null;
  qualities?: Quality[];
  audio?: number | null;
  size?: number | null;
  choose?: boolean;
  entries?: { id: string; title: string }[];
};
/** 这个页面能下载的一条（后台嗅探到的，或者整页交给下载引擎的「页面」那一条） */
export type Item = {
  id: string;
  src: "page" | "sniff" | "cover" | "link";
  kind: "video" | "audio" | "image";
  hls?: boolean;
  ext?: string;
  name: string;
  url: string;
  site?: string;
  size?: number;
  host?: string;
  dur?: string;
  variants?: Variant[];
  q?: boolean;
  img?: string;
  probe?: Probe;
  dims?: [number, number];
  named?: boolean;
};
/** 清晰度选择：q 整页的档位或 best / audio；vi 是 M3U8 的第几路 */
export type Choice = { q?: string; vi?: number; uri?: string; label?: string };
export type Dl = { job: number; ch: Choice };
/** 桌面端分类过的失败（crates/captae-core/src/errors.rs）；site 是说法里用的网站名字（「芒果TV」），认不出的网站没有 */
export type Fail = { code: string; message: string; raw?: string; action?: string; site?: string };
/** 录直播（宿主 jobs 回复里的 rec）：since 开始录的时刻（毫秒）；stopping 点了「停止并保存」正在收尾；until 停下的时刻；secs 录了多少秒 */
export type Rec = { since: number; until?: number | null; stopping?: boolean; secs?: number | null; max?: number | null; full?: boolean; interrupted?: boolean };
/** 宿主回的任务（只有卡片和面板要用的字段；大小是 MB） */
export type JobLite = {
  id: number;
  title: string;
  state: "queue" | "run" | "pause" | "done" | "failed";
  phase: string;
  got: number;
  size: number;
  err?: string | null;
  assetId?: number | null;
  fmt?: string;
  fail?: Fail | null;
  choose?: boolean;
  entries?: { id: string; title: string }[] | null;
  live?: boolean | null;
  rec?: Rec | null;
};
export type Settings = { inj: boolean; intro: boolean; qpref: string };
export type Page = { title: string; url: string; favIconUrl: string };
export type State = {
  conn: Conn;
  reason?: string;
  /** Captae 已经是这个版本、插件还是旧的（从文件夹重新加载也没换过来，background.js 的 followApp） */
  stale?: string;
  /** 面板开关「带上这个网站的登录状态」：site 是这个网页的主域名（不是网页时是空的） */
  login?: { site: string; on: boolean };
  items: Item[];
  dl: Record<string, Dl>;
  qual: Record<string, Choice>;
  settings: Settings;
  page: Page;
};
/** 素材库里有没有（宿主的 check） */
export type Check = { found: boolean; assetId?: number; title?: string; where?: string; dur?: string; kind?: string };

/** 类型的名字（原文），显示时 t(KIND[k]) */
export const KIND: Record<Item["kind"], Key> = { video: N_("视频"), audio: N_("声音"), image: N_("图片") };

/* ---------------- 清晰度 ---------------- */

export type Opt = { ch: Choice; label: string; size: string; rec?: boolean };
/** 「推荐」标在 1080p 及以下最高的一档（4K 太大，不默认推荐） */
const REC_MAX = 1080;
const sizeOf = (b?: number | null) => (b ? t("约{size}", { size: bytes(b) }) : "");

/** 一行能选的清晰度；只有一种的（直链、图片）返回空，行里就不放下拉 */
export function options(it: Item): Opt[] {
  if (it.hls && it.variants && it.variants.length > 1) {
    const rec = it.variants.findIndex((v) => !v.h || v.h <= REC_MAX);
    const o: Opt[] = it.variants.map((v, k) => ({ ch: { vi: k }, label: v.p, size: sizeOf(v.bytes), rec: k === rec }));
    o.push({ ch: { q: "audio" }, label: t("只要声音"), size: "" });
    return o;
  }
  if (it.src === "page") {
    const qs: Quality[] = [];
    for (const q of it.probe?.qualities ?? []) if (q.height && !qs.some((x) => x.height === q.height)) qs.push(q);
    if (!qs.length) {
      // 还没读到（或者读不到）：先给常用的几档，下载时由下载引擎挑最接近的
      return [
        { ch: { q: "best" }, label: t("最清晰"), size: "", rec: true },
        { ch: { q: "720p" }, label: "720p", size: "" },
        { ch: { q: "480p" }, label: "480p", size: "" },
        { ch: { q: "audio" }, label: t("只要声音"), size: "" },
      ];
    }
    const list = [qs[0]];
    for (const h of [1080, 720, 480]) {
      const q = qs.find((x) => (x.height ?? 0) <= h);
      if (q && !list.includes(q) && list.length < 4) list.push(q);
    }
    let rec = list.findIndex((q) => (q.height ?? 0) <= REC_MAX);
    if (rec < 0) rec = 0;
    const o: Opt[] = list.map((q, k) => ({ ch: k ? { q: `${q.height}p` } : { q: "best" }, label: q.label || `${q.height}p`, size: sizeOf(q.bytes), rec: k === rec }));
    o.push({ ch: { q: "audio" }, label: t("只要声音"), size: sizeOf(it.probe?.audio) });
    return o;
  }
  return [];
}

const same = (a?: Choice | null, b?: Choice | null) => !!a && !!b && a.q === b.q && a.vi === b.vi && a.uri === b.uri;

/** 这一行现在选的是第几档：面板里选过的 > 记住的偏好（只对整页的） > 推荐的那档 */
export function chosen(it: Item, opts: Opt[], qual: Record<string, Choice>, qpref?: string): number {
  if (!opts.length) return -1;
  const c = qual[it.id] ?? (qpref && it.src === "page" ? { q: qpref } : null);
  const k = c ? opts.findIndex((x) => same(x.ch, c)) : -1;
  if (k >= 0) return k;
  const rec = opts.findIndex((x) => x.rec);
  return rec >= 0 ? rec : 0;
}

/* ---------------- 每一行的状态 ---------------- */

export type RowView =
  | { k: "idle" }
  /** 素材库里已经有了（以前下过） */
  | { k: "got"; check: Check }
  | { k: "run"; p: number; busy: boolean; text: string }
  /** 正在录直播：「已录 12:34」，给「停止并保存」和「取消录制」（不显示进度环） */
  | { k: "rec"; job: JobLite }
  /** 录直播停下了，正在把录到的部分整理成能播放的文件 */
  | { k: "saving"; job: JobLite }
  | { k: "done"; job: JobLite }
  | { k: "failed"; job: JobLite }
  | { k: "choose"; job: JobLite };

export function rowView(it: Item, dl: Record<string, Dl>, jobs: Record<number, JobLite>, checks: Record<string, Check>): RowView {
  const d = dl[it.id];
  if (d) {
    const j = jobs[d.job];
    if (!j) return { k: "run", p: 0, busy: true, text: t("正在交给 Captae…") };
    if (j.state === "done") return { k: "done", job: j };
    if (j.state === "failed") return { k: "failed", job: j };
    if (j.choose) return { k: "choose", job: j };
    if (recording(j)) return { k: "rec", job: j };
    if (j.rec && (j.state === "run" || j.state === "queue")) return { k: "saving", job: j };
    const p = j.size > 0 ? Math.min(0.99, j.got / j.size) : 0;
    if (j.state === "queue") return { k: "run", p, busy: true, text: t("排队中") };
    if (j.state === "pause") return { k: "run", p, busy: false, text: t("已暂停") };
    if (j.phase === "probe") return { k: "run", p, busy: true, text: t("正在读取信息") };
    if (j.phase === "merge" || j.phase === "post") return { k: "run", p, busy: true, text: t("快好了，正在生成文件") };
    if (j.size > 0) return { k: "run", p, busy: false, text: t("正在下载{pct}%", { pct: Math.round(p * 100) }) };
    return { k: "run", p, busy: true, text: t("正在下载") };
  }
  const c = checks[it.id];
  if (c && c.found) return { k: "got", check: c };
  return { k: "idle" };
}

/** 正在录直播（录制的进程在跑、还没点停止）：同后端 Job::recording */
export function recording(j: JobLite): boolean {
  const r = j.rec;
  return !!r && j.state === "run" && j.phase === "dl" && r.until == null && !r.stopping;
}

/** 一行的小字（不含状态）：视频 · 14:48 / 图片 · 1920×1080 / 声音 · MP3 · 约 4 MB */
export function metaBits(it: Item): string[] {
  const b: string[] = [t(KIND[it.kind])];
  if (it.kind === "image" && it.dims) b.push(`${it.dims[0]}×${it.dims[1]}`);
  const dur = it.dur || (it.probe?.duration ? clock(it.probe.duration) : "");
  if (dur) b.push(dur);
  return b;
}

/** 这一行不带下拉时显示的大小 */
export function plainSize(it: Item): string {
  if (it.src === "page" || it.hls) return "";
  return sizeOf(it.size);
}

/** 这个网页的标题：去掉网站名的尾巴（「哔哩哔哩」是网站标题里的原文，拿来匹配，不翻译） */
export function cleanTitle(title: string): string {
  return (title || "").replace(/\s*[-|–_]\s*(YouTube|哔哩哔哩.*|bilibili.*)$/i, "").trim();
}

/** 后台失败时的错误：带宿主层面的 code（HOST_MISSING…）或桌面端的 fail */
export type PopupError = Error & { code?: string; fail?: Fail; reason?: string; login?: "sent" | "off" | "none" };
