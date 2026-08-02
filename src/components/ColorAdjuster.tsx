import { invoke } from "@tauri-apps/api/core";
import { Icon } from "./Icon";
import Select from "./Select";
import {
  type RgbScaleMode,
  RGB_SCALE_OPTIONS,
  scaleRange,
  scaleModeDescription,
  toDisplay,
  toInternal,
  formatDisplay,
} from "../lib/rgbScale";

interface DisplayMonitor {
  name: string;
  device_id: string;
  pnp_id: string;
  is_primary: boolean;
}

interface ColorAdjusterProps {
  brightness: number;
  contrast: number;
  gamma: number;
  digitalVibrance: number;
  rgbR: number;
  rgbG: number;
  rgbB: number;
  rgbScaleMode: RgbScaleMode;
  selectedDeviceId: string | undefined;
  monitors: DisplayMonitor[];
  currentMonitorName?: string;
  onBrightnessChange: (value: number) => void;
  onContrastChange: (value: number) => void;
  onGammaChange: (value: number) => void;
  onDigitalVibranceChange: (value: number) => void;
  onRgbChange: (r: number, g: number, b: number) => void;
  onRgbScaleModeChange: (mode: RgbScaleMode) => void;
  onDeviceChange: (deviceId: string | undefined) => void;
}

function ColorAdjuster({
  brightness,
  contrast,
  gamma,
  digitalVibrance,
  rgbR,
  rgbG,
  rgbB,
  rgbScaleMode,
  selectedDeviceId,
  currentMonitorName,
  onBrightnessChange,
  onContrastChange,
  onGammaChange,
  onDigitalVibranceChange,
  onRgbChange,
  onRgbScaleModeChange,
}: ColorAdjusterProps) {
  const handleBrightnessChange = async (value: number) => {
    onBrightnessChange(value);
    try {
      await invoke("set_nvidia_brightness", { deviceId: selectedDeviceId, value });
    } catch (err) {
      console.error("Failed to set brightness:", err);
    }
  };

  const handleContrastChange = async (value: number) => {
    onContrastChange(value);
    try {
      await invoke("set_nvidia_contrast", { deviceId: selectedDeviceId, value });
    } catch (err) {
      console.error("Failed to set contrast:", err);
    }
  };

  const handleGammaChange = async (value: number) => {
    onGammaChange(value);
    try {
      await invoke("set_nvidia_gamma", { deviceId: selectedDeviceId, value });
    } catch (err) {
      console.error("Failed to set gamma:", err);
    }
  };

  const handleDigitalVibranceChange = async (value: number) => {
    onDigitalVibranceChange(value);
    try {
      await invoke("set_nvidia_digital_vibrance", { deviceId: selectedDeviceId, value });
    } catch (err) {
      console.error("Failed to set digital vibrance:", err);
    }
  };

  const pushRgb = async (r: number, g: number, b: number) => {
    onRgbChange(r, g, b);
    try {
      await invoke("set_nvidia_rgb_gain", {
        deviceId: selectedDeviceId,
        r,
        g,
        b,
      });
    } catch (err) {
      console.error("Failed to set RGB gain:", err);
    }
  };

  const handleRgbChannel = async (channel: "r" | "g" | "b", displayValue: number) => {
    const internal = toInternal(displayValue, rgbScaleMode);
    const next = {
      r: channel === "r" ? internal : rgbR,
      g: channel === "g" ? internal : rgbG,
      b: channel === "b" ? internal : rgbB,
    };
    await pushRgb(next.r, next.g, next.b);
  };

  const rgbRange = scaleRange(rgbScaleMode);
  // 直接查表，避免 import/缓存导致文案不更新
  const rgbSectionHint =
    RGB_SCALE_OPTIONS.find((o) => o.value === rgbScaleMode)?.description ??
    scaleModeDescription(rgbScaleMode);

  return (
    <div data-components="ColorAdjuster" className="flex flex-col h-full min-h-0">
      <div className="mb-md shrink-0">
        <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface font-medium">显示参数调整</h3>
      </div>

      <div data-name="info-banner" className="mb-sm shrink-0">
        <div className="py-sm px-md bg-primary/8 rounded-md flex gap-sm items-start border border-primary/15">
          <Icon name="info" className="text-primary text-[18px] mt-px" />
          <p className="text-[12px] leading-relaxed text-on-surface-variant">
            正在调整 {currentMonitorName || "显示器"} 的设置，调整会实时改变您的显示输出。
          </p>
        </div>
      </div>

      {/* 整卡撑满列高，底部与左右列卡片对齐；内容区可滚动 */}
      <div
        data-name="sliders-card"
        className="flex-1 min-h-0 flex flex-col bg-surface-container rounded-lg border border-outline-variant/20 p-lg shadow-sm overflow-y-auto"
      >
        {/* 二级：基础参数 */}
        <div data-name="basic-section" className="shrink-0">
          <div className="mb-md">
            <h4 className="font-label-md text-label-md text-on-surface font-medium">基础参数</h4>
            <p className="text-[11px] leading-snug text-on-surface-variant mt-0.5">
              亮度、对比度、伽马与数字振动，影响整体影调与饱和度
            </p>
          </div>
          <div className="space-y-md">
            <SliderControl
              label="亮度 (Brightness)"
              description="调整屏幕整体明暗程度"
              icon="light_mode"
              value={brightness}
              min={-125}
              max={125}
              step={1}
              formatValue={(v) => `${Math.round(((v + 125) / 250) * 100)}%`}
              onChange={handleBrightnessChange}
            />

            <SliderControl
              label="对比度 (Contrast)"
              description="调整亮暗区域的差异程度"
              icon="contrast"
              value={contrast}
              min={-82}
              max={82}
              step={1}
              formatValue={(v) => `${Math.round(((v + 82) / 164) * 100)}%`}
              onChange={handleContrastChange}
            />

            <SliderControl
              label="伽马值 (Gamma)"
              description="调整中间色调的亮度曲线"
              icon="Camera"
              value={gamma}
              min={0.1}
              max={3.0}
              step={0.01}
              formatValue={(v) => v.toFixed(2)}
              onChange={handleGammaChange}
            />

            <SliderControl
              label="数字振动 (Digital Vibrance)"
              description="调整色彩饱和度，数值越高颜色越鲜艳"
              icon="palette"
              value={digitalVibrance}
              min={0}
              max={100}
              step={1}
              formatValue={(v) => `${v}%`}
              onChange={handleDigitalVibranceChange}
            />
          </div>
        </div>

        {/* 二级：显示色彩调整 — 紧贴内容，不额外撑出大块空白 */}
        <div data-name="rgb-section" className="shrink-0 pt-md mt-md border-t border-outline-variant/20">
          <div className="flex items-start justify-between gap-md mb-md">
            <div className="min-w-0 flex-1 pr-sm">
              <h4 className="font-label-md text-label-md text-on-surface font-medium">显示色彩调整</h4>
              <p
                data-name="rgb-scale-hint"
                data-mode={rgbScaleMode}
                className="text-[11px] leading-snug text-on-surface-variant mt-0.5"
              >
                {rgbSectionHint}
              </p>
            </div>
            <div className="w-[132px] shrink-0">
              <Select
                value={rgbScaleMode}
                onChange={(v) => {
                  const mode = (v === "zowie" || v === "aoc" || v === "nvidia" ? v : "nvidia") as RgbScaleMode;
                  onRgbScaleModeChange(mode);
                }}
                options={RGB_SCALE_OPTIONS.map((o) => ({
                  value: o.value,
                  label: o.label,
                }))}
                placeholder="调节方式"
              />
            </div>
          </div>

          <div className="space-y-md">
            <SliderControl
              label="红 (R)"
              description="增强或减弱红色分量，用于偏色与白平衡"
              icon="palette"
              value={toDisplay(rgbR, rgbScaleMode)}
              min={rgbRange.min}
              max={rgbRange.max}
              step={1}
              formatValue={(v) => formatDisplay(v, rgbScaleMode)}
              onChange={(d) => handleRgbChannel("r", d)}
              accentClass="accent-red-500"
            />
            <SliderControl
              label="绿 (G)"
              description="增强或减弱绿色分量"
              icon="palette"
              value={toDisplay(rgbG, rgbScaleMode)}
              min={rgbRange.min}
              max={rgbRange.max}
              step={1}
              formatValue={(v) => formatDisplay(v, rgbScaleMode)}
              onChange={(d) => handleRgbChannel("g", d)}
              accentClass="accent-green-500"
            />
            <SliderControl
              label="蓝 (B)"
              description="增强或减弱蓝色分量（降低可偏暖）"
              icon="palette"
              value={toDisplay(rgbB, rgbScaleMode)}
              min={rgbRange.min}
              max={rgbRange.max}
              step={1}
              formatValue={(v) => formatDisplay(v, rgbScaleMode)}
              onChange={(d) => handleRgbChannel("b", d)}
              accentClass="accent-blue-500"
            />
          </div>
        </div>
      </div>
    </div>
  );
}

interface SliderControlProps {
  label: string;
  description?: string;
  icon: string;
  value: number;
  min: number;
  max: number;
  step: number;
  formatValue: (v: number) => string;
  onChange: (v: number) => void;
  accentClass?: string;
}

function SliderControl({
  label,
  description,
  icon,
  value,
  min,
  max,
  step,
  formatValue,
  onChange,
  accentClass,
}: SliderControlProps) {
  return (
    <div data-components="SliderControl" data-name={label}>
      <div className="flex justify-between items-center">
        <label className="font-label-md text-label-md text-on-surface flex items-center gap-xs">
          <Icon name={icon} className="text-primary text-[18px]" />
          {label}
          {description && (
            <span className="relative group inline-flex items-center ml-xs">
              <Icon name="info" className="text-on-surface-variant/40 text-[14px] cursor-help hover:text-on-surface-variant/70 transition-colors" />
              <span className="absolute bottom-full left-1/2 -translate-x-1/2 mb-2 px-2.5 py-1.5 rounded-md bg-surface-container-highest text-on-surface text-[11px] leading-snug whitespace-nowrap opacity-0 group-hover:opacity-100 transition-opacity pointer-events-none shadow-lg border border-outline-variant/20 z-10">
                {description}
              </span>
            </span>
          )}
        </label>
        <span className="text-primary font-medium font-label-md text-label-md bg-primary/10 px-sm py-xs rounded">
          {formatValue(value)}
        </span>
      </div>
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(parseFloat(e.target.value))}
        className={`w-full mt-[16px] pt-[6px] ${accentClass || ""}`}
      />
    </div>
  );
}

export default ColorAdjuster;
