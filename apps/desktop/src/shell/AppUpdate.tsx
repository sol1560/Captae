// Captae 自己的更新在界面上的两处：侧栏的一行（下好了 / 有新版时才出现）、重新打开前的确认弹窗。
// 设置里的「关于 Captae」在 dialogs/Settings.tsx。操作在 lib/appUpdate.ts，后端在 src-tauri/src/app_update.rs。
// 侧栏这一行一个版本只出现一次（低频）：用 Collapse 展开（高度 270ms / 收起 200ms，减少动效时只淡入淡出，第一次挂载不播），
// 和上面的上手清单同一种出场（docs/MOTION.md 第 1.2、1.5 节）。
import { CircleArrowUp01Icon } from "@hugeicons/core-free-icons";
import { useEffect } from "react";
import { AlertDialog } from "@/components/ui/dialog";
import { t } from "@/i18n";
import { announceUpdated, downloadUpdate, interrupted, onUpdateStatus, requestRestart, restartNow, setRestartAsk, useRestartAsk } from "@/lib/appUpdate";
import { useApp } from "@/lib/store";
import { Collapse, Icon } from "@/parts";

/** 侧栏：「更新到 0.3.0 · 重新打开」（下好了）/「有新版本 0.3.0 · 更新」（自动更新关着）。装不了的版本不在这里出现，只在设置里说 */
export function UpdateRow() {
  const st = useApp((s) => s.appUpdate);
  const ready = st?.state === "ready";
  const show = !!st && !st.blocked && !!st.latest && (ready || st.state === "available");
  return (
    <Collapse open={show}>
      <div className="cp-side-update">
        <button className="cp-side-row" onClick={() => (ready ? requestRestart() : void downloadUpdate())}>
          <span className="glyph">
            <Icon icon={CircleArrowUp01Icon} size={15} />
          </span>
          <span className="t">{ready ? t("更新到 {version}", { version: st?.latest ?? "" }) : t("有新版本 {version}", { version: st?.latest ?? "" })}</span>
          <span className="trail">{ready ? t("重新打开§装上新版、重新打开 Captae 的按钮，要短") : t("更新§侧栏里有新版本时右边的按钮，要短")}</span>
        </button>
      </div>
    </Collapse>
  );
}

/** 挂在窗口上：更新后第一次打开的提示、「下载并更新」下好后接着重新打开、重新打开前的确认 */
export function AppUpdateHost() {
  const st = useApp((s) => s.appUpdate);
  const jobs = useApp((s) => s.jobs);
  const open = useRestartAsk();
  useEffect(() => {
    if (!st) return;
    announceUpdated(st);
    onUpdateStatus(st);
  }, [st]);
  const { downloads, recordings } = interrupted(jobs);
  return (
    <AlertDialog
      className="cp"
      open={open}
      onOpenChange={(o) => !o && setRestartAsk(false)}
      title={t("重新打开 Captae 并更新？")}
      confirmLabel={t("重新打开§装上新版、重新打开 Captae 的按钮，要短")}
      cancelLabel={t("以后再说")}
      onConfirm={() => restartNow()}
    >
      {downloads > 0 && <p className="cp-dlg-p">{t("还有{n}个任务在下载，重新打开后会接着下。", { n: downloads })}</p>}
      {recordings > 0 && <p className="cp-dlg-p">{t("正在录的{n}个直播会停止，录到的部分在重新打开后保存好。", { n: recordings })}</p>}
    </AlertDialog>
  );
}
