// 演示后端：AI 助手页的命令。返回结构照真实后端写（apps/desktop/src-tauri/src/agent_cmds.rs、main.rs 的 boot，
// 字段见 crates/captae-core/src 的 mcp.rs ClientStatus / ConfigPlan / ConfigWritten、agent.rs CallRecord、presets.rs StepResult）。
//
// 演示数据：Claude Desktop 连着（几分钟前刚用过），Cursor 昨天用过、现在没连着；最近的 8 条调用记录（有成功的、
// 要登录的下载失败、没开高级模式被拒的、一条还在进行中）；三个预设（一个带「移动到指定文件夹」，一个有 Captae 看不懂的步骤）。
// 不带 &still 时，打开 AI 助手页以后每隔十来秒会来一条新的调用，看调用记录实时追加。
//
// 网址参数（和 mock.ts 的一起用）：
//   &agent=new    第一次用：还没有 AI 助手连上过，没有调用记录
//   &agent=idle   AI 助手都没连着（最近一次是昨天）
//   &agent=long   两个连着，第一个名字很长（local-agent-mode-captae）：侧栏底部只写「已连接 2 个」，AI 助手页、上手引导第 4 页写全名
//   &calls=login  调用记录里再多两条：抖音要验证、哔哩哔哩要登录（看「去验证」「登录哔哩哔哩」按钮）
// 启动快照里的 tools、calls、presets 由 mock.ts 的 boot 从这里取（AGENT_TOOLS、demoCalls、DEMO_PRESETS）。
import skillMd from "../../../../skills/captae/SKILL.md?raw";
import type { CallRecord, Preset, TaskStatus } from "@/lib/types";
import type { MockCtx, MockHandler } from "./mock";
import { liveCancelled, liveEnd, liveStart } from "./mock-library";

const qs = new URLSearchParams(location.search);
const MODE = qs.get("mock") === "empty" ? "new" : qs.get("agent");
const T0 = Date.now();
const MIN = 60_000;
const HOME = "/Users/demo";
const CMD = "/Applications/Captae.app/Contents/MacOS/captae";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** 和 core agent.rs 的 TOOLS 一样：界面上的名字（display_name）+ 人话 */
export const AGENT_TOOLS = [
  { name: "captae.download", label: "下载视频、音乐或图片" },
  { name: "captae.convert", label: "转换文件格式" },
  { name: "captae.subtitle", label: "下载字幕或自动生成字幕" },
  { name: "captae.trim", label: "截取视频片段" },
  { name: "captae.concat", label: "把几段视频拼成一个" },
  { name: "captae.library.query", label: "在素材库里找东西" },
  { name: "captae.library.add_tag", label: "给素材加标签" },
  { name: "captae.preset.run", label: "使用你的预设" },
  { name: "captae.preset.list", label: "查看有哪些预设" },
  { name: "captae.share.video", label: "把视频整理成文字稿和关键帧" },
];

/** 三个预设：抖音素材带「移动到指定文件夹」（已经选好文件夹），小红书竖屏里有一步 Captae 看不懂 */
export const DEMO_PRESETS: Preset[] = [
  { name: "抖音素材", steps: ["下载最高画质", "去除水印", "转为 ProRes 422", "移动到指定文件夹"], agent: true, dir: `${HOME}/Movies/Final Cut 素材` },
  { name: "播客转文字", steps: ["只下载声音（MP3）", "语音转文字", "导出文字稿", "保存到「文稿」文件夹"], agent: true },
  { name: "小红书竖屏", steps: ["下载 1080p", "裁成竖屏 9:16", "去掉声音", "转为 MP4"], agent: false },
];

const MOV = `${HOME}/Movies/Captae`;

/** 最近的调用记录，新的在前（和 boot 里 lib.calls(100) 的顺序一样） */
export function demoCalls(): CallRecord[] {
  if (MODE === "new") return [];
  const at = (m: number) => T0 - m * MIN;
  const list: CallRecord[] = [
    {
      id: 108,
      at: at(0.15),
      who: "Claude Desktop",
      tool: "captae.share.video",
      args: { asset_id: 124 },
      st: "run",
      ms: null,
    },
    {
      id: 107,
      at: at(3),
      who: "Claude Desktop",
      tool: "captae.download",
      args: { url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ", quality: "1080p" },
      st: "ok",
      res: { job_id: 31, title: "Big Buck Bunny · 60fps 4K", state: "queue" },
      ms: 820,
    },
    {
      id: 106,
      at: at(4),
      who: "Claude Desktop",
      tool: "captae.library.query",
      args: { q: "Sintel", type: "video" },
      st: "ok",
      res: { count: 2, items: [{ title: "Sintel · 完整版短片" }, { title: "Sintel 预告片" }] },
      ms: 46,
    },
    {
      id: 105,
      at: at(7),
      who: "Claude Desktop",
      tool: "captae.trim",
      args: { asset_id: 126, start: "0:30", end: "1:00", ffmpeg_args: ["-crf", "18"] },
      st: "err",
      res: { error: "ADVANCED_MODE_OFF", message: "这些参数需要用户在 Captae 的「AI 助手」页里打开「高级模式」" },
      ms: 3,
    },
    {
      id: 104,
      at: at(38),
      who: "Claude Desktop",
      tool: "captae.subtitle",
      args: { asset_id: 124, language: "zh" },
      st: "ok",
      res: { path: `${MOV}/Sintel · 完整版短片.zh.srt`, asset_id: 124, source: "faster-whisper" },
      ms: 48_200,
    },
    {
      id: 103,
      at: at(26 * 60),
      who: "Cursor",
      tool: "captae.download",
      args: { url: "https://www.youtube.com/watch?v=Qn7mG1pR2xE" },
      st: "err",
      res: {
        error: "DOWNLOAD_FAILED",
        message: "有年龄限制，要登录 YouTube 才能下载",
        code: "need_login",
        raw: "ERROR: [youtube] Qn7mG1pR2xE: Sign in to confirm your age. This video may be inappropriate for some users. Use --cookies-from-browser or --cookies for the authentication.",
        action: "login",
        next: "请用户在 Captae 里登录这个网站（或在浏览器里登录后用插件下载），然后重试",
      },
      ms: 3100,
    },
    {
      id: 102,
      at: at(26 * 60 + 4),
      who: "Cursor",
      tool: "captae.convert",
      args: { asset_id: 127, format: "MP4", height: 720 },
      st: "ok",
      res: { path: `${MOV}/Tears of Steel · 720p.mp4`, asset_id: 129 },
      ms: 12_800,
    },
    {
      id: 101,
      at: at(26 * 60 + 9),
      who: "Cursor",
      tool: "captae.preset.list",
      args: {},
      st: "ok",
      res: { presets: [{ name: "抖音素材" }, { name: "播客转文字" }] },
      ms: 5,
    },
  ];
  if (qs.get("calls") === "login")
    list.splice(
      4,
      0,
      {
        id: 1052,
        at: at(9),
        who: "Claude Desktop",
        tool: "captae.download",
        args: { url: "https://www.douyin.com/video/7391234567890123456" },
        st: "err",
        res: {
          error: "DOWNLOAD_FAILED",
          message: "抖音要先完成人机验证",
          code: "need_verify",
          raw: "ERROR: [Douyin] 7391234567890123456: Fresh cookies (not necessarily logged in) are needed",
          action: "verify",
          next: "请用户在 Captae 打开的网页里完成验证，然后重试",
        },
        ms: 2400,
      },
      {
        id: 1051,
        at: at(12),
        who: "Claude Desktop",
        tool: "captae.download",
        args: { url: "https://www.bilibili.com/video/BV1GJ411x7h7", resolution: "1080p" },
        st: "err",
        res: {
          error: "DOWNLOAD_FAILED",
          message: "这个视频要登录哔哩哔哩才能下 1080P",
          code: "need_login",
          raw: "WARNING: [BiliBili] BV1GJ411x7h7: Login is required for 1080P",
          action: "login",
          next: "请用户在 Captae 里登录这个网站（或在浏览器里登录后用插件下载），然后重试",
        },
        ms: 1900,
      },
    );
  return list;
}

function clients() {
  if (MODE === "new") return [];
  const now = Date.now();
  const idle = MODE === "idle";
  const long = MODE === "long";
  return [
    ...(long
      ? [{ name: "local-agent-mode-captae", version: "1.0.0", lastSeen: now - 5_000, lastCall: T0 - 1 * MIN, lastTool: "captae.probe", calls: 4, connected: true }]
      : []),
    {
      name: "Claude Desktop",
      version: "0.14.2",
      lastSeen: idle ? T0 - 25 * 60 * MIN : now - 20_000,
      lastCall: idle ? T0 - 25 * 60 * MIN : T0 - 3 * MIN,
      lastTool: "captae.download",
      calls: 37,
      connected: !idle,
    },
    {
      name: "Cursor",
      version: "1.9.4",
      lastSeen: T0 - 26 * 60 * MIN,
      lastCall: T0 - 26 * 60 * MIN,
      lastTool: "captae.download",
      calls: 6,
      connected: false,
    },
  ];
}

/* ---------------- 预设：和 core presets.rs Step::parse 一样判断认不认识 ---------------- */

function knownStep(s: string): boolean {
  const t = s.trim();
  return (
    t.startsWith("只下载声音") ||
    t === "仅下载音频" ||
    t.startsWith("下载 ") ||
    t === "下载最高画质" ||
    t === "下载" ||
    t.includes("水印") ||
    t.startsWith("转为 ") ||
    t.startsWith("转成 ") ||
    t.includes("网站字幕") ||
    t.includes("平台字幕") ||
    t.includes("语音转文字") ||
    t.includes("识别") ||
    t.includes("导出文字稿") ||
    t.includes("导出 TXT") ||
    t.startsWith("只留前") ||
    t.includes("去掉声音") ||
    t === "静音" ||
    t.startsWith("移动到") ||
    t.startsWith("保存到")
  );
}

/** 试运行一步的结果（presets.rs run_steps 的说法） */
function stepResult(p: Preset, i: number, title: string) {
  const s = p.steps[i];
  if (!knownStep(s)) return { status: "skip" as const, detail: `看不懂「${s}」，跳过了` };
  if (/^(下载|只下载声音|仅下载音频)/.test(s)) return { status: "skip" as const, detail: "试运行不下载，直接用选中的文件" };
  if (s.startsWith("转为 ")) {
    const f = s.slice(3).trim();
    return { status: "ok" as const, detail: "", output: `${MOV}/${title} · ${f}.${f.includes("ProRes") ? "mov" : f.toLowerCase()}` };
  }
  if (/^(移动到|保存到)/.test(s)) return { status: "ok" as const, detail: "复制了一份过去", output: p.dir ?? `${HOME}/Captae` };
  if (s.includes("水印")) return { status: "ok" as const, detail: "没找到水印，原样保留" };
  return { status: "ok" as const, detail: "" };
}

/* ---------------- 实时来的调用（不带 &still 时） ---------------- */

const LIVE: Omit<CallRecord, "id" | "at" | "st">[] = [
  { who: "Claude Desktop", tool: "captae.library.query", args: { q: "Elephants Dream", type: "video" }, res: { count: 1, items: [{ title: "Elephants Dream · 片段 1" }] }, ms: 38 },
  { who: "Claude Desktop", tool: "captae.library.add_tag", args: { asset_id: 125, tag: "参考" }, res: { tags: ["开放电影", "参考"] }, ms: 12 },
  {
    who: "Claude Desktop",
    tool: "captae.download",
    args: { url: "https://vimeo.com/1084537" },
    res: { error: "DOWNLOAD_FAILED", message: "连接被你的网络拦截了，检查代理设置", code: "blocked", raw: "ERROR: [vimeo] 1084537: Unable to download webpage: [SSL: CERTIFICATE_VERIFY_FAILED]", action: "retry", next: "连接被用户的网络拦截了，请用户检查代理设置后重试" },
    ms: 2400,
  },
  { who: "Claude Desktop", tool: "captae.preset.run", args: { name: "播客转文字", asset_id: 120 }, res: { path: `${HOME}/Documents/Captae/播客 · 第 12 期.txt`, asset_id: 130 }, ms: 31_000 },
];

let live: number | null = null;
function startLive(ctx: MockCtx) {
  if (live != null || ctx.still || MODE === "new") return;
  let k = 0;
  let id = 200;
  live = window.setInterval(() => {
    const tpl = LIVE[k++ % LIVE.length];
    const call: CallRecord = { ...tpl, id: id++, at: Date.now(), st: "run", res: undefined, ms: null };
    ctx.state.calls = [call, ...ctx.state.calls].slice(0, 200);
    ctx.emit({ type: "agent_call", call });
    setTimeout(() => {
      const err = !!(tpl.res as { error?: string } | undefined)?.error;
      const done: CallRecord = { ...call, st: err ? "err" : "ok", res: tpl.res, ms: tpl.ms };
      ctx.state.calls = ctx.state.calls.map((c) => (c.id === call.id ? done : c));
      ctx.emit({ type: "agent_call", call: done });
    }, Math.min(2200, (tpl.ms ?? 500) + 400));
  }, 11_000);
}

/* ---------------- 命令 ---------------- */

let written = new Set<string>();
let nextTask = 700;

const CONFIGS = {
  claude: `${HOME}/Library/Application Support/Claude/claude_desktop_config.json`,
  cursor: `${HOME}/.cursor/mcp.json`,
};
const entry = { command: CMD, args: ["mcp", "--stdio"] };

export const agentHandlers: Record<string, MockHandler> = {
  agent_info: (_a, ctx) => {
    startLive(ctx);
    return {
      home: HOME,
      dirs: { movies: MOV, music: `${HOME}/Music/Captae`, pictures: `${HOME}/Pictures/Captae`, shares: `${HOME}/Captae/分享` },
      configs: CONFIGS,
      mcpCommand: CMD,
      skill: skillMd,
    };
  },
  mcp_status: () => ({ now: Date.now(), clients: clients() }),
  mcp_config_plan: ({ client }) => {
    const path = CONFIGS[client as "claude" | "cursor"];
    if (!path) throw new Error(`不支持的客户端：${client}`);
    const exists = client === "claude";
    return {
      path,
      exists,
      snippet: JSON.stringify({ mcpServers: { captae: entry } }, null, 2),
      previous: null,
      unchanged: written.has(client),
      others: exists ? ["filesystem", "github"] : [],
    };
  },
  mcp_config_write: async ({ client }) => {
    await sleep(500);
    const path = CONFIGS[client as "claude" | "cursor"];
    const unchanged = written.has(client);
    written = new Set([...written, client]);
    return { path, backup: client === "claude" && !unchanged ? path + ".bak" : null, unchanged };
  },
  pick_folder: async () => {
    await sleep(200);
    return `${HOME}/Movies/Final Cut 素材`;
  },
  export_skill: async () => {
    await sleep(300);
    return `${HOME}/Documents/AI Skills/captae/SKILL.md`;
  },
  preset_check: ({ preset }) => (preset as Preset).steps.map(knownStep),
  // 试运行：马上回任务编号，每一步的开始和结果用 preset_step 事件推过去，最后 task 事件说完成（和 agent_cmds.rs 一样）
  preset_trial: ({ preset, assetId }, ctx) => {
    const p = preset as Preset;
    const id = nextTask++;
    const title = "Sintel · 完整版短片";
    const task = (st: TaskStatus["st"], pct: number, result?: unknown): TaskStatus => ({ id, kind: "preset", title: `试运行「${p.name}」· ${title}`, assetId, pct, st, result });
    // 发出去的同时记进 state.tasks：task_cancel 认得出它（&still 停住时也能取消）
    const send = (t: TaskStatus) => {
      ctx.state.tasks.set(id, t);
      ctx.emit({ type: "task", task: t });
    };
    (async () => {
      const outputs: string[] = [];
      await sleep(120);
      send(task("run", 0));
      liveStart(id);
      for (let i = 0; i < p.steps.length; i++) {
        ctx.emit({ type: "preset_step", task: id, step: { index: i, step: p.steps[i], status: "run", detail: "" } });
        // 截图用的 &still：停在第三步进行中
        if (ctx.still && i >= 2) return liveEnd(id);
        await sleep(ctx.still ? 0 : 900);
        // 取消了：正在跑的那一步停下（core 里没写完的文件会删掉），不再往下走
        if (liveCancelled(id)) return send({ ...task("cancel", i / p.steps.length), err: "CANCELLED" });
        const r = stepResult(p, i, title);
        if (r.output) outputs.push(r.output);
        ctx.emit({ type: "preset_step", task: id, step: { index: i, step: p.steps[i], ...r } });
        send(task("run", (i + 1) / p.steps.length));
      }
      liveEnd(id);
      send(task("ok", 1, { steps: [], outputs, assetId: 131 }));
    })();
    return { task: id, assetId, title };
  },
};
