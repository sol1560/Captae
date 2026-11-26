// 从 affectUI 的公开 registry 拉免费组件（MIT），原样写进 src/components/ui/，不改源码。
// 用法：node scripts/fetch-affectui.mjs button dialog ...
// 会顺着 registryDependencies 把依赖的组件一起拉下来；付费组件返回 401，直接报出来，不会拉。
import { mkdir, writeFile, readFile } from "node:fs/promises";
import path from "node:path";

const out = path.resolve("src/components/ui");
const manifestPath = path.resolve("src/components/ui/AFFECTUI-SOURCES.json");
await mkdir(out, { recursive: true });

let manifest = {};
try {
  manifest = JSON.parse(await readFile(manifestPath, "utf8"));
} catch {}

const seen = new Set();
const failed = [];

async function pull(slug) {
  if (seen.has(slug)) return;
  seen.add(slug);
  const url = `https://www.affectui.com/r/${slug}.json`;
  const res = await fetch(url);
  if (!res.ok) {
    failed.push(`${slug}（HTTP ${res.status}）`);
    return;
  }
  const item = await res.json();
  for (const f of item.files ?? []) {
    const name = path.basename(f.path);
    await writeFile(path.join(out, name), f.content);
  }
  manifest[slug] = {
    title: item.title,
    source: url,
    files: (item.files ?? []).map((f) => path.basename(f.path)),
    npmDependencies: item.dependencies ?? [],
    fetchedAt: new Date().toISOString().slice(0, 10),
    license: "MIT（affectUI 官网：74 items are free and open source (MIT)）",
  };
  for (const dep of item.registryDependencies ?? []) {
    // 依赖可能写成完整地址或短名
    const m = String(dep).match(/\/r\/([a-z0-9-]+)\.json$/);
    await pull(m ? m[1] : String(dep).replace(/^@affectui\//, ""));
  }
}

for (const slug of process.argv.slice(2)) await pull(slug);
await writeFile(manifestPath, JSON.stringify(manifest, null, 2));
console.log("已拉取：", Object.keys(manifest).length, "个");
console.log("npm 依赖：", [...new Set(Object.values(manifest).flatMap((m) => m.npmDependencies))].join(", ") || "无");
if (failed.length) console.log("没拉到：", failed.join("、"));
