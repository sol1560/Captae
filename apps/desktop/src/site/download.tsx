// 页尾「现在试试」那块玻璃里的下载列表：GitHub 最新一版的每个安装包一行，点「下载」直接从 GitHub 下（数据见 release.ts）。
// - 先用构建时写进页面的那一版（vite.site.config.ts 的 __CAPTAE_RELEASE__），打开页面马上能点；
//   浏览器里再问一次 GitHub，发了新版还没重新部署官网也能拿到最新的。两边都没有时，只放一个按钮去 GitHub 的发布页。
// - 看网页的电脑是哪个系统，那个系统的第一个安装包用主按钮（近黑胶囊），其他用次按钮；手机、平板上都用次按钮。
// - 列表是内容，不透明（白卡），玻璃只是外面那一层（docs/design/README.md 第 1 节第 2 条）。
// - 不加动效：从构建时那一版换成浏览器里问到的那一版时直接换（docs/MOTION.md 1.4，大小、日期用等宽数字）。
// 这块的小字（按钮、芯片、发行版、「更新说明和校验值」）是 2026-11-28 加的功能性说明，不是 Gemini 写的营销文案；
// 不放进 locales/<语言>.json，因为那几份由维护者本机的 work/site/i18n/build-locales.py 整份重新生成，放进去会被冲掉。
import { useEffect, useState, type ReactNode } from "react";
import { ArrowRight02Icon } from "@hugeicons/core-free-icons";
import { lang, type Lang } from "@/i18n";
import { bytes } from "@/lib/format";
import { Icon } from "@/parts";
import { LinkButton } from "./bits";
import { copy, RELEASES } from "./copy";
import { fetchLatestRelease, installersOf, type Format, type Installer, type Os, type Release } from "./release";

declare const __CAPTAE_RELEASE__: Release | null;
/** 构建时问到的那一版（没问到是 null） */
const BUILT: Release | null = typeof __CAPTAE_RELEASE__ === "undefined" ? null : __CAPTAE_RELEASE__;

/* ---------------- 最新一版 ---------------- */

/** 浏览器里整页只问一次：换语言时页面整个重新挂一次，不再重问 */
let asked: Promise<Release | null> | null = null;
let answer: Release | null = null;

function useLatestRelease(): Release | null {
  const [release, setRelease] = useState<Release | null>(() => answer ?? BUILT);
  useEffect(() => {
    let live = true;
    asked ??= fetchLatestRelease().then((r) => (answer = r));
    void asked.then((r) => {
      if (live && r) setRelease(r);
    });
    return () => {
      live = false;
    };
  }, []);
  return release;
}

/** 看网页的这台电脑是什么系统。手机、平板（包括报成 Mac 的 iPad）、ChromeOS 返回 null：没有能直接装的版本 */
function detectOs(): Os | null {
  const ua = navigator.userAgent;
  const platform = (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData?.platform || navigator.platform || "";
  if (/iPhone|iPad|iPod|Android|CrOS/i.test(ua)) return null;
  if (/mac/i.test(platform) || /Macintosh/.test(ua)) return navigator.maxTouchPoints > 1 ? null : "mac";
  if (/win/i.test(platform) || /Windows/.test(ua)) return "win";
  if (/linux/i.test(platform) || /Linux|X11/.test(ua)) return "linux";
  return null;
}

/* ---------------- 小字 ---------------- */

type Labels = {
  /** 每行的按钮 */
  button: string;
  /** {v} 是版本号 */
  version: string;
  /** 链到这一版在 GitHub 上的页面（更新说明、SHA256SUMS.txt 在那里） */
  notes: string;
  appleSilicon: string;
  intel: string;
  x64: string;
  arm64: string;
  /** Linux 各格式适合的发行版 */
  deb: string;
  rpm: string;
  appimage: string;
  /** 读屏软件读的列表名 */
  list: string;
};

const LABELS: Record<Lang, Labels> = {
  "zh-CN": { button: "下载", version: "版本 {v}", notes: "更新说明和校验值", appleSilicon: "Apple 芯片", intel: "Intel 芯片", x64: "64 位", arm64: "ARM", deb: "Ubuntu、Debian", rpm: "Fedora、openSUSE", appimage: "其他发行版", list: "安装包" },
  "zh-TW": { button: "下載", version: "版本 {v}", notes: "更新說明和校驗值", appleSilicon: "Apple 晶片", intel: "Intel 晶片", x64: "64 位元", arm64: "ARM", deb: "Ubuntu、Debian", rpm: "Fedora、openSUSE", appimage: "其他發行版", list: "安裝檔" },
  en: { button: "Download", version: "Version {v}", notes: "Release notes and checksums", appleSilicon: "Apple silicon", intel: "Intel", x64: "64-bit", arm64: "ARM", deb: "Ubuntu, Debian", rpm: "Fedora, openSUSE", appimage: "Other distros", list: "Installers" },
  ja: { button: "ダウンロード", version: "バージョン {v}", notes: "リリースノートとチェックサム", appleSilicon: "Apple シリコン", intel: "Intel", x64: "64 ビット", arm64: "ARM", deb: "Ubuntu、Debian", rpm: "Fedora、openSUSE", appimage: "その他のディストリビューション", list: "インストーラー" },
  ko: { button: "다운로드", version: "버전 {v}", notes: "릴리스 노트와 체크섬", appleSilicon: "Apple 실리콘", intel: "Intel", x64: "64비트", arm64: "ARM", deb: "Ubuntu, Debian", rpm: "Fedora, openSUSE", appimage: "기타 배포판", list: "설치 파일" },
  de: { button: "Herunterladen", version: "Version {v}", notes: "Versionshinweise und Prüfsummen", appleSilicon: "Apple Silicon", intel: "Intel", x64: "64-Bit", arm64: "ARM", deb: "Ubuntu, Debian", rpm: "Fedora, openSUSE", appimage: "Andere Distributionen", list: "Installationsdateien" },
  es: { button: "Descargar", version: "Versión {v}", notes: "Notas de la versión y sumas de verificación", appleSilicon: "Apple Silicon", intel: "Intel", x64: "64 bits", arm64: "ARM", deb: "Ubuntu, Debian", rpm: "Fedora, openSUSE", appimage: "Otras distribuciones", list: "Instaladores" },
  fr: { button: "Télécharger", version: "Version {v}", notes: "Notes de version et sommes de contrôle", appleSilicon: "Puce Apple", intel: "Intel", x64: "64 bits", arm64: "ARM", deb: "Ubuntu, Debian", rpm: "Fedora, openSUSE", appimage: "Autres distributions", list: "Programmes d’installation" },
};

const OS_NAME: Record<Os, string> = { mac: "macOS", win: "Windows", linux: "Linux" };
const FORMAT_NAME: Record<Format, string> = { dmg: ".dmg", exe: ".exe", msi: ".msi", deb: ".deb", rpm: ".rpm", appimage: "AppImage" };

/** 一行的小字：「Apple 芯片 · .dmg · 156 MB」「Ubuntu、Debian · .deb · 254 MB」。Linux 的 x64 是默认，不写芯片。
 *  大小的数字和单位之间是不换行空格：窄屏上不会把「367 MB」拆成两行；其余地方照常换行（日文「その他のディストリビューション」很长，不能整段不断行） */
function detailOf(x: Installer, l: Labels): string {
  const hint = x.format === "deb" || x.format === "rpm" || x.format === "appimage" ? l[x.format] : "";
  const arch =
    x.arch === "arm64" ? (x.os === "mac" ? l.appleSilicon : l.arm64)
    : x.arch === "x64" ? (x.os === "mac" ? l.intel : x.os === "win" ? l.x64 : "")
    : "";
  // 「·」前面也是不换行空格：换行时圆点留在上一行末尾，不会出现在下一行开头
  return [hint, arch, FORMAT_NAME[x.format], bytes(x.size).replace(" ", " ")].filter(Boolean).join(" · ");
}

/** 发布日期，按这种语言的写法（「2026年11月26日」「November 26, 2026」） */
function dateOf(iso: string, l: Lang): string {
  const d = new Date(iso);
  return isNaN(d.getTime()) ? "" : new Intl.DateTimeFormat(l, { dateStyle: "long" }).format(d);
}

/* ---------------- 列表 ---------------- */

export function Downloads(): ReactNode {
  const release = useLatestRelease();
  const [os] = useState(detectOs);
  const list = release ? installersOf(release) : [];
  if (!release || !list.length) return <LinkButton href={RELEASES}>{copy.final.primary}</LinkButton>;
  const l = lang();
  const words = LABELS[l];
  const mine = list.find((x) => x.os === os);
  const meta = [words.version.replace("{v}", release.tag.replace(/^v/, "")), dateOf(release.published, l)].filter(Boolean).join(" · ");
  return (
    <div className="site-dl">
      <ul className="site-dl-list" aria-label={words.list}>
        {list.map((x) => {
          const detail = detailOf(x, words);
          return (
            <li className="site-dl-row" key={x.name}>
              <div className="site-dl-text">
                <span className="site-dl-os">{OS_NAME[x.os]}</span>
                <span className="site-dl-meta">{detail}</span>
              </div>
              <LinkButton href={x.url} size="sm" variant={x === mine ? "primary" : "secondary"} label={`${words.button} ${OS_NAME[x.os]} · ${detail}`}>
                {words.button}
              </LinkButton>
            </li>
          );
        })}
      </ul>
      <p className="site-dl-foot">
        <span>{meta}</span>
        <a className="site-arrow-link" href={release.page} target="_blank" rel="noreferrer">
          {words.notes}
          <Icon icon={ArrowRight02Icon} size={14} />
        </a>
      </p>
    </div>
  );
}
