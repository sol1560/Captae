// 浏览器插件页（第二版）。版式照旧界面（ui/app-extension.html，计划 5.7「版式不变」），换成第二版的组件和 token：
// 宽的时候两列——左边：连接状态和测试、安装插件、浏览器里发现的；右边：各家浏览器（装没装、登记没有）、插件能做什么。窄了变成一列。
// 地址 #/extension。页面骨架（两列、分节、白卡）的样式和 AI 助手页共用，写在 agent/agent.css。
// 动效：切页不动画（配方 a）；状态点只做颜色过渡（k1）；测试结果描线画勾（e3）；新发现的资源进场照下载列表（c1、c3）。
import "./agent/agent.css";
import "./extension/extension.css";
import { t } from "@/i18n";
import { Topbar } from "@/shell/Topbar";
import { useExtPath, useExtStatus } from "./extension/act";
import { Features, Found } from "./extension/Found";
import { Install } from "./extension/Install";
import { extLine } from "./extension/model";
import { Browsers, StatusCard } from "./extension/Status";

export function Extension() {
  const [st, reload, setSt] = useExtStatus();
  const path = useExtPath();
  const line = extLine(st);
  return (
    <main className="cp-main solo">
      <div className="cp-col cp-page">
        <Topbar
          title={t("浏览器插件")}
          sub={
            <span className="cp-page-status">
              <span className="cp-live" data-off={!line.live || undefined} />
              {line.text}
            </span>
          }
        />
        <div className="cp-page-in">
          <div className="cp-page-grid">
            <div className="cp-page-main">
              <StatusCard st={st} setSt={setSt} reload={reload} />
              <Install st={st} path={path} />
              <Found />
            </div>
            <div className="cp-page-side">
              <Browsers st={st} reload={reload} />
              <Features />
            </div>
          </div>
        </div>
      </div>
    </main>
  );
}
