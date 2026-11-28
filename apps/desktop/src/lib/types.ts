// 和后端（crates/captae-core）序列化出来的 JSON 一一对应。
// 后端只加字段、不改旧字段，所以这一轮新加的字段（fail、choose、entries……）都写成可选，旧数据也能显示。

export type JobState = "queue" | "run" | "pause" | "done" | "failed";
export type Phase = "probe" | "pick" | "dl" | "merge" | "post" | "done";
export type Engine = "yt-dlp" | "aria2" | "M3U8";
/** 分片状态：0 还没开始 / 1 正在下载 / 2 已下载 / 3 正在重新下载 / 4 没下载成功 */
export type SegState = 0 | 1 | 2 | 3 | 4;
/** [时间戳毫秒, 文字, 级别] */
export type LogLine = [number, string, "" | "warn" | "err"];
/** M3U8 的一档清晰度 */
export type Variant = { r: string; p: string; b: number; mb: number };

export type ErrorCode =
  | "need_login"
  | "need_verify"
  | "drm"
  | "geo"
  | "blocked"
  | "unavailable"
  | "unsupported"
  | "no_media"
  | "live_upcoming"
  | "live"
  | "rate_limited"
  | "network"
  | "disk"
  | "tool"
  | "unknown";

/** 一次失败：错误码 + 给人看的一句话 + 原文（第一阶段加的，见 errors.rs 的 Failure::to_json） */
/** site：后端说法里用的网站名字（「芒果TV」「哔哩哔哩」），按钮「登录芒果TV」用同一个；认不出的网站没有，界面退回主机名 */
/** reason：同一个错误码下的细分（image_post 图片帖、radio 电台流）；brief：去掉「可以用浏览器插件在网页里下」的短说法（errors.rs）。
 *  message 跟着界面语言变，判断情况用 code、reason，不要看 message 里的字 */
export type Failure = { code: ErrorCode; message: string; raw?: string; action?: string; next?: string; startsAt?: number; site?: string; reason?: string; brief?: string };

/** 播放列表、多视频帖子里的一条（model.rs 的 Entry）。选好以后把 id 传给 choose_entries */
export type Entry = { id: string; title: string; dur?: number | null; thumb?: string | null; url: string; n: number; item?: number };

export type Job = {
  id: number;
  title: string;
  /** 标题还是读取前的占位（「youtube.com · 正在读取标题…」，跟着界面语言变），真标题还没拿到 */
  untitled?: boolean;
  /** 这一次读取信息从什么时候开始（毫秒） */
  probeAt?: number;
  /** 不带协议的地址，显示用 */
  src: string;
  url: string;
  plat?: string;
  eng: Engine;
  fmt: string;
  dur: string;
  thumb?: string;
  audio: boolean;
  /** MB */
  size: number;
  got: number;
  /** MB/s */
  speed: number;
  state: JobState;
  phase: Phase;
  pri: number;
  /** 用户手动开始的，不排队 */
  boost: boolean;
  err?: string;
  when?: string;
  doneAt?: number;
  log: LogLine[];
  segs?: SegState[];
  tries?: number[];
  vars?: Variant[];
  vIdx: number;
  /** 清晰度倒计时结束的时刻（毫秒） */
  pickUntil?: number;
  /** 合并 / 后处理进度 0–100 */
  merge: number;
  path?: string;
  assetId?: number;
  fail?: Failure;
  /** 播放列表 / 多视频帖子：停在 state=pause、phase=probe，等用户选条目 */
  choose?: boolean;
  entries?: Entry[];
  /** 这个视频属于哪个播放列表（YouTube watch?v=…&list=…，默认只下这一个）。B 站分 P：index 是第几 P，count 是一共几 P（读不到就没有） */
  playlist?: { id: string; url: string; count?: number; index?: number };
  /** 读取时认出来是正在进行的直播，而且要从现在开始录（record_live） */
  live?: boolean;
  /** 录直播（model.rs 的 Rec）：since 开始录的时刻；until 停下的时刻；secs 录了几秒（整理好以后才有）；
   *  stopping 点了「停止并保存」、正在保存；interrupted 录制被中断（Captae 退出时还在录，下次打开才整理好） */
  rec?: { since: number; until?: number; raw?: string; out?: string; secs?: number; stopping?: boolean; interrupted?: boolean; max?: number; full?: boolean };
  /** 网站要求放慢（rate_limited）时，什么时候自动重试（毫秒） */
  retryAt?: number;
  /** 从哪台手机加的（手机名，「iPhone」）：任务行上写「来自 iPhone」（crates/captae-remote，model.rs 的 from） */
  from?: string;
};

export type Settings = {
  concurrency: number;
  threads: number;
  clipboard: boolean;
  mcpOn: boolean;
  adv: boolean;
  toolsOff: string[];
  pickSeconds: number;
  whisperModel: string;
  /** 「AI 模型」（OpenAI 兼容接口，字幕翻译和素材库分类共用，core llm.rs） */
  llmBaseUrl: string;
  llmModel: string;
  /** 密钥后端不发给界面（存在 secrets.json）；只在填新的时候往回发 */
  llmApiKey?: string;
  /** 密钥的提示「…3f9a」，空 = 没填 */
  llmKeyHint?: string;
  /** 这个模型能不能看图：空 = 不知道 / yes / no（「测试连接」测出来） */
  llmVision?: string;
  /** 素材库整理过以后，新下载的自动放进分类 */
  aiAutoCategorize?: boolean;
  /** 分类时，看不出内容的素材附上封面给模型看 */
  aiSendCovers?: boolean;
  ytdlpPath: string;
  ffmpegPath: string;
  aria2Path?: string;
  pythonPath: string;
  ytdlpUseAria2?: boolean;
  saveDir: string;
  browser: string;
  jsRuntimePath?: string;
  autoUpdate?: boolean;
  proxyMode?: "system" | "none" | "manual";
  proxyUrl?: string;
  /** 也读取浏览器里的登录状态（yt-dlp --cookies-from-browser）：空 = 不读；chrome / safari / edge / brave / firefox / vivaldi / opera / chromium */
  cookiesFromBrowser?: string;
  /** 界面语言：空 = 跟随系统；zh-CN / en / zh-TW / ja / ko / es / fr / de（i18n/index.ts 的 LANGS） */
  language?: string;
};

/** 浏览器插件发现的资源 */
export type Sniffed = {
  t: string;
  m: string;
  host: string;
  url: string;
  /** 图标：list(m3u8) / film / music / image */
  i: string;
  page_url: string;
  page_title: string;
  size?: number | null;
  content_type?: string | null;
  variants: string[];
  headers: Record<string, string>;
  at: number;
};

export type Preset = { name: string; steps: string[]; agent: boolean; dir?: string };

export type AddRequest = {
  url: string;
  quality?: string | null;
  format?: string | null;
  audio_only?: boolean;
  preset?: string | null;
  output_dir?: string | null;
  page_url?: string | null;
  page_title?: string | null;
  headers?: Record<string, string>;
  from_sniff?: boolean;
  origin?: string;
  /** 正在进行的直播：从现在开始录（失败码 live 的「从现在开始录」按钮用；字段名等第一阶段日志确认） */
  record_live?: boolean;
  /** 浏览器插件带来的 cookies（chrome.cookies.getAll 的结果）和 User-Agent。只给这一次下载用，桌面端界面自己不用 */
  cookies?: unknown[];
  user_agent?: string | null;
};

/** 后台任务（转换、字幕、剪辑、预设、安装语音识别）的进度 */
export type TaskStatus = {
  id: number;
  kind: string;
  title: string;
  assetId?: number;
  pct: number;
  st: "run" | "ok" | "err" | "cancel";
  result?: unknown;
  err?: string;
  note?: string;
  eta?: number;
};

/** AI 助手调用记录 */
export type CallRecord = { id: number; at: number; who: string; tool: string; args: unknown; st: "run" | "ok" | "err"; res?: unknown; ms?: number | null };

/** 素材库的一个分类和它下面有几条（core library.rs 的 Category） */
export type Category = { id: number; name: string; desc: string; sort: number; by: "ai" | "user" | string; count: number };

export type Boot = {
  /** 系统的首选语言里第一个支持的（都不支持时 en）；settings.language 为空时用它 */
  systemLang?: string;
  /** 后端现在用的语言（测试时环境变量 CAPTAE_LANG 能改它） */
  lang?: string;
  settings: Settings;
  jobs: Job[];
  sniffed: Sniffed[];
  presets: Preset[];
  libraryCount: number;
  /** 素材库的分类（core library.rs 的 Category） */
  categories?: Category[];
  tools: { name: string; label: string }[];
  calls: CallRecord[];
  mcpCommand: unknown;
};

export type CaptaeEvent =
  | { type: "job"; job: Job }
  | { type: "job_removed"; id: number }
  | { type: "finished"; id: number; title: string; path: string; asset_id?: number | null }
  | { type: "library" }
  | { type: "agent_call"; call: CallRecord }
  | { type: "sniffed"; item: Sniffed }
  | { type: "clipboard"; url: string }
  | { type: "task"; task: TaskStatus }
  | { type: "settings"; settings: Settings }
  /** 预设试运行的一步（agent_cmds.rs preset_trial；step 是 presets.rs StepResult） */
  | { type: "preset_step"; task: number; step: { index: number; step: string; status: "run" | "ok" | "skip" | "err"; detail: string; output?: string } }
  /** 登录窗口：开始等（open）、登录好了 / 点了完成（done，retried 是重试了几个任务）、关了（closed）。jobs 是等着这个网站的任务 */
  | { type: "login"; state: "open" | "done" | "closed"; site: string; name: string; kind: "login" | "verify"; jobs: number[]; retried: number; how?: "login" | "verify" | null }
  /** 只提示一次的事（比如第一次用 YouTube 账号下载） */
  | { type: "notice"; key: string; title: string; text: string }
  /** 「设置 › 手机」有变化：配对了、撤销了、手机连上 / 断开、开关变了（src-tauri/src/remote_cmds.rs）。页面收到后再调 remote_status */
  | { type: "remote" };

/** 一台配对过的手机（captae-remote server::DeviceView） */
export type RemoteDevice = { id: string; name: string; platform: "ios" | "android" | "other"; pairedAt: number; lastSeen?: number; online: boolean };

/** 「让手机连接这台电脑」的状态（remote_status / remote_enable / remote_revoke，captae-remote server::Status） */
export type RemoteStatus = { enabled: boolean; running: boolean; port?: number | null; hosts: string[]; name: string; error?: string; devices: RemoteDevice[] };

/** 现在显示的配对码（remote_pairing，captae-remote server::PairingView）：uri 是二维码内容，code 是 6 位数；
 *  void：6 位数错了 5 次、作废了（二维码到期前照样能用）；left：6 位数还能试几次 */
export type RemotePairing = { uri: string; code: string; expiresAt: number; void: boolean; left: number };

/** 外部工具的状态（deps_status）：ytdlp / ffmpeg / aria2 / deno（JS 运行时）/ whisper；source 是 custom / managed / bundled / system */
export type DepStatus = { key: "ytdlp" | "ffmpeg" | "aria2" | "deno" | "whisper" | string; ok: boolean; version?: string; path?: string; source?: string; models?: string[] };

/** AI 助手的连接状态（mcp_status） */
export type McpClient = { name: string; connected: boolean; [k: string]: unknown };

/** 设置里「已登录的网站」的一行（login_sites，见 src-tauri/src/login.rs）。
 *  site：youtube / bilibili / x …，其他网站是 host:example.com；at：在 Captae 里登录 / 验证的时间（毫秒，可能没有）；
 *  gain：登录以后有什么用；note：后端给的一句说明（登录失效等） */
export type LoginSite = { site: string; name: string; state: "in" | "verified" | "out"; note?: string; gain?: string; at?: number | null };

/** 登录窗口顶上那条工具条的状态（login_bar_state，Rust 也会主动推给 window.captaeLoginBar） */
export type LoginBarState = {
  kind: "login" | "verify";
  site: string;
  name: string;
  /** 网站那块现在打开的主机名（登录 YouTube 时是 accounts.google.com） */
  host: string;
  secure: boolean;
  /** 登录好以后接着下载几个任务 */
  waiting: number;
  /** 登录好会自己关窗（有登录规则的网站） */
  auto: boolean;
  /** 和主界面一样的语言 */
  lang?: string;
};

/** yt-dlp 用的 JS 运行时（tools.rs 的 JsRuntime） */
export type JsRuntime = { kind: string; path: string; source: "custom" | "bundled" | "system" | string };

/** 下载引擎的版本和自动更新情况（engine_status，见 updater.rs 的 status） */
export type EngineStatus = {
  version?: string | null;
  /** custom 设置里指定的 / managed 自动更新下载的 / bundled 安装包自带的 / system 系统里装的 */
  source?: string;
  path?: string | null;
  bundled?: string | null;
  autoUpdate?: boolean;
  checking?: boolean;
  active?: string | null;
  previous?: string | null;
  latest?: string | null;
  /** 上次检查的时间（毫秒） */
  lastCheck?: number | null;
  lastResult?: string | null;
  lastError?: string | null;
  bad?: string[];
  log?: [number, string][];
  jsRuntime?: JsRuntime | null;
};

/** 「检查更新」的结果（engine_update，updater.rs 的 Report） */
export type EngineReport = {
  result: "updated" | "latest" | "skipped" | "rejected" | "failed";
  message: string;
  current?: string | null;
  latest?: string | null;
  active?: string | null;
  steps: string[];
};

/** 现在走的代理（proxy_status，net.rs 的 status） */
export type ProxyStatus = {
  mode: "system" | "manual" | "none" | string;
  http?: string | null;
  https?: string | null;
  noProxy?: string[];
  pac?: boolean;
  system?: { http?: string | null; https?: string | null; socks?: string | null; exceptions?: string[]; pac?: boolean } | null;
  /** aria2 用得上这个代理（它只认 http 代理） */
  aria2?: boolean;
};
