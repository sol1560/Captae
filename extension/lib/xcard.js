/* X 帖子做成图片：
 * - 数据：X 给网页嵌入帖子用的公开接口（cdn.syndication.twimg.com/tweet-result，不用登录），后台去取（background.js 的 x-post），
 *   这里的 normalize 把它理成卡片要的样子。X 网页本身 2026 年改版后没有稳定的标记可抓，所以不从网页上读。
 * - 出图：卡片是一段带内联样式的 HTML，放进 SVG 的 <foreignObject> 当图片画到 canvas 上，按 3 倍画（600 宽的卡片出 1800 宽的 PNG）。
 *   图片（头像、配图）先由后台取成 data URL 塞进去：SVG 当图片用时不会去网上取东西，canvas 也就不会被「污染」、能导出。
 *   字体用系统字体（苹方、SF、系统 emoji），系统字体在 SVG 图片里能用，不用打包。
 * 图标来自 Hugeicons Free（MIT），路径数据原样。 */

/** 接口要的 token（和 X 自己的嵌入脚本算法一样） */
export const synToken = id => ((Number(id) / 1e15) * Math.PI).toString(36).replace(/(0+|\.)/g, '');

const unesc = s => String(s || '').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&amp;/g, '&');

/** 正文 → 片段：普通文字和蓝色的（@某人、#话题、链接）。t.co 短链换成显示用的地址，配图的链接去掉。
 *  X 给的位置按 Unicode 码点算（emoji 算一个），所以先拆成码点数组再切 */
function segments(j) {
  const cps = Array.from(j.text || '');
  const [from, to] = j.display_text_range || [0, cps.length];
  const e = j.entities || {};
  const marks = [
    ...(e.urls || []).map(u => ({ i: u.indices, s: u.display_url || u.expanded_url || u.url, link: true })),
    ...(e.user_mentions || []).map(u => ({ i: u.indices, link: true })),
    ...(e.hashtags || []).map(u => ({ i: u.indices, link: true })),
    ...(e.symbols || []).map(u => ({ i: u.indices, link: true })),
    ...(e.media || []).map(u => ({ i: u.indices, s: '' })),
  ].filter(m => Array.isArray(m.i)).sort((a, b) => a.i[0] - b.i[0]);
  const out = [];
  const push = (s, link) => { if (!s) return; const last = out[out.length - 1]; if (last && !!last.link === !!link) last.s += s; else out.push({ s, link: !!link }); };
  let at = from;
  for (const m of marks) {
    const [a, b] = m.i;
    // 和前一个重叠的、在显示范围以外的（结尾的配图链接）跳过
    if (a < at || a >= to) continue;
    push(unesc(cps.slice(at, a).join('')));
    push(m.s !== undefined ? m.s : unesc(cps.slice(a, b).join('')), m.link);
    at = Math.min(b, to);
  }
  push(unesc(cps.slice(at, to).join('')));
  // 结尾的空白（去掉配图链接以后留下的）
  const last = out[out.length - 1];
  if (last) last.s = last.s.replace(/\s+$/, '');
  return out.filter(x => x.s);
}

/** 接口原样的数据 → 卡片要的样子。不是帖子（删了、私密、被限制）时返回 null */
export function normalize(j) {
  if (!j || j.__typename === 'TweetTombstone' || !j.user) return null;
  const u = j.user;
  const media = (j.mediaDetails || []).map(m => ({
    kind: m.type === 'photo' ? 'photo' : 'video',
    url: m.media_url_https,
    w: m.original_info?.width || 0,
    h: m.original_info?.height || 0,
  })).filter(m => m.url);
  const bv = j.card?.binding_values || {};
  const sv = k => bv[k]?.string_value || '';
  const iv = k => bv[k]?.image_value?.url || '';
  const cardImg = iv('summary_photo_image_large') || iv('thumbnail_image_large') || iv('photo_image_full_size_large') || iv('thumbnail_image');
  return {
    id: j.id_str,
    url: `https://x.com/${u.screen_name}/status/${j.id_str}`,
    name: u.name || u.screen_name,
    handle: u.screen_name,
    avatar: (u.profile_image_url_https || '').replace(/_normal(\.\w+)$/, '_400x400$1'),
    square: u.profile_image_shape === 'Square',
    verified: u.verified_type === 'Business' ? 'gold' : u.verified_type === 'Government' ? 'gray' : u.is_blue_verified || u.verified ? 'blue' : '',
    created: j.created_at,
    segs: segments(j),
    // 超过 280 字的长帖接口只给前一段（带 note_tweet）：网页部件会试着从网页上补全
    truncated: !!j.note_tweet,
    media,
    linkCard: !media.length && sv('title') ? { title: sv('title'), desc: sv('description'), domain: sv('vanity_url') || sv('domain'), img: cardImg } : null,
    quote: j.quoted_tweet ? normalize(j.quoted_tweet) : null,
    likes: j.favorite_count || 0,
    replies: j.conversation_count || 0,
  };
}

/** 卡片里所有要换成 data URL 的图片地址（后台按这个去取） */
export function imageUrls(p) {
  if (!p) return [];
  return [p.avatar, ...p.media.map(m => m.url), p.linkCard?.img, ...imageUrls(p.quote)].filter(Boolean);
}
/** 配图取大一点的尺寸（2048 宽），够 3 倍画 */
export const bigUrl = u => /pbs\.twimg\.com\/media\//.test(u) ? u.replace(/\?.*$/, '') + '?name=large' : u;

/* ---------------- 卡片 ---------------- */

const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const svg = (paths, size, extra = '') => `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" ${extra}>${paths}</svg>`;
// Hugeicons Free：CheckmarkBadge01Icon、FavouriteIcon、Comment01Icon、PlayIcon
const BADGE = '<path d="M18.9905 19H19M18.9905 19C18.3678 19.6175 17.2393 19.4637 16.4479 19.4637C15.4765 19.4637 15.0087 19.6537 14.3154 20.347C13.7251 20.9374 12.9337 22 12 22C11.0663 22 10.2749 20.9374 9.68457 20.347C8.99128 19.6537 8.52349 19.4637 7.55206 19.4637C6.76068 19.4637 5.63218 19.6175 5.00949 19C4.38181 18.3776 4.53628 17.2444 4.53628 16.4479C4.53628 15.4414 4.31616 14.9786 3.59938 14.2618C2.53314 13.1956 2.00002 12.6624 2 12C2.00001 11.3375 2.53312 10.8044 3.59935 9.73817C4.2392 9.09832 4.53628 8.46428 4.53628 7.55206C4.53628 6.76065 4.38249 5.63214 5 5.00944C5.62243 4.38178 6.7556 4.53626 7.55208 4.53626C8.46427 4.53626 9.09832 4.2392 9.73815 3.59937C10.8044 2.53312 11.3375 2 12 2C12.6625 2 13.1956 2.53312 14.2618 3.59937C14.9015 4.23907 15.5355 4.53626 16.4479 4.53626C17.2393 4.53626 18.3679 4.38247 18.9906 5C19.6182 5.62243 19.4637 6.75559 19.4637 7.55206C19.4637 8.55858 19.6839 9.02137 20.4006 9.73817C21.4669 10.8044 22 11.3375 22 12C22 12.6624 21.4669 13.1956 20.4006 14.2618C19.6838 14.9786 19.4637 15.4414 19.4637 16.4479C19.4637 17.2444 19.6182 18.3776 18.9905 19Z" fill="currentColor" stroke="currentColor" stroke-width="1.5"/><path d="M9 12.8929C9 12.8929 10.2 13.5447 10.8 14.5C10.8 14.5 12.6 10.75 15 9.5" stroke="#fff" stroke-width="1.9" stroke-linecap="round" stroke-linejoin="round"/>';
const HEART = '<path d="M2 8.69444C2 13.0348 7.58942 17.858 10.4107 19.9677C11.3601 20.6776 12.6399 20.6776 13.5893 19.9677C16.4106 17.858 22 13.0348 22 8.69444C22 5.82563 19.8947 3.5 17 3.5C15.5 3.5 14 4 12 6C10 4 8.5 3.5 7 3.5C4.10526 3.5 2 5.82563 2 8.69444Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.6"/>';
const BUBBLE = '<path d="M8 13.0012H16M8 8.00122H12" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round" stroke-width="1.6"/><path d="M12 18.0012H15C18.2998 18.0012 19.9497 18.0012 20.9749 16.9761C22 15.951 22 14.3011 22 11.0012V9.50122C22 6.20139 22 4.55147 20.9749 3.52635C19.9497 2.50122 18.2998 2.50122 15 2.50122H9C5.70017 2.50122 4.05025 2.50122 3.02513 3.52635C2 4.55147 2 6.20139 2 9.50122V14.0012C2 14.4656 2 14.6978 2.02201 14.8931C2.20484 16.5158 3.48545 17.7964 5.10812 17.9792C5.30344 18.0012 5.53563 18.0012 6 18.0012V20.0819C6 20.9044 6 21.3157 6.26407 21.457C6.52814 21.5983 6.87033 21.3702 7.5547 20.914L12 18.0012Z" stroke="currentColor" stroke-linecap="square" stroke-linejoin="round" stroke-width="1.6"/>';
const PLAY = '<path d="M18.8906 12.846C18.5371 14.189 16.8667 15.138 13.5257 17.0361C10.296 18.8709 8.6812 19.7884 7.37983 19.4196C6.8418 19.2671 6.35159 18.9776 5.95624 18.5787C5 17.6139 5 15.7426 5 12C5 8.2574 5 6.3861 5.95624 5.42132C6.35159 5.02245 6.8418 4.73288 7.37983 4.58042C8.6812 4.21165 10.296 5.12907 13.5257 6.96393C16.8667 8.86197 18.5371 9.811 18.8906 11.154C19.0365 11.7084 19.0365 12.2916 18.8906 12.846Z" fill="#fff" stroke="#fff" stroke-linejoin="round" stroke-width="1.5"/>';

const THEMES = {
  light: { card: '#ffffff', ink: '#0f1419', ink2: '#536471', line: 'rgba(15,20,25,.1)', link: '#1d8cd8', well: '#f4f5f6',
    bg: 'radial-gradient(120% 90% at 0% 0%, #dde7f3 0%, rgba(221,231,243,0) 60%), radial-gradient(110% 90% at 100% 100%, #efe4da 0%, rgba(239,228,218,0) 60%), #eef0f3',
    shadow: '0 1px 2px rgba(15,20,25,.06), 0 12px 40px -12px rgba(15,20,25,.22)' },
  dark: { card: '#16181c', ink: '#e7e9ea', ink2: '#8b9096', line: 'rgba(255,255,255,.12)', link: '#4aa8f0', well: '#202327',
    bg: 'radial-gradient(120% 90% at 0% 0%, #1f2b3d 0%, rgba(31,43,61,0) 60%), radial-gradient(110% 90% at 100% 100%, #2b2233 0%, rgba(43,34,51,0) 60%), #0b0d10',
    shadow: '0 0 0 1px rgba(255,255,255,.06), 0 18px 48px -14px rgba(0,0,0,.7)' },
};
const BADGE_COLOR = { blue: '#1d9bf0', gold: '#e2b719', gray: '#829aab' };
const FONT = '-apple-system, BlinkMacSystemFont, "SF Pro Text", "PingFang SC", "Hiragino Sans", "Apple SD Gothic Neo", "Segoe UI", "Microsoft YaHei", "Noto Sans CJK SC", system-ui, sans-serif, "Apple Color Emoji", "Segoe UI Emoji"';

/** 卡片的宽度（CSS 像素）；出图时乘 scale */
export const CARD_W = 600;

function textHTML(segs) {
  return segs.map(x => x.link ? `<span class="lk">${esc(x.s)}</span>` : esc(x.s)).join('');
}
function mediaHTML(media, imgs) {
  if (!media.length) return '';
  const src = m => esc(imgs[m.url] || '');
  const one = (m, hh, cls = '') => `<div class="mi ${cls}" style="height:${hh}px"><img src="${src(m)}" alt=""/>${m.kind === 'video' ? `<span class="pl">${svg(PLAY, 26)}</span>` : ''}</div>`;
  const W = CARD_W - 56;
  const n = Math.min(media.length, 4);
  if (n === 1) {
    const m = media[0];
    const ratio = m.w && m.h ? m.h / m.w : 0.5625;
    return `<div class="md n1">${one(m, Math.round(W * Math.min(Math.max(ratio, 0.4), 1.25)))}</div>`;
  }
  if (n === 2) return `<div class="md n2">${media.slice(0, 2).map(m => one(m, 300)).join('')}</div>`;
  if (n === 3) return `<div class="md n3">${one(media[0], 320, 'tall')}<div class="col">${one(media[1], 158)}${one(media[2], 158)}</div></div>`;
  return `<div class="md n4">${media.slice(0, 4).map(m => one(m, 170)).join('')}</div>`;
}
function headHTML(p, imgs, small) {
  const av = `<img class="av${p.square ? ' sq' : ''}" src="${esc(imgs[p.avatar] || '')}" alt=""/>`;
  const badge = p.verified ? `<span class="bd" style="color:${BADGE_COLOR[p.verified]}">${svg(BADGE, small ? 15 : 19)}</span>` : '';
  return `<div class="hd${small ? ' sm' : ''}">${av}<div class="who"><div class="nm"><b>${esc(p.name)}</b>${badge}</div><div class="hn">@${esc(p.handle)}</div></div></div>`;
}

/** 样式都挂在 .xc-root 下面：卡片先放进网页部件的 Shadow DOM 里排版、量尺寸，不加前缀会套到部件自己的 .nm、.st 上 */
const scope = css => css.replace(/(^|})\s*([^{}]+)\{/g, (m, b, sel) => b + '\n' + sel.split(',').map(x => x.trim()).map(x => x.startsWith('.xc-root') ? x : '.xc-root ' + x).join(',') + '{');

/** 卡片的 HTML（外层是背景，里面是卡片）。opts: {theme: 'light'|'dark', bg: true|false, stats: true|false, lang} */
export function cardHTML(p, imgs, opts = {}) {
  const th = THEMES[opts.theme] || THEMES.light;
  const lang = opts.lang || 'en';
  const d = p.created ? new Date(p.created) : null;
  let when = '';
  if (d && !isNaN(d)) {
    try {
      when = `${new Intl.DateTimeFormat(lang, { hour: 'numeric', minute: '2-digit' }).format(d)} · ${new Intl.DateTimeFormat(lang, { year: 'numeric', month: 'short', day: 'numeric' }).format(d)}`;
    } catch { when = d.toISOString().slice(0, 10); }
  }
  let nf;
  try { nf = new Intl.NumberFormat(lang, { notation: 'compact', maximumFractionDigits: 1 }); } catch { nf = new Intl.NumberFormat('en', { notation: 'compact' }); }
  const stat = (ico, n) => n ? `<span class="st">${svg(ico, 18)}${esc(nf.format(n))}</span>` : '';
  const stats = opts.stats !== false ? stat(HEART, p.likes) + stat(BUBBLE, p.replies) : '';
  const q = p.quote;
  const quote = q ? `<div class="qt">${headHTML(q, imgs, true)}${q.segs.length ? `<div class="tx sm">${textHTML(q.segs)}</div>` : ''}${q.media.length ? mediaHTML(q.media.slice(0, 1), imgs).replace('class="md', 'class="md qm') : ''}</div>` : '';
  const lc = p.linkCard;
  const link = lc ? `<div class="lc">${lc.img && imgs[lc.img] ? `<img src="${esc(imgs[lc.img])}" alt=""/>` : ''}<div class="lt">${lc.domain ? `<span>${esc(lc.domain)}</span>` : ''}<b>${esc(lc.title)}</b></div></div>` : '';
  const css = `
.xc-root{width:${CARD_W + (opts.bg === false ? 0 : 96)}px;padding:${opts.bg === false ? 0 : 48}px;box-sizing:border-box;background:${opts.bg === false ? 'transparent' : th.bg};font-family:${FONT};-webkit-font-smoothing:antialiased}
.xc-root *{box-sizing:border-box;margin:0}
.xc-card{width:${CARD_W}px;padding:26px 28px 22px;border-radius:26px;background:${th.card};color:${th.ink};box-shadow:${opts.bg === false ? 'none' : th.shadow}}
.hd{display:flex;align-items:center;gap:12px}
.av{width:52px;height:52px;border-radius:50%;object-fit:cover;flex:none;background:${th.well}}
.av.sq{border-radius:12px}
.who{min-width:0}
.nm{display:flex;align-items:center;gap:4px;font-size:17px;line-height:1.25;white-space:nowrap}
.nm b{font-weight:700;overflow:hidden;text-overflow:ellipsis}
.bd{display:flex;flex:none}
.hn{font-size:15px;line-height:1.3;color:${th.ink2}}
.hd.sm{gap:8px}.hd.sm .av{width:24px;height:24px}.hd.sm .av.sq{border-radius:6px}.hd.sm .who{display:flex;gap:6px;align-items:baseline;min-width:0}
.hd.sm .nm{font-size:15px}.hd.sm .hn{font-size:14px}
.tx{margin-top:16px;font-size:20px;line-height:1.5;letter-spacing:-.005em;white-space:pre-wrap;word-wrap:break-word;overflow-wrap:anywhere}
.tx.sm{margin-top:8px;font-size:16px;line-height:1.45}
.lk{color:${th.link}}
.md{margin-top:16px;display:grid;gap:3px;border-radius:18px;overflow:hidden;border:1px solid ${th.line}}
.md.n2,.md.n4{grid-template-columns:1fr 1fr}
.md.n3{grid-template-columns:1fr 1fr}
.md .col{display:grid;gap:3px}
.mi{position:relative;overflow:hidden;background:${th.well}}
.mi img{display:block;width:100%;height:100%;object-fit:cover}
.pl{position:absolute;left:50%;top:50%;width:60px;height:60px;margin:-30px 0 0 -30px;border-radius:50%;background:rgba(15,20,25,.62);display:flex;align-items:center;justify-content:center;padding-left:3px}
.qt{margin-top:16px;padding:14px 16px;border:1px solid ${th.line};border-radius:18px}
.qt .md{margin-top:10px;border-radius:12px}
.lc{margin-top:16px;border:1px solid ${th.line};border-radius:18px;overflow:hidden}
.lc img{display:block;width:100%;height:270px;object-fit:cover;border-bottom:1px solid ${th.line}}
.lt{padding:12px 16px;display:flex;flex-direction:column;gap:2px;font-size:15px;line-height:1.35}
.lt span{color:${th.ink2}}
.lt b{font-weight:600}
.ft{display:flex;align-items:center;justify-content:space-between;gap:12px;margin-top:18px;padding-top:14px;border-top:1px solid ${th.line};font-size:14.5px;color:${th.ink2};white-space:nowrap}
.sts{display:flex;gap:18px}
.st{display:flex;align-items:center;gap:6px;font-variant-numeric:tabular-nums}`;
  return `<div class="xc-root"><style>${scope(css)}</style><div class="xc-card">${headHTML(p, imgs)}${p.segs.length ? `<div class="tx">${textHTML(p.segs)}</div>` : ''}${mediaHTML(p.media, imgs)}${link}${quote}<div class="ft"><span>${esc(when)}</span><span class="sts">${stats}</span></div></div></div>`;
}

/** 量好尺寸的卡片元素（已经在页面里、排好版）→ PNG。scale 3：600 宽的卡片出 1800 宽 */
export async function rasterize(el, scale = 3) {
  const w = Math.ceil(el.offsetWidth), hh = Math.ceil(el.offsetHeight);
  const xhtml = new XMLSerializer().serializeToString(el);
  const src = `<svg xmlns="http://www.w3.org/2000/svg" width="${w * scale}" height="${hh * scale}" viewBox="0 0 ${w} ${hh}"><foreignObject x="0" y="0" width="${w}" height="${hh}">${xhtml}</foreignObject></svg>`;
  const img = new Image();
  img.src = 'data:image/svg+xml;charset=utf-8,' + encodeURIComponent(src);
  await img.decode();
  const c = document.createElement('canvas');
  c.width = w * scale;
  c.height = hh * scale;
  c.getContext('2d').drawImage(img, 0, 0, c.width, c.height);
  const blob = await new Promise(r => c.toBlob(r, 'image/png'));
  if (!blob) throw new Error('canvas');
  return { blob, w: c.width, h: c.height };
}
