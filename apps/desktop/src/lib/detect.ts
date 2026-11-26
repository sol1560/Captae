// 输入框里的链接识别：粘贴后马上告诉用户这是什么、会怎么下。只用来显示，真正的判断在后端（detect.rs）。
import { t, tAny } from "@/i18n";
import { host } from "./format";

export type LinkKind = "page" | "hls" | "file";
export type Detected = {
  /** 补全了 https:// 的网址，加任务时用它 */
  url: string;
  host: string;
  kind: LinkKind;
  /** 「YouTube 视频」「MP4 文件」（按当前语言，detect 时生成） */
  label: string;
  /** 一句说明：会怎么下 */
  note: string;
  /** 站点代号，给 SiteIcon 用 */
  site?: string;
  /** 这一次还不支持（视频号），不让提交 */
  unsupported?: boolean;
};

/** [域名, 显示名, 站点代号]。显示名是存下来的原文（中文站点名在 i18n/sites.ts 里标了），显示时 tAny */
const SITES: [string, string, string][] = [
  ["youtube.com", "YouTube", "youtube"],
  ["youtu.be", "YouTube", "youtube"],
  ["bilibili.com", "哔哩哔哩", "bilibili"],
  ["b23.tv", "哔哩哔哩", "bilibili"],
  ["douyin.com", "抖音", "douyin"],
  ["tiktok.com", "TikTok", "tiktok"],
  ["x.com", "X", "x"],
  ["twitter.com", "X", "x"],
  ["instagram.com", "Instagram", "instagram"],
  ["vimeo.com", "Vimeo", "vimeo"],
  ["xiaohongshu.com", "小红书", "xiaohongshu"],
  ["rednote.com", "小红书", "xiaohongshu"],
  ["weibo.com", "微博", "weibo"],
];

const NAMES: Record<string, string> = Object.fromEntries(SITES.map(([, n, k]) => [k, n]));

function siteOf(h: string) {
  return SITES.find(([d]) => h === d || h.endsWith("." + d));
}

/** 用户常常复制不带 https:// 的地址（youtube.com/watch?v=…），这里补上。不像网址时返回 null */
export function normalizeUrl(input: string): string | null {
  const s = input.trim();
  if (!s || /\s/.test(s)) return null;
  const full = /^[a-z][a-z0-9+.-]*:\/\//i.test(s) ? s : /^[\w-]+(\.[\w-]+)+([/:?#]|$)/.test(s) ? "https://" + s : null;
  if (!full) return null;
  try {
    const u = new URL(full);
    if (!/^(https?|ftp):$/.test(u.protocol) || !u.hostname.includes(".")) return null;
    return full;
  } catch {
    return null;
  }
}

/** 视频号：用户决定这次先提示暂不支持 */
const isChannels = (u: URL) => u.hostname === "channels.weixin.qq.com" || (u.hostname.endsWith("weixin.qq.com") && u.pathname.startsWith("/sph"));

export function detect(input: string): Detected | null {
  const url = normalizeUrl(input);
  if (!url) return null;
  const u = new URL(url);
  const h = u.hostname.replace(/^www\./, "");
  const p = u.pathname.toLowerCase();
  if (isChannels(u)) return { url, host: h, kind: "page", label: t("微信视频号"), note: t("暂不支持，这次还下不了"), unsupported: true };
  const s = siteOf(h);
  if (s) return { url, host: h, kind: "page", label: t("{site} 视频", { site: tAny(s[1]) }), note: t("会自动选最清晰的版本"), site: s[2] };
  if (p.endsWith(".m3u8")) return { url, host: h, kind: "hls", label: t("在线视频（M3U8）"), note: t("读到以后可以选清晰度，下完存成 MP4") };
  const m = p.match(/\.(mp4|mov|webm|mkv|mp3|m4a|flac|wav|png|jpe?g|webp|avif|gif)$/);
  if (m) return { url, host: h, kind: "file", label: t("{fmt} 文件", { fmt: m[1].toUpperCase() }), note: t("直接下载，暂停后可以继续") };
  return { url, host: h, kind: "page", label: t("网页"), note: t("会找出网页里的视频") };
}

/** 后端 errors.rs 的 site_name 认得、上面那张表里没有的网站（只给 siteLabel 用：不改输入框的识别和网站图标） */
const MORE: [string, string][] = [
  ["iesdouyin.com", "抖音"],
  ["fb.watch", "Facebook"],
  ["facebook.com", "Facebook"],
  ["twitch.tv", "Twitch"],
  ["weibo.cn", "微博"],
  ["xhslink.com", "小红书"],
  ["rednote.com", "小红书"],
  ["kuaishou.com", "快手"],
  ["reddit.com", "Reddit"],
  ["soundcloud.com", "SoundCloud"],
  ["dailymotion.com", "Dailymotion"],
  ["threads.net", "Threads"],
  ["threads.com", "Threads"],
  ["weixin.qq.com", "微信"],
  ["v.qq.com", "腾讯视频"],
  ["iqiyi.com", "爱奇艺"],
  ["youku.com", "优酷"],
  ["nicovideo.jp", "niconico"],
  ["ixigua.com", "西瓜视频"],
  ["zhihu.com", "知乎"],
  ["acfun.cn", "AcFun"],
  ["mgtv.com", "芒果TV"],
  ["music.163.com", "网易云音乐"],
  ["ximalaya.com", "喜马拉雅"],
  ["patreon.com", "Patreon"],
  ["flickr.com", "Flickr"],
  ["pixiv.net", "pixiv"],
  ["9gag.com", "9GAG"],
  ["imgur.com", "Imgur"],
  ["lemon8-app.com", "Lemon8"],
];

/** 任务来自哪个网站：「YouTube」「哔哩哔哩」「芒果TV」，不认识的显示域名。
 *  返回的是存下来的原文（判断「是不是 YouTube」用它），显示时 tAny(…)。
 *  named：后端在失败信息里给的名字（Failure.site，和那句说法用的是同一个），有就用它 */
export function siteLabel(plat: string | null | undefined, url: string, named?: string | null): string {
  if (named) return named;
  if (plat && NAMES[plat]) return NAMES[plat];
  const h = host(url);
  const s = siteOf(h);
  if (s) return s[1];
  const m = MORE.find(([d]) => h === d || h.endsWith("." + d));
  return m ? m[1] : h;
}
