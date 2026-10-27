@echo off
rem ============================================================
rem  gamma-debug - screen LUT write-capability probe
rem
rem  Shares the exact code Filter-Manage uses (filter-manage-platform),
rem  so what it measures is what the app itself would get.
rem
rem  It WRITES test ramps to the display LUT and restores the original
rem  when finished. Keep this window open until it says it is done.
rem
rem  Portable: extract anywhere and run. No install, nothing created
rem  next to this file. The report goes to the Desktop, so the folder
rem  location does not matter.
rem
rem  Usage: double-click this file, or drag an .icc/.icm file onto it.
rem ============================================================
setlocal
chcp 65001 >nul
if not exist "%~dp0gamma-debug.exe" (
    echo [ERROR] gamma-debug.exe is missing next to this .bat
    pause
    exit /b 1
)
rem Call by full path instead of cd: after chcp 65001, cd into a
rem non-ASCII path can fail in cmd. All paths used by the program are
rem absolute (Desktop / APPDATA), so the working directory is irrelevant.
"%~dp0gamma-debug.exe" %*
echo.
pause
exit /b 0
