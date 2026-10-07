@echo off
cd /d "%~dp0"
if "%~1"=="" (
    cargo run -p tdu-viewer
) else (
    cargo run -p tdu-viewer -- "%~1"
)