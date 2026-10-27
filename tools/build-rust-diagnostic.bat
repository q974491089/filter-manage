@echo off
setlocal
cd /d "%~dp0.."
call npm run build:diagnostic || exit /b 1
cd /d "%~dp0rust-diagnostic\src-tauri"
call "%~dp0..\node_modules\.bin\tauri.cmd" build
exit /b %errorlevel%
