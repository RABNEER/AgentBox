@echo off
setlocal enabledelayedexpansion
title AgentBox Mail — Interactive Setup Wizard

cls
echo ====================================================================
echo           ⚡ AgentBox Mail — Interactive Setup Wizard
echo ====================================================================
echo.

:: 1. Check for Rust & Cargo
echo [1/6] Checking Rust Toolchain...
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    if exist "%USERPROFILE%\.cargo\bin\cargo.exe" (
        set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
        echo [OK] Found Cargo in %USERPROFILE%\.cargo\bin
    ) else (
        echo [ERROR] Rust and Cargo were not found!
        echo Please install Rust from https://rustup.rs/ and re-run this setup.
        echo.
        pause
        exit /b 1
    )
) else (
    echo [OK] Rust toolchain detected.
)

:: Stop any running instance to avoid Windows file locks
taskkill /F /IM agentbox-mail.exe >nul 2>nul

:: 2. Interactive Domain Prompt
echo.
echo [2/6] Domain Configuration
echo --------------------------------------------------------------------
echo Enter the domain you want your agents to use (e.g. apocalypto.in, myai.com)
set "DOMAIN=apocalypto.in"
set /p USER_DOMAIN="► Domain name [Default: apocalypto.in]: "
if not "%USER_DOMAIN%"=="" (
    set "DOMAIN=%USER_DOMAIN%"
)
echo [OK] Domain set to: %DOMAIN%

:: 3. Interactive Agent Inbox Name Prompt
echo.
echo [3/6] Primary Agent Inbox Name
echo --------------------------------------------------------------------
echo Enter your primary agent mailbox name (e.g. agent, bot, support, research)
echo This will create your primary address: [name]@%DOMAIN%
set "AGENT_NAME=agent"
set /p USER_AGENT_NAME="► Inbox name [Default: agent]: "
if not "%USER_AGENT_NAME%"=="" (
    set "AGENT_NAME=%USER_AGENT_NAME%"
)
set "PRIMARY_EMAIL=%AGENT_NAME%@%DOMAIN%"
echo [OK] Primary Agent Email: %PRIMARY_EMAIL%

:: 4. Hostinger Mailbox Integration (Optional)
echo.
echo [4/6] Hostinger Mailbox Sync (Zero DNS required!)
echo --------------------------------------------------------------------
echo Do you want to sync directly with your Hostinger Email account?
set "ENABLE_HOSTINGER=Y"
set /p USER_HOSTINGER="► Connect Hostinger Mail now? (Y/n) [Default: Y]: "
if /i "%USER_HOSTINGER%"=="n" (
    set "ENABLE_HOSTINGER=N"
)

set "HOSTINGER_PASS="
if /i "%ENABLE_HOSTINGER%"=="Y" (
    echo Enter your Hostinger email password for %PRIMARY_EMAIL%:
    set /p HOSTINGER_PASS="► Password: "
)

:: Save settings to .env
(
echo DOMAIN=%DOMAIN%
echo PRIMARY_EMAIL=%PRIMARY_EMAIL%
echo AGENT_NAME=%AGENT_NAME%
echo PORT=3000
echo SMTP_INBOUND_PORT=2525
echo DATABASE_URL=sqlite://agentbox.db?mode=rwc
if not "%HOSTINGER_PASS%"=="" (
    echo IMAP_HOST=imap.hostinger.com
    echo IMAP_PORT=993
    echo IMAP_USER=%PRIMARY_EMAIL%
    echo IMAP_PASS=%HOSTINGER_PASS%
    echo SMTP_HOST=smtp.hostinger.com
    echo SMTP_PORT=587
    echo SMTP_USER=%PRIMARY_EMAIL%
    echo SMTP_PASS=%HOSTINGER_PASS%
) else (
    echo SMTP_HOST=127.0.0.1
    echo SMTP_PORT=587
)
) > .env
echo [OK] Configuration saved to .env

:: 5. Build AgentBox Binary
echo.
echo [5/6] Compiling AgentBox Mail Engine...
echo (Building high-performance Rust binary...)
cargo build
if !errorlevel! neq 0 (
    echo [ERROR] Cargo build encountered an issue.
    pause
    exit /b 1
)

set "EXE_PATH=target\debug\agentbox-mail.exe"
if not exist "%EXE_PATH%" (
    echo [ERROR] Could not find compiled binary at %EXE_PATH%
    pause
    exit /b 1
)
echo [OK] Compiled binary successfully!

:: 6. Auto-Provision Primary Agent Inbox into SQLite
echo.
echo [6/6] Provisioning Primary Inbox '%PRIMARY_EMAIL%'...
"%EXE_PATH%" create --name "%AGENT_NAME%" --address "%PRIMARY_EMAIL%" >nul 2>nul
echo [OK] Inbox '%PRIMARY_EMAIL%' created and saved in SQLite database!

cls
echo ====================================================================
echo   🎉 Setup Complete! Your AI Mailbox is 100%% Ready!
echo ====================================================================
echo.
echo   ► Primary Inbox Address : %PRIMARY_EMAIL%
echo   ► Web Dashboard URL     : http://localhost:3000
if not "%HOSTINGER_PASS%"=="" (
    echo   ► Hostinger Live Sync   : ENABLED (imap.hostinger.com:993)
)
echo   ► Inbound SMTP Port     : 2525
echo.
echo --------------------------------------------------------------------
echo   📋 WHAT TO DO NEXT:
echo --------------------------------------------------------------------
echo.
echo   1. START THE SERVER:
echo      Double-click or run:  .\run.bat   or   .\run.ps1
echo      (Starts the server and opens your browser automatically)
echo.
echo   2. TEST WITH AN EMAIL:
echo      Send an email from Gmail to: %PRIMARY_EMAIL%
echo      Watch the OTP code appear instantly on your dashboard!
echo.
echo   3. CONNECT TO AI AGENTS (Optional):
echo      Run:  .\mcp.bat  to connect Claude Code, Cursor, or Antigravity.
echo.
echo ====================================================================
echo.
pause
