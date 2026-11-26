// 一键写入 AI 助手的配置：先 mcp_config_plan 给用户看会改哪个文件、合并什么，确认了再 mcp_config_write。
// 「AI 助手」页的「连接你的 AI 助手」和上手引导最后一页共用（样式 .cp-plan 在 agent.css，AI 助手页已经加载）。
import { useRef, useState, type ReactNode } from "react";
import { AlertDialog } from "@/components/ui/dialog";
import { CodeBlock } from "@/components/ui/code-block";
import { joinList, t } from "@/i18n";
import { errText, invoke } from "@/lib/bridge";
import { useDelayed } from "@/lib/hooks";
import { notify } from "@/lib/notify";
import { shortPath, targetName, tidyJson, type AgentInfo, type ConfigPlan, type ConfigWritten } from "./model";

export type WriteClient = "claude" | "cursor";

/** start(client) 开始；busy 是「正在看要改什么」超过 300ms（不到 300ms 的不转圈，配方 1.10）；dialog 放进页面里。
 *  onWritten：真的写进去了（或者本来就一模一样）以后调用 */
export function useConfigWriter(info: AgentInfo | null, onWritten?: (client: WriteClient) => void): { start: (c: WriteClient) => void; busy: boolean; dialog: ReactNode } {
  const [planning, setPlanning] = useState(false);
  const [plan, setPlan] = useState<{ client: WriteClient; p: ConfigPlan } | null>(null);
  const busy = useDelayed(planning, 300);

  const start = (client: WriteClient) => {
    if (planning) return;
    setPlanning(true);
    invoke<ConfigPlan>("mcp_config_plan", { client })
      .then((p) => {
        if (p.unchanged) {
          notify(t("{name} 已经连好了", { name: targetName(client) }), t("配置文件里已经有 Captae，不用再写"));
          onWritten?.(client);
        } else setPlan({ client, p });
      })
      .catch((e) => notify(t("没法自动写入"), t("{reason}。可以点「复制」自己粘贴。", { reason: errText(e) })))
      .finally(() => setPlanning(false));
  };

  // 确认弹窗退场时还要显示原来的内容，不能跟着变空（退场规则 1.4 第 7 条）
  const last = useRef(plan);
  if (plan) last.current = plan;
  const shown = last.current;
  const notes = shown
    ? [
        shown.p.exists ? t("原文件会先备份成同名的 .bak 文件。") : t("这个文件还不存在，会新建。"),
        shown.p.others.length ? t("文件里另外 {n} 个 MCP 服务器（{names}）保持不变。", { n: shown.p.others.length, names: joinList(shown.p.others) }) : "",
        shown.p.previous != null ? t("原来的 captae 设置会被换成上面这段。") : "",
        t("写好后重启 {name} 就能用。", { name: targetName(shown.client) }),
      ].filter(Boolean)
    : [];

  const dialog = (
    <AlertDialog
      className="cp"
      size="md"
      open={!!plan}
      onOpenChange={(o) => !o && setPlan(null)}
      title={shown ? t("写入 {name} 的设置？", { name: targetName(shown.client) }) : t("写入设置？")}
      description={t("Captae 会把下面这段合并进配置文件，其他内容不动。")}
      confirmLabel={t("写入")}
      cancelLabel={t("取消")}
      onConfirm={() => {
        const s = shown;
        if (!s) return;
        return invoke<ConfigWritten>("mcp_config_write", { client: s.client })
          .then((r) => {
            const n = targetName(s.client);
            if (r.unchanged) notify(t("{name} 已经连好了", { name: n }), t("配置文件里已经有 Captae"));
            else
              notify(
                t("已写入 {name} 的设置", { name: n }),
                r.backup ? t("重启 {name} 后生效，原文件备份在 {path}", { name: n, path: shortPath(r.backup, info) }) : t("重启 {name} 后生效", { name: n }),
              );
            onWritten?.(s.client);
          })
          .catch((e) => {
            notify(t("没有写入"), errText(e));
            throw e;
          });
      }}
    >
      {shown && (
        <div className="cp-plan">
          <div className="k">{t("会改动这个文件")}</div>
          <div className="v mono" title={shown.p.path}>
            {shortPath(shown.p.path, info)}
          </div>
          <div className="k">{t("合并进去的内容")}</div>
          <CodeBlock code={tidyJson(shown.p.snippet)} copyable={false} wrap maxHeight={200} />
          <ul className="cp-plan-notes">
            {notes.map((n) => (
              <li key={n}>{n}</li>
            ))}
          </ul>
        </div>
      )}
    </AlertDialog>
  );
  return { start, busy, dialog };
}
