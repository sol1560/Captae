/* 插件的多语言（网页里的部件、后台、右键菜单）。写法和桌面端一样（apps/desktop/src/i18n，规矩见 docs/I18N.md）：
 * 代码里写中文原文 tr('全部下载')、tr('共 {n} 个', { n })、tr('登录{site}', { site })；中文（简体）就是原文，
 * （函数叫 tr 不叫 t：插件代码里很多局部变量叫 t，会把它遮住）
 * 其他语言的译文在 lib/i18n/<语言>.json（scripts/i18n.py 核对、导出、导入）。
 * - 带数量的句子把数量叫 n，词表里写成 {"one": …, "other": …}
 * - 同一句中文要译成不同的话：原文后面加「§说明」，中文显示时去掉
 * - 中文、日文里占位符紧挨着汉字或假名、值以字母或数字开头 / 结尾时自动空一格，所以中文原文里占位符两边不要自己加空格
 * - 模块顶层的常量用 N_('…') 标出来，显示时再 tr(x)；不在代码里写死的原文（站点名……）用 tAny
 * 用哪种语言由后台定（background.js 的 syncLang）：跟 Captae 实际用的语言（插件里显示的错误说法、任务记录是 Captae 生成的，同一种语言才不混）；
 * 电脑上没有 Captae、从没连上过时用浏览器的界面语言。 */

let cur = 'zh-CN';
let dict = {};
let en = {};
let rules = new Intl.PluralRules('zh-CN');
const enRules = new Intl.PluralRules('en');

/** 「zh-Hans-CN」「zh_TW」「en-US」「ja」→ 支持的语言代码；不支持的 null */
export function parseLang(s) {
  if (!s) return null;
  const p = String(s).trim().replace(/_/g, '-').toLowerCase().split(/[.@]/)[0].split('-');
  if (p[0] === 'zh') return p.slice(1).some(x => ['hant', 'tw', 'hk', 'mo'].includes(x)) ? 'zh-TW' : 'zh-CN';
  return ['en', 'ja', 'ko', 'es', 'fr', 'de'].includes(p[0]) ? p[0] : null;
}

/** 浏览器的界面语言（不支持的用英文） */
export const browserLang = () => parseLang(chrome.i18n.getUILanguage()) || 'en';

export const lang = () => cur;

async function load(l) {
  if (l === 'zh-CN') return {};
  try {
    return await (await fetch(chrome.runtime.getURL(`lib/i18n/${l}.json`))).json();
  } catch {
    return {};
  }
}

/** 换语言：先把词表取好再换 */
export async function setLang(code) {
  const l = parseLang(code) || 'en';
  const needEn = l !== 'zh-CN' && l !== 'zh-TW' && l !== 'en';
  const [d, e] = await Promise.all([load(l), needEn ? load('en') : {}]);
  cur = l;
  dict = d;
  en = l === 'en' ? d : e;
  rules = new Intl.PluralRules(l);
}

const source = k => { const i = k.indexOf('§'); return i < 0 ? k : k.slice(0, i); };
const form = (e, r, n) => typeof e === 'string' ? e : (e[typeof n === 'number' ? r.select(n) : 'other'] ?? e.other ?? '');

function lookup(key, n) {
  if (cur !== 'zh-CN') {
    if (dict[key] !== undefined) return [form(dict[key], rules, n), cur === 'zh-TW' || cur === 'ja'];
    if (cur !== 'zh-TW' && en[key] !== undefined) return [form(en[key], enRules, n), false];
  }
  return [source(key), true];
}

const CJK = /[㐀-䶿一-鿿豈-﫿぀-ヿ]/;
const ALNUM = /[A-Za-z0-9]/;
const gap = (a, b) => !!a && !!b && ((CJK.test(a) && ALNUM.test(b)) || (ALNUM.test(a) && CJK.test(b)));

/** 不在代码里写死的原文：有译文用译文，没有原样返回 */
export function tAny(key, params) {
  const [tpl, cjk] = lookup(key, params && params.n);
  if (!params) return tpl;
  return tpl.replace(/\{(\w+)\}/g, (m, name, off) => {
    if (params[name] === undefined) return m;
    const v = String(params[name]);
    if (!cjk) return v;
    return (gap(tpl[off - 1], v[0]) ? ' ' : '') + v + (gap(v[v.length - 1], tpl[off + m.length]) ? ' ' : '');
  });
}

/** 界面文字：原文要直接写成字面量 tr('…')，scripts/i18n.py 从源码里找它们 */
export const tr = tAny;

/** 只标出「这句要进词表」，原样返回 */
export const N_ = k => k;

/** 列表：中文、日文用「、」连，其他语言用「, 」 */
export const joinList = items => items.join(tr('、§列表里的分隔'));

/** 带小数的数，按界面语言写小数点（德文、法文、西班牙文是逗号） */
export const decimal = (x, digits) => {
  // 用 toFixed 舍入，和原来一样（Intl.NumberFormat 有些值舍入不同）
  const s = x.toFixed(digits);
  return cur === 'de' || cur === 'es' || cur === 'fr' ? s.replace('.', ',') : s;
};
