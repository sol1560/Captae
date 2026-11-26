// 下载页（第二版设计稿，蓝本：Perplexity Computer Mac 版）。
// 一个任务都没有：居中的 logo、标题和输入框；有任务：列表 + 浮在底部的毛玻璃输入框 + 右侧详情浮卡。
// 主区不到 900px 宽时，详情改成盖在列表右边的浮卡，点了任务才出来。
// 功能和旧界面（ui/js/page-downloads.js）逐条对齐：链接识别、保存格式、@ 预设、三个页签、全部暂停、同时下载几个、
// 排队位置和「马上开始」、M3U8 选清晰度倒计时、分段、下载记录、取消确认、剪贴板提示、#/downloads?t= 和 ?paste 跳转。
import {
  CheckmarkCircle02Icon,
  Delete02Icon,
  Download04Icon,
  FileVideoIcon,
  MoreHorizontalIcon,
  PauseIcon,
  PlayIcon,
  PlayListIcon,
  Settings02Icon,
  AlertCircleIcon,
  Cancel01Icon,
} from "@hugeicons/core-free-icons";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { AlertDialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { DropdownMenu } from "@/components/ui/dropdown-menu";
import { EmptyState } from "@/components/ui/empty-state";
import { IconButton } from "@/components/ui/icon-button";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { Tooltip } from "@/components/ui/tooltip";
import { decimal, N_, t, tAny } from "@/i18n";
import { errText, invoke } from "@/lib/bridge";
import { detect, normalizeUrl } from "@/lib/detect";
import { bare, mb } from "@/lib/format";
import { useMinute, useWideEnough } from "@/lib/hooks";
import { notify } from "@/lib/notify";
import { getPref, setPref } from "@/lib/prefs";
import { clearParams, useRoute } from "@/lib/route";
import { addJob, getState, isActive, jobAction, onEvent, setSettings, useApp } from "@/lib/store";
import type { Job } from "@/lib/types";
import { CaptaeLogo, Icon, IconSwap, SiteIcon } from "@/parts";
import { openDialog, queueAhead, useLoginWait } from "@/shell/actions";
import { Topbar } from "@/shell/Topbar";
import { AT, Composer } from "./downloads/Composer";
import { Detail } from "./downloads/Detail";
import { JobRow } from "./downloads/JobRow";
import { FMT, inTab, recBusy, viewOf, type FmtChoice, type Tab } from "./downloads/model";

const EASE = [0.23, 1, 0.32, 1] as const;

/** 空页面上「试试这些链接」：都是 Blender 基金会的开放电影（CC BY），公开、能下。
 *  label 是原文（带中文的用 N_ 标出来），显示时 tAny；片名 Big Buck Bunny 不翻译 */
const TRIES = [
  { label: "Big Buck Bunny", site: "youtube", url: "https://www.youtube.com/watch?v=aqz-KE-bpKQ" },
  { label: N_("Sintel 预告片"), icon: FileVideoIcon, url: "https://download.blender.org/durian/trailer/sintel_trailer-1080p.mp4" },
  { label: N_("M3U8 测试流"), icon: PlayListIcon, url: "https://test-streams.mux.dev/x36xhzz/x36xhzz.m3u8" },
];

export function Downloads() {
  const jobs = useApp((s) => s.jobs);
  const settings = useApp((s) => s.settings);
  const route = useRoute();
  const reduce = useReducedMotion();
  const wait = useLoginWait();

  const [tab, setTab] = useState<Tab>("run");
  const [sel, setSel] = useState<number | null>(null);
  const [url, setUrl] = useState("");
  const [fmt, setFmtState] = useState<FmtChoice>(() => {
    const f = getPref<string>("fmt", "best");
    return f in FMT ? (f as FmtChoice) : "best";
  });
  const [preset, setPreset] = useState("");
  const [busy, setBusy] = useState(false);
  const [clip, setClip] = useState<string | null>(null);
  const [cancelling, setCancelling] = useState<Job | null>(null);
  /** 窄窗口：详情浮卡开着 */
  const [sheet, setSheet] = useState(false);

  const mainRef = useRef<HTMLElement>(null);
  const colRef = useRef<HTMLDivElement>(null);
  const listRef = useRef<HTMLUListElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const busyRef = useRef(false);

  const all = useMemo(() => Object.values(jobs), [jobs]);
  const lists = useMemo(
    () => ({
      run: all.filter((j) => inTab(j, "run")).sort((a, b) => a.pri - b.pri),
      done: all.filter((j) => inTab(j, "done")).sort((a, b) => (b.doneAt ?? 0) - (a.doneAt ?? 0) || b.id - a.id),
      fail: all.filter((j) => inTab(j, "fail")).sort((a, b) => b.id - a.id),
    }),
    [all],
  );
  const list = lists[tab];
  const selected = list.find((j) => j.id === sel) ?? list[0] ?? null;
  const mode = all.length ? "list" : "empty";
  // 主区 900px 以上才把详情放在右边一列（列表至少还有 540px，行里的清晰度选项放得下）；窄了改成点开才出来的浮卡
  const wide = useWideEnough(mainRef, 900, [mode], () => innerWidth - 16 - (innerWidth <= 760 || getPref("sideOff", false) ? 0 : 244) >= 900);
  const minute = useMinute(tab === "done");

  // 键盘上下键、行的回调要拿到最新的列表和选中，又不能每次都换函数（行是 memo 的）
  const live = useRef({ list, selected });
  live.current = { list, selected };

  // 这次选中是不是键盘上下键换的：键盘操作不动画，详情直接换（review-animations 第 2 条）
  const keyNav = useRef(false);
  const selectRow = useCallback((id: number) => {
    keyNav.current = false;
    setSel(id);
    setSheet(true);
  }, []);
  const askCancel = useCallback((j: Job) => {
    // 还没下到东西就不用确认，直接取消
    if (j.got <= 0) {
      jobAction(j.id, "cancel").then((ok) => ok && notify(t("已取消"), j.title));
      return;
    }
    setCancelling(j);
  }, []);
  // 上下键换选中的任务：即时，不做动画（键盘操作永远不动画，配方 1.10）
  const onRowKey = useCallback((e: KeyboardEvent<HTMLLIElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    if (e.target !== e.currentTarget) return;
    e.preventDefault();
    const { list: L, selected: s } = live.current;
    const i = L.findIndex((j) => j.id === s?.id);
    const next = L[Math.max(0, Math.min(L.length - 1, i + (e.key === "ArrowDown" ? 1 : -1)))];
    if (!next) return;
    keyNav.current = true;
    setSel(next.id);
    requestAnimationFrame(() => listRef.current?.querySelector<HTMLElement>(`[data-id="${next.id}"]`)?.focus({ preventScroll: false }));
  }, []);

  const setFmt = (f: FmtChoice) => {
    setFmtState(f);
    setPref("fmt", f);
  };

  /** 把剪贴板里的链接填进输入框（插件、命令行要求「粘贴」时也走这里） */
  const pasteClipboard = useCallback(() => {
    invoke<string>("read_clipboard")
      .then((text) => {
        const v = (text ?? "").trim();
        if (normalizeUrl(v)) setUrl(v);
        else if (v) notify(t("剪贴板里不是链接"), bare(v).slice(0, 48));
      })
      .catch(() => undefined)
      .finally(() => requestAnimationFrame(() => inputRef.current?.focus()));
  }, []);

  /** 加任务。fromClip：剪贴板提示卡上点的「下载」，不动输入框里的内容 */
  const submit = async (raw: string, fromClip = false) => {
    const d = detect(raw.replace(AT, "").trim());
    if (!d || d.unsupported || busyRef.current) return;
    busyRef.current = true;
    setBusy(true);
    try {
      const j = await addJob({ url: d.url, ...FMT[fmt].req, preset: preset || null, origin: fromClip ? "clipboard" : "ui" });
      if (!fromClip) {
        setUrl("");
        setPreset("");
      }
      setTab("run");
      setSel(j.id);
      colRef.current?.scrollTo({ top: 0, behavior: reduce ? "auto" : "smooth" });
      const ahead = queueAhead(j);
      if (ahead) notify(t("已加入排队"), t("前面还有{n}个，想先下它可以点「马上开始」", { n: ahead }));
    } catch (e) {
      notify(t("没有开始下载"), errText(e));
    } finally {
      busyRef.current = false;
      setBusy(false);
    }
  };

  // 复制了视频链接、1 分钟内回到 Captae：在输入框上方提示（别的页面由窗口骨架弹提示条；什么时候问由后端定，见 clip_ask.rs）
  useEffect(() => onEvent("clipboard", (ev) => setClip(ev.url)), []);

  // 跳转参数：?t=任务（侧栏、插件、通知点进来）、?paste（插件要求粘贴）、?tab=done|fail（直接看某个页签）；
  // 用完清掉，免得刷新时又来一次
  useEffect(() => {
    if (route.page !== "downloads") return;
    const p = route.params;
    let used = false;
    const tb = p.get("tab");
    if (tb === "run" || tb === "done" || tb === "fail") {
      used = true;
      setTab(tb);
    }
    const t = Number(p.get("t"));
    if (p.has("t") && t) {
      used = true;
      const j = getState().jobs[t];
      if (j) {
        setTab((cur) => (inTab(j, cur) ? cur : j.state === "done" ? "done" : "run"));
        setSel(t);
        setSheet(true);
        requestAnimationFrame(() => listRef.current?.querySelector(`[data-id="${t}"]`)?.scrollIntoView({ block: "nearest", behavior: reduce ? "auto" : "smooth" }));
      }
    }
    if (p.has("paste")) {
      used = true;
      pasteClipboard();
    }
    if (used) clearParams();
  }, [route, reduce, pasteClipboard]);

  // 第一次加任务、页面从空的变成列表时，输入框换了一个，把焦点交给新的
  const firstMode = useRef(mode);
  useEffect(() => {
    if (mode !== firstMode.current) requestAnimationFrame(() => inputRef.current?.focus());
  }, [mode]);

  // 窄窗口：Esc 收起详情浮卡
  useEffect(() => {
    if (wide || !sheet) return;
    const f = (e: globalThis.KeyboardEvent) => e.key === "Escape" && setSheet(false);
    addEventListener("keydown", f);
    return () => removeEventListener("keydown", f);
  }, [wide, sheet]);

  const composer = (
    <Composer
      value={url}
      onChange={setUrl}
      fmt={fmt}
      onFmt={setFmt}
      preset={preset}
      onPreset={setPreset}
      busy={busy}
      onSubmit={() => submit(url)}
      onPaste={pasteClipboard}
      inputRef={inputRef}
      autoFocus={mode === "empty"}
    />
  );
  const clipCard = (
    <AnimatePresence>
      {clip && (
        <ClipCard
          key="clip"
          url={clip}
          onDownload={() => {
            const u = clip;
            setClip(null);
            submit(u, true);
          }}
          onFill={() => {
            setUrl(clip);
            setClip(null);
            requestAnimationFrame(() => inputRef.current?.focus());
          }}
          onDismiss={() => setClip(null)}
        />
      )}
    </AnimatePresence>
  );

  // 取消确认弹窗退场时还要显示原来那个任务，不能跟着变空（退场规则 1.4 第 7 条）
  const lastCancel = useRef<Job | null>(null);
  if (cancelling) lastCancel.current = cancelling;
  const cj = lastCancel.current;
  // 录直播：取消 = 不要了，录到的删掉；想留下就「停止并保存」
  const cjRec = !!cj && recBusy(cj);
  const cancelDialog = (
    <AlertDialog
      className="cp"
      open={!!cancelling}
      onOpenChange={(o) => !o && setCancelling(null)}
      title={cjRec ? t("取消这次录制？") : t("取消这个下载？")}
      description={cj?.title}
      confirmLabel={cjRec ? t("取消录制") : t("取消下载")}
      cancelLabel={cjRec ? t("继续录") : t("继续下载")}
      danger
      onConfirm={() => {
        if (cj) jobAction(cj.id, "cancel").then((ok) => ok && notify(cjRec ? t("已取消录制") : t("已取消下载"), cj.title));
      }}
    >
      <p className="cp-dlg-p">
        {cjRec
          ? t("已经录的{size}会被删掉。想留下录到的部分，点「停止并保存」。", { size: mb(cj.got) })
          : cj && cj.got > 0
            ? t("已经下载的{size}会被删掉。想以后继续下，可以点「暂停」。", { size: mb(cj.got) })
            : t("想以后继续下，可以点「暂停」。")}
      </p>
    </AlertDialog>
  );

  // 第一次画的时候不做淡入；之后空页面和列表互相换的时候淡入 200ms，避免突然跳（配方 1.10「避免突兀变化」）
  const mounted = useRef(false);
  useEffect(() => {
    mounted.current = true;
  }, []);
  const fadeIn = mounted.current ? { opacity: 0 } : false;

  if (mode === "empty")
    return (
      <motion.main key="empty" ref={mainRef} className="cp-main solo" initial={fadeIn} animate={{ opacity: 1 }} transition={{ duration: 0.2, ease: EASE }}>
        <div className="cp-empty" data-tauri-drag-region>
          <div className="cp-empty-in">
            <CaptaeLogo size={44} />
            <h2>{t("粘贴链接，开始下载")}</h2>
            <p>{t("YouTube、B 站、X、抖音等上千个网站都可以，M3U8 和直链也行")}</p>
            <div className="cp-empty-clip">{clipCard}</div>
            {composer}
            <div className="cp-tries" aria-label={t("试试这些链接")}>
              {TRIES.map((x) => (
                <button
                  key={x.url}
                  className="cp-try"
                  title={x.url}
                  onClick={() => {
                    setUrl(x.url);
                    requestAnimationFrame(() => inputRef.current?.focus());
                  }}
                >
                  {x.site ? <SiteIcon site={x.site} size={13} /> : x.icon ? <Icon icon={x.icon} size={14} /> : null}
                  {tAny(x.label)}
                </button>
              ))}
            </div>
            <div className="foot">{t("装好浏览器插件以后，也可以在网页上直接点视频的「下载」")}</div>
          </div>
        </div>
        {cancelDialog}
      </motion.main>
    );

  /* ---------------- 有任务：列表 ---------------- */

  // 录着的直播不算（「全部暂停」不动它，后端也不让暂停）
  const act = lists.run.filter((j) => isActive(j) && !j.choose && !recBusy(j));
  const allPaused = act.length > 0 && act.every((j) => j.state === "pause");
  const running = all.filter((j) => j.state === "run");
  const speed = running.reduce((s, j) => s + (j.phase === "dl" ? j.speed : 0), 0);
  const sub = running.length
    ? t("{n}个下载中 · {speed} MB/s", { n: running.length, speed: decimal(speed, 1) })
    : act.length
      ? allPaused
        ? t("全部暂停了")
        : t("{n}个等着下载", { n: act.length })
      : "";
  const conc = settings?.concurrency ?? 3;

  /** 清空已完成的（done）或失败的（fail）：按固定的键判断，提示文字整句翻译 */
  const clearAll = (L: Job[], which: "done" | "fail") => {
    Promise.all(L.map((j) => jobAction(j.id, "remove"))).then((r) => {
      const n = r.filter(Boolean).length;
      if (!n) return;
      if (which === "done") notify(t("已清空{n}个已完成的下载", { n }), t("文件还在，素材库里也还有"));
      else notify(t("已清空{n}个失败的下载", { n }));
    });
  };

  const empty = {
    run: { icon: Download04Icon, title: t("没有进行中的下载"), text: t("在下面粘贴链接，或者在浏览器里点视频上的「下载」") },
    done: { icon: CheckmarkCircle02Icon, title: t("还没有下载完成的"), text: t("下载完成的会出现在这里，同时放进素材库") },
    fail: { icon: AlertCircleIcon, title: t("没有失败的下载"), text: t("下载出了问题会在这里说明原因") },
  }[tab];

  return (
    <motion.main key="list" ref={mainRef} className={`cp-main${wide && selected ? "" : " solo"}`} initial={fadeIn} animate={{ opacity: 1 }} transition={{ duration: 0.2, ease: EASE }}>
      <div className="cp-listwrap">
        <div className="cp-col" ref={colRef}>
          <Topbar
            title={t("下载§侧栏、顶栏里的页面名（Downloads）")}
            sub={sub}
            mid={
              <SegmentedControl
                aria-label={t("下载列表")}
                size="sm"
                value={tab}
                onChange={(v) => setTab(v as Tab)}
                options={[
                  { value: "run", label: t("进行中"), count: lists.run.length || undefined },
                  { value: "done", label: t("已完成"), count: lists.done.length || undefined },
                  { value: "fail", label: t("失败"), count: lists.fail.length || undefined },
                ]}
              />
            }
            end={
              <>
                <Tooltip content={allPaused ? t("全部继续") : t("全部暂停")}>
                  <button
                    className="cp-round"
                    aria-label={allPaused ? t("全部继续") : t("全部暂停")}
                    disabled={!act.length}
                    onClick={() => invoke(allPaused ? "resume_all" : "pause_all").catch((e) => notify(t("没有完成"), errText(e)))}
                  >
                    <IconSwap k={allPaused ? "play" : "pause"}>
                      <Icon icon={allPaused ? PlayIcon : PauseIcon} size={15} />
                    </IconSwap>
                  </button>
                </Tooltip>
                <DropdownMenu
                  align="end"
                  aria-label={t("下载的更多操作")}
                  items={[
                    {
                      label: t("同时下载{n}个", { n: conc }),
                      icon: Download04Icon,
                      submenu: [1, 2, 3, 4, 5, 6].map((n) => ({ label: t("{n}个§「同时下载几个」子菜单里的一项", { n }), checked: n === conc, onSelect: () => setSettings({ concurrency: n }) })),
                    },
                    ...(lists.done.length ? [{ label: t("清空已完成"), icon: Delete02Icon, divider: true, onSelect: () => clearAll(lists.done, "done") }] : []),
                    ...(lists.fail.length ? [{ label: t("清空失败的"), icon: Delete02Icon, divider: !lists.done.length, onSelect: () => clearAll(lists.fail, "fail") }] : []),
                    { label: t("下载设置"), icon: Settings02Icon, divider: true, onSelect: () => openDialog("settings") },
                  ]}
                  trigger={({ triggerProps }) => (
                    <button className="cp-round" aria-label={t("更多")} {...triggerProps}>
                      <Icon icon={MoreHorizontalIcon} size={16} />
                    </button>
                  )}
                />
              </>
            }
          />
          {list.length ? (
            <ul className="cp-jobs" role="listbox" aria-label={t("下载列表")} ref={listRef}>
              {/* 换页签时整个重来、不播进场（功能性数据不重播，配方 c1）；同一页签里的增删才有进退场和让位 */}
              <AnimatePresence key={tab} mode="popLayout" initial={false}>
                {list.map((j) => (
                  <JobRow
                    key={j.id}
                    job={j}
                    view={viewOf(j, !!wait[j.id])}
                    waitKind={wait[j.id]}
                    selected={selected?.id === j.id}
                    ahead={queueAhead(j, jobs)}
                    layoutOn={list.length <= 40}
                    minute={tab === "done" ? minute : 0}
                    onSelect={selectRow}
                    onKeyDown={onRowKey}
                    onAskCancel={askCancel}
                  />
                ))}
              </AnimatePresence>
            </ul>
          ) : (
            <div className="cp-listempty">
              <EmptyState bare tall icon={empty.icon} title={empty.title} text={empty.text} />
            </div>
          )}
        </div>
        <div className="cp-dock">
          {clipCard}
          {composer}
        </div>
      </div>

      {selected && wide && (
        <aside className="cp-detail" aria-label={t("任务详情")}>
          <Detail job={selected} view={viewOf(selected, !!wait[selected.id])} onAskCancel={askCancel} instant={keyNav.current} />
        </aside>
      )}
      {!wide && (
        <AnimatePresence>
          {sheet && selected && (
            // 窄窗口的详情浮卡：从右边滑进来 200ms ease-out-quart，收起 150ms（配方 e1）；减少动效时只淡入淡出
            <motion.aside
              key="sheet"
              className="cp-detail float"
              aria-label={t("任务详情")}
              // MotionConfig 管不到写成字符串的 transform，减少动效要自己判断：只淡入淡出
              initial={reduce ? { opacity: 0 } : { transform: "translateX(110%)", opacity: 0 }}
              animate={{ ...(reduce ? {} : { transform: "translateX(0%)" }), opacity: 1, transition: { duration: 0.2, ease: [0.165, 0.84, 0.44, 1] } }}
              exit={{ ...(reduce ? {} : { transform: "translateX(110%)" }), opacity: 0, transition: { duration: 0.15, ease: EASE } }}
            >
              <Detail job={selected} view={viewOf(selected, !!wait[selected.id])} onAskCancel={askCancel} instant={keyNav.current} onClose={() => setSheet(false)} />
            </motion.aside>
          )}
        </AnimatePresence>
      )}
      {cancelDialog}
    </motion.main>
  );
}

/* ---------------- 剪贴板提示卡 ---------------- */

/** 复制了视频链接：贴在输入框上方。进 200ms、退 150ms、不弹跳（配方 h2）；15 秒后自己收起，鼠标停在上面时不收；
 *  又复制了新链接时原地换文字（150ms 淡入），不重播进场 */
function ClipCard({ url, onDownload, onFill, onDismiss }: { url: string; onDownload: () => void; onFill: () => void; onDismiss: () => void }) {
  const d = detect(url);
  // MotionConfig 管不到写成字符串的 transform，减少动效要自己判断：只淡入淡出
  const reduce = useReducedMotion();
  const shift = (px: number) => (reduce ? {} : { transform: `translateY(${px}px)` });
  const hover = useRef(false);
  const dismiss = useRef(onDismiss);
  dismiss.current = onDismiss;
  useEffect(() => {
    let left = 15_000;
    let last = Date.now();
    const t = setInterval(() => {
      const now = Date.now();
      if (!hover.current && !document.hidden) left -= now - last;
      last = now;
      if (left <= 0) dismiss.current();
    }, 500);
    return () => clearInterval(t);
  }, [url]);
  return (
    <motion.div
      className="cp-suggest"
      role="status"
      initial={{ opacity: 0, ...shift(8) }}
      animate={{ opacity: 1, ...shift(0) }}
      exit={{ opacity: 0, ...shift(8), transition: { duration: 0.15, ease: EASE } }}
      transition={{ duration: 0.2, ease: EASE }}
      onPointerEnter={() => (hover.current = true)}
      onPointerLeave={() => (hover.current = false)}
    >
      <span className="ico">
        <SiteIcon site={d?.site ?? url} size={15} />
      </span>
      <span className="k">{t("你刚复制了")}</span>
      <motion.button key={url} className="t" title={t("填进输入框，先选格式再下载")} onClick={onFill} initial={{ opacity: 0 }} animate={{ opacity: 1 }} transition={{ duration: 0.15 }}>
        {bare(url)}
      </motion.button>
      <Button size="sm" className="cp-pill" disabled={!d || d.unsupported} onClick={onDownload}>
        {t("下载")}
      </Button>
      <IconButton icon={Cancel01Icon} label={t("不用了")} size="sm" onClick={onDismiss} />
    </motion.div>
  );
}
