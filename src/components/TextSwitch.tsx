interface TextSwitchProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  checkedLabel?: string;
  uncheckedLabel?: string;
  disabled?: boolean;
}

/**
 * 小号分段控制器 — 与主页显示器切换同款设计，缩小版。
 *
 * 容器 24px 高，两等分，滑块 w-1/2 用 translate-x 平移动画。
 * checked=true → 滑块在左（checkedLabel），checked=false → 滑块在右（uncheckedLabel）。
 */
function TextSwitch({
  checked,
  onChange,
  checkedLabel = "是",
  uncheckedLabel = "否",
  disabled = false,
}: TextSwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={`relative flex items-center h-6 w-[68px] rounded-md bg-surface-container-highest/60 p-[2px] transition-colors duration-200 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 disabled:opacity-40 disabled:cursor-not-allowed ${
        checked ? "bg-surface-container-highest/80" : ""
      }`}
    >
      {/* 是 */}
      <span
        className={`relative z-10 flex-1 flex items-center justify-center text-[11px] font-bold leading-none transition-colors duration-200 ${
          checked ? "text-primary" : "text-on-surface-variant/70"
        }`}
        onClick={(e) => { e.stopPropagation(); if (!checked) onChange(true); }}
      >
        {checkedLabel}
      </span>

      {/* 否 */}
      <span
        className={`relative z-10 flex-1 flex items-center justify-center text-[11px] font-bold leading-none transition-colors duration-200 ${
          !checked ? "text-on-surface" : "text-on-surface-variant/70"
        }`}
        onClick={(e) => { e.stopPropagation(); if (checked) onChange(false); }}
      >
        {uncheckedLabel}
      </span>

      {/* 滑块指示器：w-1/2 = 刚好一半，translate-x 滑动 */}
      <span
        className={`absolute top-[2px] bottom-[2px] left-[2px] w-[calc(50%-2px)] rounded-[4px] bg-surface-container shadow-sm transition-transform duration-300 ease-[cubic-bezier(0.2,0.8,0.2,1)] ${
          checked ? "translate-x-0" : "translate-x-full"
        }`}
      />
    </button>
  );
}

export default TextSwitch;
