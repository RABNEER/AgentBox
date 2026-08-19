<div align="center">

# ⚡ AgentBox

### *The Sovereign Autonomous Mailbox & Identity Layer for AI Agents*

[![CI](https://github.com/RABNEER/AgentBox/actions/workflows/ci.yml/badge.svg)](https://github.com/RABNEER/AgentBox/actions)
[![npm version](https://img.shields.io/npm/v/agentbox-mail.svg?style=for-the-badge&color=000000&labelColor=18181b)](https://www.npmjs.com/package/agentbox-mail)
[![npm downloads](https://img.shields.io/npm/dt/agentbox-mail.svg?style=for-the-badge&color=000000&labelColor=18181b)](https://www.npmjs.com/package/agentbox-mail)
[![GitHub Release](https://img.shields.io/github/v/release/RABNEER/AgentBox?style=for-the-badge&color=000000&labelColor=18181b)](https://github.com/RABNEER/AgentBox/releases/latest)
[![Rust](https://img.shields.io/badge/Engine-Rust_1.75+-000000?style=for-the-badge&logo=rust&logoColor=white&labelColor=18181b)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-Compliant-000000?style=for-the-badge&logo=anthropic&logoColor=white&labelColor=18181b)](https://modelcontextprotocol.io/)
[![License: MIT](https://img.shields.io/badge/License-MIT-000000?style=for-the-badge&labelColor=18181b)](https://opensource.org/licenses/MIT)

<br/>

**AgentBox** gives autonomous AI coding agents (**Claude Code**, **Cursor**, **Antigravity**, **OpenAI Swarm**) persistent machine-native email identities, object-level authorization, and sovereign communication infrastructure. Receive emails, capture 2FA verification codes in **<0.14ms**, verify activation magic links with anti-phishing protection, and dispatch outbound replies with **zero third-party cloud lock-in**.

<br/>

[Quick Start](#-quick-start) • [Agent Identity & Security](#-first-class-agent-identity--object-level-security) • [Benchmarks](#-reproducible-performance-benchmarks) • [MCP Tools](#-mcp-tools-reference) • [Link Safety](#-link-safety--anti-phishing-engine) • [Architecture](#-architecture)

---

</div>

<br/>

## 💡 Why AgentBox?

When autonomous AI agents build software, register accounts on developer platforms, or run automated QA pipelines, they inevitably hit **Email Verification, 2FA, and Identity Gates**.

| Problem with Traditional Approaches | The AgentBox Sovereign Solution |
|---|---|
| ❌ Paid SaaS email APIs charge per-email and require credit cards | ✅ **100% Free & Self-Hosted** on local SQLite (`agentbox.db`) |
| ❌ Webhook services require public URLs / tunneling (Ngrok) | ✅ **Built-in IMAP TLS Poller & Raw Inbound SMTP Server** (Hostinger, Titan, Google, Stalwart) |
| ❌ Polling REST APIs takes 5–30 seconds with rate limit bottlenecks | ✅ **Event-Driven Async Wake-up (<0.001ms)** via Tokio broadcast channels |
| ❌ Cross-agent data leaks with unauthenticated tools | ✅ **Mandatory Scoped Capabilities & Object-Level Resource Ownership** |
| ❌ Agents lack security and fall for phishing / open-redirect links | ✅ **Deep URL Safety Engine** (Punycode, Raw IP & Open-Redirect Defense) |
| ❌ Manual MCP setup requiring complex JSON edits in IDE configs | ✅ **`npx agentbox-mail init`** 1-click auto-configures Claude Code, Cursor & Antigravity |

<br/>

---

## 🧑‍🚀 First-Class Agent Identity & Object-Level Security

AgentBox moves beyond generic mailboxes by introducing **First-Class Agent Identities** with strict object-level resource ownership:

```bash
# Provision a scoped identity for an autonomous browser QA agent
npx agentbox-mail agent create browser-qa --capabilities "inbox.read,otp.read,links.read"
```

```
╔══════════════════════════════════════════════════════════════════╗
║             🧑‍🚀 AGENT IDENTITY PROVISIONED                      ║
╠══════════════════════════════════════════════════════════════════╣
║  Agent ID     : agent_browser-qa_7f92a1                          ║
║  Name         : browser-qa                                       ║
║  Email        : browser-qa-7f92a1@apocalypto.in                  ║
║  Auth Token   : agb_92d7e8f1c3a04b12                             ║
║  Capabilities : ["inbox.read", "otp.read", "links.read"]         ║
║  Status       : active                                           ║
╚══════════════════════════════════════════════════════════════════╝
⚠️  NOTE: Store this auth_token securely. It is only displayed once upon creation and cannot be retrieved again.
```

### 🔐 Multi-Tier Security Enforcement:

```
                Incoming Tool / API Request
                            │
                            ▼
              ┌───────────────────────────┐
              │ 1. Validate Auth Token    │ ➔ Reject if invalid or revoked
              └─────────────┬─────────────┘
                            │
                            ▼
              ┌───────────────────────────┐
              │ 2. Check Capability Scope │ ➔ E.g. Require "otp.read"
              └─────────────┬─────────────┘
                            │
                            ▼
              ┌───────────────────────────┐
              │ 3. Object-Level Ownership │ ➔ Agent A CANNOT read Agent B's mailbox
              └─────────────┬─────────────┘
                            │
                            ▼
              ┌───────────────────────────┐
              │ 4. Execute Protected Tool │
              └───────────────────────────┘
```

* **Credential Hygiene**: Tokens are displayed **only once** upon creation. Read endpoints (`get_agent_identity`, `list_agent_identities`) use safe public structs that never leak authentication secrets.
* **Cross-Agent Isolation**: An agent possessing `otp.read` is strictly restricted to mailboxes it owns (`owner_agent_id`). Attempting cross-mailbox access returns a structured `AccessDenied` error.

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
| **Average (Mean)** | **`681.3 µs`** (0.681 ms) | **1,468 complete MCP cycles/sec** |
| **p50 Median** | **`590.0 µs`** (0.590 ms) | — |
| **p95** | **`1.20 ms`** | — |
| **p99** | **`1.58 ms`** | — |

### ⚡ Sub-Component Microsecond Latencies (10,000 Iterations):
* **Event Bus Channel Dispatch**: `0.216 µs` (0.0002 ms) — **4.62 Million events/sec**
* **Link Safety & Anti-Redirect**: `0.652 µs` (0.0007 ms) — **1.53 Million checks/sec**
* **OTP Regex Extraction**: `138.2 µs` (0.138 ms) — **7,230 extractions/sec**

> *Note: External email arrival latency depends on upstream mail delivery; once bytes hit AgentBox (SMTP/IMAP/HTTP), end-to-end parsing, DB persistence, capability authorization, and JSON-RPC response completes in **<0.7ms**.*

<br/>

---

## 🛡️ Link Safety & Anti-Phishing Engine

To protect autonomous agents from credential harvesting and malicious open redirects, AgentBox parses all inbound links through a deep safety analyzer:

* 🚫 **Open-Redirect Detection**: Inspects parameters like `?redirect=`, `?url=`, `?next=`, `?dest=`, `?to=`.
* 🚫 **Raw IP Address Defense**: Blocks URLs targeting raw IPv4 addresses instead of reputable hostnames.
* 🚫 **Punycode Homograph Defense**: Flags Unicode/Punycode domain spoofing (`xn--`).
* 🔒 **Protocol Validation**: Distinguishes secure HTTPS endpoints from insecure HTTP.

```json
{
  "url": "https://signin.aws.amazon.com/verify?token=abc_123",
  "domain": "signin.aws.amazon.com",
  "is_safe": true,
  "has_open_redirect": false,
  "confidence": 0.98
}
```

<br/>

---

## 🛠️ MCP Tools Reference

AgentBox implements the **Model Context Protocol (MCP)** specification over `stdio`:

| Category | Tool | Parameters | Description |
|---|---|---|---|
| **Identity** | **`create_agent_identity`** | `name, capabilities?` | Creates a persistent identity and returns a one-time secret auth token. |
| **Identity** | **`get_agent_identity`** | `agent_id` | Retrieves public agent capabilities, status, and metadata (token sanitized). |
| **Identity** | **`list_agent_identities`** | — | Lists all registered public agent identities. |
| **Identity** | **`revoke_agent_identity`** | `agent_id` | Revokes an agent identity and invalidates its auth token immediately. |
| **Mailbox** | **`create_agent_inbox`** | `name, address?, agent_token?` | Creates a new virtual mailbox linked to the calling agent identity. |
| **Mailbox** | **`get_latest_otp`** | `account_id, agent_token?` | Extracts the newest 4–8 digit verification code with ownership validation. |
| **Mailbox** | **`wait_for_email`** | `account_id, timeout_secs?, agent_token?` | **Event-Driven Hook**: Async Tokio broadcast channel wakes the agent in **<0.001ms**. |
| **Mailbox** | **`get_verification_link`** | `account_id, agent_token?` | Returns parsed activation links with **Link Safety & Anti-Redirect Analysis**. |
| **Mailbox** | **`read_agent_inbox`** | `account_id, limit?, agent_token?` | Retrieves recent messages, full body text, HTML, and sender metadata. |
| **Mailbox** | **`send_agent_email`** | `account_id, to, subject, body, agent_token?` | Dispatches outbound emails via SMTP relay with capability check. |
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
npx agentbox-mail agent create coder --capabilities "inbox.read,otp.read,links.read"

# Retrieve latest OTP code
npx agentbox-mail otp agent@yourdomain.com

# Launch Web Dashboard
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
                                 │  • Link Safety Engine       │
                                 └──────────────┬──────────────┘
                                                │
                                                ▼
                                 ┌─────────────────────────────┐
                                 │ Embedded SQLite Storage     │
                                 │       (`agentbox.db`)       │
                                 │  • Accounts  • Identities   │
                                 │  • Messages  • Capabilities │
                                 │  • Resource Ownership Graph │
                                 └──────────────┬──────────────┘
                                                │
                 ┌──────────────────────────────┼──────────────────────────────┐
                 │                              │                              │
                 ▼                              ▼                              ▼
     ┌───────────────────────┐      ┌───────────────────────┐      ┌───────────────────────┐
     │ Realtime SSE Bus      │      │ MCP Server (stdio)    │      │ Native Desktop App /  │
     │ (`GET /v1/events`)    │      │ Scoped Capabilities   │      │ Web Dashboard (:3000) │
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
