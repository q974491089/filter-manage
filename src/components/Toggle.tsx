interface ToggleProps {
  checked: boolean;
  onChange: (checked: boolean) => void;
  size?: "sm" | "md";
  disabled?: boolean;
}

function Toggle({ checked, onChange, size = "md", disabled = false }: ToggleProps) {
  const trackClass = size === "sm" ? "w-9 h-5" : "w-11 h-6";
  const thumbClass = size === "sm" ? "w-4 h-4" : "w-5 h-5";

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
      className={`relative inline-flex items-center cursor-pointer active:scale-95 transition-transform duration-200 hover:scale-[1.02] disabled:opacity-40 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 focus-visible:ring-offset-1 rounded-full`}
    >
      <div className={`${trackClass} rounded-full transition-colors duration-300 ${checked ? "bg-primary" : "bg-surface-container-highest"}`}>
        <div className={`absolute top-[2px] left-[2px] ${thumbClass} bg-white rounded-full shadow transition-transform duration-300 ${checked ? "translate-x-full" : ""}`} />
      </div>
    </button>
  );
}

export default Toggle;
