/**
 * Icon 组件 — 替代 Material Symbols 字体图标
 *
 * 用法（与原方案对比）：
 *
 *   旧: <span className="material-symbols-outlined text-[18px]">settings</span>
 *   新: <Icon name="settings" className="text-[18px]" />
 *
 *   旧: <span className="material-symbols-outlined text-primary" style={{ fontVariationSettings: "'FILL' 1" }}>check_circle</span>
 *   新: <Icon name="check_circle" className="text-primary" filled />
 */

import { iconMap } from "../lib/icon-map";

interface IconProps {
  /** Material Symbols 图标名（如 "settings", "check_circle"） */
  name: string;
  /** 额外的 className（会透传给内部 SVG） */
  className?: string;
  /** 是否使用 filled 变体，默认 false (outlined) */
  filled?: boolean;
  /** 行内样式 */
  style?: React.CSSProperties;
  /** 点击事件 */
  onClick?: () => void;
}

export function Icon({ name, className = "", filled = false, style, onClick }: IconProps) {
  const entry = iconMap[name];

  if (!entry) {
    // 未知图标名，渲染占位
    console.warn(`[Icon] Unknown icon: "${name}"`);
    return <span className={className} style={style}>?</span>;
  }

  const IconComponent = filled ? entry.filled : entry.outlined;

  // react-icons 组件使用 currentColor，需要确保父元素的 color 被继承
  // 通过 style 显式设置 color: inherit 来继承父元素的文字颜色
  return (
    <span className={`inline-flex items-center justify-center ${className}`} style={style}>
      <IconComponent style={{ color: "inherit" }} onClick={onClick} />
    </span>
  );
}
