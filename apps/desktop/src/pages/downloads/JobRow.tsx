// 下载列表的一行：封面、标题、来源，下面一块随状态变化（读取、选清晰度、进度、排队、要登录、失败……）。
// 动效（配方 c1、c3、c7、g3）：进场 opacity + 上方 8px、弹簧 0.4s 不回弹；退场 opacity + scale .95、150ms；
// 其余行同时让位（列表外层 AnimatePresence popLayout + 这里的 layout）；状态块用 Swap 互换；暂停和继续的图标用 IconSwap。
// 进度、速度、剩余时间这些数字每秒变好几次，不做动画，只用等宽数字防抖（配方 c6）。
import { AlertCircleIcon, Copy01Icon, MoreHorizontalIcon, PauseIcon, PlayIcon, RepeatIcon, SecurityCheckIcon, SquareLock02Icon, StopIcon } from "@hugeicons/core-free-icons";
import { motion, useReducedMotion } from "motion/react";
import { memo, useEffect, useRef, useState, type KeyboardEvent, type Ref } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { DropdownMenu } from "@/components/ui/dropdown-menu";
import { IconButton } from "@/components/ui/icon-button";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { ShimmerText } from "@/components/ui/shimmer-loader";
import { Spinner } from "@/components/ui/spinner";
import { decimal, t, tAny, tx } from "@/i18n";
import { fileSrc } from "@/lib/bridge";
import { siteLabel } from "@/lib/detect";
import { clock, mb, whenLabel } from "@/lib/format";
import { REVEAL } from "@/lib/os";
import { jobAction, setVariant } from "@/lib/store";
import type { Job } from "@/lib/types";
import { Countdown, Elapsed, Icon, IconSwap, ProgressBar, SiteIcon, Swap, Thumb, TipIcon } from "@/parts";
import { cancelLogin, copyWithToast, pickEntries } from "@/shell/actions";
import { retried, revealJob, rowMenu, runFail, stopRecording } from "./act";
import { errorText, etaOf, failInfo, isHls, probeSince, progress, recBusy, recLimit, segCount, tickOf, untitled, variantName, type View } from "./model";

export const ROW_SPRING = { type: "spring", duration: 0.4, bounce: 0 } as const;
const EXIT = { duration: 0.15, ease: [0.23, 1, 0.32, 1] } as const;

/** 带进度条的几种状态共用一个 key：互换时进度条留在原地，只换下面的文字 */
const swapKey = (v: View) => (v === "dl" || v === "pause" || v === "queue" || v === "starting" || v === "merge" ? "bar" : v);

type RowProps = {
  ref?: Ref<HTMLLIElement>;
  job: Job;
  view: View;
  /** 登录窗口开着：等登录还是等验证 */
  waitKind?: "login" | "verify";
  selected: boolean;
  /** 排在前面的任务数（排队时显示） */
  ahead: number;
  /** 行数太多时关掉让位动画（布局动画在主线程，配方 c 的性能条） */
  layoutOn: boolean;
  /** 已完成页每分钟变一次，让「3 分钟前」跟着走 */
  minute: number;
  onSelect: (id: number) => void;
  onKeyDown: (e: KeyboardEvent<HTMLLIElement>) => void;
  onAskCancel: (j: Job) => void;
};

export const JobRow = memo(function JobRow({ ref, job: j, view, waitKind, selected, ahead, layoutOn, minute, onSelect, onKeyDown, onAskCancel }: RowProps) {
  const reduce = useReducedMotion();
  const noTitle = untitled(j);
  // 等登录时的「正在等你在弹出的窗口里登录芒果TV」：和失败那句话用同一个网站名字。
  // siteLabel 给的是存下来的原文（「芒果TV」），显示时翻译
  const site = tAny(siteLabel(j.plat, j.url, j.fail?.site));
  // 录直播不给暂停（继续以后中间会缺一段），只有「停止并保存」和「取消」
  const canPause = !recBusy(j) && (j.state === "run" || j.state === "queue" || (j.state === "pause" && !j.choose));
  const paused = j.state === "pause";

  // 点过重试、又失败了：状态块抖一次
  const prev = useRef(view);
  const shake = view === "fail" && prev.current !== "fail" && retried.has(j.id);
  useEffect(() => {
    prev.current = view;
    if (view !== "fail") return;
    retried.delete(j.id);
  }, [view, j.id]);

  return (
    <motion.li
      ref={ref}
      layout={layoutOn ? "position" : false}
      className="cp-job"
      role="option"
      aria-selected={selected}
      tabIndex={selected ? 0 : -1}
      data-id={j.id}
      onClick={() => onSelect(j.id)}
      onKeyDown={onKeyDown}
      initial={{ opacity: 0, y: reduce ? 0 : -8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, scale: reduce ? 1 : 0.95, transition: EXIT }}
      transition={ROW_SPRING}
    >
      <Thumb src={j.thumb} dur={j.dur} site={j.plat ?? j.url} />
      <div className="cp-job-body">
        <div className="cp-job-title" title={noTitle ? j.url : j.title}>
          {noTitle ? j.src : j.title}
        </div>
        <div className="cp-meta">
          <SiteIcon site={j.plat ?? j.url} />
          <span>{site}</span>
          {view === "choose" ? (
            <>
              <span>·</span>
              <span>{t("{n}个视频", { n: j.entries?.length ?? 0 })}</span>
            </>
          ) : (
            !noTitle &&
            j.fmt && (
              <>
                <span>·</span>
                <span>{j.fmt}</span>
              </>
            )
          )}
          {j.from && (
            <>
              <span>·</span>
              <span>{t("来自{name}", { name: j.from })}</span>
            </>
          )}
          {!noTitle && (
            <>
              <span>·</span>
              <span className="u mono">{j.src}</span>
            </>
          )}
        </div>
        <Swap k={swapKey(view)}>
          <Block j={j} view={view} waitKind={waitKind} site={site} ahead={ahead} shake={shake} minute={minute} onAskCancel={onAskCancel} />
        </Swap>
      </div>
      <div className="cp-job-actions">
        {canPause && (
          <TipIcon
            size="sm"
            label={paused ? t("继续") : t("暂停")}
            icon={
              <IconSwap k={paused ? "play" : "pause"}>
                <Icon icon={paused ? PlayIcon : PauseIcon} size={16} />
              </IconSwap>
            }
            onClick={() => jobAction(j.id, "toggle")}
          />
        )}
        <DropdownMenu
          align="end"
          aria-label={t("这个下载的更多操作")}
          items={rowMenu(j, view, onAskCancel)}
          trigger={({ triggerProps }) => <IconButton icon={MoreHorizontalIcon} label={t("更多")} size="sm" title="" {...triggerProps} />}
        />
      </div>
    </motion.li>
  );
});

/* ---------------- 状态块 ---------------- */

type BlockProps = { j: Job; view: View; waitKind?: "login" | "verify"; site: string; ahead: number; shake: boolean; minute: number; onAskCancel: (j: Job) => void };

function Block({ j, view, waitKind, site, ahead, shake, onAskCancel }: BlockProps) {
  switch (view) {
    case "rec":
      return <RecBlock j={j} onAskCancel={onAskCancel} />;

    case "probe":
      return (
        <div className="cp-block">
          <div className="cp-line">
            <ShimmerText className="cp-wait">{t("正在读取视频信息…")}</ShimmerText>
            <ProbeTimer j={j} />
          </div>
          <div className="cp-gap">
            <ProgressBar p={0} indeterminate label={t("正在读取视频信息")} />
          </div>
        </div>
      );

    case "pick":
      return (
        <div className="cp-block">
          <SegmentedControl
            aria-label={t("清晰度")}
            size="sm"
            value={String(j.vIdx)}
            onChange={(v) => setVariant(j.id, Number(v))}
            options={(j.vars ?? []).map((v, k) => ({ value: String(k), label: v.p, count: mb(v.mb) }))}
          />
          <div className="cp-pickfoot">
            {j.pickUntil ? <Countdown until={j.pickUntil}>{(left) => t("{n}秒后自动用{quality}", { n: left, quality: variantName(j) })}</Countdown> : null}
            <button className="cp-link" onClick={() => jobAction(j.id, "pickgo")}>
              {t("马上开始")}
            </button>
          </div>
        </div>
      );

    case "dl":
    case "pause":
    case "queue":
    case "starting":
    case "merge":
      return <BarBlock j={j} view={view} ahead={ahead} />;

    case "choose":
      return <ChooseBlock j={j} />;

    case "login":
    case "verify": {
      const info = failInfo(j);
      return (
        <div className="cp-block cp-note" data-tone="warn">
          <Icon icon={view === "login" ? SquareLock02Icon : SecurityCheckIcon} size={16} />
          <div className="grow">
            {info.title}
            {info.hint && <small>{info.hint}</small>}
          </div>
          <Button variant="secondary" size="sm" className="cp-pill" onClick={() => runFail(j, info.action)}>
            {info.label}
          </Button>
        </div>
      );
    }

    case "wait_login":
      return (
        <div className="cp-block cp-note" data-tone="wait">
          <Spinner size={15} />
          <div className="grow">
            {waitKind === "verify" ? t("正在等你在弹出的窗口里完成验证") : t("正在等你在弹出的窗口里登录{site}", { site })}
            <small>{t("完成以后会自动继续下载")}</small>
          </div>
          <Button variant="ghost" size="sm" onClick={() => cancelLogin(j.id)}>
            {t("取消")}
          </Button>
        </div>
      );

    case "fail": {
      const info = failInfo(j);
      return (
        <div className="cp-block cp-note" data-tone="err" data-shake={shake || undefined}>
          <Icon icon={AlertCircleIcon} size={16} />
          <div className="grow">
            {info.title}
            {info.hint && <small>{info.hint}</small>}
          </div>
          <TipIcon size="sm" icon={Copy01Icon} label={t("复制错误信息")} onClick={() => copyWithToast(errorText(j), t("错误信息已复制"), t("遇到问题时可以发给我们"))} />
          {info.action && (
            <Button variant="secondary" size="sm" className="cp-pill" icon={info.action === "retry" ? <Icon icon={RepeatIcon} size={14} /> : undefined} onClick={() => runFail(j, info.action)}>
              {info.label}
            </Button>
          )}
        </div>
      );
    }

    case "done":
      return (
        <div className="cp-block cp-stats">
          {j.rec?.interrupted ? <span className="warn">{t("录制被中断，录到的已保存")}</span> : <span className="ok">{j.rec?.full ? t("录满设定的时长，自动停止") : t("已完成")}</span>}
          {j.rec?.secs != null && <span>{t("录了{time}", { time: clock(j.rec.secs) })}</span>}
          {j.size > 0 && <span>{mb(j.size)}</span>}
          {/* j.when 是后端按当时的语言写好的（旧任务没有 doneAt 时用） */}
          <span>{whenLabel(j.doneAt, j.when ?? "")}</span>
          <button className="cp-link" onClick={() => revealJob(j)}>
            {t(REVEAL)}
          </button>
        </div>
      );
  }
}

/** 录直播中：「正在录制 · 已录 12:34」按秒走、录了多大；主按钮「停止并保存」，旁边「取消」（不要了，录到的删掉）。
 *  没有百分比和剩余时间。红点是静止的（常驻界面不放循环动画，配方 i）；按钮用 affectUI 自带的按压反馈，不加新动效 */
export function RecBlock({ j, onAskCancel }: { j: Job; onAskCancel: (j: Job) => void }) {
  return (
    <div className="cp-block cp-note cp-rec">
      <span className="cp-rec-dot" aria-hidden />
      <div className="grow">
        {/* 时间是一个元素，不会自动加空格，原文里自己留一格 */}
        {tx("正在录制 · 已录 {time}", { time: <Elapsed from={j.rec?.since ?? Date.now()} asClock /> })}
        <small className="num">
          {[mb(j.got), recLimit(j)].filter(Boolean).join(" · ")}
        </small>
      </div>
      <Button variant="ghost" size="sm" onClick={() => onAskCancel(j)}>
        {t("取消")}
      </Button>
      <Button size="sm" className="cp-pill" icon={<Icon icon={StopIcon} size={14} />} onClick={() => stopRecording(j)}>
        {t("停止并保存")}
      </Button>
    </div>
  );
}

/** 读了多久：从下载记录里那条「正在读取视频信息」算起，没有就从第一次显示算起 */
function ProbeTimer({ j }: { j: Job }) {
  const since = probeSince(j);
  const [first] = useState(() => Date.now());
  return <Elapsed from={since ?? first} />;
}

/** 下载中、暂停、排队、正在开始、合并：进度条一直在，下面那行文字随状态换 */
function BarBlock({ j, view, ahead }: { j: Job; view: View; ahead: number }) {
  const p = view === "merge" ? j.merge / 100 : progress(j);
  const seg = segCount(j);
  const eta = etaOf(j);
  return (
    <div className="cp-block">
      <ProgressBar
        p={p}
        indeterminate={(view === "dl" && !j.size) || (view === "merge" && !!j.live)}
        tone={view === "pause" || view === "queue" || view === "starting" ? "paused" : undefined}
        tick={view === "merge" ? 500 : tickOf(j)}
        label={view === "merge" ? t("处理进度") : t("下载进度")}
      />
      <div className="cp-stats">
        {view === "dl" && (
          <>
            {j.size > 0 && <span className="pct">{Math.floor(p * 100)}%</span>}
            <span>{j.size > 0 ? `${mb(j.got)} / ${mb(j.size)}` : t("已下载{size}", { size: mb(j.got) })}</span>
            <span>{decimal(j.speed, 1)} MB/s</span>
            {eta && <span>{t("剩余{time}", { time: eta })}</span>}
            {seg.retry > 0 && <span className="warn">{t("网络有点不稳，正在自动重试")}</span>}
          </>
        )}
        {view === "merge" &&
          (j.live ? (
            <>
              <Spinner size={12} />
              <span>{j.rec?.interrupted ? t("上次退出时还在录，正在整理录到的部分") : t("正在保存录到的部分")}</span>
              {j.got > 0 && <span>{mb(j.got)}</span>}
            </>
          ) : (
            <>
              <Spinner size={12} />
              <span>{isHls(j) ? t("快好了，正在生成视频文件") : j.audio ? t("下载好了，正在转换格式") : t("下载好了，正在整理成视频文件")}</span>
              <span className="pct">{Math.round(j.merge)}%</span>
            </>
          ))}
        {view === "pause" && (
          <>
            <span>{t("已暂停")}</span>
            <span>{isHls(j) ? t("已下载的部分会保留") : j.size > 0 ? `${mb(j.got)} / ${mb(j.size)}` : ""}</span>
          </>
        )}
        {view === "starting" && (
          <>
            <Spinner size={12} />
            <span>{t("正在开始…")}</span>
          </>
        )}
        {view === "queue" && (
          <>
            <span>{t("排队中 · 前面还有{n}个", { n: ahead })}</span>
            <button className="cp-link" onClick={() => jobAction(j.id, "now")}>
              {t("马上开始")}
            </button>
          </>
        )}
      </div>
    </div>
  );
}

/** 播放列表、一条帖子里有几个视频：勾选要下的，各自生成一个下载任务 */
function ChooseBlock({ j }: { j: Job }) {
  const entries = j.entries ?? [];
  const [on, setOn] = useState<Set<string>>(() => new Set(entries.map((e) => e.id)));
  const all = on.size === entries.length;
  const flip = (id: string, v: boolean) =>
    setOn((s) => {
      const n = new Set(s);
      if (v) n.add(id);
      else n.delete(id);
      return n;
    });
  return (
    <div className="cp-block">
      <div className="cp-entries">
        {entries.map((e) => (
          <label className="cp-entry" key={e.id}>
            <Checkbox aria-label={e.title} checked={on.has(e.id)} onChange={(ev) => flip(e.id, ev.target.checked)} />
            {e.thumb ? <img src={fileSrc(e.thumb)} alt="" referrerPolicy="no-referrer" loading="lazy" /> : <span className="ph" />}
            <span className="t">{e.title}</span>
            <span className="d">{clock(e.dur)}</span>
          </label>
        ))}
      </div>
      <div className="cp-entries-foot">
        <Button
          size="sm"
          className="cp-pill"
          disabled={!on.size}
          onClick={() =>
            pickEntries(
              j.id,
              entries.filter((e) => on.has(e.id)).map((e) => e.id),
            )
          }
        >
          {t("下载{n}个", { n: on.size })}
        </Button>
        <Button variant="ghost" size="sm" onClick={() => setOn(all ? new Set() : new Set(entries.map((e) => e.id)))}>
          {all ? t("全不选") : t("全部选上")}
        </Button>
      </div>
    </div>
  );
}
