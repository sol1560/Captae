// 粘贴链接的输入框。一个任务都没有时在页面中间，有任务时浮在列表底部（毛玻璃），是同一个组件。
// 认链接、选保存格式、输入 @ 选预设都在这里；加任务交给页面（onSubmit）。
// 菜单都是 affectUI 的 DropdownMenu：在代码里打开时焦点会进到第一项，选完回到触发按钮，所以 @ 预设不用另写弹层。
import {
  AlertCircleIcon,
  ArrowDown01Icon,
  ArrowUp02Icon,
  Cancel01Icon,
  ClipboardIcon,
  FileVideoIcon,
  Globe02Icon,
  PlayListIcon,
  PlusSignIcon,
  Settings02Icon,
  SparklesIcon,
} from "@hugeicons/core-free-icons";
import { useRef, useState, type ReactNode, type RefObject } from "react";
import { Button } from "@/components/ui/button";
import { DropdownMenu, type DropdownMenuItem } from "@/components/ui/dropdown-menu";
import { IconButton } from "@/components/ui/icon-button";
import { Spinner } from "@/components/ui/spinner";
import { joinList, t } from "@/i18n";
import { invoke } from "@/lib/bridge";
import { detect, type Detected } from "@/lib/detect";
import { useDelayed } from "@/lib/hooks";
import { notify } from "@/lib/notify";
import { shortcut } from "@/lib/os";
import { go } from "@/lib/route";
import { getState, useApp } from "@/lib/store";
import { stepLabel } from "@/pages/agent/model";
import { Icon, IconSwap, SiteIcon } from "@/parts";
import { openDialog } from "@/shell/actions";
import { fmtLabel, MAIN_FMTS, MORE_FMTS, type FmtChoice } from "./model";

/** 输入框末尾的「@」「@抖」：要选预设 */
export const AT = /(^|\s)@\S*$/;

export type ComposerProps = {
  value: string;
  onChange: (v: string) => void;
  fmt: FmtChoice;
  onFmt: (f: FmtChoice) => void;
  preset: string;
  onPreset: (name: string) => void;
  busy: boolean;
  onSubmit: () => void;
  onPaste: () => void;
  inputRef: RefObject<HTMLInputElement | null>;
  autoFocus?: boolean;
};

export function Composer({ value, onChange, fmt, onFmt, preset, onPreset, busy, onSubmit, onPaste, inputRef, autoFocus }: ComposerProps) {
  const presets = useApp((s) => s.presets);
  const [atOpen, setAtOpen] = useState(false);
  const latest = useRef(value);
  latest.current = value;

  const clean = value.replace(AT, "").trim();
  const det = clean ? detect(clean) : null;
  const canSend = !!det && !det.unsupported && !busy;
  const slow = useDelayed(busy, 300);
  const focusInput = () => requestAnimationFrame(() => inputRef.current?.focus());

  const change = (v: string) => {
    // 刚打出「@」时打开预设菜单；网址里的 @（tiktok.com/@某人）前面不是空格，不算
    if (presets.length && AT.test(v) && !AT.test(latest.current)) setAtOpen(true);
    onChange(v);
  };
  const dropAt = () => {
    if (AT.test(latest.current)) onChange(latest.current.replace(AT, "").trimEnd());
  };
  const pickPreset = (name: string) => {
    onPreset(name);
    dropAt();
    focusInput();
    checkPreset(name);
  };

  // 预设的名字是用户自己起的（或者生成时就按当时的语言写好），原样显示
  const presetItems: DropdownMenuItem[] = [
    ...presets.map((p) => ({ label: p.name, description: t("{n}步", { n: p.steps.length }), icon: SparklesIcon, checked: preset === p.name, onSelect: () => pickPreset(p.name) })),
    ...(preset ? [{ label: t("不用预设"), icon: Cancel01Icon, divider: true, onSelect: () => onPreset("") }] : []),
    { label: t("管理预设…"), icon: Settings02Icon, divider: !preset, onSelect: () => go("agent", { tab: "presets" }) },
  ];

  return (
    <div className="cp-composer">
      <input
        ref={inputRef}
        value={value}
        onChange={(e) => change(e.target.value)}
        onKeyDown={(e) => {
          // 中文输入法选字时的回车不算
          if (e.key !== "Enter" || e.nativeEvent.isComposing) return;
          e.preventDefault();
          if (canSend) onSubmit();
        }}
        placeholder={t("粘贴视频、音乐或图片的链接")}
        aria-label={t("链接")}
        spellCheck={false}
        autoComplete="off"
        autoCorrect="off"
        autoCapitalize="off"
        autoFocus={autoFocus}
      />
      <div className="cp-composer-bar">
        <DropdownMenu
          side="top"
          aria-label={t("更多选项")}
          items={[
            { label: t("粘贴剪贴板里的链接"), icon: ClipboardIcon, shortcut: shortcut("V"), onSelect: onPaste },
            ...(presets.length ? [{ label: t("用预设…"), icon: SparklesIcon, onSelect: () => setTimeout(() => setAtOpen(true), 0) }] : []),
            { label: t("下载设置"), icon: Settings02Icon, divider: true, onSelect: () => openDialog("settings") },
          ]}
          trigger={({ triggerProps }) => <IconButton icon={PlusSignIcon} label={t("更多选项")} size="sm" title="" {...triggerProps} />}
        />
        <Hint det={det} empty={!clean} slow={slow} hasPresets={presets.length > 0} />
        <div className="end">
          {presets.length > 0 && (
            <DropdownMenu
              align="end"
              side="top"
              title={t("用预设")}
              open={atOpen}
              onOpenChange={(o) => {
                setAtOpen(o);
                if (!o) {
                  dropAt();
                  focusInput();
                }
              }}
              items={presetItems}
              trigger={({ triggerProps }) =>
                preset ? (
                  <Button variant="secondary" size="sm" className="cp-pill" icon={<Icon icon={SparklesIcon} size={14} />} {...triggerProps}>
                    {preset}
                  </Button>
                ) : (
                  <Button variant="ghost" size="sm" {...triggerProps}>
                    {t("@ 预设")}
                  </Button>
                )
              }
            />
          )}
          {det?.kind === "hls" ? (
            <Button variant="ghost" size="sm" disabled title={t("M3U8 读到以后再选清晰度，最后存成 MP4")}>
              {t("读到后选清晰度")}
            </Button>
          ) : (
            <DropdownMenu
              align="end"
              side="top"
              title={t("保存成")}
              items={[
                ...MAIN_FMTS.map((k) => ({ label: fmtLabel(k), description: k === "best" ? t("推荐，哪里都能播") : undefined, checked: fmt === k, onSelect: () => onFmt(k) })),
                { label: t("其他格式"), divider: true, submenu: MORE_FMTS.map((k) => ({ label: fmtLabel(k), checked: fmt === k, onSelect: () => onFmt(k) })) },
              ]}
              trigger={({ triggerProps }) => (
                <Button variant="ghost" size="sm" iconRight={<Icon icon={ArrowDown01Icon} size={14} />} {...triggerProps}>
                  {fmtLabel(fmt)}
                </Button>
              )}
            />
          )}
          <button className="cp-send" aria-label={slow ? t("正在识别链接") : t("开始下载")} aria-busy={busy || undefined} disabled={!canSend} onClick={onSubmit}>
            <IconSwap k={slow ? "busy" : "go"}>{slow ? <Spinner size={14} /> : <Icon icon={ArrowUp02Icon} size={16} stroke={2} />}</IconSwap>
          </button>
        </div>
      </div>
    </div>
  );
}

/** 输入框下面那一小行：认出来是什么、会怎么下；不像链接时提醒一下。它跟着打字变，按课程的规矩不做动画，直接换 */
function Hint({ det, empty, slow, hasPresets }: { det: Detected | null; empty: boolean; slow: boolean; hasPresets: boolean }) {
  let k: string;
  let tone: "warn" | undefined;
  let body: ReactNode;
  if (slow) {
    k = "busy";
    body = (
      <>
        <Spinner size={13} />
        <span>{t("正在识别链接…")}</span>
      </>
    );
  } else if (empty) {
    k = "idle";
    body = <span>{hasPresets ? t("回车开始 · 输入 @ 选预设") : t("回车开始下载")}</span>;
  } else if (!det) {
    k = "bad";
    tone = "warn";
    body = (
      <>
        <Icon icon={AlertCircleIcon} size={14} />
        <span className="note">{t("链接好像不完整，检查一下有没有复制全")}</span>
      </>
    );
  } else {
    k = det.label;
    tone = det.unsupported ? "warn" : undefined;
    body = (
      <>
        {det.unsupported ? (
          <Icon icon={AlertCircleIcon} size={14} />
        ) : det.site ? (
          <SiteIcon site={det.site} size={14} />
        ) : (
          <Icon icon={det.kind === "hls" ? PlayListIcon : det.kind === "file" ? FileVideoIcon : Globe02Icon} size={14} />
        )}
        <b>{det.label}</b>
        <span className="note">{det.note}</span>
      </>
    );
  }
  return (
    <div className="cp-hint" data-k={k}>
      <span className="cp-hint-row" data-tone={tone}>
        {body}
      </span>
    </div>
  );
}

/** 预设里有后端认不出的步骤时，选的时候就说清楚，不要等下载完才悄悄跳过（旧界面的 preset_check） */
function checkPreset(name: string) {
  const p = getState().presets.find((x) => x.name === name);
  if (!p) return;
  invoke<boolean[]>("preset_check", { preset: p })
    .then((ok) => {
      const bad = p.steps.filter((_, k) => !ok[k]);
      // 步骤存的是中文原文，显示时按当前语言（stepLabel）
      if (bad.length) notify(t("预设里有{n}步做不了", { n: bad.length }), t("{steps}，会跳过这几步，可以在「AI 助手 › 预设」里改", { steps: joinList(bad.map(stepLabel)) }));
    })
    .catch(() => undefined);
}
