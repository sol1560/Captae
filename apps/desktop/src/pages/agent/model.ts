// AI 助手页的纯逻辑：后端返回的结构、路径缩短、连接状态的说法、调用记录的说明文字、预设步骤。
// 界面组件只管画，判断都在这里，方便和旧界面（ui/js/page-agent.js）逐条对照。
import {
  AiBrain01Icon,
  ArrowDataTransferHorizontalIcon,
  Download04Icon,
  Folder01Icon,
  Layers01Icon,
  PlayListIcon,
  EraserIcon,
  ScissorIcon,
  Search01Icon,
  SparklesIcon,
  SubtitleIcon,
  Tag01Icon,
  VolumeOffIcon,
} from "@hugeicons/core-free-icons";
import { decimal, joinList, lang, N_, t, tAny, type Key } from "@/i18n";
import { siteLabel } from "@/lib/detect";
import { isMac } from "@/lib/os";
import { dur, whenLabel } from "@/lib/format";
import type { CallRecord } from "@/lib/types";
import type { HugeIcon } from "@/parts";

/* ---------------- 后端返回的结构（serde 的 rename_all 已经对过） ---------------- */

/** mcp_status 里的一个 AI 助手（core mcp.rs ClientStatus，camelCase） */
export type Client = {
  name: string;
  version?: string;
  /** 最近一次有动静（心跳或调用），毫秒 */
  lastSeen?: number;
  lastCall?: number | null;
  /** 最近一次调用的工具，界面上的名字（captae.download） */
  lastTool?: string | null;
  calls?: number;
  connected: boolean;
};

/** agent_info：用户目录、三个保存位置、两个客户端的配置文件、MCP 命令、Skill 全文 */
export type AgentInfo = {
  home?: string;
  dirs?: Partial<Record<"movies" | "music" | "pictures" | "shares", string>>;
  configs?: Partial<Record<"claude" | "cursor", string>>;
  mcpCommand?: string;
  skill?: string;
};

/** mcp_config_plan（core mcp.rs ConfigPlan） */
export type ConfigPlan = { path: string; exists: boolean; snippet: string; previous: unknown; unchanged: boolean; others: string[] };

/** mcp_config_write（core mcp.rs ConfigWritten） */
export type ConfigWritten = { path: string; backup: string | null; unchanged: boolean };

/** 试运行时一步的结果（presets.rs StepResult），从 preset_step 事件来 */
export type StepResult = { index: number; step: string; status: "run" | "ok" | "skip" | "err"; detail: string; output?: string };

/** library_query 里的一个素材（试运行选文件只用到这几个字段） */
export type AssetLite = { id: number; title: string; type: string; fmt?: string; dur?: string; when?: string; exists: boolean };

/** 调用失败时后端存的结果（agent.rs error_json）：第一阶段以后下载类的失败另带 code / raw / action / next */
export type CallError = { error?: string; message?: string; code?: string; raw?: string; action?: string; next?: string; startsAt?: number };

/* ---------------- 路径 ---------------- */

/** 三个保存位置所在的文件夹，名字照系统的叫法（docs/I18N.md 8.3）：macOS「影片」，Windows、Linux「视频」 */
const SAVE_DIRS: ["movies" | "music" | "pictures", Key][] = isMac
  ? [
      ["movies", N_("影片§macOS 的文件夹")],
      ["music", N_("音乐§macOS 的文件夹")],
      ["pictures", N_("图片§macOS 的文件夹")],
    ]
  : [
      ["movies", N_("视频§Windows、Linux 的用户文件夹")],
      ["music", N_("音乐§Windows、Linux 的用户文件夹")],
      ["pictures", N_("图片§Windows、Linux 的用户文件夹")],
    ];
/** 路径分隔符：Windows 是反斜杠 */
const SEP = /[/\\]/;

/** Captae 的三个保存位置说成「影片 › Captae › …」，用户目录下的写成 ~/…，其他原样 */
export function shortPath(p: unknown, info: AgentInfo | null): string {
  const s = String(p ?? "");
  if (!s) return "";
  for (const [k, label] of SAVE_DIRS) {
    const d = info?.dirs?.[k];
    if (d && (s === d || s.startsWith(d + "/") || s.startsWith(d + "\\"))) {
      const rest = s.slice(d.length + 1);
      return [t(label), d.split(SEP).pop(), ...(rest ? rest.split(SEP) : [])].join(" › ");
    }
  }
  const h = info?.home;
  if (h && (s === h || s.startsWith(h + "/") || s.startsWith(h + "\\"))) return "~" + s.slice(h.length);
  return s.replace(/^\/Users\/[^/]+/, "~").replace(/^\/home\/[^/]+/, "~").replace(/^[A-Za-z]:\\Users\\[^\\]+/, "~");
}

const looksPath = (v: unknown, info: AgentInfo | null) => typeof v === "string" && (/^(\/|~[/\\]|[A-Za-z]:\\)/.test(v) || (!!info?.home && v.startsWith(info.home)));

/* ---------------- 连接状态 ---------------- */
// 「3 分钟前用过」「今天 09:47 用过」：时间（lib/format 的 whenLabel）放进整句模板，数字结尾时的空格由自动空格补上

/** AI 助手的名字：客户端没报名字时后端存的是原文「AI 助手」（mcp.rs），按界面语言显示；别的名字原样 */
/** 调用记录里的来源：两个是 Captae 自己起的名字（存的是中文原文），按界面语言显示；别的是 AI 助手报的名字，原样 */
export const clientName = (n: string) => (n === "AI 助手" ? t("AI 助手") : n === "命令行" ? t("命令行") : n);

/** 顶栏那一句：开没开、谁连着、最近谁用过 */
export function mcpLine(on: boolean, clients: Client[] | null): { text: string; live: boolean } {
  if (!on) return { text: t("已关闭，AI 助手现在用不了 Captae"), live: false };
  if (!clients) return { text: t("已开启 · 只在这台电脑上"), live: true };
  const live = clients.filter((c) => c.connected);
  if (live.length) return { text: t("已连接 · {names}", { names: joinList(live.map((c) => clientName(c.name))) }), live: true };
  const past = clients.find((c) => c.lastCall);
  if (past) return { text: t("已开启 · 现在没有 AI 助手连着 · {name} {when}用过", { name: clientName(past.name), when: whenLabel(past.lastCall) }), live: false };
  return { text: t("已开启 · 还没有 AI 助手连上过"), live: false };
}

/** 一个 AI 助手那一行的第二句：版本、最近一次用了什么、一共调用了几次 */
export function clientNote(c: Client, label: (tool: string) => string): string {
  const parts: string[] = [];
  if (c.version) parts.push(t("版本 {version}", { version: c.version }));
  if (c.lastCall) parts.push(c.lastTool ? t("{when}用过「{tool}」", { when: whenLabel(c.lastCall), tool: label(c.lastTool) }) : t("{when}用过", { when: whenLabel(c.lastCall) }));
  else if (c.lastSeen) parts.push(t("{when}连上过，还没用过", { when: whenLabel(c.lastSeen) }));
  if (c.calls) parts.push(t("一共 {n} 次", { n: c.calls }));
  return parts.join(" · ");
}

/* ---------------- 连接你的 AI 助手 ---------------- */

export type Target = "claude" | "cursor" | "code" | "skill";
export const TARGET_NAME: Record<Target, string> = { claude: "Claude Desktop", cursor: "Cursor", code: "Claude Code", skill: N_("其他 AI 助手") };
/** 显示用的名字：产品名原样，「其他 AI 助手」按界面语言 */
export const targetName = (x: Target): string => tAny(TARGET_NAME[x]);

/** 只装了几个短字符串的数组写成一行（"args": ["mcp", "--stdio"]），配置短一截、好读；内容不变 */
export const tidyJson = (s: string) => s.replace(/\[\n\s+((?:"[^"\n]*",\n\s+)*"[^"\n]*")\n\s+\]/g, (_m, inner: string) => "[" + inner.split(/,\n\s+/).join(", ") + "]");

/** 配置文件里要有的那一段（和 core mcp.rs server_entry 一样） */
export const configJson = (cmd: string) => tidyJson(JSON.stringify({ mcpServers: { captae: { command: cmd, args: ["mcp", "--stdio"] } } }, null, 2));

/** 命令行里的一个参数：有空格或特殊字符就加单引号 */
export const shq = (s: string) => (/^[\w@%+=:,./-]+$/.test(s) ? s : `'${s.replace(/'/g, `'\\''`)}'`);

export const claudeCodeLine = (cmd: string) => `claude mcp add captae -- ${shq(cmd)} mcp --stdio`;

/* ---------------- 工具 ---------------- */

/** captae.library.add_tag / library_add_tag / 旧版的 captae.library.add.tag 都归到 library_add_tag */
export const normTool = (n: string) => String(n).replace(/^captae[._]/, "").replace(/\./g, "_");

const TOOL_ICON: Record<string, HugeIcon> = {
  download: Download04Icon,
  convert: ArrowDataTransferHorizontalIcon,
  subtitle: SubtitleIcon,
  trim: ScissorIcon,
  concat: Layers01Icon,
  library_query: Search01Icon,
  library_add_tag: Tag01Icon,
  preset_run: SparklesIcon,
  preset_list: PlayListIcon,
  share_video: AiBrain01Icon,
};
export const toolIcon = (n: string): HugeIcon => TOOL_ICON[normTool(n)] ?? SparklesIcon;

/** 启动快照 tools 里的 label 是后端写的中文原文（core agent.rs 的 label，后端不翻），这里标出来进词表（一字不差地抄） */
export const TOOL_LABELS = [
  N_("下载视频、音乐或图片"),
  N_("转换文件格式"),
  N_("下载字幕或自动生成字幕"),
  N_("截取视频片段"),
  N_("把几段视频拼成一个"),
  N_("在素材库里找东西"),
  N_("给素材加标签"),
  N_("使用你的预设"),
  N_("查看有哪些预设"),
  N_("把视频整理成文字稿和关键帧"),
];
/** 工具在界面上的名字：有译文用译文，后端新加了还没进词表的原样显示 */
export const toolLabel = (label: string) => tAny(label);

/* ---------------- 调用记录 ---------------- */

/** 文字稿的来源（share.rs 的 transcript_source）：嵌在「文字稿来自…」里，和别处单独用的「网站字幕」分开 */
const SRC: Record<string, Key> = { platform: N_("网站字幕§文字稿的来源"), "faster-whisper": N_("本地识别§文字稿的来源") };

/** 参数里给人看的部分：素材 #12 · 1080p · 影片 › Captae › …；布尔值、对象和空值不显示 */
export function argText(args: unknown, info: AgentInfo | null): string {
  if (!args || typeof args !== "object") return "";
  return Object.entries(args as Record<string, unknown>)
    .filter(([k, v]) => k !== "wait" && v !== false && v !== true && v != null && v !== "" && (typeof v !== "object" || Array.isArray(v)))
    .map(([k, v]) => {
      if (k === "asset_id") return t("素材 #{id}", { id: String(v) });
      if (Array.isArray(v)) return v.map((x) => (looksPath(x, info) ? shortPath(x, info) : String(x))).join(" ");
      return looksPath(v, info) ? shortPath(v, info) : String(v);
    })
    .join(" · ");
}

export const callError = (c: CallRecord): CallError => (c.st === "err" && c.res && typeof c.res === "object" ? (c.res as CallError) : {});

/** 失败的调用：一句话（后端的 message）。旧记录只有 error 代码时给个兜底 */
export function errLine(c: CallRecord): string {
  const e = callError(c);
  if (e.message) return e.message;
  if (e.error === "ACCESS_OFF") return t("当时 Captae 关掉了 AI 助手访问");
  return t("没有完成");
}

/** 失败的调用补一句：给用户的下一步。后端的 next 是写给 AI 看的，这里只给几种常见的人话 */
export function errHint(c: CallRecord): string {
  const e = callError(c);
  switch (e.error) {
    case "ADVANCED_MODE_OFF":
      return t("这些参数要打开下面的「高级模式」才能用");
    case "TOOL_OFF":
      return t("这个功能在「AI 可以用的功能」里关掉了");
    case "ACCESS_OFF":
      return t("打开上面的「允许 AI 助手使用 Captae」，再让 AI 试一次");
    case "OUTPUT_NOT_WRITABLE":
      return t("连上移动硬盘或者换个位置之后，可以让 AI 再试一次");
    case "OUTPUT_EXISTS":
      return t("同名文件已经在了，普通模式下不会覆盖");
  }
  switch (e.code) {
    case "need_login":
      return t("在 Captae 里登录这个网站以后，让 AI 再试一次");
    case "need_verify":
      return t("在 Captae 打开的网页里点一下验证，再让 AI 试一次");
    case "drm":
      return t("受版权保护的视频，所有下载工具都下不了");
    case "unsupported":
      return t("可以在浏览器里打开这个网页，用 Captae 插件找到视频再下载");
    case "rate_limited":
      return t("过几分钟再让 AI 试");
    case "network":
    case "blocked":
    case "geo":
      return t("检查一下网络或代理，再让 AI 试一次");
  }
  return "";
}

/** 要登录、要验证的失败：按钮的说法和要打开的网址，和下载页任务行一样（「登录哔哩哔哩」「登录 YouTube」「去验证」，
 *  中文名前后不加空格）。网址取 AI 给的参数里的 url（下载、预设、发给 AI 才有），没有网址的不给按钮 */
export function loginAct(c: CallRecord): { kind: "login" | "verify"; label: string; url: string } | null {
  const e = callError(c);
  const kind = e.code === "need_login" || e.action === "login" ? "login" : e.code === "need_verify" || e.action === "verify" ? "verify" : null;
  if (!kind) return null;
  const url = c.args && typeof c.args === "object" ? (c.args as { url?: unknown }).url : null;
  if (typeof url !== "string" || !/^https?:\/\//i.test(url)) return null;
  const site = siteLabel(null, url);
  // 站点名可能是存下来的中文原文（「哔哩哔哩」），按界面语言显示；中文名前后的空格由模板的自动空格决定
  if (kind === "login") return { kind, label: t("登录{site}", { site: tAny(site) }), url };
  // YouTube 的「确认不是机器人」要登录账号才能过，登录窗口打开的是登录页（src-tauri/src/login.rs）
  if (site === "YouTube") return { kind: "login", label: t("登录{site}", { site }), url };
  return { kind, label: t("去验证"), url };
}

/** 失败的原文：复制给我们看的就是它 */
export function errRaw(c: CallRecord): string {
  const e = callError(c);
  return e.raw?.trim() || "";
}

/** 成功或者还在进行中的调用：结果说成一两句话（多行用 \n） */
export function resText(c: CallRecord, info: AgentInfo | null): string {
  if (c.st === "run") return t("还在进行中…");
  const r = (c.res ?? null) as Record<string, unknown> | null;
  if (!r || typeof r !== "object") return t("已完成");
  if (r.bundle_dir) {
    const n = Array.isArray(r.frames) ? r.frames.length : 0;
    const from = r.transcript_source ? SRC[String(r.transcript_source)] : undefined;
    const second = r.transcript_source
      ? t("{n} 张关键帧 · 文字稿来自{source}", { n, source: from ? t(from) : String(r.transcript_source) })
      : t("{n} 张关键帧 · 没有文字稿", { n });
    return t("已整理到 {path}", { path: shortPath(r.bundle_dir, info) }) + "\n" + second;
  }
  if (r.job_id != null) return r.title ? t("已加入下载队列：{title}", { title: String(r.title) }) : t("已加入下载队列");
  const out = r.path ?? r.output;
  if (out) {
    const lines = [t("已保存到 {path}", { path: shortPath(out, info) })];
    if (r.burned_path) lines.push(t("烧录字幕的视频：{path}", { path: shortPath(r.burned_path, info) }));
    if (Array.isArray(r.outputs) && r.outputs.length > 1) lines.push(t("一共生成 {n} 个文件", { n: r.outputs.length }));
    return lines.join("\n");
  }
  if (r.count != null) {
    const items = Array.isArray(r.items) ? r.items.map((i) => (typeof i === "string" ? i : String((i as { title?: string }).title ?? ""))).filter(Boolean) : [];
    const n = typeof r.count === "number" ? r.count : String(r.count);
    return items.length ? t("找到 {n} 个：{items}", { n, items: joinList(items) }) : t("找到 {n} 个", { n });
  }
  if (Array.isArray(r.presets))
    return r.presets.length ? t("有 {n} 个预设：{names}", { n: r.presets.length, names: joinList(r.presets.map((p) => (p as { name: string }).name)) }) : t("没有允许 AI 使用的预设");
  if (Array.isArray(r.tags)) return t("现在的标签：{tags}", { tags: joinList(r.tags.map(String)) });
  return t("已完成");
}

/** 成功的调用产出的素材和文件（展开后的「在素材库里看」「在访达中显示」） */
export function callOutputs(c: CallRecord): { asset: number | null; file: string | null } {
  if (c.st !== "ok" || !c.res || typeof c.res !== "object") return { asset: null, file: null };
  const r = c.res as Record<string, unknown>;
  const a = r.asset_id ?? r.burned_asset_id;
  const f = r.bundle_dir ?? r.path ?? r.output;
  return { asset: typeof a === "number" ? a : null, file: typeof f === "string" ? f : null };
}

/** 调用用了多久：820 ms / 1.2 s / 48 s；一分钟以上用 lib/format 的 dur（3 分 12 秒） */
export function msText(ms?: number | null): string {
  if (ms == null) return "";
  if (ms < 1000) return `${Math.max(0, Math.round(ms))} ms`;
  if (ms < 60_000) return `${decimal(ms / 1000, ms < 10_000 ? 1 : 0)} s`;
  return dur(ms / 1000);
}

/** 调用的时间：今天的写 14:03:21，别的天写 11月22日 22:15。中文照原来的写法，其他语言用 Intl（24 小时制） */
export function callTime(at: number): string {
  const d = new Date(at);
  const hms = d.toTimeString().slice(0, 8);
  if (d.toDateString() === new Date().toDateString()) return hms;
  if (lang() === "zh-CN") return `${d.getMonth() + 1}月${d.getDate()}日 ${hms.slice(0, 5)}`;
  return new Intl.DateTimeFormat(lang(), { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit", hourCycle: "h23" }).format(d);
}

/** 几句话连成一段：中文、日文句号后面不空格，其他语言空一格 */
export const joinSentences = (xs: string[]) => xs.join(lang() === "zh-CN" || lang() === "zh-TW" || lang() === "ja" ? "" : " ");

/* ---------------- 预设 ---------------- */
// 预设的步骤存的是中文原文（core presets.rs 按中文解析，旧版存下来的也要能读），存的值不能变；
// 界面上显示用 stepLabel 换成界面语言。下面的 stepIcon、pickDir、isDownload、isMove 照旧按原文判断。

/** 步骤编辑器里的选项（和 core presets.rs STEP_OPTIONS 一致）。值是存下来的原文，显示用 stepLabel */
export const STEP_OPTS: string[] = [
  N_("下载最高画质"),
  N_("下载 1080p"),
  N_("只下载声音（MP3）"),
  N_("去除水印"),
  N_("转为 MP4"),
  N_("转为 ProRes 422"),
  N_("转为 WebM"),
  N_("下载网站字幕"),
  N_("语音转文字"),
  N_("只留前 30 秒"),
  N_("去掉声音"),
  N_("移动到指定文件夹"),
];

/** 默认预设里另外几步（core presets.rs defaults）：也是完整的步骤，按原文查 */
const MORE_STEPS: string[] = [N_("导出文字稿"), N_("保存到「文稿」文件夹"), N_("移动到 Final Cut 素材文件夹")];
const WHOLE_STEPS = new Set([...STEP_OPTS, ...MORE_STEPS]);

/** 「移动到…」「保存到…」后面认得的文件夹；别的（用户自己写的文件夹名）原样 */
const STEP_DIRS: Record<string, Key> = {
  "「文稿」文件夹": N_("「文稿」文件夹"),
  "Final Cut 素材文件夹": N_("Final Cut 素材文件夹"),
  指定文件夹: N_("指定文件夹"),
};

/** 一步在界面上怎么说。完整的步骤按原文查；带参数的（下载 720p、转为 MOV、只留前 2 分钟、移动到 Projects）按样子套模板，
 *  样子照 presets.rs 的 Step::parse；认不出的原样显示。中文就是存的原文，原样返回 */
export function stepLabel(s: string): string {
  if (lang() === "zh-CN") return s;
  const x = s.trim();
  if (WHOLE_STEPS.has(x)) return tAny(x);
  let m: RegExpMatchArray | null;
  if ((m = x.match(/^下载 (.+)$/))) return t("下载 {quality}", { quality: m[1].trim() });
  if ((m = x.match(/^(?:转为|转成) (.+)$/))) return t("转为 {format}", { format: m[1].trim() });
  if ((m = x.match(/^只留前\s*(\d+(?:\.\d+)?)\s*(秒|分钟?)$/))) {
    const n = Number(m[1]);
    return m[2] === "秒" ? t("只留前 {n} 秒", { n }) : t("只留前 {n} 分钟", { n });
  }
  if ((m = x.match(/^(移动到|保存到)\s*(.+)$/))) {
    const dir = STEP_DIRS[m[2]] ? t(STEP_DIRS[m[2]]) : m[2];
    return m[1] === "移动到" ? t("移动到{dir}", { dir }) : t("保存到{dir}", { dir });
  }
  return s;
}

export function stepIcon(s: string): HugeIcon {
  if (/下载/.test(s) && !/字幕/.test(s)) return Download04Icon;
  if (/水印/.test(s)) return EraserIcon;
  if (/裁剪|只留/.test(s)) return ScissorIcon;
  if (/转为|转成|导出/.test(s)) return ArrowDataTransferHorizontalIcon;
  if (/识别|字幕|文字/.test(s)) return SubtitleIcon;
  if (/静音|声音/.test(s)) return VolumeOffIcon;
  if (/移动|保存/.test(s)) return Folder01Icon;
  return SparklesIcon;
}

/** 「移动到指定文件夹」：要用户自己选文件夹（存在预设的 dir 里） */
export const pickDir = (s: string) => s.trim() === "移动到指定文件夹";
export const isDownload = (s: string) => /^(下载|只下载声音|仅下载音频)/.test(s.trim());
export const isMove = (s: string) => /^(移动到|保存到)/.test(s.trim());
