#!/bin/bash

# OpenCode 配置测试脚本
# 验证 instructions.md 是否被正确加载

echo "======================================"
echo "OpenCode 配置验证"
echo "======================================"
echo ""

# 1. 检查配置文件
echo "✓ 检查配置文件..."
if [ -f ".opencode/opencode.json" ]; then
    echo "  ✓ .opencode/opencode.json 存在"
    if grep -q "instructions" .opencode/opencode.json; then
        echo "  ✓ 配置包含 instructions 字段"
    else
        echo "  ✗ 配置缺少 instructions 字段"
        exit 1
    fi
else
    echo "  ✗ .opencode/opencode.json 不存在"
    exit 1
fi

echo ""

# 2. 检查 instructions 文件
echo "✓ 检查 instructions 文件..."
if [ -f ".opencode/instructions.md" ]; then
    echo "  ✓ .opencode/instructions.md 存在"

    # 检查关键内容
    if grep -q "codegraph_context is usually SUFFICIENT" .opencode/instructions.md; then
        echo "  ✓ 包含关键规则：codegraph_context is SUFFICIENT"
    else
        echo "  ✗ 缺少关键规则"
    fi

    if grep -q "Do NOT add extra read/search calls" .opencode/instructions.md; then
        echo "  ✓ 包含关键规则：Do NOT add extra calls"
    else
        echo "  ✗ 缺少关键规则"
    fi

    if grep -q "Total: 1 tool call" .opencode/instructions.md; then
        echo "  ✓ 包含完美工作流示例"
    else
        echo "  ✗ 缺少工作流示例"
    fi
else
    echo "  ✗ .opencode/instructions.md 不存在"
    exit 1
fi

echo ""

# 3. 检查 MCP 配置
echo "✓ 检查 CodeGraph MCP 配置..."
if [ -f ".mcp.json" ]; then
    echo "  ✓ .mcp.json 存在"
    if grep -q "codegraph" .mcp.json; then
        echo "  ✓ CodeGraph MCP Server 已配置"
    else
        echo "  ✗ 缺少 CodeGraph 配置"
    fi
else
    echo "  ✗ .mcp.json 不存在"
fi

echo ""
echo "======================================"
echo "配置验证完成！"
echo "======================================"
echo ""
echo "📋 手动测试步骤："
echo ""
echo "1. 启动 OpenCode:"
echo "   $ opencode"
echo ""
echo "2. 给它一个任务:"
echo "   \"分析 src-tauri 的后端架构\""
echo ""
echo "3. 观察工具调用:"
echo "   预期: ▪ codegraph_context(...)"
echo "         ▪ 分析完毕 ✅"
echo ""
echo "   如果看到: ▪ codegraph_context(...)"
echo "            ▪ Read(lib.rs)"
echo "            ▪ Read(other.rs)"
echo "   说明: 配置未完全生效 ❌"
echo ""
echo "4. 如果配置未生效:"
echo "   - 完全退出 OpenCode"
echo "   - 重新启动 opencode"
echo "   - 等待几秒让配置加载"
echo ""
