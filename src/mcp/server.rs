use crate::db::Database;
use crate::engine::{outbound::SendEmailRequest, OutboundMailer};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

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
}

impl McpServer {
    pub fn new(db: Database, mailer: OutboundMailer, domain: String) -> Self {
        Self { db, mailer, domain }
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
                        "version": "0.2.0"
                    },
                    "capabilities": {
                        "tools": {}
                    }
                })),
                error: None,
            },
            "tools/list" => JsonRpcResponse {
                jsonrpc: "2.0".to_string(),
                id,
                result: Some(json!({
                    "tools": [
                        {
                            "name": "create_agent_inbox",
                            "description": "Create a new virtual email inbox for the AI agent (e.g. for signups, verification flows, API keys).",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "name": {
                                        "type": "string",
                                        "description": "Descriptive name for the agent (e.g. github-signup-bot)"
                                    },
                                    "address": {
                                        "type": "string",
                                        "description": "Optional custom address (e.g. agent@apocalypto.in)"
                                    }
                                }
                            }
                        },
                        {
                            "name": "get_latest_otp",
                            "description": "Get the most recent 4-8 digit OTP verification code sent to an agent's inbox.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "account_id": {
                                        "type": "string",
                                        "description": "The account ID of the inbox or email address"
                                    }
                                },
                                "required": ["account_id"]
                            }
                        },
                        {
                            "name": "wait_for_email",
                            "description": "Asynchronously wait for a new incoming email or OTP code to arrive. Blocks until received or timeout.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "account_id": {
                                        "type": "string",
                                        "description": "The account ID or email address to watch"
                                    },
                                    "timeout_secs": {
                                        "type": "integer",
                                        "description": "Maximum seconds to wait (default: 30, max: 120)"
                                    }
                                }
                            }
                        },
                        {
                            "name": "get_verification_link",
                            "description": "Get the latest verification, confirmation, or magic login URLs received in the agent's inbox.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "account_id": {
                                        "type": "string",
                                        "description": "The account ID of the inbox or email address"
                                    }
                                },
                                "required": ["account_id"]
                            }
                        },
                        {
                            "name": "read_agent_inbox",
                            "description": "Read all received email messages, parsed text, subject lines, and sender info for an agent inbox.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "account_id": {
                                        "type": "string",
                                        "description": "The account ID of the inbox or email address"
                                    }
                                },
                                "required": ["account_id"]
                            }
                        },
                        {
                            "name": "send_agent_email",
                            "description": "Send an outbound email from the agent's inbox address.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "account_id": {
                                        "type": "string",
                                        "description": "The account ID or email address to send from"
                                    },
                                    "to": {
                                        "type": "string",
                                        "description": "Recipient email address"
                                    },
                                    "subject": {
                                        "type": "string",
                                        "description": "Email subject"
                                    },
                                    "body": {
                                        "type": "string",
                                        "description": "Plain text body"
                                    }
                                },
                                "required": ["account_id", "to", "subject", "body"]
                            }
                        },
                        {
                            "name": "delete_agent_inbox",
                            "description": "Delete a virtual agent inbox and all its associated messages.",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "account_id": {
                                        "type": "string",
                                        "description": "The account ID or email address of the inbox to delete"
                                    }
                                },
                                "required": ["account_id"]
                            }
                        }
                    ]
                })),
                error: None,
            },
            "tools/call" => {
                let call_params = params.unwrap_or(Value::Null);
                let tool_name = call_params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let args = call_params.get("arguments").cloned().unwrap_or(json!({}));

                let result = self.execute_tool(tool_name, args).await;
                match result {
                    Ok(val) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: Some(json!({
                            "content": [
                                {
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&val).unwrap_or_default()
                                }
                            ]
                        })),
                        error: None,
                    },
                    Err(e) => JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id,
                        result: None,
                        error: Some(json!({
                            "code": -32000,
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
                    "message": "Method not found"
                })),
            },
        }
    }

    async fn resolve_account_id(&self, identifier: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if identifier.contains('@') {
            if let Some(acc) = self.db.get_account_by_address(identifier).await? {
                return Ok(acc.id);
            }
            let new_acc = self.db.create_account(identifier, Some("AI Agent Inbox")).await?;
            return Ok(new_acc.id);
        }
        Ok(identifier.to_string())
    }

    async fn execute_tool(
        &self,
        name: &str,
        args: Value,
    ) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
        match name {
            "create_agent_inbox" => {
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
                let identifier = args.get("account_id").and_then(|a| a.as_str()).ok_or("Missing account_id")?;
                let account_id = self.resolve_account_id(identifier).await?;
                let otp = self.db.get_latest_otp(&account_id).await?;
                Ok(json!({ "account_id": account_id, "otp": otp }))
            }
            "wait_for_email" => {
                let identifier = args.get("account_id").and_then(|a| a.as_str()).unwrap_or("agent");
                let account_id = self.resolve_account_id(identifier).await?;
                let timeout_secs = args.get("timeout_secs").and_then(|t| t.as_u64()).unwrap_or(30).min(120);
                let start_time = Utc::now().timestamp();

                // Poll every 500ms
                for _ in 0..(timeout_secs * 2) {
                    let messages = self.db.list_messages_for_account(&account_id).await?;
                    if let Some(latest) = messages.into_iter().find(|m| m.created_at >= start_time - 5) {
                        return Ok(json!({
                            "received": true,
                            "otp": latest.extracted_otp,
                            "subject": latest.subject,
                            "from": latest.from_address,
                            "body_text": latest.body_text,
                            "links": latest.extracted_links,
                            "created_at": latest.created_at
                        }));
                    }
                    tokio::time::sleep(Duration::from_millis(500)).await;
                }

                Ok(json!({
                    "received": false,
                    "message": format!("Timed out after {}s waiting for email", timeout_secs)
                }))
            }
            "get_verification_link" => {
                let identifier = args.get("account_id").and_then(|a| a.as_str()).ok_or("Missing account_id")?;
                let account_id = self.resolve_account_id(identifier).await?;
                let messages = self.db.list_messages_for_account(&account_id).await?;
                let mut links = Vec::new();
                for msg in messages {
                    if let Some(links_str) = msg.extracted_links {
                        if let Ok(parsed) = serde_json::from_str::<Vec<String>>(&links_str) {
                            for l in parsed {
                                if !links.contains(&l) {
                                    links.push(l);
                                }
                            }
                        }
                    }
                }
                Ok(json!({ "account_id": account_id, "links": links }))
            }
            "read_agent_inbox" => {
                let identifier = args.get("account_id").and_then(|a| a.as_str()).ok_or("Missing account_id")?;
                let account_id = self.resolve_account_id(identifier).await?;
                let messages = self.db.list_messages_for_account(&account_id).await?;
                Ok(json!(messages))
            }
            "send_agent_email" => {
                let identifier = args.get("account_id").and_then(|a| a.as_str()).ok_or("Missing account_id")?;
                let account_id = self.resolve_account_id(identifier).await?;
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
                let identifier = args.get("account_id").and_then(|a| a.as_str()).ok_or("Missing account_id")?;
                let account_id = self.resolve_account_id(identifier).await?;
                self.db.delete_account(&account_id).await?;
                Ok(json!({ "status": "deleted", "account_id": account_id }))
            }
            _ => Err(format!("Unknown tool: {}", name).into()),
        }
    }
}
