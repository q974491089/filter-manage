import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "../lib/ipc";
import { isFeatureEnabled } from "../lib/featureFlags";
import type { FeatureFlagsState } from "../lib/featureFlags";

/**
 * 两段式加载功能开关：
 * 1. 先读本地缓存立即渲染（毫秒级，首屏不等网络；无缓存则受控入口先隐藏）；
 * 2. 再请求接口：成功 → 按最新配置重渲染；失败 → fail-closed 清空 flags，
 *    受控入口全部隐藏（即使缓存里是 true）。单服务端部署，开关接口挂了
 *    其他接口大概率也挂了，藏入口是对的。
 * refresh 供设置页切换接口环境后重新执行两段式流程（缓存按环境隔离）。
 */
export function useFeatureFlags() {
  const [state, setState] = useState<FeatureFlagsState>({
    version: 0,
    flags: {},
    source: "none",
  });
  // 环境切换/连续 refresh 的竞态保护：只采纳最新一次的结果
  const seqRef = useRef(0);

  const refresh = useCallback(async () => {
    const seq = ++seqRef.current;

    // 阶段 1：缓存先渲染
    try {
      const cached = await invoke<FeatureFlagsState>("get_cached_feature_flags");
      if (seqRef.current !== seq) return;
      setState(cached);
    } catch {
      // 缓存读不出来不阻塞后续请求
    }

    // 阶段 2：请求最新配置；失败 → 清空（fail-closed）
    try {
      const remote = await invoke<FeatureFlagsState>("fetch_feature_flags");
      if (seqRef.current !== seq) return;
      setState(remote);
    } catch (e) {
      if (seqRef.current !== seq) return;
      console.error("[FeatureFlags] fetch failed, 受控入口按关闭处理:", e);
      setState({ version: 0, flags: {}, source: "unavailable" });
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const isEnabled = (key: string) => isFeatureEnabled(state.flags, key);

  return { state, isEnabled, refresh };
}
