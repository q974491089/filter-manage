import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ShortcutInput from "./ShortcutInput";

interface ShortcutBinding {
  shortcut: string;
  config_name: string;
}

interface AppSettings {
  close_to_tray: boolean | null;  // null=未选择，true=最小化到托盘，false=直接关闭
  close_prompted: boolean;
  autostart: boolean;
  shortcut_notification: boolean;
  tray_presets: string[];
  shortcuts: ShortcutBinding[];
}

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
  configs: string[];
  showToast: (type: "success" | "error", text: string) => void;
  themeMode: "light" | "dark" | "system";
  onThemeModeChange: (mode: "light" | "dark" | "system") => void;
  onShowAbout: () => void;
}

type SettingsTab = "general" | "shortcuts" | "display";

const NAV_ITEMS: { id: SettingsTab; icon: string; label: string }[] = [
  { id: "general", icon: "settings", label: "常规设置" },
  { id: "shortcuts", icon: "keyboard", label: "快捷键" },
  { id: "display", icon: "monitor", label: "显示适配" },
];

function SettingsModal({ open, onClose, configs, showToast, themeMode, onThemeModeChange, onShowAbout }: SettingsModalProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [settings, setSettings] = useState<AppSettings>({
    close_to_tray: null,
    close_prompted: false,
    autostart: false,
    shortcut_notification: true,
    tray_presets: [],
    shortcuts: [],
  });

  useEffect(() => {
    if (open) loadSettings();
  }, [open]);

  const loadSettings = async () => {
    try {
      const result = await invoke<AppSettings>("get_app_settings");
      // 清理已删除的托盘方案名（脏数据）
      const validPresets = result.tray_presets.filter(name => configs.includes(name));
      if (validPresets.length !== result.tray_presets.length) {
        const cleaned = { ...result, tray_presets: validPresets };
        setSettings(cleaned);
        await invoke("save_app_settings", { settings: cleaned });
      } else {
        setSettings(result);
      }
    } catch (err) {
      console.error("Failed to load settings:", err);
    }
  };

  const saveSettings = async (newSettings: AppSettings) => {
    setSettings(newSettings);
    try {
      await invoke("save_app_settings", { settings: newSettings });
    } catch (err) {
      console.error("Failed to save settings:", err);
      showToast("error", "保存设置失败");
    }
  };

  const handleCloseToTrayChange = (value: string) => {
    // "" → null（未设置）, "true" → 最小化到托盘, "false" → 直接关闭
    const closeToTray = value === "" ? null : value === "true";
    saveSettings({
      ...settings,
      close_to_tray: closeToTray,
      close_prompted: closeToTray !== null,
    });
  };

  const handleToggleAutostart = async () => {
    const newValue = !settings.autostart;
    try {
      if (newValue) {
        await invoke("enable_autostart");
      } else {
        await invoke("disable_autostart");
      }
      saveSettings({ ...settings, autostart: newValue });
      showToast("success", newValue ? "已启用开机自启" : "已禁用开机自启");
    } catch (err) {
      console.error("Failed to toggle autostart:", err);
      showToast("error", "操作失败");
    }
  };

  const handleShortcutChange = async (configName: string, shortcut: string) => {
    try {
      await invoke("bind_shortcut", { shortcut, configName });
      const newShortcuts = settings.shortcuts.filter(s => s.config_name !== configName);
      newShortcuts.push({ shortcut, config_name: configName });
      saveSettings({ ...settings, shortcuts: newShortcuts });
      // 格式化快捷键显示
      const displayShortcut = shortcut.replace(/CommandOrControl/g, "Ctrl").replace(/Control/g, "Ctrl");
      showToast("success", `已绑定快捷键: ${displayShortcut}`);
    } catch (err) {
      console.error("Failed to bind shortcut:", err);
      // 处理 __default__ 的显示名称
      const errMsg = String(err)
        .replace(/__default__/g, "恢复默认设置")
        .replace(/CommandOrControl/g, "Ctrl")
        .replace(/Control/g, "Ctrl");
      showToast("error", `绑定失败: ${errMsg}`);
    }
  };

  const handleShortcutClear = async (configName: string) => {
    try {
      await invoke("unbind_shortcut", { configName });
      const newShortcuts = settings.shortcuts.filter(s => s.config_name !== configName);
      saveSettings({ ...settings, shortcuts: newShortcuts });
      showToast("success", "已清除快捷键");
    } catch (err) {
      console.error("Failed to unbind shortcut:", err);
      showToast("error", "清除失败");
    }
  };

  const handleToggleTrayPreset = async (configName: string) => {
    const presets = [...settings.tray_presets];
    const idx = presets.indexOf(configName);
    if (idx >= 0) {
      presets.splice(idx, 1);
    } else if (presets.length < 5) {
      presets.push(configName);
    } else {
      showToast("error", "最多选择 5 个托盘方案");
      return;
    }
    await saveSettings({ ...settings, tray_presets: presets });
    // 刷新托盘菜单
    try {
      await invoke("refresh_tray_menu");
    } catch (err) {
      console.error("Failed to refresh tray menu:", err);
    }
  };

  if (!open) return null;

  return (
    <div
      data-components="SettingsModal"
      className="fixed inset-0 z-[100] flex items-center justify-center"
    >
      {/* Backdrop */}
      <div
        className="fixed inset-0 bg-black/60 backdrop-blur-md transition-opacity duration-300"
        onClick={onClose}
      />

      {/* Modal Container - 900x650 glass panel */}
      <div
        data-name="settings-modal"
        className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-[60] flex w-[900px] h-[650px] rounded-xl shadow-2xl overflow-hidden animate-[modal-spring-up_0.5s_cubic-bezier(0.2,0.8,0.2,1)_forwards] bg-surface-container/80 backdrop-blur-xl border border-outline-variant/20"
      >
        {/* Sidebar */}
        <div className="w-64 bg-surface-container-low/50 border-r border-outline-variant/20 flex flex-col gap-2 p-6">
          {NAV_ITEMS.map((item) => (
            <button
              key={item.id}
              onClick={() => setActiveTab(item.id)}
              className={`flex items-center gap-3 p-3 rounded-lg transition-all duration-200 font-title-sm text-title-sm active:scale-95 ${
                activeTab === item.id
                  ? "bg-primary/15 text-primary"
                  : "text-on-surface-variant hover:bg-surface-variant/40 hover:text-on-surface"
              }`}
            >
              <span
                className="material-symbols-outlined"
                style={activeTab === item.id ? { fontVariationSettings: "'FILL' 1" } : undefined}
              >
                {item.icon}
              </span>
              <span>{item.label}</span>
            </button>
          ))}

          {/* 关于应用 - pushed to bottom */}
          <button
            onClick={onShowAbout}
            className="flex items-center gap-3 p-3 rounded-lg transition-all duration-200 font-title-sm text-title-sm active:scale-95 mt-auto text-on-surface-variant hover:bg-surface-variant/40 hover:text-on-surface"
          >
            <span className="material-symbols-outlined">info</span>
            <span>关于应用</span>
          </button>
        </div>

        {/* Content Area */}
        <div className="flex-1 flex flex-col bg-surface/20 overflow-hidden">
          {/* Fixed Header */}
          <div className="flex justify-between items-start px-12 pt-12 pb-6 shrink-0 border-b border-outline-variant/20">
            <div>
              <h2 className="font-headline-md text-headline-md font-bold mb-1">
                {activeTab === "general" && "常规设置"}
                {activeTab === "shortcuts" && "快捷键绑定"}
                {activeTab === "display" && "显示适配"}
              </h2>
              <p className="font-body-md text-body-md text-on-surface-variant">
                {activeTab === "general" && "管理 Filter Manage 的核心运行行为"}
                {activeTab === "shortcuts" && "为每个颜色方案绑定全局快捷键"}
                {activeTab === "display" && "选择在系统托盘菜单中展示的方案"}
              </p>
            </div>
            <button
              onClick={onClose}
              className="w-8 h-8 flex items-center justify-center rounded-full text-on-surface-variant hover:text-on-surface transition-colors duration-200 hover:bg-surface-variant/40 active:scale-90"
            >
              <span className="material-symbols-outlined text-[20px]">close</span>
            </button>
          </div>

          {/* Scrollable Content */}
          <div className="flex-1 overflow-y-auto px-12 pt-4 pb-6 custom-scrollbar">
            <div className="animate-[slide-fade-up_0.3s_cubic-bezier(0.16,1,0.3,1)_forwards]">

            {/* ===== 常规设置 ===== */}
            {activeTab === "general" && (
              <div className="space-y-6">
                {/* 界面外观 - Theme Switcher */}
                <div className="space-y-4">
                  <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-primary text-[18px]">palette</span>
                    <h4 className="font-title-sm text-title-sm">界面外观</h4>
                  </div>
                  <div className="grid grid-cols-3 gap-3">
                    <button
                      onClick={() => onThemeModeChange("light")}
                      className={`flex flex-col items-center justify-center gap-2 p-4 rounded-xl border transition-colors focus:outline-none ${
                        themeMode === "light"
                          ? "border-2 border-primary bg-primary/10"
                          : "border-outline-variant/30 bg-surface-container-high hover:bg-surface-bright"
                      }`}
                    >
                      <span className={`material-symbols-outlined text-[24px] ${themeMode === "light" ? "text-primary" : "text-on-surface-variant"}`}>
                        light_mode
                      </span>
                      <span className={`font-label-sm text-label-sm ${themeMode === "light" ? "text-primary font-bold" : "text-on-surface-variant"}`}>
                        浅色模式
                      </span>
                    </button>
                    <button
                      onClick={() => onThemeModeChange("dark")}
                      className={`flex flex-col items-center justify-center gap-2 p-4 rounded-xl border transition-colors focus:outline-none ${
                        themeMode === "dark"
                          ? "border-2 border-primary bg-primary/10"
                          : "border-outline-variant/30 bg-surface-container-high hover:bg-surface-bright"
                      }`}
                    >
                      <span className={`material-symbols-outlined text-[24px] ${themeMode === "dark" ? "text-primary" : "text-on-surface-variant"}`}>
                        dark_mode
                      </span>
                      <span className={`font-label-sm text-label-sm ${themeMode === "dark" ? "text-primary font-bold" : "text-on-surface-variant"}`}>
                        深色模式
                      </span>
                    </button>
                    <button
                      onClick={() => onThemeModeChange("system")}
                      className={`flex flex-col items-center justify-center gap-2 p-4 rounded-xl border transition-colors focus:outline-none ${
                        themeMode === "system"
                          ? "border-2 border-primary bg-primary/10"
                          : "border-outline-variant/30 bg-surface-container-high hover:bg-surface-bright"
                      }`}
                    >
                      <span className={`material-symbols-outlined text-[24px] ${themeMode === "system" ? "text-primary" : "text-on-surface-variant"}`}>
                        settings_brightness
                      </span>
                      <span className={`font-label-sm text-label-sm ${themeMode === "system" ? "text-primary font-bold" : "text-on-surface-variant"}`}>
                        跟随系统
                      </span>
                    </button>
                  </div>
                </div>

                {/* Divider */}
                <div className="h-px bg-outline-variant/30 w-full" />

                {/* 关闭行为下拉框 */}
                <div className="flex items-center justify-between">
                  <div className="space-y-1 text-left">
                    <h4 className="font-title-sm text-title-sm">
                      关闭行为
                    </h4>
                    <p className="font-label-sm text-label-sm text-on-surface-variant">
                      {settings.close_to_tray === null
                        ? "首次关闭时将弹窗询问"
                        : settings.close_to_tray
                          ? "点击关闭按钮后应用将继续在后台运行"
                          : "关闭后将直接退出应用"}
                    </p>
                  </div>
                  <div className="relative">
                    <select
                      value={settings.close_to_tray === null ? "" : String(settings.close_to_tray)}
                      onChange={(e) => handleCloseToTrayChange(e.target.value)}
                      className="appearance-none bg-surface-container-highest/60 border border-outline-variant/40 text-on-surface font-label-md text-label-md rounded-lg px-3 py-1.5 pr-8 cursor-pointer hover:border-primary/50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all duration-200"
                    >
                      <option value="">未设置</option>
                      <option value="true">系统托盘</option>
                      <option value="false">直接关闭</option>
                    </select>
                    <span className="material-symbols-outlined absolute right-1.5 top-1/2 -translate-y-1/2 text-on-surface-variant text-[18px] pointer-events-none">
                      expand_more
                    </span>
                  </div>
                </div>

                <button
                  onClick={handleToggleAutostart}
                  className="w-full flex items-center justify-between group cursor-pointer"
                >
                  <div className="space-y-1 text-left">
                    <h4 className="font-title-sm text-title-sm group-hover:text-primary transition-colors duration-200">
                      开机时自动启动
                    </h4>
                    <p className="font-label-sm text-label-sm text-on-surface-variant">
                      在 Windows 启动时自动运行 Filter Manage
                    </p>
                  </div>
                  <div className="relative inline-flex items-center active:scale-95 transition-transform duration-200 hover:scale-[1.02]">
                    <div className={`w-11 h-6 rounded-full transition-colors duration-300 ${settings.autostart ? "bg-primary" : "bg-surface-container-highest"}`}>
                      <div className={`absolute top-[2px] left-[2px] w-5 h-5 bg-white rounded-full shadow transition-transform duration-300 ${settings.autostart ? "translate-x-full" : ""}`} />
                    </div>
                  </div>
                </button>

                {/* Divider */}
                <div className="h-px bg-outline-variant/30 w-full" />

                {/* 通知 */}
                <div className="space-y-5">
                  <h4 className="font-title-sm text-title-sm">通知</h4>
                  <div className="px-3 py-5 bg-surface-container-high/60 rounded-xl border border-outline-variant/20 transition-all duration-300 hover:bg-surface-container-high/80">
                    <button
                      onClick={() => saveSettings({ ...settings, shortcut_notification: !settings.shortcut_notification })}
                      className="w-full flex items-center gap-3 hover:bg-white/5 p-2 rounded-lg transition-colors duration-200 cursor-pointer"
                    >
                      <span className="material-symbols-outlined text-primary">notifications</span>
                      <span className="font-body-md text-body-md">切换方案时显示通知</span>
                      <div className="ml-auto relative inline-flex items-center active:scale-95 transition-transform duration-200">
                        <div className={`w-11 h-6 rounded-full transition-colors duration-300 ${settings.shortcut_notification ? "bg-primary" : "bg-surface-container-highest"}`}>
                          <div className={`absolute top-[2px] left-[2px] w-5 h-5 bg-white rounded-full shadow transition-transform duration-300 ${settings.shortcut_notification ? "translate-x-full" : ""}`} />
                        </div>
                      </div>
                    </button>
                  </div>
                </div>

              </div>
            )}

            {/* ===== 快捷键 ===== */}
            {activeTab === "shortcuts" && (
              <div className="space-y-6">
                {/* 恢复默认快捷键 */}
                <div className="space-y-3">
                  <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-primary text-[18px]">restart_alt</span>
                    <h4 className="font-title-sm text-title-sm">系统快捷键</h4>
                  </div>
                  <div className="flex items-center justify-between p-5 bg-surface-container-high/60 rounded-xl border border-outline-variant/20 hover:bg-surface-container-high/80 transition-all duration-300">
                    <div className="space-y-1">
                      <span className="font-title-sm text-title-sm">恢复默认设置</span>
                      <p className="font-label-sm text-on-surface-variant">一键恢复所有设置到初始状态</p>
                    </div>
                    <div className="w-52">
                      <ShortcutInput
                        value={settings.shortcuts.find(s => s.config_name === "__default__")?.shortcut || ""}
                        onChange={(shortcut) => handleShortcutChange("__default__", shortcut)}
                        onClear={() => handleShortcutClear("__default__")}
                      />
                    </div>
                  </div>
                </div>

                {/* Divider */}
                <div className="h-px bg-outline-variant/30 w-full" />

                {/* 方案快捷键 */}
                <div className="space-y-3">
                  <div className="flex items-center gap-2">
                    <span className="material-symbols-outlined text-primary text-[18px]">palette</span>
                    <h4 className="font-title-sm text-title-sm">方案快捷键</h4>
                  </div>
                  {configs.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-16 text-on-surface-variant/40">
                      <span className="material-symbols-outlined text-[48px] mb-4">keyboard</span>
                      <p className="font-body-md">暂无方案</p>
                      <p className="font-body-sm mt-1">请先在主界面保存颜色方案</p>
                    </div>
                  ) : (
                    configs.map((configName) => {
                      const binding = settings.shortcuts.find(s => s.config_name === configName);
                      return (
                        <div
                          key={configName}
                          className="flex items-center justify-between p-5 bg-surface-container-high/60 rounded-xl border border-outline-variant/20 hover:bg-surface-container-high/80 transition-all duration-300"
                        >
                          <span className="font-title-sm text-title-sm">{configName}</span>
                          <div className="w-52">
                            <ShortcutInput
                              value={binding?.shortcut || ""}
                              onChange={(shortcut) => handleShortcutChange(configName, shortcut)}
                              onClear={() => handleShortcutClear(configName)}
                            />
                          </div>
                        </div>
                      );
                    })
                  )}
                </div>
              </div>
            )}

            {/* ===== 显示适配 ===== */}
            {activeTab === "display" && (
              <div className="space-y-6">
                <div className="flex items-center justify-between">
                  <span className="font-body-md text-on-surface-variant">已选方案</span>
                  <span className="font-label-sm text-primary">{settings.tray_presets.length}/5</span>
                </div>

                {configs.length === 0 ? (
                  <div className="flex flex-col items-center justify-center py-16 text-on-surface-variant/40">
                    <span className="material-symbols-outlined text-[48px] mb-4">monitor</span>
                    <p className="font-body-md">暂无方案</p>
                    <p className="font-body-sm mt-1">请先在主界面保存颜色方案</p>
                  </div>
                ) : (
                  <>
                    <div className="space-y-3">
                      {configs.map((configName) => {
                        const isSelected = settings.tray_presets.includes(configName);
                        return (
                          <div
                            key={configName}
                            onClick={() => handleToggleTrayPreset(configName)}
                            className={`flex items-center gap-3 p-4 rounded-xl cursor-pointer transition-all duration-200 active:scale-[0.98] ${
                              isSelected
                                ? "bg-primary/15 border border-primary/30"
                                : "bg-surface-container-high/60 border border-outline-variant/20 hover:bg-surface-container-high/80"
                            }`}
                          >
                            <div
                              className={`w-5 h-5 rounded border-2 flex items-center justify-center transition-colors duration-200 ${
                                isSelected ? "bg-primary border-primary" : "border-outline-variant"
                              }`}
                            >
                              {isSelected && (
                                <span className="material-symbols-outlined text-[14px] text-on-primary">
                                  check
                                </span>
                              )}
                            </div>
                            <span className="font-title-sm text-title-sm">{configName}</span>
                          </div>
                        );
                      })}
                    </div>
                    <p className="font-label-sm text-on-surface-variant/50 mt-2">
                      选择在系统托盘菜单中展示的方案（最多5个）。不选择则默认展示前5个。
                    </p>
                  </>
                )}
              </div>
            )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

export default SettingsModal;
