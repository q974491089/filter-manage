# Updater API（自定义更新流程）

> **状态：🚧 实现中** — 本文档是 [`.docs/plans/custom-update-flow.md`](../plans/custom-update-flow.md) 的 Rust 客户端技术规格。

模块：`src-tauri/src/updater.rs`
注册：`src-tauri/src/lib.rs`（`invoke_handler` + `manage(UpdaterState)`）

---

## 设计概览

绕过 `tauri-plugin-updater` 的 `check`/`download`，自建「检查 → 流式下载（可取消/测速）→ minisign 校验 → NSIS 安装」流程。**复用现有 minisign 签名与 pubkey**，下载源由服务端下发的镜像列表决定。

### 新增依赖（`Cargo.toml`）

```toml
reqwest = { version = "0.13", default-features = false, features = ["stream", "rustls"] }
minisign-verify = "0.2"
futures-util = "0.3"
```

> `rustls` 避免 Windows OpenSSL 依赖，保证 CI 可编译。
> `reqwest` 0.13 与 Cargo.lock 中 `tauri-plugin-updater` 传递依赖版本一致，不引入重复。
> tokio 由 tauri 提供，命令用 `async fn` 即可。

### 共享状态

```rust
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub struct UpdaterState {
    /// check_update 后缓存的更新信息（含 signature）
    pub pending: std::sync::Mutex<Option<UpdateInfo>>,
    /// 下载完成并校验通过的临时文件路径
    pub staged_path: std::sync::Mutex<Option<std::path::PathBuf>>,
    /// 取消标志，download_update 循环每个 chunk 检查
    pub cancel: Arc<AtomicBool>,
    /// 并发下载防护
    pub downloading: Arc<AtomicBool>,
}

impl Default for UpdaterState {
    fn default() -> Self {
        Self {
            pending: std::sync::Mutex::new(None),
            staged_path: std::sync::Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            downloading: Arc::new(AtomicBool::new(false)),
        }
    }
}
```

`lib.rs` 中 `.manage(UpdaterState::default())`。

---

## 常量

```rust
/// tauri.conf.json plugins.updater.pubkey — minisign 公钥（base64 包裹）
const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IENEMUQ0NDVBMTMwMDhFQ0EKUldUS2pnQVRXa1Fkeld0YVdwb1N0clJybkp0OFR2cDhUdUhUSXNvZUZjOEpTdzBrSDFYQXBLcjYK";

/// 服务端 API 地址（双域名竞速，规避单点 DNS 故障）
const UPDATE_API_HOSTS: &[&str] = &[
    "https://filter-manage-api.xyls.us.kg",
    "https://filter-manage-api.6ya.kdns.fr",
];
```

> `check_update` 用 `tokio::select!` 同时请求两个 host，先返回的成功结果胜出，另一个 `abort()`。两个域名指向同一服务端（Cloudflare Tunnel 主域名 + 备用 DNS），任一可达即可，规避单点 DNS 故障。

---

## 数据结构

```rust
#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Mirror {
    pub name: String,   // 如 "GitHub 镜像 1"
    pub url: String,    // 完整可下载 URL
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub has_update: bool,
    pub version: String,    // 服务端返回的最新版本，如 "0.3.4"
    pub notes: String,      // markdown 更新说明
    pub signature: String,  // base64 minisign 签名（与镜像无关，同版本唯一）
    pub mirrors: Vec<Mirror>,
}
```

> 前端 TS 类型与此一一对应（`hasUpdate` / `mirrors` 等 camelCase）。

### 事件 Payload

```rust
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload { downloaded: u64, total: u64 }

#[derive(serde::Serialize, Clone)]
struct ErrPayload { message: String }
```

---

## 命令

### `check_update`

向服务端查询更新；读取自身版本号作为 `current`。结果缓存进 `UpdaterState.pending`。

**参数**：无（Rust 内部用 `app.package_info().version` 作为 `current`）

**返回值**：
```rust
Result<UpdateInfo, String>
```

**实现**：
```rust
#[tauri::command]
pub async fn check_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, UpdaterState>,
) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let host_a = UPDATE_API_HOSTS[0];
    let host_b = UPDATE_API_HOSTS[1];

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let fetch = |host: &str| {
        let client = client.clone();
        let url = format!("{host}/api/check-update?current={current}");
        async move {
            let resp = client.get(&url).send().await?;
            if !resp.status().is_success() {
                return Ok(None);
            }
            resp.json::<UpdateInfo>().await.map(Some)
        }
    };

    let mut fut_a = tokio::spawn(fetch(host_a));
    let mut fut_b = tokio::spawn(fetch(host_b));

    let info: Option<UpdateInfo> = tokio::select! {
        a = &mut fut_a => {
            fut_b.abort();
            a.ok().and_then(|r| r.ok()).flatten()
        }
        b = &mut fut_b => {
            fut_a.abort();
            b.ok().and_then(|r| r.ok()).flatten()
        }
    };

    let info = match info {
        Some(i) => i,
        // 降级：双域名均不可达，回退 tauri-plugin-updater 直连 GitHub
        None => return fallback_check(&app, &state).await,
    };

    *state.pending.lock().unwrap() = Some(info.clone());
    Ok(info)
}

/// 降级到 tauri-plugin-updater 直连 GitHub latest.json
async fn fallback_check(
    app: &tauri::AppHandle,
    state: &tauri::State<'_, UpdaterState>,
) -> Result<UpdateInfo, String> {
    use tauri_plugin_updater::UpdaterExt;
    let updater = app.updater().map_err(|e| e.to_string())?;
    match updater.check().await {
        Ok(Some(update)) => {
            let info = UpdateInfo {
                has_update: true,
                version: update.version.clone(),
                notes: update.body.clone().unwrap_or_default(),
                signature: update.signature.clone(),
                mirrors: vec![Mirror {
                    name: "GitHub 直连".into(),
                    url: update.download_url.to_string(),
                }],
            };
            *state.pending.lock().unwrap() = Some(info.clone());
            Ok(info)
        }
        Ok(None) => Ok(UpdateInfo {
            has_update: false,
            version: String::new(),
            notes: String::new(),
            signature: String::new(),
            mirrors: vec![],
        }),
        Err(e) => Err(format!("check_update failed: {e}")),
    }
}
```

**示例**：
```typescript
const info = await invoke<UpdateInfo>("check_update");
if (info.hasUpdate) { /* 弹窗展示 info.version / info.notes / info.mirrors */ }
```

---

### `download_update`

流式下载指定镜像 URL 到临时文件，过程 emit 进度事件，完成后做 minisign 校验。

**参数**：
- `url` (`String`) — 选定镜像的下载 URL（来自 `UpdateInfo.mirrors[].url`）

**返回值**：
```rust
Result<(), String>   // Ok 表示下载+校验流程已完成；细节走事件
```

**实现**：
```rust
#[tauri::command]
pub async fn download_update(
    app: tauri::AppHandle,
    state: tauri::State<'_, UpdaterState>,
    url: String,
) -> Result<(), String> {
    use futures_util::StreamExt;
    use std::sync::atomic::Ordering;
    use tauri::Emitter;

    // 并发防护
    if state.downloading.swap(true, Ordering::SeqCst) {
        return Err("download already in progress".into());
    }

    state.cancel.store(false, Ordering::SeqCst);
    let sig = state.pending.lock().unwrap().as_ref()
        .map(|u| u.signature.clone())
        .ok_or_else(|| {
            state.downloading.store(false, Ordering::SeqCst);
            "no pending update; call check_update first".to_string()
        })?;

    let version = state.pending.lock().unwrap().as_ref()
        .map(|u| u.version.clone())
        .unwrap_or_default();

    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = match client.get(&url).send().await {
        Ok(r) if r.status().is_success() => r,
        Ok(r) => {
            state.downloading.store(false, Ordering::SeqCst);
            let msg = format!("download failed: HTTP {}", r.status());
            let _ = app.emit("update://error", ErrPayload { message: msg.clone() });
            return Err(msg);
        }
        Err(e) => {
            state.downloading.store(false, Ordering::SeqCst);
            let msg = format!("download failed: {e}");
            let _ = app.emit("update://error", ErrPayload { message: msg.clone() });
            return Err(msg);
        }
    };

    let total = resp.content_length().unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("filter-manage-update-{version}.exe"));
    let mut file = tokio::fs::File::create(&tmp).await.map_err(|e| e.to_string())?;
    let mut downloaded: u64 = 0;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        if state.cancel.load(Ordering::SeqCst) {
            drop(file);
            let _ = tokio::fs::remove_file(&tmp).await;
            state.downloading.store(false, Ordering::SeqCst);
            let _ = app.emit("update://cancelled", ());
            return Ok(());
        }
        let chunk = chunk.map_err(|e| e.to_string())?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await.map_err(|e| e.to_string())?;
        downloaded += chunk.len() as u64;
        let _ = app.emit("update://progress", ProgressPayload { downloaded, total });
    }

    drop(file);
    state.downloading.store(false, Ordering::SeqCst);

    // 校验：必须在标记可安装之前
    match verify_minisign(&tmp, &sig) {
        Ok(()) => {
            *state.staged_path.lock().unwrap() = Some(tmp);
            let _ = app.emit("update://verified", ());
            Ok(())
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            let msg = format!("签名校验失败: {e}");
            let _ = app.emit("update://error", ErrPayload { message: msg.clone() });
            Err(msg)
        }
    }
}
```

---

### `cancel_update_download`

置取消标志，正在进行的 `download_update` 在下一个 chunk 处中断并删除临时文件、emit `update://cancelled`。

**参数**：无

**返回值**：
```rust
Result<(), String>
```

**实现**：
```rust
#[tauri::command]
pub fn cancel_update_download(state: tauri::State<'_, UpdaterState>) -> Result<(), String> {
    use std::sync::atomic::Ordering;
    state.cancel.store(true, Ordering::SeqCst);
    Ok(())
}
```

**用途**：换源（取消后用新 URL 再调 `download_update`）或用户主动取消。

---

### `install_update`

启动已校验的 installer 并退出应用以完成替换。

**参数**：无（用 `state.staged_path`）

**返回值**：
```rust
Result<(), String>   // 成功时进程退出，前端通常收不到返回
```

**实现**：
```rust
#[tauri::command]
pub fn install_update(state: tauri::State<'_, UpdaterState>) -> Result<(), String> {
    let path = state.staged_path.lock().unwrap()
        .take()
        .ok_or("no staged update; download and verify first")?;

    if !path.exists() {
        return Err("staged update file missing".into());
    }

    // NSIS Passive 模式（显示进度条 + 自动重启）
    // /P = passive, /R = restart, /UPDATE = tauri 更新标志
    // 参考 tauri-plugin-updater config.rs: Passive → ["/P", "/R"]
    std::process::Command::new(&path)
        .args(["/P", "/R", "/UPDATE"])
        .spawn()
        .map_err(|e| format!("failed to launch installer: {e}"))?;

    std::process::exit(0);
}
```

> NSIS 安装参数来源：`tauri-plugin-updater` v2.10.1 `config.rs:36-44` — `Passive` 模式对应 `["/P", "/R"]`。`/UPDATE` 是 tauri 约定的更新标志（让 installer 知道这是自动更新而非手动安装）。

---

## 签名校验（minisign）

```rust
use base64::Engine;
use minisign_verify::{PublicKey, Signature};

fn verify_minisign(file_path: &std::path::Path, signature_b64: &str) -> Result<(), String> {
    // 读取下载的安装包
    let data = std::fs::read(file_path).map_err(|e| e.to_string())?;

    // 解码公钥：base64 → minisign 文本格式
    let pubkey_bytes = base64::engine::general_purpose::STANDARD
        .decode(PUBKEY)
        .map_err(|e| e.to_string())?;
    let pubkey_text = std::str::from_utf8(&pubkey_bytes)
        .map_err(|e| e.to_string())?;
    let public_key = PublicKey::decode(pubkey_text)
        .map_err(|e| e.to_string())?;

    // 解码签名：base64 → minisign .sig 文本格式
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|e| e.to_string())?;
    let sig_text = std::str::from_utf8(&sig_bytes)
        .map_err(|e| e.to_string())?;
    let signature = Signature::decode(sig_text)
        .map_err(|e| e.to_string())?;

    // 校验（最后一个参数 true = 允许 trusted comment）
    public_key.verify(&data, &signature, true)
        .map_err(|e| e.to_string())?;

    Ok(())
}
```

> 解码链路与 `tauri-plugin-updater` v2.10.1 `updater.rs:1453-1471` 完全一致：
> 1. `base64::STANDARD.decode(pubkey_b64)` → UTF-8 minisign 公钥文本
> 2. `PublicKey::decode(pubkey_text)` → minisign 公钥对象
> 3. `base64::STANDARD.decode(signature_b64)` → UTF-8 minisign .sig 文本
> 4. `Signature::decode(sig_text)` → minisign 签名对象
> 5. `public_key.verify(data, &signature, true)` → 校验

---

## 事件

| 事件名 | payload | 时机 |
|--------|---------|------|
| `update://progress` | `{ downloaded: u64, total: u64 }` | 每个下载 chunk |
| `update://verified` | `{}` | 下载完成 + 校验通过 |
| `update://error` | `{ message: String }` | 下载/校验失败 |
| `update://cancelled` | `{}` | 取消完成 |

> emit 需要 `use tauri::Emitter;`（项目已在 `process_watcher.rs:10` 验证此 import）。

---

## lib.rs 注册

```rust
mod updater;  // 新增

// ... 在 Builder 链中：
.manage(updater::UpdaterState::default())
.invoke_handler(tauri::generate_handler![
    // ... 既有命令
    updater::check_update,
    updater::download_update,
    updater::cancel_update_download,
    updater::install_update,
])
```

---

## 安全红线

1. **先校验后安装**：下载源是不可信镜像（gh-proxy），`install_update` 前必须 `update://verified`。
2. **签名透传不可改**：`signature` 全程透传，客户端只验不改。
3. **临时文件清理**：取消/失败/校验不过都删临时文件。
4. **并发防护**：`downloading` 原子标志防止重复下载。

---

## 降级策略

服务端不可达时（网络错误、超时、非 2xx），`check_update` 自动回退到 `tauri-plugin-updater` 直连 GitHub `latest.json`：

- 需要保留 `tauri.conf.json` 中的 `plugins.updater.endpoints` 和 `pubkey` 配置
- 降级时 `mirrors` 只有一个「GitHub 直连」条目
- 降级结果同样缓存到 `state.pending`，后续 download/install 流程一致

---

## 与内置 updater 的关系

- `check_update` 降级路径使用 `tauri-plugin-updater` 的 `check()`
- `download_update` / `install_update` 完全自建，不调用插件
- `tauri.conf.json` 的 `plugins.updater` 配置保留（降级 + pubkey 用）
- `process:allow-restart` / `process:allow-exit` 权限保留

---

**关联**：[plan](../plans/custom-update-flow.md) · [前端交接](../handoff/custom-updater-frontend.md) · [服务端交接](../handoff/custom-update-server.md)
