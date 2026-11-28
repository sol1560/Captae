// 首屏：标题、一句说明、下载按钮。窄屏下面再放一张这种语言的下载页截图。
// 下面紧接着是一串 Captae 真实窗口（stack.tsx），鼠标在里面带着点一遍。
// （2026-11-25 之前这里是一张钉住的「电脑桌面」，维护者画了示意图以后改成 stack.tsx 那种竖着排的窗口。）
import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { GithubIcon } from "@hugeicons/core-free-icons";
import { LinkButton } from "./bits";
import { copy, GITHUB } from "./copy";
import { shotOf } from "./shots";

function Hello() {
  const h = copy.hero;
  return (
    <>
      <p className="site-eyebrow site-in" style={{ "--d": "0ms" } as CSSProperties}>
        {h.eyebrow}
      </p>
      <h1 className="site-in" style={{ "--d": "70ms" } as CSSProperties}>
        {h.title}
      </h1>
      <p className="site-lede site-in" style={{ "--d": "160ms" } as CSSProperties}>
        {h.subtitle}
      </p>
      <div className="site-cta site-in" style={{ "--d": "250ms" } as CSSProperties}>
        {/* 滑到页尾「下载」那块（main.tsx 的 Final，#get）：那里按系统列出 GitHub 最新一版的安装包 */}
        <LinkButton href="#get">{h.primary}</LinkButton>
        <LinkButton href={GITHUB} variant="secondary" icon={GithubIcon} external>
          {h.secondary}
        </LinkButton>
      </div>
      <p className="site-note site-in" style={{ "--d": "310ms" } as CSSProperties}>
        {h.note}
      </p>
    </>
  );
}

/** 窄屏、矮屏：标题 + 这种语言的下载页截图 */
function Narrow() {
  return (
    <section className="site-hero" id="top">
      <Hello />
      <div className="desk-narrow-shot site-in" style={{ "--d": "400ms" } as CSSProperties}>
        <img src={shotOf("downloads")} alt={copy.a11y.screenAlt.replace("{page}", "Captae")} decoding="async" />
      </div>
    </section>
  );
}

const WIDE = "(min-width: 1001px) and (min-height: 640px)";

export function Desk(): ReactNode {
  const [wide, setWide] = useState(() => matchMedia(WIDE).matches);
  useEffect(() => {
    const m = matchMedia(WIDE);
    const on = () => setWide(m.matches);
    m.addEventListener("change", on);
    return () => m.removeEventListener("change", on);
  }, []);
  return wide ? (
    <section className="site-hero" id="top">
      <Hello />
    </section>
  ) : (
    <Narrow />
  );
}
