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
    pub status: String,
    pub created_at: i64,
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

        // Run migrations
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS accounts (
                id TEXT PRIMARY KEY,
                address TEXT UNIQUE NOT NULL,
                display_name TEXT,
                status TEXT NOT NULL DEFAULT 'active',
                created_at INTEGER NOT NULL
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

            CREATE INDEX IF NOT EXISTS idx_messages_account ON messages(account_id);
            CREATE INDEX IF NOT EXISTS idx_messages_created ON messages(created_at DESC);
            CREATE INDEX IF NOT EXISTS idx_accounts_address ON accounts(address);
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
        let id = format!("acc_{}", Uuid::new_v4().to_string().replace('-', "")[..12].to_string());
        let now = Utc::now().timestamp();

        sqlx::query(
            r#"
            INSERT INTO accounts (id, address, display_name, status, created_at)
            VALUES (?, ?, ?, 'active', ?)
            "#,
        )
        .bind(&id)
        .bind(address)
        .bind(display_name)
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(Account {
            id,
            address: address.to_string(),
            display_name: display_name.map(|s| s.to_string()),
            status: "active".to_string(),
            created_at: now,
        })
    }

    pub async fn list_accounts(&self) -> Result<Vec<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>(
            "SELECT id, address, display_name, status, created_at FROM accounts ORDER BY created_at DESC",
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn get_account_by_id(&self, id: &str) -> Result<Option<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>(
            "SELECT id, address, display_name, status, created_at FROM accounts WHERE id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn get_account_by_address(&self, address: &str) -> Result<Option<Account>, sqlx::Error> {
        sqlx::query_as::<_, Account>(
            "SELECT id, address, display_name, status, created_at FROM accounts WHERE address = ?",
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
}
