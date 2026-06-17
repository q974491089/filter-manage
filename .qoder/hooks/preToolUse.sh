#!/bin/bash
# preToolUse hook v2: 强制 CodeGraph-first + 状态追踪
# 适配 WSL 环境：通过 cmd.exe 调用 Windows 侧的 codegraph

LOG_FILE="/tmp/qoder-codegraph-hook.log"
CODEGRAPH_PATH="C:\\Users\\Administrator\\AppData\\Local\\pnpm\\bin\\codegraph.CMD"
PROJECT_PATH_WIN="C:\\Users\\myuser\\Projects\\filter-manage"
PROJECT_PATH_LINUX="/mnt/c/Users/myuser/Projects/filter-manage"

log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" >> "$LOG_FILE"
}

# 读取输入
input=$(cat)
tool_name=$(echo "$input" | grep -o '"tool_name"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*"\([^"]*\)"$/\1/')
file_path=$(echo "$input" | grep -o '"file_path"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*"\([^"]*\)"$/\1/')
pattern=$(echo "$input" | grep -o '"pattern"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*"\([^"]*\)"$/\1/')

# 从输入中提取 session_id，如果没有则使用固定文件名
session_id=$(echo "$input" | grep -o '"session_id"[[:space:]]*:[[:space:]]*"[^"]*"' | sed 's/.*"\([^"]*\)"$/\1/')
if [ -z "$session_id" ] || [ "$session_id" = "null" ]; then
    STATE_FILE="/tmp/codegraph_state_current"
else
    STATE_FILE="/tmp/codegraph_state_${session_id}"
fi

log "=== Hook triggered ==="
log "Tool: $tool_name"
log "File: $file_path"
log "Pattern: $pattern"
log "State file exists: $([ -f "$STATE_FILE" ] && echo "YES" || echo "NO")"

# 1. 记录 codegraph_explore 调用
if echo "$tool_name" | grep -qE "codegraph_explore|mcp__codegraph__codegraph_explore"; then
    echo "explored" > "$STATE_FILE"
    log "✓ codegraph_explore called, state recorded"
    exit 0
fi

# 2. 完全阻止 codebase_search
if echo "$tool_name" | grep -q "codebase_search"; then
    log "❌ Blocked codebase_search"
    cat <<'EOF'
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "codebase_search 已被禁用",
    "additionalContext": "🚨 **codebase_search 已被阻止**\n\n请使用 `codegraph_explore` 搜索代码。\n\n**正确流程：**\n1. `codegraph_explore(查询描述)`\n2. 如果不够，换 query 重试（最多 2 次）\n3. 如果仍不够，使用其他 codegraph 工具\n4. 仅当 CodeGraph 完全失效时才考虑其他工具"
  }
}
EOF
    exit 0
fi

# 3. 严格限制 file_grep（仅允许搜索非代码内容）
if echo "$tool_name" | grep -q "file_grep"; then
    # 检测是否在搜索代码符号
    if echo "$pattern" | grep -Eq '\b(function|class|const|let|var|export|import|interface|type|fn|struct|impl|trait|def)\b'; then
        log "❌ Blocked file_grep (searching code symbols)"
        cat <<'EOF'
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "禁止用 file_grep 搜索代码符号",
    "additionalContext": "🚨 **禁止用 file_grep 搜索代码符号**\n\n检测到你正在尝试搜索代码关键字（function/class/const 等）。\n\n**必须使用：** `codegraph_explore`"
  }
}
EOF
        exit 0
    fi

    # 检查是否已调用过 CodeGraph
    if [ ! -f "$STATE_FILE" ]; then
        log "❌ Blocked file_grep (no codegraph called)"
        cat <<'EOF'
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "必须先使用 codegraph_explore",
    "additionalContext": "⚠️ **必须先使用 codegraph_explore**\n\n你还没有调用 `codegraph_explore`。请先用 CodeGraph 搜索代码。"
  }
}
EOF
        exit 0
    fi

    log "⚠️ Allowed file_grep (codegraph already called)"
fi

# 4. 限制 Read 工具（代码文件必须先调 CodeGraph）
if [ "$tool_name" = "Read" ]; then
    # 允许读取的文件类型（配置、文档、非代码）
    if echo "$file_path" | grep -Eq '\.(json|md|toml|yaml|yml|txt|env|lock|gitignore|sh)$'; then
        log "✓ Allowed Read (config/doc file)"
        exit 0
    fi

    # 排除特殊目录
    if echo "$file_path" | grep -qE 'node_modules|\.git|dist|build|target|\.next|\.cache|\.qoder|\.opencode'; then
        log "✓ Allowed Read (excluded directory)"
        exit 0
    fi

    # 判断是否是代码文件
    if echo "$file_path" | grep -Eq '\.(ts|tsx|js|jsx|rs|py|go|java|c|cpp|h|hpp|css|scss|vue|svelte)$'; then
        # 检查是否已调用过 CodeGraph
        if [ ! -f "$STATE_FILE" ]; then
            log "❌ Blocked Read (no codegraph called)"
            cat <<'EOF'
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "必须先使用 codegraph_explore",
    "additionalContext": "⚠️ **必须先使用 codegraph_explore**\n\n你正在尝试读取代码文件，但还没有调用 `codegraph_explore`。\n\n**请先用 CodeGraph 理解代码结构。**"
  }
}
EOF
            exit 0
        fi

        # 已调用过 CodeGraph，允许但给出警告
        log "⚠️ Allowed Read (codegraph already called, but warning)"
        cat <<'EOF'
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "允许读取，但请确认 codegraph_explore 结果确实不够用",
    "additionalContext": "⚠️ **警告：你正在读取代码文件**\n\n请确认 `codegraph_explore` 的结果确实不够用。\n\n**CodeGraph 通常已经包含完整源码，无需再次 Read。**"
  }
}
EOF
        exit 0
    fi
fi

# 5. 默认允许其他工具
log "✓ Allowed (default)"
exit 0
