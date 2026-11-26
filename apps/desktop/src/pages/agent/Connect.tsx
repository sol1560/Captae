// 「允许 AI 助手使用 Captae」和连上来的 AI 助手；「连接你的 AI 助手」：Claude Desktop、Cursor 的配置（可以一键写入）、
// Claude Code 的命令、其他 AI 助手用的 Skill。一键写入先给用户看会改哪个文件、合并什么（mcp_config_plan），确认了再写（mcp_config_write）。
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { CodeBlock } from "@/components/ui/code-block";
import { SegmentedControl } from "@/components/ui/segmented-control";
import { Switch } from "@/components/ui/switch";
import { N_, t, tx, type Key } from "@/i18n";
import { getPref, setPref } from "@/lib/prefs";
import { useApp } from "@/lib/store";
import { copyWithToast } from "@/shell/actions";
import { exportSkill, mcpCommandOf, setMcpOn } from "./act";
import { useConfigWriter } from "./ConfigWrite";
import { claudeCodeLine, clientName, clientNote, configJson, normTool, shortPath, shq, targetName, toolLabel, type AgentInfo, type Client, type Target } from "./model";

/* ---------------- 允许使用 + 连上来的 AI 助手 ---------------- */

export function Access({ on, clients }: { on: boolean; clients: Client[] | null }) {
  const tools = useApp((s) => s.tools);
  const label = (name: string) => {
    const tool = tools.find((x) => normTool(x.name) === normTool(name));
    return tool ? toolLabel(tool.label) : name;
  };
  return (
    <section className="cp-sec">
      <div className="cp-pane cp-group">
        <div className="cp-set-sw">
          <Switch
            placement="end"
            label={t("允许 AI 助手使用 Captae")}
            description={t("Claude、Cursor 等可以帮你下载、转格式、找素材，只在这台电脑上")}
            checked={on}
            onChange={(e) => setMcpOn(e.target.checked)}
          />
        </div>
        {(clients ?? []).map((c) => (
          <div className="cp-set-row cp-client" key={c.name}>
            <div className="who">
              <span className="cp-live" data-off={!(on && c.connected) || undefined} />
              <div>
                <b>
                  {clientName(c.name)}
                  <span className={`st${on && c.connected ? " on" : ""}`}>{c.connected ? (on ? t("连着") : t("连着，但现在用不了 Captae")) : t("没连着")}</span>
                </b>
                <small>{clientNote(c, label)}</small>
              </div>
            </div>
          </div>
        ))}
      </div>
      {clients && !clients.length && <p className="cp-sec-note">{t("还没有 AI 助手连上过。照下面的步骤连好以后，它们会出现在这里。")}</p>}
    </section>
  );
}

/* ---------------- 连接你的 AI 助手 ---------------- */

const HINT: Record<"claude" | "cursor", Key> = {
  claude: N_("在 Claude Desktop 的「设置 › 开发者 › 编辑配置」里合并进去，或者点「一键写入」。改完要重启 Claude Desktop。"),
  cursor: N_("也可以在 Cursor 的「设置 › MCP」里添加，或者点「一键写入」。改完要重启 Cursor。"),
};

const TARGETS: Target[] = ["claude", "cursor", "code", "skill"];

export function Connect({ info }: { info: AgentInfo | null }) {
  const [target, setTarget] = useState<Target>(() => {
    const v = getPref<string>("agentTarget", "claude");
    return (TARGETS as string[]).includes(v) ? (v as Target) : "claude";
  });
  // 一键写入（先给用户看会改什么，确认了再写）：和上手引导共用
  const writer = useConfigWriter(info);
  const cmd = mcpCommandOf(info);
  const name = targetName(target);

  const pick = (v: Target) => {
    setTarget(v);
    setPref("agentTarget", v);
  };

  let body;
  if (target === "claude" || target === "cursor") {
    const path = info?.configs?.[target];
    body = (
      <>
        <CodeBlock language="json" filename={path ? shortPath(path, info) : name} code={configJson(cmd)} copyable={false} />
        <div className="cp-connect-foot">
          <p>{t(HINT[target])}</p>
          <div className="acts">
            <Button variant="secondary" size="sm" className="cp-pill" onClick={() => copyWithToast(configJson(cmd), t("已复制"), t("粘贴到 {name} 的配置文件里", { name }))}>
              {t("复制")}
            </Button>
            <Button size="sm" className="cp-pill" loading={writer.busy} onClick={() => writer.start(target)}>
              {t("一键写入")}
            </Button>
          </div>
        </div>
      </>
    );
  } else if (target === "code") {
    body = (
      <>
        <CodeBlock language="bash" filename={t("终端")} code={claudeCodeLine(cmd)} copyable={false} />
        <div className="cp-connect-foot">
          <p>{t("在终端里运行这一行，之后新开的 Claude Code 就能用 Captae。")}</p>
          <div className="acts">
            <Button variant="secondary" size="sm" className="cp-pill" onClick={() => copyWithToast(claudeCodeLine(cmd), t("已复制"), t("粘贴到终端里运行"))}>
              {t("复制")}
            </Button>
          </div>
        </div>
      </>
    );
  } else {
    body = (
      <>
        {/* SKILL.md 的内容后端给（只有英文一种），不翻 */}
        <CodeBlock language="markdown" filename="captae/SKILL.md" code={info?.skill || t("正在读取 Skill…")} wrap maxHeight={260} copyable={false} />
        <div className="cp-connect-foot">
          <p>{tx("支持 Skill 的 AI 助手：导出后放进它的 skills 文件夹。它还要能连上 MCP，命令是 {cmd}。", { cmd: <code className="mono">{shq(cmd)} mcp --stdio</code> })}</p>
          <div className="acts">
            <Button variant="secondary" size="sm" className="cp-pill" disabled={!info?.skill} onClick={() => copyWithToast(info?.skill ?? "", t("已复制"), t("粘贴到 AI 助手的 Skill 或自定义说明里"))}>
              {t("复制")}
            </Button>
            <Button size="sm" className="cp-pill" onClick={() => exportSkill(info)}>
              {t("导出 Skill 文件")}
            </Button>
          </div>
        </div>
      </>
    );
  }

  return (
    <section className="cp-sec">
      <div className="cp-sec-h">
        <h2>{t("连接你的 AI 助手")}</h2>
        <p>{t("不知道粘贴到哪里？在 AI 助手的设置里找「MCP」或「工具」")}</p>
      </div>
      <div className="cp-pane cp-connect">
        <SegmentedControl aria-label={t("AI 助手")} size="sm" value={target} onChange={(v) => pick(v as Target)} options={TARGETS.map((x) => ({ value: x, label: targetName(x) }))} />
        {/* 换客户端：高频的切换，内容直接换，不做动画（配方 1.10） */}
        <div className="cp-connect-body">{body}</div>
      </div>

      {writer.dialog}
    </section>
  );
}
