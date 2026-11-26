// 「浏览器里发现的」：全局状态 sniffed（插件在网页里找到的视频、声音、图片、M3U8，最多 50 条；侧栏只放最近 3 条视频和声音，全部在这里）。
// 点「下载」交给 shell/actions.ts 的 downloadSniffed（带上来源页面和请求头，跳到下载页）。
// 动效：新发现的一条插在最上面，进场照下载列表（opacity + 上方 8px，弹簧 0.4s 不回弹，其余行让位，配方 c1）；
// 「不要了」退场 opacity + scale .95、150ms（c3）；换种类不播（功能性数据不重播）。
// 「插件能做什么」：静态说明。
import { ClipboardIcon, Copy01Icon, Delete02Icon, Download04Icon, Globe02Icon, Layers01Icon, LinkSquare02Icon, MoreHorizontalIcon, PlayListIcon, SearchList01Icon } from "@hugeicons/core-free-icons";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { memo, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { DropdownMenu } from "@/components/ui/dropdown-menu";
import { EmptyState } from "@/components/ui/empty-state";
import { IconButton } from "@/components/ui/icon-button";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { N_, t, type Key } from "@/i18n";
import { openUrl } from "@/lib/bridge";
import { whenLabel } from "@/lib/format";
import { useMinute } from "@/lib/hooks";
import { takeSniffed, useApp } from "@/lib/store";
import type { Sniffed } from "@/lib/types";
import { Icon, type HugeIcon } from "@/parts";
import { copyWithToast, downloadSniffed } from "@/shell/actions";
import { kindOf, sniffIcon, type Kind } from "./model";

const SPRING = { type: "spring", duration: 0.4, bounce: 0 } as const;
const EXIT = { duration: 0.15, ease: [0.23, 1, 0.32, 1] } as const;

export function Found() {
  const sniffed = useApp((s) => s.sniffed);
  const [kind, setKind] = useState<Kind>("all");
  const minute = useMinute(sniffed.length > 0);
  const counts = useMemo(() => {
    const c = { video: 0, audio: 0, image: 0 };
    for (const s of sniffed) c[kindOf(s)]++;
    return c;
  }, [sniffed]);
  const list = kind === "all" ? sniffed : sniffed.filter((s) => kindOf(s) === kind);

  return (
    <section className="cp-sec">
      <div className="cp-sec-h">
        <h2>{t("浏览器里发现的")}</h2>
        <p>{t("在浏览器里看视频时插件找到的，点「下载」交给 Captae")}</p>
        {sniffed.length > 0 && (
          <span className="end">
            <SegmentedControl
              aria-label={t("种类")}
              size="sm"
              value={kind}
              onChange={(v) => setKind(v as Kind)}
              options={[
                { value: "all", label: t("全部"), count: sniffed.length },
                { value: "video", label: t("视频"), count: counts.video || undefined, disabled: !counts.video },
                { value: "audio", label: t("声音"), count: counts.audio || undefined, disabled: !counts.audio },
                { value: "image", label: t("图片"), count: counts.image || undefined, disabled: !counts.image },
              ]}
            />
          </span>
        )}
      </div>
      {list.length ? (
        <ul className="cp-pane cp-found" aria-label={t("浏览器里发现的")}>
          <AnimatePresence key={kind} mode="popLayout" initial={false}>
            {list.map((s) => (
              <FoundRow key={s.url} s={s} minute={minute} layoutOn={list.length <= 40} />
            ))}
          </AnimatePresence>
        </ul>
      ) : (
        <div className="cp-pane cp-pane-empty">
          <EmptyState bare icon={SearchList01Icon} title={t("还没有发现能下载的")} text={t("在浏览器里看视频时，插件找到的视频、声音和图片会出现在这里，点一下就能下载")} />
        </div>
      )}
    </section>
  );
}

/** minute：每分钟变一次，让「5 分钟前」跟着走；layoutOn：行太多时关掉让位动画（布局动画在主线程，配方 c） */
const FoundRow = memo(function FoundRow({ s, layoutOn }: { s: Sniffed; minute: number; layoutOn: boolean }) {
  const reduce = useReducedMotion();
  const page = /^https?:\/\//.test(s.page_url) ? s.page_url : "";
  return (
    <motion.li
      layout={layoutOn ? "position" : false}
      className="cp-found-row"
      initial={{ opacity: 0, y: reduce ? 0 : -8 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, scale: reduce ? 1 : 0.95, transition: EXIT }}
      transition={SPRING}
    >
      <span className="ico" aria-hidden>
        <Icon icon={sniffIcon(s)} size={15} />
      </span>
      <div className="main">
        <div className="t" title={s.url}>
          {s.t}
        </div>
        <div className="s" title={s.page_title ? t("来自「{title}」", { title: s.page_title }) : undefined}>
          {[s.m, s.host, whenLabel(s.at)].filter(Boolean).join(" · ")}
          {s.page_title ? " · " + t("来自「{title}」", { title: s.page_title }) : ""}
        </div>
      </div>
      <div className="acts">
        <Button size="sm" variant="secondary" className="cp-pill" icon={<Icon icon={Download04Icon} size={14} />} onClick={() => downloadSniffed(s.url)}>
          {t("下载")}
        </Button>
        <DropdownMenu
          align="end"
          aria-label={t("「{title}」的更多操作", { title: s.t })}
          items={[
            { label: t("复制链接"), icon: Copy01Icon, onSelect: () => copyWithToast(s.url, t("链接已复制")) },
            ...(page ? [{ label: t("打开来源网页"), icon: Globe02Icon, onSelect: () => openUrl(page).catch(() => undefined) }] : []),
            { label: t("不要了"), icon: Delete02Icon, divider: true, onSelect: () => takeSniffed(s.url) },
          ]}
          trigger={({ triggerProps }) => <IconButton icon={MoreHorizontalIcon} label={t("更多")} size="sm" title="" {...triggerProps} />}
        />
      </div>
    </motion.li>
  );
});

/* ---------------- 插件能做什么 ---------------- */

const FEATS: [HugeIcon, Key, Key][] = [
  [Download04Icon, N_("视频上多两个按钮"), N_("「下载」选清晰度存到电脑上，「发给 AI」整理好文字稿和关键画面")],
  [PlayListIcon, N_("工具栏里列出这个页面能下载的"), N_("视频、声音、图片，还有网页里的在线视频（M3U8）")],
  [ClipboardIcon, N_("复制视频链接时问一句"), N_("要不要用 Captae 下载，下过的会告诉你")],
  [LinkSquare02Icon, N_("右键菜单"), N_("在链接、视频、图片上点右键「用 Captae 下载」")],
  [Layers01Icon, N_("发现的都会出现在 Captae 里"), N_("左边的「浏览器里发现的」和这一页，点一下就能下载")],
];

export function Features() {
  return (
    <section className="cp-sec">
      <div className="cp-sec-h">
        <h2>{t("插件能做什么")}</h2>
      </div>
      <ul className="cp-pane cp-group cp-feats">
        {FEATS.map(([icon, title, desc]) => (
          <li key={title}>
            <Icon icon={icon} size={16} />
            <div>
              <b>{t(title)}</b>
              <small>{t(desc)}</small>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}
