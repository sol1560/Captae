// 侧栏（蓝本：Perplexity Computer Mac 版的侧栏）：logo、导航、进行中、浏览器里发现的、预设、AI 助手状态。
// 侧栏是最高频的操作：悬停和选中都即时，不做过渡（实测 m2 I 节也是一帧切换；配方 a）。
import {
  AiBrain01Icon,
  Album02Icon,
  Download04Icon,
  FileVideoIcon,
  Image01Icon,
  MusicNote01Icon,
  PlayListIcon,
  PuzzleIcon,
  Search01Icon,
  Settings02Icon,
  SidebarLeftIcon,
  SparklesIcon,
} from "@hugeicons/core-free-icons";
import { useMemo } from "react";
import { IconButton } from "@/components/ui/icon-button";
import { joinList, t, tAny } from "@/i18n";
import { native, platform, windowCtl } from "@/lib/bridge";
import { shortcut } from "@/lib/os";
import { go, useRoute, type Page } from "@/lib/route";
import { isActive, useApp } from "@/lib/store";
import { CaptaeLogo, Icon, MiniRing, type HugeIcon } from "@/parts";
import { Checklist } from "@/dialogs/Onboarding";
import { downloadSniffed, openDialog } from "./actions";
import { UpdateRow } from "./AppUpdate";

const SNIFF_ICON: Record<string, HugeIcon> = { list: PlayListIcon, film: FileVideoIcon, music: MusicNote01Icon, image: Image01Icon };
/** 侧栏「浏览器里发现的」最多放几条 */
const SIDE_FOUND = 3;

/** macOS 的红绿灯：Captae 里用系统自己的（位置在 tauri.conf.json 里定），这里只留出位置；
 *  在浏览器里看演示数据时没有系统的，画三个圆点，截图才和真窗口一样。
 *  Windows、Linux 不用红绿灯：窗口按钮在右上角，跟着系统的样子（WindowControls.tsx） */
export function WindowDots() {
  if (platform !== "mac") return null;
  if (native) return <span className="cp-lights-space" aria-hidden />;
  return (
    <span className="cp-traffic" aria-label={t("窗口")}>
      <button className="dot" aria-label={t("关闭")} onClick={() => windowCtl("close")} />
      <button className="dot" aria-label={t("最小化")} onClick={() => windowCtl("min")} />
      <button className="dot" aria-label={t("放大§窗口的最大化按钮")} onClick={() => windowCtl("max")} />
    </span>
  );
}

export function Sidebar({ onToggle }: { onToggle: () => void }) {
  const { page } = useRoute();
  const jobs = useApp((s) => s.jobs);
  const sniffed = useApp((s) => s.sniffed);
  // 「浏览器里发现的」侧栏只放最近 3 条视频和声音：开着很多网页时找到的东西很多（图片最多），全列出来太占地方；
  // 全部（含图片）在「浏览器插件」页，标题右边的链接写着一共几个
  const found = sniffed.filter((s) => s.i !== "image").slice(0, SIDE_FOUND);
  const presets = useApp((s) => s.presets);
  const libraryCount = useApp((s) => s.libraryCount);
  const settings = useApp((s) => s.settings);
  const clients = useApp((s) => s.mcpClients);

  // 「下载」后面的数和下载页「进行中」页签一致：还没下完的都算，包括要登录、要验证、失败了等你处理的
  const { active, open } = useMemo(() => {
    const all = Object.values(jobs);
    return {
      active: all.filter(isActive).sort((a, b) => a.pri - b.pri),
      open: all.filter((j) => j.state !== "done").length,
    };
  }, [jobs]);

  const nav: [Page, HugeIcon, string, number | ""][] = [
    ["downloads", Download04Icon, t("下载§侧栏、顶栏里的页面名（Downloads）"), open || ""],
    ["library", Album02Icon, t("素材库"), libraryCount || ""],
    ["agent", AiBrain01Icon, t("AI 助手"), ""],
    ["extension", PuzzleIcon, t("浏览器插件"), ""],
  ];

  // 「AI 助手」那一行的真实连接状态（mcp_status，30 秒看一次）
  const mcpOn = settings?.mcpOn ?? true;
  const live = clients ? clients.filter((c) => c.connected) : null;
  // 右边只写状态，不写 AI 助手的名字（名字可能很长，比如 local-agent-mode-captae，挤在这里只剩半截）；
  // 名字在鼠标停上去的提示里，AI 助手页列着全部
  const mcpText = !mcpOn ? t("已关闭") : !live ? t("已开启") : live.length > 1 ? t("已连接{n}个", { n: live.length }) : live.length ? t("已连接") : t("未连接");
  // 客户端没报名字时后端存的是「AI 助手」（原文），显示时翻译
  const mcpTip = !mcpOn ? t("AI 助手现在用不了 Captae") : !live ? "" : live.length ? t("已连接：{names}", { names: joinList(live.map((c) => tAny(c.name))) }) : t("现在没有 AI 助手连着");

  return (
    <aside className="cp-side" aria-label={t("侧栏")}>
      {/* macOS：红绿灯 + 收起侧栏；Windows、Linux 的这两样在窗口最上面的标题条里（WindowControls.tsx 的 TitleBar） */}
      {platform === "mac" && (
        <div className="cp-side-top" data-tauri-drag-region>
          <WindowDots />
          <span className="end">
            <IconButton icon={SidebarLeftIcon} label={t("收起侧栏")} size="sm" onClick={onToggle} />
          </span>
        </div>
      )}

      <div className="cp-side-scroll">
        <div className="cp-brand">
          <button className="cp-brand-home" onClick={() => go("downloads")} aria-label={t("Captae，回到下载")}>
            <CaptaeLogo size={22} />
            Captae
          </button>
          <span className="end">
            <IconButton icon={Search01Icon} label={t("搜索素材 {key}", { key: shortcut("K") })} size="sm" onClick={() => go("library", { search: true })} />
          </span>
        </div>

        <nav className="cp-nav" aria-label={t("页面")}>
          {nav.map(([id, icon, label, count]) => (
            <button key={id} className="cp-nav-item" aria-current={page === id ? "page" : undefined} onClick={() => go(id)}>
              <Icon icon={icon} size={16} />
              {label}
              {count !== "" && <span className="count">{count}</span>}
            </button>
          ))}
        </nav>

        {active.length > 0 && (
          <>
            <div className="cp-side-label">{t("进行中")}</div>
            {active.slice(0, 4).map((j) => (
              <button key={j.id} className="cp-side-row" onClick={() => go("downloads", { t: j.id })}>
                <span className="glyph">
                  {/* 只有真在干活、又不知道进度（读取中、不知道大小）时才呼吸；暂停和排队是静止的 */}
                  <MiniRing p={j.size ? j.got / j.size : 0} busy={j.state === "run" && (j.phase === "probe" || (j.phase === "dl" && !j.size))} />
                </span>
                <span className="t">{j.phase === "probe" && j.state === "run" ? t("正在读取…") : j.title}</span>
                {j.state !== "run" && <span className="trail">{j.choose ? t("等你选") : j.state === "pause" ? t("已暂停") : j.boost ? t("正在开始") : t("排队")}</span>}
              </button>
            ))}
          </>
        )}

        <div className="cp-side-label">
          {t("浏览器里发现的")}
          <button className="cp-side-link" onClick={() => go("extension")}>
            {sniffed.length > found.length ? t("全部{n}个", { n: sniffed.length }) : t("看看插件")}
          </button>
        </div>
        {found.length ? (
          found.map((s) => (
            <button key={s.url} className="cp-side-row" title={t("点一下下载 · {meta}", { meta: s.m })} onClick={() => downloadSniffed(s.url)}>
              <span className="glyph">
                <Icon icon={SNIFF_ICON[s.i] ?? FileVideoIcon} size={15} />
              </span>
              <span className="t">{s.t}</span>
              <span className="cp-dot-new" aria-label={t("新发现")} />
            </button>
          ))
        ) : (
          <div className="cp-side-empty">{t("在浏览器里看视频时，能下载的会出现在这里")}</div>
        )}

        {presets.length > 0 && (
          <>
            <div className="cp-side-label">
              {t("预设")}
              <button className="cp-side-link" onClick={() => go("agent", { tab: "presets" })}>
                {t("管理")}
              </button>
            </div>
            {presets.map((p) => (
              <button key={p.name} className="cp-side-row" onClick={() => go("agent", { tab: "presets" })}>
                <span className="glyph">
                  <Icon icon={SparklesIcon} size={15} />
                </span>
                <span className="t">{p.name}</span>
                <span className="trail num">{p.steps.length}</span>
              </button>
            ))}
          </>
        )}
      </div>

      <Checklist />
      <UpdateRow />

      <div className="cp-side-foot">
        <button className="cp-side-row" title={mcpTip} onClick={() => go("agent")}>
          <span className="cp-live" data-off={!mcpOn || live?.length === 0 || undefined} />
          {/* 这一行很窄（名字 + 连接状态只有约 134px）：名字单独一条，别的语言可以译得比导航里的页面名短 */}
          <span className="t">{t("AI 助手§侧栏底部那一行的名字，后面跟着连接状态，地方很窄，尽量短")}</span>
          <span className="trail">{mcpText}</span>
        </button>
        <IconButton icon={Settings02Icon} label={t("设置")} size="sm" onClick={() => openDialog("settings")} />
      </div>
    </aside>
  );
}
