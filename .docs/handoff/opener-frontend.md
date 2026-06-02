# 打开外部浏览器 — 前端交接文档

## 后端已完成

1. `Cargo.toml` 添加 `tauri-plugin-opener = "2"`
2. `lib.rs` 注册 `tauri_plugin_opener::init()`
3. `capabilities/default.json` 添加 `"opener:default"` 权限

## 前端需要安装的依赖

```bash
pnpm add @tauri-apps/plugin-opener
```

## 前端调用方式

将 `window.open(url)` 替换为：

```ts
import { openUrl } from "@tauri-apps/plugin-opener";

await openUrl("https://github.com/xxx/releases");
```

## 行为说明

- `openUrl` 会调用系统默认浏览器打开指定 URL
- 无需自定义 Tauri 命令，插件直接提供前端 API
- 支持 `http://`、`https://` 等标准协议
