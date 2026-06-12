#!/bin/bash

# 测试 qoder codegraph hook

echo "Testing qoder codegraph hook..."
echo ""

# 测试用例 1: 读取 TypeScript 文件
echo "Test 1: Read TypeScript file (should use codegraph)"
echo '{
  "session_id": "test-123",
  "cwd": "/mnt/c/Users/myuser/Projects/filter-manage",
  "hook_event_name": "PreToolUse",
  "tool_name": "Read",
  "tool_input": {
    "file_path": "src/App.tsx"
  }
}' | .qoder/hooks/codegraph-read.sh

echo ""
echo "Exit code: $?"
echo ""
echo "---"
echo ""

# 测试用例 2: 读取 JSON 文件
echo "Test 2: Read JSON file (should allow native read)"
echo '{
  "session_id": "test-123",
  "cwd": "/mnt/c/Users/myuser/Projects/filter-manage",
  "hook_event_name": "PreToolUse",
  "tool_name": "Read",
  "tool_input": {
    "file_path": "package.json"
  }
}' | .qoder/hooks/codegraph-read.sh

echo ""
echo "Exit code: $?"
echo ""
echo "---"
echo ""

# 测试用例 3: 非 Read 工具
echo "Test 3: Non-Read tool (should allow immediately)"
echo '{
  "session_id": "test-123",
  "cwd": "/mnt/c/Users/myuser/Projects/filter-manage",
  "hook_event_name": "PreToolUse",
  "tool_name": "Write",
  "tool_input": {
    "file_path": "test.ts",
    "content": "console.log('test')"
  }
}' | .qoder/hooks/codegraph-read.sh

echo ""
echo "Exit code: $?"
echo ""
echo "---"
echo ""

# 查看日志
echo "Hook logs:"
if [ -f /tmp/qoder-codegraph-hook.log ]; then
  tail -20 /tmp/qoder-codegraph-hook.log
else
  echo "No log file found"
fi
