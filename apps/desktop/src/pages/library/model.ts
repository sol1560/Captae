// 素材库的纯逻辑：后端素材的结构、筛选条件怎么变成查询、每张卡片和详情里显示什么、
// 转换 / 字幕 / 剪辑的选项和默认值。界面组件只管画，判断都在这里，方便和旧界面（ui/js/page-library.js）逐条对照。
import { home } from "@/lib/format";
import { decimal, lang, LANGS, N_, t, tAny, type Key } from "@/i18n";
import { isMac } from "@/lib/os";

/* ---------------- 后端的结构（crates/captae-core/src/library.rs 的 Asset，字段名照 serde 原样） ---------------- */

/** video / audio / image；后端还可能有 subtitle、other（AI 助手、预设导入的文件） */
export type AssetKind = "video" | "audio" | "image" | "subtitle" | "other";

export type Asset = {
  id: number;
  type: AssetKind | string;
  title: string;
  desc: string;
  /** 下载链接 */
  url: string;
  /** 来源页面（有就用它当「来源」） */
  page: string;
  /** 来源站点：YouTube、哔哩哔哩、blender.org、本地…… */
  src: string;
  uploader: string;
  path: string;
  /** 扩展名大写：MP4、MP3、JPG…… */
  fmt: string;
  width?: number | null;
  height?: number | null;
  /** 时长（秒） */
  durS?: number | null;
  bytes: number;
  thumb?: string | null;
  tags: string[];
  /** 入库时间（毫秒） */
  created_at: number;
  /** 转换、剪辑、拼接出来的新文件指向原素材 */
  parent_id?: number | null;
  has_transcript: boolean;
  /** 「1920×1080」，不知道时是「—」 */
  res: string;
  /** 「12:14」，没有时长时是空字符串 */
  dur: string;
  /** 「420 MB」 */
  size: string;
  /** 后端算好的「今天 10:12 / 昨天 22:15 / 3 天前」 */
  when: string;
  day: number;
  /** 文件还在不在原来的地方 */
  exists: boolean;
};

/** library_query 的返回：这一页、符合筛选的总数、库里一共多少、筛选条要用的来源和标签（按数量排） */
export type LibPage = { items: Asset[]; total: number; all: number; sites: [string, number][]; tags: [string, number][] };

/** 网站上能下载的一条字幕（subtitle.rs 的 Track） */
export type Track = { code: string; name: string; auto: boolean };

/** subtitle_tracks 的返回 */
export type SubInfo = { tracks: Track[]; err: string | null; hasAudio: boolean; whisper: boolean; canBurn: boolean };

/** task_subtitle 的返回：字幕文件、来源、条数、前几条预览（[时间, 文字]），嵌进画面的新视频 */
export type SubResult = { path: string; source: string; language?: string | null; cues: number; preview?: [string, string][]; burned?: string; burnedId?: number };

/** task_convert / task_trim / task_concat 的返回 */
export type NewFile = { path: string; assetId: number };

/* ---------------- 筛选和查询 ---------------- */

export type KindFilter = "" | "video" | "audio" | "image";
export type Filters = { kind: KindFilter; site: string; days: 0 | 1 | 7 | 30; tag: string; q: string };
export const NO_FILTERS: Filters = { kind: "", site: "", days: 0, tag: "", q: "" };

/** 一页多少条：和旧界面一样 60，往下滚到底附近再要下一页 */
export const PAGE = 60;

/** 筛选条件 → library_query 的 q（后端 Query 结构） */
export function queryOf(f: Filters, offset: number, limit: number): Record<string, unknown> {
  const q: Record<string, unknown> = { limit, offset };
  if (f.kind) q.kind = f.kind;
  if (f.site) q.site = f.site;
  if (f.days) q.days = f.days;
  if (f.tag) q.tags = [f.tag];
  const text = f.q.trim();
  if (text) q.q = text;
  return q;
}

/** 除了类型以外还筛了什么（类型在分段控件上已经看得见） */
export const extraFilters = (f: Filters) => !!(f.site || f.days || f.tag);
export const anyFilter = (f: Filters) => !!(f.kind || f.site || f.days || f.tag || f.q.trim());

/** 时间筛选的几档；label 是原文，显示时 t(label)。「今天」加了说明：lib/format.ts 的「今天」嵌在句子里（英文小写），这里是单独的选项 */
export const DAYS: { v: Filters["days"]; label: Key }[] = [
  { v: 0, label: N_("任何时间") },
  { v: 1, label: N_("今天§素材库筛选菜单里的时间选项") },
  { v: 7, label: N_("近 7 天") },
  { v: 30, label: N_("近 30 天") },
];

/** 「筛选」按钮上的字：没筛就是「筛选」，筛了就写出筛了什么（来源存的是中文原文，显示时翻译） */
export function filterLabel(f: Filters): string {
  const day = f.days ? DAYS.find((d) => d.v === f.days)?.label : undefined;
  const parts = [f.site ? tAny(f.site) : "", day ? t(day) : "", f.tag ? `#${f.tag}` : ""].filter(Boolean);
  if (!parts.length) return t("筛选");
  return parts.length > 2 ? t("{first} 等 {n} 项", { first: parts[0], n: parts.length }) : parts.join(" · ");
}

/** 没有匹配时的标题：按筛的类型整句写（别的语言里名词不能直接拼进句子） */
const NO_MATCH: Record<KindFilter, Key> = {
  "": N_("没有符合条件的素材"),
  video: N_("没有符合条件的视频"),
  audio: N_("没有符合条件的音频"),
  image: N_("没有符合条件的图片"),
};

/** 空列表的说明：库是空的 / 没有匹配的 */
export function emptyText(all: number, f: Filters): { title: string; text: string; clear: boolean } {
  if (!all && !anyFilter(f)) return { title: t("素材库还是空的"), text: t("下载完成的视频、音乐和图片都会自动放进来"), clear: false };
  if (f.q.trim()) return { title: t("没有找到"), text: t("可以搜标题、简介、标签和字幕里的话，换个词试试"), clear: true };
  return { title: t(NO_MATCH[f.kind] ?? NO_MATCH[""]), text: t("换个筛选条件试试"), clear: extraFilters(f) || !!f.kind };
}

/* ---------------- 显示 ---------------- */

/** 一条素材是什么（卡片上没有格式时写它）；显示时 t()。和顶栏的类型分段控件（复数）分开写 */
export const KIND_NAME: Record<AssetKind, Key> = {
  video: N_("视频§一条素材的种类"),
  audio: N_("音频§一条素材的种类"),
  image: N_("图片§一条素材的种类"),
  subtitle: N_("字幕"),
  other: N_("文件§一条素材的种类"),
};
export const kindName = (a: Asset) => t(KIND_NAME[a.type as AssetKind] ?? KIND_NAME.other);
export const isVideo = (a: Asset) => a.type === "video";
export const isAudio = (a: Asset) => a.type === "audio";
export const isImage = (a: Asset) => a.type === "image";
/** 有声音、有时长的：能剪辑、能做字幕 */
export const isMedia = (a: Asset) => a.type === "video" || a.type === "audio";

/** 站点名 → SiteIcon 认得的代号；不认识的用来源网址找图标 */
const SITE_KEYS: Record<string, string> = { YouTube: "youtube", 哔哩哔哩: "bilibili", 抖音: "douyin", TikTok: "tiktok", X: "x", Instagram: "instagram" };
export const siteKey = (a: Asset) => SITE_KEYS[a.src] ?? (a.page || a.url || null);
export const siteKeyOf = (site: string) => SITE_KEYS[site] ?? site;

/** 来源链接：有来源页面用页面，否则用下载链接；本地导入的没有 */
export const sourceUrl = (a: Asset) => (/^https?:\/\//i.test(a.page) ? a.page : /^https?:\/\//i.test(a.url) ? a.url : "");

/** 来源站点的名字（显示用）：空的算「本地」。存的是中文原文（「本地」「哔哩哔哩」），显示时翻译 */
export const siteName = (a: Asset) => tAny(a.src || "本地");

/** 卡片第二行：图片写尺寸，其余写格式 */
export const metaFmt = (a: Asset) => (isImage(a) && a.res && a.res !== "—" ? a.res : a.fmt || kindName(a));

// 日期、时间：简体中文照原来的写法拼（「11月21日」「2026年11月21日 19:46」，一字不差），其他语言用 Intl，一律 24 小时制（docs/I18N.md 2.5）
const zh = () => lang() === "zh-CN";

/** 时:分：19:51 */
function hm(d: Date): string {
  if (zh()) return d.toTimeString().slice(0, 5);
  return new Intl.DateTimeFormat(lang(), { hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(d);
}

/** 入库时间：今天 19:51 / 昨天 22:10 / 11月21日 / 2025年3月2日（卡片上用，比后端的「3 天前」好认）。
 *  「今天」「昨天」加了说明：这里是卡片上单独的一段（英文句首大写），lib/format.ts 的是嵌在句子里的 */
export function dayLabel(ms: number, fallback = ""): string {
  if (!ms) return fallback;
  const d = new Date(ms);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  if (d >= today) return t("今天{time}§素材卡片上的入库时间", { time: hm(d) });
  if (d.getTime() >= today.getTime() - 864e5) return t("昨天{time}§素材卡片上的入库时间", { time: hm(d) });
  const thisYear = d.getFullYear() === today.getFullYear();
  if (!zh()) return new Intl.DateTimeFormat(lang(), thisYear ? { month: "short", day: "numeric" } : { year: "numeric", month: "short", day: "numeric" }).format(d);
  if (thisYear) return `${d.getMonth() + 1}月${d.getDate()}日`;
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日`;
}

/** 详情里的完整时间：2026年11月21日 19:46（英文 Nov 21, 2026, 19:46） */
export function fullTime(ms: number): string {
  if (!ms) return "";
  const d = new Date(ms);
  if (!zh()) return new Intl.DateTimeFormat(lang(), { year: "numeric", month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(d);
  return `${d.getFullYear()}年${d.getMonth() + 1}月${d.getDate()}日 ${hm(d)}`;
}

/** 文件放在哪：~/Movies/Captae/a.mp4 → 影片 › Captae（和下载页「会保存为」同一个说法）。
 *  文件夹名照系统在各语言里的叫法（docs/I18N.md 8.3），显示时 t()：macOS「影片」「文稿」，Windows、Linux「视频」「文档」。
 *  Windows 的路径是反斜杠（~\Videos\Captae），两种分隔符都认；Linux 中文界面下用户文件夹本身就叫「视频」，原样显示 */
const PLACES: [RegExp, Key][] = isMac
  ? [
      [/^~\/Movies\b/, N_("影片§访达里的文件夹")],
      [/^~\/Music\b/, N_("音乐§访达里的文件夹")],
      [/^~\/Pictures\b/, N_("图片§访达里的文件夹")],
      [/^~\/Downloads\b/, N_("下载§访达里的文件夹")],
      [/^~\/Desktop\b/, N_("桌面§访达里的文件夹")],
      [/^~\/Documents\b/, N_("文稿§访达里的文件夹")],
    ]
  : [
      [/^~[/\\]Videos\b/, N_("视频§Windows、Linux 的用户文件夹")],
      [/^~[/\\]Music\b/, N_("音乐§Windows、Linux 的用户文件夹")],
      [/^~[/\\]Pictures\b/, N_("图片§Windows、Linux 的用户文件夹")],
      [/^~[/\\]Downloads\b/, N_("下载§Windows、Linux 的用户文件夹")],
      [/^~[/\\]Desktop\b/, N_("桌面§Windows、Linux 的用户文件夹")],
      [/^~[/\\]Documents\b/, N_("文档§Windows、Linux 的用户文件夹")],
    ];
export function placeOf(path: string): string {
  const dir = home(path).replace(/[/\\][^/\\]*$/, "");
  for (const [re, name] of PLACES) if (re.test(dir)) return dir.replace(re, () => t(name)).split(/[/\\]/).filter(Boolean).join(" › ");
  return dir;
}

export const fileName = (p: string) => String(p).split(/[\\/]/).pop() ?? "";

/* ---------------- 转换 ---------------- */

export type FmtOpt = { value: string; label: string; description: string };

/** 每种素材能转成什么（和旧界面一致，ffmpeg.rs 的 target 都支持）。渲染时调用，文字直接按当前语言给 */
export function fmtOpts(a: Asset): FmtOpt[] {
  if (isAudio(a))
    return [
      { value: "MP3", label: "MP3", description: t("哪里都能播") },
      { value: "AAC", label: "AAC", description: t("体积小，苹果设备常用") },
      { value: "FLAC", label: "FLAC", description: t("无损，文件大") },
      { value: "WAV", label: "WAV", description: t("无损，剪辑软件常用") },
    ];
  if (isImage(a))
    return [
      { value: "PNG", label: "PNG", description: t("无损") },
      { value: "JPG", label: "JPG", description: t("哪里都能看") },
      { value: "WebP", label: "WebP", description: t("网页用，文件小") },
      { value: "AVIF", label: "AVIF", description: t("更小，比较新的格式") },
    ];
  return [
    { value: "MP4", label: t("MP4（H.264）"), description: t("哪里都能播") },
    { value: "MOV", label: "MOV", description: t("苹果设备和剪辑软件") },
    { value: "ProRes 422", label: "ProRes 422", description: t("给剪辑软件用，文件大") },
    { value: "WebM", label: "WebM", description: t("网页用，文件小") },
    { value: "MP3", label: t("只要声音 · {fmt}", { fmt: "MP3" }), description: t("把声音单独存出来") },
    { value: "AAC", label: t("只要声音 · {fmt}", { fmt: "AAC" }), description: t("体积小") },
    { value: "FLAC", label: t("只要声音 · {fmt}", { fmt: "FLAC" }), description: t("无损") },
  ];
}

/** 默认转成什么：视频 ProRes 422（设计稿），音频 AAC，图片 WebP；和原来一样时换下一个 */
export function defaultFmt(a: Asset): string {
  const opts = fmtOpts(a).map((o) => o.value);
  const want = isVideo(a) ? "ProRes 422" : isAudio(a) ? "AAC" : "WebP";
  if (sameFormat(a.fmt, want)) return opts.find((o) => !sameFormat(a.fmt, o)) ?? want;
  return want;
}

/** 转成声音（视频提取音频） */
export const isAudioFmt = (f: string) => ["MP3", "AAC", "FLAC", "WAV"].includes(f);

const EXT: Record<string, string> = { "ProRes 422": "mov", MOV: "mov", MP4: "mp4", WebM: "webm", MP3: "mp3", AAC: "m4a", FLAC: "flac", WAV: "wav", PNG: "png", WebP: "webp", AVIF: "avif", JPG: "jpg" };
export const extOf = (f: string) => EXT[f] ?? f.toLowerCase();

/** 来源格式和目标格式是不是同一种文件（MP4 ↔ MP4、JPEG ↔ JPG） */
export function sameFormat(src: string, target: string): boolean {
  const n = (x: string) => x.toLowerCase().replace("jpeg", "jpg").replace("m4a", "aac");
  return n(src) === n(target) || n(src) === extOf(target);
}

/** 输出文件名，和 ffmpeg.rs 的 convert_out_path 一致：扩展名没变时加「 (转换)」（重名时后端还会再加编号）。
 *  后缀后端按语言写（tr!，同一句原文），这里的原文和译文要和后端词表（crates/captae-core/locales）保持一样 */
export function outPath(a: Asset, fmt: string): string {
  const ext = extOf(fmt);
  const m = a.path.match(/^(.*?)(\.([^./\\]+))?$/);
  const stem = m?.[1] ?? a.path;
  const old = (m?.[3] ?? "").toLowerCase();
  return stem + (old === ext ? " " + t("(转换)§转换出来的文件名后缀：「Sintel (转换).mp4」") : "") + "." + ext;
}

/** 文件图标上的字：ProRes 422 → PRORES，AAC 存成 M4A */
export const fileIconFmt = (f: string) => (f === "AAC" ? "M4A" : f);

/** 转换的分辨率、画质选项（渲染时调用） */
export const resOpts = () => [
  { value: "", label: t("和原来一样") },
  { value: "1080", label: "1080p" },
  { value: "720", label: "720p" },
];
export const qualityOpts = () => [
  { value: "", label: t("和原来一样") },
  { value: "high", label: t("更清晰"), description: t("文件更大") },
  { value: "small", label: t("更省空间"), description: t("画质略差") },
];

/* ---------------- 字幕 ---------------- */

export type SubMode = "platform" | "whisper";
export const SUB_FORMATS = ["SRT", "VTT", "TXT"] as const;
/** 语音识别的语言（渲染时调用）。语言名用各自的语言写（中文、English、日本語……），不翻译，只翻「自动检测」 */
export const whisperLangs = (): [string, string][] => [
  ["", t("自动检测")],
  ["zh", "中文"],
  ["en", "English"],
  ["ja", "日本語"],
  ["ko", "한국어"],
];

/** 网站字幕的名字：「English（自动生成）」后端已经写好；没写的补上 */
// 后端给的名字不带「（自动生成）」（旧版缓存在素材库里的带着，先去掉再加）
export const trackName = (tk: Track) => (tk.auto ? t("{name}（自动生成）", { name: tk.name.replace(/（自动生成）$/, "") }) : tk.name);

/** 字幕翻译成哪种语言：界面语言。简体中文照旧传「中文」（文件名后缀「.中文.srt」和原来一样），
 *  其他语言传它自己的名字（English、日本語……），翻译接口按这个名字翻，文件名后缀也是它 */
export const translateTarget = () => (lang() === "zh-CN" ? "中文" : (LANGS.find((l) => l.code === lang())?.name ?? "English"));

/** 字幕任务的步骤（显示用；进度按百分比落在第几步）。trackLabel：网站字幕那一条的名字（不带「（自动生成）」），用语音转文字时不用 */
export function subSteps(mode: SubMode, trackLabel: string, fmt: string, translate: boolean, burn: boolean): string[] {
  const s = mode === "platform" ? [t("读取网站字幕"), t("下载{lang}字幕", { lang: trackLabel })] : [t("提取声音"), t("把语音转成文字")];
  if (translate) s.push(t("翻译成中文§字幕翻译成界面语言：每种语言都写「翻译成这种语言」（英文 Translate to English）"));
  s.push(t("导出{fmt}", { fmt }));
  if (burn) s.push(t("把字幕嵌进画面"));
  return s;
}

/* ---------------- 剪辑 ---------------- */

/** 00:12.4 */
export function tc(s: number): string {
  const ds = Math.round(Math.max(0, s) * 10);
  return String(Math.floor(ds / 600)).padStart(2, "0") + ":" + String(Math.floor(ds / 10) % 60).padStart(2, "0") + "." + (ds % 10);
}
/** 5 秒 / 1.5 秒 / 2 分 5 秒（和 lib/format.ts 的 dur 同样的说法；小数按语言写小数点） */
export function secs(s: number): string {
  const r = Math.round(s * 10) / 10;
  if (r >= 60) return t("{m}分{s}秒", { m: Math.floor(r / 60), s: Math.round(r % 60) });
  return t("{n}秒§时长的简写（剩余 26 秒）", { n: r % 1 ? decimal(r, 1) : r });
}
/** 一开始选中的片段：从 20% 处开始，长度取 5 秒和全长的 10% 里大的那个（旧界面固定 5 秒，长视频上选区细得看不见、两个把手叠在一起） */
export function defaultRange(d: number): [number, number] {
  const i = Math.round(d * 0.2 * 10) / 10;
  return [i, Math.min(d, Math.round((i + Math.max(5, d * 0.1)) * 10) / 10)];
}

/* ---------------- 多选 ---------------- */

/** 拼接：只拼还在的视频，按选中的先后顺序 */
export const concatable = (L: Asset[]) => L.filter((a) => isVideo(a) && a.exists);
/** 批量转成 MP4：还在的视频里不是 MP4 的 */
export const needMp4 = (L: Asset[]) => L.filter((a) => isVideo(a) && a.exists && !sameFormat(a.fmt, "MP4"));

/** 拼接后有多长、画面大小是不是都一样（一样就直接接起来，很快） */
export function concatInfo(L: Asset[]): { total: number; sameRes: boolean } {
  return { total: L.reduce((s, a) => s + (a.durS ?? 0), 0), sameRes: L.every((a) => a.res === L[0]?.res) };
}

/** 网格里一行几个：用来算上下键跳几个 */
export function gridColumns(el: HTMLElement | null): number {
  if (!el) return 1;
  const cols = getComputedStyle(el).gridTemplateColumns.split(" ").filter(Boolean).length;
  return Math.max(1, cols);
}
