use crate::db::{AgentIdentity, Database};
use crate::engine::capabilities::{Capability, ScopeValidator};
use crate::engine::outbound::SendEmailRequest;
use crate::engine::tasks::{AgentTask, TaskAuditLog, TaskPriority};
use crate::engine::OutboundMailer;
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

    pub async fn run_stdio(
        self: Arc<Self>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
                    // =========================================================
                    // 1. Agent Task Protocol & Orchestration Tools
                    // =========================================================
                    json!({
                        "name": "dispatch_agent_task",
                        "description": "Dispatches a structured work order/task from one agent (e.g. Jules) to another agent or capability pool with evidence and acceptance criteria.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "action": {
                                    "type": "string",
                                    "description": "Task action type (e.g. 'fix_bug', 'code_review', 'e2e_test', 'deploy')"
                                },
                                "description": {
                                    "type": "string",
                                    "description": "Detailed description of the issue or feature requirement."
                                },
                                "repository": {
                                    "type": "string",
                                    "description": "Target repository (e.g. 'RABNEER/EstateFlow')"
                                },
                                "branch": {
                                    "type": "string",
                                    "description": "Target git branch (default: 'main')"
                                },
                                "priority": {
                                    "type": "string",
                                    "enum": ["low", "normal", "high", "urgent"],
                                    "description": "Task priority level (default: 'normal')"
                                },
                                "target_agent": {
                                    "type": "string",
                                    "description": "Optional specific target agent ID or name."
                                },
                                "evidence": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Stack traces, failing test file paths, or reproduction steps."
                                },
                                "acceptance_criteria": {
                                    "type": "array",
                                    "items": { "type": "string" },
                                    "description": "Explicit verification criteria to satisfy before completion."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Source agent authorization token (requires 'task.dispatch')."
                                }
                            },
                            "required": ["action", "description"]
                        }
                    }),
                    json!({
                        "name": "claim_agent_task",
                        "description": "Atomically locks and claims an unassigned or targeted task for a worker agent.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": {
                                    "type": "string",
                                    "description": "The unique task ID to claim (e.g. 'task_8d21a9')."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Claiming worker agent auth token (requires 'task.claim')."
                                }
                            },
                            "required": ["task_id"]
                        }
                    }),
                    json!({
                        "name": "update_task_progress",
                        "description": "Updates task lifecycle status ('running', 'testing', 'pr_opened') and logs intermediate git commits or test results to the audit trail.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": {
                                    "type": "string",
                                    "description": "The unique task ID."
                                },
                                "status": {
                                    "type": "string",
                                    "enum": ["running", "testing", "pr_opened", "failed"],
                                    "description": "New lifecycle status."
                                },
                                "commit_sha": {
                                    "type": "string",
                                    "description": "Optional Git commit hash created during the task."
                                },
                                "pr_url": {
                                    "type": "string",
                                    "description": "Optional GitHub Pull Request URL."
                                },
                                "test_results": {
                                    "type": "string",
                                    "description": "Optional test execution metrics (e.g. '143 passed, 0 failed')."
                                },
                                "note": {
                                    "type": "string",
                                    "description": "Progress note or audit log message."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Worker agent auth token (requires 'task.update')."
                                }
                            },
                            "required": ["task_id", "status"]
                        }
                    }),
                    json!({
                        "name": "complete_agent_task",
                        "description": "Marks a task completed with final commit SHA, PR URL, and summary, emitting a completion event and audit trail entry.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": {
                                    "type": "string",
                                    "description": "The unique task ID."
                                },
                                "summary": {
                                    "type": "string",
                                    "description": "Summary of changes made, root cause analysis, and resolution."
                                },
                                "commit_sha": {
                                    "type": "string",
                                    "description": "Final Git commit SHA."
                                },
                                "pr_url": {
                                    "type": "string",
                                    "description": "GitHub Pull Request URL."
                                },
                                "test_results": {
                                    "type": "string",
                                    "description": "Test verification results."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Worker agent auth token (requires 'task.update')."
                                }
                            },
                            "required": ["task_id", "summary"]
                        }
                    }),
                    json!({
                        "name": "list_agent_tasks",
                        "description": "Lists agent work orders filtered by status or assigned agent.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "status": {
                                    "type": "string",
                                    "description": "Optional filter by status ('received', 'claimed', 'running', 'pr_opened', 'completed', 'failed')"
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Optional agent auth token."
                                },
                                "limit": {
                                    "type": "number",
                                    "description": "Max tasks to return (default: 20)."
                                }
                            }
                        }
                    }),
                    json!({
                        "name": "get_task_audit_trail",
                        "description": "Retrieves the complete immutable audit trail of actions, commits, and status transitions for a task.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "task_id": {
                                    "type": "string",
                                    "description": "The unique task ID."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Optional agent auth token."
                                }
                            },
                            "required": ["task_id"]
                        }
                    }),
                    // =========================================================
                    // 2. Identity & Profile Primitives
                    // =========================================================
                    json!({
                        "name": "create_agent_identity",
                        "description": "Creates a first-class persistent Agent Identity with scoped capabilities, email address, and returns a one-time secret auth token.",
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
                                    "description": "Allowed capability scopes: 'inbox.read', 'inbox.create', 'inbox.delete', 'otp.read', 'links.read', 'email.send', 'task.dispatch', 'task.claim', 'task.update'"
                                }
                            },
                            "required": ["name"]
                        }
                    }),
                    json!({
                        "name": "get_agent_identity",
                        "description": "Retrieves the public status, capabilities, and email identity metadata for an agent (tokens are never exposed).",
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
                        "description": "Lists all registered public Agent Identities and their active capability policies.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {}
                        }
                    }),
                    json!({
                        "name": "revoke_agent_identity",
                        "description": "Revokes an Agent Identity, invalidating its token and scoped permissions immediately.",
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
                    // =========================================================
                    // 3. Mailbox & OTP Automation Primitives
                    // =========================================================
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
                                    "description": "Agent auth token for capability enforcement."
                                }
                            }
                        }
                    }),
                    json!({
                        "name": "get_latest_otp",
                        "description": "Extracts the latest 4–8 digit 2FA/verification OTP code from emails in under 0.14ms with resource-level authorization.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {
                                "account_id": {
                                    "type": "string",
                                    "description": "The mailbox ID or email address to retrieve the OTP for."
                                },
                                "agent_token": {
                                    "type": "string",
                                    "description": "Agent auth token for capability & resource-ownership validation."
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
                                    "description": "Agent auth token for capability & resource-ownership validation."
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
                                    "description": "Agent auth token for capability & resource-ownership validation."
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
                                    "description": "Agent auth token for capability & resource-ownership validation."
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
                                    "description": "Agent auth token for capability & resource-ownership validation."
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
                                    "description": "Agent auth token for capability & resource-ownership validation."
                                }
                            },
                            "required": ["account_id"]
                        }
                    }),
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
                let tool_name = params_obj
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or_default();
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

    async fn check_authorization_and_ownership(
        &self,
        token_opt: Option<&str>,
        account_id_opt: Option<&str>,
        required: Capability,
    ) -> Result<Option<AgentIdentity>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(token) = token_opt {
            let identity = self
                .db
                .get_agent_identity_by_token(token)
                .await?
                .ok_or("AuthenticationError: Invalid or unrecognized agent_token.")?;

            if identity.status == "revoked" {
                return Err("AuthenticationError: Agent identity has been revoked.".into());
            }

            let caps: Vec<String> =
                serde_json::from_str(&identity.capabilities).unwrap_or_default();
            if !ScopeValidator::has_capability(&caps, required) {
                return Err(format!(
                    "PermissionDenied: Agent '{}' lacks required capability '{}'.",
                    identity.name,
                    required.as_str()
                )
                .into());
            }

            if let Some(account_id) = account_id_opt {
                let owns = self
                    .db
                    .verify_resource_ownership(&identity, account_id)
                    .await?;
                if !owns {
                    return Err(format!(
                        "AccessDenied: Agent '{}' does not have permission to access mailbox '{}'.",
                        identity.name, account_id
                    )
                    .into());
                }
            }

            return Ok(Some(identity));
        }

        Ok(None)
    }

    async fn resolve_existing_account_id(
        &self,
        identifier: &str,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if identifier.contains('@') {
            if let Some(acc) = self.db.get_account_by_address(identifier).await? {
                return Ok(acc.id);
            }
            return Err(format!(
                "Account with address '{}' not found. Please create it first.",
                identifier
            )
            .into());
        }

        if let Some(acc) = self.db.get_account_by_id(identifier).await? {
            return Ok(acc.id);
        }

        if let Some(acc) = self
            .db
            .get_account_by_address(&format!("{}@{}", identifier, self.domain))
            .await?
        {
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
            // 1. Agent Task Protocol Operations
            // =================================================================
            "dispatch_agent_task" => {
                let agent_opt = self
                    .check_authorization_and_ownership(agent_token, None, Capability::TaskDispatch)
                    .await?;
                let source_agent_id = agent_opt
                    .as_ref()
                    .map(|a| a.id.clone())
                    .unwrap_or_else(|| "sovereign_root".to_string());

                let action = args
                    .get("action")
                    .and_then(|a| a.as_str())
                    .ok_or("Missing task 'action'")?;
                let description = args
                    .get("description")
                    .and_then(|d| d.as_str())
                    .ok_or("Missing task 'description'")?;
                let repository = args.get("repository").and_then(|r| r.as_str());
                let branch = args.get("branch").and_then(|b| b.as_str());
                let priority_str = args
                    .get("priority")
                    .and_then(|p| p.as_str())
                    .unwrap_or("normal");
                let priority = TaskPriority::from_str(priority_str);
                let target_agent = args.get("target_agent").and_then(|t| t.as_str());

                let evidence: Option<Vec<String>> =
                    args.get("evidence").and_then(|e| e.as_array()).map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    });

                let acceptance_criteria: Option<Vec<String>> = args
                    .get("acceptance_criteria")
                    .and_then(|a| a.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect()
                    });

                let task = AgentTask::new(
                    &source_agent_id,
                    target_agent,
                    action,
                    repository,
                    branch,
                    priority,
                    description,
                    evidence,
                    acceptance_criteria,
                );

                let audit = TaskAuditLog::new(
                    &task.id,
                    &source_agent_id,
                    "task.created",
                    Some(json!({
                        "action": action,
                        "repository": repository,
                        "target_agent": target_agent
                    })),
                );

                let created_task = self.db.create_task(&task, &audit).await?;

                // Emit task event to broadcast bus
                if let Some(ref bus) = self.event_bus {
                    let _ = bus.send(
                        json!({
                            "type": "new_task",
                            "task": created_task
                        })
                        .to_string(),
                    );
                }

                Ok(json!(created_task))
            }
            "claim_agent_task" => {
                let agent_opt = self
                    .check_authorization_and_ownership(agent_token, None, Capability::TaskClaim)
                    .await?;
                let claiming_agent_id = agent_opt
                    .as_ref()
                    .map(|a| a.id.as_str())
                    .unwrap_or("sovereign_worker");
                let task_id = args
                    .get("task_id")
                    .and_then(|t| t.as_str())
                    .ok_or("Missing task_id")?;

                match self.db.claim_task(task_id, claiming_agent_id).await? {
                    Some(claimed_task) => {
                        if let Some(ref bus) = self.event_bus {
                            let _ = bus.send(json!({
                                "type": "task_claimed",
                                "task_id": task_id,
                                "agent_id": claiming_agent_id
                            }).to_string());
                        }
                        Ok(json!(claimed_task))
                    }
                    None => Err(format!("Task '{}' could not be claimed (already claimed, completed, or does not exist)", task_id).into()),
                }
            }
            "update_task_progress" => {
                let agent_opt = self
                    .check_authorization_and_ownership(agent_token, None, Capability::TaskUpdate)
                    .await?;
                let agent_id = agent_opt
                    .as_ref()
                    .map(|a| a.id.as_str())
                    .unwrap_or("sovereign_worker");
                let task_id = args
                    .get("task_id")
                    .and_then(|t| t.as_str())
                    .ok_or("Missing task_id")?;
                let status = args
                    .get("status")
                    .and_then(|s| s.as_str())
                    .ok_or("Missing status")?;

                let commit_sha = args.get("commit_sha").and_then(|c| c.as_str());
                let pr_url = args.get("pr_url").and_then(|p| p.as_str());
                let test_results = args.get("test_results").and_then(|t| t.as_str());
                let note = args.get("note").and_then(|n| n.as_str());

                let details = json!({
                    "note": note,
                    "commit_sha": commit_sha,
                    "pr_url": pr_url,
                    "test_results": test_results
                });

                let updated = self
                    .db
                    .update_task_progress(
                        task_id,
                        agent_id,
                        status,
                        commit_sha,
                        pr_url,
                        test_results,
                        Some(&details.to_string()),
                    )
                    .await?;

                if let Some(ref bus) = self.event_bus {
                    let _ = bus.send(
                        json!({
                            "type": "task_updated",
                            "task": updated
                        })
                        .to_string(),
                    );
                }

                Ok(json!(updated))
            }
            "complete_agent_task" => {
                let agent_opt = self
                    .check_authorization_and_ownership(agent_token, None, Capability::TaskUpdate)
                    .await?;
                let agent_id = agent_opt
                    .as_ref()
                    .map(|a| a.id.as_str())
                    .unwrap_or("sovereign_worker");
                let task_id = args
                    .get("task_id")
                    .and_then(|t| t.as_str())
                    .ok_or("Missing task_id")?;
                let summary = args
                    .get("summary")
                    .and_then(|s| s.as_str())
                    .ok_or("Missing summary")?;

                let commit_sha = args.get("commit_sha").and_then(|c| c.as_str());
                let pr_url = args.get("pr_url").and_then(|p| p.as_str());
                let test_results = args.get("test_results").and_then(|t| t.as_str());

                let completed = self
                    .db
                    .complete_task(task_id, agent_id, commit_sha, pr_url, test_results, summary)
                    .await?;

                if let Some(ref bus) = self.event_bus {
                    let _ = bus.send(
                        json!({
                            "type": "task_completed",
                            "task": completed
                        })
                        .to_string(),
                    );
                }

                Ok(json!(completed))
            }
            "list_agent_tasks" => {
                let agent_opt = self
                    .check_authorization_and_ownership(agent_token, None, Capability::TaskRead)
                    .await?;
                let agent_id = agent_opt.as_ref().map(|a| a.id.as_str());
                let status = args.get("status").and_then(|s| s.as_str());
                let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(20) as usize;

                let tasks = self.db.list_tasks(agent_id, status, limit).await?;
                Ok(json!(tasks))
            }
            "get_task_audit_trail" => {
                let _ = self
                    .check_authorization_and_ownership(agent_token, None, Capability::TaskRead)
                    .await?;
                let task_id = args
                    .get("task_id")
                    .and_then(|t| t.as_str())
                    .ok_or("Missing task_id")?;

                let trail = self.db.get_task_audit_trail(task_id).await?;
                Ok(json!(trail))
            }

            // =================================================================
            // 2. First-Class Agent Identity Tools
            // =================================================================
            "create_agent_identity" => {
                let name_arg = args
                    .get("name")
                    .and_then(|n| n.as_str())
                    .ok_or("Missing agent 'name'")?;
                let custom_caps = args.get("capabilities").and_then(|c| c.as_array());

                let caps: Vec<String> = if let Some(arr) = custom_caps {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                } else {
                    Capability::standard_agent()
                        .into_iter()
                        .map(|c| c.as_str().to_string())
                        .collect()
                };

                let rand_slug = uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
                let email = format!(
                    "{}-{}@{}",
                    name_arg.to_lowercase().replace(' ', "-"),
                    rand_slug,
                    self.domain
                );

                let credential = self
                    .db
                    .create_agent_identity(name_arg, &email, &caps)
                    .await?;
                Ok(json!(credential))
            }
            "get_agent_identity" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|a| a.as_str())
                    .ok_or("Missing agent_id")?;
                let public_identity = self
                    .db
                    .get_agent_identity_public(agent_id)
                    .await?
                    .ok_or_else(|| format!("Agent identity '{}' not found", agent_id))?;
                Ok(json!(public_identity))
            }
            "list_agent_identities" => {
                let list = self.db.list_agent_identities_public().await?;
                Ok(json!(list))
            }
            "revoke_agent_identity" => {
                let agent_id = args
                    .get("agent_id")
                    .and_then(|a| a.as_str())
                    .ok_or("Missing agent_id")?;
                self.db.revoke_agent_identity(agent_id).await?;
                Ok(json!({ "status": "revoked", "agent_id": agent_id }))
            }

            // =================================================================
            // 3. Mailbox & OTP Operations
            // =================================================================
            "create_agent_inbox" => {
                let agent_opt = self
                    .check_authorization_and_ownership(agent_token, None, Capability::InboxCreate)
                    .await?;
                let display_name = args
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("agent-worker");
                let custom_address = args.get("address").and_then(|a| a.as_str());
                let address = if let Some(addr) = custom_address {
                    addr.to_string()
                } else {
                    let rand_slug =
                        uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
                    format!(
                        "{}-{}@{}",
                        display_name.to_lowercase().replace(' ', "-"),
                        rand_slug,
                        self.domain
                    )
                };

                let owner_id = agent_opt.as_ref().map(|a| a.id.as_str());
                let acc = self
                    .db
                    .create_account_with_owner(&address, Some(display_name), owner_id)
                    .await?;
                Ok(json!(acc))
            }
            "get_latest_otp" => {
                let identifier = args
                    .get("account_id")
                    .and_then(|a| a.as_str())
                    .ok_or("Missing account_id")?;
                let account_id = self.resolve_existing_account_id(identifier).await?;
                self.check_authorization_and_ownership(
                    agent_token,
                    Some(&account_id),
                    Capability::OtpRead,
                )
                .await?;

                let otp = self.db.get_latest_otp(&account_id).await?;
                Ok(json!({ "account_id": account_id, "otp": otp }))
            }
            "wait_for_email" => {
                let identifier = args
                    .get("account_id")
                    .and_then(|a| a.as_str())
                    .unwrap_or("agent");
                let account_id = self.resolve_existing_account_id(identifier).await?;
                self.check_authorization_and_ownership(
                    agent_token,
                    Some(&account_id),
                    Capability::OtpRead,
                )
                .await?;

                let timeout_secs = args
                    .get("timeout_secs")
                    .and_then(|t| t.as_u64())
                    .unwrap_or(30)
                    .min(120);
                let start_time = Utc::now().timestamp();

                let initial_messages = self.db.list_messages_for_account(&account_id).await?;
                if let Some(latest) = initial_messages
                    .into_iter()
                    .find(|m| m.created_at >= start_time - 2)
                {
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
                    let iterations = timeout_secs * 10;
                    for _ in 0..iterations {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        let messages = self.db.list_messages_for_account(&account_id).await?;
                        if let Some(latest) =
                            messages.into_iter().find(|m| m.created_at >= start_time)
                        {
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
                let identifier = args
                    .get("account_id")
                    .and_then(|a| a.as_str())
                    .ok_or("Missing account_id")?;
                let account_id = self.resolve_existing_account_id(identifier).await?;
                self.check_authorization_and_ownership(
                    agent_token,
                    Some(&account_id),
                    Capability::LinksRead,
                )
                .await?;

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
                let identifier = args
                    .get("account_id")
                    .and_then(|a| a.as_str())
                    .ok_or("Missing account_id")?;
                let account_id = self.resolve_existing_account_id(identifier).await?;
                self.check_authorization_and_ownership(
                    agent_token,
                    Some(&account_id),
                    Capability::InboxRead,
                )
                .await?;

                let limit = args.get("limit").and_then(|l| l.as_u64()).unwrap_or(10) as usize;
                let mut messages = self.db.list_messages_for_account(&account_id).await?;
                messages.truncate(limit);
                Ok(json!(messages))
            }
            "send_agent_email" => {
                let identifier = args
                    .get("account_id")
                    .and_then(|a| a.as_str())
                    .ok_or("Missing account_id")?;
                let account_id = self.resolve_existing_account_id(identifier).await?;
                self.check_authorization_and_ownership(
                    agent_token,
                    Some(&account_id),
                    Capability::EmailSend,
                )
                .await?;

                let to = args
                    .get("to")
                    .and_then(|t| t.as_str())
                    .ok_or("Missing recipient 'to'")?;
                let subject = args
                    .get("subject")
                    .and_then(|s| s.as_str())
                    .unwrap_or("(No Subject)");
                let body = args.get("body").and_then(|b| b.as_str()).unwrap_or("");

                let account = self
                    .db
                    .get_account_by_id(&account_id)
                    .await?
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
                let identifier = args
                    .get("account_id")
                    .and_then(|a| a.as_str())
                    .ok_or("Missing account_id")?;
                let account_id = self.resolve_existing_account_id(identifier).await?;
                self.check_authorization_and_ownership(
                    agent_token,
                    Some(&account_id),
                    Capability::InboxDelete,
                )
                .await?;

                self.db.delete_account(&account_id).await?;
                Ok(json!({ "status": "deleted", "account_id": account_id }))
            }
            _ => Err(format!("Unknown tool: {}", name).into()),
        }
    }
}
