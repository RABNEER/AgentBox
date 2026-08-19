---
name: agentbox
description: "Master autonomous email engine for AI agents: create disposable/persistent inboxes, capture OTPs and verification links in 2ms, send outbound emails, wait for incoming triggers, and automate 2FA signups."
version: "2.0.0"
author: "AgentBox Team"
---

# AgentBox AI Mailbox Skill

AgentBox is an **all-in-one autonomous email engine** built specifically for AI agents, developers, and autonomous workflows. It provides zero-latency OTP extraction, magic link detection, and full IMAP/SMTP mail capabilities.

---

## ⚡ Key Capabilities for AI Agents

1. **Instant 2FA & OTP Extraction**:
   - Extract 4 to 8-digit verification codes (Google, GitHub, AWS, Stripe, Slack, Discord) in **~2ms**.
2. **Autonomous Signup & Onboarding Flows**:
   - Create isolated virtual inboxes on demand (`create_agent_inbox`) to sign up on third-party services.
3. **`wait_for_email` Blocking Hook**:
   - Pause execution and wake up the millisecond an email arrives.
4. **Outbound SMTP Delivery**:
   - Send emails and replies directly from your agent's verified domain.
5. **100% Offline & Local**:
   - Works with Hostinger, Titan, Google Workspace, or local Stalwart Mail Server via SQLite.

---

## 🛠️ MCP Tools Reference

When connected to the `agentbox` MCP server, you have access to the following tools:

### 1. `wait_for_email` (Recommended for 2FA / Verification)
Asynchronously blocks until a new email arrives for the specified account address:
```json
{
  "account_id": "agent@local.agentbox",
  "timeout_secs": 60
}
```
**Returns**:
```json
{
  "received": true,
  "otp": "849201",
  "subject": "Your GitHub verification code",
  "from": "security@github.com",
  "links": ["https://github.com/verify?token=..."]
}
```

### 2. `get_latest_otp`
Retrieve the most recent OTP code sent to an inbox:
```json
{
  "account_id": "agent@local.agentbox"
}
```
**Returns**:
```json
{
  "account_id": "agent@local.agentbox",
  "otp": "592819"
}
```

### 3. `create_agent_inbox`
Create a new virtual email inbox for the agent:
```json
{
  "name": "research-bot",
  "address": "research-bot@local.agentbox"
}
```

### 4. `read_agent_inbox`
List all incoming and outgoing messages with full body text, HTML, and timestamps:
```json
{
  "account_id": "agent@local.agentbox"
}
```

### 5. `send_agent_email`
Send an outbound email:
```json
{
  "account_id": "agent@local.agentbox",
  "to": "user@example.com",
  "subject": "Task Completed: Fixed auth.rs line 42",
  "body": "All unit tests passed. Deployment complete."
}
```

---

## 🤖 Standard Agent Workflow Examples

### Example A: Completing a User Signup & 2FA Flow
```
1. Call `create_agent_inbox` (or use primary `agent@local.agentbox`).
2. Input the email into the website's registration form.
3. Call `wait_for_email(account_id="agent@local.agentbox", timeout_secs=45)`.
4. Extract `otp` from the response.
5. Enter the OTP into the website and complete verification!
```

### Example B: Email-to-Agent Command Dispatch
```
1. Agent monitors `agent@local.agentbox` via `wait_for_email`.
2. User emails: "Fix bug in calculation logic".
3. Agent reads instructions, modifies code, runs tests.
4. Agent calls `send_agent_email` to reply back with confirmation.
```

---

## ⚙️ Configuration Cheatsheet

AgentBox configuration is managed in `.env` or the **Monochrome UI Dashboard** at `http://localhost:3000`:

* **Hostinger Email**:
  * `IMAP_HOST=imap.hostinger.com` (Port 993)
  * `SMTP_HOST=smtp.hostinger.com` (Port 587)
* **Titan Business Email**:
  * `IMAP_HOST=imap.titan.email` (Port 993)
  * `SMTP_HOST=smtp.titan.email` (Port 587)
* **Stalwart Local Server (Docker)**:
  * `IMAP_HOST=localhost` (Port 993)
  * `SMTP_HOST=localhost` (Port 587)
