// Captae 桌面端界面：React 19 + Vite。
// 正式构建（npm run build）输出到 dist/，Tauri 打包时嵌进安装包；
// 演示构建（npm run build:mock）输出到 dist-mock/，打成单个 HTML，带演示后端（src/dev/），
// 用无界面 Chrome 从 file:// 打开截图检查（演示后端的参数见 docs/DESIGN.md 第 9 节）。演示后端不会进正式构建。
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { viteSingleFile } from "vite-plugin-singlefile";
import { fileURLToPath } from "node:url";

export default defineConfig(({ mode }) => ({
  plugins: [react(), ...(mode === "mock" ? [viteSingleFile()] : [])],
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  clearScreen: false,
  server: { port: 5199, strictPort: true },
  build:
    mode === "mock"
      ? { outDir: "dist-mock", target: "safari16", assetsInlineLimit: 100_000_000, chunkSizeWarningLimit: 100_000 }
      : { outDir: "dist", target: "safari16", chunkSizeWarningLimit: 2000 },
}));
