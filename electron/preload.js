const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('agentboxDesktop', {
    isElectron: true,
    minimize: () => ipcRenderer.send('window-minimize'),
    maximize: () => ipcRenderer.send('window-maximize'),
    close: () => ipcRenderer.send('window-close'),
});
