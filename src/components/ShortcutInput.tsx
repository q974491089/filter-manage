import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Icon } from "./Icon";

interface ShortcutInputProps {
  value: string;
  onChange: (shortcut: string) => void;
  onClear: () => void;
  placeholder?: string;
}

/** 格式化快捷键为紧凑显示：CommandOrControl → Ctrl，F2 保持原样 */
function formatShortcutDisplay(value: string): string {
  return value
    .replace(/CommandOrControl/g, "Ctrl")
    .replace(/Control/g, "Ctrl");
}

function ShortcutInput({ value, onChange, onClear, placeholder = "点击录制快捷键" }: ShortcutInputProps) {
  const [isListening, setIsListening] = useState(false);
  const inputRef = useRef<HTMLDivElement>(null);

  // 进入录制：暂停全局快捷键
  const startListening = async () => {
    try {
      await invoke("pause_shortcuts");
    } catch (e) {
      console.error("Failed to pause shortcuts:", e);
    }
    setIsListening(true);
  };

  // 退出录制：恢复全局快捷键
  const stopListening = async () => {
    setIsListening(false);
    try {
      await invoke("resume_shortcuts");
    } catch (e) {
      console.error("Failed to resume shortcuts:", e);
    }
  };

  useEffect(() => {
    if (!isListening) return;

    const handleKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      // Ignore bare modifier keys
      if (["Control", "Alt", "Shift", "Meta"].includes(e.key)) return;

      const parts: string[] = [];
      if (e.ctrlKey || e.metaKey) parts.push("CommandOrControl");
      if (e.altKey) parts.push("Alt");
      if (e.shiftKey) parts.push("Shift");

      // Map key names to Tauri format
      let key = e.key;
      if (key === " ") key = "Space";
      else if (key === "Escape") {
        stopListening();
        return;
      } else if (key.length === 1) key = key.toUpperCase();

      parts.push(key);
      onChange(parts.join("+"));
      stopListening();
    };

    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, [isListening, onChange]);

  // 组件卸载时兜底恢复快捷键
  useEffect(() => {
    return () => {
      if (isListening) {
        invoke("resume_shortcuts").catch(console.error);
      }
    };
  }, [isListening]);

  const displayValue = isListening
    ? "请按下快捷键..."
    : value
    ? formatShortcutDisplay(value)
    : placeholder;

  return (
    <div className="flex items-center gap-1.5">
      <div
        ref={inputRef}
        onClick={startListening}
        className={`flex-1 px-3 py-2 rounded-lg border cursor-pointer transition-colors ${
          isListening
            ? "bg-primary/10 border-primary/50 text-primary"
            : value
            ? "bg-surface-container border-outline-variant/30 text-on-surface"
            : "bg-surface-container border-outline-variant/30 text-on-surface-variant/50"
        }`}
      >
        {value && !isListening ? (
          <div className="flex items-center justify-center gap-1">
            {value.split("+").map((key, i) => (
              <kbd
                key={i}
                className="inline-flex items-center justify-center min-w-[24px] h-6 px-1.5 bg-surface-container-highest/60 border border-outline-variant/20 rounded text-[11px] font-mono font-medium text-on-surface"
              >
                {formatShortcutDisplay(key)}
              </kbd>
            ))}
          </div>
        ) : (
          <span className="text-body-sm font-mono">{displayValue}</span>
        )}
      </div>
      {value && (
        <button
          onClick={onClear}
          className="flex-shrink-0 w-8 h-8 flex items-center justify-center rounded-full text-on-surface-variant hover:bg-error/10 hover:text-error transition-colors"
          title="清除快捷键"
        >
          <Icon name="close" className="text-[18px]" />
        </button>
      )}
    </div>
  );
}

export default ShortcutInput;
