interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  size?: "sm" | "md";
  disabled?: boolean;
}

function Toggle({ checked, onChange, size = "md", disabled = false }: ToggleProps) {
  const config = {
    sm: { track: "w-9 h-5", thumb: "w-4 h-4", translate: "translate-x-[18px]" },
    md: { track: "w-11 h-6", thumb: "w-5 h-5", translate: "translate-x-[22px]" },
  }[size];

  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onChange(!checked);
        }
      }}
      className="relative inline-flex items-center shrink-0 cursor-pointer active:scale-95 transition-transform duration-200 hover:scale-[1.02] disabled:opacity-40 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 focus-visible:ring-offset-1 rounded-full"
    >
      <div className={`${config.track} rounded-full transition-colors duration-300 ${checked ? "bg-primary" : "bg-surface-container-highest"} relative`}>
        <div className={`absolute inset-[2px] ${config.thumb} bg-white rounded-full shadow-md transition-transform duration-300 ${checked ? config.translate : "translate-x-0"}`} />
      </div>
    </button>
  );
}

export default Toggle;
