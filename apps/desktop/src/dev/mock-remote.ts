// 演示后端：设置 › 手机的命令（返回结构照 crates/captae-remote 的 server::Status / PairingView，命令见 src-tauri/src/remote_cmds.rs）。
//
// 演示数据：「让手机连接这台电脑」开着；配对过两台手机：iPhone（现在连着）、Pixel 9（昨天连过）。下载页里 23 号任务是从 iPhone 加的。
// 网址参数（和 mock.ts 的一起用）：
//   &phone=off    开关关着（第一次打开设置时的样子）
//   &phone=none   开着，还没有配对过手机
//   &phone=void   6 位数输错了 5 次、作废了
//   &phone=err    打开出错（端口被占）
// 配对码 5 分钟有效；带 &still 时倒计时停在 4:32。
import type { RemotePairing, RemoteStatus } from "@/lib/types";
import type { MockHandler } from "./mock";

const qs = new URLSearchParams(location.search);
const MODE = qs.get("mock") === "empty" ? "off" : qs.get("phone");
const STILL = qs.has("still");
const T0 = Date.now();
const MIN = 60_000;

const st: RemoteStatus = {
  enabled: MODE !== "off" && MODE !== "err",
  running: MODE !== "off" && MODE !== "err",
  port: 47863,
  hosts: ["192.168.1.23"],
  name: "Sol 的 MacBook Pro",
  error: MODE === "err" ? "端口 47863 被别的程序占着，也没有找到能用的端口" : undefined,
  devices:
    MODE === "none" || MODE === "off"
      ? []
      : [
          { id: "k2Qx9a", name: "iPhone", platform: "ios", pairedAt: T0 - 3 * 86_400_000, lastSeen: T0 - 8_000, online: true },
          { id: "pX7m1c", name: "Pixel 9", platform: "android", pairedAt: T0 - 9 * 86_400_000, lastSeen: T0 - 26 * 60 * MIN, online: false },
        ],
};

/** 演示用的二维码内容（格式照 proto.rs 的 PairLink；指纹和码是随便写的，扫了也连不上） */
function newPairing(): RemotePairing {
  const code = STILL ? "482913" : String(Math.floor(Math.random() * 1e6)).padStart(6, "0");
  const c = STILL ? "u7LxQ2cW9pVb3nRt0kYs4A" : Math.random().toString(36).slice(2, 24);
  return {
    uri: `captae://pair?h=192.168.1.23&p=47863&fp=8f3kT1q0VbX2cYw9Lr7uNs4Ea6Hj5Gm1PdZoKiRtQe8&c=${c}&n=${encodeURIComponent(st.name)}`,
    code,
    // 带 &still 时停在 4:32
    expiresAt: STILL ? T0 + 272_000 : Date.now() + 5 * MIN,
    void: MODE === "void",
    left: MODE === "void" ? 0 : 5,
  };
}
let pairing: RemotePairing | null = null;

const copy = (): RemoteStatus => ({ ...st, devices: st.devices.map((d) => ({ ...d })) });

export const remoteHandlers: Record<string, MockHandler> = {
  remote_status: () => copy(),
  remote_enable: ({ on }, { emit }) => {
    st.enabled = st.running = !!on;
    st.error = undefined;
    if (!on) pairing = null;
    emit({ type: "remote" });
    return copy();
  },
  remote_pairing: ({ refresh }) => {
    if (!st.running) throw new Error("先打开「让手机连接这台电脑」");
    // 第一轮按网址参数（&phone=void 是作废了的），点「换一个」、过期换的都是新的
    if (!pairing) pairing = newPairing();
    else if (refresh || pairing.expiresAt <= Date.now()) pairing = { ...newPairing(), void: false, left: 5 };
    return { ...pairing };
  },
  remote_revoke: ({ id }) => {
    const n = st.devices.length;
    st.devices = st.devices.filter((d) => d.id !== id);
    if (st.devices.length === n) throw new Error("没有这台手机");
    return copy();
  },
};
