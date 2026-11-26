// 安装包（dmg）窗口的背景图：在访达里打开 dmg 时看到的那一页。
// 这一页不进程序：scripts/render-dmg-background.sh 用无界面 Chrome 把它截成 apps/desktop/src-tauri/dmg/background.png（1 倍）和 background@2x.png，
// 打包时 dmgbuild 把两张合成一张带高清版的 TIFF 放进 dmg。颜色、字、图标、玻璃都和界面是同一套（theme.css 的变量和 .glass、Hugeicons）。
// Captae 和 Applications（指向「应用程序」文件夹的快捷方式）两个图标、下面的名字是访达自己画的，位置写在 src-tauri/dmg/layout.json：这一页和 dmg/settings.py 读同一份，玻璃和箭头才对得上。
// 安装包窗口只有一种语言，用英文（docs/I18N.md 第 1 节），所以这一页的字不走翻译。
// 设计区（.dmg）是标题栏下面的内容区；截图比它高出 titleBar，多出的一截在最下面（为什么见 dmg/settings.py）。
// 窗口有背景图时，访达在深色模式下也按浅色画图标的名字（深色字），所以背景只做浅色这一版。
//
// 版式：一条胶片（上手引导用的开放电影剧照，许可见 licenses/blender-stills/）横穿窗口，从一块轻玻璃（像程序坞）后面经过：
// 玻璃外面清楚、玻璃里面糊开，两个图标和箭头在玻璃上。和界面里的玻璃同一个用法：玻璃浮在内容上面。没有渐变、没有衬线字、没有装饰图。
import { StrictMode, type CSSProperties } from "react";
import { createRoot } from "react-dom/client";
import { ArrowRight02Icon } from "@hugeicons/core-free-icons";
import "@/theme.css";
import "./dmg.css";
import { Icon } from "@/parts";
import layout from "../../src-tauri/dmg/layout.json";
import sintel1 from "@/assets/img/onboarding/sintel-1.jpg";
import tos1 from "@/assets/img/onboarding/tos-01.jpg";
import bbb from "@/assets/img/onboarding/bbb-frank.jpg";
import sintel3 from "./img/sintel-3.jpg";
import tos5 from "./img/tos-05.jpg";

/** 胶片上的五格：两头那两格在玻璃外面、看得清；中间三格在玻璃后面，挑亮的、颜色暖的（暗的糊开是一团灰） */
const STRIP = [
  // 左边这格只露出右边一大半：往左取景，露出桥上的人，不是一团机器
  { src: tos5, spot: "0% 40%" },
  { src: sintel3, spot: "45% 45%" },
  { src: bbb, spot: "55% 55%" },
  { src: tos1, spot: "50% 50%" },
  { src: sintel1, spot: "35% 40%" },
];

function DmgBackground() {
  const { window: win, titleBar, iconSize, app, applications, dock } = layout;
  const vars = {
    "--w": `${win.width}px`,
    "--h": `${win.height}px`,
    "--bleed": `${titleBar}px`,
    "--icon-y": `${app.y}px`,
    "--arrow-x": `${(app.x + applications.x) / 2}px`,
    "--icon": `${iconSize}px`,
    "--dock-x": `${dock.x}px`,
    "--dock-y": `${dock.y}px`,
    "--dock-w": `${dock.width}px`,
    "--dock-h": `${dock.height}px`,
  } as CSSProperties;
  return (
    <div className="dmg" style={vars}>
      <header className="dmg-head">
        <h1>Drag Captae to Applications</h1>
        <p>Download, organize and transcribe, all on your own Mac</p>
      </header>
      <div className="dmg-strip" aria-hidden>
        {STRIP.map((s, k) => (
          <img key={k} src={s.src} alt="" style={{ objectPosition: s.spot }} />
        ))}
      </div>
      <div className="dmg-dock glass" />
      <div className="dmg-arrow">
        <Icon icon={ArrowRight02Icon} size={40} stroke={1.2} />
      </div>
    </div>
  );
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <DmgBackground />
  </StrictMode>,
);
