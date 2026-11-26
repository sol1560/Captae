// 工具栏面板的入口：先装好和后台说话的通道（演示构建在普通浏览器里用假后台），再画界面。
// 面板是浏览器自己画的小窗口，打开时不加入场动画（配方 l4）。
// 界面语言由插件后台定（Captae 里选的语言，或者浏览器的界面语言，见 extension/background.js 的 syncLang），词表加载好了再画。
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@/theme.css";
import { loadAndSetLang, parseLang, setCatalogs } from "@/i18n";
import { inExtension, send, setup } from "./bridge";
import { Popup } from "./Popup";

// 只打进插件用得到的两个分册
setCatalogs(import.meta.glob("../i18n/locales/*/{common,popup}.json", { import: "default" }));

async function pickLang() {
  // 演示构建：&lang=en 之类，默认中文
  const code = inExtension ? await send<string>({ t: "lang" }).catch(() => null) : new URLSearchParams(location.search).get("lang") || "zh-CN";
  await loadAndSetLang(parseLang(code) ?? parseLang(navigator.language) ?? "en");
}

setup()
  .then(pickLang)
  .catch((e) => console.error(e))
  .finally(() =>
    createRoot(document.getElementById("root")!).render(
      <StrictMode>
        <Popup />
      </StrictMode>,
    ),
  );
