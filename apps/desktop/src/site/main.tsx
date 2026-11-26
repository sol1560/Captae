// Captae 官网落地页。视觉照 docs/design/README.md（轻玻璃），动效照 docs/MOTION.md 和课程里讲营销页的几节：
// - 底下一层是「素材」：开放电影剧照排成的素材墙、真实界面截图；上面浮着和桌面端同一套的轻玻璃界面片段（输入框、任务卡、插件按钮、进度卡片）。
//   玻璃只给浮在内容上的东西，正文、卡片不透明；没有渐变大头图、没有衬线字、标题不配装饰图标。
// - 首屏是标题和下载按钮（desk.tsx）；下面一串 Captae 真实窗口（stack.tsx），往下滚鼠标跟着走、在每个窗口里真点一遍；首屏文字一次性错峰进场（课 04/01 Orchestration）；整页用 Lenis 平滑滚动（smooth.ts）；素材墙的慢速平移在离开视口、切到后台、系统「减少动态效果」时停下。
// 组件：affectUI 免费组件（Button、DropdownMenu、SegmentedControl）、parts（CaptaeLogo、ProgressBar、Swap、Icon、SiteIcon）、Hugeicons Free。
import { StrictMode, useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { MotionConfig, useInView, useReducedMotion } from "motion/react";
import {
  ArrowRight02Icon,
  GithubIcon,
  Globe02Icon as GlobeLike,
  Tick02Icon,
} from "@hugeicons/core-free-icons";
import "@/theme.css";
import "./site.css";
import { Button } from "@/components/ui/button";
import { CaptaeLogo, Icon, SiteIcon, type HugeIcon } from "@/parts";
import { DropdownMenu } from "@/components/ui/dropdown-menu";
import { t } from "@/i18n";
import { applyLang, LANGS, lang, langFromPath, pickLang, urlOf, type Lang } from "./i18n";
import { copy, GITHUB, RELEASES } from "./copy";
import { Stack } from "./stack";
import { Desk } from "./desk";
import { Wave, type Tile } from "./bits";
import { startSmoothScroll } from "./smooth";
import bbb from "@/dev/img/bbb-frank.jpg";
import edEmo from "@/dev/img/ed-emo-proog.jpg";
import edPlatform from "@/dev/img/ed-platform.jpg";
import sintel1 from "@/dev/img/sintel-1.jpg";
import sintel3 from "@/dev/img/sintel-3.jpg";
import tos1 from "@/dev/img/tos-01.jpg";
import tos5 from "@/dev/img/tos-05.jpg";

/* ---------------- 小部件 ---------------- */

/** 链接样式的按钮：落地页的「下载」「GitHub」要能中键、新标签打开，所以是 <a>。
 *  样式借 affectUI Button 的 .ui-button（它按 class 和 data-* 选中）；那份样式由 App 里一个不显示的 <Button> 注入到 <head>。 */
function LinkButton({ href, variant = "primary", size = "lg", icon, children, external }: { href: string; variant?: "primary" | "secondary" | "ghost"; size?: "sm" | "md" | "lg"; icon?: HugeIcon; children: ReactNode; external?: boolean }) {
  return (
    <a className="ui-button cp-pill site-btn" data-variant={variant} data-size={size} data-ready href={href} {...(external ? { target: "_blank", rel: "noreferrer" } : {})}>
      {icon && (
        <span className="ui-button__icon" aria-hidden="true">
          <Icon icon={icon} size={size === "sm" ? 15 : 17} />
        </span>
      )}
      <span className="ui-button__label">{children}</span>
    </a>
  );
}

/** 进入视口时只触发一次（滚动进场只播一次，提早一点触发） */
function useOnce<T extends Element>(margin = "0px 0px -12% 0px") {
  const ref = useRef<T>(null);
  const seen = useInView(ref, { once: true, margin: margin as `${number}px` });
  return [ref, seen] as const;
}

/* ---------------- 素材墙：开放电影剧照排成的几行封面，慢慢横移 ---------------- */

const ROWS: Tile[][] = [
  [
    { src: tos1, dur: "12:14" },
    { src: sintel3, at: "50% 40%", dur: "0:52" },
    { dur: "10:54" },
    { src: bbb, at: "50% 45%", dur: "10:34" },
    { src: edPlatform, dur: "1:18" },
    { src: sintel1, at: "40% 40%", dur: "14:48" },
  ],
  [
    { src: edEmo, dur: "0:30" },
    { src: tos5, at: "30% 40%", dur: "3:12" },
    { src: sintel1, at: "60% 40%", dur: "1:04" },
    { dur: "14:48" },
    { src: bbb, at: "30% 50%", dur: "9:56" },
    { src: tos1, at: "70% 50%", dur: "2:41" },
  ],
  [
    { src: sintel3, at: "20% 50%", dur: "4:20" },
    { src: edPlatform, at: "70% 50%", dur: "6:02" },
    { src: tos5, at: "60% 40%", dur: "12:14" },
    { src: edEmo, at: "40% 50%", dur: "0:47" },
    { dur: "3:33" },
    { src: sintel3, at: "80% 50%", dur: "8:15" },
  ],
];

function Wall({ rows = ROWS, playing }: { rows?: Tile[][]; playing: boolean }) {
  return (
    <div className="site-wall" data-playing={playing || undefined} aria-hidden>
      {rows.map((row, r) => (
        <div className="site-wall-row" key={r} style={{ "--dur": `${[150, 180, 165][r % 3]}s` } as CSSProperties} data-dir={r % 2 ? "right" : "left"}>
          {/* 同一行排两遍，平移一半的长度后接上，看不出接缝 */}
          {[...row, ...row].map((t, i) => (
            <figure className="site-tile" key={i} data-audio={!t.src || undefined}>
              {t.src ? <img src={t.src} alt="" style={{ objectPosition: t.at }} loading={r > 1 ? "lazy" : undefined} /> : <Wave />}
              <span className="site-dur">{t.dur}</span>
            </figure>
          ))}
        </div>
      ))}
    </div>
  );
}

/** 素材墙在视口里、页面在前台、系统没开「减少动态效果」时才平移 */
function useWallPlaying(ref: React.RefObject<Element | null>) {
  const inView = useInView(ref, { margin: "120px 0px" });
  const reduce = useReducedMotion();
  const [visible, setVisible] = useState(() => document.visibilityState === "visible");
  useEffect(() => {
    const on = () => setVisible(document.visibilityState === "visible");
    document.addEventListener("visibilitychange", on);
    return () => document.removeEventListener("visibilitychange", on);
  }, []);
  return inView && visible && !reduce;
}

/* ---------------- 导航 ---------------- */

/** 换语言的菜单：affectUI 的下拉菜单，单选；名字用各自的语言写（和 App 设置里一样） */
function LangMenu({ onLang }: { onLang: (l: Lang) => void }) {
  const cur = lang();
  return (
    <DropdownMenu
      align="end"
      width={170}
      items={LANGS.map((x) => ({ label: x.name, type: "radio" as const, checked: x.code === cur, onSelect: () => x.code !== cur && onLang(x.code) }))}
      trigger={({ triggerProps }) => (
        <button type="button" className="site-lang" {...triggerProps} aria-label={copy.a11y.lang}>
          <Icon icon={GlobeLike} size={16} />
          <span>{LANGS.find((x) => x.code === cur)?.name}</span>
        </button>
      )}
    />
  );
}

function Nav({ onLang }: { onLang: (l: Lang) => void }) {
  return (
    <nav className="site-nav glass" aria-label={copy.a11y.nav}>
      <a className="site-brand" href="#top" aria-label="Captae">
        <CaptaeLogo size={26} />
        <span>Captae</span>
      </a>
      <div className="site-nav-links">
        <a href="#features">{copy.nav.features}</a>
        <a href="#extension">{copy.nav.extension}</a>
        <a href="#ai">{copy.nav.ai}</a>
        <a href="#open">{copy.nav.opensource}</a>
      </div>
      <div className="site-nav-end">
        <LangMenu onLang={onLang} />
        <a className="site-icon-link" href={GITHUB} target="_blank" rel="noreferrer" aria-label="GitHub">
          <Icon icon={GithubIcon} size={18} />
        </a>
        <LinkButton href={RELEASES} size="sm">
          {copy.nav.download}
        </LinkButton>
      </div>
    </nav>
  );
}

/* ---------------- 支持的网站 ---------------- */

function Sites() {
  const list = [
    ["youtube", "YouTube"],
    ["bilibili", t("哔哩哔哩")],
    ["x", "X"],
    ["douyin", t("抖音")],
    ["tiktok", "TikTok"],
    ["instagram", "Instagram"],
  ];
  return (
    <section className="site-sites" aria-label={copy.a11y.sites}>
      <p>{copy.sites.lead}</p>
      <ul>
        {list.map(([k, name]) => (
          <li key={name}>
            <SiteIcon site={k} size={16} />
            {name}
          </li>
        ))}
        <li className="more">{copy.sites.more}</li>
      </ul>
    </section>
  );
}

/* ---------------- 本地、免费开源、未来可期 ---------------- */

function Trust() {
  return (
    <section className="site-section" id="open" aria-label={copy.nav.opensource}>
      <div className="site-trust">
        <article className="site-card plain">
          <div className="site-card-text">
            <p className="site-kicker">{copy.local.kicker}</p>
            <h3>{copy.local.title}</h3>
            <p className="site-body">{copy.local.body}</p>
            <ul className="site-points small">
              {copy.local.points.map((x) => (
                <li key={x}>
                  <Icon icon={Tick02Icon} size={14} />
                  {x}
                </li>
              ))}
            </ul>
          </div>
        </article>
        <article className="site-card plain">
          <div className="site-card-text">
            <p className="site-kicker">{copy.opensource.kicker}</p>
            <h3>{copy.opensource.title}</h3>
            <p className="site-body">{copy.opensource.body}</p>
            <a className="site-arrow-link" href={GITHUB} target="_blank" rel="noreferrer">
              {copy.opensource.link}
              <Icon icon={ArrowRight02Icon} size={14} />
            </a>
          </div>
        </article>
        <article className="site-card plain">
          <div className="site-card-text">
            {/* 这张没有小标签：空一行，标题和旁边两张对齐 */}
            <p className="site-kicker" aria-hidden>
              &nbsp;
            </p>
            <h3>{copy.future.title}</h3>
            <ul className="site-bullets">
              {copy.future.points.map((x) => (
                <li key={x}>{x}</li>
              ))}
            </ul>
          </div>
        </article>
      </div>
    </section>
  );
}

/* ---------------- 结尾：一条胶片从玻璃后面经过（和安装包窗口的背景同一个画法） ---------------- */

const STRIP: Tile[][] = [
  [
    { src: tos5, at: "0% 40%", dur: "3:12" },
    { src: sintel3, at: "45% 45%", dur: "0:52" },
    { src: bbb, at: "55% 55%", dur: "10:34" },
    { src: tos1, dur: "12:14" },
    { src: sintel1, at: "35% 40%", dur: "14:48" },
    { src: edPlatform, dur: "1:18" },
    { src: edEmo, dur: "0:30" },
  ],
];

function Final() {
  const f = copy.final;
  const box = useRef<HTMLDivElement>(null);
  const playing = useWallPlaying(box);
  return (
    <section className="site-final" id="get" aria-labelledby="final-title">
      <div className="site-final-strip" ref={box}>
        <Wall rows={STRIP} playing={playing} />
        <div className="site-final-glass glass">
          <p className="site-kicker">{f.kicker}</p>
          <h2 id="final-title">{f.title}</h2>
          <p className="site-body">{f.subtitle}</p>
          <LinkButton href={RELEASES}>{f.primary}</LinkButton>
        </div>
      </div>
    </section>
  );
}

function Footer() {
  return (
    <footer className="site-foot">
      <div className="site-foot-top">
        <span className="site-brand">
          <CaptaeLogo size={22} />
          <span>Captae</span>
        </span>
        <p>{copy.footer.tagline}</p>
        <nav aria-label={copy.a11y.footer}>
          <a href={GITHUB} target="_blank" rel="noreferrer">
            {copy.footer.github}
          </a>
        </nav>
      </div>
      <p className="site-legal">{copy.footer.legal}</p>
      <p className="site-credits">{copy.footer.credits}</p>
    </footer>
  );
}

function App({ onLang }: { onLang: (l: Lang) => void }) {
  // 整页平滑滚动（Lenis，见 smooth.ts）
  useEffect(() => startSmoothScroll(), []);
  return (
    <MotionConfig reducedMotion="user">
      {/* 不显示：只为让 affectUI Button 把 .ui-button 的样式注入 <head>，页面上的链接按钮（LinkButton）靠它 */}
      <Button aria-hidden tabIndex={-1} style={{ display: "none" }} />
      <Nav onLang={onLang} />
      <main>
        <Desk />
        <Sites />
        <Stack />
        <Trust />
        <Final />
      </main>
      <Footer />
    </MotionConfig>
  );
}

/** 根：换语言时整页按新语言重新挂一次（用语言当 key，和 App 一样），回到页面顶上；浏览器后退、前进也跟着换 */
function Root() {
  const [l, setL] = useState<Lang>(lang());
  const first = useRef(true);
  const go = async (x: Lang, push = true) => {
    await applyLang(x);
    if (push) history.pushState(null, "", urlOf(x));
    setL(x);
  };
  useEffect(() => {
    if (first.current) first.current = false;
    else window.scrollTo(0, 0);
  }, [l]);
  useEffect(() => {
    const on = () => {
      const x = langFromPath();
      if (x && x !== lang()) void go(x, false);
    };
    addEventListener("popstate", on);
    return () => removeEventListener("popstate", on);
  });
  return <App key={l} onLang={(x) => void go(x)} />;
}

/** 先定语言、把文案和 App 的词表加载好再画；网址换成 /<语言>/（不刷新） */
async function boot() {
  const l = pickLang();
  await applyLang(l);
  if (langFromPath() !== l) history.replaceState(null, "", urlOf(l));
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <Root />
    </StrictMode>,
  );
}
void boot();
