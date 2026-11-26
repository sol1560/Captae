// 上手引导卡片（第二版设计稿 6-onboarding-*.png，蓝本是参考录屏 m2 里的引导卡）：头图 + 一页内容 + 底栏（返回、页码点、继续）。
//
// 不用模态 <dialog>：模态的在浏览器最顶层，会把提示条（Sonner）盖住，而这里装语音识别、复制设置的结果要靠提示条说。
// 所以是固定定位的遮罩层：打开时给背后的 .cp-window 加 inert（Tab 进不去、读屏不读），Esc 关掉，焦点放进卡片，关掉后还给原来的地方。
//
// 动效（数值见 docs/MOTION.md 第 3 节 g 弹窗、i 上手引导，课 03/05 和 04/03）：
// - 卡片出现：opacity + scale .98→1，200ms ease-out-quint，从中心出现（modal 不跟触发点）。第一次打开正赶上界面启动、主线程忙，
//   所以进场用 CSS 动画（在合成线程上跑，课 04/02）；退场 150ms 用 Motion（要等它播完再卸载）。遮罩只淡入淡出，不给模糊做动画
// - 翻页：四页叠在同一个格子里，用 CSS 过渡交叉淡化：文字 300ms ease、方向 8px、模糊 4px（新旧同时，模糊盖住两段字重叠的那一下）；
//   剧照新的在旧的上面淡入 430ms（比文字慢，不会中间发白）；界面片段旧的 200ms 先走，新的晚 120ms 再来；页码点 150ms
//   CSS 过渡从当前值出发，连点「继续」不会跳，也不会留下两份（课 03/05 讲的 popLayout 连点问题）
// - 各页一样高（格子取最高那页），翻页时「继续」不挪位置，连点不用追按钮
// - 减少动效：去掉位移、缩放和模糊，只留透明度；界面片段不再错开
import { AnimatePresence, motion, useIsPresent, useReducedMotion } from "motion/react";
import { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { t } from "@/i18n";
import { getPref } from "@/lib/prefs";
import { go } from "@/lib/route";
import { loadDeps, pollMcp, useApp } from "@/lib/store";
import { notify } from "@/lib/notify";
import { modKey } from "@/lib/os";
import { getDialog, openDialog } from "@/shell/actions";
import { PAGES } from "./pages";
import { doneCount, ITEMS, loadExt, markOnboarded, useOb } from "./state";
import "./onboarding.css";

const EASE_OUT_QUINT = [0.23, 1, 0.32, 1] as const;

export function OnboardingHost({ open, onClose }: { open: boolean; onClose: () => void }) {
  // 第一次打开：窗口骨架挂上来时如果启动快照已经到了，它自己会打开引导；后端超过 4 秒才回的话那时还没到，
  // 这里等快照到了再补一次（后台没响应时不打开，那时只该看到出错的说明）
  const ready = useApp((s) => s.ready);
  const offline = useApp((s) => s.offline);
  useEffect(() => {
    if (ready && !offline && !getPref("onboarded", false) && !getDialog()) openDialog("onboarding");
  }, [ready, offline]);
  return <AnimatePresence>{open && <Tour key="tour" onClose={onClose} />}</AnimatePresence>;
}

function Tour({ onClose }: { onClose: () => void }) {
  const reduce = useReducedMotion();
  const present = useIsPresent();
  const uid = useId().replace(/[^a-zA-Z0-9_-]/g, "");
  const [page, setPage] = useState(0);
  const primary = useRef<HTMLButtonElement>(null);
  const ob = useOb((s) => s.ob);
  const last = page === PAGES.length - 1;

  // 打开时把要显示的真实状态都查一遍：外部工具和语音识别、插件、AI 助手
  useEffect(() => {
    loadDeps();
    loadExt();
    pollMcp();
  }, []);

  // AI 助手那页开着时 5 秒看一次连上没有（粘贴设置、重开 Claude 以后很快就能看到「已连上」）
  useEffect(() => {
    if (PAGES[page].key !== "agent") return;
    const timer = window.setInterval(pollMcp, 5000);
    return () => window.clearInterval(timer);
  }, [page]);

  // 背后的窗口不能点、不能 Tab 进去；焦点放进卡片（卡片本身，不是「继续」：第一次打开时还没人动过鼠标，
  // 直接聚焦按钮会画出焦点环，和设计稿不一样；按 Tab 从「跳过」开始往下走）。退场一开始就把这些还回去
  const card = useRef<HTMLElement>(null);
  const back = useRef<HTMLElement | null>(null);
  useLayoutEffect(() => {
    if (!present) return;
    back.current = document.activeElement as HTMLElement | null;
    const win = document.querySelector<HTMLElement>(".cp-window");
    win?.setAttribute("inert", "");
    card.current?.focus({ preventScroll: true });
    return () => {
      win?.removeAttribute("inert");
      const b = back.current;
      if (b?.isConnected && b !== document.body) b.focus({ preventScroll: true });
    };
  }, [present]);

  // 焦点所在的按钮被换掉了（「安装插件」→「等它连上」、「下载」→ 进度条），或者点了遮罩上的空白，焦点会掉到 body 上；
  // 拉回卡片里，用键盘的人接着按 Tab 还在引导里。退场时不拉（焦点要还给原来的地方）
  const presentRef = useRef(present);
  presentRef.current = present;
  useEffect(() => {
    const el = card.current;
    if (!el) return;
    const keep = () =>
      requestAnimationFrame(() => {
        const at = document.activeElement;
        if (presentRef.current && (!at || at === document.body)) el.focus({ preventScroll: true });
      });
    el.addEventListener("focusout", keep);
    return () => el.removeEventListener("focusout", keep);
  }, []);

  const close = (how: "skip" | "finish" | "esc") => {
    if (!present) return;
    markOnboarded();
    onClose();
    if (how === "skip" && !ob.hidden && doneCount(ob) < ITEMS.length) notify(t("已跳过上手引导"), t("侧栏里的「上手清单」会一直在，随时可以接着做"));
    // 走完：还没下载过东西的话，去下载页试一个（下载页空着时有输入框和「试试这些链接」）
    if (how === "finish" && !ob.items.dl) go("downloads");
  };

  // Esc 关掉：在冒泡阶段听，页面里的下拉列表先处理（它会 preventDefault），收起列表的那一下不算。
  // ⌘K：在捕获阶段拦下，不让背后的窗口跳去素材库
  const closeRef = useRef(close);
  closeRef.current = close;
  useEffect(() => {
    const esc = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      e.preventDefault();
      closeRef.current("esc");
    };
    const cmdK = (e: KeyboardEvent) => {
      if (modKey(e) && e.key.toLowerCase() === "k") e.stopImmediatePropagation();
    };
    addEventListener("keydown", esc);
    addEventListener("keydown", cmdK, true);
    return () => {
      removeEventListener("keydown", esc);
      removeEventListener("keydown", cmdK, true);
    };
  }, []);

  const next = () => (last ? close("finish") : setPage(page + 1));
  const prev = () => {
    if (page === 0) return;
    setPage(page - 1);
    // 回到第一页时「返回」会藏起来，焦点交给「继续」，不掉到页面外面
    if (page === 1) primary.current?.focus({ preventScroll: true });
  };

  const p = PAGES[page];
  const titleId = `${uid}-t${page}`;

  return (
    <div className="cp-ob-layer" data-leaving={!present || undefined}>
      {/* 进场是 CSS 动画（onboarding.css 的 cp-ob-in），这里只管退场；animate 写上终态，退场半路又打开时能回到原样 */}
      <motion.div className="cp-ob-scrim" initial={false} animate={{ opacity: 1 }} exit={{ opacity: 0 }} transition={{ duration: 0.15, ease: EASE_OUT_QUINT }} />
      <motion.section
        ref={card}
        tabIndex={-1}
        className="cp cp-ob"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        initial={false}
        // 退场的缩小写成整条 transform 字符串，交给浏览器跑（实测 scale 简写在这里是主线程逐帧算的；关掉引导时背后的页面正在重画）。
        // 字符串 transform 不归 MotionConfig 管，减少动效时不缩放、只淡出。
        // 终态写 scale(1) 而不是 none：Motion 从 none 缩到 scale(.98) 会当成从 scale(0) 开始（实测第一帧 0.155）
        animate={reduce ? { opacity: 1 } : { opacity: 1, transform: "scale(1)" }}
        exit={reduce ? { opacity: 0 } : { opacity: 0, transform: "scale(0.98)" }}
        transition={{ duration: 0.15, ease: EASE_OUT_QUINT }}
      >
        <div className="cp-ob-art">
          {PAGES.map((x, i) => (
            <img key={x.key} src={x.art.image} alt="" draggable={false} data-on={i === page || undefined} style={x.art.at ? { objectPosition: x.art.at } : undefined} />
          ))}
          {PAGES.map((x, i) => (
            <div key={x.key} className="cp-ob-scene" aria-hidden data-on={i === page || undefined}>
              <x.art.Scene />
            </div>
          ))}
          <button type="button" className="cp-ob-skip" onClick={() => close("skip")}>
            {t("跳过")}
          </button>
        </div>

        <div className="cp-ob-body">
          {PAGES.map((x, i) => (
            <div key={x.key} className="cp-ob-page" data-at={i < page ? "before" : i > page ? "after" : undefined} inert={i !== page} aria-hidden={i !== page || undefined}>
              <x.Body id={`${uid}-t${i}`} title={t(x.title)} />
            </div>
          ))}
        </div>

        <div className="cp-ob-foot">
          <span className="cp-ob-back" data-off={page === 0 || undefined}>
            <Button variant="secondary" className="cp-pill" tabIndex={page === 0 ? -1 : undefined} onClick={prev}>
              {t("返回")}
            </Button>
          </span>
          <div className="cp-dots" role="img" aria-label={t("第{n}页，共{total}页", { n: page + 1, total: PAGES.length })}>
            {PAGES.map((x, i) => (
              <i key={x.key} data-on={i === page || undefined} />
            ))}
          </div>
          <div className="end">
            <Button ref={primary} className="cp-pill cp-ob-next" onClick={next}>
              {last ? t("开始用") : t("继续§上手引导翻到下一页")}
            </Button>
          </div>
        </div>
        {/* 读屏：翻页时念出第几页和标题（卡片的名字跟着 aria-labelledby 变，但不一定会被念出来）。
            整句是一段文字（以前数字和标题各是一段），换页时读屏念的是整句 */}
        <p className="cp-ob-sr" aria-live="polite">
          {t("第{n}页，共{total}页：{title}", { n: page + 1, total: PAGES.length, title: t(p.title) })}
        </p>
      </motion.section>
    </div>
  );
}
