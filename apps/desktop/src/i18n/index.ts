// 界面文字的多语言（i18n）。规矩写在 docs/I18N.md，这里只说用法。
//
// 写法和 gettext 一样：代码里直接写中文原文，中文（简体）就是原文，不需要词表。
//   t("全部暂停")、t("共 {n} 个", { n })、t("登录{site}", { site })
// - 原文要直接写在 t("…") 里（双引号字面量）：tsc 核对它在英文词表（locales/en/*.json）里有没有，新加一句先在英文词表里加上。
//   其他语言缺的由 scripts/i18n.py check 报，运行时退回英文（繁体中文退回原文）。
// - 带数量的句子把数量叫 n：词表里写成 {"one": "…", "other": "…"}，按 n 选。
// - 同一句中文在不同地方要译成不同的话：原文后面加「§说明」，t("下载§按钮")；中文显示时去掉「§」和后面的部分。
// - 中文、日文里，占位符紧挨着汉字或假名、值又以字母或数字开头 / 结尾时自动空一格（「登录 YouTube」「登录哔哩哔哩」），
//   所以中文原文里占位符两边不要自己加空格。
// - 句子里要嵌元素：tx("点<b>继续</b>接着下", { b: (s) => <b>{s}</b> })、tx("{icon}已复制", { icon: <Icon … /> })。
// - 模块顶层的常量（选项表之类）不能在加载时就翻译（换语言不会跟着变）：用 N_("…") 标出来，显示时再 t(x)。
// - 不在代码里写死的原文（站点名、存进数据里的来源名、预设的步骤）用 tAny(s)：有译文用译文，没有原样返回；
//   这些原文要在某处用 N_ 标出来（比如 sites.ts），词表里才会有它们。
//
// 当前语言：设置里的 language（空 = 跟随系统，系统语言由后端在启动快照里给），换语言时先加载词表，
// 再让整个界面重新挂一次（store 的 lang 变了，main.tsx 的 Root 用它当 key）。
import { createElement, Fragment, type ReactNode } from "react";
import type { Key } from "./keys";

export type { Key };

/** 支持的语言，设置里按这个顺序列；名字用各自的语言写 */
export const LANGS = [
  { code: "en", name: "English" },
  { code: "zh-CN", name: "简体中文" },
  { code: "zh-TW", name: "繁體中文" },
  { code: "ja", name: "日本語" },
  { code: "ko", name: "한국어" },
  { code: "de", name: "Deutsch" },
  { code: "es", name: "Español" },
  { code: "fr", name: "Français" },
] as const;
export type Lang = (typeof LANGS)[number]["code"];

type Entry = string | Partial<Record<Intl.LDMLPluralRule, string>>;
type Dict = Record<string, Entry>;
export type Params = Record<string, string | number>;

/** 词表文件：import.meta.glob 的结果，键是文件路径（里面有「/locales/<语言>/」），用到哪种语言才加载哪种 */
export type Catalogs = Record<string, () => Promise<unknown>>;
let loaders: Catalogs = {};

/** 入口告诉这里有哪些词表：桌面端是全部分册（main.tsx），插件面板只要 common 和 popup（extension-popup/main.tsx），
 *  免得把桌面端的词表打进插件。所以插件面板用到的共用代码（lib/format.ts、parts）的文字要放在 common 分册 */
export function setCatalogs(c: Catalogs) {
  loaders = c;
}

let cur: Lang = "zh-CN";
let dict: Dict = {};
let en: Dict = {};
let rules = new Intl.PluralRules("zh-CN");
const enRules = new Intl.PluralRules("en");

/** 「zh-Hans-CN」「zh_TW」「en-US」「ja」→ 支持的语言；不支持的 null。中文带 Hant、TW、HK、MO 的算繁体 */
export function parseLang(s?: string | null): Lang | null {
  if (!s) return null;
  const p = s.trim().replace(/_/g, "-").toLowerCase().split(/[.@]/)[0].split("-");
  switch (p[0]) {
    case "zh":
      return p.slice(1).some((x) => x === "hant" || x === "tw" || x === "hk" || x === "mo") ? "zh-TW" : "zh-CN";
    case "en":
    case "ja":
    case "ko":
    case "es":
    case "fr":
    case "de":
      return p[0];
  }
  return null;
}

/** 现在用的语言，也是传给 Intl 的 locale */
export const lang = (): Lang => cur;

const cache = new Map<Lang, Dict>([["zh-CN", {}]]);

async function loadDict(l: Lang): Promise<void> {
  if (cache.has(l)) return;
  const files = Object.entries(loaders).filter(([p]) => p.includes(`/locales/${l}/`));
  const parts = (await Promise.all(files.map(([, f]) => f()))) as Dict[];
  cache.set(l, Object.assign({}, ...parts));
}

/** 繁体中文缺的退回原文；其他语言缺的退回英文 */
const needsEn = (l: Lang) => l !== "zh-CN" && l !== "zh-TW" && l !== "en";

/** 只把这种语言的词表加载好（不换）。换用 setLang */
export async function loadLang(l: Lang): Promise<void> {
  await Promise.all([loadDict(l), needsEn(l) ? loadDict("en") : undefined]);
}

/** 换成这种语言（词表要先 loadLang 好），并设好 <html lang>（字体按它选）。界面重画由调用方负责（store 的 lang） */
export function setLang(l: Lang): void {
  const d = cache.get(l);
  if (!d) throw new Error(`词表还没加载：${l}`);
  cur = l;
  dict = d;
  en = needsEn(l) ? (cache.get("en") ?? {}) : {};
  rules = new Intl.PluralRules(l);
  if (typeof document !== "undefined") document.documentElement.lang = l;
}

/** 加载好就换（只换一次、不会和别的切换打架的地方用：插件面板、登录工具条） */
export async function loadAndSetLang(l: Lang): Promise<void> {
  await loadLang(l);
  setLang(l);
}

const source = (key: string) => {
  const i = key.indexOf("§");
  return i < 0 ? key : key.slice(0, i);
};

function form(e: Entry, r: Intl.PluralRules, n: unknown): string {
  if (typeof e === "string") return e;
  return e[typeof n === "number" ? r.select(n) : "other"] ?? e.other ?? "";
}

/** 译文模板（还没填占位符），以及它要不要在汉字和字母之间自动加空格 */
function lookup(key: string, n: unknown): [string, boolean] {
  if (cur !== "zh-CN") {
    const e = dict[key];
    if (e !== undefined) return [form(e, rules, n), cur === "zh-TW" || cur === "ja"];
    if (cur !== "zh-TW") {
      const f = en[key];
      if (f !== undefined) return [form(f, enRules, n), false];
    }
  }
  return [source(key), true];
}

const CJK = /[㐀-䶿一-鿿豈-﫿぀-ヿ]/;
const ALNUM = /[A-Za-z0-9]/;
/** 两个字之间要不要空一格：汉字、假名挨着字母或数字 */
const gap = (a?: string, b?: string) => !!a && !!b && ((CJK.test(a) && ALNUM.test(b)) || (ALNUM.test(a) && CJK.test(b)));

/** 填一个字符串值；cjk 时按左右的字决定要不要加空格 */
function spaced(v: string, before: string | undefined, after: string | undefined, cjk: boolean): string {
  if (!cjk) return v;
  return (gap(before, v[0]) ? " " : "") + v + (gap(v[v.length - 1], after) ? " " : "");
}

function fill(tpl: string, params: Params, cjk: boolean): string {
  return tpl.replace(/\{(\w+)\}/g, (m, name: string, off: number) => {
    const v = params[name];
    return v === undefined ? m : spaced(String(v), tpl[off - 1], tpl[off + m.length], cjk);
  });
}

/** 界面文字。原文要直接写成字面量，见文件开头 */
export function t(key: Key, params?: Params): string {
  return tAny(key, params);
}

/** 不在代码里写死的原文（站点名、存下来的来源名……）：有译文用译文，没有原样返回 */
export function tAny(key: string, params?: Params): string {
  const [tpl, cjk] = lookup(key, params?.n);
  return params ? fill(tpl, params, cjk) : tpl;
}

/** 只标出「这句要进词表」，原样返回（gettext 的 N_）：模块顶层的常量用它，显示时再 t(x) */
export const N_ = <K extends Key>(k: K): K => k;

/** 列表：中文、日文用「、」连，其他语言用「, 」 */
export const joinList = (items: readonly string[]) => items.join(t("、§列表里的分隔"));

/** 带小数的数，按界面语言写小数点（德文、法文、西班牙文是逗号）。中文、英文的输出和原来的 toFixed 一样 */
export const decimal = (x: number, digits: number) => {
  // 用 toFixed 舍入，和原来一样（Intl.NumberFormat 有些值舍入不同：2675 MB 会显示成 2.68 GB，原来是 2.67）
  const s = x.toFixed(digits);
  return cur === "de" || cur === "es" || cur === "fr" ? s.replace(".", ",") : s;
};

type Rich = Record<string, ReactNode | ((inner: ReactNode) => ReactNode)>;

/** 句子里嵌元素：tx("点<b>继续</b>接着下", { b: (s) => <b>{s}</b> })、tx("{icon}已复制", { icon: <Icon /> })。
 *  标签名和占位符名都是英文字母；标签里还能有占位符，标签不能套标签 */
export function tx(key: Key, params: Rich): ReactNode {
  const [tpl, cjk] = lookup(key, params.n);
  return createElement(Fragment, null, ...rich(tpl, params, cjk));
}

function rich(tpl: string, params: Rich, cjk: boolean): ReactNode[] {
  const out: ReactNode[] = [];
  const re = /<(\w+)>([\s\S]*?)<\/\1>|\{(\w+)\}/g;
  let last = 0;
  let m: RegExpExecArray | null;
  while ((m = re.exec(tpl))) {
    if (m.index > last) out.push(tpl.slice(last, m.index));
    const before = tpl[m.index - 1],
      after = tpl[re.lastIndex];
    if (m[1]) {
      const f = params[m[1]];
      const inner = createElement(Fragment, null, ...rich(m[2], params, cjk));
      out.push(typeof f === "function" ? f(inner) : inner);
    } else {
      const v = params[m[3]];
      if (v === undefined) out.push(m[0]);
      else if (typeof v === "string" || typeof v === "number") out.push(spaced(String(v), before, after, cjk));
      else out.push(v as ReactNode);
    }
    last = re.lastIndex;
  }
  if (last < tpl.length) out.push(tpl.slice(last));
  return out;
}
