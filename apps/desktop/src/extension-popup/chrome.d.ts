// 工具栏面板用到的几个 chrome.* 接口。没装 @types/chrome（加 npm 包要先问），只声明用到的这几个。
// 桌面端（Tauri）里没有 chrome 这个全局变量：用之前一律先判断 typeof chrome。
declare namespace chrome {
  namespace runtime {
    const id: string | undefined;
    function sendMessage<T = unknown>(message: unknown): Promise<T>;
  }
  namespace tabs {
    type Tab = { id?: number; url?: string; title?: string; favIconUrl?: string };
    function query(q: { active?: boolean; currentWindow?: boolean }): Promise<Tab[]>;
    function sendMessage(tabId: number, message: unknown): Promise<unknown>;
  }
}
