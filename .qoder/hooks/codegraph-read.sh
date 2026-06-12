#!/bin/bash

# Qoder Hook: 拦截 Read 工具，对代码文件优先使用 codegraph
#
# 工作流程：
# 1. 读取 stdin 的 JSON 输入
# 2. 检查是否是 Read 工具调用
# 3. 判断是否是代码文件
# 4. 尝试用 codegraph 读取
# 5. 成功：返回 JSON 修改 tool_input，标记使用了 codegraph
# 6. 失败：允许原工具执行（exit 0）

CODEGRAPH_PATH="C:\\Users\\Administrator\\AppData\\Local\\pnpm\\bin\\codegraph.CMD"
PROJECT_PATH="C:\\Users\\myuser\\Projects\\filter-manage"
LOG_FILE="/tmp/qoder-codegraph-hook.log"

# 记录日志
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1" >> "$LOG_FILE"
}

# 简单的 JSON 解析函数（不依赖 jq）
parse_json() {
    local key=$1
    local json=$2
    # 使用 grep 和 sed 提取简单的 JSON 字段
    echo "$json" | grep -o "\"$key\"[[:space:]]*:[[:space:]]*\"[^\"]*\"" | sed 's/.*"\([^"]*\)"$/\1/'
}

# 读取输入
input=$(cat)
log "Hook triggered with input: $input"

# 解析 JSON
tool_name=$(parse_json "tool_name" "$input")
file_path=$(parse_json "file_path" "$input")
if [ -z "$file_path" ]; then
    file_path=$(parse_json "filePath" "$input")
fi

log "Tool: $tool_name, File: $file_path"

# 只拦截 Read 工具
if [ "$tool_name" != "Read" ]; then
    log "Not a Read tool, allowing"
    exit 0
fi

# 检查文件路径是否存在
if [ -z "$file_path" ] || [ "$file_path" = "null" ]; then
    log "No file path, allowing"
    exit 0
fi

# 判断是否是代码文件
ext="${file_path##*.}"
case "$ext" in
    ts|tsx|js|jsx|rs|py|go|java|c|cpp|h|hpp|css|scss|vue|svelte)
        log "Code file detected: $ext"
        ;;
    *)
        log "Not a code file, allowing"
        exit 0
        ;;
esac

# 排除特殊目录
if echo "$file_path" | grep -qE 'node_modules|\.git|dist|build|target|\.next|\.cache|\.qoder|\.opencode'; then
    log "Excluded directory, allowing"
    exit 0
fi

# 尝试用 codegraph 读取
log "Attempting codegraph read..."

# 提取文件名作为查询
filename=$(basename "$file_path")
log "Filename: $filename"

# 调用 codegraph context
# 注意：必须在项目目录下执行
# Windows 路径不需要额外的引号转义
log "Executing codegraph context in project directory"
codegraph_output=$(cd /mnt/c/Users/myuser/Projects/filter-manage && cmd.exe /c "C:\\Users\\Administrator\\AppData\\Local\\pnpm\\bin\\codegraph.CMD context \"$filename\"" 2>&1)
codegraph_exit_code=$?

log "Codegraph exit code: $codegraph_exit_code"
log "Codegraph output length: ${#codegraph_output}"
if [ ${#codegraph_output} -lt 500 ]; then
    log "Codegraph output: $codegraph_output"
else
    log "Codegraph output (first 500 chars): ${codegraph_output:0:500}"
fi

# 检查是否成功
if [ $codegraph_exit_code -eq 0 ] && [ -n "$codegraph_output" ] && [ ${#codegraph_output} -gt 100 ]; then
    log "✓ Codegraph read successful, length: ${#codegraph_output}"

    # 构造增强的输出
    enhanced_output="<!-- Read via CodeGraph Hook -->\n\n$codegraph_output"

    # 转义 JSON 字符串（替换反斜杠、引号、换行符）
    escaped_output=$(echo "$enhanced_output" | sed 's/\\/\\\\/g' | sed 's/"/\\"/g' | sed ':a;N;$!ba;s/\n/\\n/g')

    # 返回 JSON，使用 additionalContext 将 codegraph 输出传递给 AI
    cat <<EOF
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow",
    "permissionDecisionReason": "Using CodeGraph for enhanced code understanding",
    "additionalContext": "$escaped_output"
  }
}
EOF
    exit 0
else
    log "Codegraph failed or returned insufficient data, falling back to native read"
    # 允许原工具执行
    exit 0
fi
