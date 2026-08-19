@echo off
setlocal enabledelayedexpansion
title AgentBox Mail — Running

echo ================================================================
echo           ⚡ Starting AgentBox Mail All-In-One Engine
echo ================================================================
echo.

:: Ensure Cargo bin is in path if needed
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
    set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)

:: Read configuration from .env if it exists
set "DOMAIN=apocalypto.in"
set "PORT=3000"
set "SMTP_INBOUND_PORT=2525"
set "PRIMARY_EMAIL=agent@apocalypto.in"

if exist ".env" (
    for /f "usebackq tokens=1,* delims==" %%A in (".env") do (
        if "%%A"=="DOMAIN" set "DOMAIN=%%B"
        if "%%A"=="PORT" set "PORT=%%B"
        if "%%A"=="SMTP_INBOUND_PORT" set "SMTP_INBOUND_PORT=%%B"
        if "%%A"=="PRIMARY_EMAIL" set "PRIMARY_EMAIL=%%B"
    )
)

:: Locate Binary (Release first, fallback to Debug)
set "EXE="
if exist "target\release\agentbox-mail.exe" (
    set "EXE=target\release\agentbox-mail.exe"
) else if exist "target\debug\agentbox-mail.exe" (
    set "EXE=target\debug\agentbox-mail.exe"
) else (
    echo [!] Binary not found. Building now...
    cargo build
    if exist "target\debug\agentbox-mail.exe" (
        set "EXE=target\debug\agentbox-mail.exe"
    ) else (
        echo [ERROR] Could not build or find agentbox-mail.exe. Please run setup.bat first.
        pause
        exit /b 1
    )
)

:: Optional: Launch Stalwart Docker if container exists
where docker >nul 2>nul
if %errorlevel% equ 0 (
    docker inspect stalwart >nul 2>nul
    if %errorlevel% equ 0 (
        echo [INFO] Starting Stalwart Mail Server container...
        docker start stalwart >nul 2>nul
    )
)

echo [✓] Engine Binary   : %EXE%
echo [✓] Active Domain   : @%DOMAIN%
echo [✓] Primary Address : %PRIMARY_EMAIL%
echo [✓] Web Dashboard   : http://localhost:%PORT%
echo [✓] Raw SMTP Server : 0.0.0.0:%SMTP_INBOUND_PORT%
echo.

:: Launch browser in 2 seconds
start "" "http://localhost:%PORT%"

:: Launch the main all-in-one server
echo Starting AgentBox Server... (Press Ctrl+C to stop)
echo.
"%EXE%" server --domain "%DOMAIN%" --port %PORT% --smtp-inbound-port %SMTP_INBOUND_PORT%
