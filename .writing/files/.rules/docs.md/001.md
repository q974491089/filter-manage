# 文档同步规范

Agent 间协作通过文档同步实现。以下规则适用于所有 Agent。

---

## 后端 → 前端：API 变更通知

### 触发条件

当后端发生以下变更时：
- 新增 Tauri 命令
- 修改命令签名（参数/返回值）
- 修改命令行为
- 删除命令

### 同步步骤

#### 1. 更新 API 文档

修改 `.docs/api/<module>.md`：

**新增接口**：
```markdown
## `command_name`

**参数**：
- `param1` (`Type`) - 说明

**返回值**：
```rust
Result<ResponseType, String>
```

**示例**：
```typescript
const result = await invoke("command_name", { param1: value });
```

**新增于**：YYYY-MM-DD
```

**修改接口**：
```markdown
（更新参数/返回值说明）

**更新于**：YYYY-MM-DD — 修改了返回值结构，新增 xxx 字段
```

**删除接口**：
直接删除该条目。

#### 2. 更新迭代记录

在 `.docs/README.md` 当前版本块追加一行：

```markdown
### v0.3.0 — 2026-06-11 · 功能增强

<一句话描述>

| 功能 | 说明 | 文档 |
|------|------|------|
| 新增功能 | 简洁描述（≤20字） | [.docs/api/xxx.md#anchor] |
```

**规则**：
- 每次迭代新开一个 `### v<版本>` 块，**不修改历史块**
- 每行只写一个功能点
- 文档链接直接指向具体文件（可带锚点）

#### 3. 写入完成信号

在 `SYNC_STATUS.md` 写入固定标识：

```
【文档已完成同步更新】YYYY-MM-DD — <本次变更简述>
```

**这个信号会触发 Frontend Agent 重新读取文档。**

---

## 前端 → 后端：API 需求传递

### 触发条件

当前端需要新的后端能力时。

### 交接文档格式

在 `.docs/handoff/` 创建 `<feature>-backend.md`：

```markdown
# [功能名称] - 后端需求

## 前端需求

**功能**：[功能描述]
**调用方式**：`invoke("command_name", { param: type })`
**期望返回**：`{ field: type }`
**使用场景**：[在哪个组件/交互中使用]

## 前端已实现

- [已完成的前端部分]

## 需要后端提供

1. [具体需求 1]
2. [具体需求 2]

## 错误处理

[期望的错误场景和错误消息]
```

---

## 后端 → 前端：实现完成通知

### 交接文档格式

在 `.docs/handoff/` 创建 `<feature>-frontend.md`：

```markdown
# [功能名称] - 前端交接

## 后端已完成

**新增命令**：`command_name`
**参数**：
```rust
struct Params {
    param1: Type,
    param2: Type,
}
```

**返回值**：
```rust
struct Response {
    field1: Type,
    field2: Type,
}
```

**错误码**：
- `"error_code_1"` - 场景说明

## 前端需要实现

1. 在 `<Component.tsx>` 中调用：
   ```typescript
   const result = await invoke("command_name", { 
     param1: value1,
     param2: value2 
   });
   ```

2. 处理返回值：
   ```typescript
   if (result.field1) {
     // 处理逻辑
   }
   ```

3. 错误处理：
   ```typescript
   try {
     const result = await invoke("command_name", params);
   } catch (error) {
     if (error === "error_code_1") {
       // 处理特定错误
     }
   }
   ```

## 示例代码

[完整的前端调用示例]

## 测试场景

[如何测试这个功能]
```

---

## 文档维护责任

| 文档 | 维护者 | 更新时机 |
|------|--------|---------|
| `.docs/api/*.md` | Backend Agent | 每次后端代码变更 |
| `.docs/README.md` | Backend Agent | 每次后端代码变更 |
| `SYNC_STATUS.md` | Backend Agent | 文档同步完成后 |
| `.docs/handoff/*-backend.md` | Frontend Agent | 需要后端支持时 |
| `.docs/handoff/*-frontend.md` | Backend Agent | 后端实现完成后 |
| `.docs/architecture.md` | 双方协商 | 架构变更时 |
| `.docs/frontend-guide.md` | Frontend Agent | 前端结构变更时 |

---

## 协作流程示例

### 场景：前端需要"导出配置"功能

**1. Frontend Agent 创建需求文档**：

`.docs/handoff/export-config-backend.md`
```markdown
## 前端需求
功能：导出当前配置为 JSON 文件
调用方式：invoke("export_config", { path: string })
期望返回：{ success: boolean, path: string }
```

**2. Backend Agent 实现功能**：

修改 `src-tauri/src/config.rs`，新增 `export_config` 命令。

**3. Backend Agent 更新文档**：

- 更新 `.docs/api/config.md`
- 在 `.docs/README.md` 追加一行
- 写入 `SYNC_STATUS.md`：`【文档已完成同步更新】2026-06-11 — 新增导出配置功能`

**4. Backend Agent 创建交接文档**：

`.docs/handoff/export-config-frontend.md`（含完整前端调用示例）

**5. Frontend Agent 看到同步信号**：

重新读取 `.docs/api/config.md` 和交接文档，实现前端调用。

---

## 最佳实践

1. **API 文档是唯一权威来源** - 前端优先读文档，不猜测接口
2. **变更必须同步** - 后端改了代码就改文档，不延后
3. **完成信号明确** - 用固定标识，不用模糊描述
4. **交接文档完整** - 含示例代码，不只写接口签名
5. **保持文档与代码一致** - 代码改了，文档立即改
