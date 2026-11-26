/* 插件网页部件用的小工具：图标、Captae 的标、建 DOM 的帮手、格式化。
 * 网页部件不用 React（会注入每一个网页，计划 5.6），这里的东西都很小，原生 JS 就够。
 *
 * 图标来自 Hugeicons Free（MIT，https://hugeicons.com，版权 2025 Hugeicons），
 * 只取了用到的几个，路径数据原样，描边粗细由外层 <svg> 统一给（和桌面端一样 1.75）。 */

import { tr, decimal } from './i18n.js';

const ICONS = {
  // Download04Icon
  download: '<path d="M16.9504 12.1817C17.1981 12.814 16.5076 13.5726 15.1267 15.0899C13.6702 16.6902 12.9201 17.4904 12 17.5C11.0799 17.4904 10.3298 16.6902 8.87331 15.0899C7.49239 13.5726 6.80193 12.814 7.04964 12.1817C7.05868 12.1586 7.06851 12.1359 7.0791 12.1135C7.34928 11.542 8.24477 11.5029 10 11.5002V4.99998C10 4.53501 10 4.30253 10.0511 4.11179C10.1898 3.59414 10.5941 3.1898 11.1118 3.05111C11.3025 3 11.535 3 12 3C12.4649 3 12.6974 3 12.8882 3.05111C13.4058 3.1898 13.8102 3.59414 13.9489 4.11179C14 4.30253 14 4.53501 14 4.99998V11.5002C15.7552 11.5029 16.6507 11.542 16.9209 12.1135C16.9315 12.1359 16.9413 12.1586 16.9504 12.1817Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M5.00006 21H19.0001" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // ArrowDown01Icon
  chevron: '<path d="M18 9.00005C18 9.00005 13.5811 15 12 15C10.4188 15 6 9 6 9" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // AiBrain01Icon
  ai: '<path d="M9 1.99994C10.6569 1.99994 12 3.34308 12 4.99994V18.9999C12 20.6568 10.6569 21.9999 9 21.9999C7.513 21.9999 6.27868 20.9181 6.04124 19.4985C5.99765 19.2379 5.76422 18.9999 5.5 18.9999C3.567 18.9999 2 17.4329 2 15.4999C2 14.4932 2.42496 13.5853 3.10536 12.9465C3.55702 12.5225 3.69627 11.6846 3.40597 11.1373C3.14681 10.6488 3 10.0915 3 9.49994C3 8.03508 3.89998 6.78115 5.17714 6.25964C5.6621 6.06162 6 5.52377 6 4.99994C6 3.34308 7.34315 1.99994 9 1.99994Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M15 1.99994C13.3431 1.99994 12 3.34308 12 4.99994V18.9999C12 20.6568 13.3431 21.9999 15 21.9999C16.487 21.9999 17.7213 20.9181 17.9588 19.4985C18.0024 19.2379 18.2358 18.9999 18.5 18.9999C20.433 18.9999 22 17.4329 22 15.4999C22 14.4932 21.575 13.5853 20.8946 12.9465C20.443 12.5225 20.3037 11.6846 20.594 11.1373C20.8532 10.6488 21 10.0915 21 9.49994C21 8.03508 20.1 6.78115 18.8229 6.25964C18.3379 6.06162 18 5.52377 18 4.99994C18 3.34308 16.6569 1.99994 15 1.99994Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 9.99994C12 11.1045 11.1046 11.9999 10 11.9999C8.89543 11.9999 8 11.1045 8 9.99994" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M16 14.9999C16 13.8954 15.1046 12.9999 14 12.9999C12.8954 12.9999 12 13.8954 12 14.9999" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // Cancel01Icon
  close: '<path d="M18.9991 19L5 5.00098" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M19 5L5.00092 18.999" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // CheckmarkCircle02Icon
  done: '<path d="M22 12C22 6.47715 17.5228 2 12 2C6.47715 2 2 6.47715 2 12C2 17.5228 6.47715 22 12 22C17.5228 22 22 17.5228 22 12Z" stroke="currentColor"/><path d="M8 12.5L10.5 15L16 9" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // AlertCircleIcon
  alert: '<circle cx="12" cy="12" r="10" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 8V12" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M12.125 15.75H12M12.25 15.75C12.25 15.8881 12.1381 16 12 16C11.8619 16 11.75 15.8881 11.75 15.75C11.75 15.6119 11.8619 15.5 12 15.5C12.1381 15.5 12.25 15.6119 12.25 15.75Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // SquareLock02Icon
  lock: '<path d="M12 14.5V17.5M13 14.5C13 15.0523 12.5523 15.5 12 15.5C11.4477 15.5 11 15.0523 11 14.5C11 13.9477 11.4477 13.5 12 13.5C12.5523 13.5 13 13.9477 13 14.5Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M16.5 9V6.5C16.5 4.01472 14.4853 2 12 2C9.51471 2 7.49999 4.01472 7.49999 6.5V9" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M4.2678 18.8447C4.49268 20.515 5.87612 21.8235 7.55965 21.9009C8.97627 21.966 10.4153 22 12 22C13.5847 22 15.0237 21.966 16.4403 21.9009C18.1239 21.8235 19.5073 20.515 19.7322 18.8447C19.8789 17.7547 20 16.6376 20 15.5C20 14.3624 19.8789 13.2453 19.7322 12.1553C19.5073 10.485 18.1239 9.17649 16.4403 9.09909C15.0237 9.03397 13.5847 9 12 9C10.4153 9 8.97627 9.03397 7.55965 9.09909C5.87612 9.17649 4.49268 10.485 4.2678 12.1553C4.12104 13.2453 3.99999 14.3624 3.99999 15.5C3.99999 16.6376 4.12104 17.7547 4.2678 18.8447Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // PlayListIcon
  list: '<path d="M2.50012 7.5H21.5001" stroke="currentColor" stroke-linejoin="round"/><path d="M17.0001 2.5L14.0001 7.5" stroke="currentColor" stroke-linejoin="round"/><path d="M10.0001 2.5L7.00012 7.5" stroke="currentColor" stroke-linejoin="round"/><path d="M2.5 12C2.5 7.52166 2.5 5.28249 3.89124 3.89124C5.28249 2.5 7.52166 2.5 12 2.5C16.4783 2.5 18.7175 2.5 20.1088 3.89124C21.5 5.28249 21.5 7.52166 21.5 12C21.5 16.4783 21.5 18.7175 20.1088 20.1088C18.7175 21.5 16.4783 21.5 12 21.5C7.52166 21.5 5.28249 21.5 3.89124 20.1088C2.5 18.7175 2.5 16.4783 2.5 12Z" stroke="currentColor"/><path d="M14.9531 14.8948C14.8016 15.5215 14.0857 15.9644 12.6539 16.8502C11.2697 17.7064 10.5777 18.1346 10.0199 17.9625C9.78934 17.8913 9.57925 17.7562 9.40982 17.57C9 17.1198 9 16.2465 9 14.5C9 12.7535 9 11.8802 9.40982 11.4299C9.57925 11.2438 9.78934 11.1087 10.0199 11.0375C10.5777 10.8654 11.2697 11.2936 12.6539 12.1498C14.0857 13.0356 14.8016 13.4785 14.9531 14.1052C15.0156 14.3639 15.0156 14.6361 14.9531 14.8948Z" stroke="currentColor" stroke-linejoin="round"/>',
  // Copy01Icon
  copy: '<path d="M7.5 14.5C7.5 11.2002 7.5 9.55025 8.52513 8.52513C9.55025 7.5 11.2002 7.5 14.5 7.5C17.7998 7.5 19.4497 7.5 20.4749 8.52513C21.5 9.55025 21.5 11.2002 21.5 14.5C21.5 17.7998 21.5 19.4497 20.4749 20.4749C19.4497 21.5 17.7998 21.5 14.5 21.5C11.2002 21.5 9.55025 21.5 8.52513 20.4749C7.5 19.4497 7.5 17.7998 7.5 14.5Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M7.5 16.5C6.10355 16.5 5.40533 16.5 4.84402 16.3036C3.83866 15.9518 3.0482 15.1613 2.69641 14.156C2.5 13.5947 2.5 12.8964 2.5 11.5V9.5C2.5 6.20017 2.5 4.55025 3.52513 3.52513C4.55025 2.5 6.20017 2.5 9.5 2.5H11.5C12.8964 2.5 13.5947 2.5 14.156 2.69641C15.1613 3.0482 15.9518 3.83866 16.3036 4.84402C16.5 5.40533 16.5 6.10355 16.5 7.5" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // File01Icon
  doc: '<path d="M8 7L16 7" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M8 11L12 11" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M13 21.5V21C13 18.1716 13 16.7574 13.8787 15.8787C14.7574 15 16.1716 15 19 15H19.5M20 13.3431V10C20 6.22876 20 4.34315 18.8284 3.17157C17.6569 2 15.7712 2 12 2C8.22877 2 6.34315 2 5.17157 3.17157C4 4.34314 4 6.22876 4 10L4 14.5442C4 17.7892 4 19.4117 4.88607 20.5107C5.06508 20.7327 5.26731 20.9349 5.48933 21.1139C6.58831 22 8.21082 22 11.4558 22C12.1614 22 12.5141 22 12.8372 21.886C12.9044 21.8623 12.9702 21.835 13.0345 21.8043C13.3436 21.6564 13.593 21.407 14.0919 20.9081L18.8284 16.1716C19.4065 15.5935 19.6955 15.3045 19.8478 14.9369C20 14.5694 20 14.1606 20 13.3431Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // Image01Icon
  image: '<circle cx="7.5" cy="7.5" r="1.5" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M2.5 12C2.5 7.52166 2.5 5.28249 3.89124 3.89124C5.28249 2.5 7.52166 2.5 12 2.5C16.4783 2.5 18.7175 2.5 20.1088 3.89124C21.5 5.28249 21.5 7.52166 21.5 12C21.5 16.4783 21.5 18.7175 20.1088 20.1088C18.7175 21.5 16.4783 21.5 12 21.5C7.52166 21.5 5.28249 21.5 3.89124 20.1088C2.5 18.7175 2.5 16.4783 2.5 12Z" stroke="currentColor"/><path d="M5 21C9.37246 15.775 14.2741 8.88406 21.4975 13.5424" stroke="currentColor"/>',
  // Clock01Icon
  clock: '<circle cx="12" cy="12" r="10" stroke="currentColor"/><path d="M12 8V12L14 14" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // RefreshIcon
  retry: '<path d="M20.0092 2V5.13219C20.0092 5.42605 19.6418 5.55908 19.4537 5.33333C17.6226 3.2875 14.9617 2 12 2C6.47715 2 2 6.47715 2 12C2 17.5228 6.47715 22 12 22C17.5228 22 22 17.5228 22 12" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // StopCircleIcon
  stop: '<circle cx="12" cy="12" r="10" stroke="currentColor"/><path d="M9.38886 15.1629C9.89331 15.5 10.5955 15.5 12 15.5C13.4045 15.5 14.1067 15.5 14.6111 15.1629C14.8295 15.017 15.017 14.8295 15.1629 14.6111C15.5 14.1067 15.5 13.4045 15.5 12C15.5 10.5955 15.5 9.89331 15.1629 9.38886C15.017 9.17048 14.8295 8.98298 14.6111 8.83706C14.1067 8.5 13.4045 8.5 12 8.5C10.5955 8.5 9.89331 8.5 9.38886 8.83706C9.17048 8.98298 8.98298 9.17048 8.83706 9.38886C8.5 9.89331 8.5 10.5955 8.5 12C8.5 13.4045 8.5 14.1067 8.83706 14.6111C8.98298 14.8295 9.17048 15.017 9.38886 15.1629Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // Tick02Icon
  tick: '<path d="M5 14L8.5 17.5L19 6.5" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // MusicNote01Icon
  music: '<path d="M7 9.5C7 10.8807 5.88071 12 4.5 12C3.11929 12 2 10.8807 2 9.5C2 8.11929 3.11929 7 4.5 7C5.88071 7 7 8.11929 7 9.5ZM7 9.5V2C7.33333 2.5 7.6 4.6 10 5" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><circle cx="10.5" cy="19.5" r="2.5" stroke="currentColor"/><circle cx="20" cy="18" r="2" stroke="currentColor"/><path d="M13 19.5L13 11C13 10.09 13 9.63502 13.2466 9.35248C13.4932 9.06993 13.9938 9.00163 14.9949 8.86504C18.0085 8.45385 20.2013 7.19797 21.3696 6.42937C21.6498 6.24509 21.7898 6.15295 21.8949 6.20961C22 6.26627 22 6.43179 22 6.76283V17.9259" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M13 13C17.8 13 21 10.6667 22 10" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // Video01Icon
  video: '<path d="M2 11C2 7.70017 2 6.05025 3.02513 5.02513C4.05025 4 5.70017 4 9 4H10C13.2998 4 14.9497 4 15.9749 5.02513C17 6.05025 17 7.70017 17 11V13C17 16.2998 17 17.9497 15.9749 18.9749C14.9497 20 13.2998 20 10 20H9C5.70017 20 4.05025 20 3.02513 18.9749C2 17.9497 2 16.2998 2 13V11Z" stroke="currentColor"/><path d="M17 8.90585L17.1259 8.80196C19.2417 7.05623 20.2996 6.18336 21.1498 6.60482C22 7.02628 22 8.42355 22 11.2181V12.7819C22 15.5765 22 16.9737 21.1498 17.3952C20.2996 17.8166 19.2417 16.9438 17.1259 15.198L17 15.0941" stroke="currentColor" stroke-linecap="round"/><circle cx="11.5" cy="9.5" r="1.5" stroke="currentColor"/>',
  // InformationCircleIcon
  info: '<circle cx="12" cy="12" r="10" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 16V12" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M12.125 8.25H12M12.25 8.25C12.25 8.11193 12.1381 8 12 8C11.8619 8 11.75 8.11193 11.75 8.25C11.75 8.38807 11.8619 8.5 12 8.5C12.1381 8.5 12.25 8.38807 12.25 8.25Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // Shield01Icon
  shield: '<path d="M18.7088 3.49534C16.8165 2.55382 14.5009 2 12 2C9.4991 2 7.1835 2.55382 5.29116 3.49534C4.36318 3.95706 3.89919 4.18792 3.4496 4.91378C3 5.63965 3 6.34248 3 7.74814V11.2371C3 16.9205 7.54236 20.0804 10.173 21.4338C10.9067 21.8113 11.2735 22 12 22C12.7265 22 13.0933 21.8113 13.8269 21.4338C16.4576 20.0804 21 16.9205 21 11.2371L21 7.74814C21 6.34249 21 5.63966 20.5504 4.91378C20.1008 4.18791 19.6368 3.95706 18.7088 3.49534Z" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',
  // WifiDisconnected01Icon
  offline: '<path d="M8.5 15.9996C9.36651 15.1331 10.4207 14.642 11.5 14.5264" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M5.5 12.5C8.67327 9.32673 12.6221 8.67087 16 10.5324" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M2 8.5C8.31579 3.16669 15.6842 3.16668 22 8.49989" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/><path d="M21.0002 13.5L15.0002 19.5M21.0002 19.5L15.0002 13.5" stroke="currentColor" stroke-linecap="round"/>',
  // ViewIcon
  view: '<path d="M21.544 11.045C21.848 11.4713 22 11.6845 22 12C22 12.3155 21.848 12.5287 21.544 12.955C20.1779 14.8706 16.6892 19 12 19C7.31078 19 3.8221 14.8706 2.45604 12.955C2.15201 12.5287 2 12.3155 2 12C2 11.6845 2.15201 11.4713 2.45604 11.045C3.8221 9.12944 7.31078 5 12 5C16.6892 5 20.1779 9.12944 21.544 11.045Z" stroke="currentColor"/><path d="M15 12C15 10.3431 13.6569 9 12 9C10.3431 9 9 10.3431 9 12C9 13.6569 10.3431 15 12 15C13.6569 15 15 13.6569 15 12Z" stroke="currentColor"/>',
  // ArrowRight01Icon
  next: '<path d="M9.00005 6C9.00005 6 15 10.4189 15 12C15 13.5812 9 18 9 18" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>',

};

const SVG_NS = 'http://www.w3.org/2000/svg';
const tpl = document.createElement('template');

/** 图标 → <svg> 元素（图标数据是写死的，不含网页内容，可以放心用 innerHTML） */
export function icon(name, size = 16, stroke = 1.75) {
  tpl.innerHTML = `<svg xmlns="${SVG_NS}" viewBox="0 0 24 24" width="${size}" height="${size}" fill="none" stroke-width="${stroke}" aria-hidden="true" focusable="false">${ICONS[name] || ''}</svg>`;
  return tpl.content.firstElementChild;
}

/** Captae 的标：黑色圆角方块 + 白色取景框和圆点（和应用图标、桌面端侧栏是同一个） */
let logoSeq = 0;
export function logo(size = 20) {
  const id = 'cx-logo-' + (++logoSeq);
  tpl.innerHTML = `<svg xmlns="${SVG_NS}" class="cx-logo" viewBox="0 0 64 64" width="${size}" height="${size}" aria-hidden="true" focusable="false">
<defs><linearGradient id="${id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#2e2e30"/><stop offset="1" stop-color="#0e0e0f"/></linearGradient></defs>
<path d="M0 18C0 5.4 5.4 0 18 0h28c12.6 0 18 5.4 18 18v28c0 12.6-5.4 18-18 18H18C5.4 64 0 58.6 0 46Z" fill="url(#${id})"/>
<g transform="translate(14.6 14.6) scale(1.45)" fill="none" stroke="#fff" stroke-width="2.35" stroke-linecap="round" stroke-linejoin="round">
<path d="M4 8V5a1 1 0 0 1 1-1h3M16 4h3a1 1 0 0 1 1 1v3M20 16v3a1 1 0 0 1-1 1h-3M8 20H5a1 1 0 0 1-1-1v-3"/><circle cx="12" cy="12" r="2.6"/></g></svg>`;
  return tpl.content.firstElementChild;
}

/** 进度环（16px）：p 是 0–1；busy 时是一个慢慢呼吸的点（还不知道进度） */
export function ring(p = 0, busy = false) {
  tpl.innerHTML = `<svg xmlns="${SVG_NS}" class="cx-ring" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true" focusable="false">
<circle cx="8" cy="8" r="6" fill="none" stroke="rgba(0,0,0,.12)" stroke-width="2"/>
<circle class="fg" cx="8" cy="8" r="6" fill="none" stroke="var(--accent)" stroke-width="2" stroke-linecap="round" pathLength="100" stroke-dasharray="0 100" transform="rotate(-90 8 8)"/>
<circle class="dot" cx="8" cy="8" r="2.5" fill="var(--ink-3)"/></svg>`;
  const el = tpl.content.firstElementChild;
  setRing(el, p, busy);
  return el;
}
export function setRing(el, p, busy) {
  const v = Math.max(0, Math.min(1, isFinite(p) ? p : 0));
  el.querySelector('.fg').setAttribute('stroke-dasharray', `${busy ? 0 : v * 100} 100`);
  el.toggleAttribute('data-busy', !!busy);
}

/** 完成的勾：画圈 250ms + 打勾 200ms（配方 m1；fresh=false 时直接显示画好的样子） */
export function doneMark(fresh = true, size = 16) {
  tpl.innerHTML = `<svg xmlns="${SVG_NS}" class="cx-done${fresh ? ' go' : ''}" viewBox="0 0 18 18" width="${size}" height="${size}" fill="none" aria-hidden="true" focusable="false">
<circle cx="9" cy="9" r="7.25" stroke="currentColor" stroke-width="1.5" pathLength="100"/>
<path class="tk" d="M5.9 9.2l2.1 2.1 4.1-4.4" stroke="currentColor" stroke-width="1.7" stroke-linecap="round" stroke-linejoin="round" pathLength="100"/></svg>`;
  return tpl.content.firstElementChild;
}

/** 建 DOM：h('button', {class: 'x', on: {click: f}, data: {k: 1}, attrs: {'aria-label': '关闭'}}, '文字', 子元素…)
 *  文字一律走 textContent，网页里拿来的标题不会被当成 HTML。 */
export function h(tag, props, ...kids) {
  const el = document.createElement(tag);
  if (props) {
    for (const [k, v] of Object.entries(props)) {
      if (v == null || v === false) continue;
      if (k === 'class') el.className = v;
      else if (k === 'text') el.textContent = v;
      else if (k === 'on') for (const [ev, f] of Object.entries(v)) el.addEventListener(ev, f);
      else if (k === 'data') for (const [dk, dv] of Object.entries(v)) { if (dv != null && dv !== false) el.dataset[dk] = dv === true ? '' : String(dv); }
      else if (k === 'attrs') for (const [ak, av] of Object.entries(v)) { if (av != null && av !== false) el.setAttribute(ak, av === true ? '' : String(av)); }
      else if (k === 'style') el.style.cssText = v;
      else el[k] = v;
    }
  }
  for (const c of kids.flat()) {
    if (c == null || c === false || c === '') continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
  return el;
}

/* ---------------- 格式化 ---------------- */

/** 字节 → 「约 312 MB」 */
export function mb(bytes) {
  if (!bytes || !isFinite(bytes)) return '';
  if (bytes >= 1e9) return tr('约{size} GB', { size: decimal(bytes / 1e9, 1) });
  if (bytes >= 1e6) return tr('约{size} MB', { size: Math.round(bytes / 1e6) });
  return tr('约{size} KB', { size: Math.max(1, Math.round(bytes / 1e3)) });
}
/** 桌面端任务里的大小是 MB（小数）；单位各语言一样，小数点按界面语言 */
export function mbOf(m) {
  if (!m || !isFinite(m)) return '';
  if (m >= 1000) return `${decimal(m / 1000, 1)} GB`;
  if (m >= 10) return `${Math.round(m)} MB`;
  return `${decimal(m, 1)} MB`;
}
/** 速度（MB/s） */
export function speedOf(m) {
  if (!m || !isFinite(m) || m <= 0) return '';
  if (m >= 1) return `${decimal(m, 1)} MB/s`;
  return `${Math.max(1, Math.round(m * 1000))} KB/s`;
}
/** 秒 → 14:48 / 1:02:03 */
export function clock(s) {
  if (!s || !isFinite(s)) return '';
  s = Math.round(s);
  const hh = Math.floor(s / 3600), m = Math.floor(s % 3600 / 60), x = s % 60;
  return hh ? `${hh}:${String(m).padStart(2, '0')}:${String(x).padStart(2, '0')}` : `${m}:${String(x).padStart(2, '0')}`;
}
/** 网址 → 不带 www 的主机名 */
export function host(u) {
  try { return new URL(u).hostname.replace(/^www\./, ''); } catch { return ''; }
}
