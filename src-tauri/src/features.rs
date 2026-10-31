use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::config::get_config_dir;
use crate::server_env;

/// 远程功能开关（feature flags）。
///
/// 后台在 feature_flags 表里维护开关，客户端拉 GET /api/feature-flags
/// 决定显示哪些入口（分享按钮、ICC 市场等）。
///
/// 加载与降级语义（fail-closed，服务不可达时受控入口一律隐藏）：
/// 1. 先读本地缓存 feature-flags.json 立即渲染（不发起网络，首屏快）；
/// 2. 再请求接口：成功 → 覆盖缓存并按最新配置重渲染；
/// 3. 请求失败 → 前端清空 flags：受控入口全部隐藏。单服务端部署，
///    开关接口挂了其他接口大概率也挂了，藏起不可用的入口是对的，
///    即使缓存里是 true 也隐藏。
///
/// 缓存按接口环境隔离（记录 env 字段）：预发/线上的开关互不串用。

#[derive(Serialize, Deserialize, Clone)]
pub struct FeatureFlagsPayload {
    #[serde(default)]
    pub version: i64,
    /// key → enabled。前端判断规则为「仅显式 true 才展示」（缺省/未知 key = 隐藏）。
    #[serde(default)]
    pub flags: HashMap<String, bool>,
    /// 写缓存时记录来源环境；读取时不匹配视为无缓存（预发/线上互不覆盖）。
    #[serde(default)]
    pub env: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FeatureFlagsResult {
    pub version: i64,
    pub flags: HashMap<String, bool>,
    /// remote | cache | none | unavailable，便于调试日志定位数据来源
    pub source: String,
}

const CACHE_FILE: &str = "feature-flags.json";

fn current_env() -> String {
    server_env::get_runtime_env().api_env
}

fn read_cache() -> Option<FeatureFlagsPayload> {
    let path = get_config_dir().ok()?.join(CACHE_FILE);
    let json = std::fs::read_to_string(path).ok()?;
    let payload = serde_json::from_str::<FeatureFlagsPayload>(&json).ok()?;
    if payload.env != current_env() {
        return None;
    }
    Some(payload)
}

fn write_cache(payload: &FeatureFlagsPayload) {
    let Ok(dir) = get_config_dir() else {
        return;
    };
    let mut tagged = payload.clone();
    tagged.env = current_env();
    let Ok(json) = serde_json::to_string(&tagged) else {
        return;
    };
    // 缓存写失败不影响本次结果，下次成功会重新覆盖
    let _ = std::fs::write(dir.join(CACHE_FILE), json);
}

/// 双域名竞速拉取开关（语义同 get_announcements）：
/// 首个成功者赢、失败者等另一个；两者都失败返回 None。
async fn fetch_remote() -> Option<FeatureFlagsPayload> {
    let hosts = server_env::update_api_hosts();

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

    // 单域名（预发）直接请求；双域名竞速：首个成功者赢，失败者等另一个。
    if hosts.len() < 2 {
        return match hosts.first() {
            Some(host) => fetch(host).await,
            None => None,
        };
    }

    let mut fut_a = tokio::spawn(fetch(hosts[0]));
    let mut fut_b = tokio::spawn(fetch(hosts[1]));

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

/// 读取本地缓存的功能开关（首屏用，毫秒级返回，不发起网络请求）。
/// 无缓存或缓存环境不匹配时返回空 flags，前端按 fail-closed 先隐藏受控入口。
#[tauri::command]
pub fn get_cached_feature_flags() -> FeatureFlagsResult {
    match read_cache() {
        Some(cached) => FeatureFlagsResult {
            version: cached.version,
            flags: cached.flags,
            source: "cache".into(),
        },
        None => FeatureFlagsResult {
            version: 0,
            flags: HashMap::new(),
            source: "none".into(),
        },
    }
}

/// 请求最新开关配置（双域名竞速）。成功则覆盖缓存并返回；
/// 失败返回 Err，由前端按 fail-closed 清空 flags、隐藏全部受控入口。
#[tauri::command]
pub async fn fetch_feature_flags() -> Result<FeatureFlagsResult, String> {
    let payload = fetch_remote()
        .await
        .ok_or_else(|| "feature flags 服务不可达".to_string())?;
    // 响应结构可疑（flags 为空）按失败处理：宁可让前端隐藏入口
    if payload.flags.is_empty() {
        return Err("feature flags 响应为空".into());
    }
    write_cache(&payload);
    Ok(FeatureFlagsResult {
        version: payload.version,
        flags: payload.flags,
        source: "remote".into(),
    })
}
