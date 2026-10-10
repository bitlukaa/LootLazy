const { app, BrowserWindow, Tray, Menu, ipcMain, dialog } = require('electron');
const path = require('path');
const fs=require('fs');
const verificationArgument=process.argv.find(argument=>argument.startsWith('--verify-portable-storage='));
const applicationRoot=path.resolve(__dirname,'..');
let verificationOutput=null;
if(verificationArgument){verificationOutput=path.resolve(verificationArgument.slice('--verify-portable-storage='.length));if(!verificationOutput.startsWith(path.join(applicationRoot,'data','.verification')+path.sep))throw new Error('Invalid verification path');}
require('./portableStorage').configurePortableStorage(app,verificationOutput?path.join(path.dirname(verificationOutput),'application'):applicationRoot,{migratePreferences:!verificationOutput});
const server = verificationOutput?null:require('./server');

let mainWindow = null;
let tray = null;
app.setAppUserModelId('Questro');

const gotTheLock = !verificationOutput&&app.requestSingleInstanceLock();

if(verificationOutput){
  app.whenReady().then(()=>{const names=['userData','sessionData','temp','logs','crashDumps'];fs.writeFileSync(verificationOutput,JSON.stringify(Object.fromEntries(names.map(name=>[name,app.getPath(name)]))));app.quit();});
} else if (!gotTheLock) {
  app.quit();
} else {
  app.on('second-instance', (event, commandLine, workingDirectory) => {
    if (mainWindow) {
      if (mainWindow.isMinimized()) mainWindow.restore();
      mainWindow.show();
      mainWindow.focus();
    }
  });

  app.on('ready', async () => {
    try {
      await server.start();
    } catch(e) { dialog.showErrorBox('Questro startup failed', e.message); app.quit(); return; }
    
    createWindow();
    createTray();
    
    ipcMain.on('window-minimize', () => {
      if (mainWindow) mainWindow.minimize();
    });
    
    ipcMain.on('window-maximize', () => { if (mainWindow) mainWindow.isMaximized() ? mainWindow.unmaximize() : mainWindow.maximize(); });
    ipcMain.on('window-close', () => {
      app.quit();
    });
  });

  app.on('window-all-closed', () => {
    app.quit();
  });
  app.on('before-quit', () => {
    server.queueManager.dispose();
    server.questQueue.dispose();
    require('./processManager').stopSimulationSync();
    server.server.close();
  });
}

function createWindow() {
  if (mainWindow) {
    mainWindow.show();
    return;
  }

  mainWindow = new BrowserWindow({
    title: 'Questro',
    width: 1100,
    height: 750,
    minWidth: 800,
    minHeight: 600,
    frame: false,
    show: false,
    backgroundColor: '#0c0e15',
    icon: path.join(__dirname, '../public/favicon.ico'),
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js')
    }
  });

  mainWindow.setMenu(null);
  mainWindow.on('maximize',()=>mainWindow?.webContents.send('window-maximized',true));
  mainWindow.on('unmaximize',()=>mainWindow?.webContents.send('window-maximized',false));
  mainWindow.once('ready-to-show',()=>mainWindow?.show());

  mainWindow.loadURL(`http://127.0.0.1:${server.PORT}`);

  mainWindow.on('closed', () => { mainWindow = null; });
}

function createTray() {
  tray = new Tray(path.join(__dirname, '../public/favicon.ico'));
  const contextMenu = Menu.buildFromTemplate([
    { label: 'Show App', click: () => { createWindow(); } },
    { type: 'separator' },
    { label: 'Quit', click: () => { 
        app.isQuiting = true; 
        app.quit(); 
      } 
    }
  ]);
  tray.setToolTip('Questro');
  tray.setContextMenu(contextMenu);
  tray.on('double-click', () => {
    createWindow();
  });
}
