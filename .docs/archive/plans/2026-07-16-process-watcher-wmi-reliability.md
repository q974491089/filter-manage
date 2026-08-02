> 📦 已归档 2026-08-02 · 仅供历史追溯，非当前开发依据

# Process Watcher WMI 可靠性加固 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在不换 ETW、不做周期轮询的前提下，把现有 WMI 进程监听做成「断线自愈 + 启动一次对账 + 状态机不脏 + 可观测」，消除「重启后才正常」的玄学行为。

**Architecture:** 继续以 WMI `__InstanceOperationEvent` 为唯一持续事件源；主 watcher 线程在订阅失败 / channel 断开时按指数退避自动重订；仅在（重新）订阅成功后做一次性 `reconcile`；`Resubscribe` / 关闭监听时校正 `active_rule`；用 stderr 日志与扩展后的 `WatcherStatus` 暴露健康状态。不引入应用层周期扫进程表，不引入 ETW。

**Tech Stack:** Rust / Tauri 2 / `windows` crate WMI (`IWbemObjectSink`) / 现有 `process_watcher.rs` + `tray::apply_color_config`

**Branch:** `fix/process-watcher-fullscreen-cs2`（已存在则沿用）

**Out of scope（明确不做）:**
- ETW / Kernel-Process
- 应用层定时轮询进程列表作主路径
- 独占全屏下 LUT 必生效（系统限制，可后续另开任务）
- 前端大改 UI（本计划最多扩展 status 字段；UI 展示可选）

---

## 背景与根因（实施前必读）

用户复现与代码结论：

1. WMI 在多数时候工作正常；失败一次后**重启应用**又好 → 典型「订阅静默死亡，无自愈」。
2. 0.3.5 的 `reconcile` 只覆盖「订阅瞬间游戏已在跑」；**不**覆盖中途 WMI 断开。
3. `RecvTimeoutError::Disconnected` 后主循环把 `event_rx` 丢掉并空转，**永不重订**。
4. `active_rule` 在规则变更 / 关监听时可能残留，挡住后续对账。
5. 错误几乎全部 `let _ =` / `eprintln` 吞掉，正式版难排障。

目标体验：

| 场景 | 期望 |
|------|------|
| 软件先开 → 再开 CS2 | WMI Creation → 切方案（订阅保持存活） |
| CS2 先开 → 再开软件 | 启动 reconcile → 切方案 |
| WMI 线程/订阅意外断开 | 自动退避重订，无需用户重启 |
| 关总开关 / 删规则 | 清 `active_rule`，按需 restore |
| 开发者看日志 | 能看到 subscribe / disconnect / reconcile / started / stopped |

---

## 文件与职责

| 文件 | 职责 |
|------|------|
| `src-tauri/src/process_watcher.rs` | **主改动**：订阅生命周期、重订、reconcile 校正、日志、status |
| `.docs/api/process_watcher.md` | API / 行为说明同步 |
| `.docs/README.md` | 迭代记录一行 |
| `SYNC_STATUS.md` | 同步信号（按 `.rules/docs.md`） |
| 前端（可选） | 仅当要展示 `wmi_connected` 时改 `SettingsModal`；**默认本计划不做 UI** |

不改：`tray.rs` 配色逻辑、规则 CRUD 语义（仅保证 resubscribe 路径更稳）、WQL 结构（可顺手修大小写，见 Task 4）。

---

## 设计要点（实现约束）

### 1. 订阅状态机（主线程内）

```
Idle ──subscribe_ok──► Live
  ▲                      │
  │                      ├── channel Disconnected / monitor 退出
  │                      ▼
  └──── backoff ──── Reconnecting
```

- **禁止** Live 断开后进入永久 `event_rx = None` 空转。
- 无启用规则或总开关关闭 → 保持 Idle，**不**重试。
- 退避：`1s, 2s, 4s, 8s, 16s, 30s` 封顶；订阅成功清零。

### 2. reconcile（仅订阅成功后，一次）

保持「非周期」：

- 订阅成功后调用 `reconcile_running_processes`。
- 若 `active_rule` 指向的进程**已不在跑** → 先走 Stopped 语义（restore + 清空），再扫当前进程补 Started。
- 若 `active_rule` 对应规则已被禁用/删除 → 清空（关监听时 restore 见 Task 3）。

### 3. 日志前缀

统一：`[process-watcher] ...`，便于用户从 `tauri dev` 终端过滤。

### 4. WatcherStatus 扩展（向后兼容）

```rust
pub struct WatcherStatus {
    pub enabled: bool,
    pub active_rule: Option<ProcessRule>,
    pub active_config_name: Option<String>,
    pub subscribed_processes: Vec<String>,
    // 新增
    pub wmi_connected: bool,           // 当前是否持有有效 event_rx / 订阅存活
    pub last_error: Option<String>,    // 最近一次订阅/断开错误，成功可保留或清空（实现选：成功时 clear）
    pub reconnect_attempt: u32,        // 当前退避次数，Live 时为 0
}
```

前端旧代码忽略新字段即可（serde 多字段无妨；TS 侧可选更新类型）。

---

### Task 1: 诊断日志骨架 + WatcherStatus 扩展

**Files:**
- Modify: `src-tauri/src/process_watcher.rs`（`WatcherStatus`、`WatcherState`、`get_watcher_status`、`handle_event` 关键日志）
- Modify: `.docs/api/process_watcher.md`（status 字段说明）

- [ ] **Step 1: 扩展状态结构**

在 `WatcherStatus` 与内部 `WatcherState`（或主线程局部变量 + 同步到 state）增加：

```rust
// WatcherStatus 新增字段
pub wmi_connected: bool,
pub last_error: Option<String>,
pub reconnect_attempt: u32,
```

内部建议在 `WatcherState` 同步镜像，便于 `get_watcher_status` 读取；主线程在 subscribe / disconnect 时更新。

- [ ] **Step 2: 统一日志宏或函数**

```rust
fn pw_log(msg: impl AsRef<str>) {
    eprintln!("[process-watcher] {}", msg.as_ref());
}
```

在以下位置打日志（最小集）：

| 点 | 消息示例 |
|----|----------|
| 订阅成功 | `WMI subscribed: names=[cs2.exe]` |
| 订阅失败 | `WMI subscribe failed: ...` |
| channel 断开 | `WMI event channel disconnected, will reconnect` |
| Started | `event Started name=cs2.exe` |
| Stopped | `event Stopped name=cs2.exe` |
| reconcile 命中 | `reconcile Started name=cs2.exe` |
| reconcile 跳过 | `reconcile skip: no matching running process` |
| apply 失败 | `apply config failed: ...`（若后续打开 Result） |

- [ ] **Step 3: `handle_event` 打开关键错误日志**

```rust
// Started 分支：load_config / apply 不要静默
match config::load_config(config_name.clone()) {
    Ok(cfg) => {
        if let Err(e) = tray::apply_color_config(&cfg) {
            pw_log(format!("apply_color_config failed: {e}"));
        } else {
            let _ = app.emit("config-applied", &config_name);
            // toast 保持原逻辑
        }
        st.active_rule = Some(rule); // 见 Task 3：仅成功时写入更佳
    }
    Err(e) => {
        pw_log(format!("load_config failed for '{config_name}': {e}"));
        // 不写 active_rule（Task 3 一并改）
    }
}
```

本 Task 可先只加日志，active_rule 写入策略在 Task 3 收紧。

- [ ] **Step 4: 更新 `get_watcher_status` 返回新字段**

- [ ] **Step 5: 文档** — `.docs/api/process_watcher.md` 的 `WatcherStatus` 增加三字段说明

- [ ] **Step 6: 本地验证**

```powershell
# Windows PowerShell，项目根
npm run tauri dev
# 终端应出现 [process-watcher] 启动/订阅日志
```

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/process_watcher.rs .docs/api/process_watcher.md
git commit -m "$(cat <<'EOF'
fix(process-watcher): 增加诊断日志与 WatcherStatus 健康字段

为 WMI 可靠性加固提供可观测性：wmi_connected / last_error / reconnect_attempt。
EOF
)"
```

---

### Task 2: WMI 断线 / 订阅失败自动重订（核心）

**Files:**
- Modify: `src-tauri/src/process_watcher.rs`（`init_watcher` 主循环、`spawn_wmi_monitor`）

- [ ] **Step 1: 抽出 `try_subscribe` 辅助函数**

避免初始订阅与 Resubscribe / 重连三处复制粘贴：

```rust
/// 尝试建立 WMI 订阅。成功返回 (monitor_handle, event_rx, stop_tx)。
/// 失败时已写日志；调用方负责退避。
fn try_subscribe(
    wql: String,
    names: Vec<String>,
) -> Result<
    (
        thread::JoinHandle<()>,
        mpsc::Receiver<ProcessEvent>,
        mpsc::Sender<()>,
    ),
    String,
> {
    state().lock().unwrap().subscribed_processes = names;
    let (etx, erx) = mpsc::channel();
    let (stx, srx) = mpsc::channel();
    // spawn_wmi_monitor 需改为返回 Result，或内部失败时立即 drop sender 并返回 Err
    let handle = spawn_wmi_monitor(etx, wql, srx)
        .ok_or_else(|| "failed to spawn wmi-monitor thread".to_string())?;
    // 注意：当前 WmiSubscription::new 失败只在线程内 eprintln 后 return，
    // 会导致 erx 很快 Disconnected。重连循环依赖这一点即可，
    // 可选改进：用 oneshot 把 new() 结果回传主线程再标记 last_error。
    Ok((handle, erx, stx))
}
```

**推荐小改进（本 Task 内完成）：** `spawn_wmi_monitor` 在 `WmiSubscription::new` 失败时通过 `event_tx` 无法表达错误——可增加 `std::sync::mpsc::SyncSender<Result<(), String>>` 握手，或让 `new` 在**调用线程**执行（当前已在 wmi 线程）。最小可行：主线程发现 Disconnected 即视为失败并重订，并用 `last_error = Some("event channel disconnected")`。

- [ ] **Step 2: 主循环加入重连逻辑**

伪代码（替换当前 `Disconnected => drop and forget`）：

```rust
let mut backoff_secs: u64 = 0;
let mut next_retry_at: Option<Instant> = None;
// event_rx: Option<Receiver<...>> 同现有

// 在 loop 中：
// 1) 处理 cmd_receiver（Resubscribe / Stop）—— Resubscribe 时 backoff 清零
// 2) 若 event_rx.is_some()：recv_timeout(100ms) 处理事件
// 3) 若 event_rx.is_none() 且 应保持订阅（enabled + 有规则）：
//      if next_retry_at.map(|t| Instant::now() >= t).unwrap_or(true) {
//          match try_subscribe(...) {
//            Ok(...) => { backoff=0; next_retry=None; reconcile(...); wmi_connected=true }
//            Err(e) | 随即 Disconnected => {
//              last_error=...; wmi_connected=false;
//              backoff = (backoff*2).clamp(1, 30) 或从 1 起跳;
//              next_retry_at = now + backoff;
//              pw_log(format!("reconnect in {backoff}s"));
//            }
//          }
//      } else { sleep 短间隔 }
```

退避序列建议：

```rust
fn next_backoff(prev: u64) -> u64 {
    if prev == 0 { 1 } else { (prev.saturating_mul(2)).min(30) }
}
```

- [ ] **Step 3: 初始订阅失败也走同一套**

当前 `spawn_wmi_monitor` 失败仅 `eprintln`，主线程仍设 `event_rx = Some(erx)`，随后立刻 Disconnected。改为：失败则 `event_rx = None` + 进入重连调度（若 enabled）。

- [ ] **Step 4: Resubscribe 时**

- 停旧订阅（stop_tx + drop handle；**尽量 join 带超时**，避免双订阅竞态；若 join 困难至少 cancel + 短 sleep 50ms）
- `backoff = 0`
- 立即 `try_subscribe` + `reconcile`

- [ ] **Step 5: 验证（手动）**

1. 正常启动 → 日志 `WMI subscribed`
2. 无规则时不订阅、不重试
3. 有规则时：可临时在 `WmiSubscription::new` 首次强制失败（调试用）验证退避日志，再恢复
4. 改规则触发 Resubscribe → 新 names + reconcile

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/process_watcher.rs
git commit -m "$(cat <<'EOF'
fix(process-watcher): WMI 订阅断开后自动退避重订

避免 event channel 断开后主循环永久空转，消除必须重启应用才能恢复监听的问题。
EOF
)"
```

---

### Task 3: active_rule 校正 + reconcile 可纠正脏状态

**Files:**
- Modify: `src-tauri/src/process_watcher.rs`（`reconcile_running_processes`、`handle_event`、Resubscribe / disable 路径）

- [ ] **Step 1: 收紧 Started 写 active_rule**

仅当 `load_config` + `apply_color_config` **都成功**时设置 `active_rule`。  
（若产品希望「登记但未 apply」另议；默认：失败不登记，避免退出误 restore。）

- [ ] **Step 2: 改写 reconcile**

```text
reconcile(app):
  settings = load
  if !enabled: return

  running = running_process_names()  // 一次性快照，非周期

  // A. 若 active_rule 存在：
  //    - 规则已不在 enabled 列表 → clear_active(restore_if_needed)
  //    - 进程名已不在 running → 合成 Stopped（走 handle_event）
  // B. 若 active_rule 为空：
  //    - 找第一个 running 命中的 enabled 规则 → Started
```

`clear_active`：若原 `restore_on_exit` 为 true 则 `apply_default_config` + emit；然后 `active_rule = None`。

- [ ] **Step 3: 关闭监听 / 无规则时清理**

在 `Resubscribe` 分支：

```rust
if !settings.process_watcher_enabled || build_wql(...).is_none() {
    // 停止 WMI
    // 若有 active_rule && restore_on_exit → restore
    state.active_rule = None;
    subscribed_processes.clear();
    wmi_connected = false;
    return;
}
```

- [ ] **Step 4: 多实例 Stopped 防护（轻量、事件触发、非轮询）**

在 `handle_event(Stopped(name))` 中，清空 active 前：

```rust
let still = running_process_names()
    .iter()
    .any(|p| p.eq_ignore_ascii_case(&name));
if still {
    pw_log(format!("Stopped ignored, instance still running: {name}"));
    return;
}
```

仅在收到 Stopped 时扫一次，**不是**周期轮询。

- [ ] **Step 5: 验证矩阵**

| # | 步骤 | 期望 |
|---|------|------|
| 1 | 先开软件再开 cs2.exe（规则正确） | Started 日志 + 配色 |
| 2 | 先开 CS2 再开软件 | reconcile Started |
| 3 | 关 CS2 | Stopped + 默认方案（restore_on_exit） |
| 4 | 监听中禁用该规则 | active 清除；可选 restore |
| 5 | 关总开关再开 | 不残留 active；重新订阅 |
| 6 | 同名进程双开，关一个 | 不 restore（若能造双实例） |

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/process_watcher.rs
git commit -m "$(cat <<'EOF'
fix(process-watcher): 校正 active_rule 与可纠正 reconcile

规则变更/关监听时清理脏状态；Stopped 时确认无残留同名实例；apply 失败不登记 active。
EOF
)"
```

---

### Task 4: WQL / 进程名规范化（防「只有 reconcile 好、事件永远不来」）

**Files:**
- Modify: `src-tauri/src/process_watcher.rs`（`build_wql`、`add_process_rule` / `update_process_rule` 可选规范化）

- [ ] **Step 1: 规范化函数**

```rust
fn normalize_process_name(name: &str) -> String {
    let n = name.trim().to_string();
    // 存库与 WQL 使用同一形式；匹配仍 eq_ignore_ascii_case
    n
}
```

WQL 中：

```rust
// Win32_Process.Name 在 Windows 上通常大小写不敏感，但为减少差异：
// 保持用户输入；在回调与规则匹配已 ignore_ascii_case。
// 额外：转义单引号，防止 WQL 注入/语法错误
fn wql_escape(name: &str) -> String {
    name.replace('\'', "\\'")
}
```

`build_wql`：

```rust
format!("TargetInstance.Name = '{}'", wql_escape(n))
```

- [ ] **Step 2: add/update 时 trim 空名（已有）+ 拒绝仅空白**

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/process_watcher.rs
git commit -m "$(cat <<'EOF'
fix(process-watcher): WQL 进程名转义，降低订阅查询失败风险
EOF
)"
```

---

### Task 5: 前端双路径补强（可选但建议）

**问题：** `config::save_app_settings` 不触发 `send_resubscribe`；`ConfigManager` 通过整包 settings 写 `process_rules` 时，WMI 仍用旧订阅直到重启。

**Files:**
- Modify: `src-tauri/src/config.rs` 或 `process_watcher.rs` + `lib.rs`
- 或 Modify: `src/components/ConfigManager.tsx` 改为调用 `add/update/delete_process_rule`

**推荐后端最小改动：**

- [ ] **Step 1:** 在 `save_app_settings` 中比较旧/新 `process_rules` 与 `process_watcher_enabled`，有变化则 `process_watcher::notify_settings_changed()`（内部 `send_resubscribe`）。

注意：避免循环依赖——`config` 调 `process_watcher` 若模块结构别扭，可在 `lib.rs` 包一层 command，或 `process_watcher` 提供 `pub fn on_settings_saved(old, new)`。

更干净做法：

```rust
// process_watcher.rs
pub fn resubscribe_if_process_settings_changed(old: &AppSettings, new: &AppSettings) {
    if old.process_watcher_enabled != new.process_watcher_enabled
        || old.process_rules != new.process_rules
    {
        send_resubscribe();
    }
}
```

`ProcessRule` 需 `PartialEq`（已有 `Clone`，可 derive `PartialEq`）。

- [ ] **Step 2: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/process_watcher.rs
git commit -m "$(cat <<'EOF'
fix(process-watcher): save_app_settings 变更规则时触发 WMI 重订

修复 ConfigManager 等经 save_app_settings 写规则后订阅不更新的问题。
EOF
)"
```

---

### Task 6: 文档与收尾验证

**Files:**
- Modify: `.docs/api/process_watcher.md`
- Modify: `.docs/README.md`（迭代条目）
- Modify: `SYNC_STATUS.md`

- [ ] **Step 1: API 文档补充「可靠性行为」**

写明：

- 订阅失败 / 断开后自动退避重订（1s…30s）
- reconcile 仅在订阅成功后执行一次（非周期轮询）
- Stopped 多实例防护
- WatcherStatus 新字段

- [ ] **Step 2: README 迭代记录**

示例：

```markdown
| 修复 | 后端 | WMI 监听可靠性：断线自愈、脏 active_rule 校正、可观测 status | process_watcher.rs | process_watcher.md |
```

- [ ] **Step 3: 完整手动回归（CS2 或任意规则进程）**

```text
[ ] 冷启动有规则 → subscribed + wmi_connected
[ ] 先软件后游戏 → 切换
[ ] 先游戏后软件 → 切换
[ ] 退出游戏 → 恢复默认
[ ] 运行中拨关再开监听 → 恢复订阅
[ ] 故意杀不了 WMI 时：看退避日志；恢复后自动 Live
[ ] get_watcher_status 字段合理
```

- [ ] **Step 4: Commit docs**

```bash
git add .docs/api/process_watcher.md .docs/README.md SYNC_STATUS.md
git commit -m "$(cat <<'EOF'
docs(process-watcher): 同步 WMI 可靠性行为与 WatcherStatus 字段
EOF
)"
```

---

## 实施顺序与依赖

```
Task 1 日志 + status
   ↓
Task 2 自动重订     ← 解决「重启才好」
   ↓
Task 3 active_rule / reconcile 校正
   ↓
Task 4 WQL 转义
   ↓
Task 5 save_app_settings 重订（建议做）
   ↓
Task 6 文档 + 回归
```

预计工作量：**1～2 个专注开发日**（含 CS2 实机验证）。

---

## 风险与缓解

| 风险 | 缓解 |
|------|------|
| 重连过快打爆 WMI | 指数退避 + 30s 封顶 |
| Resubscribe 与自动重连竞态 | 同一主线程串行处理 cmd 与 retry；Resubscribe 清 backoff |
| join WMI 线程卡死 | cancel 后 timeout join 或 detach + 代际 token 忽略旧事件 |
| apply 失败不写 active 导致反复 Started | Started 成功才写；同 rule id 已 active 则 skip（现有逻辑） |
| 日志过多 | 仅状态变化与事件，不每 tick 打 |

---

## 验收标准（Definition of Done）

1. WMI 订阅断开后，**无需重启应用**，在退避窗口内自动恢复订阅（日志可证）。
2. 先开游戏 / 先开软件 两条路径均能切方案（规则与进程名正确的前提下）。
3. 关监听 / 删规则不会留下永久脏 `active_rule`。
4. 无应用层周期进程轮询；无 ETW。
5. `.docs/api/process_watcher.md` 与行为一致。
6. `cargo check` / `tauri dev` 可编译运行。

---

## 明确不做（防 scope creep）

- ❌ 每 N 秒扫进程表
- ❌ ETW 迁移
- ❌ 全屏 LUT 强制生效
- ❌ 启动器→子进程自动解析（可后续 PRD）

---

## Self-Review

| 需求 | 对应 Task |
|------|-----------|
| 断线自愈（重启才好） | Task 2 |
| 启动已运行进程 | Task 3（保留并增强 reconcile） |
| 脏 active_rule | Task 3 |
| 可观测 | Task 1 |
| WQL 脆弱 | Task 4 |
| 前端 save 不重订 | Task 5 |
| 文档 | Task 6 |
| 无轮询 / 无 ETW | 全文约束 |

无 TBD 占位；路径与提交信息已给出。

---

## 执行交接

计划已保存到：

`.docs/plans/2026-07-16-process-watcher-wmi-reliability.md`

**执行方式可选：**

1. **Subagent-Driven（推荐）** — 每 Task 独立执行并在 Task 间 review  
2. **Inline Execution** — 本会话按 Task 顺序直接改代码，检查点停顿  

你确认 **1 或 2** 后开始实现；默认从 Task 1 起在分支 `fix/process-watcher-fullscreen-cs2` 上推进。
