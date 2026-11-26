// 自己写的小部件：affectUI 免费组件里没有的（logo、进度条、步骤时间线、倒计时、状态互换、站点图标、纸张格式图标）。
// 动效数值都来自课程，注释里的「配方 x」对应 docs/MOTION.md 的小节。
import { HugeiconsIcon } from "@hugeicons/react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useId, useState, type ReactNode } from "react";
import { IconButton, type IconButtonProps } from "@/components/ui/icon-button";
import { Tooltip } from "@/components/ui/tooltip";
import { t } from "@/i18n";
import { fileSrc } from "@/lib/bridge";
import { clock, since } from "@/lib/format";
import bilibili from "@/assets/img/bilibili.svg";
import instagram from "@/assets/img/instagram.svg";
import tiktok from "@/assets/img/tiktok.svg";
import x from "@/assets/img/x.svg";
import youtube from "@/assets/img/youtube.svg";

// Hugeicons 的图标对象（数组），affectUI 组件也接受这种格式
export type HugeIcon = readonly (readonly [string, { readonly [key: string]: string | number }])[];

export function Icon({ icon, size = 16, stroke = 1.75, className }: { icon: HugeIcon; size?: number; stroke?: number; className?: string }) {
  return <HugeiconsIcon icon={icon as never} size={size} strokeWidth={stroke} className={className} />;
}

/** 取景框四角 + 中间一个圆：和应用图标、插件里的标是同一个 */
const MARK = "M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3";

/** Captae 的 logo：黑色圆角方块 + 白色取景框和圆点 */
export function CaptaeLogo({ size = 22 }: { size?: number }) {
  // 页面上可能同时有好几个 logo（侧栏、空页面），渐变的 id 要各不相同；useId 的结果带特殊字符，去掉再用
  const gid = "cp-logo-" + useId().replace(/[^a-zA-Z0-9_-]/g, "");
  return (
    <svg width={size} height={size} viewBox="0 0 64 64" aria-hidden style={{ flex: "none", display: "block" }}>
      <defs>
        <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#2e2e30" />
          <stop offset="1" stopColor="#0e0e0f" />
        </linearGradient>
      </defs>
      <path d="M0 18C0 5.4 5.4 0 18 0h28c12.6 0 18 5.4 18 18v28c0 12.6-5.4 18-18 18H18C5.4 64 0 58.6 0 46Z" fill={`url(#${gid})`} />
      <g transform="translate(14.6 14.6) scale(1.45)" fill="none" stroke="#fff" strokeWidth="2.35" strokeLinecap="round" strokeLinejoin="round">
        <path d={MARK} />
        <circle cx="12" cy="12" r="2.6" />
      </g>
    </svg>
  );
}

/* ---------------- 站点图标（Simple Icons，CC0） ---------------- */

const SITE: [RegExp, string][] = [
  [/youtube|youtu\.be/, youtube],
  [/bilibili|b23\.tv/, bilibili],
  [/douyin|tiktok/, tiktok],
  // plat 是「x」；网址里是 x.com 前面跟着 / 或 .（不能误认 box.com 之类）
  [/^x$|(^|[/.])x\.com|twitter/, x],
  [/instagram/, instagram],
];

/** 按 plat（youtube、bilibili……）或网址找站点图标，找不到返回空 */
export function siteIconOf(key?: string | null): string | undefined {
  if (!key) return undefined;
  const k = key.toLowerCase();
  return SITE.find(([re]) => re.test(k))?.[1];
}

export function SiteIcon({ site, size = 13 }: { site?: string | null; size?: number }) {
  const src = siteIconOf(site);
  if (src) return <img className="cp-site-ico" src={src} alt="" width={size} height={size} />;
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" aria-hidden style={{ flex: "none" }}>
      <circle cx="12" cy="12" r="9" />
      <path d="M3 12h18M12 3c2.5 3 2.5 15 0 18M12 3c-2.5 3-2.5 15 0 18" />
    </svg>
  );
}

/* ---------------- 缩略图 ---------------- */

/** 封面：有图显示图（读到封面时 opacity + 3px 模糊淡入 0.3s，配方 j3）；没有图显示站点图标 */
export function Thumb({ src, dur, site, className = "cp-thumb" }: { src?: string | null; dur?: string; site?: string | null; className?: string }) {
  const url = fileSrc(src);
  if (!url) {
    const ico = siteIconOf(site);
    return (
      <div className={`${className} is-empty`}>
        {ico ? (
          <img src={ico} alt="" />
        ) : (
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.6" aria-hidden>
            <rect x="3" y="5" width="18" height="14" rx="3" />
            <path d="M10 9.5v5l4-2.5-4-2.5Z" fill="currentColor" stroke="none" />
          </svg>
        )}
      </div>
    );
  }
  return (
    <div className={className}>
      <Cover key={url} url={url} />
      {dur && <span className="dur">{dur}</span>}
    </div>
  );
}

/** 这次打开以来已经读到过的封面地址：同一张图在列表行里读到了，点开详情时就不用再淡入 */
const shownCovers = new Set<string>();

/** 封面图：真的读到的那一刻才淡入（theme.css 的 .cp-cover）。已经读到过的（换详情、换页签、重新挂载）第一帧就直接显示，
 *  不再跟着卡片又淡入一遍（一个容器只做一次入场，课 01/10）；读不出来的保持透明，露出灰底，不显示破图标。
 *  「读到过」在渲染时就判断：等挂上去以后再 setState，同一轮里别的组件量布局时浏览器已经按透明算过一次样式，过渡照样会播 */
function Cover({ url }: { url: string }) {
  const [on, setOn] = useState(() => shownCovers.has(url));
  const show = useCallback(() => {
    shownCovers.add(url);
    setOn(true);
  }, [url]);
  // 浏览器缓存里已经有、但没经过这里的：挂上去那一刻直接在元素上标好，不等 React 下一轮
  const ref = useCallback(
    (el: HTMLImageElement | null) => {
      if (el && !el.hasAttribute("data-on") && el.complete && el.naturalWidth) {
        el.setAttribute("data-on", "");
        show();
      }
    },
    [show],
  );
  return <img ref={ref} className="cp-cover" src={url} alt="" draggable={false} referrerPolicy="no-referrer" data-on={on || undefined} onLoad={show} />;
}

/* ---------------- 进度 ---------------- */

/** 进度条：clip-path 填充，linear，过渡时长 = 刷新间隔（配方 c5），不动 width；挂载时不从 0 重播 */
export function ProgressBar({ p, tone, indeterminate, tick = 300, label }: { p: number; tone?: "paused" | "err"; indeterminate?: boolean; tick?: number; label?: string }) {
  const v = Math.max(0, Math.min(1, isFinite(p) ? p : 0));
  return (
    <div
      className="cp-progress"
      role="progressbar"
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={indeterminate ? undefined : Math.round(v * 100)}
      data-tone={tone}
      data-indeterminate={indeterminate || undefined}
      style={{ ["--tick" as string]: `${tick}ms` }}
    >
      <i style={indeterminate ? undefined : { clipPath: `inset(0 ${(1 - v) * 100}% 0 0 round 99px)` }} />
    </div>
  );
}

/** 侧栏、插件面板里的小进度环；busy 时是一个呼吸的圆点 */
export function MiniRing({ p, busy }: { p: number; busy?: boolean }) {
  return (
    <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden style={{ transform: "rotate(-90deg)", flex: "none" }}>
      <circle cx="8" cy="8" r="6" fill="none" stroke="rgba(0,0,0,.12)" strokeWidth="2" />
      {!busy && <circle cx="8" cy="8" r="6" fill="none" stroke="var(--accent)" strokeWidth="2" pathLength={100} strokeDasharray={`${Math.max(0, Math.min(1, p)) * 100} 100`} strokeLinecap="round" />}
      {busy && <circle className="cp-pulse" cx="8" cy="8" r="2.5" fill="var(--ink-3)" />}
    </svg>
  );
}

/** 已经等了多久（2.4s、1m 05s），每 100ms 走一次；asClock 时写成 12:34、按秒走（录直播的「已录」） */
export function Elapsed({ from, asClock }: { from: number; asClock?: boolean }) {
  const [, tick] = useState(0);
  useEffect(() => {
    const t = setInterval(() => tick((n) => n + 1), asClock ? 250 : 100);
    return () => clearInterval(t);
  }, [asClock]);
  return <span className="cp-elapsed num">{asClock ? clock(Math.floor(Math.max(0, Date.now() - from) / 1000)) || "0:00" : since(from)}</span>;
}

/** 「几秒后自动选」：圆环匀速走完（时间在流逝，用 linear，配方 d2）；until 是结束的时刻（毫秒） */
export function Countdown({ until, children }: { until: number; children: (left: number) => ReactNode }) {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(t);
  }, []);
  const left = Math.max(0, Math.ceil((until - now) / 1000));
  // 动画从挂载那一刻按剩余时间走完，之后不再重算（重算会让圆环跳）
  const [total] = useState(() => Math.max(0.2, (until - Date.now()) / 1000));
  // 减少动效：圆环不连续走，按秒跳；直接跟旁边的秒数用同一个值，两者同一刻变（中途重新挂载时也对得上）
  const reduce = useReducedMotion();
  const steps = reduce ? { animation: "none", strokeDashoffset: (1 - left / Math.ceil(total)) * 100 } : undefined;
  return (
    <div className="cp-countdown" style={{ ["--countdown" as string]: `${total}s` }}>
      <svg width="14" height="14" viewBox="0 0 16 16" aria-hidden>
        <circle cx="8" cy="8" r="6" fill="none" stroke="rgba(0,0,0,.12)" strokeWidth="2" />
        <circle className="run" cx="8" cy="8" r="6" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" pathLength={100} style={steps} />
      </svg>
      <span>{children(left)}</span>
    </div>
  );
}

/* ---------------- 步骤时间线 ---------------- */

export type StepState = "done" | "now" | "todo" | "err";
export type Step = { label: string; sub?: ReactNode; s: StepState; fresh?: boolean; trail?: ReactNode };

/** 灰字 + 图标 + 1px 连线；完成时描线画圈再打勾（配方 e3 / m1），已经完成的直接显示终态（fresh=false） */
export function StepTimeline({ steps }: { steps: Step[] }) {
  return (
    <ol className="cp-steps">
      {steps.map((st) => (
        <li key={st.label} className="cp-step" data-s={st.s}>
          <svg className="ico" viewBox="0 0 18 18" fill="none" aria-hidden>
            {st.s === "done" && (
              <>
                <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5" pathLength={100} className={`cp-draw${st.fresh ? " go" : ""}`} />
                <path
                  d="M5.9 9.2l2.1 2.1 4.1-4.4"
                  stroke="currentColor"
                  strokeWidth="1.7"
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  pathLength={100}
                  className={`cp-draw${st.fresh ? " go late" : ""}`}
                />
              </>
            )}
            {st.s === "now" && (
              <>
                <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeOpacity=".35" strokeWidth="1.5" />
                <circle className="cp-pulse" cx="9" cy="9" r="3.4" fill="currentColor" />
              </>
            )}
            {st.s === "todo" && <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5" />}
            {st.s === "err" && (
              <>
                <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5" />
                <path d="M9 5.6v4" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
                <circle cx="9" cy="12.3" r="1" fill="currentColor" />
              </>
            )}
          </svg>
          <div className="body">
            <div className="label">{st.label}</div>
            {st.sub && <div className="sub">{st.sub}</div>}
          </div>
          {st.trail}
        </li>
      ))}
    </ol>
  );
}

/* ---------------- 状态互换 ---------------- */

/** 新状态从上方 8px 带 4px 模糊进来，旧状态向下淡出；弹簧 0.3s、不回弹（配方 g3；课程 STANDARDS 的文字和状态互换值，参考实测 321–350ms）。
 *  popLayout：退场的立刻不占位置，周围的东西同时让位。
 *  位移写成整条 transform 字符串（课 04/02、MOTION 1.6），交给浏览器跑；字符串 transform 不归 MotionConfig 管，减少动效时自己换成只淡入淡出。
 *  动完把 transform 和 filter 清成 none：不在每一行上留一个 blur(0px) 的滤镜层。 */
const SWAP_IN = { opacity: 1, transform: "translateY(0px)", filter: "blur(0px)", transitionEnd: { transform: "none", filter: "none" } };
export function Swap({ k, children, className }: { k: string; children: ReactNode; className?: string }) {
  const reduce = useReducedMotion();
  const off = (y: number) => (reduce ? { opacity: 0 } : { opacity: 0, transform: `translateY(${y}px)`, filter: "blur(4px)" });
  return (
    <div className={className} style={{ position: "relative" }}>
      <AnimatePresence mode="popLayout" initial={false}>
        <motion.div key={k} initial={off(-8)} animate={reduce ? { opacity: 1 } : SWAP_IN} exit={off(8)} transition={{ type: "spring", duration: 0.3, bounce: 0 }}>
          {children}
        </motion.div>
      </AnimatePresence>
    </div>
  );
}

/** 图标互换（暂停↔继续、复制↔已复制）：先出后进，scale .5 + 4px 模糊，100ms（配方 c7）。
 *  和 Swap 一样写成 transform 字符串、减少动效时只淡入淡出、动完清掉 filter。
 *  transform 停在 scale(1)，不清成 none：Motion 从 none 缩到 scale(.5) 会当成从 scale(0) 开始（实测第一帧 0.098） */
const ICON_IN = { opacity: 1, transform: "scale(1)", filter: "blur(0px)", transitionEnd: { filter: "none" } };
export function IconSwap({ k, children }: { k: string; children: ReactNode }) {
  const reduce = useReducedMotion();
  const hidden = reduce ? { opacity: 0 } : { opacity: 0, transform: "scale(0.5)", filter: "blur(4px)" };
  return (
    <span style={{ display: "grid", placeItems: "center" }}>
      <AnimatePresence mode="wait" initial={false}>
        <motion.span key={k} initial={hidden} animate={reduce ? { opacity: 1 } : ICON_IN} exit={hidden} transition={{ duration: 0.1 }} style={{ display: "grid" }}>
          {children}
        </motion.span>
      </AnimatePresence>
    </span>
  );
}

/* ---------------- 纸张格式图标（brand-spec v2 的 file3d，按格式着色） ---------------- */

const fmtTone = (f: string) => {
  f = f.toUpperCase();
  if (/PRORES|MOV/.test(f)) return "slate";
  if (/WEBM|MKV/.test(f)) return "web";
  if (/MP3|AAC|FLAC|WAV|M4A|OPUS/.test(f)) return "audio";
  if (/SRT|VTT|TXT|JSON|ASS/.test(f)) return "text";
  if (/PNG|JPG|JPEG|WEBP|AVIF|GIF/.test(f)) return "image";
  if (/M3U8|HLS/.test(f)) return "stream";
  return "video";
};

export function FileIcon({ fmt, size }: { fmt: string; size?: "lg" }) {
  const label = fmt.replace(/ 422$/, "").replace("ProRes", "PRORES").toUpperCase();
  return <span className="cp-file" data-ext={label} data-tone={fmtTone(fmt)} data-size={size} role="img" aria-label={t("{fmt} 文件", { fmt })} />;
}

/* ---------------- 带提示的图标按钮 ---------------- */

/** affectUI 的 IconButton 外面套 affectUI 的 Tooltip：首个提示等 500ms，300ms 内移到相邻按钮立刻出现、不做动画（配方 f3）。
 *  IconButton 自带的 title 是系统提示，慢而且样式不一样，所以关掉 */
export function TipIcon({ tip, ...p }: IconButtonProps & { tip?: string }) {
  return (
    <Tooltip content={tip ?? p.label}>
      <IconButton title="" {...p} />
    </Tooltip>
  );
}

/* ---------------- 展开和收起 ---------------- */

/** 高度从 0 到内容高度：0.27s、课程 05/04 的高度曲线；收起更短（退场规则 1.4）。减少动效时直接到位 */
// 减少动效时：不动高度（不做尺寸动画），只 150ms 淡入淡出（课程：减少动效是「更温和」，不是全部去掉；透明度用 ease，配方 1.1）。
// 展开时内容直接占好位置再淡入；收起时原地淡出，淡完元素移除，空间才收起（高度要是跟着变成 0，内容一下子被裁掉，看不到淡出）。
const FADE = { duration: 0.15, ease: [0.25, 0.1, 0.25, 1] } as const;

export function Collapse({ open, children }: { open: boolean; children: ReactNode }) {
  const reduce = useReducedMotion();
  return (
    <AnimatePresence initial={false}>
      {open && (
        <motion.div
          style={{ overflow: "hidden" }}
          initial={reduce ? { opacity: 0 } : { height: 0, opacity: 0 }}
          animate={reduce ? { opacity: 1, transition: FADE } : { height: "auto", opacity: 1, transition: { duration: 0.27, ease: [0.25, 1, 0.5, 1] } }}
          exit={reduce ? { opacity: 0, transition: FADE } : { height: 0, opacity: 0, transition: { duration: 0.2, ease: [0.23, 1, 0.32, 1] } }}
        >
          {children}
        </motion.div>
      )}
    </AnimatePresence>
  );
}
