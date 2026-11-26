// 上手引导的四页。版式都一样：标题、导语、三条要点、一行操作、注脚（照设计稿 6-onboarding-*.png，三条要点的文字也照设计稿）。
// 「一行操作」接住旧向导（第二轮原生 HTML 界面的 onboarding.js，已删除，git 历史里有）每一步能做的事，结果都看真实状态：
//   1 存进 Mac：复制链接时提醒（设置 clipboard）；外部工具缺了才在注脚换成提醒（deps_status）
//   2 浏览器：选浏览器（设置 browser）+ 安装插件（extension_install_help，等 extension_status 连上）
//   3 语音转文字：下载语音识别（lib/whisper.ts，进度和设置弹窗共用一份）
//   4 AI 助手：复制给 Claude 的设置、连接状态（mcp_status）；「你说了算」那条右边是总开关（设置 mcpOn）
import {
  AiBrain01Icon,
  AiTranscribeAudioIcon,
  Album02Icon,
  AlertCircleIcon,
  ArrowDown01Icon,
  BrowserIcon,
  CheckmarkCircle02Icon,
  ClipboardIcon,
  ClosedCaptionIcon,
  Copy01Icon,
  FolderLibraryIcon,
  LaptopIcon,
  Link01Icon,
  Mic01Icon,
  PuzzleIcon,
  Search01Icon,
  Shield01Icon,
  SquareLock02Icon,
  Tick02Icon,
  Video02Icon,
} from "@hugeicons/core-free-icons";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { joinList, N_, t, tAny, type Key } from "@/i18n";
import { copyText, errText, platform } from "@/lib/bridge";
import { etaText } from "@/lib/format";
import { notify } from "@/lib/notify";
import { setSettings, useApp } from "@/lib/store";
import type { DepStatus } from "@/lib/types";
import { installWhisper, modelInfo, useWhisper } from "@/lib/whisper";
import { Icon, IconSwap, ProgressBar, Swap, type HugeIcon } from "@/parts";
import { useConfigWriter } from "@/pages/agent/ConfigWrite";
import { seenBrowser } from "@/pages/extension/model";
import { ART, type Art } from "./scenes";
import { installExtension, useOb } from "./state";

/** title 是原文（模块顶层的表，用 N_ 标出），显示时 t(title) */
export type PageDef = { key: string; title: Key; art: Art; Body: (p: { id: string; title: string }) => ReactNode };

type Item = { icon: HugeIcon; name: string; d: string; end?: ReactNode };

/** 一页的版式 */
function Layout({ id, title, lead, items, action, note }: { id: string; title: string; lead: string; items: Item[]; action: ReactNode; note: ReactNode }) {
  return (
    <>
      <h2 id={id}>{title}</h2>
      <p className="lead">{lead}</p>
      <div className="cp-ob-list">
        {items.map((it) => (
          <div className="cp-ob-item" key={it.name} data-end={it.end ? "" : undefined}>
            <span className="tile">
              <Icon icon={it.icon} size={19} />
            </span>
            <div>
              <strong>{it.name}</strong>
              <span>{it.d}</span>
            </div>
            {it.end}
          </div>
        ))}
      </div>
      {action}
      {typeof note === "string" ? <p className="cp-ob-note">{note}</p> : note}
    </>
  );
}

/** 「一行操作」：浅灰凹面里的一行（参考录屏里「下载本地模型」那一行）。
 *  状态变了（没装 → 等它连上 → 已连上）说明和右边的按钮一起换：新的从上方 8px 带 4px 模糊进来，旧的往下淡出，弹簧 0.4s 不回弹（parts 的 Swap，配方 g3）。
 *  同一个状态里的数值（安装进度）原地更新，不重播动画 */
function Action({ icon, title, state, sub, tone, end }: { icon: HugeIcon; title: string; state: string; sub: ReactNode; tone?: "ok" | "warn"; end?: ReactNode }) {
  return (
    <div className="cp-ob-do">
      <span className="tile">
        <Icon icon={icon} size={18} />
      </span>
      <div className="txt">
        <b>{title}</b>
        <Swap k={state}>
          <div className="sub" data-tone={tone}>
            {tone && <Icon icon={tone === "ok" ? CheckmarkCircle02Icon : AlertCircleIcon} size={14} />}
            {/* 说明最多两行（后端给的出错原因可能很长，完整的放在悬停提示里），这一行的高度就不会变，卡片也不会跟着变高 */}
            {typeof sub === "string" ? (
              <span className="line" title={sub}>
                {sub}
              </span>
            ) : (
              <div className="rich">{sub}</div>
            )}
          </div>
        </Swap>
      </div>
      <Swap k={state} className="end">
        <div className="acts">{end}</div>
      </Swap>
    </div>
  );
}

/** 只有一个开关的「一行操作」：整行文字是开关的标签，点文字也能切换（和设置里的开关行一样） */
function SwitchAction({ icon, title, sub, id, checked, onChange }: { icon: HugeIcon; title: string; sub: string; id: string; checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <div className="cp-ob-do">
      <span className="tile">
        <Icon icon={icon} size={18} />
      </span>
      <label className="txt" htmlFor={id}>
        <b>{title}</b>
        <span className="sub">{sub}</span>
      </label>
      <div className="end">
        <Switch id={id} checked={checked} onChange={(e) => onChange(e.target.checked)} />
      </div>
    </div>
  );
}

/* ---------------- 1 把网上的视频存进 Mac ---------------- */

const TOOL_NAME: Record<string, Key> = { ytdlp: N_("网站视频下载（yt-dlp）"), ffmpeg: N_("格式转换（ffmpeg）"), aria2: N_("加速下载（aria2）") };

/** 下载要用的外部工具里缺了哪几个（还没查到时当作都在，不闪警告） */
function missingTools(deps: DepStatus[] | null): string[] {
  if (!deps) return [];
  return Object.keys(TOOL_NAME).filter((k) => !deps.find((d) => d.key === k)?.ok);
}

function SaveBody({ id, title }: { id: string; title: string }) {
  const clip = useApp((s) => s.settings?.clipboard ?? true);
  const deps = useApp((s) => s.deps);
  const missing = missingTools(deps);
  return (
    <Layout
      id={id}
      title={title}
      lead={t("粘贴一个链接，Captae 挑好清晰度、下载、存成好用的文件。")}
      items={[
        { icon: Link01Icon, name: t("粘贴就能下"), d: t("YouTube、B 站、X 等上千个网站") },
        { icon: SquareLock02Icon, name: t("要登录的也能下"), d: t("在 Captae 里登录一次，以后自动带上") },
        { icon: Album02Icon, name: t("自动整理"), d: t("下好的视频进素材库，能搜到字幕里的话") },
      ]}
      action={
        <SwitchAction
          icon={ClipboardIcon}
          title={t("复制链接时问我要不要下载")}
          sub={t("复制了视频链接、1 分钟内回到 Captae 时，在输入框上方问一句")}
          id="cp-ob-clip"
          checked={clip}
          onChange={(on) => setSettings({ clipboard: on })}
        />
      }
      note={
        missing.length ? (
          <p className="cp-ob-note" data-tone="warn">
            <Icon icon={AlertCircleIcon} size={14} />
            <span>{t("有的组件没找到：{list}，对应的功能暂时用不了。重新安装 Captae 通常就能解决。", { list: joinList(missing.map((k) => t(TOOL_NAME[k]))) })}</span>
          </p>
        ) : (
          t("下载的文件、登录状态和素材都只留在这台电脑上。")
        )
      }
    />
  );
}

/* ---------------- 2 在浏览器里一键下载 ---------------- */

/** extension_install_help 认识的浏览器（main.rs）；别的值它会当成 Chrome */
const BROWSERS = [
  { value: "chrome", label: "Chrome" },
  { value: "edge", label: "Edge" },
  { value: "brave", label: "Brave" },
  { value: "arc", label: "Arc" },
];

function BrowserBody({ id, title }: { id: string; title: string }) {
  const browser = useApp((s) => s.settings?.browser) || "chrome";
  const ext = useOb((s) => s.ext);
  const wait = useOb((s) => s.extWait);
  const err = useOb((s) => s.extErr);
  const known = BROWSERS.find((b) => b.value === browser);
  const name = known?.label ?? "Chrome";
  const state = ext?.connected ? "on" : err ? "err" : wait ? "wait" : "off";
  const sub =
    state === "on"
      ? // 连上来的浏览器：宿主看父进程认出来的（16 家都认得，Arc、Aside 不会再说成 Chrome），名字和「浏览器插件」页同一张表
        [ext?.browser ? t("已连上{browser}", { browser: seenBrowser(ext.browser) }) : t("已连上"), ext?.version ? t("插件{version}", { version: ext.version }) : ""].filter(Boolean).join(" · ")
      : state === "err"
        ? err
        : state === "wait"
          ? t("在打开的页面里打开「开发者模式」，点「加载已解压的扩展程序」，选 extension 文件夹")
          : t("会打开{name}的扩展程序页面，照着装好就行", { name });
  return (
    <Layout
      id={id}
      title={title}
      lead={t("装上插件，网页里的视频旁边会出现「下载」按钮。")}
      items={[
        { icon: Video02Icon, name: t("看到就能下"), d: t("视频、声音、图片和 M3U8 都能找到") },
        { icon: ArrowDown01Icon, name: t("先看每一档多大"), d: t("选好清晰度再下，不浪费空间") },
        { icon: BrowserIcon, name: t("带上网页里的登录"), d: t("在浏览器里登录过的网站，直接能下") },
      ]}
      action={
        <Action
          icon={PuzzleIcon}
          title={t("装上 Captae 插件")}
          state={state}
          tone={state === "on" ? "ok" : state === "err" ? "warn" : undefined}
          sub={sub}
          end={
            state === "on" ? null : state === "wait" ? (
              // 等它连上的时候浏览器已经选定了，下拉收起来，把地方让给安装步骤
              <Button size="sm" variant="secondary" className="cp-pill" loading>
                {t("等它连上")}
              </Button>
            ) : (
              <>
                <Select
                  size="sm"
                  aria-label={t("你用的浏览器")}
                  className="cp-ob-browser"
                  value={known ? browser : "chrome"}
                  onChange={(v) => setSettings({ browser: v })}
                  options={BROWSERS}
                />
                <Button size="sm" variant="secondary" className="cp-pill" onClick={() => installExtension(known ? browser : "chrome")}>
                  {t("安装插件")}
                </Button>
              </>
            )
          }
        />
      }
      note={t("插件只在你点按钮时才把链接交给 Captae。")}
    />
  );
}

/* ---------------- 3 把视频里说的话转成文字 ---------------- */

function WhisperBody({ id, title }: { id: string; title: string }) {
  const { state, model, task } = useWhisper();
  const tasks = useApp((s) => s.tasks);
  const m = modelInfo(model);
  // 最近一次安装（task 事件 kind = setup）没成功：说明里写原因，按钮换成「重试」
  const last = useMemo(
    () =>
      Object.values(tasks)
        .filter((x) => x.kind === "setup")
        .sort((a, b) => b.id - a.id)[0],
    [tasks],
  );
  const failed = (state === "need_install" || state === "need_model") && last?.st === "err" ? last.err || t("安装没有完成") : "";
  const key = failed ? "err" : state;
  const pct = Math.round(task?.pct ?? 0);
  const sub =
    key === "err" ? (
      t("没有装好：{error}", { error: failed })
    ) : state === "checking" ? (
      t("正在检查…")
    ) : state === "ready" ? (
      t("可以用了 · 「{name}」模型，在这台电脑上识别", { name: m.label })
    ) : state === "installing" ? (
      // 两行：说明和剩余时间一行，进度条一行（和设置里的安装进度同一份数据）
      <>
        <span className="num">
          {task?.note || t("正在准备…")} · {etaText(task?.eta) || `${pct}%`}
        </span>
        <ProgressBar p={pct / 100} indeterminate={!task || pct <= 0} tick={900} label={t("安装语音识别")} />
      </>
    ) : state === "need_model" ? (
      t("组件已经装好，还要下载「{name}」模型 · {size}", { name: m.label, size: m.model })
    ) : (
      t("「{name}」模型 · 要下载{size}，以后在设置里也能下", { name: m.label, size: m.full })
    );
  return (
    <Layout
      id={id}
      title={title}
      lead={t("装上语音识别，没有字幕的视频也能生成字幕和文字稿。")}
      items={[
        { icon: LaptopIcon, name: t("在这台电脑上识别"), d: t("声音不上传，没有网也能转") },
        { icon: Search01Icon, name: t("能搜到说的话"), d: t("在素材库里按台词找到视频") },
        { icon: ClosedCaptionIcon, name: t("字幕拿去就能用"), d: t("导出 SRT、VTT 或纯文字，也能烧进画面") },
      ]}
      action={
        <Action
          icon={AiTranscribeAudioIcon}
          title={t("语音识别")}
          state={key}
          tone={key === "ready" ? "ok" : key === "err" ? "warn" : undefined}
          sub={sub}
          end={
            key === "err" || state === "need_install" || state === "need_model" ? (
              <Button size="sm" variant="secondary" className="cp-pill" onClick={installWhisper}>
                {key === "err" ? t("重试") : t("下载")}
              </Button>
            ) : null
          }
        />
      }
      note={t("可以先跳过，以后在设置的「字幕」里下载。")}
    />
  );
}

/* ---------------- 4 让 AI 助手也能用 ---------------- */

/** 给 Claude 的设置：和旧界面、AI 助手页同一段（命令由桌面端算好：装在 PATH 上是 captae，否则是安装包里的完整路径） */
function claudeConfig(cmd: unknown): string {
  const command = typeof cmd === "string" && cmd ? cmd : "captae";
  return JSON.stringify({ mcpServers: { captae: { command, args: ["mcp", "--stdio"] } } }, null, 2);
}

function AgentBody({ id, title }: { id: string; title: string }) {
  const mcpOn = useApp((s) => s.settings?.mcpOn ?? true);
  const clients = useApp((s) => s.mcpClients);
  const cmd = useApp((s) => s.mcpCommand);
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const timer = window.setTimeout(() => setCopied(false), 2000);
    return () => window.clearTimeout(timer);
  }, [copied]);
  // 一键连接：和「AI 助手」页同一个确认弹窗（先给看会改哪个文件、改成什么，确认了才写）；写好以后要重启 Claude 才连得上
  const [written, setWritten] = useState(false);
  const writer = useConfigWriter(null, () => setWritten(true));
  const live = (clients ?? []).filter((c) => c.connected);
  const state = !mcpOn ? "off" : live.length ? "on" : "idle";
  const sub =
    state === "off"
      ? t("已经关掉，AI 助手现在用不了 Captae")
      : state === "on"
        ? // 客户端没报名字时后端存的是「AI 助手」，显示时翻译
          t("已连上：{names}", { names: joinList(live.map((c) => tAny(c.name))) })
        : written
          ? t("已经写好了，重启 Claude 以后就能用；Cursor 等在「AI 助手」页连接")
          : t("点「连接」把 Captae 写进 Claude Desktop 的设置；Cursor 等在「AI 助手」页连接");
  const copy = () =>
    copyText(claudeConfig(cmd))
      .then(() => setCopied(true))
      .catch((e) => notify(t("没有复制"), errText(e)));
  return (
    <Layout
      id={id}
      title={title}
      lead={t("Claude、Cursor 这些 AI 助手可以直接让 Captae 下载、找素材、转文字。")}
      items={[
        { icon: FolderLibraryIcon, name: t("查素材库"), d: t("按标题、标签、字幕里的话找视频") },
        { icon: Mic01Icon, name: t("转成文字"), d: t("没有字幕的视频，在本机识别成文字") },
        {
          icon: Shield01Icon,
          name: t("你说了算"),
          d: t("随时可以关掉，或者只开放一部分"),
          end: <Switch aria-label={t("让 AI 助手使用 Captae")} checked={mcpOn} onChange={(e) => setSettings({ mcpOn: e.target.checked })} />,
        },
      ]}
      action={
        <Action
          icon={AiBrain01Icon}
          title={t("连接 Claude")}
          state={state}
          tone={state === "on" ? "ok" : undefined}
          sub={sub}
          end={
            <>
              <Button
                size="sm"
                variant="ghost"
                onClick={copy}
                icon={
                  <IconSwap k={copied ? "done" : "copy"}>
                    <Icon icon={copied ? Tick02Icon : Copy01Icon} size={14} />
                  </IconSwap>
                }
              >
                {copied ? t("已复制") : t("复制")}
              </Button>
              {state !== "on" && (
                <Button size="sm" variant="secondary" className="cp-pill" loading={writer.busy} onClick={() => writer.start("claude")}>
                  {t("连接")}
                </Button>
              )}
              {writer.dialog}
            </>
          }
        />
      }
      note={t("AI 助手看不到你的登录状态；它让 Captae 下载时，和你自己下载一样会带上。")}
    />
  );
}

export const PAGES: PageDef[] = [
  { key: "save", title: platform === "mac" ? N_("把网上的视频存进 Mac") : N_("把网上的视频存进电脑"), art: ART.save, Body: SaveBody },
  { key: "browser", title: N_("在浏览器里一键下载"), art: ART.browser, Body: BrowserBody },
  { key: "whisper", title: N_("把视频里说的话转成文字"), art: ART.whisper, Body: WhisperBody },
  { key: "agent", title: N_("让 AI 助手也能用"), art: ART.agent, Body: AgentBody },
];
