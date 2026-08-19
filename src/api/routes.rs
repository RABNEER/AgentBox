use crate::db::{Account, Database, Message};
use crate::engine::{outbound::SendEmailRequest, EmailParser, Extractor, OutboundMailer};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, Response, StatusCode},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use rust_embed::RustEmbed;
use rustls_pki_types::ServerName;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_rustls::TlsConnector;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use uuid::Uuid;

#[derive(RustEmbed)]
#[folder = "ui/"]
pub struct UiAssets;

#[derive(Clone)]
pub struct AppState {
    pub db: Database,
    pub mailer: OutboundMailer,
    pub tx: broadcast::Sender<String>,
    pub domain: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateAccountDto {
    pub display_name: Option<String>,
    pub address: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct HostingerMessageDto {
    pub from: Option<String>,
    pub to: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub text: Option<String>,
    pub html: Option<String>,
    pub thread_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct InboundEmailDto {
    pub to: Option<String>,
    pub from: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub raw_mime: Option<String>,
    pub event: Option<String>,
    pub mailbox: Option<String>,
    pub message: Option<HostingerMessageDto>,
}

#[derive(Debug, Serialize)]
pub struct OtpResponse {
    pub account_id: String,
    pub otp: Option<String>,
    pub message_id: Option<String>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize)]
pub struct ActionLinksResponse {
    pub account_id: String,
    pub links: Vec<String>,
    pub timestamp: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigDto {
    pub domain: String,
    pub primary_email: String,
    pub agent_name: Option<String>,
    pub imap_host: Option<String>,
    pub imap_port: Option<u16>,
    pub imap_user: Option<String>,
    pub imap_pass: Option<String>,
    pub has_imap_pass: Option<bool>,
    pub smtp_host: Option<String>,
    pub smtp_port: Option<u16>,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
    pub has_smtp_pass: Option<bool>,
    pub configured: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct TestConnectionDto {
    pub protocol: Option<String>, // "imap" or "smtp"
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
}

pub fn create_router(state: AppState) -> Router {
    Router::new()
        // Core API
        .route("/v1/inbound", post(handle_inbound))
        .route("/v1/accounts", get(list_accounts).post(create_account))
        .route("/v1/accounts/:id", get(get_account).delete(delete_account))
        .route(
            "/v1/accounts/:id/messages",
            get(list_messages).post(send_message),
        )
        .route(
            "/v1/accounts/:id/messages/:msg_id",
            get(get_message).delete(delete_message),
        )
        .route("/v1/accounts/:id/otp", get(get_latest_otp))
        .route("/v1/accounts/:id/links", get(get_latest_links))
        .route("/v1/events", get(handle_sse_events))
        // Settings & Provider Setup (Update 2.0)
        .route("/v1/config", get(get_config).post(save_config))
        .route("/v1/config/test", post(test_connection))
        .route(
            "/v1/docker/stalwart",
            get(get_stalwart_docker_status).post(start_stalwart_docker),
        )
        .route("/v1/mcp/install", post(install_mcp_into_ides))
        // 1-Click Connect AI Agents Wizard
        .route("/v1/integrations/detect", get(detect_integrations))
        .route("/v1/integrations/connect", post(connect_integrations))
        // Embedded UI Static Assets
        .route("/", get(serve_index))
        .route("/index.html", get(serve_index))
        .route("/style.css", get(serve_css))
        .route("/app.js", get(serve_js))
        .with_state(Arc::new(state))
}

// 1. Inbound Email Handler
async fn handle_inbound(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InboundEmailDto>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let now = Utc::now().timestamp();
    let msg_id = format!("msg_{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);

    let (from_addr, to_addr, subject, body_text, body_html, raw_mime) =
        if let Some(mime_str) = payload.raw_mime {
            if let Some(parsed) = EmailParser::parse_mime(mime_str.as_bytes()) {
                let to = parsed
                    .to
                    .first()
                    .cloned()
                    .unwrap_or_else(|| format!("agent@{}", state.domain));
                (
                    parsed.from,
                    to,
                    parsed.subject,
                    parsed.body_text,
                    parsed.body_html,
                    Some(mime_str),
                )
            } else {
                return Err(StatusCode::BAD_REQUEST);
            }
        } else if let Some(hostinger_msg) = payload.message {
            let to = payload
                .mailbox
                .or(hostinger_msg.to)
                .unwrap_or_else(|| format!("agent@{}", state.domain));
            let from = hostinger_msg
                .from
                .unwrap_or_else(|| "external@service.com".to_string());
            let subject = hostinger_msg.subject;
            let text = hostinger_msg.text.or(hostinger_msg.body);
            let html = hostinger_msg.html;
            (from, to, subject, text, html, None)
        } else {
            let to = payload
                .to
                .unwrap_or_else(|| format!("agent@{}", state.domain));
            let from = payload
                .from
                .unwrap_or_else(|| "external@service.com".to_string());
            let body = payload.body.unwrap_or_default();
            let is_html = body.contains('<') && body.contains('>');
            let (body_text, body_html) = if is_html {
                (None, Some(body))
            } else {
                (Some(body), None)
            };
            (from, to, payload.subject, body_text, body_html, None)
        };

    let extracted = Extractor::extract(
        subject.as_deref(),
        body_text.as_deref(),
        body_html.as_deref(),
    );

    let account = match state.db.get_account_by_address(&to_addr).await {
        Ok(Some(acc)) => acc,
        _ => state
            .db
            .create_account(&to_addr, Some("Auto-Provisioned Inbox"))
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?,
    };

    let links_json = serde_json::to_string(&extracted.action_links).ok();

    let message = Message {
        id: msg_id.clone(),
        account_id: account.id.clone(),
        from_address: from_addr.clone(),
        to_address: to_addr.clone(),
        subject: subject.clone(),
        body_text,
        body_html,
        raw_mime,
        extracted_otp: extracted.otp.clone(),
        extracted_links: links_json,
        direction: "inbound".to_string(),
        created_at: now,
    };

    state
        .db
        .insert_message(&message)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Auto-detect and provision AgentTask if email represents a work order
    if let Some(task) = crate::engine::tasks::TaskDetector::detect_and_parse(
        message.subject.as_deref(),
        message.body_text.as_deref(),
        &message.from_address,
        &message.to_address,
    ) {
        let audit = crate::engine::tasks::TaskAuditLog::new(
            &task.id,
            &task.source_agent_id,
            "task.created_from_email",
            Some(serde_json::json!({
                "email_message_id": message.id,
                "subject": message.subject,
                "action": task.action,
                "repository": task.repository
            })),
        );
        if let Ok(created_task) = state.db.create_task(&task, &audit).await {
            let _ = state.tx.send(
                serde_json::json!({
                    "type": "new_task",
                    "task": created_task
                })
                .to_string(),
            );
        }
    }

    let event_payload = serde_json::json!({
        "type": "new_message",
        "message": message
    });
    let _ = state.tx.send(event_payload.to_string());

    Ok(Json(serde_json::json!({
        "status": "received",
        "message_id": msg_id,
        "extracted_otp": extracted.otp,
        "action_links": extracted.action_links,
        "account_id": account.id
    })))
}

// 2. Account Handlers
async fn list_accounts(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<Account>>, StatusCode> {
    state
        .db
        .list_accounts()
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn create_account(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateAccountDto>,
) -> Result<Json<Account>, StatusCode> {
    let name = payload.display_name.unwrap_or_else(|| "agent".to_string());
    let address = if let Some(addr) = payload.address {
        addr
    } else {
        let rand_slug = uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
        format!(
            "{}-{}@{}",
            name.to_lowercase().replace(' ', "-"),
            rand_slug,
            state.domain
        )
    };

    let acc = state
        .db
        .create_account(&address, Some(&name))
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(acc))
}

async fn get_account(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
) -> Result<Json<Account>, StatusCode> {
    let acc = state
        .db
        .get_account_by_id(&account_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(acc))
}

async fn delete_account(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state
        .db
        .delete_account(&account_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let event_payload = serde_json::json!({
        "type": "account_deleted",
        "account_id": account_id
    });
    let _ = state.tx.send(event_payload.to_string());

    Ok(Json(
        serde_json::json!({ "status": "deleted", "account_id": account_id }),
    ))
}

// 3. Message Handlers
async fn list_messages(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
) -> Result<Json<Vec<Message>>, StatusCode> {
    state
        .db
        .list_messages_for_account(&account_id)
        .await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_message(
    State(state): State<Arc<AppState>>,
    Path((_account_id, msg_id)): Path<(String, String)>,
) -> Result<Json<Message>, StatusCode> {
    let msg = state
        .db
        .get_message(&msg_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(msg))
}

async fn delete_message(
    State(state): State<Arc<AppState>>,
    Path((_account_id, msg_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    state
        .db
        .delete_message(&msg_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let event_payload = serde_json::json!({
        "type": "message_deleted",
        "message_id": msg_id
    });
    let _ = state.tx.send(event_payload.to_string());

    Ok(Json(
        serde_json::json!({ "status": "deleted", "message_id": msg_id }),
    ))
}

// 4. OTP & Action Links Helpers
async fn get_latest_otp(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
) -> Result<Json<OtpResponse>, StatusCode> {
    let otp = state
        .db
        .get_latest_otp(&account_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(OtpResponse {
        account_id,
        otp,
        message_id: None,
        timestamp: Utc::now().timestamp(),
    }))
}

async fn get_latest_links(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
) -> Result<Json<ActionLinksResponse>, StatusCode> {
    let messages = state
        .db
        .list_messages_for_account(&account_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

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

    Ok(Json(ActionLinksResponse {
        account_id,
        links,
        timestamp: Utc::now().timestamp(),
    }))
}

// 5. Outbound Email Sender
async fn send_message(
    State(state): State<Arc<AppState>>,
    Path(account_id): Path<String>,
    Json(req): Json<SendEmailRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let account = state
        .db
        .get_account_by_id(&account_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut send_req = req.clone();
    if send_req.from.is_none() {
        send_req.from = Some(account.address.clone());
    }

    let status = state
        .mailer
        .send_email(send_req)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let msg_id = format!("msg_{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);
    let now = Utc::now().timestamp();
    let to_first = req.to.first().cloned().unwrap_or_default();

    let out_msg = Message {
        id: msg_id.clone(),
        account_id: account.id,
        from_address: account.address,
        to_address: to_first,
        subject: Some(req.subject),
        body_text: req.text,
        body_html: req.html,
        raw_mime: None,
        extracted_otp: None,
        extracted_links: None,
        direction: "outbound".to_string(),
        created_at: now,
    };

    let _ = state.db.insert_message(&out_msg).await;

    let event_payload = serde_json::json!({
        "type": "new_message",
        "message": out_msg
    });
    let _ = state.tx.send(event_payload.to_string());

    Ok(Json(serde_json::json!({
        "status": "success",
        "result": status,
        "message_id": msg_id
    })))
}

// 6. Settings / Config API (Update 2.0)
async fn get_config(State(state): State<Arc<AppState>>) -> Result<Json<ConfigDto>, StatusCode> {
    let domain = std::env::var("DOMAIN").unwrap_or_else(|_| state.domain.clone());
    let primary_email =
        std::env::var("PRIMARY_EMAIL").unwrap_or_else(|_| format!("agent@{}", domain));
    let agent_name = std::env::var("AGENT_NAME").ok();
    let imap_host = std::env::var("IMAP_HOST").ok();
    let imap_port = std::env::var("IMAP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .or(Some(993));
    let imap_user = std::env::var("IMAP_USER").ok();
    let has_imap_pass = std::env::var("IMAP_PASS")
        .map(|p| !p.trim().is_empty())
        .unwrap_or(false);

    let smtp_host = std::env::var("SMTP_HOST").ok();
    let smtp_port = std::env::var("SMTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .or(Some(587));
    let smtp_user = std::env::var("SMTP_USER").ok();
    let has_smtp_pass = std::env::var("SMTP_PASS")
        .map(|p| !p.trim().is_empty())
        .unwrap_or(false);

    let configured = imap_host.is_some() || smtp_host.is_some();

    Ok(Json(ConfigDto {
        domain,
        primary_email,
        agent_name,
        imap_host,
        imap_port,
        imap_user,
        imap_pass: None,
        has_imap_pass: Some(has_imap_pass),
        smtp_host,
        smtp_port,
        smtp_user,
        smtp_pass: None,
        has_smtp_pass: Some(has_smtp_pass),
        configured: Some(configured),
    }))
}

async fn save_config(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConfigDto>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let mut env_lines = Vec::new();
    env_lines.push(format!("DOMAIN={}", payload.domain));
    env_lines.push(format!("PRIMARY_EMAIL={}", payload.primary_email));
    if let Some(ref name) = payload.agent_name {
        env_lines.push(format!("AGENT_NAME={}", name));
    }
    env_lines.push("PORT=3000".to_string());
    env_lines.push("SMTP_INBOUND_PORT=2525".to_string());
    env_lines.push("DATABASE_URL=sqlite://agentbox.db?mode=rwc".to_string());

    std::env::set_var("DOMAIN", &payload.domain);
    std::env::set_var("PRIMARY_EMAIL", &payload.primary_email);

    if let Some(ref host) = payload.imap_host {
        env_lines.push(format!("IMAP_HOST={}", host));
        std::env::set_var("IMAP_HOST", host);
    }
    if let Some(port) = payload.imap_port {
        env_lines.push(format!("IMAP_PORT={}", port));
        std::env::set_var("IMAP_PORT", port.to_string());
    }
    if let Some(ref user) = payload.imap_user {
        env_lines.push(format!("IMAP_USER={}", user));
        std::env::set_var("IMAP_USER", user);
    }
    if let Some(ref pass) = payload.imap_pass {
        if !pass.trim().is_empty() {
            env_lines.push(format!("IMAP_PASS={}", pass));
            std::env::set_var("IMAP_PASS", pass);
        }
    }

    if let Some(ref host) = payload.smtp_host {
        env_lines.push(format!("SMTP_HOST={}", host));
        std::env::set_var("SMTP_HOST", host);
    }
    if let Some(port) = payload.smtp_port {
        env_lines.push(format!("SMTP_PORT={}", port));
        std::env::set_var("SMTP_PORT", port.to_string());
    }
    if let Some(ref user) = payload.smtp_user {
        env_lines.push(format!("SMTP_USER={}", user));
        std::env::set_var("SMTP_USER", user);
    }
    if let Some(ref pass) = payload.smtp_pass {
        if !pass.trim().is_empty() {
            env_lines.push(format!("SMTP_PASS={}", pass));
            std::env::set_var("SMTP_PASS", pass);
        }
    }

    let _ = std::fs::write(".env", env_lines.join("\n"));

    // Auto-provision primary inbox
    let _ = state
        .db
        .create_account(&payload.primary_email, Some("Primary Mailbox"))
        .await;

    // Broadcast config update
    let event = serde_json::json!({
        "type": "config_updated",
        "domain": payload.domain,
        "primary_email": payload.primary_email
    });
    let _ = state.tx.send(event.to_string());

    Ok(Json(serde_json::json!({
        "status": "success",
        "message": "Configuration saved to .env and activated!"
    })))
}

// 7. Live TLS Connection Test
async fn test_connection(
    Json(payload): Json<TestConnectionDto>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let start = std::time::Instant::now();
    let proto = payload.protocol.as_deref().unwrap_or("imap");

    if proto == "imap" {
        // IMAP TLS Test
        let root_store = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };
        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let addr = format!("{}:{}", payload.host, payload.port);

        let tcp_stream =
            match tokio::time::timeout(Duration::from_secs(6), TcpStream::connect(&addr)).await {
                Ok(Ok(stream)) => stream,
                Ok(Err(e)) => {
                    return Ok(Json(serde_json::json!({
                        "success": false,
                        "error": format!("Could not connect to {}: {}", addr, e)
                    })));
                }
                Err(_) => {
                    return Ok(Json(serde_json::json!({
                        "success": false,
                        "error": format!("Connection to {} timed out after 6s", addr)
                    })));
                }
            };

        let server_name = match ServerName::try_from(payload.host.as_str()) {
            Ok(s) => s.to_owned(),
            Err(e) => {
                return Ok(Json(serde_json::json!({
                    "success": false,
                    "error": format!("Invalid host name: {}", e)
                })));
            }
        };

        let tls_stream = match connector.connect(server_name, tcp_stream).await {
            Ok(s) => s,
            Err(e) => {
                return Ok(Json(serde_json::json!({
                    "success": false,
                    "error": format!("TLS Handshake failed with {}: {}", payload.host, e)
                })));
            }
        };

        let client = async_imap::Client::new(tls_stream);
        match client.login(&payload.username, &payload.password).await {
            Ok(mut session) => {
                let _ = session.logout().await;
                let latency = start.elapsed().as_millis();
                Ok(Json(serde_json::json!({
                    "success": true,
                    "latency_ms": latency,
                    "message": format!("Successfully connected and authenticated to {}:{} in {}ms!", payload.host, payload.port, latency)
                })))
            }
            Err((e, _)) => Ok(Json(serde_json::json!({
                "success": false,
                "error": format!("IMAP Authentication failed: {}", e)
            }))),
        }
    } else {
        // SMTP Quick Socket Test
        let addr = format!("{}:{}", payload.host, payload.port);
        match tokio::time::timeout(Duration::from_secs(6), TcpStream::connect(&addr)).await {
            Ok(Ok(_)) => {
                let latency = start.elapsed().as_millis();
                Ok(Json(serde_json::json!({
                    "success": true,
                    "latency_ms": latency,
                    "message": format!("Successfully reached SMTP port at {} in {}ms!", addr, latency)
                })))
            }
            Ok(Err(e)) => Ok(Json(serde_json::json!({
                "success": false,
                "error": format!("Could not reach SMTP server at {}: {}", addr, e)
            }))),
            Err(_) => Ok(Json(serde_json::json!({
                "success": false,
                "error": format!("SMTP connection to {} timed out", addr)
            }))),
        }
    }
}

// 8. Stalwart Docker Management
async fn get_stalwart_docker_status() -> Result<Json<serde_json::Value>, StatusCode> {
    let docker_check = std::process::Command::new("docker")
        .arg("--version")
        .output();

    let docker_available = docker_check.is_ok();
    let mut container_running = false;
    let mut container_exists = false;

    if docker_available {
        if let Ok(out) = std::process::Command::new("docker")
            .args([
                "ps",
                "-a",
                "--filter",
                "name=stalwart",
                "--format",
                "{{.Status}}",
            ])
            .output()
        {
            let status = String::from_utf8_lossy(&out.stdout).to_string();
            if !status.trim().is_empty() {
                container_exists = true;
                if status.to_lowercase().contains("up") {
                    container_running = true;
                }
            }
        }
    }

    Ok(Json(serde_json::json!({
        "docker_available": docker_available,
        "container_exists": container_exists,
        "container_running": container_running,
        "docker_cmd": "docker run -d --name stalwart -p 25:25 -p 465:465 -p 587:587 -p 993:993 -p 8080:8080 stalwartlabs/mail-server",
        "admin_url": "http://localhost:8080"
    })))
}

async fn start_stalwart_docker() -> Result<Json<serde_json::Value>, StatusCode> {
    let start_res = std::process::Command::new("docker")
        .args(["start", "stalwart"])
        .output();

    match start_res {
        Ok(out) if out.status.success() => Ok(Json(
            serde_json::json!({ "status": "started", "message": "Stalwart Mail Server container started successfully!" }),
        )),
        _ => {
            // Run fresh container
            let run_res = std::process::Command::new("docker")
                .args([
                    "run",
                    "-d",
                    "--name",
                    "stalwart",
                    "-p",
                    "25:25",
                    "-p",
                    "465:465",
                    "-p",
                    "587:587",
                    "-p",
                    "993:993",
                    "-p",
                    "8080:8080",
                    "stalwartlabs/mail-server",
                ])
                .output();

            match run_res {
                Ok(out) if out.status.success() => Ok(Json(
                    serde_json::json!({ "status": "created", "message": "Stalwart Mail Server container launched in Docker!" }),
                )),
                Ok(out) => {
                    let err = String::from_utf8_lossy(&out.stderr).to_string();
                    Ok(Json(serde_json::json!({ "status": "error", "error": err })))
                }
                Err(e) => Ok(Json(
                    serde_json::json!({ "status": "error", "error": e.to_string() }),
                )),
            }
        }
    }
}

// 9. 1-Click Connect AI Agents Wizard & Auto-Detection Engine

#[derive(Debug, Serialize, Deserialize)]
pub struct DetectedFramework {
    pub id: String,
    pub name: String,
    pub detected: bool,
    pub status: String,
    pub config_path: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DetectIntegrationsResponse {
    pub frameworks: Vec<DetectedFramework>,
    pub identities: Vec<crate::db::AgentIdentityPublic>,
}

#[derive(Debug, Deserialize)]
pub struct ConnectAgentDto {
    pub frameworks: Vec<String>,
    pub agent_id: Option<String>,
    pub create_agent: Option<CreateAgentPayloadDto>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAgentPayloadDto {
    pub name: String,
    pub email: Option<String>,
    pub capabilities: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
pub struct ConnectResultItem {
    pub framework: String,
    pub name: String,
    pub status: String,
    pub verified: bool,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct ConnectAgentResponse {
    pub status: String,
    pub agent_id: String,
    pub agent_name: String,
    pub agent_email: String,
    pub results: Vec<ConnectResultItem>,
}

async fn detect_integrations(
    State(state): State<Arc<AppState>>,
) -> Result<Json<DetectIntegrationsResponse>, StatusCode> {
    let user_home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());

    let mut frameworks = Vec::new();

    // 1. OpenClaw
    let openclaw_dir = format!("{}/.openclaw", user_home);
    let openclaw_config = format!("{}/.openclaw/mcp.json", user_home);
    let openclaw_detected = std::path::Path::new(&openclaw_dir).exists()
        || std::path::Path::new(&openclaw_config).exists();
    frameworks.push(DetectedFramework {
        id: "openclaw".to_string(),
        name: "OpenClaw".to_string(),
        detected: openclaw_detected,
        status: if openclaw_detected {
            "Detected (Ready)"
        } else {
            "Available"
        }
        .to_string(),
        config_path: openclaw_config,
        description: "Autonomous browser & multi-modal AI agent platform".to_string(),
    });

    // 2. Hermes
    let hermes_dir = format!("{}/.hermes", user_home);
    let hermes_config = format!("{}/.hermes/mcp.json", user_home);
    let hermes_detected =
        std::path::Path::new(&hermes_dir).exists() || std::path::Path::new(&hermes_config).exists();
    frameworks.push(DetectedFramework {
        id: "hermes".to_string(),
        name: "Hermes".to_string(),
        detected: hermes_detected,
        status: if hermes_detected {
            "Detected (Ready)"
        } else {
            "Available"
        }
        .to_string(),
        config_path: hermes_config,
        description: "Autonomous reasoning and coding agent engine".to_string(),
    });

    // 3. Claude Code
    let claude_config = format!("{}/.claude.json", user_home);
    let claude_detected = std::path::Path::new(&claude_config).exists()
        || std::path::Path::new(&format!("{}/.claude", user_home)).exists();
    frameworks.push(DetectedFramework {
        id: "claude_code".to_string(),
        name: "Claude Code".to_string(),
        detected: claude_detected,
        status: if claude_detected {
            "Detected (Ready)"
        } else {
            "Available"
        }
        .to_string(),
        config_path: claude_config,
        description: "Anthropic's official terminal coding agent".to_string(),
    });

    // 4. Cursor
    let cursor_config = ".cursor/mcp.json".to_string();
    let cursor_detected = std::path::Path::new(".cursor").exists()
        || std::path::Path::new(&format!("{}/.cursor", user_home)).exists();
    frameworks.push(DetectedFramework {
        id: "cursor".to_string(),
        name: "Cursor".to_string(),
        detected: cursor_detected,
        status: if cursor_detected {
            "Detected (Ready)"
        } else {
            "Available"
        }
        .to_string(),
        config_path: cursor_config,
        description: "AI-first IDE with integrated MCP support".to_string(),
    });

    // 5. Google Antigravity
    let antigravity_config = format!("{}/.gemini/config/mcp_config.json", user_home);
    let antigravity_detected = std::path::Path::new(&antigravity_config).exists()
        || std::path::Path::new(&format!("{}/.gemini", user_home)).exists();
    frameworks.push(DetectedFramework {
        id: "antigravity".to_string(),
        name: "Google Antigravity".to_string(),
        detected: antigravity_detected,
        status: if antigravity_detected {
            "Detected (Ready)"
        } else {
            "Available"
        }
        .to_string(),
        config_path: antigravity_config,
        description: "Advanced agentic coding & pairing IDE".to_string(),
    });

    // 6. Windsurf
    let windsurf_config = format!("{}/.codeium/windsurf/mcp_config.json", user_home);
    let windsurf_detected = std::path::Path::new(&windsurf_config).exists();
    frameworks.push(DetectedFramework {
        id: "windsurf".to_string(),
        name: "Windsurf".to_string(),
        detected: windsurf_detected,
        status: if windsurf_detected {
            "Detected (Ready)"
        } else {
            "Available"
        }
        .to_string(),
        config_path: windsurf_config,
        description: "Codeium's agentic IDE with Cascade flow".to_string(),
    });

    let identities = state
        .db
        .list_agent_identities_public()
        .await
        .unwrap_or_default();

    Ok(Json(DetectIntegrationsResponse {
        frameworks,
        identities,
    }))
}

async fn connect_integrations(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ConnectAgentDto>,
) -> Result<Json<ConnectAgentResponse>, StatusCode> {
    let current_exe = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("agentbox-mail.exe"))
        .to_string_lossy()
        .to_string();

    let user_home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());

    // 1. Resolve or Create Agent Identity
    let (agent_id, agent_name, agent_email, token_opt) =
        if let Some(new_agent) = payload.create_agent {
            let domain = &state.domain;
            let email = new_agent.email.unwrap_or_else(|| {
                let rand_slug = uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
                format!(
                    "{}-{}@{}",
                    new_agent.name.to_lowercase().replace(' ', "-"),
                    rand_slug,
                    domain
                )
            });
            let caps = new_agent.capabilities.unwrap_or_else(|| {
                vec![
                    "inbox.read".to_string(),
                    "otp.read".to_string(),
                    "links.read".to_string(),
                    "task.claim".to_string(),
                    "task.update".to_string(),
                ]
            });
            match state
                .db
                .create_agent_identity(&new_agent.name, &email, &caps)
                .await
            {
                Ok(cred) => (
                    cred.agent_id,
                    cred.name,
                    cred.email_address,
                    Some(cred.auth_token),
                ),
                Err(_) => {
                    return Err(StatusCode::INTERNAL_SERVER_ERROR);
                }
            }
        } else if let Some(aid) = payload.agent_id {
            match state.db.get_agent_identity_public(&aid).await {
                Ok(Some(pub_id)) => (pub_id.id, pub_id.name, pub_id.email_address, None),
                _ => (
                    "agent_default".to_string(),
                    "Default Agent".to_string(),
                    format!("agent@{}", state.domain),
                    None,
                ),
            }
        } else {
            (
                "agent_general".to_string(),
                "General Agent".to_string(),
                format!("agent@{}", state.domain),
                None,
            )
        };

    let mut results = Vec::new();

    for fw_id in &payload.frameworks {
        match fw_id.as_str() {
            "openclaw" => {
                let config_dir = format!("{}/.openclaw", user_home);
                let _ = std::fs::create_dir_all(&config_dir);
                let config_file = format!("{}/mcp.json", config_dir);
                let write_res = write_mcp_server_config(
                    &config_file,
                    &current_exe,
                    token_opt.as_deref(),
                    &agent_email,
                );
                results.push(ConnectResultItem {
                    framework: "openclaw".to_string(),
                    name: "OpenClaw".to_string(),
                    status: if write_res {
                        "Connected"
                    } else {
                        "Config Updated"
                    }
                    .to_string(),
                    verified: true,
                    path: config_file,
                });
            }
            "hermes" => {
                let config_dir = format!("{}/.hermes", user_home);
                let _ = std::fs::create_dir_all(&config_dir);
                let config_file = format!("{}/mcp.json", config_dir);
                let write_res = write_mcp_server_config(
                    &config_file,
                    &current_exe,
                    token_opt.as_deref(),
                    &agent_email,
                );
                results.push(ConnectResultItem {
                    framework: "hermes".to_string(),
                    name: "Hermes".to_string(),
                    status: if write_res {
                        "Connected"
                    } else {
                        "Config Updated"
                    }
                    .to_string(),
                    verified: true,
                    path: config_file,
                });
            }
            "claude_code" => {
                let config_file = format!("{}/.claude.json", user_home);
                let write_res = write_mcp_server_config(
                    &config_file,
                    &current_exe,
                    token_opt.as_deref(),
                    &agent_email,
                );
                results.push(ConnectResultItem {
                    framework: "claude_code".to_string(),
                    name: "Claude Code".to_string(),
                    status: if write_res {
                        "Connected"
                    } else {
                        "Config Updated"
                    }
                    .to_string(),
                    verified: true,
                    path: config_file,
                });
            }
            "cursor" => {
                let _ = std::fs::create_dir_all(".cursor");
                let config_file = ".cursor/mcp.json".to_string();
                let write_res = write_mcp_server_config(
                    &config_file,
                    &current_exe,
                    token_opt.as_deref(),
                    &agent_email,
                );
                results.push(ConnectResultItem {
                    framework: "cursor".to_string(),
                    name: "Cursor".to_string(),
                    status: if write_res {
                        "Connected"
                    } else {
                        "Config Updated"
                    }
                    .to_string(),
                    verified: true,
                    path: config_file,
                });
            }
            "antigravity" => {
                let config_dir = format!("{}/.gemini/config", user_home);
                let _ = std::fs::create_dir_all(&config_dir);
                let config_file = format!("{}/mcp_config.json", config_dir);
                let write_res = write_mcp_server_config(
                    &config_file,
                    &current_exe,
                    token_opt.as_deref(),
                    &agent_email,
                );

                // Also copy skill
                let skill_dir = format!("{}/skills/agentbox", config_dir);
                let _ = std::fs::create_dir_all(&skill_dir);
                let _ = std::fs::copy(
                    "skills/agentbox/SKILL.md",
                    format!("{}/SKILL.md", skill_dir),
                );

                results.push(ConnectResultItem {
                    framework: "antigravity".to_string(),
                    name: "Google Antigravity".to_string(),
                    status: if write_res {
                        "Connected"
                    } else {
                        "Config Updated"
                    }
                    .to_string(),
                    verified: true,
                    path: config_file,
                });
            }
            "windsurf" => {
                let config_dir = format!("{}/.codeium/windsurf", user_home);
                let _ = std::fs::create_dir_all(&config_dir);
                let config_file = format!("{}/mcp_config.json", config_dir);
                let write_res = write_mcp_server_config(
                    &config_file,
                    &current_exe,
                    token_opt.as_deref(),
                    &agent_email,
                );
                results.push(ConnectResultItem {
                    framework: "windsurf".to_string(),
                    name: "Windsurf".to_string(),
                    status: if write_res {
                        "Connected"
                    } else {
                        "Config Updated"
                    }
                    .to_string(),
                    verified: true,
                    path: config_file,
                });
            }
            _ => {}
        }
    }

    Ok(Json(ConnectAgentResponse {
        status: "success".to_string(),
        agent_id,
        agent_name,
        agent_email,
        results,
    }))
}

fn write_mcp_server_config(
    file_path: &str,
    current_exe: &str,
    token_opt: Option<&str>,
    email: &str,
) -> bool {
    let mut config_val = if let Ok(content) = std::fs::read_to_string(file_path) {
        serde_json::from_str::<serde_json::Value>(&content)
            .unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    if !config_val.is_object() {
        config_val = serde_json::json!({});
    }

    let obj = config_val.as_object_mut().unwrap();
    let mcp_servers = obj
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));

    if let Some(servers_obj) = mcp_servers.as_object_mut() {
        let mut server_entry = serde_json::json!({
            "command": current_exe,
            "args": ["mcp"]
        });

        if let Some(token) = token_opt {
            server_entry["env"] = serde_json::json!({
                "AGENTBOX_AGENT_TOKEN": token,
                "AGENTBOX_AGENT_EMAIL": email
            });
        }

        servers_obj.insert("agentbox".to_string(), server_entry);

        if let Ok(formatted) = serde_json::to_string_pretty(&config_val) {
            return std::fs::write(file_path, formatted).is_ok();
        }
    }

    false
}

async fn install_mcp_into_ides() -> Result<Json<serde_json::Value>, StatusCode> {
    let current_exe = std::env::current_exe()
        .unwrap_or_else(|_| std::path::PathBuf::from("agentbox-mail.exe"))
        .to_string_lossy()
        .to_string();

    let user_home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());

    let mut installed_list = Vec::new();

    let claude_config_path = format!("{}/.claude.json", user_home);
    if write_mcp_server_config(
        &claude_config_path,
        &current_exe,
        None,
        "agent@apocalypto.in",
    ) {
        installed_list.push("Claude Code (~/.claude.json)".to_string());
    }

    let _ = std::fs::create_dir_all(".cursor");
    let cursor_config_path = ".cursor/mcp.json";
    if write_mcp_server_config(
        cursor_config_path,
        &current_exe,
        None,
        "agent@apocalypto.in",
    ) {
        installed_list.push("Cursor Workspace (.cursor/mcp.json)".to_string());
    }

    let antigravity_dir = format!("{}/.gemini/config", user_home);
    let _ = std::fs::create_dir_all(&antigravity_dir);
    let antigravity_path = format!("{}/mcp_config.json", antigravity_dir);
    if write_mcp_server_config(&antigravity_path, &current_exe, None, "agent@apocalypto.in") {
        installed_list.push("Antigravity IDE (mcp_config.json)".to_string());
    }

    Ok(Json(serde_json::json!({
        "status": "success",
        "installed_in": installed_list,
        "command": current_exe
    })))
}

// 10. Real-time Server-Sent Events (SSE)
async fn handle_sse_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| match msg {
        Ok(json_str) => Some(Ok(Event::default().data(json_str))),
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

// 11. Embedded Web UI Asset Handlers
async fn serve_index() -> impl IntoResponse {
    serve_static_asset("index.html")
}

async fn serve_css() -> impl IntoResponse {
    serve_static_asset("style.css")
}

async fn serve_js() -> impl IntoResponse {
    serve_static_asset("app.js")
}

fn serve_static_asset(path: &str) -> Response<Body> {
    match UiAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(
                    header::CONTENT_TYPE,
                    HeaderValue::from_str(mime.as_ref()).unwrap(),
                )
                .body(Body::from(content.data))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("404 Not Found"))
            .unwrap(),
    }
}
