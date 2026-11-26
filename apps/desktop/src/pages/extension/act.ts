// 浏览器插件页的数据和操作：连接状态（这页开着时 5 秒看一次，和旧界面一样；插件每分钟打一次招呼）、
// 测试连接、重新登记宿主、插件文件夹、打开扩展程序页、选浏览器（设置 browser）。
import { useCallback, useEffect, useState } from "react";
import { N_, t } from "@/i18n";
import { copyText, errText, invoke, platform, reveal } from "@/lib/bridge";
import { notify } from "@/lib/notify";
import { setSettings } from "@/lib/store";
import type { BrowserInfo, ExtStatus, ExtTest, HostStatus } from "./model";

/** 复制了扩展程序页的地址、插件文件夹已经在文件管理器里显示出来时的那句话（文件管理器的叫法按系统，见 lib/os.ts） */
const SHOWN_IN =
  platform === "mac"
    ? N_("在 {name} 的地址栏粘贴打开，插件文件夹已经在访达里显示")
    : platform === "win"
      ? N_("在 {name} 的地址栏粘贴打开，插件文件夹已经在文件资源管理器里显示")
      : N_("在 {name} 的地址栏粘贴打开，插件文件夹已经在文件管理器里显示");

/** 上一次看到的状态：再回到这一页时先用它，不闪一下「正在看看」 */
let last: ExtStatus | null = null;

/** 连接状态；第二个返回值是马上再看一次（测试、修复之后），第三个是直接换成测试结果里的 */
export function useExtStatus(): [ExtStatus | null, () => void, (s: ExtStatus) => void] {
  const [st, setRaw] = useState<ExtStatus | null>(last);
  const setSt = useCallback((s: ExtStatus) => {
    last = s;
    setRaw(s);
  }, []);
  const load = useCallback(() => {
    invoke<ExtStatus>("extension_status")
      .then((s) => s && setSt(s))
      .catch(() => !last && setSt({ connected: false, lastSeen: null, version: null, browser: null }));
  }, [setSt]);
  useEffect(() => {
    load();
    const timer = setInterval(() => !document.hidden && load(), 5000);
    return () => clearInterval(timer);
  }, [load]);
  return [st, load, setSt];
}

/** 插件文件夹（安装包里的 Resources/extension）；找不到是空字符串，还没问到是 null */
export function useExtPath(): string | null {
  const [p, setP] = useState<string | null>(null);
  useEffect(() => {
    invoke<string>("extension_path")
      .then((x) => setP(x || ""))
      .catch(() => setP(""));
  }, []);
  return p;
}

/** 测试连接：失败也返回一份结果，界面照样把三步画出来 */
export function runTest(): Promise<ExtTest> {
  return invoke<ExtTest>("extension_test").catch((e) => ({ host: { ok: false }, reply: null, error: errText(e) }));
}

/** 重新登记宿主（给装了的浏览器重写清单） */
export function register(): Promise<HostStatus | null> {
  return invoke<HostStatus>("extension_register")
    .then((h) => {
      notify(t("已经重新登记"), t("浏览器下次叫 Captae 就能找到了"));
      return h;
    })
    .catch((e) => {
      notify(t("没修好"), errText(e));
      return null;
    });
}

/** 打开扩展程序页：后端认识的浏览器直接打开（顺便在文件管理器里显示插件文件夹）；不认识的复制地址、显示文件夹 */
export function openExtPage(b: BrowserInfo, path: string) {
  const fallback = (why?: string) =>
    copyText(b.page)
      .then(() => {
        if (path) reveal(path).catch(() => undefined);
        const how = path ? t(SHOWN_IN, { name: b.name }) : t("在 {name} 的地址栏粘贴打开", { name: b.name });
        // 打不开的原因（后端的说法）在前，接下来怎么做在后
        notify(t("已复制 {page}", { page: b.page }), why ? t("{reason}。{next}", { reason: why, next: how }) : how);
      })
      .catch(() => notify(t("在 {name} 的地址栏输入", { name: b.name }), b.page));
  if (!b.help) return fallback();
  invoke("extension_install_help", { browser: b.id })
    .then(() => notify(t("已打开 {name}", { name: b.name }), t("如果没有自动跳到扩展程序页，在地址栏输入 {page}", { page: b.page })))
    .catch((e) => fallback(errText(e)));
}

export function chooseBrowser(id: string) {
  setSettings({ browser: id });
}

export function revealPath(path: string) {
  reveal(path).catch((e) => notify(t("打不开"), errText(e)));
}
