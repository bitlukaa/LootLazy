// Tauri uses native commands. Electron's compatibility bridge stays available in 2.x.
if (window.__TAURI__) {
  const { invoke } = window.__TAURI__.core;
  const win = window.__TAURI__.window.getCurrentWindow();
  window.questroRequest = (url, body) => invoke('api', { route: url, body: body ?? null });
  window.electronAPI = {
    minimize: () => win.minimize(), maximize: () => win.toggleMaximize(), close: () => win.close(),
    onMaximized: callback => { win.onResized(async () => callback(await win.isMaximized())); }
  };
  document.addEventListener('mousedown', event => {
    if (event.button === 0 && event.target.closest('.titlebar') && !event.target.closest('button')) win.startDragging();
  });
  document.addEventListener('dblclick', event => {
    if (event.target.closest('.titlebar') && !event.target.closest('button')) win.toggleMaximize();
  });
}
