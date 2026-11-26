// 浏览器插件的工具栏面板：React 19 + affectUI 免费组件，和桌面端共用组件、theme.css 的变量和 parts。
// 正式构建（npm run build:popup）输出到 extension/popup/，构建产物提交进仓库：插件文件夹拿来就能加载，不用再构建。
//   - base 用相对路径（chrome-extension://<id>/popup/…）；文件名不带哈希，每次构建只有真正改了的文件会变
//   - 插件页面的 CSP 不许内联脚本：关掉 modulepreload 的内联补丁
// 演示构建（--mode mock）打成单个 HTML，带演示数据（src/extension-popup/mock.ts），在普通浏览器里带 ?mock 打开截图用；演示数据不进正式构建。
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { viteSingleFile } from "vite-plugin-singlefile";
import { fileURLToPath } from "node:url";

const src = fileURLToPath(new URL("./src", import.meta.url));

export default defineConfig(({ mode }) => ({
  root: fileURLToPath(new URL("./src/extension-popup", import.meta.url)),
  base: "./",
  plugins: [react(), ...(mode === "mock" ? [viteSingleFile()] : [])],
  resolve: { alias: { "@": src } },
  clearScreen: false,
  build:
    mode === "mock"
      ? // 演示构建放在 apps/desktop/dist-popup-mock/（不进仓库，.gitignore 里写着，和 dist-mock/ 一样）
        { outDir: fileURLToPath(new URL("./dist-popup-mock", import.meta.url)), emptyOutDir: true, target: "es2022", assetsInlineLimit: 100_000_000, chunkSizeWarningLimit: 100_000 }
      : {
          outDir: fileURLToPath(new URL("../../extension/popup", import.meta.url)),
          emptyOutDir: true,
          target: "es2022",
          modulePreload: { polyfill: false },
          chunkSizeWarningLimit: 1000,
          rollupOptions: {
            output: {
              entryFileNames: "assets/popup.js",
              // 词表（src/i18n/locales/<语言>/<分册>.json）每个一块，按语言和分册起名，文件名每次构建都一样
              chunkFileNames: (c) => {
                const m = c.facadeModuleId?.match(/locales\/([^/]+)\/([^/]+)\.json$/);
                return m ? `assets/i18n/${m[1]}-${m[2]}.js` : "assets/[name].js";
              },
              assetFileNames: "assets/[name][extname]",
            },
          },
        },
}));
