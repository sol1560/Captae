// 演示后端：素材库的命令。返回结构照真实后端写（apps/desktop/src-tauri/src/main.rs、library_cmds.rs，
// 素材字段见 crates/captae-core/src/library.rs 的 Asset），界面拿到的东西和在 Captae 里一样。
//
// 演示数据：一共 128 条（和侧栏的数一致），最新的十几条是手写的：视频、音频、图片都有，
// YouTube 和哔哩哔哩的带网站字幕，X 和 blender.org 的没有；「Big Buck Bunny · 预告片」「Sintel_poster」两条的文件已经不在了。
// 再往前的是按模板生成的，用来试翻页（一页 60 条）。下载页里已完成的四个任务就是这里的 128、127、126、125。
//
// 网址参数（和 mock.ts 的一起用）：
//   &whisper   语音转文字已经装好（默认没装，字幕面板会引导安装）
//   &fail      转换、字幕、剪辑、拼接都会失败（看出错的样子）
//   &ltask     打开时已经有一个转换在跑（124 号 Sintel，停在 37%，截图用）
//   ?mock=empty 素材库是空的
// 想一打开就选中某一条，用真实的跳转地址：#/library?asset=124
import type { Asset, SubInfo, Track } from "@/pages/library/model";
import type { MockCtx, MockHandler } from "./mock";

const qs = new URLSearchParams(location.search);
const EMPTY = qs.get("mock") === "empty";
const WHISPER = qs.has("whisper");
const FAIL = qs.has("fail");
const T0 = Date.now();
const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

type Img = MockCtx["img"];
type Seed = Partial<Asset> & Pick<Asset, "id" | "type" | "title" | "src" | "fmt" | "path"> & { ago: number; transcript?: string; hasAudio?: boolean };

/* ---------------- 和后端一样的派生字段 ---------------- */

function humanSize(b: number): string {
  if (b >= 1e9) return (b / 1e9).toFixed(2) + " GB";
  if (b >= 1e6) return Math.round(b / 1e6) + " MB";
  if (b >= 1e3) return Math.round(b / 1e3) + " KB";
  return b + " B";
}
function clock(s?: number | null): string {
  if (!s || s <= 0) return "";
  const r = Math.round(s);
  const h = Math.floor(r / 3600),
    m = Math.floor((r % 3600) / 60),
    x = String(r % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${x}` : `${m}:${x}`;
}
/** util.rs 的 when：今天 10:12 / 昨天 22:15 / 3 天前 */
function when(ms: number): string {
  const d = new Date(ms);
  const today = new Date();
  today.setHours(0, 0, 0, 0);
  const days = Math.max(0, Math.ceil((today.getTime() - new Date(d).setHours(0, 0, 0, 0)) / DAY));
  const hm = d.toTimeString().slice(0, 5);
  if (days <= 0) return Date.now() - ms < MIN ? "刚刚" : `今天 ${hm}`;
  if (days === 1) return `昨天 ${hm}`;
  return `${days} 天前`;
}

const transcripts = new Map<number, string>();
const silent = new Set<number>();

function make(s: Seed): Asset {
  const created = T0 - s.ago;
  if (s.transcript) transcripts.set(s.id, s.transcript);
  if (s.hasAudio === false) silent.add(s.id);
  const w = s.width ?? null,
    h = s.height ?? null;
  return {
    id: s.id,
    type: s.type,
    title: s.title,
    desc: s.desc ?? "",
    url: s.url ?? "",
    page: s.page ?? "",
    src: s.src,
    uploader: s.uploader ?? "",
    path: s.path,
    fmt: s.fmt,
    width: w,
    height: h,
    durS: s.durS ?? null,
    bytes: s.bytes ?? 0,
    thumb: s.thumb ?? null,
    tags: s.tags ?? [],
    created_at: created,
    parent_id: s.parent_id ?? null,
    has_transcript: !!s.transcript,
    res: w && h ? `${w}×${h}` : "—",
    dur: clock(s.durS),
    size: humanSize(s.bytes ?? 0),
    when: when(created),
    day: Math.floor((T0 - created) / DAY),
    exists: s.exists ?? true,
  };
}

const MOV = "/Users/demo/Movies/Captae/";
const MUS = "/Users/demo/Music/Captae/";
const PIC = "/Users/demo/Pictures/Captae/";

/** 手写的那十几条（最新的在前）。128–125 和下载页里已完成的四个任务一一对应 */
function handmade(img: Img): Seed[] {
  return [
    {
      id: 128,
      type: "video",
      title: "Big Buck Bunny · 60fps 4K",
      desc: "一只大兔子和三只捣蛋的啮齿动物。Blender 基金会 2008 年的开放电影。",
      page: "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
      url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
      src: "YouTube",
      uploader: "Blender",
      path: MOV + "Big Buck Bunny 60fps 4K.mp4",
      fmt: "MP4",
      width: 3840,
      height: 2160,
      durS: 634,
      bytes: 640e6,
      thumb: img.bbb,
      tags: ["短片", "动画"],
      ago: 2 * MIN,
      transcript: "兔子 松鼠 蝴蝶 森林 早上好",
    },
    {
      id: 127,
      type: "video",
      title: "Sintel · 预告片",
      url: "https://download.blender.org/durian/trailer/sintel_trailer-1080p.mp4",
      src: "blender.org",
      path: MOV + "sintel_trailer-1080p.mp4",
      fmt: "MP4",
      width: 1920,
      height: 818,
      durS: 52,
      bytes: 48e6,
      thumb: img.sintel3,
      tags: ["预告"],
      ago: 41 * MIN,
    },
    {
      id: 126,
      type: "video",
      title: "Tears of Steel · 4K 片段",
      url: "https://mango.blender.org/tos/4k/master.m3u8",
      page: "https://mango.blender.org/",
      src: "blender.org",
      path: MOV + "Tears of Steel 4K.mp4",
      fmt: "MP4",
      width: 3840,
      height: 1714,
      durS: 192,
      bytes: 412e6,
      thumb: img.tos5,
      tags: ["科幻"],
      ago: 3 * HOUR,
    },
    {
      id: 125,
      type: "audio",
      title: "Elephants Dream · 原声",
      page: "https://www.youtube.com/watch?v=TLkA0RELQ1g",
      url: "https://www.youtube.com/watch?v=TLkA0RELQ1g",
      src: "YouTube",
      path: MUS + "Elephants Dream.mp3",
      fmt: "MP3",
      durS: 654,
      bytes: 11e6,
      tags: ["配乐"],
      ago: 26 * HOUR,
    },
    {
      id: 124,
      type: "video",
      title: "Sintel · 完整版短片",
      desc: "少女 Sintel 在寒冷的高地上寻找她的小龙。Blender 基金会 2010 年的开放电影。",
      page: "https://www.bilibili.com/video/BV1qJ411m7Sg",
      url: "https://www.bilibili.com/video/BV1qJ411m7Sg",
      src: "哔哩哔哩",
      uploader: "Blender 基金会",
      path: MOV + "Sintel 完整版短片.mp4",
      fmt: "MP4",
      width: 1920,
      height: 818,
      durS: 888,
      bytes: 312e6,
      thumb: img.sintel1,
      tags: ["短片"],
      ago: 27 * HOUR,
      transcript: "小龙 雪山 卫兵 你见过一只龙吗",
    },
    {
      id: 123,
      type: "audio",
      title: "Sintel · 原声",
      page: "https://www.bilibili.com/video/BV1qJ411m7Sg",
      url: "https://www.bilibili.com/video/BV1qJ411m7Sg",
      src: "哔哩哔哩",
      path: MUS + "Sintel 原声.mp3",
      fmt: "MP3",
      durS: 888,
      bytes: 34e6,
      tags: ["配乐"],
      ago: 27 * HOUR + 4 * MIN,
    },
    {
      id: 122,
      type: "video",
      title: "Tears of Steel · 完整版",
      desc: "一群科学家和战士在阿姆斯特丹试图挽回过去。Blender 基金会 2012 年的开放电影。",
      url: "https://mango.blender.org/tos/master.m3u8",
      page: "https://mango.blender.org/",
      src: "blender.org",
      path: MOV + "Tears of Steel 完整版.mp4",
      fmt: "MP4",
      width: 1920,
      height: 800,
      durS: 734,
      bytes: 420e6,
      thumb: img.tos1,
      tags: ["科幻"],
      ago: 28 * HOUR,
    },
    {
      id: 121,
      type: "video",
      title: "Elephants Dream · 片段 1",
      page: "https://x.com/BlenderFoundation/status/1856200118852337664",
      url: "https://video.twimg.com/ext_tw_video/1856200118852337664/pu/vid/1280x720/ed1.mp4",
      src: "X",
      uploader: "Blender",
      path: MOV + "Elephants Dream 片段 1.mp4",
      fmt: "MP4",
      width: 1280,
      height: 720,
      durS: 30,
      bytes: 8.4e6,
      thumb: img.edEmo,
      ago: 2 * DAY + 2 * HOUR,
    },
    {
      id: 120,
      type: "video",
      title: "Elephants Dream · 片段 2",
      page: "https://x.com/BlenderFoundation/status/1856200118852337664",
      url: "https://video.twimg.com/ext_tw_video/1856200118852337664/pu/vid/1280x720/ed2.mp4",
      src: "X",
      uploader: "Blender",
      path: MOV + "Elephants Dream 片段 2.mp4",
      fmt: "MP4",
      width: 1280,
      height: 720,
      durS: 78,
      bytes: 21e6,
      thumb: img.edPlat,
      ago: 2 * DAY + 2 * HOUR + MIN,
    },
    {
      id: 119,
      type: "image",
      title: "Tears_of_Steel_frame_05",
      url: "https://mango.blender.org/wp-content/gallery/4k-renders/05_thom_robot.jpg",
      page: "https://mango.blender.org/production/4k-renders/",
      src: "blender.org",
      path: PIC + "Tears_of_Steel_frame_05.jpg",
      fmt: "JPG",
      width: 960,
      height: 400,
      bytes: 182e3,
      thumb: img.tos5,
      ago: 2 * DAY + 4 * HOUR,
    },
    {
      id: 118,
      type: "image",
      title: "Elephants_Dream_platform",
      url: "https://orange.blender.org/wp-content/uploads/platform.jpg",
      src: "blender.org",
      path: PIC + "Elephants_Dream_platform.jpg",
      fmt: "JPG",
      width: 960,
      height: 540,
      bytes: 204e3,
      thumb: img.edPlat,
      tags: ["参考"],
      ago: 3 * DAY,
    },
    {
      id: 117,
      type: "video",
      title: "Big Buck Bunny · 预告片",
      page: "https://www.youtube.com/watch?v=YE7VzlLtp-4",
      url: "https://www.youtube.com/watch?v=YE7VzlLtp-4",
      src: "YouTube",
      path: "/Users/demo/Desktop/Big Buck Bunny 预告片.mp4",
      fmt: "MP4",
      width: 1280,
      height: 720,
      durS: 33,
      bytes: 9.8e6,
      thumb: img.bbb,
      ago: 4 * DAY,
      exists: false,
    },
    {
      id: 116,
      type: "image",
      title: "Sintel_poster",
      url: "https://durian.blender.org/wp-content/uploads/2010/06/poster.jpg",
      page: "https://durian.blender.org/",
      src: "blender.org",
      path: PIC + "Sintel_poster.jpg",
      fmt: "JPG",
      width: 1240,
      height: 1754,
      bytes: 1.2e6,
      thumb: null,
      ago: 4 * DAY + HOUR,
      exists: false,
    },
    {
      id: 115,
      type: "video",
      title: "Big Buck Bunny 60fps 4K 片段",
      page: "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
      url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ",
      src: "YouTube",
      path: MOV + "Big Buck Bunny 60fps 4K 片段.mp4",
      fmt: "MP4",
      width: 3840,
      height: 2160,
      durS: 12,
      bytes: 18e6,
      thumb: img.bbb,
      tags: ["短片", "动画"],
      parent_id: 128,
      ago: 5 * DAY,
    },
    {
      id: 114,
      type: "video",
      title: "Tears of Steel 完整版",
      url: "https://mango.blender.org/tos/master.m3u8",
      page: "https://mango.blender.org/",
      src: "blender.org",
      path: MOV + "Tears of Steel 完整版.mov",
      fmt: "MOV",
      width: 1920,
      height: 800,
      durS: 734,
      bytes: 9.6e9,
      thumb: img.tos1,
      tags: ["科幻"],
      parent_id: 122,
      ago: 5 * DAY + 2 * HOUR,
    },
    {
      id: 113,
      type: "audio",
      title: "开放电影是怎么做出来的 · 播客",
      desc: "Blender Studio 的导演和美术聊开放电影的制作流程。",
      page: "https://www.youtube.com/watch?v=WhWc3b3KhnY",
      url: "https://www.youtube.com/watch?v=WhWc3b3KhnY",
      src: "YouTube",
      path: MUS + "开放电影是怎么做出来的.m4a",
      fmt: "M4A",
      durS: 2712,
      bytes: 41e6,
      tags: ["播客"],
      ago: 6 * DAY,
      transcript: "开放电影 渲染 分镜 预算 社区",
    },
    {
      id: 112,
      type: "subtitle",
      title: "Sintel 完整版短片.zh",
      page: "https://www.bilibili.com/video/BV1qJ411m7Sg",
      src: "哔哩哔哩",
      path: MOV + "Sintel 完整版短片.zh.srt",
      fmt: "SRT",
      bytes: 18e3,
      parent_id: 124,
      ago: 6 * DAY + HOUR,
    },
    {
      id: 111,
      type: "video",
      title: "城市夜景延时",
      src: "本地",
      path: "/Users/demo/Movies/延时/城市夜景延时.mov",
      fmt: "MOV",
      width: 3840,
      height: 2160,
      durS: 46,
      bytes: 1.1e9,
      thumb: img.tos1,
      ago: 7 * DAY,
      hasAudio: false,
    },
  ];
}

/** 再往前的：按模板生成，试翻页用 */
function generated(img: Img, from: number): Seed[] {
  const T: { t: string; type: Asset["type"]; src: string; fmt: string; thumb?: keyof Img; dur?: number; w?: number; h?: number; page?: string; tags?: string[] }[] = [
    { t: "Big Buck Bunny · 镜头", type: "video", src: "YouTube", fmt: "MP4", thumb: "bbb", dur: 18, w: 1920, h: 1080, page: "https://www.youtube.com/watch?v=aqz-KE-bpKQ", tags: ["动画"] },
    { t: "Sintel · 镜头", type: "video", src: "哔哩哔哩", fmt: "MP4", thumb: "sintel1", dur: 24, w: 1920, h: 818, page: "https://www.bilibili.com/video/BV1qJ411m7Sg" },
    { t: "Tears of Steel · 镜头", type: "video", src: "blender.org", fmt: "MP4", thumb: "tos1", dur: 31, w: 1920, h: 800 },
    { t: "Elephants Dream · 镜头", type: "video", src: "X", fmt: "MP4", thumb: "edEmo", dur: 15, w: 1280, h: 720, page: "https://x.com/BlenderFoundation/status/1856200118852337664" },
    { t: "环境声 · 第", type: "audio", src: "archive.org", fmt: "MP3", dur: 182, page: "https://archive.org/details/ambient" },
    { t: "剧照 ", type: "image", src: "blender.org", fmt: "JPG", thumb: "tos5", w: 960, h: 400 },
    { t: "Sintel · 预告片镜头", type: "video", src: "YouTube", fmt: "WEBM", thumb: "sintel3", dur: 9, w: 1280, h: 545, page: "https://www.youtube.com/watch?v=eRsGyueVLvQ", tags: ["预告"] },
    { t: "Elephants Dream · 场景", type: "image", src: "blender.org", fmt: "PNG", thumb: "edPlat", w: 960, h: 540 },
  ];
  const out: Seed[] = [];
  for (let id = from, k = 0; id >= 1; id--, k++) {
    const x = T[k % T.length];
    const n = Math.floor(k / T.length) + 1;
    const title = x.type === "image" ? `${x.t}${String(n).padStart(2, "0")}` : x.type === "audio" ? `${x.t} ${n} 段` : `${x.t} ${n}`;
    const dir = x.type === "audio" ? MUS : x.type === "image" ? PIC : MOV;
    out.push({
      id,
      type: x.type,
      title,
      src: x.src,
      fmt: x.fmt,
      page: x.page ?? "",
      url: x.page ?? `https://${x.src}/media/${id}`,
      path: `${dir}${title}.${x.fmt.toLowerCase()}`,
      width: x.w,
      height: x.h,
      durS: x.dur ? x.dur + (k % 7) * 3 : null,
      bytes: x.type === "image" ? 150e3 + (k % 5) * 40e3 : x.type === "audio" ? 3e6 + (k % 4) * 1e6 : 12e6 + (k % 9) * 6e6,
      thumb: x.thumb ? img[x.thumb] : null,
      tags: x.tags ?? [],
      ago: 8 * DAY + k * 9 * HOUR,
    });
  }
  return out;
}

/* ---------------- 演示库 ---------------- */

let db: Map<number, Asset> | null = null;
const gone = new Set<number>();
const SITE_NAME: Record<string, string> = { youtube: "YouTube", bilibili: "哔哩哔哩", x: "X", tiktok: "TikTok", douyin: "抖音", instagram: "Instagram" };

function lib(ctx: MockCtx): Map<number, Asset> {
  if (!db) {
    db = new Map();
    if (!EMPTY) {
      const hand = handmade(ctx.img);
      const low = Math.min(...hand.map((s) => s.id));
      for (const s of [...hand, ...generated(ctx.img, low - 1)]) db.set(s.id, make(s));
    }
  }
  // 下载页里刚下完的任务（演示后端给它们分了新的 assetId）也放进来
  for (const j of ctx.state.jobs.values()) {
    if (j.state !== "done" || j.assetId == null || db.has(j.assetId) || gone.has(j.assetId)) continue;
    const isAudio = j.audio;
    db.set(
      j.assetId,
      make({
        id: j.assetId,
        type: isAudio ? "audio" : "video",
        title: j.title,
        url: j.url,
        page: j.plat ? j.url : "",
        src: j.plat ? (SITE_NAME[j.plat] ?? j.plat) : j.src.split("/")[0].replace(/^www\./, ""),
        path: j.path ?? MOV + j.title + ".mp4",
        fmt: isAudio ? "MP3" : "MP4",
        width: isAudio ? null : 1920,
        height: isAudio ? null : 1080,
        durS: parseClock(j.dur),
        bytes: j.size * 1e6,
        thumb: isAudio ? null : (j.thumb ?? null),
        ago: T0 - (j.doneAt ?? Date.now()),
      }),
    );
  }
  return db;
}

function parseClock(s?: string): number | null {
  if (!s) return null;
  const p = s.split(":").map(Number);
  if (p.some((x) => !isFinite(x))) return null;
  return p.reduce((a, b) => a * 60 + b, 0);
}

function matches(a: Asset, text: string): boolean {
  const t = text.toLowerCase();
  const hay = [a.title, a.desc, a.src, ...a.tags, transcripts.get(a.id) ?? ""].join("\n").toLowerCase();
  return hay.includes(t);
}

function counted(L: string[]): [string, number][] {
  const m = new Map<string, number>();
  for (const x of L) if (x) m.set(x, (m.get(x) ?? 0) + 1);
  return [...m.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

const copy = (a: Asset): Asset => ({ ...a, tags: [...a.tags] });

function need(ctx: MockCtx, id: number): Asset {
  const a = lib(ctx).get(Number(id));
  if (!a) throw new Error("素材不存在");
  return a;
}
function needFile(ctx: MockCtx, id: number): Asset {
  const a = need(ctx, id);
  if (!a.exists) throw new Error(`文件已经不在了：${a.path}`);
  return a;
}

/* ---------------- 后台任务（进度走 task 事件） ---------------- */

let taskSeq = 500;

/** 还在跑、能取消的演示任务（素材库的处理、AI 助手页的试运行）→ 点了取消没有 */
const live = new Map<number, boolean>();
/** 演示任务开始 / 结束（mock-agent 的试运行也用） */
export const liveStart = (id: number) => void live.set(id, false);
export const liveEnd = (id: number) => void live.delete(id);
/** 这个任务被取消了吗（取消了就从表里拿掉） */
export const liveCancelled = (id: number) => live.get(id) === true && live.delete(id);

/** 跑一个后台任务：每 ~250ms 发一次进度，跑完返回；带 &fail 时跑到一半失败。&still 时一下子跑完。
 *  被取消（task_cancel）时下一步就停下：发 st = cancel，命令返回 CANCELLED（和 core tasks.rs 一样） */
async function runTask(ctx: MockCtx, kind: string, title: string, assetId: number | undefined, ms: number, fail?: string): Promise<void> {
  const id = ++taskSeq;
  const send = (pct: number, st: "run" | "ok" | "err" | "cancel", err?: string) => {
    const t = { id, kind, title, assetId, pct, st, err };
    ctx.state.tasks.set(id, t);
    ctx.emit({ type: "task", task: t });
  };
  send(0, "run");
  liveStart(id);
  const steps = ctx.still ? 1 : Math.max(4, Math.round(ms / 250));
  let last = 0;
  for (let i = 1; i <= steps; i++) {
    await sleep(ctx.still ? 0 : ms / steps);
    if (liveCancelled(id)) {
      send(last, "cancel", "CANCELLED");
      throw new Error("CANCELLED");
    }
    const pct = Math.min(100, Math.round((i / steps) * 1000) / 10);
    last = pct;
    if (FAIL && pct >= 55) {
      const e = fail ?? "ffmpeg 出错：Conversion failed!";
      liveEnd(id);
      send(pct, "err", e);
      throw new Error(e);
    }
    if (pct >= 100) liveEnd(id);
    send(pct, pct >= 100 ? "ok" : "run");
  }
}

/** 转换、剪辑、拼接出来的新文件：继承原素材的来源和标签，入库 */
function addChild(ctx: MockCtx, parent: Asset, patch: Partial<Asset> & Pick<Asset, "path" | "fmt" | "type">): number {
  const id = ++ctx.state.libraryCount;
  const stem = patch.path.split("/").pop()!.replace(/\.[^.]+$/, "");
  const a = make({
    ...parent,
    ...patch,
    id,
    title: patch.title ?? stem,
    parent_id: parent.id,
    exists: true,
    ago: T0 - Date.now(),
    tags: [...parent.tags],
  } as Seed);
  lib(ctx).set(id, a);
  ctx.emit({ type: "library" });
  return id;
}

const EXT: Record<string, string> = { "prores 422": "mov", mov: "mov", mp4: "mp4", webm: "webm", mp3: "mp3", aac: "m4a", flac: "flac", wav: "wav", png: "png", webp: "webp", avif: "avif", jpg: "jpg" };

/** 一条声音的波形：按 id 算出来的「像真的」的峰值（0..1） */
function peaks(id: number, n: number, speech: boolean): number[] {
  let s = id * 9301 + 49297;
  const rnd = () => ((s = (s * 9301 + 49297) % 233280) / 233280);
  const out: number[] = [];
  for (let i = 0; i < n; i++) {
    const x = i / n;
    const env = speech ? 0.55 + 0.25 * Math.sin(x * 23 + id) : 0.35 + 0.45 * Math.sin(x * Math.PI) * (0.8 + 0.2 * Math.sin(x * 9 + id));
    const gap = speech && rnd() < 0.12 ? 0.15 : 1;
    out.push(Math.max(0.04, Math.min(1, env * gap * (0.55 + 0.45 * rnd()))));
  }
  return out;
}

const YT_TRACKS: Track[] = [
  { code: "en", name: "English", auto: false },
  { code: "zh-Hans", name: "中文（简体）", auto: false },
  { code: "en-orig", name: "English（自动生成）", auto: true },
];
const BILI_TRACKS: Track[] = [{ code: "ai-zh", name: "中文（自动生成）", auto: true }];

const CUES: [string, string][] = [
  ["0:00", "（风声）"],
  ["0:07", "你好？有人在吗？"],
  ["0:12", "我在找一只小龙，你见过它吗？"],
  ["0:19", "它大概这么大，翅膀是红色的。"],
  ["0:26", "往山上走吧，那里还有人。"],
  ["0:33", "谢谢你。"],
];

/** &ltask：打开时就有一个转换在跑，停在 37%（截图用） */
let demoTask = false;
function startDemoTask(ctx: MockCtx) {
  if (demoTask || !qs.has("ltask")) return;
  demoTask = true;
  const t = { id: 499, kind: "convert", title: "Sintel · 完整版短片 → ProRes 422", assetId: 124, pct: 37, st: "run" as const };
  ctx.state.tasks.set(t.id, t);
  ctx.emit({ type: "task", task: t });
}

/* ---------------- 命令 ---------------- */

export const libraryHandlers: Record<string, MockHandler> = {
  // 取消后台任务（task_cmds.rs task_cancel）：跑着的下一步停下；停住的演示任务（&ltask、&still 时的试运行）直接算取消。已经结束的回 false
  task_cancel: ({ id }, ctx) => {
    if (live.has(id)) {
      live.set(id, true);
      return true;
    }
    const t = ctx.state.tasks.get(id);
    if (!t || t.st !== "run") return false;
    const c = { ...t, st: "cancel" as const, err: "CANCELLED" };
    ctx.state.tasks.set(id, c);
    ctx.emit({ type: "task", task: c });
    return true;
  },
  library_query: ({ q = {} }, ctx) => {
    startDemoTask(ctx);
    const all = [...lib(ctx).values()];
    let L = all;
    if (q.kind) L = L.filter((a) => a.type === q.kind);
    if (q.site) L = L.filter((a) => a.src.toLowerCase() === String(q.site).toLowerCase());
    if (q.format) L = L.filter((a) => a.fmt.toUpperCase() === String(q.format).toUpperCase());
    if (q.days) L = L.filter((a) => a.created_at >= Date.now() - Number(q.days) * DAY);
    for (const t of (q.tags ?? []) as string[]) L = L.filter((a) => a.tags.includes(t));
    const text = String(q.q ?? "").trim();
    if (text) L = L.filter((a) => matches(a, text));
    L = L.sort((a, b) => b.created_at - a.created_at);
    const offset = Number(q.offset ?? 0),
      limit = Math.min(1000, Number(q.limit ?? 200));
    return {
      items: L.slice(offset, offset + limit).map(copy),
      total: L.length,
      all: all.length,
      sites: counted(all.map((a) => a.src)),
      tags: counted(all.flatMap((a) => a.tags)),
    };
  },
  library_get: ({ id }, ctx) => {
    const a = lib(ctx).get(Number(id));
    return a ? copy(a) : null;
  },
  library_tags: ({ id, add, remove }, ctx) => {
    const a = need(ctx, id);
    if (remove) a.tags = a.tags.filter((t) => t !== remove);
    else if (add && !a.tags.includes(add)) a.tags = [...a.tags, add];
    ctx.emit({ type: "library" });
    return [...a.tags];
  },
  library_remove: ({ id }, ctx) => {
    need(ctx, id);
    lib(ctx).delete(Number(id));
    gone.add(Number(id));
    ctx.emit({ type: "library" });
  },
  library_trash: async ({ id }, ctx) => {
    need(ctx, id);
    await sleep(ctx.still ? 0 : 200);
    lib(ctx).delete(Number(id));
    gone.add(Number(id));
    ctx.emit({ type: "library" });
  },
  library_share: async ({ id }, ctx) => {
    const a = needFile(ctx, id);
    await runTask(ctx, "share", `把「${a.title}」发给 AI`, a.id, 2400, "没能整理好文字稿");
    return { copied: true, dir: `/Users/demo/Library/Caches/Captae/share/${a.id}`, frames: 12, transcript: transcripts.has(a.id) ? "subtitles" : "whisper" };
  },
  library_frames: async ({ id, n = 10 }, ctx) => {
    const a = needFile(ctx, id);
    if (!a.durS) throw new Error("不知道这个视频有多长");
    await sleep(ctx.still ? 0 : 450);
    // 演示：没有真的抽帧，用几张剧照轮着当画面
    const pool = [a.thumb, ctx.img.tos1, ctx.img.sintel1, ctx.img.edEmo, ctx.img.bbb, ctx.img.tos5, ctx.img.edPlat, ctx.img.sintel3].filter(Boolean) as string[];
    return Array.from({ length: Math.max(1, Math.min(30, Number(n))) }, (_, k) => pool[(k * 3 + a.id) % pool.length]);
  },
  library_waveform: async ({ id, n = 48 }, ctx) => {
    const a = needFile(ctx, id);
    await sleep(ctx.still ? 0 : 260);
    return peaks(a.id, Math.max(8, Math.min(400, Number(n))), /播客|podcast/i.test(a.title));
  },
  subtitle_tracks: async ({ id }, ctx): Promise<SubInfo> => {
    const a = need(ctx, id);
    await sleep(ctx.still ? 0 : 650);
    const tracks = a.src === "YouTube" ? YT_TRACKS : a.src === "哔哩哔哩" ? BILI_TRACKS : [];
    return { tracks, err: null, hasAudio: a.exists && (a.type === "video" || a.type === "audio") && !silent.has(a.id), whisper: WHISPER, canBurn: true };
  },
  pick_audio_file: async () => {
    await sleep(300);
    return "/Users/demo/Music/背景音乐 · 夏天.m4a";
  },
  task_convert: async ({ id, format, height }, ctx) => {
    const a = needFile(ctx, id);
    const f = String(format);
    const ext = EXT[f.toLowerCase()] ?? f.toLowerCase();
    const stem = a.path.replace(/\.[^./]+$/, "");
    const same = a.path.toLowerCase().endsWith("." + ext);
    const path = `${stem}${same ? " (转换)" : ""}.${ext}`;
    await runTask(ctx, "convert", `${a.title} → ${f}`, a.id, f.startsWith("ProRes") ? 4200 : 2600);
    const audio = ["mp3", "m4a", "flac", "wav"].includes(ext);
    const h = height && a.height && a.height > height ? Number(height) : a.height;
    const w = h && a.width && a.height ? Math.round((a.width * h) / a.height / 2) * 2 : a.width;
    const nid = addChild(ctx, a, {
      path,
      fmt: ext.toUpperCase(),
      type: audio ? "audio" : a.type,
      width: audio ? null : w,
      height: audio ? null : h,
      thumb: audio ? null : a.thumb,
      bytes: f.startsWith("ProRes") ? a.bytes * 18 : audio ? (a.durS ?? 60) * 24e3 : a.bytes * 0.8,
    });
    return { path, assetId: nid };
  },
  task_subtitle: async ({ id, opts = {}, burn }, ctx) => {
    const a = needFile(ctx, id);
    const fmt = String(opts.format ?? "srt").toLowerCase();
    const lang = opts.language || (opts.source === "whisper" ? "zh" : "en");
    await runTask(ctx, "subtitle", `${a.title} 的字幕`, a.id, burn ? 5200 : opts.source === "whisper" ? 4200 : 1800, "没能下载字幕：HTTP Error 429");
    const path = a.path.replace(/\.[^./]+$/, "") + `.${lang}.${fmt}`;
    const r: Record<string, unknown> = { path, source: opts.source === "whisper" ? "whisper" : "platform", language: lang, cues: 182, preview: CUES };
    if (burn) {
      const bpath = a.path.replace(/\.[^./]+$/, "") + " (字幕).mp4";
      r.burned = bpath;
      r.burnedId = addChild(ctx, a, { path: bpath, fmt: "MP4", type: "video" });
    }
    return r;
  },
  task_trim: async ({ id, opts = {} }, ctx) => {
    const a = needFile(ctx, id);
    const len = Math.max(0.1, Number(opts.end) - Number(opts.start));
    await runTask(ctx, "trim", `${a.title} 片段`, a.id, opts.precise ? 3200 : 1600);
    const ext = a.path.split(".").pop() ?? "mp4";
    const path = a.path.replace(/\.[^./]+$/, "") + ` 片段.${ext}`;
    const nid = addChild(ctx, a, { path, fmt: a.fmt, type: a.type, durS: len, bytes: (a.bytes * len) / Math.max(1, a.durS ?? len) });
    return { path, assetId: nid };
  },
  task_concat: async ({ ids }, ctx) => {
    const L = (ids as number[]).map((i) => needFile(ctx, i));
    if (L.length < 2) throw new Error("至少选两段视频");
    const bad = L.find((a) => a.type !== "video");
    if (bad) throw new Error(`「${bad.title}」不是视频，只能拼接视频`);
    await runTask(ctx, "concat", `拼接 ${L.length} 段视频`, L[0].id, 3600);
    const path = L[0].path.replace(/\.[^./]+$/, "") + " (拼接).mp4";
    const nid = addChild(ctx, L[0], {
      path,
      fmt: "MP4",
      type: "video",
      title: `${L[0].title} 等 ${L.length} 段`,
      durS: L.reduce((s, a) => s + (a.durS ?? 0), 0),
      bytes: L.reduce((s, a) => s + a.bytes, 0),
    });
    return { path, assetId: nid };
  },
};
