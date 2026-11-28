// 浏览器插件的工具栏面板（界面规范见 docs/design/README.md）：
// 顶上 logo + 连接状态 + 设置；这个网页的卡片；类型分段；能下载的一条条（清晰度、下载、进度环点一下停止、下载过了）；
// 全部下载；三个开关；底部「打开 Captae」。没开、没装、连不上 Captae 时，中间换成说明和按钮。
//
// 动效（配方 l4）：面板是浏览器自己画的窗口，打开不加动画；行里的状态直接换，不做布局动画；
// 只有按钮按下（affectUI 自带）、悬停底色（150ms ease）、读到缩略图（150ms 淡入，已经在缓存里的直接显示）。
// 「带上这个网站的登录状态」：按网站记（主域名），默认开；关掉的网站下载、读取信息时不读 cookies（后台 background.js 的「登录状态」一节）。
// 录直播的那一行：不放进度环，写「正在录制 · 已录 12:34」（按秒走，等宽数字，不做动画），底下一行「取消录制」「停止并保存」；
// 取消会删掉录到的，先在这一行里问一句（照桌面端的说法），再点一次「取消录制」才生效。
import {
  AiBrain01Icon,
  AlertCircleIcon,
  ArrowDown01Icon,
  ArrowRight01Icon,
  CheckmarkCircle02Icon,
  Download04Icon,
  Image01Icon,
  Link01Icon,
  MusicNote01Icon,
  PlayListIcon,
  RefreshIcon,
  Settings02Icon,
  StopIcon,
  Video01Icon,
} from "@hugeicons/core-free-icons";
import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { DropdownMenu } from "@/components/ui/dropdown-menu";
import { EmptyState } from "@/components/ui/empty-state";
import { IconButton } from "@/components/ui/icon-button";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { Spinner } from "@/components/ui/spinner";
import { Switch } from "@/components/ui/switch";
import { Tooltip } from "@/components/ui/tooltip";
import { N_, t, tx } from "@/i18n";
import { host, mb } from "@/lib/format";
import { CaptaeLogo, Elapsed, Icon, type HugeIcon } from "@/parts";
import { closePopup, inExtension, send, tellPage } from "./bridge";
import {
  chosen,
  cleanTitle,
  KIND,
  metaBits,
  options,
  plainSize,
  rowView,
  type Check,
  type Choice,
  type Conn,
  type Dl,
  type Fail,
  type Item,
  type JobLite,
  type Opt,
  type PopupError,
  type Probe,
  type RowView,
  type State,
} from "./model";
import "./popup.css";

type Filter = "all" | Item["kind"];
const HOST_DOWN = /HOST_MISSING|HOST_BROKEN|APP_NOT_RUNNING|HOST_GONE|TIMEOUT/;

/** 电脑是什么系统：去哪里打开 Captae、怎么装，说法不一样（macOS「应用程序」，Windows 开始菜单、安装程序，Linux 应用列表、deb / AppImage）。
 *  面板不走桌面端的 lib/bridge（那里带着 Tauri 的接口），这里自己看；演示构建可以用 `&os=win` / `&os=linux` 换 */
const OS = (() => {
  const forced = inExtension ? null : new URLSearchParams(location.search).get("os");
  if (forced === "mac" || forced === "win" || forced === "linux") return forced;
  return /Mac/.test(navigator.platform) ? "mac" : /Win/.test(navigator.platform) ? "win" : "linux";
})();
const WAKE_FAILED = OS === "mac" ? N_("没能打开 Captae。在「应用程序」里打开它，再点一次。") : OS === "win" ? N_("没能打开 Captae。从开始菜单打开它，再点一次。") : N_("没能打开 Captae。从应用列表里打开它，再点一次。");
const OPEN_APP = OS === "mac" ? N_("打开 Captae（在「应用程序」里）。") : OS === "win" ? N_("打开 Captae（在开始菜单里）。") : N_("打开 Captae（在应用列表里）。");
/** 下载安装包的地方（GitHub 上最新的 Release） */
const RELEASES_URL = "https://github.com/sol1560/Captae/releases/latest";
const INSTALL_APP = OS === "mac" ? N_("打开 Captae 的安装包，把 Captae 拖进「Applications」。") : OS === "win" ? N_("运行 Captae 的安装程序。") : N_("安装 Captae（.deb 包或 AppImage）。");

export function Popup() {
  const [st, setSt] = useState<State | null>(null);
  const [fatal, setFatal] = useState("");
  const [dl, setDl] = useState<Record<string, Dl>>({});
  const [jobs, setJobs] = useState<Record<number, JobLite>>({});
  const [checks, setChecks] = useState<Record<string, Check>>({});
  const [qual, setQual] = useState<Record<string, Choice>>({});
  const [rowErr, setRowErr] = useState<Record<string, string>>({});
  const [filter, setFilter] = useState<Filter>("all");
  const [waking, setWaking] = useState<"" | "busy" | "fail">("");
  const [hello, setHello] = useState(false);

  /** 读一次后台的状态：连接、这个页面的条目、进行中的下载、开关 */
  const load = useCallback(async () => {
    const s = await send<State>({ t: "state" });
    setSt(s);
    setDl(s.dl || {});
    setQual(s.qual || {});
    return s;
  }, []);

  useEffect(() => {
    load().catch((e: Error) => setFatal(e.message));
  }, [load]);

  const conn: Conn | null = st?.conn ?? null;
  const items = useMemo(() => st?.items ?? [], [st]);

  // 补信息：整页的问下载引擎（各清晰度多大，几秒），每一条查一下素材库里有没有，图片读一下尺寸
  const asked = useRef(new Set<string>());
  useEffect(() => {
    if (conn !== "ok") return;
    for (const it of items) {
      if (asked.current.has(it.id)) continue;
      asked.current.add(it.id);
      send<Check>({ t: "check", itemId: it.id })
        .then((c) => c.found && setChecks((m) => ({ ...m, [it.id]: c })))
        .catch(() => undefined);
      if (it.src === "page" && !it.probe)
        send<Probe>({ t: "probe", itemId: it.id })
          .then((p) => setSt((s) => s && { ...s, items: s.items.map((x) => (x.id === it.id ? { ...x, probe: p, name: p.title || x.name, img: x.img || p.thumbnail || undefined } : x)) }))
          .catch(() => undefined);
    }
  }, [conn, items]);

  // 进度：有下载在进行就每 800ms 问一次；在 Captae 里取消了的任务不再回来，那一行回到可以下载
  const ids = useMemo(() => Object.values(dl).map((d) => d.job).filter((x): x is number => typeof x === "number" && x > 0), [dl]);
  const idKey = ids.join(",");
  useEffect(() => {
    if (conn !== "ok" || !ids.length) return;
    let alive = true;
    const tick = async () => {
      // 这一次问的是哪几个任务：回复回来以前，「全部下载」可能又开始了新的一条（dl 里多了它，ids 里还没有）
      const asked = ids;
      try {
        const list = await send<JobLite[]>({ t: "jobs", ids: asked });
        if (!alive) return;
        setJobs((m) => {
          const next = { ...m };
          for (const j of list) next[j.id] = j;
          return next;
        });
        const live = new Set(list.map((j) => j.id));
        setDl((d) => {
          // 只把这一次问到、回复里却没有的当成「在 Captae 里取消了」；没问到的新任务等下一轮
          // （原来按最新的 dl 判断，刚开始的那一条会被误删、悄悄回到「可以下载」：第三轮插件回归 t4 的偶发问题）
          const gone = Object.entries(d).filter(([, v]) => v.job > 0 && asked.includes(v.job) && !live.has(v.job));
          if (!gone.length) return d;
          const n = { ...d };
          for (const [k] of gone) delete n[k];
          return n;
        });
      } catch {
        /* Captae 刚关掉：下一次 state 会说清楚 */
      }
    };
    tick();
    const timer = setInterval(tick, 800);
    return () => {
      alive = false;
      clearInterval(timer);
    };
    // ids 用 idKey 比较，避免每次渲染都重新开计时器
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [conn, idKey]);

  /** 和 Captae 断开了（中途关掉、卸载）：重新读一次状态，界面换成对应的说明。
   *  Captae 还开着、只是这一次没交出去（等太久没回话、连接程序中途断了）：这一行写清楚，不悄悄回到「可以下载」 */
  const failed = useCallback(
    (it: Item | null, e: unknown) => {
      const err = e as PopupError;
      const code = err.code || err.message || "";
      if (HOST_DOWN.test(code)) {
        load().catch(() => undefined);
        if (it && /TIMEOUT|HOST_GONE/.test(code))
          setRowErr((m) => ({ ...m, [it.id]: /TIMEOUT/.test(code) ? t("Captae 一直没回话，可能已经收下了：先看看 Captae 的下载列表") : t("和 Captae 的连接断了一下，没交出去，再点一次下载") }));
        return;
      }
      if (it) setRowErr((m) => ({ ...m, [it.id]: err.fail?.message || err.message || t("没办成") }));
    },
    [load],
  );

  const choiceOf = useCallback(
    (it: Item): Choice => {
      const o = options(it);
      const k = chosen(it, o, qual, st?.settings.qpref);
      return k >= 0 ? o[k].ch : {};
    },
    [qual, st?.settings.qpref],
  );

  const download = useCallback(
    async (it: Item) => {
      const ch = choiceOf(it);
      setRowErr((m) => {
        const n = { ...m };
        delete n[it.id];
        return n;
      });
      setDl((d) => ({ ...d, [it.id]: { job: 0, ch } }));
      try {
        const job = await send<JobLite>({ t: "download", itemId: it.id, ch });
        setDl((d) => ({ ...d, [it.id]: { job: job.id, ch } }));
        setJobs((m) => ({ ...m, [job.id]: job }));
      } catch (e) {
        setDl((d) => {
          const n = { ...d };
          delete n[it.id];
          return n;
        });
        failed(it, e);
      }
    },
    [choiceOf, failed],
  );

  /** 点进度环：停止这个下载，那一行回到可以下载的样子 */
  const stop = useCallback(
    async (it: Item) => {
      const d = dl[it.id];
      setDl((m) => {
        const n = { ...m };
        delete n[it.id];
        return n;
      });
      try {
        await send({ t: "stop", itemId: it.id, job: d?.job || undefined });
      } catch (e) {
        failed(it, e);
      }
    },
    [dl, failed],
  );

  /** 录直播「停止并保存」：录到的部分整理好放进素材库；点了马上显示「正在保存」，不等下一轮 */
  const save = useCallback(
    async (it: Item) => {
      const d = dl[it.id];
      if (!d?.job) return;
      try {
        await send({ t: "save", itemId: it.id, job: d.job });
        setJobs((m) => {
          const j = m[d.job];
          return j?.rec ? { ...m, [d.job]: { ...j, phase: "merge", rec: { ...j.rec, stopping: true, until: Date.now() } } } : m;
        });
      } catch (e) {
        // 刚好直播自己结束了（已经在保存）：下一轮会显示真实的样子
        const pe = e as PopupError;
        if (pe.code !== "NOT_RECORDING" && !/只有正在录制/.test(pe.message || "")) failed(it, e);
      }
    },
    [dl, failed],
  );

  const open = useCallback(
    (target: Record<string, unknown>) => {
      send({ t: "open", target })
        .then(() => closePopup())
        .catch((e) => failed(null, e));
    },
    [failed],
  );

  const pick = useCallback((it: Item, o: Opt) => {
    setQual((m) => ({ ...m, [it.id]: o.ch }));
    send({ t: "qual", itemId: it.id, ch: o.ch }).catch(() => undefined);
  }, []);

  /** 发给 AI：进度在网页角落的卡片里显示，面板可以关掉；网页里没有 Captae（浏览器自己的页面）就直接交给 Captae */
  const toAI = useCallback((it: Item) => {
    tellPage({ t: "share-item", itemId: it.id, name: it.name, img: it.img, dur: it.dur })
      .catch(() => send({ t: "share", itemId: it.id }))
      .finally(() => closePopup());
  }, []);

  const setOpt = useCallback(
    (k: "inj", v: boolean) => {
      setSt((s) => s && { ...s, settings: { ...s.settings, [k]: v } });
      send({ t: "settings", patch: { [k]: v } }).catch(() => undefined);
    },
    [setSt],
  );

  /** 「带上这个网站的登录状态」：只记这个网站 */
  const setLogin = useCallback((on: boolean) => {
    setSt((s) => (s && s.login ? { ...s, login: { ...s.login, on } } : s));
    send({ t: "login", on }).catch(() => undefined);
  }, []);

  /** 失败了再试一次：先把 Captae 里那条失败的去掉（不留两条），再下载（会带上现在的登录状态） */
  const retry = useCallback(
    async (it: Item) => {
      const d = dl[it.id];
      if (d?.job) await send({ t: "stop", itemId: it.id, job: d.job }).catch(() => undefined);
      download(it);
    },
    [dl, download],
  );

  /** 打开 Captae：宿主会去启动它；之后每 0.8 秒看一次，最多 10 秒 */
  const wake = useCallback(async () => {
    setWaking("busy");
    await send({ t: "open", target: {} }).catch(() => undefined);
    for (let i = 0; i < 12; i++) {
      const s = await load().catch(() => null);
      if (s?.conn === "ok") {
        setWaking("");
        setHello(true);
        setTimeout(() => setHello(false), 3000);
        return;
      }
      await new Promise((r) => setTimeout(r, 800));
    }
    setWaking("fail");
  }, [load]);

  const views = useMemo(() => {
    const v: Record<string, RowView> = {};
    for (const it of items) v[it.id] = rowView(it, dl, jobs, checks);
    return v;
  }, [items, dl, jobs, checks]);

  if (fatal) return <Shell conn={null} onGear={() => undefined}><EmptyState bare icon={null} title={t("出了点问题")} text={fatal} /></Shell>;
  if (!st) return <Shell conn={null} onGear={() => undefined} loading />;

  const host_ = host(st.page.url || "");
  const title = cleanTitle(st.page.title) || host_;
  const counts: Record<Filter, number> = { all: items.length, video: 0, audio: 0, image: 0 };
  for (const it of items) counts[it.kind]++;
  const kinds = (["video", "audio", "image"] as const).filter((k) => counts[k]);
  const shown = items.filter((i) => filter === "all" || i.kind === filter);
  const idle = items.filter((i) => views[i.id]?.k === "idle");
  const running = items.filter((i) => views[i.id]?.k === "run").length;
  // 正在录直播、正在保存录到的部分：按钮不说「都在 Captae 里了」
  const recs = items.filter((i) => views[i.id]?.k === "rec" || views[i.id]?.k === "saving").length;

  return (
    <Shell conn={st.conn} onGear={() => open({ page: "extension" })} onOpen={() => open({})}>
      {st.conn !== "ok" ? (
        <Offline conn={st.conn} reason={st.reason} waking={waking} onWake={wake} onRetry={() => load().catch(() => undefined)} />
      ) : (
        <>
          {hello && (
            <div className="xp-hello" role="status">
              <Icon icon={CheckmarkCircle02Icon} size={15} />
              {t("Captae 已连上，现在可以下载了")}
            </div>
          )}
          {st.stale && (
            <div className="xp-hello xp-stale" role="status">
              <Icon icon={AlertCircleIcon} size={15} />
              <span>{t("插件比 Captae {version} 旧，到扩展程序页面点「重新加载」", { version: st.stale })}</span>
              <button type="button" className="xp-link" onClick={openExtensionsPage}>
                {t("打开")}
              </button>
            </div>
          )}
          <section className="xp-page" aria-label={t("这个网页")}>
            <strong title={title}>{title}</strong>
            <span>
              {host_}
              {items.length ? ` · ${t("在这个页面找到{n}个", { n: items.length })}` : ""}
            </span>
          </section>
          {items.length ? (
            <>
              {kinds.length > 1 && (
                <div className="xp-seg">
                  <SegmentedControl<Filter>
                    aria-label={t("按类型看")}
                    size="sm"
                    fullWidth
                    value={filter}
                    onChange={setFilter}
                    options={[{ value: "all", label: t("全部"), count: counts.all }, ...kinds.map((k) => ({ value: k as Filter, label: t(KIND[k]), count: counts[k] }))]}
                  />
                </div>
              )}
              <ul className="xp-rows" aria-label={t("这个页面能下载的")}>
                {shown.map((it) => (
                  <Row
                    key={it.id}
                    it={it}
                    view={views[it.id]}
                    err={rowErr[it.id]}
                    loginOff={st.login?.on === false}
                    qual={qual}
                    qpref={st.settings.qpref}
                    onDownload={download}
                    onRetry={retry}
                    onStop={stop}
                    onSave={save}
                    onPick={pick}
                    onAI={toAI}
                    onOpen={open}
                  />
                ))}
              </ul>
              <div className="xp-all">
                <Button
                  className="cp-pill"
                  disabled={!idle.length}
                  icon={idle.length ? <Icon icon={Download04Icon} size={15} /> : running || recs ? <Spinner size={14} /> : <Icon icon={CheckmarkCircle02Icon} size={15} />}
                  onClick={async () => {
                    for (const it of idle) await download(it);
                  }}
                >
                  {idle.length ? t("全部下载（{n}）", { n: idle.length }) : running ? t("正在下载{n}个…", { n: running }) : recs ? t("正在录直播…") : t("都在 Captae 里了")}
                </Button>
              </div>
            </>
          ) : (
            <EmptyState
              className="xp-empty"
              bare
              icon={Video01Icon}
              title={t("这个页面没找到能下载的视频或图片")}
              text={t("有的网站要先点播放，视频才会出现。手上有链接的话，也可以直接交给 Captae。")}
              action={
                <Button variant="secondary" size="sm" icon={<Icon icon={Link01Icon} size={15} />} onClick={() => open({ paste: true })}>
                  {t("粘贴链接下载")}
                </Button>
              }
            />
          )}
          <div className="xp-opts cp-group" role="group" aria-label={t("插件设置")}>
            <div className="cp-set-sw">
              <Switch size="sm" placement="end" label={t("在视频旁边显示按钮")} checked={st.settings.inj} onChange={(e) => setOpt("inj", e.target.checked)} />
            </div>
            {st.login?.site && (
              <div className="cp-set-sw">
                <Switch
                  size="sm"
                  placement="end"
                  label={t("带上这个网站的登录状态")}
                  description={t("要登录才能看的视频也能下")}
                  checked={st.login.on}
                  onChange={(e) => setLogin(e.target.checked)}
                />
              </div>
            )}
          </div>
        </>
      )}
    </Shell>
  );
}

/** 打开这个浏览器的扩展程序页面、定位到 Captae 插件（Edge、Brave 等也认 chrome:// 的写法） */
function openExtensionsPage() {
  if (typeof chrome === "undefined" || !chrome.runtime?.id) return;
  chrome.tabs.create({ url: `chrome://extensions/?id=${chrome.runtime.id}` }).catch(() => undefined);
}

/** 面板外框：顶上 logo + 状态 + 设置，底下「打开 Captae」 */
function Shell({ conn, onGear, onOpen, loading, children }: { conn: Conn | null; onGear: () => void; onOpen?: () => void; loading?: boolean; children?: ReactNode }) {
  const label = conn === "ok" ? t("已连上") : conn === "off" ? t("没有打开") : conn === "nohost" ? t("没有安装") : conn === "broken" ? t("连不上") : "";
  return (
    <div className="xp" data-conn={conn ?? undefined}>
      <header className="xp-head">
        <CaptaeLogo size={20} />
        <b>Captae</b>
        <span className="st">
          {loading ? <Spinner size={12} label={t("正在连接 Captae")} /> : conn && <span className="cp-live" data-off={conn !== "ok" || undefined} data-tone={conn === "ok" ? undefined : "warn"} />}
          {label}
          {conn === "ok" && (
            <Tooltip content={t("在 Captae 里打开「浏览器插件」页")} side="bottom" align="end">
              <IconButton icon={Settings02Icon} label={t("插件设置")} title="" size="sm" onClick={onGear} />
            </Tooltip>
          )}
        </span>
      </header>
      {children}
      <footer className="xp-foot">
        <span>{t("下载的东西都在 Captae 里")}</span>
        {onOpen && conn === "ok" && (
          <button type="button" className="xp-link" onClick={onOpen}>
            {t("打开 Captae")}
            <Icon icon={ArrowRight01Icon} size={13} />
          </button>
        )}
      </footer>
    </div>
  );
}

/** 没开、没装、连不上：说清楚是哪一种，给能做的那一步 */
function Offline({ conn, reason, waking, onWake, onRetry }: { conn: Conn; reason?: string; waking: "" | "busy" | "fail"; onWake: () => void; onRetry: () => void }) {
  if (conn === "off")
    return (
      <EmptyState
        className="xp-off"
        bare
        icon={<CaptaeLogo size={36} />}
        title={t("Captae 没有打开")}
        text={waking === "fail" ? t(WAKE_FAILED) : t("下载都由电脑上的 Captae 完成，打开它就能用。")}
        action={
          <Button size="sm" loading={waking === "busy"} onClick={onWake}>
            {waking === "busy" ? t("正在打开…") : waking === "fail" ? t("再试一次") : t("打开 Captae")}
          </Button>
        }
      />
    );
  const broken = conn === "broken";
  return (
    <div className="xp-off">
      <EmptyState
        bare
        icon={<CaptaeLogo size={36} />}
        title={broken ? t("连不上 Captae") : t("电脑上还没有 Captae")}
        text={
          broken
            ? reason === "forbidden"
              ? t("Captae 没认出这个插件（插件的身份对不上）。")
              : t("Captae 的连接程序没能启动，可能被移动或删掉了。")
            : t("下载都由电脑上的 Captae 完成，装好就能用。")
        }
        action={
          <>
            {!broken && (
              <Button size="sm" variant="secondary" onClick={() => window.open(RELEASES_URL, "_blank", "noopener")}>
                {t("下载 Captae")}
              </Button>
            )}
            <Button size="sm" variant="secondary" icon={<Icon icon={RefreshIcon} size={14} />} onClick={onRetry}>
              {t("再检查一次")}
            </Button>
          </>
        }
      />
      <ol className="xp-steps">
        {broken ? (
          <>
            <li>{t(OPEN_APP)}</li>
            <li>{t("到 Captae 的「浏览器插件」页，点「修复」。")}</li>
            <li>{t("回到这里点「再检查一次」。")}</li>
          </>
        ) : (
          <>
            <li>{t(INSTALL_APP)}</li>
            <li>{t("打开一次 Captae，它会自己连上这个浏览器。")}</li>
            <li>{t("已经装了还连不上：到 Captae 的「浏览器插件」页点「修复」。")}</li>
          </>
        )}
      </ol>
    </div>
  );
}

type RowProps = {
  it: Item;
  view: RowView;
  err?: string;
  /** 这个网站的「带上登录状态」关着：要登录的失败提示去打开它 */
  loginOff?: boolean;
  qual: Record<string, Choice>;
  qpref?: string;
  onDownload: (it: Item) => void;
  onRetry: (it: Item) => void;
  onStop: (it: Item) => void;
  onSave: (it: Item) => void;
  onPick: (it: Item, o: Opt) => void;
  onAI: (it: Item) => void;
  onOpen: (target: Record<string, unknown>) => void;
};

/** 一行：缩略图、名字、小字（类型 · 时长 · 清晰度 · 大小，或者状态），右边一个按钮 */
/** 失败的一句话；要登录、要验证而这个网站的开关关着时，告诉用户去打开它（msg 是 Captae 按当时的语言写好的说法） */
function failLine(f: Fail | null | undefined, fallback: string, loginOff?: boolean) {
  const msg = f?.message || fallback;
  return loginOff && (f?.code === "need_login" || f?.code === "need_verify") ? t("{msg}（下面「带上这个网站的登录状态」关着）", { msg }) : msg;
}

function Row({ it, view, err, loginOff, qual, qpref, onDownload, onRetry, onStop, onSave, onPick, onAI, onOpen }: RowProps) {
  const opts = options(it);
  const k = chosen(it, opts, qual, qpref);
  const cur = k >= 0 ? opts[k] : null;
  const bits = metaBits(it);
  // 「取消录制」点了一次：这一行先问一句，再点一次才取消（录完了、开始保存了就不再问）
  const [asking, setAsking] = useState(false);
  const isRec = view.k === "rec";
  useEffect(() => {
    if (!isRec) setAsking(false);
  }, [isRec]);

  let meta: ReactNode;
  let act: ReactNode;
  switch (view.k) {
    case "rec": {
      const got = view.job.got > 0 ? mb(view.job.got) : "";
      meta = asking ? (
        <span className="bad">
          {got ? t("取消这次录制？已经录的{size}会被删掉。想留下录到的部分，点「停止并保存」。", { size: got }) : t("取消这次录制？已经录到的会被删掉。想留下录到的部分，点「停止并保存」。")}
        </span>
      ) : (
        <span className="num">
          <span className="cp-rec-dot" aria-hidden />
          {tx("正在录制 · 已录 {time}", { time: <Elapsed from={view.job.rec!.since} asClock /> })}
          {got && ` · ${got}`}
        </span>
      );
      act = asking ? (
        <>
          <Button variant="ghost" size="sm" onClick={() => setAsking(false)} autoFocus>
            {t("继续录")}
          </Button>
          <Button variant="secondary" size="sm" onClick={() => onSave(it)}>
            {t("停止并保存")}
          </Button>
          <Button variant="destructive" size="sm" onClick={() => onStop(it)}>
            {t("取消录制")}
          </Button>
        </>
      ) : (
        <>
          <Button variant="ghost" size="sm" onClick={() => setAsking(true)}>
            {t("取消录制")}
          </Button>
          <Button size="sm" icon={<Icon icon={StopIcon} size={14} />} onClick={() => onSave(it)}>
            {t("停止并保存")}
          </Button>
        </>
      );
      break;
    }
    case "saving":
      meta = (
        <span className="num">
          {view.job.rec?.interrupted ? t("正在整理上次录到的部分") : t("正在保存录到的部分")}
          {view.job.got > 0 ? ` · ${mb(view.job.got)}` : ""}
        </span>
      );
      act = <Spinner size={14} label={t("正在保存录到的部分")} />;
      break;
    case "run":
      meta = <span className="num">{`${t(KIND[it.kind])} · ${view.text}`}</span>;
      act = (
        <Tooltip content={t("停止下载")} side="left">
          <button type="button" className="xp-ring" aria-label={t("{status}，点一下停止", { status: view.text })} onClick={() => onStop(it)}>
            <Ring p={view.p} busy={view.busy} />
          </button>
        </Tooltip>
      );
      break;
    case "done":
    case "got":
      meta = (
        <span>
          {bits.join(" · ")} · <span className="ok">{view.k === "done" && view.job.rec ? t("录好了") : t("下载过了")}</span>
        </span>
      );
      act = (
        <IconButton
          icon={CheckmarkCircle02Icon}
          label={view.k === "done" && view.job.rec ? t("录好了，在 Captae 里打开") : t("下载过了，在 Captae 里打开")}
          size="sm"
          className="xp-got"
          onClick={() => onOpen(view.k === "got" ? { asset: view.check.assetId } : view.job.assetId != null ? { asset: view.job.assetId } : { page: "library" })}
        />
      );
      break;
    case "failed":
      meta = <span className="bad">{failLine(view.job.fail, view.job.err || t("没下载成功"), loginOff)}</span>;
      act = <IconButton icon={RefreshIcon} label={t("再试一次")} variant="secondary" size="sm" onClick={() => onRetry(it)} />;
      break;
    case "choose":
      meta = <span>{(view.job.entries || []).length ? t("{n}个视频，在 Captae 里选", { n: view.job.entries!.length }) : t("播放列表，在 Captae 里选")}</span>;
      act = <IconButton icon={PlayListIcon} label={t("在 Captae 里选要下的")} variant="secondary" size="sm" onClick={() => onOpen({ page: "downloads", job: view.job.id })} />;
      break;
    default: {
      const size = cur?.size || plainSize(it);
      const chip = opts.length > 1 && !!cur;
      meta = (
        <>
          <span>{bits.join(" · ") + (!chip && size ? " · " + size : "")}</span>
          {chip ? (
            <DropdownMenu
              align="start"
              title={t("清晰度")}
              width={200}
              items={opts.map((o, i) => ({ label: o.label, description: [o.size, o.rec ? t("推荐") : ""].filter(Boolean).join(" · ") || undefined, checked: i === k, onSelect: () => onPick(it, o) }))}
              trigger={({ triggerProps }) => (
                <button type="button" className="xp-q" aria-label={t("{name} 的清晰度：{quality}", { name: it.name, quality: cur.label })} {...triggerProps}>
                  {cur.label}
                  <Icon icon={ArrowDown01Icon} size={11} />
                </button>
              )}
            />
          ) : null}
          {chip && size && <span className="num">{size}</span>}
        </>
      );
      if (err) meta = <span className="bad">{err}</span>;
      act = (
        <>
          {it.kind === "video" && (
            <Tooltip content={t("发给 AI")} side="left">
              <IconButton icon={AiBrain01Icon} label={t("把「{name}」发给 AI", { name: it.name })} title="" size="sm" className="xp-ai" onClick={() => onAI(it)} />
            </Tooltip>
          )}
          <Tooltip content={t("下载")} side="left">
            <IconButton icon={Download04Icon} label={t("下载「{name}」", { name: it.name })} title="" variant="secondary" size="sm" onClick={() => onDownload(it)} />
          </Tooltip>
        </>
      );
    }
  }

  return (
    <li className="xp-row" data-v={view.k} data-ask={asking || undefined}>
      <Thumb it={it} />
      <div className="body">
        <strong title={it.name}>{it.name}</strong>
        <div className="m">{meta}</div>
      </div>
      <div className="act">{act}</div>
    </li>
  );
}

const GLYPH: Record<Item["kind"], HugeIcon> = { video: Video01Icon, audio: MusicNote01Icon, image: Image01Icon };

/** 缩略图：已经在缓存里的直接显示；要等的读到后 150ms 淡入（面板里取下限，配方 l4） */
function Thumb({ it }: { it: Item }) {
  const src = it.kind === "image" ? it.url : it.img || it.probe?.thumbnail || "";
  const ref = useRef<HTMLImageElement>(null);
  const [ok, setOk] = useState<"" | "now" | "fade">("");
  const [bad, setBad] = useState(false);
  useLayoutEffect(() => {
    setBad(false);
    setOk(ref.current?.complete && ref.current.naturalWidth ? "now" : "");
  }, [src]);
  return (
    <span className="xp-th" data-kind={it.kind}>
      <Icon icon={GLYPH[it.kind]} size={16} />
      {src && !bad && (
        <img
          ref={ref}
          src={src}
          alt=""
          referrerPolicy="no-referrer"
          data-ok={ok || undefined}
          onLoad={(e) => {
            setOk((v) => v || "fade");
            // 图片：读到了顺便记下尺寸，面板和网页上都显示
            const im = e.currentTarget;
            if (it.kind === "image" && !it.dims && im.naturalWidth) send({ t: "dims", itemId: it.id, dims: [im.naturalWidth, im.naturalHeight] }).catch(() => undefined);
          }}
          onError={() => setBad(true)}
        />
      )}
    </span>
  );
}

/** 进度环（16px）：p 0–1；busy 时是一个呼吸的点 */
function Ring({ p, busy }: { p: number; busy: boolean }) {
  return (
    <svg width="18" height="18" viewBox="0 0 16 16" aria-hidden className="xp-ringsvg">
      <circle cx="8" cy="8" r="6" fill="none" stroke="rgba(0,0,0,.12)" strokeWidth="2" />
      {busy ? (
        <circle className="cp-pulse" cx="8" cy="8" r="2.5" fill="var(--ink-3)" />
      ) : (
        <circle cx="8" cy="8" r="6" fill="none" stroke="var(--accent)" strokeWidth="2" pathLength={100} strokeDasharray={`${Math.max(0, Math.min(1, p)) * 100} 100`} strokeLinecap="round" transform="rotate(-90 8 8)" />
      )}
      {/* 悬停时环里出现一个小方块：点一下就是停止 */}
      <rect className="sq" x="5.6" y="5.6" width="4.8" height="4.8" rx="1" fill="var(--ink)" />
    </svg>
  );
}
