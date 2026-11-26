// 工具栏面板的演示后台：只在演示构建（vite build -c vite.popup.config.ts --mode mock）里，普通浏览器带 ?mock 打开时用。
// 回的结构和 extension/background.js 一样（state / jobs / probe / check / download / stop / qual / settings / open / share / dims）。
// 网址参数：
//   conn=off|nohost|broken   没开、没装、连不上（broken 时 reason=exited|forbidden）
//   scene=design|empty|page|many   design 照设计稿的三条；empty 这个页面什么都没有；page 整页交给下载引擎的网站（YouTube）；many 很多条
//   still                    进度停住（截图用）
//   loginoff                 「带上这个网站的登录状态」关着
//   rec / rec=saving         多一条正在录的直播（已录 12:34，「停止并保存」「取消录制」）/ 正在保存录到的部分
// 图片是开放电影的剧照（CC BY，来源见 src/dev/img/CREDITS.md）。
import sintel1 from "@/dev/img/sintel-1.jpg";
import sintel3 from "@/dev/img/sintel-3.jpg";
import tos1 from "@/dev/img/tos-01.jpg";
import tos5 from "@/dev/img/tos-05.jpg";
import bbb from "@/dev/img/bbb-frank.jpg";
import type { Check, Dl, Item, JobLite, PopupError, Settings, State } from "./model";
import { localizer } from "@/dev/mock-i18n";

type Msg = { t: string; tabId: number | null; [k: string]: unknown };

const MB = 1e6;

export function createMock(q: URLSearchParams) {
  const scene = q.get("scene") || "design";
  const still = q.has("still");
  const conn = (q.get("conn") as State["conn"]) || "ok";
  const settings: Settings = { inj: true, intro: true, qpref: "" };
  let items: Item[] = [];
  const dl: Record<string, Dl> = {};
  const checks: Record<string, Check> = {};
  const jobs: Record<number, JobLite> = {};
  let page = { title: "Sintel · 完整版短片", url: "https://video.example.org/films/sintel", favIconUrl: "" };
  let seq = 20;
  // 「带上这个网站的登录状态」：按网站记；?loginoff 时这个网站关着
  const login = { site: "", on: !q.has("loginoff") };

  if (scene === "design" || scene === "many") {
    items = [
      {
        id: "i1",
        src: "sniff",
        kind: "video",
        hls: true,
        q: true,
        name: "Sintel · 完整版短片",
        url: "https://video.example.org/hls/sintel/master.m3u8",
        host: "video.example.org",
        dur: "14:48",
        img: sintel1,
        variants: [
          { p: "1080p", h: 1080, uri: "https://video.example.org/hls/sintel/1080.m3u8", bytes: 312 * MB, audio: false },
          { p: "720p", h: 720, uri: "https://video.example.org/hls/sintel/720.m3u8", bytes: 158 * MB, audio: false },
          { p: "480p", h: 480, uri: "https://video.example.org/hls/sintel/480.m3u8", bytes: 74 * MB, audio: false },
        ],
      },
      { id: "i2", src: "sniff", kind: "video", name: "Tears of Steel · 预告片", url: "https://video.example.org/media/tos-trailer.mp4", ext: "mp4", size: 64 * MB, host: "video.example.org", dur: "1:34", img: tos5 },
      { id: "i3", src: "sniff", kind: "image", name: "Sintel_poster.jpg", url: sintel3, ext: "jpg", size: 412_000, host: "video.example.org", dims: [1920, 1080] },
    ];
    dl.i2 = { job: 12, ch: { q: "best" } };
    jobs[12] = { id: 12, title: "Tears of Steel · 预告片", state: "run", phase: "dl", got: 29.4, size: 64, fmt: "MP4" };
    checks.i3 = { found: true, assetId: 7, title: "Sintel_poster.jpg", where: "图片 › Captae", kind: "image" };
    if (q.has("rec")) {
      // 录直播：从插件里点了「从现在开始录」的那一条
      const saving = q.get("rec") === "saving";
      items.unshift({ id: "live", src: "sniff", kind: "video", hls: true, name: "直播：开放电影放映会", url: "https://video.example.org/live/stream.m3u8", host: "video.example.org", img: tos1 });
      dl.live = { job: 30, ch: {} };
      jobs[30] = {
        id: 30,
        title: "直播：开放电影放映会",
        state: "run",
        phase: saving ? "merge" : "dl",
        got: 41.2,
        size: 0,
        fmt: "MP4",
        live: true,
        rec: saving ? { since: Date.now() - 754_000, until: Date.now() - 2000, stopping: true } : { since: Date.now() - 754_000 },
      };
    }
    if (scene === "many") {
      for (let k = 0; k < 9; k++)
        items.push({ id: "m" + k, src: "sniff", kind: k % 3 ? "image" : "audio", name: k % 3 ? `still-${k + 1}.jpg` : `track-${k + 1}.mp3`, url: k % 3 ? tos1 : "https://video.example.org/a.mp3", ext: k % 3 ? "jpg" : "mp3", size: (k + 2) * 210_000, img: k % 3 ? (k % 2 ? tos1 : bbb) : undefined, dims: k % 3 ? [1600, 900] : undefined });
      dl.m0 = { job: 13, ch: {} };
      jobs[13] = { id: 13, title: "track-1.mp3", state: "failed", phase: "dl", got: 0, size: 0, err: "网络不稳定，没连上对方网站", fail: { code: "network", message: "网络不稳定，没连上对方网站" } };
    }
  } else if (scene === "page") {
    page = { title: "Big Buck Bunny 60fps 4K - Official Blender Foundation Short Film - YouTube", url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ", favIconUrl: "" };
    items = [
      { id: "page", src: "page", kind: "video", q: true, name: "Big Buck Bunny 60fps 4K - Official Blender Foundation Short Film", url: page.url, site: "YouTube", host: "www.youtube.com", img: bbb },
      { id: "cover", src: "cover", kind: "image", name: "视频封面", url: bbb, ext: "jpg", host: "i.ytimg.com", dims: [1280, 720] },
    ];
  } else {
    page = { title: "关于我们 · Example", url: "https://example.org/about", favIconUrl: "" };
  }

  login.site = new URL(page.url).hostname.split(".").slice(-2).join(".");
  const err = (code: string, reason?: string) => Object.assign(new Error(code), { code, reason }) as PopupError;

  const answer = async (m: Msg): Promise<unknown> => {
    await new Promise((r) => setTimeout(r, 40));
    switch (m.t) {
      case "state":
        return { conn, reason: conn === "broken" ? q.get("reason") || "exited" : undefined, items, dl, qual: {}, settings, login, page } satisfies State;
      case "login":
        login.on = m.on !== false;
        return login;
      case "jobs": {
        const ids = m.ids as number[];
        return ids.map((id) => jobs[id]).filter(Boolean);
      }
      case "probe":
        await new Promise((r) => setTimeout(r, 900));
        return {
          kind: "page",
          title: items[0]?.name,
          duration: 634,
          thumbnail: bbb,
          qualities: [
            { label: "2160p", height: 2160, bytes: 1.21e9 },
            { label: "1440p", height: 1440, bytes: 640 * MB },
            { label: "1080p", height: 1080, bytes: 318 * MB },
            { label: "720p", height: 720, bytes: 152 * MB },
            { label: "480p", height: 480, bytes: 71 * MB },
          ],
          audio: 10 * MB,
        };
      case "check":
        return checks[m.itemId as string] ?? { found: false };
      case "download": {
        if (conn !== "ok") throw err("APP_NOT_RUNNING");
        const id = ++seq;
        const it = items.find((i) => i.id === m.itemId);
        jobs[id] = { id, title: it?.name || "", state: "run", phase: "probe", got: 0, size: 0, fmt: "MP4" };
        dl[m.itemId as string] = { job: id, ch: (m.ch as Dl["ch"]) || {} };
        setTimeout(() => Object.assign(jobs[id], { phase: "dl", size: 120 }), 1200);
        return jobs[id];
      }
      case "save": {
        // 停止并保存：先「正在保存」，一会儿以后录好了
        const j = jobs[m.job as number];
        if (!j?.rec) throw new Error("只有正在录制的直播才能「停止并保存」");
        Object.assign(j, { phase: "merge", rec: { ...j.rec, stopping: true, until: Date.now() } });
        setTimeout(() => Object.assign(j, { state: "done", phase: "done", assetId: 98, rec: { ...j.rec, stopping: false, secs: (Date.now() - j.rec!.since) / 1000 } }), 1500);
        return true;
      }
      case "stop": {
        const d = dl[m.itemId as string];
        if (d) delete jobs[d.job];
        delete dl[m.itemId as string];
        return true;
      }
      case "open":
      case "qual":
      case "dims":
      case "share":
        return true;
      case "settings":
        Object.assign(settings, m.patch);
        return settings;
      default:
        throw new Error("演示后台不认识 " + m.t);
    }
  };

  // 进度往前走（still 时停住）
  if (!still)
    setInterval(() => {
      for (const j of Object.values(jobs)) {
        if (j.state !== "run" || j.phase !== "dl" || !j.size) continue;
        j.got = Math.min(j.size, j.got + j.size * 0.03);
        if (j.got >= j.size) Object.assign(j, { state: "done", phase: "done", assetId: 99 });
      }
    }, 800);

  // 官网嵌演示版、按语言拍截图时带 &lang=en 之类：交给面板的数据换成这种语言，中文原样（src/dev/mock-i18n.ts，和桌面端演示版同一份译文表）
  const loc = localizer(q.get("lang"));
  const send = async (m: Msg): Promise<unknown> => loc(await answer(m));
  return { send, tell: (m: unknown) => console.info("网页收到", m) };
}
