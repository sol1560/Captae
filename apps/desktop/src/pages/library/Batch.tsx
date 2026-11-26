// 多选：网格底部浮着的工具条（磨砂玻璃）——已选几项、转成 MP4、拼接、加标签、完成；以及拼接的确认弹窗。
// 工具条进出照课程 03/06 的工具栏：从下面 20px 浮上来 + 淡入，弹簧 0.3s、不回弹；退场原路回去。
// 工具条本身是磨砂玻璃，不给它加 blur 滤镜（filter 会让 backdrop-filter 失效，也不给 backdrop-filter 做动画，BRIEF-v2）。
import { ArrowDown01Icon, ArrowUp01Icon, Exchange01Icon, GitMergeIcon, Tag01Icon } from "@hugeicons/core-free-icons";
import { motion, useReducedMotion } from "motion/react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Popover } from "@/components/ui/popover";
import { TextField } from "@/components/ui/text-field";
import { Tooltip } from "@/components/ui/tooltip";
import { t, tx } from "@/i18n";
import { fileSrc } from "@/lib/bridge";
import { clock } from "@/lib/format";
import { Icon, TipIcon } from "@/parts";
import { concat, convertMany, tagMany } from "./act";
import { concatable, concatInfo, needMp4, type Asset } from "./model";

export function BatchBar({ picked, tags, onClear, onDone }: { picked: Asset[]; tags: [string, number][]; onClear: () => void; onDone: () => void }) {
  const reduce = useReducedMotion();
  const [catOpen, setCatOpen] = useState(false);
  const [tagOpen, setTagOpen] = useState(false);
  const vids = concatable(picked);
  const toMp4 = needMp4(picked);
  const n = picked.length;
  // 用字符串写 transform（硬件加速，课程 04/02）；减少动效时只淡入淡出
  const off = reduce ? { opacity: 0 } : { opacity: 0, transform: "translateY(20px)" };

  const startConvert = () => {
    const videos = picked.filter((a) => a.type === "video" && a.exists);
    // 两句独立的说明，各自翻译，中间按语言用「，」或「, 」隔开
    const notes = [
      picked.length - videos.length ? t("{n}项不是视频，跳过了", { n: picked.length - videos.length }) : "",
      videos.length - toMp4.length ? t("{n}个已经是 MP4", { n: videos.length - toMp4.length }) : "",
    ]
      .filter(Boolean)
      .join(t("，§两句说明之间的分隔"));
    convertMany(toMp4, notes);
    onDone();
  };

  return (
    <>
      <motion.div
        className="lib-batch"
        role="toolbar"
        aria-label={t("多选的操作")}
        initial={off}
        animate={{ opacity: 1, transform: "translateY(0px)" }}
        exit={off}
        transition={{ type: "spring", duration: 0.3, bounce: 0 }}
      >
        <span className="n" aria-live="polite">
          {n ? tx("已选 <b>{n}</b> 项", { n, b: (s) => <b className="num">{s}</b> }) : t("点素材来选")}
        </span>
        <Tooltip content={toMp4.length ? t("{n}个视频会一个接一个转", { n: toMp4.length }) : t("选中的视频都已经是 MP4")}>
          <Button size="sm" variant="ghost" icon={<Icon icon={Exchange01Icon} size={14} />} disabled={!toMp4.length} onClick={startConvert}>
            {t("转成 MP4")}
          </Button>
        </Tooltip>
        <Tooltip content={vids.length < 2 ? t("选两段以上的视频") : t("按选中的先后顺序拼成一个视频")}>
          <Button size="sm" variant="ghost" icon={<Icon icon={GitMergeIcon} size={14} />} disabled={vids.length < 2} onClick={() => setCatOpen(true)}>
            {t("拼接")}
          </Button>
        </Tooltip>
        <span className="lib-anchor">
          <Button size="sm" variant="ghost" icon={<Icon icon={Tag01Icon} size={14} />} disabled={!n} aria-haspopup="dialog" aria-expanded={tagOpen} onClick={() => setTagOpen((o) => !o)}>
            {t("加标签")}
          </Button>
          <TagPopover open={tagOpen} n={n} tags={tags} onClose={() => setTagOpen(false)} onPick={(tag) => tagMany(picked, tag).then((ok) => ok && onDone())} />
        </span>
        <span className="sep" aria-hidden />
        <Button size="sm" variant="secondary" className="cp-pill" onClick={onClear}>
          {t("完成")}
        </Button>
      </motion.div>
      <ConcatDialog
        open={catOpen}
        initial={vids}
        skipped={picked.length - vids.length}
        onClose={() => setCatOpen(false)}
        onGo={(L) => {
          setCatOpen(false);
          concat(L);
          onDone();
        }}
      />
    </>
  );
}

/** 给选中的几项加同一个标签：输入新的，或者点一个常用的 */
function TagPopover({ open, n, tags, onClose, onPick }: { open: boolean; n: number; tags: [string, number][]; onClose: () => void; onPick: (tag: string) => void }) {
  const [text, setText] = useState("");
  const go = (tag: string) => {
    if (!tag.trim()) return;
    onPick(tag.trim());
    setText("");
    onClose();
  };
  return (
    <Popover open={open} onClose={onClose} side="top" align="center" width={280} title={t("给{n}项加标签", { n })} className="cp lib-tagpop">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          go(text);
        }}
      >
        <TextField size="sm" aria-label={t("标签名")} placeholder={t("新标签，回车加上")} value={text} maxLength={24} onChange={(e) => setText(e.target.value)} />
      </form>
      {tags.length > 0 && (
        <div className="lib-sugg" aria-label={t("常用的标签")}>
          {tags.slice(0, 10).map(([tag]) => (
            <button key={tag} type="button" className="lib-chip" onClick={() => go(tag)}>
              {tag}
            </button>
          ))}
        </div>
      )}
    </Popover>
  );
}

/** 拼接确认：按选中的先后顺序，可以上下调；画面大小一样时直接接起来，不一样时以第一段为准重新编码 */
function ConcatDialog({ open, initial, skipped, onClose, onGo }: { open: boolean; initial: Asset[]; skipped: number; onClose: () => void; onGo: (L: Asset[]) => void }) {
  const [list, setList] = useState<Asset[]>(initial);
  const [wasOpen, setWasOpen] = useState(open);
  // 每次打开都从当前选中的顺序开始
  if (open !== wasOpen) {
    setWasOpen(open);
    if (open) setList(initial);
  }
  const { total, sameRes } = concatInfo(list);
  const move = (k: number, d: number) =>
    setList((L) => {
      const j = k + d;
      if (j < 0 || j >= L.length) return L;
      const next = L.slice();
      [next[k], next[j]] = [next[j], next[k]];
      return next;
    });
  return (
    <Dialog
      className="cp"
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={t("拼成一个视频")}
      description={t("按下面的顺序接起来，一共{time}", { time: clock(total) })}
      closeLabel={t("关闭")}
      footer={
        <>
          <Button variant="secondary" className="cp-pill" onClick={onClose}>
            {t("取消")}
          </Button>
          <Button className="cp-pill" icon={<Icon icon={GitMergeIcon} size={14} />} onClick={() => onGo(list)}>
            {t("开始拼接")}
          </Button>
        </>
      }
    >
      <ol className="lib-cat">
        {list.map((a, k) => (
          // 上下调顺序：让位用布局动画（偶尔用、能看出谁和谁换了位置）；减少动效时 MotionConfig 会关掉
          <motion.li key={a.id} layout="position" transition={{ type: "spring", duration: 0.3, bounce: 0 }}>
            <span className="k num">{k + 1}</span>
            <span className="th">{a.thumb ? <img src={fileSrc(a.thumb)} alt="" /> : null}</span>
            <span className="t">{a.title}</span>
            <span className="d num">{a.dur}</span>
            <TipIcon size="sm" icon={ArrowUp01Icon} label={t("往前挪")} disabled={k === 0} onClick={() => move(k, -1)} />
            <TipIcon size="sm" icon={ArrowDown01Icon} label={t("往后挪")} disabled={k === list.length - 1} onClick={() => move(k, 1)} />
          </motion.li>
        ))}
      </ol>
      <p className="cp-dnote lib-catnote">
        {sameRes ? t("画面大小一样，会直接接起来，很快；原来的几段都不变。") : t("画面大小不一样，会以第一段为准统一以后重新编码，要多等一会儿。")}
        {/* 独立的一句，前面空一格和上一句隔开 */}
        {skipped > 0 && " " + t("选中的里面有{n}项不是视频或文件不在了，不拼。", { n: skipped })}
      </p>
    </Dialog>
  );
}
