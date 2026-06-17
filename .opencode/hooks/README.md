# OpenCode Hooks

本目录包含 OpenCode 的 hooks 配置，与 `.qoder/hooks/` 共享相同的实现。

## 📦 文件列表

```
.opencode/hooks/
├── preToolUse.sh      ✅ 主 hook（拦截工具调用）
├── sessionStart.sh    ✅ 会话启动 hook（注入上下文）
└── README.md         ✅ 本文档
```

## 🔗 详细文档

完整说明请查看：[`.qoder/hooks/README.md`](../../.qoder/hooks/README.md)

## 🎯 功能概述

### 三层防御体系

| 工具 | 策略 |
|------|------|
| `codebase_search` | **完全阻止** |
| `file_grep` | **条件阻止** |
| `Read` 代码文件 | **条件警告** |

### 核心逻辑

1. AI 尝试搜索代码 → 被引导使用 `codegraph_explore`
2. `codegraph_explore` 调用 → 记录状态
3. 后续 `grep`/`Read` → 检查状态，未调用 codegraph 则阻止

## 🧪 测试

```bash
# 使用 qoder 的测试脚本
bash .qoder/hooks/test-hook-v2.sh
```

## 🔧 WSL 环境适配

本项目运行在 WSL2 环境下，hooks 已适配：
- 通过 `cmd.exe /c codegraph.CMD` 调用 Windows 侧的 codegraph
- 使用 Windows 路径格式 (`C:\Users\...`)

## 📝 维护

`.opencode/hooks/` 和 `.qoder/hooks/` 共享相同的脚本。

**更新方式**：
```bash
# 修改 .qoder/hooks/ 中的脚本，然后同步到 .opencode/hooks/
cp .qoder/hooks/preToolUse.sh .opencode/hooks/
cp .qoder/hooks/sessionStart.sh .opencode/hooks/
```

或者使用符号链接：
```bash
# 删除复制的文件
rm .opencode/hooks/*.sh

# 创建符号链接（推荐）
ln -s ../../.qoder/hooks/preToolUse.sh .opencode/hooks/preToolUse.sh
ln -s ../../.qoder/hooks/sessionStart.sh .opencode/hooks/sessionStart.sh
```

---

**日期**: 2026-06-16  
**版本**: v2.0 (WSL 适配版)
