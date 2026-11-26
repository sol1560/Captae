// 演示数据换语言。演示后端（mock.ts）的数据是中文写死的（片名后缀、标签、预设、文件名、调用记录、提示……），
// 官网嵌演示版时会带 &lang=en 之类，界面换了语言，这些数据也要跟着换：演示后端把数据交给界面之前（invoke 的返回值、错误、推送的事件）过一遍这里。
// - 中文（默认，截图用的）原样返回；只换返回给界面的副本，演示后端自己的状态不动
// - 译文表 mock-i18n.json 由脚本生成（维护者本机 work/site/mock-i18n/）：App 词表里有的系统说法用正式译文，其余演示内容由 Gemini 4 Argon 写
// - 一段字的换法：整句表 → 模板（${名字} 是空位，空位里的再换一次）→ 句子里夹着的短语（路径、文件名里的片名）
// - 网址、编号这类字段不换（界面会拿它们回头找数据）
import { parseLang } from "@/i18n";
import table from "./mock-i18n.json";

type Table = { exact: Record<string, string>; patterns: [string, string][] };

const SKIP = new Set(["url", "page_url", "page", "href", "raw", "id"]);
const HAN = /[㐀-鿿]/;
const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

export function localizer(code: string | null): <V>(v: V) => V {
  const l = parseLang(code);
  const t = l && l !== "zh-CN" ? (table as unknown as Record<string, Table>)[l] : undefined;
  if (!t) return (v) => v;
  // 模板：固定的字越多越先试，免得被太宽的模板先吃掉
  const pats = t.patterns
    .map(([zh, tr]) => {
      const names: string[] = [];
      const parts = zh.split(/\$\{(\w+)\}/);
      const rx = new RegExp("^" + parts.map((p, i) => (i % 2 ? (names.push(p), "(.+?)") : esc(p))).join("") + "$");
      return { rx, names, tr, fixed: parts.filter((_, i) => i % 2 === 0).join("").length };
    })
    .sort((a, b) => b.fixed - a.fixed);
  // 短语：长的先换
  const phrases = Object.keys(t.exact)
    .filter((k) => HAN.test(k) && k.trim() === k && [...k].length >= 2)
    .sort((a, b) => b.length - a.length);
  const str = (s: string): string => {
    if (!HAN.test(s)) return s;
    const e = t.exact[s];
    if (e !== undefined) return e;
    for (const p of pats) {
      const m = p.rx.exec(s);
      if (m) return p.tr.replace(/\$\{(\w+)\}/g, (_, n: string) => str(m[p.names.indexOf(n) + 1] ?? ""));
    }
    let out = s;
    for (const k of phrases) if (out.includes(k)) out = out.split(k).join(t.exact[k]);
    return out;
  };
  const walk = (v: unknown, key?: string): unknown => {
    if (typeof v === "string") return key && SKIP.has(key) ? v : str(v);
    if (Array.isArray(v)) return v.map((x) => walk(x));
    if (v && typeof v === "object" && Object.getPrototypeOf(v) === Object.prototype) {
      return Object.fromEntries(Object.entries(v).map(([k, x]) => [k, walk(x, k)]));
    }
    return v;
  };
  return <V,>(v: V) => walk(v) as V;
}
