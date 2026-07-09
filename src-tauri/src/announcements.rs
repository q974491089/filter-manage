use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::updater::UPDATE_API_HOSTS;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Announcement {
    pub id: String,
    pub title: String,
    pub body: String,
    /// "normal" | "important"
    pub level: String,
    /// ISO8601 UTC，前端排序用
    pub published_at: String,
    #[serde(default)]
    pub start_at: Option<String>,
    #[serde(default)]
    pub end_at: Option<String>,
}

/// 双域名竞速拉取公告。
///
/// 语义（区别于 updater::check_update）：
/// - 首个成功者赢，abort 另一个；
/// - 首个完成者失败 → 等另一个；
/// - 两者都失败 → 返回空列表（无公告不是错误，静默不打扰，无 GitHub 兜底）。
#[tauri::command]
pub async fn get_announcements() -> Result<Vec<Announcement>, String> {
    let host_a = UPDATE_API_HOSTS[0];
    let host_b = UPDATE_API_HOSTS[1];

    let client = match reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(_) => return Ok(Vec::new()),
    };

    let fetch = |host: &str| {
        let client = client.clone();
        let url = format!("{host}/api/announcements");
        async move {
            let resp = client.get(&url).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            resp.json::<Vec<Announcement>>().await.ok()
        }
    };

    let mut fut_a = tokio::spawn(fetch(host_a));
    let mut fut_b = tokio::spawn(fetch(host_b));

    let list: Option<Vec<Announcement>> = tokio::select! {
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
    };

    Ok(list.unwrap_or_default())
}
