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
    #[serde(rename = "task.dispatch", alias = "agentbox.task.dispatch")]
    TaskDispatch,
    #[serde(rename = "task.claim", alias = "agentbox.task.claim")]
    TaskClaim,
    #[serde(rename = "task.update", alias = "agentbox.task.update")]
    TaskUpdate,
    #[serde(rename = "task.read", alias = "agentbox.task.read")]
    TaskRead,
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
            Capability::TaskDispatch => "task.dispatch",
            Capability::TaskClaim => "task.claim",
            Capability::TaskUpdate => "task.update",
            Capability::TaskRead => "task.read",
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
            "task.dispatch" | "dispatch" => Some(Capability::TaskDispatch),
            "task.claim" | "claim" => Some(Capability::TaskClaim),
            "task.update" | "update" => Some(Capability::TaskUpdate),
            "task.read" => Some(Capability::TaskRead),
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
            Capability::TaskDispatch,
            Capability::TaskClaim,
            Capability::TaskUpdate,
            Capability::TaskRead,
        ]
    }

    pub fn standard_agent() -> Vec<Capability> {
        vec![
            Capability::InboxRead,
            Capability::InboxCreate,
            Capability::OtpRead,
            Capability::LinksRead,
            Capability::TaskDispatch,
            Capability::TaskClaim,
            Capability::TaskUpdate,
            Capability::TaskRead,
        ]
    }

    pub fn browser_qa_agent() -> Vec<Capability> {
        vec![
            Capability::InboxRead,
            Capability::OtpRead,
            Capability::LinksRead,
            Capability::TaskDispatch,
            Capability::TaskRead,
        ]
    }

    pub fn coding_agent() -> Vec<Capability> {
        vec![
            Capability::InboxRead,
            Capability::OtpRead,
            Capability::LinksRead,
            Capability::EmailSend,
            Capability::TaskClaim,
            Capability::TaskUpdate,
            Capability::TaskRead,
        ]
    }
}

pub struct ScopeValidator;

impl ScopeValidator {
    pub fn has_capability(assigned_capabilities: &[String], required: Capability) -> bool {
        let req_str = required.as_str();

        for cap in assigned_capabilities {
            let clean = cap.trim().trim_start_matches("agentbox.").to_lowercase();
            // 1. Wildcard / Root permissions
            if clean == "*" || clean == "all" || clean == "admin" || clean == "identity.manage" {
                return true;
            }

            // 2. Exact match
            if clean == req_str {
                return true;
            }

            // 3. Category wildcard match (e.g., "inbox.*", "task.*")
            if let Some(prefix) = req_str.split('.').next() {
                if clean == format!("{}.*", prefix) {
                    return true;
                }
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capability_parsing_and_checking() {
        let caps = vec![
            "inbox.read".to_string(),
            "otp.read".to_string(),
            "task.*".to_string(),
        ];

        assert!(ScopeValidator::has_capability(&caps, Capability::InboxRead));
        assert!(ScopeValidator::has_capability(&caps, Capability::OtpRead));
        assert!(ScopeValidator::has_capability(
            &caps,
            Capability::TaskDispatch
        ));
        assert!(ScopeValidator::has_capability(&caps, Capability::TaskClaim));
        assert!(!ScopeValidator::has_capability(
            &caps,
            Capability::EmailSend
        ));
    }

    #[test]
    fn test_wildcard_permissions() {
        let admin_caps = vec!["*".to_string()];
        assert!(ScopeValidator::has_capability(
            &admin_caps,
            Capability::EmailSend
        ));
        assert!(ScopeValidator::has_capability(
            &admin_caps,
            Capability::InboxDelete
        ));
        assert!(ScopeValidator::has_capability(
            &admin_caps,
            Capability::TaskUpdate
        ));
    }
}
