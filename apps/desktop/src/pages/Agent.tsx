// AI 助手页（第二版）。版式照旧界面（ui/app-agent.html，计划 5.7「版式不变」），换成第二版的组件和 token：
// 宽的时候两列——左边：允许使用和连上来的 AI 助手、连接你的 AI 助手、AI 最近做了什么、AI 可以用的功能；右边：高级模式、预设。窄了变成一列。
// 地址 #/agent；#/agent?tab=presets 直接到预设（侧栏「预设 › 管理」、输入框的「管理预设…」会这样跳）。
// 动效：切页不动画（配方 a）；连接状态点只做 150ms 颜色过渡、不脉冲（配方 k1）；调用记录新来的一条按 k2 进场，其余见各文件。
import "./agent/agent.css";
import { useEffect, useRef } from "react";
import { t } from "@/i18n";
import { clearParams, useRoute } from "@/lib/route";
import { useApp } from "@/lib/store";
import { Topbar } from "@/shell/Topbar";
import { useAgentInfo, useClients } from "./agent/act";
import { Calls } from "./agent/Calls";
import { Access, Connect } from "./agent/Connect";
import { clientName, mcpLine } from "./agent/model";
import { Presets } from "./agent/Presets";
import { Advanced, Tools } from "./agent/Tools";

export function Agent() {
  const on = useApp((s) => s.settings?.mcpOn ?? true);
  const clients = useClients();
  const info = useAgentInfo();
  const route = useRoute();
  const presetsRef = useRef<HTMLElement>(null);
  const line = mcpLine(on, clients);

  // ?tab=presets：滚到预设。刚切到这一页时直接到位（切页不动画），已经在这一页时平滑滚过去（减少动效时也直接到位）
  const mountedAt = useRef(Date.now());
  useEffect(() => {
    if (route.page !== "agent" || route.params.get("tab") !== "presets") return;
    const smooth = Date.now() - mountedAt.current > 400 && !matchMedia("(prefers-reduced-motion: reduce)").matches;
    requestAnimationFrame(() => presetsRef.current?.scrollIntoView({ block: "start", behavior: smooth ? "smooth" : "auto" }));
    clearParams();
  }, [route]);

  return (
    <main className="cp-main solo">
      <div className="cp-col cp-page">
        <Topbar
          title={t("AI 助手")}
          sub={
            <span
              className="cp-page-status"
              title={(clients ?? []).map((c) => (c.connected ? t("{name}：连着", { name: clientName(c.name) }) : t("{name}：没连着", { name: clientName(c.name) }))).join("\n") || undefined}
            >
              <span className="cp-live" data-off={!line.live || undefined} />
              {line.text}
            </span>
          }
        />
        <div className="cp-page-in">
          <div className="cp-page-grid">
            <div className="cp-page-main">
              <Access on={on} clients={clients} />
              <Connect info={info} />
              <Calls info={info} />
              <Tools />
            </div>
            <div className="cp-page-side">
              <Advanced />
              <Presets ref={presetsRef} info={info} />
            </div>
          </div>
        </div>
      </div>
    </main>
  );
}
