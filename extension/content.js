/* 网页里的 Captae（第二版，轻玻璃）：
 * - 视频右上角的按钮组「下载 ⌄」「发给 AI」，清晰度菜单（分辨率、格式、大小、推荐、封面图）
 * - 右下角的卡片：下载进度（停止、收起成小胶囊、在 Captae 里查看）、下载过了、发给 AI、要打开 / 安装 Captae、下载失败的原因
 *   （复制了视频链接不弹卡片，只告诉 Captae 一声：1 分钟内打开 Captae 时由它来问，见文件末尾）
 * 全部放在一个 Shadow DOM 里，不受网页样式影响，也不影响网页；不用 React（每个网页都会注入，计划 5.6），动画全是 CSS（样式在 lib/ui.css）。 */
(async () => {
  if (window.top !== window || window.__captae) return;
  window.__captae = true;
  const { icon, logo, ring, setRing, doneMark, h, mb, mbOf, speedOf, clock, host } = await import(chrome.runtime.getURL('lib/kit.js'));
  const { tr, tAny, N_, setLang, lang } = await import(chrome.runtime.getURL('lib/i18n.js'));

  /* ---------------- 外壳 ---------------- */

  /* 字体要声明在网页上（Shadow DOM 里的 @font-face 不生效）。名字带 Captae 前缀，不和网页自己的 Geist 混在一起 */
  const LATIN = 'U+0000-00FF,U+0131,U+0152-0153,U+02BB-02BC,U+02C6,U+02DA,U+02DC,U+2000-206F,U+20AC,U+2122,U+2191,U+2193,U+2212,U+2215,U+FEFF,U+FFFD';
  const ff = document.createElement('style');
  ff.textContent = `@font-face{font-family:"Captae Geist";src:url(${chrome.runtime.getURL('fonts/geist-latin.woff2')}) format("woff2");font-weight:100 900;font-display:swap;unicode-range:${LATIN}}
@font-face{font-family:"Captae Geist Mono";src:url(${chrome.runtime.getURL('fonts/geist-mono-latin.woff2')}) format("woff2");font-weight:100 900;font-display:swap;unicode-range:${LATIN}}`;
  (document.head || document.documentElement).appendChild(ff);

  const hostEl = document.createElement('captae-ui');
  hostEl.style.cssText = 'all:initial;position:fixed;inset:0;pointer-events:none;z-index:2147483646;';
  const root = hostEl.attachShadow({ mode: 'closed' });
  const style = document.createElement('style');
  style.textContent = await fetch(chrome.runtime.getURL('lib/ui.css')).then(r => r.text());
  const dockEl = h('div', { class: 'cx-dock' });
  root.append(style, dockEl);
  document.documentElement.appendChild(hostEl);

  /* 插件被重新加载或停用后，这个旧脚本还留在网页里但已经和插件断开：收起自己，不留一堆点不动的按钮 */
  let dead = false;
  function teardown() {
    if (dead) return;
    dead = true;
    hostEl.remove();
    ff.remove();
  }
  /** 和后台说话；失败时错误上带着 code（HOST_MISSING…）和 fail（桌面端分类过的失败：{code, message, raw, action}） */
  function send(m) {
    if (dead) return Promise.reject(new Error('GONE'));
    try {
      return chrome.runtime.sendMessage(m).then(r => {
        if (!r) throw new Error(tr('没有回应'));
        if (!r.ok) throw Object.assign(new Error(r.e || tr('没办成')), { code: r.code, fail: r.fail, reason: r.reason, login: r.login });
        return r.v;
      }, e => {
        if (!chrome.runtime?.id) teardown();
        throw e;
      });
    } catch (e) {
      teardown();
      return Promise.reject(e);
    }
  }

  const KEY = /Mac/.test(navigator.platform) ? '⌘V' : 'Ctrl+V';
  /* 去哪里打开、怎么装 Captae：macOS「应用程序」，Windows 开始菜单、安装程序，Linux 应用列表、deb / AppImage（原文都写成字面量，scripts/i18n.py 才找得到） */
  const OS = /Mac/.test(navigator.platform) ? 'mac' : /Win/.test(navigator.platform) ? 'win' : 'linux';
  const wakeFailed = () => OS === 'mac' ? tr('没能打开 Captae。在「应用程序」里打开它，再点一次「再试一次」。')
    : OS === 'win' ? tr('没能打开 Captae。从开始菜单打开它，再点一次「再试一次」。')
    : tr('没能打开 Captae。从应用列表里打开它，再点一次「再试一次」。');
  const installStep = () => OS === 'mac' ? tr('装好 Captae：打开安装包，把 Captae 拖进「Applications」。')
    : OS === 'win' ? tr('装好 Captae：运行 Captae 的安装程序。')
    : tr('装好 Captae：安装 .deb 包或 AppImage。');
  /* 最近一次操作是不是键盘：只有用键盘时，新出现的卡片才把焦点接过去（鼠标用户不需要，也别从网页里抢焦点） */
  let byKey = false;
  addEventListener('keydown', e => { if (!e.metaKey && !e.ctrlKey) byKey = true; }, true);
  addEventListener('pointerdown', () => { byKey = false; }, true);
  const focusIfKey = el => { if (byKey && el) el.focus({ preventScroll: true }); };
  let S = { inj: true, intro: false, qpref: '' };
  try { S = { ...S, ...(await send({ t: 'get-settings' })) }; } catch { /* 用默认值 */ }
  // 界面语言由后台定（Captae 里选的，或者浏览器的界面语言）；换了语言以后新出来的按钮、卡片用新的
  await setLang(S.lang || 'zh-CN').catch(() => {});
  hostEl.setAttribute('lang', lang());

  chrome.runtime.onMessage.addListener((m, _sender, reply) => {
    if (m.t === 'collect') { reply(domMedia()); return; }
    if (m.t === 'settings') { S = { ...S, ...m.s }; if (S.lang && S.lang !== lang()) setLang(S.lang).then(() => hostEl.setAttribute('lang', lang()), () => {}); scan(); }
    if (m.t === 'share-item') shareAI({ itemId: m.itemId, name: m.name, img: m.img, dur: m.dur });
    // 右键菜单只知道地址：先用地址占位，Captae 读到标题后换上（guess）
    if (m.t === 'ctx-download') startDownload({ url: m.url }, { q: 'best' }, { name: shortUrl(m.url), img: m.img || '', guess: true });
    if (m.t === 'ctx-share') shareAI({ url: m.url });
    if (m.t === 'ctx-ximg') xImage(postFromUrl(m.url));
    if (m.t === 'ctx-md') saveArticle();
  });

  const og = p => document.querySelector(`meta[property="${p}"],meta[name="${p}"]`)?.content || '';
  /* og 标签是首次加载时的；单页应用（YouTube）换了视频以后就过时了，只在网址没变时用 */
  const firstHref = location.href;
  const fresh = () => location.href === firstHref;
  const pageImg = () => (fresh() ? og('og:image') || og('twitter:image') : '');
  const tidy = t => (t || '').replace(/\s*[-|–_]\s*(YouTube|哔哩哔哩.*|bilibili.*)$/i, '').trim();
  const pageTitle = () => tidy(fresh() && og('og:title')) || tidy(document.title) || host(location.href);
  /* 右键菜单下载时，Captae 读到标题之前卡片上先用地址当名字：去掉协议和 www，最多 60 个字 */
  const shortUrl = u => String(u || '').replace(/^https?:\/\/(www\.)?/, '').slice(0, 60);

  /** 网页元素里的大图（至少 800×450）、有直链地址的视频和音频：补上浏览器缓存里、旁听不到请求的那些 */
  function domMedia() {
    const out = [];
    for (const im of document.images) {
      const w = im.naturalWidth, hh = im.naturalHeight;
      if (w >= 800 && hh >= 450 && /^https?:/.test(im.currentSrc || im.src)) out.push({ kind: 'image', url: im.currentSrc || im.src, w, h: hh });
    }
    for (const m of document.querySelectorAll('video, audio')) {
      const u = m.currentSrc || m.src || m.querySelector('source[src]')?.src || '';
      if (/^https?:/.test(u)) out.push({ kind: m.tagName === 'AUDIO' ? 'audio' : 'video', url: u });
    }
    return out;
  }
  function reportPage() {
    const v = mainVideo();
    send({ t: 'page', image: pageImg(), title: fresh() ? og('og:title') : '', duration: v && isFinite(v.duration) ? v.duration : 0, hasVideo: !!v, media: domMedia() }).catch(() => {});
  }

  /* ---------------- 右下角的卡片栈 ----------------
   * 最新的在最下面。每张卡片的位置 = 下面那些卡片的高度之和 + 间距，写进它自己的 --y；
   * 增删和内容变高时重新排（ResizeObserver），CSS transition 负责移动（可打断）。 */
  const GAP = 10;
  const stack = [];
  function layout() {
    let y = 0;
    for (let i = stack.length - 1; i >= 0; i--) {
      stack[i].style.setProperty('--y', `${-y}px`);
      y += stack[i].offsetHeight + GAP;
    }
  }
  const ro = new ResizeObserver(() => layout());
  /** 放进卡片栈。swapFor：收起 / 展开时换掉原来那一张，位置不变 */
  function dockIn(el, swapFor) {
    if (swapFor && stack.includes(swapFor)) {
      el.dataset.swap = '';
      stack.splice(stack.indexOf(swapFor), 1, el);
      el.style.setProperty('--y', swapFor.style.getPropertyValue('--y'));
      dockOut(swapFor, 'swap');
    } else {
      stack.push(el);
    }
    dockEl.append(el);
    ro.observe(el);
    layout();
    // 太多了：收走最早的一张不在进行中的
    if (stack.length > 4) {
      const old = stack.find(x => !x._busy);
      if (old) old._close ? old._close() : dockOut(old);
    }
  }
  /** 退场：立刻不占位置（其余卡片同时合拢），淡出后移除 */
  function dockOut(el, how = '') {
    if (!el || !el.isConnected || el.dataset.out != null) return;
    const i = stack.indexOf(el);
    if (i >= 0) stack.splice(i, 1);
    ro.unobserve(el);
    if (el.contains(root.activeElement)) root.activeElement.blur();
    el.dataset.out = how;
    layout();
    const gone = () => el.remove();
    el.addEventListener('transitionend', e => { if (e.target === el && e.propertyName === 'opacity') gone(); });
    setTimeout(gone, 450);
  }
  /** 自动收起：鼠标在卡片上、或者焦点在卡片里时暂停计时（配方 h2） */
  function autoClose(el, ms, close = () => dockOut(el)) {
    let t = 0;
    const arm = () => { clearTimeout(t); t = setTimeout(() => { if (!el.matches(':hover') && !el.contains(root.activeElement)) close(); else arm(); }, ms); };
    el.addEventListener('mouseenter', () => clearTimeout(t));
    el.addEventListener('mouseleave', arm);
    arm();
    return () => clearTimeout(t);
  }

  /* ---------------- 卡片的零件 ---------------- */
  const stopEv = e => e.stopPropagation();
  function card(cls = '') {
    const el = h('div', { class: `cx cx-card cx-glass ${cls}`, attrs: { role: 'group' } });
    el.addEventListener('pointerdown', stopEv);
    return el;
  }
  /** 卡片顶上一行：标（或状态图标）+ 标题 + 关闭 */
  function topBar(title, { mark, tone, onX, xLabel = tr('关闭') } = {}) {
    const st = h('span', { class: 'st', data: { tone } }, mark || logo(20));
    // 只有标题是读屏的播报区：状态变了（下载好了、没下载成功）念一句，进度数字不念
    const t = h('span', { class: 't', text: title, attrs: { 'aria-live': 'polite' } });
    const x = onX ? h('button', { class: 'cx-x', attrs: { type: 'button', 'aria-label': xLabel }, on: { click: onX } }, icon('close', 14)) : null;
    return { el: h('div', { class: 'top' }, st, t, x), st, t, x };
  }
  function setThumb(th, src) {
    if (!src || th._src === src) return;
    th._src = src;
    // 读到了才放进去，淡入动画对着真正出现的那一刻
    const im = new Image();
    im.alt = '';
    im.referrerPolicy = 'no-referrer';
    im.onload = () => { if (th._src === src) { th.querySelector('img')?.remove(); th.append(im); } };
    im.src = src;
  }
  /** 缩略图 + 名字 + 一行小字 */
  function mediaRow(name, meta, img, kind = 'video') {
    const th = h('span', { class: 'cx-th' }, icon(kind === 'image' ? 'image' : kind === 'audio' ? 'music' : kind === 'doc' ? 'doc' : 'video', 18));
    setThumb(th, img);
    const nm = h('span', { class: 'nm', text: name || '', attrs: { title: name || '' } });
    const m = h('span', { class: 'm', text: meta || '' });
    return { el: h('div', { class: 'cx-row' }, th, h('div', { style: 'min-width:0' }, nm, m)), th, nm, m };
  }
  function btn(text, onClick, { primary, danger, ico, label } = {}) {
    return h('button', { class: 'cx-b' + (primary ? ' primary' : '') + (danger ? ' danger' : ''), attrs: { type: 'button', 'aria-label': label }, on: { click: onClick } }, ico, text);
  }
  /* 录直播中的红点：静止（常驻界面不放循环动画，同桌面端 .cp-rec-dot） */
  const recDot = () => h('span', { class: 'cx-rec-dot', attrs: { 'aria-hidden': 'true' } });
  function note(tone, ico, ...kids) {
    return h('div', { class: 'cx-note', data: { tone } }, icon(ico, 16), h('div', { class: 'grow' }, ...kids));
  }
  const spinner = () => h('span', { class: 'cx-spin', attrs: { 'aria-hidden': 'true' } }, ...Array.from({ length: 8 }, () => h('i')));
  const open = target => send({ t: 'open', target }).catch(e => problem(e));
  /** 句子里嵌一个元素（插件没有 tx）：译文里的 {name} 换成元素 el，返回 [前半句, el, 后半句]，交给 h() 当子节点 */
  const withEl = (s, name, el) => {
    const i = s.indexOf(`{${name}}`);
    return i < 0 ? [s, el] : [s.slice(0, i), el, s.slice(i + name.length + 2)];
  };

  /** 一张简单的提示卡片，几秒后自己收起 */
  function noteCard(title, text, { tone = 'ok', ico = 'done', ms = 3200 } = {}) {
    const el = card();
    const tb = topBar(title, { onX: () => dockOut(el) });
    el.append(tb.el);
    if (text) el.append(note(tone, ico, text));
    dockIn(el);
    autoClose(el, ms);
    return el;
  }

  /* ---------------- 出了问题怎么说 ---------------- */

  /* 桌面端分类过的失败（错误码见 crates/captae-core/src/errors.rs）：标题 + 下一步。文字用 N_ 标出来，显示时（failOf）按当时的语言翻译 */
  const FAIL = {
    need_login: { t: N_('要登录才能下载'), tone: 'warn', ico: 'lock' },
    need_verify: { t: N_('要先在网页上确认一下'), tone: 'warn', ico: 'shield' },
    drm: { t: N_('这个视频不能下载'), tone: 'err', ico: 'lock' },
    geo: { t: N_('这个地区看不了'), tone: 'warn', ico: 'alert', hint: N_('换个网络或代理后，可以在 Captae 里重试。') },
    blocked: { t: N_('连接被拦截了'), tone: 'warn', ico: 'offline', hint: N_('检查一下代理设置，然后在 Captae 里重试。') },
    unavailable: { t: N_('视频不在了'), tone: 'err', ico: 'alert' },
    unsupported: { t: N_('下载引擎读不了这个网页'), tone: 'warn', ico: 'info', hint: N_('如果网页上有视频，先点播放，再点工具栏上的 Captae 图标，从「这个页面能下载的」里选。') },
    no_media: { t: N_('没找到视频'), tone: 'warn', ico: 'info', hint: N_('有的网站要先点播放，视频才会出现。') },
    live_upcoming: { t: N_('直播还没开始'), tone: 'warn', ico: 'clock' },
    live: { t: N_('这是正在进行的直播'), tone: 'warn', ico: 'video', hint: N_('可以从现在开始录，录到你在 Captae 里停下为止。') },
    rate_limited: { t: N_('网站让我们慢一点'), tone: 'warn', ico: 'clock', hint: N_('Captae 会过一会儿自动再试，不用管它。') },
    network: { t: N_('网络不太稳'), tone: 'warn', ico: 'offline', hint: N_('可以在 Captae 里重试。') },
    disk: { t: N_('存不下了'), tone: 'err', ico: 'alert', hint: N_('在 Captae 的设置里换一个保存位置。') },
    tool: { t: N_('下载组件出了问题'), tone: 'err', ico: 'alert', hint: N_('在 Captae 的设置里检查下载引擎。') },
    unknown: { t: N_('没下载成功'), tone: 'err', ico: 'alert' },
  };
  /* 图片帖（小红书图文、抖音和 TikTok 图集、Instagram 多图……）：桌面端只下视频和声音，图片在插件里下 */
  const IMAGE_POST = { t: N_('这是图片帖'), tone: 'warn', ico: 'info', hint: N_('点工具栏上的 Captae 图标，从「这个页面能下载的」里选要的图片。') };
  // reason 是后端给的细分（errors.rs）；看文字那条照顾旧版 Captae
  const isImagePost = f => !!f && f.code === 'unsupported' && (f.reason === 'image_post' || /图片帖/.test(f.message || ''));
  /** 失败的标题、下一步（按现在的语言）和图标 */
  const failOf = f => {
    const k = isImagePost(f) ? IMAGE_POST : FAIL[f && f.code] || FAIL.unknown;
    return { ...k, t: tr(k.t), hint: k.hint ? tr(k.hint) : '' };
  };
  /* 桌面端的说法后半句「可以用浏览器插件在网页里下」是说给在 Captae 里粘贴链接的人听的；已经在插件里了，去掉这半句，下一步看提示 */
  function msgOf(f, fallback) {
    if (isImagePost(f)) return tr('下载引擎只下视频和声音，图片由插件在网页里下。');
    const m = ((f && (f.brief || f.message)) || fallback || '').replace(/[，,。]\s*(可以)?用浏览器插件在网页里下(载|图片)?[。.]?$/, '');
    return m || fallback || tr('没下载成功');
  }
  /* 要登录、要验证：插件下载时会带上浏览器里这个网站的登录状态，所以下一步是在网页上登录（或验证）后从这里再下载一次。
   * login 是后台告诉的这次的情况：sent 带上了 / off 面板里这个网站的开关关着 / none 这个网站没有登录状态 */
  const needsLogin = f => !!f && (f.code === 'need_login' || f.code === 'need_verify');
  /* 失败原因里的网站名字（fail.site，「芒果TV」「哔哩哔哩」，crates/captae-core/src/errors.rs 的 site_name）是桌面端存的中文原文，显示时用 tAny 翻译。
   * 这里用 N_ 标出来，词表里才有它们；后端加了新的中文站点名，这里也要加（同 apps/desktop/src/i18n/sites.ts） */
  const SITE_NAMES = [N_('哔哩哔哩'), N_('抖音'), N_('微博'), N_('小红书'), N_('快手'), N_('微信'), N_('腾讯视频'), N_('爱奇艺'), N_('优酷'), N_('西瓜视频'), N_('知乎'), N_('芒果TV'), N_('网易云音乐'), N_('喜马拉雅')];
  function hintOf(f, login) {
    if (!needsLogin(f)) return failOf(f).hint;
    if (login === 'off') return tr('工具栏里 Captae 面板的「带上这个网站的登录状态」关着。打开它，再下载一次。');
    if (f.code === 'need_verify') return tr('在这个网页上把视频播放一次（有验证就先完成），再下载一次。');
    // 网站名字用桌面端说法里的那一个（fail.site），和上面那句话对得上；中文里夹英文名字时自动空一格（「登录 YouTube」「登录芒果TV」）
    return f.site
      ? tr('先在网页上登录{site}（已经登录了，就确认这个账号能看这个视频），再下载一次。', { site: tAny(f.site) })
      : tr('先在这个网页上登录（已经登录了，就确认这个账号能看这个视频），再下载一次。');
  }

  /** 没装、没开、连不上 Captae，或者桌面端报的失败：都在右下角说清楚，能接着做的给按钮 */
  function problem(e, retry) {
    const code = e && (e.code || e.message) || '';
    if (/HOST_MISSING/.test(code)) return needInstall(retry);
    if (/HOST_BROKEN/.test(code)) return hostBroken(e.reason, retry);
    // 等太久没回话：Captae 是开着的（只是忙），东西可能已经交过去了，别说「先打开 Captae」
    if (/TIMEOUT/.test(code)) return slowApp(retry);
    if (/APP_NOT_RUNNING|HOST_GONE/.test(code)) return needOpen(retry);
    if (/GONE/.test(code)) return null;
    if (e && e.fail) return failCard(e.fail, { retry });
    const el = card();
    el.append(topBar(tr('没办成'), { mark: icon('alert', 18), tone: 'err', onX: () => dockOut(el) }).el, note('err', 'alert', (e && e.message) || tr('出了点问题')));
    dockIn(el);
    autoClose(el, 6000);
    return el;
  }

  function needOpen(retry) {
    const el = card();
    const go = btn(tr('打开 Captae'), async () => {
      go.disabled = true;
      go.replaceChildren(spinner(), tr('正在打开…'));
      try {
        await send({ t: 'open', target: {} });
        dockOut(el);
        noteCard(tr('Captae 已连上'), retry ? tr('接着刚才的事继续。') : tr('现在可以下载了。'), { ms: 2400 });
        retry && retry();
      } catch {
        go.disabled = false;
        go.textContent = tr('再试一次');
        msg.replaceChildren(wakeFailed());
      }
    }, { primary: true });
    const msg = h('span', { text: tr('下载要靠电脑上的 Captae 完成，它现在没有打开。') });
    el.append(topBar(tr('先打开 Captae'), { onX: () => dockOut(el) }).el, note('warn', 'info', msg), h('div', { class: 'cx-foot' }, btn(tr('稍后'), () => dockOut(el)), go));
    dockIn(el);
    focusIfKey(go);
    return el;
  }

  function slowApp(retry) {
    const el = card();
    el.append(
      topBar(tr('Captae 一直没回话'), { mark: icon('clock', 18), tone: 'warn', onX: () => dockOut(el) }).el,
      note('warn', 'info', tr('等了一会儿 Captae 没有回应，下载可能已经交给它了。'), h('small', { text: tr('先在 Captae 的下载列表里看看；没有的话再试一次。') })),
      h('div', { class: 'cx-foot' },
        btn(tr('在 Captae 里查看'), () => { dockOut(el); open({ page: 'downloads' }); }),
        retry ? btn(tr('再试一次'), () => { dockOut(el); retry(); }, { primary: true }) : btn(tr('好的'), () => dockOut(el), { primary: true })));
    dockIn(el);
    return el;
  }

  function needInstall(retry) {
    const el = card();
    const again = btn(tr('再试一次'), async () => {
      again.disabled = true;
      again.replaceChildren(spinner(), tr('正在检查…'));
      const st = await send({ t: 'state' }).catch(() => null);
      if (st && st.conn === 'ok') { dockOut(el); noteCard(tr('Captae 已连上'), retry ? tr('接着刚才的事继续。') : tr('现在可以下载了。'), { ms: 2400 }); retry && retry(); return; }
      if (st && st.conn === 'off') { dockOut(el); needOpen(retry); return; }
      again.disabled = false;
      again.textContent = tr('再试一次');
    }, { primary: true });
    el.append(
      topBar(tr('电脑上还没有 Captae'), { onX: () => dockOut(el) }).el,
      note('warn', 'info',
        tr('下载和保存都由电脑上的 Captae 完成。'),
        h('ol', null,
          h('li', { text: installStep() }),
          h('li', { text: tr('打开一次 Captae，它会自己连上这个浏览器。') })),
        h('small', { text: tr('已经装了还连不上：在 Captae 的「浏览器插件」页点「修复」。') })),
      h('div', { class: 'cx-foot' }, btn(tr('好的'), () => dockOut(el)),
        btn(tr('下载 Captae'), () => window.open('https://github.com/sol1560/Captae/releases/latest', '_blank', 'noopener')), again));
    dockIn(el);
    focusIfKey(again);
    return el;
  }

  function hostBroken(reason, retry) {
    const el = card();
    const why = reason === 'forbidden'
      ? tr('Captae 没认出这个插件（插件的身份对不上）。')
      : tr('Captae 的连接程序没能启动，可能 Captae 被移动或删掉了。');
    el.append(
      topBar(tr('连不上 Captae'), { mark: icon('offline', 18), tone: 'warn', onX: () => dockOut(el) }).el,
      note('warn', 'alert', why, h('small', { text: tr('打开 Captae，到「浏览器插件」页点「修复」，然后再试一次。') })),
      h('div', { class: 'cx-foot' }, btn(tr('好的'), () => dockOut(el)), btn(tr('再试一次'), () => { dockOut(el); retry ? retry() : send({ t: 'state' }).catch(() => {}); }, { primary: true })));
    dockIn(el);
    return el;
  }

  /** 读取或下载失败（还没有任务的时候，比如菜单里读清晰度失败后仍然点了下载） */
  function failCard(f, { retry, name, img, login } = {}) {
    const k = failOf(f);
    const el = card();
    const kids = [topBar(k.t, { mark: icon(k.ico, 18), tone: k.tone, onX: () => dockOut(el) }).el];
    if (name) kids.push(mediaRow(name, host(location.href), img).el);
    const hint = hintOf(f, login);
    kids.push(note(k.tone, k.ico, msgOf(f, k.t), hint ? h('small', { text: hint }) : null));
    const foot = h('div', { class: 'cx-foot' }, btn(tr('好的'), () => dockOut(el)));
    if (f.code === 'unknown' && f.raw) foot.prepend(btn(tr('复制错误信息'), e => copyText(f.raw, e.currentTarget), { ico: icon('copy', 14) }));
    if (retry && ['network', 'geo', 'blocked', 'unknown', 'need_login', 'need_verify'].includes(f.code)) foot.append(btn(needsLogin(f) ? tr('再下载一次') : tr('再试一次'), () => { dockOut(el); retry(); }, { primary: true }));
    kids.push(foot);
    el.append(...kids);
    dockIn(el);
    return el;
  }

  async function copyText(text, b) {
    try {
      await navigator.clipboard.writeText(text);
      b.replaceChildren(icon('tick', 14), tr('已复制'));
    } catch {
      b.textContent = tr('复制不了');
    }
  }

  /* ---------------- 下载 ---------------- */

  /** target：{itemId}（插件嗅探到的条目）或 {url}；ch：清晰度选择；info：卡片上显示的名字、封面、清晰度和大小 */
  async function startDownload(target, ch, info = {}, force = false) {
    endIntro();
    if (!force) {
      let dup = null;
      try { dup = await send({ t: 'check', ...target }); } catch { /* 查不了就直接下 */ }
      if (dup && dup.found) return dupCard(dup, info, () => startDownload(target, ch, info, true));
    }
    const jc = new JobCard(info);
    let job;
    try {
      job = await send(target.itemId ? { t: 'download', itemId: target.itemId, ch } : { t: 'download-url', url: target.url, ch });
    } catch (e) {
      jc.drop();
      return e.fail ? failCard(e.fail, { name: info.name, img: info.img, login: e.login, retry: () => startDownload(target, ch, info, true) }) : problem(e, () => startDownload(target, ch, info, true));
    }
    jc.attach(job, { target, ch });
  }

  /** 这个视频已经下载过了：打开看看，或者再下一次 */
  function dupCard(d, info, again) {
    const el = card();
    const see = btn(tr('打开看看'), () => { dockOut(el); open({ asset: d.assetId }); }, { primary: true });
    el.append(
      topBar(tr('这个视频下载过了'), { mark: icon('done', 18), tone: 'ok', onX: () => dockOut(el) }).el,
      mediaRow(d.title || info.name || '', [tr('在 {where} 里', { where: d.where || 'Captae' }), d.dur].filter(Boolean).join(' · '), info.img, d.kind).el,
      h('div', { class: 'cx-foot' }, btn(tr('再下一次'), () => { dockOut(el); again(); }), see));
    dockIn(el);
    focusIfKey(see);
    autoClose(el, 9000);
  }

  /* 进度：所有卡片共用一个轮询（900ms），过渡时长和它一样（配方 c5） */
  const TICK = 900;
  const watching = new Map();
  let pollT = 0;
  function watch(jc) {
    watching.set(jc.job, jc);
    if (!pollT) pollT = setInterval(poll, TICK);
  }
  function unwatch(jc) {
    watching.delete(jc.job);
    if (!watching.size) { clearInterval(pollT); pollT = 0; fails = 0; failSince = 0; nextAt = 0; }
  }
  /* 连不上 Captae 的次数、从什么时候开始连不上、下一次什么时候再问（连不上时放慢到 3 秒一次） */
  let polling = false, fails = 0, failSince = 0, nextAt = 0;
  const LOST_AFTER = 3, LOST_MS = 3000, LOST_EVERY = 3000;
  async function poll() {
    if (polling || !watching.size || Date.now() < nextAt) return;
    polling = true;
    // 只更新这一次问到的任务：请求在路上时新加进来的卡片等下一轮，不然会被当成「在 Captae 里没了」（第三轮插件回归 t3、t4 的偶发问题）
    const ids = [...watching.keys()];
    try {
      const list = await send({ t: 'jobs', ids });
      fails = 0; failSince = 0; nextAt = 0;
      const by = new Map(list.map(j => [j.id, j]));
      for (const id of ids) watching.get(id)?.update(by.get(id) || null);
    } catch (e) {
      if (/APP_NOT_RUNNING|HOST_GONE|HOST_MISSING|TIMEOUT/.test(e.code || e.message)) {
        // 连着几次、过了几秒还连不上才说「Captae 关掉了」：桌面端一时忙得没回话（宿主那边已经多等了一会儿）不算；
        // 说了以后放慢继续问，Captae 重新打开后卡片自己接上
        fails++;
        if (!failSince) failSince = Date.now();
        if (fails >= LOST_AFTER && Date.now() - failSince >= LOST_MS) {
          for (const jc of [...watching.values()]) jc.lost();
          nextAt = Date.now() + LOST_EVERY;
        }
      }
    } finally {
      polling = false;
    }
  }
  /* 录了多久：12:34 / 1:02:03，刚开始是 0:00 */
  const recClock = ms => clock(Math.floor(Math.max(0, ms) / 1000)) || '0:00';

  /** 一个下载任务的卡片：展开是完整的卡片，收起是右下角的小胶囊；两种样子随时互换 */
  class JobCard {
    constructor(info) {
      this.info = info;
      this.job = null;
      this.j = null;
      this.mini = false;
      this.speed = 0;
      this.last = null;
      this.closed = false;
      this.buildFull();
      this.full._busy = true;
      this.full._close = () => this.close();
      // 先占位：交给 Captae 要一会儿（Captae 没开时宿主会先把它打开）
      this.paint({ state: 'queue', phase: 'probe', pending: true });
      dockIn(this.full);
    }
    buildFull() {
      const el = card();
      this.top = topBar(tr('正在交给 Captae…'), { onX: () => this.close(), xLabel: tr('关闭卡片（下载照常进行）') });
      this.row = mediaRow(this.info.name || '', '', this.info.img, this.info.kind);
      this.bar = h('div', { class: 'cx-bar', attrs: { role: 'progressbar', 'aria-label': tr('下载进度'), 'aria-valuemin': 0, 'aria-valuemax': 100 } }, h('i'));
      this.bar.style.setProperty('--tick', TICK + 'ms');
      this.stopB = h('button', { class: 'cx-stop', attrs: { type: 'button', 'aria-label': tr('停止下载'), title: tr('停止下载') }, on: { click: () => this.stop() } }, icon('stop', 16));
      this.prog = h('div', { class: 'cx-prog' }, this.bar, this.stopB);
      this.extra = h('div');
      this.foot = h('div', { class: 'cx-foot' });
      el.append(this.top.el, this.row.el, this.prog, this.extra, this.foot);
      this.full = el;
      this.footKey = '';
    }
    attach(job, again) {
      if (this.closed) return;
      this.job = job.id;
      this.again = again;
      this.login = job.login;
      if (job.title && !this.info.name) this.row.nm.textContent = job.title;
      if (job.thumb && /^https?:/.test(job.thumb)) setThumb(this.row.th, job.thumb);
      this.update(job);
      watch(this);
    }
    drop() {
      this.closed = true;
      dockOut(this.full);
    }
    close() {
      this.closed = true;
      if (this.job != null) unwatch(this);
      clearTimeout(this.autoT);
      this.recTick(false);
      dockOut(this.mini ? this.cap : this.full);
    }
    async stop() {
      if (this.job == null) return;
      this.stopB.disabled = true;
      try {
        await send({ t: 'stop', job: this.job });
      } catch (e) {
        this.stopB.disabled = false;
        return problem(e);
      }
      unwatch(this);
      this.paint({ state: 'stopped' });
    }
    /** 录直播「停止并保存」：录制的进程收尾，录到的部分整理好放进素材库（后端 JobAction::Stop）。点了马上换成「正在保存」，不等下一轮 */
    async saveRec(b) {
      if (this.job == null || !this.j) return;
      b.disabled = true;
      try {
        await send({ t: 'save', job: this.job });
      } catch (e) {
        b.disabled = false;
        // 刚好直播自己结束了（已经在保存）：下一轮轮询会显示真实的样子
        if (e.code === 'NOT_RECORDING' || /只有正在录制/.test(e.message || '')) return;
        return problem(e);
      }
      this.confirming = false;
      this.paint(this.j = { ...this.j, phase: 'merge', rec: { ...this.j.rec, stopping: true, until: Date.now() } });
    }
    /** 取消录制 = 不要了，录到的删掉：先在卡片里问一句（照桌面端的说法），第二次点「取消录制」才生效 */
    askCancelRec(on) {
      this.confirming = on;
      if (this.j) this.paint(this.j);
    }
    async cancelRec(b) {
      if (this.job == null) return;
      b.disabled = true;
      try {
        await send({ t: 'stop', job: this.job });
      } catch (e) {
        b.disabled = false;
        return problem(e);
      }
      unwatch(this);
      this.confirming = false;
      this.paint({ state: 'rec-cancelled' });
    }
    /** 录制中每秒更新一次「已录 12:34」（只换文字，等宽数字，不做动画）；对齐到开始录的那一刻，和桌面端同一秒变 */
    recTick(on) {
      clearTimeout(this.recT);
      this.recT = 0;
      if (!on || !this.j || !this.j.rec) return;
      const since = this.j.rec.since;
      const step = () => {
        if (!this.isRec(this.j)) return this.recTick(false);
        this.row.m.textContent = this.recLine(this.j);
        if (this.mini && this.cap && this.cap._txt) this.cap._txt.textContent = tr('已录{time}', { time: recClock(Date.now() - since) });
        this.recT = setTimeout(step, 1000 - ((Date.now() - since) % 1000) + 5);
      };
      this.recT = setTimeout(step, 1000 - ((Date.now() - since) % 1000) + 5);
    }
    /** 正在录（录制的进程在跑、还没点停止）：同后端 Job::recording */
    isRec(j) {
      const r = j && j.rec;
      return !!r && j.state === 'run' && j.phase === 'dl' && r.until == null && !r.stopping;
    }
    recLine(j) {
      return [tr('正在录制 · 已录{time}', { time: recClock(Date.now() - j.rec.since) }), j.got ? mbOf(j.got) : ''].filter(Boolean).join(' · ');
    }
    lost() {
      this.isLost = true;
      this.recTick(false);
      this.paint({ state: 'lost' });
    }
    /** 轮询拿到的任务；null = 任务在 Captae 里没了（取消了，或者播放列表选好了条目） */
    update(j) {
      if (this.closed) return;
      if (!j) {
        unwatch(this);
        // Captae 关掉又打开了：没下完的任务恢复成了新的任务（编号变了），在 Captae 里接着进行
        if (this.isLost) return this.paint({ state: 'reopened' });
        return this.paint({ state: this.j && this.j.choose ? 'chosen' : 'gone' });
      }
      this.isLost = false;
      // 速度：宿主不回速度，按两次的已下载量自己算，再平滑一下
      const now = performance.now();
      if (j.state === 'run' && j.phase === 'dl' && this.last && j.got >= this.last.got) {
        const dt = (now - this.last.t) / 1000;
        if (dt > 0.3) {
          const v = (j.got - this.last.got) / dt;
          this.speed = this.speed ? this.speed * 0.6 + v * 0.4 : v;
          this.last = { t: now, got: j.got };
        }
      } else if (j.state !== 'run' || j.phase !== 'dl') {
        this.speed = 0;
        this.last = { t: now, got: j.got || 0 };
      }
      if (!this.last) this.last = { t: now, got: j.got || 0 };
      this.j = j;
      if (this.info.guess && j.title && !j.untitled && !/正在读取/.test(j.title) && this.row.nm.textContent !== j.title) {
        this.row.nm.textContent = j.title;
        this.row.nm.title = j.title;
      }
      if (j.state === 'done' || j.state === 'failed') unwatch(this);
      this.paint(j);
    }
    /** 按任务状态画：标题、小字、进度条、底部按钮 */
    paint(j) {
      const pct = j.size > 0 ? Math.min(0.99, (j.got || 0) / j.size) : 0;
      const meta = [this.info.label, this.info.size].filter(Boolean).join(' · ');
      let title = tr('已交给 Captae'), line = '', busy = false, tone = '', bar = true, full = false, mark = null, extra = null, foot = 'run';
      // 录直播：正在录（「已录 12:34」，没有百分比和剩余时间）、正在保存录到的部分
      const rec = j.rec || null;
      const recording = this.isRec(j);
      const saving = !!rec && !recording && (j.state === 'run' || j.state === 'queue');
      if (!recording) this.confirming = false;
      switch (j.state) {
        case 'queue':
          if (saving) {
            title = rec.interrupted ? tr('正在整理上次录到的部分') : tr('正在保存录到的部分');
            line = [rec.interrupted ? tr('Captae 上次退出时还在录') : tr('录完了，正在整理成能播放的文件'), j.got ? mbOf(j.got) : ''].filter(Boolean).join(' · ');
            busy = true;
            foot = 'saving';
            break;
          }
          title = j.pending ? tr('正在交给 Captae…') : tr('已交给 Captae');
          line = j.pending ? meta || tr('马上就好') : tr('排队中…');
          busy = true;
          break;
        case 'run':
          if (recording) {
            title = tr('正在录直播');
            mark = recDot();
            line = this.recLine(j);
            bar = false;
            foot = this.confirming ? 'rec-ask' : 'rec';
            if (this.confirming) extra = note('warn', 'alert', tr('取消这次录制？'), h('small', { text: j.got ? tr('已经录的{size}会被删掉。想留下录到的部分，点「停止并保存」。', { size: mbOf(j.got) }) : tr('已经录到的会被删掉。想留下录到的部分，点「停止并保存」。') }));
          } else if (saving) {
            title = rec.interrupted ? tr('正在整理上次录到的部分') : tr('正在保存录到的部分');
            line = [rec.interrupted ? tr('Captae 上次退出时还在录') : tr('录完了，正在整理成能播放的文件'), j.got ? mbOf(j.got) : ''].filter(Boolean).join(' · ');
            busy = true;
            foot = 'saving';
          } else if (j.phase === 'probe') { line = tr('正在读取视频信息…'); busy = true; }
          else if (j.phase === 'pick') { line = tr('正在选清晰度…'); busy = true; }
          else if (j.phase === 'merge' || j.phase === 'post') { line = tr('快好了，正在生成文件…'); busy = true; }
          else if (j.size > 0) line = [tr('正在下载'), Math.round(pct * 100) + '%', speedOf(this.speed)].filter(Boolean).join(' · ');
          else { line = [tr('正在下载'), j.got ? tr('已下{size}', { size: mbOf(j.got) }) : '', speedOf(this.speed)].filter(Boolean).join(' · '); busy = true; }
          break;
        case 'pause':
          if (j.choose) {
            const n = (j.entries || []).length;
            title = tr('要选一下');
            mark = icon('list', 18);
            tone = 'warn';
            line = n ? tr('一共{n}个视频', { n }) : '';
            bar = false;
            extra = note('warn', 'list', n > 1 ? tr('这是播放列表（或多视频帖子），有{n}个视频。', { n }) : tr('这是一个播放列表。'), h('small', { text: tr('在 Captae 里选要下的，再开始下载。') }));
            foot = 'choose';
          } else {
            line = tr('已暂停');
            tone = 'paused';
          }
          break;
        case 'done':
          title = rec ? tr('录好了') : tr('下载好了');
          mark = doneMark(!this.doneShown);
          tone = 'ok';
          line = rec
            ? [tr('已存进 Captae 素材库'), rec.secs ? tr('录了{time}', { time: recClock(rec.secs * 1000) }) : '', j.size ? mbOf(j.size) : ''].filter(Boolean).join(' · ')
            : [tr('已存进 Captae 素材库'), j.fmt || '', j.size ? mbOf(j.size) : ''].filter(Boolean).join(' · ');
          // 进度条走满留在原处（不收掉这一行）：卡片高度不变，贴着屏幕底边的卡片顶边就不会跳
          full = true;
          foot = 'done';
          break;
        case 'failed': {
          const k = failOf(j.fail);
          title = k.t;
          mark = icon(k.ico, 18);
          tone = k.tone;
          line = '';
          bar = false;
          const hint = hintOf(j.fail, this.login);
          extra = note(k.tone, k.ico, msgOf(j.fail, j.err || tr('没下载成功')), hint ? h('small', { text: hint }) : null);
          foot = j.fail && j.fail.code === 'live' ? 'live' : needsLogin(j.fail) && this.again ? 'relogin' : 'failed';
          break;
        }
        case 'stopped':
          title = tr('已停止');
          mark = icon('stop', 18);
          line = tr('已在 Captae 里取消这个下载');
          bar = false;
          foot = 'none';
          break;
        case 'gone':
          title = tr('已在 Captae 里取消');
          mark = icon('info', 18);
          line = '';
          bar = false;
          foot = 'none';
          break;
        case 'chosen':
          title = tr('已在 Captae 里选好');
          mark = icon('done', 18);
          tone = 'ok';
          line = tr('选中的视频会在 Captae 里接着下载');
          bar = false;
          foot = 'none';
          break;
        case 'lost':
          title = tr('Captae 关掉了');
          mark = icon('offline', 18);
          tone = 'warn';
          line = tr('重新打开 Captae，下载会接着进行');
          bar = false;
          foot = 'lost';
          break;
        case 'reopened':
          title = tr('Captae 重新打开了');
          mark = icon('info', 18);
          line = tr('这个任务在 Captae 的下载列表里');
          bar = false;
          foot = 'reopened';
          break;
        case 'rec-cancelled':
          title = tr('已取消录制');
          mark = icon('stop', 18);
          line = tr('录到的部分已经删掉');
          bar = false;
          foot = 'none';
          break;
      }
      if (j.state === 'done') this.doneShown = true;
      this.state = j.state;
      this.full._busy = ['queue', 'run', 'pause'].includes(j.state);
      if (this.cap) this.cap._busy = this.full._busy;
      this.recTick(recording);

      // 标题和图标：变了才换（图标重新挂上会重播画勾）
      if (this.top.t.textContent !== title) this.top.t.textContent = title;
      const markKey = mark ? j.state + (j.fail ? j.fail.code : '') + (j.choose ? 'c' : '') : 'logo';
      if (this.markKey !== markKey) {
        this.markKey = markKey;
        this.top.st.replaceChildren(mark || logo(20));
        this.top.st.dataset.tone = tone === 'paused' ? '' : tone;
      }
      this.row.m.textContent = line;

      this.prog.hidden = !bar;
      // 不能停的时候去掉停止按钮，进度条占满一行；这一行有最小高度（ui.css .cx-prog），卡片不会变矮。
      // 录直播（在录、在保存）不放这个按钮：它是「取消」，会删掉录到的；录制中的取消在底下，要先确认
      this.stopB.hidden = !(this.job != null && ['queue', 'run', 'pause'].includes(j.state) && !j.choose && !rec);
      const shown = full ? 1 : pct;
      this.bar.toggleAttribute('data-busy', busy);
      this.bar.dataset.tone = tone === 'paused' ? 'paused' : '';
      this.bar.firstChild.style.clipPath = busy ? '' : `inset(0 ${(1 - shown) * 100}% 0 0 round 99px)`;
      if (busy) this.bar.removeAttribute('aria-valuenow');
      else this.bar.setAttribute('aria-valuenow', Math.round(shown * 100));

      const extraKey = extra ? markKey : '';
      if (this.extraKey !== extraKey) {
        this.extraKey = extraKey;
        this.extra.replaceChildren(...(extra ? [extra] : []));
      }
      this.paintFoot(foot, j);
      if (this.mini) this.paintCap(j, pct, busy, saving ? tr('正在保存') : line);

      // 完成、停止、取消后：一会儿自己收起（鼠标放在上面就不收）
      if (['done', 'stopped', 'gone', 'chosen', 'rec-cancelled', 'reopened'].includes(j.state) && !this.autoT) {
        const el = this.mini ? this.cap : this.full;
        this.autoT = 1;
        autoClose(el, j.state === 'done' || j.state === 'reopened' ? 8000 : 2600, () => this.close());
      }
    }
    paintFoot(kind, j) {
      if (this.footKey === kind) return;
      this.footKey = kind;
      const see = (label = tr('在 Captae 里查看'), primary = true) => btn(label, () => this.see(), { primary });
      const b = [];
      if (kind === 'run') b.push(btn(tr('收起'), () => this.collapse()), see());
      else if (kind === 'done') b.push(btn(tr('好的'), () => this.close()), see());
      else if (kind === 'choose') b.push(btn(tr('好的'), () => this.close()), see(tr('在 Captae 里选')));
      else if (kind === 'live') b.push(btn(tr('好的'), () => this.close()), see(tr('在 Captae 里查看'), false), btn(tr('从现在开始录'), () => this.recordLive(), { primary: true }));
      // 要登录、要验证：在网页上登录（或验证）后从这里再下载一次，会带上新的登录状态；在 Captae 里重试是带不上的
      else if (kind === 'relogin') b.push(see(tr('在 Captae 里查看'), false), btn(tr('再下载一次'), () => this.redownload(), { primary: true }));
      else if (kind === 'failed') {
        if (j.fail && j.fail.code === 'unknown' && j.fail.raw) b.push(btn(tr('复制错误信息'), e => copyText(j.fail.raw, e.currentTarget), { ico: icon('copy', 14) }));
        b.push(btn(tr('好的'), () => this.close()), see());
      } else if (kind === 'lost') b.push(btn(tr('好的'), () => this.close()), btn(tr('打开 Captae'), () => { open({}); this.close(); }, { primary: true }));
      // 录直播：主按钮「停止并保存」（录到的整理好放进素材库）；「取消录制」会删掉录到的，先问一句
      else if (kind === 'rec') {
        const save = btn(tr('停止并保存'), () => this.saveRec(save), { primary: true, ico: icon('stop', 14) });
        b.push(btn(tr('收起'), () => this.collapse()), btn(tr('取消录制'), () => this.askCancelRec(true)), save);
      } else if (kind === 'rec-ask') {
        // 问的那句话说了「想留下录到的部分，点『停止并保存』」，这个按钮还在；连点两下时第二下落在它上面，不会误删
        const save = btn(tr('停止并保存'), () => this.saveRec(save));
        const yes = btn(tr('取消录制'), () => this.cancelRec(yes), { danger: true });
        b.push(btn(tr('继续录'), () => this.askCancelRec(false)), save, yes);
      } else if (kind === 'saving') b.push(btn(tr('收起'), () => this.collapse()), see());
      // Captae 关掉又打开了：任务编号变了，只打开下载页
      else if (kind === 'reopened') b.push(btn(tr('好的'), () => this.close()), btn(tr('在 Captae 里查看'), () => { open({ page: 'downloads' }); this.close(); }, { primary: true }));
      else if (kind === 'none') b.push(btn(tr('好的'), () => this.close()));
      const had = this.foot.contains(root.activeElement);
      this.foot.replaceChildren(...b);
      // 焦点原来在底部按钮上：落到最后一个（主按钮）；问要不要取消录制时落到「继续录」，回车不会误删
      if (had) (kind === 'rec-ask' ? b[0] : b[b.length - 1])?.focus({ preventScroll: true });
    }
    /** 在 Captae 里查看：下好了直接打开素材，没下好打开下载页（带上任务号，桌面端支持后直接选中这一条） */
    see() {
      const j = this.j;
      if (j && j.state === 'done' && j.assetId != null) open({ asset: j.assetId });
      else open({ page: 'downloads', job: this.job });
      if (j && ['done', 'failed'].includes(j.state)) this.close();
    }
    /** 再下载一次：先把 Captae 里这条失败的任务去掉（不留两条），再用新的登录状态重新下 */
    redownload() {
      const a = this.again, old = this.job;
      this.close();
      if (old != null) send({ t: 'stop', job: old }).catch(() => {});
      if (a) startDownload(a.target, a.ch, this.info, true);
    }
    /** 从现在开始录：录制是新加的一个任务；Captae 里那条「这是正在进行的直播」先去掉，不留两条（和桌面端、「再下载一次」一样） */
    recordLive() {
      const a = this.again, old = this.job;
      this.close();
      if (old != null) send({ t: 'stop', job: old }).catch(() => {});
      if (a) startDownload(a.target, { ...a.ch, live: true }, this.info, true);
    }
    /* 收起：卡片往右下角缩小淡出，小胶囊从同一个角放大出来（配方 l3 第一档，只动 transform 和 opacity） */
    collapse() {
      if (this.mini || this.closed) return;
      const kb = this.full.contains(root.activeElement) && root.activeElement.matches(':focus-visible');
      this.mini = true;
      this.cap = h('button', { class: 'cx cx-cap cx-glass', attrs: { type: 'button', 'aria-label': tr('展开下载卡片') }, on: { click: () => this.expand(), pointerdown: stopEv } });
      this.cap._busy = this.full._busy;
      this.cap._close = () => this.close();
      const j = this.j || { state: 'queue' };
      this.paintCap(j, j.size > 0 ? Math.min(0.99, j.got / j.size) : 0, !(j.size > 0), '');
      dockIn(this.cap, this.full);
      if (kb) this.cap.focus({ preventScroll: true });
    }
    expand() {
      if (!this.mini || this.closed) return;
      const done = this.j && this.j.state === 'done';
      if (done && this.j.assetId != null) { open({ asset: this.j.assetId }); return this.close(); }
      this.mini = false;
      this.buildFull();
      this.full._busy = this.cap._busy;
      this.full._close = () => this.close();
      this.markKey = this.extraKey = this.footKey = null;
      this.paint(this.j || { state: 'queue', pending: true });
      dockIn(this.full, this.cap);
      this.cap = null;
    }
    paintCap(j, pct, busy, line) {
      const c = this.cap;
      if (!c) return;
      let key = j.state;
      if (j.state === 'run' || j.state === 'queue' || j.state === 'pause') key = 'run';
      // 正在录：红点 +「已录 12:34」（每秒由 recTick 更新）
      if (this.isRec(j)) key = 'rec';
      if (c._key !== key) {
        c._key = key;
        if (key === 'rec') {
          c._txt = h('span', { class: 'num', text: tr('已录{time}', { time: recClock(Date.now() - j.rec.since) }) });
          c._name = h('span', { class: 't', text: this.row.nm.textContent });
          c.replaceChildren(recDot(), c._txt, c._name);
          c.setAttribute('aria-label', tr('展开录制卡片'));
        } else if (key === 'run') {
          c._ring = ring(pct, busy);
          c._txt = h('span', { class: 'num' });
          c._name = h('span', { class: 't', text: this.row.nm.textContent });
          c.replaceChildren(c._ring, c._txt, c._name);
        } else if (key === 'done') {
          c.replaceChildren(h('span', { class: 'ok' }, doneMark(true)), tr('下载好了'));
          c.setAttribute('aria-label', tr('下载好了，在 Captae 里打开'));
        } else {
          const k = j.state === 'failed' ? failOf(j.fail) : null;
          c.replaceChildren(h('span', { class: k ? 'bad' : '' }, icon(k ? k.ico : 'info', 16)), k ? k.t : this.top.t.textContent);
        }
      }
      if (key === 'run') {
        setRing(c._ring, pct, busy);
        c._txt.textContent = j.state === 'pause' && !j.choose ? tr('已暂停') : j.choose ? tr('要选一下') : busy ? (line || tr('准备中')).replace(/…$/, '') : Math.round(pct * 100) + '%';
      } else if (key === 'rec') {
        c._txt.textContent = tr('已录{time}', { time: recClock(Date.now() - j.rec.since) });
      }
    }
  }

  /* ---------------- 发给 AI ---------------- */
  const AI_STEPS = [
    { t: N_('下载视频'), d: N_('用清晰度够 AI 看清的版本'), upto: 45 },
    { t: N_('把说的话转成文字'), d: N_('带时间点'), upto: 75 },
    { t: N_('挑出关键画面'), d: N_('从头到尾每一段挑一张'), upto: 97 },
    { t: N_('整理好，放进剪贴板'), d: '', upto: 100 },
  ];
  let aiRun = null;
  async function shareAI({ itemId, url, name, img, dur }) {
    endIntro();
    // 一次只准备一个：上一个还在跑就把它那张卡片抖到前面
    if (aiRun && !aiRun.over) {
      focusIfKey(aiRun.el.querySelector('.cx-b'));
      return;
    }
    name = name || pageTitle();
    const el = card();
    const run = { el, task: null, over: false, timer: 0 };
    aiRun = run;
    const cancel = () => {
      if (run.over) return;
      run.over = true;
      clearInterval(run.timer);
      if (run.task) send({ t: 'task-cancel', id: run.task }).catch(() => {});
      dockOut(el);
      noteCard(tr('已取消'), tr('没有复制任何内容。'), { tone: '', ico: 'info', ms: 2400 });
    };
    const steps = AI_STEPS.map(s => {
      const ico = h('span', { class: 'ico' });
      const pct = h('span', { class: 'pct num' });
      const li = h('li', { class: 'cx-step' }, ico, h('span', { class: 'lb' }, tr(s.t), s.d ? h('span', { class: 'd', text: tr(s.d) }) : null), pct);
      return { li, ico, pct, s: '' };
    });
    el.append(
      topBar(tr('正在准备发给 AI'), { mark: icon('ai', 18), onX: cancel, xLabel: tr('取消') }).el,
      mediaRow(name, [dur, tr('来自{host}', { host: host(location.href) })].filter(Boolean).join(' · '), img).el,
      h('ol', { class: 'cx-steps' }, ...steps.map(s => s.li)),
      h('div', { class: 'cx-foot' }, btn(tr('取消'), cancel)));
    el._busy = true;
    el._close = cancel;
    const paint = p => steps.forEach((st, i) => {
      const lo = i ? AI_STEPS[i - 1].upto : 0, hi = AI_STEPS[i].upto;
      const s = p >= hi ? 'done' : p >= lo ? 'now' : 'todo';
      if (st.s !== s) {
        // 刚完成的那一步画圈打勾；一开始就已经完成的直接显示（配方 e3）
        st.ico.replaceChildren(s === 'done' ? doneMark(st.s === 'now', 18) : s === 'now' ? stepNow() : stepTodo());
        st.li.dataset.s = s;
        st.s = s;
      }
      // 这一步还没有进度可报（下载视频那一步后端不报百分比）就不写 0%，只留呼吸的点
      const pc = s === 'now' && hi > lo ? Math.round((p - lo) / (hi - lo) * 100) : 0;
      st.pct.textContent = pc > 0 ? pc + '%' : '';
    });
    paint(0);
    dockIn(el);
    try {
      run.task = (await send({ t: 'share', itemId, url })).task;
    } catch (e) {
      run.over = true;
      dockOut(el);
      return problem(e, () => shareAI({ itemId, url, name, img, dur }));
    }
    if (run.over) { send({ t: 'task-cancel', id: run.task }).catch(() => {}); return; }
    run.timer = setInterval(async () => {
      if (run.over || !el.isConnected) return clearInterval(run.timer);
      let s;
      try { s = await send({ t: 'task', id: run.task }); } catch { return; }
      if (run.over) return;
      if (s.st === 'run') return paint(s.pct || 0);
      clearInterval(run.timer);
      run.over = true;
      el._busy = false;
      if (s.st === 'cancel') return dockOut(el);
      if (s.st === 'err') {
        el.replaceChildren(
          topBar(tr('没准备好'), { mark: icon('alert', 18), tone: 'err', onX: () => dockOut(el) }).el,
          note('err', 'alert', s.err || tr('出了点问题')),
          h('div', { class: 'cx-foot' }, btn(tr('好的'), () => dockOut(el))));
        return;
      }
      paint(100);
      aiDone(el, s.result || {}, run.task, { name, img });
    }, 700);
  }
  const stepNow = () => {
    const t = document.createElement('template');
    t.innerHTML = '<svg viewBox="0 0 18 18" width="18" height="18" fill="none" aria-hidden="true"><circle cx="9" cy="9" r="7.25" stroke="currentColor" stroke-opacity=".35" stroke-width="1.5"/><circle class="pulse" cx="9" cy="9" r="3.4" fill="currentColor"/></svg>';
    return t.content.firstElementChild;
  };
  const stepTodo = () => {
    const t = document.createElement('template');
    t.innerHTML = '<svg viewBox="0 0 18 18" width="18" height="18" fill="none" aria-hidden="true"><circle cx="9" cy="9" r="7.25" stroke="currentColor" stroke-width="1.5"/></svg>';
    return t.content.firstElementChild;
  };

  function aiDone(el, b, task, info) {
    const md = b.markdown || '';
    const title = b.title || (md.match(/- 标题：(.*)/) || [])[1] || info.name;
    const lines = [...md.matchAll(/^\[(\d+:\d{2}(?::\d{2})?)\]\s+(.+)$/gm)].map(m => [m[1], m[2]]);
    const more = Math.max(0, lines.length - 3);
    const frames = (b.frames || []).length, thumbs = b.thumbs || [];
    const again = btn(tr('再复制一次'), async () => {
      try {
        await send({ t: 'recopy', id: task });
        again.replaceChildren(icon('tick', 14), tr('已复制'));
      } catch {
        try { await navigator.clipboard.writeText(md); again.replaceChildren(icon('tick', 14), tr('已复制文字')); } catch { again.textContent = tr('复制不了，去 Captae 里复制'); return; }
      }
      setTimeout(() => { if (again.isConnected) again.replaceChildren(icon('copy', 14), tr('再复制一次')); }, 1600);
    }, { ico: icon('copy', 14) });
    const x = topBar(b.copied ? tr('已复制，去 AI 对话里粘贴') : tr('准备好了'), { mark: doneMark(true, 18), tone: 'ok', onX: () => dockOut(el) });
    const okLine = b.copied ? h('span', null, ...withEl(tr('在 AI 的输入框里按 {key} 就行。'), 'key', h('kbd', { text: KEY }))) : tr('没能放进剪贴板，点「再复制一次」试试。');
    el.replaceChildren(
      x.el,
      note(b.copied ? 'ok' : 'warn', b.copied ? 'done' : 'alert', okLine),
      h('div', { class: 'cx-paste', attrs: { 'aria-label': tr('将要粘贴的内容') } },
        h('div', { class: 'ph' }, icon('copy', 13), tr('会粘贴这些内容')),
        h('div', { class: 'pt', text: title }),
        h('div', { class: 'pm', text: [b.site ? tAny(b.site) : host(location.href), b.dur].filter(Boolean).join(' · ') }),
        lines.length ? h('div', { class: 'tr' }, ...lines.slice(0, 3).flatMap(l => [h('s', { text: l[0] }), h('span', { text: l[1] })]), more ? [h('s', { text: '…' }), h('span', { text: tr('还有{n}段', { n: more }), style: 'color:var(--ink-3)' })] : null) : null,
        thumbs.length ? h('div', { class: 'frames' }, ...thumbs.slice(0, 6).map(t => h('img', { attrs: { src: t, alt: '' } }))) : null,
        h('div', { class: 'inc' },
          h('span', null, icon('doc', 13), b.transcript_source ? tr('完整文字稿') : tr('视频信息')),
          frames ? h('span', null, icon('image', 13), tr('关键画面{n}张', { n: frames })) : null)),
      h('div', { class: 'cx-foot' }, again, btn(tr('在 Captae 里查看'), () => { dockOut(el); open(b.asset_id != null ? { asset: b.asset_id } : { page: 'library' }); }, { primary: true })));
    focusIfKey(x.x);
  }

  /* ---------------- 视频旁边的按钮组 ---------------- */
  const overlays = new Map(); // video → {box, dl, label, v, got, key}
  function mainVideo() {
    let best = null, area = 0;
    for (const v of document.querySelectorAll('video')) {
      const r = v.getBoundingClientRect(), a = r.width * r.height;
      if (a > area) { best = v; area = a; }
    }
    return best;
  }
  function eligible(v) {
    const r = v.getBoundingClientRect();
    return r.width >= 240 && r.height >= 135 && getComputedStyle(v).visibility !== 'hidden';
  }
  function makeOverlay(v) {
    const label = h('span', { class: 't', text: tr('下载') });
    const dl = h('button', { class: 'cx-pill', attrs: { type: 'button', 'aria-haspopup': 'menu', 'aria-expanded': 'false' } }, logo(20), label, h('span', { class: 'chev' }, icon('chevron', 13)));
    const ai = h('button', { class: 'cx-pill plain', attrs: { type: 'button' } }, icon('ai', 15), tr('发给 AI'));
    const box = h('div', { class: 'cx cx-inj' }, dl, ai);
    box.addEventListener('pointerdown', stopEv);
    root.append(box);
    const o = { box, dl, label, v, got: null, key: '' };
    dl.addEventListener('click', e => {
      e.stopPropagation();
      if (menu && menu.v === v && menu.open) closeMenu(true);
      else openMenu(o, e.detail === 0);
    });
    ai.addEventListener('click', async e => {
      e.stopPropagation();
      const m = await mediaOf(v);
      shareAI({ ...(m.item ? { itemId: m.item.id } : { url: m.url }), name: m.name, img: m.img, dur: m.dur });
    });
    box.addEventListener('focusin', () => showBox(o, true));
    box.addEventListener('focusout', () => setTimeout(() => { if (!box.contains(root.activeElement)) hover(); }, 0));
    overlays.set(v, o);
    return o;
  }
  function showBox(o, on) {
    if (on && !o.box.hasAttribute('data-show')) {
      place();
      o.box.toggleAttribute('data-show', true);
      checkGot(o);
      if (!S.intro) showIntro(o);
    } else if (!on && o.box.hasAttribute('data-show')) {
      o.box.removeAttribute('data-show');
    }
  }

  /* 这个 <video> 实际要下的是什么：
   * - 整页交给下载引擎的网站（YouTube、B 站……）：页面本身
   * - 有 http 地址的直链：它自己（<source> 里有别的清晰度就一起列出来）
   * - blob: 播放器：这个标签页里嗅探到的 M3U8，都没有才退回页面地址 */
  async function mediaOf(v) {
    let item = null;
    try { item = await send({ t: 'tab-media' }); } catch { /* 后台不在 */ }
    const isMain = v === mainVideo();
    const img = v.poster || (isMain ? pageImg() : '') || (item && item.img) || '';
    const name = isMain ? pageTitle() : document.title;
    const dur = isFinite(v.duration) && v.duration ? clock(v.duration) : '';
    // preload="none" 的视频可能还没选好地址：用第一个 <source>
    const src = v.currentSrc || v.src || v.querySelector('source[src]')?.src || '';
    // 浏览器自己能播 M3U8 时，<video> 的地址就是列表本身：按 M3U8 处理（列出各档清晰度），不当成一个文件
    const hls = /\.m3u8(\?|#|$)/i.test(src);
    if (item && item.src === 'page') return { kind: 'page', item, url: location.href, name: item.named ? item.name : name, img: item.img || img, dur: item.dur || dur };
    if (hls) {
      const it = item && item.hls ? item : null;
      return { kind: 'hls', item: it, url: it ? it.url : src, alts: [], name, img, dur: (it && it.dur) || dur, size: 0, height: shortEdge(v) };
    }
    if (/^https?:/.test(src)) {
      const alts = [...v.querySelectorAll('source[src]')]
        .map(s => ({ uri: s.src, label: s.getAttribute('label') || s.getAttribute('title') || s.getAttribute('data-quality') || s.getAttribute('res') || s.getAttribute('size') || '' }))
        .filter(a => /^https?:/.test(a.uri) && a.label);
      const seen = new Set();
      const ext = (src.split('?')[0].match(/\.([a-z0-9]{2,4})$/i) || [])[1] || 'mp4';
      return { kind: 'file', url: src, ext, alts: alts.filter(a => !seen.has(a.uri) && seen.add(a.uri)), name, img, dur, size: item && item.url === src ? item.size : 0, height: shortEdge(v) };
    }
    if (item) return { kind: item.hls ? 'hls' : 'file', item, url: item.url, ext: item.ext || 'mp4', alts: [], name, img, dur: item.dur || dur, size: item.size, height: shortEdge(v) };
    return { kind: 'page', url: location.href, name, img, dur };
  }

  /** 清晰度数字按短边算：竖屏视频（1080×1920）叫 1080p，不叫 1920p（和桌面端读到的清晰度一样） */
  function shortEdge(v) {
    const w = v.videoWidth || 0, h = v.videoHeight || 0;
    return w && h ? Math.min(w, h) : h;
  }

  /** 下载过没有：按钮上写「下载过了」，菜单最上面给「打开」 */
  async function checkGot(o) {
    const key = location.href + '|' + (o.v.currentSrc || o.v.src || '');
    if (o.key === key) return;
    o.key = key;
    o.got = null;
    o.label.textContent = tr('下载');
    let m;
    try { m = await mediaOf(o.v); } catch { return; }
    const q = m.item ? { t: 'check', itemId: m.item.id } : m.alts && m.alts.length ? { t: 'check', urls: [m.url, ...m.alts.map(a => a.uri)] } : { t: 'check', url: m.url };
    const d = await send(q).catch(() => null);
    if (o.key !== key || !d || !d.found) return;
    o.got = d;
    o.label.textContent = tr('下载过了');
    // 字多了按钮变宽：重新贴到视频右上角，不往右边长出去
    place();
  }

  /* ---------------- 清晰度菜单（配方 f1 / l2） ---------------- */
  let menu = null; // {el, o, v, open, m, opts}
  const REC_MAX = 1080;

  /** 菜单里的选项：[{ch, label, fmt, rec, size, sub, cover}] */
  function menuOptions(m, p) {
    const out = [];
    if (m.kind === 'hls') {
      const vs = (m.item && m.item.variants) || [];
      if (!vs.length) out.push({ ch: { q: 'best' }, label: tr('原画质'), fmt: 'MP4', size: mb(m.size), rec: true });
      const rec = vs.findIndex(v => !v.h || v.h <= REC_MAX);
      vs.forEach((v, k) => out.push({ ch: { vi: k }, label: v.p, fmt: 'MP4', size: mb(v.bytes), rec: k === rec, sub: v.audio ? tr('声音单独一路，Captae 会合在一起') : '' }));
      out.push({ ch: { q: 'audio' }, label: tr('只要声音'), fmt: 'MP3', size: mb(p && p.audio) });
      return out;
    }
    if (m.kind === 'file') {
      const fmt = (m.ext || 'mp4').toUpperCase();
      if (m.alts.length > 1) m.alts.forEach((a, k) => out.push({ ch: { uri: a.uri, label: a.label }, label: a.label, fmt, size: mb(a.size), rec: k === 0 }));
      else out.push({ ch: { q: 'best' }, label: m.height ? `${m.height}p` : tr('原画质'), fmt, size: mb(m.size), rec: true });
      out.push({ ch: { q: 'audio' }, label: tr('只要声音'), fmt: 'MP3', size: '' });
      return out;
    }
    // 整页：按下载引擎读到的分辨率列，最多 4 档（最高的 + 1080/720/480 里有的）
    const qs = [];
    for (const q of (p && p.qualities) || []) if (q.height && !qs.some(x => x.height === q.height)) qs.push(q);
    const list = qs.length ? [qs[0]] : [];
    for (const hh of [1080, 720, 480]) {
      const q = qs.find(x => x.height <= hh);
      if (q && !list.includes(q) && list.length < 4) list.push(q);
    }
    const rec = list.findIndex(q => q.height <= REC_MAX);
    if (!list.length) out.push({ ch: { q: 'best' }, label: tr('最清晰'), fmt: 'MP4', size: '', rec: true });
    list.forEach((q, k) => out.push({ ch: k ? { q: `${q.height}p` } : { q: 'best' }, label: q.label || `${q.height}p`, fmt: 'MP4', size: mb(q.bytes), rec: k === (rec < 0 ? 0 : rec) }));
    out.push({ ch: { q: 'audio' }, label: tr('只要声音'), fmt: 'MP3', size: mb(p && p.audio) });
    return out;
  }

  async function openMenu(o, byKey) {
    endIntro();
    closeMenu(false, true);
    const el = h('div', { class: 'cx cx-menu cx-glass', attrs: { role: 'menu', 'aria-label': tr('选择清晰度'), tabindex: '-1' }, data: { state: 'open', instant: byKey } });
    el.addEventListener('pointerdown', stopEv);
    const mm = { el, o, v: o.v, open: true, m: null, opts: [], probe: null, cover: null };
    menu = mm;
    o.dl.setAttribute('aria-expanded', 'true');
    el.append(h('div', { class: 'h' }, h('span', { class: 't', text: tr('这个视频') })), h('div', { class: 'cx-wait' }, spinner(), tr('正在读取清晰度…')));
    root.append(el);
    wireMenu(mm);
    place();
    const m = await mediaOf(o.v);
    if (menu !== mm) return;
    mm.m = m;
    mm.probe = m.item && m.item.probe || null;
    renderMenu(mm, true);
    // 大小、封面尺寸慢慢补上：整页的问下载引擎（几秒），直链问一下文件大小，封面读一下尺寸
    if (m.kind === 'page' && !mm.probe) {
      send(m.item ? { t: 'probe', itemId: m.item.id } : { t: 'probe', url: m.url }).then(p => {
        if (p.title) m.name = p.title;
        if (p.thumbnail && !m.img) m.img = p.thumbnail;
        if (p.duration && !m.dur) m.dur = clock(p.duration);
        mm.probe = p;
        if (menu === mm) renderMenu(mm);
      }, e => {
        mm.probeErr = e;
        if (menu === mm) renderMenu(mm);
      });
    } else if (m.kind === 'file') {
      const need = [...(m.alts.length > 1 ? m.alts : []), ...(!m.size ? [{ uri: m.url, self: true }] : [])];
      Promise.all(need.map(a => send({ t: 'head', url: a.uri }).then(n => { if (a.self) m.size = n; else a.size = n; }).catch(() => {}))).then(() => { if (need.length && menu === mm) renderMenu(mm); });
    }
    coverOf(m).then(c => { mm.cover = c; if (c && menu === mm) renderMenu(mm); });
  }

  /** 封面图：地址 + 像素尺寸（读不到图就不列） */
  async function coverOf(m) {
    const src = m.img || (m.item && m.item.img) || '';
    if (!/^https?:/.test(src)) return null;
    return new Promise(res => {
      const im = new Image();
      im.referrerPolicy = 'no-referrer';
      im.onload = () => res({ url: src, w: im.naturalWidth, h: im.naturalHeight });
      im.onerror = () => res(null);
      im.src = src;
      setTimeout(() => res(null), 6000);
    });
  }

  function renderMenu(mm, first) {
    const { el, m } = mm;
    const had = el.contains(root.activeElement) ? root.activeElement.dataset.k : null;
    const kids = [];
    const head = [mm.probe && mm.probe.choose ? '' : tr('这个视频'), m.dur].filter(Boolean).join(' · ');
    if (mm.o.got) {
      const d = mm.o.got;
      kids.push(h('button', { class: 'cx-item got', attrs: { type: 'button', role: 'menuitem' }, data: { k: 'got' } },
        h('span', { class: 'l' }, icon('done', 16), h('span', null, tr('下载过了'), h('span', { class: 'sub', text: tr('在 {where} 里', { where: d.where || 'Captae' }) }))), h('span', { class: 'size', text: tr('打开') })));
      kids.push(h('div', { class: 'cx-sep', attrs: { role: 'separator' } }));
    }
    let opts = [];
    if (mm.probe && mm.probe.choose) {
      const n = (mm.probe.entries || []).length;
      kids.unshift(h('div', { class: 'h' }, h('span', { class: 't', text: n > 1 ? tr('这里有{n}个视频', { n }) : tr('这是一个播放列表') })));
      opts = [{ ch: { q: 'best' }, label: tr('在 Captae 里选要下的'), sub: tr('交给 Captae，列出全部视频，选好再下'), size: '', choose: true }];
    } else {
      kids.unshift(h('div', { class: 'h' }, h('span', { class: 't', text: head })));
      if (m.kind === 'page' && !mm.probe && !mm.probeErr) {
        kids.push(h('div', { class: 'cx-wait' }, spinner(), tr('正在读取清晰度…')));
      } else if (mm.probeErr) {
        const f = mm.probeErr.fail;
        const k = f ? failOf(f) : null;
        if (/HOST_MISSING|HOST_BROKEN|APP_NOT_RUNNING|HOST_GONE|TIMEOUT/.test(mm.probeErr.code || mm.probeErr.message)) {
          kids.push(h('div', { class: 'cx-note', style: 'margin:2px 4px 6px' }, icon('info', 16), h('div', { class: 'grow', text: tr('读不到清晰度：Captae 没有连上。点下面会先打开 Captae。') })));
        } else {
          const hint = f ? hintOf(f, mm.probeErr.login) : '';
          kids.push(h('div', { class: 'cx-note', data: { tone: k ? k.tone : 'warn' }, style: 'margin:2px 4px 6px' }, icon(k ? k.ico : 'alert', 16), h('div', { class: 'grow' }, (f && f.message) || mm.probeErr.message, hint ? h('small', { text: hint }) : null)));
        }
        opts = f && ['drm', 'unavailable', 'no_media'].includes(f.code) ? [] : [{ ch: { q: 'best' }, label: tr('仍然交给 Captae'), sub: tr('按最清晰的下载'), size: '' }];
      } else {
        opts = menuOptions(m, mm.probe);
      }
    }
    if (mm.cover && !(mm.probe && mm.probe.choose)) opts.push({ cover: mm.cover.url, label: tr('封面图'), size: mm.cover.w ? `${mm.cover.w}×${mm.cover.h}` : '', sep: true });
    mm.opts = opts;
    opts.forEach((o, k) => {
      if (o.sep) kids.push(h('div', { class: 'cx-sep', attrs: { role: 'separator' } }));
      const l = h('span', { class: 'l' }, h('span', { text: o.label }), o.fmt ? h('small', { text: o.fmt }) : null, o.rec ? h('span', { class: 'rec', text: tr('推荐') }) : null);
      const body = o.sub ? h('span', null, l, h('span', { class: 'sub', text: o.sub })) : l;
      kids.push(h('button', { class: 'cx-item', attrs: { type: 'button', role: 'menuitem' }, data: { k } }, body, h('span', { class: 'size', text: o.size || '' })));
    });
    el.replaceChildren(...kids);
    place();
    const items = [...el.querySelectorAll('[data-k]')];
    if (had != null) items.find(b => b.dataset.k === had)?.focus({ preventScroll: true });
    else if (first) {
      // 记住的选择优先获得焦点；键盘打开时让焦点落在菜单里（鼠标打开不抢焦点高亮）
      // 用户上次选过的那档优先，没选过就是「推荐」那档
      let pref = S.qpref ? opts.findIndex(x => x.ch && x.ch.q === S.qpref) : -1;
      if (pref < 0) pref = Math.max(0, opts.findIndex(x => x.rec));
      const target = items.find(b => b.dataset.k === String(pref)) || items[0];
      if (el.dataset.instant != null) target?.focus({ preventScroll: true });
      else el.focus({ preventScroll: true });
    }
  }

  function wireMenu(mm) {
    const { el } = mm;
    el.addEventListener('click', e => {
      e.stopPropagation();
      const b = e.target.closest('[data-k]');
      if (!b || !mm.m) return;
      const m = mm.m;
      if (b.dataset.k === 'got') { closeMenu(byKey); open({ asset: mm.o.got.assetId }); return; }
      const o = mm.opts[+b.dataset.k];
      closeMenu(byKey);
      if (!o) return;
      if (o.cover) {
        startDownload({ url: o.cover }, {}, { name: tr('{name} · 封面图', { name: m.name }), img: o.cover, kind: 'image', label: tr('封面图'), size: o.size });
        return;
      }
      if (o.ch.q && o.ch.q !== 'audio') send({ t: 'settings', patch: { qpref: o.ch.q } }).catch(() => {});
      const info = { name: m.name, img: m.img, label: o.choose ? '' : o.ch.q === 'audio' ? tr('只要声音 · MP3') : [o.label, o.fmt].filter(Boolean).join(' '), size: o.size };
      const target = m.item ? { itemId: m.item.id } : { url: o.ch.uri || m.url };
      // 菜单上已经说了「下载过了」，用户还是选了一档：就是要再下一次
      startDownload(target, o.ch, info, !!mm.o.got);
    });
    /* 悬停的那一项拿到焦点：鼠标和键盘的高亮是同一个（课程：高亮即时，不渐变） */
    el.addEventListener('pointermove', e => {
      const b = e.target.closest('[data-k]');
      if (b && root.activeElement !== b) b.focus({ preventScroll: true });
    });
    el.addEventListener('pointerleave', () => { if (el.contains(root.activeElement)) el.focus({ preventScroll: true }); });
    el.addEventListener('keydown', e => {
      const items = [...el.querySelectorAll('[data-k]')];
      const i = items.indexOf(root.activeElement);
      const go = j => { e.preventDefault(); items[(j + items.length) % items.length]?.focus({ preventScroll: true }); };
      if (e.key === 'ArrowDown') go(i + 1);
      else if (e.key === 'ArrowUp') go(i < 0 ? -1 : i - 1);
      else if (e.key === 'Home') go(0);
      else if (e.key === 'End') go(-1);
      else if (e.key === 'Tab') closeMenu();
      else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); closeMenu(true); }
    });
  }

  /** 关菜单：退场动画放完再移除（配方 f1）；refocus 时焦点回到「下载」按钮 */
  function closeMenu(refocus, instant) {
    const mm = menu;
    if (!mm) return;
    menu = null;
    mm.open = false;
    mm.o.dl.setAttribute('aria-expanded', 'false');
    const kb = refocus && mm.el.contains(root.activeElement) && mm.el.querySelector(':focus-visible');
    if (refocus) mm.o.dl.focus({ preventScroll: true });
    if (instant || kb || mm.el.dataset.instant != null) { mm.el.remove(); hover(); return; }
    mm.el.dataset.state = 'closed';
    mm.el.addEventListener('animationend', () => mm.el.remove(), { once: true });
    setTimeout(() => mm.el.remove(), 300);
    hover();
  }

  /* ---------------- 位置 ---------------- */
  let tip = null;
  function place() {
    for (const [v, o] of overlays) {
      if (!v.isConnected || !S.inj) { o.box.remove(); overlays.delete(v); continue; }
      const r = v.getBoundingClientRect();
      // 藏着（hidden）的时候量不出宽度：先取消隐藏再量，下面按位置重新决定藏不藏
      o.box.hidden = false;
      const w = o.box.offsetWidth || 205;
      const y = Math.max(r.top + 12, 8);
      // 贴着视频露在窗口里的那部分的右上角：视频比窗口宽、右边超出去时，按钮组不跟着跑到窗口外
      const right = Math.min(r.right, document.documentElement.clientWidth || innerWidth);
      o.box.hidden = r.bottom < y + 44 || r.top > innerHeight - 44 || r.width < 240 || right - r.left < w + 24;
      o.box.style.translate = `${Math.round(Math.max(r.left + 12, right - 12 - w))}px ${Math.round(y)}px`;
    }
    if (menu) {
      const r = menu.o.box.getBoundingClientRect();
      const el = menu.el;
      const mh = el.offsetHeight, mw = el.offsetWidth;
      let top = r.bottom + 8;
      const above = top + mh > innerHeight - 8 && r.top - 8 - mh > 8;
      if (above) top = r.top - 8 - mh;
      top = Math.max(8, Math.min(top, innerHeight - mh - 8));
      el.style.transformOrigin = above ? 'bottom right' : 'top right';
      el.style.translate = `${Math.round(Math.max(8, r.right - mw))}px ${Math.round(top)}px`;
    }
    placeTool();
    if (tip && tip.o) {
      const r = tip.o.box.getBoundingClientRect();
      tip.el.style.translate = `${Math.round(Math.max(8, r.right - tip.el.offsetWidth))}px ${Math.round(r.bottom + 10)}px`;
    }
  }
  function scan() {
    if (dead) return;
    if (!S.inj) {
      for (const o of overlays.values()) o.box.remove();
      overlays.clear();
      closeMenu(false, true);
      endIntro(true);
      return;
    }
    for (const v of document.querySelectorAll('video')) if (!overlays.has(v) && eligible(v)) makeOverlay(v);
    place();
  }
  let raf = 0;
  const schedule = () => { if (!raf) raf = requestAnimationFrame(() => { raf = 0; place(); }); };
  addEventListener('scroll', schedule, true);
  // 滚动时鼠标没动、底下换了一条帖子：「做成图片」跟着换（只在 X 和文章页上有用）
  addEventListener('scroll', () => { if (tool && tool.el && !hoverT) hoverT = requestAnimationFrame(() => { hoverT = 0; hover(); }); }, { capture: true, passive: true });
  addEventListener('resize', schedule);
  new MutationObserver(() => { clearTimeout(scan._t); scan._t = setTimeout(scan, 400); }).observe(document.documentElement, { childList: true, subtree: true });

  /* 鼠标在视频上停一下（100ms，免得划过就闪）才显示按钮组；菜单开着、焦点在按钮上时一直显示（配方 l1） */
  let pt = { x: -1, y: -1 }, hoverT = 0;
  function hover() {
    for (const [v, o] of overlays) {
      const r = v.getBoundingClientRect();
      const inside = pt.x >= r.left && pt.x <= r.right && pt.y >= r.top && pt.y <= r.bottom;
      const keep = (menu && menu.v === v) || (tip && tip.o === o) || o.box.contains(root.activeElement) || o.box.matches(':hover');
      if (keep || (inside && o.box.hasAttribute('data-show'))) showBox(o, true);
      else if (inside) {
        clearTimeout(o.intent);
        o.intent = setTimeout(() => { if (o.box.isConnected) showBox(o, true); }, 100);
      } else {
        clearTimeout(o.intent);
        showBox(o, false);
      }
    }
    hoverTool();
  }
  addEventListener('mousemove', e => {
    pt = { x: e.clientX, y: e.clientY };
    if (!hoverT) hoverT = requestAnimationFrame(() => { hoverT = 0; hover(); });
  }, { passive: true });
  document.documentElement.addEventListener('mouseleave', () => { pt = { x: -1, y: -1 }; hover(); });

  /* 点菜单和「下载」按钮以外的地方：关菜单（「下载」按钮自己负责开关）。
   * 封闭的 Shadow DOM 里的事件，外面只看得到宿主元素：网页上的点击在 window 上判断，我们自己界面里的点击在 Shadow 根上判断 */
  addEventListener('pointerdown', e => { if (menu && e.target !== hostEl) closeMenu(); }, true);
  root.addEventListener('pointerdown', e => {
    if (!menu) return;
    const path = e.composedPath();
    if (!path.includes(menu.el) && !path.includes(menu.o.dl)) closeMenu();
  }, true);
  /* Esc：先关菜单，再关最新的一张卡片（在输入框里打字时不抢） */
  addEventListener('keydown', e => {
    if (e.key !== 'Escape') return;
    if (menu) { closeMenu(true); return; }
    const t = e.composedPath()[0];
    if (t && t !== hostEl && (t.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(t.tagName || ''))) return;
    const last = stack[stack.length - 1];
    if (!last) return;
    last._close ? last._close() : dockOut(last);
  });

  /* 单页应用换了视频（YouTube 不刷新页面）：清掉按钮上的「下载过了」，重新报一次页面信息 */
  let href = location.href;
  setInterval(() => {
    if (location.href === href) return;
    href = location.href;
    closeMenu(false, true);
    for (const o of overlays.values()) { o.key = ''; o.got = null; o.label.textContent = tr('下载'); }
    setTimeout(reportPage, 1500);
    scan();
    place();
  }, 1000);

  /* ---------------- X 帖子做成图片、文章存成 Markdown ----------------
   * 鼠标停在 X 的帖子上：帖子右上角（X 自己的按钮左边）出一个「做成图片」；停在公众号文章、X 长文章的正文上：正文右上角出「存成 Markdown」。
   * 同一时间只有一个，跟着鼠标换位置；出现、消失和视频上的按钮组一样（.cx-inj，配方 l1：停 100ms 才出现，150ms 进、100ms 出）。
   * 右键菜单里也有（background.js 的 captae-ximg、captae-md），其他网页的文章也能从右键菜单存。
   * 帖子数据、卡片和出图在 lib/xcard.js，读文章、转 Markdown 在 lib/article.js，打包在 lib/zip.js（用到时才加载）。 */
  const onX = /(^|\.)(x|twitter)\.com$/.test(location.hostname);
  const isArticlePage = () => (/(^|\.)mp\.weixin\.qq\.com$/.test(location.hostname) && /^\/s/.test(location.pathname)) || (onX && /\/article\/\d+/.test(location.pathname));
  const articleRoot = () => document.querySelector(onX ? '.x-article-body' : '#js_content');
  const libs = {};
  const lib = async n => libs[n] || (libs[n] = await import(chrome.runtime.getURL(`lib/${n}.js`)));
  /** 交给浏览器下载（存到浏览器的下载文件夹，和网页上点下载链接一样） */
  function saveFile(blob, name) {
    const u = URL.createObjectURL(blob);
    h('a', { href: u, download: name }).click();
    setTimeout(() => URL.revokeObjectURL(u), 60000);
  }
  /** 后台取回的 data URL → 字节 */
  function bytesOf(d) {
    const s = atob(d.slice(d.indexOf(',') + 1));
    const b = new Uint8Array(s.length);
    for (let i = 0; i < s.length; i++) b[i] = s.charCodeAt(i);
    return b;
  }
  /** 帖子的编号：帖子里第一个「/用户名/status/数字」的链接（发帖时间那个），帖子页面上的主帖没有时用网址里的 */
  function postId(a) {
    for (const l of a.querySelectorAll('a[href*="/status/"]')) {
      const m = (l.getAttribute('href') || '').match(/^\/\w{1,15}\/status\/(\d+)$/);
      if (m) return m[1];
    }
    return (location.pathname.match(/\/status\/(\d+)/) || [])[1] || '';
  }
  const postFromUrl = u => (String(u || '').match(/\/status(?:es)?\/(\d+)/) || [])[1] || '';

  // var：place() 可能在脚本跑完之前就被调用（设置变了、网页变了），那时 tool 还是 undefined，placeTool 里判断
  var tool = { el: null, btn: null, target: null, kind: '', intent: 0 };
  function toolBox() {
    if (tool.el) return tool.el;
    tool.btn = h('button', { class: 'cx-pill plain', attrs: { type: 'button' } });
    tool.el = h('div', { class: 'cx cx-inj' }, tool.btn);
    tool.el.addEventListener('pointerdown', stopEv);
    tool.btn.addEventListener('click', e => {
      e.stopPropagation();
      if (tool.kind === 'ximg') xImage(tool.target);
      else saveArticle();
    });
    tool.el.addEventListener('focusout', () => setTimeout(() => { if (!tool.el.contains(root.activeElement)) hover(); }, 0));
    root.append(tool.el);
    return tool.el;
  }
  function hoverTool() {
    if (!(onX || isArticlePage())) return;
    const box = toolBox();
    if (!S.inj) { tool.target = null; box.removeAttribute('data-show'); return; }
    if (box.matches(':hover') || box.contains(root.activeElement)) return;
    let target = null, kind = '';
    if (pt.x >= 0) {
      const ar = isArticlePage() && articleRoot();
      if (ar) {
        const r = ar.getBoundingClientRect();
        // 正文上面一点（标题、作者那几行）也算
        if (pt.x >= r.left && pt.x <= r.right && pt.y >= r.top - 120 && pt.y <= r.bottom) { target = ar; kind = 'md'; }
      }
      if (!target && onX) {
        const a = document.elementFromPoint(pt.x, pt.y)?.closest?.('article');
        if (a && postId(a)) { target = a; kind = 'ximg'; }
      }
    }
    clearTimeout(tool.intent);
    if (!target) { tool.target = null; box.removeAttribute('data-show'); return; }
    if (target === tool.target && box.hasAttribute('data-show')) return;
    const show = () => {
      if (tool.kind !== kind) {
        tool.kind = kind;
        tool.btn.replaceChildren(icon(kind === 'ximg' ? 'image' : 'doc', 15), kind === 'ximg' ? tr('做成图片') : tr('存成 Markdown'));
      }
      tool.target = target;
      placeTool();
      box.toggleAttribute('data-show', true);
    };
    // 已经显示着、只是换到另一条帖子：直接挪过去；刚出现时停 100ms，划过不闪
    if (box.hasAttribute('data-show')) show();
    else tool.intent = setTimeout(show, 100);
  }
  function placeTool() {
    const box = tool && tool.el;
    if (!box || !tool.target) return;
    if (!tool.target.isConnected) { tool.target = null; box.removeAttribute('data-show'); return; }
    const r = tool.target.getBoundingClientRect();
    const w = box.offsetWidth || 120;
    const vw = document.documentElement.clientWidth || innerWidth;
    let x, y;
    if (tool.kind === 'ximg') {
      // X 的帖子右上角是「解释这个帖子」和「更多」两个 32px 的按钮（约 72px 宽），放在它们左边、同一行
      x = Math.min(r.right, vw) - 76 - w;
      y = r.top - 8;
    } else {
      // 正文右上角；往下滚时停在窗口顶上（X 顶上有一条 53px 的标题栏），正文快结束时跟着走
      x = Math.min(r.right, vw - 12) - w;
      y = Math.min(Math.max(r.top - 44, onX ? 64 : 12), r.bottom - 44);
    }
    box.style.translate = `${Math.round(Math.max(8, x))}px ${Math.round(y)}px`;
  }

  /** 页面是深色的（X 的「深色」「黑色」显示模式）：图片也先用深色 */
  const pageDark = () => {
    const c = (getComputedStyle(document.body).backgroundColor.match(/\d+/g) || []).map(Number);
    return c.length >= 3 && c[0] + c[1] + c[2] < 200 && c[3] !== 0;
  };
  /** 长帖（超过 280 字）接口只给前一段：从网页上的帖子补全（只要纯文字） */
  function fullText(art, p) {
    const el = art && art.querySelector('[data-testid="tweetText"], [dir="auto"].whitespace-pre-wrap');
    const s = el ? el.innerText.trim() : '';
    const have = p.segs.reduce((n, x) => n + x.s.length, 0);
    return s.length > have ? s : '';
  }

  /** 一条帖子做成图片：右下角一张卡片，预览、换浅色深色、复制、下载 PNG（3 倍，约 2000 像素宽） */
  const shots = new Map();
  async function xImage(target) {
    const id = typeof target === 'string' ? target : target ? postId(target) : '';
    const art = typeof target === 'string' ? [...document.querySelectorAll('article')].find(a => postId(a) === id) : target;
    if (!id) return noteCard(tr('没找到帖子'), tr('在帖子上点右键，或者打开这条帖子再试。'), { tone: 'warn', ico: 'alert', ms: 5000 });
    // 同一条已经有卡片了：不再开一张
    const had = shots.get(id);
    if (had && had.isConnected && had.dataset.out == null) { focusIfKey(had.querySelector('.cx-b')); return; }
    const el = card('cx-shot');
    shots.set(id, el);
    const tb = topBar(tr('帖子图片'), { mark: icon('image', 18), onX: () => dockOut(el) });
    el.append(tb.el, h('div', { class: 'cx-wait' }, spinner(), tr('正在读取帖子…')));
    el._busy = true;
    dockIn(el);
    let data, xc;
    try {
      [data, xc] = await Promise.all([send({ t: 'x-post', id }), lib('xcard')]);
    } catch (e) {
      dockOut(el);
      return problem(e, () => xImage(target));
    }
    const { post, imgs } = data;
    if (post.truncated) { const full = fullText(art, post); if (full) post.segs = [{ s: full, link: false }]; }
    let theme = pageDark() ? 'dark' : 'light', png = null, seq = 0, url = '';
    const prev = h('div', { class: 'cx-shot-prev' });
    const render = async () => {
      const my = ++seq;
      const holder = h('div', { class: 'cx-measure' });
      // cardHTML 里网页来的文字都转义过，图片是后台取回的 data URL；放在我们自己的封闭 Shadow DOM 里，网页碰不到
      holder.innerHTML = xc.cardHTML(post, imgs, { theme, lang: lang() });
      root.append(holder);
      try {
        await Promise.all([...holder.querySelectorAll('img')].map(i => i.decode().catch(() => {})));
        const out = await xc.rasterize(holder.firstElementChild, 3);
        if (my !== seq) return;
        png = out;
        if (url) URL.revokeObjectURL(url);
        url = URL.createObjectURL(out.blob);
        prev.replaceChildren(h('img', { src: url, alt: tr('帖子图片预览') }));
      } finally {
        holder.remove();
      }
    };
    try {
      await render();
    } catch (e) {
      el._busy = false;
      el.replaceChildren(topBar(tr('没做出图片'), { mark: icon('alert', 18), tone: 'err', onX: () => dockOut(el) }).el, note('err', 'alert', tr('这条帖子没能画成图片。')), h('div', { class: 'cx-foot' }, btn(tr('好的'), () => dockOut(el))));
      return;
    }
    // 浅色 / 深色：两个按钮的单选组，换了马上重画预览（按钮本身不加动画，同桌面端分段控件用键盘时）
    const seg = h('div', { class: 'cx-seg lead', attrs: { role: 'radiogroup', 'aria-label': tr('配色') } });
    const pick = k => {
      if (theme === k) return;
      theme = k;
      for (const b of seg.children) b.setAttribute('aria-checked', String(b.dataset.k === k));
      render().catch(() => {});
    };
    for (const [k, label] of [['light', tr('浅色')], ['dark', tr('深色')]]) {
      seg.append(h('button', { attrs: { type: 'button', role: 'radio', 'aria-checked': String(theme === k) }, data: { k }, on: { click: () => pick(k) } }, label));
    }
    const cp = btn(tr('复制图片'), async () => {
      try {
        await navigator.clipboard.write([new ClipboardItem({ 'image/png': png.blob })]);
        cp.replaceChildren(icon('tick', 14), tr('已复制'));
      } catch {
        cp.textContent = tr('复制不了');
      }
    }, { ico: icon('copy', 14) });
    const dl = btn(tr('下载'), () => png && saveFile(png.blob, `x-${post.handle}-${post.id}.png`), { primary: true, ico: icon('download', 14) });
    el._busy = false;
    el.replaceChildren(
      topBar(tr('帖子图片'), { mark: icon('image', 18), onX: () => { if (url) URL.revokeObjectURL(url); dockOut(el); } }).el,
      prev,
      h('div', { class: 'cx-foot' }, seg, cp, dl));
    focusIfKey(dl);
  }

  /** 这一页的文章存成 Markdown：有图片时连图片打成 .zip（解压后 Markdown 和 images/ 在一起），没有图片就是一个 .md */
  let mdBusy = false;
  async function saveArticle() {
    if (mdBusy) return;
    const A = await lib('article').catch(() => null);
    const a = A && A.readArticle();
    if (!a) return noteCard(tr('这个网页上没找到文章'), tr('打开文章本身的网页再试：公众号文章、X 的长文章，或者正文在一块里的博客。'), { tone: 'warn', ico: 'alert', ms: 5000 });
    mdBusy = true;
    const el = card();
    let stop = false;
    const status = h('span');
    el.append(
      topBar(tr('正在存成 Markdown'), { mark: icon('doc', 18), onX: () => { stop = true; dockOut(el); }, xLabel: tr('取消') }).el,
      mediaRow(a.title, [a.author, host(location.href)].filter(Boolean).join(' · '), a.cover, 'doc').el,
      h('div', { class: 'cx-wait' }, spinner(), status));
    status.textContent = tr('正在整理文字…');
    el._busy = true;
    dockIn(el);
    try {
      const { md, images } = A.toMarkdown(a.root, { skip: a.skipTitle });
      // X 长文章标题上面那张大图是文章的一部分，放在最前面；公众号的封面只在分享卡片上，正文里没有，不加
      let body = md;
      if (a.kind === 'x' && a.cover) { images.push({ url: a.cover, alt: '' }); body = `![](\u0000IMG${images.length - 1}\u0000)\n\n` + md; }
      const local = [], files = [];
      const EXT = { 'image/jpeg': 'jpg', 'image/png': 'png', 'image/webp': 'webp', 'image/gif': 'gif', 'image/avif': 'avif', 'image/svg+xml': 'svg' };
      let next = 0, done = 0;
      const one = async () => {
        while (next < images.length && !stop) {
          const k = next++;
          try {
            const d = await send({ t: 'fetch-data', url: images[k].url });
            const ext = EXT[d.slice(5, d.indexOf(';'))];
            if (ext) { local[k] = `images/${String(k + 1).padStart(2, '0')}.${ext}`; files.push({ name: local[k], data: bytesOf(d) }); }
          } catch { /* 这一张取不到：Markdown 里留网上的地址 */ }
          done++;
          status.textContent = tr('正在下载图片 {done}/{n}', { done, n: images.length });
        }
      };
      if (images.length) status.textContent = tr('正在下载图片 {done}/{n}', { done: 0, n: images.length });
      await Promise.all([one(), one(), one(), one()]);
      if (stop) return;
      const head = A.frontMatter(a);
      const remote = head + A.finishImages(body, images) + '\n';
      const name = A.safeName(a.title);
      const mdBlob = s => new Blob([s], { type: 'text/markdown;charset=utf-8' });
      if (files.length) {
        const { zip } = await lib('zip');
        files.sort((x, y) => x.name.localeCompare(y.name));
        saveFile(zip([{ name: `${name}.md`, data: head + A.finishImages(body, images, local) + '\n' }, ...files]), `${name}.zip`);
      } else {
        saveFile(mdBlob(remote), `${name}.md`);
      }
      el._busy = false;
      const cp = btn(tr('复制 Markdown'), () => copyText(remote, cp), { ico: icon('copy', 14) });
      el.replaceChildren(
        topBar(tr('已存成 Markdown'), { mark: doneMark(true, 18), tone: 'ok', onX: () => dockOut(el) }).el,
        note('ok', 'done', files.length
          ? tr('「{name}.zip」在浏览器的下载文件夹里，解压后 Markdown 和图片在一起。', { name })
          : tr('「{name}.md」在浏览器的下载文件夹里。', { name })),
        h('div', { class: 'cx-foot' },
          files.length ? btn(tr('只下载 .md'), () => saveFile(mdBlob(remote), `${name}.md`), { label: tr('只下载 .md（图片用网上的地址）') }) : null,
          cp));
      autoClose(el, 8000);
    } catch (e) {
      dockOut(el);
      problem(e, saveArticle);
    } finally {
      mdBusy = false;
    }
  }

  /* ---------------- 第一次看到按钮时的说明 ---------------- */
  function showIntro(o) {
    if (tip || S.intro) return;
    const el = h('div', { class: 'cx cx-tip cx-glass', attrs: { role: 'note' } },
      h('b', { text: tr('Captae 加的按钮') }),
      tr('点「下载」选清晰度，或者把视频发给 AI 帮你看。'),
      h('div', { class: 'cx-foot' }, btn(tr('知道了'), () => { endIntro(); focusIfKey(o.dl); }, { primary: true })));
    el.addEventListener('pointerdown', stopEv);
    tip = { el, o };
    root.append(el);
    place();
  }
  function endIntro(silent) {
    if (!tip) return;
    tip.el.remove();
    tip = null;
    if (silent) return;
    S.intro = true;
    send({ t: 'settings', patch: { intro: true } }).catch(() => {});
  }

  /* ---------------- 复制了视频链接 ----------------
   * 复制的那一刻什么都不弹（原来右下角会弹卡片问要不要下载，用户觉得打扰：复制链接多半是要发给别人）。
   * 只告诉 Captae 一声（后台判断是视频链接，经宿主记下什么链接、什么时候）：1 分钟内打开 Captae 时，它在下载页问一句。
   * Captae 开着时它自己看得到剪贴板什么时候变了，这一声是给 Captae 没开的时候用的。
   * 两个来源：copy 事件（选中文字复制、execCommand），和 page-hook.js 转来的 navigator.clipboard.writeText（网站自己的「复制链接」按钮）。 */
  let lastCopy = { text: '', at: 0 };
  function copied(text) {
    text = (text || '').trim();
    if (!text || text.length > 2048) return;
    if (text === lastCopy.text && Date.now() - lastCopy.at < 2000) return;
    lastCopy = { text, at: Date.now() };
    send({ t: 'copied', text }).catch(() => {});
  }
  document.addEventListener('copy', () => {
    const a = document.activeElement;
    const text = a && (a.tagName === 'TEXTAREA' || a.tagName === 'INPUT') && typeof a.selectionStart === 'number'
      ? a.value.slice(a.selectionStart, a.selectionEnd)
      : String(document.getSelection() || '');
    copied(text);
  });
  addEventListener('message', e => {
    if (e.source !== window || !e.data || e.data.__captae !== 'copy') return;
    copied(String(e.data.text || ''));
  });

  reportPage();
  setTimeout(reportPage, 2500);
  scan();
})();
