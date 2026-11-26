// 处理 · 剪辑：视频显示胶片条（library_frames 均匀取的 10 帧），音频显示真实波形（library_waveform）；
// 两个把手拖出要的那一段（原生 range，跟手 1:1、方向键也能调，配方 j3），选区外压暗。
// 视频还能选：精确剪切、去掉声音、换一段声音（素材库里的音乐，或电脑里的文件 pick_audio_file）。
// 后端命令 task_trim，进度走 task 事件（kind = trim），画成步骤时间线；新片段自动进素材库。
import { ScissorIcon } from "@hugeicons/core-free-icons";
import { memo, useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { t } from "@/i18n";
import { errText, fileSrc, invoke } from "@/lib/bridge";
import { notify } from "@/lib/notify";
import { onEvent } from "@/lib/store";
import { Icon } from "@/parts";
import { trim, useOp } from "./act";
import { useFrames } from "./data";
import { Wave } from "./Grid";
import { defaultRange, fileName, isVideo, secs, tc, type Asset, type LibPage } from "./model";
import { OpBlock, StopButton } from "./Steps";

/** 每条素材选过的片段（换素材再回来还在，和旧界面一样） */
const ranges = new Map<number, [number, number]>();

type Ra = { path: string; label: string } | null;

export function Trim({ a }: { a: Asset }) {
  const d = a.durS ?? 0;
  const video = isVideo(a);
  const [[s, e], setRangeState] = useState<[number, number]>(() => ranges.get(a.id) ?? defaultRange(d));
  const [precise, setPrecise] = useState(false);
  const [mute, setMute] = useState(false);
  const [ra, setRa] = useState<Ra>(null);
  const audioLib = useAudioLib(video);
  const op = useOp(a.id, "trim");
  const busy = op?.st === "run";

  const setRange = (r: [number, number]) => {
    ranges.set(a.id, r);
    setRangeState(r);
  };

  if (!(d > 0))
    return (
      <div className="lib-tool">
        <div className="lib-empty">
          <b>{t("不知道这个文件有多长")}</b>
          <span>{t("没法选片段，可以先「转换」成 MP4 再试")}</span>
        </div>
      </div>
    );

  const len = Math.max(0, e - s);
  const pct = (x: number) => (x / d) * 100;

  const pick = (v: string) => {
    if (v === "pick") {
      invoke<string | null>("pick_audio_file")
        .then((p) => p && setRa({ path: p, label: fileName(p) }))
        .catch((err) => notify(t("没有选上"), errText(err)));
      return;
    }
    if (!v) return setRa(null);
    const x = audioLib.find((y) => y.path === v);
    setRa({ path: v, label: x?.title ?? fileName(v) });
  };
  const raOpts = [
    { value: "", label: t("不换") },
    ...audioLib.map((x) => ({ value: x.path, label: x.title, description: t("素材库里的") })),
    ...(ra && !audioLib.some((x) => x.path === ra.path) ? [{ value: ra.path, label: ra.label, description: t("电脑里的") }] : []),
    { value: "pick", label: t("从电脑里选…") },
  ];

  const start = () => {
    if (busy) return;
    if (len < 0.1) {
      notify(t("片段太短了"), t("拖动两边的把手选一段"));
      return;
    }
    const r = video ? ra : null;
    // 去掉声音和换声音不会同时有（换了声音时「去掉声音」不起作用）
    const m = video && mute && !r;
    const steps = [precise && video ? t("精确截取片段") : t("截取片段"), ...(r ? [t("换上「{name}」", { name: r.label })] : []), m ? t("保存新片段（去掉声音）") : t("保存新片段"), t("放进素材库")];
    const range = `${tc(s)} – ${tc(e)}`;
    trim(a, { start: s, end: e, mute: m, precise: video && precise, replace_audio: r?.path ?? null }, steps, m ? t("{range} · 去掉了声音", { range }) : r ? t("{range} · 换了声音", { range }) : range);
  };

  return (
    <div className="lib-tool">
      <div className="lib-strip" data-kind={a.type}>
        <div className="lib-strip-in">{video ? <Frames a={a} /> : <Wave a={a} bars={64} className="lib-wave lib-strip-wave" />}</div>
        {/* 选区外压暗：两块盖板只动 scaleX（composite），选区框用百分比定位 */}
        <i className="lib-dim l" style={{ transform: `scaleX(${pct(s) / 100})` }} />
        <i className="lib-dim r" style={{ transform: `scaleX(${(100 - pct(e)) / 100})` }} />
        <i className="lib-sel" style={{ left: `${pct(s)}%`, width: `${Math.max(0.5, pct(len))}%` }} />
        <input
          type="range"
          className="lib-handle"
          aria-label={t("起点")}
          aria-valuetext={tc(s)}
          min={0}
          max={d}
          step={0.1}
          value={s}
          disabled={busy}
          onChange={(ev) => setRange([Math.min(Number(ev.target.value), e - 0.1), e])}
        />
        <input
          type="range"
          className="lib-handle"
          aria-label={t("终点")}
          aria-valuetext={tc(e)}
          min={0}
          max={d}
          step={0.1}
          value={e}
          disabled={busy}
          onChange={(ev) => setRange([s, Math.max(Number(ev.target.value), s + 0.1)])}
        />
      </div>
      <div className="lib-times">
        <div>
          <span className="lib-flabel">{t("起点")}</span>
          <span className="lib-tbox num">{tc(s)}</span>
        </div>
        <div>
          <span className="lib-flabel">{t("终点")}</span>
          <span className="lib-tbox num">{tc(e)}</span>
        </div>
      </div>
      {video && (
        <div className="lib-switches">
          <Switch
            placement="end"
            size="sm"
            label={t("精确剪切")}
            description={precise ? t("从选中的那一帧开始，会慢一些") : t("从最近的关键帧开始，很快")}
            checked={precise}
            disabled={busy}
            onChange={(ev) => setPrecise(ev.target.checked)}
          />
          <Switch
            placement="end"
            size="sm"
            label={t("去掉声音")}
            description={ra ? t("换了声音时用新的声音") : undefined}
            checked={mute && !ra}
            disabled={!!ra || busy}
            onChange={(ev) => setMute(ev.target.checked)}
          />
          <Select
            size="sm"
            label={t("换一段声音")}
            hint={ra ? t("新声音从头开始，不够长会补上静音") : t("用素材库里的音乐，或电脑里的文件")}
            value={ra?.path ?? ""}
            disabled={busy}
            onChange={pick}
            options={raOpts}
          />
        </div>
      )}
      <OpBlock op={op} />
      <div className="lib-run">
        <StopButton a={a} k="trim" op={op} />
        <Button size="sm" className="cp-pill" icon={<Icon icon={ScissorIcon} size={14} />} loading={busy} onClick={start}>
          {busy ? t("正在导出") : t("导出片段 · {time}", { time: secs(len) })}
        </Button>
      </div>
    </div>
  );
}

/** 胶片条：帧到之前先用封面铺满，帧到了淡入（0.3s，配方 j3）；没取到就一直是封面 */
const Frames = memo(function Frames({ a }: { a: Asset }) {
  const f = useFrames(a, 10);
  const L = Array.isArray(f) ? f : null;
  const cover = fileSrc(a.thumb);
  return (
    <>
      {cover && <span className="lib-strip-cover" style={{ backgroundImage: `url("${cover}")` }} />}
      {L && <Strip key={a.id} id={a.id} L={L} />}
    </>
  );
});

/** 这次打开以来已经淡入过胶片条的素材：帧有缓存，换个处理再切回「剪辑」、换回这条素材时直接显示 */
const shownStrips = new Set<number>();

/** 帧真的刚取到时才从封面淡入；淡入过的不再每次挂上都重播（功能性的图不重播，课程 remove-animations）。
 *  要不要淡入在挂上的那一刻定下来，之后不变：中途改掉会把正在播的淡入一下子截断 */
function Strip({ id, L }: { id: number; L: string[] }) {
  const [fade] = useState(() => !shownStrips.has(id));
  useEffect(() => {
    shownStrips.add(id);
  }, [id]);
  return (
    <span className="lib-frames" data-fade={fade || undefined}>
      {L.map((p, i) => (
        <img key={i} src={fileSrc(p)} alt="" draggable={false} />
      ))}
    </span>
  );
}

/** 「换一段声音」能选的：素材库里还在的音频（最多 200 条），素材库变了重新要 */
function useAudioLib(on: boolean): { path: string; title: string }[] {
  const [L, setL] = useState<{ path: string; title: string }[]>([]);
  useEffect(() => {
    if (!on) return;
    let live = true;
    const load = () =>
      invoke<LibPage>("library_query", { q: { kind: "audio", limit: 200 } })
        .then((r) => live && setL(r.items.filter((x) => x.exists).map((x) => ({ path: x.path, title: x.title }))))
        .catch(() => undefined);
    load();
    const off = onEvent("library", load);
    return () => {
      live = false;
      off();
    };
  }, [on]);
  return L;
}
