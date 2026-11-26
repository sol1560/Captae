// 上手引导（四页卡片）和侧栏底部的上手清单。实现在 dialogs/onboarding/：
//   Tour.tsx       引导卡片（OnboardingHost：窗口骨架按 openDialog("onboarding") 打开）
//   pages.tsx      四页的内容和每页那一行操作；scenes.tsx 头图（剧照 + 界面片段）
//   Checklist.tsx  侧栏的上手清单；state.ts 两者共用的状态和自动打勾
// 约定（和旧界面一样，旧的值在 lib/prefs.ts 里已经搬过来了）：
//   getPref("onboarded")  引导看过了（走完、跳过、关掉都算）
//   getPref("ob")         清单 {items: {dl, convert, ext, agent}, hidden}
// 设置里的「重新查看上手引导」用 restartOnboarding()。
export { Checklist } from "./onboarding/Checklist";
export { OnboardingHost } from "./onboarding/Tour";
export { restartOnboarding } from "./onboarding/state";
