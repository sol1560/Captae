// 设置弹窗（第二版）：旧界面的设置项全部保留，新增「已登录的网站」「网络」「下载引擎」（来自这次的登录和可靠性工作）。
// 分组用浅灰圆角块装设置行（macOS 系统设置的写法），不用彩色标签。
// 弹窗用 affectUI 的 Dialog，进退场用它自带的（退场曲线在 theme.css 里换成了 ease-out）。
// 「已登录的网站」「检查更新」「代理现状」的命令由第一、第二阶段提供：后端还没有这些命令时，对应的那一块不显示。
import { Add01Icon, Logout01Icon } from "@hugeicons/core-free-icons";
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Select } from "@/components/ui/select";
import { Switch } from "@/components/ui/switch";
import { TextField } from "@/components/ui/text-field";
import { lang, LANGS, N_, t, tAny } from "@/i18n";
import { errText, invoke, platform } from "@/lib/bridge";
import { normalizeUrl } from "@/lib/detect";
import { bare, dayLabel, etaText, home, whenLabel } from "@/lib/format";
import { notify } from "@/lib/notify";
import { effectiveLang, getSystemLang, loadDeps, onEvent, setSettings, useApp } from "@/lib/store";
import type { EngineReport, EngineStatus, LoginSite, ProxyStatus, Settings } from "@/lib/types";
import { installWhisper, modelInfo, useWhisper, WHISPER_MODELS } from "@/lib/whisper";
import { Collapse, Icon, ProgressBar, SiteIcon } from "@/parts";
import { restartOnboarding } from "@/dialogs/Onboarding";
import { Phones } from "@/dialogs/SettingsPhones";
import { openSiteLogin } from "@/shell/actions";
import "./settings-login.css";

/** 换了界面语言以后整个窗口重新挂一次（main.tsx），设置弹窗也是新挂的：焦点放回语言这一栏，不跳到「完成」。
 *  语言那一栏挂好时把它的下拉框放进 langFocus，弹窗打开时按 initialFocus 把焦点给它，用完清掉 */
let refocusLanguage = false;
const langFocus: { current: HTMLElement | null } = { current: null };

export function SettingsDialog({ open, onClose }: { open: boolean; onClose: () => void }) {
  const s = useApp((st) => st.settings);
  useEffect(() => {
    if (open) loadDeps();
  }, [open]);
  return (
    <Dialog
      className="cp"
      open={open}
      onOpenChange={(o) => !o && onClose()}
      size="lg"
      title={t("设置")}
      closeLabel={t("关闭")}
      initialFocus={langFocus}
      footer={
        <Button className="cp-pill" data-autofocus onClick={onClose}>
          {t("完成")}
        </Button>
      }
    >
      {s && <Body s={s} open={open} />}
    </Dialog>
  );
}

function Body({ s, open }: { s: Settings; open: boolean }) {
  return (
    <div className="cp-set">
      <h3 className="cp-set-h">{t("通用")}</h3>
      <div className="cp-group">
        <Language s={s} />
      </div>

      <h3 className="cp-set-h">{t("下载§设置里的一组")}</h3>
      <div className="cp-group">
        <div className="cp-set-sw">
          <Switch
            placement="end"
            label={t("复制链接时问我要不要下载")}
            description={t("复制了视频链接、1 分钟内回到 Captae 时，在输入框上方问一句")}
            checked={s.clipboard}
            onChange={(e) => setSettings({ clipboard: e.target.checked })}
          />
        </div>
        <Row title={t("同时下载")} sub={t("最多同时下载几个")}>
          <Select
            size="sm"
            aria-label={t("同时下载")}
            className="cp-set-select"
            value={String(s.concurrency)}
            onChange={(v) => setSettings({ concurrency: Number(v) })}
            options={[1, 2, 3, 4, 5, 6].map((n) => ({ value: String(n), label: t("{n} 个§同时下载几个", { n }) }))}
          />
        </Row>
        <Row title={t("下载加速")} sub={t("大文件分成几段同时下，网速好时更快")}>
          <Select
            size="sm"
            aria-label={t("下载加速")}
            className="cp-set-select"
            value={String(s.threads)}
            onChange={(v) => setSettings({ threads: Number(v) })}
            options={[
              { value: "4", label: t("标准") },
              { value: "8", label: t("较快") },
              { value: "16", label: t("最快") },
            ]}
          />
        </Row>
        <SaveDir s={s} />
      </div>

      <LoginSites s={s} open={open} />

      <h3 className="cp-set-h">{t("网络")}</h3>
      <div className="cp-group">
        <Proxy s={s} open={open} />
      </div>

      <h3 className="cp-set-h">{t("下载引擎")}</h3>
      <div className="cp-group">
        <Engines s={s} open={open} />
      </div>

      <h3 className="cp-set-h">{t("字幕")}</h3>
      <div className="cp-group">
        <Whisper />
        <Translate s={s} />
      </div>

      <h3 className="cp-set-h">{t("AI 助手")}</h3>
      <div className="cp-group">
        <div className="cp-set-sw">
          <Switch placement="end" label={t("让 AI 助手使用 Captae")} description={t("Claude、Cursor 等可以帮你下载和找素材")} checked={s.mcpOn} onChange={(e) => setSettings({ mcpOn: e.target.checked })} />
        </div>
      </div>

      <Phones open={open} />

      <h3 className="cp-set-h">{t("帮助")}</h3>
      <div className="cp-group">
        <Row title={t("上手引导")} sub={t("再看一遍第一次打开时的介绍，侧栏的上手清单也会回来")}>
          <Button variant="secondary" size="sm" className="cp-pill" onClick={restartOnboarding}>
            {t("重新查看")}
          </Button>
        </Row>
      </div>
    </div>
  );
}

/** 界面语言。行标题在英文以外的语言里后面带「· Language」：万一切到看不懂的语言，还找得回来。
 *  下拉框的值「system」= 设置里存空（跟随系统） */
function Language({ s }: { s: Settings }) {
  const wrap = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    if (!refocusLanguage) return;
    refocusLanguage = false;
    langFocus.current = wrap.current?.querySelector<HTMLElement>("button") ?? null;
  }, []);
  useEffect(() => {
    langFocus.current = null;
  }, []);
  const sys = LANGS.find((l) => l.code === getSystemLang())?.name ?? "English";
  const options = [{ value: "system", label: t("跟随系统（{lang}）", { lang: sys }) }, ...LANGS.map((l) => ({ value: l.code, label: l.name }))];
  return (
    <Row
      title={
        <>
          {t("语言")}
          {lang() !== "en" && <span className="cp-set-en"> · Language</span>}
        </>
      }
      sub={t("界面、提示和命令行用的语言")}
    >
      <div ref={wrap} style={{ display: "contents" }}>
        <Select
          size="sm"
          aria-label={lang() === "en" ? "Language" : `${t("语言")} · Language`}
          className="cp-set-select lang"
          value={s.language || "system"}
          onChange={(v) => {
            const language = v === "system" ? "" : v;
            // 真的换了语言才会重新挂载
            refocusLanguage = effectiveLang({ ...s, language }) !== lang();
            setSettings({ language });
          }}
          options={options}
        />
      </div>
    </Row>
  );
}

function Row({ title, sub, children }: { title: ReactNode; sub?: ReactNode; children?: ReactNode }) {
  return (
    <div className="cp-set-row">
      <div>
        <b>{title}</b>
        {sub && <small>{sub}</small>}
      </div>
      {children ?? <span />}
    </div>
  );
}

/* ---------------- 保存位置 ---------------- */

function SaveDir({ s }: { s: Settings }) {
  const choose = () =>
    invoke<string | null>("choose_save_dir")
      .then((d) => d && notify(t("保存位置已更改"), home(d)))
      .catch((e) => notify(t("没有更改"), errText(e)));
  return (
    <Row title={t("保存位置")} sub={s.saveDir ? home(s.saveDir) : platform === "mac" ? t("按类型存到 影片 / 音乐 / 图片 › Captae") : t("按类型存到 视频 / 音乐 / 图片 › Captae")}>
      <div className="acts">
        {s.saveDir && (
          <Button variant="ghost" size="sm" onClick={() => setSettings({ saveDir: "" })}>
            {t("恢复默认")}
          </Button>
        )}
        <Button variant="secondary" size="sm" className="cp-pill" onClick={choose}>
          {t("更改")}
        </Button>
      </div>
    </Row>
  );
}

/* ---------------- 已登录的网站（第二阶段，后端见 src-tauri/src/login.rs） ---------------- */

/** yt-dlp --cookies-from-browser 读得了的浏览器（Arc 读不了：它的钥匙串项 yt-dlp 解不开）。Safari 只有 macOS 有 */
const BROWSERS = [
  { value: "chrome", label: "Chrome" },
  { value: "safari", label: "Safari" },
  { value: "edge", label: "Edge" },
  { value: "brave", label: "Brave" },
  { value: "firefox", label: "Firefox" },
  { value: "vivaldi", label: "Vivaldi" },
  { value: "opera", label: "Opera" },
  { value: "chromium", label: "Chromium" },
].filter((b) => platform === "mac" || b.value !== "safari");

/** Windows 上的 Chrome、Edge（127 起）给登录信息加了只有它自己能解开的加密（app-bound），yt-dlp 读不了 */
const SEALED = (b: string) => platform === "win" && (b === "chrome" || b === "edge");

/** 读这个浏览器的时候用户要做什么 */
function browserHint(b: string): string {
  if (b === "safari") return t("先在「系统设置 › 隐私与安全性 › 完全磁盘访问权限」里打开 Captae");
  if (b === "firefox") return t("直接读，不用授权");
  if (SEALED(b)) return t("Windows 上读不了：它给登录信息加了只有它自己能解开的加密。建议改读 Firefox");
  if (platform === "win") return t("读的时候要先关掉这个浏览器，它开着时不让别的程序读");
  if (platform === "linux") return t("要用系统的密钥环（GNOME Keyring / KWallet）解密，密钥环锁着时会请你解锁");
  return t("第一次读会弹出钥匙串授权，点「始终允许」");
}

/** 开关下面那句说明：各系统读浏览器登录状态时要注意的不一样 */
const COOKIES_NOTE =
  platform === "mac"
    ? N_("在 Captae 里没登录的网站，用浏览器里的登录状态下载。读 Chrome 会弹出钥匙串授权，读 Safari 要在系统设置里给「完全磁盘访问权限」。YouTube 也只在需要时才带")
    : platform === "win"
      ? N_("在 Captae 里没登录的网站，用浏览器里的登录状态下载。Windows 上的 Chrome、Edge 读不了（加了只有它们自己能解开的加密），建议读 Firefox。YouTube 也只在需要时才带")
      : N_("在 Captae 里没登录的网站，用浏览器里的登录状态下载。读 Chrome 这类浏览器要用系统的密钥环（GNOME Keyring / KWallet）。YouTube 也只在需要时才带");

/** 每一行的第二行字：什么时候登录的、登录以后有什么用（gain、note 是后端按当时的语言写好的） */
function siteNote(x: LoginSite): string {
  const day = dayLabel(x.at);
  if (x.state === "in") return [day && t("{day}登录", { day }), x.gain].filter(Boolean).join(" · ");
  if (x.state === "verified") return day ? t("{day}验证过", { day }) : t("验证过");
  return x.note || x.gain || "";
}

const siteState = (x: LoginSite) => (x.state === "in" ? t("已登录") : x.state === "verified" ? t("验证过") : t("没有登录"));

function LoginSites({ s, open }: { s: Settings; open: boolean }) {
  const [sites, setSites] = useState<LoginSite[] | null>(null);
  const [other, setOther] = useState(false);
  const [url, setUrl] = useState("");
  const load = () =>
    invoke<LoginSite[]>("login_sites")
      .then(setSites)
      .catch(() => setSites(null));
  useEffect(() => {
    if (open) load();
    else setOther(false);
  }, [open]);
  // 在登录窗口里登录好了、清除了、关了：刷新这一组
  useEffect(() => onEvent("login", (ev) => void (open && ev.state !== "open" && load())), [open]);
  if (!sites) return null;
  // 网站名存的是中文原文（哔哩哔哩），显示时翻译；YouTube 这类没有译文的原样显示
  const logout = (x: LoginSite) =>
    invoke("login_logout", { site: x.site })
      .then(() => {
        notify(x.state === "verified" ? t("已清除{site}的验证", { site: tAny(x.name) }) : t("已退出{site}", { site: tAny(x.name) }), t("这台电脑上存的登录状态已经删掉"));
        return load();
      })
      .catch((e) => notify(t("没有退出"), errText(e)));
  const typed = normalizeUrl(url);
  const openOther = () => {
    if (!typed) return;
    openSiteLogin({ url: typed }).then((ok) => {
      if (ok) {
        setUrl("");
        setOther(false);
      }
    });
  };
  const browser = s.cookiesFromBrowser ?? "";
  // 打开开关时先用上手引导里选的浏览器（Arc 读不了，换成 Chrome）；Windows 上 Chrome、Edge 读不了，先用 Firefox
  const picked = BROWSERS.some((b) => b.value === s.browser) ? s.browser : "chrome";
  const firstBrowser = SEALED(picked) ? "firefox" : picked;
  return (
    <>
      <h3 className="cp-set-h">{t("已登录的网站")}</h3>
      <p className="cp-set-p">{t("在 Captae 里登录过的网站。登录状态只存在这台电脑上；AI 助手看不到，它让 Captae 下载时和你自己下载一样会带上。")}</p>
      <div className="cp-group">
        {sites.map((x) => (
          <div className="cp-set-row cp-site" key={x.site}>
            <div className="who">
              <SiteIcon site={x.site} size={16} />
              <div>
                <b>
                  {tAny(x.name)}
                  <span className={`st${x.state !== "out" ? " on" : ""}`}>{siteState(x)}</span>
                </b>
                {siteNote(x) && <small>{siteNote(x)}</small>}
              </div>
            </div>
            {x.state === "out" ? (
              <Button variant="secondary" size="sm" className="cp-pill" onClick={() => openSiteLogin({ site: x.site })}>
                {t("登录")}
              </Button>
            ) : (
              <Button variant="ghost" size="sm" icon={<Icon icon={Logout01Icon} size={14} />} onClick={() => logout(x)}>
                {x.state === "verified" ? t("清除") : t("退出")}
              </Button>
            )}
          </div>
        ))}
        <div className="cp-site-more">
          <Button variant="ghost" size="sm" icon={<Icon icon={Add01Icon} size={14} />} aria-expanded={other} onClick={() => setOther(!other)}>
            {t("登录其他网站")}
          </Button>
        </div>
        <Collapse open={other}>
          <div className="cp-site-other">
            <TextField
              size="sm"
              aria-label={t("要登录的网站")}
              placeholder={t("网址，比如 https://www.example.com")}
              value={url}
              inputClassName="mono"
              onChange={(e) => setUrl(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && openOther()}
              error={url.trim() && !typed ? t("这不像一个网址") : undefined}
            />
            <Button variant="secondary" size="sm" className="cp-pill" disabled={!typed} onClick={openOther}>
              {t("打开")}
            </Button>
          </div>
        </Collapse>
      </div>
      <div className="cp-group cp-group-next">
        <div className="cp-set-sw">
          <Switch
            placement="end"
            label={t("也读取浏览器里的登录状态")}
            description={t(COOKIES_NOTE)}
            checked={!!browser}
            onChange={(e) => setSettings({ cookiesFromBrowser: e.target.checked ? firstBrowser : "" })}
          />
        </div>
        <Collapse open={!!browser}>
          <Row title={t("从哪个浏览器读")} sub={browserHint(browser)}>
            <Select size="sm" aria-label={t("从哪个浏览器读")} className="cp-set-select" value={browser || firstBrowser} onChange={(v) => setSettings({ cookiesFromBrowser: v })} options={BROWSERS} />
          </Row>
        </Collapse>
      </div>
    </>
  );
}

/* ---------------- 网络 ---------------- */

const PROXY_RE = /^(https?|socks5h?):\/\/[^\s/]+:\d+\/?$/i;

function Proxy({ s, open }: { s: Settings; open: boolean }) {
  const mode = s.proxyMode ?? "system";
  const [net, setNet] = useState<ProxyStatus | null>(null);
  useEffect(() => {
    if (open)
      invoke<ProxyStatus>("proxy_status")
        .then(setNet)
        .catch(() => setNet(null));
  }, [open, mode, s.proxyUrl]);
  const sys = net?.system;
  const sysAddr = sys?.https || sys?.http || (sys?.socks ? `socks5://${sys.socks}` : "");
  const sub =
    mode === "none"
      ? t("不经过代理，直接连接")
      : mode === "manual"
        ? t("用下面填的地址")
        : !net
          ? t("跟随系统设置")
          : sys?.pac
            ? t("跟随系统：系统用的是自动代理配置（PAC），Captae 按直连处理")
            : sysAddr
              ? t("跟随系统：{addr}", { addr: sysAddr })
              : t("跟随系统：系统里没有设代理");
  return (
    <>
      <Row title={t("代理")} sub={sub}>
        <Select
          size="sm"
          aria-label={t("代理")}
          className="cp-set-select wide"
          value={mode}
          onChange={(v) => setSettings({ proxyMode: v as Settings["proxyMode"] })}
          options={[
            { value: "system", label: t("跟随系统") },
            { value: "none", label: t("不用代理") },
            { value: "manual", label: t("手动填写") },
          ]}
        />
      </Row>
      <Collapse open={mode === "manual"}>
        <div className="cp-set-fields">
          <LazyField
            k="proxyUrl"
            value={s.proxyUrl ?? ""}
            label={t("代理地址")}
            placeholder={t("http://127.0.0.1:7890 或 socks5://127.0.0.1:1080")}
            check={(v) => (v && !PROXY_RE.test(v) ? t("要写成 协议://地址:端口，比如 http://127.0.0.1:7890") : undefined)}
          />
        </div>
      </Collapse>
    </>
  );
}

/* ---------------- 下载引擎 ---------------- */

const DEP_NAME: Record<string, string> = { ffmpeg: "ffmpeg", aria2: "aria2", deno: "deno", python: "Python" };

/** 「deno 2.9.8 (stable, release, aarch64-apple-darwin)」「ffmpeg version 8.0.1 Copyright…」只留版本号 */
function shortVersion(v?: string | null): string {
  const m = (v ?? "").match(/\d+(\.\d+)+/);
  return m ? m[0] : (v ?? "");
}

function Engines({ s, open }: { s: Settings; open: boolean }) {
  const deps = useApp((st) => st.deps);
  // undefined：还没查；null：后端还没有这个命令（第一阶段做完以前）
  const [eng, setEng] = useState<EngineStatus | null | undefined>(undefined);
  const [checking, setChecking] = useState(false);
  const loadEng = () =>
    invoke<EngineStatus>("engine_status")
      .then(setEng)
      .catch(() => setEng(null));
  useEffect(() => {
    if (open) loadEng();
  }, [open]);
  const yt = deps?.find((d) => d.key === "ytdlp");
  const version = eng?.version ?? yt?.version;
  const custom = eng?.source === "custom";
  const sub = !version
    ? deps
      ? t("没找到下载组件，重新安装 Captae 可以修好")
      : t("正在检查…")
    : custom
      ? t("用的是你在设置里指定的 yt-dlp，不会自动更新")
      : eng?.checking
        ? t("正在检查更新…")
        : eng?.latest && eng.latest !== version
          ? t("有新版{version}", { version: eng.latest })
          : eng?.lastError
            ? t("上次没检查成：{error}", { error: eng.lastError })
            : eng?.lastCheck
              ? t("已经是最新的 · 上次检查：{when}", { when: whenLabel(eng.lastCheck) })
              : t("网站改版以后，要新版的下载引擎才读得到视频");
  const check = () => {
    setChecking(true);
    invoke<EngineReport>("engine_update")
      .then((r) => {
        if (r.result === "updated") notify(t("下载引擎已更新到{version}", { version: r.active ?? r.latest ?? "" }), t("之后的下载都用新版本"));
        else if (r.result === "latest") notify(t("已经是最新的"), `yt-dlp ${r.current ?? version ?? ""}`);
        else notify(r.result === "skipped" ? t("这次没有检查") : t("没有更新"), r.message);
      })
      .catch((e) => notify(t("没有检查成"), errText(e)))
      .finally(() => {
        setChecking(false);
        loadEng();
        loadDeps();
      });
  };
  // JS 运行时：yt-dlp 读 YouTube 要用（安装包自带 deno；设置里指定的、系统里的 node / bun 也行）
  const js = deps?.find((d) => d.key === "deno");
  const jsKind = eng?.jsRuntime?.kind ?? "deno";
  const others = (deps ?? []).filter((d) => d.key !== "ytdlp" && d.key !== "whisper" && d.key !== "deno");
  return (
    <>
      <Row title={`yt-dlp${version ? ` ${version}` : ""}`} sub={sub}>
        {eng !== null && !custom && (
          <Button variant="secondary" size="sm" className="cp-pill" loading={checking || !!eng?.checking} onClick={check}>
            {t("检查更新")}
          </Button>
        )}
      </Row>
      <div className="cp-set-sw">
        <Switch
          placement="end"
          label={t("自动更新下载引擎")}
          description={t("网站改版后旧版本会读不到视频。每天查一次，核对官方签名后才换，新版出问题会自动换回旧版")}
          checked={s.autoUpdate ?? true}
          onChange={(e) => setSettings({ autoUpdate: e.target.checked })}
        />
      </div>
      {(js || others.length > 0) && (
        <div className="cp-engines">
          {js && (
            <span data-bad={!js.ok || undefined}>
              {jsKind} {js.ok ? shortVersion(js.version) : t("没找到")}
              <small>{js.ok ? t("读取 YouTube 用") : t("YouTube 可能读不了")}</small>
            </span>
          )}
          {others.map((d) => (
            <span key={d.key} data-bad={!d.ok || undefined}>
              {DEP_NAME[d.key] ?? d.key} {d.ok ? shortVersion(d.version) : t("没找到")}
            </span>
          ))}
        </div>
      )}
    </>
  );
}

/* ---------------- 字幕：语音转文字、翻译 ---------------- */

function Whisper() {
  const { state, model, task } = useWhisper();
  const m = modelInfo(model);
  const sub =
    state === "checking"
      ? t("正在检查…")
      : state === "ready"
        ? t("可以用 · 在这台电脑上识别，不上传")
        : state === "need_model"
          ? t("「{name}」模型还没下载 · {size}", { name: m.label, size: m.model })
          : state === "installing"
            ? task?.note || t("正在准备…")
            : t("给没有字幕的视频生成字幕 · 要下载 {size}", { size: m.full });
  return (
    <div className="cp-set-row">
      <div>
        <b>{t("语音转文字")}</b>
        <small>{sub}</small>
        {state === "installing" && (
          <div className="cp-tprog">
            <ProgressBar p={(task?.pct ?? 0) / 100} indeterminate={!task || task.pct <= 0} tick={900} label={t("安装语音识别")} />
            <span className="num">{etaText(task?.eta) || `${Math.round(task?.pct ?? 0)}%`}</span>
          </div>
        )}
      </div>
      <div className="acts">
        <Select
          size="sm"
          aria-label={t("识别模型")}
          className="cp-set-select"
          value={model}
          disabled={state === "installing"}
          onChange={(v) => setSettings({ whisperModel: v })}
          options={WHISPER_MODELS.map((x) => ({ value: x.value, label: x.label, description: x.model }))}
        />
        {(state === "need_model" || state === "need_install") && (
          <Button variant="secondary" size="sm" className="cp-pill" onClick={installWhisper}>
            {state === "need_model" ? t("下载") : t("安装")}
          </Button>
        )}
      </div>
    </div>
  );
}

function Translate({ s }: { s: Settings }) {
  const [open, setOpen] = useState(false);
  const sub = s.llmBaseUrl ? `${t("已设置")} · ${bare(s.llmBaseUrl).slice(0, 40)}${s.llmModel ? ` · ${s.llmModel}` : ""}` : t("填一个 OpenAI 兼容的接口，本地的 Ollama 也可以");
  return (
    <>
      <Row title={t("字幕翻译")} sub={sub}>
        <Button variant="secondary" size="sm" className="cp-pill" aria-expanded={open} onClick={() => setOpen(!open)}>
          {open ? t("收起") : s.llmBaseUrl ? t("修改") : t("填写")}
        </Button>
      </Row>
      <Collapse open={open}>
        <div className="cp-set-fields">
          <LazyField k="llmBaseUrl" value={s.llmBaseUrl} label={t("接口地址")} placeholder="http://localhost:11434/v1" />
          <LazyField k="llmModel" value={s.llmModel} label={t("模型")} placeholder="qwen2.5:7b" />
          <LazyField k="llmApiKey" value={s.llmApiKey} label={t("密钥（本地的可以不填）")} placeholder="sk-…" type="password" />
        </div>
      </Collapse>
    </>
  );
}

/** 输入框：打字时只改本地，离开输入框（或回车）才保存，免得每个字都写一次设置 */
function LazyField({
  k,
  value,
  label,
  placeholder,
  type,
  check,
}: {
  k: "llmBaseUrl" | "llmModel" | "llmApiKey" | "proxyUrl";
  value: string;
  label: string;
  placeholder?: string;
  type?: string;
  check?: (v: string) => string | undefined;
}) {
  const [v, setV] = useState(value);
  useEffect(() => setV(value), [value]);
  const err = check?.(v.trim());
  const save = () => {
    const val = v.trim();
    if (!err && val !== value) setSettings({ [k]: val } as Partial<Settings>);
  };
  return (
    <TextField
      size="sm"
      label={label}
      placeholder={placeholder}
      type={type}
      value={v}
      error={err}
      inputClassName="mono"
      spellCheck={false}
      autoComplete="off"
      onChange={(e) => setV(e.target.value)}
      onBlur={save}
      onKeyDown={(e) => e.key === "Enter" && save()}
    />
  );
}
