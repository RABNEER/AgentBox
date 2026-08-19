use crate::db::{Database, Message};
use crate::engine::{EmailParser, Extractor};
use chrono::Utc;
use rustls_pki_types::ServerName;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::sync::broadcast;
use tokio_rustls::TlsConnector;
use tracing::{info, warn};
use uuid::Uuid;

pub struct ImapSyncWorker {
    pub db: Database,
    pub tx: broadcast::Sender<String>,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub poll_interval_secs: u64,
}

impl ImapSyncWorker {
    pub fn new(
        db: Database,
        tx: broadcast::Sender<String>,
        host: String,
        port: u16,
        username: String,
        password: String,
    ) -> Self {
        Self {
            db,
            tx,
            host,
            port,
            username,
            password,
            poll_interval_secs: 5,
        }
    }

    pub async fn start(self: Arc<Self>) {
        info!(
            "Hostinger IMAP Live Sync worker active for {} on {}:993 (Listening to main domain & all aliases)",
            self.username, self.host
        );

        loop {
            if let Err(e) = self.sync_once().await {
                warn!("IMAP sync connection error (will retry in 10s): {}", e);
                tokio::time::sleep(Duration::from_secs(10)).await;
            } else {
                tokio::time::sleep(Duration::from_secs(self.poll_interval_secs)).await;
            }
        }
    }

    async fn sync_once(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 1. Setup TLS connection to imap.hostinger.com:993
        let root_store = rustls::RootCertStore {
            roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
        };

        let config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_no_client_auth();

        let connector = TlsConnector::from(Arc::new(config));
        let addr = format!("{}:{}", self.host, self.port);
        let tcp_stream = TcpStream::connect(&addr).await?;

        let server_name = ServerName::try_from(self.host.as_str())?.to_owned();
        let tls_stream = connector.connect(server_name, tcp_stream).await?;

        // 2. Connect IMAP client
        let client = async_imap::Client::new(tls_stream);
        let mut session = client
            .login(&self.username, &self.password)
            .await
            .map_err(|(e, _)| format!("IMAP login failed: {}", e))?;

        session.select("INBOX").await?;

        // 3. Search for unseen messages
        let unseen_uids = session.search("UNSEEN").await?;

        if !unseen_uids.is_empty() {
            info!(
                "Found {} new unseen messages in Hostinger inbox!",
                unseen_uids.len()
            );

            for uid in unseen_uids {
                let mut messages_stream = session.fetch(uid.to_string(), "RFC822").await?;

                use tokio_stream::StreamExt;
                while let Some(msg_res) = messages_stream.next().await {
                    if let Ok(msg) = msg_res {
                        if let Some(body_bytes) = msg.body() {
                            self.process_incoming_bytes(body_bytes).await?;
                        }
                    }
                }
            }
        }

        // Logout cleanly
        let _ = session.logout().await;
        Ok(())
    }

    async fn process_incoming_bytes(
        &self,
        raw_bytes: &[u8],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let now = Utc::now().timestamp();
        let msg_id = format!("msg_{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);

        if let Some(parsed) = EmailParser::parse_mime(raw_bytes) {
            let to_addr = parsed
                .to
                .first()
                .cloned()
                .unwrap_or_else(|| self.username.clone());

            // Extract OTP & Links
            let extracted = Extractor::extract(
                parsed.subject.as_deref(),
                parsed.body_text.as_deref(),
                parsed.body_html.as_deref(),
            );

            // Auto-provision or match account in database
            let account = match self.db.get_account_by_address(&to_addr).await {
                Ok(Some(acc)) => acc,
                _ => {
                    self.db
                        .create_account(&to_addr, Some("Live Hostinger Inbox"))
                        .await?
                }
            };

            let links_json = serde_json::to_string(&extracted.action_links).ok();

            let message = Message {
                id: msg_id.clone(),
                account_id: account.id.clone(),
                from_address: parsed.from.clone(),
                to_address: to_addr.clone(),
                subject: parsed.subject.clone(),
                body_text: parsed.body_text,
                body_html: parsed.body_html,
                raw_mime: String::from_utf8(raw_bytes.to_vec()).ok(),
                extracted_otp: extracted.otp.clone(),
                extracted_links: links_json,
                direction: "inbound".to_string(),
                created_at: now,
            };

            self.db.insert_message(&message).await?;
            info!(
                "📥 Ingested Live Email for '{}' from {} -> OTP: {:?}",
                to_addr, parsed.from, extracted.otp
            );

            // Broadcast to Web Dashboard & AI Agents via SSE
            let event_payload = serde_json::json!({
                "type": "new_message",
                "message": message
            });
            let _ = self.tx.send(event_payload.to_string());
        }

        Ok(())
    }
}
