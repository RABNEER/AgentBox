Write-Host "====================================================================" -ForegroundColor Cyan
Write-Host "          ⚡ AgentBox Mail — Interactive Setup Wizard" -ForegroundColor Cyan
Write-Host "====================================================================" -ForegroundColor Cyan
Write-Host ""

# 1. Check for Rust & Cargo
Write-Host "[1/6] Checking Rust Toolchain..." -ForegroundColor Yellow
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    if (Test-Path "$HOME\.cargo\bin\cargo.exe") {
        $env:Path = "$HOME\.cargo\bin;$env:Path"
        Write-Host "[OK] Found Cargo in $HOME\.cargo\bin" -ForegroundColor Green
    } else {
        Write-Host "[ERROR] Rust and Cargo are not installed! Please install from https://rustup.rs/" -ForegroundColor Red
        exit 1
    }
} else {
    Write-Host "[OK] Rust toolchain detected." -ForegroundColor Green
}

# Stop any running instances
Stop-Process -Name "agentbox-mail" -Force -ErrorAction SilentlyContinue

# 2. Interactive Domain Prompt
Write-Host ""
Write-Host "[2/6] Domain Configuration" -ForegroundColor Yellow
Write-Host "--------------------------------------------------------------------"
$domainInput = Read-Host "► Domain name [Default: local.agentbox]"
$domain = if ([string]::IsNullOrWhiteSpace($domainInput)) { "local.agentbox" } else { $domainInput.Trim() }
Write-Host "[OK] Domain set to: $domain" -ForegroundColor Green

# 3. Interactive Agent Inbox Name Prompt
Write-Host ""
Write-Host "[3/6] Primary Agent Inbox Name" -ForegroundColor Yellow
Write-Host "--------------------------------------------------------------------"
$nameInput = Read-Host "► Inbox name [Default: agent]"
$agentName = if ([string]::IsNullOrWhiteSpace($nameInput)) { "agent" } else { $nameInput.Trim() }
$primaryEmail = "$agentName@$domain"
Write-Host "[OK] Primary Agent Email: $primaryEmail" -ForegroundColor Green

# 4. Hostinger Mailbox Integration (Optional)
Write-Host ""
Write-Host "[4/6] Hostinger Mailbox Sync (Zero DNS required!)" -ForegroundColor Yellow
Write-Host "--------------------------------------------------------------------"
$connectHostinger = Read-Host "► Connect Hostinger Mail now? (Y/n) [Default: Y]"
$hostingerPass = ""
if ($connectHostinger -ne "n" -and $connectHostinger -ne "N") {
    $hostingerPass = Read-Host "► Enter Hostinger email password for $primaryEmail"
}

# Save .env
$envLines = @(
    "DOMAIN=$domain",
    "PRIMARY_EMAIL=$primaryEmail",
    "AGENT_NAME=$agentName",
    "PORT=3000",
    "SMTP_INBOUND_PORT=2525",
    "DATABASE_URL=sqlite://agentbox.db?mode=rwc"
)

if (-not [string]::IsNullOrWhiteSpace($hostingerPass)) {
    $envLines += "IMAP_HOST=imap.hostinger.com"
    $envLines += "IMAP_PORT=993"
    $envLines += "IMAP_USER=$primaryEmail"
    $envLines += "IMAP_PASS=$hostingerPass"
    $envLines += "SMTP_HOST=smtp.hostinger.com"
    $envLines += "SMTP_PORT=587"
    $envLines += "SMTP_USER=$primaryEmail"
    $envLines += "SMTP_PASS=$hostingerPass"
} else {
    $envLines += "SMTP_HOST=127.0.0.1"
    $envLines += "SMTP_PORT=587"
}

$envLines | Out-File -FilePath ".env" -Encoding utf8
Write-Host "[OK] Configuration saved to .env" -ForegroundColor Green

# 5. Build AgentBox Binary
Write-Host ""
Write-Host "[5/6] Compiling AgentBox Mail Engine..." -ForegroundColor Yellow
cargo build
if ($LASTEXITCODE -ne 0) {
    Write-Host "[ERROR] Cargo build failed!" -ForegroundColor Red
    exit 1
}
Write-Host "[OK] Compiled binary successfully!" -ForegroundColor Green

# 6. Provision Primary Inbox in SQLite
Write-Host ""
Write-Host "[6/6] Provisioning Primary Inbox '$primaryEmail'..." -ForegroundColor Yellow
& ".\target\debug\agentbox-mail.exe" create --name "$agentName" --address "$primaryEmail" | Out-Null
Write-Host "[OK] Inbox '$primaryEmail' created in SQLite database!" -ForegroundColor Green

Clear-Host
Write-Host "====================================================================" -ForegroundColor Cyan
Write-Host "  🎉 Setup Complete! Your AI Mailbox is 100% Ready!" -ForegroundColor Green
Write-Host "====================================================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  ► Primary Inbox Address : $primaryEmail" -ForegroundColor White
Write-Host "  ► Web Dashboard URL     : http://localhost:3000" -ForegroundColor White
if (-not [string]::IsNullOrWhiteSpace($hostingerPass)) {
    Write-Host "  ► Hostinger Live Sync   : ENABLED (imap.hostinger.com:993)" -ForegroundColor Green
}
Write-Host "  ► Inbound SMTP Port     : 2525" -ForegroundColor White
Write-Host ""
Write-Host "--------------------------------------------------------------------" -ForegroundColor DarkGray
Write-Host "  📋 WHAT TO DO NEXT:" -ForegroundColor Yellow
Write-Host "--------------------------------------------------------------------" -ForegroundColor DarkGray
Write-Host ""
Write-Host "  1. START THE SERVER:" -ForegroundColor White
Write-Host "     Run:  .\run.bat   or   .\run.ps1" -ForegroundColor Cyan
Write-Host "     (This starts the server and opens your browser automatically)"
Write-Host ""
Write-Host "  2. TEST WITH AN EMAIL:" -ForegroundColor White
Write-Host "     Send an email from Gmail to: $primaryEmail" -ForegroundColor Cyan
Write-Host "     Watch the OTP code appear instantly on your dashboard!"
Write-Host ""
Write-Host "  3. CONNECT TO AI AGENTS (Optional):" -ForegroundColor White
Write-Host "     Run:  .\mcp.bat  to connect Claude Code, Cursor, or Antigravity."
Write-Host ""
Write-Host "====================================================================" -ForegroundColor Cyan
