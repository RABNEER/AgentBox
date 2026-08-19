<div align="center">

# ⚡ AgentBox

### *The Sovereign Autonomous Communication & Task Orchestration Layer for AI Agents*

[![CI](https://github.com/RABNEER/AgentBox/actions/workflows/ci.yml/badge.svg)](https://github.com/RABNEER/AgentBox/actions)
[![npm version](https://img.shields.io/npm/v/agentbox-mail.svg?style=for-the-badge&color=000000&labelColor=18181b)](https://www.npmjs.com/package/agentbox-mail)
[![npm downloads](https://img.shields.io/npm/dt/agentbox-mail.svg?style=for-the-badge&color=000000&labelColor=18181b)](https://www.npmjs.com/package/agentbox-mail)
[![GitHub Release](https://img.shields.io/github/v/release/RABNEER/AgentBox?style=for-the-badge&color=000000&labelColor=18181b)](https://github.com/RABNEER/AgentBox/releases/latest)
[![Rust](https://img.shields.io/badge/Engine-Rust_1.75+-000000?style=for-the-badge&logo=rust&logoColor=white&labelColor=18181b)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-Compliant-000000?style=for-the-badge&logo=anthropic&logoColor=white&labelColor=18181b)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-000000?style=for-the-badge&labelColor=18181b)](https://opensource.org/licenses/MIT)

<br/>

**AgentBox** is the sovereign control plane and work protocol for autonomous AI agents (**Claude Code**, **Cursor**, **Antigravity**, **OpenAI Swarm**). It provides machine-native email identities, object-level authorization, instant event-driven task dispatch, and immutable audit trails — enabling one AI agent to delegate work to another agent without human intervention or third-party cloud lock-in.

<br/>

[Quick Start](#-quick-start) • [Task Protocol](#-agent-task-protocol--work-orchestration) • [Agent Identity & Security](#-first-class-agent-identity--object-level-security) • [Benchmarks](#-reproducible-performance-benchmarks) • [MCP Tools](#-mcp-tools-reference) • [Architecture](#-architecture)

---

</div>

<br/>

## 💡 Why AgentBox?

Autonomous AI coding agents need two fundamental capabilities to operate independently:
1. **Machine Identity & Authentication**: The ability to receive 2FA/OTPs and verify magic links on developer platforms without humans.
2. **Inter-Agent Work Delegation**: The ability for a QA agent (e.g. Jules) to discover a bug, dispatch a structured work order to a Coding Agent, track repository progress, and verify the resulting Pull Request.

| Problem with Traditional Approaches | The AgentBox Sovereign Solution |
|---|---|
| ❌ Agents work in isolation with no structured way to delegate tasks | ✅ **Agent Task Protocol**: Structured work orders, atomic claiming & immutable audit lineage |
| ❌ Polling REST APIs burns tokens and introduces 5–30s latency | ✅ **Sub-Millisecond Tokio Event Bus (<0.001ms)** for instant agent wake-up |
| ❌ Cross-agent data leaks and unauthorized actions | ✅ **Multi-Tier Security**: Scoped Capability Matrix + Object-Level Mailbox Ownership |
| ❌ Complex cloud infrastructure requiring Webhooks/Ngrok | ✅ **Self-Hosted Rust Daemon**: Embedded SQLite (`agentbox.db`), Raw SMTP & IMAP TLS |
| ❌ Manual MCP setup requiring JSON edits in IDE configs | ✅ **`npx agentbox-mail init`**: 1-click auto-configures Claude Code, Cursor & Antigravity |

<br/>

---

## 📋 Agent Task Protocol & Work Orchestration

AgentBox implements a stateful work protocol allowing agents to dispatch, claim, track, and complete software engineering tasks:

```
             Jules (QA Agent)
                    │
                    │ 1. dispatch_agent_task (action, repo, evidence, criteria)
                    ▼
           ┌─────────────────┐
           │    AgentBox     │ ──► Status: "received"
           └────────┬────────┘
                    │ 2. Realtime Event Bus Dispatch (<0.001ms)
                    ▼
           Coder (Worker Agent)
                    │ 3. claim_agent_task
                    │ 4. update_task_progress (status: "running", commit: "a91f4b")
                    │ 5. Opens GitHub PR & runs CI tests
                    │ 6. complete_agent_task (pr_url, test_results, summary)
                    ▼
           ┌─────────────────┐
           │    AgentBox     │ ──► Status: "completed" + Immutable Audit Lineage
           └────────┬────────┘
                    │ 7. Notifies Jules / User
                    ▼
             Jules (QA Agent) closes ticket
```

### 🔍 Immutable Task Audit Lineage

Every state change, git commit hash, PR link, and test output is permanently recorded in SQLite (`task_audit_logs`):

```json
[
  { "event_type": "task.created", "agent_id": "agent_jules_8a12", "created_at": 1771485600 },
  { "event_type": "task.claimed", "agent_id": "agent_coder_7f92", "created_at": 1771485601 },
  { "event_type": "task.pr_opened", "details": { "commit_sha": "a91f4b23", "pr_url": "https://github.com/RABNEER/EstateFlow/pull/42" } },
  { "event_type": "task.completed", "details": { "summary": "Fixed duplicate listings. 143 tests passed." } }
]
```

<br/>

---

## 🧑‍🚀 First-Class Agent Identity & Object-Level Security

AgentBox provisions persistent identities with fine-grained capability scopes and object-level resource isolation:

```bash
# Provision a scoped identity for an autonomous coding agent
npx agentbox-mail agent create coder --capabilities "task.claim,task.update,inbox.read,otp.read"
```

```
╔══════════════════════════════════════════════════════════════════╗
║             🧑‍🚀 AGENT IDENTITY PROVISIONED                      ║
╠══════════════════════════════════════════════════════════════════╣
║  Agent ID     : agent_coder_7f92a1                               ║
║  Name         : coder                                            ║
║  Email        : coder-7f92a1@apocalypto.in                       ║
║  Auth Token   : agb_92d7e8f1c3a04b12                             ║
║  Capabilities : ["task.claim", "task.update", "inbox.read"]      ║
║  Status       : active                                           ║
╚══════════════════════════════════════════════════════════════════╝
⚠️  NOTE: Store this auth_token securely. It is only displayed once upon creation and cannot be retrieved again.
```

### 🔐 Multi-Tier Security Enforcement:
1. **Token Authentication**: Verifies agent identity and status (`active` vs `revoked`).
2. **Capability Check**: Validates required scope (e.g. `task.dispatch`, `otp.read`, `email.send`).
3. **Object-Level Resource Ownership**: Agent A possessing `otp.read` is strictly restricted to its own assigned mailboxes (`owner_agent_id`). Attempting cross-agent access returns a structured `AccessDenied` error.
4. **Credential Hygiene**: Public queries (`get_agent_identity`, `list_agent_identities`) use sanitized structs that never expose tokens.

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
| **Average (Mean)** | **`492.2 µs`** (0.492 ms) | **2,032 complete MCP cycles/sec** |
| **p50 Median** | **`453.9 µs`** (0.453 ms) | — |
| **p95** | **`783.1 µs`** (0.783 ms) | — |
| **p99** | **`1.18 ms`** | — |

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
| **Task Protocol** | **`dispatch_agent_task`** | `action, description, repository?, branch?, priority?, target_agent?, evidence?, acceptance_criteria?, agent_token?` | Dispatches a structured work order from one agent to another. |
| **Task Protocol** | **`claim_agent_task`** | `task_id, agent_token` | Atomically locks and assigns a task to the claiming worker agent. |
| **Task Protocol** | **`update_task_progress`** | `task_id, status, commit_sha?, pr_url?, test_results?, note?, agent_token` | Updates task status (`running`, `testing`, `pr_opened`) and records audit log. |
| **Task Protocol** | **`complete_agent_task`** | `task_id, summary, commit_sha?, pr_url?, test_results?, agent_token` | Closes a task with completion details and emits completion event. |
| **Task Protocol** | **`list_agent_tasks`** | `status?, agent_token?, limit?` | Lists tasks filtered by lifecycle state or agent identity. |
| **Task Protocol** | **`get_task_audit_trail`** | `task_id, agent_token?` | Retrieves the immutable audit log and lifecycle history for a task. |
| **Identity** | **`create_agent_identity`** | `name, capabilities?` | Creates a persistent identity and returns a one-time secret auth token. |
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

# Create an Agent Identity with scoped capabilities
npx agentbox-mail agent create coder --capabilities "task.claim,task.update,inbox.read,otp.read"

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
                                  │   Incoming Mail & Tasks   │
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
                                 │  • Link Safety Engine       │
                                 └──────────────┬──────────────┘
                                                │
                                                ▼
                                 ┌─────────────────────────────┐
                                 │ Embedded SQLite Storage     │
                                 │       (`agentbox.db`)       │
                                 │  • Accounts  • Identities   │
                                 │  • Messages  • Tasks        │
                                 │  • Resource Ownership Graph │
                                 │  • Task Audit Logs          │
                                 └──────────────┬──────────────┘
                                                │
                 ┌──────────────────────────────┼──────────────────────────────┐
                 │                              │                              │
                 ▼                              ▼                              ▼
     ┌───────────────────────┐      ┌───────────────────────┐      ┌───────────────────────┐
     │ Realtime SSE Bus      │      │ MCP Server (stdio)    │      │ Native Desktop App /  │
     │ (`GET /v1/events`)    │      │ Task Protocol & State │      │ Web Dashboard (:3000) │
     │                       │      │ Object-Level Auth     │      │                       │
     └───────────────────────┘      └───────────────────────┘      └───────────────────────┘
```

<br/>

---

## 📄 License

Distributed under the **MIT License**. See [`LICENSE`](LICENSE) for more information.

<div align="center">

**Built with 🖤 by [RABNEER](https://github.com/RABNEER) & The AgentBox Open Source Community**

</div>
