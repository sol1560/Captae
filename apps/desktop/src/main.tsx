// 入口：先装演示后端（只在浏览器里带 ?mock 时），再从后端取启动快照，然后画界面。
// 等快照再画，是为了不先闪一下「什么都没有」的空页面；后端 4 秒还没回就先画出来，之后照常靠事件更新。
// 登录窗口里的工具条（#/login-bar）也是这个页面：只画工具条，不启动主界面（不取快照、不收事件）。
// 界面语言：快照里带着，词表加载好了才画第一屏；换语言时整个窗口按新语言重新挂一次（Root 的 key），
// 弹窗开着与否、当前页面、任务这些都在组件外面（store、actions、地址栏），重挂不会丢。
import { MotionConfig, MotionGlobalConfig } from "motion/react";
import { StrictMode, useLayoutEffect, useRef } from "react";
import { createRoot } from "react-dom/client";
import "./theme.css";
import { loadAndSetLang, parseLang, setCatalogs } from "./i18n";
import { invoke, setupMock } from "./lib/bridge";
import { boot, useApp } from "./lib/store";
import type { LoginBarState } from "./lib/types";
import { openDialog, type DialogName } from "./shell/actions";
import { LoginBar } from "./shell/LoginBar";
import { Window } from "./shell/Window";

setCatalogs(import.meta.glob("./i18n/locales/*/*.json", { import: "default" }));

async function start() {
  const mocked = await setupMock().catch((e) => {
    console.error(e);
    return false;
  });
  if (location.hash.startsWith("#/login-bar")) {
    // 工具条和主界面用同一种语言：状态里带着（后端那边的当前语言）
    const st = await invoke<LoginBarState | null>("login_bar_state").catch(() => null);
    await loadAndSetLang(parseLang(st?.lang) ?? parseLang(navigator.language) ?? "en").catch((e) => console.error(e));
    createRoot(document.getElementById("root")!).render(
      <StrictMode>
        <LoginBar />
      </StrictMode>,
    );
    return;
  }
  await Promise.race([boot(), new Promise((r) => setTimeout(r, 4000))]);
  // 演示数据截图用：?mock&open=settings 打开时直接弹出设置
  if (mocked) {
    const open = new URLSearchParams(location.search).get("open");
    if (open === "settings" || open === "onboarding") openDialog(open as DialogName);
  }
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <Root />
    </StrictMode>,
  );
}

function Root() {
  const lang = useApp((s) => s.lang);
  const shown = useRef(lang);
  // 换语言时整个窗口按新语言重新挂一次。界面上的东西本来就在，不该再播一遍进场动画（设置弹窗弹出、列表淡入）。
  // 这里执行时子组件已经挂好（它们的布局 effect 先执行，弹窗已经 showModal），Motion 的动画在子组件的 effect 里才开始：
  // 先让 Motion 的动画直接到终点，再把这次开始的 CSS 动画跳到结尾。有的进场动画挂载以后才开始
  // （affectUI 的按钮下一次渲染才加 data-ready，文字和图标的进场动画那时才播），所以接下来十帧每帧都在画之前看一眼，然后恢复
  useLayoutEffect(() => {
    if (shown.current === lang) return;
    shown.current = lang;
    MotionGlobalConfig.skipAnimations = true;
    settle();
    let frames = 0;
    const tick = () => {
      settle();
      if (++frames < 10) requestAnimationFrame(tick);
      else MotionGlobalConfig.skipAnimations = false;
    };
    requestAnimationFrame(tick);
  }, [lang]);
  return (
    // 减少动效：Motion 只保留透明度，位移、缩放和布局动画都关掉（配方 1.7）
    <MotionConfig reducedMotion="user">
      <Window key={lang} />
    </MotionConfig>
  );
}

start();

/** 刚开始的进场动画直接跳到结尾。一直转的（转圈、流光）照常；
 *  按真实时间走的也照常：倒计时圆环（parts 的 Countdown）重新挂载时按剩下的秒数重新开始，几秒到几分钟，跳到结尾就提前走完了。
 *  进场动画都在 1 秒以内（docs/MOTION.md 1.2），所以只动 1 秒以内的 */
function settle() {
  for (const a of document.getAnimations()) {
    const timing = a.effect?.getComputedTiming();
    if (!timing || timing.iterations === Infinity || a.playState !== "running") continue;
    if (Number(timing.activeDuration ?? 0) > 1000) continue;
    if (Number(a.currentTime ?? 0) > 150) continue;
    try {
      a.finish();
    } catch {
      // 跳不到结尾的就让它播完
    }
  }
}
