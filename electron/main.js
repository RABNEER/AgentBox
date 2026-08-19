const { app, BrowserWindow, Tray, Menu, nativeImage, ipcMain, Notification } = require('electron');
const path = require('path');
const fs = require('fs');
const { spawn } = require('child_process');
const http = require('http');

let mainWindow = null;
let tray = null;
let rustProcess = null;
let isQuitting = false;
let activePort = 3000;

// 1. Fast, non-blocking server check
function checkServer(port = 3000) {
    return new Promise((resolve) => {
        const req = http.get(`http://127.0.0.1:${port}/v1/config`, (res) => {
            resolve(res.statusCode === 200 || res.statusCode === 404);
        });
        req.on('error', () => resolve(false));
        req.setTimeout(300, () => {
            req.destroy();
            resolve(false);
        });
    });
}

// 2. Start background Rust Engine asynchronously
async function startBackendEngine() {
    // Check if already running on 3000 or 3001
    if (await checkServer(3000)) {
        activePort = 3000;
        setupSSEListener(activePort);
        return activePort;
    }
    if (await checkServer(3001)) {
        activePort = 3001;
        setupSSEListener(activePort);
        return activePort;
    }

    const possiblePaths = [
        path.join(process.resourcesPath, 'bin', 'agentbox-mail.exe'),
        path.join(process.resourcesPath, 'agentbox-mail.exe'),
        path.join(__dirname, '..', 'bin', 'agentbox-mail.exe'),
        path.join(__dirname, '..', 'target', 'release', 'agentbox-mail.exe'),
        path.join(__dirname, '..', 'target', 'debug', 'agentbox-mail.exe')
    ];

    let binPath = possiblePaths.find(p => fs.existsSync(p));
    if (!binPath && app.isPackaged) {
        binPath = path.join(process.resourcesPath, 'bin', 'agentbox-mail.exe');
    } else if (!binPath) {
        binPath = path.join(__dirname, '..', 'target', 'release', 'agentbox-mail.exe');
    }

    if (fs.existsSync(binPath)) {
        try {
            rustProcess = spawn(binPath, ['server', '--port', '3000'], {
                cwd: app.isPackaged ? app.getPath('userData') : path.join(__dirname, '..'),
                stdio: 'ignore',
                detached: false,
                windowsHide: true,
            });

            rustProcess.on('error', (err) => {
                console.error('[Electron] Backend daemon error:', err);
            });
        } catch (err) {
            console.error('[Electron] Failed to start backend engine:', err);
        }
    }

    // Fast poll for server readiness (max 4 seconds)
    for (let i = 0; i < 20; i++) {
        await new Promise((r) => setTimeout(r, 200));
        if (await checkServer(3000)) {
            activePort = 3000;
            break;
        }
        if (await checkServer(3001)) {
            activePort = 3001;
            break;
        }
    }

    setupSSEListener(activePort);
    return activePort;
}

// 3. Realtime SSE Notifications
function setupSSEListener(port) {
    try {
        const req = http.get(`http://127.0.0.1:${port}/v1/events`, (res) => {
            let buffer = '';
            res.on('data', (chunk) => {
                buffer += chunk.toString();
                const lines = buffer.split('\n\n');
                buffer = lines.pop();

                for (const line of lines) {
                    if (line.startsWith('data: ')) {
                        try {
                            const data = JSON.parse(line.substring(6));
                            if (data.type === 'new_message' && data.message) {
                                showNativeNotification(data.message);
                            }
                        } catch (e) {}
                    }
                }
            });
        });
        req.on('error', () => {
            setTimeout(() => setupSSEListener(port), 4000);
        });
    } catch (e) {
        setTimeout(() => setupSSEListener(port), 4000);
    }
}

function showNativeNotification(msg) {
    if (!Notification.isSupported()) return;

    let title = `✉️ New Email from ${msg.from_address}`;
    let body = msg.subject || '(No Subject)';

    if (msg.extracted_otp) {
        title = `🔑 OTP Code: ${msg.extracted_otp}`;
        body = `From: ${msg.from_address} | ${msg.subject || ''}`;
    }

    const notification = new Notification({
        title,
        body,
        silent: false,
    });

    notification.on('click', () => {
        if (mainWindow) {
            mainWindow.show();
            mainWindow.focus();
        }
    });

    notification.show();
}

// 4. Create Desktop Window
function createWindow() {
    mainWindow = new BrowserWindow({
        width: 1300,
        height: 840,
        minWidth: 960,
        minHeight: 620,
        backgroundColor: '#000000',
        title: 'AgentBox Mail',
        show: false, // Show gracefully once ready
        autoHideMenuBar: true,
        webPreferences: {
            preload: path.join(__dirname, 'preload.js'),
            contextIsolation: true,
            nodeIntegration: false,
        },
    });

    // Show window as soon as DOM is ready
    mainWindow.once('ready-to-show', () => {
        mainWindow.show();
        mainWindow.focus();
    });

    // Start loading backend
    startBackendEngine().then((port) => {
        if (mainWindow && !mainWindow.isDestroyed()) {
            mainWindow.loadURL(`http://127.0.0.1:${port}`).catch(() => {
                // Fallback to local ui if network issue
                const uiIndex = path.join(__dirname, '..', 'ui', 'index.html');
                if (fs.existsSync(uiIndex)) {
                    mainWindow.loadFile(uiIndex);
                }
            });
        }
    });

    // Minimize to tray on close
    mainWindow.on('close', (event) => {
        if (!isQuitting) {
            event.preventDefault();
            mainWindow.hide();
            if (Notification.isSupported()) {
                new Notification({
                    title: 'AgentBox Mail',
                    body: 'AgentBox is running in the background listening for emails and AI tasks.',
                }).show();
            }
        }
    });
}

// 5. System Tray Integration
function createTray() {
    const icon = nativeImage.createFromBuffer(
        Buffer.from(
            'iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAAXNSR0IArs4c6QAAAExJREFUOE9jZKAQMFKon2HUAAYGBoa/QPz///8fqhhGomvhBhgNIIyGAYyGgYGBgeEvPvX4/P//n5GBgeE3UP0/ooxkhw5aGMBoGMAgAwC14x4x6n6KwwAAAABJRU5ErkJggg==',
            'base64'
        )
    );

    tray = new Tray(icon);
    tray.setToolTip('AgentBox Mail — Autonomous AI Mailbox');

    const contextMenu = Menu.buildFromTemplate([
        {
            label: '⚡ Open AgentBox Mail',
            click: () => {
                if (mainWindow) {
                    mainWindow.show();
                    mainWindow.focus();
                }
            },
        },
        {
            label: '🤖 Connect AI Agents',
            click: () => {
                if (mainWindow) {
                    mainWindow.show();
                    mainWindow.focus();
                    mainWindow.webContents.executeJavaScript('openConnectAgentsModal();');
                }
            },
        },
        { type: 'separator' },
        {
            label: '❌ Quit AgentBox',
            click: () => {
                isQuitting = true;
                if (rustProcess) {
                    try { rustProcess.kill(); } catch (e) {}
                }
                app.quit();
            },
        },
    ]);

    tray.setContextMenu(contextMenu);
    tray.on('double-click', () => {
        if (mainWindow) {
            mainWindow.show();
            mainWindow.focus();
        }
    });
}

// IPC Handlers
ipcMain.on('window-minimize', () => {
    if (mainWindow) mainWindow.minimize();
});
ipcMain.on('window-maximize', () => {
    if (mainWindow) {
        if (mainWindow.isMaximized()) {
            mainWindow.unmaximize();
        } else {
            mainWindow.maximize();
        }
    }
});
ipcMain.on('window-close', () => {
    if (mainWindow) mainWindow.close();
});

// App Lifecycle
app.whenReady().then(() => {
    createWindow();
    createTray();

    app.on('activate', () => {
        if (BrowserWindow.getAllWindows().length === 0) {
            createWindow();
        }
    });
});

app.on('before-quit', () => {
    isQuitting = true;
    if (rustProcess) {
        try { rustProcess.kill(); } catch (e) {}
    }
});

app.on('window-all-closed', () => {
    if (process.platform !== 'darwin' && isQuitting) {
        app.quit();
    }
});
