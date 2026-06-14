# Update Modal Design

## Overview

将 UpdateBanner（顶部横幅）改为居中模态弹窗，添加"30天不再提示"静默期功能。

## Problem

当前 UpdateBanner 使用 `z-50`，被同样 `z-50 fixed top-0` 的 header 遮挡，用户可能看不到更新通知。

## Solution

### 1. 组件重构

**UpdateBanner.tsx → UpdateModal.tsx**

- 从固定顶部横幅改为居中模态弹窗
- z-index 提升到 `z-[100]`
- 使用 Glassmorphism 设计风格
- 添加 "30天不再提示" 勾选框

**UI 结构：**
```
┌─────────────────────────────────────┐
│  半透明遮罩 (backdrop-blur-sm)      │
│  ┌─────────────────────────────┐    │
│  │  发现新版本 v1.0.0          │    │
│  │                             │    │
│  │  [立即更新]  [稍后]         │    │
│  │                             │    │
│  │  ☐ 30天不再提示             │    │
│  └─────────────────────────────┘    │
└─────────────────────────────────────┘
```

**样式规范（遵循 DESIGN.md）：**
- 背景: `bg-surface-container/80 backdrop-blur-md`
- 边框: `border border-outline-variant/20`
- 圆角: `rounded-xl`
- 阴影: `shadow-2xl shadow-black/40`
- 主按钮: `bg-primary text-on-primary`
- 次按钮: `text-on-surface-variant hover:bg-surface-variant/50`

### 2. 缓存逻辑（localStorage）

**存储策略：**
- Key: `update_snooze_until`
- Value: 过期时间戳 (毫秒)
- 过期时间: 当前时间 + 30天

**检查逻辑：**
```typescript
function checkSnooze(): boolean {
  const snoozeUntil = localStorage.getItem("update_snooze_until");
  if (!snoozeUntil) return false;
  return Date.now() < parseInt(snoozeUntil, 10);
}
```

**静默期设置：**
```typescript
function setSnooze(): void {
  const thirtyDays = 30 * 24 * 60 * 60 * 1000;
  localStorage.setItem("update_snooze_until", String(Date.now() + thirtyDays));
}
```

### 3. useUpdater.ts 增强

**新增状态：**
- `snoozed: boolean` — 是否在静默期

**新增函数：**
- `snooze()` — 设置30天静默期
- 启动时检查静默期，如果在静默期内则不检查更新

**流程：**
1. App 启动 → `useUpdater()` 初始化
2. 检查 `localStorage` 是否在静默期
3. 如果在静默期 → 跳过更新检查
4. 如果不在静默期 → 调用 `check()` 检查更新
5. 有更新 → 显示弹窗
6. 用户勾选"30天不再提示" + 点击"稍后" → 调用 `snooze()`，关闭弹窗

### 4. 文件改动清单

| 文件 | 改动 |
|------|------|
| `src/components/UpdateModal.tsx` | 新建，模态弹窗组件 |
| `src/components/UpdateBanner.tsx` | 删除（被 UpdateModal 替代） |
| `src/hooks/useUpdater.ts` | 添加 snooze 逻辑和静默期检查 |
| `src/App.tsx` | UpdateBanner → UpdateModal |

## Success Criteria

1. 更新弹窗在 header 之上显示（z-index > 50）
2. 弹窗样式符合设计规范
3. 勾选"30天不再提示"后，30天内不再弹出更新提醒
4. 静默期到期后，恢复更新检查
5. 下载进度条正常显示

## Out of Scope

- 后端存储（localStorage 足够）
- 更新通知的自定义时间选择（仅30天）
- 多语言支持（当前仅中文）
