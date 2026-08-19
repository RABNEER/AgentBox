@echo off
setlocal enabledelayedexpansion

:: Read configuration from .env if it exists
set "DOMAIN=local.agentbox"
if exist ".env" (
    for /f "usebackq tokens=1,* delims==" %%A in (".env") do (
        if "%%A"=="DOMAIN" set "DOMAIN=%%B"
    )
)

:: Locate Binary
set "EXE="
if exist "target\release\agentbox-mail.exe" (
    set "EXE=target\release\agentbox-mail.exe"
) else if exist "target\debug\agentbox-mail.exe" (
    set "EXE=target\debug\agentbox-mail.exe"
) else (
    cargo build >nul 2>nul
    set "EXE=target\debug\agentbox-mail.exe"
)

:: Start MCP stdio server
"%EXE%" mcp --domain "%DOMAIN%"
