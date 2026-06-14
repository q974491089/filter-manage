# 工具使用规范

所有 Agent 必须遵循的统一工具使用规则。

---

## ⚠️ 强制规则：所有代码操作必须先用 CodeGraph

### 核心原则

**任何涉及代码的操作（读取、搜索、理解、分析），都必须先尝试 CodeGraph。**

包括但不限于：
- 查看代码内容
- 搜索函数/类/变量
- 理解代码结构
- 分析调用关系
- 查找文件位置
- 浏览目录结构（如果是代码目录）

### 🚫 禁止行为

**未经 CodeGraph 尝试，禁止直接使用**：
- ❌ `Read <代码文件>` - 包括 `.ts`, `.tsx`, `.js`, `.jsx`, `.rs`, `.py` 等所有代码文件
- ❌ `Read <代码目录>` - 包括 `src/`, `src/components/`, `src-tauri/` 等
- ❌ `Grep` - 禁止用于搜索代码符号（函数名、类名、变量名）
- ❌ `Glob` - 禁止用于列出代码文件
- ❌ `file_grep` / `codebase_search` - 禁止用于代码符号搜索

### ✅ 唯一合法跳过 CodeGraph 的情况

**只有以下 3 种情况才能不用 CodeGraph**：

1. **非代码文件**：
   - 配置：`package.json`, `.json`, `.yaml`, `.toml`, `.env`
   - 文档：`.md`, `.txt`, `README`, `LICENSE`
   - 样式：`.css`（纯 CSS，CSS-in-JS 仍需用 codegraph）
   
2. **纯文本内容搜索**（不是代码符号）：
   - 字符串常量：`"hello world"`
   - CSS 类名：`className="btn-primary"`
   - 注释内容：`// TODO`
   - JSON 字段名：`"apiKey"`
   
3. **CodeGraph 已尝试且明确失败**：
   - 返回 "not found" / "no results"
   - 索引未初始化
   - 文件在 staleness banner 中

### ⚠️ 降级前的强制检查清单

**不要看到一次失败就放弃！必须按顺序尝试**：

1. **检查参数是否正确**：
   - ❌ 错误：`query="src/"` - 这是路径不是查询
   - ✅ 正确：`query="frontend code structure React components"`
   - codegraph_explore 需要**自然语言描述**，不是文件路径

2. **检查索引状态**：
   ```
   codegraph_status
   ```
   如果返回 "not initialized"，建议用户运行 `codegraph init`

3. **尝试其他 codegraph 工具**：
   - `codegraph_search` - 搜索具体 symbol
   - `codegraph_files` - 列出目录下的文件
   - `codegraph_node` - 查看具体 symbol

4. **说明失败原因**：
   ```
   已尝试：
   - codegraph_explore "..." - 返回: [错误信息]
   - codegraph_status - 返回: [状态信息]
   - codegraph_search "..." - 返回: [结果]
   
   因为 [具体原因]，现在降级使用 Read。
   ```

**只有完成以上 4 步，才能用 Read！**

---

## 代码读取（最高优先级）

### CodeGraph 工具（强制第一选择）

**必须优先使用 `mcp__codegraph__*` 系列工具** 读取和理解代码：

#### 工具选择策略

| 意图 | 工具 |
|------|------|
| **几乎所有问题**（"X 怎么工作"、架构、bug、"X 在哪"） | `codegraph_explore`（**首选，第一个调用**） |
| **"X 怎么到达 Y？/ 流程 / 调用链"** | `codegraph_explore`，命名跨越流程的 symbol |
| **"X 这个 symbol 叫什么？"（只要位置）** | `codegraph_search` |
| **"谁调用了它？" / "它调用了谁？"** | `codegraph_callers` / `codegraph_callees` |
| **看某个特定 symbol 的完整源码** | `codegraph_node(includeCode=true)` |
| **"目录 X 里有什么？"** | `codegraph_files` |
| **"索引准备好了吗？/ 大小？"** | `codegraph_status` |

#### 核心原则

> **直接回答，不要委托探索**
> 
> 对于"X 怎么工作的"、架构、调用链、"X 在哪"等问题，**直接回答**——通常只需 **一次 `codegraph_explore` 调用**。

`codegraph_explore` 返回相关 symbol 的**按文件分组的完整源码**，等价于 Read 多个文件，且**大多数情况下是唯一需要的 codegraph 调用**。

#### 🚫 反模式（严格禁止）

### 绝对禁止的行为

1. **❌ 跳过 CodeGraph 直接读代码**
   ```
   违规：Read src/App.tsx
   正确：codegraph_explore "App component"
   ```

2. **❌ 用 grep 搜索代码**
   ```
   违规：Grep "loadConfig"
   正确：codegraph_search "loadConfig"
   ```

3. **❌ 用 grep 验证 codegraph 结果**
   ```
   违规：codegraph_search → Grep 验证
   正确：信任 codegraph（来自 AST 解析）
   ```

4. **❌ 链式调用代替 explore**
   ```
   违规：codegraph_search → codegraph_node → codegraph_node...
   正确：一次 codegraph_explore
   ```

5. **❌ 循环调用 node**
   ```
   违规：for symbol in symbols: codegraph_node(symbol)
   正确：一次 codegraph_explore 返回全部
   ```

### 违规后果

**如果违反以上规则**：
- 浪费 token（重复查询）
- 效率低下（多次调用代替一次）
- 结果不准确（grep 不理解代码结构）

**记住**：CodeGraph 是 **AST 解析 + 知识图谱**，比 grep 强大 100 倍。

#### 例外场景（CodeGraph 不支持的内容）

**只有以下情况才允许用 Read/Grep/Glob**：

1. **非代码文件**：
   - 配置文件：`.json`, `.yaml`, `.toml`, `.env`
   - 文档文件：`.md`, `.txt`, `README`
   - 样式文件：`.css`（纯 CSS，不是 CSS-in-JS）
   
2. **纯文本搜索**（不是代码结构）：
   - CSS 类名：`className="xxx"`
   - 字符串常量：`"hello world"`
   - 注释内容：`// TODO: fix this`
   - JSON key：查找配置字段名

3. **CodeGraph 明确失败后**：
   - 工具返回 "not found" 或 "no results"
   - 索引未初始化（需先运行 `codegraph init`）
   - 文件在 staleness banner 中列出

**强制流程**：
```
涉及代码的任何操作
  ↓
尝试 codegraph_explore（万能工具，第一选择）
  ↓
不够？→ 尝试 codegraph_search / codegraph_node / codegraph_callers
  ↓
仍然解决不了？→ 说明为什么 codegraph 不行 → 才能用 Read/Grep
```

**判断标准**：
- 要读的是代码文件（`.ts`, `.tsx`, `.js`, `.rs` 等）？ → 必须先 codegraph
- 要看的是代码目录（`src/`, `components/`）？ → 必须先 codegraph  
- 要搜索的是代码符号（函数名、类名）？ → 必须先 codegraph
- 要理解代码结构/调用关系？ → 必须先 codegraph

只有上述都不是（配置文件、文档、纯文本搜索），或者 codegraph 已尝试且失败，才能直接 Read/Grep。

**错误示例**（违规）：
```
❌ 用户："查看 App.tsx 的代码"
   Agent：直接 Read src/App.tsx  # 违规！应该用 codegraph_explore

❌ 用户："找到 loadConfig 函数"
   Agent：Grep "loadConfig"  # 违规！应该用 codegraph_search

❌ 用户："分析我负责的代码部分"
   Agent：Read src → Read src/components → Read src/hooks  # 违规！应该用 codegraph_explore

❌ 用户："看看前端代码结构"
   Agent：Read . → Read src → Read src/components  # 违规！应该用 codegraph_explore
```

**正确示例**：
```
✅ 用户："查看 App.tsx 的代码"
   Agent：codegraph_explore "App component main entry"

✅ 用户："找到 loadConfig 函数"
   Agent：codegraph_search "loadConfig"

✅ 用户："分析我负责的代码部分"
   Agent：codegraph_explore "frontend code structure src components hooks"

✅ 用户："看看前端代码结构"
   Agent：codegraph_explore "frontend architecture React components"

✅ 用户："查看 package.json"
   Agent：Read package.json  # 允许，非代码文件
```

#### WSL 环境调用

本项目在 WSL 环境下开发，但 codegraph 需要在 Windows 环境下运行。

**MCP 工具调用**（推荐）：
- MCP Server 已配置在 `.mcp.json`，启动时用 `--path C:\Users\myuser\Projects\filter-manage`
- **调用时不要传 `projectPath` 参数**，让工具使用 Server 启动时的路径
- ❌ 错误：`codegraph_explore({projectPath: "/mnt/c/...", query: "..."})`
- ✅ 正确：`codegraph_explore({query: "frontend React components"})`

**CLI 调用方式**（MCP 工具失败时降级）：

```bash
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" <command> [args]
```

**常用命令**：
- `context "<query>"` - 构建上下文（自然语言查询）
- `status` - 查看索引状态
- `search "<name>"` - 搜索 symbol

**示例**：
```bash
# 查询上下文（自然语言）
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" context "frontend React components App ConfigManager"

# 查看状态
cmd.exe /c "C:\Users\Administrator\AppData\Local\pnpm\bin\codegraph.CMD" status
```

#### Staleness 注意

当工具响应以 "⚠️ Some files referenced below were edited since the last index sync..." 开头时，列出的文件正在等待重新索引——对这些特定文件用 `Read` 获取准确内容。Banner 中**没有列出的文件都是最新的**，仍然信任 codegraph。

---

## 文档查询

### ctx7 技能（优先）

**优先使用 ctx7** (`npx ctx7@latest`) 查询官方文档：
- API 语法
- 配置选项
- 版本迁移
- 库特定调试
- 设置说明
- CLI 工具用法

**即使是已知技术（React, Next.js, Prisma 等）也要用 ctx7**——训练数据可能过时。

#### 使用步骤

1. **解析库名**：
   ```bash
   npx ctx7@latest library <name> "<user's question>"
   ```
   使用官方库名（带正确标点）：`Next.js` 不是 `nextjs`，`Three.js` 不是 `threejs`

2. **选择最佳匹配**（ID 格式：`/org/project`）：
   - 精确名称匹配
   - 描述相关性
   - 代码片段数量
   - 来源声誉（High/Medium 优先）
   - 基准分数（越高越好）

3. **获取文档**：
   ```bash
   npx ctx7@latest docs <libraryId> "<user's question>"
   ```

4. **用获取的文档回答**

**版本特定文档**：使用 `/org/project/version` 格式（如 `/vercel/next.js/v14.3.0`）

**配额限制**：如果失败，告知用户运行 `npx ctx7@latest login` 或设置 `CONTEXT7_API_KEY` 环境变量。

### Tavily MCP（降级）

**ctx7 没找到的内容，降级使用 Tavily**：

```
mcp__tavily__tavily_search
mcp__tavily__tavily_extract
mcp__tavily__tavily_crawl
```

**不要使用 `WebSearch`** - 统一使用 Tavily。

---

## 优先级总结

### 代码读取
```
CodeGraph MCP > Read/Grep/Glob
```

### 文档查询
```
ctx7 > Tavily > WebSearch
```

### 通用规则

- 遇到不熟悉的技术或 API，**先查文档再写代码**
- 不要从训练数据回答 API 细节，**始终验证当前文档**
- 代码读取用 codegraph，纯文本搜索（CSS 类名、字符串）才用 grep
