---
name: stitch-d2c
description: Stitch Design-to-Code 开发规范，避免常见踩坑，确保设计稿精确还原
---

# Stitch D2C 开发规范

## 核心原则

**永远从设计稿 HTML 中提取实际颜色值，不要只看 design tokens。**

---

## 踩坑记录 & 规避清单

### 1. CSS @import 顺序错误

**问题**: `@import url(...)` 放在 `@tailwind` 指令之后，导致 PostCSS 报错，整个 CSS 失效。

**正确做法**:
```css
/* @import 必须在最前面 */
@import url('https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap');

@tailwind base;
@tailwind components;
@tailwind utilities;
```

**验证**: 如果页面样式全部丢失（白色边框、黑色文字、无背景色），首先检查 CSS @import 顺序。

---

### 2. 颜色命名必须匹配设计系统

**问题**: 用嵌套命名 `lumina.primary`，但 Stitch 设计稿用扁平命名 `primary`。

**错误示例**:
```js
// 嵌套命名 - 错误
colors: {
  lumina: {
    primary: "#adc6ff",
    "on-surface": "#e2e2e8",
  }
}
// 使用: text-lumina-primary, bg-lumina-on-surface
```

**正确做法**:
```js
// 扁平命名 - 完全匹配 Stitch
colors: {
  "primary": "#adc6ff",
  "on-primary": "#002e6a",
  "surface-container": "#1e2024",
  "on-surface": "#e2e2e8",
}
// 使用: text-primary, bg-surface-container, text-on-surface
```

---

### 3. 颜色值必须从 HTML 源码验证

**问题**: 设计系统的 `overridePrimaryColor` 是 `#3b82f6`，但实际生成的 `primary` 是 `#adc6ff`。

**正确流程**:
1. 用 Stitch MCP 的 `get_screen` 获取 screen ID
2. 从 `htmlCode.downloadUrl` 下载 HTML 文件
3. 在 HTML 中搜索 `tailwind.config` 提取实际颜色值
4. 不要只看 `list_design_systems` 返回的 `overridePrimaryColor`

**关键验证点**:
```bash
# 下载设计稿 HTML
curl -sL "<htmlCode.downloadUrl>" -o /tmp/stitch-screen.html

# 提取 tailwind config 中的颜色
grep -A 50 '"colors"' /tmp/stitch-screen.html
```

---

### 4. 图标必须使用设计系统的图标库

**问题**: 用了 emoji（💾、☀️、🌙），但设计稿用 Material Symbols Outlined。

**正确做法**:
1. 在 `index.html` 添加字体:
```html
<link href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:wght,FILL@100..700,0..1&display=swap" rel="stylesheet"/>
<style>
  .material-symbols-outlined {
    font-variation-settings: 'FILL' 0, 'wght' 400, 'GRAD' 0, 'opsz' 24;
  }
</style>
```

2. 在组件中使用:
```jsx
// 正确
<span className="material-symbols-outlined text-[18px]">settings_backup_restore</span>

// 错误
<span>↻</span>
```

3. 填充图标使用 `FILL` 变量:
```jsx
<span 
  className="material-symbols-outlined text-primary" 
  style={{ fontVariationSettings: "'FILL' 1" }}
>check_circle</span>
```

---

### 5. 字体必须匹配设计系统

**问题**: 设计规范指定了 Inter + Geist，但没有导入 Geist。

**正确做法**:
```html
<link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&family=Geist:wght@400;500;600;700&display=swap" rel="stylesheet"/>
```

```js
// tailwind.config.js
fontFamily: {
  "headline-lg": ["Inter"],
  "headline-md": ["Inter"],
  "headline-sm": ["Inter"],
  "body-lg": ["Inter"],
  "body-md": ["Inter"],
  "label-md": ["Geist"],
  "label-sm": ["Geist"],
}
```

---

## D2C 开发流程 (Checklist)

### Phase 1: 设计分析
- [ ] 获取 Stitch project 信息 (`get_project`)
- [ ] 列出所有 screens (`list_screens`)
- [ ] 列出所有设计系统 (`list_design_systems`)
- [ ] 下载主设计稿 HTML，提取实际颜色值
- [ ] 记录所有使用的图标名称

### Phase 2: 基础配置
- [ ] `index.html`: 添加字体 (Inter, Geist, Material Symbols)
- [ ] `index.css`: @import 放在最前面，然后 @tailwind
- [ ] `tailwind.config.js`: 使用扁平颜色命名，匹配设计稿

### Phase 3: 组件开发
- [ ] 按照设计稿 HTML 的结构编写组件
- [ ] 使用 Material Symbols 图标，不要用 emoji
- [ ] 验证每个组件的颜色是否匹配设计稿

### Phase 4: 验证
- [ ] TypeScript 编译通过 (`npx tsc --noEmit`)
- [ ] 页面样式正确显示（无白色边框、无黑色文字）
- [ ] 所有图标正确显示
- [ ] 颜色与设计稿一致

---

## 常用 Material Symbols 图标对照表

| 功能 | 图标名 | 备注 |
|------|--------|------|
| 重置 | settings_backup_restore | |
| 保存 | save | |
| 确认/应用 | check_circle | FILL=1 用于选中态 |
| 取消/未选中 | radio_button_unchecked | |
| 暗色模式 | dark_mode | |
| 亮度 | light_mode | |
| 对比度 | contrast | |
| 伽马 | Camera | |
| 调色板 | palette | |
| 信息 | info | |
| 刷新 | refresh | |
| 下拉 | expand_more | |
| 前进 | arrow_forward_ios | |
| 图片 | add_photo_alternate | |
| 电影 | movie | |
| 游戏 | sports_esports | |
| 阅读 | edit_note | |
| 设置 | tune | |

---

## 设计系统颜色快速参考 (Lumina Pro)

```
Background:     #111317
Surface:        #111317
Surface Container: #1e2024
Surface Container High: #282a2e
Surface Variant: #333539

Primary:        #adc6ff (淡蓝)
On Primary:     #002e6a
Primary Container: #4d8eff

Secondary:      #c0c1ff (淡紫)
Secondary Container: #3131c0

Tertiary:       #4edea3 (翠绿)
Tertiary Container: #00a572

On Surface:     #e2e2e8
On Surface Variant: #c2c6d6
Outline:        #8c909f
Outline Variant: #424754
```
