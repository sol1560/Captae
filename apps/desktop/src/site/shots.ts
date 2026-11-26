// 按语言拍的界面截图（用 App 的演示版拍：减少动效、1440×900、2 倍图再缩小；面板 372 宽），在 img/shots/<语言>/：
// downloads、library、agent、extension 四张给手机上的「界面一览」，popup 给导览里插件那一站。
import { lang, slugOf } from "./i18n";

const SHOT_URLS = import.meta.glob<string>("./img/shots/*/*.jpg", { eager: true, query: "?url", import: "default" });

/** 这种语言的截图；万一缺了用英文的 */
export const shotOf = (name: string) => SHOT_URLS[`./img/shots/${slugOf(lang())}/${name}.jpg`] ?? SHOT_URLS[`./img/shots/en/${name}.jpg`]!;
