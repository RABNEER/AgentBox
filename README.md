<div align="center">

# ⚡ AgentBox Mail

### *The Sovereign Autonomous Mailbox & 2ms OTP Engine for AI Agents*

[![npm version](https://img.shields.io/npm/v/agentbox-mail.svg?style=for-the-badge&color=000000&labelColor=18181b)](https://www.npmjs.com/package/agentbox-mail)
[![npm downloads](https://img.shields.io/npm/dt/agentbox-mail.svg?style=for-the-badge&color=000000&labelColor=18181b)](https://www.npmjs.com/package/agentbox-mail)
[![GitHub Release](https://img.shields.io/github/v/release/RABNEER/AgentBox?style=for-the-badge&color=000000&labelColor=18181b)](https://github.com/RABNEER/AgentBox/releases/latest)
[![Rust](https://img.shields.io/badge/Engine-Rust_1.75+-000000?style=for-the-badge&logo=rust&logoColor=white&labelColor=18181b)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-Compliant-000000?style=for-the-badge&logo=anthropic&logoColor=white&labelColor=18181b)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-000000?style=for-the-badge&labelColor=18181b)](https://opensource.org/licenses/MIT)

<br/>

**AgentBox** gives autonomous AI coding agents (**Claude Code**, **Cursor**, **Antigravity**, **OpenAI Swarm**) their own sovereign email inbox. Receive emails, capture 2FA verification codes in **~2ms**, extract activation magic links, and dispatch outbound replies with **zero third-party cloud lock-in**.

<br/>

[Quick Start](#-quick-start) • [Features](#-key-features) • [MCP Tools](#-mcp-tools-reference) • [Architecture](#-architecture) • [Setup Wizard](#-in-dashboard-setup-wizard) • [Documentation](#-configuration)

---

</div>

<br/>

## 💡 Why AgentBox?

When autonomous AI agents build software, register accounts on developer platforms, or run automated QA pipelines, they inevitably hit **Email Verification & 2FA Gates**.

| Problem with Traditional Approaches | The AgentBox Solution |
|---|---|
| ❌ Paid SaaS email APIs charge per-email and require credit cards | ✅ **100% Free & Self-Hosted** on local SQLite (`agentbox.db`) |
| ❌ Webhook services require public URLs / tunneling (Ngrok) | ✅ **Built-in IMAP TLS Poller & Raw SMTP Server** (Hostinger, Titan, Google, Stalwart) |
| ❌ High latency (polling APIs takes 5–30 seconds) | ✅ **Blazing Fast Ingestion (<2ms)** with regex OTP extraction |
| ❌ Manual MCP setup requiring complex JSON edits in IDE configs | ✅ **`npx agentbox-mail init`** 1-click auto-configures Claude Code, Cursor & Antigravity |

<br/>

---

## 🌟 Key Features

```
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                               ⚡ CORE CAPABILITIES                                     │
├─────────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                         │
│  🔑 Instant 2ms OTP Extraction      │  🧙 In-Dashboard Setup Wizard                     │
│  Automatically isolates 4–8 digit   │  Self-serve UI presets for Hostinger, Titan,      │
│  verification codes from any email. │  Google Workspace, and Stalwart Docker Hub.       │
│                                     │                                                   │
│  ⏱️ `wait_for_email` Blocking Hook  │  🖥️ Native Electron Desktop App                   │
│  Pauses agents and wakes them up in │  Frameless dark window, system tray 24/7 poller,  │
│  ~2ms the instant an OTP arrives.   │  and native Windows OS desktop notifications.     │
│                                     │                                                   │
│  🔌 Model Context Protocol (MCP)    │  📦 Headless NPM CLI (`npx agentbox-mail`)        │
│  7 production tools over stdio for  │  Run anywhere in terminal or CI/CD pipelines with │
│  creating, reading, and sending.    │  zero build prerequisites.                        │
│                                     │                                                   │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

<br/>

---

## 🚀 Quick Start

### 1. Headless NPM CLI (Zero Setup)

Instantly auto-configure your AI tools in 1 second:

```bash
# 1-Click Auto-Install MCP Server & AI Skill into Claude Code, Cursor, Antigravity
npx agentbox-mail init

# Start MCP stdio server
npx agentbox-mail mcp

# Retrieve the latest OTP code for an address
npx agentbox-mail otp agent@yourdomain.com

# Launch the visual web dashboard
npx agentbox-mail ui
```

---

### 2. Native Electron Desktop App

For a complete standalone desktop experience with system tray and OS notifications:

```bash
# Clone the repository
git clone https://github.com/RABNEER/AgentBox.git
cd AgentBox

# Install dependencies and start Desktop App
npm install
npm run app
```

---

### 3. High-Speed Rust Binary

For high-throughput developer environments:

```bash
# Build the optimized production binary
cargo build --release

# Start all-in-one daemon (HTTP Port 3000 + SMTP Port 2525)
./target/release/agentbox-mail server --port 3000
```

<br/>

---

## 🏗️ Architecture

```
                                  ┌───────────────────────────┐
                                  │   Incoming Mail Sources   │
                                  └─────────────┬─────────────┘
                                                │
                 ┌──────────────────────────────┼──────────────────────────────┐
                 │                              │                              │
                 ▼                              ▼                              ▼
     ┌───────────────────────┐      ┌───────────────────────┐      ┌───────────────────────┐
     │ Hostinger / Titan /   │      │ Raw SMTP Listener     │      │ Inbound HTTP Webhook  │
     │ Google IMAP TLS (993) │      │ (0.0.0.0:2525)        │      │ (POST /v1/inbound)    │
     └───────────┬───────────┘      └───────────┬───────────┘      └───────────┬───────────┘
                 │                              │                              │
                 └──────────────────────────────┼──────────────────────────────┘
                                                │
                                                ▼
                                 ┌─────────────────────────────┐
                                 │   High-Speed Regex Parser   │
                                 │  • 4–8 Digit OTP Isolator   │
                                 │  • Magic Action Link Parser │
                                 └──────────────┬──────────────┘
                                                │
                                                ▼
                                 ┌─────────────────────────────┐
                                 │ Embedded SQLite Storage Engine │
                                 │       (`agentbox.db`)       │
                                 └──────────────┬──────────────┘
                                                │
                 ┌──────────────────────────────┼──────────────────────────────┐
                 │                              │                              │
                 ▼                              ▼                              ▼
     ┌───────────────────────┐      ┌───────────────────────┐      ┌───────────────────────┐
     │ Realtime SSE Bus      │      │ MCP Server (stdio)    │      │ Native Desktop App /  │
     │ (`GET /v1/events`)    │      │ Claude, Cursor, Agy   │      │ Web Dashboard (:3000) │
     └───────────────────────┘      └───────────────────────┘      └───────────────────────┘
```

<br/>

---

## 🛠️ MCP Tools Reference

AgentBox implements the **Model Context Protocol (MCP)** specification over `stdio`:

| Tool | Input Schema | Description |
|---|---|---|
| **`create_agent_inbox`** | `name: string` | Creates a new virtual or aliased mailbox address in SQLite. |
| **`get_latest_otp`** | `account_id: string` | Extracts the newest 4–8 digit verification code in under **2ms**. |
| **`wait_for_email`** | `account_id: string, timeout_secs?: number` | **Blocking hook**: Sleeps and resumes immediately when an email/OTP arrives. |
| **`get_verification_link`** | `account_id: string` | Returns all parsed activation, confirmation, and magic login URLs. |
| **`read_agent_inbox`** | `account_id: string` | Retrieves all messages, full body text, HTML, and sender metadata. |
| **`send_agent_email`** | `account_id, to, subject, body` | Dispatches outbound emails and replies through your SMTP relay. |
| **`delete_agent_inbox`** | `account_id: string` | Deletes a temporary or disposable agent mailbox and purges messages. |

<br/>

---

## 🧙 In-Dashboard Setup Wizard

AgentBox features a self-serve onboarding wizard directly inside the browser UI:

1. **Step 1: Mailbox Choice**
   * *"Do you have an existing Mailbox or Domain?"* ➔ Choose **Yes** (Hostinger, Titan, Google Workspace) or **No** (Local Stalwart Mail Server).
2. **Step 2A: Provider Presets & Live TLS Test**
   * 1-Click presets auto-fill server hosts and ports.
   * **`⚡ Test Connection Live`** button performs real-time TLS handshakes and authentication diagnostics.
3. **Step 2B: Stalwart Mail Server Docker Hub**
   * 1-Click Docker run command snippet, Docker status detection, and link to Stalwart Admin UI (`http://localhost:8080`).
4. **Step 3: Agent Identity & 1-Click MCP Auto-Installer**
   * Configures primary agent address and auto-registers MCP across all installed AI IDEs.

<br/>

---

## ⚙️ Configuration

AgentBox is configured via a standard `.env` file or directly through the UI Setup Wizard:

```env
# Domain & Primary Agent Mailbox
DOMAIN=apocalypto.in
PRIMARY_AGENT_EMAIL=agent@apocalypto.in
AGENT_NAME=agent

# Local Ports & Database
SMTP_PORT=2525
HTTP_PORT=3000
DATABASE_URL=sqlite://agentbox.db?mode=rwc

# Upstream Mailbox Sync (Hostinger / Titan / Google Workspace)
IMAP_HOST=imap.hostinger.com
IMAP_PORT=993
IMAP_USER=hello@apocalypto.in
IMAP_PASS=your_secure_password_here

# Outbound SMTP Relay
SMTP_HOST=smtp.hostinger.com
SMTP_PORT=587
SMTP_USER=hello@apocalypto.in
SMTP_PASS=your_secure_password_here
```

<br/>

---

## 🧠 Official AI Agent Skill

AgentBox comes bundled with an **Agent Skill specification** (`skills/agentbox/SKILL.md`). When installed into your assistant (Claude Code, Antigravity, Cursor), the agent automatically knows:
* When and how to generate disposable mailboxes.
* How to use `wait_for_email` for instantaneous 2FA bypass during automated account registrations.
* How to click verification links and handle confirmation flows autonomously.

<br/>

---

## 📄 License

Distributed under the **MIT License**. See [`LICENSE`](LICENSE) for more information.

<div align="center">

**Built with 🖤 by [RABNEER](https://github.com/RABNEER) & The AgentBox Open Source Community**

</div>
