Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "          ⚡ Starting AgentBox Mail All-In-One Engine" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

$domain = "apocalypto.in"
$port = "3000"
$smtpPort = "2525"
$primaryEmail = "agent@apocalypto.in"

if (Test-Path ".env") {
    Get-Content ".env" | ForEach-Object {
        $line = $_.Trim()
        if ($line -and -not $line.StartsWith("#")) {
            $parts = $line.Split("=", 2)
            if ($parts[0] -eq "DOMAIN") { $domain = $parts[1] }
            if ($parts[0] -eq "PORT") { $port = $parts[1] }
            if ($parts[0] -eq "SMTP_INBOUND_PORT") { $smtpPort = $parts[1] }
            if ($parts[0] -eq "PRIMARY_EMAIL") { $primaryEmail = $parts[1] }
        }
    }
}

$exe = if (Test-Path "target\release\agentbox-mail.exe") { "target\release\agentbox-mail.exe" } else { "target\debug\agentbox-mail.exe" }

if (-not (Test-Path $exe)) {
    Write-Host "[!] Binary not found. Building now..." -ForegroundColor Yellow
    cargo build
}

# Optional: Start Stalwart container if it exists
if (Get-Command docker -ErrorAction SilentlyContinue) {
    $stalwartExists = docker ps -a --filter "name=stalwart" --format "{{.Names}}" 2>$null
    if ($stalwartExists -eq "stalwart") {
        Write-Host "[INFO] Starting Stalwart Mail Server container..." -ForegroundColor Green
        docker start stalwart | Out-Null
    }
}

Write-Host "[✓] Engine Binary   : $exe" -ForegroundColor Green
Write-Host "[✓] Active Domain   : @$domain" -ForegroundColor Green
Write-Host "[✓] Primary Address : $primaryEmail" -ForegroundColor Green
Write-Host "[✓] Web Dashboard   : http://localhost:$port" -ForegroundColor Green
Write-Host "[✓] Raw SMTP Server : 0.0.0.0:$smtpPort" -ForegroundColor Green
Write-Host ""

Start-Process "http://localhost:$port"

Write-Host "Starting AgentBox Server... (Press Ctrl+C to stop)" -ForegroundColor Yellow
Write-Host ""
& $exe server --domain $domain --port ([int]$port) --smtp-inbound-port ([int]$smtpPort)
