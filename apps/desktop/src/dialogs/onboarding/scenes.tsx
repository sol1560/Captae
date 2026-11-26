// 上手引导的头图：开放电影剧照 + 浮在上面的 Captae 真实界面片段（说明这一页讲的功能），不用渐变色块和装饰图标。
// 片段只是画面（aria-hidden），和真界面用同一套玻璃、字号、圆角；文字都是示例。
// 剧照是 Blender 基金会的开放电影（CC BY），随安装包发布，出处见 src/assets/img/CREDITS.md。
// 片段是渲染时才画的组件（Scene）：里面的字要按当前语言翻译，不能在加载模块时就定下来。
import { AiTranscribeAudioIcon, ArrowDown01Icon, CheckmarkCircle02Icon } from "@hugeicons/core-free-icons";
import type { ReactNode } from "react";
import { t } from "@/i18n";
import bbb from "@/assets/img/onboarding/bbb-frank.jpg";
import edEmo from "@/assets/img/onboarding/ed-emo-proog.jpg";
import sintel from "@/assets/img/onboarding/sintel-1.jpg";
import tos from "@/assets/img/onboarding/tos-01.jpg";
import { CaptaeLogo, Icon, ProgressBar } from "@/parts";

export type Art = { image: string; /** 剧照裁切的位置（object-position），让人物落在片段旁边 */ at?: string; Scene: () => ReactNode };

/** 第 1 页：粘贴链接的输入框 + 正在下载的那一行（照下载页的输入框和任务行） */
const save: Art = {
  image: bbb,
  at: "center 38%",
  Scene: () => (
    <>
      <div className="cp-frag cp-ob-f-input">
        <span className="mono">youtube.com/watch?v=aqz-KE-bpKQ</span>
        <span className="send">
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.6" strokeLinecap="round" strokeLinejoin="round">
            <path d="M12 19V5M5 12l7-7 7 7" />
          </svg>
        </span>
      </div>
      <div className="cp-frag cp-ob-f-job">
        <img src={bbb} alt="" />
        <div>
          <b>Big Buck Bunny · 1080p</b>
          <ProgressBar p={0.42} />
        </div>
      </div>
    </>
  ),
};

/** 第 2 页：插件放在网页视频右上角的按钮组和清晰度菜单（照插件的注入部件） */
const browser: Art = {
  image: sintel,
  at: "center 30%",
  Scene: () => (
    <>
      <div className="cp-ob-f-xbar">
        <span className="cp-ob-f-xbtn">
          <CaptaeLogo size={20} />
          {t("下载")}
          <Icon icon={ArrowDown01Icon} size={13} />
        </span>
        <span className="cp-ob-f-xbtn plain">{t("发给 AI")}</span>
      </div>
      <div className="cp-frag cp-ob-f-xmenu">
        <div className="item hl">
          <span>
            1080p<small>MP4</small>
          </span>
          <span className="size">{t("约{size}", { size: "312 MB" })}</span>
        </div>
        <div className="item">
          <span>
            720p<small>MP4</small>
          </span>
          <span className="size">{t("约{size}", { size: "158 MB" })}</span>
        </div>
      </div>
    </>
  ),
};

/** 第 3 页：正在转成文字的进度 + 生成出来的两句字幕 */
const whisper: Art = {
  image: edEmo,
  at: "center 32%",
  Scene: () => (
    <>
      <div className="cp-frag cp-ob-f-task">
        <div className="row">
          <Icon icon={AiTranscribeAudioIcon} size={15} />
          <b>{t("正在转成文字")}</b>
          <span className="num">62%</span>
        </div>
        <ProgressBar p={0.62} />
      </div>
      <div className="cp-frag cp-ob-f-lines">
        <div>
          <span className="mono">00:12</span>
          {t("跟紧我，别回头。")}
        </div>
        <div>
          <span className="mono">00:15</span>
          {t("我们到底要去哪？")}
        </div>
      </div>
    </>
  ),
};

/** 第 4 页：AI 助手让 Captae 整理好素材以后的结果 */
const agent: Art = {
  image: tos,
  at: "center 40%",
  Scene: () => (
    <div className="cp-frag cp-ob-f-ready">
      <div className="row">
        <Icon icon={CheckmarkCircle02Icon} size={16} />
        {t("已经整理好，复制到剪贴板")}
      </div>
      <div className="sub">{t("文字稿 · {n} 张关键帧 · context.md", { n: 7 })}</div>
    </div>
  ),
};

export const ART = { save, browser, whisper, agent };
