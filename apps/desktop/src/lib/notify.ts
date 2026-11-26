// 提示条：Sonner（MIT），外观在 theme.css 的 .cp-toast，进出场用 Sonner 自带的动效（课程 02/03 讲的就是它）。
import { toast } from "sonner";

export type ToastAction = { label: string; run: () => void };

/** 一条提示：标题 + 一句说明 + 至多一个按钮（比如「撤销」「打开」） */
export function notify(title: string, desc?: string, action?: ToastAction, opts?: { duration?: number; id?: string | number }) {
  return toast(title, {
    description: desc,
    action: action ? { label: action.label, onClick: action.run } : undefined,
    ...opts,
  });
}

export const dismiss = (id?: string | number) => toast.dismiss(id);
