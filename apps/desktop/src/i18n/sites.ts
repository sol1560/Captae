// 后端存进数据、直接拿来显示的中文名字：素材的来源（src）、失败原因里的网站（fail.site）、登录过的网站、AI 助手的默认名字。
// 显示时用 tAny(name)（没有译文的原样显示，比如 YouTube）。这里用 N_ 标出来，词表里才有它们；
// 后端加了新的中文站点名（crates/captae-core/src/errors.rs 的 site_name、login.rs 的 SITES），这里也要加。
import { N_ } from ".";

export const STORED_NAMES = [
  N_("哔哩哔哩"),
  N_("抖音"),
  N_("微博"),
  N_("小红书"),
  N_("快手"),
  N_("微信"),
  N_("腾讯视频"),
  N_("爱奇艺"),
  N_("优酷"),
  N_("西瓜视频"),
  N_("知乎"),
  N_("芒果TV"),
  N_("网易云音乐"),
  N_("喜马拉雅"),
  // 素材库里本地导入的文件的来源
  N_("本地"),
  // MCP 客户端没报名字时的叫法（mcp.rs）
  N_("AI 助手"),
];
