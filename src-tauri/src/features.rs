use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::get_config_dir;
use crate::updater::UPDATE_API_HOSTS;

/// 远程功能开关（feature flags）。
///
/// 后台在 feature_flags 表里维护开关，客户端启动时拉一次
/// GET /api/feature-flags，决定显示哪些入口（如 ICC 市场导入）。
///
/// 降级语义（fail-open，服务端故障不把用户锁在门外）：
/// 1. 拉取成功 → 覆盖本地缓存 feature-flags.json 并生效；
/// 2. 拉取失败或响应结构可疑（flags 为空）→ 用上次缓存；
/// 3. 从未成功过（新装机且服务不可达）→ 内置默认值：全部开放。

#[derive(Serialize, Deserialize, Clone)]
pub struct FeatureFlagsPayload {
    #[serde(default)]
    pub version: i64,
    /// key → enabled。未知 key 由前端按「缺省即开放」处理，向前兼容。
    #[serde(default)]
    pub flags: HashMap<String, bool>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlagsResult {
    pub version: i64,
    pub flags: HashMap<String, bool>,
    /// remote | cache | default，便于调试日志定位数据来源
    pub source: &'static str,
}

const CACHE_FILE: &str = "feature-flags.json";

fn builtin_defaults() -> HashMap<String, bool> {
    // 与服务端 seed 保持一致的已知开关；新增受控入口时在这里补默认值。
    HashMap::from([("icc_market".to_string(), true)])
}

fn read_cache() -> Option<FeatureFlagsPayload> {
    let path = get_config_dir().ok()?.join(CACHE_FILE);
    let json = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&json).ok()
}

fn write_cache(payload: &FeatureFlagsPayload) {
    let Ok(dir) = get_config_dir() else {
        return;
    };
    let Ok(json) = serde_json::to_string(payload) else {
        return;
    };
    // 缓存写失败不影响本次结果，下次启动会重新覆盖
    let _ = std::fs::write(dir.join(CACHE_FILE), json);
}

/// 双域名竞速拉取开关（语义同 get_announcements）：
/// 首个成功者赢、失败者等另一个；两者都失败返回 None。
async fn fetch_remote() -> Option<FeatureFlagsPayload> {
    let host_a = UPDATE_API_HOSTS[0];
    let host_b = UPDATE_API_HOSTS[1];

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .ok()?;

    let fetch = |host: &str| {
        let client = client.clone();
        let url = format!("{host}/api/feature-flags");
        async move {
            let resp = client.get(&url).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            resp.json::<FeatureFlagsPayload>().await.ok()
        }
    };

    let mut fut_a = tokio::spawn(fetch(host_a));
    let mut fut_b = tokio::spawn(fetch(host_b));

    tokio::select! {
        a = &mut fut_a => {
            match a.ok().flatten() {
                Some(v) => { fut_b.abort(); Some(v) }
                None => fut_b.await.ok().flatten(),
            }
        }
        b = &mut fut_b => {
            match b.ok().flatten() {
                Some(v) => { fut_a.abort(); Some(v) }
                None => fut_a.await.ok().flatten(),
            }
        }
    }
}

/// 启动时拉取一次功能开关。永不报错：失败链路是 远程 → 缓存 → 内置默认。
#[tauri::command]
pub async fn get_feature_flags() -> Result<FeatureFlagsResult, String> {
    if let Some(payload) = fetch_remote().await {
        if !payload.flags.is_empty() {
            write_cache(&payload);
            return Ok(FeatureFlagsResult {
                version: payload.version,
                flags: payload.flags,
                source: "remote",
            });
        }
    }
    if let Some(cached) = read_cache() {
        return Ok(FeatureFlagsResult {
            version: cached.version,
            flags: cached.flags,
            source: "cache",
        });
    }
    Ok(FeatureFlagsResult {
        version: 0,
        flags: builtin_defaults(),
        source: "default",
    })
}
