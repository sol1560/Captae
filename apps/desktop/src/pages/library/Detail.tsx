// 右侧详情浮卡（和下载页同一张卡：照 m4 右上角「来源 / 用量」）：预览（能在这里直接播）、标题、简介、操作，
// 下面分节：信息（来源、格式、分辨率、时长、大小、时间、位置、标签）、处理（转换 / 字幕 / 剪辑）。
// 动效：换素材是高频操作，鼠标点只做 120ms 透明度交叉淡化，键盘上下键直接换、不动画（配方 e2）；分节展开收起用 Collapse。
// 拼接按第一段记：第一段的「处理」最上面是拼接的进度和「取消」（Steps.tsx ConcatOp）。
import {
  AiBrain01Icon,
  ArrowDown01Icon,
  Cancel01Icon,
  Delete02Icon,
  Download04Icon,
  Folder01Icon,
  Link01Icon,
  MoreHorizontalIcon,
  PlayIcon,
  PlusSignIcon,
  SquareArrowUpRight02Icon,
} from "@hugeicons/core-free-icons";
import { AnimatePresence, motion } from "motion/react";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { DropdownMenu } from "@/components/ui/dropdown-menu";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { Spinner } from "@/components/ui/spinner";
import { TextField } from "@/components/ui/text-field";
import { t } from "@/i18n";
import { fileSrc, invoke, native, platform } from "@/lib/bridge";
import { bare, home } from "@/lib/format";
import { getPref, setPref } from "@/lib/prefs";
import { Collapse, Icon, IconSwap, TipIcon } from "@/parts";
import { addTag, copyLink, moveToTrash, openFile, openSource, redownload, removeTag, REVEAL, revealFile, shareToAI, useOp } from "./act";
import { Convert } from "./Convert";
import { fetchAsset, knownAsset } from "./data";
import { Cover } from "./Grid";
import { ConcatOp } from "./Steps";
import { assetMenu, type Tool } from "./menu";
import { fullTime, isAudio, isImage, isMedia, isVideo, placeOf, sourceUrl, type Asset } from "./model";
import { Subtitle } from "./Subtitle";
import { Trim } from "./Trim";

const EASE = [0.23, 1, 0.32, 1] as const;

export type DetailProps = {
  a: Asset;
  /** 换素材是键盘触发的：直接换，不淡入淡出 */
  instant: boolean;
  tool: Tool;
  onTool: (t: Tool) => void;
  onAskTrash: (a: Asset) => void;
  onAskRemove: (a: Asset) => void;
  /** 点「原素材」：选中那一条 */
  onOpenAsset: (id: number) => void;
  /** 每变一次，「处理」那一节就打开一次（右键菜单里点了「字幕…」之类） */
  openTools?: number;
  /** 窄窗口的浮卡有关闭按钮 */
  onClose?: () => void;
};

/** 换素材的交叉淡化：鼠标点 120ms，键盘直接换。退场的那张拿不到新 props，
 *  所以「这次是不是键盘」要经 custom 同时传给 AnimatePresence 和卡片（配方 1.4 第 7 条，课 03/05） */
const SWAP = {
  hide: { opacity: 0 },
  show: (instant: boolean) => ({ opacity: 1, transition: { duration: instant ? 0 : 0.12, ease: EASE } }),
  gone: (instant: boolean) => ({ opacity: 0, transition: { duration: instant ? 0 : 0.12, ease: EASE } }),
};

export function Detail(p: DetailProps) {
  const { a } = p;
  return (
    <AnimatePresence mode="popLayout" initial={false} custom={p.instant}>
      <motion.div key={a.id} className="cp-dcard lib-dcard" custom={p.instant} variants={SWAP} initial="hide" animate="show" exit="gone">
        {p.onClose && (
          <div className="cp-dclose">
            <TipIcon size="sm" variant="secondary" icon={Cancel01Icon} label={t("关上详情")} onClick={p.onClose} />
          </div>
        )}
        <Preview a={a} />
        <div className="cp-dtitle lib-dtitle">{a.title}</div>
        {a.desc && <p className="cp-desc lib-desc">{a.desc}</p>}
        {!a.exists && (
          <div className="cp-note lib-missing" data-tone="err">
            <svg width="16" height="16" viewBox="0 0 18 18" fill="none" aria-hidden>
              <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5" />
              <path d="M9 5.6v4" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" />
              <circle cx="9" cy="12.3" r="1" fill="currentColor" />
            </svg>
            <div className="grow">
              {t("文件已经不在了")}
              {/* 路径以「~」开头不会自动空格，原文里的空格要留着 */}
              <small>{t("原来放在 {path}，可能被移走或删掉了", { path: home(a.path) })}</small>
            </div>
          </div>
        )}
        <Actions {...p} />

        <Section id="info" title={t("信息")}>
          <Info a={a} onOpenAsset={p.onOpenAsset} />
          <Tags a={a} />
        </Section>
        {a.exists && (isMedia(a) || isImage(a)) && (
          <Section id="tools" title={isImage(a) ? t("转换格式") : t("处理")} openSignal={p.openTools}>
            {isVideo(a) && <ConcatOp a={a} />}
            {isMedia(a) && (
              <SegmentedControl
                aria-label={t("处理")}
                size="sm"
                fullWidth
                className="lib-tools"
                value={p.tool}
                onChange={(v) => p.onTool(v as Tool)}
                options={[
                  { value: "convert", label: t("转换") },
                  { value: "subtitle", label: t("字幕") },
                  { value: "trim", label: t("剪辑") },
                ]}
              />
            )}
            {/* 换处理是点分段控件，一下子换，不做动画（高频、功能性，配方 j1「筛选不动」同理） */}
            {!isMedia(a) || p.tool === "convert" ? <Convert a={a} /> : p.tool === "subtitle" ? <Subtitle a={a} /> : <Trim a={a} />}
          </Section>
        )}
      </motion.div>
    </AnimatePresence>
  );
}

/* ---------------- 预览：能在这里直接播（本地文件用 fileSrc 转成 asset: 地址） ---------------- */

/** 点了播放以后多久还没有画面就算放不了。实测（Captae 窗口，2026-11-24）：本地 MP4、MP3 一秒内就开始；
 *  VP9 的 WebM 只读到尺寸就停住（readyState 1），不报错、play() 也不返回，靠这个退回「用默认程序打开」 */
const STALL_MS = 4000;

/** 播放用的地址。Linux 上 WebKitGTK 放不了 asset 协议的视频，要先问 Captae 要一个本机小服务的地址（media_server.rs）；
 *  别的系统直接用 fileSrc。要不到（文件不在放行范围里）时是空的，靠下面的「放不了」兜住 */
function useMediaSrc(path: string, want: boolean): string {
  const direct = platform !== "linux" || !native;
  const [src, setSrc] = useState("");
  useEffect(() => {
    if (direct || !want) return;
    let live = true;
    setSrc("");
    invoke<string>("media_src", { path })
      .then((u) => live && setSrc(u))
      .catch(() => undefined);
    return () => {
      live = false;
    };
  }, [direct, want, path]);
  return direct ? fileSrc(path) : src;
}

function Preview({ a }: { a: Asset }) {
  const [play, setPlay] = useState(false);
  const [bad, setBad] = useState(false);
  const media = useRef<HTMLMediaElement | null>(null);
  const keep = (el: HTMLMediaElement | null) => {
    media.current = el;
  };
  const canPlay = a.exists && (isVideo(a) || isAudio(a));
  const src = useMediaSrc(a.path, play);
  useEffect(() => {
    if (!play || bad) return;
    const timer = window.setTimeout(() => {
      const m = media.current;
      if (m && m.readyState < HTMLMediaElement.HAVE_CURRENT_DATA) setBad(true);
    }, STALL_MS);
    return () => window.clearTimeout(timer);
  }, [play, bad]);
  return (
    <div className="cp-preview lib-preview" data-kind={a.type} data-gone={!a.exists || undefined} data-playing={(play && !bad) || undefined}>
      {play && isVideo(a) && !bad ? (
        // 系统自带的播放控件：键盘、全屏、画中画都有；放不了（MKV 这类 WebKit 不认的报错，WebM 停住不动）就退回「用默认程序打开」
        <video ref={keep} src={src || undefined} controls autoPlay playsInline onError={() => setBad(true)} />
      ) : (
        <Cover a={a} bars={44} />
      )}
      {play && isAudio(a) && !bad && <audio ref={keep} className="lib-audio" src={src || undefined} controls autoPlay onError={() => setBad(true)} />}
      {canPlay && !play && (
        <button className="cp-play lib-play" aria-label={t("在这里播放")} title="" onClick={() => setPlay(true)}>
          <Icon icon={PlayIcon} size={15} />
        </button>
      )}
      {!play && a.dur && <span className="dur">{a.dur}</span>}
      {bad && (
        <div className="lib-unplayable">
          <span>{t("这个文件在这里放不了")}</span>
          <Button size="sm" variant="secondary" className="cp-pill" onClick={() => openFile(a)}>
            {t("用默认程序打开")}
          </Button>
        </div>
      )}
    </div>
  );
}

/* ---------------- 操作 ---------------- */

function Actions({ a, onAskTrash, onAskRemove, onTool }: DetailProps) {
  const url = sourceUrl(a);
  const more = (
    <DropdownMenu
      align="end"
      aria-label={t("素材的更多操作")}
      items={assetMenu(a, { askTrash: onAskTrash, askRemove: onAskRemove, tool: (_x, tl) => onTool(tl) })}
      trigger={({ triggerProps }) => <TipIcon size="sm" variant="secondary" icon={MoreHorizontalIcon} label={t("更多")} {...triggerProps} />}
    />
  );
  if (!a.exists)
    return (
      <div className="cp-dactions">
        {url && (
          <Button size="sm" className="cp-pill" icon={<Icon icon={Download04Icon} size={14} />} onClick={() => redownload(a)}>
            {t("重新下载")}
          </Button>
        )}
        {url && <TipIcon size="sm" variant="secondary" icon={Link01Icon} label={t("复制来源链接")} onClick={() => copyLink(a)} />}
        <span className="end">
          <Button variant="ghost" size="sm" icon={<Icon icon={Delete02Icon} size={14} />} onClick={() => onAskRemove(a)}>
            {t("从素材库移除")}
          </Button>
        </span>
      </div>
    );
  return (
    <div className="cp-dactions">
      <TipIcon size="sm" variant="secondary" icon={SquareArrowUpRight02Icon} label={t("用默认程序打开")} onClick={() => openFile(a)} />
      <TipIcon size="sm" variant="secondary" icon={Folder01Icon} label={t(REVEAL)} onClick={() => revealFile(a)} />
      {!isImage(a) && <ShareButton a={a} />}
      {url && <TipIcon size="sm" variant="secondary" icon={Link01Icon} label={t("复制来源链接")} onClick={() => copyLink(a)} />}
      {more}
      <span className="end">
        <Button variant="ghost" size="sm" icon={<Icon icon={Delete02Icon} size={14} />} onClick={() => onAskTrash(a)}>
          {moveToTrash()}
        </Button>
      </span>
    </div>
  );
}

/** 发给 AI：整理文字稿和关键帧放进剪贴板；整理的时候图标换成转圈（图标互换 100ms，配方 c7），提示里写进度 */
function ShareButton({ a }: { a: Asset }) {
  const op = useOp(a.id, "share");
  const busy = op?.st === "run";
  return (
    <TipIcon
      size="sm"
      variant="secondary"
      label={busy ? t("正在整理{pct}%", { pct: Math.round(op?.p ?? 0) }) : t("发给 AI：文字稿和关键帧放进剪贴板")}
      aria-busy={busy || undefined}
      icon={
        <IconSwap k={busy ? "busy" : "idle"}>
          {busy ? <Spinner size={14} /> : <Icon icon={AiBrain01Icon} size={16} />}
        </IconSwap>
      }
      onClick={() => !busy && shareToAI(a)}
    />
  );
}

/* ---------------- 分节（和下载页的详情同一个写法，收起状态记在界面偏好里） ---------------- */

function Section({ id, title, openSignal, children }: { id: string; title: string; openSignal?: number; children: ReactNode }) {
  const [open, setOpen] = useState(() => getPref(`lsec:${id}`, true));
  useEffect(() => {
    if (openSignal) setOpen(true);
  }, [openSignal]);
  const toggle = () =>
    setOpen((v) => {
      setPref(`lsec:${id}`, !v);
      return !v;
    });
  return (
    <section className="cp-dsec" data-open={open || undefined}>
      <div className="cp-dsec-h">
        <button className="t" aria-expanded={open} onClick={toggle}>
          {title}
          <Icon icon={ArrowDown01Icon} size={14} className="chev" />
        </button>
      </div>
      <Collapse open={open}>
        <div className="cp-dsec-b">{children}</div>
      </Collapse>
    </section>
  );
}

/* ---------------- 信息 ---------------- */

function Info({ a, onOpenAsset }: { a: Asset; onOpenAsset: (id: number) => void }) {
  const url = sourceUrl(a);
  const parent = useParent(a.parent_id ?? null);
  return (
    <dl className="cp-kv lib-kv">
      <dt>{t("来源")}</dt>
      <dd>
        {url ? (
          <button className="lib-src" title={t("在浏览器里打开{url}", { url })} onClick={() => openSource(a)}>
            {bare(url)}
          </button>
        ) : (
          <span className="lib-muted">{t("本地文件")}</span>
        )}
      </dd>
      <dt>{t("格式")}</dt>
      <dd>{a.fmt || "—"}</dd>
      {a.res && a.res !== "—" && (
        <>
          <dt>{isImage(a) ? t("尺寸") : t("分辨率")}</dt>
          <dd className="num">{a.res}</dd>
        </>
      )}
      {a.dur && (
        <>
          <dt>{t("时长")}</dt>
          <dd className="num">{a.dur}</dd>
        </>
      )}
      <dt>{t("大小")}</dt>
      <dd className="num">{a.size}</dd>
      <dt>{t("下载于")}</dt>
      <dd className="num">{fullTime(a.created_at) || a.when}</dd>
      <dt>{t("位置")}</dt>
      <dd>
        {a.exists ? (
          <button className="lib-place" title={t("{action}：{path}", { action: t(REVEAL), path: home(a.path) })} onClick={() => revealFile(a)}>
            {placeOf(a.path)}
          </button>
        ) : (
          <span className="lib-muted" title={home(a.path)}>
            {placeOf(a.path)}
          </span>
        )}
      </dd>
      {parent && (
        <>
          <dt>{t("原素材")}</dt>
          <dd>
            <button className="lib-place" title={t("看原来那一条")} onClick={() => onOpenAsset(parent.id)}>
              {parent.title}
            </button>
          </dd>
        </>
      )}
    </dl>
  );
}

/** 转换、剪辑出来的新文件：找到原来那一条（列表里没有时问一次后端） */
function useParent(id: number | null): Asset | null {
  const [p, setP] = useState<Asset | null>(() => (id != null ? (knownAsset(id) ?? null) : null));
  useEffect(() => {
    if (id == null) return setP(null);
    const k = knownAsset(id);
    if (k) return setP(k);
    let live = true;
    fetchAsset(id).then((x) => live && setP(x));
    return () => {
      live = false;
    };
  }, [id]);
  return p;
}

/* ---------------- 标签 ---------------- */

/** 标签：每个是一个小胶囊，右边的 × 去掉；「+ 标签」点开变成输入框，回车加上，Esc 不加了 */
function Tags({ a }: { a: Asset }) {
  const [tags, setTags] = useState(a.tags);
  const [adding, setAdding] = useState(false);
  const [text, setText] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const plus = useRef<HTMLButtonElement>(null);
  /** 回车或 Esc 已经收起了：随后的 blur 不再算一次 */
  const closed = useRef(false);
  const sig = a.tags.join("\n");
  useEffect(() => setTags(a.tags), [a.id, sig]); // eslint-disable-line react-hooks/exhaustive-deps

  const done = (keep: boolean) => {
    if (closed.current) return;
    closed.current = true;
    const name = text.trim();
    setAdding(false);
    setText("");
    if (keep && name && !tags.includes(name)) {
      setTags((L) => [...L, name]);
      addTag({ ...a, tags }, name).then((r) => r && setTags(r));
    }
    requestAnimationFrame(() => plus.current?.focus({ preventScroll: true }));
  };

  return (
    <div className="cp-tags lib-tags">
      {tags.map((tag) => (
        <Badge key={tag} size="md" className="lib-tag">
          {tag}
          <button
            className="x"
            aria-label={t("去掉标签「{tag}」", { tag })}
            onClick={() => {
              setTags((L) => L.filter((x) => x !== tag));
              removeTag(a, tag).then((r) => r && setTags(r));
            }}
          >
            <Icon icon={Cancel01Icon} size={10} stroke={2.2} />
          </button>
        </Badge>
      ))}
      {adding ? (
        <TextField
          ref={input}
          size="sm"
          autoFocus
          aria-label={t("新标签")}
          placeholder={t("标签名，回车加上")}
          className="lib-tagin"
          value={text}
          maxLength={24}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              done(true);
            } else if (e.key === "Escape") {
              e.preventDefault();
              e.stopPropagation();
              done(false);
            }
          }}
          onBlur={() => done(true)}
        />
      ) : (
        <Button
          ref={plus}
          variant="ghost"
          size="sm"
          className="lib-tagadd"
          icon={<Icon icon={PlusSignIcon} size={13} />}
          onClick={() => {
            closed.current = false;
            setAdding(true);
          }}
        >
          {t("标签§详情里加标签的按钮")}
        </Button>
      )}
    </div>
  );
}
