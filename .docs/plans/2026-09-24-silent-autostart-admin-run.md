# 静默自启 + 管理员运行 实现方案与手动测试计划

> 对应 PRD：`.docs/prd/2026-09-24-silent-autostart-admin-run.md`
> 状态：**代码已实现，待 Windows 侧构建 + 手动验证**

**Goal:** 开机自启时静默进托盘（不弹窗）；设置里新增「以管理员身份运行」勾选，持久提权；两者叠加时用计划任务实现管理员级开机静默自启且无 UAC。

**Architecture:** 两种开机自启机制互斥——非管理员用 `tauri-plugin-autostart` 的注册表 Run 项（带 `--silent` 参数），管理员用 Windows 计划任务（登录触发 + 最高权限 + 交互令牌）。`admin::reconcile_autostart()` 依据 `(autostart, run_as_admin)` 与当前提权状态切换机制并清理另一种。启动时按 `--silent` 参数决定是否显示主窗口；`run_as_admin` 为真且未提权时自提权重启。

**Tech Stack:** Tauri 2 / Rust（`windows` 0.58：Token 提权检测、`ShellExecuteW runas`；`schtasks.exe` + XML 管理计划任务）+ React/TS 设置界面。

> ⚠ 自动化测试不适用：提权、UAC、计划任务、开机启动都是 Windows 系统级行为，且本仓库 WSL 编辑 + Windows 构建，无法在 CI/WSL 跑单测。验证以下方**手动测试计划**为准。

---

## 已改动文件

| 文件 | 改动 |
|------|------|
| `src-tauri/src/admin.rs` | **新增**。提权检测 `is_elevated`、自提权重启 `relaunch_elevated`、计划任务增删查、`reconcile_autostart`，命令 `is_running_as_admin` / `set_run_as_admin`。 |
| `src-tauri/src/lib.rs` | 注册 `mod admin`；autostart 插件注册带 `--silent`；`setup` 增加自提权 + 计划任务补建 + 按 `--silent` 决定显示窗口；`enable/disable/is_autostart_enabled` 改走 `reconcile_autostart`；注册两个新命令。 |
| `src-tauri/src/config.rs` | `AppSettings` 新增 `run_as_admin: bool`（`#[serde(default)]`）+ `Default`。 |
| `src-tauri/Cargo.toml` | `windows` features 增加 `Win32_Security`。 |
| `src-tauri/tauri.conf.json` | 主窗口 `visible: false`（由 setup 按启动来源显示，避免闪现）。 |
| `src/components/SettingsModal.tsx` | `AppSettings` 加字段 + 初始值；新增 `handleToggleRunAsAdmin`（含确认弹窗）+ 常规设置里「以管理员身份运行」开关。 |
| `src/App.tsx` / `src/components/ConfigManager.tsx` | `AppSettings` interface 补 `run_as_admin` 字段（类型一致）。 |
| `.docs/api/config.md` | 同步 `AppSettings` 字段、自启机制说明、新增命令文档。 |

<!-- APPEND-MARKER -->

## 行为矩阵（测试对照表）

| 自启 | 管理员 | 开机启动机制 | 开机后表现 | 手动双击 |
|------|--------|--------------|------------|----------|
| 关 | 关 | 无 | — | 正常显示窗口，非提权 |
| 开 | 关 | 注册表 Run 项 + `--silent` | 静默进托盘，非提权 | 正常显示窗口，非提权 |
| 关 | 开 | 无 | — | 自提权（一次 UAC）后显示窗口 |
| 开 | 开 | 计划任务（登录触发 + 最高权限） | 静默进托盘 + 已提权 + **无 UAC** | 自提权（一次 UAC）后显示窗口 |

---

## 构建（必须在 Windows PowerShell 执行）

WSL 只用于编辑；依赖安装与构建都在 Windows 侧。

```powershell
Set-Location 'C:\Users\myuser\Projects\filter-manage\.claude\worktrees\feature+silent-autostart'
npm install
npm run tauri build
```

- 若本地无 `TAURI_SIGNING_PRIVATE_KEY`，先把 `tauri.conf.json` 的 `createUpdaterArtifacts` 临时设为 `false`，构建完再恢复。
- 计划任务需要「已安装的固定 exe 路径」才有意义——**建议用 `tauri build` 产出的安装包装好后测试**，而不是 `tauri dev`（dev 的 exe 路径是临时 target 目录，计划任务会指向它）。
- 编译若报 `windows` crate 某符号路径/feature 不对（如 `OpenProcessToken`/`ShellExecuteW` 的模块路径），按报错调整 `use` 路径或 `Cargo.toml` feature 即可，逻辑不受影响。

---

## 手动测试计划

> 每组测试前，建议先用任务管理器「详细信息」页确认进程的「已提升」列，并用 `Win+R` → `taskschd.msc` 查看计划任务 `FilterManageAutostart` 是否存在。注册表自启项在 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`（键名含 app identifier）。

### A. 静默自启（非管理员）
1. 确保「以管理员身份运行」**关**。打开设置 → 常规，打开「开机时自动启动」。
   - [ ] `taskschd.msc` 中**没有** `FilterManageAutostart`；注册表 `Run` 里**有** Filter-Manage 项，且命令行结尾带 `--silent`。
2. 重启电脑并登录。
   - [ ] 应用随登录启动，**不弹主窗口**，仅出现托盘图标。
   - [ ] 双击托盘图标（或右键「显示主窗口」）能正常打开窗口。
3. 关闭窗口后手动双击程序图标。
   - [ ] 正常弹出主窗口（非静默）。

### B. 手动启动不受影响
1. 「开机自启」关、「管理员」关。双击程序。
   - [ ] 正常显示窗口。
   - [ ] 任务管理器「已提升」= 否。

### C. 开启「以管理员身份运行」
1. 设置里勾选「以管理员身份运行」。
   - [ ] 弹出确认框，说明将重启为管理员。点「取消」→ 开关回弹、无变化。
2. 再次勾选并在确认框点「确定」。
   - [ ] 弹出一次 UAC。点「否」→ 应用不提权、开关回滚为关（`app.json` 里 `run_as_admin` 仍为 false）。
   - [ ] 再次勾选并在 UAC 点「是」→ 应用以管理员重启；任务管理器「已提升」= 是；`app.json` 里 `run_as_admin: true`。
3. 完全退出应用后再手动双击。
   - [ ] 出现一次 UAC；点「是」后以管理员运行（符合预期）。

### D. 管理员 + 自启（核心）
1. 在管理员运行状态下，打开「开机时自动启动」。
   - [ ] `taskschd.msc` 中**出现** `FilterManageAutostart`：常规页「使用最高权限运行」勾选、触发器为「登录时」、操作指向已安装 exe 且参数为 `--silent`。
   - [ ] 注册表 `Run` 里**没有** Filter-Manage 项（互斥，避免双启）。
2. 重启电脑并登录。
   - [ ] 应用静默进托盘（不弹窗）。
   - [ ] **全程没有 UAC 弹窗**。
   - [ ] 任务管理器「已提升」= 是。
   - [ ] 只有一个 Filter-Manage 进程（没有被启动两次）。

### E. 关闭「以管理员身份运行」回退
1. 在 D 的状态下，取消勾选「以管理员身份运行」。
   - [ ] `taskschd.msc` 中 `FilterManageAutostart` 被删除。
   - [ ] 因为自启仍开着 → 注册表 `Run` 项恢复出现（带 `--silent`）。
2. 重启电脑并登录。
   - [ ] 应用静默进托盘、**非管理员**运行（任务管理器「已提升」= 否）、无 UAC。

### F. 全部关闭无残留
1. 关闭「开机自启」与「以管理员身份运行」。
   - [ ] `taskschd.msc` 无 `FilterManageAutostart`；注册表 `Run` 无 Filter-Manage 项。
2. 重启电脑。
   - [ ] 应用不自启。

### G. 配置一致性
- [ ] 在提权与非提权两种运行下，改动的方案/设置都写进同一份 `%APPDATA%\filter-manage\app.json`，来回切换设置不丢失。

---

## 已知限制 / 测试时留意

- **计划任务用户上下文**：若 D 组仍弹 UAC，多半是任务的「用户账户」被设成了 `Administrators` 组而非当前登录用户。当前实现用 `InteractiveToken` + 当前用户 SID/名。若环境异常，检查 `taskschd.msc` 里该任务的「常规」用户是否为当前登录用户。
- **提权边界下的单实例**：应用以管理员运行时，普通权限的第二次启动可能无法通过 `tauri-plugin-single-instance` 唤醒已有窗口（提权边界会阻断 IPC）。本次不处理，测 D/E 时若双击不唤醒窗口属已知限制。
- **dev 构建不适合测计划任务**：计划任务记录的是构建时 exe 路径，`tauri dev` 的临时路径重建后失效。用安装包测。
- **云同步字段**：`run_as_admin` 与 `autostart` 一样是本机状态，不应跨设备同步。目前前端尚无主动排除逻辑（`autostart` 也未排除），属既有问题；接入云同步时需一并排除，否则非管理员设备可能因同步下来的 `run_as_admin=true` 反复请求 UAC。
- **真实行为只能 Windows 侧验证**：WSL 无法验证开机/UAC/计划任务。

