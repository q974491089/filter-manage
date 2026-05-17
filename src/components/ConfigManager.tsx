import { invoke } from "@tauri-apps/api/core";

interface ColorConfig {
  name: string;
  icon?: string;
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

  const getDefaultIcon = (name: string) => {
    if (name.includes("电影") || name.toLowerCase().includes("theater")) return "movie";
    if (name.includes("游戏") || name.toLowerCase().includes("gaming")) return "sports_esports";
    if (name.includes("护眼") || name.toLowerCase().includes("reading")) return "edit_note";
    if (name.includes("摄影") || name.toLowerCase().includes("photo")) return "photo_camera";
    if (name.includes("设计") || name.toLowerCase().includes("design")) return "palette";
    if (name.includes("编程") || name.toLowerCase().includes("code")) return "code";
    return "tune";
  };

  const getIconBg = (icon: string | undefined, name: string) => {
    if (icon === "movie") return "bg-secondary-container";
    if (icon === "sports_esports") return "bg-tertiary-container";
    if (icon === "edit_note") return "bg-error-container";
    if (icon === "photo_camera" || icon === "palette") return "bg-primary-container";
    if (icon === "code") return "bg-tertiary-container";
    if (name.includes("电影")) return "bg-secondary-container";
    if (name.includes("游戏")) return "bg-tertiary-container";
    if (name.includes("护眼")) return "bg-error-container";
    return "bg-primary-container";
  };

  const renderIcon = (config: { name: string; icon?: string }) => {
    const icon = config.icon || getDefaultIcon(config.name);
    const bgClass = getIconBg(config.icon, config.name);

    if (config.icon && config.icon.startsWith("http")) {
      return (
        <div className={`w-9 h-9 rounded-md ${bgClass} flex items-center justify-center overflow-hidden shadow-sm`}>
          <img src={config.icon} alt="" className="w-full h-full object-cover" />
        </div>
      );
    }

    return (
      <div className={`w-9 h-9 rounded-md ${bgClass} flex items-center justify-center shadow-sm`}>
        <span className="material-symbols-outlined text-on-primary text-[18px] leading-none relative top-[1px]">{icon}</span>
      </div>
    );
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
              return (
                <div
                  key={name}
                  onClick={() => handleLoad(name)}
                  className="flex items-center justify-between p-sm bg-primary/8 hover:bg-primary/15 rounded-lg cursor-pointer transition-all group border border-primary/10 hover:border-primary/30"
                >
                  <div className="flex items-center gap-md">
                    {renderIcon({ name })}
                    <span className="font-label-md text-label-md text-on-surface font-medium">{name}</span>
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
