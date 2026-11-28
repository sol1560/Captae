// 落地页几处共用的小部件：链接样式的按钮、箭头的图形、声音素材的封面。
import type { ReactNode } from "react";
import { Icon, type HugeIcon } from "@/parts";

export type Tile = { src?: string; at?: string; dur: string };

/** 链接样式的按钮：落地页的「下载」「GitHub」要能中键、新标签打开，所以是 <a>。
 *  样式借 affectUI Button 的 .ui-button（它按 class 和 data-* 选中）；那份样式由 main.tsx 的 App 里一个不显示的 <Button> 注入到 <head>。 */
export function LinkButton({
  href,
  variant = "primary",
  size = "lg",
  icon,
  children,
  external,
  label,
}: {
  href: string;
  variant?: "primary" | "secondary" | "ghost";
  size?: "sm" | "md" | "lg";
  icon?: HugeIcon;
  children: ReactNode;
  external?: boolean;
  /** 读屏软件读的名字（一列都叫「下载」时说清是哪一个） */
  label?: string;
}): ReactNode {
  return (
    <a className="ui-button cp-pill site-btn" data-variant={variant} data-size={size} data-ready href={href} aria-label={label} {...(external ? { target: "_blank", rel: "noreferrer" } : {})}>
      {icon && (
        <span className="ui-button__icon" aria-hidden="true">
          <Icon icon={icon} size={size === "sm" ? 15 : 17} />
        </span>
      )}
      <span className="ui-button__label">{children}</span>
    </a>
  );
}

/** 箭头：首屏演示里的指针和功能导览里带路的箭头是同一个（Copilot 片子里从头到尾也只有一个光标）。
 *  图形默认朝上，尖在 (16, 3)（32×32 的画布）。tilt 让它像鼠标指针那样斜着，绕尖转，尖的位置不变 */
export function ArrowGlyph({ size = 40, tilt = 0 }: { size?: number; tilt?: number }): ReactNode {
  return (
    <svg width={size} height={size} viewBox="0 0 32 32" overflow="visible" aria-hidden>
      <path d="M16 3 27 28l-11-5.5L5 28Z" transform={tilt ? `rotate(${tilt} 16 3)` : undefined} fill="#141414" stroke="#fff" strokeWidth="2" strokeLinejoin="round" />
    </svg>
  );
}

/** 声音素材的封面：和素材库里一样，深色底上一排竖条 */
export function Wave(): ReactNode {
  const bars = [6, 12, 20, 14, 26, 18, 30, 22, 34, 24, 30, 16, 26, 20, 12, 22, 28, 14, 20, 10, 16, 8];
  return (
    <svg viewBox="0 0 132 40" aria-hidden>
      {bars.map((h, i) => (
        <rect key={i} x={i * 6} y={20 - h / 2} width="2.4" height={h} rx="1.2" fill="rgba(255,255,255,.82)" />
      ))}
    </svg>
  );
}
