// 数字、时间、路径的显示格式。
// 插件工具栏面板也用这个文件（面板只加载 common 和 popup 两个分册），这里的文字都放在 common 分册。
// 日期、时间：简体中文照原来的写法拼（「9月3日 10:02」「11 月 22 日」，和 Intl 的中文写法不一样），其他语言用 Intl，一律 24 小时制。
import { decimal, lang, t } from "@/i18n";

/** 中文里夹名字时，名字是英文、数字开头 / 结尾的那一边加空格：「登录 YouTube」「登录哔哩哔哩」「在 X 验证」「交给 127.0.0.1，」 */
export function cjk(a: string, name: string, b = ""): string {
  const ascii = (c?: string) => !!c && /[A-Za-z0-9]/.test(c);
  return `${a}${a && ascii(name[0]) ? " " : ""}${name}${b && ascii(name[name.length - 1]) ? " " : ""}${b}`;
}

/** 12 MB / 1.40 GB；10 MB 以下保留一位小数。单位各语言一样，小数点跟语言（decimal） */
export function mb(n: number): string {
  if (!isFinite(n) || n <= 0) return "0 MB";
  if (n >= 1000) return decimal(n / 1000, 2) + " GB";
  if (n < 10) return decimal(n, 1) + " MB";
  return Math.round(n) + " MB";
}

/** 字节 → 「约 312 MB」里的数字部分 */
export function bytes(n?: number | null): string {
  if (!n || n <= 0) return "";
  return mb(n / 1e6);
}

/** 剩余时间：26 秒 / 3 分 12 秒 / 1 小时 5 分（英文 26 sec / 3 min 12 sec / 1 hr 5 min） */
export function dur(s: number): string {
  s = Math.max(0, Math.round(s));
  if (s >= 3600) return t("{h}小时{m}分", { h: Math.floor(s / 3600), m: Math.floor((s % 3600) / 60) });
  if (s >= 60) return t("{m}分{s}秒", { m: Math.floor(s / 60), s: s % 60 });
  return t("{n}秒§时长的简写（剩余 26 秒）", { n: s });
}

/** 视频时长：0:52 / 14:48 / 1:02:03 */
export function clock(s?: number | null): string {
  if (s == null || !isFinite(s)) return "";
  s = Math.max(0, Math.round(s));
  const h = Math.floor(s / 3600),
    m = Math.floor((s % 3600) / 60),
    r = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${r}` : `${m}:${r}`;
}

const zh = () => lang() === "zh-CN";

/** 时:分，24 小时制：09:47 */
function hm(d: Date): string {
  if (zh()) return d.toTimeString().slice(0, 5);
  return new Intl.DateTimeFormat(lang(), { hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(d);
}

/** 「今天」「昨天」「11 月 22 日」「2025 年 11 月 22 日」（只到天，设置里「什么时候登录的」用）。
 *  英文 today / yesterday / Nov 22 / Nov 22, 2025：设置里嵌在句子中间（Signed in today），所以小写。
 *  原文带「§」说明：别处单独显示的「今天」可能要大写，分开两条 */
export function dayLabel(ms?: number | null): string {
  if (!ms) return "";
  const d = new Date(ms);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  if (d >= today) return t("今天§只到天，常嵌在句子中间（今天登录），开头不大写");
  if (d.getTime() >= today.getTime() - 864e5) return t("昨天§只到天，常嵌在句子中间（昨天登录），开头不大写");
  const thisYear = d.getFullYear() === today.getFullYear();
  if (!zh()) return new Intl.DateTimeFormat(lang(), thisYear ? { month: "short", day: "numeric" } : { year: "numeric", month: "short", day: "numeric" }).format(d);
  const y = thisYear ? "" : `${d.getFullYear()} 年 `;
  return `${y}${d.getMonth() + 1} 月 ${d.getDate()} 日`;
}

/** 完成时间：刚刚 / 3 分钟前 / 今天 09:47 / 昨天 22:15 / 9月3日 10:02。
 *  英文 just now / 3 minutes ago / today at 09:47 / yesterday at 22:15 / Sep 3, 10:02：
 *  别的页面会把它嵌在句子中间（「上次见到是…」「…用过」），所以小写开头（原文带「§」说明，和别处单独显示的分开） */
export function whenLabel(ms?: number | null, fallback = ""): string {
  if (!ms) return fallback;
  const d = new Date(ms),
    s = (Date.now() - ms) / 1000;
  if (s < 60) return t("刚刚§相对时间，常嵌在句子中间，开头不大写");
  if (s < 3600) return t("{n}分钟前§相对时间，常嵌在句子中间，开头不大写", { n: Math.floor(s / 60) });
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  if (d >= today) return t("今天{time}§相对时间，常嵌在句子中间，开头不大写", { time: hm(d) });
  if (d.getTime() >= today.getTime() - 864e5) return t("昨天{time}§相对时间，常嵌在句子中间，开头不大写", { time: hm(d) });
  if (!zh()) return new Intl.DateTimeFormat(lang(), { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(d);
  return `${d.getMonth() + 1}月${d.getDate()}日 ${hm(d)}`;
}

/** 已经等了多久：2.4s / 1m 05s */
export function since(ms: number): string {
  const d = Math.max(0, (Date.now() - ms) / 1000);
  if (d < 60) return t("{s}s§已经等了多久：秒数，带一位小数（2.4s）", { s: decimal(d, 1) });
  return t("{m}m {s}s§已经等了多久：分和秒（1m 05s）", { m: Math.floor(d / 60), s: String(Math.floor(d % 60)).padStart(2, "0") });
}

/** 还要多久（安装、转换）：马上就好 / 还要约 40 秒 / 还要约 3 分钟 */
export function etaText(s?: number | null): string {
  if (s == null) return "";
  if (s <= 5) return t("马上就好");
  if (s < 60) return t("还要约{n}秒", { n: Math.max(10, Math.ceil(s / 10) * 10) });
  if (s < 3600) return t("还要约{n}分钟", { n: Math.ceil(s / 60) });
  return t("还要 1 小时以上");
}

/** 家目录缩成 ~ */
/** 用户目录写成 ~：macOS /Users/<名字>、Linux /home/<名字>、Windows C:\Users\<名字>（Windows 的路径照样用反斜杠，只换开头） */
export const home = (p?: string | null) =>
  String(p ?? "")
    .replace(/^\/Users\/[^/]+/, "~")
    .replace(/^\/home\/[^/]+/, "~")
    .replace(/^[A-Za-z]:\\Users\\[^\\]+/, "~");

/** 网址的主机名，去掉 www. */
export function host(u: string): string {
  try {
    return new URL(/^[a-z]+:\/\//i.test(u) ? u : "https://" + u).hostname.replace(/^www\./, "");
  } catch {
    return u.split("/")[0];
  }
}

/** 去掉协议，显示用 */
export const bare = (u: string) => u.replace(/^https?:\/\/(www\.)?/, "");
