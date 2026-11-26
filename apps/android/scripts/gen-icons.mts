// 图标生成：SF Symbol 名 → Hugeicons Free 图标，生成 glass/src/main/kotlin/app/captae/glass/icons/Symbols.kt；
// 顺带把桌面端的网站图标（apps/desktop/src/assets/img/*.svg，Simple Icons，CC0）转成 app 的 VectorDrawable。
//
// 为什么：SF Symbols 只许在苹果平台用。安卓用桌面端同款 Hugeicons Free（MIT），页面里照 iOS 写 SF 名
// （Icon(Sym.arrowDownCircle) / Icon("arrow.down.circle")），两边说法和图标一一对应。
// 做法照 medcxy-v7 的 scripts/gen-icons.mts（那边是 Lucide）。
//
// 缺图标就在下面 TABLE 里加一行再跑（Node 22.6+ 直接跑 .mts）：
//   node apps/android/scripts/gen-icons.mts
// Hugeicons 从桌面端的 node_modules 读（apps/desktop 里先 npm install）；装在别处时用 HUGEICONS_DIR 指过去。
// 生成文件入库。
import { existsSync, readFileSync, writeFileSync, readdirSync, mkdirSync } from "node:fs";
import { dirname, join, resolve, basename } from "node:path";
import { fileURLToPath } from "node:url";

// SF 名 → Hugeicons 名（去掉 Icon 后缀前的那个导出名）。和桌面端同义的地方用桌面端那一个
const TABLE: Record<string, string> = {
  // 底部标签（BRIEF 2.2）：形状照 iOS 的 SF Symbol 选（2026-11-24 用户对照 iOS 定）：
  // 下载是圆圈里一个向下箭头（arrow.down.circle），素材库是两张叠着、一张斜放的照片（photo.on.rectangle.angled），搜索是放大镜
  "arrow.down.circle": "DownloadCircle01Icon",
  // photo.on.rectangle.angled 在下面 CUSTOM 里自己画（Hugeicons 没有前横后斜两张叠放的照片）
  laptopcomputer: "LaptopIcon",
  // 导航栏、菜单
  ellipsis: "MoreHorizontalIcon",
  "chevron.left": "ArrowLeft01Icon",
  "chevron.right": "ArrowRight01Icon",
  "chevron.down": "ArrowDown01Icon",
  "chevron.up": "ArrowUp01Icon",
  "chevron.up.chevron.down": "UnfoldMoreIcon",
  xmark: "Cancel01Icon",
  checkmark: "Tick02Icon",
  gearshape: "Settings02Icon",
  magnifyingglass: "Search01Icon",
  // 任务操作
  pause: "PauseIcon",
  play: "PlayIcon",
  stop: "StopIcon",
  "arrow.clockwise": "RefreshIcon",
  "arrow.up": "ArrowUp02Icon",
  "doc.on.clipboard": "ClipboardIcon",
  "doc.on.doc": "Copy01Icon",
  // 状态行的小图标
  "exclamationmark.circle": "AlertCircleIcon",
  "exclamationmark.triangle": "Alert02Icon",
  "checkmark.circle": "CheckmarkCircle02Icon",
  "xmark.circle": "CancelCircleIcon",
  lock: "SquareLock02Icon",
  clock: "Clock01Icon",
  "wifi.slash": "WifiDisconnected01Icon",
  wifi: "Wifi01Icon",
  // 素材
  globe: "Globe02Icon",
  link: "Link01Icon",
  film: "FileVideoIcon",
  // 桌面端「M3U8 测试流」、侧栏「浏览器里发现的」播放列表
  "list.and.film": "PlayListIcon",
  video: "Video01Icon",
  "music.note": "MusicNote01Icon",
  photo: "Image01Icon",
  waveform: "AudioWave01Icon",
  folder: "Folder01Icon",
  trash: "Delete02Icon",
  "square.and.arrow.up": "Share08Icon",
  "square.and.arrow.down": "Download04Icon",
  "arrow.up.forward.app": "LinkSquare02Icon",
  tag: "Tag01Icon",
  paperplane: "SentIcon",
  // 电脑、配对
  iphone: "SmartPhone01Icon",
  // 设备菜单（BRIEF 2.1，和 iOS 同名）：全部设备、管理电脑
  "square.stack": "SquareStackIcon",
  "laptopcomputer.and.iphone": "LaptopPhoneSyncIcon",
  "qrcode.viewfinder": "QrCodeIcon",
  "viewfinder": "ScanIcon",
  keyboard: "KeyboardIcon",
  plus: "PlusSignIcon",
  // 设置、关于
  "info.circle": "InformationCircleIcon",
  "chevron.left.forwardslash.chevron.right": "SourceCodeIcon",
  "doc.text": "LicenseIcon",
};

// 实心版（SF 的 .fill）：Hugeicons Free 只有描边，实心版用同一个图标改出来。
// fill：这几条路径填实（描边照留，外形和描边版一样大）；knock：这几条从填实的地方挖掉（画面里的线、点变成镂空，同 SF 的实心符号）；
// drop：不要的路径。标签栏选中的那个用实心版（BRIEF 2.2）。
const FILLED: Record<string, { base: string; fill: number[]; knock?: number[]; drop?: number[] }> = {
  // 下载：圆圈填实，里面的箭头挖空（同 SF 的 arrow.down.circle.fill）
  "arrow.down.circle.fill": { base: "arrow.down.circle", fill: [0], knock: [1] },
  // 任务卡、下载条的暂停 / 继续（BRIEF 2.8 第 2 条：实心两竖条，同 SF 的 pause.fill、play.fill）
  "pause.fill": { base: "pause", fill: [0, 1] },
  "play.fill": { base: "play", fill: [0] },
};

type CustomPath = { d: string; fill?: boolean; stroke?: boolean; knockout?: boolean };
/**
 自己画的图标（Hugeicons Free 里没有对得上 iOS 的）：24 格、圆头圆角，线宽和 Hugeicons 一样由 CaptaeIcons 定，看上去是同一套。
 photo.on.rectangle.angled（素材库标签，2026-11-24 用户要求和 iOS 一样）：前面一张横放的照片（太阳 + 两座山），
 后面一张往左倾斜约 8° 的照片，只露出左边和上边。描边版后面那张只画露出来的线，离前面那张留 2.2 的空；
 实心版两张都填实，前面那张外面挖一圈空隙（同 SF 的实心符号），太阳和山挖空。
 */
const PHOTO_FRONT = "M10 8.5H20A2.5 2.5 0 0 1 22.5 11V18A2.5 2.5 0 0 1 20 20.5H10A2.5 2.5 0 0 1 7.5 18V11A2.5 2.5 0 0 1 10 8.5Z";
// 后面那张：左上角 (1.6, 5.0)，宽 13.5、高 11.5，转 -8°，圆角 2
const PHOTO_BACK_FULL = "M3.58 4.72L12.99 3.40Q14.97 3.12 15.25 5.10L16.29 12.53Q16.57 14.51 14.59 14.79L5.18 16.11Q3.20 16.39 2.92 14.41L1.88 6.98Q1.60 5.00 3.58 4.72Z";
const PHOTO_BACK_VISIBLE = "M15.42 6.3L15.25 5.10Q14.97 3.12 12.99 3.40L3.58 4.72Q1.60 5.00 1.88 6.98L2.92 14.41Q3.20 16.39 5.18 16.11L5.3 16.09";
const PHOTO_SUN = "M10 12.1a1.3 1.3 0 1 0 2.6 0a1.3 1.3 0 1 0 -2.6 0z";
const PHOTO_HILLS = "M8 18.8L11.6 16Q12.1 15.6 12.6 16L14.2 17.3L17.3 14.3Q17.8 13.85 18.3 14.3L22 17.9";
// 实心版挖空的山：山脊线以下、离卡片内边一点的一块
const PHOTO_HILLS_FILL = "M9.1 19.1L11.6 17.1Q12.1 16.7 12.6 17.1L14.2 18.3L17.3 15.4Q17.8 14.95 18.3 15.4L21.1 18.1V19.1Z";
// 前面那张往外扩 1.9 的轮廓：用线宽挖出前后两张之间的空隙
const PHOTO_FRONT_GAP = "M10 6.6H20A4.4 4.4 0 0 1 24.4 11V18A4.4 4.4 0 0 1 20 22.4H10A4.4 4.4 0 0 1 5.6 18V11A4.4 4.4 0 0 1 10 6.6Z";
const CUSTOM: Record<string, { name: string; paths: CustomPath[] }> = {
  "photo.on.rectangle.angled": {
    name: "CaptaePhotoStack",
    paths: [{ d: PHOTO_BACK_VISIBLE }, { d: PHOTO_FRONT }, { d: PHOTO_SUN }, { d: PHOTO_HILLS }],
  },
  "photo.on.rectangle.angled.fill": {
    name: "CaptaePhotoStackFilled",
    paths: [
      { d: PHOTO_BACK_FULL, fill: true },
      { d: PHOTO_FRONT, fill: true },
      { d: PHOTO_FRONT_GAP, knockout: true },
      { d: PHOTO_SUN, fill: true, stroke: false, knockout: true },
      { d: PHOTO_HILLS_FILL, fill: true, stroke: false, knockout: true },
    ],
  },
};

const here = dirname(fileURLToPath(import.meta.url));
const androidDir = resolve(here, "..");
const repo = resolve(androidDir, "../..");

function hugeDir(): string {
  const candidates = [process.env.HUGEICONS_DIR, join(repo, "apps/desktop/node_modules/@hugeicons/core-free-icons")].filter(Boolean) as string[];
  for (const c of candidates) if (existsSync(join(c, "dist/esm"))) return c;
  throw new Error("找不到 @hugeicons/core-free-icons：先在 apps/desktop 里 npm install，或设 HUGEICONS_DIR");
}

const dir = hugeDir();
const version = JSON.parse(readFileSync(join(dir, "package.json"), "utf8")).version as string;

type Attrs = Record<string, string>;
type Node = [string, Attrs];

/** 读一个图标：不 import（ESM 文件就是一个数组字面量），直接从源码里取出数组 */
function huge(name: string): Node[] {
  const file = join(dir, "dist/esm", `${name}.js`);
  if (!existsSync(file)) throw new Error(`Hugeicons 没有 ${name}`);
  const src = readFileSync(file, "utf8");
  const m = src.match(/=\s*(\[[\s\S]*\]);\s*\n/);
  if (!m) throw new Error(`读不懂 ${name}.js`);
  return Function(`return ${m[1]}`)() as Node[];
}

const n = (v: string | undefined, d = 0) => (v === undefined ? d : Number(v));
/** SVG 元素 → 一条路径数据（24×24 格） */
function toPath([tag, a]: Node): string {
  switch (tag) {
    case "path":
      return a.d;
    case "circle": {
      const cx = n(a.cx), cy = n(a.cy), r = n(a.r);
      return `M${cx - r} ${cy}a${r} ${r} 0 1 0 ${2 * r} 0a${r} ${r} 0 1 0 ${-2 * r} 0z`;
    }
    case "ellipse": {
      const cx = n(a.cx), cy = n(a.cy), rx = n(a.rx), ry = n(a.ry);
      return `M${cx - rx} ${cy}a${rx} ${ry} 0 1 0 ${2 * rx} 0a${rx} ${ry} 0 1 0 ${-2 * rx} 0z`;
    }
    case "rect": {
      const x = n(a.x), y = n(a.y), w = n(a.width), h = n(a.height);
      let rx = n(a.rx, n(a.ry)), ry = n(a.ry, rx);
      rx = Math.min(rx, w / 2);
      ry = Math.min(ry, h / 2);
      if (!rx) return `M${x} ${y}h${w}v${h}h${-w}z`;
      return `M${x + rx} ${y}h${w - 2 * rx}a${rx} ${ry} 0 0 1 ${rx} ${ry}v${h - 2 * ry}a${rx} ${ry} 0 0 1 ${-rx} ${ry}h${-(w - 2 * rx)}a${rx} ${ry} 0 0 1 ${-rx} ${-ry}v${-(h - 2 * ry)}a${rx} ${ry} 0 0 1 ${rx} ${-ry}z`;
    }
    case "line":
      return `M${a.x1} ${a.y1}L${a.x2} ${a.y2}`;
    case "polyline":
    case "polygon": {
      const pts = a.points.trim().split(/[\s,]+/).map(Number);
      let d = `M${pts[0]} ${pts[1]}`;
      for (let i = 2; i < pts.length; i += 2) d += `L${pts[i]} ${pts[i + 1]}`;
      return tag === "polygon" ? d + "z" : d;
    }
    default:
      throw new Error(`不认识的 SVG 元素 ${tag}`);
  }
}

const kt = (s: string) => JSON.stringify(s);
const camel = (sf: string) =>
  sf
    .split(".")
    .map((s, i) => (i === 0 ? s : s[0].toUpperCase() + s.slice(1)))
    .join("")
    .replace(/^(\d)/, "_$1");

const entries: string[] = [];
const names: string[] = [];
const used = new Set<string>();
for (const [sf, name] of Object.entries(TABLE).sort(([a], [b]) => a.localeCompare(b))) {
  const nodes = huge(name);
  used.add(name);
  const paths = nodes.map((node) => {
    const a = node[1];
    // 描边图标：stroke="currentColor"；少数部件是实心的（fill="currentColor"），两种可以同时有
    const stroke = a.stroke !== undefined && a.stroke !== "none";
    const fill = a.fill !== undefined && a.fill !== "none";
    const args = [kt(toPath(node))];
    if (!stroke) args.push("stroke = false");
    if (fill) args.push("fill = true");
    if (a.fillRule === "evenodd" || a.clipRule === "evenodd") args.push("evenOdd = true");
    if (a.opacity !== undefined) args.push(`alpha = ${Number(a.opacity)}f`);
    return `HugePath(${args.join(", ")})`;
  });
  entries.push(`    ${kt(sf)} to HugeSpec(${kt(name)}, listOf(${paths.join(", ")})),`);
  names.push(sf);
}

// 实心版
for (const [sf, spec] of Object.entries(FILLED).sort(([a], [b]) => a.localeCompare(b))) {
  const name = TABLE[spec.base];
  if (!name) throw new Error(`${sf} 的底图 ${spec.base} 不在 TABLE 里`);
  const nodes = huge(name);
  const paths: string[] = [];
  nodes.forEach((node, i) => {
    if (spec.drop?.includes(i)) return;
    const args = [kt(toPath(node))];
    if (spec.fill.includes(i)) args.push("fill = true");
    if (spec.knock?.includes(i)) args.push("knockout = true");
    paths.push(`HugePath(${args.join(", ")})`);
  });
  entries.push(`    ${kt(sf)} to HugeSpec(${kt(name + "Filled")}, listOf(${paths.join(", ")})),`);
  names.push(sf);
}

// 自己画的
for (const [sf, spec] of Object.entries(CUSTOM).sort(([a], [b]) => a.localeCompare(b))) {
  if (TABLE[sf] || FILLED[sf]) throw new Error(`${sf} 在 CUSTOM 和 TABLE / FILLED 里都有`);
  const paths = spec.paths.map((p) => {
    const args = [kt(p.d)];
    if (p.stroke === false) args.push("stroke = false");
    if (p.fill) args.push("fill = true");
    if (p.knockout) args.push("knockout = true");
    return `HugePath(${args.join(", ")})`;
  });
  entries.push(`    ${kt(sf)} to HugeSpec(${kt(spec.name)}, listOf(${paths.join(", ")})),`);
  names.push(sf);
}
names.sort((a, b) => a.localeCompare(b));

const out = `// 生成文件，别手改：apps/android/scripts/gen-icons.mts
// 图标来自 Hugeicons Free（@hugeicons/core-free-icons ${version}），MIT 许可，见 glass/src/main/assets/licenses/Hugeicons-MIT.md。
// 共 ${names.length} 个 SF Symbol 名，用到 ${used.size} 个 Hugeicons 图标。
@file:Suppress("SpellCheckingInspection")

package app.captae.glass.icons

/** 页面里用的 SF Symbol 名（和 iOS 版同名；写错名字编译就报错） */
object Sym {
${names.map((s) => `  const val ${camel(s)} = ${kt(s)}`).join("\n")}
}

internal object SymbolTable {
  val specs: Map<String, HugeSpec> = mapOf(
${entries.join("\n")}
  )
}
`;
const target = join(androidDir, "glass/src/main/kotlin/app/captae/glass/icons/Symbols.kt");
writeFileSync(target, out);

// 许可证随包带上
const licenses = join(androidDir, "glass/src/main/assets/licenses");
mkdirSync(licenses, { recursive: true });
const lic = ["LICENSE.md", "LICENSE"].map((f) => join(dir, f)).find(existsSync);
if (lic) writeFileSync(join(licenses, "Hugeicons-MIT.md"), readFileSync(lic, "utf8"));

// ---------------- 网站图标：SVG → VectorDrawable ----------------
// 桌面端 parts/index.tsx 的 SITE 表用的那几个（Simple Icons，CC0）。都是 24×24 的单条路径，颜色照原样
const siteDir = join(repo, "apps/desktop/src/assets/img");
const drawable = join(androidDir, "app/src/main/res/drawable");
mkdirSync(drawable, { recursive: true });
const sites: string[] = [];
for (const f of readdirSync(siteDir).filter((x) => x.endsWith(".svg")).sort()) {
  const svg = readFileSync(join(siteDir, f), "utf8");
  const vb = svg.match(/viewBox="([\d.\s-]+)"/)?.[1].trim().split(/\s+/).map(Number) ?? [0, 0, 24, 24];
  const paths = [...svg.matchAll(/<path([^>]*)\/?>/g)].map((m) => {
    const attr = (k: string) => m[1].match(new RegExp(`\\s${k}="([^"]*)"`))?.[1];
    const d = attr("d");
    if (!d) throw new Error(`${f} 里有没有 d 的 path`);
    // 没写颜色的（X、TikTok）是黑色字形
    const color = attr("fill") ?? "#000000";
    const rule = attr("fill-rule") === "evenodd" ? `\n    android:fillType="evenOdd"` : "";
    return `  <path\n    android:fillColor="${color}"${rule}\n    android:pathData="${d}" />`;
  });
  const name = `site_${basename(f, ".svg").replace(/[^a-z0-9]/gi, "_").toLowerCase()}`;
  const xml = `<?xml version="1.0" encoding="utf-8"?>
<!-- 生成文件，别手改：apps/android/scripts/gen-icons.mts。来源 apps/desktop/src/assets/img/${f}（Simple Icons，CC0） -->
<vector xmlns:android="http://schemas.android.com/apk/res/android"
  android:width="24dp"
  android:height="24dp"
  android:viewportWidth="${vb[2]}"
  android:viewportHeight="${vb[3]}">
${paths.join("\n")}
</vector>
`;
  writeFileSync(join(drawable, `${name}.xml`), xml);
  sites.push(name);
}

console.log(`写好 ${target}：${names.length} 个符号，${used.size} 个 Hugeicons 图标（${version}）；网站图标 ${sites.join("、")}`);
