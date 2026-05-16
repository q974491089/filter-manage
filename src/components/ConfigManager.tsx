import { invoke } from "@tauri-apps/api/core";

interface ColorConfig {
  name: string;
  brightness: number;
  contrast: number;
  gamma: number;
  digital_vibrance: number;
  icc_profile: string | null;
}

interface ConfigManagerProps {
  configs: string[];
  onConfigLoad: (config: ColorConfig) => void;
  onConfigsChange: () => void;
  showToast: (type: "success" | "error", text: string) => void;
}

function ConfigManager({
  configs,
  onConfigLoad,
  onConfigsChange,
  showToast,
}: ConfigManagerProps) {
  const handleLoad = async (name: string) => {
    try {
      const config = await invoke<ColorConfig>("load_config", { name });
      onConfigLoad(config);
      showToast("success", `已加载「${name}」`);
    } catch (err) {
      showToast("error", `加载失败: ${err}`);
    }
  };

  const handleDelete = async (e: React.MouseEvent, name: string) => {
    e.stopPropagation();
    try {
      await invoke("delete_config", { name });
      showToast("success", `已删除「${name}」`);
      onConfigsChange();
    } catch (err) {
      showToast("error", `删除失败: ${err}`);
    }
  };

  const getPresetIcon = (name: string) => {
    if (name.includes("电影") || name.toLowerCase().includes("theater")) return { icon: "movie", bg: "bg-secondary-container/30", color: "text-secondary" };
    if (name.includes("游戏") || name.toLowerCase().includes("gaming")) return { icon: "sports_esports", bg: "bg-tertiary-container/30", color: "text-tertiary" };
    if (name.includes("护眼") || name.toLowerCase().includes("reading")) return { icon: "edit_note", bg: "bg-error-container/30", color: "text-error" };
    return { icon: "tune", bg: "bg-surface-variant/30", color: "text-on-surface-variant" };
  };

  return (
    <div data-components="ConfigManager" className="flex-1 flex flex-col min-h-0">
      <div className="mb-md">
        <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface">快速方案</h3>
      </div>

      <div data-name="config-list" className="flex-1 bg-surface-container rounded-xl border border-outline-variant/20 p-md overflow-y-auto">
        {configs.length === 0 ? (
          <div className="text-center py-8 text-body-md text-on-surface-variant">
            暂无保存的配置
          </div>
        ) : (
          <div className="space-y-sm">
            {configs.map((name) => {
              const preset = getPresetIcon(name);
              return (
                <div
                  key={name}
                  onClick={() => handleLoad(name)}
                  className="flex items-center justify-between p-sm hover:bg-surface-variant/30 rounded-lg cursor-pointer transition-colors group"
                >
                  <div className="flex items-center gap-md">
                    <div className={`w-8 h-8 rounded-md ${preset.bg} flex items-center justify-center`}>
                      <span className={`material-symbols-outlined ${preset.color} text-[18px] leading-none relative top-[1px]`}>{preset.icon}</span>
                    </div>
                    <span className="font-label-md text-label-md">{name}</span>
                  </div>
                  <button
                    onClick={(e) => handleDelete(e, name)}
                    className="p-xs rounded opacity-0 group-hover:opacity-100 hover:bg-error-container/30 transition-all flex items-center"
                    title="删除"
                  >
                    <span className="material-symbols-outlined text-[18px] text-error">delete</span>
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}

export default ConfigManager;
