// 几个界面上反复用到的小 hook。
import { useEffect, useLayoutEffect, useState, type DependencyList, type RefObject } from "react";

/** on 持续超过 ms 才变成 true：不到 300ms 就结束的等待不显示转圈，闪一下反而像出错（配方 1.10） */
export function useDelayed(on: boolean, ms = 300): boolean {
  const [v, setV] = useState(false);
  useEffect(() => {
    if (!on) {
      setV(false);
      return;
    }
    const t = setTimeout(() => setV(true), ms);
    return () => clearTimeout(t);
  }, [on, ms]);
  return v;
}

/** 每分钟变一次的数，让「3 分钟前」这类文字跟着走；active 为 false 时不计时 */
export function useMinute(active: boolean): number {
  const [n, setN] = useState(0);
  useEffect(() => {
    if (!active) return;
    const t = setInterval(() => setN((x) => x + 1), 60_000);
    return () => clearInterval(t);
  }, [active]);
  return n;
}

/** 元素宽度够不够 min（ResizeObserver）；deps 变了重新绑定（元素换了的时候）。
 *  guess：第一次画之前先估一个，估对了就不会「先按宽版画、量完再改成窄版」，列表也不会因此在载入时做布局动画 */
export function useWideEnough(ref: RefObject<HTMLElement | null>, min: number, deps: DependencyList = [], guess?: () => boolean): boolean {
  const [ok, setOk] = useState(() => (guess ? guess() : true));
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    setOk(el.getBoundingClientRect().width >= min);
    const ro = new ResizeObserver(([e]) => setOk(e.contentRect.width >= min));
    ro.observe(el);
    return () => ro.disconnect();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [min, ...deps]);
  return ok;
}
