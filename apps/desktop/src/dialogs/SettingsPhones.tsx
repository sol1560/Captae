// 设置 › 手机：让手机在同一个 Wi-Fi 下遥控这台电脑（后端 src-tauri/src/remote_cmds.rs → crates/captae-remote）。
// 一组设置：「让手机连接这台电脑」开关（打开以后下面一行防火墙的说明）；打开时下面展开二维码和 6 位数（5 分钟有效，倒计时，过期了自动换新的）；
// 「已配对的手机」列表（名字、在线 / 上次连接、撤销）。后端还没有这些命令时整组不显示（和「已登录的网站」一样）。
//
// 动效（docs/MOTION.md）：
// - 打开开关时二维码那块用 parts 的 Collapse 展开（配方 e：270ms / 200ms；减少动效时只 150ms 淡入淡出）。
// - 二维码过期换新的、点「换一个」：新码原地淡入 200ms + 3px 模糊（--ease-out-quint，和封面读到时同一种做法），不位移；
//   第一次出现跟着 Collapse 展开，不再另外淡入（一个容器只做一次入场）。减少动效时只淡入。
// - 倒计时：parts 的 Countdown（圆环 linear，时长等于剩下的秒数；减少动效时按秒跳），数字用等宽数字、不做动画（配方 c6）。
// - 撤销、手机连上 / 断开：行直接换，不做动画（低频、只有几行，下面有提示条交代发生了什么）。
import { RefreshIcon, SmartPhone01Icon, Unlink03Icon } from "@hugeicons/core-free-icons";
import { useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { encode } from "uqr";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { t } from "@/i18n";
import { errText, invoke } from "@/lib/bridge";
import { clock, dayLabel, whenLabel } from "@/lib/format";
import { notify } from "@/lib/notify";
import { onEvent } from "@/lib/store";
import type { RemoteDevice, RemotePairing, RemoteStatus } from "@/lib/types";
import { Collapse, Countdown, Icon } from "@/parts";
import "./settings-phone.css";

export function Phones({ open }: { open: boolean }) {
  // undefined：还没查；null：后端没有这个命令
  const [st, setSt] = useState<RemoteStatus | null | undefined>(undefined);
  const [busy, setBusy] = useState(false);
  const load = () =>
    invoke<RemoteStatus>("remote_status")
      .then(setSt)
      .catch(() => setSt(null));
  useEffect(() => {
    if (open) load();
  }, [open]);
  // 配对了、撤销了、手机连上 / 断开：重新取一次
  useEffect(() => onEvent("remote", () => void (open && load())), [open]);
  if (!st) return null;

  const toggle = (on: boolean) => {
    setBusy(true);
    invoke<RemoteStatus>("remote_enable", { on })
      .then(setSt)
      .catch((e) => {
        notify(on ? t("没有打开§让手机连接的开关没打开成") : t("没有关掉"), errText(e));
        load();
      })
      .finally(() => setBusy(false));
  };
  const revoke = (d: RemoteDevice) =>
    invoke<RemoteStatus>("remote_revoke", { id: d.id })
      .then((s) => {
        setSt(s);
        notify(t("已撤销「{name}」", { name: d.name }), t("这台手机要重新配对才能连接"));
      })
      .catch((e) => notify(t("没有撤销"), errText(e)));

  return (
    <>
      <h3 className="cp-set-h">{t("手机")}</h3>
      <p className="cp-set-p">{t("用手机上的 Captae 把链接发到这台电脑下载、看进度、取下载好的文件。只在手机和电脑连着同一个 Wi-Fi 时能用。")}</p>
      <div className="cp-group">
        <div className="cp-set-sw">
          <Switch
            placement="end"
            label={t("让手机连接这台电脑")}
            description={st.error ? <span className="cp-phone-err">{t("没有打开：{why}", { why: st.error })}</span> : t("打开以后，配对过的手机能在同一个 Wi-Fi 下加下载、暂停和继续。手机改不了设置，也删不了素材")}
            checked={st.enabled && st.running}
            disabled={busy}
            onChange={(e) => toggle(e.target.checked)}
          />
          {/* 打开以后才有：macOS 防火墙开着时第一次监听会问一次，不去检测防火墙状态，直接说清楚该怎么选 */}
          <Collapse open={st.running}>
            <p className="cp-phone-fw">{t("如果 macOS 问要不要允许 Captae 接受传入连接，选允许，手机才连得上")}</p>
          </Collapse>
        </div>
        <Collapse open={st.running}>
          <Pairing active={open && st.running} />
        </Collapse>
      </div>
      {st.devices.length > 0 && (
        <>
          <h3 className="cp-set-h">{t("已配对的手机")}</h3>
          <div className="cp-group">
            {st.devices.map((d) => (
              <div className="cp-set-row cp-site cp-phone" key={d.id}>
                <div className="who">
                  <span className="ico" aria-hidden>
                    <Icon icon={SmartPhone01Icon} size={16} />
                  </span>
                  <div>
                    <b>
                      {d.name}
                      {d.online && <span className="st on">{t("连着")}</span>}
                    </b>
                    <small>{deviceNote(d)}</small>
                  </div>
                </div>
                <Button variant="ghost" size="sm" icon={<Icon icon={Unlink03Icon} size={14} />} onClick={() => revoke(d)}>
                  {t("撤销§撤销一台配对过的手机")}
                </Button>
              </div>
            ))}
          </div>
        </>
      )}
    </>
  );
}

/** 每台手机的第二行：上次连接、什么时候配对的 */
function deviceNote(d: RemoteDevice): string {
  const seen = d.online ? "" : d.lastSeen ? t("上次连接：{when}", { when: whenLabel(d.lastSeen) }) : "";
  const paired = t("{day}配对", { day: dayLabel(d.pairedAt) });
  return [seen, paired].filter(Boolean).join(" · ");
}

/* ---------------- 二维码和 6 位数 ---------------- */

function Pairing({ active }: { active: boolean }) {
  const [p, setP] = useState<RemotePairing | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const get = (refresh = false) =>
    invoke<RemotePairing>("remote_pairing", { refresh })
      .then((x) => {
        setP(x);
        setErr(null);
      })
      .catch((e) => setErr(errText(e)));
  useEffect(() => {
    if (active) get();
    else setP(null);
  }, [active]);
  // 配对成功以后这一轮作废了：后端给新的；6 位数试错了：剩几次跟着变
  useEffect(() => onEvent("remote", () => void (active && get())), [active]);
  // 到期了自动换新的
  useEffect(() => {
    if (!active || !p) return;
    const ms = p.expiresAt - Date.now();
    const timer = setTimeout(() => get(), Math.max(0, ms) + 300);
    return () => clearTimeout(timer);
  }, [active, p?.expiresAt]);

  // 第一次出现跟着 Collapse 展开，换码时才淡入
  const first = useRef<string | null>(null);
  if (p && first.current == null) first.current = p.uri;
  const fresh = !!p && p.uri !== first.current;

  if (err) return <div className="cp-pair cp-pair-msg">{err}</div>;
  if (!p) return <div className="cp-pair" aria-busy />;
  return (
    <div className="cp-pair">
      <div className="cp-qr" key={p.uri} data-fresh={fresh || undefined}>
        <Qr text={p.uri} />
      </div>
      <div className="cp-pair-side">
        <b>{t("用手机扫这个二维码")}</b>
        <small>{t("在手机上打开 Captae › 管理电脑 › 添加电脑")}</small>
        <div className="cp-pair-code">
          <span className="lbl">{t("或者在手机上输入配对码")}</span>
          {p.void ? (
            <span className="void">{t("配对码输错 5 次，已经作废。二维码还能用，也可以换一个")}</span>
          ) : (
            <Digits code={p.code} label={t("配对码")} />
          )}
        </div>
        <div className="cp-pair-foot">
          <Countdown key={p.expiresAt} until={p.expiresAt}>
            {(left) => t("{time} 后换新的", { time: clock(left) })}
          </Countdown>
          <Button variant="ghost" size="sm" icon={<Icon icon={RefreshIcon} size={14} />} onClick={() => get(true)}>
            {t("换一个§换一个新的配对码")}
          </Button>
        </div>
      </div>
    </div>
  );
}

/** 6 位数分成两组三位（「482 913」），读屏读成一串数字 */
function Digits({ code, label }: { code: string; label: string }): ReactNode {
  return (
    <span className="digits mono" role="text" aria-label={`${label} ${code.split("").join(" ")}`}>
      <span>{code.slice(0, 3)}</span>
      <span>{code.slice(3)}</span>
    </span>
  );
}

/** 二维码：uqr（MIT）算出模块，画成一条 SVG 路径。纠错 M 级，四周的空白由外面的白底留 */
function Qr({ text }: { text: string }) {
  const q = useMemo(() => {
    const { data, size } = encode(text, { ecc: "M", border: 0 });
    let d = "";
    data.forEach((row, y) => {
      let x = 0;
      while (x < size) {
        if (!row[x]) {
          x++;
          continue;
        }
        // 一行里连着的黑块合成一段，路径短一点
        let w = 1;
        while (x + w < size && row[x + w]) w++;
        d += `M${x} ${y}h${w}v1h-${w}z`;
        x += w;
      }
    });
    return { d, size };
  }, [text]);
  return (
    <svg viewBox={`0 0 ${q.size} ${q.size}`} shapeRendering="crispEdges" role="img" aria-label={t("配对用的二维码")}>
      <path d={q.d} fill="currentColor" />
    </svg>
  );
}
