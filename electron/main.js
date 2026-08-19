const { app, BrowserWindow, Tray, Menu, nativeImage, ipcMain, Notification } = require('electron');
const path = require('path');
const { spawn } = require('child_process');
const http = require('http');

let mainWindow = null;
let tray = null;
let rustProcess = null;
let isQuitting = false;

const fs = require('fs');

let activePort = 3000;

// 1. Child Process Management (Spawn background Rust engine if not active)
function isServerRunning(port = 3000) {
    return new Promise((resolve) => {
        const req = http.get(`http://localhost:${port}/v1/config`, (res) => {
            resolve(res.statusCode === 200 || res.statusCode === 404);
        });
        req.on('error', () => resolve(false));
        req.setTimeout(800, () => {
            req.destroy();
            resolve(false);
        });
    });
}

async function ensureBackendEngine() {
    // Check if server is already running on 3000..3015
    for (let p = 3000; p <= 3015; p++) {
        if (await isServerRunning(p)) {
            activePort = p;
            console.log(`[Electron] AgentBox Rust daemon is already running on port ${activePort}.`);
            setupSSEListener(activePort);
            return activePort;
        }
    }

    console.log('[Electron] Spawning AgentBox Rust engine background daemon...');
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

    try {
        rustProcess = spawn(binPath, ['server', '--port', '3000'], {
            cwd: app.isPackaged ? app.getPath('userData') : path.join(__dirname, '..'),
            stdio: 'ignore',
            detached: false,
            windowsHide: true,
        });

        rustProcess.on('error', (err) => {
            console.error('[Electron] Failed to spawn Rust binary:', err);
        });

        // Wait for server to boot on 3000..3015
        for (let i = 0; i < 25; i++) {
            await new Promise((r) => setTimeout(r, 300));
            for (let p = 3000; p <= 3015; p++) {
                if (await isServerRunning(p)) {
                    activePort = p;
                    console.log(`[Electron] Connected to freshly spawned AgentBox engine on port ${activePort}.`);
                    setupSSEListener(activePort);
                    return activePort;
                }
            }
        }
    } catch (err) {
        console.error('[Electron] Error starting background engine:', err);
    }

    setupSSEListener(activePort);
    return activePort;
}

// 2. Native Desktop OS Notifications via SSE
function setupSSEListener() {
    try {
        const req = http.get('http://localhost:3000/v1/events', (res) => {
            let buffer = '';
            res.on('data', (chunk) => {
                buffer += chunk.toString();
                const lines = buffer.split('\n\n');
                buffer = lines.pop(); // keep remainder

                for (const line of lines) {
                    if (line.startsWith('data: ')) {
                        try {
                            const data = JSON.parse(line.substring(6));
                            if (data.type === 'new_message' && data.message) {
                                const msg = data.message;
                                showNativeNotification(msg);
                            }
                        } catch (e) {}
                    }
                }
            });
        });
        req.on('error', (e) => {
            // Reconnect after 3 seconds
            setTimeout(setupSSEListener, 3000);
        });
    } catch (e) {
        setTimeout(setupSSEListener, 3000);
    }
}

function showNativeNotification(msg) {
    if (!Notification.isSupported()) return;

    let title = `✉️ New Email from ${msg.from_address}`;
    let body = msg.subject || '(No Subject)';

    if (msg.extracted_otp) {
        title = `🔑 OTP Verification Code: ${msg.extracted_otp}`;
        body = `From: ${msg.from_address}\nSubject: ${msg.subject || ''}`;
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

// 3. Create Main Window
function createWindow() {
    mainWindow = new BrowserWindow({
        width: 1280,
        height: 820,
        minWidth: 900,
        minHeight: 600,
        backgroundColor: '#000000',
        title: 'AgentBox Mail',
        titleBarStyle: 'hidden',
        titleBarOverlay: {
            color: '#000000',
            symbolColor: '#ffffff',
            height: 38,
        },
        webPreferences: {
            preload: path.join(__dirname, 'preload.js'),
            contextIsolation: true,
            nodeIntegration: false,
        },
    });

    mainWindow.loadURL(`http://localhost:${activePort}`);

    // Minimize to tray on close
    mainWindow.on('close', (event) => {
        if (!isQuitting) {
            event.preventDefault();
            mainWindow.hide();
            if (Notification.isSupported()) {
                new Notification({
                    title: 'AgentBox Mail',
                    body: 'AgentBox is running in the background listening for emails and OTPs.',
                }).show();
            }
        }
    });
}

// 4. System Tray Integration
function createTray() {
    const icon = nativeImage.createFromBuffer(
        Buffer.from(
            'iVBORw0KGgoAAAANSUhEUgAAABAAAAAQCAYAAAAf8/9hAAAAAXNSR0IArs4c6QAAAExJREFUOE9jZKAQMFKon2HUAAYGBoa/QPz///8fqhhGomvhBhgNIIyGAYyGgYGBgeEvPvX4/P//n5GBgeE3UP0/ooxkhw5aGMBoGMAgAwC14x4x6n6KwwAAAABJRU5ErkJggg==',
            'base64'
        )
    );

    tray = new Tray(icon);
    tray.setToolTip('AgentBox Mail — 24/7 Autonomous Mailbox');

    const contextMenu = Menu.buildFromTemplate([
        {
            label: '⚡ Open AgentBox Dashboard',
            click: () => {
                if (mainWindow) {
                    mainWindow.show();
                    mainWindow.focus();
                }
            },
        },
        {
            label: '⚙️ Mailbox Setup Wizard',
            click: () => {
                if (mainWindow) {
                    mainWindow.show();
                    mainWindow.focus();
                    mainWindow.webContents.executeJavaScript('openSetupWizard();');
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

// 5. IPC Handlers
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
app.whenReady().then(async () => {
    await ensureBackendEngine();
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
