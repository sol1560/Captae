// 功能这一段：页面里从上往下排着一串 Captae 的真实窗口（App 的演示版，嵌在 iframe 里），每个窗口旁边是这个功能的标题和说明，左右交替。
// 往下滚，一个鼠标跟着往下走（带阻尼的弹簧，慢半拍），在窗口之间左右荡；走到哪个窗口，就在里面真的点一遍这个功能，窗口下面一行小字说它在做什么。
// 点完（或者访客点了一下窗口）就交给访客：盖一层「自己操作试试」，点了才能操作，鼠标移开盖回去（滚轮不会被里面吞掉）。
// - 维护者 2026-11-25 的示意图：窗口竖着排，字左右交替，一条 S 形的线带着往下。示意图只是意思，窗口数按功能来，鼠标不用贴着线走。
// - 鼠标：和首屏以前的演示、功能导览同一个箭头（bits.tsx）；按下 67ms 压到 .89 再弹回（Copilot 宣传片逐帧量的，work/site/ref/ANALYSIS.md）。
// - 演示版带 &tour（src/dev/mock.ts）：下载快、YouTube 链接让人挑清晰度、能收到「刚复制了链接」、语音识别当作已装好。
// - 窗口快滚到才加载（每个是一份演示版）；只找结构（类名、第几个），不找文字，换语言照样点得到。
// - 窄屏、矮屏、系统开了「减少动态效果」：换成功能导览的卡片列表（tour.tsx 的 TourList）。
import { motion, useMotionValue, useReducedMotion, useSpring, useTransform } from "motion/react";
import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { Tick02Icon } from "@hugeicons/core-free-icons";
import { Icon, Swap } from "@/parts";
import { ArrowGlyph } from "./bits";
import { copy } from "./copy";
import { lang } from "./i18n";
import { TourList } from "./tour";

/* ---------------- 每个窗口：开哪一页、点哪些东西 ---------------- */

type Step =
  | { q: string; i?: number; has?: string; after?: number; ms?: number } // 移过去、按下、点
  | { type: string; q: string } // 移过去点一下，再一个字一个字打进去
  | { pause: number }
  | { clipboard: string }; // 告诉演示版「刚复制了这个链接」（它照真后端那样弹出提示）

type Scene = { id: string; anchor?: string; full: boolean; page: string; steps: Step[]; text: () => { kicker: string; title: string; body: string }; points?: () => string[] };

const LINK = "https://www.youtube.com/watch?v=aqz-KE-bpKQ";
const SCENES: Scene[] = [
  {
    id: "pick",
    full: false,
    page: "downloads",
    steps: [{ pause: 500 }, { clipboard: LINK }, { q: ".cp-suggest .ui-button", after: 500 }, { q: ".cp-job [role=radio]", i: 1, ms: 9000 }, { q: ".cp-job .cp-pickfoot .cp-link" }, { pause: 2500 }],
    text: () => copy.pick,
  },
  {
    id: "login",
    full: true,
    page: "downloads",
    steps: [{ q: ".cp-topbar [role=radio]", i: 2 }, { q: '.cp-job[data-id="27"] .ui-button.cp-pill' }, { pause: 3800 }, { q: ".cp-topbar [role=radio]", i: 0 }, { pause: 900 }],
    text: () => copy.login,
  },
  {
    id: "reasons",
    full: true,
    page: "downloads",
    steps: [{ q: ".cp-topbar [role=radio]", i: 2 }, { q: ".cp-job", i: 0, after: 1300 }, { q: ".cp-job", i: 1, after: 1300 }, { q: ".cp-job", i: 2, after: 1300 }],
    text: () => copy.reasons,
  },
  {
    id: "library",
    full: true,
    page: "library",
    steps: [{ type: "Sintel", q: ".cp-topbar .ui-tf__input" }, { pause: 900 }, { q: ".lib-tile:not([aria-hidden])", i: 0 }, { pause: 900 }],
    text: () => copy.library,
    points: () => copy.library.chips,
  },
  {
    id: "speech",
    full: true,
    page: "library",
    steps: [{ q: ".lib-tile:not([aria-hidden])", i: 1 }, { q: ".lib-tools [role=radio]", i: 1, after: 600 }, { q: ".lib-tool [role=radio]", i: 1, ms: 6000 }, { q: ".lib-run .cp-pill" }, { pause: 4800 }],
    text: () => copy.speech,
  },
  {
    id: "live",
    full: true,
    page: "downloads",
    steps: [{ q: '.cp-job[data-id="31"] .ui-button.cp-pill', ms: 6000 }, { pause: 2200 }, { q: ".cp-topbar [role=radio]", i: 1 }, { pause: 900 }],
    text: () => copy.live,
  },
  {
    id: "extension",
    anchor: "extension",
    full: true,
    page: "extension",
    steps: [{ q: ".ui-button.cp-pill", i: 1 }, { pause: 1400 }],
    text: () => copy.extension,
    points: () => copy.extension.points,
  },
  {
    id: "ai",
    anchor: "ai",
    full: true,
    page: "agent",
    steps: [{ q: "[role=radio]", i: 1 }, { q: ".ui-button.cp-pill", i: 1 }, { q: ".ui-dlg__btn", i: 1, ms: 4000, after: 1200 }],
    text: () => copy.ai,
    points: () => copy.ai.points,
  },
  {
    id: "send",
    full: true,
    page: "library",
    steps: [{ q: ".lib-tile:not([aria-hidden])", i: 0 }, { q: ".cp-dactions .ui-icb", i: 2, ms: 5000 }, { pause: 1500 }],
    text: () => copy.send,
  },
];

/* ---------------- 数值 ---------------- */

const APP_W = 1200; // 演示版按这个大小排，再整体缩到窗口里
const APP_H = 750;
// 鼠标跟随的弹簧：过阻尼（阻尼比约 1.2），慢半拍、不回弹，有分量
const SPRING = { stiffness: 70, damping: 20, mass: 1 };
const PRESS = { stiffness: 900, damping: 30 };
const STOP = Symbol("stop");

type Mode = "wait" | "play" | "over" | "live";
type Pt = { x: number; y: number };

const sleep = (ms: number) => new Promise<void>((r) => setTimeout(r, ms));

/* ---------------- 一个窗口 ---------------- */

function Row({
  i,
  s,
  mode,
  load,
  caption,
  setFrame,
  onTake,
  onLeave,
}: {
  i: number;
  s: Scene;
  mode: Mode;
  load: boolean;
  caption: string;
  setFrame: (el: HTMLIFrameElement | null) => void;
  onTake: () => void;
  onLeave: () => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [k, setK] = useState(0.6);
  useEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setK(el.clientWidth / APP_W));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const tx = s.text();
  const pts = s.points?.();
  const src = `/demo/?${s.full ? "mock&rec" : "mock=empty"}&tour&lang=${lang()}#/${s.page}`;
  return (
    <article className="stack-row" id={s.anchor} data-side={i % 2 ? "left" : "right"} aria-labelledby={`stack-${s.id}`}>
      <div className="stack-text">
        <p className="site-kicker">{tx.kicker}</p>
        <h3 id={`stack-${s.id}`}>{tx.title}</h3>
        <p className="site-body">{tx.body}</p>
        {pts && (
          <ul className="site-points small">
            {pts.map((x) => (
              <li key={x}>
                <Icon icon={Tick02Icon} size={14} />
                {x}
              </li>
            ))}
          </ul>
        )}
      </div>
      <div className="stack-win-wrap">
        <div className="stack-win" ref={box} style={{ "--k": k } as CSSProperties} data-live={mode === "live" || undefined} onMouseLeave={onLeave}>
          {load && <iframe ref={setFrame} className="stack-frame" src={src} title={copy.a11y.screenAlt.replace("{page}", tx.kicker)} tabIndex={mode === "live" ? 0 : -1} />}
          {mode !== "live" &&
            (mode === "over" ? (
              <button type="button" className="stack-cover" onClick={onTake}>
                <span className="stack-take">
                  <ArrowGlyph size={18} tilt={-32} />
                  {copy.stack.take}
                </span>
              </button>
            ) : (
              <div className="stack-cover" onPointerDown={onTake} aria-hidden />
            ))}
        </div>
        <div className="stack-caption" aria-live="polite">
          <Swap k={mode === "wait" ? "w" : "p"}>
            <span data-on={mode !== "wait" || undefined}>{caption}</span>
          </Swap>
        </div>
      </div>
    </article>
  );
}

/* ---------------- 一串窗口 + 一个鼠标 ---------------- */

function Wide() {
  // 演示版和官网在同一个网址下、本地存储共用：先记成「上手引导看过了」，不然每个窗口一打开都弹引导、挡住界面（键名见 src/lib/prefs.ts）
  useState(() => {
    try {
      localStorage.setItem("captae3:migrated", "1");
      localStorage.setItem("captae3:onboarded", "true");
    } catch {}
    return 0;
  });
  const sec = useRef<HTMLElement>(null);
  const rows = useRef<(HTMLElement | null)[]>([]);
  const frames = useRef<(HTMLIFrameElement | null)[]>([]);
  const [modes, setModes] = useState<Mode[]>(() => SCENES.map(() => "wait"));
  const [loaded, setLoaded] = useState<boolean[]>(() => SCENES.map(() => false));
  const setMode = (i: number, m: Mode) => setModes((a) => (a[i] === m ? a : a.map((x, k) => (k === i ? m : x))));
  const modesRef = useRef(modes);
  modesRef.current = modes;

  // 鼠标：目标点（这一段里的坐标）每帧算，弹簧带着跟过去
  const tx = useMotionValue(0);
  const ty = useMotionValue(0);
  const x = useSpring(tx, SPRING);
  const y = useSpring(ty, SPRING);
  const press = useMotionValue(1);
  const scale = useSpring(press, PRESS);
  const shown = useMotionValue(0);
  const opacity = useSpring(shown, { stiffness: 120, damping: 24 });
  const visibility = useTransform(opacity, (o) => (o > 0.01 ? "visible" : "hidden"));
  const aim = useRef<(() => Pt | null) | null>(null); // 正在演示时：要点的那个东西在哪

  /** 一个元素（可能在某个窗口的演示版里）在这一段里的坐标 */
  const pointOf = (el: Element, frame?: HTMLIFrameElement | null, fx = 0.55, fy = 0.62): Pt | null => {
    const s = sec.current;
    if (!s || !el.isConnected) return null;
    const S = s.getBoundingClientRect();
    const r = el.getBoundingClientRect();
    if (!frame) return { x: r.left - S.left + r.width * fx, y: r.top - S.top + r.height * fy };
    const F = frame.getBoundingClientRect();
    const k = F.width / APP_W;
    return { x: F.left - S.left + (r.left + r.width * fx) * k, y: F.top - S.top + (r.top + r.height * fy) * k };
  };

  // 现在轮到哪个窗口：中线离视口中间最近的那个
  const activeOf = () => {
    const vc = window.innerHeight / 2;
    let best = -1;
    let d = Infinity;
    rows.current.forEach((r, i) => {
      if (!r) return;
      const R = r.getBoundingClientRect();
      const dd = Math.abs(R.top + R.height / 2 - vc);
      if (dd < d) {
        d = dd;
        best = i;
      }
    });
    return { i: best, d };
  };

  // 每帧：算目标点；快到哪个窗口就先把它加载上；中线到了视口中间附近就开始演示
  useEffect(() => {
    let raf = 0;
    let on = false;
    const io = new IntersectionObserver(([e]) => {
      on = !!e?.isIntersecting;
      if (on && !raf) raf = requestAnimationFrame(tick);
    }, { rootMargin: "100% 0px" });
    if (sec.current) io.observe(sec.current);
    const tick = () => {
      raf = 0;
      if (!on) return;
      const s = sec.current;
      if (s) {
        const S = s.getBoundingClientRect();
        const H = window.innerHeight;
        // 加载：下方 1.6 屏、上方 0.8 屏以内的窗口；离开 2.4 屏以外的卸掉（每个窗口是一整份演示版，九个同时跑太重）。
        // 访客正在里面操作的不卸。卸掉以后状态不变：没轮到的回来照样演示，演示过的回来是新的一份、直接交给访客（只播一次）
        rows.current.forEach((r, i) => {
          if (!r) return;
          const R = r.getBoundingClientRect();
          const near = R.top < H * 1.6 && R.bottom > -H * 0.8;
          const far = R.top > H * 2.4 || R.bottom < -H * 1.6;
          if (near) setLoaded((a) => (a[i] ? a : a.map((v, k) => v || k === i)));
          else if (far && modesRef.current[i] !== "live") setLoaded((a) => (a[i] ? a.map((v, k) => v && k !== i) : a));
        });
        const { i, d } = activeOf();
        const target = aim.current?.();
        if (target) {
          tx.set(target.x);
          ty.set(target.y);
        } else {
          // 在窗口之间：跟着滚动往下，x 在相邻两个窗口靠字那一侧的边之间荡（S 形）
          const vc = H * 0.52;
          const edge = (k: number): Pt | null => {
            const r = rows.current[k]?.querySelector(".stack-win");
            if (!r) return null;
            const R = r.getBoundingClientRect();
            return { x: (k % 2 ? R.left + 34 : R.right - 34) - S.left, y: R.top + R.height / 2 - S.top };
          };
          let a = 0;
          for (let k = 0; k < rows.current.length - 1; k++) if ((edge(k + 1)?.y ?? Infinity) + S.top <= vc) a = k + 1;
          const p = edge(a);
          const q = edge(Math.min(a + 1, rows.current.length - 1));
          if (p && q) {
            const t = q.y === p.y ? 0 : Math.min(1, Math.max(0, (vc - S.top - p.y) / (q.y - p.y)));
            const w = (1 - Math.cos(Math.PI * t)) / 2;
            tx.set(p.x + (q.x - p.x) * w);
            ty.set(vc - S.top);
          }
        }
        // 段落进到视口中间才露出鼠标，滚过最后一个窗口就收起来
        const first = rows.current[0]?.getBoundingClientRect();
        const last = rows.current[rows.current.length - 1]?.getBoundingClientRect();
        shown.set(first && last && first.top < H * 0.62 && last.bottom > H * 0.3 ? 1 : 0);
        // 开始演示
        if (i >= 0 && d < H * 0.22 && modesRef.current[i] === "wait" && frames.current[i]) play(i);
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      io.disconnect();
      cancelAnimationFrame(raf);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // 演示一个窗口：一步步移过去、按下、点；当前窗口换了或者访客接手就停
  const running = useRef<number | null>(null);
  const play = (i: number) => {
    if (running.current !== null) return;
    running.current = i;
    setMode(i, "play");
    const frame = frames.current[i]!;
    const alive = () => {
      if (modesRef.current[i] !== "play" && running.current === i) throw STOP;
      const a = activeOf();
      if (a.i !== i || a.d > window.innerHeight * 0.45) throw STOP;
    };
    const wait = async (ms: number) => {
      for (let t = 0; t < ms; t += 100) {
        await sleep(100);
        alive();
      }
    };
    const doc = () => frame.contentDocument;
    const find = (q: string, k = 0, has?: string) => {
      const all = [...(doc()?.querySelectorAll(q) ?? [])].filter((e) => !has || (e.textContent ?? "").includes(has));
      return all[k] as HTMLElement | undefined;
    };
    const until = async (q: string, k = 0, ms = 8000) => {
      for (let t = 0; t < ms; t += 120) {
        const e = find(q, k);
        if (e) return e;
        await wait(120);
      }
      throw STOP;
    };
    const reach = async (el: HTMLElement) => {
      aim.current = () => pointOf(el, frame);
      // 等弹簧追上（离目标 4px 以内），最多 2.4 秒
      for (let t = 0; t < 2400; t += 50) {
        const p = aim.current?.();
        if (p && Math.hypot(x.get() - p.x, y.get() - p.y) < 4) break;
        await wait(50);
      }
    };
    const tap = async (el: HTMLElement, after = 350) => {
      await reach(el);
      await wait(120);
      press.set(0.89);
      await wait(70);
      press.set(1);
      el.click();
      await wait(after);
    };
    const type = async (el: HTMLInputElement, text: string) => {
      await tap(el, 150);
      el.focus();
      const W = frame.contentWindow as (Window & typeof globalThis) | null;
      const set = W && Object.getOwnPropertyDescriptor(W.HTMLInputElement.prototype, "value")?.set;
      for (let n = 1; n <= text.length; n++) {
        set?.call(el, text.slice(0, n));
        el.dispatchEvent(new Event("input", { bubbles: true }));
        // 打字速度：约每秒 20 个字，不均匀（Copilot 片子结尾实测）
        await wait(35 + Math.random() * 35);
      }
    };
    const run = async () => {
      // 等演示版画好（第一页的第一个东西出现）
      await until(".cp-main, .cp-empty", 0, 15000);
      await wait(400);
      for (const st of SCENES[i]!.steps) {
        if ("pause" in st) await wait(st.pause);
        else if ("clipboard" in st) frame.contentWindow?.postMessage({ captaeDemo: "clipboard", url: st.clipboard }, location.origin);
        else if ("type" in st) await type((await until(st.q)) as HTMLInputElement, st.type);
        else await tap(await until(st.q, st.i ?? 0, st.ms), st.after);
      }
    };
    run()
      .catch(() => {})
      .finally(() => {
        aim.current = null;
        running.current = null;
        if (modesRef.current[i] !== "live") setMode(i, "over");
      });
  };

  const take = (i: number) => {
    if (running.current === i) {
      aim.current = null;
    }
    setMode(i, "live");
    frames.current[i]?.focus();
  };
  const leave = (i: number) => {
    if (modesRef.current[i] === "live") setMode(i, "over");
  };

  return (
    <section className="stack" id="features" ref={sec} aria-label={copy.nav.features}>
      {SCENES.map((s, i) => (
        <div
          key={s.id}
          ref={(el) => {
            rows.current[i] = el;
          }}
        >
          <Row
            i={i}
            s={s}
            mode={modes[i]!}
            load={loaded[i]!}
            caption={copy.stack.captions[i] ?? ""}
            setFrame={(el) => {
              frames.current[i] = el;
            }}
            onTake={() => take(i)}
            onLeave={() => leave(i)}
          />
        </div>
      ))}
      <motion.div className="stack-cursor" style={{ x, y, scale, opacity, visibility }} aria-hidden>
        <ArrowGlyph size={30} tilt={-32} />
      </motion.div>
    </section>
  );
}

const WIDE = "(min-width: 1001px) and (min-height: 640px)";

export function Stack(): ReactNode {
  const reduce = !!useReducedMotion();
  const [wide, setWide] = useState(() => matchMedia(WIDE).matches);
  useEffect(() => {
    const m = matchMedia(WIDE);
    const on = () => setWide(m.matches);
    m.addEventListener("change", on);
    return () => m.removeEventListener("change", on);
  }, []);
  return wide && !reduce ? <Wide /> : <TourList />;
}
