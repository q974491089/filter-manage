use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use futures_util::StreamExt;
use minisign_verify::{PublicKey, Signature};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

const PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IENEMUQ0NDVBMTMwMDhFQ0EKUldUS2pnQVRXa1Fkeld0YVdwb1N0clJybkp0OFR2cDhUdUhUSXNvZUZjOEpTdzBrSDFYQXBLcjYK";

pub(crate) const UPDATE_API_HOSTS: &[&str] = &[
    "https://filter-manage-api.xyls.us.kg",
    "https://filter-manage-api.6ya.kdns.fr",
];

// ─── 数据结构 ────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Mirror {
    pub name: String,
    pub url: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub has_update: bool,
    pub version: String,
    pub notes: String,
    pub signature: String,
    pub mirrors: Vec<Mirror>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ProgressPayload {
    downloaded: u64,
    total: u64,
}

#[derive(Serialize, Clone)]
struct ErrPayload {
    message: String,
}

// ─── 共享状态 ────────────────────────────────────────────────────────────────

pub struct UpdaterState {
    pub pending: Mutex<Option<UpdateInfo>>,
    pub staged_path: Mutex<Option<PathBuf>>,
    pub cancel: Arc<AtomicBool>,
    pub downloading: Arc<AtomicBool>,
}

impl Default for UpdaterState {
    fn default() -> Self {
        Self {
            pending: Mutex::new(None),
            staged_path: Mutex::new(None),
            cancel: Arc::new(AtomicBool::new(false)),
            downloading: Arc::new(AtomicBool::new(false)),
        }
    }
}

// ─── 命令 ────────────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    state: State<'_, UpdaterState>,
) -> Result<UpdateInfo, String> {
    let current = app.package_info().version.to_string();
    let host_a = UPDATE_API_HOSTS[0];
    let host_b = UPDATE_API_HOSTS[1];

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
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
        None => return fallback_check(&app, &state).await,
    };

    *state.pending.lock().unwrap() = Some(info.clone());
    Ok(info)
}

#[tauri::command]
pub async fn download_update(
    app: AppHandle,
    state: State<'_, UpdaterState>,
    url: String,
) -> Result<(), String> {
    if state.downloading.swap(true, Ordering::SeqCst) {
        return Err("download already in progress".into());
    }

    state.cancel.store(false, Ordering::SeqCst);

    let sig = match state.pending.lock().unwrap().as_ref() {
        Some(u) => u.signature.clone(),
        None => {
            state.downloading.store(false, Ordering::SeqCst);
            return Err("no pending update; call check_update first".into());
        }
    };

    let version = state
        .pending
        .lock()
        .unwrap()
        .as_ref()
        .map(|u| u.version.clone())
        .unwrap_or_default();

    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(120))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            state.downloading.store(false, Ordering::SeqCst);
            return Err(e.to_string());
        }
    };

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

    let mut file = match tokio::fs::File::create(&tmp).await {
        Ok(f) => f,
        Err(e) => {
            state.downloading.store(false, Ordering::SeqCst);
            return Err(e.to_string());
        }
    };

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

        let chunk = match chunk {
            Ok(c) => c,
            Err(e) => {
                drop(file);
                let _ = tokio::fs::remove_file(&tmp).await;
                state.downloading.store(false, Ordering::SeqCst);
                let msg = format!("download stream error: {e}");
                let _ = app.emit("update://error", ErrPayload { message: msg.clone() });
                return Err(msg);
            }
        };

        if let Err(e) = tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await {
            drop(file);
            let _ = tokio::fs::remove_file(&tmp).await;
            state.downloading.store(false, Ordering::SeqCst);
            let msg = format!("write error: {e}");
            let _ = app.emit("update://error", ErrPayload { message: msg.clone() });
            return Err(msg);
        }

        downloaded += chunk.len() as u64;
        let _ = app.emit("update://progress", ProgressPayload { downloaded, total });
    }

    drop(file);
    state.downloading.store(false, Ordering::SeqCst);

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

#[tauri::command]
pub fn cancel_update_download(state: State<'_, UpdaterState>) -> Result<(), String> {
    state.cancel.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn install_update(state: State<'_, UpdaterState>) -> Result<(), String> {
    let path = state
        .staged_path
        .lock()
        .unwrap()
        .take()
        .ok_or("no staged update; download and verify first")?;

    if !path.exists() {
        return Err("staged update file missing".into());
    }

    std::process::Command::new(&path)
        .args(["/P", "/R", "/UPDATE"])
        .spawn()
        .map_err(|e| format!("failed to launch installer: {e}"))?;

    std::process::exit(0);
}

// ─── 内部函数 ────────────────────────────────────────────────────────────────

async fn fallback_check(
    app: &AppHandle,
    state: &State<'_, UpdaterState>,
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

fn verify_minisign(file_path: &Path, signature_b64: &str) -> Result<(), String> {
    let data = std::fs::read(file_path).map_err(|e| e.to_string())?;

    let pubkey_bytes = base64::engine::general_purpose::STANDARD
        .decode(PUBKEY)
        .map_err(|e| e.to_string())?;
    let pubkey_text =
        std::str::from_utf8(&pubkey_bytes).map_err(|e| e.to_string())?;
    let public_key =
        PublicKey::decode(pubkey_text).map_err(|e| e.to_string())?;

    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_b64)
        .map_err(|e| e.to_string())?;
    let sig_text =
        std::str::from_utf8(&sig_bytes).map_err(|e| e.to_string())?;
    let signature =
        Signature::decode(sig_text).map_err(|e| e.to_string())?;

    public_key
        .verify(&data, &signature, true)
        .map_err(|e| e.to_string())?;

    Ok(())
}
