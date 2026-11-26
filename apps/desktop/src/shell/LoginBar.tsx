// 登录窗口里 Captae 自己的那一层（index.html#/login-bar）：铺满窗口，上面一条工具条，下面一行说明；
// 网站自己的页面是另一块 webview，由 Rust 盖在中间（上 58、下 52 逻辑像素，见 src-tauri/src/login.rs 的 TOP / BOTTOM）。
// 状态由 Rust 推过来（window.captaeLoginBar），刚加载好时自己取一次（login_bar_state）。
// 这块 webview 只能调 login_bar_state / login_done / login_cancel / js_log（main.rs 的白名单）。
// 窗口的出现和消失交给系统，这里不加动画（配方 g）；要登录、要验证的提示静止不动（配方 m）。
import { SquareLock02Icon, SquareUnlock02Icon } from "@hugeicons/core-free-icons";
import { useEffect, useState } from "react";
import { Button } from "@/components/ui/button";
import { t, tAny } from "@/i18n";
import { invoke } from "@/lib/bridge";
import type { LoginBarState } from "@/lib/types";
import { Icon } from "@/parts";
import "./login-bar.css";

declare global {
  interface Window {
    captaeLoginBar?: (s: LoginBarState | null) => void;
  }
}

/** 工具条中间那句话（等着的任务数 n 拼在整句里，不分开翻译） */
export function barMessage(s: LoginBarState): string {
  const n = s.waiting;
  if (s.kind === "verify") return n > 0 ? t("完成验证后点「完成」，会接着下载{n}个任务", { n }) : t("完成验证后点「完成」");
  if (s.auto) return n > 0 ? t("登录好会自动接着下载{n}个任务", { n }) : t("登录好这个窗口会自己关上");
  return n > 0 ? t("登录好以后点「完成」，会接着下载{n}个任务", { n }) : t("登录好以后点「完成」");
}

/** 底下那行说明。账号密码交给的是当前打开的网站：YouTube 用 Google 账号登录，这时说「交给 Google」。
 *  s.name 是后端存的网站名（中文原文），显示时翻译 */
export function footNote(s: LoginBarState): string {
  if (s.kind === "verify") return t("验证在{site}自己的网页里完成，Captae 不会替你点。验证结果只留在这台电脑上，在「设置 › 已登录的网站」里可以清除。", { site: tAny(s.name) });
  const who = /(^|\.)google\.com$/.test(s.host) ? "Google" : tAny(s.name);
  return t("账号密码直接交给{site}，Captae 看不到。登录状态只留在这台电脑上，在「设置 › 已登录的网站」里随时可以退出。", { site: who });
}

export function LoginBar() {
  const [s, setS] = useState<LoginBarState | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    window.captaeLoginBar = (x) => {
      setS(x);
      setBusy(false);
    };
    invoke<LoginBarState | null>("login_bar_state")
      .then((x) => x && setS(x))
      .catch(() => undefined);
    return () => {
      delete window.captaeLoginBar;
    };
  }, []);
  const cancel = () => invoke("login_cancel").catch(() => undefined);
  const done = () => {
    setBusy(true);
    invoke("login_done").catch(() => setBusy(false));
  };
  // Esc = 取消（和系统弹窗一样）
  useEffect(() => {
    const k = (e: KeyboardEvent) => {
      if (e.key === "Escape") cancel();
    };
    addEventListener("keydown", k);
    return () => removeEventListener("keydown", k);
  }, []);
  if (!s) return <div className="cp cp-loginpage" />;
  return (
    <div className="cp cp-loginpage">
      <div className="cp-loginbar">
        <span className="site" title={s.secure ? t("加密连接") : t("这个网页没有加密，别在这里输密码")} data-insecure={!s.secure || undefined}>
          <Icon icon={s.secure ? SquareLock02Icon : SquareUnlock02Icon} size={13} />
          {s.host || tAny(s.name)}
        </span>
        <span className="msg">{barMessage(s)}</span>
        <div className="end">
          <Button variant="ghost" size="sm" onClick={cancel}>
            {t("取消")}
          </Button>
          <Button size="sm" className="cp-pill" loading={busy} onClick={done}>
            {t("完成")}
          </Button>
        </div>
      </div>
      <div className="cp-loginfoot">
        <span>{footNote(s)}</span>
      </div>
    </div>
  );
}
