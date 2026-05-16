import { useEffect, useRef, useState } from "react";

interface SaveModalProps {
  open: boolean;
  onClose: () => void;
  onSave: (name: string) => Promise<void>;
  loading: boolean;
}

function SaveModal({ open, onClose, onSave, loading }: SaveModalProps) {
  const [name, setName] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setName("");
      setTimeout(() => inputRef.current?.focus(), 100);
    }
  }, [open]);

  const handleSave = async () => {
    if (!name.trim()) return;
    await onSave(name.trim());
    onClose();
  };

  if (!open) return null;

  return (
    <div data-components="SaveModal" className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />

      <div data-name="modal" className="relative bg-surface-container-high border border-outline-variant/30 rounded-xl shadow-xl w-full max-w-sm mx-4 overflow-hidden">
        <div className="p-xl">
          <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface mb-xs">保存配置方案</h3>
          <p className="text-body-md text-on-surface-variant mb-lg">为当前的颜色设置命名</p>

          <input
            data-name="name-input"
            ref={inputRef}
            type="text"
            value={name}
            onChange={(e) => setName(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSave()}
            placeholder="例如：电影观赏、游戏模式..."
            className="w-full px-md py-sm bg-surface-container border border-outline-variant/30 rounded-lg text-body-md text-on-surface placeholder:text-on-surface-variant/50 focus:outline-none focus:ring-2 focus:ring-primary/30 focus:border-primary transition-colors"
          />
        </div>

        <div className="flex border-t border-outline-variant/20">
          <button
            data-name="cancel-button"
            onClick={onClose}
            disabled={loading}
            className="flex-1 px-md py-md text-body-md text-on-surface-variant hover:bg-surface-variant font-medium transition-colors disabled:opacity-40"
          >
            取消
          </button>
          <div className="w-px bg-outline-variant/20" />
          <button
            data-name="save-button"
            onClick={handleSave}
            disabled={loading || !name.trim()}
            className="flex-1 px-md py-md text-body-md text-primary hover:bg-primary/10 font-medium transition-colors disabled:opacity-40"
          >
            {loading ? "保存中..." : "保存"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default SaveModal;
