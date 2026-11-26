// 每一页顶上的那一条：标题、一句状态、中间的分段控件、右边的圆形按钮。
// 列表从它下面滚过去，所以是毛玻璃；也是窗口的拖动区（Tauri 2 的拖动区只认属性所在的元素本身，文字上也要加）。
// 侧栏收起时左边要给红绿灯和「展开侧栏」让出位置，靠 .cp-window[data-side="off"] 上的 --top-lead。
import type { ReactNode } from "react";

export function Topbar({ title, sub, mid, end }: { title: ReactNode; sub?: ReactNode; mid?: ReactNode; end?: ReactNode }) {
  return (
    <header className="cp-topbar" data-tauri-drag-region>
      <h1 data-tauri-drag-region>{title}</h1>
      {sub != null && sub !== "" && (
        <span className="sub" data-tauri-drag-region>
          {sub}
        </span>
      )}
      {mid && <div className="mid">{mid}</div>}
      <div className="end" data-tauri-drag-region>
        {end}
      </div>
    </header>
  );
}
