use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, Pool, Sqlite};
use std::error::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Account {
    pub id: String,
    pub address: String,
    pub display_name: Option<String>,
    pub owner_agent_id: Option<String>,
    pub status: String,
    pub created_at: i64,
}

/// Public Agent Identity (Safe for serialization, NEVER exposes auth token)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentIdentityPublic {
    pub id: String,
    pub name: String,
    pub email_address: String,
    pub capabilities: String,
    pub status: String,
    pub created_at: i64,
    pub last_active_at: i64,
}

/// Internal Agent Identity with token for auth validation
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentIdentity {
    pub id: String,
    pub name: String,
    pub email_address: String,
    pub capabilities: String,
    pub token: String,
    pub status: String,
    pub created_at: i64,
    pub last_active_at: i64,
}

impl AgentIdentity {
    pub fn to_public(&self) -> AgentIdentityPublic {
        AgentIdentityPublic {
            id: self.id.clone(),
            name: self.name.clone(),
            email_address: self.email_address.clone(),
            capabilities: self.capabilities.clone(),
            status: self.status.clone(),
            created_at: self.created_at,
            last_active_at: self.last_active_at,
        }
    }
}

/// One-time credential returned only upon creation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCredentialResponse {
    pub agent_id: String,
    pub name: String,
    pub email_address: String,
    pub capabilities: Vec<String>,
    pub auth_token: String,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Message {
    pub id: String,
    pub account_id: String,
    pub from_address: String,
    pub to_address: String,
    pub subject: Option<String>,
    pub body_text: Option<String>,
    pub body_html: Option<String>,
    pub raw_mime: Option<String>,
    pub extracted_otp: Option<String>,
    pub extracted_links: Option<String>,
    pub direction: String, // "inbound" or "outbound"
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[allow(dead_code)]
pub struct Attachment {
    pub id: String,
    pub message_id: String,
    pub filename: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub data_base64: Option<String>,
}

#[derive(Clone)]
pub struct Database {
    pub pool: Pool<Sqlite>,
}

impl Database {
    pub async fn init(database_url: &str) -> Result<Self, Box<dyn Error + Send + Sync>> {
        // Ensure sqlite connection
        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;

        // Run base table migrations
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                address TEXT UNIQUE NOT NULL,
                display_name TEXT,
                owner_agent_id TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS agent_identities (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                email_address TEXT UNIQUE NOT NULL,
                capabilities TEXT NOT NULL,
                token TEXT UNIQUE NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                created_at INTEGER NOT NULL,
                last_active_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                account_id TEXT NOT NULL,
                from_address TEXT NOT NULL,
                to_address TEXT NOT NULL,
                subject TEXT,
                body_text TEXT,
                body_html TEXT,
                raw_mime TEXT,
                extracted_otp TEXT,
                extracted_links TEXT,
                direction TEXT NOT NULL DEFAULT 'inbound',
                created_at INTEGER NOT NULL,
                FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS attachments (
                id TEXT PRIMARY KEY,
                message_id TEXT NOT NULL,
                filename TEXT NOT NULL,
                content_type TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                data_base64 TEXT,
                FOREIGN KEY (message_id) REFERENCES messages(id) ON DELETE CASCADE
            );
            "#,
        )
        .execute(&pool)
        .await?;

        // Add owner_agent_id column if upgrading from older schema
        let _ = sqlx::query("ALTER TABLE accounts ADD COLUMN owner_agent_id TEXT")
            .execute(&pool)
            .await;

        // Create indexes safely after columns exist
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_messages_account ON messages(account_id);
            CREATE INDEX IF NOT EXISTS idx_messages_created ON messages(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_accounts_address ON accounts(address);
            CREATE INDEX IF NOT EXISTS idx_accounts_owner ON accounts(owner_agent_id);
            CREATE INDEX IF NOT EXISTS idx_agent_identities_token ON agent_identities(token);
            CREATE INDEX IF NOT EXISTS idx_agent_identities_email ON agent_identities(email_address);
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    pub async fn create_account(
        &self,
        address: &str,
        display_name: Option<&str>,
    ) -> Result<Account, sqlx::Error> {
        self.create_account_with_owner(address, display_name, None)
            .await
    }

    pub async fn create_account_with_owner(
        &self,
        address: &str,
        display_name: Option<&str>,
        owner_agent_id: Option<&str>,
    ) -> Result<Account, sqlx::Error> {
        let id = format!("acc_{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);
        let now = Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT INTO accounts (id, address, display_name, owner_agent_id, status, created_at)
            VALUES (?, ?, ?, ?, 'active', ?)
            "#,
        )
        .bind(&id)
        .bind(address)
        .bind(display_name)
        .bind(owner_agent_id)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(Account {
            id,
            address: address.to_string(),
            display_name: display_name.map(|s| s.to_string()),
            owner_agent_id: owner_agent_id.map(|s| s.to_string()),
            status: "active".to_string(),
            created_at: now,
        })
    }

    pub async fn list_accounts(&self) -> Result<Vec<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>(
            "SELECT id, address, display_name, owner_agent_id, status, created_at FROM accounts ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_account_by_id(&self, id: &str) -> Result<Option<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>(
            "SELECT id, address, display_name, owner_agent_id, status, created_at FROM accounts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn get_account_by_address(
        &self,
        address: &str,
    ) -> Result<Option<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>(
            "SELECT id, address, display_name, owner_agent_id, status, created_at FROM accounts WHERE address = ?",
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn delete_account(&self, account_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM messages WHERE account_id = ?")
            .bind(account_id)
            .execute(&self.pool)
            .await?;

        sqlx::query("DELETE FROM accounts WHERE id = ?")
            .bind(account_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    // =========================================================================
    // Agent Identity & Resource Ownership Management
    // =========================================================================

    pub async fn create_agent_identity(
        &self,
        name: &str,
        email_address: &str,
        capabilities: &[String],
    ) -> Result<AgentCredentialResponse, sqlx::Error> {
        let rand_slug = Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
        let id = format!(
            "agent_{}_{}",
            name.to_lowercase().replace(' ', "-"),
            rand_slug
        );
        let token = format!("agb_{}", Uuid::new_v4().to_string().replace('-', ""));
        let now = Utc::now().timestamp();
        let caps_json = serde_json::to_string(capabilities).unwrap_or_else(|_| "[]".to_string());

        // Ensure a dedicated mailbox exists with owner_agent_id set
        let _ = self
            .create_account_with_owner(email_address, Some(&format!("Agent: {}", name)), Some(&id))
            .await;

        sqlx::query(
            r#"
            INSERT INTO agent_identities (id, name, email_address, capabilities, token, status, created_at, last_active_at)
            VALUES (?, ?, ?, ?, ?, 'active', ?, ?)
            "#,
        )
        .bind(&id)
        .bind(name)
        .bind(email_address)
        .bind(&caps_json)
        .bind(&token)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(AgentCredentialResponse {
            agent_id: id,
            name: name.to_string(),
            email_address: email_address.to_string(),
            capabilities: capabilities.to_vec(),
            auth_token: token,
            note: "Store this auth_token securely. It is only displayed once upon creation and cannot be retrieved again.".to_string(),
        })
    }

    pub async fn get_agent_identity(&self, id: &str) -> Result<Option<AgentIdentity>, sqlx::Error> {
        sqlx::query_as::<_, AgentIdentity>(
            "SELECT id, name, email_address, capabilities, token, status, created_at, last_active_at FROM agent_identities WHERE id = ? OR name = ?",
        )
        .bind(id)
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn get_agent_identity_public(
        &self,
        id: &str,
    ) -> Result<Option<AgentIdentityPublic>, sqlx::Error> {
        let internal = self.get_agent_identity(id).await?;
        Ok(internal.map(|i| i.to_public()))
    }

    pub async fn get_agent_identity_by_token(
        &self,
        token: &str,
    ) -> Result<Option<AgentIdentity>, sqlx::Error> {
        sqlx::query_as::<_, AgentIdentity>(
            "SELECT id, name, email_address, capabilities, token, status, created_at, last_active_at FROM agent_identities WHERE token = ?",
        )
        .bind(token)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn list_agent_identities_public(
        &self,
    ) -> Result<Vec<AgentIdentityPublic>, sqlx::Error> {
        let identities = sqlx::query_as::<_, AgentIdentity>(
            "SELECT id, name, email_address, capabilities, token, status, created_at, last_active_at FROM agent_identities ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(identities.into_iter().map(|i| i.to_public()).collect())
    }

    pub async fn revoke_agent_identity(&self, id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE agent_identities SET status = 'revoked' WHERE id = ? OR name = ?")
            .bind(id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Verifies if an Agent owns a specific account/mailbox or if it's the agent's primary mailbox
    pub async fn verify_resource_ownership(
        &self,
        agent: &AgentIdentity,
        account_id: &str,
    ) -> Result<bool, sqlx::Error> {
        let account = match self.get_account_by_id(account_id).await? {
            Some(acc) => acc,
            None => return Ok(false),
        };

        // 1. Direct owner_agent_id match
        if account.owner_agent_id.as_deref() == Some(&agent.id) {
            return Ok(true);
        }

        // 2. Email address match
        if account.address.to_lowercase() == agent.email_address.to_lowercase() {
            return Ok(true);
        }

        // 3. If account has no owner (legacy/root mailbox), grant access only if agent has wildcard/admin capability
        if account.owner_agent_id.is_none() {
            let caps: Vec<String> = serde_json::from_str(&agent.capabilities).unwrap_or_default();
            if caps.iter().any(|c| c == "*" || c == "admin" || c == "all") {
                return Ok(true);
            }
        }

        Ok(false)
    }

    // =========================================================================
    // Message Ingestion & Queries
    // =========================================================================

    pub async fn insert_message(&self, msg: &Message) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            INSERT INTO messages (
                id, account_id, from_address, to_address, subject,
                body_text, body_html, raw_mime, extracted_otp,
                extracted_links, direction, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&msg.id)
        .bind(&msg.account_id)
        .bind(&msg.from_address)
        .bind(&msg.to_address)
        .bind(&msg.subject)
        .bind(&msg.body_text)
        .bind(&msg.body_html)
        .bind(&msg.raw_mime)
        .bind(&msg.extracted_otp)
        .bind(&msg.extracted_links)
        .bind(&msg.direction)
        .bind(msg.created_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_messages_for_account(
        &self,
        account_id: &str,
    ) -> Result<Vec<Message>, sqlx::Error> {
        sqlx::query_as::<_, Message>(
            "SELECT * FROM messages WHERE account_id = ? ORDER BY created_at DESC",
        )
        .bind(account_id)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_message(&self, message_id: &str) -> Result<Option<Message>, sqlx::Error> {
        sqlx::query_as::<_, Message>("SELECT * FROM messages WHERE id = ?")
            .bind(message_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn delete_message(&self, message_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM messages WHERE id = ?")
            .bind(message_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_latest_otp(&self, account_id: &str) -> Result<Option<String>, sqlx::Error> {
        let row = sqlx::query_as::<_, Message>(
            "SELECT * FROM messages WHERE account_id = ? AND extracted_otp IS NOT NULL ORDER BY created_at DESC LIMIT 1",
        )
        .bind(account_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.and_then(|m| m.extracted_otp))
    }

    #[allow(dead_code)]
    pub async fn purge_expired_otps(&self, max_age_seconds: i64) -> Result<u64, sqlx::Error> {
        let cutoff = Utc::now().timestamp() - max_age_seconds;
        let res = sqlx::query("UPDATE messages SET extracted_otp = NULL WHERE created_at < ? AND extracted_otp IS NOT NULL")
            .bind(cutoff)
            .execute(&self.pool)
            .await?;
        Ok(res.rows_affected())
    }
}
