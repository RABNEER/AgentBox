use lettre::{
    message::{header::ContentType, Mailbox, Message as LettreMessage},
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendEmailRequest {
    pub from: Option<String>,
    pub to: Vec<String>,
    pub cc: Option<Vec<String>>,
    pub bcc: Option<Vec<String>>,
    pub subject: String,
    pub text: Option<String>,
    pub html: Option<String>,
    pub in_reply_to: Option<String>,
}

#[derive(Clone)]
pub struct OutboundMailer {
    pub smtp_host: Option<String>,
    pub smtp_port: u16,
    pub smtp_user: Option<String>,
    pub smtp_pass: Option<String>,
}

impl OutboundMailer {
    pub fn new(
        smtp_host: Option<String>,
        smtp_port: u16,
        smtp_user: Option<String>,
        smtp_pass: Option<String>,
    ) -> Self {
        Self {
            smtp_host,
            smtp_port,
            smtp_user,
            smtp_pass,
        }
    }

    pub async fn send_email(
        &self,
        req: SendEmailRequest,
    ) -> Result<String, Box<dyn Error + Send + Sync>> {
        let from_addr = req.from.unwrap_or_else(|| "agent@agentbox.io".to_string());

        let from_mailbox: Mailbox = from_addr.parse()?;
        let mut builder = LettreMessage::builder()
            .from(from_mailbox)
            .subject(&req.subject);

        for recipient in &req.to {
            let to_mailbox: Mailbox = recipient.parse()?;
            builder = builder.to(to_mailbox);
        }

        if let Some(cc_list) = &req.cc {
            for cc_addr in cc_list {
                let cc_mailbox: Mailbox = cc_addr.parse()?;
                builder = builder.cc(cc_mailbox);
            }
        }

        let body_content = if let Some(html) = req.html {
            builder = builder.header(ContentType::TEXT_HTML);
            html
        } else {
            builder = builder.header(ContentType::TEXT_PLAIN);
            req.text.unwrap_or_default()
        };

        let email = builder.body(body_content)?;

        // If SMTP credentials are provided, send via SMTP server
        if let Some(host) = &self.smtp_host {
            let mut transport_builder =
                AsyncSmtpTransport::<Tokio1Executor>::relay(host)?.port(self.smtp_port);

            if let (Some(u), Some(p)) = (&self.smtp_user, &self.smtp_pass) {
                let creds = Credentials::new(u.clone(), p.clone());
                transport_builder = transport_builder.credentials(creds);
            }

            let mailer = transport_builder.build();
            mailer.send(email).await?;
            Ok("Dispatched via SMTP".to_string())
        } else {
            // Local mock dispatch for dev / testing
            tracing::info!(
                "Outbound email queued/simulated: Subject '{}' to {:?}",
                req.subject,
                req.to
            );
            Ok("Dispatched (Mock/Dev Mode)".to_string())
        }
    }
}
