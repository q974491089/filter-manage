/**
 * 客户端功能开关（feature flags）。
 *
 * 后台（Filter Manage Admin → 仪表盘）维护开关，服务端经 /api/feature-flags
 * 下发 {key: enabled}；客户端两段式加载（见 src/hooks/useFeatureFlags.ts 与
 * src-tauri/src/features.rs）：先读本地缓存立即渲染，再请求接口更新；
 * 请求失败则清空 flags——受控入口全部隐藏（fail-closed）。
 *
 * 判断规则统一为 `flags[key] === true`：仅显式 true 才展示。缺省、未知 key、
 * 无缓存、拉取失败一律视为关闭。单服务端部署，开关接口挂了其他接口大概率
 * 也挂了，藏入口比留着不可用的入口更对。
 *
 * 新增受控入口时：在这里加 key 常量 + 在对应组件接 isEnabled
 * （服务端 seed 需要内置时在 internal/features/repo.go 的 defaultFlags 补一行）。
 *
 * 子项（如 icc_market_upload）由服务端做父级级联：父项关闭时下发值已是
 * false，客户端无需关心层级，仍按扁平 key 判断。
 */
export const FEATURE_ICC_MARKET = "icc_market";
/** 子项：市场投稿（客户端「我要提供」/「我的提交」+ 后台审核列表） */
export const FEATURE_ICC_MARKET_UPLOAD = "icc_market_upload";
/** 子项：管理端投稿审核列表 */
export const FEATURE_ICC_MARKET_REVIEW = "icc_market_review";
/** 方案分享：客户端「分享」按钮与「导入分享链接」入口 */
export const FEATURE_PLAN_SHARE = "plan_share";

export interface FeatureFlagsState {
  version: number;
  flags: Record<string, boolean>;
  /** remote | cache | none | unavailable，标记数据来源便于排查 */
  source: string;
}

export function isFeatureEnabled(
  flags: Record<string, boolean>,
  key: string,
): boolean {
  return flags[key] === true;
}
