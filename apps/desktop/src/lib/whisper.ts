// 语音转文字（faster-whisper）：装没装、用哪个模型、安装进度。设置和上手引导共用这一份状态。
import { useMemo } from "react";
import { decimal, N_, t, type Key } from "@/i18n";
import { errText, invoke } from "./bridge";
import { notify } from "./notify";
import { getState, loadDeps, setState, useApp } from "./store";
import type { TaskStatus } from "./types";

type WhisperModel = { value: string; label: string; full: string; model: string };

/** 「约 200 MB」「约 1.6 GB」 */
const approx = (mbs: number) => t("约{size}", { size: mbs >= 1000 ? decimal(mbs / 1000, 1) + " GB" : mbs + " MB" });

/** label、full、model 是给人看的：读的时候才按当前语言生成（getter），换语言跟着变，用的地方照旧直接读 x.label */
function entry(value: string, label: Key, fullMb: number, modelMb: number): WhisperModel {
  return {
    value,
    get label() {
      return t(label);
    },
    get full() {
      return approx(fullMb);
    },
    get model() {
      return approx(modelMb);
    },
  };
}

/** 第一次安装要下组件（约 60 MB）加模型；组件装好以后换模型只下模型 */
export const WHISPER_MODELS: WhisperModel[] = [
  entry("base", N_("快一些"), 200, 145),
  entry("small", N_("均衡"), 550, 480),
  entry("medium", N_("更准"), 1600, 1500),
  entry("large-v3", N_("最准"), 3200, 3100),
];
export const modelInfo = (m: string) => WHISPER_MODELS.find((x) => x.value === m) ?? WHISPER_MODELS[1];

export type WhisperState = "checking" | "ready" | "need_model" | "need_install" | "installing";

export function useWhisper(): { state: WhisperState; model: string; task?: TaskStatus } {
  const deps = useApp((s) => s.deps);
  const busy = useApp((s) => s.setupBusy);
  const model = useApp((s) => s.settings?.whisperModel || "small");
  const tasks = useApp((s) => s.tasks);
  // 最新的一条安装任务（task 事件里 kind = setup）
  const task = useMemo(
    () =>
      Object.values(tasks)
        .filter((t) => t.kind === "setup")
        .sort((a, b) => b.id - a.id)[0],
    [tasks],
  );
  const w = deps?.find((d) => d.key === "whisper");
  const state: WhisperState = busy || task?.st === "run" ? "installing" : !deps ? "checking" : w?.ok && (w.models ?? []).includes(model) ? "ready" : w?.ok ? "need_model" : "need_install";
  return { state, model, task: state === "installing" ? task : undefined };
}

/** 安装（或只下载模型）。进度走 task 事件，装完重新查一次外部工具 */
export async function installWhisper() {
  const st = getState();
  if (st.setupBusy) return;
  setState({ setupBusy: true }, true);
  try {
    await invoke("install_whisper", { model: st.settings?.whisperModel || "small" });
    notify(t("语音转文字可以用了"), t("没有字幕的视频也能生成字幕"));
  } catch (e) {
    notify(t("没有装好语音识别"), errText(e));
  } finally {
    setState({ setupBusy: false }, true);
    await loadDeps();
  }
}
