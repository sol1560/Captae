/* 网页文章存成 Markdown：微信公众号文章、X 的长文章，其他网页取 <article> / <main> 里正文最多的那块。
 * 在网页里直接读排好版的 DOM（要用 getComputedStyle 判断藏起来的、保留换行的元素），图片地址收集出来交给 content.js 去下载、打包。 */

const abs = (u, base) => { try { return new URL(u, base).href; } catch { return ''; } };

/** 这个网页是哪一种文章：'wechat' | 'x' | null（不是我们认识的那两种） */
export function articleKind(loc = location) {
  if (/(^|\.)mp\.weixin\.qq\.com$/.test(loc.hostname) && /^\/s/.test(loc.pathname)) return 'wechat';
  if (/(^|\.)(x|twitter)\.com$/.test(loc.hostname) && /\/article\/\d+/.test(loc.pathname)) return 'x';
  return null;
}

const txt = el => (el?.textContent || '').replace(/\s+/g, ' ').trim();

/** 读出文章：{kind, title, author, date, url, cover, root}；root 是正文所在的元素。读不到正文时 null */
export function readArticle(doc = document, loc = location) {
  const kind = articleKind(loc);
  if (kind === 'wechat') {
    const root = doc.querySelector('#js_content');
    if (!root) return null;
    const date = txt(doc.querySelector('#publish_time')) || '';
    const cover = doc.querySelector('meta[property="og:image"]')?.content || '';
    return { kind, title: txt(doc.querySelector('#activity-name')) || doc.querySelector('meta[property="og:title"]')?.content || doc.title, author: txt(doc.querySelector('#js_name')) || txt(doc.querySelector('.rich_media_meta_nickname')), date, url: canonical(doc, loc), cover, root };
  }
  if (kind === 'x') {
    const root = doc.querySelector('.x-article-body');
    const h1 = doc.querySelector('main h1') || doc.querySelector('h1');
    if (!root || !h1) return null;
    // 标题上面那张大图是封面；标题下面一行是 @作者 · 时间
    let box = h1.parentElement;
    const cover = box?.querySelector('img')?.src || '';
    // 作者：网址里的用户名最可靠（标题下那一行的 @用户名 和「2分钟」挨着，文字连在一起分不开）
    const handle = (box?.querySelector('span[class*="middot"] span')?.textContent || '').trim();
    const t = box?.querySelector('time')?.getAttribute('datetime') || '';
    const user = (loc.pathname.match(/^\/(\w{1,15})\/article\//) || [])[1];
    return { kind, title: txt(h1), author: user ? '@' + user : /^@\w{1,15}$/.test(handle) ? handle : '', date: t ? t.slice(0, 10) : '', url: loc.href.split('?')[0], cover: bigImage(cover), root };
  }
  return readGeneric(doc, loc);
}

function canonical(doc, loc) {
  return doc.querySelector('meta[property="og:url"]')?.content || doc.querySelector('link[rel="canonical"]')?.href || loc.href;
}

/** 其他网页：<article>、<main>、[role=main] 里段落文字最多的那一块 */
function readGeneric(doc, loc) {
  const cands = [...doc.querySelectorAll('article, main, [role="main"], .post, .entry-content, .article, #content')];
  let best = null, score = 0;
  for (const c of cands) {
    const s = [...c.querySelectorAll('p')].reduce((n, p) => n + (p.textContent || '').trim().length, 0);
    if (s > score) { best = c; score = s; }
  }
  if (!best || score < 200) return null;
  const h1 = best.querySelector('h1') || doc.querySelector('h1');
  return {
    kind: 'page',
    title: doc.querySelector('meta[property="og:title"]')?.content || txt(h1) || doc.title,
    author: doc.querySelector('meta[name="author"]')?.content || '',
    date: (doc.querySelector('meta[property="article:published_time"]')?.content || doc.querySelector('time[datetime]')?.getAttribute('datetime') || '').slice(0, 10),
    url: canonical(doc, loc),
    cover: '',
    root: best,
    skipTitle: h1 && best.contains(h1) ? h1 : null,
  };
}

/** X 的配图要大图；其他原样 */
export const bigImage = u => {
  if (!/pbs\.twimg\.com\/media\//.test(u)) return u;
  const path = u.split('?')[0];
  return path + (/\.\w+$/.test(path) ? '?name=large' : '?format=jpg&name=large');
};

/* ---------------- HTML → Markdown ---------------- */

const SKIP = new Set(['SCRIPT', 'STYLE', 'NOSCRIPT', 'TEMPLATE', 'BUTTON', 'INPUT', 'SELECT', 'TEXTAREA', 'FORM', 'NAV', 'svg', 'SVG', 'CANVAS', 'OBJECT', 'MPVOICE', 'MP-COMMON-PROFILE']);
const BLOCK = new Set(['P', 'DIV', 'SECTION', 'ARTICLE', 'HEADER', 'FOOTER', 'ASIDE', 'FIGURE', 'MAIN', 'CENTER', 'DL', 'DT', 'DD', 'ADDRESS', 'DETAILS', 'SUMMARY']);

/** 正文元素 → {md, images: [{url, alt}]}。图片的地址在 md 里先写成占位 \u0000IMG{n}\u0000，下载好以后再换成本地路径或原地址（finishImages） */
export function toMarkdown(root, { base = location.href, skip } = {}) {
  const images = [];
  const imgIndex = new Map();
  const ws = new WeakMap();
  const pre = el => {
    if (!el || el.nodeType !== 1) return false;
    if (!ws.has(el)) { let v = false; try { v = /^pre/.test(getComputedStyle(el).whiteSpace); } catch { /* 不在页面里 */ } ws.set(el, v); }
    return ws.get(el);
  };
  const hidden = el => {
    try { const s = getComputedStyle(el); return s.display === 'none' || s.visibility === 'hidden'; } catch { return false; }
  };
  const escText = s => s.replace(/([\\`*_[\]])/g, '\\$1');

  function image(el) {
    const raw = el.getAttribute('data-src') || el.getAttribute('data-original') || el.currentSrc || el.getAttribute('src') || '';
    if (!raw || /^data:/.test(raw)) return '';
    // 小图标、表情、占位的 1 像素图不要
    // 还没滚到、没读进来的图（公众号先放一张 1 像素的占位图，真地址在 data-src）：看它自己写的尺寸，不看占位图的
    const lazy = /^data:/.test(el.getAttribute('src') || '') || !el.complete || !el.naturalWidth;
    const w = (lazy ? +el.dataset.w : el.naturalWidth) || +el.getAttribute('width') || 0;
    const hh = (lazy ? 0 : el.naturalHeight) || +el.getAttribute('height') || 0;
    if ((w && w < 40) || (hh && hh < 40)) return '';
    const url = bigImage(abs(raw, base));
    if (!url) return '';
    if (!imgIndex.has(url)) { imgIndex.set(url, images.length); images.push({ url, alt: (el.alt || '').trim() }); }
    return `![${escText((el.alt || '').trim())}](\u0000IMG${imgIndex.get(url)}\u0000)`;
  }

  /** 行内：返回一段文字（可能带 \n 硬换行） */
  function inline(node, ctx) {
    let out = '';
    for (const c of node.childNodes) {
      if (c.nodeType === 3) {
        let s = c.nodeValue;
        if (ctx.code) out += s;
        else if (pre(c.parentElement)) out += escText(s).replace(/\n/g, '  \n');
        else out += escText(s.replace(/\s+/g, ' '));
        continue;
      }
      if (c.nodeType !== 1 || SKIP.has(c.tagName) || c === skip) continue;
      if (hidden(c)) continue;
      const tag = c.tagName;
      if (tag === 'BR') { out += '  \n'; continue; }
      if (tag === 'IMG') { out += image(c); continue; }
      if (BLOCK.has(tag) || /^(H[1-6]|UL|OL|LI|BLOCKQUOTE|PRE|TABLE|HR)$/.test(tag)) {
        // 行内里套了块（微信常见：<span><section>…）：当成换行
        const b = blocks(c, ctx).trim();
        if (b) out += '\n\n' + b + '\n\n';
        continue;
      }
      const wrap = (m, s) => { const t = s.trim(); if (!t) return s; const lead = s.match(/^\s*/)[0], tail = s.match(/\s*$/)[0]; return lead + m + t + m + tail; };
      const inner = () => inline(c, ctx);
      const fw = parseInt(c.style?.fontWeight, 10) || (c.style?.fontWeight === 'bold' ? 700 : 0);
      if ((tag === 'STRONG' || tag === 'B' || fw >= 600) && !ctx.bold) { out += wrap('**', inline(c, { ...ctx, bold: true })); continue; }
      if ((tag === 'EM' || tag === 'I') && !ctx.em) { out += wrap('*', inline(c, { ...ctx, em: true })); continue; }
      if (tag === 'DEL' || tag === 'S' || tag === 'STRIKE') { out += wrap('~~', inner()); continue; }
      if (tag === 'CODE' || tag === 'KBD') { const s = inline(c, { ...ctx, code: true }); out += s.includes('`') ? '`` ' + s + ' ``' : '`' + s + '`'; continue; }
      if (tag === 'A') {
        const href = abs(c.getAttribute('href') || '', base);
        const s = inner();
        if (!s.trim()) continue;
        if (!href || /^javascript:/.test(href) || s.includes('![')) { out += s; continue; }
        out += `[${s.trim()}](${href.replace(/\)/g, '%29')})`;
        continue;
      }
      if (tag === 'IFRAME') { const src = c.getAttribute('data-src') || c.src; if (src) out += `[▶](${abs(src, base)})`; continue; }
      out += inner();
    }
    return out;
  }

  /** 块：返回用空行隔开的几段 */
  function blocks(node, ctx) {
    const parts = [];
    let buf = '';
    const flush = () => { const s = buf.replace(/[ \t]+\n/g, m => m.endsWith('  \n') ? m : '\n').trim(); if (s) parts.push(s); buf = ''; };
    for (const c of node.childNodes) {
      if (c.nodeType === 3) { buf += inline({ childNodes: [c] }, ctx); continue; }
      if (c.nodeType !== 1 || SKIP.has(c.tagName) || c === skip || hidden(c)) continue;
      const tag = c.tagName;
      const h = tag.match(/^H([1-6])$/);
      if (h) { flush(); const s = inline(c, ctx).replace(/\s*\n\s*/g, ' ').trim(); if (s) parts.push('#'.repeat(+h[1]) + ' ' + s); continue; }
      if (tag === 'UL' || tag === 'OL') { flush(); const s = list(c, ctx, tag === 'OL'); if (s) parts.push(s); continue; }
      if (tag === 'BLOCKQUOTE') { flush(); const s = blocks(c, ctx).trim(); if (s) parts.push(s.split('\n').map(l => '> ' + l).join('\n').replace(/> $/gm, '>')); continue; }
      if (tag === 'PRE') { flush(); const code = c.innerText.replace(/\n+$/, ''); const lang = (c.querySelector('code')?.className.match(/language-([\w+-]+)/) || [])[1] || ''; const fence = code.includes('```') ? '~~~' : '```'; parts.push(fence + lang + '\n' + code + '\n' + fence); continue; }
      if (tag === 'HR') { flush(); parts.push('---'); continue; }
      if (tag === 'TABLE') { flush(); const s = table(c, ctx); if (s) parts.push(s); continue; }
      if (tag === 'FIGCAPTION') { flush(); const s = inline(c, ctx).trim(); if (s) parts.push('*' + s.replace(/\*/g, '') + '*'); continue; }
      if (BLOCK.has(tag) || tag === 'LI') { flush(); const s = blocks(c, ctx).trim(); if (s) parts.push(s); continue; }
      buf += inline({ childNodes: [c] }, ctx);
    }
    flush();
    return parts.join('\n\n');
  }

  /** 列表：里面一层的列表照样是一个块，接在这一项的文字下面、缩进 3 格（「1. 」的宽度，「- 」也够） */
  function list(el, ctx, ordered) {
    let i = +(el.getAttribute('start') || 1);
    const rows = [];
    for (const li of el.children) {
      if (li.tagName !== 'LI' || hidden(li)) continue;
      const mark = ordered ? `${i++}. ` : '- ';
      // 这一项的文字和里面一层的列表之间不要空行（不然会变成「松散列表」，每项之间都空一行）
      const body = blocks(li, ctx).trim().replace(/\n\n(?=(?:- |\d+\. ))/g, '\n');
      if (!body) continue;
      const lines = body.split('\n');
      rows.push(mark + lines[0] + lines.slice(1).map(l => '\n' + (l.trim() ? '   ' + l : '')).join(''));
    }
    return rows.join('\n');
  }

  function table(el, ctx) {
    const rows = [...el.querySelectorAll('tr')].map(tr => [...tr.children].filter(c => /^T[HD]$/.test(c.tagName)).map(c => inline(c, ctx).replace(/\s*\n\s*/g, ' ').replace(/\|/g, '\\|').trim()));
    const n = Math.max(0, ...rows.map(r => r.length));
    if (!n) return '';
    const line = r => '| ' + Array.from({ length: n }, (_, i) => r[i] || '').join(' | ') + ' |';
    return [line(rows[0]), '| ' + Array(n).fill('---').join(' | ') + ' |', ...rows.slice(1).map(line)].join('\n');
  }

  const md = blocks(root, {})
    .replace(/\u00a0/g, ' ')
    // 只有空格的行（连着两个 <br> 留下的）当空行；段落最后的硬换行去掉
    .replace(/^[ \t]+$/gm, '')
    .replace(/ {2}\n(?=\n)/g, '\n')
    .replace(/ {2}$/, '')
    .replace(/\n{3,}/g, '\n\n')
    // 空的强调（** **）去掉
    .replace(/\*\*\s*\*\*/g, '')
    .trim();
  return { md, images };
}

/** 占位换成图片的地址：local[i] 有就用本地路径，没有用原地址 */
export function finishImages(md, images, local = []) {
  return md.replace(/\u0000IMG(\d+)\u0000/g, (_, i) => local[+i] || images[+i]?.url || '');
}

/** 文件头（YAML，Obsidian 之类认识）+ 标题 */
export function frontMatter(a) {
  const q = s => JSON.stringify(String(s || ''));
  const lines = ['---', `title: ${q(a.title)}`];
  if (a.author) lines.push(`author: ${q(a.author)}`);
  if (a.date) lines.push(`date: ${q(a.date)}`);
  lines.push(`source: ${q(a.url)}`, '---', '', `# ${a.title}`, '', '');
  return lines.join('\n');
}

/** 文件名里不能有的字符换掉，最长 80 个字 */
export const safeName = s => (String(s || '').replace(/[\\/:*?"<>|\u0000-\u001f]+/g, ' ').replace(/\s+/g, ' ').trim().slice(0, 80).trim() || 'article');
