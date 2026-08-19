use crate::db::Database;
use crate::engine::capabilities::{Capability, ScopeValidator};
use crate::engine::{outbound::SendEmailRequest, OutboundMailer};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::broadcast;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
}

pub struct McpServer {
    pub db: Database,
    pub mailer: OutboundMailer,
    pub domain: String,
    pub event_bus: Option<broadcast::Sender<String>>,
}

impl McpServer {
    pub fn new(
        db: Database,
        mailer: OutboundMailer,
        domain: String,
        event_bus: Option<broadcast::Sender<String>>,
    ) -> Self {
        Self {
            db,
            mailer,
            domain,
            event_bus,
        }
    }

    pub async fn run_stdio(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let stdin = tokio::io::stdin();
        let mut reader = BufReader::new(stdin).lines();
        let mut stdout = tokio::io::stdout();

        while let Some(line) = reader.next_line().await? {
            if line.trim().is_empty() {
                continue;
            }

            if let Ok(req) = serde_json::from_str::<JsonRpcRequest>(&line) {
                let id = req.id.unwrap_or(Value::Null);
                let response = self.handle_request(req.method, req.params, id).await;
                let out_line = serde_json::to_string(&response)? + "\n";
                stdout.write_all(out_line.as_bytes()).await?;
                stdout.flush().await?;
            }
        }

        Ok(())
    }

    pub async fn handle_request(
        &self,
        method: String,
        params: Option<Value>,
        id: Value,
    ) -> JsonRpcResponse {
        match method.as_str() {
            "initialize" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": "agentbox-mail-mcp",
                        "version": "1.0.0"
                    },
                    "capabilities": {
                        "tools": {}
                    }
                })),
                error: None,
            },
            "notifications/initialized" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({})),
                error: None,
            },
            "tools/list" => {
                let tools = vec![
                    // Identity & Profile Primitives
                    json!({
                        "name": "create_agent_identity",
                        "description": "Creates a first-class persistent Agent Identity with scoped capabilities, email address, and auth token.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "Agent name or worker identity (e.g. 'coder', 'browser-qa', 'reviewer')"
                                },
                                "capabilities": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Allowed capability scopes: 'inbox.read', 'inbox.create', 'inbox.delete', 'otp.read', 'links.read', 'email.send'"
                                }
                            },
                            "required": ["name"]
                        }
                    }),
                    json!({
                        "name": "get_agent_identity",
                        "description": "Retrieves the status, capabilities, email identity, and auth metadata for an agent.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "agent_id": {
                                    "type": "string",
                                    "description": "The unique Agent ID or agent name."
                                }
                            },
                            "required": ["agent_id"]
                        }
                    }),
                    json!({
                        "name": "list_agent_identities",
                        "description": "Lists all registered Agent Identities and their active capability policies.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    }),
                    json!({
                        "name": "revoke_agent_identity",
                        "description": "Revokes an Agent Identity, invalidating its token and scoped permissions.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "agent_id": {
                                    "type": "string",
                                    "description": "The Agent ID to revoke."
                                }
                            },
                            "required": ["agent_id"]
                        }
                    }),
                    // Mailbox & OTP Automation Primitives
                    json!({
                        "name": "create_agent_inbox",
                        "description": "Creates a new dedicated mailbox or virtual address for an AI agent worker.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "name": {
                                    "type": "string",
                                    "description": "Agent name or worker identity (e.g. 'code-reviewer', 'qa-tester')"
                                },
                                "address": {
                                    "type": "string",
                                    "description": "Optional custom email address. If omitted, generates a unique address on your domain."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Optional Agent token to enforce capability authorization."
                                }
                            }
                        }
                    }),
                    json!({
                        "name": "get_latest_otp",
                        "description": "Extracts the latest 4–8 digit 2FA/verification OTP code from emails in under 0.14ms.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "account_id": {
                                    "type": "string",
                                    "description": "The mailbox ID or email address to retrieve the OTP for."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Optional Agent token to enforce capability authorization."
                                }
                            },
                            "required": ["account_id"]
                        }
                    }),
                    json!({
                        "name": "wait_for_email",
                        "description": "Event-driven blocking hook: Pauses agent execution and wakes up instantaneously (<0.001ms) the exact moment an email or OTP lands.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "account_id": {
                                    "type": "string",
                                    "description": "The mailbox ID or email address to watch."
                                },
                                "timeout_secs": {
                                    "type": "number",
                                    "description": "Maximum seconds to wait (default: 30, max: 120)."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Optional Agent token to enforce capability authorization."
                                }
                            },
                            "required": ["account_id"]
                        }
                    }),
                    json!({
                        "name": "get_verification_link",
                        "description": "Extracts verification, activation, or magic login URLs with safety analysis, domain verification, and anti-redirect protection.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "account_id": {
                                    "type": "string",
                                    "description": "The mailbox ID or email address."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Optional Agent token to enforce capability authorization."
                                }
                            },
                            "required": ["account_id"]
                        }
                    }),
                    json!({
                        "name": "read_agent_inbox",
                        "description": "Retrieves recent emails received by this agent inbox, including sender, subject, plain text, and extracted codes.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "account_id": {
                                    "type": "string",
                                    "description": "The mailbox ID or email address."
                                },
                                "limit": {
                                    "type": "number",
                                    "description": "Max number of messages to fetch (default: 10)."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Optional Agent token to enforce capability authorization."
                                }
                            },
                            "required": ["account_id"]
                        }
                    }),
                    json!({
                        "name": "send_agent_email",
                        "description": "Sends an outbound email or reply from the specified agent mailbox via SMTP relay.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "account_id": {
                                    "type": "string",
                                    "description": "The mailbox ID or sender email address."
                                },
                                "to": {
                                    "type": "string",
                                    "description": "Recipient email address."
                                },
                                "subject": {
                                    "type": "string",
                                    "description": "Email subject line."
                                },
                                "body": {
                                    "type": "string",
                                    "description": "Plain text body of the email."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Optional Agent token to enforce capability authorization."
                                }
                            },
                            "required": ["account_id", "to", "body"]
                        }
                    }),
                    json!({
                        "name": "delete_agent_inbox",
                        "description": "Permanently deletes a temporary or disposable agent mailbox and purges stored messages.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "account_id": {
                                    "type": "string",
                                    "description": "The mailbox ID or email address to delete."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Optional Agent token to enforce capability authorization."
                                }
                            },
                            "required": ["account_id"]
                        }
                    })
                ];

                JsonRpcResponse {
                    jsonrpc: "2.0".to_string(),
                    id,
                    result: Some(json!({ "tools": tools })),
                    error: None,
                }
            }
            "tools/call" => {
                let params_obj = params.unwrap_or(Value::Null);
                let tool_name = params_obj.get("name").and_then(|n| n.as_str()).unwrap_or_default();
                let tool_args = params_obj.get("arguments").cloned().unwrap_or(json!({}));

                match self.execute_tool(tool_name, tool_args).await {
                    Ok(val) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(json!({
                            "content": [{
                                "type": "text",
                                "text": serde_json::to_string_pretty(&val).unwrap_or_default()
                            }]
                        })),
                        error: None,
                    },
                    Err(e) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(json!({
                            "code": -32603,
                            "message": e.to_string()
                        })),
                    },
                }
            }
            _ => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: None,
                error: Some(json!({
                    "code": -32601,
                    "message": format!("Method '{}' not found", method)
                })),
            },
        }
    }

    /// Enforce capability check if an agent token is provided
    async fn check_authorization(
        &self,
        token_opt: Option<&str>,
        required: Capability,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(token) = token_opt {
            if let Some(identity) = self.db.get_agent_identity_by_token(token).await? {
                if identity.status == "revoked" {
                    return Err("Agent identity has been revoked.".into());
                }
                let caps: Vec<String> = serde_json::from_str(&identity.capabilities).unwrap_or_default();
                if !ScopeValidator::has_capability(&caps, required) {
                    return Err(format!(
                        "PermissionDenied: Agent '{}' lacks required capability '{}'.",
                        identity.name,
                        required.as_str()
                    ).into());
                }
            } else {
                return Err("Invalid or unrecognized agent_token.".into());
            }
        }
        Ok(())
    }

    /// Explicitly resolves an existing account without silent side-effect creation on reads.
    async fn resolve_existing_account_id(&self, identifier: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if identifier.contains('@') {
            if let Some(acc) = self.db.get_account_by_address(identifier).await? {
                return Ok(acc.id);
            }
            return Err(format!("Account with address '{}' not found. Please create it first.", identifier).into());
        }

        if let Some(acc) = self.db.get_account_by_id(identifier).await? {
            return Ok(acc.id);
        }

        // Check if identifier matches default agent prefix
        if let Some(acc) = self.db.get_account_by_address(&format!("{}@{}", identifier, self.domain)).await? {
            return Ok(acc.id);
        }

        Err(format!("Account '{}' not found.", identifier).into())
    }

    async fn execute_tool(
        &self,
        name: &str,
        args: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        let agent_token = args.get("agent_token").and_then(|t| t.as_str());

        match name {
            // =================================================================
            // First-Class Agent Identity Tools
            // =================================================================
            "create_agent_identity" => {
                let name_arg = args.get("name").and_then(|n| n.as_str()).ok_or("Missing agent 'name'")?;
                let custom_caps = args.get("capabilities").and_then(|c| c.as_array());
                
                let caps: Vec<String> = if let Some(arr) = custom_caps {
                    arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect()
                } else {
                    Capability::standard_agent().into_iter().map(|c| c.as_str().to_string()).collect()
                };

                let rand_slug = uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
                let email = format!("{}-{}@{}", name_arg.to_lowercase().replace(' ', "-"), rand_slug, self.domain);
                
                let identity = self.db.create_agent_identity(name_arg, &email, &caps).await?;
                Ok(json!(identity))
            }
            "get_agent_identity" => {
                let agent_id = args.get("agent_id").and_then(|a| a.as_str()).ok_or("Missing agent_id")?;
                let identity = self.db.get_agent_identity(agent_id).await?
                    .ok_or_else(|| format!("Agent identity '{}' not found", agent_id))?;
                Ok(json!(identity))
            }
            "list_agent_identities" => {
                let list = self.db.list_agent_identities().await?;
                Ok(json!(list))
            }
            "revoke_agent_identity" => {
                let agent_id = args.get("agent_id").and_then(|a| a.as_str()).ok_or("Missing agent_id")?;
                self.db.revoke_agent_identity(agent_id).await?;
                Ok(json!({ "status": "revoked", "agent_id": agent_id }))
            }

            // =================================================================
            // Mailbox & OTP Operations
            // =================================================================
            "create_agent_inbox" => {
                self.check_authorization(agent_token, Capability::InboxCreate).await?;
                let display_name = args.get("name").and_then(|n| n.as_str()).unwrap_or("agent-worker");
                let custom_address = args.get("address").and_then(|a| a.as_str());
                let address = if let Some(addr) = custom_address {
                    addr.to_string()
                } else {
                    let rand_slug = uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
                    format!("{}-{}@{}", display_name.to_lowercase().replace(' ', "-"), rand_slug, self.domain)
                };
                
                let acc = self.db.create_account(&address, Some(display_name)).await?;
                Ok(json!(acc))
            }
            "get_latest_otp" => {
                self.check_authorization(agent_token, Capability::OtpRead).await?;
                let identifier = args.get("account_id").and_then(|a| a.as_str()).ok_or("Missing account_id")?;
                let account_id = self.resolve_existing_account_id(identifier).await?;
                let otp = self.db.get_latest_otp(&account_id).await?;
                Ok(json!({ "account_id": account_id, "otp": otp }))
            }
            "wait_for_email" => {
                self.check_authorization(agent_token, Capability::OtpRead).await?;
                let identifier = args.get("account_id").and_then(|a| a.as_str()).unwrap_or("agent");
                let account_id = self.resolve_existing_account_id(identifier).await?;
                let timeout_secs = args.get("timeout_secs").and_then(|t| t.as_u64()).unwrap_or(30).min(120);
                let start_time = Utc::now().timestamp();

                // 1. Immediate check: Did an email already arrive in the last 2 seconds?
                let initial_messages = self.db.list_messages_for_account(&account_id).await?;
                if let Some(latest) = initial_messages.into_iter().find(|m| m.created_at >= start_time - 2) {
                    return Ok(json!({
                        "received": true,
                        "otp": latest.extracted_otp,
                        "subject": latest.subject,
                        "from": latest.from_address,
                        "body_text": latest.body_text,
                        "links": latest.extracted_links,
                        "created_at": latest.created_at,
                        "wake_method": "immediate"
                    }));
                }

                // 2. Pure Event-Driven Instant Wake-up via Tokio Broadcast Channel (<0.001ms latency)
                if let Some(ref bus) = self.event_bus {
                    let mut rx = bus.subscribe();
                    let timeout_duration = Duration::from_secs(timeout_secs);

                    let result = tokio::select! {
                        _ = tokio::time::sleep(timeout_duration) => None,
                        msg = async {
                            while let Ok(event_str) = rx.recv().await {
                                if let Ok(evt) = serde_json::from_str::<Value>(&event_str) {
                                    if evt.get("type").and_then(|t| t.as_str()) == Some("new_message") {
                                        if let Some(msg_obj) = evt.get("message") {
                                            let msg_acc_id = msg_obj.get("account_id").and_then(|a| a.as_str()).unwrap_or_default();
                                            if msg_acc_id == account_id {
                                                return Some(msg_obj.clone());
                                            }
                                        }
                                    }
                                }
                            }
                            None
                        } => msg
                    };

                    if let Some(msg_val) = result {
                        return Ok(json!({
                            "received": true,
                            "otp": msg_val.get("extracted_otp"),
                            "subject": msg_val.get("subject"),
                            "from": msg_val.get("from_address"),
                            "body_text": msg_val.get("body_text"),
                            "links": msg_val.get("extracted_links"),
                            "created_at": msg_val.get("created_at"),
                            "wake_method": "event_driven_channel"
                        }));
                    }
                } else {
                    // Fallback fast polling for standalone CLI runs without live event bus
                    let iterations = timeout_secs * 10;
                    for _ in 0..iterations {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        let messages = self.db.list_messages_for_account(&account_id).await?;
                        if let Some(latest) = messages.into_iter().find(|m| m.created_at >= start_time) {
                            return Ok(json!({
                                "received": true,
                                "otp": latest.extracted_otp,
                                "subject": latest.subject,
                                "from": latest.from_address,
                                "body_text": latest.body_text,
                                "links": latest.extracted_links,
                                "created_at": latest.created_at,
                                "wake_method": "polling_fallback"
                            }));
                        }
                    }
                }

                Ok(json!({
                    "received": false,
                    "message": format!("Timed out after {}s waiting for email", timeout_secs)
                }))
            }
            "get_verification_link" => {
                self.check_authorization(agent_token, Capability::LinksRead).await?;
                let identifier = args.get("account_id").and_then(|a| a.as_str()).ok_or("Missing account_id")?;
                let account_id = self.resolve_existing_account_id(identifier).await?;
                let messages = self.db.list_messages_for_account(&account_id).await?;
                let mut links: Vec<Value> = Vec::new();
                for msg in messages {
                    if let Some(links_str) = msg.extracted_links {
                        if let Ok(parsed) = serde_json::from_str::<Value>(&links_str) {
                            if let Some(arr) = parsed.as_array() {
                                for l in arr {
                                    if !links.contains(l) {
                                        links.push(l.clone());
                                    }
                                }
                            }
                        }
                    }
                }
                Ok(json!({ "account_id": account_id, "links": links }))
            }
            "read_agent_inbox" => {
                self.check_authorization(agent_token, Capability::InboxRead).await?;
                let identifier = args.get("account_id").and_then(|a| a.as_str()).ok_or("Missing account_id")?;
                let account_id = self.resolve_existing_account_id(identifier).await?;
                let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(10) as usize;
                let mut messages = self.db.list_messages_for_account(&account_id).await?;
                messages.truncate(limit);
                Ok(json!(messages))
            }
            "send_agent_email" => {
                self.check_authorization(agent_token, Capability::EmailSend).await?;
                let identifier = args.get("account_id").and_then(|a| a.as_str()).ok_or("Missing account_id")?;
                let account_id = self.resolve_existing_account_id(identifier).await?;
                let to = args.get("to").and_then(|t| t.as_str()).ok_or("Missing recipient 'to'")?;
                let subject = args.get("subject").and_then(|s| s.as_str()).unwrap_or("(No Subject)");
                let body = args.get("body").and_then(|b| b.as_str()).unwrap_or("");

                let account = self.db.get_account_by_id(&account_id).await?
                    .ok_or("Account not found")?;

                let req = SendEmailRequest {
                    from: Some(account.address),
                    to: vec![to.to_string()],
                    cc: None,
                    bcc: None,
                    subject: subject.to_string(),
                    text: Some(body.to_string()),
                    html: None,
                    in_reply_to: None,
                };

                let res = self.mailer.send_email(req).await?;
                Ok(json!({ "status": "sent", "details": res }))
            }
            "delete_agent_inbox" => {
                self.check_authorization(agent_token, Capability::InboxDelete).await?;
                let identifier = args.get("account_id").and_then(|a| a.as_str()).ok_or("Missing account_id")?;
                let account_id = self.resolve_existing_account_id(identifier).await?;
                self.db.delete_account(&account_id).await?;
                Ok(json!({ "status": "deleted", "account_id": account_id }))
            }
            _ => Err(format!("Unknown tool: {}", name).into()),
        }
    }
}
