use crate::db::{Database, Message};
use crate::engine::{EmailParser, Extractor};
use chrono::Utc;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct SmtpServer {
    pub db: Database,
    pub tx: broadcast::Sender<String>,
    pub domain: String,
    pub port: u16,
}

impl SmtpServer {
    pub fn new(db: Database, tx: broadcast::Sender<String>, domain: String, port: u16) -> Self {
        Self {
            db,
            tx,
            domain,
            port,
        }
    }

    pub async fn start(self: Arc<Self>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = format!("0.0.0.0:{}", self.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("Raw SMTP Inbound Server listening on {}", addr);

        loop {
            match listener.accept().await {
                Ok((stream, peer)) => {
                    info!("Incoming SMTP connection from {}", peer);
                    let server = Arc::clone(&self);
                    tokio::spawn(async move {
                        if let Err(e) = server.handle_connection(stream).await {
                            warn!("SMTP session ended with error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    error!("SMTP listener error: {}", e);
                }
            }
        }
    }

    async fn handle_connection(
        &self,
        mut stream: TcpStream,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (reader, mut writer) = stream.split();
        let mut buf_reader = BufReader::new(reader);

        // Send Initial 220 Greeting
        writer
            .write_all(format!("220 {} ESMTP AgentBox Mail Ready\r\n", self.domain).as_bytes())
            .await?;
        writer.flush().await?;

        let mut from_address = String::new();
        let mut to_addresses = Vec::new();
        let mut in_data_mode = false;
        let mut data_buffer = String::new();

        let mut line = String::new();
        while buf_reader.read_line(&mut line).await? > 0 {
            let trimmed = line.trim();

            if in_data_mode {
                if trimmed == "." {
                    // End of DATA stream
                    in_data_mode = false;
                    let msg_id =
                        format!("msg_{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);

                    self.process_raw_email(&msg_id, &from_address, &to_addresses, &data_buffer)
                        .await?;

                    writer
                        .write_all(format!("250 2.0.0 Ok: queued as {}\r\n", msg_id).as_bytes())
                        .await?;
                    writer.flush().await?;

                    // Reset session state for next transaction
                    from_address.clear();
                    to_addresses.clear();
                    data_buffer.clear();
                } else {
                    data_buffer.push_str(&line);
                }
                line.clear();
                continue;
            }

            let upper = trimmed.to_uppercase();

            if upper.starts_with("EHLO") || upper.starts_with("HELO") {
                writer
                    .write_all(
                        format!(
                            "250-{}\r\n250-8BITMIME\r\n250-SMTPUTF8\r\n250 OK\r\n",
                            self.domain
                        )
                        .as_bytes(),
                    )
                    .await?;
            } else if upper.starts_with("MAIL FROM:") {
                let raw_from = trimmed[10..].trim();
                from_address = raw_from.trim_matches(|c| c == '<' || c == '>').to_string();
                writer.write_all(b"250 2.1.0 Ok\r\n").await?;
            } else if upper.starts_with("RCPT TO:") {
                let raw_to = trimmed[8..].trim();
                let clean_to = raw_to.trim_matches(|c| c == '<' || c == '>').to_string();
                to_addresses.push(clean_to);
                writer.write_all(b"250 2.1.5 Ok\r\n").await?;
            } else if upper == "DATA" {
                if to_addresses.is_empty() {
                    writer
                        .write_all(b"503 5.5.1 Error: need RCPT command\r\n")
                        .await?;
                } else {
                    in_data_mode = true;
                    data_buffer.clear();
                    writer
                        .write_all(b"354 End data with <CR><LF>.<CR><LF>\r\n")
                        .await?;
                }
            } else if upper == "RSET" {
                from_address.clear();
                to_addresses.clear();
                data_buffer.clear();
                in_data_mode = false;
                writer.write_all(b"250 2.0.0 Ok\r\n").await?;
            } else if upper == "NOOP" {
                writer.write_all(b"250 2.0.0 Ok\r\n").await?;
            } else if upper == "QUIT" {
                writer.write_all(b"221 2.0.0 Bye\r\n").await?;
                writer.flush().await?;
                break;
            } else {
                writer
                    .write_all(b"502 5.5.2 Error: command not recognized\r\n")
                    .await?;
            }

            writer.flush().await?;
            line.clear();
        }

        Ok(())
    }

    async fn process_raw_email(
        &self,
        msg_id: &str,
        from_address: &str,
        to_addresses: &[String],
        raw_mime: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now().timestamp();
        let target_to = to_addresses
            .first()
            .cloned()
            .unwrap_or_else(|| format!("agent@{}", self.domain));

        let (from_parsed, to_parsed, subject, body_text, body_html) =
            if let Some(parsed) = EmailParser::parse_mime(raw_mime.as_bytes()) {
                let to = parsed.to.first().cloned().unwrap_or(target_to.clone());
                (
                    parsed.from,
                    to,
                    parsed.subject,
                    parsed.body_text,
                    parsed.body_html,
                )
            } else {
                (
                    from_address.to_string(),
                    target_to.clone(),
                    Some("Inbound SMTP Email".to_string()),
                    Some(raw_mime.to_string()),
                    None,
                )
            };

        // Extract OTP and Links
        let extracted = Extractor::extract(
            subject.as_deref(),
            body_text.as_deref(),
            body_html.as_deref(),
        );

        // Find or auto-provision matching account
        let account = match self.db.get_account_by_address(&to_parsed).await {
            Ok(Some(acc)) => acc,
            _ => {
                self.db
                    .create_account(&to_parsed, Some("Auto-Provisioned SMTP Inbox"))
                    .await?
            }
        };

        let links_json = serde_json::to_string(&extracted.action_links).ok();

        let message = Message {
            id: msg_id.to_string(),
            account_id: account.id.clone(),
            from_address: from_parsed,
            to_address: to_parsed,
            subject,
            body_text,
            body_html,
            raw_mime: Some(raw_mime.to_string()),
            extracted_otp: extracted.otp.clone(),
            extracted_links: links_json,
            direction: "inbound".to_string(),
            created_at: now,
        };

        self.db.insert_message(&message).await?;
        info!(
            "Ingested SMTP Email {} -> OTP: {:?}, Links: {}",
            message.id,
            extracted.otp,
            extracted.action_links.len()
        );

        // Broadcast to SSE
        let event_payload = serde_json::json!({
            "type": "new_message",
            "message": message
        });
        let _ = self.tx.send(event_payload.to_string());

        Ok(())
    }
}
