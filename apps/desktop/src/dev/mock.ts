// 演示后端：在浏览器里带 ?mock 打开界面时代替 Captae 的后台，给截图和检查界面用。
// 只在 vite 开发服务器和 `npm run build:mock` 的产物里有，正式安装包里没有（见 lib/bridge.ts 的 setupMock）。
//
// 网址参数：
//   ?mock          演示数据齐全的下载页（读取中、选清晰度、下载中、排队、暂停、要登录、要验证、多视频、失败、已完成）
//   ?mock=empty    第一次打开，什么都没有
//   &still         停住所有计时（截图用：进度、倒计时、读取都不动）
//   &clip          打开后马上来一条「复制了视频链接」
//   &open=settings 打开时直接弹出设置（onboarding 同理）
//   &side=off      侧栏收起来
//   &wait          打开后 YouTube 那个要登录的任务（27）进入「正在等你在弹出的窗口里登录」
//   &notice        打开后来一条「这次带上了你的 YouTube 登录状态」（只提示一次的那条）
//   &browser=chrome 设置里「也读取浏览器里的登录状态」打开，读 Chrome（safari 等同理）
//   &rec          多一个正在录的直播（31，已录 12:34）和一个录完的（15）；点「停止并保存」1.5 秒后录完、放进素材库
//   &rec=saving   录直播那个停在「正在保存录到的部分」；&rec=interrupted 录完的那个是「录制被中断」以后整理好的
//   &fails        多几种第三阶段的失败说法（41–45）：下载引擎读不了、图片帖、要登录的试看片段、地区限制、正在直播
//   &bili=parts   B 站那个下载中的任务（23）是分 P 视频里的第 2 P（共 5 P）
//   &tour         官网「桌面」演示用（src/site/desk.tsx）：下载快 10 倍（312 MB 约 5 秒）、读取 1.4 秒；YouTube 链接读到以后让人挑清晰度；
//                 官网用 postMessage({ captaeDemo: "clipboard", url }) 告诉它「刚复制了链接」，它照真后端那样弹出剪贴板提示
//   &lang=en      界面换成这种语言（en、zh-TW、ja、ko、de、es、fr），演示数据也跟着换（mock-i18n.ts；不带就是中文，截图照旧）
//   #/login-bar    登录窗口里的工具条（&bar=verify 人机验证、&bar=insecure 没加密的网页、&bar=other 没有登录规则的网站）
// 带 &still 时先清掉界面偏好（localStorage 里 captae3: 开头的），每张截图都从同一个状态开始。
//
// 素材库、AI 助手、浏览器插件三页的演示命令分别在 mock-library.ts、mock-agent.ts、mock-extension.ts；设置 › 手机在 mock-remote.ts（&phone=off|none|void|err）。
import type { CallRecord, CaptaeEvent, Entry, Job, Preset, SegState, Settings, Sniffed, TaskStatus } from "@/lib/types";
import { AGENT_TOOLS, agentHandlers, DEMO_PRESETS, demoCalls } from "./mock-agent";
import { extensionHandlers } from "./mock-extension";
import { libraryHandlers } from "./mock-library";
import { remoteHandlers } from "./mock-remote";
import bbb from "./img/bbb-frank.jpg";
import edEmo from "./img/ed-emo-proog.jpg";
import edPlat from "./img/ed-platform.jpg";
import sintel1 from "./img/sintel-1.jpg";
import sintel3 from "./img/sintel-3.jpg";
import tos1 from "./img/tos-01.jpg";
import tos5 from "./img/tos-05.jpg";
import { localizer } from "./mock-i18n";

export const IMG = { bbb, edEmo, edPlat, sintel1, sintel3, tos1, tos5 };

export type MockState = {
  settings: Settings;
  jobs: Map<number, Job>;
  sniffed: Sniffed[];
  presets: Preset[];
  calls: CallRecord[];
  tasks: Map<number, TaskStatus>;
  libraryCount: number;
  nextId: number;
};
export type MockCtx = {
  state: MockState;
  emit: (ev: CaptaeEvent) => void;
  /** 计时都停住了（截图用） */
  still: boolean;
  img: typeof IMG;
};
// eslint-disable-next-line @typescript-eslint/no-explicit-any
export type MockHandler = (args: Record<string, any>, ctx: MockCtx) => unknown;

const q = new URLSearchParams(location.search);
const STILL = q.has("still");
/** 带 ?clip 时剪贴板提示已经问过一次（一次复制只问一次） */
let clipAsked = false;
const EMPTY = q.get("mock") === "empty";
const TOUR = q.has("tour");
if (STILL) {
  for (const k of Object.keys(localStorage)) if (k.startsWith("captae3:")) localStorage.removeItem(k);
  localStorage.setItem("captae3:migrated", "1");
  localStorage.setItem("captae3:onboarded", "true");
  if (q.get("side") === "off") localStorage.setItem("captae3:sideOff", "true");
}
const T0 = Date.now();
const MIN = 60_000;

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const bare = (u: string) => u.replace(/^https?:\/\/(www\.)?/, "");

/* ---------------- 演示数据 ---------------- */

const SETTINGS: Settings = {
  concurrency: 3,
  threads: 16,
  clipboard: true,
  mcpOn: true,
  adv: false,
  toolsOff: [],
  pickSeconds: 6,
  whisperModel: "small",
  llmBaseUrl: "",
  llmModel: "",
  llmApiKey: "",
  ytdlpPath: "",
  ffmpegPath: "",
  aria2Path: "",
  pythonPath: "",
  ytdlpUseAria2: false,
  saveDir: "",
  browser: "chrome",
  jsRuntimePath: "",
  autoUpdate: true,
  proxyMode: "system",
  proxyUrl: "",
  cookiesFromBrowser: "",
};

/* ---------------- 登录（第二阶段，命令和字段照 src-tauri/src/login.rs） ---------------- */

const SITE_NAME: Record<string, string> = { youtube: "YouTube", bilibili: "哔哩哔哩", douyin: "抖音", x: "X", instagram: "Instagram", tiktok: "TikTok" };
const SITE_OF = (url: string) =>
  /youtu\.?be/.test(url) ? "youtube" : /bilibili|b23\.tv/.test(url) ? "bilibili" : /douyin/.test(url) ? "douyin" : /(^|\/\/|\.)(x|twitter)\.com/.test(url) ? "x" : /instagram/.test(url) ? "instagram" : /tiktok/.test(url) ? "tiktok" : null;
type DemoSite = { site: string; name: string; state: "in" | "verified" | "out"; gain: string; at?: number | null; note?: string };
const demoSites = (): DemoSite[] => [
  { site: "youtube", name: "YouTube", state: "in", gain: "只在年龄限制、会员视频、要确认不是机器人时才带上", at: T0 - 3 * 864e5 },
  { site: "bilibili", name: "哔哩哔哩", state: "in", gain: "可以下 1080P 以上和大会员视频", at: T0 - 864e5 },
  { site: "x", name: "X", state: "out", gain: "受限的帖子要登录才能下", note: "受限的帖子要登录才能下" },
  { site: "instagram", name: "Instagram", state: "out", gain: "大部分帖子要登录才能下", note: "大部分帖子要登录才能下" },
  { site: "douyin", name: "抖音", state: "verified", gain: "提示要新的 cookies 时，在网页里打开一次就行", at: new Date(2026, 10, 22, 21, 10).getTime(), note: "验证过" },
  { site: "tiktok", name: "TikTok", state: "out", gain: "年龄限制、私密视频要登录", note: "年龄限制、私密视频要登录" },
];
function demoBar() {
  switch (q.get("bar")) {
    case "verify":
      return { kind: "verify", site: "douyin", name: "抖音", host: "douyin.com", secure: true, waiting: 1, auto: false };
    case "insecure":
      return { kind: "login", site: "host:video.example.com", name: "video.example.com", host: "video.example.com", secure: false, waiting: 2, auto: false };
    case "other":
      return { kind: "login", site: "host:example.com", name: "example.com", host: "example.com", secure: true, waiting: 0, auto: false };
    default:
      return { kind: "login", site: "youtube", name: "YouTube", host: "accounts.google.com", secure: true, waiting: 1, auto: true };
  }
}

function job(p: Partial<Job> & Pick<Job, "id" | "title" | "url">): Job {
  return {
    src: bare(p.url),
    eng: "yt-dlp",
    fmt: "MP4 · 1080p",
    dur: "",
    audio: false,
    size: 0,
    got: 0,
    speed: 0,
    state: "run",
    phase: "dl",
    pri: p.id,
    boost: false,
    log: [],
    vIdx: 0,
    merge: 0,
    ...p,
  };
}

/** 一个 M3U8 的分片状态：前 done 段下完，接着 live 段正在下，其中第 retry 段在重新下 */
function segs(n: number, done: number, live = 4, retry = -1): SegState[] {
  return Array.from({ length: n }, (_, i) => (i === retry ? 3 : i < done ? 2 : i < done + live ? 1 : 0)) as SegState[];
}

/** &tour：YouTube 上的 Big Buck Bunny 读到以后的几档清晰度（和官网首屏、导览里写的大小一样） */
const BBB_VARS = [
  { r: "3840x2160", p: "2160p", b: 16_000_000, mb: 1240 },
  { r: "1920x1080", p: "1080p", b: 4_100_000, mb: 312 },
  { r: "1280x720", p: "720p", b: 2_000_000, mb: 158 },
];

const TOS_VARS = [
  { r: "3840x1714", p: "2160p", b: 14_000_000, mb: 1400 },
  { r: "1920x856", p: "1080p", b: 4_200_000, mb: 420 },
  { r: "1280x534", p: "720p", b: 2_100_000, mb: 210 },
  { r: "854x356", p: "480p", b: 1_100_000, mb: 112 },
];

function demoJobs(): Job[] {
  const t = T0;
  return [
    job({
      id: 21,
      title: "youtube.com/watch?v=aqz-KE-bpKQ",
      url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
      plat: "youtube",
      phase: "probe",
      fmt: "MP4 · 最清晰",
      pri: 1,
      log: [[t - 2400, "正在读取视频信息", ""]],
    }),
    job({
      id: 22,
      title: "Tears of Steel · 完整版",
      url: "https://mango.blender.org/tos/master.m3u8",
      eng: "M3U8",
      fmt: "M3U8 → MP4",
      dur: "12:14",
      thumb: IMG.tos1,
      phase: "pick",
      vars: TOS_VARS,
      vIdx: 0,
      pickUntil: t + 6000,
      pri: 2,
      log: [
        [t - 3100, "正在读取视频信息", ""],
        [t - 2300, "找到 4 种清晰度", ""],
      ],
    }),
    job({
      id: 23,
      title: "Sintel · 完整版短片",
      url: q.get("bili") === "parts" ? "https://www.bilibili.com/video/BV1qJ411m7Sg?p=2" : "https://www.bilibili.com/video/BV1qJ411m7Sg",
      playlist: q.get("bili") === "parts" ? { id: "BV1qJ411m7Sg", url: "https://www.bilibili.com/video/BV1qJ411m7Sg", count: 5, index: 2 } : undefined,
      plat: "bilibili",
      // 从手机加的（设置 › 手机，mock-remote.ts）：任务行上写「来自 iPhone」
      from: "iPhone",
      dur: "14:48",
      thumb: IMG.sintel1,
      size: 312,
      got: 143.2,
      speed: 6.4,
      pri: 3,
      log: [
        [t - 52_500, "来自 iPhone：https://www.bilibili.com/video/BV1qJ411m7Sg", ""],
        [t - 52_000, "正在读取视频信息", ""],
        [t - 49_000, "选好画质：1080p · 约 312 MB", ""],
        [t - 48_500, "开始下载", ""],
      ],
    }),
    job({
      id: 24,
      title: "Elephants Dream",
      url: "https://cdn.example.org/ed/1080/index.m3u8",
      eng: "M3U8",
      fmt: "M3U8 → MP4",
      dur: "10:54",
      thumb: IMG.edPlat,
      size: 286,
      got: 107,
      speed: 4.8,
      vars: TOS_VARS.slice(1),
      vIdx: 0,
      segs: segs(48, 18, 4, 21),
      pri: 4,
      log: [
        [t - 70_000, "正在读取视频信息", ""],
        [t - 64_000, "用 1080p 下载 · 48 段", ""],
        [t - 9_000, "第 22 段连接断了，正在重新下载", "warn"],
      ],
    }),
    job({
      id: 25,
      title: "Tears of Steel · 预告片",
      url: "https://mango.blender.org/download/tears_of_steel_trailer.mp4",
      eng: "aria2",
      fmt: "MP4 文件",
      dur: "1:04",
      thumb: IMG.tos5,
      size: 96,
      got: 61.5,
      state: "pause",
      pri: 5,
    }),
    job({
      id: 26,
      title: "Sintel · 预告片",
      url: "https://download.blender.org/durian/trailer/sintel_trailer-1080p.mp4",
      eng: "aria2",
      fmt: "MP4 文件",
      dur: "0:52",
      thumb: IMG.sintel3,
      size: 48,
      state: "queue",
      pri: 6,
    }),
    job({
      id: 27,
      title: "youtube.com/watch?v=Qn7mG1pR2xE",
      url: "https://www.youtube.com/watch?v=Qn7mG1pR2xE",
      plat: "youtube",
      state: "failed",
      phase: "probe",
      err: "这个视频有年龄限制，要登录 YouTube 才能看",
      fail: {
        code: "need_login",
        message: "这个视频有年龄限制，要登录 YouTube 才能看",
        raw: "ERROR: [youtube] Qn7mG1pR2xE: Sign in to confirm your age. This video may be inappropriate for some users.",
        action: "login",
        site: "YouTube",
      },
      pri: 7,
      log: [
        [t - 30_000, "正在读取视频信息", ""],
        [t - 27_000, "Sign in to confirm your age", "err"],
      ],
    }),
    job({
      id: 28,
      title: "douyin.com/video/7312094587203624219",
      url: "https://www.douyin.com/video/7312094587203624219",
      plat: "douyin",
      state: "failed",
      phase: "probe",
      err: "抖音要先在网页里打开一次（需要新的 cookies）",
      fail: {
        code: "need_verify",
        message: "抖音要先在网页里打开一次（需要新的 cookies）",
        raw: "ERROR: [Douyin] 7312094587203624219: Fresh cookies (not necessarily logged in) are needed",
        action: "verify",
      },
      pri: 8,
    }),
    job({
      id: 29,
      title: "@BlenderFoundation 的帖子",
      url: "https://x.com/BlenderFoundation/status/1856200118852337664",
      plat: "x",
      thumb: IMG.edEmo,
      state: "pause",
      phase: "probe",
      choose: true,
      entries: [
        { id: "1", n: 1, item: 1, title: "Elephants Dream · 片段 1", dur: 30, thumb: IMG.edEmo, url: "https://x.com/BlenderFoundation/status/1856200118852337664" },
        { id: "2", n: 2, item: 2, title: "Elephants Dream · 片段 2", dur: 78, thumb: IMG.edPlat, url: "https://x.com/BlenderFoundation/status/1856200118852337664" },
      ],
      pri: 9,
    }),
    job({
      id: 30,
      title: "vimeo.com/1084537",
      url: "https://vimeo.com/1084537",
      state: "failed",
      phase: "probe",
      err: "连接被你的网络拦截了，检查代理设置",
      fail: {
        code: "blocked",
        message: "连接被你的网络拦截了，检查代理设置",
        raw: "ERROR: [vimeo] 1084537: Unable to download webpage: [SSL: CERTIFICATE_VERIFY_FAILED] certificate verify failed: Hostname mismatch, certificate is not valid for 'vimeo.com'. (caused by CertificateVerifyFailed)",
        action: "retry",
      },
      pri: 10,
    }),
    ...(q.has("rec") ? recJobs(t) : []),
    ...(q.has("fails") ? failJobs() : []),
    // 已完成
    job({
      id: 11,
      title: "Big Buck Bunny · 60fps 4K",
      url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
      plat: "youtube",
      dur: "10:34",
      thumb: IMG.bbb,
      size: 640,
      got: 640,
      state: "done",
      phase: "done",
      doneAt: t - 2 * MIN,
      path: "/Users/demo/Movies/Captae/Big Buck Bunny 60fps 4K.mp4",
      assetId: 128,
      playlist: { id: "PL6B3937A5D230E335", url: "https://www.youtube.com/playlist?list=PL6B3937A5D230E335" },
    }),
    job({
      id: 12,
      title: "Sintel · 预告片",
      url: "https://download.blender.org/durian/trailer/sintel_trailer-1080p.mp4",
      eng: "aria2",
      fmt: "MP4 文件",
      dur: "0:52",
      thumb: IMG.sintel3,
      size: 48,
      got: 48,
      state: "done",
      phase: "done",
      doneAt: t - 41 * MIN,
      path: "/Users/demo/Movies/Captae/sintel_trailer-1080p.mp4",
      assetId: 127,
    }),
    job({
      id: 13,
      title: "Tears of Steel · 4K 片段",
      url: "https://mango.blender.org/tos/4k/master.m3u8",
      eng: "M3U8",
      fmt: "M3U8 → MP4",
      dur: "3:12",
      thumb: IMG.tos5,
      size: 412,
      got: 412,
      state: "done",
      phase: "done",
      doneAt: t - 3 * 60 * MIN,
      path: "/Users/demo/Movies/Captae/Tears of Steel 4K.mp4",
      assetId: 126,
    }),
    job({
      id: 14,
      title: "Elephants Dream · 原声",
      url: "https://www.youtube.com/watch?v=TLkA0RELQ1g",
      plat: "youtube",
      audio: true,
      fmt: "MP3 · 只要声音",
      dur: "10:54",
      thumb: IMG.edEmo,
      size: 11,
      got: 11,
      state: "done",
      phase: "done",
      doneAt: t - 26 * 60 * MIN,
      path: "/Users/demo/Music/Captae/Elephants Dream.mp3",
      assetId: 125,
    }),
  ];
}

/** 第三阶段的几种失败（&fails）：说法照 errors.rs / ytdlp.rs 的原文 */
function failJobs(): Job[] {
  const f = (id: number, title: string, url: string, fail: NonNullable<Job["fail"]>, extra: Partial<Job> = {}) =>
    job({ id, title, url, state: "failed", phase: "probe", err: fail.message, fail, pri: id, ...extra });
  return [
    f(41, "meipai.com/media/1234567", "https://www.meipai.com/media/1234567", {
      code: "unsupported",
      message: "下载引擎暂时读不了这个网站（可能是网站改版了），可以用浏览器插件在网页里下",
      raw: "ERROR: [Meipai] 1234567: Unable to extract video url",
      action: "use_extension",
    }),
    f(42, "flickr.com/photos/blender/53712345", "https://www.flickr.com/photos/blender/53712345", {
      code: "unsupported",
      message: "这是图片帖，用浏览器插件在网页里下图片",
      raw: "playlist has no entries (https://www.flickr.com/photos/blender/53712345)",
      action: "use_extension",
    }),
    f(43, "我家那闺女2026 第1期", "https://www.mgtv.com/b/899395/24812225.html", {
      code: "need_login",
      message: "这是会员内容，没登录只能下到 5 分钟的试看片段，完整版要登录开了会员的芒果TV 账号",
      raw: "segments add up to 300s, duration says 4849s",
      action: "login",
      // 后端给的网站名字：按钮写「登录芒果TV」，和这句话对得上（收尾修补第 4 项）
      site: "芒果TV",
    }),
    f(44, "svtplay.se/video/jXvZ1nB", "https://www.svtplay.se/video/jXvZ1nB", {
      code: "geo",
      message: "这个视频在你所在的地区看不了",
      raw: "ERROR: [SVTPlay] jXvZ1nB: This video is only available in Sweden",
      action: "retry",
    }),
    f(45, "Blender Conference 2026 · 现场直播", "https://www.youtube.com/watch?v=live-demo2", { code: "live", message: "这是正在进行的直播", raw: "live_status=is_live", action: "record_live" }, { plat: "youtube", thumb: IMG.tos5 }),
  ];
}

/** 录直播（&rec）：一个正在录（YouTube 直播，已录 12:34），一个录完的电台。
 *  &rec=saving 正在保存；&rec=interrupted 电台那条是录制中退出、下次打开才整理好的；&rec=limit 正在录的那条是 AI 助手让录的、录满 10 分钟自动停止 */
function recJobs(t: number): Job[] {
  const mode = q.get("rec");
  const saving = mode === "saving";
  const limit = mode === "limit";
  return [
    job({
      id: 31,
      title: "Blender Conference 2026 · 现场直播",
      url: "https://www.youtube.com/watch?v=live-demo",
      plat: "youtube",
      fmt: "MP4 · 最清晰",
      thumb: IMG.tos1,
      live: true,
      rec: { since: t - (limit ? 254_000 : 754_000), stopping: saving || undefined, until: saving ? t - 2000 : undefined, max: limit ? 600 : undefined },
      phase: saving ? "merge" : "dl",
      got: 186.4,
      speed: saving ? 0 : 0.3,
      pri: 0,
      log: [
        [t - 760_000, "正在读取视频信息", ""],
        [t - 755_000, limit ? "开始录直播，最多录 10 分钟，录满自动停止并保存（也可以提前点「停止并保存」）" : "开始录直播，录到你点「停止并保存」为止", ""],
        ...(saving ? ([[t - 2000, "停止录制，正在保存录到的部分", ""]] as Job["log"]) : []),
      ],
    }),
    job({
      id: 15,
      title: "FIP · 电台直播",
      url: "https://icecast.radiofrance.fr/fip-midfi.mp3",
      eng: "aria2",
      fmt: "MP3",
      audio: true,
      live: true,
      rec: { since: t - 40 * MIN, until: t - 18 * MIN, secs: 1312, interrupted: mode === "interrupted" || undefined },
      size: 21,
      got: 21,
      dur: "21:52",
      state: "done",
      phase: "done",
      doneAt: t - 18 * MIN,
      path: "/Users/demo/Music/Captae/fip-midfi.mp3",
      assetId: 124,
      log: [[t - 18 * MIN, mode === "interrupted" ? "录制被中断（Captae 退出时还在录），录到的 21 分 52 秒已经整理好，保存到 音乐 › Captae" : "录了 21 分 52 秒，已保存到 音乐 › Captae", ""]],
    }),
  ];
}

/** 读到信息以后的样子：按链接猜一个 */
function revealFor(url: string): Partial<Job> & { failWith?: Job["fail"] } {
  const u = url.toLowerCase();
  if (/weixin\.qq\.com|channels\.weixin/.test(u)) return { failWith: { code: "unsupported", message: "微信视频号暂不支持", raw: "ERROR: Unsupported URL: " + url, action: "use_extension" } };
  if (u.endsWith(".m3u8")) return { title: "Tears of Steel", thumb: IMG.tos1, dur: "12:14", eng: "M3U8", fmt: "M3U8 → MP4", vars: TOS_VARS, vIdx: 0, phase: "pick" };
  if (/\.(mp4|mov|webm)$/.test(u)) return { title: decodeURIComponent(u.split("/").pop() ?? "视频"), thumb: IMG.sintel3, dur: "0:52", eng: "aria2", fmt: "MP4 文件", size: 48 };
  if (TOUR && u.includes("youtube")) return { title: "Big Buck Bunny", thumb: IMG.bbb, dur: "10:34", plat: "youtube", fmt: "MP4", vars: BBB_VARS, vIdx: 0, phase: "pick" };
  if (u.includes("bilibili")) return { title: "Sintel · 完整版短片", thumb: IMG.sintel1, dur: "14:48", plat: "bilibili", size: 312 };
  return { title: "Big Buck Bunny · 60fps 4K", thumb: IMG.bbb, dur: "10:34", plat: "youtube", fmt: "MP4 · 2160p", size: 640 };
}

/** ?many 的演示数据：开着十几个网页时，插件找到的多半是图片，还有广告视频、播客 */
function manySniffed(): Sniffed[] {
  const item = (k: number, t: string, m: string, host: string, i: string, page: string): Sniffed => ({
    t,
    m,
    host,
    url: `https://${host}/media/${k}-${t}`,
    i,
    page_url: `https://${host}/`,
    page_title: page,
    variants: [],
    headers: {},
    at: T0 - (14 + k) * MIN,
  });
  return [
    item(1, "hero-banner.jpg", "JPG · 860 KB", "news.example.com", "image", "今日新闻"),
    item(2, "preroll-ad.mp4", "MP4 · 3.1 MB", "ads.example.net", "film", "今日新闻"),
    item(3, "episode-42.mp3", "MP3 · 41 MB", "podcast.example.org", "music", "第 42 期"),
    item(4, "avatar-1.png", "PNG · 24 KB", "forum.example.com", "image", "讨论区"),
    item(5, "avatar-2.png", "PNG · 31 KB", "forum.example.com", "image", "讨论区"),
    item(6, "product-1.webp", "WEBP · 210 KB", "shop.example.com", "image", "新品"),
    item(7, "product-demo.mp4", "MP4 · 18 MB", "shop.example.com", "film", "新品"),
    item(8, "product-2.webp", "WEBP · 190 KB", "shop.example.com", "image", "新品"),
    item(9, "cover.jpg", "JPG · 1.4 MB", "blog.example.com", "image", "旅行日记"),
    item(10, "clip.webm", "WEBM · 6.2 MB", "blog.example.com", "film", "旅行日记"),
    item(11, "theme.m4a", "M4A · 2.8 MB", "music.example.com", "music", "每日一曲"),
    item(12, "map.png", "PNG · 640 KB", "blog.example.com", "image", "旅行日记"),
  ];
}

/* ---------------- 演示后端 ---------------- */

export function createMock() {
  const state: MockState = {
    settings: { ...SETTINGS, cookiesFromBrowser: q.get("browser") ?? "", language: q.get("lang") ?? "" },
    jobs: new Map(EMPTY ? [] : demoJobs().map((j) => [j.id, j])),
    sniffed: EMPTY
      ? []
      : [
          {
            t: "sintel_trailer-1080p.mp4",
            m: "MP4 · 48 MB",
            host: "durian.blender.org",
            url: "https://download.blender.org/durian/trailer/sintel_trailer-1080p.mp4",
            i: "film",
            page_url: "https://durian.blender.org/download/",
            page_title: "Sintel 下载",
            size: 48_000_000,
            variants: [],
            headers: {},
            at: T0 - 5 * MIN,
          },
          {
            t: "master.m3u8",
            m: "M3U8 · 4 种清晰度",
            host: "mango.blender.org",
            url: "https://mango.blender.org/tos/master.m3u8",
            i: "list",
            page_url: "https://mango.blender.org/",
            page_title: "Tears of Steel",
            variants: ["2160p", "1080p", "720p", "480p"],
            headers: {},
            at: T0 - 9 * MIN,
          },
          {
            t: "Sintel_poster.jpg",
            m: "JPG · 1.2 MB",
            host: "durian.blender.org",
            url: "https://durian.blender.org/wp-content/uploads/2010/06/poster.jpg",
            i: "image",
            page_url: "https://durian.blender.org/",
            page_title: "Sintel",
            variants: [],
            headers: {},
            at: T0 - 12 * MIN,
          },
          // ?many：开着很多网页时插件找到一大堆（侧栏只放最近 3 条视频和声音，看它怎么收）
          ...(q.has("many") ? manySniffed() : []),
        ],
    // 预设和调用记录是 AI 助手页的演示数据，写在 mock-agent.ts
    presets: EMPTY ? [] : DEMO_PRESETS.map((p) => ({ ...p, steps: [...p.steps] })),
    calls: demoCalls(),
    tasks: new Map(),
    libraryCount: EMPTY ? 0 : 128,
    nextId: 100,
  };
  const subs = new Set<(ev: CaptaeEvent) => void>();
  // 官网嵌演示版时带 &lang=en 之类：交给界面的数据（返回值、错误、事件）换成这种语言，中文原样（mock-i18n.ts）
  const loc = localizer(q.get("lang"));
  const emit = (ev: CaptaeEvent) =>
    queueMicrotask(() => {
      const e = loc(ev);
      subs.forEach((f) => f(e));
    });
  const ctx: MockCtx = { state, emit, still: STILL, img: IMG };
  // &tour：官网告诉演示版「刚复制了链接」（只认同一个网址下的页面发来的）
  if (TOUR)
    addEventListener("message", (e: MessageEvent) => {
      if (e.origin !== location.origin || e.data?.captaeDemo !== "clipboard" || typeof e.data.url !== "string") return;
      emit({ type: "clipboard", url: e.data.url });
    });
  // 登录窗口：现在开着的（演示后端里没有真窗口，只发事件）
  let loginCur: { site: string; name: string; kind: "login" | "verify"; jobs: number[] } | null = null;
  const loginRows = demoSites();
  const copy = (j: Job): Job => ({ ...j, log: [...j.log], segs: j.segs && [...j.segs] });
  const send = (j: Job) => emit({ type: "job", job: copy(j) });
  const log = (j: Job, text: string, lv: "" | "warn" | "err" = "") => j.log.push([Date.now(), text, lv]);
  const active = () => [...state.jobs.values()].filter((j) => j.state === "run" || j.state === "queue" || j.state === "pause").sort((a, b) => a.pri - b.pri);
  const probeStart = new Map<number, number>([[21, T0 - 2400]]);
  const reveals = new Map<number, ReturnType<typeof revealFor>>([[21, revealFor("https://www.youtube.com/watch?v=aqz-KE-bpKQ")]]);

  /** 排队的任务按优先级补上空位；手动开始的（boost）不占名额 */
  function schedule() {
    const running = active().filter((j) => j.state === "run" && !j.boost).length;
    let free = state.settings.concurrency - running;
    for (const j of active()) {
      if (free <= 0) break;
      if (j.state !== "queue") continue;
      j.state = "run";
      j.phase = j.size ? "dl" : "probe";
      if (j.phase === "probe") probeStart.set(j.id, Date.now());
      log(j, j.phase === "dl" ? "开始下载" : "正在读取视频信息");
      free--;
      send(j);
    }
  }

  function startDl(j: Job) {
    j.phase = "dl";
    if (j.eng === "M3U8" && j.vars?.length) {
      const v = j.vars[j.vIdx] ?? j.vars[0];
      j.size = v.mb;
      j.segs = segs(40, 0, 0);
      j.fmt = `M3U8 → MP4 · ${v.p}`;
      log(j, `用 ${v.p} 下载 · 40 段`);
    } else if (TOUR && j.vars?.length) {
      const v = j.vars[j.vIdx] ?? j.vars[0];
      j.size = v.mb;
      j.fmt = `MP4 · ${v.p}`;
      log(j, "开始下载");
    } else log(j, "开始下载");
    j.speed = 6;
  }

  function finish(j: Job) {
    j.state = "done";
    j.phase = "done";
    j.got = j.size;
    j.speed = 0;
    j.doneAt = Date.now();
    j.assetId = ++state.libraryCount;
    j.path = `/Users/demo/Movies/Captae/${j.title}.mp4`;
    log(j, "已放进素材库");
    send(j);
    emit({ type: "finished", id: j.id, title: j.title, path: j.path, asset_id: j.assetId });
    emit({ type: "library" });
    schedule();
  }

  function tick() {
    const now = Date.now();
    for (const j of state.jobs.values()) {
      if (j.state !== "run") continue;
      if (j.phase === "probe") {
        if (now - (probeStart.get(j.id) ?? now) < (TOUR ? 1400 : 2600)) continue;
        const r = reveals.get(j.id) ?? revealFor(j.url);
        reveals.delete(j.id);
        if (r.failWith) {
          j.state = "failed";
          j.fail = r.failWith;
          j.err = r.failWith.message;
          log(j, r.failWith.raw ?? r.failWith.message, "err");
        } else {
          const { failWith: _f, ...meta } = r;
          Object.assign(j, meta);
          log(j, j.vars?.length ? `找到 ${j.vars.length} 种清晰度` : `读到了：${j.title}`);
          if (meta.phase === "pick") j.pickUntil = now + state.settings.pickSeconds * 1000;
          else startDl(j);
        }
        send(j);
        continue;
      }
      if (j.phase === "pick") {
        if (now >= (j.pickUntil ?? 0)) {
          startDl(j);
          send(j);
        }
        continue;
      }
      if (j.phase === "dl" && j.rec) {
        // 录直播：一直往下录，没有总大小
        j.speed = 0.25 + Math.random() * 0.1;
        j.got += j.speed * 0.5;
        send(j);
        continue;
      }
      if (j.phase === "dl") {
        const base = j.eng === "aria2" ? 9 : j.eng === "M3U8" ? 5 : 6.5;
        j.speed = base * (0.85 + Math.random() * 0.3) * (TOUR ? 10 : 1);
        j.got = Math.min(j.size, j.got + j.speed * 0.5);
        if (j.segs) {
          const n = j.segs.length;
          const done = Math.floor((j.got / j.size) * n);
          j.segs = j.segs.map((s, i) => (i < done ? 2 : i < done + 4 ? (s === 3 ? 3 : 1) : s === 3 ? 3 : 0)) as SegState[];
          // 重新下载的那一段过一会儿也下好
          if (Math.random() < 0.08) j.segs = j.segs.map((s) => (s === 3 ? 2 : s)) as SegState[];
        }
        if (j.got >= j.size) {
          j.phase = j.eng === "M3U8" ? "merge" : j.audio ? "post" : "merge";
          j.merge = 0;
          j.speed = 0;
          log(j, j.eng === "M3U8" ? "下载完了，正在生成视频文件" : "下载完了，正在整理成 MP4");
        }
        send(j);
        continue;
      }
      if (j.phase === "merge" || j.phase === "post") {
        j.merge = Math.min(100, j.merge + (TOUR ? 50 : 22));
        if (j.merge >= 100) finish(j);
        else send(j);
      }
    }
  }
  if (!STILL) setInterval(tick, 500);

  const find = (id: number) => {
    const j = state.jobs.get(id);
    if (!j) throw new Error("任务不存在");
    return j;
  };
  const drop = (id: number) => {
    state.jobs.delete(id);
    emit({ type: "job_removed", id });
    schedule();
  };

  const core: Record<string, MockHandler> = {
    boot: () => ({
      // 演示时的「系统语言」是中文（维护者的中文界面检查脚本照旧能过）；&lang=en 之类相当于在设置里选了这种语言
      systemLang: "zh-CN",
      settings: { ...state.settings },
      jobs: [...state.jobs.values()].map(copy),
      sniffed: state.sniffed,
      presets: state.presets,
      libraryCount: state.libraryCount,
      tools: AGENT_TOOLS,
      calls: state.calls,
      mcpCommand: "/Applications/Captae.app/Contents/MacOS/captae",
    }),
    set_settings: ({ patch }) => {
      state.settings = { ...state.settings, ...patch };
      emit({ type: "settings", settings: { ...state.settings } });
      if ("concurrency" in patch) schedule();
      return { ...state.settings };
    },
    add_job: ({ req }) => {
      const id = state.nextId++;
      // 和真后端（queue.rs）一样：用户自己加的马上开始、排在最前（boost，不占同时下载的名额）；插件嗅探来的排在最后
      const manual = !req.from_sniff;
      const pris = [...state.jobs.values()].map((x) => x.pri);
      const running = active().filter((j) => j.state === "run" && !j.boost).length;
      const fmt = req.audio_only
        ? `${req.format || "MP3"} · 只要声音`
        : req.quality
          ? `${req.format || "MP4"} · ${req.quality}`
          : `${req.format === "保持原格式" ? "原格式" : req.format || "MP4"} · 最清晰`;
      const j = job({
        id,
        title: bare(req.url),
        url: req.url,
        phase: "probe",
        state: manual || running < state.settings.concurrency ? "run" : "queue",
        boost: manual,
        fmt,
        audio: !!req.audio_only,
        pri: manual ? Math.min(0, ...pris) - 1 : Math.max(0, ...pris) + 1,
      });
      if (j.state === "run") {
        probeStart.set(id, Date.now());
        log(j, "正在读取视频信息");
      }
      state.jobs.set(id, j);
      send(j);
      return copy(j);
    },
    job_action: ({ id, action }) => {
      const j = find(id);
      switch (action) {
        case "toggle":
          return core.job_action({ id, action: j.state === "pause" ? "resume" : "pause" }, ctx);
        case "stop":
          // 和后端一样：只对正在录的有效；1.5 秒后整理好、放进素材库
          if (!(j.rec && j.state === "run" && j.phase === "dl" && !j.rec.until)) throw new Error("只有正在录制的直播才能「停止并保存」");
          j.rec = { ...j.rec, stopping: true, until: Date.now() };
          j.phase = "merge";
          j.speed = 0;
          log(j, "停止录制，正在保存录到的部分");
          setTimeout(() => {
            if (!state.jobs.has(j.id)) return;
            const secs = (Date.now() - (j.rec?.since ?? Date.now())) / 1000;
            j.rec = { ...j.rec!, stopping: undefined, secs };
            j.size = j.got;
            j.dur = `${Math.floor(secs / 60)}:${String(Math.floor(secs % 60)).padStart(2, "0")}`;
            finish(j);
          }, STILL ? 0 : 1500);
          break;
        case "pause":
          if (j.rec && j.state === "run") throw new Error("录直播不能暂停：点「停止并保存」把录到的部分放进素材库，或者「取消」");
          if (j.state === "run" || j.state === "queue") {
            j.state = "pause";
            j.speed = 0;
            log(j, "已暂停");
          }
          break;
        case "resume":
          if (j.state === "pause") {
            j.state = "queue";
            log(j, "继续下载");
            send(j);
            schedule();
            return;
          }
          break;
        case "now":
          j.state = "run";
          j.boost = true;
          if (!j.size) {
            j.phase = "probe";
            probeStart.set(j.id, Date.now());
          }
          log(j, "马上开始，不用排队");
          break;
        case "up":
        case "down": {
          const list = active();
          const i = list.findIndex((x) => x.id === id);
          const k = action === "up" ? i - 1 : i + 1;
          if (i < 0 || k < 0 || k >= list.length) return;
          const o = list[k];
          [j.pri, o.pri] = [o.pri, j.pri];
          send(o);
          break;
        }
        case "cancel":
        case "remove":
          drop(id);
          return;
        case "retry":
          j.state = "run";
          j.phase = "probe";
          j.fail = undefined;
          j.err = undefined;
          probeStart.set(j.id, Date.now());
          log(j, "重试：正在读取视频信息");
          break;
        case "segretry":
          if (j.segs) j.segs = j.segs.map((s) => (s === 4 ? 1 : s)) as SegState[];
          break;
        case "pickgo":
          if (j.phase === "pick") startDl(j);
          break;
      }
      send(j);
    },
    set_variant: ({ id, idx }) => {
      const j = find(id);
      j.vIdx = idx;
      send(j);
    },
    pause_all: () => active().forEach((j) => core.job_action({ id: j.id, action: "pause" }, ctx)),
    resume_all: () => active().forEach((j) => core.job_action({ id: j.id, action: "resume" }, ctx)),
    // 播放列表、多视频帖子选好条目以后，每条单独生成一个任务，原来那个去掉（和 main.rs 的 choose_entries 一样）
    choose_entries: ({ id, ids }) => {
      const j = find(id);
      const all = (ids as string[]).includes("all");
      const picked = (j.entries ?? []).filter((e: Entry) => all || (ids as string[]).includes(e.id));
      drop(id);
      return picked.map((e) => core.add_job({ req: { url: e.url || j.url } }, ctx));
    },
    // 第二阶段：登录窗口（和 src-tauri/src/login.rs 一样：开始等 → 3 秒后「登录好了」，重试等着这个网站的任务）
    login_open: ({ jobId, site, url, kind }) => {
      const j = jobId != null ? state.jobs.get(jobId) : undefined;
      const key: string = site ?? SITE_OF(j?.url ?? url ?? "") ?? `host:${bare(j?.url ?? url ?? "example.com").split("/")[0]}`;
      const name = SITE_NAME[key] ?? key.replace(/^host:/, "");
      const k: "login" | "verify" = key === "youtube" ? "login" : kind === "verify" ? "verify" : "login";
      const jobs = [...state.jobs.values()].filter((x) => x.state === "failed" && (x.fail?.action === "login" || x.fail?.action === "verify") && SITE_OF(x.url) === key).map((x) => x.id);
      if (jobId != null && !jobs.includes(jobId)) jobs.unshift(jobId);
      if (loginCur) emit({ type: "login", state: "closed", site: loginCur.site, name: loginCur.name, kind: loginCur.kind, jobs: loginCur.jobs, retried: 0 });
      const cur = { site: key, name, kind: k, jobs };
      loginCur = cur;
      emit({ type: "login", state: "open", ...cur, retried: 0 });
      if (!STILL)
        setTimeout(() => {
          if (loginCur !== cur) return;
          loginCur = null;
          for (const id of jobs) core.job_action({ id, action: "retry" }, ctx);
          const row = loginRows.find((r) => r.site === key);
          if (row) Object.assign(row, { state: k === "verify" ? "verified" : "in", at: Date.now() });
          emit({ type: "login", state: "done", ...cur, retried: jobs.length, how: k });
        }, 3000);
      return { site: key, name, kind: k, waiting: jobs.length, url: url ?? j?.url ?? "" };
    },
    login_close: ({ jobId }) => {
      if (!loginCur || (jobId != null && !loginCur.jobs.includes(jobId))) return;
      emit({ type: "login", state: "closed", ...loginCur, retried: 0 });
      loginCur = null;
    },
    login_sites: () => loginRows.map((r) => ({ ...r })),
    login_logout: ({ site }) => {
      const row = loginRows.find((r) => r.site === site);
      if (row) Object.assign(row, { state: "out", at: null, note: row.gain });
      return { site, removed: 7 };
    },
    login_bar_state: () => ({ ...demoBar(), lang: q.get("lang") || "zh-CN" }),
    login_done: () => undefined,
    login_cancel: () => undefined,
    // 下载引擎和网络（结构照 updater.rs 的 status / Report、net.rs 的 status）
    engine_status: () => ({
      version: "2026.10.26",
      source: "bundled",
      bundled: "2026.10.26",
      autoUpdate: state.settings.autoUpdate,
      checking: false,
      active: null,
      latest: "2026.10.26",
      lastCheck: T0 - 47 * MIN,
      lastResult: "已经是最新的",
      bad: [],
      log: [],
      jsRuntime: { kind: "deno", path: "/Applications/Captae.app/Contents/MacOS/deno", source: "bundled" },
    }),
    engine_update: async () => {
      await sleep(STILL ? 0 : 1200);
      return { result: "latest", message: "已经是最新的", current: "2026.10.26", latest: "2026.10.26", active: null, steps: [] };
    },
    proxy_status: () => {
      const mode = state.settings.proxyMode ?? "system";
      const sysp = { http: "127.0.0.1:6152", https: "127.0.0.1:6152", socks: "127.0.0.1:6153", exceptions: ["127.0.0.1", "localhost", "*.local"], pac: false };
      const use = mode === "none" ? null : mode === "manual" ? state.settings.proxyUrl || null : "http://127.0.0.1:6152";
      return { mode, http: use, https: use, noProxy: ["localhost", ".local"], pac: false, system: sysp, aria2: true };
    },
    take_sniffed: ({ url }) => {
      state.sniffed = state.sniffed.filter((s) => s.url !== url);
    },
    library_count: () => state.libraryCount,
    // mcp_status、preset_check 在 mock-agent.ts（按真实后端的结构）
    deps_status: () => [
      { key: "ytdlp", ok: true, version: "2026.10.26" },
      { key: "ffmpeg", ok: true, version: "8.0.1" },
      { key: "aria2", ok: true, version: "aria2 version 1.37.0" },
      { key: "deno", ok: true, version: "deno 2.9.8 (stable, release, aarch64-apple-darwin)", source: "bundled" },
      // &tour：官网演示里当成语音识别已经装好，鼠标直接点「生成字幕」
      TOUR ? { key: "whisper", ok: true, version: "1.2.0", models: ["small"] } : { key: "whisper", ok: false, models: [] },
    ],
    install_whisper: async () => {
      const id = 900 + state.tasks.size;
      const steps = [
        [6, "正在下载组件", 140],
        [28, "正在下载组件", 96],
        [52, "正在下载「均衡」模型", 60],
        [78, "正在下载「均衡」模型", 24],
        [96, "正在检查", 4],
        [100, "装好了", 0],
      ] as const;
      for (const [pct, note, eta] of steps) {
        const t: TaskStatus = { id, kind: "setup", title: "语音转文字", pct, st: pct < 100 ? "run" : "ok", note, eta };
        state.tasks.set(id, t);
        emit({ type: "task", task: t });
        await sleep(STILL ? 0 : 900);
      }
      return { model: state.settings.whisperModel };
    },
    save_dir_label: () => state.settings.saveDir || "影片 › Captae",
    choose_save_dir: () => {
      state.settings.saveDir = "/Users/demo/Movies/Captae 素材";
      emit({ type: "settings", settings: { ...state.settings } });
      return state.settings.saveDir;
    },
    read_clipboard: () => "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
    // 页面开始听事件以后问一次（真后端：复制后 1 分钟内把 Captae 拿到前面才问）。演示带 ?clip 时当作刚复制过，问一次
    clip_check: () => {
      if (!q.has("clip") || clipAsked) return;
      clipAsked = true;
      setTimeout(() => emit({ type: "clipboard", url: "https://www.youtube.com/watch?v=eRsGyueVLvQ" }), 0);
    },
    set_presets: ({ presets }) => {
      state.presets = presets;
    },
    copy_text: () => undefined,
    reveal: () => undefined,
    open_url: () => undefined,
    open_path: () => undefined,
    js_log: () => undefined,
    window_ctl: () => undefined,
  };

  const tables = [core, libraryHandlers, agentHandlers, extensionHandlers, remoteHandlers];

  // 等界面画出来、下载页订阅了事件再发（截图时也要等一下）
  if (q.has("wait")) setTimeout(() => void core.login_open({ jobId: 27, kind: "login" }, ctx), STILL ? 400 : 1200);
  if (q.has("notice"))
    setTimeout(
      () =>
        emit({
          type: "notice",
          key: "youtube_login",
          title: "这次带上了你的 YouTube 登录状态",
          text: "这个视频要登录才能看。只有遇到年龄限制、会员视频、要确认不是机器人时才带；用账号大量下载，账号可能被 YouTube 限制。",
        }),
      STILL ? 400 : 1200,
    );

  return {
    async invoke(cmd: string, args: Record<string, unknown> = {}) {
      await sleep(30);
      const h = tables.find((t) => cmd in t)?.[cmd];
      if (!h) throw new Error(`演示后端没有这个命令：${cmd}`);
      try {
        return loc(await h(args, ctx));
      } catch (e) {
        // 错误说法也换语言；错误上带的其他字段（错误码之类）照样带着
        throw e instanceof Error ? Object.assign(new Error(loc(e.message)), loc({ ...e })) : loc(e);
      }
    },
    listen(fn: (ev: CaptaeEvent) => void) {
      subs.add(fn);
      return () => {
        subs.delete(fn);
      };
    },
  };
}
