# Hooks 优化完成 ✅

## 🎯 解决的问题

**原问题**：AI 调用 `codegraph_explore` 后，还会继续调用 `grep` 和 `Read`，造成重复工作。

**根本原因**：旧版 hooks 只拦截了 `Read`，没有拦截 `file_grep` 和 `codebase_search`。

---

## ✅ 优化内容

### 1. 三层防御体系

| 工具 | 策略 | 说明 |
|------|------|------|
| `codebase_search` | **完全阻止** | deny，强制使用 codegraph_explore |
| `file_grep` | **条件阻止** | 检测是否搜索代码符号 + 是否已调用 codegraph |
| `Read` 代码文件 | **条件警告** | 未调用 codegraph 前阻止，调用后警告 |

### 2. 状态追踪

- 使用 `session_id` 创建状态文件
- `codegraph_explore` 调用时记录状态
- 后续工具检查状态，未调用则阻止

### 3. WSL 环境适配

```bash
# 通过 cmd.exe 调用 Windows 侧的 codegraph
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" ...
```

---

## 📦 创建的文件

```
.qoder/hooks/
├── preToolUse.sh        ✅ 主 hook（拦截工具调用）
├── sessionStart.sh      ✅ 会话启动 hook（注入上下文）
├── test-hook-v2.sh      ✅ 测试脚本
├── README.md           ✅ 完整说明文档
└── SUMMARY.md          ✅ 本文档
```

---

## 🧪 测试结果

全部 7 个测试用例通过 ✅

```bash
bash .qoder/hooks/test-hook-v2.sh
```

---

## 🚀 下一步

Hooks 已配置完成，**下次启动新会话时自动生效**。

**验证方式**：
1. 开启新会话
2. 尝试搜索代码（如"查找 AppSettings"）
3. 观察是否被引导使用 `codegraph_explore`

---

## 📝 关键差异：WSL vs Mac

| 特性 | Mac 环境 | WSL 环境（本项目） |
|------|---------|------------------|
| 调用方式 | `codegraph` | `cmd.exe /c codegraph.CMD` |
| 路径格式 | Unix 路径 | Windows 路径 |
| 核心逻辑 | ✅ 相同 | ✅ 相同 |

---

**日期**: 2026-06-16  
**版本**: v2.0 (WSL 适配版)
