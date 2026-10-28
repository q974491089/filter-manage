/**
 * 客户端功能开关（feature flags）。
 *
 * 后台（Filter Manage Admin → 仪表盘）维护开关，服务端经 /api/feature-flags
 * 下发 {key: enabled}；Rust 端启动时拉取一次并落本地缓存（见 src-tauri/src/features.rs）。
 *
 * 判断规则统一为 `flags[key] !== false`：缺省、未知 key、拉取失败一律视为
 * 开放（fail-open），服务端故障不会把入口藏掉。新增受控入口时：在 Rust
 * builtin_defaults 补默认值 + 在这里加 key 常量 + 在对应组件接 isEnabled。
 */
export const FEATURE_ICC_MARKET = "icc_market";

export interface FeatureFlagsState {
  version: number;
  flags: Record<string, boolean>;
  /** remote | cache | default，标记数据来源便于排查 */
  source: string;
}

export function isFeatureEnabled(
  flags: Record<string, boolean>,
  key: string,
): boolean {
  return flags[key] !== false;
}
