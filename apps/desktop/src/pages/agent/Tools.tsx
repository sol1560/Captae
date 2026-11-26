// 「AI 可以用的功能」：后端的真实工具列表（启动快照 tools），每个可以单独关掉（设置 toolsOff）。
// 「高级模式」（设置 adv）：打开前要确认一次，勾上「我知道……」才能开；关掉直接关。
// 开关和勾选框是 affectUI 的，自带的过渡曲线在 theme.css 里统一换成了课程的 ease-out-quint；这里不另加动效。
import { Edit02Icon, Layers01Icon, Settings02Icon, TerminalIcon } from "@hugeicons/core-free-icons";
import { useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Dialog } from "@/components/ui/dialog";
import { Switch } from "@/components/ui/switch";
import { N_, t, type Key } from "@/i18n";
import { useApp } from "@/lib/store";
import { Icon, type HugeIcon } from "@/parts";
import { setAdv, setToolOn } from "./act";
import { normTool, toolLabel } from "./model";

export function Tools() {
  const tools = useApp((s) => s.tools);
  const offList = useApp((s) => s.settings?.toolsOff);
  const on = useApp((s) => s.settings?.mcpOn ?? true);
  const off = useMemo(() => new Set((offList ?? []).map(normTool)), [offList]);
  if (!tools.length) return null;
  return (
    <section className="cp-sec">
      <div className="cp-sec-h">
        <h2>{t("AI 可以用的功能")}</h2>
        <p>{on ? t("关掉的功能，AI 就用不了") : t("现在整个关着，上面打开以后这里的设置才起作用")}</p>
      </div>
      <div className="cp-pane cp-tools">
        {tools.map((tool) => (
          <div className="cp-tool" key={tool.name}>
            {/* 名字是后端给的中文原文（agent.rs 的 label），按界面语言显示 */}
            <Switch
              placement="end"
              label={toolLabel(tool.label)}
              description={<span className="mono">{tool.name}</span>}
              checked={!off.has(normTool(tool.name))}
              onChange={(e) => setToolOn(tool.name, e.target.checked)}
            />
          </div>
        ))}
      </div>
    </section>
  );
}

const UNLOCK: [HugeIcon, Key][] = [
  [TerminalIcon, N_("自己决定转换的画质和编码（ffmpeg 参数）")],
  [Layers01Icon, N_("自己调整下载方式和线程数（yt-dlp、aria2 参数）")],
  [Settings02Icon, N_("处理更复杂的网站，比如要登录的")],
  [Edit02Icon, N_("写明文件名时，可以覆盖已有的文件")],
];

export function Advanced() {
  const adv = useApp((s) => s.settings?.adv ?? false);
  const [ask, setAsk] = useState(false);
  const [ack, setAck] = useState(false);
  return (
    <section className="cp-sec">
      <div className="cp-sec-h">
        <h2>{t("高级模式")}</h2>
      </div>
      <div className="cp-pane cp-group cp-adv" data-on={adv || undefined}>
        <div className="cp-set-sw">
          <Switch
            placement="end"
            label={t("让 AI 自己调整细节")}
            description={adv ? t("已开启。AI 做的每件事都会记在「AI 最近做了什么」里") : t("现在是普通模式：AI 只能用安全的默认设置，不会误删或覆盖文件")}
            checked={adv}
            onChange={(e) => {
              if (e.target.checked) {
                setAck(false);
                setAsk(true);
              } else setAdv(false);
            }}
          />
        </div>
        <ul className="cp-unlock" aria-label={t("高级模式下 AI 还可以")}>
          {UNLOCK.map(([icon, text]) => (
            <li key={text}>
              <Icon icon={icon} size={15} />
              <span>{t(text)}</span>
            </li>
          ))}
        </ul>
      </div>
      <Dialog
        className="cp"
        size="sm"
        open={ask}
        onOpenChange={setAsk}
        title={t("开启高级模式？")}
        description={t("AI 可以直接给 yt-dlp、aria2、ffmpeg 传参数，也可以在写明完整文件名时覆盖已有的文件（原文件本身不会被覆盖）。它做的每件事都会记下来。只在你信任这个 AI 时开启。")}
        closeLabel={t("关闭")}
        footer={
          <>
            <Button variant="secondary" className="cp-pill" onClick={() => setAsk(false)}>
              {t("先不开启")}
            </Button>
            <Button
              className="cp-pill"
              disabled={!ack}
              onClick={() => {
                setAdv(true);
                setAsk(false);
              }}
            >
              {t("开启高级模式")}
            </Button>
          </>
        }
      >
        <Checkbox label={t("我知道 AI 的操作会直接在这台电脑上执行")} checked={ack} onChange={(e) => setAck(e.target.checked)} />
      </Dialog>
    </section>
  );
}
