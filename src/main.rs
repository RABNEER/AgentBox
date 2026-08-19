mod api;
mod cli;
mod db;
mod engine;
mod mcp;

use clap::Parser;
use cli::{
    AgentArgs, AgentSubcommands, Cli, Commands, CreateArgs, ListArgs, McpArgs, OtpArgs, ServerArgs,
};
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

    let is_interactive = std::env::args().len() <= 1;
    let cli = Cli::parse();

    let result = match cli.command.unwrap_or_else(|| {
        Commands::Server(ServerArgs {
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
        })
    }) {
        Commands::Server(args) => run_server(args).await,
        Commands::Mcp(args) => run_mcp(args).await,
        Commands::Create(args) => run_create(args).await,
        Commands::List(args) => run_list(args).await,
        Commands::Otp(args) => run_otp(args).await,
        Commands::Agent(args) => run_agent(args).await,
    };

    if let Err(ref e) = result {
        eprintln!("\n❌ AgentBox Error: {}\n", e);
        if is_interactive {
            eprintln!("Press Enter to exit...");
            let mut buf = String::new();
            let _ = std::io::stdin().read_line(&mut buf);
        }
    }

    result
}

async fn run_server(args: ServerArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Initializing AgentBox Mail SQLite Database at {}", args.db);
    let db = Database::init(&args.db).await?;

    let smtp_host = args.smtp_host.or_else(|| std::env::var("SMTP_HOST").ok());
    let smtp_port = std::env::var("SMTP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(args.smtp_port);
    let smtp_user = args.smtp_user.or_else(|| std::env::var("SMTP_USER").ok());
    let smtp_pass = args.smtp_pass.or_else(|| std::env::var("SMTP_PASS").ok());

    let domain = std::env::var("DOMAIN").unwrap_or(args.domain);

    // Auto-provision main and agent inboxes
    let _ = db
        .create_account(&format!("hello@{}", domain), Some("Primary Domain Mailbox"))
        .await;
    let _ = db
        .create_account(
            &format!("agent@{}", domain),
            Some("Autonomous Agent Mailbox"),
        )
        .await;

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
                tracing::warn!("Embedded SMTP server note: {} (HTTP API remains active)", e);
            }
        });
    }

    // 2. Start Hostinger IMAP Live Sync Worker if credentials provided
    let imap_host = args.imap_host.or_else(|| std::env::var("IMAP_HOST").ok());
    let imap_port = std::env::var("IMAP_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(args.imap_port);
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

    // Resilient Port Allocation: Try requested port, then fallback up to 10 ports
    let mut chosen_port = args.port;
    let mut listener_opt = None;

    for port_candidate in args.port..(args.port + 20) {
        let bind_addr = format!("{}:{}", args.host, port_candidate);
        match tokio::net::TcpListener::bind(&bind_addr).await {
            Ok(l) => {
                chosen_port = port_candidate;
                listener_opt = Some(l);
                break;
            }
            Err(e) => {
                tracing::warn!("Port {} unavailable ({}). Checking next available port...", port_candidate, e);
            }
        }
    }

    let listener = match listener_opt {
        Some(l) => l,
        None => {
            let bind_addr = format!("{}:{}", args.host, args.port);
            tokio::net::TcpListener::bind(&bind_addr).await?
        }
    };

    let dashboard_url = format!("http://localhost:{}", chosen_port);

    println!("\n╔══════════════════════════════════════════════════════════════════╗");
    println!("║        ⚡ AgentBox Mail — All-In-One Autonomous Engine          ║");
    println!("╠══════════════════════════════════════════════════════════════════╣");
    if !args.no_smtp {
        println!(
            "║  ► Raw SMTP Inbound: 0.0.0.0:{} (Direct Email Listener)     ║",
            args.smtp_inbound_port
        );
    }
    if imap_enabled {
        println!("║  ► Hostinger Sync  : IMAP TLS Active (Live Poller)          ║");
    }
    println!(
        "║  ► Web Dashboard   : {} (Monochrome UI)     ║",
        dashboard_url
    );
    println!(
        "║  ► Inbound HTTP API: http://localhost:{}/v1/inbound         ║",
        chosen_port
    );
    println!(
        "║  ► Realtime SSE    : http://localhost:{}/v1/events          ║",
        chosen_port
    );
    println!(
        "║  ► Agent Domain    : @{}                               ║",
        domain
    );
    println!("╚══════════════════════════════════════════════════════════════════╝\n");

    // Automatically open browser window for user convenience
    let open_url = dashboard_url.clone();
    tokio::spawn(async move {
        tokio::time::sleep(tokio::time::Duration::from_millis(350)).await;
        #[cfg(target_os = "windows")]
        {
            let _ = std::process::Command::new("cmd")
                .args(["/c", "start", &open_url])
                .spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(&open_url).spawn();
        }
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("xdg-open").arg(&open_url).spawn();
        }
    });

    axum::serve(listener, router).await?;
    Ok(())
}

async fn run_mcp(args: McpArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db = Database::init(&args.db).await?;
    let mailer = OutboundMailer::new(None, 587, None, None);
    let (tx, _) = tokio::sync::broadcast::channel::<String>(200);

    // Spawn background SSE listener to bridge events from running AgentBox daemon
    let tx_bridge = tx.clone();
    tokio::spawn(async move {
        bridge_daemon_sse_events(tx_bridge).await;
    });

    let mcp = Arc::new(mcp::McpServer::new(db, mailer, args.domain, Some(tx)));

    mcp.run_stdio().await?;
    Ok(())
}

async fn bridge_daemon_sse_events(tx: tokio::sync::broadcast::Sender<String>) {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    loop {
        if let Ok(stream) = tokio::net::TcpStream::connect("127.0.0.1:3000").await {
            let (reader, mut writer) = tokio::io::split(stream);
            let req = "GET /v1/events HTTP/1.1\r\nHost: 127.0.0.1:3000\r\nAccept: text/event-stream\r\nConnection: keep-alive\r\n\r\n";
            if writer.write_all(req.as_bytes()).await.is_ok() {
                let mut lines = BufReader::new(reader).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(data) = line.strip_prefix("data:") {
                        let trimmed = data.trim();
                        if !trimmed.is_empty() {
                            let _ = tx.send(trimmed.to_string());
                        }
                    }
                }
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    }
}

async fn run_create(args: CreateArgs) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let db = Database::init(&args.db).await?;
    let rand_slug = uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
    let address = if let Some(custom) = args.address {
        custom
    } else {
        format!(
            "{}-{}@{}",
            args.name.to_lowercase().replace(' ', "-"),
            rand_slug,
            "agentbox.io"
        )
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

    println!(
        "\n{:<16} {:<34} {:<20}",
        "ACCOUNT ID", "EMAIL ADDRESS", "NAME"
    );
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
        AgentSubcommands::Create {
            name,
            email,
            capabilities,
            db,
        } => {
            let db_inst = Database::init(&db).await?;
            let domain = std::env::var("DOMAIN").unwrap_or_else(|_| "apocalypto.in".to_string());
            let agent_email = if let Some(custom) = email {
                custom
            } else {
                let rand_slug = uuid::Uuid::new_v4().to_string().replace('-', "")[..6].to_string();
                format!(
                    "{}-{}@{}",
                    name.to_lowercase().replace(' ', "-"),
                    rand_slug,
                    domain
                )
            };

            let caps: Vec<String> = if let Some(caps_str) = capabilities {
                caps_str.split(',').map(|s| s.trim().to_string()).collect()
            } else {
                vec![
                    "inbox.read".to_string(),
                    "otp.read".to_string(),
                    "links.read".to_string(),
                ]
            };

            let creds = db_inst
                .create_agent_identity(&name, &agent_email, &caps)
                .await?;

            println!("\n╔══════════════════════════════════════════════════════════════════╗");
            println!("║             🧑‍🚀 AGENT IDENTITY PROVISIONED                      ║");
            println!("╠══════════════════════════════════════════════════════════════════╣");
            println!("║  Agent ID     : {:<48} ║", creds.agent_id);
            println!("║  Name         : {:<48} ║", creds.name);
            println!("║  Email        : {:<48} ║", creds.email_address);
            println!("║  Auth Token   : {:<48} ║", creds.auth_token);
            println!(
                "║  Capabilities : {:<48} ║",
                format!("{:?}", creds.capabilities)
            );
            println!("║  Status       : {:<48} ║", "active");
            println!("╚══════════════════════════════════════════════════════════════════╝\n");
            println!("⚠️  NOTE: {}", creds.note);
        }
        AgentSubcommands::List { db } => {
            let db_inst = Database::init(&db).await?;
            let list = db_inst.list_agent_identities_public().await?;

            if list.is_empty() {
                println!("No registered Agent Identities found.");
                return Ok(());
            }

            println!(
                "\n{:<24} {:<16} {:<32} {:<10}",
                "AGENT ID", "NAME", "EMAIL IDENTITY", "STATUS"
            );
            println!("{:-<84}", "");
            for agent in list {
                println!(
                    "{:<24} {:<16} {:<32} {:<10}",
                    agent.id, agent.name, agent.email_address, agent.status
                );
            }
            println!();
        }
        AgentSubcommands::Revoke { agent_id, db } => {
            let db_inst = Database::init(&db).await?;
            db_inst.revoke_agent_identity(&agent_id).await?;
            println!(
                "🔒 Revoked Agent Identity '{}'. Token and permissions invalidated.",
                agent_id
            );
        }
    }
    Ok(())
}
