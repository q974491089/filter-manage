/** 本机目标显示器的调色后端；不写入 ColorConfig 或分享数据。 */
export type DisplayColorBackend = "standard" | "nvidia";

export interface DisplayColorStatus {
  device_id: string;
  stable_key: string | null;
  monitor_state: "enabled" | "disabled" | "unknown";
  gpu_vendor: "nvidia" | "other" | "unknown";
  hdr_enabled: boolean | null;
  preferred_backend: DisplayColorBackend;
  active_backend: DisplayColorBackend;
  /** available 仅表示驱动接口写入成功，不代表已验证屏幕输出。 */
  nvidia_support: "available" | "unavailable" | "unverified";
  reason: string | null;
  warnings: string[];
}

export type PresetResolutionResult =
  | { status: "unchanged" }
  | { status: "applied" }
  | { status: "restored" }
  | { status: "skipped_unsupported"; requested: string; reason: string }
  | { status: "skipped_invalid"; requested: string };

export interface DisplayColorApplyResult {
  status: DisplayColorStatus;
  resolution: PresetResolutionResult;
  warnings: string[];
}

interface ColorSettingNumbers {
  brightness: number;
  contrast: number;
  gamma: number;
  digital_vibrance: number;
  rgb_r?: number;
  rgb_g?: number;
  rgb_b?: number;
}

/** 分享可能携带更宽的数值范围；发送和显示统一采用当前输出支持的有效范围。 */
export function normalizeColorSettings<T extends ColorSettingNumbers>(config: T): { config: T; warnings: string[] } {
  const normalized = { ...config };
  let adjusted = false;
  const clamp = (value: number, low: number, high: number, integer = true) => {
    if (!Number.isFinite(value)) throw new Error("颜色参数必须是有效数值");
    const bounded = Math.max(low, Math.min(high, value));
    const result = integer ? Math.round(bounded) : bounded;
    if (result !== value) adjusted = true;
    return result;
  };
  normalized.brightness = clamp(config.brightness, -125, 125);
  normalized.contrast = clamp(config.contrast, -82, 82);
  normalized.gamma = clamp(config.gamma, 0.4, 2.8, false);
  normalized.digital_vibrance = clamp(config.digital_vibrance, 0, 100);
  normalized.rgb_r = clamp(config.rgb_r ?? 0, -100, 100);
  normalized.rgb_g = clamp(config.rgb_g ?? 0, -100, 100);
  normalized.rgb_b = clamp(config.rgb_b ?? 0, -100, 100);
  return { config: normalized, warnings: adjusted ? ["部分颜色参数超出可用范围，已调整到当前支持的范围"] : [] };
}
