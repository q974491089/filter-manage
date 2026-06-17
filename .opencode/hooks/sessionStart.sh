#!/bin/bash
# sessionStart hook: 注入 CodeGraph-first 上下文

cat > /dev/null

# 自动检测项目根目录（包含 .codegraph 的目录）
PROJECT_ROOT=$(pwd)
while [ "$PROJECT_ROOT" != "/" ]; do
  if [ -d "$PROJECT_ROOT/.codegraph" ]; then
    break
  fi
  PROJECT_ROOT=$(dirname "$PROJECT_ROOT")
done

if [ "$PROJECT_ROOT" = "/" ]; then
  PROJECT_ROOT=$(pwd)  # 回退到当前目录
fi

cat <<EOF
{
  "additionalContext": "🚨 **CodeGraph 硬约束已启用**

## 执行流程（严格遵守）

1️⃣ **首先调用 codegraph_explore**（必须）
   - 接受自然语言查询或 symbol/文件名集合
   - 返回相关 symbol 的完整源码（按文件分组）
   - 通常这一次调用就是完整答案

2️⃣ **如果不够，换 query 重试**（最多 2 次）
   - 尝试不同的查询描述
   - 尝试不同的 symbol 名称组合

3️⃣ **如果仍不够，使用其他 codegraph 工具**
   - codegraph_search - 按名称搜索 symbol
   - codegraph_callers - 查找调用者
   - codegraph_callees - 查找被调用者
   - codegraph_impact - 分析影响范围
   - codegraph_node - 查看单个 symbol 详情

4️⃣ **仅当 CodeGraph 完全失效时才允许回退**
   - 告知用户 CodeGraph 失效的原因
   - 获得明确许可后才使用其他工具

---

## ❌ 禁止规则

- **禁止直接使用 codebase_search**（已完全阻止）
- **禁止用 file_grep 搜索代码符号**（function/class/const 等）
- **禁止在未调用 codegraph_explore 前读取代码文件**
- **禁止对 codegraph_explore 已返回源码的文件再调 Read**

---

## 📍 项目信息

- **项目路径**: $PROJECT_ROOT
- **CodeGraph 状态**: 已启用并索引
- **MCP 工具**: codegraph_explore, codegraph_search, codegraph_callers, codegraph_callees, codegraph_impact, codegraph_node, codegraph_files

---

## 💡 最佳实践

**理解代码流程：**
\`\`\`
codegraph_explore(\"用户登录流程 - 从前端到后端\")
\`\`\`

**查找特定功能：**
\`\`\`
codegraph_explore(\"AppSettings loadSettings saveSettings\")
\`\`\`

**调试问题：**
\`\`\`
codegraph_explore(\"配置文件加载错误处理\")
\`\`\`

**CodeGraph 返回的源码已经是完整的，无需再次 Read！**"
}
EOF
exit 0
