// 素材库页（第二版界面规范见 docs/design/README.md）。
// 顶栏（毛玻璃）：标题和数量、类型分段控件、搜索、筛选（来源 / 时间 / 标签）、多选；下面是网格；右边一张详情浮卡。
// 主区不到 900px 宽时，详情改成盖在网格右边的浮卡，点了素材才出来（和下载页一样）。
// 功能和旧界面（ui/js/page-library.js、app-library.html）逐条对齐：分页读取、筛选和搜索、空库和没有匹配的说明、
// 文件不在了、详情（播放、胶片条、波形、信息、标签、在访达中显示、打开、复制来源链接、发给 AI、移除、移到废纸篓）、
// 转换 / 字幕 / 剪辑、多选后批量转换 / 拼接 / 加标签、后台任务的进度、#/library?asset= 和 ?search 跳转。
// 键盘：方向键换素材（不动画）、空格选上 / 取消（多选）、⌘A 全选、⌘⌫ 移到废纸篓、⌘F 搜索、Esc 退出多选或收起详情、Shift F10 右键菜单。
import { Album02Icon, ArrowDown01Icon, CheckmarkSquare02Icon, Search01Icon, Cancel01Icon, Download04Icon } from "@hugeicons/core-free-icons";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { useCallback, useEffect, useMemo, useRef, useState, type KeyboardEvent, type MouseEvent } from "react";
import { Button } from "@/components/ui/button";
import { ContextMenu } from "@/components/ui/context-menu";
import { AlertDialog } from "@/components/ui/dialog";
import { DropdownMenu, type DropdownMenuItem } from "@/components/ui/dropdown-menu";
import { EmptyState } from "@/components/ui/empty-state";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { ShimmerText } from "@/components/ui/shimmer-loader";
import { TextField } from "@/components/ui/text-field";
import { Tooltip } from "@/components/ui/tooltip";
import { t, tAny } from "@/i18n";
import { home } from "@/lib/format";
import { useDelayed, useWideEnough } from "@/lib/hooks";
import { notify } from "@/lib/notify";
import { isMac, modKey } from "@/lib/os";
import { getPref, setPref } from "@/lib/prefs";
import { clearParams, go, useRoute } from "@/lib/route";
import { onEvent, useApp } from "@/lib/store";
import { Icon, SiteIcon } from "@/parts";
import { Topbar } from "@/shell/Topbar";
import { moveToTrash, openFile, removeFromLibrary, TRASH, trash, useOps } from "./library/act";
import { BatchBar } from "./library/Batch";
import { fetchAsset, knownAsset, useHidden, useLibrary } from "./library/data";
import { Detail } from "./library/Detail";
import { GridSkeleton, Tile } from "./library/Grid";
import { assetMenu, type Tool } from "./library/menu";
import { anyFilter, DAYS, emptyText, extraFilters, filterLabel, gridColumns, isMedia, NO_FILTERS, siteKeyOf, type Asset, type Filters, type KindFilter } from "./library/model";
import "./library/library.css";

const EASE = [0.23, 1, 0.32, 1] as const;

/** 回到素材库时还是上次的筛选和搜索（只在这次打开 Captae 期间记住） */
let lastFilters: Filters = NO_FILTERS;

export function Library() {
  const route = useRoute();
  const tasks = useApp((s) => s.tasks);
  const ops = useOps();
  const hidden = useHidden();
  const reduce = useReducedMotion();

  const [filters, setFilters] = useState<Filters>(lastFilters);
  const [qText, setQText] = useState(lastFilters.q);
  const lib = useLibrary(filters);

  const [selId, setSelId] = useState<number | null>(() => getPref<number | null>("libSel", null));
  /** 选中的素材不在当前列表里（跳转过来、被筛掉、在后面的页）：单独问来的那一条 */
  const [extra, setExtra] = useState<Asset | null>(null);
  /** 这次换选中是键盘触发的：详情直接换，不淡入淡出（配方 e2） */
  const [instant, setInstant] = useState(false);
  const [sheet, setSheet] = useState(false);
  const [tool, setToolState] = useState<Tool>(() => getPref<Tool>("libTool", "convert"));
  /** 从菜单里点了某个处理：详情里「处理」那一节收着也要打开 */
  const [toolBump, setToolBump] = useState(0);
  const [picked, setPicked] = useState<number[]>([]);
  /** 最近一次选上 / 取消是键盘做的（空格、⌘A、Shift+方向键）：勾直接出现，不放大淡入 */
  const [kbdPick, setKbdPick] = useState(false);
  const [selectMode, setSelectMode] = useState(false);
  const [trashing, setTrashing] = useState<Asset | null>(null);

  const mainRef = useRef<HTMLElement>(null);
  const colRef = useRef<HTMLDivElement>(null);
  const gridRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  /** Shift 点选的起点 */
  const anchor = useRef<number | null>(null);
  /** 选中以后要滚到它（跳转过来的）；scrollTick 让同一页里再跳一次时也会滚 */
  const scrollTo = useRef<number | null>(null);
  const [scrollTick, setScrollTick] = useState(0);

  // 搜索：打字时只改输入框，停 200ms 再查（和旧界面一样）
  useEffect(() => {
    const timer = setTimeout(() => setFilters((f) => (f.q === qText ? f : { ...f, q: qText })), 200);
    return () => clearTimeout(timer);
  }, [qText]);
  useEffect(() => {
    lastFilters = filters;
  }, [filters]);

  const items = useMemo(() => lib.items.filter((a) => !hidden.has(a.id)), [lib.items, hidden]);
  // 后面还有：底下那一行「正在读取更多」等 300ms 才出现、文字扫光（配方 m）；位置先留着，出现时列表不跳
  const hasMore = lib.loaded && !lib.done && items.length > 0;
  const moreSlow = useDelayed(hasMore, 300);
  // 藏起来（等着真删、还能撤销）的那几条不算数；真删以后列表刷新、它已经不在 lib.items 里了，也就不会多减一次
  const hiddenInList = lib.items.length - items.length;
  const total = Math.max(0, lib.total - hiddenInList);
  const all = Math.max(0, lib.all - hiddenInList);

  /** 正在单独问的那一条：问到之前详情先留着上一条，不闪一下第一条 */
  const [fetching, setFetching] = useState<number | null>(null);
  const prevSel = useRef<Asset | null>(null);
  const inList = items.find((a) => a.id === selId);
  const selected: Asset | null =
    inList ?? (extra && extra.id === selId && !hidden.has(extra.id) ? extra : null) ?? (fetching != null && fetching === selId ? prevSel.current : lib.loaded ? (items[0] ?? null) : null);
  prevSel.current = selected;
  const selecting = selectMode || picked.length > 0;
  const pickedAssets = useMemo(() => picked.map((id) => items.find((a) => a.id === id) ?? knownAsset(id)).filter(Boolean) as Asset[], [picked, items]);

  // 主区 900px 以上才把详情放在右边一列（和下载页同一个分界）；窄了改成点开才出来的浮卡
  const wide = useWideEnough(mainRef, 900, [], () => innerWidth - 16 - (innerWidth <= 760 || getPref("sideOff", false) ? 0 : 244) >= 900);

  // 回调里要拿到最新的列表和状态，又不想每次都换函数（卡片是 memo 的）
  const live = useRef({ items, selected, selecting, picked, wide });
  live.current = { items, selected, selecting, picked, wide };

  /* ---------------- 选中 ---------------- */

  /** how：mouse 点的（详情淡入淡出，窄窗口打开浮卡）；key 回车（不动画，打开浮卡）；nav 方向键（不动画，浮卡开关不变） */
  const select = useCallback((id: number, how: "mouse" | "key" | "nav" = "mouse") => {
    setSelId(id);
    setPref("libSel", id);
    setInstant(how !== "mouse");
    if (how !== "nav") setSheet(true);
  }, []);

  /** 单独问一条（不在当前列表里的） */
  const fetchOne = useCallback((id: number, loud: boolean) => {
    setFetching(id);
    fetchAsset(id).then((a) => {
      setFetching((f) => (f === id ? null : f));
      if (a) setExtra(a);
      else if (loud) notify(t("找不到这个素材"), t("可能已经从素材库里移除了"));
    });
  }, []);

  /** 按 id 选中（跳转、「原素材」、提示条上的「查看」）：列表里没有就单独问一次 */
  const selectById = useCallback(
    (id: number) => {
      select(id);
      scrollTo.current = id;
      setScrollTick((n) => n + 1);
      if (!live.current.items.some((a) => a.id === id)) fetchOne(id, true);
    },
    [select, fetchOne],
  );

  // 打开素材库时，上次选的那一条不在第一页里：问一次；问不到就算了（选第一条）。只在刚打开时做——
  // 之后换了筛选、搜索，选中的不在结果里就换成结果的第一条，不再单独去问（访达、照片也是这样）
  const restored = useRef(false);
  useEffect(() => {
    if (!lib.loaded || restored.current) return;
    restored.current = true;
    if (selId != null && !inList && !(extra && extra.id === selId) && fetching !== selId) fetchOne(selId, false);
  }, [lib.loaded, selId, inList, extra, fetching, fetchOne]);

  // 单独问来的那一条，素材库变了也要跟着刷新（标签改了、文件没了）
  useEffect(
    () =>
      onEvent("library", () => {
        const x = live.current.selected;
        if (x && !live.current.items.some((a) => a.id === x.id)) fetchAsset(x.id).then((a) => a && setExtra(a));
      }),
    [],
  );

  // 选中的滚进可视区（跳转过来时）；只在需要时滚。它不在已经读到的几页里就算了——
  // 不能留着等，否则用户往下翻、读到它那一页时会突然跳过去
  useEffect(() => {
    const id = scrollTo.current;
    if (id == null || !lib.loaded) return;
    scrollTo.current = null;
    if (items.some((a) => a.id === id)) requestAnimationFrame(() => gridRef.current?.querySelector<HTMLElement>(`[data-id="${id}"]`)?.scrollIntoView({ block: "nearest" }));
  }, [items, lib.loaded, scrollTick]);

  /* ---------------- 跳转参数：?asset=<id>（下载完成的提示条、下载页、插件）、?search（侧栏搜索、⌘K） ---------------- */

  useEffect(() => {
    if (route.page !== "library") return;
    const p = route.params;
    let used = false;
    const id = Number(p.get("asset"));
    if (p.has("asset") && id) {
      used = true;
      selectById(id);
    }
    if (p.has("search")) {
      used = true;
      requestAnimationFrame(() => {
        searchRef.current?.focus();
        searchRef.current?.select();
      });
    }
    if (used) clearParams();
  }, [route, selectById]);

  /* ---------------- 翻页：滚到底附近要下一页；一页不够一屏时接着要 ---------------- */

  const fill = useCallback(() => {
    const el = colRef.current;
    if (el && el.scrollHeight - el.scrollTop - el.clientHeight < 600) lib.loadMore();
  }, [lib.loadMore]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (lib.loaded && !lib.done) requestAnimationFrame(fill);
  }, [lib.loaded, lib.done, items.length, fill]);

  // 换了筛选条件：回到顶上
  const fkey = JSON.stringify(filters);
  const firstKey = useRef(fkey);
  useEffect(() => {
    if (fkey === firstKey.current) return;
    firstKey.current = fkey;
    colRef.current?.scrollTo({ top: 0 });
    // 单独问来的那一条（跳转过来的）不再留着：选中的不在新结果里就换成第一条
    setExtra(null);
  }, [fkey]);

  /* ---------------- 多选 ---------------- */

  const togglePick = useCallback((id: number, kbd = false) => {
    setKbdPick(kbd);
    setPicked((L) => (L.includes(id) ? L.filter((x) => x !== id) : [...L, id]));
  }, []);
  const clearPicks = useCallback(() => {
    setPicked([]);
    setSelectMode(false);
  }, []);

  const onPress = useCallback(
    (a: Asset, e: MouseEvent) => {
      const L = live.current;
      if (e.shiftKey && anchor.current != null) {
        // Shift 点：从上次点的到这一个，全部选上
        const i = L.items.findIndex((x) => x.id === anchor.current);
        const j = L.items.findIndex((x) => x.id === a.id);
        if (i >= 0 && j >= 0) {
          const range = L.items.slice(Math.min(i, j), Math.max(i, j) + 1).map((x) => x.id);
          setKbdPick(false);
          setPicked((P) => [...P, ...range.filter((id) => !P.includes(id))]);
          return;
        }
      }
      anchor.current = a.id;
      if (modKey(e) || L.selecting) togglePick(a.id);
      else select(a.id);
    },
    [select, togglePick],
  );

  /** 双击：用默认程序打开（访达也是这样）；多选的时候双击只是选两下，不打开 */
  const onOpen = useCallback((a: Asset) => {
    if (!live.current.selecting) openFile(a);
  }, []);

  /* ---------------- 键盘（网格里） ---------------- */

  const onTileKey = useCallback(
    (a: Asset, e: KeyboardEvent<HTMLDivElement>) => {
      if (e.target !== e.currentTarget) return;
      const L = live.current;
      const i = L.items.findIndex((x) => x.id === a.id);
      const cols = gridColumns(gridRef.current);
      const step: Record<string, number> = { ArrowRight: 1, ArrowLeft: -1, ArrowDown: cols, ArrowUp: -cols };
      let next: number | null = null;
      if (e.key in step) next = Math.max(0, Math.min(L.items.length - 1, i + step[e.key]));
      else if (e.key === "Home") next = 0;
      else if (e.key === "End") next = L.items.length - 1;
      if (next != null) {
        e.preventDefault();
        const b = L.items[next];
        if (!b) return;
        // 键盘换选中：即时，不做任何动画（配方 1.10）；Shift+方向键顺手选上（多选）
        if (e.shiftKey) {
          setKbdPick(true);
          setPicked((P) => (P.includes(b.id) ? P : [...P, b.id]));
        }
        select(b.id, "nav");
        requestAnimationFrame(() => gridRef.current?.querySelector<HTMLElement>(`[data-id="${b.id}"]`)?.focus());
        return;
      }
      if (e.key === " ") {
        e.preventDefault();
        togglePick(a.id, true);
      } else if (e.key === "Enter") {
        e.preventDefault();
        if (L.selecting) togglePick(a.id, true);
        else select(a.id, "key");
      } else if (((e.key === "Backspace" || e.key === "Delete") && modKey(e)) || (!isMac && e.key === "Delete" && !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey)) {
        // macOS ⌘⌫；Windows、Linux 照文件管理器的习惯，单按 Delete 也是移到回收站（都先弹确认）
        e.preventDefault();
        if (a.exists) setTrashing(a);
        else askRemove(a);
      }
    },
    [select, togglePick], // eslint-disable-line react-hooks/exhaustive-deps
  );

  // 页面级的快捷键：⌘F 搜索、⌘A 全选（Windows、Linux 是 Ctrl，lib/os.ts 的 modKey）、Esc 退出多选 / 收起浮卡
  useEffect(() => {
    const f = (e: globalThis.KeyboardEvent) => {
      const el = e.target as HTMLElement | null;
      const typing = !!el?.closest("input, textarea, select, [contenteditable]");
      const inLayer = !!el?.closest("dialog, [popover], [role=menu]");
      if (modKey(e) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        searchRef.current?.focus();
        searchRef.current?.select();
      } else if (modKey(e) && e.key.toLowerCase() === "a" && !typing && !inLayer) {
        e.preventDefault();
        setKbdPick(true);
        setPicked(live.current.items.map((a) => a.id));
      } else if (e.key === "Escape" && !typing && !inLayer && !e.defaultPrevented) {
        if (live.current.picked.length || selectMode) clearPicks();
        else if (!live.current.wide) setSheet(false);
      }
    };
    addEventListener("keydown", f);
    return () => removeEventListener("keydown", f);
  }, [selectMode, clearPicks]);

  /* ---------------- 移除、移到废纸篓 ---------------- */

  /** 选中的那一条要被藏起来了：先换到下一条（和旧界面一样，下一条没有就上一条） */
  const stepAway = useCallback(
    (a: Asset) => {
      const L = live.current.items;
      const k = L.findIndex((x) => x.id === a.id);
      setPicked((P) => P.filter((id) => id !== a.id));
      if (live.current.selected?.id !== a.id) return;
      const next = L[k + 1] ?? L[k - 1];
      if (next) select(next.id, "nav");
      else setSheet(false);
    },
    [select],
  );
  const askRemove = useCallback(
    (a: Asset) => {
      stepAway(a);
      removeFromLibrary(a, () => select(a.id));
    },
    [stepAway, select],
  );
  const askTrash = useCallback((a: Asset) => setTrashing(a), []);
  const lastTrash = useRef<Asset | null>(null);
  if (trashing) lastTrash.current = trashing;
  const tj = lastTrash.current;

  /* ---------------- 处理：右键菜单里点「转换…」之类 ---------------- */

  const setTool = useCallback((tl: Tool) => {
    setToolState(tl);
    setPref("libTool", tl);
    setPref("lsec:tools", true);
  }, []);
  const openTool = useCallback(
    (a: Asset, tl: Tool) => {
      select(a.id);
      setTool(tl);
      setToolBump((n) => n + 1);
    },
    [select, setTool],
  );

  /* ---------------- 进度：后台任务（转换、字幕……，包括 AI 助手发起的）按素材画在卡片上 ---------------- */

  const progressOf = useMemo(() => {
    const m = new Map<number, number>();
    for (const task of Object.values(tasks)) if (task.st === "run" && task.assetId != null) m.set(task.assetId, Math.max(m.get(task.assetId) ?? 0, task.pct));
    for (const [k, op] of Object.entries(ops)) {
      const id = Number(k.split(":")[0]);
      if (op.st === "queued" && !m.has(id)) m.set(id, -1);
      if (op.st === "run" && !m.has(id)) m.set(id, op.p);
    }
    return m;
  }, [tasks, ops]);

  /* ---------------- 顶栏 ---------------- */

  const setF = (patch: Partial<Filters>) => setFilters((f) => ({ ...f, ...patch }));
  const clearFilters = () => {
    setQText("");
    setFilters({ ...NO_FILTERS });
  };
  const sub = !lib.loaded ? "" : anyFilter(filters) ? t("{total} / {n}项", { total, n: all }) : t("{n}项", { n: all });

  // 站点名是存下来的中文原文（「哔哩哔哩」「本地」），显示时 tAny；筛选条件里照旧存原文
  const filterItems: DropdownMenuItem[] = useMemo(() => {
    const top = lib.sites.slice(0, 7);
    const rest = lib.sites.slice(7);
    const siteItem = ([s, n]: [string, number]): DropdownMenuItem => ({
      label: tAny(s),
      description: t("{n}项", { n }),
      logo: <SiteIcon site={siteKeyOf(s)} size={14} />,
      checked: filters.site === s,
      onSelect: () => setF({ site: filters.site === s ? "" : s }),
    });
    const L: DropdownMenuItem[] = [{ label: t("全部来源"), checked: !filters.site, onSelect: () => setF({ site: "" }) }, ...top.map(siteItem)];
    if (rest.length) L.push({ label: t("更多来源"), description: rest.some(([s]) => s === filters.site) ? tAny(filters.site) : undefined, submenu: rest.map(siteItem) });
    DAYS.forEach((d, i) => L.push({ label: t(d.label), divider: i === 0, checked: filters.days === d.v, onSelect: () => setF({ days: d.v }) }));
    if (lib.tags.length)
      L.push({
        label: t("标签"),
        description: filters.tag || t("全部"),
        divider: true,
        submenu: [
          { label: t("全部标签"), checked: !filters.tag, onSelect: () => setF({ tag: "" }) },
          ...lib.tags.slice(0, 40).map(([tag, n]) => ({ label: tag, description: t("{n}项", { n }), checked: filters.tag === tag, onSelect: () => setF({ tag }) })),
        ],
      });
    if (extraFilters(filters)) L.push({ label: t("清除筛选"), icon: Cancel01Icon, divider: true, onSelect: () => setF({ site: "", days: 0, tag: "" }) });
    return L;
  }, [lib.sites, lib.tags, filters]); // eslint-disable-line react-hooks/exhaustive-deps

  const topbar = (
    <Topbar
      title={t("素材库")}
      sub={sub}
      mid={
        <SegmentedControl
          aria-label={t("素材类型")}
          size="sm"
          value={filters.kind || "all"}
          onChange={(v) => setF({ kind: (v === "all" ? "" : v) as KindFilter })}
          options={[
            { value: "all", label: t("全部") },
            { value: "video", label: t("视频§顶栏按类型筛选的分段控件") },
            { value: "audio", label: t("音频§顶栏按类型筛选的分段控件") },
            { value: "image", label: t("图片§顶栏按类型筛选的分段控件") },
          ]}
        />
      }
      end={
        <>
          <TextField
            ref={searchRef}
            size="sm"
            type="search"
            aria-label={t("搜索素材")}
            placeholder={t("搜标题、标签、字幕里的话")}
            prefix={<Icon icon={Search01Icon} size={15} />}
            className="cp-libsearch lib-search"
            value={qText}
            clearable
            onChange={(e) => setQText(e.target.value)}
            onKeyDown={(e) => {
              // 搜索框里按 ↓：到网格里第一张
              if (e.key === "ArrowDown" && items.length) {
                e.preventDefault();
                const id = live.current.selected?.id ?? items[0].id;
                gridRef.current?.querySelector<HTMLElement>(`[data-id="${id}"]`)?.focus();
              }
            }}
          />
          <DropdownMenu
            align="end"
            aria-label={t("筛选素材")}
            items={filterItems}
            trigger={({ triggerProps }) => (
              <Button variant="ghost" size="sm" className="lib-filter" data-on={extraFilters(filters) || undefined} iconRight={<Icon icon={ArrowDown01Icon} size={14} />} {...triggerProps}>
                {filterLabel(filters)}
              </Button>
            )}
          />
          <Tooltip content={selecting ? t("退出多选") : t("多选：批量转换、拼接、加标签")}>
            <button
              className="cp-round lib-multi"
              aria-label={selecting ? t("退出多选") : t("多选")}
              aria-pressed={selecting}
              onClick={() => (selecting ? clearPicks() : setSelectMode(true))}
            >
              <Icon icon={CheckmarkSquare02Icon} size={16} />
            </button>
          </Tooltip>
        </>
      }
    />
  );

  /* ---------------- 网格 ---------------- */

  const numbered = picked.length > 1;
  const focusId = selected && items.some((a) => a.id === selected.id) ? selected.id : (items[0]?.id ?? null);
  const empty = emptyText(lib.all, filters);

  const grid = !lib.loaded ? (
    <GridSkeleton />
  ) : items.length ? (
    <ContextMenu
      aria-label={t("素材的操作")}
      onOpenChange={(open, el) => {
        // 右键点在哪一张上，就先选中哪一张（访达也是这样）
        const id = Number(el?.closest<HTMLElement>("[data-id]")?.dataset.id);
        if (open && id && !live.current.selecting) select(id);
      }}
      items={(el) => {
        const id = Number(el.closest<HTMLElement>("[data-id]")?.dataset.id);
        const a = live.current.items.find((x) => x.id === id);
        if (!a) return [];
        return assetMenu(a, { tool: openTool, pick: (x) => togglePick(x.id), picked: live.current.picked.includes(a.id), askTrash, askRemove });
      }}
    >
      <div ref={gridRef} className="cp-grid lib-grid" role="listbox" aria-label={t("素材")} aria-multiselectable={selecting} data-selecting={selecting || undefined} data-kbd={kbdPick || undefined}>
        {items.map((a) => (
          <Tile
            key={a.id}
            a={a}
            selected={selecting ? picked.includes(a.id) : selected?.id === a.id}
            pick={picked.indexOf(a.id) + 1}
            numbered={numbered}
            progress={progressOf.get(a.id) ?? null}
            focusable={a.id === focusId}
            onPress={onPress}
            onPick={(x) => togglePick(x.id)}
            onOpen={onOpen}
            onKey={onTileKey}
          />
        ))}
      </div>
    </ContextMenu>
  ) : (
    <div className="cp-listempty">
      <EmptyState
        bare
        tall
        icon={lib.all || anyFilter(filters) ? Search01Icon : Album02Icon}
        title={empty.title}
        text={empty.text}
        action={
          empty.clear ? (
            <Button variant="secondary" size="sm" className="cp-pill" onClick={clearFilters}>
              {t("清除筛选和搜索")}
            </Button>
          ) : !lib.all ? (
            <Button variant="secondary" size="sm" className="cp-pill" icon={<Icon icon={Download04Icon} size={14} />} onClick={() => go("downloads")}>
              {t("去下载")}
            </Button>
          ) : undefined
        }
      />
    </div>
  );

  const detail = (onClose?: () => void) =>
    selected && (
      <Detail
        a={selected}
        instant={instant}
        tool={isMedia(selected) ? tool : "convert"}
        onTool={setTool}
        onAskTrash={askTrash}
        onAskRemove={askRemove}
        onOpenAsset={selectById}
        openTools={toolBump}
        onClose={onClose}
      />
    );

  return (
    <main ref={mainRef} className={`cp-main cp-lib${wide && selected ? "" : " solo"}`}>
      <div className="cp-listwrap">
        <div className="cp-col lib-col" ref={colRef} onScroll={fill} data-batch={selecting || undefined}>
          {topbar}
          {grid}
          {hasMore && (
            <div className="lib-more" aria-live="polite">
              {moreSlow && <ShimmerText className="cp-wait">{t("正在读取更多")}</ShimmerText>}
            </div>
          )}
        </div>
        <div className="lib-batch-wrap">
          <AnimatePresence>{selecting && <BatchBar key="batch" picked={pickedAssets} tags={lib.tags} onClear={clearPicks} onDone={clearPicks} />}</AnimatePresence>
        </div>
      </div>

      {selected && wide && (
        <aside className="cp-detail" aria-label={t("素材详情")}>
          {detail()}
        </aside>
      )}
      {!wide && (
        <AnimatePresence>
          {sheet && selected && (
            // 窄窗口的详情浮卡：从右边滑进来 200ms ease-out-quart，收起 150ms（配方 e1，和下载页同样的数值）。
            // 打开的同时详情在画图、读波形，主线程忙：写成 transform 字符串走合成层（课程 04/02），不用 Motion 的 x；
            // 字符串 transform 不归 MotionConfig 管，减少动效时自己换成只淡入淡出
            <motion.aside
              key="sheet"
              className="cp-detail float"
              aria-label={t("素材详情")}
              initial={reduce ? { opacity: 0 } : { transform: "translateX(110%)", opacity: 0 }}
              animate={{ transform: "translateX(0%)", opacity: 1, transition: { duration: 0.2, ease: [0.165, 0.84, 0.44, 1] } }}
              exit={reduce ? { opacity: 0, transition: { duration: 0.15, ease: EASE } } : { transform: "translateX(110%)", opacity: 0, transition: { duration: 0.15, ease: EASE } }}
            >
              {detail(() => setSheet(false))}
            </motion.aside>
          )}
        </AnimatePresence>
      )}

      <AlertDialog
        className="cp"
        open={!!trashing}
        onOpenChange={(o) => !o && setTrashing(null)}
        title={t("把这个文件移到{trash}？", { trash: t(TRASH) })}
        description={tj?.title}
        confirmLabel={moveToTrash()}
        cancelLabel={t("先不删")}
        danger
        onConfirm={() => {
          const a = tj;
          if (!a) return;
          stepAway(a);
          trash(a, () => select(a.id));
        }}
      >
        <p className="cp-dlg-p">
          {t("文件会移到{trash}，素材库里也不再显示。几秒内可以撤销，之后想找回的话，可以去{trash}里拿。", { trash: t(TRASH) })}
          {tj && <span className="lib-dlg-path">{home(tj.path)}</span>}
        </p>
      </AlertDialog>
    </main>
  );
}
