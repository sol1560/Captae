// 安装包（dmg）窗口背景图的源码页（src/dmg-background/）：打成单个 HTML，给 scripts/render-dmg-background.sh 截图用，不进程序。
// 输出到 apps/desktop/dist-dmg/（不进仓库，.gitignore 里写着）。
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { viteSingleFile } from "vite-plugin-singlefile";
import { fileURLToPath } from "node:url";

const src = fileURLToPath(new URL("./src", import.meta.url));

export default defineConfig({
  root: fileURLToPath(new URL("./src/dmg-background", import.meta.url)),
  base: "./",
  plugins: [react(), viteSingleFile()],
  resolve: { alias: { "@": src } },
  clearScreen: false,
  logLevel: "warn",
  build: { outDir: fileURLToPath(new URL("./dist-dmg", import.meta.url)), emptyOutDir: true, target: "es2022", assetsInlineLimit: 100_000_000, chunkSizeWarningLimit: 100_000 },
});
