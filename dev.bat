@echo off
echo Starting Filter Manage development server...
echo.
echo Make sure you have installed:
echo - Node.js v18+
echo - Rust (via rustup)
echo - Tauri CLI (cargo install tauri-cli)
echo.
echo Press any key to continue...
pause > nul

echo.
echo Installing npm dependencies...
call npm install

echo.
echo Starting Tauri development server...
call npm run tauri dev
