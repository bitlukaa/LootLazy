// Questro native window controls and Rust command bridge.
if (window.__TAURI__) {
  const { invoke } = window.__TAURI__.core;
  const win = window.__TAURI__.window.getCurrentWindow();
  window.questroRequest = (url, body) => invoke('api', { route: url, body: body ?? null });
  window.desktopAPI = {
    minimize: () => win.minimize(), maximize: () => win.toggleMaximize(), close: () => invoke('quit'),
    onMaximized: callback => { win.onResized(async () => callback(await win.isMaximized())); }
  };
  document.addEventListener('mousedown', event => {
    if (event.button === 0 && event.target.closest('.titlebar') && !event.target.closest('button')) {
      if (event.detail === 2) win.toggleMaximize(); else win.startDragging();
    }
  });
}
