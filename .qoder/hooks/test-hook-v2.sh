#!/bin/bash

# 测试新的 preToolUse hook

echo "=== Testing preToolUse hook v2 ==="
echo ""

# 清理旧状态
rm -f /tmp/codegraph_state_*
STATE_FILE="/tmp/codegraph_state_current"

# 测试 1: 阻止 codebase_search
echo "Test 1: Block codebase_search"
echo '{
  "tool_name": "codebase_search",
  "tool_input": {
    "query": "AppSettings"
  }
}' | .qoder/hooks/preToolUse.sh
echo "Exit code: $?"
echo "---"
echo ""

# 测试 2: 阻止 file_grep 搜索代码符号（未调用 codegraph）
echo "Test 2: Block file_grep searching code symbols (no codegraph)"
echo '{
  "tool_name": "file_grep",
  "tool_input": {
    "pattern": "function loadSettings"
  }
}' | .qoder/hooks/preToolUse.sh
echo "Exit code: $?"
echo "---"
echo ""

# 测试 3: 允许 codegraph_explore（并记录状态）
echo "Test 3: Allow codegraph_explore (and record state)"
echo '{
  "tool_name": "mcp__codegraph__codegraph_explore",
  "tool_input": {
    "query": "AppSettings"
  }
}' | .qoder/hooks/preToolUse.sh
echo "Exit code: $?"
echo "State file created: $([ -f "$STATE_FILE" ] && echo "YES" || echo "NO")"
echo "---"
echo ""

# 测试 4: 允许 file_grep（codegraph 已调用）
echo "Test 4: Allow file_grep (codegraph already called)"
echo '{
  "tool_name": "file_grep",
  "tool_input": {
    "pattern": "TODO"
  }
}' | .qoder/hooks/preToolUse.sh
echo "Exit code: $?"
echo "---"
echo ""

# 测试 5: 阻止 Read 代码文件（未调用 codegraph）
rm -f "$STATE_FILE"
echo "Test 5: Block Read code file (no codegraph)"
echo '{
  "tool_name": "Read",
  "tool_input": {
    "file_path": "src/App.tsx"
  }
}' | .qoder/hooks/preToolUse.sh
echo "Exit code: $?"
echo "---"
echo ""

# 测试 6: 允许 Read 配置文件
echo "Test 6: Allow Read config file"
echo '{
  "tool_name": "Read",
  "tool_input": {
    "file_path": "package.json"
  }
}' | .qoder/hooks/preToolUse.sh
echo "Exit code: $?"
echo "---"
echo ""

# 测试 7: 警告 Read 代码文件（codegraph 已调用）
echo "explored" > "$STATE_FILE"
echo "Test 7: Warn Read code file (codegraph already called)"
echo '{
  "tool_name": "Read",
  "tool_input": {
    "file_path": "src/App.tsx"
  }
}' | .qoder/hooks/preToolUse.sh
echo "Exit code: $?"
echo "---"
echo ""

# 查看日志
echo "=== Hook logs ==="
tail -30 /tmp/qoder-codegraph-hook.log 2>/dev/null || echo "No log file"
