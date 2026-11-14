import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";

interface SegmentedTabsProps<T extends string> {
  items: Array<{ id: T; label: string }>;
  value: T;
  onChange: (id: T) => void;
  disabled?: boolean;
  /** surface：页面内亮色分段（与主页显示器切换同款视觉）；overlay：叠在图片上的暗色浮层 */
  variant?: "surface" | "overlay";
  /** sm：紧凑（高约 22px，适合嵌在卡片行内）；md：标准 */
  size?: "sm" | "md";
  ariaLabel?: string;
  /**
   * 附加到容器上。注意：组件根**不含 position 类**（避免与外部 absolute 冲突），
   * 滑块是 absolute 定位，调用方必须保证容器为 positioned（传 relative 或 absolute）。
   * 容器按内容收缩（inline-flex），需要占满时传 w-full 等。
   */
  className?: string;
}

/**
 * N 选项滑块分段控制器 — 主页显示器切换（TextSwitch）的 N 选项推广。
 *
 * 选项按文字自然宽度排布（不做等分拉伸），滑块实时测量选中项的位置与宽度，
 * 切换时以 300ms 同款缓动滑动过去；容器尺寸变化时 ResizeObserver 自动对齐。
 */
function SegmentedTabs<T extends string>({
  items,
  value,
  onChange,
  disabled = false,
  variant = "surface",
  size = "md",
  ariaLabel,
  className = "",
}: SegmentedTabsProps<T>) {
  const containerRef = useRef<HTMLDivElement>(null);
  const tabRefs = useRef<Array<HTMLButtonElement | null>>([]);
  const [thumb, setThumb] = useState<{ left: number; width: number } | null>(null);

  const measure = useCallback(() => {
    const index = Math.max(
      0,
      items.findIndex((item) => item.id === value),
    );
    const el = tabRefs.current[index];
    if (el) setThumb({ left: el.offsetLeft, width: el.offsetWidth });
  }, [items, value]);

  useLayoutEffect(() => {
    measure();
  }, [measure]);

  useEffect(() => {
    const el = containerRef.current;
    if (!el || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(() => measure());
    observer.observe(el);
    return () => observer.disconnect();
  }, [measure]);

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
  const sizeCls =
    size === "sm" ? "px-2 py-[3px] text-[10px] leading-4" : "px-2.5 py-1 text-[11px] leading-5";

  return (
    <div
      ref={containerRef}
      role="group"
      aria-label={ariaLabel}
      className={`relative inline-flex items-stretch rounded-md p-[2px] ${palette.track} ${
        disabled ? "cursor-not-allowed opacity-40" : ""
      } ${className}`}
    >
      {thumb ? (
        <span
          aria-hidden
          className={`absolute top-[2px] bottom-[2px] rounded-[4px] transition-[left,width] duration-300 ease-[cubic-bezier(0.2,0.8,0.2,1)] motion-reduce:transition-none ${palette.thumb}`}
          style={{ left: thumb.left, width: thumb.width }}
        />
      ) : null}
      {items.map((item, i) => {
        const active = item.id === value;
        return (
          <button
            key={item.id}
            ref={(el) => {
              tabRefs.current[i] = el;
            }}
            type="button"
            aria-pressed={active}
            disabled={disabled}
            onClick={() => onChange(item.id)}
            className={`relative z-10 whitespace-nowrap rounded-[4px] transition-colors duration-200 ${
              active ? palette.activeText : palette.idleText
            } ${disabled ? "cursor-not-allowed" : ""} ${sizeCls}`}
          >
            {item.label}
          </button>
        );
      })}
    </div>
  );
}

export default SegmentedTabs;
