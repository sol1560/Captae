// 网格里的一张素材卡片：封面（视频和图片是缩略图，音频是真实波形）、时长、标题、来源 · 格式 · 时间；
// 文件不在了时封面变淡、第二行写「文件已经不在了」；在跑的处理（转换、字幕……）在封面底边画一条细进度。
// 动效（配方 j1）：悬停只让描边出现（opacity 150ms ease），不抬起不放大；按下 scale(.98)；
// 多选的圆圈悬停或键盘聚焦时淡入，勾上时勾从 .5 放大到 1（100ms，不从 0 开始）；选中高亮即时，不过渡。
import { memo, useCallback, useState, type KeyboardEvent, type MouseEvent } from "react";
import { Loading, Skeleton } from "@/components/ui/skeleton";
import { t } from "@/i18n";
import { fileSrc } from "@/lib/bridge";
import { FileIcon, SiteIcon } from "@/parts";
import { useWave } from "./data";
import { dayLabel, isAudio, isImage, isVideo, metaFmt, siteKey, siteName, type Asset } from "./model";

/* ---------------- 波形 ---------------- */

/** 声音的波形：每根条子满高，用 scaleY 画高低（只动 transform）。还没拿到峰值时是一排矮条子，
 *  第一次拿到时长到真实高度（300ms ease-out-quint）；拿过的直接画终态，不重播（配方 j3） */
export function Wave({ a, bars, className = "lib-wave" }: { a: Asset; bars: number; className?: string }) {
  const peaks = useWave(a.exists ? a : null, bars);
  return (
    <div className={className} data-ready={peaks ? "" : undefined} aria-hidden>
      {Array.from({ length: bars }, (_, i) => {
        const p = peaks ? peaks[Math.floor((i * peaks.length) / bars)] ?? 0 : 0.08;
        return <i key={i} style={{ transform: `scaleY(${Math.max(0.06, Math.min(1, p)).toFixed(3)})` }} />;
      })}
    </div>
  );
}

/* ---------------- 封面 ---------------- */

/** 这次打开以来已经读到过的缩略图地址：换筛选、换素材时重新挂上的直接显示 */
const shownThumbs = new Set<string>();

/** 缩略图读到以后淡入（0.3s ease-out，配方 j3）；读到过的直接显示，不闪。
 *  「读到过」在渲染时就判断：等挂上去以后再 setState，同一轮里别的组件量布局时浏览器已经按透明算过一次样式，
 *  过渡照样会播（实测换回「全部」时 12 张都又淡入了一次、换素材时详情预览图比卡片晚 160ms 才清楚） */
function Img({ src }: { src: string }) {
  const [on, setOn] = useState(() => shownThumbs.has(src));
  const show = useCallback(() => {
    shownThumbs.add(src);
    setOn(true);
  }, [src]);
  // 浏览器缓存里已经有、但没经过这里的：挂上去那一刻直接在元素上标好，不等 React 下一轮
  const ref = useCallback(
    (el: HTMLImageElement | null) => {
      if (el && !el.hasAttribute("data-on") && el.complete && el.naturalWidth) {
        el.setAttribute("data-on", "");
        show();
      }
    },
    [show],
  );
  return <img ref={ref} src={src} alt="" draggable={false} loading="lazy" decoding="async" data-on={on || undefined} onLoad={show} />;
}

/** 网格和详情共用的封面内容 */
export function Cover({ a, bars = 32 }: { a: Asset; bars?: number }) {
  if (isAudio(a)) return <Wave a={a} bars={bars} />;
  const src = fileSrc(a.thumb) || (isImage(a) && a.exists ? fileSrc(a.path) : "");
  if (src && (isVideo(a) || isImage(a))) return <Img key={src} src={src} />;
  return (
    <div className="lib-cover-file">
      <FileIcon fmt={a.fmt || "FILE"} size="lg" />
    </div>
  );
}

/* ---------------- 卡片 ---------------- */

const Check = () => (
  <svg viewBox="0 0 16 16" width="12" height="12" fill="none" aria-hidden>
    <path d="M3.5 8.4l2.9 2.9 6.1-6.6" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
  </svg>
);

export type TileProps = {
  a: Asset;
  selected: boolean;
  /** 多选的第几个（1 开始），0 = 没选 */
  pick: number;
  /** 选了不止一个：圆圈里写顺序（拼接就按这个顺序） */
  numbered: boolean;
  /** 在跑的处理的进度（0–100），null = 没有；-1 = 排队等着 */
  progress: number | null;
  /** 只有键盘焦点在网格里时，选中的那张才接 Tab（roving tabindex） */
  focusable: boolean;
  onPress: (a: Asset, e: MouseEvent) => void;
  onPick: (a: Asset) => void;
  onOpen: (a: Asset) => void;
  onKey: (a: Asset, e: KeyboardEvent<HTMLDivElement>) => void;
};

export const Tile = memo(function Tile({ a, selected, pick, numbered, progress, focusable, onPress, onPick, onOpen, onKey }: TileProps) {
  return (
    <div
      className="cp-tile lib-tile"
      role="option"
      aria-selected={selected}
      aria-label={a.title}
      data-id={a.id}
      data-gone={!a.exists || undefined}
      data-picked={pick > 0 || undefined}
      tabIndex={focusable ? 0 : -1}
      onClick={(e) => onPress(a, e)}
      onDoubleClick={() => a.exists && onOpen(a)}
      onKeyDown={(e) => onKey(a, e)}
    >
      <div className="cp-tile-media" data-kind={a.type}>
        <Cover a={a} />
        {a.dur && <span className="dur">{a.dur}</span>}
        <span
          className="lib-pick"
          data-on={pick > 0 || undefined}
          aria-hidden
          onClick={(e) => {
            e.stopPropagation();
            onPick(a);
          }}
        >
          {pick > 0 && (numbered ? <b className="num">{pick}</b> : <Check />)}
        </span>
        {progress != null && (
          <span className="lib-tprog" data-queued={progress < 0 || undefined}>
            <i style={{ clipPath: `inset(0 ${100 - Math.max(0, Math.min(100, progress))}% 0 0)` }} />
          </span>
        )}
      </div>
      <div className="cp-tile-title">{a.title}</div>
      <div className="cp-meta">
        {a.exists ? (
          <>
            <SiteIcon site={siteKey(a)} size={13} />
            <span className="u">{siteName(a)}</span>
            <span>·</span>
            <span className="num">{metaFmt(a)}</span>
            <span>·</span>
            <span className="t">{dayLabel(a.created_at, a.when)}</span>
          </>
        ) : (
          <span className="lib-gone">{t("文件已经不在了")}</span>
        )}
      </div>
    </div>
  );
});

/** 第一页还没回来：affectUI 的骨架屏，和网格同一个版式（Loading 晚一拍才淡入，快的时候根本不出现，配方 m6） */
export function GridSkeleton() {
  return (
    <Loading label={t("正在读取素材库")} className="cp-grid lib-grid lib-skel">
      {Array.from({ length: 9 }, (_, i) => (
        <div key={i} className="cp-tile lib-tile" aria-hidden>
          <Skeleton height="auto" radius={9} className="lib-skel-media" />
          <Skeleton line={22} height={11} width={`${62 + ((i * 17) % 30)}%`} className="lib-skel-t" />
          <Skeleton line={18} height={9} width={`${40 + ((i * 11) % 24)}%`} className="lib-skel-m" />
        </div>
      ))}
    </Loading>
  );
}
