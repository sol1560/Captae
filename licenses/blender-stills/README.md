# 上手引导和安装包窗口里的开放电影剧照

都是 Blender Foundation 开放电影的剧照，来自 Wikimedia Commons，原样使用（按卡片裁切显示，没有改画面内容）。

- 上手引导的四张插图：文件在 `apps/desktop/src/assets/img/onboarding/`，打包时嵌进桌面端程序。
- 安装包（dmg）窗口的背景图：一条五格的「胶片」从玻璃后面经过（`apps/desktop/src/dmg-background/`，截成 `apps/desktop/src-tauri/dmg/background*.png` 放进 dmg）。
  用了上手引导里的 `sintel-1.jpg`、`tos-01.jpg`、`bbb-frank.jpg`，加上 `apps/desktop/src/dmg-background/img/` 里的两张。中间三格在玻璃后面是模糊的。

| 文件 | 出处 | 许可 | 用在 |
| --- | --- | --- | --- |
| `bbb-frank.jpg` | *Big Buck Bunny* (2008) © Blender Foundation · peach.blender.org | [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/) | 上手引导、安装包窗口 |
| `sintel-1.jpg` | *Sintel* (2010) © Blender Foundation · durian.blender.org | [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/) | 上手引导、安装包窗口 |
| `tos-01.jpg` | *Tears of Steel* (2012) © Blender Foundation · mango.blender.org | [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/) | 上手引导、安装包窗口 |
| `ed-emo-proog.jpg` | *Elephants Dream* (2006) © Blender Foundation / Netherlands Media Art Institute · orange.blender.org | [CC BY 2.5](https://creativecommons.org/licenses/by/2.5/) | 上手引导 |
| `sintel-3.jpg` | *Sintel* (2010) © Blender Foundation · durian.blender.org | [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/) | 安装包窗口 |
| `tos-05.jpg` | *Tears of Steel* (2012) © Blender Foundation · mango.blender.org | [CC BY 3.0](https://creativecommons.org/licenses/by/3.0/) | 安装包窗口 |

演示后端用的另几张同系列剧照在 `apps/desktop/src/dev/img/`，只在演示构建里，不进安装包。
