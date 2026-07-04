# Agent 交接文档格式

Agent 间通过 `.docs/handoff/` 目录交换工作。

---

## 文件命名

```
.docs/handoff/<feature>-<target-agent>.md
```

**示例**：
- `export-config-backend.md` - 前端请求后端实现
- `export-config-frontend.md` - 后端实现完成，交给前端
- `icon-field-frontend.md` - 后端完成图标字段，前端需适配

---

## 前端 → 后端

**文件名**：`<feature>-backend.md`

**模板**：

```markdown
# [功能名称] - 后端需求

## 前端需求

**功能**：[功能描述]
**调用方式**：`invoke("command_name", { param: type })`
**期望返回**：`{ field: type }`
**使用场景**：[在哪个组件/交互中使用]

## 背景

[为什么需要这个功能，解决什么问题]

## 前端已实现

- [已完成的前端部分]
- [占位代码位置]

## 需要后端提供

1. [具体需求 1]
2. [具体需求 2]

## 错误处理

[期望的错误场景和错误消息]

## 测试场景

[如何测试这个功能]
```

---

## 后端 → 前端

**文件名**：`<feature>-frontend.md`

**模板**：

```markdown
# [功能名称] - 前端交接

## 后端已完成

### 新增命令：`command_name`

**参数**：
```rust
struct Params {
    param1: Type,  // 说明
    param2: Type,  // 说明
}
```

**返回值**：
```rust
struct Response {
    field1: Type,  // 说明
    field2: Type,  // 说明
}
```

**错误码**：
- `"error_code_1"` - 场景说明
- `"error_code_2"` - 场景说明

### 相关文件

- 修改了：`src-tauri/src/xxx.rs`
- 新增了：`src-tauri/src/yyy.rs`

## 前端需要实现

### 1. 基本调用

在 `<Component.tsx>` 中：

```typescript
import { invoke } from "@tauri-apps/api/core";

const result = await invoke<ResponseType>("command_name", { 
  param1: value1,
  param2: value2 
});
```

### 2. 处理返回值

```typescript
if (result.field1) {
  // 处理逻辑
}
```

### 3. 错误处理

```typescript
try {
  const result = await invoke("command_name", params);
} catch (error) {
  if (error === "error_code_1") {
    // 处理特定错误
    toast.error("错误提示");
  } else {
    // 通用错误处理
    toast.error(`操作失败: ${error}`);
  }
}
```

## 完整示例

```typescript
// src/components/FeatureComponent.tsx

const handleAction = async () => {
  try {
    const result = await invoke<ResponseType>("command_name", {
      param1: state.value1,
      param2: state.value2,
    });
    
    // 更新 UI 状态
    setState(prev => ({
      ...prev,
      field1: result.field1,
      field2: result.field2,
    }));
    
    toast.success("操作成功");
  } catch (error) {
    console.error("操作失败:", error);
    toast.error(`操作失败: ${error}`);
  }
};
```

## 测试场景

1. **正常流程**：[如何测试]
2. **边界情况**：[如何触发]
3. **错误场景**：[如何复现]

## 注意事项

- [特殊说明 1]
- [特殊说明 2]

## API 文档

详见 `.docs/api/<module>.md#command_name`
```

---

## DevOps → 全体

**文件名**：`<change>-all-agents.md`

**模板**：

```markdown
# [变更名称] - 全体通知

## 变更内容

[描述变更]

## 影响范围

- Frontend Agent：[影响说明]
- Backend Agent：[影响说明]
- DevOps Agent：[影响说明]

## 需要适配

### Frontend
- [具体适配步骤]

### Backend
- [具体适配步骤]

## 生效时间

[何时生效，如何回滚]
```

---

## 最佳实践

1. **标题清晰** - 功能名 + 目标 agent
2. **示例完整** - 含可直接复制的代码
3. **错误处理明确** - 列出所有错误码和场景
4. **测试场景具体** - 说明如何验证功能
5. **及时删除** - 功能完成后，归档或删除交接文档（避免累积）
6. **双向确认** - 实现方完成后，在文档底部加 `[已完成] YYYY-MM-DD`
7. **不写真实敏感信息** - 服务器地址、SSH key 路径、账号、密码、token、cookie、secret 一律写成占位符；真实值放 `.env.local`（本地）或 GitHub Secrets（CI）

---

## 归档策略

功能完成并验证后，将交接文档移动到 `.docs/handoff/archive/YYYY-MM/`：

```bash
mkdir -p .docs/handoff/archive/2026-06/
mv .docs/handoff/export-config-*.md .docs/handoff/archive/2026-06/
```

保留近 3 个月的归档，之后可删除。
