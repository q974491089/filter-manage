@echo off
echo Building Filter Manage for Windows...
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
echo Building production version...
call npm run tauri build

echo.
echo Build complete!
echo Installation package is located at: src-tauri\target\release\bundle\
echo.
pause
