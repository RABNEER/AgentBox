use agentbox_mail::db::{Database, Message};
use agentbox_mail::engine::extractor::Extractor;
use agentbox_mail::engine::outbound::OutboundMailer;
use agentbox_mail::engine::parser::EmailParser;
use agentbox_mail::mcp::McpServer;
use serde_json::json;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::broadcast;
use uuid::Uuid;

#[tokio::test]
async fn benchmark_full_e2e_mcp_pipeline_latency() {
    let db = Database::init("sqlite::memory:")
        .await
        .expect("InMemory SQLite DB failed");
    let mailer = OutboundMailer::new(None, 587, None, None);
    let (tx, _) = broadcast::channel::<String>(100);

    let mcp = Arc::new(McpServer::new(
        db.clone(),
        mailer,
        "test.agentbox".to_string(),
        Some(tx.clone()),
    ));

    // Create an authenticated Agent Identity with scoped capabilities
    let creds = db
        .create_agent_identity(
            "bench-agent",
            "bench@test.agentbox",
            &["otp.read".to_string(), "inbox.read".to_string()],
        )
        .await
        .unwrap();
    let account = db
        .get_account_by_address("bench@test.agentbox")
        .await
        .unwrap()
        .unwrap();

    let raw_email = b"From: auth@github.com\r\nTo: bench@test.agentbox\r\nSubject: [GitHub] Verification code is 782014\r\nContent-Type: text/html\r\n\r\n<html><body><p>Your OTP code is <b>782014</b>.</p><p><a href=\"https://github.com/verify?token=782014\">Verify Now</a></p></body></html>";

    let iterations = 1_000;
    let mut latencies_nanos = Vec::with_capacity(iterations);

    // Warm up
    for _ in 0..10 {
        let _ = EmailParser::parse_mime(raw_email);
    }

    let total_start = Instant::now();
    for i in 0..iterations {
        let t0 = Instant::now();

        // 1. MIME Parsing
        let parsed = EmailParser::parse_mime(raw_email).expect("MIME parse failed");

        // 2. Extractor (Regex OTP + SafeLink Analysis)
        let extracted = Extractor::extract(
            parsed.subject.as_deref(),
            parsed.body_text.as_deref(),
            parsed.body_html.as_deref(),
        );

        // 3. SQLite Database Transaction INSERT
        let msg = Message {
            id: format!("msg_{}_{}", i, Uuid::new_v4()),
            account_id: account.id.clone(),
            from_address: parsed.from.clone(),
            to_address: account.address.clone(),
            subject: parsed.subject.clone(),
            body_text: parsed.body_text.clone(),
            body_html: parsed.body_html.clone(),
            raw_mime: None,
            extracted_otp: extracted.otp.clone(),
            extracted_links: serde_json::to_string(&extracted.action_links).ok(),
            direction: "inbound".to_string(),
            created_at: chrono::Utc::now().timestamp(),
        };
        db.insert_message(&msg).await.unwrap();

        // 4. Tokio Broadcast Event Bus Dispatch
        let evt_payload = json!({
            "type": "new_message",
            "message": {
                "account_id": account.id,
                "extracted_otp": extracted.otp,
                "created_at": msg.created_at
            }
        });
        let _ = tx.send(evt_payload.to_string());

        // 5. Full MCP Tool Call Execution (tools/call -> get_latest_otp with token auth)
        let mcp_response = mcp
            .handle_request(
                "tools/call".to_string(),
                Some(json!({
                    "name": "get_latest_otp",
                    "arguments": {
                        "account_id": account.id,
                        "agent_token": creds.auth_token
                    }
                })),
                json!(1),
            )
            .await;

        assert!(
            mcp_response.result.is_some(),
            "MCP call must return successful result"
        );

        let elapsed_nanos = t0.elapsed().as_nanos();
        latencies_nanos.push(elapsed_nanos);
    }
    let total_elapsed = total_start.elapsed();

    latencies_nanos.sort_unstable();
    let p50_ns = latencies_nanos[iterations * 50 / 100];
    let p95_ns = latencies_nanos[iterations * 95 / 100];
    let p99_ns = latencies_nanos[iterations * 99 / 100];
    let avg_ns = total_elapsed.as_nanos() as f64 / iterations as f64;

    println!("\n==========================================================================");
    println!(" ⚡ AGENTBOX FULL END-TO-END MCP PIPELINE BENCHMARK");
    println!("==========================================================================");
    println!(" Full Pipeline Stages Tested:");
    println!("   1. Raw MIME Stream Parsing (mail-parser)");
    println!("   2. Regex 4-8 Digit OTP Extraction (once_cell)");
    println!("   3. SafeLink Anti-Redirect & Phishing Analysis (url)");
    println!("   4. SQLite Database Transaction INSERT (sqlx)");
    println!("   5. Tokio Broadcast Channel Event Dispatch");
    println!("   6. Authenticated MCP Tool Call (tools/call ➔ get_latest_otp)");
    println!("   7. Full JSON-RPC Serialization & Result Output");
    println!("--------------------------------------------------------------------------");
    println!(" Sample Size : {} complete pipeline cycles", iterations);
    println!(
        " Average     : {:.3} µs ({:.4} ms)",
        avg_ns / 1000.0,
        avg_ns / 1_000_000.0
    );
    println!(
        " p50 Median  : {:.3} µs ({:.4} ms)",
        p50_ns as f64 / 1000.0,
        p50_ns as f64 / 1_000_000.0
    );
    println!(
        " p95         : {:.3} µs ({:.4} ms)",
        p95_ns as f64 / 1000.0,
        p95_ns as f64 / 1_000_000.0
    );
    println!(
        " p99         : {:.3} µs ({:.4} ms)",
        p99_ns as f64 / 1000.0,
        p99_ns as f64 / 1_000_000.0
    );
    println!(
        " Throughput  : {:.0} full cycles/sec",
        iterations as f64 / total_elapsed.as_secs_f64()
    );
    println!("==========================================================================\n");

    assert!(
        avg_ns < 10_000_000.0,
        "Complete MCP pipeline should execute under 10ms in debug mode"
    );
}

#[tokio::test]
async fn test_object_level_resource_ownership_and_access_denial() {
    let db = Database::init("sqlite::memory:")
        .await
        .expect("InMemory SQLite DB failed");
    let mailer = OutboundMailer::new(None, 587, None, None);
    let mcp = Arc::new(McpServer::new(
        db.clone(),
        mailer,
        "test.agentbox".to_string(),
        None,
    ));

    // 1. Provision Agent A and Agent B
    let agent_a = db
        .create_agent_identity(
            "agent-a",
            "agent-a@test.agentbox",
            &["inbox.read".to_string(), "otp.read".to_string()],
        )
        .await
        .unwrap();
    let _agent_b = db
        .create_agent_identity(
            "agent-b",
            "agent-b@test.agentbox",
            &["inbox.read".to_string(), "otp.read".to_string()],
        )
        .await
        .unwrap();

    let account_a = db
        .get_account_by_address("agent-a@test.agentbox")
        .await
        .unwrap()
        .unwrap();
    let account_b = db
        .get_account_by_address("agent-b@test.agentbox")
        .await
        .unwrap()
        .unwrap();

    // 2. Agent A accesses its OWN inbox -> SUCCEEDS
    let res_own = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "get_latest_otp",
                "arguments": {
                    "account_id": account_a.id,
                    "agent_token": agent_a.auth_token
                }
            })),
            json!(1),
        )
        .await;
    assert!(
        res_own.result.is_some(),
        "Agent A accessing its own inbox must succeed"
    );

    // 3. Agent A tries to access AGENT B'S inbox -> ACCESS DENIED
    let res_cross = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "get_latest_otp",
                "arguments": {
                    "account_id": account_b.id,
                    "agent_token": agent_a.auth_token
                }
            })),
            json!(2),
        )
        .await;
    assert!(
        res_cross.error.is_some(),
        "Agent A accessing Agent B's inbox must be denied"
    );
    let err_msg = res_cross
        .error
        .unwrap()
        .get("message")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        err_msg.contains("AccessDenied"),
        "Error must contain AccessDenied"
    );

    // 4. Agent A attempts to SEND EMAIL without capability -> PERMISSION DENIED
    let res_unauth_action = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "send_agent_email",
                "arguments": {
                    "account_id": account_a.id,
                    "to": "target@example.com",
                    "body": "Hello",
                    "agent_token": agent_a.auth_token
                }
            })),
            json!(3),
        )
        .await;
    assert!(
        res_unauth_action.error.is_some(),
        "Unauthorized action must be denied"
    );
    let perm_err = res_unauth_action
        .error
        .unwrap()
        .get("message")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        perm_err.contains("PermissionDenied"),
        "Error must contain PermissionDenied"
    );

    // 5. Revoked token access -> AUTHENTICATION ERROR
    db.revoke_agent_identity(&agent_a.agent_id).await.unwrap();
    let res_revoked = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "get_latest_otp",
                "arguments": {
                    "account_id": account_a.id,
                    "agent_token": agent_a.auth_token
                }
            })),
            json!(4),
        )
        .await;
    assert!(
        res_revoked.error.is_some(),
        "Revoked agent access must fail"
    );
    let auth_err = res_revoked
        .error
        .unwrap()
        .get("message")
        .unwrap()
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        auth_err.contains("revoked"),
        "Error must indicate token is revoked"
    );
}

#[tokio::test]
async fn test_autonomous_agent_task_orchestration_and_audit_lineage() {
    let db = Database::init("sqlite::memory:")
        .await
        .expect("InMemory SQLite DB failed");
    let mailer = OutboundMailer::new(None, 587, None, None);
    let (tx, _) = broadcast::channel::<String>(100);

    let mcp = Arc::new(McpServer::new(
        db.clone(),
        mailer,
        "test.agentbox".to_string(),
        Some(tx.clone()),
    ));

    // 1. Provision Jules (QA Orchestrator) and Coder (Worker Agent)
    let jules = db
        .create_agent_identity(
            "jules",
            "jules@test.agentbox",
            &["task.dispatch".to_string(), "task.read".to_string()],
        )
        .await
        .unwrap();

    let coder = db
        .create_agent_identity(
            "coder",
            "coder@test.agentbox",
            &[
                "task.claim".to_string(),
                "task.update".to_string(),
                "task.read".to_string(),
            ],
        )
        .await
        .unwrap();

    // 2. Jules Dispatches a Work Order (Task) via MCP
    let dispatch_res = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "dispatch_agent_task",
                "arguments": {
                    "action": "fix_bug",
                    "repository": "RABNEER/EstateFlow",
                    "branch": "main",
                    "priority": "high",
                    "description": "Property search returns duplicate listings when multiple filters are applied",
                    "evidence": ["tests/property-search.spec.ts:87"],
                    "acceptance_criteria": [
                        "Deduplicate property query results",
                        "Add regression spec test",
                        "All unit & e2e tests pass"
                    ],
                    "agent_token": jules.auth_token
                }
            })),
            json!(1),
        )
        .await;

    assert!(
        dispatch_res.result.is_some(),
        "Jules dispatching task must succeed"
    );
    let res_text = dispatch_res.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    let task_obj: serde_json::Value = serde_json::from_str(&res_text).unwrap();
    let task_id = task_obj["id"].as_str().unwrap();
    assert_eq!(task_obj["status"], "received");
    assert_eq!(task_obj["priority"], "high");

    // 3. Coder Agent Claims the Task via MCP
    let claim_res = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "claim_agent_task",
                "arguments": {
                    "task_id": task_id,
                    "agent_token": coder.auth_token
                }
            })),
            json!(2),
        )
        .await;

    assert!(
        claim_res.result.is_some(),
        "Coder claiming task must succeed"
    );

    // 4. Coder Agent Updates Progress (Running & Git Commit)
    let progress_res = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "update_task_progress",
                "arguments": {
                    "task_id": task_id,
                    "status": "pr_opened",
                    "commit_sha": "a91f4b23",
                    "pr_url": "https://github.com/RABNEER/EstateFlow/pull/42",
                    "test_results": "143 passed, 0 failed",
                    "note": "Fixed duplicate SQL JOIN and verified regression suite",
                    "agent_token": coder.auth_token
                }
            })),
            json!(3),
        )
        .await;

    assert!(
        progress_res.result.is_some(),
        "Coder updating progress must succeed"
    );

    // 5. Coder Agent Completes the Task
    let complete_res = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "complete_agent_task",
                "arguments": {
                    "task_id": task_id,
                    "summary": "Resolved duplicate listings by adding DISTINCT ON (id) clause. PR #42 opened and verified against CI.",
                    "commit_sha": "a91f4b23",
                    "pr_url": "https://github.com/RABNEER/EstateFlow/pull/42",
                    "test_results": "143 passed, 0 failed",
                    "agent_token": coder.auth_token
                }
            })),
            json!(4),
        )
        .await;

    assert!(
        complete_res.result.is_some(),
        "Coder completing task must succeed"
    );

    // 6. Retrieve Immutable Task Audit Trail Lineage
    let audit_res = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "get_task_audit_trail",
                "arguments": {
                    "task_id": task_id,
                    "agent_token": jules.auth_token
                }
            })),
            json!(5),
        )
        .await;

    assert!(
        audit_res.result.is_some(),
        "Fetching audit trail must succeed"
    );
    let audit_text = audit_res.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    let audit_logs: Vec<serde_json::Value> = serde_json::from_str(&audit_text).unwrap();

    // Verify complete lineage: created -> claimed -> pr_opened -> completed
    assert!(
        audit_logs.len() >= 4,
        "Audit trail must contain at least 4 lifecycle events"
    );
    assert_eq!(audit_logs[0]["event_type"], "task.created");
    assert_eq!(audit_logs[1]["event_type"], "task.claimed");
    assert_eq!(audit_logs[2]["event_type"], "task.pr_opened");
    assert_eq!(audit_logs[3]["event_type"], "task.completed");
}

#[tokio::test]
async fn test_automatic_email_to_agent_task_and_mcp_orchestration() {
    let db = Database::init("sqlite::memory:")
        .await
        .expect("InMemory SQLite DB failed");
    let mailer = OutboundMailer::new(None, 587, None, None);
    let (tx, _) = broadcast::channel::<String>(100);

    let mcp = Arc::new(McpServer::new(
        db.clone(),
        mailer,
        "test.agentbox".to_string(),
        Some(tx.clone()),
    ));

    // 1. Provision Coder Agent Identity
    let coder = db
        .create_agent_identity(
            "coder",
            "coder@test.agentbox",
            &[
                "task.claim".to_string(),
                "task.update".to_string(),
                "task.read".to_string(),
            ],
        )
        .await
        .unwrap();

    // 2. Jules sends an email over SMTP format
    let raw_email = b"From: jules@external.ai\r\nTo: coder@test.agentbox\r\nSubject: [TASK:BUG] Fix duplicate property filter in EstateFlow\r\nContent-Type: text/plain\r\n\r\nRepository: RABNEER/EstateFlow\r\nPriority: high\r\nEvidence: tests/property-search.spec.ts:87\r\nExpected: Deduplicate property query results";

    let parsed = EmailParser::parse_mime(raw_email).expect("MIME parse failed");

    // Simulate Inbound Ingestion Pipeline with TaskDetector
    let msg_id = format!("msg_{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);
    let account = db
        .get_account_by_address("coder@test.agentbox")
        .await
        .unwrap()
        .unwrap();
    let message = Message {
        id: msg_id.clone(),
        account_id: account.id.clone(),
        from_address: parsed.from.clone(),
        to_address: "coder@test.agentbox".to_string(),
        subject: parsed.subject.clone(),
        body_text: parsed.body_text.clone(),
        body_html: None,
        raw_mime: Some(String::from_utf8_lossy(raw_email).to_string()),
        extracted_otp: None,
        extracted_links: None,
        direction: "inbound".to_string(),
        created_at: chrono::Utc::now().timestamp(),
    };
    db.insert_message(&message).await.unwrap();

    // TaskDetector auto-provisions task
    let task = agentbox_mail::engine::tasks::TaskDetector::detect_and_parse(
        message.subject.as_deref(),
        message.body_text.as_deref(),
        &message.from_address,
        &message.to_address,
    )
    .expect("Email must be recognized as an AgentTask");

    let audit = agentbox_mail::engine::tasks::TaskAuditLog::new(
        &task.id,
        &task.source_agent_id,
        "task.created_from_email",
        Some(json!({ "email_msg_id": message.id })),
    );
    let created_task = db.create_task(&task, &audit).await.unwrap();
    assert_eq!(created_task.action, "fix_bug");
    assert_eq!(
        created_task.repository.as_deref(),
        Some("RABNEER/EstateFlow")
    );
    assert_eq!(created_task.priority, "high");

    // 3. Coder Agent queries tasks via MCP and claims the auto-created work order
    let list_res = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "list_agent_tasks",
                "arguments": {
                    "status": "received",
                    "agent_token": coder.auth_token
                }
            })),
            json!(1),
        )
        .await;

    assert!(list_res.result.is_some());
    let list_text = list_res.result.unwrap()["content"][0]["text"]
        .as_str()
        .unwrap()
        .to_string();
    let tasks: Vec<serde_json::Value> = serde_json::from_str(&list_text).unwrap();
    assert!(
        !tasks.is_empty(),
        "Auto-created task must appear in task list"
    );

    // 4. Coder claims the auto-created task
    let claim_res = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "claim_agent_task",
                "arguments": {
                    "task_id": created_task.id,
                    "agent_token": coder.auth_token
                }
            })),
            json!(2),
        )
        .await;

    assert!(
        claim_res.result.is_some(),
        "Coder must successfully claim the auto-created task"
    );

    // 5. Coder completes the task
    let complete_res = mcp
        .handle_request(
            "tools/call".to_string(),
            Some(json!({
                "name": "complete_agent_task",
                "arguments": {
                    "task_id": created_task.id,
                    "summary": "Fixed duplicate query results from email work order",
                    "commit_sha": "f12a890",
                    "pr_url": "https://github.com/RABNEER/EstateFlow/pull/43",
                    "agent_token": coder.auth_token
                }
            })),
            json!(3),
        )
        .await;

    assert!(
        complete_res.result.is_some(),
        "Task must be marked completed"
    );
}
