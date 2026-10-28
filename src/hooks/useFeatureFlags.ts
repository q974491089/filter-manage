import { useCallback, useEffect, useState } from "react";
import { invoke } from "../lib/ipc";
import { isFeatureEnabled } from "../lib/featureFlags";
import type { FeatureFlagsState } from "../lib/featureFlags";

/** 与 Rust 端内置默认一致：没有任何配置时全部开放。 */
const DEFAULT_STATE: FeatureFlagsState = {
  version: 0,
  flags: {},
  source: "default",
};

/**
 * 启动时经 Rust 拉取一次功能开关（远程 → 缓存 → 默认，永不报错）。
 * 语义与 useAnnouncements 相同：失败静默，不打扰用户。
 * refresh 供设置页切换接口环境后重新拉取。
 */
export function useFeatureFlags() {
  const [state, setState] = useState<FeatureFlagsState>(DEFAULT_STATE);

  const refresh = useCallback(async () => {
    try {
      const result = await invoke<FeatureFlagsState>("get_feature_flags");
      setState(result);
    } catch (e) {
      console.error("[FeatureFlags] fetch failed:", e);
      // 静默：保持当前状态（缺省全开放）
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const isEnabled = (key: string) => isFeatureEnabled(state.flags, key);

  return { state, isEnabled, refresh };
}
