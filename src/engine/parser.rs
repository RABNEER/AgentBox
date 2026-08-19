use mail_parser::{Address, MessageParser, MimeHeaders};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedEmailData {
    pub from: String,
    pub to: Vec<String>,
    pub subject: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub raw_mime: String,
    pub attachments: Vec<ParsedAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedAttachment {
    pub filename: String,
    pub content_type: String,
    pub size_bytes: usize,
    pub data_base64: Option<String>,
}

pub struct EmailParser;

impl EmailParser {
    pub fn parse_mime(raw_bytes: &[u8]) -> Option<ParsedEmailData> {
        let raw_mime_str = String::from_utf8_lossy(raw_bytes).to_string();
        let parsed = MessageParser::default().parse(raw_bytes)?;

        // Extract sender
        let from = parsed
            .from()
            .and_then(|f| f.first())
            .map(|addr| {
                if let Some(name) = addr.name() {
                    if let Some(em) = addr.address() {
                        format!("{} <{}>", name, em)
                    } else {
                        name.to_string()
                    }
                } else {
                    addr.address().unwrap_or("unknown@sender").to_string()
                }
            })
            .unwrap_or_else(|| "unknown@sender".to_string());

        // Extract recipients
        let mut to = Vec::new();
        match parsed.to() {
            Some(Address::List(addrs)) => {
                for addr in addrs {
                    if let Some(em) = addr.address() {
                        to.push(em.to_string());
                    }
                }
            }
            Some(Address::Group(groups)) => {
                for g in groups {
                    for addr in &g.addresses {
                        if let Some(em) = addr.address() {
                            to.push(em.to_string());
                        }
                    }
                }
            }
            None => {}
        }

        // Extract subject
        let subject = parsed.subject().map(|s| s.to_string());

        // Extract bodies
        let body_text = parsed.body_text(0).map(|s| s.to_string());
        let body_html = parsed.body_html(0).map(|s| s.to_string());

        // Extract attachments
        let mut attachments = Vec::new();
        for att in parsed.attachments() {
            let filename = att.attachment_name().unwrap_or("attachment.bin").to_string();
            let content_type = att
                .content_type()
                .map(|c| c.c_type.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());
            let contents = att.contents();
            let size_bytes = contents.len();
            let data_base64 = Some(base64_encode(contents));

            attachments.push(ParsedAttachment {
                filename,
                content_type,
                size_bytes,
                data_base64,
            });
        }

        Some(ParsedEmailData {
            from,
            to,
            subject,
            body_text,
            body_html,
            raw_mime: raw_mime_str,
            attachments,
        })
    }
}

// Simple base64 encoding without extra heavy crates
fn base64_encode(data: &[u8]) -> String {
    const STANDARD: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::with_capacity((data.len() + 2) / 3 * 4);

    for chunk in data.chunks(3) {
        let b0 = chunk[0];
        let b1 = chunk.get(1).copied().unwrap_or(0);
        let b2 = chunk.get(2).copied().unwrap_or(0);

        let n = ((b0 as u32) << 16) | ((b1 as u32) << 8) | (b2 as u32);

        result.push(STANDARD[((n >> 18) & 63) as usize] as char);
        result.push(STANDARD[((n >> 12) & 63) as usize] as char);

        if chunk.len() > 1 {
            result.push(STANDARD[((n >> 6) & 63) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(STANDARD[(n & 63) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}
