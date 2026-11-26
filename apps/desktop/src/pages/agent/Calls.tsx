// 「AI 最近做了什么」：全局状态 calls（启动快照 + agent_call 事件实时追加、更新）。
// 一行一次调用：谁、用了什么、参数（失败的换成一句话）、时间、结果；点开看结果说明，失败的看下一步和原文。
// 动效（配方 k2）：新来的一条插在最上面，opacity + 上方 8px，弹簧 0.4s 不回弹，其余行同时让位（popLayout + layout）；
// 一秒内来了三条以上就不播（高频不动画）；换筛选、第一次画出来都不播（功能性数据不重播）。
// 状态从「进行中」变成用时或「失败」：图标互换 100ms（配方 c7）。展开收起用 Collapse，箭头转 200ms。
// 要登录、要验证的下载失败：展开后第一个按钮是「登录 X」「去验证」（和下载页任务行一样），打开登录窗口。
import { AlertCircleIcon, ArrowDown01Icon, Copy01Icon, Folder01Icon, ListViewIcon } from "@hugeicons/core-free-icons";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { memo, useCallback, useMemo, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import { EmptyState } from "@/components/ui/empty-state";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { ShimmerText } from "@/components/ui/shimmer-loader";
import { t, tx } from "@/i18n";
import { errText, reveal } from "@/lib/bridge";
import { useDelayed } from "@/lib/hooks";
import { notify } from "@/lib/notify";
import { REVEAL } from "@/lib/os";
import { go } from "@/lib/route";
import { useApp } from "@/lib/store";
import type { CallRecord } from "@/lib/types";
import { Collapse, Icon, IconSwap } from "@/parts";
import { copyWithToast } from "@/shell/actions";
import { loginForCall } from "./act";
import { argText, callOutputs, callTime, clientName, errHint, errLine, errRaw, loginAct, msText, normTool, resText, shortPath, toolIcon, toolLabel, type AgentInfo } from "./model";

const SPRING = { type: "spring", duration: 0.4, bounce: 0 } as const;
const EXIT = { duration: 0.15, ease: [0.23, 1, 0.32, 1] } as const;
const PAGE = 30;

export function Calls({ info }: { info: AgentInfo | null }) {
  const calls = useApp((s) => s.calls);
  const tools = useApp((s) => s.tools);
  const [filter, setFilter] = useState<"all" | "err">("all");
  const [open, setOpen] = useState<Set<number>>(() => new Set());
  const [limit, setLimit] = useState(PAGE);

  const errs = useMemo(() => calls.filter((c) => c.st === "err"), [calls]);
  const all = filter === "err" ? errs : calls;
  const list = all.slice(0, limit);

  // 工具的名字：后端给的是中文原文，按界面语言显示
  const labels = useMemo(() => new Map(tools.map((tool) => [normTool(tool.name), toolLabel(tool.label)])), [tools]);
  const label = useCallback((name: string) => labels.get(normTool(name)) ?? name, [labels]);

  // 一秒内来了三条以上：直接出现，不播进场（AI 连着调用好几个工具时，一排排往下推会很吵）
  const seen = useRef(new Map<number, number>());
  const now = Date.now();
  for (const c of list) if (!seen.current.has(c.id)) seen.current.set(c.id, now);
  let burst = 0;
  for (const at of seen.current.values()) if (now - at < 1000) burst++;

  const toggle = useCallback((id: number) => {
    setOpen((s) => {
      const n = new Set(s);
      if (n.has(id)) n.delete(id);
      else n.add(id);
      return n;
    });
  }, []);

  return (
    <section className="cp-sec">
      <div className="cp-sec-h">
        <h2>{t("AI 最近做了什么")}</h2>
        {calls.length > 0 && (
          <span className="end">
            <SegmentedControl
              aria-label={t("筛选记录")}
              size="sm"
              value={filter}
              onChange={(v) => {
                setFilter(v as "all" | "err");
                setLimit(PAGE);
              }}
              options={[
                { value: "all", label: t("全部"), count: calls.length || undefined },
                { value: "err", label: t("出错的"), count: errs.length || undefined },
              ]}
            />
          </span>
        )}
      </div>
      {list.length ? (
        <ul className="cp-pane cp-calls" aria-label={t("AI 的调用记录")}>
          {/* 换筛选时整个重来、不播进场（配方 c1）；同一筛选里新来的才有进场和让位 */}
          <AnimatePresence key={filter} mode="popLayout" initial={false}>
            {list.map((c) => (
              <CallRow key={c.id} c={c} label={label(c.tool)} info={info} open={open.has(c.id)} onToggle={toggle} instant={burst > 3} layoutOn={list.length <= 40} />
            ))}
          </AnimatePresence>
        </ul>
      ) : (
        <div className="cp-pane cp-pane-empty">
          {filter === "err" ? (
            <EmptyState bare icon={AlertCircleIcon} title={t("没有出错的记录")} text={t("AI 最近做的事都成功了")} />
          ) : (
            <EmptyState bare icon={ListViewIcon} title={t("还没有记录§AI 的调用记录是空的")} text={t("AI 助手用 Captae 做的每件事都会记在这里")} />
          )}
        </div>
      )}
      {all.length > list.length && (
        <button className="cp-more" onClick={() => setLimit((n) => n + PAGE)}>
          {t("再看 {n} 条更早的", { n: Math.min(PAGE, all.length - list.length) })}
        </button>
      )}
    </section>
  );
}

type RowProps = { c: CallRecord; label: string; info: AgentInfo | null; open: boolean; onToggle: (id: number) => void; instant: boolean; layoutOn: boolean };

const CallRow = memo(function CallRow({ c, label, info, open, onToggle, instant, layoutOn }: RowProps) {
  const reduce = useReducedMotion();
  // 进行中：300ms 以内就做完的不显示，超过了「进行中」三个字扫光（配方 m），不放转圈
  const slow = useDelayed(c.st === "run", 300);
  const err = c.st === "err";
  const sub = err ? errLine(c) : argText(c.args, info);
  return (
    <motion.li
      layout={layoutOn ? "position" : false}
      className="cp-call"
      data-st={c.st}
      data-open={open || undefined}
      initial={instant ? false : { opacity: 0, y: reduce ? 0 : -8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, transition: EXIT }}
      transition={SPRING}
    >
      <button className="cp-call-h" aria-expanded={open} onClick={() => onToggle(c.id)}>
        <span className="ico" aria-hidden>
          <Icon icon={toolIcon(c.tool)} size={15} />
        </span>
        <span className="main">
          <span className="t">
            <b>{clientName(c.who)}</b>
            {label}
          </span>
          <span className={err ? "s err" : "s mono"} title={sub || undefined}>
            {sub || (c.st === "run" ? t("正在进行…") : t("没有参数"))}
          </span>
        </span>
        <span className="meta">
          <time className="num">{callTime(c.at)}</time>
          <IconSwap k={c.st}>
            {c.st === "run" ? (
              slow ? (
                <ShimmerText className="cp-wait">{t("进行中")}</ShimmerText>
              ) : (
                <span className="run" aria-hidden>
                  {"\u00a0"}
                </span>
              )
            ) : err ? (
              <span className="bad">
                <Icon icon={AlertCircleIcon} size={13} />
                {t("失败")}
              </span>
            ) : (
              <span className="num">{msText(c.ms)}</span>
            )}
          </IconSwap>
        </span>
        <Icon icon={ArrowDown01Icon} size={14} className="chev" />
      </button>
      <Collapse open={open}>
        <CallBody c={c} info={info} />
      </Collapse>
    </motion.li>
  );
});

function CallBody({ c, info }: { c: CallRecord; info: AgentInfo | null }) {
  const [raw, setRaw] = useState(false);
  if (c.st === "err") {
    // 那句话已经在行里了，展开后只补：下一步、AI 给的参数、原文
    const hint = errHint(c);
    const text = errRaw(c);
    const args = argText(c.args, info);
    const login = loginAct(c);
    return (
      <div className="cp-call-b">
        {hint && <p className="cp-call-res">{hint}</p>}
        {args && <p className="cp-call-args">{tx("AI 给的参数：{args}", { args: <span className="mono">{args}</span> })}</p>}
        <div className="cp-call-acts">
          {login && (
            <Button size="sm" variant="secondary" className="cp-pill" onClick={() => loginForCall(login)}>
              {login.label}
            </Button>
          )}
          {text && (
            <button className="cp-rawbtn" aria-expanded={raw} onClick={() => setRaw(!raw)}>
              {raw ? t("收起原文") : t("看原文")}
              <Icon icon={ArrowDown01Icon} size={13} className="chev" />
            </button>
          )}
          <Button
            size="sm"
            variant="ghost"
            icon={<Icon icon={Copy01Icon} size={14} />}
            onClick={() => copyWithToast([errLine(c), text, JSON.stringify(c.args ?? {})].filter(Boolean).join("\n"), t("错误信息已复制"), t("遇到问题时可以发给我们"))}
          >
            {t("复制错误信息")}
          </Button>
        </div>
        {text && (
          <Collapse open={raw}>
            <pre className="cp-raw">{text}</pre>
          </Collapse>
        )}
      </div>
    );
  }
  const out = callOutputs(c);
  return (
    <div className="cp-call-b">
      <p className="cp-call-res">{resText(c, info)}</p>
      {(out.asset != null || out.file) && (
        <div className="cp-call-acts">
          {out.asset != null && (
            <Button size="sm" variant="secondary" className="cp-pill" onClick={() => go("library", { asset: out.asset! })}>
              {t("在素材库里看")}
            </Button>
          )}
          {out.file && (
            <Button size="sm" variant="ghost" icon={<Icon icon={Folder01Icon} size={14} />} title={shortPath(out.file, info)} onClick={() => reveal(out.file).catch((e) => notify(t("打不开"), errText(e)))}>
              {t(REVEAL)}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
