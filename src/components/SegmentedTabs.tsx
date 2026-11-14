interface SegmentedTabsProps<T extends string> {
  items: Array<{ id: T; label: string }>;
  value: T;
  onChange: (id: T) => void;
  disabled?: boolean;
  /** surface：页面内亮色分段（与主页显示器切换同款视觉）；overlay：叠在图片上的暗色浮层 */
  variant?: "surface" | "overlay";
  ariaLabel?: string;
  /** 附加到容器上（定位用，如 absolute right-sm top-sm） */
  className?: string;
}

/**
 * N 选项滑块分段控制器 — 主页显示器切换（TextSwitch）的 N 选项推广。
 *
 * 滑块宽 = 单项宽（选项 flex-1 等分），切换时外层按 translateX(百分比) 平移，
 * 300ms 同款缓动；百分比相对滑块外层（宽 = 容器减 padding），逐项精确对齐无累积漂移。
 */
function SegmentedTabs<T extends string>({
  items,
  value,
  onChange,
  disabled = false,
  variant = "surface",
  ariaLabel,
  className = "",
}: SegmentedTabsProps<T>) {
  const index = Math.max(
    0,
    items.findIndex((item) => item.id === value),
  );
  const count = Math.max(items.length, 1);
  const palette =
    variant === "overlay"
      ? {
          track: "bg-black/60 backdrop-blur-[2px]",
          thumb: "bg-white/25",
          activeText: "text-white",
          idleText: "text-white/75 hover:text-white",
        }
      : {
          track: "bg-surface-container-highest/60",
          thumb: "bg-surface-container shadow-sm",
          activeText: "text-primary",
          idleText: "text-on-surface-variant/70 hover:text-on-surface-variant",
        };

  return (
    <div
      role="group"
      aria-label={ariaLabel}
      className={`relative flex items-stretch rounded-md p-[2px] ${palette.track} ${
        disabled ? "cursor-not-allowed opacity-40" : ""
      } ${className}`}
    >
      {/* 滑块：外层负责平移（translateX 百分比相对自身宽 = 单项宽 × index），内层铺满单项区域 */}
      <span
        aria-hidden
        className="absolute inset-y-[2px] left-[2px] flex transition-transform duration-300 ease-[cubic-bezier(0.2,0.8,0.2,1)] motion-reduce:transition-none"
        style={{
          width: `calc((100% - 4px) / ${count})`,
          transform: `translateX(${(index * 100) / count}%)`,
        }}
      >
        <span className={`flex-1 rounded-[4px] ${palette.thumb}`} />
      </span>
      {items.map((item) => {
        const active = item.id === value;
        return (
          <button
            key={item.id}
            type="button"
            aria-pressed={active}
            disabled={disabled}
            onClick={() => onChange(item.id)}
            className={`relative z-10 min-w-0 flex-1 whitespace-nowrap rounded-[4px] px-2 py-0.5 text-[11px] leading-5 transition-colors duration-200 ${
              active ? palette.activeText : palette.idleText
            } ${disabled ? "cursor-not-allowed" : ""}`}
          >
            {item.label}
          </button>
        );
      })}
    </div>
  );
}

export default SegmentedTabs;
