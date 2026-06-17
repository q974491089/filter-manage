import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";

interface SelectOption {
  label: string;
  value: string;
}

interface SelectProps {
  value: string;
  onChange: (value: string) => void;
  options: SelectOption[];
  placeholder?: string;
  className?: string;
}

function Select({ value, onChange, options, placeholder = "请选择...", className = "" }: SelectProps) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);

  const selected = options.find((o) => o.value === value);

  useEffect(() => {
    const handleClick = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handleClick);
    return () => document.removeEventListener("mousedown", handleClick);
  }, []);

  return (
    <div ref={ref} data-components="Select" className={`relative ${className}`}>
      <button
        data-name="trigger"
        onClick={() => setOpen(!open)}
        className="bg-surface-container-high px-md py-xs rounded-lg border border-outline-variant/30 flex items-center gap-sm text-on-surface-variant hover:bg-surface-variant/50 transition-colors w-full"
      >
        <span className={`font-label-md text-label-md truncate flex-1 text-left ${selected ? "text-on-surface" : "text-on-surface-variant"}`}>
          {selected ? selected.label : placeholder}
        </span>
        <Icon name="expand_more" className={`text-[18px] transition-transform duration-200 ${open ? "rotate-180" : ""}`} />
      </button>

      {open && (
        <div data-name="dropdown" className="absolute z-50 mt-1.5 w-full bg-surface-container-high border border-outline-variant/30 rounded-lg shadow-lg overflow-hidden">
          <div className="max-h-60 overflow-y-auto py-1">
            {options.length === 0 ? (
              <div className="px-3 py-2 text-label-md text-on-surface-variant">暂无选项</div>
            ) : (
              options.map((option) => (
                <button
                  key={option.value}
                  onClick={() => {
                    onChange(option.value);
                    setOpen(false);
                  }}
                  className={`w-full flex items-center justify-between gap-2 px-3 py-2 text-label-md transition-colors ${
                    option.value === value
                      ? "bg-primary/10 text-primary"
                      : "text-on-surface hover:bg-surface-variant/50"
                  }`}
                >
                  <span className="truncate">{option.label}</span>
                  {option.value === value && (
                    <Icon name="check" className="text-primary text-[16px]" />
                  )}
                </button>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  );
}

export default Select;
