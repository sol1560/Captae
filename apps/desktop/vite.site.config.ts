// 官网落地页（src/site/）：和桌面端共用 theme.css 的变量、affectUI 免费组件、parts 和 App 的词表，不进程序。
// npm run dev:site 本地看；npm run build:site 输出到 apps/desktop/dist-site/（不进仓库，.gitignore 里写着），整个文件夹放到静态托管上就能用。
// 两件额外的事（下面两个小插件）：
// - 多语言：根地址和每种语言（/zh-cn/、/en/……）各出一个页面，标题、摘要、<html lang> 各用各的，互相标出其他语言的地址，方便搜索引擎收录；
//   页面内容一样，打开以后按网址里的语言画（src/site/i18n.ts）。
// - 演示版：「界面一览」嵌的是 App 的演示版（npm run build:mock 的产物 dist-mock/index.html），发布在 /demo/；本地看时开发服务器也在 /demo/ 给它。
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { zhCN } from "./src/site/copy";

const src = fileURLToPath(new URL("./src", import.meta.url));
const out = fileURLToPath(new URL("./dist-site", import.meta.url));
const demoHtml = fileURLToPath(new URL("./dist-mock/index.html", import.meta.url));
const SITE = "https://captae.app";
// 和 App 的语言一样（src/i18n/index.ts 的 LANGS）
const LANGS = ["zh-CN", "zh-TW", "en", "ja", "ko", "de", "es", "fr"];
const slug = (l: string) => l.toLowerCase();

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
const metaOf = (l: string): { title: string; description: string } =>
  l === "zh-CN" ? zhCN.meta : JSON.parse(readFileSync(join(src, "site/locales", `${l}.json`), "utf8")).meta;

function localized(): Plugin {
  return {
    name: "captae-site-langs",
    apply: "build",
    closeBundle() {
      const base = readFileSync(join(out, "index.html"), "utf8");
      const alternates = [...LANGS.map((l) => `<link rel="alternate" hreflang="${l}" href="${SITE}/${slug(l)}/" />`), `<link rel="alternate" hreflang="x-default" href="${SITE}/" />`].join("\n    ");
      const page = (l: string, canonical: string) => {
        const m = metaOf(l);
        return base
          .replace(/<html lang="[^"]*">/, `<html lang="${l}">`)
          .replace(/<title>.*?<\/title>/, `<title>${esc(m.title)}</title>`)
          .replace(/<meta name="description" content=".*?" \/>/, `<meta name="description" content="${esc(m.description)}" />`)
          .replace("</head>", `    <link rel="canonical" href="${canonical}" />\n    ${alternates}\n  </head>`);
      };
      for (const l of LANGS) {
        mkdirSync(join(out, slug(l)), { recursive: true });
        writeFileSync(join(out, slug(l), "index.html"), page(l, `${SITE}/${slug(l)}/`));
      }
      // 根地址：打开以后按浏览器语言换到 /<语言>/；静态的标题和摘要用英文
      writeFileSync(join(out, "index.html"), page("en", `${SITE}/`));
    },
  };
}

function demo(): Plugin {
  const need = () => {
    if (!existsSync(demoHtml)) throw new Error("没有演示版：先在 apps/desktop 里 npm run build:mock");
  };
  return {
    name: "captae-site-demo",
    configureServer(server) {
      server.middlewares.use("/demo", (req, res, next) => {
        if (req.url && req.url !== "/" && !req.url.startsWith("/?")) return next();
        need();
        res.setHeader("Content-Type", "text/html; charset=utf-8");
        res.end(readFileSync(demoHtml));
      });
    },
    closeBundle() {
      need();
      mkdirSync(join(out, "demo"), { recursive: true });
      copyFileSync(demoHtml, join(out, "demo", "index.html"));
    },
  };
}

export default defineConfig({
  root: fileURLToPath(new URL("./src/site", import.meta.url)),
  base: "/",
  plugins: [react(), localized(), demo()],
  resolve: { alias: { "@": src } },
  clearScreen: false,
  // 本机装依赖时 fsevents 的安装脚本被跳过了，开发服务器收不到文件改动的通知（改了代码页面还是旧的）：改成每 0.3 秒查一次
  server: { port: 5181, watch: { usePolling: true, interval: 300 } },
  build: {
    outDir: out,
    emptyOutDir: true,
    target: "es2022",
    chunkSizeWarningLimit: 1000,
  },
});
