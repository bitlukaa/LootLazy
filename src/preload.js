const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  maximize: () => ipcRenderer.send('window-maximize'),
  minimize: () => ipcRenderer.send('window-minimize'),
  close: () => ipcRenderer.send('window-close'),
  onMaximized: callback => {const listener=(_event,value)=>callback(value===true);ipcRenderer.on('window-maximized',listener);return ()=>ipcRenderer.removeListener('window-maximized',listener);}
});
