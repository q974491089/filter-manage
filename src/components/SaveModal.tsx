import { useEffect, useRef, useState } from "react";
import { Icon } from "./Icon";

const PRESET_ICONS = [
  { icon: "tune", label: "默认" },
  { icon: "movie", label: "电影" },
  { icon: "sports_esports", label: "游戏" },
  { icon: "edit_note", label: "护眼" },
  { icon: "photo_camera", label: "摄影" },
  { icon: "palette", label: "设计" },
  { icon: "code", label: "编程" },
  { icon: "music_note", label: "音乐" },
  { icon: "visibility", label: "标准" },
  { icon: "auto_awesome", label: "自定义" },
];

interface SaveModalProps {
  open: boolean;
  onClose: () => void;
  onSave: (name: string, icon?: string) => Promise<void>;
  loading: boolean;
  /** 已有方案名，用于禁止重名 */
  existingNames?: string[];
  /** 弹窗标题 */
  title?: string;
  /** 副标题说明 */
  description?: string;
  /** 确认按钮文案 */
  confirmLabel?: string;
}

function SaveModal({
  open,
  onClose,
  onSave,
  loading,
  existingNames = [],
  title = "保存配置方案",
  description = "为当前的颜色设置命名",
  confirmLabel = "保存",
}: SaveModalProps) {
  const [name, setName] = useState("");
  const [selectedIcon, setSelectedIcon] = useState("tune");
  const [customIconUrl, setCustomIconUrl] = useState("");
  const [useCustomIcon, setUseCustomIcon] = useState(false);
  const [error, setError] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (open) {
      setName("");
      setSelectedIcon("tune");
      setCustomIconUrl("");
      setUseCustomIcon(false);
      setError("");
      setTimeout(() => inputRef.current?.focus(), 100);
    }
  }, [open]);

  const handleSave = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;

    const duplicate = existingNames.some(
      (n) => n.toLowerCase() === trimmed.toLowerCase()
    );
    if (duplicate) {
      setError(`方案「${trimmed}」已存在，请换一个名称`);
      return;
    }

    const icon = useCustomIcon && customIconUrl ? customIconUrl : selectedIcon;
    try {
      await onSave(trimmed, icon);
      onClose();
    } catch {
      // 父组件已 Toast；保持弹窗打开以便改名重试
    }
  };

  if (!open) return null;

  return (
    <div data-components="SaveModal" className="fixed inset-0 z-[100] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />

      <div data-name="modal" className="relative bg-surface-container-high border border-outline-variant/30 rounded-xl shadow-xl w-full max-w-sm mx-4 overflow-hidden">
        <div className="p-xl">
          <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface mb-xs">{title}</h3>
          <p className="text-body-md text-on-surface-variant mb-md">{description}</p>

          <input
            data-name="name-input"
            ref={inputRef}
            type="text"
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              if (error) setError("");
            }}
            onKeyDown={(e) => e.key === "Enter" && handleSave()}
            placeholder="例如：电影观赏、游戏模式..."
            className="w-full px-md py-sm bg-surface-container border border-outline-variant/30 rounded-lg text-body-md text-on-surface placeholder:text-on-surface-variant/50 focus:outline-none focus:ring-2 focus:ring-primary/30 focus:border-primary transition-colors"
          />
          {error ? (
            <p className="text-body-sm text-error mt-sm mb-md" role="alert">
              {error}
            </p>
          ) : (
            <div className="mb-md" />
          )}

          <p className="text-body-sm text-on-surface-variant mb-sm">选择图标</p>
          <div className="flex flex-wrap gap-sm mb-md">
            {PRESET_ICONS.map((item) => (
              <button
                key={item.icon}
                onClick={() => { setSelectedIcon(item.icon); setUseCustomIcon(false); }}
                className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors ${
                  !useCustomIcon && selectedIcon === item.icon
                    ? "bg-primary/20 text-primary"
                    : "bg-surface-container hover:bg-surface-variant text-on-surface-variant"
                }`}
                title={item.label}
              >
                <Icon name={item.icon} className="text-[20px]" />
              </button>
            ))}
          </div>

          <p className="text-body-sm text-on-surface-variant mb-sm">或上传自定义图标</p>
          <input
            type="text"
            value={customIconUrl}
            onChange={(e) => { setCustomIconUrl(e.target.value); setUseCustomIcon(true); }}
            placeholder="输入图标 URL..."
            className="w-full px-md py-sm bg-surface-container border border-outline-variant/30 rounded-lg text-body-sm text-on-surface placeholder:text-on-surface-variant/50 focus:outline-none focus:ring-2 focus:ring-primary/30 focus:border-primary transition-colors"
          />
          {customIconUrl && useCustomIcon && (
            <div className="mt-sm flex items-center gap-sm">
              <img src={customIconUrl} alt="预览" className="w-8 h-8 rounded object-cover" onError={(e) => (e.currentTarget.style.display = "none")} />
              <span className="text-body-sm text-on-surface-variant">预览</span>
            </div>
          )}
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
            {loading ? "保存中..." : confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}

export default SaveModal;
