# ⚡ AgentBox 2.0 — Autonomous Mailbox & Instant 2ms OTP Engine for AI Agents

[![npm version](https://img.shields.io/npm/v/agentbox-mail.svg?style=flat-square&color=black)](https://www.npmjs.com/package/agentbox-mail)
[![GitHub Release](https://img.shields.io/github/v/release/RABNEER/AgentBox?style=flat-square&color=black)](https://github.com/RABNEER/AgentBox/releases/latest)
[![License: MIT](https://img.shields.io/badge/License-MIT-black.svg?style=flat-square)](https://opensource.org/licenses/MIT)

> **The Sovereign Email Layer for AI Agents** (Claude Code, Cursor, Antigravity, OpenAI Swarm). Receive emails, capture 2FA OTP codes in ~2ms, click verification links, and dispatch outbound replies with zero third-party cloud lock-in.

---

## 🌟 What's New in AgentBox 2.0

* **📦 Headless NPM Package (`npx agentbox-mail`)**: Live on NPM! Run anywhere with `npx agentbox-mail init`, `mcp`, `otp`, `wait`, `ui`.
* **🧙 In-Dashboard Setup Wizard**: Self-serve provider onboarding with 1-click presets for **Hostinger Mail**, **Titan Mail**, **Google Workspace**, or **Stalwart Mail Server Hub**.
* **⚡ Live TLS Connection Tester**: Test IMAP and SMTP credentials directly in the UI before saving.
* **🧠 Official AI Agent Skill (`agentbox`)**: Auto-installs into your AI coding assistants so they understand how to manage inboxes and capture OTPs autonomously.
* **⏱️ `wait_for_email` Blocking Hook**: Agents can pause and wake up in ~2ms the exact instant an email or OTP lands.
* **🗑️ 1-Click Inbox Deletion**: Clean up temporary or disposable test inboxes from the sidebar or via the `delete_agent_inbox` MCP tool.

---

## 🚀 Quick Start

### Option 1: Headless NPM CLI
```bash
# 1-Click Auto-Install MCP Server & Skill into Claude Code, Cursor, Antigravity
npx agentbox init

# Start MCP stdio server
npx agentbox mcp

# Capture latest OTP for an address
npx agentbox otp agent@yourdomain.com

# Launch Web Dashboard
npx agentbox ui
```

### Option 2: Native Electron Desktop App
```bash
# Clone the repository
git clone https://github.com/RABNEER/AgentBox.git
cd AgentBox

# Install dependencies and start Desktop App
npm install
npm run app
```

### Option 3: High-Speed Rust Binary
```bash
# Build and start the high-speed server daemon
cargo build --release
./target/release/agentbox-mail server --port 3000
```

---

## 🛠️ MCP Tools (Model Context Protocol)

AgentBox exposes 6 native MCP tools for AI agents over `stdio`:

| Tool | Parameters | Description |
|---|---|---|
| `create_agent_inbox` | `name` | Creates a new virtual or aliased agent inbox in SQLite. |
| `get_latest_otp` | `account_id` | Extracts the newest 4–8 digit 2FA verification code in under 2ms. |
| `wait_for_email` | `account_id`, `timeout_secs` | Blocks and wakes the agent the millisecond an email/OTP arrives. |
| `get_verification_link` | `account_id` | Returns parsed magic links and activation URLs. |
| `read_agent_inbox` | `account_id` | Retrieves full email metadata, text, HTML, and sender details. |
| `send_agent_email` | `account_id`, `to`, `subject`, `body` | Dispatches outbound emails and replies via SMTP relay. |
| `delete_agent_inbox` | `account_id` | Deletes a virtual inbox and purges its stored messages. |

---

## 🏗️ Architecture

```
┌────────────────────────────────────────────────────────────────────────┐
│                        AGENTBOX 2.0 ECOSYSTEM                          │
├────────────────────────────────────────────────────────────────────────┤
│                                                                        │
│  [ Inbound Ingestion Engines ]                                         │
│  • Hostinger / Titan / Google TLS Poller (IMAP 993)                    │
│  • Raw Inbound SMTP Daemon (0.0.0.0:2525)                              │
│  • REST API Ingest Webhook (POST /v1/inbound)                          │
│                                                                        │
│  [ Core Processing & Storage ]                                         │
│  • High-Speed Regex OTP & Magic Link Isolator (<2ms)                   │
│  • Local Embedded SQLite Database (`agentbox.db`)                      │
│  • Realtime Server-Sent Events (SSE) Bus (`/v1/events`)                │
│                                                                        │
│  [ Agent & User Interfaces ]                                           │
│  • Native Electron Desktop App (System Tray + Native OS Toasts)        │
│  • Luxury Monochrome Web Dashboard (`http://localhost:3000`)           │
│  • Model Context Protocol (MCP) Stdio Server                           │
│  • Headless CLI Wrapper (`bin/agentbox.js`)                            │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 📄 License

MIT © [RABNEER](https://github.com/RABNEER) & The AgentBox Contributors.
