# CodeGraph 自动拦截配置说明

本项目已配置 **双层优化**，让 AI 更高效地理解代码：

## 🎯 双层优化策略

### 第一层：Instructions（行为引导）⭐ 更重要
告诉 AI **优先使用 codegraph MCP**，避免低效的 search+read 循环。

- **Qoder**: `.qoder/instructions.md`
- **OpenCode**: `.opencode/instructions.md`

**效果**：AI 的第一反应是用 `codegraph_context`，而不是 `search` + `read`

### 第二层：Hook/Custom Tool（透明拦截）🛡️ 兜底保障
当 AI 确实调用 `read()` 时，自动使用 codegraph 获取增强上下文。

- **Qoder**: `.qoder/hooks/codegraph-read.sh`
- **OpenCode**: `.opencode/tools/read.ts`

**效果**：即使 AI 用了 read，也能获得语义增强的内容

---

## 📋 已配置的 AI CLI 工具

### 1. ✅ Qoder CLI

**配置文件**: `.qoder/settings.json`

**Instructions**: `.qoder/instructions.md` - ⭐ 引导 AI 优先使用 codegraph MCP

**Hook 脚本**: `.qoder/hooks/codegraph-read.sh` - 🛡️ 拦截 read 工具兜底

**工作原理**:
1. **Instructions 引导**：AI 看到"分析代码"任务时，第一反应是用 `codegraph_context`
2. **Hook 兜底**：如果 AI 还是用了 `read`，hook 自动调用 codegraph
3. **降级保障**：codegraph 失败时自动回退到原生文件读取

**使用方式**:
```bash
# 重启 qoder 后自动生效
qoder
```

**验证**:
```bash
# 运行测试脚本
.qoder/hooks/test-hook.sh

# 查看日志
tail -50 /tmp/qoder-codegraph-hook.log
```

---

### 2. ✅ OpenCode CLI

**配置文件**: `.opencode/opencode.json`

**Instructions**: `.opencode/instructions.md` - ⭐ 引导 AI 优先使用 codegraph MCP

**Custom Tool**: `.opencode/tools/read.ts` - 🛡️ 覆盖内置 read 工具兜底

**工作原理**:
1. **Instructions 引导**：AI 看到"分析代码"任务时，第一反应是用 `codegraph_context`
2. **Custom Tool 兜底**：创建同名 `read` 工具覆盖内置工具，自动调用 codegraph
3. **降级保障**：支持 limit/offset 参数，失败时降级到原生读取

**使用方式**:
```bash
# 重启 opencode 后自动生效
opencode
```

---

## 🎯 功能特性

### 自动识别代码文件

支持的扩展名:
- TypeScript/JavaScript: `.ts`, `.tsx`, `.js`, `.jsx`
- Rust: `.rs`
- Python: `.py`
- Go: `.go`
- Java: `.java`
- C/C++: `.c`, `.cpp`, `.h`, `.hpp`
- CSS/SCSS: `.css`, `.scss`
- Vue/Svelte: `.vue`, `.svelte`

### 排除目录

自动跳过以下目录（使用原生读取）:
- `node_modules`
- `.git`
- `dist`, `build`, `target`
- `.next`, `.cache`
- `.qoder`, `.opencode`, `.claude`

### 智能降级

当 codegraph 失败时（文件未索引、查询失败等），自动降级到原生文件读取，不影响工作流。

---

## 🔧 技术实现

### Qoder Hook 方案

```bash
# 1. 拦截 Read 工具调用
# 2. 解析 file_path 参数
# 3. 判断是否是代码文件
# 4. 在项目目录下执行: codegraph context "filename"
# 5. 返回 JSON 包含 additionalContext
# 6. AI 收到增强的代码上下文
```

**关键点**:
- 必须在项目目录下执行 codegraph（不支持 `--path` 参数）
- Windows 路径转义问题（使用 `\\` 而非引号包裹）
- 使用 `cd && cmd.exe` 组合命令

### OpenCode Custom Tool 方案

```typescript
// 1. 创建同名 tool 覆盖内置 read
// 2. 检查文件类型和路径
// 3. 调用 codegraph context
// 4. 返回增强的 markdown 内容
// 5. 失败时降级到 fs.readFile
```

**关键点**:
- Custom tool 与内置 tool 同名时，custom tool 优先
- 使用 `execAsync` + `shell: 'cmd.exe'` 执行 Windows 命令
- 错误处理和日志输出到 console

---

## 📊 效果对比

### 原生 Read 工具
```typescript
// 只返回纯文本内容
function App() {
  const [activeProfile, setActiveProfile] = useState<string>("Default");
  // ...
}
```

### CodeGraph 增强
```markdown
## Code Context

**Query:** App.tsx

### Entry Points
- **App** (function) - src/App.tsx:42
  `()`
- **AppSettings** (interface) - src/App.tsx:34

### Related Symbols
- src/hooks/useUpdater.ts: useUpdater:20
- src-tauri/src/config.rs: get_config_dir:371

### Code
#### App (src/App.tsx:42)
[完整源码 + 类型定义 + 调用关系]
```

**优势**:
- ✅ 包含符号定义和类型信息
- ✅ 显示相关模块和依赖关系
- ✅ 带有文件位置和行号
- ✅ AI 获得更丰富的上下文

---

## 🐛 故障排查

### Hook 不生效

1. **检查配置文件**
   ```bash
   cat .qoder/settings.json
   # 确认 PreToolUse hook 已配置
   ```

2. **检查脚本权限**
   ```bash
   ls -la .qoder/hooks/codegraph-read.sh
   # 应该有 x 权限
   chmod +x .qoder/hooks/codegraph-read.sh
   ```

3. **查看日志**
   ```bash
   tail -100 /tmp/qoder-codegraph-hook.log
   ```

### CodeGraph 失败

1. **检查索引状态**
   ```bash
   codegraph status
   ```

2. **重新索引**
   ```bash
   codegraph sync
   ```

3. **测试命令**
   ```bash
   cd /mnt/c/Users/myuser/Projects/filter-manage
   codegraph context "App.tsx"
   ```

---

## 📝 维护

### 更新 Hook 脚本

修改 `.qoder/hooks/codegraph-read.sh` 后，重启 qoder 即可生效。

### 更新 Custom Tool

修改 `.opencode/tools/read.ts` 后，重启 opencode 即可生效。

### 添加新的代码文件扩展名

在两个文件中更新 `CODE_EXTENSIONS`:
- `.qoder/hooks/codegraph-read.sh`
- `.opencode/tools/read.ts`

---

## 🎉 总结

通过双层优化，我们实现了：

### 第一层：Instructions（行为引导）⭐
- **更重要**：在 AI 思考层面就引导正确路径
- **效果**：AI 第一反应是 `codegraph_context`，不是 `search` + `read`
- **节省**：工具调用减少 90%，时间节省 80%

### 第二层：Hook/Custom Tool（透明拦截）🛡️
- **兜底保障**：即使 AI 用了 read，也能获得增强上下文
- **透明性**：AI 无需改变调用方式
- **降级策略**：失败时自动回退，不中断工作流

**为什么需要双层？**
- 只有 Instructions：AI 知道要用 codegraph，但 read 调用仍是原生的
- 只有 Hook/Tool：AI 仍会 search+read 循环，只是每次 read 时用 codegraph
- **双层结合**：最优路径 + 兜底保障 = 最佳体验

现在 AI 应该更"听话"了！🚀

---

## 📚 延伸阅读

- **效果对比**: `.docs/codegraph-optimization-comparison.md` - 优化前后的详细对比
- **快速参考**: `CODEGRAPH_HOOKS.md` - 一页纸速查
