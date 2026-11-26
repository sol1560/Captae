// 只跟界面有关的小偏好（侧栏收起、上手引导看过了……），存在 WebView 的 localStorage 里，不进 settings.json。
// 旧界面（第二轮的原生 HTML 界面，已删除，git 历史里有）用的前缀是 captae2:，第一次打开新界面时把还有用的几项搬过来，老用户不会再看一遍上手引导。
const PREFIX = "captae3:";
const mem: Record<string, unknown> = {};

/** 旧键 → 新键。ob 是上手清单 {items: {dl, convert, ext, agent}, hidden}，形状不变 */
const FROM_V2: [string, string][] = [
  ["obSeen", "onboarded"],
  ["ob", "ob"],
  ["sideOff", "sideOff"],
];

function migrate() {
  try {
    if (localStorage.getItem(PREFIX + "migrated")) return;
    for (const [from, to] of FROM_V2) {
      const v = localStorage.getItem("captae2:" + from);
      if (v != null && localStorage.getItem(PREFIX + to) == null) localStorage.setItem(PREFIX + to, v);
    }
    localStorage.setItem(PREFIX + "migrated", "1");
  } catch {
    // localStorage 用不了（极少见）：只存在内存里，不迁移
  }
}
migrate();

export function getPref<T>(k: string, d: T): T {
  try {
    const v = localStorage.getItem(PREFIX + k);
    return v == null ? d : (JSON.parse(v) as T);
  } catch {
    return k in mem ? (mem[k] as T) : d;
  }
}

export function setPref(k: string, v: unknown) {
  try {
    localStorage.setItem(PREFIX + k, JSON.stringify(v));
  } catch {
    mem[k] = v;
  }
}
