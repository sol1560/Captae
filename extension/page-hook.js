/* 跑在网页自己的环境里（MAIN world），只做一件事：
 * 网站的「复制链接」按钮大多用 navigator.clipboard.writeText 或 document.execCommand('copy')，
 * 不一定触发能读到内容的 copy 事件。这里包一层，把复制的文字转告 content.js（它再判断是不是视频链接）。
 * 原来的行为完全不变，也不读剪贴板。 */
(() => {
  if (window.__captaeHook) return;
  window.__captaeHook = true;
  const tell = text => {
    try {
      if (typeof text === 'string' && text.length < 2048 && /^\s*https?:\/\//.test(text)) window.postMessage({ __captae: 'copy', text: text.trim() }, '*');
    } catch { /* 不影响网页 */ }
  };
  const cb = navigator.clipboard;
  if (cb && cb.writeText) {
    const orig = cb.writeText;
    cb.writeText = function (text) { tell(text); return orig.apply(this, arguments); };
  }
  const exec = Document.prototype.execCommand;
  Document.prototype.execCommand = function (cmd) {
    if (String(cmd).toLowerCase() === 'copy') {
      try {
        const a = document.activeElement;
        const text = a && (a.tagName === 'TEXTAREA' || a.tagName === 'INPUT') && typeof a.selectionStart === 'number'
          ? a.value.slice(a.selectionStart, a.selectionEnd)
          : String(document.getSelection() || '');
        tell(text);
      } catch { /* 不影响网页 */ }
    }
    return exec.apply(this, arguments);
  };
})();
