mod api;
mod cli;
mod db;
mod engine;
mod mcp;

use clap::Parser;
use cli::{AgentArgs, AgentSubcommands, Cli, Commands, CreateArgs, ListArgs, McpArgs, OtpArgs, ServerArgs};
use db::Database;
use engine::{ImapSyncWorker, OutboundMailer, SmtpServer};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Automatically load .env if present
    if let Ok(env_str) = std::fs::read_to_string(".env") {
        for line in env_str.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() && !trimmed.starts_with('#') {
                if let Some((k, v)) = trimmed.split_once('=') {
                    std::env::set_var(k.trim(), v.trim());
                }
            }
        }
    }

    // Install Rustls Crypto Provider
    let _ = rustls::crypto::ring::default_provider().install_default();

    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "agentbox_mail=info,tower_http=info,axum=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    match cli.command.unwrap_or_else(|| Commands::Server(ServerArgs {
        port: 3000,
        host: "0.0.0.0".to_string(),
        smtp_inbound_port: 2525,
        no_smtp: false,
        db: "sqlite://agentbox.db?mode=rwc".to_string(),
        domain: "apocalypto.in".to_string(),
        smtp_host: None,
        smtp_port: 587,
        smtp_user: None,
        smtp_pass: None,
        imap_host: None,
        imap_port: 993,
        imap_user: None,
        imap_pass: None,
    })) {
        Commands::Server(args) => run_server(args).await,
        Commands::Mcp(args) => run_mcp(args).await,
        Commands::Create(args) => run_create(args).await,
        Commands::List(args) => run_list(args).await,
        Commands::Otp(args) => run_otp(args).await,
        Commands::Agent(args) => run_agent(args).await,
    }
}

async fn run_server(args: ServerArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Initializing AgentBox Mail SQLite Database at {}", args.db);
    let db = Database::init(&args.db).await?;

    let smtp_host = args.smtp_host.or_else(|| std::env::var("SMTP_HOST").ok());
    let smtp_port = std::env::var("SMTP_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(args.smtp_port);
    let smtp_user = args.smtp_user.or_else(|| std::env::var("SMTP_USER").ok());
    let smtp_pass = args.smtp_pass.or_else(|| std::env::var("SMTP_PASS").ok());

    let domain = std::env::var("DOMAIN").unwrap_or(args.domain);

    // Auto-provision main and agent inboxes
    let _ = db.create_account(&format!("hello@{}", domain), Some("Primary Domain Mailbox")).await;
    let _ = db.create_account(&format!("agent@{}", domain), Some("Autonomous Agent Mailbox")).await;

    let mailer = OutboundMailer::new(smtp_host, smtp_port, smtp_user, smtp_pass);

    let (tx, _rx) = broadcast::channel(100);

    // 1. Start Embedded Raw Inbound SMTP Server if not disabled
    if !args.no_smtp {
        let smtp_server = Arc::new(SmtpServer::new(
            db.clone(),
            tx.clone(),
            domain.clone(),
            args.smtp_inbound_port,
        ));

        tokio::spawn(async move {
            if let Err(e) = smtp_server.start().await {
                tracing::error!("Embedded SMTP server error: {}", e);
            }
        });
    }

    // 2. Start Hostinger IMAP Live Sync Worker if credentials provided
    let imap_host = args.imap_host.or_else(|| std::env::var("IMAP_HOST").ok());
    let imap_port = std::env::var("IMAP_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(args.imap_port);
    let imap_user = args.imap_user.or_else(|| std::env::var("IMAP_USER").ok());
    let imap_pass = args.imap_pass.or_else(|| std::env::var("IMAP_PASS").ok());

    let imap_enabled = imap_host.is_some() && imap_user.is_some() && imap_pass.is_some();
    if let (Some(host), Some(user), Some(pass)) = (imap_host, imap_user, imap_pass) {
        let worker = Arc::new(ImapSyncWorker::new(
            db.clone(),
            tx.clone(),
            host,
            imap_port,
            user,
            pass,
        ));

        tokio::spawn(async move {
            worker.start().await;
        });
    }

    let state = api::AppState {
        db: db.clone(),
        mailer,
        tx,
        domain: domain.clone(),
    };

    let router = api::create_router(state);
    let bind_addr = format!("{}:{}", args.host, args.port);
    let listener = tokio::net::TcpListener::bind(&bind_addr).await?;

    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║        ⚡ AgentBox Mail — All-In-One Autonomous Engine          ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    if !args.no_smtp {
        println!("║  ► Raw SMTP Inbound: 0.0.0.0:{} (Direct Email Listener)     ║", args.smtp_inbound_port);
    }
    if imap_enabled {
        println!("║  ► Hostinger Sync  : IMAP TLS Active (Live Poller)          ║");
    }
    println!("║  ► Web Dashboard   : http://localhost:{} (Monochrome UI)     ║", args.port);
    println!("║  ► Inbound HTTP API: http://localhost:{}/v1/inbound         ║", args.port);
    println!("║  ► Realtime SSE    : http://localhost:{}/v1/events          ║", args.port);
    println!("║  ► Agent Domain    : @{}                               ║", domain);
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    axum::serve(listener, router).await?;
    Ok(())
}

async fn run_mcp(args: McpArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db = Database::init(&args.db).await?;
    let mailer = OutboundMailer::new(None, 587, None, None);
    let mcp = Arc::new(mcp::McpServer::new(db, mailer, args.domain, None));
    
    mcp.run_stdio().await?;
    Ok(())
}

async fn run_create(args: CreateArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db = Database::init(&args.db).await?;
    let rand_slug = uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
    let address = if let Some(custom) = args.address {
        custom
    } else {
        format!("{}-{}@{}", args.name.to_lowercase().replace(' ', "-"), rand_slug, "agentbox.io")
    };

    let account = db.create_account(&address, Some(&args.name)).await?;
    println!("✅ Created virtual inbox successfully!");
    println!("   ID      : {}", account.id);
    println!("   Address : {}", account.address);
    println!("   Name    : {}", account.display_name.unwrap_or_default());
    Ok(())
}

async fn run_list(args: ListArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db = Database::init(&args.db).await?;
    let accounts = db.list_accounts().await?;

    if accounts.is_empty() {
        println!("No active agent inboxes found.");
        return Ok(());
    }

    println!("\n{:<16} {:<34} {:<20}", "ACCOUNT ID", "EMAIL ADDRESS", "NAME");
    println!("{:-<72}", "");
    for acc in accounts {
        println!(
            "{:<16} {:<34} {:<20}",
            acc.id,
            acc.address,
            acc.display_name.unwrap_or_default()
        );
    }
    println!();
    Ok(())
}

async fn run_otp(args: OtpArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db = Database::init(&args.db).await?;
    
    let account = if args.account.contains('@') {
        db.get_account_by_address(&args.account).await?
    } else {
        db.get_account_by_id(&args.account).await?
    };

    let acc = match account {
        Some(a) => a,
        None => {
            eprintln!("❌ Account '{}' not found.", args.account);
            return Ok(());
        }
    };

    let otp = db.get_latest_otp(&acc.id).await?;
    match otp {
        Some(code) => {
            println!("🔑 LATEST OTP CODE: {}", code);
        }
        None => {
            println!("ℹ️ No OTP codes found in inbox '{}'.", acc.address);
        }
    }

    Ok(())
}

async fn run_agent(args: AgentArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    match args.action {
        AgentSubcommands::Create { name, capabilities, db } => {
            let db_inst = Database::init(&db).await?;
            let domain = std::env::var("DOMAIN").unwrap_or_else(|_| "apocalypto.in".to_string());
            let rand_slug = uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
            let email = format!("{}-{}@{}", name.to_lowercase().replace(' ', "-"), rand_slug, domain);

            let caps: Vec<String> = if let Some(caps_str) = capabilities {
                caps_str.split(',').map(|s| s.trim().to_string()).collect()
            } else {
                vec!["inbox.read".to_string(), "otp.read".to_string(), "links.read".to_string()]
            };

            let identity = db_inst.create_agent_identity(&name, &email, &caps).await?;

            println!("\n╔══════════════════════════════════════════════════════════════════╗");
            println!("║             🧑‍🚀 AGENT IDENTITY PROVISIONED                      ║");
            println!("╠══════════════════════════════════════════════════════════════════╣");
            println!("║  Agent ID     : {:<48} ║", identity.id);
            println!("║  Name         : {:<48} ║", identity.name);
            println!("║  Email        : {:<48} ║", identity.email_address);
            println!("║  Auth Token   : {:<48} ║", identity.token);
            println!("║  Capabilities : {:<48} ║", identity.capabilities);
            println!("║  Status       : {:<48} ║", identity.status);
            println!("╚══════════════════════════════════════════════════════════════════╝\n");
        }
        AgentSubcommands::List { db } => {
            let db_inst = Database::init(&db).await?;
            let list = db_inst.list_agent_identities().await?;

            if list.is_empty() {
                println!("No registered Agent Identities found.");
                return Ok(());
            }

            println!("\n{:<24} {:<16} {:<32} {:<10}", "AGENT ID", "NAME", "EMAIL IDENTITY", "STATUS");
            println!("{:-<84}", "");
            for agent in list {
                println!(
                    "{:<24} {:<16} {:<32} {:<10}",
                    agent.id,
                    agent.name,
                    agent.email_address,
                    agent.status
                );
            }
            println!();
        }
        AgentSubcommands::Revoke { agent_id, db } => {
            let db_inst = Database::init(&db).await?;
            db_inst.revoke_agent_identity(&agent_id).await?;
            println!("🔒 Revoked Agent Identity '{}'. Token and permissions invalidated.", agent_id);
        }
    }
    Ok(())
}
