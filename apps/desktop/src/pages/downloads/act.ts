// 下载页的操作：任务行的「更多」菜单、失败时的主按钮、移除。行和右侧详情共用，行为一致。
import { Album02Icon, ArrowDown01Icon, ArrowUp01Icon, Cancel01Icon, Copy01Icon, Delete02Icon, Folder01Icon, Globe02Icon, PlayIcon, RepeatIcon, StopIcon } from "@hugeicons/core-free-icons";
import type { DropdownMenuItem } from "@/components/ui/dropdown-menu";
import { t } from "@/i18n";
import { errText, invoke, openUrl, reveal } from "@/lib/bridge";
import { notify } from "@/lib/notify";
import { REVEAL } from "@/lib/os";
import { go } from "@/lib/route";
import { addJob, jobAction } from "@/lib/store";
import type { Job } from "@/lib/types";
import { copyWithToast, openDialog, openLogin } from "@/shell/actions";
import { errorText, recBusy, recording, type FailAction, type View } from "./model";

/** 用户点过「重试」的任务：再失败时状态块抖一次（配方 b3） */
export const retried = new Set<number>();

export function retry(j: Job) {
  retried.add(j.id);
  jobAction(j.id, "retry");
}

export function revealJob(j: Job) {
  reveal(j.path).catch((e) => notify(t("打不开文件夹"), errText(e)));
}

/** 录直播「停止并保存」：录制的进程收尾，录到的部分整理好放进素材库（后端 JobAction::Stop）。
 *  store.ts 的 jobAction 类型里没有 stop，这里直接调 job_action */
export function stopRecording(j: Job) {
  invoke<void>("job_action", { id: j.id, action: "stop" }).catch((e) => notify(t("没有停下来"), errText(e)));
}

/** title 不给时是「已移除」（按当前语言，调用时才翻译） */
export function removeJob(j: Job, title?: string, desc?: string) {
  jobAction(j.id, "remove").then((ok) => ok && notify(title ?? t("已移除"), desc ?? j.title));
}

/** 失败时的主按钮做的事（按钮文字见 model.failInfo） */
export function runFail(j: Job, a: FailAction) {
  switch (a) {
    case "login":
    case "verify":
      openLogin(j, a);
      break;
    case "retry":
      retry(j);
      break;
    case "open_page":
      openUrl(j.url).catch((e) => notify(t("打不开网页"), errText(e)));
      break;
    case "record_live":
      addJob({ url: j.url, record_live: true, origin: "ui" })
        .then(() => jobAction(j.id, "remove"))
        .catch((e) => notify(t("没有开始录"), errText(e)));
      break;
    case "change_dir":
      invoke<string | null>("choose_save_dir")
        .then((d) => d && retry(j))
        .catch((e) => notify(t("没有换成§换保存位置没有成功"), errText(e)));
      break;
    case "settings":
      openDialog("settings");
      break;
  }
}

/** 任务行右边「更多」菜单的内容 */
export function rowMenu(j: Job, view: View, askCancel: (j: Job) => void): DropdownMenuItem[] {
  const copyLink: DropdownMenuItem = { label: t("复制链接"), icon: Copy01Icon, onSelect: () => copyWithToast(j.url, t("链接已复制")) };
  const openPage: DropdownMenuItem = { label: t("在浏览器里打开"), icon: Globe02Icon, onSelect: () => openUrl(j.url).catch(() => undefined) };
  if (j.state === "done")
    return [
      { label: t(REVEAL), icon: Folder01Icon, onSelect: () => revealJob(j) },
      ...(j.assetId != null ? [{ label: t("在素材库查看"), icon: Album02Icon, onSelect: () => go("library", { asset: j.assetId! }) }] : []),
      { ...copyLink, divider: true },
      openPage,
      { label: t("从列表里移除"), icon: Delete02Icon, divider: true, onSelect: () => removeJob(j, t("已从列表里移除"), t("文件还在，素材库里也还有")) },
    ];
  if (j.state === "failed")
    return [
      { label: t("重试"), icon: RepeatIcon, onSelect: () => retry(j) },
      { label: t("复制错误信息"), icon: Copy01Icon, onSelect: () => copyWithToast(errorText(j), t("错误信息已复制"), t("遇到问题时可以发给我们")) },
      { ...copyLink, divider: true },
      openPage,
      { label: t("移除"), icon: Delete02Icon, danger: true, divider: true, onSelect: () => removeJob(j) },
    ];
  if (view === "choose") return [copyLink, openPage, { label: t("移除"), icon: Delete02Icon, danger: true, divider: true, onSelect: () => removeJob(j) }];
  // 录直播：不排先后、不暂停；「取消录制」会删掉录到的（先确认）
  if (recBusy(j))
    return [
      ...(recording(j) ? [{ label: t("停止并保存"), icon: StopIcon, onSelect: () => stopRecording(j) }] : []),
      { ...copyLink, divider: recording(j) },
      openPage,
      { label: t("取消录制"), icon: Cancel01Icon, danger: true, divider: true, onSelect: () => askCancel(j) },
    ];
  return [
    ...(j.state === "queue" && !j.boost ? [{ label: t("马上开始"), icon: PlayIcon, onSelect: () => jobAction(j.id, "now") }] : []),
    { label: t("往前排"), icon: ArrowUp01Icon, onSelect: () => jobAction(j.id, "up") },
    { label: t("往后排"), icon: ArrowDown01Icon, onSelect: () => jobAction(j.id, "down") },
    { ...copyLink, divider: true },
    openPage,
    { label: t("取消下载"), icon: Cancel01Icon, danger: true, divider: true, onSelect: () => askCancel(j) },
  ];
}
