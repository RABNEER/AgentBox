<div align="center">

# ⚡ AgentBox

### *The Email & Identity Layer for AI Agents*

[![CI](https://github.com/RABNEER/AgentBox/actions/workflows/ci.yml/badge.svg)](https://github.com/RABNEER/AgentBox/actions)
[![npm version](https://img.shields.io/npm/v/agentbox-mail.svg?style=for-the-badge&color=000000&labelColor=18181b)](https://www.npmjs.com/package/agentbox-mail)
[![npm downloads](https://img.shields.io/npm/dt/agentbox-mail.svg?style=for-the-badge&color=000000&labelColor=18181b)](https://www.npmjs.com/package/agentbox-mail)
[![GitHub Release](https://img.shields.io/github/v/release/RABNEER/AgentBox?style=for-the-badge&color=000000&labelColor=18181b)](https://github.com/RABNEER/AgentBox/releases/latest)
[![Rust](https://img.shields.io/badge/Engine-Rust_1.75+-000000?style=for-the-badge&logo=rust&logoColor=white&labelColor=18181b)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-Compliant-000000?style=for-the-badge&logo=anthropic&logoColor=white&labelColor=18181b)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-000000?style=for-the-badge&labelColor=18181b)](https://opensource.org/licenses/MIT)

<br/>

**AgentBox** gives any autonomous AI agent (**Claude Code**, **Cursor**, **Antigravity**, **OpenAI Swarm**) its own machine-native email identity, inbox, outbound communication, authentication, and event-driven email capabilities — self-hosted, sovereign, and blazingly fast.

<br/>

[Quick Start](#-quick-start) • [Core Abstraction](#-core-abstraction) • [Agent Identity & Security](#-agent-identity--security-model) • [Use Cases](#-versatile-use-cases) • [Benchmarks](#-reproducible-performance-benchmarks) • [MCP Tools](#-mcp-tools-reference) • [Architecture](#-architecture)

---

</div>

<br/>

## 💡 The Core Problem

Autonomous AI agents need a way to interact with the human world and each other. Today, email is the universal communication protocol across all software and platforms:

* How does a browser agent verify its account on GitHub or AWS? **Email.**
* How does a customer contact your AI support assistant? **Email.**
* How does an external QA agent delegate a bug report to a coding agent? **Email.**
* How does a research agent receive arXiv digests and industry alerts? **Email.**

Without machine-native email infrastructure, developers are forced to use brittle API polling, hack personal Gmail inboxes, or manually click verification links.

### **AgentBox solves this entirely.**

```
                    ┌─────────────────────────┐
                    │        AGENTBOX         │
                    └────────────┬────────────┘
                                 │
              ┌──────────────────┴──────────────────┐
              ▼                                     ▼
        🧑🚀 IDENTITY                         📬 COMMUNICATION
  • User-Defined Name & Email           • Inbound Inbox (SMTP/IMAP/HTTP)
  • Persistent Agent ID                 • Outbound SMTP Relay
  • Scoped Capability Matrix            • Realtime Event Bus (<0.001ms)
  • Object-Level Ownership              • OTP Isolator & SafeLink Engine
              │                                     │
              └──────────────────┬──────────────────┘
                                 │
                                 ▼
                     Autonomous AI Agent
```

<br/>

---

## 🧑‍🚀 Agent Identity & Security Model

AgentBox does not prescribe who your agent is. **You define the agent's name, email, and capability policy:**

```bash
# 1. Create a Support Agent with a custom company email
npx agentbox-mail agent create support \
  --email support@mycompany.com \
  --capabilities "inbox.read,email.send"

# 2. Create an Autonomous Coding Agent
npx agentbox-mail agent create coder \
  --email coder@mycompany.com \
  --capabilities "inbox.read,task.claim,task.update,otp.read"

# 3. Create a Browser QA Agent with standard verification permissions
npx agentbox-mail agent create browser-qa \
  --capabilities "inbox.read,otp.read,links.read"
```

```
╔══════════════════════════════════════════════════════════════════╗
║             🧑‍🚀 AGENT IDENTITY PROVISIONED                      ║
╠══════════════════════════════════════════════════════════════════╣
║  Agent ID     : agent_coder_7f92a1                               ║
║  Name         : coder                                            ║
║  Email        : coder@mycompany.com                              ║
║  Auth Token   : agb_92d7e8f1c3a04b12                             ║
║  Capabilities : ["inbox.read", "task.claim", "otp.read"]        ║
║  Status       : active                                           ║
╚══════════════════════════════════════════════════════════════════╝
⚠️  NOTE: Store this auth_token securely. It is only displayed once upon creation.
```

### 🔐 Multi-Tier Security Enforcement:
1. **Token Authentication**: Verifies agent identity and status (`active` vs `revoked`).
2. **Capability Scopes**: Validates required permissions (`inbox.read`, `email.send`, `otp.read`, `task.claim`).
3. **Object-Level Mailbox Ownership**: Agent A possessing `otp.read` is strictly restricted to its own assigned mailboxes (`owner_agent_id`). Attempting cross-agent access returns an explicit `AccessDenied` error.
4. **Credential Hygiene**: Public queries (`get_agent_identity`, `list_agent_identities`) use sanitized structs that never expose tokens.

<br/>

---

## 🌐 Versatile Use Cases

AgentBox provides the foundational email identity layer. Here are some of the most powerful workflows built on top of it:

### 1. 🤖 Agent-to-Agent Work Delegation & Task Protocols
An external QA or discovery agent (like Jules) sends an email with a bug or feature request. AgentBox's built-in `TaskDetector` automatically parses the subject (`[TASK:BUG]`), extracts the repository, branch, priority, and line citations, provisions an `AgentTask`, and wakes the Coding Agent via the event bus:

```
   Jules (QA Agent)
          │
          │ 1. Sends email: "[TASK:BUG] Fix duplicate property filter in EstateFlow"
          │    Body: "Repository: RABNEER/EstateFlow\nPriority: high\nEvidence: tests/search.spec.ts:87"
          ▼
 ┌─────────────────┐
 │    AgentBox     │ ──► Auto-detects Work Order via `TaskDetector`
 └────────┬────────┘ ──► Provisions `AgentTask` & records audit event
          │
          │ 2. Realtime Event Bus Dispatch (<0.001ms) / SSE Daemon Bridge
          ▼
 Coder (Worker Agent / Claude Code)
          │ 3. Instantaneously claims task via `claim_agent_task`
          │ 4. Fixes code, opens GitHub PR, calls `update_task_progress`
          │ 5. Calls `complete_agent_task` with CI results
          ▼
 ┌─────────────────┐
 │    AgentBox     │ ──► Status: "completed" + Immutable Audit Lineage
 └────────┬────────┘
          │ 6. Emits completion notification to Jules / User
          ▼
   Jules closes ticket
```

---

### 2. 🔐 Autonomous SaaS Signups & 2FA / OTP Verification
Browser agents (Puppeteer, Playwright, Stagehand) need to sign up for tools, verify email addresses, and solve OTP challenges:
* Agent creates inbox `create_agent_inbox(name: "signup-bot")`.
* Triggers signup on platform (e.g. AWS, Stripe, Vercel).
* Calls `get_latest_otp()` (extracted via regex in **<0.14ms**) or `get_verification_link()` (checked with **Anti-Redirect & Phishing Defense**).
* Account is verified autonomously with zero human intervention.

---

### 3. 💬 Autonomous Inbound Support & Customer Triage
Give your customer support agent its own email address (`support@yourcompany.com`):
* Customer emails support with an issue.
* AgentBox ingests the email via raw SMTP or IMAP sync.
* Realtime SSE event notifies the support agent.
* Agent analyzes the inquiry, consults internal docs, and replies via `send_agent_email()`.

---

### 4. 🔬 Research & Intelligence Gathering
Give your research agent an identity (`researcher@yourcompany.com`):
* Subscribes to industry newsletters, security advisories (CVEs), and arXiv digest feeds.
* Agent reads inbound emails periodically using `read_agent_inbox()`.
* Synthesizes executive briefings, summarizes findings, and forwards digests to your team.

---

### 5. 🛡️ DevOps Alerting & Automated Incident Response
Give your incident response agent an identity (`oncall@yourcompany.com`):
* Receives critical error alerts from Datadog, Sentry, or PagerDuty.
* Realtime event hook wakes the agent immediately.
* Agent queries logs, identifies the failing commit, and dispatches a fix order to the coding agent.

<br/>

---

## 📊 Reproducible Performance Benchmarks

AgentBox includes a complete benchmark test suite (`tests/benchmark.rs`) measuring the entire pipeline from raw bytes to full JSON-RPC output:

```bash
cargo test --release --test benchmark -- --nocapture
```

### ⚡ Verified Full End-to-End MCP Pipeline (1,000 Cycles):

Tested Pipeline: `Raw MIME Ingestion ➔ mail-parser ➔ SafeLink Analysis ➔ Regex OTP ➔ SQLite INSERT ➔ Broadcast Dispatch ➔ Authenticated MCP Tool Call (tools/call) ➔ JSON-RPC Result Output`

| Pipeline Metric | Measured Latency | Throughput |
|---|---|---|
| **Average (Mean)** | **`451.9 µs`** (0.451 ms) | **2,213 complete MCP cycles/sec** |
| **p50 Median** | **`431.5 µs`** (0.431 ms) | — |
| **p95** | **`586.2 µs`** (0.586 ms) | — |
| **p99** | **`1.04 ms`** | — |

### ⚡ Sub-Component Microsecond Latencies (10,000 Iterations):
* **Event Bus Channel Dispatch**: `0.216 µs` (0.0002 ms) — **4.62 Million events/sec**
* **Link Safety & Anti-Redirect**: `0.652 µs` (0.0007 ms) — **1.53 Million checks/sec**
* **OTP Regex Extraction**: `138.2 µs` (0.138 ms) — **7,230 extractions/sec**

<br/>

---

## 🛠️ MCP Tools Reference

AgentBox implements the **Model Context Protocol (MCP)** specification over `stdio`:

| Category | Tool | Parameters | Description |
|---|---|---|---|
| **Identity** | **`create_agent_identity`** | `name, email?, capabilities?` | Creates a persistent identity with custom/auto email and returns a one-time auth token. |
| **Identity** | **`get_agent_identity`** | `agent_id` | Retrieves public agent metadata (tokens are sanitized). |
| **Identity** | **`list_agent_identities`** | — | Lists all registered public agent identities and active policies. |
| **Identity** | **`revoke_agent_identity`** | `agent_id` | Revokes an agent identity and invalidates its auth token immediately. |
| **Mailbox** | **`create_agent_inbox`** | `name, address?, agent_token?` | Creates a new virtual mailbox linked to the calling agent identity. |
| **Mailbox** | **`get_latest_otp`** | `account_id, agent_token?` | Extracts the newest 4–8 digit verification code in **<0.14ms** with ownership check. |
| **Mailbox** | **`wait_for_email`** | `account_id, timeout_secs?, agent_token?` | **Event-Driven Hook**: Async Tokio broadcast channel wakes the agent in **<0.001ms**. |
| **Mailbox** | **`get_verification_link`** | `account_id, agent_token?` | Returns parsed activation links with **Deep Link Safety & Anti-Redirect Defense**. |
| **Mailbox** | **`read_agent_inbox`** | `account_id, limit?, agent_token?` | Retrieves recent messages, full body text, HTML, and sender metadata. |
| **Mailbox** | **`send_agent_email`** | `account_id, to, subject, body, agent_token?` | Dispatches outbound emails via SMTP relay with capability authorization. |
| **Mailbox** | **`delete_agent_inbox`** | `account_id, agent_token?` | Deletes a temporary mailbox and purges stored messages. |
| **Task Protocol** | **`dispatch_agent_task`** | `action, description, repository?, branch?, priority?, target_agent?, evidence?, acceptance_criteria?, agent_token?` | Dispatches a structured work order from one agent to another. |
| **Task Protocol** | **`claim_agent_task`** | `task_id, agent_token` | Atomically locks and assigns a task to the claiming worker agent. |
| **Task Protocol** | **`update_task_progress`** | `task_id, status, commit_sha?, pr_url?, test_results?, note?, agent_token` | Updates task status (`running`, `testing`, `pr_opened`) and records audit log. |
| **Task Protocol** | **`complete_agent_task`** | `task_id, summary, commit_sha?, pr_url?, test_results?, agent_token` | Closes a task with completion details and emits completion event. |
| **Task Protocol** | **`list_agent_tasks`** | `status?, agent_token?, limit?` | Lists tasks filtered by lifecycle state or agent identity. |
| **Task Protocol** | **`get_task_audit_trail`** | `task_id, agent_token?` | Retrieves the immutable audit log and lifecycle history for a task. |

<br/>

---

## 🚀 Quick Start

### 1. Headless NPM CLI (Zero Setup)

Instantly auto-configure your AI tools in 1 second:

```bash
# 1-Click Auto-Install MCP Server & AI Skill into Claude Code, Cursor, Antigravity
npx agentbox-mail init

# Start MCP stdio server with live daemon SSE event bridge
npx agentbox-mail mcp

# Create an Agent Identity with scoped capabilities
npx agentbox-mail agent create support --email support@mycompany.com --capabilities "inbox.read,email.send"

# Retrieve latest OTP code
npx agentbox-mail otp agent@yourdomain.com

# Launch Web Dashboard
npx agentbox-mail ui
```

---

### 2. Native Electron Desktop App

```bash
# Clone the repository
git clone https://github.com/RABNEER/AgentBox.git
cd AgentBox

# Install dependencies and start Desktop App
npm install
npm run app
```

---

### 3. High-Speed Rust Core Daemon

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
                                  │   Inbound Emails & Tasks  │
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
                                 │   High-Speed Parser Engine  │
                                 │  • 4–8 Digit OTP Isolator   │
                                 │  • Link Safety Engine       │
                                 │  • TaskDetector (Work Order)│
                                 └──────────────┬──────────────┘
                                                │
                                                ▼
                                 ┌─────────────────────────────┐
                                 │ Embedded SQLite Storage     │
                                 │       (`agentbox.db`)       │
                                 │  • Identities & Auth Tokens │
                                 │  • Mailboxes & Messages     │
                                 │  • Resource Ownership Graph │
                                 │  • Agent Tasks & Audit Logs │
                                 └──────────────┬──────────────┘
                                                │
                 ┌──────────────────────────────┼──────────────────────────────┐
                 │                              │                              │
                 ▼                              ▼                              ▼
     ┌───────────────────────┐      ┌───────────────────────┐      ┌───────────────────────┐
     │ Realtime SSE Bus      │      │ MCP Server (stdio)    │      │ Native Desktop App /  │
     │ (`GET /v1/events`)    │      │ Full Tool Interface   │      │ Web Dashboard (:3000) │
     │ (Live Daemon Bridge)  │      │ Object-Level Auth     │      │                       │
     └───────────────────────┘      └───────────────────────┘      └───────────────────────┘
```

<br/>

---

## 📄 License

Distributed under the **MIT License**. See [`LICENSE`](LICENSE) for more information.

<div align="center">

**Built with 🖤 by [RABNEER](https://github.com/RABNEER) & The AgentBox Open Source Community**

</div>
