import { invoke } from "./ipc";

/**
 * 客户端账号登录（邮箱验证码 + GitHub OAuth）。
 *
 * 服务端契约见 .docs/api/client-auth.md：market 服务签发不透明 Bearer 令牌
 * （30 天，refresh 原地轮换），令牌存 Windows 凭据管理器（Rust auth.rs 的
 * keyring），前端只持有 Session 快照用于渲染。401 = 令牌失效/被吊销（旧
 * 令牌重放会吊销整个会话），见 isUnauthorizedError——回登录态，禁止自动重试。
 */

export interface AuthAccount {
  id: string;
  displayName: string;
  email: string;
  status: string;
  createdAt: string;
  updatedAt: string;
}

export interface AuthSession {
  token: string;
  expiresAt: string;
  account: AuthAccount;
}

export interface AuthProvider {
  id: string;
  startUrl?: string;
}

/** Rust 侧把 401 统一包成 "unauthorized:<中文消息>"，前端据此回登录态。 */
export function isUnauthorizedError(err: unknown): boolean {
  return typeof err === "string" && err.startsWith("unauthorized:");
}

/** 服务端错误消息是面向用户的中文，直接可展示。 */
export function authErrorMessage(err: unknown): string {
  if (typeof err === "string") return err.replace(/^unauthorized:/, "");
  if (err instanceof Error) return err.message;
  return String(err);
}

export function getAuthProviders(): Promise<AuthProvider[]> {
  return invoke<AuthProvider[]>("auth_get_providers");
}

export function sendEmailCode(email: string): Promise<{ ok: boolean; expiresIn: number }> {
  return invoke<{ ok: boolean; expiresIn: number }>("auth_send_email_code", { email });
}

export function emailRegister(
  email: string,
  password: string,
  displayName: string,
  code: string,
): Promise<AuthSession> {
  return invoke<AuthSession>("auth_email_register", { email, password, displayName, code });
}

export function emailLogin(email: string, password: string): Promise<AuthSession> {
  return invoke<AuthSession>("auth_email_login", { email, password });
}

/** 起本地 loopback 监听，返回端口；浏览器回调的 ticket 经 auth://ticket 事件送回。 */
export function githubStart(): Promise<number> {
  return invoke<number>("auth_github_start");
}

export function exchangeTicket(ticket: string): Promise<AuthSession> {
  return invoke<AuthSession>("auth_exchange_ticket", { ticket });
}

/** 启动时校验；离线时返回缓存的上次已知会话。 */
export function getAuthSession(): Promise<AuthSession | null> {
  return invoke<AuthSession | null>("auth_get_session");
}

export function refreshAuth(): Promise<AuthSession> {
  return invoke<AuthSession>("auth_refresh");
}

export function authLogout(): Promise<void> {
  return invoke<void>("auth_logout");
}

/** 把本地投稿匿名身份（contributor UUID）绑到当前账号，登录成功后调一次。 */
export function bindContributor(contributorId: string, nickname: string): Promise<void> {
  return invoke<void>("auth_bind_contributor", { contributorId, nickname });
}
