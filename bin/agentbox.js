#!/usr/bin/env node

const { spawn } = require('child_process');
const path = require('path');
const fs = require('fs');
const os = require('os');

const ROOT_DIR = path.resolve(__dirname, '..');
const isWindows = process.platform === 'win32';
const BINARY_NAME = isWindows ? 'agentbox-mail.exe' : 'agentbox-mail';

// Locate binary: Release first, then Debug, or cargo run
function getBinaryPath() {
  const releasePath = path.join(ROOT_DIR, 'target', 'release', BINARY_NAME);
  if (fs.existsSync(releasePath)) return releasePath;

  const debugPath = path.join(ROOT_DIR, 'target', 'debug', BINARY_NAME);
  if (fs.existsSync(debugPath)) return debugPath;

  return null;
}

const args = process.argv.slice(2);
const command = args[0] || 'help';

switch (command) {
  case 'mcp':
    runMcp();
    break;
  case 'init':
  case 'install':
    runInit();
    break;
  case 'ui':
  case 'run':
  case 'start':
    runServer();
    break;
  case 'otp':
    runOtp(args[1]);
    break;
  case 'list':
    runList();
    break;
  case 'create':
    runCreate(args[1], args[2]);
    break;
  case 'help':
  default:
    printHelp();
    break;
}

function runMcp() {
  const bin = getBinaryPath();
  if (!bin) {
    console.error('❌ AgentBox binary not compiled. Please run "cargo build" first.');
    process.exit(1);
  }

  const child = spawn(bin, ['mcp'], {
    stdio: ['inherit', 'inherit', 'inherit'],
    cwd: ROOT_DIR,
    env: process.env
  });

  child.on('exit', (code) => process.exit(code || 0));
}

function runServer() {
  const bin = getBinaryPath();
  if (!bin) {
    console.error('❌ AgentBox binary not compiled. Building now with cargo...');
    const build = spawn('cargo', ['build'], { cwd: ROOT_DIR, stdio: 'inherit' });
    build.on('exit', (code) => {
      if (code === 0) runServer();
      else process.exit(code || 1);
    });
    return;
  }

  console.log('\n⚡ Starting AgentBox Mail Engine...');
  const child = spawn(bin, ['server', '--port', '3000'], {
    stdio: 'inherit',
    cwd: ROOT_DIR,
    env: process.env
  });

  // Open browser after 1.5s
  setTimeout(() => {
    const openCmd = isWindows ? 'start' : (process.platform === 'darwin' ? 'open' : 'xdg-open');
    spawn(openCmd, ['http://localhost:3000'], { shell: true });
  }, 1500);

  child.on('exit', (code) => process.exit(code || 0));
}

function runInit() {
  console.log('\n====================================================================');
  console.log('   ⚡ AgentBox — 1-Click MCP & AI Skill Auto-Installer');
  console.log('====================================================================\n');

  const bin = getBinaryPath() || path.join(ROOT_DIR, 'target', 'debug', BINARY_NAME);
  const homeDir = os.homedir();
  let installedCount = 0;

  // 1. Claude Code (~/.claude.json)
  const claudeConfigPath = path.join(homeDir, '.claude.json');
  try {
    let cfg = fs.existsSync(claudeConfigPath) ? JSON.parse(fs.readFileSync(claudeConfigPath, 'utf8')) : {};
    cfg.mcpServers = cfg.mcpServers || {};
    cfg.mcpServers.agentbox = {
      command: bin,
      args: ['mcp']
    };
    fs.writeFileSync(claudeConfigPath, JSON.stringify(cfg, null, 2));
    console.log('  [✓] Configured MCP in Claude Code (~/.claude.json)');
    installedCount++;
  } catch (e) {
    console.log('  [!] Claude Code config skipped: ' + e.message);
  }

  // 2. Cursor (.cursor/mcp.json)
  const cursorDir = path.join(process.cwd(), '.cursor');
  try {
    if (!fs.existsSync(cursorDir)) fs.mkdirSync(cursorDir, { recursive: true });
    const cursorFile = path.join(cursorDir, 'mcp.json');
    let cfg = fs.existsSync(cursorFile) ? JSON.parse(fs.readFileSync(cursorFile, 'utf8')) : {};
    cfg.mcpServers = cfg.mcpServers || {};
    cfg.mcpServers.agentbox = {
      command: bin,
      args: ['mcp']
    };
    fs.writeFileSync(cursorFile, JSON.stringify(cfg, null, 2));
    console.log('  [✓] Configured MCP in Cursor (.cursor/mcp.json)');
    installedCount++;
  } catch (e) {
    console.log('  [!] Cursor config skipped: ' + e.message);
  }

  // 3. Antigravity IDE (mcp_config.json)
  const antigravityConfigPath = path.join(homeDir, '.gemini', 'config', 'mcp_config.json');
  try {
    if (fs.existsSync(path.dirname(antigravityConfigPath))) {
      let cfg = fs.existsSync(antigravityConfigPath) ? JSON.parse(fs.readFileSync(antigravityConfigPath, 'utf8')) : {};
      cfg.mcpServers = cfg.mcpServers || {};
      cfg.mcpServers.agentbox = {
        command: bin,
        args: ['mcp']
      };
      fs.writeFileSync(antigravityConfigPath, JSON.stringify(cfg, null, 2));
      console.log('  [✓] Configured MCP in Antigravity IDE (mcp_config.json)');
      installedCount++;
    }
  } catch (e) {
    console.log('  [!] Antigravity config skipped: ' + e.message);
  }

  // 4. Install Skill into global skills directory
  const globalSkillDir = path.join(homeDir, '.gemini', 'config', 'skills', 'agentbox');
  const sourceSkill = path.join(ROOT_DIR, 'skills', 'agentbox', 'SKILL.md');
  try {
    if (fs.existsSync(sourceSkill)) {
      if (!fs.existsSync(globalSkillDir)) fs.mkdirSync(globalSkillDir, { recursive: true });
      fs.copyFileSync(sourceSkill, path.join(globalSkillDir, 'SKILL.md'));
      console.log('  [✓] Installed AgentBox AI Skill (agentbox/SKILL.md)');
    }
  } catch (e) {
    console.log('  [!] Skill install skipped: ' + e.message);
  }

  console.log('\n🎉 Done! AgentBox MCP Server is now registered and ready in your AI tools.\n');
}

function runOtp(accountId) {
  const bin = getBinaryPath();
  if (!bin) {
    console.error('❌ AgentBox binary not compiled. Please run "cargo build" first.');
    process.exit(1);
  }

  const child = spawn(bin, ['otp', '--account', accountId || 'agent'], {
    stdio: 'inherit',
    cwd: ROOT_DIR,
    env: process.env
  });
  child.on('exit', (code) => process.exit(code || 0));
}

function runList() {
  const bin = getBinaryPath();
  if (!bin) {
    console.error('❌ AgentBox binary not compiled. Please run "cargo build" first.');
    process.exit(1);
  }

  const child = spawn(bin, ['list'], {
    stdio: 'inherit',
    cwd: ROOT_DIR,
    env: process.env
  });
  child.on('exit', (code) => process.exit(code || 0));
}

function runCreate(name, address) {
  const bin = getBinaryPath();
  if (!bin) {
    console.error('❌ AgentBox binary not compiled. Please run "cargo build" first.');
    process.exit(1);
  }

  const childArgs = ['create', '--name', name || 'agent'];
  if (address) childArgs.push('--address', address);

  const child = spawn(bin, childArgs, {
    stdio: 'inherit',
    cwd: ROOT_DIR,
    env: process.env
  });
  child.on('exit', (code) => process.exit(code || 0));
}

function printHelp() {
  console.log(`
⚡ AgentBox CLI — Autonomous AI Mailbox Engine (v1.0.0)

Usage:
  npx agentbox-mail <command> [options]
  agentbox <command> [options]

Commands:
  mcp             Start the stdio Model Context Protocol (MCP) server for AI agents
  init, install   1-Click auto-configure MCP & Skill into Claude Code, Cursor, Antigravity
  ui, start       Start the web dashboard & IMAP/SMTP engine at http://localhost:3000
  otp [account]   Retrieve the latest OTP verification code
  list            List all active agent mailboxes
  create <name>   Create a new virtual agent mailbox
  help            Show this help message
`);
}
