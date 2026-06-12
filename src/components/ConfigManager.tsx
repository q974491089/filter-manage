import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";
import ShortcutInput from "./ShortcutInput";
import Toggle from "./Toggle";

interface ColorConfig {
  name: string;
  icon?: string;
  brightness: number;
  contrast: number;
  gamma: number;
  digital_vibrance: number;
  icc_profile: string | null;
}

interface ProcessRule {
  id: string;
  process_name: string;
  config_name: string;
  enabled: boolean;
  restore_on_exit: boolean;
}

interface ShortcutBinding {
  shortcut: string;
  config_name: string;
}

interface AppSettings {
  close_to_tray: boolean | null;
  close_prompted: boolean;
  autostart: boolean;
  shortcut_notification: boolean;
  tray_presets: string[];
  shortcuts: ShortcutBinding[];
  process_watcher_enabled: boolean;
  process_notification: boolean;
  process_rules: ProcessRule[];
}

interface ConfigManagerProps {
  configs: ColorConfig[];
  selectedConfig: string;
  onConfigLoad: (config: ColorConfig) => void;
  onConfigsChange: () => void;
  showToast: (type: "success" | "error", text: string) => void;
}

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

function ConfigManager({
  configs,
  selectedConfig,
  onConfigLoad,
  onConfigsChange,
  showToast,
}: ConfigManagerProps) {
  const [showEditModal, setShowEditModal] = useState(false);
  const [editingConfig, setEditingConfig] = useState<string | null>(null);
  const [editName, setEditName] = useState("");
  const [editIcon, setEditIcon] = useState("tune");
  const [editCustomIconUrl, setEditCustomIconUrl] = useState("");
  const [editUseCustomIcon, setEditUseCustomIcon] = useState(false);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [newProcessName, setNewProcessName] = useState("");
  const [processes, setProcesses] = useState<{ name: string; pid: number; icon: string | null }[]>([]);
  const [showProcessPicker, setShowProcessPicker] = useState(false);
  const [processSearchQuery, setProcessSearchQuery] = useState("");

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

  const handleEdit = async (e: React.MouseEvent, name: string) => {
    e.stopPropagation();
    setEditingConfig(name);
    setEditName(name);
    setShowEditModal(true);
    
    // 从列表中获取配置的 icon
    const config = configs.find(c => c.name === name);
    if (config?.icon && config.icon.startsWith("http")) {
      setEditIcon("auto_awesome");
      setEditCustomIconUrl(config.icon);
      setEditUseCustomIcon(true);
    } else {
      setEditIcon(config?.icon || getDefaultIcon(name));
      setEditCustomIconUrl("");
      setEditUseCustomIcon(false);
    }
    
    await loadSettings();
  };

  const loadSettings = async () => {
    try {
      const result = await invoke<AppSettings>("get_app_settings");
      setSettings(result);
    } catch (err) {
      console.error("Failed to load settings:", err);
      showToast("error", "加载设置失败");
    }
  };

  const loadRunningProcesses = async () => {
    try {
      const list = await invoke<{ name: string; pid: number; icon: string | null }[]>("get_running_processes");
      const unique = [...new Map(list.map(p => [p.name.toLowerCase(), p])).values()]
        .sort((a, b) => a.name.localeCompare(b.name));
      setProcesses(unique);
    } catch (err) {
      console.error("Failed to load running processes:", err);
    }
  };

  const handleSaveEdit = async () => {
    if (!settings || !editingConfig) return;

    try {
      // 如果名称改变了，需要重命名配置
      if (editName !== editingConfig) {
        await invoke("rename_config", { oldName: editingConfig, newName: editName });
      }

      // 保存 icon 到配置
      const icon = editUseCustomIcon && editCustomIconUrl ? editCustomIconUrl : editIcon;
      const config = await invoke<ColorConfig>("load_config", { name: editName });
      await invoke("save_config", {
        config: {
          ...config,
          icon,
        },
      });

      // 保存设置
      await invoke("save_app_settings", { settings });
      showToast("success", "设置已保存");
      setShowEditModal(false);
      onConfigsChange();
    } catch (err) {
      console.error("Failed to save settings:", err);
      showToast("error", `保存失败: ${err}`);
    }
  };

  const handleAddProcessRule = () => {
    if (!settings || !newProcessName.trim()) return;

    const newRule: ProcessRule = {
      id: crypto.randomUUID(),
      process_name: newProcessName.trim(),
      config_name: editingConfig || "",
      enabled: true,
      restore_on_exit: true,
    };

    setSettings({
      ...settings,
      process_rules: [...settings.process_rules, newRule],
    });
    setNewProcessName("");
    setShowProcessPicker(false);
  };

  const handleDeleteProcessRule = (id: string) => {
    if (!settings) return;
    setSettings({
      ...settings,
      process_rules: settings.process_rules.filter(r => r.id !== id),
    });
  };

  const handleToggleProcessRule = (id: string) => {
    if (!settings) return;
    setSettings({
      ...settings,
      process_rules: settings.process_rules.map(r =>
        r.id === id ? { ...r, enabled: !r.enabled } : r
      ),
    });
  };

  const handleUpdateProcessRule = (id: string, updates: Partial<ProcessRule>) => {
    if (!settings) return;
    setSettings({
      ...settings,
      process_rules: settings.process_rules.map(r =>
        r.id === id ? { ...r, ...updates } : r
      ),
    });
  };

  const handleShortcutChange = async (shortcut: string) => {
    if (!settings || !editingConfig) return;

    try {
      await invoke("bind_shortcut", { shortcut, configName: editingConfig });
      const newShortcuts = settings.shortcuts.filter(s => s.config_name !== editingConfig);
      newShortcuts.push({ shortcut, config_name: editingConfig });
      setSettings({ ...settings, shortcuts: newShortcuts });
      showToast("success", "快捷键已绑定");
    } catch (err) {
      console.error("Failed to bind shortcut:", err);
      showToast("error", `绑定失败: ${err}`);
    }
  };

  const handleShortcutClear = async () => {
    if (!settings || !editingConfig) return;

    try {
      await invoke("unbind_shortcut", { configName: editingConfig });
      const newShortcuts = settings.shortcuts.filter(s => s.config_name !== editingConfig);
      setSettings({ ...settings, shortcuts: newShortcuts });
      showToast("success", "快捷键已清除");
    } catch (err) {
      console.error("Failed to unbind shortcut:", err);
      showToast("error", "清除失败");
    }
  };

  const getConfigRules = () => {
    if (!settings || !editingConfig) return [];
    return settings.process_rules.filter(r => r.config_name === editingConfig);
  };

  const getConfigShortcut = () => {
    if (!settings || !editingConfig) return "";
    return settings.shortcuts.find(s => s.config_name === editingConfig)?.shortcut || "";
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
    <div data-components="ConfigManager" className="flex-[1.5] flex flex-col min-h-0 overflow-hidden">
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
            {configs.map((config) => {
              const name = config.name;
              const isActive = selectedConfig === name;
              return (
                <div
                  key={name}
                  onClick={() => handleLoad(name)}
                  className={`flex items-center justify-between p-sm rounded-lg cursor-pointer transition-all group border ${
                    isActive
                      ? "bg-primary/15 border-primary/40"
                      : "bg-primary/8 hover:bg-primary/15 border-primary/10 hover:border-primary/30"
                  }`}
                >
                  <div className="flex items-center gap-md">
                    {renderIcon(config)}
                    <span className="font-label-md text-label-md text-on-surface font-medium">{name}</span>
                    {isActive && (
                      <span className="flex items-center gap-1 px-2 py-0.5 bg-primary/20 text-primary rounded-full text-[10px] font-medium">
                        <span className="material-symbols-outlined text-[12px]">check_circle</span>
                        正在应用
                      </span>
                    )}
                  </div>
                  <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-all">
                    <button
                      onClick={(e) => handleEdit(e, name)}
                      className="p-xs rounded hover:bg-primary-container/30 transition-all flex items-center"
                      title="编辑"
                    >
                      <span className="material-symbols-outlined text-[18px] text-primary">edit</span>
                    </button>
                    <button
                      onClick={(e) => handleDelete(e, name)}
                      className="p-xs rounded hover:bg-error-container/30 transition-all flex items-center"
                      title="删除"
                    >
                      <span className="material-symbols-outlined text-[18px] text-error">delete</span>
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* 编辑弹窗 */}
      {showEditModal && settings && (
        <div className="fixed inset-0 z-[100] flex items-center justify-center">
          <div
            className="fixed inset-0 bg-black/60 backdrop-blur-md"
            onClick={() => setShowEditModal(false)}
          />
          <div className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-[110] w-[600px] max-h-[800px] rounded-xl shadow-2xl overflow-hidden bg-surface-container/80 backdrop-blur-xl border border-outline-variant/20">
            {/* Header */}
            <div className="flex justify-between items-center px-6 pt-6 pb-4 border-b border-outline-variant/20">
              <h3 className="font-headline-sm text-headline-sm font-bold">编辑方案</h3>
              <button
                onClick={() => setShowEditModal(false)}
                className="w-8 h-8 flex items-center justify-center rounded-full text-on-surface-variant hover:text-on-surface transition-colors duration-200 hover:bg-surface-variant/40 active:scale-90"
              >
                <span className="material-symbols-outlined text-[20px]">close</span>
              </button>
            </div>

            {/* Content */}
            <div className="p-6 space-y-6 max-h-[600px] overflow-y-auto">
              {/* 方案名称 */}
              <div className="space-y-2">
                <label className="font-label-md text-label-md text-on-surface">方案名称</label>
                <input
                  type="text"
                  value={editName}
                  onChange={(e) => setEditName(e.target.value)}
                  className="w-full bg-surface-container-highest/60 border border-outline-variant/40 text-on-surface font-body-md text-body-md rounded-lg px-3 py-2 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all duration-200"
                />
              </div>

              {/* Divider */}
              <div className="h-px bg-outline-variant/30 w-full" />

              {/* 图标选择 */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <span className="material-symbols-outlined text-primary text-[18px]">palette</span>
                  <h4 className="font-title-sm text-title-sm">图标</h4>
                </div>
                <div className="flex flex-wrap gap-sm">
                  {PRESET_ICONS.map((item) => (
                    <button
                      key={item.icon}
                      onClick={() => { setEditIcon(item.icon); setEditUseCustomIcon(false); }}
                      className={`w-10 h-10 rounded-lg flex items-center justify-center transition-colors ${
                        !editUseCustomIcon && editIcon === item.icon
                          ? "bg-primary/20 text-primary"
                          : "bg-surface-container hover:bg-surface-variant text-on-surface-variant"
                      }`}
                      title={item.label}
                    >
                      <span className="material-symbols-outlined text-[20px]">{item.icon}</span>
                    </button>
                  ))}
                </div>
                <div className="space-y-2">
                  <label className="font-label-sm text-label-sm text-on-surface-variant">或输入自定义图标 URL</label>
                  <input
                    type="text"
                    value={editCustomIconUrl}
                    onChange={(e) => { setEditCustomIconUrl(e.target.value); setEditUseCustomIcon(true); }}
                    placeholder="https://..."
                    className="w-full bg-surface-container-highest/60 border border-outline-variant/40 text-on-surface font-body-sm text-body-sm rounded-lg px-3 py-2 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all duration-200"
                  />
                  {editCustomIconUrl && editUseCustomIcon && (
                    <div className="flex items-center gap-sm">
                      <img src={editCustomIconUrl} alt="" className="w-8 h-8 rounded object-cover" onError={(e) => (e.currentTarget.style.display = "none")} />
                      <span className="text-label-sm text-on-surface-variant">预览</span>
                    </div>
                  )}
                </div>
              </div>

              {/* Divider */}
              <div className="h-px bg-outline-variant/30 w-full" />

              {/* 快捷键设置 */}
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <span className="material-symbols-outlined text-primary text-[18px]">keyboard</span>
                  <h4 className="font-title-sm text-title-sm">快捷键</h4>
                </div>
                <div className="flex items-center justify-between p-4 bg-surface-container-high/60 rounded-xl border border-outline-variant/20">
                  <div className="space-y-1">
                    <span className="font-body-md text-body-md">全局快捷键</span>
                    <p className="font-label-sm text-label-sm text-on-surface-variant">
                      按下快捷键可快速切换到此方案
                    </p>
                  </div>
                  <div className="w-48">
                    <ShortcutInput
                      value={getConfigShortcut()}
                      onChange={handleShortcutChange}
                      onClear={handleShortcutClear}
                    />
                  </div>
                </div>
              </div>

              {/* Divider */}
              <div className="h-px bg-outline-variant/30 w-full" />

              {/* 监听进程设置 */}
              <div className="space-y-3">
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-primary text-[18px]">terminal</span>
                    <h4 className="font-title-sm text-title-sm">监听进程</h4>
                  </div>
                  <button
                    onClick={() => {
                      setShowProcessPicker(true);
                      setProcessSearchQuery("");
                      loadRunningProcesses();
                    }}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-primary/15 text-primary rounded-lg hover:bg-primary/25 transition-colors duration-200 active:scale-95"
                  >
                    <span className="material-symbols-outlined text-[16px]">add</span>
                    <span className="font-label-md text-label-md">添加进程</span>
                  </button>
                </div>

                <p className="font-label-sm text-label-sm text-on-surface-variant">
                  当指定进程运行时，自动切换到此方案
                </p>

                {/* 进程规则列表 */}
                {getConfigRules().length === 0 ? (
                  <div className="text-center py-6 text-on-surface-variant/40">
                    <span className="material-symbols-outlined text-[32px] mb-2">terminal</span>
                    <p className="font-body-sm">暂无监听进程</p>
                  </div>
                ) : (
                  <div className="space-y-2">
                    {getConfigRules().map((rule) => (
                      <div
                        key={rule.id}
                        className={`flex items-center gap-3 p-3 rounded-lg border transition-all duration-200 ${
                          rule.enabled
                            ? "bg-surface-container-high/60 border-outline-variant/20"
                            : "bg-surface-container-high/30 border-outline-variant/10 opacity-60"
                        }`}
                      >
                        {/* 启用开关 */}
                        <Toggle size="sm" checked={rule.enabled} onChange={() => handleToggleProcessRule(rule.id)} />

                        {/* 进程名 */}
                        <span className="font-mono text-sm flex-1 text-on-surface">
                          {rule.process_name}
                        </span>

                        {/* 恢复开关 */}
                        <div className="flex items-center gap-1.5">
                          <span className="font-label-sm text-label-sm text-on-surface-variant">恢复</span>
                          <Toggle size="sm" checked={rule.restore_on_exit} onChange={() => handleUpdateProcessRule(rule.id, { restore_on_exit: !rule.restore_on_exit })} />
                        </div>

                        {/* 删除按钮 */}
                        <button
                          onClick={() => handleDeleteProcessRule(rule.id)}
                          className="w-8 h-8 flex items-center justify-center rounded-full text-on-surface-variant hover:text-error hover:bg-error/10 transition-colors duration-200 active:scale-90"
                        >
                          <span className="material-symbols-outlined text-[18px]">delete</span>
                        </button>
                      </div>
                    ))}
                  </div>
                )}

                <p className="font-label-sm text-on-surface-variant/50">
                  规则按列表顺序匹配，第一个命中的生效。进程名不区分大小写。
                </p>
              </div>
            </div>

            {/* Footer */}
            <div className="flex justify-end gap-3 px-6 py-4 border-t border-outline-variant/20">
              <button
                onClick={() => setShowEditModal(false)}
                className="px-4 py-2 font-label-md text-label-md text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/40 rounded-lg transition-colors duration-200 active:scale-95"
              >
                取消
              </button>
              <button
                onClick={handleSaveEdit}
                className="px-4 py-2 font-label-md text-label-md bg-primary text-on-primary rounded-lg hover:bg-primary/90 transition-colors duration-200 active:scale-95"
              >
                保存
              </button>
            </div>
          </div>
        </div>
      )}

      {/* 进程选择器弹窗 */}
      {showProcessPicker && (
        <div className="fixed inset-0 z-[120] flex items-center justify-center">
          <div
            className="fixed inset-0 bg-black/60 backdrop-blur-md"
            onClick={() => setShowProcessPicker(false)}
          />
          <div className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-[130] w-[500px] max-h-[600px] rounded-xl shadow-2xl overflow-hidden bg-surface-container/80 backdrop-blur-xl border border-outline-variant/20">
            {/* Header */}
            <div className="flex justify-between items-center px-6 pt-6 pb-4 border-b border-outline-variant/20">
              <h3 className="font-headline-sm text-headline-sm font-bold">选择进程</h3>
              <button
                onClick={() => setShowProcessPicker(false)}
                className="w-8 h-8 flex items-center justify-center rounded-full text-on-surface-variant hover:text-on-surface transition-colors duration-200 hover:bg-surface-variant/40 active:scale-90"
              >
                <span className="material-symbols-outlined text-[20px]">close</span>
              </button>
            </div>

            {/* Content */}
            <div className="p-6 space-y-4">
              {/* 进程名输入 */}
              <div className="space-y-2">
                <label className="font-label-md text-label-md text-on-surface">进程名</label>
                <div className="flex gap-2">
                  <input
                    type="text"
                    value={newProcessName}
                    onChange={(e) => setNewProcessName(e.target.value)}
                    placeholder="例如: chrome.exe"
                    className="flex-1 bg-surface-container-highest/60 border border-outline-variant/40 text-on-surface font-body-md text-body-md rounded-lg px-3 py-2 placeholder:text-on-surface-variant/50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all duration-200"
                  />
                </div>
              </div>

              {/* 运行中进程列表 */}
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <label className="font-label-md text-label-md text-on-surface">运行中进程</label>
                  <button
                    onClick={loadRunningProcesses}
                    className="flex items-center gap-1 px-2 py-1 text-primary hover:bg-primary/10 rounded transition-colors duration-200"
                  >
                    <span className="material-symbols-outlined text-[16px]">refresh</span>
                    <span className="font-label-sm text-label-sm">刷新</span>
                  </button>
                </div>
                {/* 搜索框 */}
                <div className="relative">
                  <span className="material-symbols-outlined absolute left-2.5 top-1/2 -translate-y-1/2 text-on-surface-variant/50 text-[18px] pointer-events-none">search</span>
                  <input
                    type="text"
                    value={processSearchQuery}
                    onChange={(e) => setProcessSearchQuery(e.target.value)}
                    placeholder="搜索进程名..."
                    className="w-full bg-surface-container-highest/60 border border-outline-variant/40 text-on-surface font-body-md text-body-md rounded-lg pl-9 pr-3 py-2 placeholder:text-on-surface-variant/50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all duration-200"
                  />
                </div>
                <div className="max-h-48 overflow-y-auto space-y-1 bg-surface-container-highest/30 rounded-lg p-2">
                  {processes.length === 0 ? (
                    <div className="text-center py-4 text-on-surface-variant/50">
                      <span className="font-body-sm">点击"刷新"获取进程列表</span>
                    </div>
                  ) : (
                    (() => {
                      const filtered = processSearchQuery.trim()
                        ? processes.filter(p => p.name.toLowerCase().includes(processSearchQuery.trim().toLowerCase()))
                        : processes;
                      return filtered.length === 0 ? (
                        <div className="text-center py-4 text-on-surface-variant/50">
                          <span className="font-body-sm">未找到匹配的进程</span>
                        </div>
                      ) : (
                        filtered.map((p) => (
                          <button
                            key={p.name}
                            onClick={() => setNewProcessName(p.name)}
                            className={`w-full text-left px-3 py-2 rounded-lg transition-colors duration-200 active:scale-[0.98] flex items-center gap-2 ${
                              newProcessName === p.name
                                ? "bg-primary/15 text-primary"
                                : "hover:bg-surface-variant/40 text-on-surface"
                            }`}
                          >
                            {p.icon ? (
                              <img src={p.icon} alt="" className="w-5 h-5 rounded-sm object-contain" />
                            ) : (
                              <span className="material-symbols-outlined text-[18px] text-on-surface-variant/50">terminal</span>
                            )}
                            <span className="font-mono text-sm flex-1">{p.name}</span>
                            <span className="text-xs text-on-surface-variant">PID: {p.pid}</span>
                          </button>
                        ))
                      );
                    })()
                  )}
                </div>
              </div>
            </div>

            {/* Footer */}
            <div className="flex justify-end gap-3 px-6 py-4 border-t border-outline-variant/20">
              <button
                onClick={() => setShowProcessPicker(false)}
                className="px-4 py-2 font-label-md text-label-md text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/40 rounded-lg transition-colors duration-200 active:scale-95"
              >
                取消
              </button>
              <button
                onClick={handleAddProcessRule}
                className="px-4 py-2 font-label-md text-label-md bg-primary text-on-primary rounded-lg hover:bg-primary/90 transition-colors duration-200 active:scale-95"
              >
                添加
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default ConfigManager;
