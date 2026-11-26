// 「安装插件」：插件还没上架商店，要「加载已解压的扩展程序」。四步，第一步打开所选浏览器的扩展程序页（extension_install_help），
// 第三步给出插件文件夹（extension_path），能复制、能在访达里显示。选的浏览器存在设置 browser 里。
// 插件已经连上时，步骤收起来（Collapse），需要时再展开。
import { ArrowDown01Icon, Copy01Icon, Folder01Icon, LinkSquare02Icon } from "@hugeicons/core-free-icons";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Select } from "@/components/ui/select";
import { t, tx } from "@/i18n";
import { useApp } from "@/lib/store";
import { REVEAL } from "@/lib/os";
import { Collapse, Icon, TipIcon } from "@/parts";
import { copyWithToast } from "@/shell/actions";
import { chooseBrowser, openExtPage, revealPath } from "./act";
import { browserOf, browserOptions, type ExtStatus } from "./model";

export function Install({ st, path }: { st: ExtStatus | null; path: string | null }) {
  const b = browserOf(useApp((s) => s.settings?.browser) || "chrome");
  const connected = !!st?.connected;
  // 用户自己点过就听用户的；没点过：连上了就收起，没连上就展开
  const [open, setOpen] = useState<boolean | null>(null);
  const shown = open ?? !connected;

  return (
    <section className="cp-sec">
      <div className="cp-sec-h">
        <h2>{t("安装插件§插件页的分节标题")}</h2>
        <p>{t("插件还没上架商店，要用「加载已解压的扩展程序」装")}</p>
        <span className="end">
          <Select size="sm" aria-label={t("你用的浏览器")} className="cp-brsel" value={b.id} onChange={chooseBrowser} options={browserOptions(st?.host ?? null)} />
        </span>
      </div>
      <div className="cp-pane cp-install">
        {connected && (
          <button className="cp-install-toggle" aria-expanded={shown} onClick={() => setOpen(!shown)}>
            <span>{t("插件已经装好了。换浏览器或者换电脑时，照着步骤再装一次")}</span>
            <span className="r">
              {shown ? t("收起") : t("看步骤")}
              <Icon icon={ArrowDown01Icon} size={14} className="chev" />
            </span>
          </button>
        )}
        <Collapse open={shown}>
          <ol className="cp-how">
            <li>
              <span className="n num">1</span>
              <div>
                <b>{tx("打开 {name} 的扩展程序页", { name: b.name })}</b>
                <p>
                  {tx("在地址栏输入 {page}，或者点下面的按钮", { page: <code className="mono">{b.page}</code> })}
                </p>
                <div className="acts">
                  <Button size="sm" className="cp-pill" icon={<Icon icon={b.help ? LinkSquare02Icon : Copy01Icon} size={14} />} onClick={() => openExtPage(b, path ?? "")}>
                    {b.help ? t("打开 {name} 的扩展程序页", { name: b.name }) : t("复制地址")}
                  </Button>
                </div>
              </div>
            </li>
            <li>
              <span className="n num">2</span>
              <div>
                <b>{t("打开右上角的「开发者模式」")}</b>
                <p>{t("只需要打开一次")}</p>
              </div>
            </li>
            <li>
              <span className="n num">3</span>
              <div>
                <b>{t("点「加载已解压的扩展程序」，选中这个文件夹")}</b>
                <p>{t("插件就在 Captae 里，不用另外下载")}</p>
                <div className="cp-path">
                  <span className="mono" title={path || undefined}>
                    {path === null ? "…" : path || t("找不到插件文件夹，重新安装 Captae 试试")}
                  </span>
                  <TipIcon size="sm" icon={Copy01Icon} label={t("复制位置")} disabled={!path} onClick={() => path && copyWithToast(path, t("已复制"), t("在「加载已解压的扩展程序」里粘贴这个位置"))} />
                  <TipIcon size="sm" icon={Folder01Icon} label={t(REVEAL)} disabled={!path} onClick={() => path && revealPath(path)} />
                </div>
              </div>
            </li>
            <li>
              <span className="n num">4</span>
              <div>
                <b>{t("回到这里")}</b>
                <p>{t("浏览器开着时，插件每分钟会来打个招呼，上面就会显示「已连上」")}</p>
              </div>
            </li>
          </ol>
        </Collapse>
      </div>
    </section>
  );
}
