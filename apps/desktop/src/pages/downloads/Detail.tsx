// 右侧详情浮卡（照 m4 右上角「来源 / 用量」那张卡）：封面、标题、来源、操作，下面分节：
// 进行到哪一步、清晰度（M3U8）、分段下载（M3U8）、会保存为、下载记录。
// 动效：换选中的任务是高频操作，只做 120ms 透明度交叉淡化，不位移不缩放（配方 e2）；
// 步骤一步步完成时描线画勾，打开时已经完成的直接显示终态（配方 e3）；分节展开收起用 Collapse。
import { ArrowDown01Icon, ArrowUp01Icon, Cancel01Icon, Copy01Icon, Delete02Icon, Folder01Icon, PauseIcon, PlayIcon, RepeatIcon, StopIcon } from "@hugeicons/core-free-icons";
import { AnimatePresence, motion, useIsPresent } from "motion/react";
import { useRef, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { decimal, t, tx } from "@/i18n";
import { errText, openUrl } from "@/lib/bridge";
import { host, mb } from "@/lib/format";
import { notify } from "@/lib/notify";
import { REVEAL } from "@/lib/os";
import { getPref, setPref } from "@/lib/prefs";
import { go } from "@/lib/route";
import { addJob, jobAction, setVariant, useApp } from "@/lib/store";
import type { Job, SegState, Settings } from "@/lib/types";
import { Collapse, Elapsed, FileIcon, Icon, IconSwap, ProgressBar, StepTimeline, Thumb, TipIcon } from "@/parts";
import { copyWithToast } from "@/shell/actions";
import { removeJob, revealJob, runFail, stopRecording } from "./act";
import { errorText, etaOf, failInfo, formatPair, isHls, logText, progress, recBusy, recLimit, saveDirOf, segCount, stepIndex, stepsOf, untitled, variantName, variantNote, type View } from "./model";

const EASE = [0.23, 1, 0.32, 1] as const;

/** 换选中的任务：鼠标点的 120ms 透明度交叉淡化；键盘上下键换的直接换（custom = 这次是不是键盘） */
const CARD = {
  in: (instant: boolean) => ({ opacity: 1, transition: { duration: instant ? 0 : 0.12, ease: EASE } }),
  out: (instant: boolean) => ({ opacity: 0, transition: { duration: instant ? 0 : 0.12, ease: EASE } }),
};

/** instant：这次是键盘上下键换的任务，直接换，不淡入淡出 */
export function Detail({ job: j, view, onAskCancel, onClose, instant }: { job: Job; view: View; onAskCancel: (j: Job) => void; onClose?: () => void; instant?: boolean }) {
  const settings = useApp((s) => s.settings);
  const fresh = useFreshSteps(j, settings);
  return (
    // 退场的那张卡拿到的是它上一次渲染时的值；「这次是不是键盘换的」要经 custom 同时交给 AnimatePresence 和卡片（配方 1.4 第 7 条）
    <AnimatePresence mode="popLayout" initial={false} custom={!!instant}>
      <motion.div key={j.id} className="cp-dcard" custom={!!instant} variants={CARD} initial="out" animate="in" exit="out">
        <Live>
        {onClose && (
          <div className="cp-dclose">
            <TipIcon size="sm" variant="secondary" icon={Cancel01Icon} label={t("关上详情")} onClick={onClose} />
          </div>
        )}
        <Thumb className="cp-preview" src={j.thumb} dur={j.dur} site={j.plat ?? j.url} />
        <div className="cp-dtitle">{untitled(j) ? (view === "probe" ? t("还没读到标题") : j.src) : j.title}</div>
        <button className="cp-dsrc" title={t("在浏览器里打开")} onClick={() => openUrl(j.url).catch(() => undefined)}>
          {j.src}
        </button>
        {j.playlist && <PlaylistNote url={j.playlist.url} count={j.playlist.count} index={j.playlist.index} />}
        <Actions j={j} view={view} onAskCancel={onAskCancel} />

        <Section id="steps" title={t("进行到哪一步")}>
          <StepTimeline steps={stepsOf(j, settings, fresh)} />
        </Section>
        {isHls(j) && (j.vars?.length ?? 0) > 0 && (
          <Section id="vars" title={t("清晰度")}>
            <Variants j={j} view={view} />
          </Section>
        )}
        {isHls(j) && (j.segs?.length ?? 0) > 0 && j.state !== "done" && (
          <Section id="segs" title={t("分段下载")} note={t("视频分成很多小段下载，每格是一段")}>
            <Segments j={j} />
          </Section>
        )}
        {j.state !== "failed" && view !== "choose" && (
          <Section id="save" title={j.state === "done" ? t("已保存为") : t("会保存为")}>
            <SaveAs j={j} view={view} settings={settings} />
          </Section>
        )}
        <Section
          id="log"
          title={t("下载记录")}
          count={j.log.length}
          closedByDefault
          extra={
            <TipIcon size="sm" icon={Copy01Icon} label={t("复制下载记录")} onClick={() => copyWithToast([logText(j), j.fail?.raw].filter(Boolean).join("\n"), t("下载记录已复制"), t("遇到问题时可以发给我们"))} />
          }
        >
          <Log j={j} />
        </Section>
        </Live>
      </motion.div>
    </AnimatePresence>
  );
}

/** 正在退场的那张卡（换了任务、旧的还在淡出）不能再被点到：两张叠在一起时点「取消」可能点在旧的上面 */
function Live({ children }: { children: ReactNode }) {
  const present = useIsPresent();
  return <div inert={!present || undefined}>{children}</div>;
}

/** YouTube 的 watch?v=…&list=…、B 站带 ?p= 的分 P：默认只下这一个，这里给一个「整个列表都下」的入口（加进来以后会让你选条目） */
function PlaylistNote({ url, count, index }: { url: string; count?: number; index?: number }) {
  const all = () =>
    addJob({ url, origin: "ui" })
      .then((n) => go("downloads", { t: n.id }))
      .catch((e) => notify(t("没有开始下载"), errText(e)));
  const parts = /(^|\.)bilibili\.com$/.test(host(url));
  return (
    <p className="cp-dplaylist">
      {parts
        ? count && index
          ? t("这是分 P 视频里的第{index} P，共{n} P，只下了这一 P。", { index, n: count })
          : t("这是分 P 视频里的一 P，只下了这一 P。")
        : t("这个视频在一个播放列表里，只下了这一个。")}
      <button className="cp-link" onClick={all}>
        {parts ? (count ? t("选别的分 P") : t("下载全部分 P")) : t("下载整个列表")}
      </button>
    </p>
  );
}

/* ---------------- 操作 ---------------- */

function Actions({ j, view, onAskCancel }: { j: Job; view: View; onAskCancel: (j: Job) => void }) {
  if (j.state === "done")
    return (
      <div className="cp-dactions">
        {j.assetId != null && (
          <Button size="sm" className="cp-pill" onClick={() => go("library", { asset: j.assetId! })}>
            {t("在素材库查看")}
          </Button>
        )}
        <Button size="sm" variant="secondary" className="cp-pill" icon={<Icon icon={Folder01Icon} size={14} />} onClick={() => revealJob(j)}>
          {t(REVEAL)}
        </Button>
        <span className="end">
          <TipIcon size="sm" icon={Delete02Icon} label={t("从列表里移除")} onClick={() => removeJob(j, t("已从列表里移除"), t("文件还在，素材库里也还有"))} />
        </span>
      </div>
    );
  if (j.state === "failed") {
    const info = failInfo(j);
    return (
      <div className="cp-dactions">
        {info.action && view !== "wait_login" && (
          <Button size="sm" className="cp-pill" icon={info.action === "retry" ? <Icon icon={RepeatIcon} size={14} /> : undefined} onClick={() => runFail(j, info.action)}>
            {info.label}
          </Button>
        )}
        <TipIcon size="sm" variant="secondary" icon={Copy01Icon} label={t("复制错误信息")} onClick={() => copyWithToast(errorText(j), t("错误信息已复制"), t("遇到问题时可以发给我们"))} />
        <span className="end">
          <Button variant="ghost" size="sm" icon={<Icon icon={Delete02Icon} size={14} />} onClick={() => removeJob(j)}>
            {t("移除")}
          </Button>
        </span>
      </div>
    );
  }
  // 录直播：「正在录制 · 已录 12:34」和行里一样；主按钮「停止并保存」，旁边「取消」（不要了，录到的删掉）。不能暂停、不排先后
  if (view === "rec")
    return (
      <>
        <div className="cp-note cp-rec cp-drec">
          <span className="cp-rec-dot" aria-hidden />
          <div className="grow">
            {/* 时间是一个元素，不会自动加空格，原文里自己留一格 */}
            {tx("正在录制 · 已录 {time}", { time: <Elapsed from={j.rec?.since ?? Date.now()} asClock /> })}
            <small className="num">
              {[mb(j.got), recLimit(j)].filter(Boolean).join(" · ")}
            </small>
          </div>
        </div>
        <div className="cp-dactions">
          <Button size="sm" className="cp-pill" icon={<Icon icon={StopIcon} size={14} />} onClick={() => stopRecording(j)}>
            {t("停止并保存")}
          </Button>
          <span className="end">
            <Button variant="ghost" size="sm" icon={<Icon icon={Cancel01Icon} size={14} />} onClick={() => onAskCancel(j)}>
              {t("取消")}
            </Button>
          </span>
        </div>
      </>
    );
  // 正在保存录到的部分：只能取消
  if (recBusy(j))
    return (
      <div className="cp-dactions">
        <span className="end">
          <Button variant="ghost" size="sm" icon={<Icon icon={Cancel01Icon} size={14} />} onClick={() => onAskCancel(j)}>
            {t("取消")}
          </Button>
        </span>
      </div>
    );
  if (view === "choose")
    return (
      <div className="cp-dactions">
        <span className="end">
          <Button variant="ghost" size="sm" icon={<Icon icon={Delete02Icon} size={14} />} onClick={() => removeJob(j)}>
            {t("移除")}
          </Button>
        </span>
      </div>
    );
  const paused = j.state === "pause";
  return (
    <div className="cp-dactions">
      <TipIcon
        size="sm"
        variant="secondary"
        label={paused ? t("继续") : t("暂停")}
        icon={
          <IconSwap k={paused ? "play" : "pause"}>
            <Icon icon={paused ? PlayIcon : PauseIcon} size={16} />
          </IconSwap>
        }
        onClick={() => jobAction(j.id, "toggle")}
      />
      {j.state === "queue" && !j.boost && (
        <Button size="sm" className="cp-pill" icon={<Icon icon={PlayIcon} size={14} />} onClick={() => jobAction(j.id, "now")}>
          {t("马上开始")}
        </Button>
      )}
      <TipIcon size="sm" variant="secondary" icon={ArrowUp01Icon} label={t("往前排")} onClick={() => jobAction(j.id, "up")} />
      <TipIcon size="sm" variant="secondary" icon={ArrowDown01Icon} label={t("往后排")} onClick={() => jobAction(j.id, "down")} />
      <span className="end">
        <Button variant="ghost" size="sm" icon={<Icon icon={Cancel01Icon} size={14} />} onClick={() => onAskCancel(j)}>
          {t("取消")}
        </Button>
      </span>
    </div>
  );
}

/* ---------------- 分节 ---------------- */

/** 一节：标题可以点开收起，收起状态记在界面偏好里（下载记录默认收起） */
function Section({
  id,
  title,
  note,
  count,
  extra,
  closedByDefault,
  children,
}: {
  id: string;
  title: string;
  note?: string;
  count?: number;
  extra?: ReactNode;
  closedByDefault?: boolean;
  children: ReactNode;
}) {
  const [open, setOpen] = useState(() => getPref(`dsec:${id}`, !closedByDefault));
  const toggle = () =>
    setOpen((v) => {
      setPref(`dsec:${id}`, !v);
      return !v;
    });
  return (
    <section className="cp-dsec" data-open={open || undefined}>
      <div className="cp-dsec-h">
        <button className="t" aria-expanded={open} onClick={toggle}>
          {title}
          {count != null && count > 0 && <span className="n num">{count}</span>}
          <Icon icon={ArrowDown01Icon} size={14} className="chev" />
        </button>
        {open && extra}
      </div>
      <Collapse open={open}>
        <div className="cp-dsec-b">
          {note && <p className="cp-dnote">{note}</p>}
          {children}
        </div>
      </Collapse>
    </section>
  );
}

/** 详情里一步步往前走时，刚完成的那几步画一次勾；换了任务、或者一下子跳到完成，都直接显示终态 */
function useFreshSteps(j: Job, s: Settings | null): [number, number] | null {
  const cur = stepIndex(j, s);
  const ref = useRef<{ id: number; cur: number; range: [number, number] | null; until: number }>({ id: j.id, cur, range: null, until: 0 });
  const r = ref.current;
  if (r.id !== j.id) ref.current = { id: j.id, cur, range: null, until: 0 };
  else if (cur > r.cur && j.state !== "failed") ref.current = { id: j.id, cur, range: [r.cur, cur], until: Date.now() + 700 };
  else if (cur !== r.cur) ref.current = { ...r, cur };
  const now = ref.current;
  return now.range && Date.now() < now.until ? now.range : null;
}

/* ---------------- 清晰度（M3U8） ---------------- */

function Variants({ j, view }: { j: Job; view: View }) {
  const vars = j.vars ?? [];
  const picking = view === "pick";
  return (
    <>
      <div className="cp-qlist" role="radiogroup" aria-label={t("清晰度")}>
        {vars.map((v, k) => (
          <button key={k} className="cp-qrow" role="radio" aria-checked={k === j.vIdx} disabled={!picking} onClick={() => setVariant(j.id, k)}>
            <span className="radio" />
            <span>
              {v.p}
              {variantNote(k, vars.length) && <small>{variantNote(k, vars.length)}</small>}
            </span>
            <span className="size">{t("约{size}", { size: mb(v.mb) })}</span>
          </button>
        ))}
      </div>
      {picking ? (
        <div className="cp-qfoot">
          <Button size="sm" className="cp-pill" icon={<Icon icon={PlayIcon} size={14} />} onClick={() => jobAction(j.id, "pickgo")}>
            {t("就用{quality}", { quality: variantName(j) })}
          </Button>
        </div>
      ) : (
        view !== "probe" && <p className="cp-dnote">{t("已经开始下载，清晰度不能再改了")}</p>
      )}
    </>
  );
}

/* ---------------- 分段下载（M3U8） ---------------- */

/** 分段太多时几段并成一格，最多画 480 格：有没下载成功的算失败，有重下的算重下，全部下完才算完成 */
function cells(segs: SegState[]): SegState[] {
  const per = Math.ceil(segs.length / 480);
  if (per <= 1) return segs;
  const out: SegState[] = [];
  for (let i = 0; i < segs.length; i += per) {
    const g = segs.slice(i, i + per);
    out.push(g.includes(4) ? 4 : g.includes(3) ? 3 : g.every((x) => x === 2) ? 2 : g.some((x) => x === 1 || x === 2) ? 1 : 0);
  }
  return out;
}

function Segments({ j }: { j: Job }) {
  const c = segCount(j);
  const running = j.state === "run" && j.phase === "dl";
  const left = j.state === "pause" ? t("已暂停") : j.state === "queue" ? t("排队中") : running ? etaOf(j) || t("计算中") : t("快好了");
  const segsLabel = c.retry || c.failed ? t("已下载{done}段，一共{n}段，有几段在重新下载", { done: c.done, n: c.all }) : t("已下载{done}段，一共{n}段", { done: c.done, n: c.all });
  return (
    <>
      <div className="cp-hstats">
        <div>
          <b className="num">{Math.floor(progress(j) * 100)}%</b>
          <span>{t("完成§分段下载的统计：百分比下面的字")}</span>
        </div>
        <div>
          <b className="num">{mb(j.got)}</b>
          <span>{t("已下载")}</span>
        </div>
        <div>
          <b className="num">{running ? `${decimal(j.speed, 1)} MB/s` : "0 MB/s"}</b>
          <span>{t("速度")}</span>
        </div>
        <div>
          <b className="num">{left}</b>
          <span>{t("剩余§分段下载的统计：剩余时间下面的字")}</span>
        </div>
      </div>
      <div className="cp-segs" role="img" aria-label={segsLabel}>
        {cells(j.segs ?? []).map((s, k) => (
          <i key={k} data-s={s} />
        ))}
      </div>
      <div className="cp-legend" aria-hidden>
        <span data-s="2">{t("已下载")}</span>
        <span data-s="1">{t("正在下载")}</span>
        <span data-s="0">{t("还没开始")}</span>
        {c.retry > 0 && <span data-s="3">{t("重新下载中")}</span>}
        {c.failed > 0 && <span data-s="4">{t("没下载成功")}</span>}
      </div>
      {c.failed > 0 ? (
        <div className="cp-segact">
          <span>{t("有一小部分一直没下载成功")}</span>
          <Button size="sm" variant="secondary" className="cp-pill" icon={<Icon icon={RepeatIcon} size={14} />} onClick={() => jobAction(j.id, "segretry")}>
            {t("再试一次")}
          </Button>
        </div>
      ) : c.retry > 0 ? (
        <div className="cp-segact">
          <span>{t("网络有点不稳，有几段正在自动重新下载")}</span>
        </div>
      ) : null}
    </>
  );
}

/* ---------------- 会保存为 ---------------- */

function SaveAs({ j, view, settings }: { j: Job; view: View; settings: Settings | null }) {
  const { from, to } = formatPair(j);
  // 还没下完时大小是估的：「约 312 MB」
  const size = j.size > 0 ? ` · ${j.state === "done" ? mb(j.size) : t("约{size}", { size: mb(j.size) })}` : "";
  return (
    <>
      <div className="cp-saveas">
        <FileIcon fmt={to} size="lg" />
        <div>
          {/* j.fmt 是后端写的（「MP4 · 1080p」），原样显示 */}
          <strong>{untitled(j) && view === "probe" ? t("{fmt} · 读到以后再定", { fmt: to }) : j.fmt}</strong>
          <span>
            {saveDirOf(j, settings)}
            {size}
          </span>
        </div>
      </div>
      {from && j.state !== "done" && (
        <p className="cp-dnote">{view === "merge" ? t("正在转成{fmt} · {pct}%", { fmt: to, pct: Math.round(j.merge) }) : t("下载的是{from}，下完会自动转成{to}", { from, to })}</p>
      )}
      {view === "merge" && (
        <div className="cp-gap">
          <ProgressBar p={j.merge / 100} tick={500} label={t("转换进度")} />
        </div>
      )}
    </>
  );
}

/* ---------------- 下载记录 ---------------- */

function Log({ j }: { j: Job }) {
  const lines = j.log.slice().reverse();
  return (
    <div className="cp-log">
      {lines.length ? (
        lines.map(([t, x, lv], i) => (
          <div key={i} data-lv={lv || undefined}>
            <time className="num">{new Date(t).toTimeString().slice(0, 8)}</time>
            <span>{x}</span>
          </div>
        ))
      ) : (
        <div>
          <time />
          <span>{t("还没有记录")}</span>
        </div>
      )}
    </div>
  );
}
