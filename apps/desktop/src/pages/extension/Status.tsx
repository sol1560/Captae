// 连接状态卡片（extension_status）和「测试连接」（extension_test）：三步，哪一步没通过就说哪一步，第一步能「修复」（extension_register）。
// 「各家浏览器」：宿主清单在哪些浏览器里登记了（native_host.rs 的 16 个浏览器），没装的收在下面。
// 动效：测试结果出现用 Collapse（高度 0.27s），「正在测试」和结果之间用 Swap（配方 g3）；通过的那一步描线画勾（配方 e3），
// 结果出来时才画一次，之后状态刷新不重播。
import { ArrowDown01Icon, PuzzleIcon, RefreshIcon } from "@hugeicons/core-free-icons";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { t, tx } from "@/i18n";
import { useDelayed } from "@/lib/hooks";
import { notify } from "@/lib/notify";
import { Collapse, Icon, StepTimeline, Swap } from "@/parts";
import { register, runTest } from "./act";
import { extCard, testSteps, testToast, type BrowserStatus, type ExtStatus, type ExtTest } from "./model";

export function StatusCard({ st, setSt, reload }: { st: ExtStatus | null; setSt: (s: ExtStatus) => void; reload: () => void }) {
  const [test, setTest] = useState<ExtTest | null>(null);
  const [testing, setTesting] = useState(false);
  const [fixing, setFixing] = useState(false);
  // 不到 300ms 就有结果的不显示「正在测试」，直接出结果（配方 1.10）
  const slow = useDelayed(testing, 300);
  const fixSlow = useDelayed(fixing, 300);
  const card = extCard(st);

  const run = async () => {
    if (testing) return;
    setTesting(true);
    const result = await runTest();
    setTesting(false);
    setTest(result);
    if (result.extension) setSt({ ...result.extension, host: result.host });
    reload();
    notify(...testToast(result));
  };

  const fix = async () => {
    if (fixing) return;
    setFixing(true);
    const h = await register();
    setFixing(false);
    if (h) run();
  };

  const view = test ? testSteps(test, st) : null;
  const steps = view
    ? view.steps.map((s, i) =>
        i === 0 && view.fix
          ? {
              ...s,
              trail: (
                <Button size="sm" variant="secondary" className="cp-pill" loading={fixSlow} onClick={fix}>
                  {t("修复")}
                </Button>
              ),
            }
          : s,
      )
    : [];

  return (
    <section className="cp-sec">
      <div className="cp-pane cp-extcard" data-on={st?.connected || undefined}>
        <div className="cp-extcard-h">
          <span className="tile" aria-hidden>
            <Icon icon={PuzzleIcon} size={18} />
          </span>
          <div className="txt">
            <b>{card.title}</b>
            {card.desc && <small>{card.desc}</small>}
          </div>
          <Button variant="secondary" size="sm" className="cp-pill" loading={slow} icon={<Icon icon={RefreshIcon} size={14} />} onClick={run}>
            {t("测试连接")}
          </Button>
        </div>
        <Collapse open={slow || (!testing && !!view)}>
          <div className="cp-extcard-b">
            <Swap k={slow ? "testing" : "done-" + (test ? JSON.stringify(steps.map((s) => s.s)) : "")}>
              {slow ? <StepTimeline steps={[{ label: t("正在测试…"), sub: t("像浏览器一样叫一次 Captae"), s: "now" }]} /> : <StepTimeline steps={steps} />}
            </Swap>
          </div>
        </Collapse>
      </div>
    </section>
  );
}

/* ---------------- 各家浏览器 ---------------- */

export function Browsers({ st, reload }: { st: ExtStatus | null; reload: () => void }) {
  const host = st?.host ?? null;
  const [more, setMore] = useState(false);
  const [busy, setBusy] = useState(false);
  const busySlow = useDelayed(busy, 300);
  const list = host?.browsers ?? [];
  const inst = list.filter((b) => b.installed);
  const rest = list.filter((b) => !b.installed);
  const fix = async () => {
    if (busy) return;
    setBusy(true);
    await register();
    setBusy(false);
    reload();
  };
  const needFix = !!host && !host.missing && (!host.ok || host.wrapperOk === false);

  return (
    <section className="cp-sec">
      <div className="cp-sec-h">
        <h2>{t("各家浏览器")}</h2>
        <p className="wide">{t("每个装了的浏览器都要登记一次，插件才叫得到 Captae")}</p>
      </div>
      {host?.missing ? (
        <div className="cp-pane cp-pane-pad">
          <div className="cp-note" data-tone="warn">
            <Icon icon={PuzzleIcon} size={16} />
            <div className="grow">
              {t("没找到 Captae 自带的 captae 程序")}
              {/* 插件连着时（之前登记好的还能用），不说「连不上」，免得和上面的「已连上」打架 */}
              <small>{st?.connected ? t("已经连着的浏览器照常能用；要给别的浏览器登记，得先重新安装 Captae") : t("插件暂时连不上 Captae。重新安装 Captae 试试")}</small>
            </div>
          </div>
        </div>
      ) : (
        <div className="cp-pane cp-group cp-browsers">
          {!host && (
            <div className="cp-set-row cp-browser">
              <div>
                <small>{t("正在查看…")}</small>
              </div>
            </div>
          )}
          {inst.map((b) => (
            <BrowserRow key={b.id} b={b} />
          ))}
          {host && !inst.length && (
            <div className="cp-set-row">
              <div>
                <b>{t("没找到装好的浏览器")}</b>
                <small>{t("插件要装在 Chrome、Edge、Arc、Brave 这类浏览器里")}</small>
              </div>
            </div>
          )}
          {needFix && (
            <div className="cp-set-row">
              <div>
                <b>{host?.wrapperOk === false ? t("登记的不是现在这个 Captae") : t("有浏览器还没登记")}</b>
                <small>{t("新装的浏览器，Captae 下次打开时会自动登记，也可以现在就登记")}</small>
              </div>
              <Button size="sm" variant="secondary" className="cp-pill" loading={busySlow} onClick={fix}>
                {t("重新登记")}
              </Button>
            </div>
          )}
          {rest.length > 0 && (
            <button className="cp-browsers-more" aria-expanded={more} onClick={() => setMore(!more)}>
              <span>{tx("另外 {n} 个浏览器没装", { n: rest.length })}</span>
              <Icon icon={ArrowDown01Icon} size={14} className="chev" />
            </button>
          )}
          {rest.length > 0 && (
            <Collapse open={more}>
              <p className="cp-browsers-rest">
                {rest.map((b, i) => (
                  <span key={b.id} title={b.manifest}>
                    {b.name}
                    {b.registered && <small>{t("（已登记）")}</small>}
                    {i < rest.length - 1 ? t("、§列表里的分隔") : ""}
                  </span>
                ))}
              </p>
              <p className="cp-browsers-note">{t("装好以后，Captae 下次打开时会自动登记")}</p>
            </Collapse>
          )}
        </div>
      )}
    </section>
  );
}

function BrowserRow({ b }: { b: BrowserStatus }) {
  const text = !b.installed ? (b.registered ? t("没装 · 已登记") : t("没装")) : b.registered ? t("已登记") : t("还没登记");
  const tone = !b.installed ? "" : b.registered ? " on" : " warn";
  return (
    <div className="cp-set-row cp-browser" title={b.manifest}>
      <div>
        <b>
          {b.name}
          <span className={`st${tone}`}>{text}</span>
        </b>
      </div>
    </div>
  );
}
