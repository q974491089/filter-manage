/**
 * RGB 增益：内部统一 -100..+100（0 = 无偏色）。
 * 调节方式只换算滑条显示，不改变底层值。
 */

export type RgbScaleMode = "nvidia" | "zowie" | "aoc";

export const RGB_SCALE_OPTIONS: {
  value: RgbScaleMode;
  label: string;
  /** 下拉旁简短提示 */
  hint: string;
  /** 「显示色彩调整」二级标题下的说明，随调节方式切换 */
  description: string;
}[] = [
  {
    value: "nvidia",
    label: "NVIDIA",
    hint: "±100，默认 0",
    description: "NVIDIA 标度：-100 ~ +100，0 为无偏色，正负双向调节",
  },
  {
    value: "zowie",
    label: "卓伟",
    hint: "0–100，默认 100",
    description: "卓伟标度：0 ~ 100，默认 100，从 100 往下拉减弱该色",
  },
  {
    value: "aoc",
    label: "AOC",
    hint: "0–100，默认 50",
    description: "AOC 标度：0 ~ 100，默认 50，向两侧拉高或压低通道",
  },
];

/** 当前调节方式在二级标题下展示的说明文案 */
export function scaleModeDescription(mode: RgbScaleMode): string {
  return RGB_SCALE_OPTIONS.find((o) => o.value === mode)?.description
    ?? "RGB 增益偏色调节";
}

export interface ScaleRange {
  min: number;
  max: number;
  /** 该方式下「无偏色」的显示值 */
  neutral: number;
}

export function scaleRange(mode: RgbScaleMode): ScaleRange {
  switch (mode) {
    case "nvidia":
      return { min: -100, max: 100, neutral: 0 };
    case "zowie":
      // 显示 0–100，默认 100；内部 = 显示 - 100（从 100 往下拉减弱）
      return { min: 0, max: 100, neutral: 100 };
    case "aoc":
      // 显示 0–100，默认 50；内部 = (显示 - 50) × 2
      return { min: 0, max: 100, neutral: 50 };
  }
}

/** 内部值 → 当前调节方式下的显示值 */
export function toDisplay(internal: number, mode: RgbScaleMode): number {
  const v = Math.round(Math.max(-100, Math.min(100, internal)));
  switch (mode) {
    case "nvidia":
      return v;
    case "zowie":
      return Math.max(0, Math.min(100, v + 100));
    case "aoc":
      return Math.max(0, Math.min(100, Math.round(v / 2 + 50)));
  }
}

/** 显示值 → 内部 -100..+100 */
export function toInternal(display: number, mode: RgbScaleMode): number {
  switch (mode) {
    case "nvidia":
      return Math.max(-100, Math.min(100, Math.round(display)));
    case "zowie":
      return Math.max(-100, Math.min(100, Math.round(display) - 100));
    case "aoc":
      return Math.max(-100, Math.min(100, Math.round((display - 50) * 2)));
  }
}

export function formatDisplay(display: number, mode: RgbScaleMode): string {
  if (mode === "nvidia") {
    return display > 0 ? `+${display}` : `${display}`;
  }
  return `${display}`;
}
