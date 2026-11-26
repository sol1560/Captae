// 演示后端：浏览器插件页的命令。返回结构照真实后端写（apps/desktop/src-tauri/src/ext_cmds.rs、main.rs 的 extension_path /
// extension_install_help；字段见 crates/captae-core/src 的 tasks.rs ExtStatus、native_host.rs Status / BrowserStatus 和宿主对 hello 的回复）。
//
// 演示数据：这台电脑装了 Chrome、Arc、Aside，都已经登记；插件 20 秒前从 Chrome 打过招呼。
// 网址参数（和 mock.ts 的一起用）：
//   &ext=off     插件从来没连上过（第一次装）
//   &ext=gone    以前连上过，现在没连着（浏览器关了）
//   &ext=fix     Aside 装了但还没登记（「修复」「重新登记」会把它登记上）
//   &ext=nohost  找不到 Captae 自带的 captae 程序
//   &ext=nohost-on  找不到 captae 程序，但以前登记好的浏览器里插件还连着（测试版外壳里没放 captae 时就是这样）
//   &extfrom=arc 插件是从哪家浏览器连上来的（宿主看父进程认出来的 id：arc / aside / vivaldi / chrome-beta……），默认 chrome
import type { MockHandler } from "./mock";

const qs = new URLSearchParams(location.search);
const MODE = qs.get("mock") === "empty" ? "off" : qs.get("ext");
const FROM = qs.get("extfrom") || "chrome";
const T0 = Date.now();
const HOME = "/Users/demo";
const DATA = `${HOME}/Library/Application Support/app.Captae.Captae`;
const EXT = "/Applications/Captae.app/Contents/Resources/extension";
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** native_host.rs browsers() 的 16 个，顺序一样；装没装是这台演示电脑的情况 */
const ALL: [string, string, string, string[]?][] = [
  ["chrome", "Chrome", "Google/Chrome"],
  ["chrome-beta", "Chrome Beta", "Google/Chrome Beta"],
  ["chrome-dev", "Chrome Dev", "Google/Chrome Dev"],
  ["chrome-canary", "Chrome Canary", "Google/Chrome Canary"],
  ["chrome-for-testing", "Chrome for Testing", "Google/Chrome for Testing", ["Chromium"]],
  ["chromium", "Chromium", "Chromium"],
  ["edge", "Edge", "Microsoft Edge"],
  ["edge-beta", "Edge Beta", "Microsoft Edge Beta"],
  ["edge-dev", "Edge Dev", "Microsoft Edge Dev"],
  ["edge-canary", "Edge Canary", "Microsoft Edge Canary"],
  ["brave", "Brave", "BraveSoftware/Brave-Browser"],
  ["vivaldi", "Vivaldi", "Vivaldi"],
  ["opera", "Opera", "com.operasoftware.Opera", ["Google/Chrome"]],
  ["opera-gx", "Opera GX", "com.operasoftware.OperaGX", ["Google/Chrome"]],
  ["arc", "Arc", "Arc/User Data"],
  ["aside", "Aside", "Aside"],
];
const INSTALLED = new Set(["chrome", "arc", "aside"]);
const registered = new Set(MODE === "fix" ? ["chrome", "arc"] : ["chrome", "arc", "aside"]);

const nmh = (root: string) => `${HOME}/Library/Application Support/${root}/NativeMessagingHosts/app.captae.bridge.json`;

function host() {
  if (MODE === "nohost" || MODE === "nohost-on") return { ok: false, missing: true };
  const browsers = ALL.map(([id, name, root, also]) => ({
    id,
    name,
    installed: INSTALLED.has(id),
    registered: registered.has(id),
    manifest: nmh(root),
    ...(also ? { also: also.map(nmh) } : {}),
  }));
  return {
    exe: "/Applications/Captae.app/Contents/MacOS/captae",
    wrapper: `${DATA}/captae-native-host`,
    wrapperOk: true,
    browsers,
    ok: browsers.filter((b) => b.installed).every((b) => b.registered),
  };
}

function seen() {
  if (MODE === "off" || MODE === "nohost") return { connected: false, lastSeen: null, version: null, browser: null };
  if (MODE === "gone") return { connected: false, lastSeen: T0 - 3 * 3600_000, version: "0.4.0", browser: FROM };
  return { connected: true, lastSeen: Date.now() - 20_000, version: "0.4.0", browser: FROM };
}

export const extensionHandlers: Record<string, MockHandler> = {
  extension_status: () => ({ ...seen(), host: host() }),
  extension_register: async () => {
    if (MODE === "nohost") throw new Error("没有找到 captae 命令行程序");
    await sleep(300);
    for (const id of INSTALLED) registered.add(id);
    return host();
  },
  extension_test: async (_a, ctx) => {
    await sleep(ctx.still ? 0 : 900);
    const h = host();
    if (!h.ok) return { host: h, reply: null, error: "宿主还没有注册", extension: seen() };
    return { host: h, reply: { app: true, version: "0.9.0", clipboard: true }, error: null, extension: seen() };
  },
  extension_path: () => EXT,
  extension_install_help: ({ browser }) => {
    const b = browser || "chrome";
    if (!INSTALLED.has(b)) throw new Error(`没有找到 ${b === "edge" ? "Microsoft Edge" : b === "brave" ? "Brave Browser" : b === "arc" ? "Arc" : "Google Chrome"}，请先安装它，或者在设置里换一个浏览器`);
    return EXT;
  },
};
