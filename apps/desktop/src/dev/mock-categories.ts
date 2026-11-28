// 演示后端：素材库的分类和「用 AI 整理」（后端是 apps/desktop/src-tauri/src/category_cmds.rs、crates/captae-core/src/categorize.rs）。
// 返回结构照真实后端写。演示里没有真的问模型：整理按标题里的词分，进度一步步走（&still 时一下子走完）。
//
// 网址参数（和 mock.ts 的一起用）：
//   &llm    设置里已经填好 AI 模型（DeepSeek，能看图）
//   &cats   素材库已经整理过：有几个分类、AI 标签，侧栏有「分类」
//   &orgfail 整理会失败（看出错的样子）
// 打开整理弹窗用真实的跳转地址：#/library?org=organize，管理分类 #/library?org=manage
import type { Category, Settings } from "@/lib/types";
import type { Asset } from "@/pages/library/model";
import type { MockCtx, MockHandler } from "./mock";
import { libOf } from "./mock-library";

const qs = new URLSearchParams(location.search);
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

type C = { id: number; name: string; desc: string; sort: number; by: string };
let cats: C[] = [];
let catSeq = 0;
/** 素材 id → 分类（null = 未分类）和谁放的 */
const filed = new Map<number, { cat: number | null; by: string }>();
const aiTags = new Map<number, string[]>();
let ready = false;

/** 按标题、类型猜一个分类（演示用的「模型」） */
function guess(a: Asset): { key: string; tags: string[] } | null {
  const t = a.title;
  if (a.type === "image") return { key: "img", tags: ["海报"] };
  if (/播客|podcast|访谈/i.test(t)) return { key: "talk", tags: ["访谈"] };
  if (a.type === "audio" || /原声|配乐|music|OST/i.test(t)) return { key: "music", tags: ["配乐", /Sintel/.test(t) ? "Sintel" : "Blender"] };
  if (/Sintel|Tears|Big Buck|Elephants|Spring|Cosmos|Agent 327|Charge|Wing It/i.test(t)) {
    const film = (t.match(/Sintel|Tears of Steel|Big Buck Bunny|Elephants Dream|Spring|Cosmos Laundromat|Agent 327|Charge|Wing It/i) ?? ["Blender"])[0];
    return { key: "film", tags: ["Blender", film] };
  }
  return null;
}

const PROPOSAL_CATS: { key: string; name: string; desc: string }[] = [
  { key: "n1", name: "开放电影", desc: "Blender 基金会的开放电影、预告和片段" },
  { key: "n2", name: "配乐原声", desc: "电影原声、配乐和音乐" },
  { key: "n3", name: "访谈播客", desc: "访谈、播客和讲座" },
  { key: "n4", name: "海报剧照", desc: "海报、剧照和截图" },
];
const KEY_OF: Record<string, string> = { film: "n1", music: "n2", talk: "n3", img: "n4" };

function init(ctx: MockCtx) {
  if (ready) return;
  ready = true;
  if (!qs.has("cats")) return;
  // 已经整理过：建好分类、把素材放进去
  const ids = new Map<string, number>();
  for (const c of PROPOSAL_CATS) {
    const id = ++catSeq;
    cats.push({ id, name: c.name, desc: c.desc, sort: id, by: "ai" });
    ids.set(c.key, id);
  }
  for (const a of libOf(ctx).values()) {
    const g = guess(a);
    filed.set(a.id, { cat: g ? (ids.get(KEY_OF[g.key]) ?? null) : null, by: "ai" });
    if (g) aiTags.set(a.id, g.tags);
  }
}

/** 给界面的一份素材：带上分类和 AI 标签（tags 里也有 AI 的，ai_tags 记着是哪几个） */
export function decorate(a: Asset): Asset {
  const f = filed.get(a.id);
  const ai = (aiTags.get(a.id) ?? []).filter((t) => !a.tags.includes(t));
  return { ...a, tags: [...a.tags, ...ai], ai_tags: ai, category_id: f?.cat ?? null, category_by: f?.by ?? "" };
}

/** library_query 的 cat：0 = 未分类 */
export function catFilter(id: number, cat: number): boolean {
  const c = filed.get(id)?.cat ?? null;
  return cat === 0 ? c == null : c === cat;
}

function list(ctx: MockCtx): { items: Category[]; none: number } {
  init(ctx);
  const all = [...libOf(ctx).keys()];
  const items = [...cats].sort((a, b) => a.sort - b.sort).map((c) => ({ ...c, count: all.filter((id) => filed.get(id)?.cat === c.id).length }));
  return { items, none: all.filter((id) => (filed.get(id)?.cat ?? null) == null).length };
}

function settingsOf(ctx: MockCtx): Settings {
  return ctx.state.settings as Settings;
}

/** 整理出来、等确认的 */
let pending: { assign: Map<number, { key: string | null; tags: string[] }> } | null = null;
let taskSeq = 9000;

export const categoryHandlers: Record<string, MockHandler> = {
  library_categories: (_a, ctx) => list(ctx),
  category_save: ({ id, name, desc }, ctx) => {
    init(ctx);
    const n = String(name).trim();
    if (!n) throw new Error("分类名不能是空的");
    const clash = cats.find((c) => c.name.toLowerCase() === n.toLowerCase() && c.id !== id);
    if (id != null) {
      if (clash) throw new Error(`已经有叫「${n}」的分类了`);
      const c = cats.find((x) => x.id === id);
      if (!c) throw new Error("这个分类已经不在了");
      if (c.name !== n) Object.assign(c, { name: n, by: "user" });
      if (desc != null) c.desc = desc;
      ctx.emit({ type: "library" });
      return c.id;
    }
    if (clash) return clash.id;
    const c = { id: ++catSeq, name: n, desc: desc ?? "", sort: catSeq, by: "user" };
    cats.push(c);
    ctx.emit({ type: "library" });
    return c.id;
  },
  category_delete: ({ id }, ctx) => {
    cats = cats.filter((c) => c.id !== id);
    for (const [k, f] of filed) if (f.cat === id) filed.set(k, { ...f, cat: null });
    ctx.emit({ type: "library" });
  },
  category_merge: ({ from, into }, ctx) => {
    cats = cats.filter((c) => c.id !== from);
    for (const [k, f] of filed) if (f.cat === from) filed.set(k, { ...f, cat: into });
    ctx.emit({ type: "library" });
  },
  category_reorder: ({ ids }, ctx) => {
    (ids as number[]).forEach((id, i) => {
      const c = cats.find((x) => x.id === id);
      if (c) c.sort = i + 1;
    });
    ctx.emit({ type: "library" });
  },
  asset_set_category: ({ ids, cat }, ctx) => {
    for (const id of ids as number[]) filed.set(id, { cat: cat ?? null, by: "user" });
    ctx.emit({ type: "library" });
  },
  categorize_estimate: (_a, ctx) => {
    init(ctx);
    const s = settingsOf(ctx);
    const items = [...libOf(ctx).values()].filter((a) => filed.get(a.id)?.by !== "user");
    const covers = items.filter((a) => a.desc.length < 40 && a.thumb).length;
    const host = s.llmBaseUrl ? s.llmBaseUrl.replace(/^https?:\/\//, "").split("/")[0] : "";
    return {
      hasModel: !!(s.llmBaseUrl && s.llmModel),
      host,
      model: s.llmModel,
      vision: s.llmVision ?? "",
      items: items.length,
      covers: s.aiSendCovers === false || s.llmVision === "no" ? 0 : covers,
      tokens: items.length * 200 + Math.min(items.length, 400) * 150 + covers * 350 + 2000,
      existing: cats.length,
      userFiled: [...filed.values()].filter((f) => f.by === "user").length,
    };
  },
  categorize_propose: async (_a, ctx) => {
    init(ctx);
    const id = ++taskSeq;
    const send = (pct: number, note: string, st: "run" | "ok" | "err" | "cancel" = "run", err?: string) => {
      const t = { id, kind: "categorize", title: "用 AI 整理素材库", pct, st, note, err };
      ctx.state.tasks.set(id, t);
      ctx.emit({ type: "task", task: t });
    };
    const items = [...libOf(ctx).values()].filter((a) => filed.get(a.id)?.by !== "user");
    const thin = items.filter((a) => a.desc.length < 40 && a.thumb);
    const cancelled = () => ctx.state.tasks.get(id)?.st === "cancel";
    const step = ctx.still ? 0 : 1;
    // 看封面
    for (let d = 0; d <= thin.length; d += 6) {
      send((30 * Math.min(d, thin.length)) / Math.max(1, thin.length), `正在看封面 ${Math.min(d, thin.length)} / ${thin.length}`);
      await sleep(step * 320);
      if (cancelled()) throw new Error("CANCELLED");
    }
    send(30, "正在想怎么分");
    await sleep(step * 1800);
    if (cancelled()) throw new Error("CANCELLED");
    if (qs.has("orgfail")) {
      send(30, "", "err", "AI 模型返回了错误 429，请求太频繁或额度用完了：Insufficient Balance");
      throw new Error("AI 模型返回了错误 429，请求太频繁或额度用完了：Insufficient Balance");
    }
    for (let d = 0; d <= items.length; d += 40) {
      send(40 + (60 * Math.min(d, items.length)) / items.length, `正在归类 ${Math.min(d, items.length)} / ${items.length}`);
      await sleep(step * 450);
      if (cancelled()) throw new Error("CANCELLED");
    }
    const assign = new Map<number, { key: string | null; tags: string[] }>();
    for (const a of items) {
      const g = guess(a);
      assign.set(a.id, { key: g ? KEY_OF[g.key] : null, tags: g?.tags ?? [] });
    }
    pending = { assign };
    send(100, "", "ok");
    const categories = PROPOSAL_CATS.map((c) => {
      const mine = items.filter((a) => assign.get(a.id)?.key === c.key);
      return { ...c, count: mine.length, samples: mine.filter((a) => a.thumb).slice(0, 4).map((a) => ({ id: a.id, title: a.title, thumb: a.thumb })), id: null, fixed: false };
    }).filter((c) => c.count > 0);
    return { categories, total: items.length, none: items.filter((a) => !assign.get(a.id)?.key).length, usage: { prompt: 41200, completion: 6900 } };
  },
  categorize_apply: ({ edits }, ctx) => {
    if (!pending) throw new Error("整理的结果已经不在了，再整理一次");
    const E = edits as { key: string; name?: string; into?: string; drop?: boolean }[];
    const resolve = (k: string): string | null => {
      for (let i = 0, cur = k; i < 16; i++) {
        const e = E.find((x) => x.key === cur);
        if (e?.drop) return null;
        if (e?.into && e.into !== cur) cur = e.into;
        else return cur;
      }
      return null;
    };
    cats = cats.filter((c) => c.by === "user");
    const ids = new Map<string, number>();
    for (const c of PROPOSAL_CATS) {
      if (resolve(c.key) !== c.key) continue;
      const e = E.find((x) => x.key === c.key);
      const id = ++catSeq;
      cats.push({ id, name: e?.name || c.name, desc: c.desc, sort: id, by: e?.name && e.name !== c.name ? "user" : "ai" });
      ids.set(c.key, id);
    }
    let fil = 0,
      none = 0;
    for (const [aid, x] of pending.assign) {
      const k = x.key ? resolve(x.key) : null;
      const cat = k ? (ids.get(k) ?? null) : null;
      filed.set(aid, { cat, by: "ai" });
      aiTags.set(aid, x.tags);
      if (cat) fil++;
      else none++;
    }
    pending = null;
    ctx.emit({ type: "library" });
    return { categories: ids.size, filed: fil, none };
  },
  categorize_discard: () => {
    pending = null;
  },
  categorize_rest: async (_a, ctx) => {
    init(ctx);
    await sleep(ctx.still ? 0 : 1600);
    const none = [...libOf(ctx).values()].filter((a) => (filed.get(a.id)?.cat ?? null) == null && filed.get(a.id)?.by !== "user");
    let fil = 0;
    for (const a of none) {
      const g = guess(a);
      const c = g ? cats.find((x) => x.name === PROPOSAL_CATS.find((p) => p.key === KEY_OF[g.key])?.name) : undefined;
      filed.set(a.id, { cat: c?.id ?? null, by: "ai" });
      if (c) fil++;
    }
    ctx.emit({ type: "library" });
    return { filed: fil, none: none.length - fil };
  },
  ai_tags_clear: (_a, ctx) => {
    let n = 0;
    for (const L of aiTags.values()) n += L.length;
    aiTags.clear();
    ctx.emit({ type: "library" });
    return n;
  },
  llm_test: async (_a, ctx) => {
    await sleep(ctx.still ? 0 : 900);
    const s = settingsOf(ctx);
    if (!s.llmBaseUrl || !s.llmModel) throw new Error("先填上地址和模型名");
    const vision = !/localhost/.test(s.llmBaseUrl);
    ctx.state.settings = { ...s, llmVision: vision ? "yes" : "no" };
    ctx.emit({ type: "settings", settings: { ...ctx.state.settings } });
    return { ms: 1240, vision };
  },
};
