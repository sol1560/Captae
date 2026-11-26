// 功能卡片：九个功能各一张卡片，上半是剧照上浮着的界面片段，下半是标题和说明，画面直接摆成演示完的样子。
// 宽屏用的是 stack.tsx（一串真实窗口，鼠标在里面点一遍）；这一列卡片给窄屏、矮屏和开了「减少动态效果」的时候用（TourList）。
// （2026-11-24 到 25 这里是一个钉住一屏、箭头带着卡片走的「导览」，换成 stack.tsx 以后删掉了，git 历史里没有，备份在维护者本机 work/site/stack/tour.tsx.before。）
// 画面里的按钮、状态用 App 自己的词表（t），和产品界面一字不差；其余文案在 copy.ts。
import { useEffect, useState, type CSSProperties, type ReactNode } from "react";
import { AiBrain01Icon, AiTranscribeAudioIcon, Alert02Icon, ArrowDown01Icon, CheckmarkCircle02Icon, ClipboardCheckIcon, LockIcon, Search01Icon, StopCircleIcon, Tick02Icon } from "@hugeicons/core-free-icons";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { CaptaeLogo, Icon, ProgressBar, SiteIcon } from "@/parts";
import { copy } from "./copy";
import { t } from "@/i18n";
import { shotOf } from "./shots";
import bbb from "@/dev/img/bbb-frank.jpg";
import edEmo from "@/dev/img/ed-emo-proog.jpg";
import edPlatform from "@/dev/img/ed-platform.jpg";
import sintel1 from "@/dev/img/sintel-1.jpg";
import sintel3 from "@/dev/img/sintel-3.jpg";
import tos1 from "@/dev/img/tos-01.jpg";
import tos5 from "@/dev/img/tos-05.jpg";

/* ---------------- 每一站的画面 ----------------
 * on = 演示开始了；live = 正在看这一张；instant = 直接摆出演示完的样子（TourList 一直是 instant）。 */

type ArtProps = { on: boolean; live: boolean; instant: boolean };

/** 一步步出来的东西（打字、字幕一行行出来）：on 了以后按 gap 的间隔从 0 数到 total；instant 时直接是 total */
function useCount(on: boolean, instant: boolean, total: number, gap: (k: number) => number, delay = 260) {
  const [n, setN] = useState(instant ? total : 0);
  useEffect(() => {
    if (instant) {
      setN(total);
      return;
    }
    if (!on) return;
    let k = 0;
    let id = 0;
    const step = () => {
      k += 1;
      setN(k);
      if (k < total) id = window.setTimeout(step, gap(k));
    };
    id = window.setTimeout(step, delay);
    return () => window.clearTimeout(id);
    // gap 每次渲染都是新函数，不放进依赖
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [on, instant, total]);
  return n;
}

/** on 以后过 ms 毫秒才变 true（先看到按下去，再看到结果） */
function useAfter(on: boolean, instant: boolean, ms: number) {
  const [v, setV] = useState(instant);
  useEffect(() => {
    if (instant) return setV(true);
    if (!on) return;
    const id = window.setTimeout(() => setV(true), ms);
    return () => window.clearTimeout(id);
  }, [on, instant, ms]);
  return v;
}

const QUALITIES = [
  { value: "2160p", size: "1.21 GB" },
  { value: "1080p", size: "312 MB" },
  { value: "720p", size: "158 MB" },
];

/** 挑清晰度：箭头点 1080p，开始下载 */
function PickArt({ on, instant }: ArtProps) {
  const [q, setQ] = useState(instant ? "1080p" : "2160p");
  const started = useAfter(on, instant, 380);
  useEffect(() => {
    if (on) setQ("1080p");
  }, [on]);
  return (
    <>
      <img className="site-art-bg" src={sintel3} alt="" style={{ objectPosition: "50% 40%" }} />
      <div className="site-frag tour-pick">
        <b>Sintel</b>
        <span className="meta">
          <SiteIcon site="bilibili" size={12} /> {t("哔哩哔哩")} · 14:48
        </span>
        <SegmentedControl
          size="sm"
          aria-label={copy.a11y.qualityDemo}
          value={q}
          onChange={setQ}
          options={QUALITIES.map((x) => ({ value: x.value, label: x.value === "1080p" ? <span data-cue="">1080p</span> : x.value, count: x.size }))}
        />
        <div className="tour-pick-run">
          <ProgressBar p={started ? 0.38 : 0} tick={1600} label={t("下载进度")} />
          <span className="num">{started ? `${t("正在下载")} · ${q} · 312 MB` : t("{n}秒后自动用{quality}", { n: 3, quality: q })}</span>
        </div>
      </div>
    </>
  );
}

/** 要登录的也能下：箭头点「完成」，下面出来设置里「已登录的网站」那一行（真实界面的说法） */
function LoginArt({ on }: ArtProps) {
  return (
    <>
      <img className="site-art-bg" src={edEmo} alt="" />
      <div className="tour-login" data-on={on || undefined}>
        <div className="site-frag bar">
          <Icon icon={LockIcon} size={14} />
          <span className="host mono">passport.bilibili.com</span>
          <span className="say">{t("登录好以后点「完成」")}</span>
          <span className="done" data-cue="">
            {t("完成")}
          </span>
        </div>
        <div className="site-frag row">
          <SiteIcon site="bilibili" size={16} />
          <span className="who">
            <b>{t("哔哩哔哩")}</b>
            <small>{copy.ui.biliGain}</small>
          </span>
          <span className="ok">
            <Icon icon={CheckmarkCircle02Icon} size={13} />
            {t("已登录")}
          </span>
        </div>
      </div>
    </>
  );
}

/** 说清楚为什么：箭头点第一行的「登录哔哩哔哩」，那一行变成接着下载 */
function ReasonsArt({ on, instant }: ArtProps) {
  const fixed = useAfter(on, instant, 200);
  return (
    <>
      <img className="site-art-bg" src={tos1} alt="" />
      <ul className="site-reasons tour-reasons">
        {copy.reasons.examples.map((r, k) => {
          const done = fixed && k === 0;
          return (
            <li key={r.text} data-tone={done ? "ok" : r.tone}>
              <Icon icon={done ? CheckmarkCircle02Icon : Alert02Icon} size={14} />
              <span>{done ? copy.reasons.fixed : r.text}</span>
              {r.action && !done && (
                <span className="act" data-cue={k === 0 ? "" : undefined}>
                  {r.action}
                </span>
              )}
            </li>
          );
        })}
      </ul>
    </>
  );
}

const WORD = "dragon";
const THUMBS: { src: string; at?: string }[] = [
  { src: bbb },
  { src: sintel3, at: "50% 40%" },
  { src: tos1 },
  { src: edEmo },
  { src: sintel1, at: "40% 40%" }, // 正中间这张是搜到的那条
  { src: tos5, at: "30% 40%" },
  { src: edPlatform },
  { src: bbb, at: "20% 50%" },
  { src: sintel3, at: "80% 50%" },
];
/** 素材库：箭头点搜索框，打进「dragon」，搜到字幕里的那一句 */
function LibraryArt({ on, instant }: ArtProps) {
  // 打字的速度照 Copilot 片子结尾实测：约每秒 20 个字，每个字 1 到 2 帧，不均匀
  const n = useCount(on, instant, WORD.length, () => 33 + Math.random() * 34, 300);
  const hit = n >= WORD.length;
  return (
    <>
      <div className="site-art-grid tour-grid" data-hit={hit || undefined}>
        {THUMBS.map((t, i) => (
          <img key={i} src={t.src} alt="" style={{ objectPosition: t.at }} data-match={i === 4 || undefined} />
        ))}
      </div>
      <div className="site-frag site-art-search tour-search" data-cue="">
        <Icon icon={Search01Icon} size={14} />
        {n > 0 ? <span className="q">{WORD.slice(0, n)}</span> : <span className="ph">{t("搜标题、标签、字幕里的话")}</span>}
        {hit && (
          <span className="hit">
            <b>Sintel</b> · {t("字幕")} 02:26 A <mark>dragon</mark>.
          </span>
        )}
      </div>
    </>
  );
}

/** Sintel 的对白（Blender 基金会的开放电影，CC BY 3.0，出处写在页脚） */
const SUBS: [string, string][] = [
  ["01:47", "This blade has a dark past."],
  ["01:51", "It has shed much innocent blood."],
  ["01:56", "You're a fool for traveling alone, so completely unprepared."],
  ["02:02", "You're lucky your blood's still flowing."],
  ["02:08", "Thank you."],
  ["02:14", "What brings you to the land of the gatekeepers?"],
  ["02:19", "I'm searching for someone."],
  ["02:26", "A dragon."],
];
const SUB_GAPS = [520, 700, 560, 820, 460, 760, 600, 680];

/** 转文字：箭头点「转文字」，字幕在本机一行行识别出来；贴着底部，新的一行加在最下面、整列往上顶（行与行之间不做缓动，新行只淡入） */
function SpeechArt({ on, instant }: ArtProps) {
  const n = useCount(on, instant, SUBS.length, (k) => SUB_GAPS[k % SUB_GAPS.length]!, 380);
  const done = n >= SUBS.length;
  return (
    <>
      <img className="site-art-bg" src={sintel1} alt="" style={{ objectPosition: "40% 40%" }} />
      <div className="site-frag tour-subs">
        <div className="head">
          <span>{on || instant ? "Sintel.srt" : `Sintel · ${copy.speech.none}`}</span>
          {!on && !instant ? (
            <span className="tour-btn" data-cue="">
              <Icon icon={AiTranscribeAudioIcon} size={14} />
              {t("语音转文字")}
            </span>
          ) : done ? (
            <span className="ok">
              <Icon icon={Tick02Icon} size={13} /> {copy.speech.done}
            </span>
          ) : (
            <span className="num">
              {copy.speech.working} · {Math.round((n / SUBS.length) * 100)}%
            </span>
          )}
        </div>
        <div className="body">
          <div className="list">
            {SUBS.slice(0, n).map(([t, s]) => (
              <p key={t}>
                <span className="num mono">{t}</span>
                {s}
              </p>
            ))}
          </div>
        </div>
      </div>
    </>
  );
}

/** 录直播：认出是直播，箭头点「从现在开始录」，开始计时（红点静止，不闪） */
function LiveArt({ on, live, instant }: ArtProps) {
  const rec = useAfter(on, instant, 160);
  const [s, setS] = useState(instant ? 24 * 60 + 16 : 0);
  useEffect(() => {
    if (!rec || !live || instant) return;
    const id = window.setInterval(() => setS((v) => v + 1), 1000);
    return () => window.clearInterval(id);
  }, [rec, live, instant]);
  const mm = String(Math.floor(s / 60)).padStart(2, "0");
  const ss = String(s % 60).padStart(2, "0");
  return (
    <>
      <img className="site-art-bg" src={tos5} alt="" style={{ objectPosition: "35% 40%" }} />
      <div className="site-frag site-art-live tour-live" data-rec={rec || undefined}>
        <span className="dot" />
        {rec ? (
          <span className="num">{t("正在录制 · 已录 {time}", { time: `00:${mm}:${ss}` })}</span>
        ) : (
          <span>{copy.ui.live}</span>
        )}
        <span className="stop" data-cue="">
          {rec ? (
            <>
              <Icon icon={StopCircleIcon} size={14} /> {t("停止并保存")}
            </>
          ) : (
            t("从现在开始录")
          )}
        </span>
      </div>
    </>
  );
}

/** 浏览器插件：箭头点清晰度菜单里的 1080p，菜单收起，右下角的进度卡片上来 */
function ExtensionArt({ on, instant }: ArtProps) {
  const started = useAfter(on, instant, 440);
  const p = started ? 0.62 : 0;
  return (
    <div className="tour-browser" data-on={on || undefined}>
      <div className="site-browser-bar">
        <i />
        <i />
        <i />
        <span className="url mono">durian.blender.org/sintel</span>
      </div>
      <div className="site-browser-page">
        <img src={sintel1} alt="" />
        <div className="site-xbar">
          <span className="site-xbtn">
            <CaptaeLogo size={20} />
            {t("下载")}
            <Icon icon={ArrowDown01Icon} size={13} />
          </span>
          <span className="site-xbtn plain">{t("发给 AI")}</span>
        </div>
        <div className="site-frag site-xmenu">
          <div className="item hl" data-cue="">
            <span>
              1080p<small>MP4</small>
            </span>
            <span className="size">{t("约{size}", { size: "312 MB" })}</span>
          </div>
          <div className="item">
            <span>
              720p<small>MP4</small>
            </span>
            <span className="size">{t("约{size}", { size: "158 MB" })}</span>
          </div>
          <div className="item">
            <span>{t("只要声音 · {fmt}", { fmt: "M4A" })}</span>
            <span className="size">{t("约{size}", { size: "14 MB" })}</span>
          </div>
        </div>
        <div className="site-frag site-xcard">
          <img src={sintel1} alt="" />
          <div>
            <b>Sintel</b>
            <ProgressBar p={p} tick={1600} />
            <span className="num">
              {Math.round(p * 100)}% · {Math.round(312 * p)} MB / 312 MB
            </span>
          </div>
        </div>
        <img className="tour-popup" src={shotOf("popup")} alt="" />
      </div>
    </div>
  );
}


/** AI 助手：箭头点 Claude Desktop 那一行的「一键写入」，连上以后调用记录一条条出来（只淡入，不位移，间隔手挑；配方 k） */
function AiArt({ on, instant }: ArtProps) {
  const linked = useAfter(on, instant, 160);
  return (
    <>
      <img className="site-art-bg" src={edPlatform} alt="" />
      <div className="site-frag tour-ai-link" data-on={linked || undefined}>
        <span className="dot" />
        <b>Claude Desktop</b>
        <span className="state">{linked ? t("已连接") : t("未连接")}</span>
        {!linked && (
          <span className="tour-btn" data-cue="">
            {t("一键写入")}
          </span>
        )}
      </div>
      <div className="site-log tour-log" data-seen={linked || undefined}>
        <div className="site-log-head">
          <span>{t("AI 最近做了什么")}</span>
        </div>
        {copy.ai.log.map((c, i) => (
          <div className="site-log-row" key={c.did} style={{ "--d": `${[250, 360, 440, 550][i]}ms` } as CSSProperties}>
            <span className="what">{c.did}</span>
            <span className="ok">
              <Icon icon={CheckmarkCircle02Icon} size={13} />
              {c.result}
            </span>
          </div>
        ))}
      </div>
    </>
  );
}

/** 发给 AI：三张关键画面和文字稿，箭头点「发给 AI」，出来「已经整理好，复制到剪贴板」（桌面端上手引导里的原话） */
function SendArt({ on, instant }: ArtProps) {
  const done = useAfter(on, instant, 220);
  return (
    <div className="tour-send" data-done={done || undefined}>
      <div className="frames">
        <img src={bbb} alt="" />
        <img src={edPlatform} alt="" />
        <img src={tos1} alt="" />
        <div className="script">
          <b>{copy.send.script}</b>
          {SUBS.slice(0, 3).map(([t, s]) => (
            <span key={t}>
              <i className="num mono">{t}</i> {s}
            </span>
          ))}
        </div>
      </div>
      <span className="site-frag btn" data-cue="">
        <Icon icon={AiBrain01Icon} size={14} />
        {t("发给 AI")}
      </span>
      <span className="clip">
        <Icon icon={ClipboardCheckIcon} size={15} />
        {t("已经整理好，复制到剪贴板")}
      </span>
    </div>
  );
}

/* ---------------- 每一站文字栏下面多出来的东西 ---------------- */

function Points({ items }: { items: string[] }) {
  return (
    <ul className="site-points small">
      {items.map((x) => (
        <li key={x}>
          <Icon icon={Tick02Icon} size={14} />
          {x}
        </li>
      ))}
    </ul>
  );
}

function LibraryExtra() {
  return (
    <ul className="site-chips">
      {copy.library.chips.map((c) => (
        <li key={c}>{c}</li>
      ))}
    </ul>
  );
}

function ExtensionExtra() {
  return <Points items={copy.extension.points} />;
}

function AiExtra() {
  return <Points items={copy.ai.points} />;
}

/* ---------------- 站 ---------------- */

type Scene = { id: string; anchor?: string; kicker: string; title: string; body: string; Art: (p: ArtProps) => ReactNode; Extra?: () => ReactNode };

/** 每一站；渲染时才读文案，所以写成函数（模块加载时还没定语言） */
const scenes = (): Scene[] => [
  { id: "pick", kicker: copy.pick.kicker, title: copy.pick.title, body: copy.pick.body, Art: PickArt },
  { id: "login", kicker: copy.login.kicker, title: copy.login.title, body: copy.login.body, Art: LoginArt },
  { id: "reasons", kicker: copy.reasons.kicker, title: copy.reasons.title, body: copy.reasons.body, Art: ReasonsArt },
  { id: "library", kicker: copy.library.kicker, title: copy.library.title, body: copy.library.body, Art: LibraryArt, Extra: LibraryExtra },
  { id: "speech", kicker: copy.speech.kicker, title: copy.speech.title, body: copy.speech.body, Art: SpeechArt },
  { id: "live", kicker: copy.live.kicker, title: copy.live.title, body: copy.live.body, Art: LiveArt },
  { id: "extension", anchor: "extension", kicker: copy.extension.kicker, title: copy.extension.title, body: copy.extension.body, Art: ExtensionArt, Extra: ExtensionExtra },
  { id: "ai", anchor: "ai", kicker: copy.ai.kicker, title: copy.ai.title, body: copy.ai.body, Art: AiArt, Extra: AiExtra },
  { id: "send", kicker: copy.send.kicker, title: copy.send.title, body: copy.send.body, Art: SendArt },
];

/* ---------------- 窄屏、减少动态效果：排成普通卡片 ---------------- */

/** 也给 stack.tsx 用：窄屏、减少动态效果时的那一列卡片 */
export function TourList() {
  return (
    <section className="site-section tour-list" id="features" aria-label={copy.nav.features}>
      {scenes().map((s) => (
        <article className="site-card tour-item" key={s.id} id={s.anchor} aria-labelledby={`tour-${s.id}`}>
          <div className="site-art tour-item-art" aria-hidden>
            <s.Art on live={false} instant />
          </div>
          <div className="site-card-text">
            <p className="site-kicker">{s.kicker}</p>
            <h3 id={`tour-${s.id}`}>{s.title}</h3>
            <p className="site-body">{s.body}</p>
            {s.Extra && <s.Extra />}
          </div>
        </article>
      ))}
    </section>
  );
}
