// 侧栏底部的上手清单（在「AI 助手」那一行上面）：一张浅白小卡，标题 + 进度环 + n/4 + 关闭，下面四行。
// 照第二版的克制：不用彩色块，只有进度环用强调色；完成的那行是深色的圈和勾、字变灰。
// 每一行点了去对应的地方；打勾只看真实结果（state.ts 的 useAutoTick）。
//
// 动效（配方 e3 / m1、1.10「很少、第一次」）：
// - 刚完成的那一项：画圈 250ms + 打勾 200ms（勾在圈结束前 100ms 开始，ease-in-out-cubic，theme.css 的 .cp-draw）；本来就完成的直接显示终态
// - 进度环：在屏幕上变化，用 ease-in-out-cubic 400ms 走到新位置
// - 整块出现、收起：高度 + 透明度（parts 的 Collapse，0.27s 课 05/04 的高度曲线，收起 0.2s），下面的侧栏跟着让位，不会一跳
// - 悬停和侧栏其他行一样即时，不过渡（配方 a）；减少动效时不画线、不走环、不变高度
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { useRef } from "react";
import { IconButton } from "@/components/ui/icon-button";
import { t } from "@/i18n";
import { notify } from "@/lib/notify";
import { go } from "@/lib/route";
import { Collapse } from "@/parts";
import { doneCount, hideChecklist, ITEMS, useAutoTick, useOb, type ItemKey } from "./state";
import "./onboarding.css";

/** 每一项点了去哪：下载页、素材库、插件页、AI 助手页 */
function goTo(k: ItemKey) {
  if (k === "dl") go("downloads");
  if (k === "convert") {
    go("library");
    notify(t("转换一次格式"), t("在素材库里点开一个视频，在「处理 › 转换」里选一个格式"));
  }
  if (k === "ext") go("extension");
  if (k === "agent") go("agent");
}

export function Checklist() {
  useAutoTick();
  const ob = useOb((s) => s.ob);
  const finishing = useOb((s) => s.finishing);
  const n = doneCount(ob);
  // 全部完成以后先停一下（finishing），让最后一个勾画完、看到 4/4，再收起
  const show = !ob.hidden && (n < ITEMS.length || finishing);
  return (
    <Collapse open={show}>
      <section className="cp-ck" aria-label={t("上手清单")}>
        <div className="cp-ck-head">
          <Ring p={n / ITEMS.length} />
          <b>{t("上手清单")}</b>
          <span className="num" aria-label={t("已完成{n}项，共{total}项", { n, total: ITEMS.length })}>
            {n}/{ITEMS.length}
          </span>
          <IconButton icon={Cancel01Icon} label={t("关闭上手清单")} size="sm" onClick={hideChecklist} />
        </div>
        <ul>
          {ITEMS.map((it) => (
            <Row key={it.k} k={it.k} label={t(it.label)} done={!!ob.items[it.k]} />
          ))}
        </ul>
      </section>
    </Collapse>
  );
}

function Row({ k, label, done }: { k: ItemKey; label: string; done: boolean }) {
  // 挂上来以后才变成完成的，才画圈打勾；一打开就已经完成的直接显示终态
  const doneAtMount = useRef(done);
  const fresh = done && !doneAtMount.current;
  return (
    <li>
      <button className="cp-ck-row" data-done={done || undefined} onClick={() => goTo(k)}>
        <svg className="glyph" viewBox="0 0 18 18" fill="none" aria-hidden>
          {done ? (
            <>
              <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5" pathLength={100} className={`cp-draw${fresh ? " go" : ""}`} />
              <path d="M5.9 9.2l2.1 2.1 4.1-4.4" stroke="currentColor" strokeWidth="1.7" strokeLinecap="round" strokeLinejoin="round" pathLength={100} className={`cp-draw${fresh ? " go late" : ""}`} />
            </>
          ) : (
            <circle cx="9" cy="9" r="7.25" stroke="currentColor" strokeWidth="1.5" />
          )}
        </svg>
        <span className="t">{label}</span>
        {done && <span className="cp-ob-sr">{t("已完成")}</span>}
      </button>
    </li>
  );
}

/** 标题前的小进度环（和侧栏「进行中」的环同样大小、同样的蓝）。一项都没完成时不画蓝色，圆头线帽会在 0 长度时露出一个点 */
function Ring({ p }: { p: number }) {
  const v = Math.max(0, Math.min(1, p));
  return (
    <svg className="cp-ck-ring" width="14" height="14" viewBox="0 0 16 16" aria-hidden>
      <circle cx="8" cy="8" r="6" fill="none" stroke="rgba(0,0,0,.12)" strokeWidth="2" />
      <circle cx="8" cy="8" r="6" fill="none" stroke="var(--accent)" strokeWidth="2" strokeLinecap="round" pathLength={100} style={{ strokeDasharray: `${v * 100} 100`, opacity: v > 0 ? 1 : 0 }} />
    </svg>
  );
}
