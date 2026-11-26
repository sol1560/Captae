/* 判断一个网络请求是不是值得下载的媒体，以及解析 M3U8 清晰度。
 * 只看响应头和链接，不改请求，也不读正文（M3U8 除外，要读列表才知道有哪些清晰度）。 */

import { tr, decimal } from './i18n.js';

const VIDEO_EXT = ['mp4', 'm4v', 'webm', 'mov', 'mkv', 'flv', 'avi', 'ogv'];
const AUDIO_EXT = ['mp3', 'm4a', 'aac', 'flac', 'wav', 'ogg', 'oga', 'opus'];
const IMAGE_EXT = ['jpg', 'jpeg', 'png', 'webp', 'avif', 'gif'];
const HLS_EXT = ['m3u8'];
/* 分片和分段清单：单独下载没有意义，由 M3U8 或整页下载负责 */
const SKIP_EXT = ['ts', 'm4s', 'mpd', 'key', 'vtt', 'srt'];
/* 这些站点的视频是切成很多段的直链，交给 yt-dlp 按整页下载更稳，嗅探到的分段不列出来 */
const SEGMENT_HOSTS = [/googlevideo\.com$/, /bilivideo\.(com|cn)$/, /akamaized\.net$/, /douyinvod\.com$/, /tiktokcdn/, /twimg\.com$/, /cdninstagram\.com$/, /fbcdn\.net$/, /vimeocdn\.com$/];

/* 这些网站整页交给 yt-dlp（和桌面端 detect.rs 的规则一致） */
export const PAGE_SITES = [
  { re: /^https?:\/\/(www\.|m\.)?youtube\.com\/(watch|shorts\/)|^https?:\/\/youtu\.be\//, site: 'YouTube' },
  // B 站从海外访问会跳到带语言前缀的地址：bilibili.com/zh-Hans/video/…（2026-11-24 实测）
  { re: /^https?:\/\/(www\.|m\.)?bilibili\.com\/([a-z]{2}(-[A-Za-z]{2,4})?\/)?(video|bangumi)\/|^https?:\/\/b23\.tv\//, site: '哔哩哔哩' },
  { re: /^https?:\/\/(www\.)?douyin\.com\/(video|note)\/|^https?:\/\/v\.douyin\.com\//, site: '抖音' },
  { re: /^https?:\/\/(www\.)?tiktok\.com\/@[^/]+\/video\//, site: 'TikTok' },
  { re: /^https?:\/\/(www\.)?(x|twitter)\.com\/[^/]+\/status\//, site: 'X' },
  { re: /^https?:\/\/(www\.)?instagram\.com\/(p|reel|reels)\//, site: 'Instagram' },
  { re: /^https?:\/\/(www\.)?vimeo\.com\/\d+/, site: 'Vimeo' },
  /* 大陆以外 xiaohongshu.com 会跳到 rednote.com；桌面端 detect.rs 的 canonical 会把链接换回 xiaohongshu.com 再交给 yt-dlp */
  { re: /^https?:\/\/(www\.)?(xiaohongshu|rednote)\.com\/(explore|discovery\/item)\//, site: '小红书' },
];

export function pageSite(url) {
  const hit = PAGE_SITES.find(s => s.re.test(url || ''));
  return hit ? hit.site : null;
}

/* 交给 Captae 之前把网址理顺：带语言前缀的 B 站地址下载引擎不认（只读得到标题、没有清晰度），去掉前缀 */
export function canonicalPage(url) {
  try {
    const u = new URL(url);
    if (/(^|\.)bilibili\.com$/.test(u.hostname)) {
      const p = u.pathname.replace(/^\/[a-z]{2}(?:-[A-Za-z]{2,4})?(?=\/(?:video|bangumi)\/)/, '');
      if (p !== u.pathname) {
        u.pathname = p;
        return u.href;
      }
    }
  } catch { /* 不是网址，原样返回 */ }
  return url;
}

export function extOf(url) {
  try {
    const p = new URL(url).pathname.toLowerCase();
    const m = p.match(/\.([a-z0-9]{2,5})$/);
    return m ? m[1] : '';
  } catch { return ''; }
}

export function fileName(url) {
  try {
    const p = decodeURIComponent(new URL(url).pathname);
    return p.split('/').filter(Boolean).pop() || new URL(url).hostname;
  } catch { return url; }
}

/* 请求 → {kind, ext} 或 null。kind: video / audio / image / hls */
export function classify({ url, type, contentType, length, status }) {
  if (status && status >= 400) return null;
  if (!/^https?:/.test(url)) return null;
  let host = '';
  try { host = new URL(url).hostname; } catch { return null; }
  const ext = extOf(url);
  const ct = (contentType || '').split(';')[0].trim().toLowerCase();
  if (SKIP_EXT.includes(ext) || ct === 'video/mp2t' || ct === 'video/iso.segment' || ct === 'application/dash+xml') return null;
  if (HLS_EXT.includes(ext) || ct === 'application/vnd.apple.mpegurl' || ct === 'application/x-mpegurl' || ct === 'audio/mpegurl') {
    return { kind: 'hls', ext: 'm3u8' };
  }
  if (SEGMENT_HOSTS.some(r => r.test(host))) return null;
  if (VIDEO_EXT.includes(ext) || ct.startsWith('video/')) {
    // 很小的视频一般是网站的动图、占位，没意义
    if (length && length < 300 * 1024) return null;
    return { kind: 'video', ext: VIDEO_EXT.includes(ext) ? ext : (ct.split('/')[1] || 'mp4').replace('quicktime', 'mov') };
  }
  if (AUDIO_EXT.includes(ext) || ct.startsWith('audio/')) {
    if (length && length < 200 * 1024) return null;
    return { kind: 'audio', ext: AUDIO_EXT.includes(ext) ? ext : (ct.split('/')[1] || 'mp3').replace('mpeg', 'mp3') };
  }
  if (type === 'image' || type === 'main_frame') {
    if (!(IMAGE_EXT.includes(ext) || ct.startsWith('image/'))) return null;
    // 只收大图：图标、头像、缩略图太多了，列出来反而找不到要的
    if (!length || length < 120 * 1024) return null;
    if (ct === 'image/svg+xml') return null;
    return { kind: 'image', ext: IMAGE_EXT.includes(ext) ? ext.replace('jpeg', 'jpg') : (ct.split('/')[1] || 'jpg').replace('jpeg', 'jpg') };
  }
  return null;
}

/* M3U8 文本 → 主列表的清晰度，或者分段列表的总时长 */
export function parseM3U8(text, base) {
  const lines = text.split(/\r?\n/).map(s => s.trim()).filter(Boolean);
  if (!lines[0] || !lines[0].startsWith('#EXTM3U')) return null;
  const variants = [];
  let duration = 0, live = true, encrypted = false;
  /* 声音单独放在另一个列表里的组（EXT-X-MEDIA TYPE=AUDIO 带 URI）：直接下某一路画面会没有声音 */
  const audioGroups = new Set();
  for (const l of lines) if (l.startsWith('#EXT-X-MEDIA:')) { const a = attrs(l.slice(13)); if (a.TYPE === 'AUDIO' && a.URI && a['GROUP-ID']) audioGroups.add(a['GROUP-ID']); }
  for (let i = 0; i < lines.length; i++) {
    const l = lines[i];
    if (l.startsWith('#EXT-X-STREAM-INF:')) {
      const a = attrs(l.slice(18));
      const uri = lines[i + 1] && !lines[i + 1].startsWith('#') ? lines[i + 1] : null;
      if (!uri) continue;
      // 清晰度按短边叫：竖屏的 1080x1920 是 1080p，不是 1920p（和桌面端 hls.rs、probe.rs 一样）
      const [rw, rh] = a.RESOLUTION ? a.RESOLUTION.split('x').map(Number) : [0, 0];
      const h = rw && rh ? Math.min(rw, rh) : (rh || 0);
      const bw = +(a['AVERAGE-BANDWIDTH'] || a.BANDWIDTH) || 0;
      variants.push({ p: h ? `${h}p` : `${Math.round(bw / 1000)} kbps`, h, bw, uri: new URL(uri, base).href, audio: !!(a.AUDIO && audioGroups.has(a.AUDIO)) });
    }
    if (l.startsWith('#EXTINF:')) duration += parseFloat(l.slice(8)) || 0;
    if (l.startsWith('#EXT-X-ENDLIST')) live = false;
    if (l.startsWith('#EXT-X-KEY:') && !/METHOD=NONE/.test(l)) encrypted = true;
  }
  if (variants.length) {
    variants.sort((a, b) => b.h - a.h || b.bw - a.bw);
    /* 同一个清晰度只留码率最高的一路 */
    const seen = new Set();
    return { master: true, variants: variants.filter(v => !seen.has(v.p) && seen.add(v.p)) };
  }
  return { master: false, duration, live, encrypted };
}

function attrs(s) {
  const out = {};
  const re = /([A-Z0-9-]+)=("[^"]*"|[^,]*)/g;
  let m;
  while ((m = re.exec(s))) out[m[1]] = m[2].replace(/^"|"$/g, '');
  return out;
}

/* 剪贴板里的文字是不是一个能下载的链接 */
export function isMediaLink(text) {
  const t = (text || '').trim();
  if (!/^https?:\/\/\S+$/.test(t) || t.length > 2000) return false;
  if (pageSite(t)) return true;
  const ext = extOf(t);
  return VIDEO_EXT.includes(ext) || AUDIO_EXT.includes(ext) || HLS_EXT.includes(ext);
}

export function clock(sec) {
  if (!sec || !isFinite(sec)) return '';
  sec = Math.round(sec);
  const h = Math.floor(sec / 3600), m = Math.floor(sec % 3600 / 60), s = sec % 60;
  return h ? `${h}:${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}` : `${m}:${String(s).padStart(2, '0')}`;
}

/* 按码率和时长估算大小（字节） */
export function estBytes(bw, sec) {
  return bw && sec ? Math.round(bw / 8 * sec) : 0;
}

export function mbText(bytes) {
  if (!bytes) return '';
  if (bytes >= 1e9) return tr('约{size} GB', { size: decimal(bytes / 1e9, 1) });
  if (bytes >= 1e6) return tr('约{size} MB', { size: Math.round(bytes / 1e6) });
  return tr('约{size} KB', { size: Math.max(1, Math.round(bytes / 1e3)) });
}
