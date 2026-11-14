import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { invoke } from "../lib/ipc";
import type { DisplayColorBackend, DisplayColorStatus } from "../lib/displayColor";

/** 查询结果只属于发起请求的目标；晚返回的旧查询不得覆盖切屏或切模式结果。 */
export function useDisplayColorBackend(enabled: boolean, deviceId: string | undefined, refreshKey: number) {
  const [status, setStatus] = useState<DisplayColorStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [querying, setQuerying] = useState(false);
  const [switching, setSwitching] = useState(false);
  const sequence = useRef(0);
  const switchPending = useRef(false);
  const target = useRef(deviceId);
  const isEnabled = useRef(enabled);
  target.current = deviceId;
  isEnabled.current = enabled;

  const refresh = useCallback(async () => {
    const requestedDevice = target.current;
    if (!requestedDevice) return;
    const request = ++sequence.current;
    setQuerying(true);
    try {
      const result = await invoke<DisplayColorStatus>("get_display_color_status", {
        deviceId: requestedDevice,
      });
      if (request !== sequence.current || target.current !== requestedDevice) return;
      setStatus(result);
      setError(null);
    } catch (err) {
      if (request !== sequence.current || target.current !== requestedDevice) return;
      // 保留同一目标最近已知的 B 状态，查询失败不能把紧急返回标准模式的入口藏掉。
      setError(String(err));
    } finally {
      if (request === sequence.current) setQuerying(false);
    }
  }, []);

  const adoptStatus = useCallback((result: DisplayColorStatus) => {
    if (result.device_id !== target.current) return;
    ++sequence.current;
    setQuerying(false);
    setStatus(result);
    setError(null);
  }, []);

  const switchBackend = useCallback(async (backend: DisplayColorBackend) => {
    const requestedDevice = target.current;
    if (!requestedDevice || switchPending.current || (backend === "nvidia" && !isEnabled.current)) return false;
    switchPending.current = true;
    ++sequence.current;
    setQuerying(false);
    setSwitching(true);
    setError(null);
    try {
      const result = await invoke<DisplayColorStatus>("set_display_color_backend", {
        deviceId: requestedDevice,
        backend,
      });
      adoptStatus(result);
      return result.active_backend === backend;
    } catch (err) {
      if (target.current === requestedDevice) setError(String(err));
      // 失败可能已回滚；查询实际后端，错误说明保留给调用方。
      await refresh();
      if (target.current === requestedDevice) setError(String(err));
      throw err;
    } finally {
      switchPending.current = false;
      setSwitching(false);
    }
  }, [adoptStatus, refresh]);

  useEffect(() => {
    ++sequence.current;
    setStatus(null);
    setError(null);
    setQuerying(false);
    return () => { ++sequence.current; };
  }, [deviceId, refresh]);

  useEffect(() => {
    if (deviceId) void refresh();
  }, [enabled, deviceId, refreshKey, refresh]);

  useEffect(() => {
    let cancelled = false;
    const unlisteners: (() => void)[] = [];
    for (const event of ["monitors-changed", "display-color-env-changed", "config-applied"]) {
      void listen(event, () => { if (!cancelled) void refresh(); }).then((unlisten) => {
        if (cancelled) unlisten();
        else unlisteners.push(unlisten);
      }).catch((err) => console.error(`[DisplayColor] ${event} 监听失败:`, err));
    }
    const onFocus = () => { if (!cancelled) void refresh(); };
    window.addEventListener("focus", onFocus);
    return () => {
      cancelled = true;
      ++sequence.current;
      window.removeEventListener("focus", onFocus);
      for (const unlisten of unlisteners) unlisten();
    };
  }, [refresh]);

  // 切屏 render 到 effect 清理之前也不展示旧屏幕能力，避免误启用新屏幕的按钮。
  const currentStatus = status?.device_id === deviceId ? status : null;
  return { status: currentStatus, error, querying, switching, refresh, adoptStatus, switchBackend };
}
