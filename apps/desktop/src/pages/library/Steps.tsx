// 字幕、剪辑的进度：画成步骤时间线（和下载页同一个 StepTimeline），进度百分比落在第几步就停在第几步。
// 一步完成时描线画勾（配方 e3 / m1），已经完成的直接显示终态；动画由真实进度触发，不人为延迟。
// 正在跑的处理旁边有「取消」（affectUI Button ghost，不加动效）；取消以后进度块用现成的 Collapse 收起（配方 e，200ms）。
// 拼接按第一段记，进度和「取消」画在第一段详情的「处理」最上面。
import { useRef } from "react";
import { Button } from "@/components/ui/button";
import { t } from "@/i18n";
import { Collapse, ProgressBar, StepTimeline, type Step } from "@/parts";
import { cancelOp, useOp, type Op, type OpKind } from "./act";
import type { Asset } from "./model";

/** 刚刚完成、要画一次勾的那几步 [从, 到)；换了一次任务或者一下子跳完，都直接显示终态 */
function useFresh(k: number, at: number): [number, number] | null {
  const ref = useRef<{ at: number; k: number; range: [number, number] | null; until: number }>({ at, k, range: null, until: 0 });
  const r = ref.current;
  if (r.at !== at) ref.current = { at, k, range: null, until: 0 };
  else if (k > r.k) ref.current = { at, k, range: [r.k, k], until: Date.now() + 700 };
  const now = ref.current;
  return now.range && Date.now() < now.until ? now.range : null;
}

export function OpSteps({ op }: { op: Op }) {
  const steps = op.steps ?? [];
  const n = steps.length;
  const k = op.st === "ok" ? n : Math.max(0, Math.min(n - 1, Math.floor((op.p / 100) * n)));
  const fresh = useFresh(k, op.at);
  // 别处发起的任务（AI 助手、预设……）不知道分几步：只画一条进度
  if (!n)
    return (
      <div className="cp-tprog lib-oprog" aria-live="polite">
        <ProgressBar p={op.p / 100} tone={op.st === "err" ? "err" : undefined} tick={250} label={t("进度")} />
        <span className="num">{op.st === "err" ? t("没有成功") : op.st === "ok" ? t("好了") : `${Math.round(op.p)}%`}</span>
      </div>
    );
  const list: Step[] = steps.map((label, i) => ({
    label,
    s: i < k ? "done" : i === k ? (op.st === "err" ? "err" : "now") : "todo",
    sub: i === k ? (op.st === "err" ? op.err || t("没有成功") : op.st === "queued" ? t("排队中") : `${Math.round(op.p)}%`) : undefined,
    fresh: !!fresh && i >= fresh[0] && i < fresh[1],
  }));
  return (
    <div className="lib-steps" aria-live="polite">
      <StepTimeline steps={list} />
    </div>
  );
}

/** 进度块：开始时展开，取消以后收起（Collapse）；收起的那 200ms 里还画最后的样子 */
export function OpBlock({ op }: { op: Op | undefined }) {
  const last = useRef(op);
  if (op) last.current = op;
  return <Collapse open={!!op}>{last.current && <OpSteps op={last.current} />}</Collapse>;
}

/** 正在跑（或排着队）的处理旁边的「取消」。任务编号还没从 task 事件认出来的那几十毫秒、已经点过的，先不能点 */
export function StopButton({ a, k, op }: { a: Asset; k: OpKind; op: Op | undefined }) {
  if (!op || (op.st !== "run" && op.st !== "queued")) return null;
  const wait = op.st === "run" && (op.task == null || !!op.stopping);
  return (
    <Button variant="ghost" size="sm" disabled={wait} onClick={() => cancelOp(a, k, op)}>
      {op.stopping ? t("正在取消") : t("取消")}
    </Button>
  );
}

/** 拼接的进度（按第一段记，别处发起的拼接也算）：第一段详情「处理」最上面，带「取消」；做完、失败、取消以后收起 */
export function ConcatOp({ a }: { a: Asset }) {
  const op = useOp(a.id, "concat");
  const on = op?.st === "run" || op?.st === "queued";
  const last = useRef(op);
  if (on) last.current = op;
  const o = last.current;
  return (
    <Collapse open={on}>
      {o && (
        <div className="lib-catop">
          <div className="lib-catop-h">
            {/* 这里发起的按段数整句写；别处发起的（AI 助手）只有后端的任务标题 */}
            <span>{o.count ? t("正在拼接{n}段视频", { n: o.count }) : o.label ? t("正在{task}", { task: o.label }) : t("正在拼接")}</span>
            <StopButton a={a} k="concat" op={on ? op : undefined} />
          </div>
          <OpSteps op={o} />
        </div>
      )}
    </Collapse>
  );
}
