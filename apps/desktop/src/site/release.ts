// 官网「下载」那块的数据：GitHub 上最新一版正式发布（不算预发布和草稿）的安装包。这里只放不依赖页面的部分，
// 构建时 vite.site.config.ts 也用它先问一次 GitHub；页面里怎么用见 download.tsx。
// 只认安装包：macOS 的 .dmg、Windows 的 -setup.exe / .msi、Linux 的 .deb / .rpm / .AppImage；源码包、校验文件不列。
import { GITHUB_REPO, RELEASES } from "./copy";

export type Os = "mac" | "win" | "linux";
export type Arch = "arm64" | "x64";
export type Format = "dmg" | "exe" | "msi" | "deb" | "rpm" | "appimage";

export type Asset = { name: string; size: number; url: string };
export type Release = { tag: string; published: string; page: string; assets: Asset[] };
export type Installer = Asset & { os: Os; arch: Arch | null; format: Format };

/** GitHub 的接口：最新一版正式发布 */
export const LATEST_API = `https://api.github.com/repos/${GITHUB_REPO}/releases/latest`;

/** GitHub 接口返回的 JSON → 这里用的几项；结构不对返回 null */
export function parseRelease(j: unknown): Release | null {
  const r = j as { tag_name?: unknown; published_at?: unknown; html_url?: unknown; assets?: unknown } | null;
  if (!r || typeof r.tag_name !== "string" || !Array.isArray(r.assets)) return null;
  const assets: Asset[] = [];
  for (const a of r.assets as { name?: unknown; size?: unknown; browser_download_url?: unknown }[]) {
    if (typeof a?.name === "string" && typeof a.size === "number" && typeof a.browser_download_url === "string") {
      assets.push({ name: a.name, size: a.size, url: a.browser_download_url });
    }
  }
  return {
    tag: r.tag_name,
    published: typeof r.published_at === "string" ? r.published_at : "",
    page: typeof r.html_url === "string" ? r.html_url : RELEASES,
    assets,
  };
}

/** 从文件名认出是哪个系统、什么格式、什么芯片；不是安装包返回 null */
function classify(a: Asset): Installer | null {
  const n = a.name.toLowerCase();
  const arch: Arch | null = /aarch64|arm64/.test(n) ? "arm64" : /x64|x86_64|amd64/.test(n) ? "x64" : null;
  const hit = (os: Os, format: Format): Installer => ({ ...a, os, arch, format });
  if (n.endsWith(".dmg")) return hit("mac", "dmg");
  if (n.endsWith("-setup.exe")) return hit("win", "exe");
  if (n.endsWith(".msi")) return hit("win", "msi");
  if (n.endsWith(".deb")) return hit("linux", "deb");
  if (n.endsWith(".rpm")) return hit("linux", "rpm");
  if (n.endsWith(".appimage")) return hit("linux", "appimage");
  return null;
}

const OS_ORDER: Os[] = ["mac", "win", "linux"];
const FORMAT_ORDER: Format[] = ["dmg", "exe", "msi", "deb", "rpm", "appimage"];

/** 这一版的安装包，按 macOS、Windows、Linux 排；同一个系统里按格式排，同一种格式 arm64 在前 */
export function installersOf(r: Release): Installer[] {
  return r.assets
    .map(classify)
    .filter((x): x is Installer => !!x)
    .sort(
      (a, b) =>
        OS_ORDER.indexOf(a.os) - OS_ORDER.indexOf(b.os) ||
        FORMAT_ORDER.indexOf(a.format) - FORMAT_ORDER.indexOf(b.format) ||
        (a.arch === "arm64" ? 0 : 1) - (b.arch === "arm64" ? 0 : 1),
    );
}

/** 问一次 GitHub。连不上、被限流（不登录时每个 IP 每小时 60 次）、这一版没有安装包，都返回 null */
export async function fetchLatestRelease(init?: RequestInit): Promise<Release | null> {
  try {
    const res = await fetch(LATEST_API, { ...init, headers: { Accept: "application/vnd.github+json", ...init?.headers } });
    if (!res.ok) return null;
    const r = parseRelease(await res.json());
    return r && installersOf(r).length ? r : null;
  } catch {
    return null;
  }
}
