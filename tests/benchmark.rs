use agentbox_mail::db::{Database, Message};
use agentbox_mail::engine::capabilities::{Capability, ScopeValidator};
use agentbox_mail::engine::extractor::Extractor;
use agentbox_mail::engine::parser::EmailParser;
use std::time::Instant;
use tokio::sync::broadcast;
use uuid::Uuid;

#[tokio::test]
async fn benchmark_end_to_end_local_pipeline_latency() {
    let db = Database::init("sqlite::memory:").await.expect("InMemory SQLite DB failed");
    let account = db.create_account("agent@apocalypto.in", Some("Bench Agent")).await.unwrap();
    let (tx, _) = broadcast::channel::<String>(100);
    let mut rx = tx.subscribe();

    let raw_email = b"From: sender@github.com\r\nTo: agent@apocalypto.in\r\nSubject: [GitHub] Verification Code: 938102\r\nContent-Type: text/html\r\n\r\n<html><body><p>Your verification code is <b>938102</b>.</p><p><a href=\"https://github.com/verify?token=xyz_938102\">Verify Account</a></p></body></html>";

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

        // 2. Extractor (OTP + SafeLink Anti-Phishing)
        let extracted = Extractor::extract(
            parsed.subject.as_deref(),
            parsed.body_text.as_deref(),
            parsed.body_html.as_deref(),
        );
        assert_eq!(extracted.otp.as_deref(), Some("938102"));
        assert!(!extracted.action_links.is_empty());
        assert!(extracted.action_links[0].is_safe);

        // 3. SQLite Persistence
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
        let evt_payload = serde_json::json!({
            "type": "new_message",
            "message": {
                "account_id": account.id,
                "extracted_otp": extracted.otp,
                "created_at": msg.created_at
            }
        });
        tx.send(evt_payload.to_string()).unwrap();

        // 5. Event Received & MCP Response Construction
        let received = rx.recv().await.unwrap();
        let _parsed_evt: serde_json::Value = serde_json::from_str(&received).unwrap();

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
    println!(" ⚡ AGENTBOX END-TO-END PIPELINE BENCHMARK (Full Ingestion ➔ MCP Result)");
    println!("==========================================================================");
    println!(" Pipeline Stages Tested:");
    println!("   1. Raw MIME Parsing (mail-parser)");
    println!("   2. Regex 4-8 Digit OTP Extraction (once_cell)");
    println!("   3. SafeLink Anti-Redirect / Phishing Analysis (url parser)");
    println!("   4. SQLite Database Transaction INSERT (sqlx)");
    println!("   5. Tokio Broadcast Channel Event Dispatch");
    println!("   6. Realtime Event Bus Receive & MCP Response Construction");
    println!("--------------------------------------------------------------------------");
    println!(" Sample Size : {} complete pipeline cycles", iterations);
    println!(" Average     : {:.3} µs ({:.4} ms)", avg_ns / 1000.0, avg_ns / 1_000_000.0);
    println!(" p50 Median  : {:.3} µs ({:.4} ms)", p50_ns as f64 / 1000.0, p50_ns as f64 / 1_000_000.0);
    println!(" p95         : {:.3} µs ({:.4} ms)", p95_ns as f64 / 1000.0, p95_ns as f64 / 1_000_000.0);
    println!(" p99         : {:.3} µs ({:.4} ms)", p99_ns as f64 / 1000.0, p99_ns as f64 / 1_000_000.0);
    println!(" Throughput  : {:.0} full cycles/sec", iterations as f64 / total_elapsed.as_secs_f64());
    println!("==========================================================================\n");

    assert!(avg_ns < 10_000_000.0, "Complete local pipeline should execute under 10ms in debug mode");
}

#[tokio::test]
async fn test_agent_identity_and_capability_security_enforcement() {
    let db = Database::init("sqlite::memory:").await.expect("InMemory SQLite DB failed");
    
    // 1. Create a restricted Browser QA Agent with only OtpRead and LinksRead
    let qa_caps = vec!["otp.read".to_string(), "links.read".to_string()];
    let identity = db.create_agent_identity("browser-qa", "qa@apocalypto.in", &qa_caps).await.unwrap();

    assert_eq!(identity.name, "browser-qa");
    assert!(identity.token.starts_with("agb_"));

    // 2. Validate capability scopes
    let caps: Vec<String> = serde_json::from_str(&identity.capabilities).unwrap();
    assert!(ScopeValidator::has_capability(&caps, Capability::OtpRead));
    assert!(ScopeValidator::has_capability(&caps, Capability::LinksRead));
    assert!(!ScopeValidator::has_capability(&caps, Capability::EmailSend));
    assert!(!ScopeValidator::has_capability(&caps, Capability::InboxDelete));

    // 3. Test revocation
    db.revoke_agent_identity(&identity.id).await.unwrap();
    let updated = db.get_agent_identity(&identity.id).await.unwrap().unwrap();
    assert_eq!(updated.status, "revoked");
}
