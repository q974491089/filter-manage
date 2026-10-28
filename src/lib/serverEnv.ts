import { invoke } from "./ipc";

/**
 * 接口环境（线上 / 预发）运行时状态。
 *
 * Rust 端（src-tauri/src/server_env.rs）按 cfg!(debug_assertions) 判定开发
 * 构建：只有开发构建（tauri dev）才允许切到预发；正式版 isDev 恒为 false、
 * 强制走线上，设置页不显示开发设置区块。
 */
export type ApiEnv = "production" | "staging";

export interface RuntimeEnv {
  isDev: boolean;
  apiEnv: ApiEnv;
  apiHosts: string[];
  marketApiBase: string;
}

const PROD_MARKET_API = "https://filter-manage-admin.xyls.us.kg";

// 未加载完成 / get_runtime_env 调用失败时的兜底：与线上行为完全一致
let current: RuntimeEnv = {
  isDev: false,
  apiEnv: "production",
  apiHosts: [],
  marketApiBase: PROD_MARKET_API,
};

/** 同步读取当前运行时环境（模块级缓存，未加载完返回线上兜底值）。 */
export function serverEnv(): RuntimeEnv {
  return current;
}

/** 启动时调用一次；之后切换环境由 switchApiEnv 更新缓存。 */
export async function initServerEnv(): Promise<RuntimeEnv> {
  try {
    current = await invoke<RuntimeEnv>("get_runtime_env");
  } catch (e) {
    console.error("[ServerEnv] load failed:", e);
    // 静默：保持线上兜底值
  }
  return current;
}

/**
 * 切换接口环境（仅开发构建，正式版 Rust 端会拒绝）。
 * 成功后更新本地缓存并返回新环境；功能开关 / 市场请求即刻走新环境，
 * 公告与更新检查在下次启动生效。
 */
export async function switchApiEnv(env: ApiEnv): Promise<RuntimeEnv> {
  current = await invoke<RuntimeEnv>("set_api_env", { env });
  return current;
}
