// 一条素材的操作菜单：网格上的右键菜单和详情里的「更多」用同一份（affectUI 的 ContextMenu 和 DropdownMenu 的条目同形）。
// 右键是藏起来的手势，这里的每一项在详情里都有看得见的入口（按钮、更多菜单、处理分节）。
import {
  AiBrain01Icon,
  CheckmarkSquare02Icon,
  ClosedCaptionIcon,
  Copy01Icon,
  Delete02Icon,
  Download04Icon,
  Exchange01Icon,
  Folder01Icon,
  Globe02Icon,
  Link01Icon,
  ScissorIcon,
  SquareArrowUpRight02Icon,
  Cancel01Icon,
} from "@hugeicons/core-free-icons";
import type { DropdownMenuItem } from "@/components/ui/dropdown-menu";
import { t } from "@/i18n";
import { copyLink, copyPath, moveToTrash, openFile, openSource, redownload, REVEAL, revealFile, shareToAI } from "./act";
import { isImage, isMedia, sourceUrl, type Asset } from "./model";

export type Tool = "convert" | "subtitle" | "trim";

export type MenuHandlers = {
  /** 选中这一条并打开详情里的某个处理 */
  tool?: (a: Asset, t: Tool) => void;
  /** 多选：选上 / 取消 */
  pick?: (a: Asset) => void;
  picked?: boolean;
  askTrash: (a: Asset) => void;
  askRemove: (a: Asset) => void;
};

// 菜单是打开时才生成的（右键那一刻、详情渲染时），文字直接按当前语言给
export function assetMenu(a: Asset, h: MenuHandlers): DropdownMenuItem[] {
  const url = sourceUrl(a);
  const src: DropdownMenuItem[] = url
    ? [
        { label: t("复制来源链接"), icon: Link01Icon, divider: true, onSelect: () => copyLink(a) },
        { label: t("在浏览器里打开来源"), icon: Globe02Icon, onSelect: () => openSource(a) },
      ]
    : [];
  const pick: DropdownMenuItem[] = h.pick ? [{ label: h.picked ? t("取消选择") : t("选择"), icon: h.picked ? Cancel01Icon : CheckmarkSquare02Icon, divider: true, onSelect: () => h.pick!(a) }] : [];

  // 文件已经不在了：能做的只有重新下载、看来源、从库里去掉这条记录
  if (!a.exists)
    return [
      ...(url ? [{ label: t("重新下载"), icon: Download04Icon, onSelect: () => redownload(a) }, ...src] : []),
      ...pick,
      { label: t("从素材库移除"), icon: Delete02Icon, danger: true, divider: !!url || !!pick.length, onSelect: () => h.askRemove(a) },
    ];

  const tools: DropdownMenuItem[] = h.tool
    ? [
        { label: t("转换格式…"), icon: Exchange01Icon, divider: true, onSelect: () => h.tool!(a, "convert") },
        ...(isMedia(a)
          ? [
              { label: t("字幕…"), icon: ClosedCaptionIcon, onSelect: () => h.tool!(a, "subtitle") },
              { label: t("剪辑…"), icon: ScissorIcon, onSelect: () => h.tool!(a, "trim") },
            ]
          : []),
      ]
    : [];

  return [
    { label: t("用默认程序打开"), icon: SquareArrowUpRight02Icon, onSelect: () => openFile(a) },
    { label: t(REVEAL), icon: Folder01Icon, onSelect: () => revealFile(a) },
    ...(!isImage(a) ? [{ label: t("发给 AI"), description: t("文字稿和关键帧放进剪贴板"), icon: AiBrain01Icon, onSelect: () => shareToAI(a) }] : []),
    ...tools,
    ...src,
    { label: t("复制文件路径"), icon: Copy01Icon, divider: !url, onSelect: () => copyPath(a) },
    ...pick,
    { label: t("从素材库移除"), description: t("文件留在原处"), icon: Cancel01Icon, divider: true, onSelect: () => h.askRemove(a) },
    { label: moveToTrash(), icon: Delete02Icon, danger: true, onSelect: () => h.askTrash(a) },
  ];
}
