// 下载页的纯逻辑：任务现在显示成哪种样子、步骤时间线、剩余时间、失败时给什么说明和按钮、保存格式选项。
// 界面组件（JobRow、Detail、Composer）只管画，判断都在这里，方便和旧界面（ui/js/page-downloads.js）逐条对照。
import { decimal, lang, N_, t, tAny } from "@/i18n";
import { siteLabel } from "@/lib/detect";
import { dur, home, mb } from "@/lib/format";
import { MOVIES_CAPTAE } from "@/lib/os";
import type { Job, Settings } from "@/lib/types";
import type { Step } from "@/parts";

export type Tab = "run" | "done" | "fail";

/** 「进行中」包括失败的：要登录、要验证的任务得留在用户眼前（设计稿第二版） */
export function inTab(j: Job, tab: Tab): boolean {
  if (tab === "done") return j.state === "done";
  if (tab === "fail") return j.state === "failed";
  return j.state !== "done";
}

/** 一行任务现在显示成哪种样子。也是状态块互换动画的 key */
export type View =
  | "probe" // 正在读取视频信息
  | "pick" // M3U8：选清晰度，倒计时结束自动用最清晰的
  | "dl" // 下载中
  | "rec" // 录直播中（「停止并保存」「取消」）
  | "merge" // 合并分段、转格式
  | "starting" // 点了「马上开始」，后端还没接手
  | "queue"
  | "pause"
  | "choose" // 播放列表、多视频帖子：选要下的条目
  | "login" // 要登录
  | "verify" // 要人机验证
  | "wait_login" // 登录窗口开着，等用户
  | "fail"
  | "done";

export function viewOf(j: Job, waitingLogin: boolean): View {
  switch (j.state) {
    case "done":
      return "done";
    case "failed": {
      const a = j.fail?.action;
      if ((a === "login" || a === "verify") && waitingLogin) return "wait_login";
      return a === "login" ? "login" : a === "verify" ? "verify" : "fail";
    }
    case "pause":
      return j.choose ? "choose" : "pause";
    case "queue":
      return j.boost ? "starting" : "queue";
    default:
      if (j.phase === "probe") return "probe";
      if (j.phase === "pick") return "pick";
      if (j.phase === "merge" || j.phase === "post") return "merge";
      return recording(j) ? "rec" : "dl";
  }
}

/** 正在录直播（录制的进程在跑、还没点停止）：和后端 Job::recording 一样 */
export const recording = (j: Job) => j.state === "run" && j.phase === "dl" && !!j.rec && !j.rec.until && !j.rec.stopping;

/** 录直播设了最多录多久（AI 助手默认 10 分钟、命令行 --max-minutes）：「录满 10 分钟自动停止」；没设的是空的。
 *  桌面端不加这个设置，只是让人看得到这条录制会自己停 */
export function recLimit(j: Job): string {
  const s = j.rec?.max;
  if (!s) return "";
  const m = s / 60;
  // 整小时、整分钟写全称（一句一条，英文按数量分单复数）；不是整分钟、或者几小时几分：和剩余时间同一种写法（dur）
  if (s % 60 || (m >= 60 && m % 60)) return t("录满{time}自动停止", { time: dur(s) });
  return m >= 60 ? t("录满{n}小时自动停止", { n: m / 60 }) : t("录满{n}分钟自动停止", { n: m });
}

/** 录直播进行中或正在保存录到的部分：不能暂停（继续以后中间会缺一段），也不用排先后 */
export const recBusy = (j: Job) => j.state === "run" && !!j.rec && (j.phase === "dl" || j.phase === "merge" || j.phase === "post");

/** 还没有真标题：标题是网址，或者后端读取时先放的占位「youtube.com · 正在读取标题…」（queue.rs） */
// 后端标了 untitled 就是占位；后面几条照顾旧版存下来的任务（queue.json 里没有 untitled）
export const untitled = (j: Job) => !!j.untitled || !j.title || j.title === j.src || j.title === j.url || j.title.endsWith("正在读取标题…");

export const isHls = (j: Job) => j.eng === "M3U8";

export const progress = (j: Job) => (j.size > 0 ? Math.min(1, j.got / j.size) : 0);

/** 剩余时间：26 秒 / 3 分 12 秒；速度为 0 时不显示 */
export function etaOf(j: Job): string {
  if (!j.speed || !j.size) return "";
  return dur((j.size - j.got) / j.speed);
}

/** 进度条过渡时长 = 后端推送进度的间隔，条子才会匀速前进（配方 c5）：yt-dlp 250ms、M3U8 400ms、aria2 1s */
export const tickOf = (j: Job) => (j.eng === "aria2" ? 1000 : isHls(j) ? 400 : 250);

/** 这一步从什么时候开始：后端记的 probeAt；旧版没有，退回找下载记录里最后一条「正在读取视频信息」 */
export function probeSince(j: Job): number | null {
  if (j.probeAt) return j.probeAt;
  for (let i = j.log.length - 1; i >= 0; i--) if (j.log[i][1].includes("正在读取视频信息")) return j.log[i][0];
  return null;
}

/** M3U8 分段：已下载、重新下载中、一直没下载成功 */
export function segCount(j: Job) {
  const s = j.segs ?? [];
  return { all: s.length, done: s.filter((x) => x === 2).length, retry: s.filter((x) => x === 3).length, failed: s.filter((x) => x === 4).length };
}

/** 清晰度说明：最高一档叫「最清晰」，最低一档叫「省流量」 */
export function variantNote(k: number, n: number): string {
  if (n <= 1) return "";
  if (k === 0) return t("最清晰");
  if (k === n - 1) return t("省流量");
  return k === 1 ? t("高清") : t("标清");
}

export const variantName = (j: Job) => (j.vars?.[j.vIdx] ?? j.vars?.[0])?.p ?? "";

/** 保存到哪：已经有文件就用文件所在的文件夹，否则用设置里的，再否则按类型（系统文件夹的叫法，docs/I18N.md 8.3；Windows、Linux 是「视频」） */
export function saveDirOf(j: Job, s: Settings | null): string {
  if (j.path) return home(j.path.replace(/[/\\][^/\\]*$/, ""));
  if (s?.saveDir) return home(s.saveDir);
  return j.audio ? t("音乐 › Captae") : t(MOVIES_CAPTAE);
}

/** 下载下来是什么、最后存成什么：「WebM → MP4」拆成两个，M3U8 一律存成 MP4。
 *  fmt 是后端写的（queue.rs、ytdlp.rs 的 fmt_label）：格式名、「 · 」「 → 」各语言一样，按它们拆照旧能用。
 *  「分片」「 文件」两个判断原来就有，现在的后端不会在 fmt 里写这两个词，留着不影响 */
export function formatPair(j: Job): { from: string | null; to: string } {
  if (isHls(j)) return { from: "M3U8", to: "MP4" };
  const f = j.fmt.split(" · ")[0];
  if (f.includes("→")) {
    const [a, b] = f.split("→").map((x) => x.trim());
    return { from: /分片|M3U8/.test(a) ? "M3U8" : a, to: b };
  }
  return { from: null, to: f.replace(/ 文件$/, "") || "MP4" };
}

/* ---------------- 步骤时间线 ---------------- */

type Flow = { names: string[]; subs: (string | undefined)[]; at: Partial<Record<Job["phase"], number>> };

function flowOf(j: Job, s: Settings | null): Flow {
  const dir = saveDirOf(j, s);
  // 网站名是存下来的原文（「哔哩哔哩」），显示时翻译
  const site = tAny(siteLabel(j.plat, j.url));
  // 录直播：没有总大小，录到点了「停止并保存」为止
  if (j.live)
    return {
      names: [t("读取直播信息"), t("录制§步骤名"), t("整理成能播放的文件"), t("放进素材库")],
      subs: [isHls(j) ? j.src.split("/")[0] : site, undefined, undefined, dir],
      at: { probe: 0, pick: 0, dl: 1, merge: 2, post: 2 },
    };
  if (isHls(j))
    return {
      names: [t("读取视频信息"), t("选择清晰度"), t("下载"), t("生成视频文件"), t("放进素材库")],
      subs: [j.src.split("/")[0], variantName(j) || undefined, undefined, t("存成一个 MP4"), dir],
      at: { probe: 0, pick: 1, dl: 2, merge: 3, post: 3 },
    };
  if (j.eng === "yt-dlp")
    return {
      names: [t("读取视频信息"), t("选择画质"), j.audio ? t("下载声音") : t("下载视频"), j.audio ? t("转成{fmt}", { fmt: formatPair(j).to }) : t("整理成视频文件"), t("放进素材库")],
      subs: [site, untitled(j) ? undefined : j.fmt, undefined, undefined, dir],
      at: { probe: 0, pick: 1, dl: 2, merge: 3, post: 3 },
    };
  return { names: [t("连接服务器"), t("下载"), t("检查文件是否完整"), t("放进素材库")], subs: [j.src.split("/")[0], t("暂停后可以继续"), undefined, dir], at: { probe: 0, pick: 0, dl: 1, merge: 2, post: 2 } };
}

/** 当前停在第几步（0 开始）；已完成时等于步骤数 */
export function stepIndex(j: Job, s: Settings | null): number {
  const f = flowOf(j, s);
  return j.state === "done" ? f.names.length : (f.at[j.phase] ?? 0);
}

/** fresh：刚刚完成、要画一次勾的那几步 [从, 到)（只在详情里一步步往前走时给） */
export function stepsOf(j: Job, s: Settings | null, fresh: [number, number] | null): Step[] {
  const f = flowOf(j, s);
  const cur = stepIndex(j, s);
  return f.names.map((label, i) => {
    let sub = f.subs[i];
    let st: Step["s"] = i < cur ? "done" : i === cur ? "now" : "todo";
    if (i === cur) {
      if (j.state === "failed") {
        st = "err";
        sub = j.fail?.message || j.err || t("没有成功");
      } else if (j.state === "queue") sub = j.boost ? t("正在开始…") : t("排队中 · 最多同时下载{n}个", { n: s?.concurrency ?? 3 });
      else if (j.state === "pause") sub = j.choose ? t("选好要下载的视频后继续") : t("已暂停 · 已下载的{size}会保留", { size: mb(j.got) });
      else if (j.phase === "probe") sub = t("正在打开网页");
      else if (j.phase === "pick") sub = t("选一个清晰度，倒计时结束自动用{quality}", { quality: variantName(j) });
      else if (j.live && (j.phase === "merge" || j.phase === "post")) sub = j.rec?.interrupted ? t("上次退出时还在录，正在整理录到的部分") : t("正在保存录到的部分");
      else if (j.live && j.phase === "dl") sub = t("正在录制 · 已录{size}", { size: mb(j.got) });
      else if (j.phase === "merge" || j.phase === "post") sub = `${Math.round(j.merge)}%`;
      else if (j.phase === "dl") sub = j.size ? `${Math.floor(progress(j) * 100)}% · ${decimal(j.speed, 1)} MB/s` : `${mb(j.got)} · ${decimal(j.speed, 1)} MB/s`;
    }
    return { label, sub, s: st, fresh: !!fresh && i >= fresh[0] && i < fresh[1] };
  });
}

/* ---------------- 失败：说明和按钮 ---------------- */

export type FailAction = "login" | "verify" | "retry" | "open_page" | "record_live" | "change_dir" | "settings" | null;

/** 后端 errors.rs 给的 action → 界面上的主按钮；第二行小字补充后端那句话没说到的 */
export function failInfo(j: Job): { title: string; hint?: string; action: FailAction; label?: string } {
  const f = j.fail;
  const title = f?.message || j.err || t("下载失败");
  // 网站名字和上面那句说法用同一个（后端给的 fail.site）：「登录芒果TV」，不是「登录 mgtv.com」。
  // site 是存下来的原文（判断是不是 YouTube 用它），按钮上显示翻译过的 name
  const site = siteLabel(j.plat, j.url, f?.site);
  const name = tAny(site);
  switch (f?.action) {
    case "login":
      return {
        title,
        // YouTube 默认不带登录状态，只在年龄限制、会员、机器人检测时才带（计划 3.4）
        hint: site === "YouTube" ? t("登录一次，以后遇到年龄限制、会员视频会自动带上") : t("在 Captae 里登录一次，以后下这个网站会自动带上"),
        action: "login",
        label: t("登录{site}", { site: name }),
      };
    case "verify":
      // YouTube 的「确认不是机器人」要登录账号才能过，登录窗口打开的是登录页（src-tauri/src/login.rs）
      if (site === "YouTube") return { title, hint: t("YouTube 要登录账号才能确认，登录好会自动接着下"), action: "verify", label: t("登录{site}", { site: "YouTube" }) };
      return { title, hint: t("在弹出的网页里完成验证，点「完成」以后接着下载"), action: "verify", label: t("去验证") };
    case "none":
      return { title, hint: f.code === "drm" ? t("受版权保护的视频，所有下载工具都下不了") : undefined, action: null };
    case "use_extension":
      // 后端那句话已经说了「用浏览器插件在网页里下」，这里补怎么做；图片帖（errors.rs 的 IMAGE_POST）说图片
      return {
        title,
        hint: f.reason === "image_post" || /图片帖/.test(title) ? t("打开网页以后，点工具栏上的 Captae 图标，选要下的图片") : t("打开网页以后先点播放，再点工具栏上的 Captae 图标，选要下的视频"),
        action: "open_page",
        label: t("在浏览器里打开"),
      };
    case "wait_live":
      return {
        title,
        hint: f.startsAt ? t("直播在{time}开始，开始以后再下载", { time: startsAt(f.startsAt) }) : t("直播开始以后再下载"),
        action: "retry",
        label: t("重试"),
      };
    case "record_live":
      // 电台流不会自己结束（errors 里说「会一直播下去」）：录够了点「停止并保存」，录到的部分整理好放进素材库
      return {
        title,
        hint: f.reason === "radio" || /电台/.test(title) ? t("可以从现在开始录。电台不会自己停，录够了点「停止并保存」") : t("可以从现在开始录，录够了点「停止并保存」，直播结束时也会自己停"),
        action: "record_live",
        label: t("从现在开始录"),
      };
    case "auto_retry": {
      // 后端在 retryAt 那一刻自己重试（rate_limited）
      const min = j.retryAt ? Math.max(1, Math.ceil((j.retryAt - Date.now()) / 60_000)) : null;
      return { title, hint: min ? t("约{n}分钟后自动再试，也可以现在重试", { n: min }) : t("过几分钟会自动再试，也可以现在重试"), action: "retry", label: t("重试") };
    }
    case "change_dir":
      return { title, hint: t("换一个保存位置，或者清出一些空间"), action: "change_dir", label: t("换保存位置") };
    case "repair":
      return { title, hint: t("下载组件出了问题，到设置里检查一下"), action: "settings", label: t("打开设置") };
    case "retry":
      return { title, hint: f.code === "geo" ? t("换个网络或者打开代理再试") : f.code === "network" ? t("网络恢复以后重试，会从停下的地方继续") : undefined, action: "retry", label: t("重试") };
    default:
      return { title, hint: t("可以复制错误信息，再试一次"), action: "retry", label: t("重试") };
  }
}

/** 直播开始的时刻：中文照原来的「11/24 20:00」，其他语言「Nov 24, 20:00」；一律 24 小时制 */
function startsAt(ms: number): string {
  const zh = lang() === "zh-CN";
  return new Date(ms).toLocaleString(lang(), { month: zh ? "numeric" : "short", day: "numeric", hour: "2-digit", minute: "2-digit", hourCycle: "h23" });
}

/** 复制给用户（或者贴给我们）的错误信息：一句话 + 原文 + 链接 */
export const errorText = (j: Job) => [j.fail?.message || j.err || t("下载失败"), j.fail?.raw, j.url].filter(Boolean).join("\n");

/** 下载记录，按时间顺序，复制用 */
export const logText = (j: Job) => j.log.map(([t, x]) => `${new Date(t).toTimeString().slice(0, 8)}  ${x}`).join("\n");

/* ---------------- 保存格式（输入框右边的「保存成」） ---------------- */

export type FmtChoice = "best" | "1080p" | "720p" | "mp3" | "webm" | "mkv" | "keep" | "aac" | "flac";
type FmtReq = { quality: string | null; format: string; audio_only: boolean };
/** label 是原文（带中文的用 N_ 标出来），显示时 fmtLabel(k)。req.format 的「保持原格式」是协议值，不翻译 */
export const FMT: Record<FmtChoice, { label: string; req: FmtReq }> = {
  best: { label: N_("MP4 · 最清晰"), req: { quality: null, format: "MP4", audio_only: false } },
  "1080p": { label: "MP4 · 1080p", req: { quality: "1080p", format: "MP4", audio_only: false } },
  "720p": { label: "MP4 · 720p", req: { quality: "720p", format: "MP4", audio_only: false } },
  mp3: { label: N_("只要声音 · MP3"), req: { quality: null, format: "MP3", audio_only: true } },
  webm: { label: N_("WebM · 最清晰"), req: { quality: null, format: "WebM", audio_only: false } },
  mkv: { label: N_("MKV · 最清晰"), req: { quality: null, format: "MKV", audio_only: false } },
  keep: { label: N_("保持原来的格式"), req: { quality: null, format: "保持原格式", audio_only: false } },
  aac: { label: N_("只要声音 · AAC"), req: { quality: null, format: "AAC", audio_only: true } },
  flac: { label: N_("只要声音 · FLAC"), req: { quality: null, format: "FLAC", audio_only: true } },
};
/** 保存格式的名字（按当前语言）；「MP4 · 1080p」这种没有要翻的，原样 */
export const fmtLabel = (k: FmtChoice) => tAny(FMT[k].label);
export const MAIN_FMTS: FmtChoice[] = ["best", "1080p", "720p", "mp3"];
export const MORE_FMTS: FmtChoice[] = ["webm", "mkv", "keep", "aac", "flac"];
