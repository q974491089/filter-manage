import { invoke } from "@tauri-apps/api/core";

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
  selectedDeviceId: string | undefined;
  monitors: DisplayMonitor[];
  currentMonitorName?: string;
  onBrightnessChange: (value: number) => void;
  onContrastChange: (value: number) => void;
  onGammaChange: (value: number) => void;
  onDigitalVibranceChange: (value: number) => void;
  onDeviceChange: (deviceId: string | undefined) => void;
}

function ColorAdjuster({
  brightness,
  contrast,
  gamma,
  digitalVibrance,
  selectedDeviceId,
  currentMonitorName,
  onBrightnessChange,
  onContrastChange,
  onGammaChange,
  onDigitalVibranceChange,
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

  return (
    <div data-components="ColorAdjuster">
      <div className="mb-md">
        <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface font-medium">显示参数调整</h3>
      </div>

      <div data-name="info-banner" className="mb-sm">
        <div className="py-sm px-md bg-primary/8 rounded-md flex gap-sm items-start border border-primary/15">
          <span className="material-symbols-outlined text-primary text-[18px] mt-px">info</span>
          <p className="text-[12px] leading-relaxed text-on-surface-variant">
            正在调整 {currentMonitorName || "显示器"} 的设置，调整会实时改变您的显示输出。
          </p>
        </div>
      </div>

      <div data-name="sliders-card" className="flex-1 bg-surface-container rounded-lg border border-outline-variant/20 p-lg shadow-sm">
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
}: SliderControlProps) {
  return (
    <div data-components="SliderControl" data-name={label}>
      <div className="flex justify-between items-center">
        <label className="font-label-md text-label-md text-on-surface flex items-center gap-xs">
          <span className="material-symbols-outlined text-primary text-[18px]">{icon}</span>
          {label}
          {description && (
            <span className="relative group inline-flex items-center ml-xs">
              <span className="material-symbols-outlined text-on-surface-variant/40 text-[14px] cursor-help hover:text-on-surface-variant/70 transition-colors">info</span>
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
        className="w-full mt-[16px] pt-[6px]"
      />
    </div>
  );
}

export default ColorAdjuster;
