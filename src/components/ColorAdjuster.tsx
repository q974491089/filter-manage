import { invoke } from "@tauri-apps/api/core";

interface ColorAdjusterProps {
  brightness: number;
  contrast: number;
  gamma: number;
  digitalVibrance: number;
  onBrightnessChange: (value: number) => void;
  onContrastChange: (value: number) => void;
  onGammaChange: (value: number) => void;
  onDigitalVibranceChange: (value: number) => void;
}

function ColorAdjuster({
  brightness,
  contrast,
  gamma,
  digitalVibrance,
  onBrightnessChange,
  onContrastChange,
  onGammaChange,
  onDigitalVibranceChange,
}: ColorAdjusterProps) {
  const handleBrightnessChange = async (value: number) => {
    onBrightnessChange(value);
    try {
      await invoke("set_nvidia_brightness", { display: 1, value });
    } catch (err) {
      console.error("Failed to set brightness:", err);
    }
  };

  const handleContrastChange = async (value: number) => {
    onContrastChange(value);
    try {
      await invoke("set_nvidia_contrast", { display: 1, value });
    } catch (err) {
      console.error("Failed to set contrast:", err);
    }
  };

  const handleGammaChange = async (value: number) => {
    onGammaChange(value);
    try {
      await invoke("set_nvidia_gamma", { display: 1, value });
    } catch (err) {
      console.error("Failed to set gamma:", err);
    }
  };

  const handleDigitalVibranceChange = async (value: number) => {
    onDigitalVibranceChange(value);
    try {
      await invoke("set_nvidia_digital_vibrance", { display: 1, value });
    } catch (err) {
      console.error("Failed to set digital vibrance:", err);
    }
  };

  return (
    <div data-components="ColorAdjuster">
      <div className="mb-md">
        <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface">显示参数调整</h3>
      </div>

      <div data-name="info-banner" className="mb-sm">
        <div className="py-sm px-md bg-primary-container/20 rounded-lg flex gap-sm items-start">
          <span className="material-symbols-outlined text-primary text-[18px] mt-px">info</span>
          <p className="text-[12px] leading-relaxed text-on-surface-variant">
            调整这些设置会实时改变您的显示输出。建议先加载对应的 ICC 配置文件。
          </p>
        </div>
      </div>

      <div data-name="sliders-card" className="flex-1 bg-surface-container rounded-xl border border-outline-variant/20 p-xl">
        <div className="space-y-xl">
        <SliderControl
          label="亮度 (Brightness)"
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
  icon,
  value,
  min,
  max,
  step,
  formatValue,
  onChange,
}: SliderControlProps) {
  return (
    <div data-components="SliderControl" data-name={label} className="space-y-md">
      <div className="flex justify-between items-center">
        <label className="font-label-md text-label-md text-on-surface flex items-center gap-xs">
          <span className="material-symbols-outlined text-primary text-[20px]">{icon}</span>
          {label}
        </label>
        <span className="text-primary font-bold font-label-md text-label-md">
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
        className="w-full"
      />
    </div>
  );
}

export default ColorAdjuster;
