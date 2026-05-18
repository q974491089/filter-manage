# 前端交接文档

> 本文档包含所有需要前端实现的功能，基于后端 API 文档整理。

---

## 后端 API 依赖

前端通过 `invoke` 调用后端命令，所有命令详见：
- `docs/api/config.md` — 配置管理
- `docs/api/icc.md` — ICC 配置文件管理
- `docs/api/nvidia.md` — NVIDIA 颜色设置

### 前端已安装的 npm 依赖

```json
"@tauri-apps/api": "^2",
"@tauri-apps/plugin-dialog": "^2",
"@tauri-apps/plugin-updater": "^2.10.1",
"@tauri-apps/plugin-process": "^2.3.1"
```

---

## 功能模块

### 1. 版本号显示

在 Header 标题右侧显示版本号，从 `package.json` 动态读取。

**步骤 1：修改 `vite.config.ts`**

```typescript
import { readFileSync } from "fs";

const pkg = JSON.parse(readFileSync("./package.json", "utf-8"));

// 在 defineConfig 中添加：
define: {
  __APP_VERSION__: JSON.stringify(pkg.version),
},
```

**步骤 2：修改 `src/vite-env.d.ts`**

```typescript
declare const __APP_VERSION__: string;
```

**步骤 3：修改 `src/App.tsx` Header 区域**

```tsx
<div className="flex items-center gap-md">
  <img src="/favicon.png" alt="icon" className="w-8 h-8 rounded-md" />
  <div className="flex items-baseline gap-sm">
    <h1 className="font-headline-md text-headline-md font-medium text-primary">Filter Manage</h1>
    <span className="font-label-sm text-label-sm text-on-surface-variant/60">v{__APP_VERSION__}</span>
  </div>
</div>
```

---

### 2. 在线自动更新

启动时检查 GitHub Releases 是否有新版本。

**后端已完成：**
| 改动 | 文件 | 说明 |
|------|------|------|
| Rust 依赖 | `src-tauri/Cargo.toml` | `tauri-plugin-updater` + `tauri-plugin-process` |
| 插件注册 | `src-tauri/src/lib.rs` | 注册 updater 和 process 插件 |
| Updater 配置 | `src-tauri/tauri.conf.json` | `plugins.updater` endpoint 指向 GitHub Releases |

**前端实现：**

创建 `src/hooks/useUpdater.ts`：

```typescript
import { useEffect, useState } from "react";
import { check, Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "done" | "error";

export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [version, setVersion] = useState("");
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState("");
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function checkForUpdate() {
      try {
        setStatus("checking");
        const update = await check();
        if (cancelled) return;

        if (update) {
          setVersion(update.version);
          setPendingUpdate(update);
          setStatus("available");
        } else {
          setStatus("idle");
        }
      } catch (e) {
        if (cancelled) return;
        setError(String(e));
        setStatus("error");
      }
    }

    checkForUpdate();
    return () => { cancelled = true; };
  }, []);

  async function installUpdate() {
    if (!pendingUpdate) return;
    try {
      setStatus("downloading");
      let totalLen = 0;
      let downloaded = 0;

      await pendingUpdate.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          totalLen = event.data.contentLength;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (totalLen > 0) setProgress(Math.round((downloaded / totalLen) * 100));
        } else if (event.event === "Finished") {
          setStatus("done");
        }
      });

      await relaunch();
    } catch (e) {
      setError(String(e));
      setStatus("error");
    }
  }

  function dismiss() {
    setStatus("idle");
    setPendingUpdate(null);
  }

  return { status, version, progress, error, installUpdate, dismiss };
}
```

创建 `src/components/UpdateBanner.tsx`：

```tsx
import { useUpdater } from "../hooks/useUpdater";

export default function UpdateBanner() {
  const { status, version, progress, installUpdate, dismiss } = useUpdater();

  if (status === "idle" || status === "checking" || status === "error") return null;

  if (status === "downloading" || status === "done") {
    return (
      <div className="fixed top-0 inset-x-0 z-50 bg-blue-600 text-white text-center py-2 text-sm">
        正在下载更新 v{version}... {progress}%
      </div>
    );
  }

  return (
    <div className="fixed top-0 inset-x-0 z-50 bg-green-600 text-white text-center py-2 text-sm flex items-center justify-center gap-4">
      <span>发现新版本 v{version}</span>
      <button
        onClick={installUpdate}
        className="px-3 py-0.5 bg-white text-green-700 rounded text-xs font-medium hover:bg-green-50"
      >
        立即更新
      </button>
      <button
        onClick={dismiss}
        className="px-2 py-0.5 text-green-100 hover:text-white text-xs"
      >
        稍后
      </button>
    </div>
  );
}
```

在 `src/App.tsx` 中引入：
```tsx
import UpdateBanner from "./components/UpdateBanner";

// 在 return 的最外层 div 内顶部添加：
<UpdateBanner />
```

**行为说明：**
- 启动时自动检查更新
- 有新版本 → 顶部绿色横幅 + "立即更新" / "稍后" 按钮
- 点击"立即更新" → 下载进度条 → 安装完成自动重启
- 无新版本 → 静默，不显示 UI

---

### 3. 配置管理（ConfigManager 组件）

对应后端 API：`docs/api/config.md`

**数据类型：**
```typescript
interface ColorConfig {
  name: string;
  brightness: number;
  contrast: number;
  gamma: number;
  digital_vibrance: number;
  icc_profile: string | null;
}
```

**需要实现的 API 调用：**

| 功能 | API 调用 | 说明 |
|------|----------|------|
| 保存配置 | `invoke('save_config', { config })` | 保存当前设置为预设 |
| 加载配置 | `invoke('load_config', { name })` | 加载指定预设 |
| 列出配置 | `invoke('list_configs')` | 返回所有预设名称 |
| 删除配置 | `invoke('delete_config', { name })` | 删除指定预设 |
| 加载默认配置 | `invoke('load_default_config')` | 返回默认配置或 null |
| 保存默认配置 | `invoke('save_default_config', { config })` | 首次调用生效 |
| 覆盖默认配置 | `invoke('overwrite_default_config', { config })` | 强制覆盖默认值 |

---

### 4. ICC 配置文件管理（ProfileList 组件）

对应后端 API：`docs/api/icc.md`

**数据类型：**
```typescript
interface IccProfile {
  name: string;
  path: string;
  is_active: boolean;
}

interface DisplayMonitor {
  name: string;
  device_id: string;
  pnp_id: string;
  is_primary: boolean;
}
```

**需要实现的 API 调用：**

| 功能 | API 调用 | 说明 |
|------|----------|------|
| 获取 ICC 列表 | `invoke('get_icc_profiles')` | 返回系统 ICC 文件列表 |
| 搜索 ICC | `invoke('search_icc_profiles', { query })` | 按名称过滤 |
| 导入 ICC | `invoke('import_icc_profile', { srcPath })` | 复制外部文件到系统目录 |
| 导出 ICC | `invoke('export_icc_profile', { profileName, destDir })` | 导出到指定目录 |
| 应用 ICC | `invoke('set_icc_profile', { profilePath, deviceId })` | 立即生效 |
| 获取当前 ICC | `invoke('get_current_icc_profile')` | 从注册表读取 |
| 恢复默认 ICC | `invoke('restore_default_icc_profile', { deviceId })` | 恢复 sRGB |
| 获取显示器列表 | `invoke('get_display_monitors')` | 枚举连接的显示器 |
| 打开 ICC 目录 | `invoke('open_icc_directory')` | 用资源管理器打开 |
| 设置预览图片 | `invoke('set_preview_image', { imagePath })` | 验证并返回路径 |

**ICC 导入/导出接入要点：**

```typescript
import { open } from '@tauri-apps/plugin-dialog';

// 点击导入
const selected = await open({ filters: [{ name: 'ICC', extensions: ['icc', 'icm'] }] });
if (selected) await invoke('import_icc_profile', { srcPath: selected as string });

// 导出
const dir = await open({ directory: true, title: '选择导出目录' });
if (dir) await invoke('export_icc_profile', { profileName: name, destDir: dir as string });
```

**预览图片显示：**
```typescript
import { convertFileSrc } from '@tauri-apps/api/core';
const src = convertFileSrc(validPath);  // asset:// 协议
// <img src={src} />
```

---

### 5. NVIDIA 颜色设置（ColorAdjuster 组件）

对应后端 API：`docs/api/nvidia.md`

**数据类型：**
```typescript
interface NvidiaSettings {
  brightness: number;        // -125 ~ 125，默认 0
  contrast: number;          // -82 ~ 82，默认 0
  gamma: number;             // 0.4 ~ 2.8，默认 1.0
  digital_vibrance: number;  // 0 ~ 100，默认 50
}
```

**需要实现的 API 调用：**

| 功能 | API 调用 | 范围 |
|------|----------|------|
| 设置亮度 | `invoke('set_nvidia_brightness', { display: 1, value })` | -125 ~ 125 |
| 设置对比度 | `invoke('set_nvidia_contrast', { display: 1, value })` | -82 ~ 82 |
| 设置伽马 | `invoke('set_nvidia_gamma', { display: 1, value })` | 0.4 ~ 2.8 |
| 设置数字振动 | `invoke('set_nvidia_digital_vibrance', { display: 1, value })` | 0 ~ 100 |

**注意：** 需要系统安装 NVIDIA 驱动（nvapi64.dll），非 NVIDIA 显卡调用会返回错误。

---

## 错误处理

所有 `invoke` 命令失败时抛出 `string` 类型错误：

```typescript
try {
  await invoke('some_command', { ... });
} catch (err) {
  // err 是 string 类型
  showToast('error', `操作失败: ${err}`);
}
```

---

## 完整行为说明

1. **Header**：显示版本号（从 package.json 动态读取）
2. **启动时**：自动检查更新 + 加载默认配置
3. **配置管理**：保存/加载/删除用户预设
4. **ICC 管理**：浏览/导入/导出/应用 ICC 配置文件
5. **NVIDIA 设置**：实时调节亮度/对比度/伽马/数字振动
6. **更新提示**：有新版本时顶部横幅，支持下载安装
