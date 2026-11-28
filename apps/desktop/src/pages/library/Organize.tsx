// 「用 AI 整理」弹窗和「管理分类」（core categorize.rs、docs/DESIGN.md 6.1）。
// 整理分三步：说明（会发什么、发给谁、大约多少 token）→ 整理中（进度、能取消）→ 看结果（改名、合并、不要某一类）→ 用这套分类。
// 确认前不改素材库；关掉弹窗 = 不要这次的结果（整理中关掉 = 取消任务）。
// 动效（docs/MOTION.md）：弹窗用 affectUI 自带的进退场；三步之间是状态互换（parts 的 Swap，弹簧 0.3s、不回弹），
// 内容高度跟着变（270ms --ease-height，课程 03/05 多步骤组件的高度动画，不写死高度）；
// 去掉、合并一类时那一行 150ms 淡出缩到 .95、其余行同时合拢（配方 c3，AnimatePresence popLayout + layout="position"，和下载列表一样）。
// 减少动效时只淡入淡出、高度直接到位、其余行直接补位。
import { AlertCircleIcon, Delete02Icon, GitMergeIcon, MoreHorizontalIcon, PlusSignIcon, SparklesIcon } from "@hugeicons/core-free-icons";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { DropdownMenu, type DropdownMenuItem } from "@/components/ui/dropdown-menu";
import { ShimmerText } from "@/components/ui/shimmer-loader";
import { TextField } from "@/components/ui/text-field";
import { t } from "@/i18n";
import { errText, fileSrc, invoke } from "@/lib/bridge";
import { notify } from "@/lib/notify";
import { loadCategories, useApp } from "@/lib/store";
import type { Category } from "@/lib/types";
import { Icon, ProgressBar, Swap } from "@/parts";
import { openSettingsAt } from "@/shell/actions";
import { deleteCategory, mergeCategory, saveCategory, tokenText, type Edit, type Estimate, type Proposal } from "./categories";
import "./organize.css";

export type OrganizeMode = "organize" | "manage";
type Step = "intro" | "run" | "review" | "error";

const EASE_HEIGHT = [0.25, 1, 0.5, 1] as const;
/** 去掉、合并一类时那一行的退场：150ms --ease-out-quint，和下载列表、预设列表同一个值（配方 c3）。
 *  缩放用 Motion 的 scale（不写 transform 字符串）：这一行同时在做布局投影，transform 归 Motion 管 */
const EXIT = { duration: 0.15, ease: [0.23, 1, 0.32, 1] as const };

/** 内容高度跟着变：量里面那一层的高度，外面这层动过去（课程 03/05 的 useMeasure 做法，用 ResizeObserver） */
function AutoHeight({ children }: { children: ReactNode }) {
  const inner = useRef<HTMLDivElement>(null);
  const [h, setH] = useState<number | "auto">("auto");
  const reduce = useReducedMotion();
  useLayoutEffect(() => {
    const el = inner.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setH(el.offsetHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  return (
    <motion.div animate={{ height: h }} initial={false} transition={reduce ? { duration: 0 } : { duration: 0.27, ease: EASE_HEIGHT }} style={{ overflow: "hidden" }}>
      <div ref={inner}>{children}</div>
    </motion.div>
  );
}

export function OrganizeDialog({ open, mode, onClose, onMode }: { open: boolean; mode: OrganizeMode; onClose: () => void; onMode: (m: OrganizeMode) => void }) {
  return mode === "manage" ? <Manage open={open} onClose={onClose} onOrganize={() => onMode("organize")} /> : <Organize open={open} onClose={onClose} />;
}

/* ---------------- 用 AI 整理 ---------------- */

function Organize({ open, onClose }: { open: boolean; onClose: () => void }) {
  const [step, setStep] = useState<Step>("intro");
  const [est, setEst] = useState<Estimate | null>(null);
  const [prop, setProp] = useState<Proposal | null>(null);
  const [edits, setEdits] = useState<Record<string, Edit>>({});
  const [err, setErr] = useState("");
  const [busy, setBusy] = useState(false);
  const tasks = useApp((s) => s.tasks);
  /** 这次整理开始以后才出现的任务（整理任务的编号要等结果回来才知道，按开始的时刻找） */
  const since = useRef(0);
  const settings = useApp((s) => s.settings);

  // 每次打开从头来；设置变了（刚填好模型）重新估一次
  useEffect(() => {
    if (!open) return;
    setStep("intro");
    setProp(null);
    setEdits({});
    setErr("");
    invoke<Estimate>("categorize_estimate")
      .then(setEst)
      .catch(() => setEst(null));
  }, [open, settings?.llmBaseUrl, settings?.llmModel, settings?.aiSendCovers]);

  const task = useMemo(() => {
    if (step !== "run") return null;
    const L = Object.values(tasks).filter((x) => x.kind === "categorize" && x.id > since.current);
    return L.sort((a, b) => b.id - a.id)[0] ?? null;
  }, [tasks, step]);

  const start = async () => {
    since.current = Math.max(0, ...Object.keys(tasks).map(Number));
    setStep("run");
    try {
      const p = await invoke<Proposal>("categorize_propose");
      setProp(p);
      setEdits({});
      setStep("review");
    } catch (e) {
      const m = errText(e);
      if (m === "CANCELLED") setStep("intro");
      else {
        setErr(m.replace(/^[A-Z_]+: /, ""));
        setStep("error");
      }
    }
  };

  const close = () => {
    if (step === "run" && task) invoke("task_cancel", { id: task.id }).catch(() => undefined);
    if (step === "review") invoke("categorize_discard").catch(() => undefined);
    onClose();
  };

  const kept = prop ? prop.categories.filter((c) => !edits[c.key]?.drop && !edits[c.key]?.into) : [];
  /** 合并、删掉以后各类会有几条（合并进来的加上），未分类几条 */
  const counts = useMemo(() => {
    const m: Record<string, number> = {};
    let none = prop?.none ?? 0;
    if (!prop) return { m, none };
    const target = (k: string): string | null => {
      for (let i = 0, cur = k; i < 16; i++) {
        const e = edits[cur];
        if (e?.drop) return null;
        if (e?.into && e.into !== cur) cur = e.into;
        else return cur;
      }
      return null;
    };
    for (const c of prop.categories) {
      const to = target(c.key);
      if (to) m[to] = (m[to] ?? 0) + c.count;
      else none += c.count;
    }
    return { m, none };
  }, [prop, edits]);

  const apply = async () => {
    setBusy(true);
    try {
      // 按现在的顺序发：没动过的也发一条（后端照这个顺序排）
      const list: Edit[] = prop!.categories.map((c) => ({ ...edits[c.key], key: c.key }));
      const r = await invoke<{ categories: number; filed: number; none: number }>("categorize_apply", { edits: list });
      await loadCategories();
      onClose();
      const note = [t("{n}项放进了分类", { n: r.filed }), r.none ? t("{n}项没把握，还在未分类", { n: r.none }) : ""].filter(Boolean).join(t("，§两句说明之间的分隔"));
      notify(t("整理好了：{n}个分类", { n: r.categories }), note);
    } catch (e) {
      notify(t("没有用上这套分类"), errText(e));
    } finally {
      setBusy(false);
    }
  };

  const title = step === "review" ? t("分成了{n}类", { n: kept.length }) : t("用 AI 整理素材库");

  const footer =
    step === "intro" ? (
      <>
        <Button variant="secondary" className="cp-pill" onClick={close}>
          {t("取消")}
        </Button>
        {est?.hasModel ? (
          <Button className="cp-pill" icon={<Icon icon={SparklesIcon} size={14} />} disabled={!est.items} onClick={start} data-autofocus>
            {t("开始整理")}
          </Button>
        ) : (
          <Button
            className="cp-pill"
            data-autofocus
            onClick={() => {
              onClose();
              openSettingsAt("ai");
            }}
          >
            {t("去设置 AI 模型")}
          </Button>
        )}
      </>
    ) : step === "run" ? (
      <Button variant="secondary" className="cp-pill" onClick={close}>
        {t("取消整理")}
      </Button>
    ) : step === "error" ? (
      <>
        <Button variant="secondary" className="cp-pill" onClick={onClose}>
          {t("关闭")}
        </Button>
        <Button className="cp-pill" onClick={start} data-autofocus>
          {t("再试一次")}
        </Button>
      </>
    ) : (
      <>
        <Button variant="secondary" className="cp-pill" onClick={close}>
          {t("不用了")}
        </Button>
        <Button className="cp-pill" disabled={!kept.length || busy} onClick={apply} data-autofocus>
          {t("用这套分类")}
        </Button>
      </>
    );

  return (
    <Dialog className="cp org-dlg" open={open} onOpenChange={(o) => !o && close()} size="md" title={title} closeLabel={t("关闭")} closeOnBackdrop={step !== "run" && step !== "review"} footer={footer}>
      <AutoHeight>
        <Swap k={step}>
          {step === "intro" && <Intro est={est} />}
          {step === "run" && <Running pct={task?.pct ?? 0} note={task?.note} />}
          {step === "error" && (
            <div className="cp-note org-err" data-tone="err">
              <Icon icon={AlertCircleIcon} size={16} />
              <div className="grow">
                {t("这次没有整理完")}
                <small>{err}</small>
              </div>
            </div>
          )}
          {step === "review" && prop && (
            <Review
              prop={prop}
              kept={kept}
              counts={counts}
              edit={(k, e) => setEdits((E) => ({ ...E, [k]: { ...E[k], ...e, key: k } }))}
            />
          )}
        </Swap>
      </AutoHeight>
    </Dialog>
  );
}

function Intro({ est }: { est: Estimate | null }) {
  if (!est) return <div className="org-body org-wait" />;
  return (
    <div className="org-body">
      <p className="cp-dlg-p">{t("按内容把素材分成几类，每条放进一类，再加两三个标签方便以后搜。分好以后可以改名、合并、删掉；新下载的会自动放进去。")}</p>
      {!est.hasModel ? (
        <div className="cp-note" data-tone="warn">
          <Icon icon={SparklesIcon} size={16} />
          <div className="grow">
            {t("还没有设置 AI 模型")}
            <small>{t("用你自己的模型服务和密钥（DeepSeek、OpenAI、本地的 Ollama……），在设置 › AI 模型里填上")}</small>
          </div>
        </div>
      ) : !est.items ? (
        <div className="cp-note">
          <div className="grow">{est.existing ? t("素材都由你亲手放好了，没有要 AI 整理的") : t("素材库还是空的，先下载一些再来")}</div>
        </div>
      ) : (
        <dl className="cp-kv org-kv">
          <dt>{t("发给§整理前的说明：发给哪个模型服务")}</dt>
          <dd>
            <span className="mono">{est.host}</span> · {est.model}
          </dd>
          <dt>{t("内容§整理前的说明：发过去的是什么")}</dt>
          <dd>
            {t("{n}项素材的标题、简介和来源", { n: est.items })}
            {est.covers > 0 && <small>{t("其中{n}项光看文字看不出内容，会附上封面", { n: est.covers })}</small>}
          </dd>
          <dt>{t("用量§整理前的说明：大约多少 token")}</dt>
          <dd>
            {t("大约 {n} token", { n: tokenText(est.tokens) })}
            <small>{t("按你的模型服务的价格算，费用记在你自己的账户上")}</small>
          </dd>
          {est.existing > 0 && (
            <>
              <dt>{t("已有§整理前的说明：已经有的分类")}</dt>
              <dd>{est.userFiled ? t("你建的分类和你亲手放的{n}项素材不会动", { n: est.userFiled }) : t("你建的分类不会动，AI 建的会重新整理")}</dd>
            </>
          )}
        </dl>
      )}
    </div>
  );
}

function Running({ pct, note }: { pct: number; note?: string }) {
  return (
    <div className="org-body org-run">
      <ProgressBar p={pct / 100} indeterminate={pct <= 0} tick={400} label={t("整理进度")} />
      <div className="org-run-note" aria-live="polite">
        {/* 没有数字的那一步（「正在想怎么分」）让文字扫光（配方 m） */}
        {note && /\d/.test(note) ? note : <ShimmerText className="cp-wait">{note || t("正在准备")}</ShimmerText>}
      </div>
      <p className="org-run-hint">{t("关掉这个窗口就是取消，已经看过的封面描述会留着，下次不用再看")}</p>
    </div>
  );
}

function Covers({ samples }: { samples: { id: number; title: string; thumb?: string | null }[] }) {
  return (
    <div className="org-covers" aria-hidden>
      {samples.slice(0, 3).map((s) => (s.thumb ? <img key={s.id} src={fileSrc(s.thumb)} alt="" loading="lazy" decoding="async" /> : <span key={s.id} className="ph" />))}
      {!samples.length && <span className="ph" />}
    </div>
  );
}

function Review({
  prop,
  kept,
  counts,
  edit,
}: {
  prop: Proposal;
  kept: Proposal["categories"];
  counts: { m: Record<string, number>; none: number };
  edit: (key: string, e: Partial<Edit>) => void;
}) {
  const reduce = useReducedMotion();
  return (
    <div className="org-body">
      <p className="cp-dlg-p">
        {t("整理了{n}项。名字可以直接改；不合适的可以并进别的类，或者不要这一类（里面的放回未分类）。", { n: prop.total })}
      </p>
      <motion.ul className="org-list" layoutScroll>
        <AnimatePresence mode="popLayout" initial={false}>
          {kept.map((c) => {
            const others = kept.filter((x) => x.key !== c.key);
            const items: DropdownMenuItem[] = [
              ...(others.length ? [{ label: t("并进别的类"), icon: GitMergeIcon, submenu: others.map((o) => ({ label: o.name, onSelect: () => edit(c.key, { into: o.key }) })) }] : []),
              { label: t("不要这一类"), icon: Delete02Icon, danger: true, divider: others.length > 0, onSelect: () => edit(c.key, { drop: true }) },
            ];
            return (
              <motion.li
                key={c.key}
                layout={reduce ? false : "position"}
                className="org-row"
                exit={{ opacity: 0, scale: reduce ? 1 : 0.95, transition: EXIT }}
                transition={{ type: "spring", duration: 0.3, bounce: 0 }}
              >
                <Covers samples={c.samples} />
                <div className="org-main">
                  {c.fixed ? (
                    <b className="org-fixed">{c.name}</b>
                  ) : (
                    <NameField value={c.name} label={t("分类名")} onChange={(name) => edit(c.key, { name })} />
                  )}
                  {c.desc && <small>{c.desc}</small>}
                </div>
                <span className="org-n num">{t("{n}项", { n: counts.m[c.key] ?? 0 })}</span>
                {c.fixed ? (
                  <span className="org-tag">{t("你建的")}</span>
                ) : (
                  <DropdownMenu
                    align="end"
                    aria-label={t("这一类的操作")}
                    items={items}
                    trigger={({ triggerProps }) => (
                      <button className="cp-round org-more" aria-label={t("这一类的操作")} {...triggerProps}>
                        <Icon icon={MoreHorizontalIcon} size={16} />
                      </button>
                    )}
                  />
                )}
              </motion.li>
            );
          })}
        </AnimatePresence>
      </motion.ul>
      {counts.none > 0 && <p className="org-none">{t("{n}项没把握，会留在未分类，之后可以自己放", { n: counts.none })}</p>}
    </div>
  );
}

/** 分类名输入框：打字时只改本地，离开输入框或回车才算 */
function NameField({ value, label, onChange, placeholder }: { value: string; label: string; onChange: (v: string) => void; placeholder?: string }) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  const commit = () => {
    const n = v.trim();
    if (n && n !== value) onChange(n);
    else setV(value);
  };
  return (
    <TextField
      size="sm"
      aria-label={label}
      placeholder={placeholder}
      className="org-name"
      value={v}
      maxLength={24}
      onChange={(e) => setV(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          commit();
        } else if (e.key === "Escape") {
          e.stopPropagation();
          setV(value);
        }
      }}
    />
  );
}

/* ---------------- 管理分类 ---------------- */

function Manage({ open, onClose, onOrganize }: { open: boolean; onClose: () => void; onOrganize: () => void }) {
  const cats = useApp((s) => s.categories);
  const none = useApp((s) => s.uncategorized);
  const reduce = useReducedMotion();
  const [adding, setAdding] = useState("");

  const menu = (c: Category): DropdownMenuItem[] => {
    const others = cats.filter((x) => x.id !== c.id);
    return [
      ...(others.length ? [{ label: t("并进别的类"), icon: GitMergeIcon, submenu: others.map((o) => ({ label: o.name, onSelect: () => mergeCategory(c, o) })) }] : []),
      { label: t("删掉这一类"), icon: Delete02Icon, danger: true, divider: others.length > 0, onSelect: () => deleteCategory(c) },
    ];
  };

  const add = async () => {
    const n = adding.trim();
    if (!n) return;
    if (await saveCategory(n)) setAdding("");
  };

  return (
    <Dialog
      className="cp org-dlg"
      open={open}
      onOpenChange={(o) => !o && onClose()}
      size="md"
      title={t("管理分类")}
      closeLabel={t("关闭")}
      footer={
        <>
          <Button variant="secondary" className="cp-pill" icon={<Icon icon={SparklesIcon} size={14} />} onClick={onOrganize}>
            {t("重新整理")}
          </Button>
          <Button className="cp-pill" onClick={onClose} data-autofocus>
            {t("完成")}
          </Button>
        </>
      }
    >
      <div className="org-body">
        <p className="cp-dlg-p">{t("改名、合并、删掉都马上生效。删掉的分类里的素材放回未分类。")}</p>
        <motion.ul className="org-list" layoutScroll>
          <AnimatePresence mode="popLayout" initial={false}>
            {cats.map((c) => (
              <motion.li
                key={c.id}
                layout={reduce ? false : "position"}
                className="org-row"
                exit={{ opacity: 0, scale: reduce ? 1 : 0.95, transition: EXIT }}
                transition={{ type: "spring", duration: 0.3, bounce: 0 }}
              >
                <div className="org-main">
                  <NameField value={c.name} label={t("分类名")} onChange={(name) => saveCategory(name, c.id)} />
                  {c.desc && <small>{c.desc}</small>}
                </div>
                <span className="org-n num">{t("{n}项", { n: c.count })}</span>
                <DropdownMenu
                  align="end"
                  aria-label={t("这一类的操作")}
                  items={menu(c)}
                  trigger={({ triggerProps }) => (
                    <button className="cp-round org-more" aria-label={t("这一类的操作")} {...triggerProps}>
                      <Icon icon={MoreHorizontalIcon} size={16} />
                    </button>
                  )}
                />
              </motion.li>
            ))}
          </AnimatePresence>
          <li className="org-row org-add">
            <div className="org-main">
              <TextField
                size="sm"
                aria-label={t("新分类的名字")}
                placeholder={t("新分类的名字，回车建好")}
                prefix={<Icon icon={PlusSignIcon} size={14} />}
                value={adding}
                maxLength={24}
                onChange={(e) => setAdding(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    add();
                  }
                }}
              />
            </div>
          </li>
        </motion.ul>
        {none > 0 && <p className="org-none">{t("还有{n}项未分类", { n: none })}</p>}
      </div>
    </Dialog>
  );
}
