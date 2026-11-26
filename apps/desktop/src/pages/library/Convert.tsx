// 处理 · 转换：原格式 → 目标格式两张纸张图标（设计稿），中间那条线转换时变成进度条；
// 下面选「转成」，视频转视频时还能选分辨率和画质；写明存到哪；「开始转换」。结果（新文件）自动进素材库。
// 后端命令 task_convert，进度走 task 事件（kind = convert）。
import { ArrowRight01Icon } from "@hugeicons/core-free-icons";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { t, tx } from "@/i18n";
import { home } from "@/lib/format";
import { getPref, setPref } from "@/lib/prefs";
import { FileIcon, Icon } from "@/parts";
import { clearOp, convert, showAsset, useOp } from "./act";
import { StopButton } from "./Steps";
import { defaultFmt, fileIconFmt, fmtOpts, isAudioFmt, isVideo, outPath, qualityOpts, resOpts, sameFormat, type Asset, type NewFile } from "./model";

export function Convert({ a }: { a: Asset }) {
  const opts = fmtOpts(a);
  // 上次选的格式按素材种类记住（转视频的人多半每次都转成同一种）
  const [fmt, setFmtState] = useState(() => {
    const f = getPref(`lfmt:${a.type}`, "");
    return opts.some((o) => o.value === f) && !sameFormat(a.fmt, f) ? f : defaultFmt(a);
  });
  const [res, setRes] = useState("");
  const [q, setQ] = useState("");
  const op = useOp(a.id, "convert");
  const busy = op?.st === "run" || op?.st === "queued";
  // 做完的那一次只在还选着同一个格式时显示；换了格式就是一次新的转换
  const shown = op && (busy || op.fmt === fmt) ? op : undefined;
  // 正在转的（可能是 AI 助手发起的）按它自己的目标格式画
  const target = busy && shown?.fmt ? shown.fmt : fmt;
  const audioOut = isVideo(a) && isAudioFmt(fmt);

  const setFmt = (f: string) => {
    setFmtState(f);
    setPref(`lfmt:${a.type}`, f);
    if (op && !busy) clearOp(a.id, "convert");
  };

  const cap =
    shown?.st === "queued" ? (
      t("排队等着转成{fmt}…", { fmt: target })
    ) : shown?.st === "run" ? (
      tx("正在转成{fmt} <b>{pct}%</b>", { fmt: target, pct: Math.round(shown.p), b: (s) => <b className="num">{s}</b> })
    ) : shown?.st === "ok" ? (
      <>
        {t("已转成{fmt}，放进素材库了", { fmt })}
        {(shown.result as NewFile | undefined)?.assetId != null && (
          <button className="cp-link" onClick={() => showAsset((shown.result as NewFile).assetId)}>
            {t("查看")}
          </button>
        )}
      </>
    ) : shown?.st === "err" ? (
      <span className="lib-err">{shown.err || t("没有转换成功")}</span>
    ) : sameFormat(a.fmt, fmt) ? (
      t("已经是{fmt}，会另存一份", { fmt: a.fmt })
    ) : audioOut ? (
      t("只把声音存成{fmt}", { fmt })
    ) : (
      // 箭头右边写目标格式本身（选项上的「MP4（H.264）」括号里的不写）；到这一支时 fmt 不是「只要声音」那几项
      `${a.fmt} → ${fmt}`
    );

  return (
    <div className="lib-tool">
      <div className="cp-convert lib-xfer" data-st={shown?.st}>
        <FileIcon fmt={a.fmt || "FILE"} size="lg" />
        <span className="lib-xline" aria-hidden>
          <Icon icon={ArrowRight01Icon} size={16} className="arrow" />
          <span className="bar">
            <i style={{ clipPath: `inset(0 ${100 - (shown?.st === "ok" ? 100 : (shown?.p ?? 0))}% 0 0 round 99px)` }} />
          </span>
        </span>
        <FileIcon fmt={fileIconFmt(target)} size="lg" />
        <p className="lib-cap" aria-live="polite">
          {cap}
        </p>
      </div>
      <Select size="sm" label={t("转成")} value={fmt} disabled={busy} onChange={(v) => setFmt(v)} options={opts} />
      {isVideo(a) && !audioOut && (
        <div className="lib-two">
          <Select size="sm" label={t("分辨率")} value={res} disabled={busy} onChange={setRes} options={resOpts()} />
          <Select size="sm" label={t("画质")} value={q} disabled={busy} onChange={setQ} options={qualityOpts()} />
        </div>
      )}
      <p className="lib-out">{tx("存到 {path}", { path: <span className="mono">{home(outPath(a, fmt))}</span> })}</p>
      <div className="lib-run">
        <StopButton a={a} k="convert" op={op} />
        <Button size="sm" className="cp-pill" loading={busy} onClick={() => !busy && convert(a, fmt, res ? Number(res) : null, q || null)}>
          {shown?.st === "queued" ? t("排队中") : busy ? t("正在转换") : shown?.st === "ok" ? t("再转一份") : t("开始转换")}
        </Button>
      </div>
    </div>
  );
}
