use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Capability {
    #[serde(rename = "inbox.read", alias = "agentbox.inbox.read")]
    InboxRead,
    #[serde(rename = "inbox.create", alias = "agentbox.inbox.create")]
    InboxCreate,
    #[serde(rename = "inbox.delete", alias = "agentbox.inbox.delete")]
    InboxDelete,
    #[serde(rename = "otp.read", alias = "agentbox.otp.read")]
    OtpRead,
    #[serde(rename = "links.read", alias = "agentbox.links.read")]
    LinksRead,
    #[serde(rename = "email.send", alias = "agentbox.email.send")]
    EmailSend,
    #[serde(rename = "identity.manage", alias = "agentbox.identity.manage")]
    IdentityManage,
}

#[allow(dead_code)]
impl Capability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Capability::InboxRead => "inbox.read",
            Capability::InboxCreate => "inbox.create",
            Capability::InboxDelete => "inbox.delete",
            Capability::OtpRead => "otp.read",
            Capability::LinksRead => "links.read",
            Capability::EmailSend => "email.send",
            Capability::IdentityManage => "identity.manage",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let clean = s.trim().trim_start_matches("agentbox.").to_lowercase();
        match clean.as_str() {
            "inbox.read" | "read" => Some(Capability::InboxRead),
            "inbox.create" | "create" => Some(Capability::InboxCreate),
            "inbox.delete" | "delete" => Some(Capability::InboxDelete),
            "otp.read" | "otp" => Some(Capability::OtpRead),
            "links.read" | "links" => Some(Capability::LinksRead),
            "email.send" | "send" => Some(Capability::EmailSend),
            "identity.manage" | "identity" | "admin" => Some(Capability::IdentityManage),
            _ => None,
        }
    }

    pub fn all() -> Vec<Capability> {
        vec![
            Capability::InboxRead,
            Capability::InboxCreate,
            Capability::InboxDelete,
            Capability::OtpRead,
            Capability::LinksRead,
            Capability::EmailSend,
            Capability::IdentityManage,
        ]
    }

    pub fn standard_agent() -> Vec<Capability> {
        vec![
            Capability::InboxRead,
            Capability::InboxCreate,
            Capability::OtpRead,
            Capability::LinksRead,
        ]
    }

    pub fn browser_qa_agent() -> Vec<Capability> {
        vec![
            Capability::InboxRead,
            Capability::OtpRead,
            Capability::LinksRead,
        ]
    }
}

/// Scope validator for MCP tools and API callers
#[derive(Debug, Clone)]
pub struct ScopeValidator;

impl ScopeValidator {
    pub fn has_capability(allowed_caps: &[String], required: Capability) -> bool {
        // If wildcard or admin present, grant all
        if allowed_caps.iter().any(|c| c == "*" || c == "admin" || c == "all") {
            return true;
        }

        let req_str = required.as_str();
        allowed_caps.iter().any(|c| {
            let clean = c.trim().trim_start_matches("agentbox.");
            clean == req_str
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_parsing_and_checking() {
        let caps = vec!["inbox.read".to_string(), "otp.read".to_string()];
        assert!(ScopeValidator::has_capability(&caps, Capability::OtpRead));
        assert!(ScopeValidator::has_capability(&caps, Capability::InboxRead));
        assert!(!ScopeValidator::has_capability(&caps, Capability::EmailSend));
        assert!(!ScopeValidator::has_capability(&caps, Capability::InboxDelete));
    }

    #[test]
    fn test_wildcard_permissions() {
        let caps = vec!["*".to_string()];
        assert!(ScopeValidator::has_capability(&caps, Capability::EmailSend));
        assert!(ScopeValidator::has_capability(&caps, Capability::IdentityManage));
    }
}
