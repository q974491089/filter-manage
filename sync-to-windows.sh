#!/bin/bash

# Sync project to Windows for building
# Usage: ./sync-to-windows.sh [windows_project_path]

WINDOWS_PATH="${1:-/mnt/c/Users/$USER/Projects/filter-manage}"

echo "Syncing project to Windows..."
echo "Target: $WINDOWS_PATH"

# Create Windows directory if it doesn't exist
mkdir -p "$WINDOWS_PATH"

# Sync source files (exclude node_modules, target, .git)
rsync -av --progress \
  --exclude='node_modules' \
  --exclude='target' \
  --exclude='.git' \
  --exclude='dist' \
  --exclude='src-tauri/target' \
  . "$WINDOWS_PATH/"

echo ""
echo "Sync complete!"
echo ""
echo "To build on Windows:"
echo "  1. Open Windows PowerShell"
echo "  2. cd $WINDOWS_PATH"
echo "  3. npm install"
echo "  4. npm run tauri build"
echo ""
echo "Build output will be at:"
echo "  $WINDOWS_PATH/src-tauri/target/release/bundle/"
