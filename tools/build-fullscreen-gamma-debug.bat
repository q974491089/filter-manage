@echo off
setlocal
cd /d "%~dp0.."
where python >nul 2>&1
if errorlevel 1 (
  echo Python 3 is required. Install it from https://www.python.org/downloads/windows/
  pause
  exit /b 1
)
python -m pip install --user pyinstaller
if errorlevel 1 (
  echo Failed to install PyInstaller.
  pause
  exit /b 1
)
python -m PyInstaller --noconfirm --clean --onefile --windowed ^
  --name Filter-Manage-Fullscreen-Debug tools\fullscreen_gamma_debug.py
if errorlevel 1 (
  echo Build failed.
  pause
  exit /b 1
)
echo Built: dist\Filter-Manage-Fullscreen-Debug.exe
pause
