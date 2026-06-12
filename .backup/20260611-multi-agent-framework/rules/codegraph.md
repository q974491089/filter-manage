---
description: 代码查询优先使用 CodeGraph 工具，以及 CodeGraph 自动更新特性说明
alwaysApply: true
---

# Codegraph — code intelligence over an indexed knowledge graph

Codegraph 是工作区中所有 symbol、edge 和文件的 SQLite 知识图谱。查询是亚毫秒级的；索引通过 file watcher 滞后写入约一秒。**在写代码或编辑代码之前查阅它，不是在过程中。**

## 核心原则：直接回答，不要委托探索

对于"X 怎么工作的"、架构、调用链、"X 在哪"等问题，**直接回答**——通常只需 **一次 `codegraph_explore` 调用**。

`codegraph_explore` 接受自然语言问题或 symbol/文件名集合，返回相关 symbol 的**按文件分组的完整源码**，所以它等价于 Read 多个文件，且**大多数情况下是你唯一需要的 codegraph 调用**。

Codegraph 本身就是预构建的搜索索引——把查找委托给单独的文件读取子任务/agent，或自己跑 grep + read 循环，是在重复 codegraph 已经做过的工作，花更多代价得到相同答案。**只有在 codegraph 确实没覆盖到的具体细节时，才用 Read/Grep。**

直接用 codegraph 回答通常 1~3 次调用；用 grep/read 探索则需要几十次调用。

## 工具选择策略

| 意图 | 工具 |
|------|------|
| **几乎所有问题**——"X 怎么工作"、架构、bug、"X 在哪"、调研某区域 | `codegraph_explore`（**首选，第一个调用；一次调用返回相关 symbol 源码按文件分组；大多数情况下是唯一需要的调用**） |
| **"X 怎么到达 Y？/ 流程 / 从 X 到 Y 的路径"** | `codegraph_explore`，命名跨越流程的 symbol（如 `mutateElement renderScene`）——它能发现动态分发跳转（回调、React re-render、JSX children）grep 无法追踪的调用路径 |
| **"X 这个 symbol 叫什么？"（只要位置）** | `codegraph_search` |
| **"谁调用了它？" / "它调用了谁？" / "改它会破坏什么？"** | `codegraph_callers` / `codegraph_callees` / `codegraph_impact` |
| **看某个特定 symbol 的完整源码**（explore 截断了的，或重名的） | `codegraph_node(includeCode=true)`：对于有歧义的名称，它一次返回所有匹配定义的完整代码 |
| **"目录 X 里有什么？"** | `codegraph_files` |
| **"索引准备好了吗？/ 大小？"** | `codegraph_status` |

## 常用链路

- **理解流程 / "X 怎么到达 Y"**：一次 `codegraph_explore`，命名跨越流程的 symbol——它发现调用路径（包括动态分发跳转）并返回源码。不需要用 `codegraph_search` + `codegraph_callers` 重建路径。
- **入门 / 理解任何区域**：一次 `codegraph_explore` 通常就是完整答案。只有在某处仍不清楚时才跟进 `codegraph_node` 看特定 symbol。
- **重构规划**：`codegraph_search` → `codegraph_callers` → `codegraph_impact`。爆炸半径答案来自 impact，不是手动遍历 callers。
- **调试回归**：`codegraph_callers` 找可疑 symbol 的调用者；如果出现意外调用，用 `codegraph_impact` 扩大范围。

## 反模式（禁止）

- ❌ **不要用 grep 重新验证 codegraph 的结果**——它们来自完整 AST 解析；用 grep 复查更慢、更不准确、浪费上下文。
- ❌ **不要先 grep 再查 symbol**——`codegraph_search` 更快且返回 kind + location + signature。
- ❌ **不要用 `codegraph_search` + `codegraph_node` 链式调用来理解一个区域**——一次 `codegraph_explore` 在一个往返中返回相关 symbol 源码。
- ❌ **不要循环调用 `codegraph_node`**——一次 `codegraph_explore` 按文件分组返回它们全部，而每次单独调用重新读取整个上下文且成本更高。`codegraph_node` 用于看单个 symbol。
- ❌ **不要在 codegraph 能做的事上用 file_grep / codebase_search**——只有搜纯文本（CSS 类名、字符串常量、注释内容、JSON key）时才用 file_grep。

## 编辑后注意 staleness banner

当工具响应以"⚠️ Some files referenced below were edited since the last index sync…"开头时，列出的文件正在等待重新索引——对这些特定文件用 Read 获取准确内容。banner 中**没有列出的文件都是最新的**，仍然信任 codegraph。`codegraph_status` 也会在 "Pending sync" 下列出待同步文件。

## 局限性

- 如果工具报告项目未初始化，`.codegraph/` 还不存在——提议运行 `codegraph init -i` 来构建索引。
- 索引滞后文件写入约 1 秒。
- 跨文件解析是尽力而为的名称匹配；有歧义的调用可能返回多个候选。
- 不做活跃的正确性验证——那仍然是 TypeScript compiler / test suite / linter 的工作。Codegraph 用它们没有的结构化上下文来补充它们。

## 配置注意

在 `.claude/settings.local.json` 或 MCP 配置中配置 codegraph 时，**必须**通过 `--path` 参数指定项目绝对路径：

```json
"codegraph": {
  "type": "stdio",
  "command": "codegraph",
  "args": ["serve", "--mcp", "--path", "<项目绝对路径>"]
}
```

不加 `--path` 会导致 "CodeGraph not initialized" 错误。

## WSL 环境下调用方式

本项目在 WSL 环境下开发，但 codegraph 需要在 Windows 环境下运行。

### MCP 配置（.mcp.json）

```json
"codegraph": {
  "command": "cmd.exe",
  "args": [
    "/c",
    "C:\\Users\\Administrator\\AppData\\Local\\pnpm\\bin\\codegraph.CMD",
    "serve",
    "--mcp",
    "--path",
    "C:\\Users\\myuser\\Projects\\filter-manage"
  ]
}
```

### CLI 调用方式

当 MCP 工具调用失败时，降级使用 CLI：

```bash
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" <command> --path "C:\Users\myuser\Projects\filter-manage" [args]
```

**常用命令：**
- `context` - 构建上下文（输出 markdown）
- `query` - 搜索 symbol
- `callers` - 查找调用者
- `callees` - 查找被调用者
- `impact` - 分析影响范围
- `status` - 查看索引状态

**示例：**
```bash
# 查询上下文
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" context --path "C:\Users\myuser\Projects\filter-manage" "AppSettings loadSettings"

# 搜索 symbol
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" query --path "C:\Users\myuser\Projects\filter-manage" "AppSettings"

# 查看状态
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" status --path "C:\Users\myuser\Projects\filter-manage"
```
