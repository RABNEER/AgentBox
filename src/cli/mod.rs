use clap::{Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "agentbox")]
#[command(author = "AgentBox Team")]
#[command(version = "0.1.0")]
#[command(about = "High-performance autonomous AI Agent Mailbox & Automation Engine", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the AgentBox Mail REST API, Raw SMTP Inbound Server, and Web Dashboard
    Server(ServerArgs),
    /// Start the Model Context Protocol (MCP) server over stdio for AI assistants
    Mcp(McpArgs),
    /// Create a new virtual agent inbox
    Create(CreateArgs),
    /// List all active virtual inboxes
    List(ListArgs),
    /// Fetch the latest extracted OTP verification code for an inbox
    Otp(OtpArgs),
}

#[derive(Args, Debug)]
pub struct ServerArgs {
    /// Port for Web Dashboard & REST API
    #[arg(short, long, default_value = "3000")]
    pub port: u16,

    /// Host address to bind to
    #[arg(long, default_value = "0.0.0.0")]
    pub host: String,

    /// Port for Raw Inbound SMTP Server (Direct Internet Receiving)
    #[arg(long, default_value = "2525")]
    pub smtp_inbound_port: u16,

    /// Disable the embedded raw SMTP server
    #[arg(long, default_value = "false")]
    pub no_smtp: bool,

    /// SQLite database URL
    #[arg(long, default_value = "sqlite://agentbox.db?mode=rwc")]
    pub db: String,

    /// Email domain for agent inboxes
    #[arg(long, default_value = "agentbox.io")]
    pub domain: String,

    /// Optional SMTP Host for outbound emails
    #[arg(long)]
    pub smtp_host: Option<String>,

    /// Optional SMTP Port for outbound emails
    #[arg(long, default_value = "587")]
    pub smtp_port: u16,

    /// Optional SMTP Username
    #[arg(long)]
    pub smtp_user: Option<String>,

    /// Optional SMTP Password
    #[arg(long)]
    pub smtp_pass: Option<String>,

    /// Optional IMAP Host for Live Mailbox Sync (e.g. imap.hostinger.com)
    #[arg(long)]
    pub imap_host: Option<String>,

    /// Optional IMAP Port (e.g. 993)
    #[arg(long, default_value = "993")]
    pub imap_port: u16,

    /// Optional IMAP Username (e.g. agent@apocalypto.in)
    #[arg(long)]
    pub imap_user: Option<String>,

    /// Optional IMAP Password
    #[arg(long)]
    pub imap_pass: Option<String>,
}

#[derive(Args, Debug)]
pub struct McpArgs {
    /// SQLite database URL
    #[arg(long, default_value = "sqlite://agentbox.db?mode=rwc")]
    pub db: String,

    /// Email domain for agent inboxes
    #[arg(long, default_value = "agentbox.io")]
    pub domain: String,
}

#[derive(Args, Debug)]
pub struct CreateArgs {
    /// Agent display name
    #[arg(short, long)]
    pub name: String,

    /// Optional custom address
    #[arg(short, long)]
    pub address: Option<String>,

    /// SQLite database URL
    #[arg(long, default_value = "sqlite://agentbox.db?mode=rwc")]
    pub db: String,
}

#[derive(Args, Debug)]
pub struct ListArgs {
    /// SQLite database URL
    #[arg(long, default_value = "sqlite://agentbox.db?mode=rwc")]
    pub db: String,
}

#[derive(Args, Debug)]
pub struct OtpArgs {
    /// Account ID or Address
    #[arg(short, long)]
    pub account: String,

    /// SQLite database URL
    #[arg(long, default_value = "sqlite://agentbox.db?mode=rwc")]
    pub db: String,
}
