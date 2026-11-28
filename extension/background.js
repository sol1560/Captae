/* Captae 插件后台：
 * 1. 旁听网络请求，记下每个标签页里出现的视频、声音、大图和 M3U8（只看，不拦）
 * 2. 通过 Native Messaging 和电脑上的 Captae 说话（app.captae.bridge），每分钟打一次招呼
 * 3. 给工具栏面板（popup/，React 构建产物）和网页里的按钮提供数据
 * 4. 下载、读取信息时带上浏览器里这个网站的登录状态（cookies 和 User-Agent），见「登录状态」一节
 * 服务进程随时可能被浏览器停掉，标签页数据放在 storage.session 里。
 *
 * 失败的回复：{ok:false, e:一句话, code, fail?, reason?, login?}
 *   code 是宿主层面的（HOST_MISSING 没装 / HOST_BROKEN 装了但宿主起不来 / APP_NOT_RUNNING 没开 / HOST_GONE / TIMEOUT），
 *   或者桌面端分类过的错误码（need_login、drm……，这时 fail 里有 {code, message, raw, action}）。 */
import { classify, parseM3U8, pageSite, canonicalPage, fileName, isMediaLink, clock, estBytes } from './lib/sniff.js';
import { tr, tAny, setLang, lang, parseLang, browserLang } from './lib/i18n.js';
import { synToken, normalize, imageUrls, bigUrl } from './lib/xcard.js';

/* 界面语言：跟 Captae 实际用的语言（hello 回复的 lang：设置里选的，跟随系统时是系统语言）。插件里的错误说法、任务记录是 Captae 生成的，
 * 同一种语言才不混。电脑上没有 Captae、从没连上过时用浏览器的界面语言。
 * 记在 storage.local 的 lang 里：网页部件、面板打开时用（get-settings、lang），连不上 Captae 时沿用上次的 */
const langReady = (async () => {
  const { lang: l } = await chrome.storage.local.get('lang');
  await setLang(l || browserLang());
})();
async function syncLang(code) {
  await langReady;
  const l = parseLang(code) || browserLang();
  if (l === lang()) return;
  await setLang(l);
  await chrome.storage.local.set({ lang: l });
  menus();
  broadcastSettings();
  // 工具栏图标的提示也换成新的语言（不等下一次刷新）
  for (const tab of await chrome.tabs.query({})) badge(tab.id);
}

const HOST = 'app.captae.bridge';
const MAX_ITEMS = 60;
const VERSION = chrome.runtime.getManifest().version;

/* ---------------- 标签页数据 ---------------- */
const key = tabId => `tab:${tabId}`;
async function getTab(tabId) {
  const k = key(tabId);
  const v = (await chrome.storage.session.get(k))[k];
  return Object.assign({ url: '', title: '', items: [], dl: {}, qual: {} }, v || {});
}
/* 同一个标签页的写入排队，避免并发请求互相覆盖 */
const locks = new Map();
function withTab(tabId, fn) {
  const prev = locks.get(tabId) || Promise.resolve();
  const next = prev.then(async () => {
    const t = await getTab(tabId);
    const r = await fn(t);
    await chrome.storage.session.set({ [key(tabId)]: t });
    return r;
  }).catch(e => console.warn('captae', e));
  locks.set(tabId, next);
  return next;
}

/* ---------------- 工具栏图标：数量角标；Captae 没开时灰掉并加一个橙点，不显示数量 ---------------- */
/* ok 连着 / off 装了没开 / nohost 没装（找不到宿主清单）/ broken 装了但宿主起不来（reason 说明原因）；
 * 服务进程刚醒来时还不知道（null），先按连着处理，打完招呼再改 */
let conn = null, connReason = '';
const offline = () => conn === 'off' || conn === 'nohost' || conn === 'broken';
async function badge(tabId, t) {
  const n = (t || await getTab(tabId)).items.length;
  try {
    // 提示文字跟界面语言：服务进程刚醒来时先等词表读好
    await langReady;
    await chrome.action.setBadgeText({ tabId, text: !offline() && n ? String(n) : '' });
    await chrome.action.setBadgeBackgroundColor({ tabId, color: '#007aff' });
    await chrome.action.setBadgeTextColor?.({ tabId, color: '#ffffff' });
    await chrome.action.setTitle({ tabId, title: conn === 'nohost' ? tr('Captae：电脑上还没有 Captae') : conn === 'broken' ? tr('Captae：连不上，点开看看怎么修') : conn === 'off' ? tr('Captae：没有打开') : n ? tr('Captae：这个页面有{n}个可以下载', { n }) : tr('Captae：这个页面没有可下载的内容') });
  } catch { /* 标签页已经关了 */ }
}

let offIcon = null;
async function drawOffIcon() {
  const out = {};
  for (const size of [16, 32]) {
    const bmp = await createImageBitmap(await (await fetch(chrome.runtime.getURL(`icons/${size}.png`))).blob());
    const c = new OffscreenCanvas(size, size), g = c.getContext('2d');
    g.globalAlpha = 0.45; g.drawImage(bmp, 0, 0, size, size); g.globalAlpha = 1;
    // 右下角的橙点（第二版的状态色 --warn），外面一圈白边和图标分开
    const r = size * 0.22, x = size - r - size * 0.04, y = size - r - size * 0.04;
    g.beginPath(); g.arc(x, y, r + size * 0.06, 0, Math.PI * 2); g.fillStyle = '#ffffff'; g.fill();
    g.beginPath(); g.arc(x, y, r, 0, Math.PI * 2); g.fillStyle = '#c2660a'; g.fill();
    out[size] = g.getImageData(0, 0, size, size);
  }
  return out;
}
async function setConn(c) {
  if (c === conn) return;
  conn = c;
  try {
    if (c === 'ok') await chrome.action.setIcon({ path: { 16: 'icons/16.png', 32: 'icons/32.png' } });
    else await chrome.action.setIcon({ imageData: offIcon || (offIcon = await drawOffIcon()) });
  } catch (e) { console.warn('captae icon', e); }
  for (const tab of await chrome.tabs.query({})) badge(tab.id);
}

/* 换页面就清空（YouTube 这种单页应用切视频也会换网址） */
chrome.tabs.onUpdated.addListener((tabId, info, tab) => {
  if (info.url) {
    withTab(tabId, t => {
      if (t.url && sameDoc(t.url, info.url)) return;
      t.url = info.url; t.title = tab.title || ''; t.items = []; t.dl = {}; t.qual = {};
      const site = pageSite(info.url);
      if (site && failedNav.get(tabId) !== info.url) t.items.push(pageItem(info.url, tab.title, site));
    }).then(() => badge(tabId));
  }
  if (info.title) {
    withTab(tabId, t => {
      t.title = info.title;
      const p = t.items.find(i => i.src === 'page');
      if (p && !p.named) p.name = cleanTitle(info.title);
    });
  }
});
chrome.tabs.onRemoved.addListener(tabId => chrome.storage.session.remove(key(tabId)));

function sameDoc(a, b) {
  try { const x = new URL(a), y = new URL(b); return x.origin + x.pathname + x.search === y.origin + y.pathname + y.search; } catch { return a === b; }
}
/* 网页标题去掉网站名的尾巴：「… - YouTube」「…_哔哩哔哩_bilibili」 */
function cleanTitle(t) { return (t || '').replace(/\s*[-|–_]\s*(YouTube|哔哩哔哩.*|bilibili.*)$/i, '').trim(); }
/* 还不知道网页标题时，名字先用网站名（lib/sniff.js 的 PAGE_SITES，「哔哩哔哩」「抖音」「小红书」是中文原文）：按界面语言显示，
 * 词表里有它们（content.js 的 SITE_NAMES）。site 字段照旧存原文 */
function pageItem(url, title, site) {
  url = canonicalPage(url);
  return { id: 'page', src: 'page', kind: 'video', name: cleanTitle(title) || tAny(site), url, site, q: true, host: new URL(url).hostname };
}

/* 网页本身没打开（证书错误、被网络拦截、网站回 4xx / 5xx）：不把这一页算成能下载的，工具栏上也不显示数量。
 * 浏览器的事件先后不一定：出错先到就不建页面条目，页面条目先建了就删掉；之后同一个标签页正常打开了再补回来。 */
const failedNav = new Map(); // 标签页 → 出错的网址
function navFailed(tabId, url) {
  failedNav.set(tabId, url);
  withTab(tabId, t => { t.items = t.items.filter(i => i.src !== 'page' && i.src !== 'cover'); }).then(() => badge(tabId));
}
function navOk(tabId, url) {
  if (!failedNav.has(tabId)) return;
  failedNav.delete(tabId);
  const site = pageSite(url);
  if (!site) return;
  withTab(tabId, t => { if (!t.items.some(i => i.src === 'page')) t.items.unshift(pageItem(url, t.title, site)); }).then(() => badge(tabId));
}
chrome.webRequest.onErrorOccurred.addListener(d => {
  // ERR_ABORTED：用户自己停了、或者跳去下载 / 别的应用，不算网页坏了
  if (d.tabId >= 0 && !/ERR_ABORTED/.test(d.error || '')) navFailed(d.tabId, d.url);
}, { urls: ['<all_urls>'], types: ['main_frame'] });

/* ---------------- 嗅探 ---------------- */
chrome.webRequest.onHeadersReceived.addListener(d => {
  if (d.tabId < 0) return;
  if (d.type === 'main_frame') {
    if (d.statusCode >= 400) navFailed(d.tabId, d.url);
    else if (d.statusCode < 300) navOk(d.tabId, d.url);
  }
  const h = Object.fromEntries((d.responseHeaders || []).map(x => [x.name.toLowerCase(), x.value || '']));
  let length = +h['content-length'] || 0;
  const range = h['content-range'];
  if (range && /\/(\d+)$/.test(range)) length = +range.match(/\/(\d+)$/)[1];
  const c = classify({ url: d.url, type: d.type, contentType: h['content-type'], length, status: d.statusCode });
  if (!c) return;
  onMedia(d.tabId, d.url, c, length, h['content-type'] || '');
}, { urls: ['<all_urls>'] }, ['responseHeaders']);

async function readM3U8(url) {
  const hls = parseM3U8(await (await fetch(url, { credentials: 'include' })).text(), url);
  // 主列表：再读最高清晰度的分段列表拿到时长，按码率估算每一路多大
  if (hls && hls.master && hls.variants.length) {
    try {
      const top = parseM3U8(await (await fetch(hls.variants[0].uri, { credentials: 'include' })).text(), hls.variants[0].uri);
      if (top && !top.master) {
        hls.duration = top.duration; hls.live = top.live;
        for (const v of hls.variants) v.bytes = estBytes(v.bw, top.duration);
      }
    } catch { /* 读不到就不显示大小 */ }
  }
  return hls;
}

async function onMedia(tabId, url, c, length, contentType) {
  let hls = null;
  if (c.kind === 'hls') {
    try { hls = await readM3U8(url); } catch { /* 读不到列表也照样列出来，桌面端会再读一次 */ }
    if (hls && hls.live) return; // 直播不收
  }
  let tab;
  try { tab = await chrome.tabs.get(tabId); } catch { return; }
  const added = await withTab(tabId, t => {
    if (!t.url) { t.url = tab.url; t.title = tab.title; }
    const clean = url.split('#')[0];
    if (t.items.some(i => i.url === clean)) return null;
    // 有整页下载的网站，只补收大图
    if (t.items.some(i => i.src === 'page') && c.kind !== 'image') return null;
    // 分段列表如果已经是某个主列表的一个清晰度，就不单独列
    if (hls && !hls.master && t.items.some(i => (i.variants || []).some(v => v.uri === clean))) return null;
    if (hls && hls.master) t.items = t.items.filter(i => !hls.variants.some(v => v.uri === i.url));
    if (t.items.length >= MAX_ITEMS) return null;
    const it = {
      id: 'i' + Math.random().toString(36).slice(2, 9),
      src: 'sniff', kind: c.kind === 'hls' ? 'video' : c.kind, hls: c.kind === 'hls', ext: c.ext,
      // M3U8 的文件名多半是 master.m3u8、index.m3u8，看不出是什么：用网页标题
      name: c.kind === 'hls' && cleanTitle(tab.title) ? cleanTitle(tab.title) : fileName(clean), url: clean, size: length || 0, contentType,
      host: new URL(clean).hostname,
      dur: hls && hls.duration ? clock(hls.duration) : '',
      variants: hls && hls.master ? hls.variants.map(v => ({ p: v.p, h: v.h, uri: v.uri, bytes: v.bytes || 0, audio: v.audio })) : [],
      q: c.kind === 'hls' && !!(hls && hls.master && hls.variants.length > 1),
      at: Date.now(),
    };
    t.items.push(it);
    return it;
  });
  if (!added) return;
  badge(tabId);
  chrome.tabs.sendMessage(tabId, { t: 'items-changed' }).catch(() => {});
  // 告诉桌面端，侧栏「浏览器里发现的」会出现它（桌面端没开就算了）
  if (added.kind !== 'image') {
    // 类型说明（「在线视频」「MP4 视频」）桌面端直接显示，按界面语言写（和 Captae 是同一种语言）
    await langReady;
    native('sniffed', { item: toSniffed(added, tab) }).catch(() => {});
  }
}

function toSniffed(it, tab) {
  return {
    t: it.name, m: it.hls ? tr('在线视频') : it.kind === 'audio' ? tr('音频') : tr('{ext}视频', { ext: (it.ext || 'mp4').toUpperCase() }),
    host: it.host, url: it.url, i: it.hls ? 'list' : it.kind === 'audio' ? 'music' : 'film',
    page_url: tab.url || '', page_title: tab.title || '', size: it.size || null, content_type: it.contentType || null,
    variants: (it.variants || []).map(v => v.p), headers: tab.url ? { Referer: tab.url } : {}, at: Date.now(),
  };
}

/* ---------------- 和 Captae 说话 ---------------- */
let port = null, seq = 0;
const pending = new Map();
/* 上一次连接失败的原因：missing 没有宿主清单 / forbidden 清单里没有这个插件 / exited 宿主一启动就退出了（Captae 被挪走、删掉） */
let hostFail = '';

function hostError(code, reason) {
  return Object.assign(new Error(code), { code, reason });
}

function connect() {
  let p;
  try {
    p = chrome.runtime.connectNative(HOST);
  } catch (e) {
    port = null; hostFail = 'missing'; return;
  }
  port = p;
  let talked = false;
  p.onMessage.addListener(m => {
    talked = true;
    hostFail = '';
    const q = pending.get(m.id);
    if (!q) return;
    pending.delete(m.id);
    if (m.ok) return q.res(m.data);
    // 桌面端分类过的失败带着错误码和详情，原样交给界面
    q.rej(Object.assign(new Error(m.error || tr('没办成')), { code: m.code || (m.fail && m.fail.code) || undefined, fail: m.fail || undefined }));
  });
  p.onDisconnect.addListener(() => {
    const err = (chrome.runtime.lastError && chrome.runtime.lastError.message) || '';
    // Chrome 的说法：Specified native messaging host not found / Access to the specified native messaging host is forbidden /
    // Native host has exited / Error when communicating with the native messaging host
    if (/not found/i.test(err)) hostFail = 'missing';
    else if (/forbidden|not allowed/i.test(err)) hostFail = 'forbidden';
    else if (!talked && /exited|communicating/i.test(err)) hostFail = 'exited';
    if (port === p) port = null;
    const e = hostFail === 'missing' ? hostError('HOST_MISSING') : hostFail ? hostError('HOST_BROKEN', hostFail) : hostError('HOST_GONE');
    for (const q of pending.values()) q.rej(e);
    pending.clear();
    if (hostFail === 'missing') setConn('nohost');
    else if (hostFail) { connReason = hostFail; setConn('broken'); }
  });
}

function native(type, payload = {}, timeout = 30000) {
  if (!port) connect();
  if (!port) return Promise.reject(hostError('HOST_MISSING'));
  const id = ++seq;
  return new Promise((res, rej) => {
    pending.set(id, { res, rej });
    try { port.postMessage({ id, type, ...payload }); } catch (e) { pending.delete(id); rej(e); return; }
    setTimeout(() => { if (pending.delete(id)) rej(hostError('TIMEOUT')); }, timeout);
  });
}

/* 告诉桌面端是哪个浏览器连上来的（「浏览器插件」页显示用）；认不出的按 Chrome 算 */
function browserName() {
  const brands = (navigator.userAgentData && navigator.userAgentData.brands || []).map(b => b.brand).join(' ');
  const ua = navigator.userAgent || '';
  if (navigator.brave) return 'brave';
  if (/Edge/i.test(brands) || /Edg\//.test(ua)) return 'edge';
  if (/Opera/i.test(brands) || /OPR\//.test(ua)) return 'opera';
  if (/Vivaldi/i.test(brands) || /Vivaldi/.test(ua)) return 'vivaldi';
  if (/Chrome for Testing/i.test(brands)) return 'chrome-for-testing';
  if (/Chromium/i.test(brands) && !/Google Chrome/i.test(brands)) return 'chromium';
  return 'chrome';
}

/* ok：Captae 开着；off：装了但没开；nohost：电脑上没装；broken：装了但连接程序起不来。顺便让桌面端知道插件在。 */
async function connState() {
  let r;
  try {
    // 最多等 10 秒：Captae 开着但正忙时，宿主要问它几次（native_host.rs 的 connect_patient，最多约 7 秒）才回话。
    // 原来只等 4 秒，机器很忙时开着的 Captae 会被当成「没有打开」，跟着换版本（followApp）也要拖到下一分钟。Captae 真没开时宿主马上就回
    const h = await native('hello', { version: VERSION, browser: browserName() }, 10000);
    r = { conn: h.app ? 'ok' : 'off', clipboardApp: !!h.clipboard, hostVersion: h.version || '', appVersion: h.appVersion || '' };
    syncLang(h.lang).catch(() => {});
    followApp(h.appVersion || h.version || '').catch(() => {});
  } catch (e) {
    if (e.code === 'HOST_MISSING') r = { conn: 'nohost' };
    else if (e.code === 'HOST_BROKEN') r = { conn: 'broken', reason: e.reason || connReason };
    else r = { conn: 'off' };
  }
  connReason = r.reason || '';
  setConn(r.conn);
  return r;
}

/* 「1.10.2」比「1.9.0」新：按点分开逐段比数字 */
function newerVersion(a, b) {
  const x = String(a).split('.').map(n => parseInt(n, 10) || 0), y = String(b).split('.').map(n => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(x.length, y.length); i++) if ((x[i] || 0) !== (y[i] || 0)) return (x[i] || 0) > (y[i] || 0);
  return false;
}

/* Captae 更新以后插件跟着换（docs/DESIGN.md 第 15 节）：插件是「加载已解压」装的，文件夹在 Captae 的安装包里，
 * Captae 换成新版以后文件夹里已经是新文件，但浏览器里跑的还是旧代码。Captae 比插件新时从文件夹重新加载一次；
 * 同一个版本只试一次：重新加载以后还是旧的（从别的文件夹装的），记下 stale，工具栏面板里说明怎么更新 */
async function followApp(appVersion) {
  if (!appVersion || !newerVersion(appVersion, VERSION)) {
    await chrome.storage.local.remove('stale');
    return;
  }
  const self = chrome.management && chrome.management.getSelf ? await chrome.management.getSelf().catch(() => null) : null;
  const { reloadedFor } = await chrome.storage.local.get('reloadedFor');
  if (self && self.installType === 'development' && reloadedFor !== appVersion) {
    await chrome.storage.local.set({ reloadedFor: appVersion });
    chrome.runtime.reload();
    return;
  }
  await chrome.storage.local.set({ stale: appVersion });
}

/* 浏览器开着就每分钟打一次招呼：桌面端据此显示「插件已连上」，工具栏图标也跟着变 */
chrome.alarms.create('ping', { periodInMinutes: 1 });
chrome.alarms.onAlarm.addListener(a => { if (a.name === 'ping') connState(); });
chrome.runtime.onStartup.addListener(() => connState());

/* ---------------- 设置 ---------------- */
/* qpref：用户上次选的清晰度；没选过是空的，界面默认选「推荐」那档（1080p 及以下最高的） */
const DEF = { inj: true, intro: false, qpref: '' };
async function settings() { await langReady; return { ...DEF, ...(await chrome.storage.local.get(Object.keys(DEF))), lang: lang() }; }

/* ---------------- 登录状态（规则见 docs/DESIGN.md 第 5 节「浏览器插件带来的」） ----------------
 * 下载、读取信息时把浏览器里的 cookies 和 User-Agent 一起交给 Captae，要登录才能看的视频也能下。取两组再去重：
 *   - 标签页网址的主域名（www.bilibili.com → bilibili.com）
 *   - 要下载的媒体地址的主机名（M3U8、直链常在 CDN 上，比如 upos-sz-mirror.bilivideo.com）
 * 只在发消息那一刻读，发完就丢：不存进 chrome.storage、不写日志、不在界面上显示。YouTube 也照发，由 Captae 决定什么时候用。
 * 面板里「带上这个网站的登录状态」按网站记：关掉的网站（主域名）记在 storage.local 的 loginOff 里，默认都开。 */

/* 两级的公共后缀（co.uk、com.cn、ne.jp……）：最后一段是两个字母的国家代码、倒数第二段是这些通用词时，主域名取三段 */
const SUFFIX2 = /^(co|com|net|org|gov|edu|ac|or|ne|go|mil|nom|sch|ltd|plc)$/;
/* 托管平台：每个用户一个子域名，主域名要多算一段（alice.github.io 和 bob.github.io 是两个网站） */
const SHARED = ['github.io', 'gitlab.io', 'blogspot.com', 'netlify.app', 'vercel.app', 'pages.dev', 'workers.dev', 'herokuapp.com', 'appspot.com', 'web.app', 'firebaseapp.com', 'azurewebsites.net', 'cloudfront.net'];
function mainDomain(host) {
  host = String(host || '').toLowerCase().replace(/\.$/, '');
  // localhost、IP 地址原样
  if (!host.includes('.') || /^[\d.]+$/.test(host) || /[:[]/.test(host)) return host;
  const p = host.split('.');
  const shared = SHARED.find(x => host === x || host.endsWith('.' + x));
  let n = 2;
  if (shared) n = shared.split('.').length + 1;
  else if (p.length >= 3 && /^[a-z]{2}$/.test(p[p.length - 1]) && SUFFIX2.test(p[p.length - 2])) n = 3;
  return p.slice(-n).join('.');
}
function hostOf(url) {
  try { const u = new URL(url); return /^https?:$/.test(u.protocol) ? u.hostname.toLowerCase() : ''; } catch { return ''; }
}
/** 这个标签页算哪个网站（面板开关按它记）：网页网址的主域名；不是 http(s) 网页时是空的 */
const siteOf = url => mainDomain(hostOf(url));

async function loginOff() {
  const { loginOff: off } = await chrome.storage.local.get('loginOff');
  return off && typeof off === 'object' ? off : {};
}
/** 面板开关的状态：{site, on} */
async function loginSetting(tabId) {
  const site = siteOf((await tabInfo(tabId)).url || '');
  return { site, on: !site || !(await loginOff())[site] };
}
async function setLogin(tabId, on) {
  const site = siteOf((await tabInfo(tabId)).url || '');
  if (!site) return { site, on: true };
  const off = await loginOff();
  if (on) delete off[site];
  else off[site] = true;
  await chrome.storage.local.set({ loginOff: off });
  return { site, on: !!on };
}

/* 标签页所在的 cookie 存储（无痕窗口的标签页在另一个存储里） */
async function storeOf(tabId) {
  try {
    const stores = await chrome.cookies.getAllCookieStores();
    return (stores.find(s => (s.tabIds || []).includes(tabId)) || {}).id;
  } catch { return undefined; }
}

const COOKIE_KEYS = ['name', 'value', 'domain', 'path', 'secure', 'httpOnly', 'hostOnly', 'expirationDate'];
/** 给要发给 Captae 的消息 msg 加上 cookies 和 user_agent（下载放在 req 里，读取信息放在顶层），返回这次的情况：
 *  sent 带上了 / off 面板里关了这个网站 / none 这个网站没有 cookies（或不是网页）。
 *  cookies 只放进这条消息，函数结束就没有别的引用了。 */
async function withLogin(msg, tabId, tab, mediaUrl) {
  const site = siteOf(tab.url || '');
  if (site && (await loginOff())[site]) return 'off';
  const domains = [...new Set([site, hostOf(mediaUrl)].filter(Boolean))];
  if (!domains.length) return 'none';
  const storeId = await storeOf(tabId);
  const seen = new Set();
  const cookies = [];
  for (const domain of domains) {
    let got = [];
    try { got = await chrome.cookies.getAll(storeId ? { domain, storeId } : { domain }); } catch { /* 读不到就当没有 */ }
    for (const c of got) {
      const k = `${c.name}\t${c.domain}\t${c.path}`;
      if (seen.has(k)) continue;
      seen.add(k);
      const o = {};
      for (const f of COOKIE_KEYS) if (c[f] !== undefined) o[f] = c[f];
      cookies.push(o);
    }
  }
  if (!cookies.length) return 'none';
  msg.cookies = cookies;
  msg.user_agent = navigator.userAgent;
  return 'sent';
}
/** 失败也带上这次有没有带登录状态：网页上的卡片据此说下一步（比如「面板里的开关关着」） */
const withState = login => e => { throw Object.assign(e instanceof Error ? e : new Error(String(e)), { login }); };

/* ---------------- 下载 / 发给 AI ---------------- */
/* 清晰度选择 ch：
 *   {q:'best'|'1080p'|'720p'|'480p'}      整页交给 yt-dlp，或 M3U8 按高度找最接近的一路
 *   {q:'audio'}                           只要声音
 *   {vi: k}                               M3U8 第 k 路
 *   {uri}                                 直链的另一个清晰度（<video> 里的 <source>） */
function pickVariant(it, ch) {
  const vs = it.variants || [];
  if (!vs.length) return null;
  if (ch.vi != null) return vs[ch.vi] || vs[0];
  if (ch.q === 'audio') return vs[vs.length - 1];
  const h = parseInt(ch.q, 10);
  if (!h) return vs[0];
  return vs.find(v => v.h && v.h <= h) || vs[vs.length - 1];
}
function request(it, tab, ch = {}) {
  const q = ch.q || 'best';
  const r = {
    url: ch.uri || it.url, page_url: tab.url || null, page_title: it.src === 'page' ? null : (tab.title || null),
    headers: tab.url && it.src !== 'page' ? { Referer: tab.url } : {},
  };
  if (q === 'audio') { r.audio_only = true; r.format = 'MP3'; }
  else if (q !== 'best' && it.src === 'page') r.quality = q;
  // 正在直播：用户点了「从现在开始录」
  if (ch.live) r.record_live = true;
  const v = it.hls ? pickVariant(it, ch) : null;
  if (v) {
    // 选了哪一路就直接给那一路，不再让桌面端等选择；
    // 声音在单独列表里的，直接下那一路会没声音，只能给主列表，由桌面端带上声音
    if (!v.audio) r.url = v.uri;
    else r.quality = v.p;
  }
  return r;
}
function chLabel(it, ch = {}) {
  if (ch.q === 'audio') return tr('只要声音');
  const v = it && it.hls ? pickVariant(it, ch) : null;
  if (v) return v.p;
  return ch.label || (ch.q && ch.q !== 'best' ? ch.q : tr('最清晰'));
}

async function tabInfo(tabId) {
  try { return await chrome.tabs.get(tabId); } catch { return { url: '', title: '' }; }
}

/* 交给 Captae 最多等多久：宿主在 Captae 一时忙得没回话时会多等一会儿，没开时先把它打开（最多十来秒）。
 * 原来是默认的 30 秒：有一次 Captae 过了 50 多秒才收下，面板那一行先因为「超时」悄悄回到了「可以下载」（第三轮插件回归 t4 的偶发问题） */
const DL_WAIT = 120000;

async function download(tabId, itemId, ch) {
  const tab = await tabInfo(tabId);
  const t = await getTab(tabId);
  const it = t.items.find(i => i.id === itemId);
  if (!it) throw new Error(tr('这个文件已经不在列表里了'));
  const req = request(it, tab, ch);
  const login = await withLogin(req, tabId, tab, req.url);
  const job = await native('download', { req }, DL_WAIT).catch(withState(login));
  await withTab(tabId, t => { t.dl[itemId] = { job: job.id, ch: ch || {} }; });
  return { ...job, label: chLabel(it, ch), login };
}

async function downloadUrl(tabId, url, ch) {
  const tab = await tabInfo(tabId);
  url = canonicalPage(url);
  const isPage = url === canonicalPage(tab.url || '');
  const it = { url, src: isPage ? 'page' : 'link', hls: /\.m3u8(\?|$)/i.test(url), variants: [] };
  const req = request(it, tab, ch);
  if (!isPage) req.page_title = null;
  const login = await withLogin(req, tabId, tab, req.url);
  const job = await native('download', { req }, DL_WAIT).catch(withState(login));
  // 面板里对应的那一行也显示进度
  let added = false;
  await withTab(tabId, t => {
    let hit = t.items.find(i => i.url === url || (isPage && i.src === 'page'));
    // 录直播：直播的 M3U8 不在嗅探列表里（上面「直播不收」），从网页卡片开始录以后补上一行，面板里才看得到「正在录制」、能停止并保存
    if (!hit && ch && ch.live && t.items.length < MAX_ITEMS) {
      let host = '';
      try { host = new URL(url).hostname; } catch { /* 地址不对就不补 */ }
      if (host) {
        hit = { id: 'l' + Math.random().toString(36).slice(2, 9), src: 'link', kind: 'video', hls: it.hls, live: true, name: cleanTitle(tab.title) || fileName(url), url, host, size: 0, variants: [], q: false, at: Date.now() };
        t.items.push(hit);
        added = true;
      }
    }
    if (hit) t.dl[hit.id] = { job: job.id, ch: ch || {} };
  });
  if (added) {
    badge(tabId);
    chrome.tabs.sendMessage(tabId, { t: 'items-changed' }).catch(() => {});
  }
  return { ...job, label: chLabel(it, ch), login };
}

/* 停止下载：桌面端取消任务，面板那一行回到可以下载的样子 */
async function stop(tabId, itemId, jobId) {
  const t = await getTab(tabId);
  const d = itemId ? t.dl[itemId] : null;
  const job = jobId || (d && d.job);
  await withTab(tabId, t => {
    for (const [k, v] of Object.entries(t.dl)) if (k === itemId || v.job === job) delete t.dl[k];
  });
  if (job) await native('job', { job, action: 'cancel' });
  return true;
}

/* 录直播「停止并保存」：桌面端让录制的进程收尾，录到的部分整理好放进素材库（JobAction::Stop）。
 * 和上面的「停止」不一样：那个是取消，会删掉录到的。面板那一行接着显示「正在保存」到「下载过了」，所以不去掉 dl 里的记录 */
async function save(jobId) {
  if (!jobId) throw new Error(tr('没有这个任务'));
  await native('job', { job: jobId, action: 'stop' });
  return true;
}

/* 下载前查一下是不是已经下过；urls：同一个视频的几个地址（<source> 里的各档清晰度），下过其中一个就算 */
async function check(tabId, itemId, url, urls) {
  const tab = await tabInfo(tabId);
  let req;
  if (Array.isArray(urls) && urls.length) {
    req = { urls: urls.filter(u => /^https?:/.test(u)).slice(0, 12), page_url: null };
  } else if (itemId) {
    const it = (await getTab(tabId)).items.find(i => i.id === itemId);
    if (!it) return { found: false };
    req = { urls: [it.url, ...(it.variants || []).map(v => v.uri)], page_url: it.src === 'page' ? it.url : null };
  } else {
    url = canonicalPage(url);
    req = { urls: [url], page_url: url === canonicalPage(tab.url || '') || pageSite(url) ? url : null };
  }
  try { return await native('check', { req }, 8000); } catch { return { found: false }; }
}

/* 标题、时长、封面、各清晰度大小。整页的要问 yt-dlp，几秒钟；结果记在条目上 */
async function probe(tabId, itemId, url) {
  const t = await getTab(tabId);
  const it = itemId ? t.items.find(i => i.id === itemId) : null;
  if (it && it.probe) return it.probe;
  const tab = await tabInfo(tabId);
  const target = canonicalPage(it ? it.url : url);
  const own = it ? it.src === 'page' : target === canonicalPage(tab.url || '') || !!pageSite(target);
  const msg = { url: target, headers: tab.url && !own ? { Referer: tab.url } : {} };
  const login = await withLogin(msg, tabId, tab, target);
  const r = await native('probe', msg, 60000).catch(withState(login));
  if (it) await withTab(tabId, t => {
    const x = t.items.find(i => i.id === itemId); if (!x) return;
    x.probe = r;
    if (r.duration && !x.dur) x.dur = clock(r.duration);
    if (r.thumbnail && !x.img) x.img = r.thumbnail;
    if (r.title && x.src === 'page') { x.name = r.title; x.named = true; }
  });
  return r;
}

/* 这个标签页里最像「正在看的那个视频」的条目：blob: 播放器用它，而不是页面地址 */
async function tabMedia(tabId) {
  const t = await getTab(tabId);
  const vids = t.items.filter(i => i.kind === 'video');
  return vids.find(i => i.src === 'page')
    || vids.filter(i => i.hls).sort((a, b) => (b.variants || []).length - (a.variants || []).length || b.at - a.at)[0]
    || vids.sort((a, b) => (b.size || 0) - (a.size || 0))[0]
    || null;
}

async function share(tabId, itemId, url) {
  const tab = await tabInfo(tabId);
  let target = url, title = tab.title;
  if (itemId) {
    const it = (await getTab(tabId)).items.find(i => i.id === itemId);
    if (it) { target = it.url; title = it.src === 'page' ? it.name : tab.title; }
  }
  const opts = { url: canonicalPage(target || tab.url), page_title: title, headers: tab.url ? { Referer: tab.url } : {} };
  // 带上这个网站的登录状态（和下载同一套取法、同一个开关，cookies 放在 opts 里）：Captae 交给按网址下载那一步，只用这一次
  const login = await withLogin(opts, tabId, tab, opts.url);
  const r = await native('share', { opts }).catch(withState(login));
  return { ...r, login };
}

/* 网页元素里的大图、视频、音频：浏览器缓存里的东西不会再发网络请求，旁听不到，靠内容脚本从网页里找出来补上。
 * 只补直链（http/https），blob: 播放器靠嗅探到的 M3U8；整页交给下载引擎的网站不补视频 */
function addFromDom(t, media) {
  if (!Array.isArray(media)) return;
  const pageSiteOn = t.items.some(i => i.src === 'page');
  for (const m of media.slice(0, 40)) {
    if (!m || !/^https?:/.test(m.url || '') || t.items.length >= MAX_ITEMS) continue;
    const clean = m.url.split('#')[0];
    if (t.items.some(i => i.url === clean || (i.variants || []).some(v => v.uri === clean))) continue;
    // 整页下载的网站：视频交给下载引擎，封面已经按网页声明的列了，网页元素里的图多半是同一张封面换了尺寸，不再补
    if (pageSiteOn) continue;
    const c = classify({ url: clean, type: m.kind === 'image' ? 'image' : 'media', contentType: '', length: m.kind === 'image' ? 1e6 : 0, status: 200 });
    if (!c || (m.kind === 'image' && c.kind !== 'image')) continue;
    let host = '';
    try { host = new URL(clean).hostname; } catch { continue; }
    t.items.push({
      id: 'd' + Math.random().toString(36).slice(2, 9), src: 'sniff', kind: c.kind === 'hls' ? 'video' : c.kind, hls: c.kind === 'hls', ext: c.ext,
      name: fileName(clean), url: clean, size: 0, host, dims: m.w ? [m.w, m.h] : undefined, variants: [], q: false, at: Date.now(), dom: true,
    });
  }
}

/* 页面里的封面（og:image）单独列成一张图片 */
function addCover(t, image) {
  if (!image || !/^https?:/.test(image) || t.items.some(i => i.url === image)) return false;
  if (!t.items.some(i => i.kind === 'video') || t.items.length >= MAX_ITEMS) return false;
  let host = '';
  try { host = new URL(image).hostname; } catch { return false; }
  const ext = ((image.match(/\.(jpe?g|png|webp|avif|gif)(\?|$)/i) || [])[1] || 'jpg').toLowerCase().replace('jpeg', 'jpg');
  t.items.push({ id: 'cover', src: 'cover', kind: 'image', name: tr('视频封面'), url: image, ext, host, size: 0, at: Date.now() });
  return true;
}

/* ---------------- 消息 ---------------- */
chrome.runtime.onMessage.addListener((m, sender, reply) => {
  const tabId = m.tabId ?? sender.tab?.id;
  (async () => {
    // 回复里的文字（报错、清晰度的名字、封面条目的名字）跟界面语言：服务进程刚醒来时先等词表读好
    await langReady;
    switch (m.t) {
      case 'state': {
        // 面板打开时让网页再报一次元素里的大图和媒体（懒加载的、浏览器缓存里的）
        const media = tabId != null && tabId >= 0 ? await chrome.tabs.sendMessage(tabId, { t: 'collect' }).catch(() => null) : null;
        if (media) { await withTab(tabId, t => addFromDom(t, media)); badge(tabId); }
        const [c, t, s] = await Promise.all([connState(), getTab(tabId), settings()]);
        const tab = await tabInfo(tabId);
        // stale：Captae 比插件新、重新加载也没换过来（插件是从别的文件夹装的），面板里提示
        const { stale } = await chrome.storage.local.get('stale');
        return { ...c, stale: stale || '', version: VERSION, items: t.items, dl: t.dl, qual: t.qual, settings: s, login: await loginSetting(tabId), page: { title: tab.title || t.title, url: tab.url || t.url, favIconUrl: tab.favIconUrl || '' } };
      }
      case 'login': return setLogin(tabId, m.on !== false);
      case 'page': {
        // 内容脚本报上来的页面信息：封面、真实标题、主视频时长
        await withTab(tabId, t => {
          const p = t.items.find(i => i.src === 'page');
          if (p) { if (m.image && !p.img) p.img = m.image; if (m.title && !p.named) p.name = cleanTitle(m.title); if (m.duration && !p.dur) p.dur = clock(m.duration); }
          for (const i of t.items) if (i.src === 'sniff' && !i.img && m.image && i.kind === 'video') i.img = m.image;
          if (m.hasVideo || p) addCover(t, m.image);
          addFromDom(t, m.media);
        });
        badge(tabId);
        return true;
      }
      case 'dims': await withTab(tabId, t => { const i = t.items.find(i => i.id === m.itemId); if (i) i.dims = m.dims; }); return true;
      case 'qual':
        await withTab(tabId, t => { t.qual[m.itemId] = m.ch; });
        if (m.ch && m.ch.q) await chrome.storage.local.set({ qpref: m.ch.q });
        return true;
      case 'download': return download(tabId, m.itemId, m.ch);
      case 'download-url': return downloadUrl(tabId, m.url, m.ch);
      case 'stop': return stop(tabId, m.itemId, m.job);
      case 'save': return save(m.job || ((await getTab(tabId)).dl[m.itemId] || {}).job);
      case 'check': return check(tabId, m.itemId, m.url, m.urls);
      case 'probe': return probe(tabId, m.itemId, m.url);
      case 'head': {
        const r = await fetch(m.url, { method: 'HEAD', credentials: 'include' }).catch(() => null);
        return r && r.ok ? +r.headers.get('content-length') || 0 : 0;
      }
      case 'tab-media': return tabMedia(tabId);
      case 'jobs': return native('jobs', { ids: m.ids });
      case 'share': return share(tabId, m.itemId, m.url);
      case 'task': return native('task', { task: m.id });
      case 'task-cancel': return native('task-cancel', { task: m.id });
      case 'recopy': return native('recopy', { task: m.id });
      case 'open': return native('open', { target: m.target || {} }, 15000);
      case 'settings': await chrome.storage.local.set(m.patch || {}); broadcastSettings(); return settings();
      case 'get-settings': return settings();
      case 'lang': await langReady; return lang();
      // 网页里复制了东西：是视频链接就让宿主记下来（不弹任何东西；1 分钟内打开 Captae 时它来问，见 content.js 末尾）
      // X 帖子做成图片：帖子数据和卡片里的图片（lib/xcard.js）
      case 'x-post': return xPost(m.id);
      // 文章存成 Markdown 时下载配图：后台取不受网页跨域的限制
      case 'fetch-data': return fetchData(m.url);
      case 'copied': if (isMediaLink(m.text)) await native('copied', { url: String(m.text).trim() }, 5000).catch(() => {}); return {};
      default: throw new Error('unknown ' + m.t);
    }
  })().then(v => reply({ ok: true, v }), e => reply({ ok: false, e: e.message, code: e.code, fail: e.fail, reason: e.reason, login: e.login }));
  return true;
});

/* ---------------- X 帖子图片、文章里的图片 ---------------- */
/** 网址 → data URL。网页部件做帖子图片、打包 Markdown 用；不带 cookie、不带来源（微信的图片带了别的网站的来源会被换成「此图片来自微信公众平台」） */
async function fetchData(url) {
  if (!/^https?:\/\//.test(url || '')) throw new Error('bad url');
  const r = await fetch(url, { credentials: 'omit', referrerPolicy: 'no-referrer' });
  if (!r.ok) throw new Error('HTTP ' + r.status);
  const b = new Uint8Array(await r.arrayBuffer());
  if (b.length > 30e6) throw new Error('too big');
  let s = '';
  for (let i = 0; i < b.length; i += 0x8000) s += String.fromCharCode.apply(null, b.subarray(i, i + 0x8000));
  return `data:${(r.headers.get('content-type') || 'application/octet-stream').split(';')[0]};base64,${btoa(s)}`;
}
/** 一条帖子：X 给网页嵌入帖子用的公开接口（不用登录），图片一起取成 data URL。
 *  读不到（删了、私密账号、被限制）时 code 是 X_UNAVAILABLE */
async function xPost(id) {
  if (!/^\d{5,25}$/.test(String(id || ''))) throw new Error('bad id');
  const r = await fetch(`https://cdn.syndication.twimg.com/tweet-result?id=${id}&lang=en&token=${synToken(id)}`, { credentials: 'omit' }).catch(() => null);
  if (!r) throw Object.assign(new Error(tr('连不上 X')), { code: 'X_OFFLINE' });
  const p = normalize(r.ok ? await r.json().catch(() => null) : null);
  if (!p) throw Object.assign(new Error(tr('读不到这条帖子')), { code: 'X_UNAVAILABLE' });
  const imgs = {};
  await Promise.all(imageUrls(p).map(async u => { imgs[u] = await fetchData(bigUrl(u)).catch(() => fetchData(u)).catch(() => ''); }));
  return { post: p, imgs };
}

async function broadcastSettings() {
  const s = await settings();
  for (const tab of await chrome.tabs.query({})) chrome.tabs.sendMessage(tab.id, { t: 'settings', s }).catch(() => {});
}

/* 右键菜单：链接、视频上都能直接交给 Captae。换语言时重建 */
async function menus() {
  await langReady;
  await chrome.contextMenus.removeAll();
  chrome.contextMenus.create({ id: 'captae-dl', title: tr('用 Captae 下载'), contexts: ['link', 'video', 'audio', 'image'] });
  chrome.contextMenus.create({ id: 'captae-ai', title: tr('用 Captae 发给 AI'), contexts: ['link', 'video', 'page'] });
  // X 帖子：帖子的链接上、帖子页面上
  const X = ['*://x.com/*/status/*', '*://twitter.com/*/status/*', '*://mobile.x.com/*/status/*'];
  chrome.contextMenus.create({ id: 'captae-ximg', title: tr('把这条帖子做成图片'), contexts: ['link'], targetUrlPatterns: X });
  chrome.contextMenus.create({ id: 'captae-ximg-page', title: tr('把这条帖子做成图片'), contexts: ['page'], documentUrlPatterns: X });
  chrome.contextMenus.create({ id: 'captae-md', title: tr('把这篇文章存成 Markdown'), contexts: ['page'], documentUrlPatterns: ['http://*/*', 'https://*/*'] });
}
chrome.runtime.onInstalled.addListener(() => {
  menus();
  connState();
});
chrome.contextMenus.onClicked.addListener(async (info, tab) => {
  if (!tab) return;
  if (info.menuItemId === 'captae-ximg' || info.menuItemId === 'captae-ximg-page') {
    chrome.tabs.sendMessage(tab.id, { t: 'ctx-ximg', url: info.linkUrl || info.pageUrl }).catch(() => {});
    return;
  }
  if (info.menuItemId === 'captae-md') { chrome.tabs.sendMessage(tab.id, { t: 'ctx-md' }).catch(() => {}); return; }
  let url = info.srcUrl || info.linkUrl || info.pageUrl;
  if (!url || !tab) return;
  // 播放器里的 blob: 地址下载不了：换成这个页面嗅探到的视频，没有就交整页
  if (/^blob:/.test(url)) {
    const m = await tabMedia(tab.id).catch(() => null);
    url = m ? m.url : tab.url;
  }
  const act = info.menuItemId === 'captae-dl' ? { t: 'ctx-download', url } : { t: 'ctx-share', url };
  chrome.tabs.sendMessage(tab.id, act).catch(() => {
    (info.menuItemId === 'captae-dl' ? downloadUrl(tab.id, url) : share(tab.id, null, url)).catch(() => {});
  });
});
