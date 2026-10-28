//! 接口环境（线上 / 预发）的统一解析。
//!
//! 环境判定用 `cfg!(debug_assertions)`：`tauri dev` 与 debug 构建为开发环境，
//! `tauri build` 正式产物恒为 false——所以 `AppSettings.api_env` 设置只可能
//! 在开发构建里生效，正式版即使配置残留也强制走线上接口。
//!
//! 各请求方（更新检查 / 公告 / 功能开关 / 市场下载）在发起请求时调用
//! `update_api_hosts()` / `market_api_base()`，无缓存，切换即刻生效。

use serde::Serialize;

pub const ENV_PRODUCTION: &str = "production";
pub const ENV_STAGING: &str = "staging";

/// 线上 API 双域名（更新检查 / 公告 / 功能开关共用，竞速取先成功者）
pub const PROD_API_HOSTS: &[&str] = &[
    "https://filter-manage-api.xyls.us.kg",
    "https://filter-manage-api.6ya.kdns.fr",
];

/// 线上市场/后台同域（openresty 按 /api/market、/api/internal/icc 分流到 market 服务）
pub const PROD_MARKET_API: &str = "https://filter-manage-admin.xyls.us.kg";

/// 预发：openresty 同域分流（/api/ → api-staging，/api/market/ → market-staging）
pub const STAGING_API: &str = "https://filter-manage-admin-staging.xyls.us.kg";

/// 开发构建 = `tauri dev` / cargo debug；正式发布（tauri build）恒为 false。
pub fn is_dev() -> bool {
    cfg!(debug_assertions)
}

/// 纯函数便于单测：返回 (API 主机列表, 市场接口基址)。
fn resolve(api_env: &str, dev: bool) -> (Vec<&'static str>, &'static str) {
    if dev && api_env == ENV_STAGING {
        (vec![STAGING_API], STAGING_API)
    } else {
        (PROD_API_HOSTS.to_vec(), PROD_MARKET_API)
    }
}

fn stored_api_env() -> String {
    crate::config::read_store()
        .map(|store| store.settings.api_env)
        .unwrap_or_default()
}

/// 请求发起时解析当前 API 主机列表（预发只有单域名，调用方需兼容 len==1）。
pub fn update_api_hosts() -> Vec<&'static str> {
    resolve(&stored_api_env(), is_dev()).0
}

pub fn market_api_base() -> &'static str {
    resolve(&stored_api_env(), is_dev()).1
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEnv {
    pub is_dev: bool,
    pub api_env: String,
    pub api_hosts: Vec<String>,
    pub market_api_base: String,
}

#[tauri::command]
pub fn get_runtime_env() -> RuntimeEnv {
    let dev = is_dev();
    let stored = stored_api_env();
    let (hosts, market) = resolve(&stored, dev);
    RuntimeEnv {
        is_dev: dev,
        api_env: if stored.is_empty() {
            ENV_PRODUCTION.to_string()
        } else {
            stored
        },
        api_hosts: hosts.into_iter().map(String::from).collect(),
        market_api_base: market.to_string(),
    }
}

/// 切换接口环境并持久化到 app.json。仅开发构建可用；正式版直接拒绝。
/// 返回切换后的运行时环境，前端据此刷新 UI。
#[tauri::command]
pub fn set_api_env(env: String) -> Result<RuntimeEnv, String> {
    if !is_dev() {
        return Err("接口环境切换仅在开发构建（tauri dev）中可用".into());
    }
    let env = env.trim().to_string();
    if env != ENV_PRODUCTION && env != ENV_STAGING {
        return Err(format!("未知环境：{env}"));
    }
    let mut store = crate::config::read_store()?;
    // 线上持久化为空串（缺省值），避免正式版残留预发地址
    store.settings.api_env = if env == ENV_PRODUCTION {
        String::new()
    } else {
        env
    };
    crate::config::write_store(&store)?;
    Ok(get_runtime_env())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_build_always_uses_production() {
        // dev=false：无论存了什么设置都强制线上
        let (hosts, market) = resolve(ENV_STAGING, false);
        assert_eq!(hosts, PROD_API_HOSTS.to_vec());
        assert_eq!(market, PROD_MARKET_API);
    }

    #[test]
    fn dev_staging_points_to_staging() {
        let (hosts, market) = resolve(ENV_STAGING, true);
        assert_eq!(hosts, vec![STAGING_API]);
        assert_eq!(market, STAGING_API);
    }

    #[test]
    fn dev_default_is_production() {
        let (hosts, market) = resolve("", true);
        assert_eq!(hosts, PROD_API_HOSTS.to_vec());
        assert_eq!(market, PROD_MARKET_API);
    }
}
