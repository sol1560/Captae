// Windows / Linux 的标题条和窗口按钮（2026-11-24）。
// 这两个系统上主窗口是无边框的（src-tauri/tauri.windows.conf.json、tauri.linux.conf.json），系统不画标题栏，由这里画：
// - 窗口最上面一条（--tb-h 高，和侧栏同色）：左边「收起 / 展开侧栏」，右边最小化、最大化 / 还原、关闭；整条可以拖动窗口，双击最大化 / 还原
//   （拖动、双击都是 Tauri 的 data-tauri-drag-region 做的：按下开始拖、双击切换最大化，Windows 上拖到屏幕边缘照常贴靠）。
// - 按钮长相跟着系统：Windows 11 是贴着窗口右上角的 46×32 方块，悬停浅灰、关闭悬停红底白叉；
//   Linux 照 GNOME（libadwaita）的标题栏：24px 的浅灰圆钮，关闭也不变红。
// - 窗口不在前台时图标变淡（两个系统都这样），最大化以后中间那个换成「还原」。
// macOS 不走这里：用系统的红绿灯（Sidebar.tsx 的 WindowDots 只留位置；在浏览器里看演示时画三个圆点）。
// 三个按钮没有现成的组件（affectUI、HeroUI 开源版里都没有窗口按钮），按系统的样子自己画；「收起侧栏」用 affectUI 的 IconButton。
// 动效（docs/MOTION.md 1.2、课 01/05「Timing and purpose」、01/09「Practical Animation Tips」、02/03「Transitions」）：
// 悬停只变底色和图标颜色，120ms ease；按下也只变底色，不缩放——系统自己的窗口按钮就是这样，贴着窗口边的按钮一缩就露出缝；
// 只在有鼠标的设备上响应悬停；减少动态效果时照样只是颜色变化（不涉及位移、缩放）。样式在 theme.css「标题条和窗口按钮」。
import { SidebarLeftIcon } from "@hugeicons/core-free-icons";
import { useEffect, useState } from "react";
import { IconButton } from "@/components/ui/icon-button";
import { t } from "@/i18n";
import { native, platform, windowCtl } from "@/lib/bridge";

/** 窗口是不是最大化、在不在前台。同时写到 <html data-win-max / data-win-inactive> 上，样式按它换（Linux 最大化时不画窗口边框） */
function useWindowState() {
  const [max, setMax] = useState(false);
  const [active, setActive] = useState(() => document.hasFocus());

  useEffect(() => {
    const on = () => setActive(true);
    const off = () => setActive(false);
    addEventListener("focus", on);
    addEventListener("blur", off);
    return () => {
      removeEventListener("focus", on);
      removeEventListener("blur", off);
    };
  }, []);

  useEffect(() => {
    if (!native) return;
    let dead = false;
    let unlisten: (() => void) | null = null;
    import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
      if (dead) return;
      const w = getCurrentWindow();
      const check = () => w.isMaximized().then((m) => !dead && setMax(m)).catch(() => undefined);
      check();
      // 最大化、还原、贴靠、拖动边缘改大小都会触发 resized：每次问一下现在是不是最大化
      w.onResized(check).then((u) => (dead ? u() : (unlisten = u)));
    });
    return () => {
      dead = true;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    const el = document.documentElement;
    el.toggleAttribute("data-win-max", max);
    el.toggleAttribute("data-win-inactive", !active);
  }, [max, active]);

  return { max, active, setMax };
}

/** 窗口最上面那一条（只在 Windows、Linux 上画） */
export function TitleBar({ collapsed, onToggle }: { collapsed: boolean; onToggle: () => void }) {
  return (
    <div className="cp-titlebar" data-tauri-drag-region>
      <span className="cp-titlebar-lead">
        <IconButton icon={SidebarLeftIcon} label={collapsed ? t("展开侧栏") : t("收起侧栏")} size="sm" onClick={onToggle} />
      </span>
      <span className="cp-titlebar-fill" data-tauri-drag-region />
      <WindowControls />
    </div>
  );
}

function WindowControls() {
  const { max, setMax } = useWindowState();
  const os = platform === "win" ? "win" : "linux";
  // 在浏览器里看演示时没有真窗口：点最大化只换图标，方便截图看两种样子
  const toggleMax = () => {
    windowCtl("max");
    if (!native) setMax((m) => !m);
  };
  return (
    // 系统的窗口按钮不在 Tab 顺序里（键盘用 Alt+F4、Win / Super + 方向键），这里也一样
    <span className="cp-wc" data-os={os} role="group" aria-label={t("窗口")}>
      <button type="button" tabIndex={-1} className="cp-wc-btn" data-kind="min" aria-label={t("最小化")} title={t("最小化")} onClick={() => windowCtl("min")}>
        <Glyph os={os} kind="min" />
      </button>
      <button
        type="button"
        tabIndex={-1}
        className="cp-wc-btn"
        data-kind="max"
        aria-label={max ? t("还原§窗口按钮：从最大化还原") : t("最大化§窗口按钮")}
        title={max ? t("还原§窗口按钮：从最大化还原") : t("最大化§窗口按钮")}
        onClick={toggleMax}
      >
        <Glyph os={os} kind={max ? "restore" : "max"} />
      </button>
      <button type="button" tabIndex={-1} className="cp-wc-btn" data-kind="close" aria-label={t("关闭")} title={t("关闭")} onClick={() => windowCtl("close")}>
        <Glyph os={os} kind="close" />
      </button>
    </span>
  );
}

/** 按钮里的图形。Windows 照 Segoe Fluent Icons 的窗口按钮（10px、1px 线、方块带一点圆角）；
 *  Linux 照 Adwaita 的 window-*-symbolic（16px、1.5px 线、圆头） */
function Glyph({ os, kind }: { os: "win" | "linux"; kind: "min" | "max" | "restore" | "close" }) {
  if (os === "win")
    return (
      <svg width="10" height="10" viewBox="0 0 10 10" fill="none" stroke="currentColor" strokeWidth="1" aria-hidden focusable="false">
        {kind === "min" && <path d="M0 5.5h10" />}
        {kind === "max" && <rect x="0.5" y="0.5" width="9" height="9" rx="1.25" />}
        {kind === "restore" && (
          <>
            <rect x="0.5" y="2.5" width="7" height="7" rx="1" />
            <path d="M2.5 2.5v-.75A1.25 1.25 0 0 1 3.75.5h4.5A1.25 1.25 0 0 1 9.5 1.75v4.5A1.25 1.25 0 0 1 8.25 7.5H7.5" />
          </>
        )}
        {kind === "close" && <path d="M.5.5l9 9m0-9l-9 9" />}
      </svg>
    );
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" aria-hidden focusable="false">
      {kind === "min" && <path d="M4.75 10.75h6.5" />}
      {kind === "max" && <rect x="4.75" y="4.75" width="6.5" height="6.5" rx="1" />}
      {kind === "restore" && (
        <>
          <rect x="4.25" y="6.25" width="5.5" height="5.5" rx="1" />
          <path d="M6.25 4.25h4.5a1 1 0 0 1 1 1v4.5" />
        </>
      )}
      {kind === "close" && <path d="M5 5l6 6m0-6l-6 6" />}
    </svg>
  );
}
