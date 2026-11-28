// 窗口骨架：侧栏 + 主区面板 + 弹窗 + 提示条，以及几件全局的事（剪贴板提示、下载完成提示、⌘K）。
import { SidebarLeftIcon } from "@hugeicons/core-free-icons";
import { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { Toaster } from "sonner";
import { IconButton } from "@/components/ui/icon-button";
import { t } from "@/i18n";
import { addJob, onEvent, useApp } from "@/lib/store";
import { errText, invoke } from "@/lib/bridge";
import { bare } from "@/lib/format";
import { notify } from "@/lib/notify";
import { isMac, modKey } from "@/lib/os";
import { getPref, setPref } from "@/lib/prefs";
import { go, useRoute } from "@/lib/route";
import { Downloads } from "@/pages/Downloads";
import { Library } from "@/pages/Library";
import { Agent } from "@/pages/Agent";
import { Extension } from "@/pages/Extension";
import { SettingsDialog } from "@/dialogs/Settings";
import { OnboardingHost } from "@/dialogs/Onboarding";
import { getDialog, openDialog, subscribeDialog } from "./actions";
import { AppUpdateHost } from "./AppUpdate";
import { Sidebar, WindowDots } from "./Sidebar";
import { TitleBar } from "./WindowControls";

const narrowQuery = matchMedia("(max-width: 760px)");

export function Window() {
  const { page } = useRoute();
  const offline = useApp((s) => s.offline);
  // 下载页有任务时输入框浮在底部，提示条要停在它上面，不能盖住它
  const dockBelow = useApp((s) => page === "downloads" && !s.offline && Object.keys(s.jobs).length > 0);
  const dialog = useSyncExternalStore(subscribeDialog, getDialog);
  const [sideOff, setSideOff] = useState(() => getPref("sideOff", false));
  const [narrow, setNarrow] = useState(narrowQuery.matches);
  const [sideOpen, setSideOpen] = useState(false); // 窄窗口时侧栏盖在上面

  useEffect(() => {
    const f = () => setNarrow(narrowQuery.matches);
    narrowQuery.addEventListener("change", f);
    return () => narrowQuery.removeEventListener("change", f);
  }, []);

  const collapsed = narrow ? !sideOpen : sideOff;
  const toggle = () => {
    if (narrow) setSideOpen((v) => !v);
    else
      setSideOff((v) => {
        setPref("sideOff", !v);
        return !v;
      });
  };

  // 换页面时，窄窗口里盖着的侧栏收起来
  useEffect(() => setSideOpen(false), [page]);

  // ⌘K（Windows、Linux 是 Ctrl+K）：去素材库搜索；Esc：收起盖着的侧栏
  useEffect(() => {
    const f = (e: KeyboardEvent) => {
      if (modKey(e) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        go("library", { search: true });
      }
      if (e.key === "Escape") setSideOpen(false);
    };
    addEventListener("keydown", f);
    return () => removeEventListener("keydown", f);
  }, []);

  // 剪贴板提示：复制了视频链接、1 分钟内把 Captae 拿到前面时后端发 clipboard 事件（复制那一刻什么都不弹，见 src-tauri/src/clip_ask.rs）。
  // 刚打开 Captae 的那一下，后端等页面开始听事件了才问：这里（子组件的监听都挂好以后）告诉它一声
  useEffect(() => {
    invoke("clip_check").catch(() => undefined);
  }, []);

  // 下载页自己在输入框上方提示；别的页面弹一条提示条
  useEffect(
    () =>
      onEvent("clipboard", (ev) => {
        if (location.hash.startsWith("#/downloads") || location.hash === "" || location.hash === "#/") return;
        notify(
          t("你刚复制了一个视频链接"),
          bare(ev.url).slice(0, 60),
          {
            label: t("下载"),
            run: () =>
              addJob({ url: ev.url })
                .then((j) => go("downloads", { t: j.id }))
                .catch((e) => notify(t("没有开始下载"), errText(e))),
          },
          { duration: 8000 },
        );
      }),
    [],
  );

  // 下载完成：在哪一页都提示，能直接去素材库看
  useEffect(
    () =>
      onEvent("finished", (ev) => {
        notify(t("下载完成"), t("{title} · 已放进素材库", { title: ev.title }), ev.asset_id != null ? { label: t("查看"), run: () => go("library", { asset: ev.asset_id! }) } : undefined);
      }),
    [],
  );

  // 第一次打开的上手引导由 OnboardingHost 自己打开：等启动快照到了、后台正常、还没看过时才出现

  // 弹窗和提示条放在网格外面：它们不是版面的一格（Sonner 的外框是普通元素，放进网格会多出一行）
  return (
    <>
      <div className="cp cp-window" data-side={collapsed ? "off" : "on"} data-narrow={narrow || undefined}>
        {/* Windows、Linux：窗口最上面一条，放「收起 / 展开侧栏」和窗口按钮（主窗口无边框，系统不画标题栏） */}
        {!isMac && <TitleBar collapsed={collapsed} onToggle={toggle} />}
        <Sidebar onToggle={toggle} />
        {/* 遮罩在窄窗口里一直挂着、用 data-open 开关：收起时和侧栏一起淡出（原来一收就卸掉，遮罩第一帧就没了，侧栏还在往外滑） */}
        {narrow && <div className="cp-side-scrim" data-open={sideOpen || undefined} onClick={() => setSideOpen(false)} />}
        {collapsed && isMac && (
          <div className="cp-side-float" data-tauri-drag-region>
            <WindowDots />
            <IconButton icon={SidebarLeftIcon} label={t("展开侧栏")} size="sm" onClick={toggle} />
          </div>
        )}
        {offline ? (
          <main className="cp-main solo">
            <div className="cp-offline">
              <b>{t("Captae 的后台没有响应")}</b>
              <span>{t("退出 Captae 再打开一次试试。还是不行的话，把下面这行字发给我们。")}</span>
              <code>{offline}</code>
            </div>
          </main>
        ) : page === "library" ? (
          <Library />
        ) : page === "agent" ? (
          <Agent />
        ) : page === "extension" ? (
          <Extension />
        ) : (
          <Downloads />
        )}
      </div>
      <SettingsDialog open={dialog === "settings"} onClose={() => openDialog(null)} />
      <OnboardingHost open={dialog === "onboarding"} onClose={() => openDialog(null)} />
      <AppUpdateHost />
      <ToastLayer offset={dockBelow ? 132 : 24} />
    </>
  );
}

/** 提示条：Sonner 原样，只换外观（unstyled + 自己的 class），进出场用它自带的动效（课 02/03 讲的就是它）。
 *  放在浏览器的「顶层」里：设置这类模态弹窗（affectUI 的 Dialog 用 showModal）也在顶层，不这样提示条会被弹窗盖住，
 *  在设置里改了保存位置、检查更新的结果都看不到。弹窗打开以后把这一层重新放一次，它就排在弹窗上面
 *  （弹窗开着时背后都是 inert 的，提示条看得见、点不了，只是提示，不影响）。不支持 popover 的老系统照旧画在页面里。 */
function ToastLayer({ offset }: { offset: number }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el || typeof el.showPopover !== "function") return;
    const lift = () => {
      try {
        if (el.matches(":popover-open")) el.hidePopover();
        el.showPopover();
      } catch {
        // 不支持或者这一刻放不上去：留在页面里
      }
    };
    lift();
    const mo = new MutationObserver((list) => {
      if (list.some((m) => m.target instanceof HTMLDialogElement && m.target.open)) lift();
    });
    mo.observe(document.body, { subtree: true, attributes: true, attributeFilter: ["open"] });
    return () => mo.disconnect();
  }, []);
  return (
    <div ref={ref} popover="manual" className="cp-toast-layer">
      <Toaster containerAriaLabel={t("通知")} position="bottom-center" offset={offset} visibleToasts={3} toastOptions={{ unstyled: true, classNames: { toast: "cp-toast", actionButton: "cp-toast-action" } }} />
    </div>
  );
}
