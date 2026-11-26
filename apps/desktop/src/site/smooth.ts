// 整页平滑滚动：Lenis。原来照 Slate 落地页（BetterX 仓库 apps/web/src/landing/smooth-scroll.tsx）的 duration 1.6s，
// 维护者 2026-11-24 试了觉得太轻（实测触控板轻扫一下走 1.8 屏，五六下滑完整页），所以加了阻力：
// - 滚轮 ×0.45：同样的手势只走 0.45 倍
// - 跟手用 lerp 0.06（每帧追上剩下距离的 6%）：连续扫的时候更黏、更沉
// 点箭头、点导航跳转（scrollTo）仍按 Slate 的 1.6s 缓动滑过去。
// 页面内容带着阻尼滑；功能导览里的箭头和卡片在这之上再走一层弹簧（tour.tsx）。
// 系统开了「减少动态效果」时不开，用浏览器原生滚动。页内锚点（href="#…"）也交给 Lenis 滑过去；
// 目标带 data-exact 的（导览的各站）滚到它正好在屏幕顶上，其余的留出导航条的高度。
import Lenis from "lenis";

const LENIS_OPTIONS = {
  lerp: 0.06,
  orientation: "vertical" as const,
  gestureOrientation: "vertical" as const,
  smoothWheel: true,
  wheelMultiplier: 0.45,
  touchMultiplier: 1,
};
/** 跳转（点箭头、点导航）时的滑动：Slate 的 1.6s、1.001 − 2^(−10t) */
const GLIDE = { duration: 1.6, easing: (t: number) => Math.min(1, 1.001 - Math.pow(2, -10 * t)) };

let lenis: Lenis | null = null;

const offsetOf = (el: HTMLElement) => (el.hasAttribute("data-exact") ? 0 : -80);

/** 滚到某个元素或某个位置。有 Lenis 就滑过去，没有就用原生滚动 */
export function scrollToTarget(target: HTMLElement | number, offset?: number) {
  const off = offset ?? (typeof target === "number" ? 0 : offsetOf(target));
  if (lenis) {
    lenis.scrollTo(target, { offset: off, ...GLIDE });
    return;
  }
  const y = typeof target === "number" ? target : target.getBoundingClientRect().top + window.scrollY;
  window.scrollTo({ top: y + off });
}

export function startSmoothScroll(): () => void {
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return () => {};
  lenis = new Lenis(LENIS_OPTIONS);
  let raf = 0;
  const loop = (t: number) => {
    lenis?.raf(t);
    raf = requestAnimationFrame(loop);
  };
  raf = requestAnimationFrame(loop);
  const onClick = (e: MouseEvent) => {
    const a = (e.target as HTMLElement).closest?.('a[href^="#"]');
    const href = a?.getAttribute("href");
    if (!href || href === "#") return;
    const el = document.querySelector<HTMLElement>(href);
    if (!el) return;
    e.preventDefault();
    scrollToTarget(el);
  };
  document.addEventListener("click", onClick);
  return () => {
    cancelAnimationFrame(raf);
    document.removeEventListener("click", onClick);
    lenis?.destroy();
    lenis = null;
  };
}
