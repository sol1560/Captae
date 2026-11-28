// 落地页文案：全部由 Gemini 4 Argon 写（第五、六轮，2026-11-24；提示词和原始输出在维护者本机的 work/site/copy-prompt-5.md、-6.md），原样放进来，不改字。
// 功能窗口下面的旁白小字（stack）也由 Gemini 4 Argon 写（work/site/stack/）。desk 是之前桌面版的，现在没用到。
// 读屏软件用的短标签（a11y）也由 Gemini 4 Argon 写。其他语言的版本在 locales/<语言>.json，也由它写，结构和这里一样。
// 维护者 2026-11-24 亲自改的（原样用）：「免费公开」都改成「免费开源」；免费开源那张卡片的正文；「安心使用」那张换成「未来可期」和四条计划。
// 不算文案、照桌面端真实界面写的：演示画面里的按钮、状态和失败说法（reasons.examples 来自 crates/captae-core/src/errors.rs）。

export const GITHUB_REPO = "sol1560/Captae";
export const GITHUB = `https://github.com/${GITHUB_REPO}`;
export const RELEASES = `${GITHUB}/releases/latest`;

const written = {
  "meta": {
    "title": "Captae · 把网上的视频存进你的设备",
    "description": "贴个链接或在网页上点一下，把网课、素材和常看的视频存回自己的设备。能挑清晰度、搜视频里说过的话、转出字幕和剪辑。"
  },
  "nav": {
    "features": "功能导览",
    "extension": "浏览器插件",
    "ai": "交给 AI",
    "opensource": "免费开源",
    "download": "免费下载"
  },
  "hero": {
    "eyebrow": "电脑和手机都能用的视频收集工具",
    "title": "看到好视频，存进设备随时用",
    "subtitle": "不管是网课、剪辑素材还是常看的视频，贴个链接就能挑画质存回设备，还能转字幕、按台词找片段。",
    "primary": "免费下载",
    "secondary": "去 GitHub 看看",
    "note": "免费，不用注册账号，视频都在自己设备上"
  },
  "stage": {
    "caption": "从贴链接、存进素材库到转出字幕的全过程",
    "replay": "再看一遍"
  },
  "sites": {
    "lead": "平时常逛的视频网站，看到就能存",
    "more": "还有上千个网站"
  },
  "showcase": {
    "kicker": "界面一览",
    "title": "四个页面，管好下载和素材",
    "body": "下面就是真实界面，直接在网页里切页面、挑清晰度、看进度，按钮都能点。",
    "captions": {
      "downloads": "看清每档画质的大小再下载，正在排队的和下完的一目了然。",
      "library": "下好的视频自动归档，按台词搜索、剪辑、转格式都在这里。",
      "agent": "一键连上常用的 AI 工具，它替你做过的每件事都有记录。",
      "extension": "在网页视频旁加上下载按钮，不用切窗口就能挑清晰度保存。"
    },
    "try": "点一下动手试试"
  },
  "tour": {
    "next": "下一个",
    "end": "继续往下看"
  },
  "pick": {
    "kicker": "挑清晰度",
    "title": "每档画质多大，看清楚再下",
    "body": "贴入链接后，每档清晰度占多大空间都列得明明白白。你可以动手挑一档，不想选也没关系，等几秒就会自动按最高画质开始下。"
  },
  "login": {
    "kicker": "账号登录",
    "title": "要登录才能看的，登一次就好",
    "body": "有些网课、高画质档位或收藏夹里的视频，得登录网站才能看。在应用里像平时一样登录一次，以后遇到同个网站就能直接下。"
  },
  "reasons": {
    "kicker": "原因清楚",
    "title": "下不了的时候，告诉你差在哪一步",
    "body": "不甩给你看不懂的报错。不管是还没登录、网页要验证，还是所在地区看不了，都会直接说清楚原因，在旁边放好下一步的按钮。",
    "fixed": "已登录，正在继续下载"
  },
  "library": {
    "kicker": "素材库",
    "title": "记得视频里说过哪句话，就能搜出来",
    "body": "视频存多了也好找。搜一个词，就能找出说过这句话的片段。还能直接在里面剪辑、拼接、加字幕，或者转成你需要的格式。",
    "chips": [
      "搜台词",
      "转格式",
      "剪片段",
      "多段拼接",
      "加字幕",
      "提取音频"
    ]
  },
  "speech": {
    "kicker": "转出字幕",
    "title": "没配字幕的视频，在设备上转成文字",
    "body": "遇到没字幕的讲座或访谈，点一下就能转出带时间轴的字幕和完整文字稿。全程都在你自己的设备上完成，视频声音不会往外传。",
    "none": "还没有字幕",
    "working": "正在转文字",
    "done": "字幕已生成"
  },
  "live": {
    "kicker": "录制直播",
    "title": "正在播的直播，从现在开始录下来",
    "body": "遇到没回放的线上分享或正在播的节目，贴入链接就能自动认出是直播。点一下从当前画面开始录，录够了随时停下，直接存成视频。"
  },
  "extension": {
    "kicker": "网页插件",
    "title": "刷网页看到好视频，在旁边点一下就下",
    "body": "装上浏览器插件，不用来回复制链接。网页视频旁直接出现下载按钮，正在播的视频也能认出来，原地挑好清晰度就能存回电脑。",
    "points": [
      "视频旁直接出按钮，不用复制链接",
      "原地挑好画质，右下角实时看进度",
      "下完自动放进素材库，打开就能用"
    ]
  },
  "ai": {
    "kicker": "连接 AI",
    "title": "让常用的 AI 工具替你下视频、找素材",
    "body": "用 Claude 或 Cursor 时，点一下就能连上。直接让它帮你下视频、找某句台词的片段，或者给视频转出字幕，不用自己动手。",
    "points": [
      "需要时一键连上，不用时随时关掉",
      "做过的每件事都有记录，一目了然",
      "只在你电脑里干活，文件不往外传"
    ],
    "log": [
      {
        "did": "下载了一段视频",
        "result": "已存入素材库"
      },
      {
        "did": "Sintel 预告片转字幕",
        "result": "已转出完整字幕"
      },
      {
        "did": "找说了 dragon 的片段",
        "result": "已定位到对应片段"
      },
      {
        "did": "Big Buck Bunny 转 MP3",
        "result": "已导出音频文件"
      }
    ]
  },
  "send": {
    "kicker": "发给 AI",
    "title": "长视频不想看全片，发给 AI 帮你抓重点",
    "body": "遇到很长的讲座或教程，点一下「发给 AI」，就能把文字稿和几张关键画面一起复制好。贴进聊天窗口，直接问它视频里讲了什么。",
    "script": "文字稿"
  },
  "local": {
    "kicker": "留在设备上",
    "title": "不用注册账号，视频都在你自己设备上",
    "body": "打开就能用，不用注册账号。你下的视频、转出的字幕和搜索记录，全都留在你自己的设备上，不往外传。",
    "points": [
      "不用注册账号，打开就能用",
      "存成通用格式，播放剪辑都顺手",
      "在自己设备上转文字，声音不外传"
    ]
  },
  "opensource": {
    "kicker": "免费开源",
    "title": "完全免费，代码也摊开放在阳光下",
    "body": "所有功能都免费，没有广告和次数限制。全部代码都公开在 GitHub 上，路才能越走越宽，用着真不烧心。",
    "link": "去 GitHub 看代码"
  },
  "future": {
    "title": "未来可期",
    "points": [
      "更广泛并更易于使用",
      "全新的下载器功能",
      "web 版本",
      "更好的视频预览和编辑"
    ]
  },
  "final": {
    "kicker": "现在试试",
    "title": "把下一段好视频，存进你的设备",
    "subtitle": "不用注册，打开就能用。贴个链接，把想留下的视频存进设备吧。",
    "primary": "免费下载"
  },
  "footer": {
    "tagline": "支持 macOS、Windows、Linux、Android、iOS 的免费视频下载与素材整理工具",
    "legal": "请只下载你有权保存和使用的内容，并遵守所在网站的使用规定与当地法律法规。",
    "credits": "页面演示画面来自 Blender 基金会的四部开放电影 Big Buck Bunny、Sintel、Tears of Steel、Elephants Dream，按 CC BY 3.0（Elephants Dream 是 CC BY 2.5）使用，© Blender Foundation。",
    "github": "GitHub 仓库"
  },
  "a11y": {
    "nav": "主导航",
    "sites": "支持的网站",
    "screens": "界面截图",
    "footer": "页脚",
    "screenAlt": "Captae 的{page}页",
    "qualityDemo": "清晰度选择示例",
    "lang": "切换语言"
  },
  "desk": {
    "scroll": "往下滚，看鼠标演示怎么存视频",
    "steps": [
      "在浏览器地址栏复制视频链接，随手就能存",
      "切回窗口，自动认出刚复制的链接，不用粘贴",
      "点「下载」，各档清晰度和文件大小一目了然",
      "挑好 1080p 点「马上开始」，开始下载",
      "下载完成后自动收进素材库，不用到处找文件",
      "在素材库点开视频，能直接转格式、加字幕和剪辑",
      "演示结束，现在轮到你在窗口里亲手点着试试"
    ],
    "take": "自己点着试试",
    "replay": "再看一遍"
  },
  "stack": {
    "captions": [
      "点「下载」，挑 1080p，点「马上开始」",
      "点「登录 YouTube」，登录好自己接着下",
      "一条条点开失败视频，看原因和下一步怎么做",
      "在搜索框打 Sintel，点开找到的视频看详情",
      "切到「字幕」，选「语音转文字」，点「生成字幕」",
      "点「停止并保存」，把录到的直播存进「已完成」",
      "在插件列表里点「下载」，直接交给 Captae",
      "选 Cursor，点「一键写入」再确认就能用",
      "点「发给 AI」，复制好文字稿和关键画面去提问"
    ],
    "take": "自己操作试试"
  }
};

/** 演示里四条失败说法（桌面端的原话）和对应的下一步按钮（地区限制那条真实界面是「重试」）；其他语言的由 work/site/i18n/build-locales.py 从 App 词表取 */
const examples: { text: string; tone: "warn" | "err"; action: string }[] = [
      { text: "这个视频要登录哔哩哔哩才能看", tone: "warn", action: "登录哔哩哔哩" },
      { text: "YouTube 要先在网页里确认一下（人机验证）", tone: "warn", action: "去验证" },
      { text: "这个视频在你所在的地区看不了", tone: "err", action: "重试" },
      { text: "这个视频受版权保护（DRM），Captae 不能下载", tone: "err", action: "" },
];

/** 简体中文原版（其他语言在 locales/<语言>.json，结构和它一样） */
/** 演示里另外两句核心库的原话（界面上由后台传过来，网页里的 t() 查不到，所以和失败说法一样放在这里） */
const ui = { live: "这是正在进行的直播", biliGain: "可以下 1080P 以上和大会员视频" };

export const zhCN = { ...written, reasons: { ...written.reasons, examples }, ui };
export type Copy = typeof zhCN;

/** 现在这种语言的文案：换语言时 i18n.ts 的 applyLang 换掉它，然后整个页面重新挂一次，各处读到的就是新的 */
export let copy: Copy = zhCN;
export function setCopy(c: Copy) {
  copy = c;
}
