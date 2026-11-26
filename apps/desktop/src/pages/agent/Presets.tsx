// 「预设」：把常做的几步存下来（set_presets），AI 助手也能按名字用。
// 每张卡片：步骤（Captae 看不懂的那一步提前说清楚，preset_check）、「移动到指定文件夹」选文件夹（pick_folder）、
// 「AI 助手也能用」开关、试运行（选一个素材，preset_trial，每一步的结果从 preset_step 事件来）。
// 动效：卡片加进来和删掉照下载列表（c1、c3）；试运行时一步做完描线画勾（配方 e3，进来时已经完成的直接显示）；
// 进行中的那一步是 opacity 脉冲（配方 m4）；试运行的结果那一行出现用 Collapse，状态变化用 Swap（配方 g3）。
// 试运行中可以「取消」（task_cancel）：取消以后那一行用 Collapse 收起，步骤回到平时的图标。
import {
  Alert02Icon,
  CheckmarkCircle02Icon,
  Delete02Icon,
  Edit02Icon,
  MoreHorizontalIcon,
  PlayIcon,
  PlusSignIcon,
  SparklesIcon,
} from "@hugeicons/core-free-icons";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useEffect, useRef, useState, type Ref } from "react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { DropdownMenu } from "@/components/ui/dropdown-menu";
import { EmptyState } from "@/components/ui/empty-state";
import { IconButton } from "@/components/ui/icon-button";
import { Select } from "@/components/ui/select";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { TextField } from "@/components/ui/text-field";
import { t, tx } from "@/i18n";
import { errText } from "@/lib/bridge";
import { useDelayed } from "@/lib/hooks";
import { notify } from "@/lib/notify";
import { go } from "@/lib/route";
import { getState, useApp } from "@/lib/store";
import type { Preset } from "@/lib/types";
import { Collapse, Icon, Swap, TipIcon } from "@/parts";
import {
  cancelTrial,
  dropTrial,
  loadTrialAssets,
  pickFolder,
  removePreset,
  renameTrial,
  savePresets,
  startTrial,
  useKnownSteps,
  useTrials,
  useTrialSteps,
  type Trial,
} from "./act";
import { isDownload, isMove, joinSentences, pickDir, shortPath, STEP_OPTS, stepIcon, stepLabel, type AgentInfo, type AssetLite, type StepResult } from "./model";

const SPRING = { type: "spring", duration: 0.4, bounce: 0 } as const;
const EXIT = { duration: 0.15, ease: [0.23, 1, 0.32, 1] } as const;

export function Presets({ ref, info }: { ref?: Ref<HTMLElement>; info: AgentInfo | null }) {
  const presets = useApp((s) => s.presets);
  const trials = useTrials();
  const stepsByTask = useTrialSteps();
  const known = useKnownSteps(presets.flatMap((p) => p.steps));
  const reduce = useReducedMotion();
  const [editing, setEditing] = useState<number | "new" | null>(null);
  const [trial, setTrial] = useState<{ p: Preset; items: AssetLite[] } | null>(null);
  const [loadingTrial, setLoadingTrial] = useState<string | null>(null);
  const trialSlow = useDelayed(loadingTrial != null, 300);

  const openTrial = (p: Preset) => {
    setLoadingTrial(p.name);
    loadTrialAssets()
      .then((items) => {
        if (items.length) setTrial({ p, items });
        else notify(t("素材库里还没有视频或音频"), t("先下载一个，再来试运行"));
      })
      .catch((e) => notify(t("读不到素材库"), errText(e)))
      .finally(() => setLoadingTrial(null));
  };

  const update = (name: string, patch: Partial<Preset>) => savePresets(getState().presets.map((p) => (p.name === name ? { ...p, ...patch } : p)));

  return (
    <section className="cp-sec" ref={ref} id="presets">
      <div className="cp-sec-h">
        <h2>{t("预设")}</h2>
        <span className="end">
          <Button variant="secondary" size="sm" className="cp-pill" icon={<Icon icon={PlusSignIcon} size={14} />} onClick={() => setEditing("new")}>
            {t("新建")}
          </Button>
        </span>
        <p className="wide">{t("把常做的几步存下来，下次一键完成；AI 助手也能按名字使用")}</p>
      </div>
      {presets.length ? (
        <div className="cp-presets">
          <AnimatePresence mode="popLayout" initial={false}>
            {presets.map((p, k) => {
              const trial = trials[p.name];
              return (
                <motion.article
                  key={p.name}
                  layout="position"
                  className="cp-preset"
                  initial={{ opacity: 0, y: reduce ? 0 : -8 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, scale: reduce ? 1 : 0.95, transition: EXIT }}
                  transition={SPRING}
                >
                  <div className="cp-preset-h">
                    <h3>{p.name}</h3>
                    <DropdownMenu
                      align="end"
                      aria-label={t("预设「{name}」的更多操作", { name: p.name })}
                      items={[
                        { label: t("编辑…"), icon: Edit02Icon, onSelect: () => setEditing(k) },
                        { label: t("试运行…"), icon: PlayIcon, disabled: trial?.st === "run", onSelect: () => openTrial(p) },
                        { label: t("删除"), icon: Delete02Icon, danger: true, divider: true, onSelect: () => removePreset(p.name) },
                      ]}
                      trigger={({ triggerProps }) => <IconButton icon={MoreHorizontalIcon} label={t("更多")} size="sm" title="" {...triggerProps} />}
                    />
                  </div>
                  <PresetSteps p={p} results={trial?.task != null ? stepsByTask[trial.task] : undefined} known={known} info={info} onPickDir={() => chooseDir(p, info)} />
                  <Collapse open={!!trial}>{trial && <TrialNote name={p.name} trial={trial} />}</Collapse>
                  <div className="cp-preset-f">
                    <Switch size="sm" label={t("AI 助手也能用")} checked={p.agent} onChange={(e) => update(p.name, { agent: e.target.checked })} />
                    <Button
                      variant="secondary"
                      size="sm"
                      className="cp-pill"
                      icon={<Icon icon={PlayIcon} size={14} />}
                      loading={trial?.st === "run" || (trialSlow && loadingTrial === p.name)}
                      onClick={() => openTrial(p)}
                    >
                      {trial?.st === "run" ? t("试运行中") : t("试运行")}
                    </Button>
                  </div>
                </motion.article>
              );
            })}
          </AnimatePresence>
        </div>
      ) : (
        <div className="cp-pane cp-pane-empty">
          <EmptyState
            bare
            icon={SparklesIcon}
            title={t("还没有预设")}
            text={t("把常用的下载和处理步骤组合起来，比如「下载 1080p → 去掉声音 → 转为 WebM」")}
            action={
              <Button variant="secondary" size="sm" className="cp-pill" icon={<Icon icon={PlusSignIcon} size={14} />} onClick={() => setEditing("new")}>
                {t("新建预设")}
              </Button>
            }
          />
        </div>
      )}
      <Editor editing={editing} info={info} onClose={() => setEditing(null)} />
      <TrialDialog trial={trial} onClose={() => setTrial(null)} known={known} />
    </section>
  );
}

function chooseDir(p: Preset, info: AgentInfo | null) {
  pickFolder(t("「{name}」要把文件放到哪里", { name: p.name })).then((d) => {
    if (!d) return;
    savePresets(getState().presets.map((x) => (x.name === p.name ? { ...x, dir: d } : x)));
    notify(t("已设置文件夹"), shortPath(d, info));
  });
}

/* ---------------- 步骤 ---------------- */

type StepView = "idle" | "now" | "done" | "err" | "skip";

/** 用 .cp-steps / .cp-step 的样子（和下载详情的步骤时间线一样）；平时每一步是它的图标，试运行时换成状态 */
function PresetSteps({
  p,
  results,
  known,
  info,
  onPickDir,
}: {
  p: Preset;
  results?: Record<number, StepResult>;
  known: (s: string) => boolean | undefined;
  info: AgentInfo | null;
  onPickDir: () => void;
}) {
  // 这次打开页面以后才做完的那一步画一次勾；进来时已经做完的直接显示（配方 e3）
  const prev = useRef<Record<number, StepView> | null>(null);
  const fresh = useRef<Record<number, number>>({});
  const views = p.steps.map((_, j): StepView => {
    const r = results?.[j];
    return !r ? "idle" : r.status === "run" ? "now" : r.status === "ok" ? "done" : r.status === "err" ? "err" : "skip";
  });
  const now = Date.now();
  const last = useRef(0);
  if (prev.current) {
    const just = views.map((v, j) => v === "done" && prev.current![j] !== "done");
    // 一下子做完好几步，或者离上一步做完不到 300ms：直接显示终态，不排队画勾（配方 e3「快任务跳过」）
    const quick = just.filter(Boolean).length > 1 || now - last.current < 300;
    just.forEach((on, j) => {
      if (!on) return;
      if (!quick) fresh.current[j] = now + 600;
      last.current = now;
    });
  }
  useEffect(() => {
    prev.current = Object.fromEntries(views.map((v, j) => [j, v]));
  });

  return (
    <ol className="cp-steps cp-psteps">
      {p.steps.map((s, j) => {
        const v = views[j];
        const r = results?.[j];
        const go = (fresh.current[j] ?? 0) > now;
        const bad = known(s) === false;
        return (
          <li key={j} className="cp-step" data-s={v} data-bad={(bad && v === "idle") || undefined}>
            <span className="ico" aria-hidden>
              {bad && v === "idle" ? <Icon icon={Alert02Icon} size={16} /> : <StepGlyph v={v} step={s} fresh={go} />}
            </span>
            <div className="body">
              <div className="label">{stepLabel(s)}</div>
              {r && (r.output || r.detail || r.status === "skip" || r.status === "err") && (
                <div className="sub">
                  {r.status === "ok" ? (r.output ? "→ " + shortPath(r.output, info) : r.detail) : r.status === "run" ? r.detail : r.detail || (r.status === "skip" ? t("跳过了") : t("没有成功"))}
                </div>
              )}
              {bad && !r && <div className="sub warn">{t("Captae 看不懂这一步，运行时会跳过")}</div>}
              {pickDir(s) && (
                <div className="sub">
                  {p.dir ? t("放到 {path}", { path: shortPath(p.dir, info) }) : t("还没选文件夹，会放到 ~/Captae")}
                  {" · "}
                  <button className="cp-link" onClick={onPickDir}>
                    {p.dir ? t("换一个") : t("选文件夹")}
                  </button>
                </div>
              )}
            </div>
          </li>
        );
      })}
    </ol>
  );
}

function StepGlyph({ v, step, fresh }: { v: StepView; step: string; fresh: boolean }) {
  if (v === "idle") return <Icon icon={stepIcon(step)} size={16} />;
  return (
    <svg viewBox="0 0 18 18" width="18" height="18" fill="none">
      {v === "done" && (
        <>
          <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5" pathLength={100} className={`cp-draw${fresh ? " go" : ""}`} />
          <path d="M5.9 9.2l2.1 2.1 4.1-4.4" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" pathLength={100} className={`cp-draw${fresh ? " go late" : ""}`} />
        </>
      )}
      {v === "now" && (
        <>
          <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeOpacity=".35" strokeWidth="1.5" />
          <circle className="cp-pulse" cx="9" cy="9" r="3.4" fill="currentColor" />
        </>
      )}
      {v === "skip" && (
        <>
          <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5" strokeDasharray="2.2 2.4" />
          <path d="M6 9h6" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </>
      )}
      {v === "err" && (
        <>
          <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5" />
          <path d="M9 5.6v4" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
          <circle cx="9" cy="12.3" r="1" fill="currentColor" />
        </>
      )}
    </svg>
  );
}

/** 试运行的结果那一行：进行中、完成（生成了几个文件，能去素材库看）、没走完 */
function TrialNote({ name, trial }: { name: string; trial: Trial }) {
  return (
    <Swap k={trial.st} className="cp-trialnote">
      {trial.st === "run" ? (
        <div className="cp-note" data-tone="wait">
          <Spinner size={15} />
          <div className="grow">
            {t("正在试运行")}
            {trial.asset && <small>{tx("用的是「{asset}」，新文件会放进素材库，原文件不动", { asset: trial.asset })}</small>}
          </div>
          <Button variant="ghost" size="sm" disabled={trial.task == null || trial.stopping} onClick={() => cancelTrial(name)}>
            {trial.stopping ? t("正在取消") : t("取消")}
          </Button>
        </div>
      ) : trial.st === "ok" ? (
        <div className="cp-note" data-tone="ok">
          <Icon icon={CheckmarkCircle02Icon} size={16} />
          <div className="grow">
            {t("试运行完成")}
            <small>{trial.outputs ? t("生成了 {n} 个文件", { n: trial.outputs }) : t("每一步都走完了")}</small>
          </div>
          {trial.outputs && trial.assetId != null ? (
            <Button variant="secondary" size="sm" className="cp-pill" onClick={() => go("library", { asset: trial.assetId! })}>
              {t("在素材库里看")}
            </Button>
          ) : null}
          <TipIcon size="sm" icon={Delete02Icon} label={t("清掉这次的结果")} onClick={() => dropTrial(name)} />
        </div>
      ) : (
        <div className="cp-note" data-tone="err">
          <Icon icon={Alert02Icon} size={16} />
          <div className="grow">
            {t("试运行没有走完")}
            {trial.err && <small>{trial.err}</small>}
          </div>
          <TipIcon size="sm" icon={Delete02Icon} label={t("清掉这次的结果")} onClick={() => dropTrial(name)} />
        </div>
      )}
    </Swap>
  );
}

/* ---------------- 新建 / 编辑 ---------------- */

// 存下来的是步骤的中文原文（不翻译），显示用 stepLabel
const NEW_STEPS = ["下载最高画质", "转为 MP4"];

function Editor({ editing, info, onClose }: { editing: number | "new" | null; info: AgentInfo | null; onClose: () => void }) {
  const presets = useApp((s) => s.presets);
  const open = editing != null;
  const [name, setName] = useState("");
  const [steps, setSteps] = useState<string[]>([]);
  const [dir, setDir] = useState<string | null>(null);
  const known = useKnownSteps(steps);

  // 打开时按要编辑的预设（或新建的默认两步）填好；关上时不清，退场动画里还是原来的内容
  const [shownFor, setShownFor] = useState<number | "new" | null>(null);
  if (open && editing !== shownFor) {
    setShownFor(editing);
    const p = editing === "new" ? null : presets[editing];
    setName(p?.name ?? "");
    setSteps(p ? p.steps.slice() : NEW_STEPS.slice());
    setDir(p?.dir ?? null);
  }
  if (!open && shownFor !== null) setShownFor(null);

  const idx = typeof editing === "number" ? editing : -1;
  const clean = name.trim();
  const dup = presets.some((p, i) => p.name === clean && i !== idx);
  const can = !!clean && !dup && steps.length > 0;

  const save = () => {
    if (!can) return;
    const d = steps.some(pickDir) ? dir : null;
    const list = presets.slice();
    if (idx < 0) list.push({ name: clean, steps, agent: true, ...(d ? { dir: d } : {}) });
    else {
      const old = list[idx];
      renameTrial(old.name, clean);
      const { dir: _old, ...rest } = old;
      list[idx] = { ...rest, name: clean, steps, ...(d ? { dir: d } : {}) };
    }
    savePresets(list);
    notify(
      idx < 0 ? t("已保存预设「{name}」", { name: clean }) : t("已更新预设「{name}」", { name: clean }),
      steps.some((s) => known(s) === false) ? t("有步骤 Captae 看不懂，运行时会跳过") : t("下次一键就能用"),
    );
    onClose();
  };

  const setStep = (i: number, v: string) => setSteps((L) => L.map((x, k) => (k === i ? v : x)));

  return (
    <Dialog
      className="cp"
      size="md"
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={editing === "new" || shownFor === "new" ? t("新建预设") : t("编辑预设")}
      description={t("按顺序选几步，以后一键完成")}
      closeLabel={t("关闭")}
      footer={
        <>
          <Button variant="secondary" className="cp-pill" onClick={onClose}>
            {t("取消")}
          </Button>
          <Button className="cp-pill" disabled={!can} onClick={save}>
            {t("保存预设")}
          </Button>
        </>
      }
    >
      <div className="cp-editor">
        <TextField
          label={t("名称")}
          placeholder={t("例如：小红书素材")}
          value={name}
          error={dup ? t("已经有叫这个名字的预设了") : undefined}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && !e.nativeEvent.isComposing && save()}
          data-autofocus
        />
        <div className="cp-editor-steps">
          <div className="lbl">{t("步骤")}</div>
          <ol>
            {steps.map((s, i) => {
              const bad = known(s) === false;
              const opts = STEP_OPTS.includes(s) ? STEP_OPTS : [s, ...STEP_OPTS];
              return (
                <li key={i}>
                  <span className="n num">{i + 1}</span>
                  {/* 值是存下来的原文，显示的是界面语言 */}
                  <Select
                    size="sm"
                    aria-label={t("第 {n} 步", { n: i + 1 })}
                    placeholder={t("选一步")}
                    value={s}
                    onChange={(v) => setStep(i, v)}
                    options={opts.map((o) => ({ value: o, label: stepLabel(o) }))}
                  />
                  {pickDir(s) && (
                    <Button
                      variant="secondary"
                      size="sm"
                      className="cp-pill"
                      onClick={() =>
                        pickFolder(t("要把文件放到哪里")).then((d) => {
                          if (d) setDir(d);
                        })
                      }
                    >
                      {dir ? t("换文件夹") : t("选文件夹")}
                    </Button>
                  )}
                  <TipIcon size="sm" icon={Delete02Icon} label={t("去掉第 {n} 步", { n: i + 1 })} onClick={() => setSteps((L) => L.filter((_, k) => k !== i))} />
                  {bad ? (
                    <p className="note" data-tone="warn">
                      {t("Captae 看不懂这一步，运行时会跳过。换成列表里的步骤吧。")}
                    </p>
                  ) : pickDir(s) ? (
                    <p className="note">{dir ? t("放到 {path}", { path: shortPath(dir, info) }) : t("还没选文件夹，会放到 ~/Captae")}</p>
                  ) : null}
                </li>
              );
            })}
          </ol>
          <Button variant="ghost" size="sm" icon={<Icon icon={PlusSignIcon} size={14} />} onClick={() => setSteps((L) => [...L, STEP_OPTS[0]])}>
            {t("添加步骤")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

/* ---------------- 试运行：选素材 ---------------- */

function TrialDialog({ trial, onClose, known }: { trial: { p: Preset; items: AssetLite[] } | null; onClose: () => void; known: (s: string) => boolean | undefined }) {
  const [pick, setPick] = useState<string>("");
  // 退场时还显示原来的内容
  const last = useRef(trial);
  if (trial) last.current = trial;
  const shown = last.current;
  const [shownFor, setShownFor] = useState<Preset | null>(null);
  if (trial && trial.p !== shownFor) {
    setShownFor(trial.p);
    setPick(String(trial.items[0]?.id ?? ""));
  }
  const p = shown?.p;
  const notes = p
    ? [
        p.steps.some(isDownload) ? t("下载这几步会跳过，直接用选中的文件。") : "",
        p.steps.some(isMove) ? t("「移动到…」会复制一份过去，不会搬走原文件。") : "",
        p.steps.some((s) => known(s) === false) ? t("看不懂的步骤会跳过。") : "",
      ].filter(Boolean)
    : [];
  return (
    <Dialog
      className="cp"
      size="md"
      open={!!trial}
      onOpenChange={(o) => !o && onClose()}
      title={p ? t("试运行「{name}」", { name: p.name }) : t("试运行")}
      description={t("拿素材库里的一个文件真的跑一遍。新生成的文件会放进素材库，原文件不会被移动或改动。")}
      closeLabel={t("关闭")}
      footer={
        <>
          <Button variant="secondary" className="cp-pill" onClick={onClose}>
            {t("取消")}
          </Button>
          <Button
            className="cp-pill"
            icon={<Icon icon={PlayIcon} size={14} />}
            disabled={!pick}
            onClick={() => {
              const a = shown?.items.find((x) => String(x.id) === pick);
              if (p && a) startTrial(p, a);
              onClose();
            }}
          >
            {t("开始试运行")}
          </Button>
        </>
      }
    >
      <div className="cp-editor">
        <Select
          label={t("用哪个文件")}
          placeholder={t("选一个素材")}
          value={pick}
          onChange={setPick}
          options={(shown?.items ?? []).map((a) => ({ value: String(a.id), label: a.title, description: [a.fmt || a.type, a.dur, a.when].filter(Boolean).join(" · ") }))}
        />
        {notes.length > 0 && <p className="cp-editor-note">{joinSentences(notes)}</p>}
      </div>
    </Dialog>
  );
}
