// 客户端账号登录（client auth）。
//
// 服务端契约见 filter-manage 仓库 .docs/api/client-auth.md：market 服务
// （/api/market/auth/*）签发不透明 Bearer 令牌，30 天有效，refresh 原地轮换。
// 令牌只存 Windows 凭据管理器（keyring crate，按平台选后端），不进明文配置。
//
// 关键语义：
// - 401 = 令牌无效/过期/被吊销 → 清除本地令牌，前端回登录态；旧令牌重放
//   会吊销整个会话（RFC 9700），所以任何 401 都不允许自动重试。
// - 登录/注册/发码都是带副作用的 POST，走 market 单域名（market_api_base），
//   绝不套更新检查那套双域名竞速——竞速会把 POST 发两遍。
// - GitHub OAuth：服务端是唯一参与方（client_secret 不落地客户端）。本模块
//   只负责起 loopback 监听（std::net + 独立线程，tokio 未开 net 特性），把
//   回调 302 带来的一次性 ticket 经事件交给前端。

use crate::config::get_config_dir;
use crate::server_env;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::time::Duration;
use tauri::Emitter;

const KEYRING_SERVICE: &str = "filter-manage";
const KEYRING_USER: &str = "auth_token";
const TICKET_EVENT: &str = "auth://ticket";
const SESSION_CACHE_FILE: &str = "auth-session.json";
/// loopback 监听的最长寿命：用户在浏览器里停留的合理上限。
const LISTEN_TIMEOUT_SECS: u64 = 300;

// ─── 数据模型（与服务端 JSON 一一对应，camelCase 直传） ─────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub display_name: String,
    #[serde(default)]
    pub email: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub token: String,
    pub expires_at: String,
    pub account: Account,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthProvider {
    pub id: String,
    #[serde(default)]
    pub start_url: String,
}

#[derive(Debug, Deserialize)]
struct ProviderPayload {
    providers: Vec<AuthProvider>,
}

// ─── 令牌存取（keyring → Windows 凭据管理器） ───────────────────────────────

fn keyring_entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER).map_err(|e| format!("凭据管理器不可用: {e}"))
}

fn load_token() -> Option<String> {
    let entry = keyring_entry().ok()?;
    entry.get_password().ok().filter(|s| !s.is_empty())
}

fn save_token(token: &str) -> Result<(), String> {
    keyring_entry()?.set_password(token).map_err(|e| format!("保存令牌失败: {e}"))
}

fn clear_token() -> Result<(), String> {
    // 条目不存在时删除会报错，视为已清除。
    match keyring_entry().and_then(|e| e.delete_credential().map_err(|e| e.to_string())) {
        Ok(()) => Ok(()),
        Err(_) => Ok(()),
    }
}

// ─── 会话缓存（离线启动时展示上次已知身份，语义同 feature flags 缓存） ───────

fn read_session_cache() -> Option<Session> {
    let path = get_config_dir().ok()?.join(SESSION_CACHE_FILE);
    let json = std::fs::read_to_string(path).ok()?;
    let cached: Session = serde_json::from_str(&json).ok()?;
    if cached.token != load_token().unwrap_or_default() {
        return None; // 缓存与凭据管理器里的令牌不一致：以凭据管理器为准
    }
    Some(cached)
}

fn write_session_cache(session: &Session) {
    let Ok(dir) = get_config_dir() else { return };
    let Ok(json) = serde_json::to_string(session) else { return };
    let _ = std::fs::write(dir.join(SESSION_CACHE_FILE), json);
}

fn clear_session_cache() {
    if let Ok(dir) = get_config_dir() {
        let _ = std::fs::remove_file(dir.join(SESSION_CACHE_FILE));
    }
}

// ─── HTTP 封装 ──────────────────────────────────────────────────────────────

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| format!("HTTP client: {e}"))
}

/// 非 2xx 响应 → Err(message)。401 加固定前缀，前端据此回登录态。
async fn api_error(resp: reqwest::Response) -> String {
    let status = resp.status().as_u16();
    let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::Value::Null);
    let message = body
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("请求失败，请稍后再试");
    if status == 401 {
        format!("unauthorized:{message}")
    } else {
        message.to_string()
    }
}

async fn post_json(url: &str, body: &serde_json::Value) -> Result<reqwest::Response, String> {
    client()?
        .post(url)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {e}"))
}

fn auth_base() -> String {
    format!("{}/api/market/auth", server_env::market_api_base())
}

// ─── 命令：登录 / 注册 / 发码 ───────────────────────────────────────────────

#[tauri::command]
pub async fn auth_get_providers() -> Result<Vec<AuthProvider>, String> {
    let url = format!("{}/providers", auth_base());
    let resp = client()?
        .get(url)
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }
    let payload: ProviderPayload = resp.json().await.map_err(|e| format!("解析响应失败: {e}"))?;
    Ok(payload.providers)
}

#[tauri::command]
pub async fn auth_send_email_code(email: String) -> Result<serde_json::Value, String> {
    let url = format!("{}/email/send-code", auth_base());
    let resp = post_json(&url, &serde_json::json!({ "email": email })).await?;
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }
    resp.json().await.map_err(|e| format!("解析响应失败: {e}"))
}

#[tauri::command]
pub async fn auth_email_register(
    email: String,
    password: String,
    display_name: String,
    code: String,
) -> Result<Session, String> {
    let url = format!("{}/email/register", auth_base());
    let resp = post_json(
        &url,
        &serde_json::json!({
            "email": email,
            "password": password,
            "displayName": display_name,
            "code": code,
        }),
    )
    .await?;
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }
    let session: Session = resp.json().await.map_err(|e| format!("解析响应失败: {e}"))?;
    persist_session(&session)?;
    Ok(session)
}

#[tauri::command]
pub async fn auth_email_login(email: String, password: String) -> Result<Session, String> {
    let url = format!("{}/email/login", auth_base());
    let resp = post_json(&url, &serde_json::json!({ "email": email, "password": password })).await?;
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }
    let session: Session = resp.json().await.map_err(|e| format!("解析响应失败: {e}"))?;
    persist_session(&session)?;
    Ok(session)
}

// ─── 命令：GitHub OAuth（loopback 回调） ────────────────────────────────────

/// 起 loopback 监听并返回端口。前端拿到端口后用系统浏览器打开
/// `{marketApiBase}/api/market/auth/github/start?port=<port>`；服务端回调
/// 302 回 127.0.0.1:<port>/callback?ticket=…，本监听收到后 emit
/// `auth://ticket` 交给前端换会话。监听 5 分钟无回调自动退出。
#[tauri::command]
pub async fn auth_github_start(app: tauri::AppHandle) -> Result<u16, String> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| format!("本地监听失败: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    std::thread::spawn(move || loopback_listen(listener, &app));
    Ok(port)
}

fn loopback_listen(listener: std::net::TcpListener, app: &tauri::AppHandle) {
    let deadline = std::time::Instant::now() + Duration::from_secs(LISTEN_TIMEOUT_SECS);
    listener
        .set_nonblocking(true)
        .expect("loopback listener: set_nonblocking");
    loop {
        if std::time::Instant::now() > deadline {
            return; // 超时静默退出，前端由浏览器端超时提示兜底
        }
        match listener.accept() {
            Ok((mut stream, _)) => {
                if let Some(ticket) = read_ticket(&mut stream) {
                    let page = "<html lang=\"zh-CN\"><head><meta charset=\"utf-8\">\
                        <title>Filter Manage 授权</title></head>\
                        <body style=\"font-family:sans-serif;text-align:center;padding-top:80px;\">\
                        <h2>✅ 授权成功</h2>\
                        <p>请回到 Filter Manage 客户端，登录会自动完成。</p>\
                        </body></html>";
                    let _ = stream.write_all(
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                             Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                            page.len(),
                            page
                        )
                        .as_bytes(),
                    );
                    let _ = stream.flush();
                    let _ = app.emit(TICKET_EVENT, ticket);
                    return;
                }
                // 非 /callback 请求（如浏览器预连接）：应答后继续等。
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n");
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(_) => return,
        }
    }
}

/// 从 GET 请求行里抠 ticket 参数。ticket 是 base64url（无保留字符），服务端
/// QueryEscape 不会改变它，按原样读取即可。
fn read_ticket(stream: &mut std::net::TcpStream) -> Option<String> {
    let mut buf = [0u8; 2048];
    let n = stream.read(&mut buf).ok()?;
    let head = String::from_utf8_lossy(&buf[..n]);
    let line = head.lines().next()?;
    let path = line.split_whitespace().nth(1)?;
    for pair in path.split('?').nth(1)?.split('&') {
        if let Some(value) = pair.strip_prefix("ticket=") {
            let ticket = value.trim();
            if !ticket.is_empty() {
                return Some(ticket.to_string());
            }
        }
    }
    None
}

#[tauri::command]
pub async fn auth_exchange_ticket(ticket: String) -> Result<Session, String> {
    let url = format!("{}/ticket", auth_base());
    let resp = post_json(&url, &serde_json::json!({ "ticket": ticket })).await?;
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }
    let session: Session = resp.json().await.map_err(|e| format!("解析响应失败: {e}"))?;
    persist_session(&session)?;
    Ok(session)
}

// ─── 命令：会话维护 ─────────────────────────────────────────────────────────

/// 启动时校验。令牌无效（401）→ 清令牌返回 None；网络失败 → 返回缓存的
/// 上次已知会话，让界面保持登录样（下次联网自动校验）。
#[tauri::command]
pub async fn auth_get_session() -> Result<Option<Session>, String> {
    let Some(token) = load_token() else {
        return Ok(None);
    };
    let url = format!("{}/session", auth_base());
    let resp = match client() {
        Ok(c) => c,
        Err(e) => return Err(e),
    }
    .get(url)
    .bearer_auth(&token)
    .send()
    .await;
    let resp = match resp {
        Ok(resp) => resp,
        // 离线启动：凭据管理器里有令牌但服务端不可达，先按上次已知会话渲染。
        Err(_) => return Ok(read_session_cache()),
    };
    if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
        clear_token()?;
        clear_session_cache();
        return Ok(None);
    }
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }
    let payload: serde_json::Value = resp.json().await.map_err(|e| format!("解析响应失败: {e}"))?;
    let account: Account = serde_json::from_value(payload.get("account").cloned().ok_or("响应缺少 account")?)
        .map_err(|e| format!("解析账号失败: {e}"))?;
    let session = Session {
        token,
        expires_at: String::new(),
        account,
    };
    write_session_cache(&session);
    Ok(Some(session))
}

/// 临期前轮换令牌（30 天 TTL，启动时调用一次即可）。
#[tauri::command]
pub async fn auth_refresh() -> Result<Session, String> {
    let Some(token) = load_token() else {
        return Err("unauthorized:未登录".into());
    };
    let url = format!("{}/refresh", auth_base());
    let resp = client()?
        .post(url)
        .bearer_auth(&token)
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {e}"))?;
    if !resp.status().is_success() {
        let err = api_error(resp).await;
        if err.starts_with("unauthorized:") {
            clear_token()?;
            clear_session_cache();
        }
        return Err(err);
    }
    let session: Session = resp.json().await.map_err(|e| format!("解析响应失败: {e}"))?;
    persist_session(&session)?;
    Ok(session)
}

#[tauri::command]
pub async fn auth_logout() -> Result<(), String> {
    if let Some(token) = load_token() {
        if let Ok(client) = client() {
            // 服务端吊销尽力而为：网络失败也照样清本地令牌。
            let _ = client
                .post(format!("{}/logout", auth_base()))
                .bearer_auth(&token)
                .send()
                .await;
        }
    }
    clear_token()?;
    clear_session_cache();
    Ok(())
}

/// 把投稿匿名身份绑定到当前账号（登录成功后前端调用，409 冲突由前端提示）。
#[tauri::command]
pub async fn auth_bind_contributor(contributor_id: String, nickname: String) -> Result<(), String> {
    let Some(token) = load_token() else {
        return Err("unauthorized:未登录".into());
    };
    let url = format!("{}/bind-contributor", auth_base());
    let resp = client()?
        .post(url)
        .bearer_auth(&token)
        .json(&serde_json::json!({ "contributorId": contributor_id, "nickname": nickname }))
        .send()
        .await
        .map_err(|e| format!("网络请求失败: {e}"))?;
    if !resp.status().is_success() {
        return Err(api_error(resp).await);
    }
    Ok(())
}

fn persist_session(session: &Session) -> Result<(), String> {
    save_token(&session.token)?;
    write_session_cache(session);
    Ok(())
}
