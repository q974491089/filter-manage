# 前端开发指南

## 组件结构

```
App.tsx
├── ProfileList        — ICC 列表（搜索、导入、导出、切换）
├── ColorAdjuster      — NVIDIA 颜色滑块（亮度/对比度/伽马/数字振动）
├── PreviewImage       — 图片预览（支持上传本地图片）
├── ConfigManager      — 配置预设列表（加载/删除）
└── SaveModal          — 保存配置弹窗
```

## 新功能接入清单（2026-05-16）

### 1. ICC 搜索框（待实现）
- 在 `ProfileList` 顶部添加 `<input>` 搜索框
- 输入时调用 `search_icc_profiles({ query })` 替换列表数据
- 空字符串时调用 `get_icc_profiles()` 恢复全量列表

### 2. ICC 导入（已实现）
- **点击导入按钮**：调用 `@tauri-apps/plugin-dialog` 的 `open()` 选文件，再调用 `import_icc_profile`
- **拖拽到列表区域**：监听 `onDragOver` / `onDrop`，从 `e.dataTransfer.files` 取路径，调用 `import_icc_profile`
- 导入成功后刷新列表

### 3. ICC 导出（待实现）
- 在列表项添加导出按钮（或右键菜单）
- 调用 `open({ directory: true })` 选目录，再调用 `export_icc_profile`

### 4. 图片预览上传（已实现）
- 点击按钮调用 `open()` 选择本地图片
- 拖拽图片到预览区域自动加载
- 调用 `set_preview_image` 验证路径，`convertFileSrc()` 转换为 asset:// 协议显示
- 支持更换和清除图片
- 调用 `set_preview_image` 验证路径
- 用 `convertFileSrc(path)` 转换为 `asset://` 协议后赋给 `<img src>`

```ts
import { convertFileSrc } from '@tauri-apps/api/core'
const src = convertFileSrc(validPath)
```

## 常用 import

```ts
import { invoke } from '@tauri-apps/api/core'
import { convertFileSrc } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { listen } from '@tauri-apps/api/event'
```

## DevTools 标识属性

每个组件及其关键子元素必须添加 `data-components` 和 `data-name` 属性，方便在 DevTools 中定位。

```tsx
<div data-components="ProfileList">
  <h3 data-name="title">ICC 配置文件</h3>
  <div data-name="action-buttons" className="flex gap-sm">
    <button data-name="restore-button">恢复默认</button>
    <button data-name="import-button">导入 ICC</button>
  </div>
  <div data-name="profile-list">
    <div data-name="profile-item">...</div>
  </div>
</div>
```

- `data-components` — **仅放在组件根元素**，值为组件文件名（如 `ProfileList`）
- `data-name` — **所有关键元素都加**，包括原子级结构（按钮组、列表项、弹窗等），值为元素用途简述
- 原子结构只加 `data-name`，不加 `data-components`

## 错误处理约定

`invoke` 失败时抛出 `string` 类型错误，统一用 toast 提示：

```ts
try {
  await invoke('...')
} catch (err) {
  // 显示 toast，err 是 string
}
```
