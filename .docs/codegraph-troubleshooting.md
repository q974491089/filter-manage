# CodeGraph 常见问题排查

## 🐛 问题 1: AI 陷入无限循环思考

### 症状
```
Thinking
需要查看 X...使用 codegraph_node...
⚙codegraph_codegraph_node [symbol=X, includeCode=true]
Thinking
输出被截断了...需要查看 X...使用 codegraph_node...
⚙codegraph_codegraph_node [symbol=X, includeCode=true]
Thinking
输出被截断了...需要查看 X...使用 codegraph_node...
(循环重复)
```

### 原因
1. **大型组件内容被截断** - codegraph_node 返回内容超出限制
2. **AI 不知道该降级** - 没意识到应该用 `read` 工具
3. **Instructions 过于强调 codegraph** - 让 AI 以为必须用 codegraph

### 解决方案

✅ **已修复**：更新了 instructions，明确说明：
- ❌ 大型组件（>500 行）不要用 codegraph_node includeCode
- ✅ 审查完整组件、UI 设计时直接用 `read`

**手动干预**（如果 AI 还在循环）：
```
请直接用 read 工具读取 src/components/SettingsModal.tsx，不要用 codegraph_node
```

---

## 🐛 问题 2: AI 还在疯狂 search

### 症状
```
User: "分析后端"
⚙ search("*.rs")
⚙ Read(lib.rs)
⚙ search("command")
⚙ Read(nvidia.rs)
...
```

### 原因
1. **配置未生效** - 需要重启 CLI
2. **instructions 未加载** - 配置文件路径错误
3. **AI 习惯未改变** - 需要多次强化

### 解决方案

1. **重启 CLI**
   ```bash
   # 完全退出再重新启动
   opencode  # 或 qoder
   ```

2. **检查配置**
   ```bash
   # OpenCode
   cat .opencode/opencode.json
   # 确认有 "instructions": [".opencode/instructions.md"]
   
   # Qoder
   cat .qoder/settings.json
   # 确认有 "instructions": [".qoder/instructions.md"]
   ```

3. **手动提示**
   ```
   请用 codegraph_context 一次性分析，不要用 search + read 循环
   ```

4. **验证 instructions 内容**
   ```bash
   # 确认文件存在且内容正确
   cat .opencode/instructions.md
   cat .qoder/instructions.md
   ```

---

## 🐛 问题 3: codegraph_node 返回 "输出被截断"

### 症状
```
⚙codegraph_codegraph_node [symbol=LargeComponent, includeCode=true]
返回: ... (truncated) ...
```

### 原因
- 组件代码太大（>500 行）
- codegraph 有输出限制

### 解决方案

**不要用 codegraph_node includeCode，改用 read**：
```
❌ 错误: codegraph_node symbol=SettingsModal includeCode=true
✅ 正确: read src/components/SettingsModal.tsx
```

**或者只看符号信息，不看代码**：
```
codegraph_node symbol=SettingsModal includeCode=false
→ 返回: 位置、类型、调用关系（无代码）
```

---

## 🐛 问题 4: Hook 未生效（Qoder）

### 症状
```bash
tail -f /tmp/qoder-codegraph-hook.log
# 没有任何输出
```

### 原因
1. 脚本无执行权限
2. Hook 配置错误
3. qoder 未重启

### 解决方案

1. **检查权限**
   ```bash
   ls -la .qoder/hooks/codegraph-read.sh
   # 应该有 x 权限（-rwxr-xr-x）
   
   # 如果没有，添加权限
   chmod +x .qoder/hooks/codegraph-read.sh
   ```

2. **检查配置**
   ```bash
   cat .qoder/settings.json
   # 确认 hooks 配置存在
   ```

3. **重启 qoder**
   ```bash
   # 完全退出再启动
   qoder
   ```

4. **测试 hook**
   ```bash
   .qoder/hooks/test-hook.sh
   # 查看是否有输出
   ```

---

## 🐛 问题 5: Custom Tool 未生效（OpenCode）

### 症状
- AI 调用 read 时仍然是原生读取
- 没有看到 "Read via CodeGraph" 标记

### 原因
1. Custom tool 文件有语法错误
2. OpenCode 未重启
3. TypeScript 类型错误

### 解决方案

1. **检查文件**
   ```bash
   cat .opencode/tools/read.ts
   # 确认语法正确
   ```

2. **重启 opencode**
   ```bash
   opencode
   ```

3. **查看 console 日志**
   ```
   应该看到:
   [codegraph-read] Processing read request: ...
   [codegraph-read] ✓ Successfully read via codegraph
   ```

---

## 🐛 问题 6: codegraph 命令失败

### 症状
```bash
[2026-06-12] Codegraph exit code: 1
[2026-06-12] Codegraph failed, falling back to native read
```

### 原因
1. codegraph 索引过期
2. 文件未被索引
3. 查询语法错误

### 解决方案

1. **检查索引状态**
   ```bash
   cd /mnt/c/Users/myuser/Projects/filter-manage
   codegraph status
   ```

2. **重新同步索引**
   ```bash
   codegraph sync
   ```

3. **手动测试命令**
   ```bash
   codegraph context "App.tsx"
   # 应该返回代码上下文
   ```

4. **查看详细日志**
   ```bash
   tail -100 /tmp/qoder-codegraph-hook.log
   # 查看实际执行的命令和错误信息
   ```

---

## 📋 快速诊断清单

```bash
# 1. 检查配置文件
cat .opencode/opencode.json
cat .qoder/settings.json

# 2. 检查 instructions 文件
cat .opencode/instructions.md
cat .qoder/instructions.md

# 3. 检查 hook 权限
ls -la .qoder/hooks/codegraph-read.sh

# 4. 测试 hook
.qoder/hooks/test-hook.sh

# 5. 查看日志
tail -50 /tmp/qoder-codegraph-hook.log

# 6. 检查 codegraph 状态
codegraph status

# 7. 重启 CLI
# 完全退出后重新启动
```

---

## 💡 最佳实践

1. **遇到循环时手动干预**
   ```
   "请停止循环，直接用 read 工具读取文件"
   ```

2. **大型文件用 read，小型符号用 codegraph**
   - 完整组件 → read
   - 查找符号定义 → codegraph_search + codegraph_node (不加 includeCode)
   - 理解架构 → codegraph_context

3. **配置修改后重启 CLI**
   - instructions 修改 → 重启
   - hook 修改 → 重启
   - custom tool 修改 → 重启

4. **善用日志调试**
   ```bash
   # Qoder hook 日志
   tail -f /tmp/qoder-codegraph-hook.log
   
   # OpenCode console 输出
   # 直接在终端查看
   ```

---

## 🆘 还是不行？

提供以下信息：
1. 使用的 CLI（opencode/qoder）
2. 完整的错误信息或循环 thinking 内容
3. 相关配置文件内容
4. 日志输出（如果有）
