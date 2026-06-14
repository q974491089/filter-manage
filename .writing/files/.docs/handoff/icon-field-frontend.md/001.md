# Icon 字段支持 - 前端交接文档

## 后端已完成的改动

### 1. `ColorConfig` 新增 `icon` 字段

**文件**: `src-tauri/src/config.rs`

```rust
pub struct ColorConfig {
    pub name: String,
    pub icon: Option<String>,  // 新增：Material Icon 名称
    pub brightness: i32,
    pub contrast: i32,
    pub gamma: f64,
    pub digital_vibrance: i32,
    pub icc_profile: Option<String>,
}
```

旧配置文件没有 `icon` 字段时 serde 自动设为 `None`，无需迁移。

### 2. `list_configs` 返回类型变更

**返回类型由 `string[]` 改为 `ColorConfig[]`**，前端一次调用即可获取所有配置的完整信息（含 `icon`）。

```ts
// 旧
const names: string[] = await invoke('list_configs')

// 新
const configs: ColorConfig[] = await invoke('list_configs')
```

---

## 前端需要的改动

### 1. App.tsx — `configs` 状态类型变更

```tsx
// 旧
const [configs, setConfigs] = useState<string[]>([]);
const result = await invoke<string[]>("list_configs");

// 新
const [configs, setConfigs] = useState<ColorConfig[]>([]);
const result = await invoke<ColorConfig[]>("list_configs");
```

传递给子组件时按需提取 `name`：
- `<ConfigManager configs={configs} />` → 子组件 props 类型改为 `ColorConfig[]`

### 2. ConfigManager.tsx — props 与渲染适配

```tsx
// props 类型
configs: ColorConfig[];  // 原来是 string[]

// 渲染列表
{configs.map((config) => {
  const name = config.name;
  const icon = config.icon || "tune";
  // ...
})}
```

### 3. 编辑弹窗 — 直接从列表读取 icon

不再需要为每个配置单独调用 `load_config` 获取 icon：

```tsx
// 旧：逐个 load_config
const config = await invoke<ColorConfig>("load_config", { name });
setEditingIcon(config.icon || "tune");

// 新：直接从 configs 列表查找
const config = configs.find(c => c.name === name);
setEditingIcon(config?.icon || "tune");
```

---

## 行为说明

| 场景 | 预期行为 |
|------|----------|
| 新增方案 | 用户选择的 icon 正确保存到配置 |
| 编辑方案 | 显示当前 icon，允许修改 |
| 加载方案 | 根据 icon 字段显示对应图标 |
| 旧配置兼容 | icon 为 null 时使用默认图标 `"tune"` |
| 列表性能 | 一次 `list_configs` 获取全部，不再逐个 `load_config` |

---

**创建日期**: 2026-06-11
**更新日期**: 2026-06-11 — `list_configs` 返回 `ColorConfig[]`
