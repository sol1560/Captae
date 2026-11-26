// 处理 · 字幕：先看网站上有没有自带的字幕（subtitle_tracks），有就直接下载；没有就用语音转文字（本地识别，不上传）。
// 语音识别还没装时，在这里就能装（lib/whisper.ts，和设置、上手引导共用同一份状态和进度）。
// 选项：语言、导出格式（SRT / VTT / TXT）、翻译成界面语言（要先在设置里填翻译服务）、把字幕嵌进画面（另存一份新视频）。
// 后端命令 task_subtitle，进度走 task 事件（kind = subtitle），画成步骤时间线；做完显示前几句。
import { Settings02Icon } from "@hugeicons/core-free-icons";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { Select } from "@/components/ui/select";
import { ShimmerText } from "@/components/ui/shimmer-loader";
import { Switch } from "@/components/ui/switch";
import { lang as uiLang, parseLang, t } from "@/i18n";
import { etaText } from "@/lib/format";
import { useDelayed } from "@/lib/hooks";
import { getPref, setPref } from "@/lib/prefs";
import { getState, loadDeps, useApp } from "@/lib/store";
import { installWhisper, modelInfo, useWhisper } from "@/lib/whisper";
import { Icon, ProgressBar } from "@/parts";
import { openDialog } from "@/shell/actions";
import { subtitle, useOp } from "./act";
import { useSubInfo } from "./data";
import { isVideo, SUB_FORMATS, subSteps, trackName, whisperLangs, type Asset, type SubMode, type SubResult, type Track, translateTarget } from "./model";
import { OpBlock, StopButton } from "./Steps";

/** 网站字幕默认选哪条：中文的人工字幕 > 其他人工字幕 > 第一条 */
function defaultTrack(L: Track[]): string {
  return (L.find((tk) => !tk.auto && /^zh/i.test(tk.code)) ?? L.find((tk) => !tk.auto) ?? L[0])?.code ?? "";
}

export function Subtitle({ a }: { a: Asset }) {
  const info = useSubInfo(a);
  const settings = useApp((s) => s.settings);
  const w = useWhisper();
  const op = useOp(a.id, "subtitle");
  const busy = op?.st === "run";

  const [modeChoice, setModeChoice] = useState<SubMode | null>(null);
  const [lang, setLang] = useState<Record<SubMode, string | null>>({ platform: null, whisper: "" });
  const [fmt, setFmtState] = useState<string>(() => getPref("subFmt", "SRT"));
  const [translate, setTranslate] = useState(false);
  const [burn, setBurn] = useState(false);

  // 语音识别装没装要看外部工具的状态；还没查过就查一次
  useEffect(() => {
    if (!getState().deps) loadDeps();
  }, []);
  // 等网站字幕：300ms 以内就有结果的不显示，超过了文字扫光（配方 m）
  const slow = useDelayed(!info || info === "loading" || info === "err", 300);

  if (!info || info === "loading" || info === "err")
    return (
      <div className="lib-tool">
        <p className="lib-wait">{slow && <ShimmerText className="cp-wait">{t("正在看网站上有没有字幕…")}</ShimmerText>}</p>
      </div>
    );

  const tracks = info.tracks ?? [];
  const hasPlat = tracks.length > 0;
  // 后端的检查结果缓存一分钟；刚装好的那一分钟里按「装好了」处理
  const whisperOk = info.hasAudio && (info.whisper || w.state === "ready");
  if (!hasPlat && !info.hasAudio)
    return (
      <div className="lib-tool">
        <div className="lib-empty">
          <b>{t("没有可用的字幕")}</b>
          <span>{t("网站没有提供字幕，文件里也没有声音")}</span>
        </div>
      </div>
    );

  let mode: SubMode = modeChoice ?? (hasPlat ? "platform" : "whisper");
  if (mode === "platform" && !hasPlat) mode = "whisper";
  // 没有网站字幕、改用语音转文字时的说明：为什么没有 + 可以在本机识别，各写一整句
  const whisperHint = info.err
    ? t("没能读到网站字幕，可以在这台电脑上识别出来，不上传")
    : a.page || a.url
      ? t("这个网站没有提供字幕，可以在这台电脑上识别出来，不上传")
      : t("本地文件没有网站字幕，可以在这台电脑上识别出来，不上传");

  const langs: [string, string][] = mode === "platform" ? tracks.map((tk) => [tk.code, trackName(tk)]) : whisperLangs();
  const curLang = mode === "platform" ? (lang.platform ?? defaultTrack(tracks)) : (lang.whisper ?? "");
  // 步骤里写网站字幕那一条的名字（不带「（自动生成）」；旧版缓存的名字带着，去掉）
  const curTrack = tracks.find((tk) => tk.code === curLang) ?? tracks[0];
  const trackLabel = curTrack ? curTrack.name.replace(/（自动生成）$/, "") : "";
  const llm = !!(settings?.llmBaseUrl?.trim() && settings?.llmModel?.trim());
  // 字幕已经是界面语言（翻译的目标）就不用翻了。简体中文界面照旧：中文字幕（简体、繁体）都算
  const sameLang = !!curLang && (uiLang() === "zh-CN" ? /^zh/i.test(curLang) : parseLang(curLang) === uiLang());
  const canTranslate = llm && !sameLang;
  const doTranslate = translate && canTranslate;
  const doBurn = burn && isVideo(a) && info.canBurn;
  const result = op?.st === "ok" ? (op.result as SubResult | undefined) : undefined;

  const start = () => {
    if (busy) return;
    const steps = subSteps(mode, trackLabel, fmt, doTranslate, doBurn);
    subtitle(a, { source: mode, language: curLang || null, format: fmt.toLowerCase(), translate_to: doTranslate ? translateTarget() : null }, doBurn, steps);
  };

  return (
    <div className="lib-tool">
      <SegmentedControl
        aria-label={t("字幕从哪来")}
        size="sm"
        fullWidth
        value={mode}
        disabled={busy}
        onChange={(v) => setModeChoice(v as SubMode)}
        options={[
          { value: "platform", label: t("网站自带字幕"), disabled: !hasPlat },
          { value: "whisper", label: t("语音转文字"), disabled: !info.hasAudio },
        ]}
      />
      {mode === "platform" ? (
        <p className="lib-hint">{!info.hasAudio ? t("直接下载网站上的字幕，很快") : t("直接下载网站上的字幕，很快；想要更准的可以换成语音转文字")}</p>
      ) : (
        <p className="lib-hint">{hasPlat ? t("在这台电脑上识别，不上传；比网站字幕慢一些") : whisperHint}</p>
      )}

      {mode === "whisper" && !whisperOk ? (
        <WhisperSetup />
      ) : (
        <>
          <div className="lib-two">
            <Select
              size="sm"
              label={mode === "platform" ? t("字幕语言") : t("视频里说的语言")}
              value={curLang}
              disabled={busy}
              onChange={(v) => setLang((L) => ({ ...L, [mode]: v }))}
              options={langs.map(([v, l]) => ({ value: v, label: l }))}
            />
            <div className="lib-field">
              <span className="lib-flabel" id={`subfmt-${a.id}`}>
                {t("导出格式")}
              </span>
              <SegmentedControl
                aria-labelledby={`subfmt-${a.id}`}
                size="sm"
                fullWidth
                value={fmt}
                disabled={busy}
                onChange={(v) => {
                  setFmtState(v);
                  setPref("subFmt", v);
                }}
                options={SUB_FORMATS.map((f) => ({ value: f, label: f }))}
              />
            </div>
          </div>
          <div className="lib-switches">
            <Switch
              placement="end"
              size="sm"
              label={t("翻译成中文§字幕翻译成界面语言：每种语言都写「翻译成这种语言」（英文 Translate to English）")}
              description={
                !llm ? (
                  <>
                    {t("先在设置里填翻译服务")}{" "}
                    <button className="cp-link" onClick={() => openDialog("settings")}>
                      {t("去设置")}
                    </button>
                  </>
                ) : sameLang ? (
                  t("已经是中文了§字幕已经是界面语言，不用翻译：每种语言都写「已经是这种语言了」（英文 It's already in English）")
                ) : (
                  t("用你在设置里填的 AI 翻译")
                )
              }
              checked={doTranslate}
              disabled={!canTranslate || busy}
              onChange={(e) => setTranslate(e.target.checked)}
            />
            {isVideo(a) && (
              <Switch
                placement="end"
                size="sm"
                label={t("把字幕嵌进画面")}
                description={info.canBurn ? t("另存一份新视频，原文件不变") : t("现在的 ffmpeg 做不到，需要完整版")}
                checked={doBurn}
                disabled={!info.canBurn || busy}
                onChange={(e) => setBurn(e.target.checked)}
              />
            )}
          </div>
          <OpBlock op={op} />
          {result && <Preview r={result} />}
          <div className="lib-run">
            <StopButton a={a} k="subtitle" op={op} />
            <Button size="sm" className="cp-pill" loading={busy} onClick={start}>
              {busy ? (mode === "platform" ? t("正在下载") : t("正在识别")) : mode === "platform" ? t("下载字幕") : t("生成字幕")}
            </Button>
          </div>
        </>
      )}
    </div>
  );
}

/** 字幕的前几句（task_subtitle 的 preview：[时间, 文字]） */
function Preview({ r }: { r: SubResult }) {
  const L = r.preview ?? [];
  return (
    <div className="cp-log lib-srt" aria-label={t("字幕的前几句")}>
      {L.length ? (
        L.map(([time, x], i) => (
          <div key={i}>
            <time className="num">{time}</time>
            <span>{x}</span>
          </div>
        ))
      ) : (
        <div>
          <time />
          <span>{t("没有识别到说话的声音")}</span>
        </div>
      )}
      {r.cues > L.length && <p className="more">{t("一共{n}句", { n: r.cues })}</p>}
    </div>
  );
}

/** 语音识别还没装 / 模型没下载 / 正在装：在这里就能装（和设置里同一个按钮、同一份进度） */
function WhisperSetup() {
  const { state, model, task } = useWhisper();
  const m = modelInfo(model);
  const slow = useDelayed(state === "checking", 300);
  if (state === "checking") return <p className="lib-wait">{slow && <ShimmerText className="cp-wait">{t("正在检查语音识别装了没有…")}</ShimmerText>}</p>;
  if (state === "installing")
    return (
      <div className="lib-setup">
        <b>{t("正在安装语音转文字")}</b>
        <span>{task?.note || t("正在准备…")}</span>
        <div className="cp-tprog">
          <ProgressBar p={(task?.pct ?? 0) / 100} indeterminate={!task || task.pct <= 0} tick={900} label={t("安装语音识别")} />
          <span className="num">{etaText(task?.eta) || `${Math.round(task?.pct ?? 0)}%`}</span>
        </div>
      </div>
    );
  // m.label、m.model、m.full 是 lib/whisper.ts 按当前语言给的（「均衡」「约 480 MB」）
  return (
    <div className="lib-setup">
      <b>{state === "need_model" ? t("还差「{model}」识别模型", { model: m.label }) : t("先装一下语音转文字")}</b>
      <span>{state === "need_model" ? t("要下载{size}，装好以后没有字幕的视频也能生成字幕", { size: m.model }) : t("在这台电脑上识别，不上传。第一次要下载{size}", { size: m.full })}</span>
      <div className="acts">
        <Button size="sm" className="cp-pill" onClick={installWhisper}>
          {state === "need_model" ? t("下载模型") : t("安装")}
        </Button>
        <Button size="sm" variant="ghost" icon={<Icon icon={Settings02Icon} size={14} />} onClick={() => openDialog("settings")}>
          {t("选别的模型")}
        </Button>
      </div>
    </div>
  );
}
