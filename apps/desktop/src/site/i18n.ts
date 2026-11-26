// 官网的多语言。
// - 语言和 App 一样（@/i18n 的 LANGS：简体中文、繁体中文、英文、日文、韩文、德文、西班牙文、法文）。
//   网址是 /<语言>/（小写：/zh-cn/、/en/、/ja/……）；根地址按浏览器语言选，对不上的用英文，选好以后把网址换成 /<语言>/，不刷新。
// - 两种文字分开管：
//   官网文案（copy）：简体中文原版在 copy.ts，其他语言在 locales/<语言>.json，都由 Gemini 4 Argon 写；
//   演示画面里的按钮、状态、失败说法：用 App 自己的词表（@/i18n 的 t），和产品界面一字不差。
import { LANGS, lang, loadAndSetLang, parseLang, setCatalogs, type Lang } from "@/i18n";
import { setCopy, zhCN, type Copy } from "./copy";

setCatalogs(import.meta.glob("../i18n/locales/*/*.json", { import: "default" }));
const siteCopy = import.meta.glob<Copy>("./locales/*.json", { import: "default" });

export { LANGS, lang, type Lang };

/** 网址里的写法：小写 */
export const slugOf = (l: Lang) => l.toLowerCase();
const fromSlug = (s: string): Lang | null => LANGS.find((x) => slugOf(x.code) === s.toLowerCase())?.code ?? null;

/** 网址第一段是不是一种语言 */
export function langFromPath(): Lang | null {
  const s = location.pathname.split("/").filter(Boolean)[0];
  return s ? fromSlug(s) : null;
}

/** 打开页面时用哪种语言：网址里写了就用它，没写按浏览器的语言，都对不上用英文 */
export function pickLang(): Lang {
  const p = langFromPath();
  if (p) return p;
  for (const x of navigator.languages ?? [navigator.language]) {
    const l = parseLang(x);
    if (l) return l;
  }
  return "en";
}

/** 这种语言的网址（保留问号后面的参数和 # 后面的部分） */
export const urlOf = (l: Lang) => `/${slugOf(l)}/${location.search}${location.hash}`;

/** 换成这种语言：App 的词表、官网文案都加载好再换；顺便换掉网页标题和摘要 */
export async function applyLang(l: Lang): Promise<void> {
  const [, c] = await Promise.all([loadAndSetLang(l), l === "zh-CN" ? zhCN : siteCopy[`./locales/${l}.json`]!()]);
  setCopy(c);
  document.title = c.meta.title;
  document.querySelector('meta[name="description"]')?.setAttribute("content", c.meta.description);
}
