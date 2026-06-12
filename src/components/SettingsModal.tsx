import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ShortcutInput from "./ShortcutInput";
import Toggle from "./Toggle";
import TextSwitch from "./TextSwitch";

interface ColorConfig {
  name: string;
  icon?: string;
  brightness: number;
  contrast: number;
  gamma: number;
  digital_vibrance: number;
  icc_profile: string | null;
}

interface ShortcutBinding {
  shortcut: string;
  config_name: string;
}

interface ProcessRule {
  id: string;                    // UUID，前端生成（crypto.randomUUID()）
  process_name: string;          // 进程名，如 "delta_force.exe"（不区分大小写）
  config_name: string;           // 绑定的预设名
  enabled: boolean;              // 是否启用
  restore_on_exit: boolean;      // 进程退出时是否恢复上一方案（默认 true）
}

interface RunningProcess {
  name: string;     // 进程名，如 "chrome.exe"
  pid: number;      // 进程 ID
  icon: string | null;  // 进程图标（base64 data URL）
}

interface AppSettings {
  close_to_tray: boolean | null;  // null=未选择，true=最小化到托盘，false=直接关闭
  close_prompted: boolean;
  autostart: boolean;
  shortcut_notification: boolean;
  tray_presets: string[];
  shortcuts: ShortcutBinding[];
  process_watcher_enabled: boolean;   // 进程监听总开关（默认 true）
  process_notification: boolean;      // 自动切换时是否弹 Toast（默认 true）
  process_rules: ProcessRule[];       // 规则列表
}

interface SettingsModalProps {
  open: boolean;
  onClose: () => void;
  configs: ColorConfig[];
  showToast: (type: "success" | "error", text: string) => void;
  themeMode: "light" | "dark" | "system";
  onThemeModeChange: (mode: "light" | "dark" | "system") => void;
  onShowAbout: () => void;
}

type SettingsTab = "general" | "shortcuts" | "display" | "process";

const NAV_ITEMS: { id: SettingsTab; icon: string; label: string }[] = [
  { id: "general", icon: "settings", label: "常规设置" },
  { id: "shortcuts", icon: "keyboard", label: "快捷键" },
  { id: "display", icon: "monitor", label: "显示适配" },
  { id: "process", icon: "terminal", label: "进程监听" },
];

const PRESET_ICONS: Record<string, string> = {
  movie: "bg-secondary-container",
  sports_esports: "bg-tertiary-container",
  edit_note: "bg-error-container",
  photo_camera: "bg-primary-container",
  palette: "bg-primary-container",
  code: "bg-tertiary-container",
  music_note: "bg-secondary-container",
  visibility: "bg-primary-container",
  auto_awesome: "bg-primary-container",
  tune: "bg-primary-container",
};

function getDefaultIcon(name: string): string {
  if (name.includes("电影") || name.toLowerCase().includes("theater")) return "movie";
  if (name.includes("游戏") || name.toLowerCase().includes("gaming")) return "sports_esports";
  if (name.includes("护眼") || name.toLowerCase().includes("reading")) return "edit_note";
  if (name.includes("摄影") || name.toLowerCase().includes("photo")) return "photo_camera";
  if (name.includes("设计") || name.toLowerCase().includes("design")) return "palette";
  if (name.includes("编程") || name.toLowerCase().includes("code")) return "code";
  return "tune";
}

function getIconBg(icon: string | undefined, name: string): string {
  if (icon && PRESET_ICONS[icon]) return PRESET_ICONS[icon];
  const fallback = getDefaultIcon(name);
  return PRESET_ICONS[fallback] || "bg-primary-container";
}

function renderConfigIcon(config: { name: string; icon?: string }) {
  const icon = config.icon || getDefaultIcon(config.name);
  const bgClass = getIconBg(config.icon, config.name);

  if (config.icon && config.icon.startsWith("http")) {
    return (
      <div className={`w-8 h-8 rounded-md ${bgClass} flex items-center justify-center overflow-hidden shadow-sm`}>
        <img src={config.icon} alt="" className="w-full h-full object-cover" />
      </div>
    );
  }

  return (
    <div className={`w-8 h-8 rounded-md ${bgClass} flex items-center justify-center shadow-sm`}>
      <span className="material-symbols-outlined text-on-primary text-[16px] leading-none relative top-[px]">{icon}</span>
    </div>
  );
}

function SettingsModal({ open, onClose, configs, showToast, themeMode, onThemeModeChange, onShowAbout }: SettingsModalProps) {
  const [activeTab, setActiveTab] = useState<SettingsTab>("general");
  const [settings, setSettings] = useState<AppSettings>({
    close_to_tray: null,
    close_prompted: false,
    autostart: false,
    shortcut_notification: true,
    tray_presets: [],
    shortcuts: [],
    process_watcher_enabled: true,
    process_notification: true,
    process_rules: [],
  });
  const [processes, setProcesses] = useState<RunningProcess[]>([]);
  const [showProcessPicker, setShowProcessPicker] = useState(false);
  const [newRuleProcessName, setNewRuleProcessName] = useState("");
  const [newRuleConfigName, setNewRuleConfigName] = useState("");
  const [processSearchQuery, setProcessSearchQuery] = useState("");

  useEffect(() => {
    if (open) loadSettings();
  }, [open]);

  const loadSettings = async () => {
    try {
      const result = await invoke<AppSettings>("get_app_settings");
      // 清理已删除的托盘方案名（脏数据）
      const validPresets = result.tray_presets.filter(name => configs.some(c => c.name === name));
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

  // 进程监听相关函数
  const loadRunningProcesses = async () => {
    try {
      const list = await invoke<RunningProcess[]>("get_running_processes");
      // 去重并按名称排序
      const unique = [...new Map(list.map(p => [p.name.toLowerCase(), p])).values()]
        .sort((a, b) => a.name.localeCompare(b.name));
      setProcesses(unique);
    } catch (err) {
      console.error("Failed to load running processes:", err);
      showToast("error", "获取进程列表失败");
    }
  };

  const handleToggleProcessWatcher = async () => {
    const newValue = !settings.process_watcher_enabled;
    try {
      await invoke("set_process_watcher_enabled", { enabled: newValue });
      saveSettings({ ...settings, process_watcher_enabled: newValue });
      showToast("success", newValue ? "已启用进程监听" : "已禁用进程监听");
    } catch (err) {
      console.error("Failed to toggle process watcher:", err);
      showToast("error", "操作失败");
    }
  };

  const handleAddProcessRule = async () => {
    if (!newRuleProcessName.trim()) {
      showToast("error", "请输入进程名");
      return;
    }
    if (!newRuleConfigName) {
      showToast("error", "请选择配置方案");
      return;
    }

    try {
      await invoke("add_process_rule", {
        rule: {
          id: crypto.randomUUID(),
          process_name: newRuleProcessName.trim(),
          config_name: newRuleConfigName,
          enabled: true,
          restore_on_exit: true,
        }
      });
      // 重新加载设置以获取最新规则
      await loadSettings();
      setNewRuleProcessName("");
      setNewRuleConfigName("");
      setShowProcessPicker(false);
      showToast("success", "规则添加成功");
    } catch (err) {
      console.error("Failed to add process rule:", err);
      const errorMsg = String(err);
      if (errorMsg.includes("进程名不能为空")) {
        showToast("error", "请输入进程名");
      } else if (errorMsg.includes("Config")) {
        showToast("error", "选择的配置方案不存在");
      } else if (errorMsg.includes("已存在规则")) {
        showToast("error", "该进程已有规则，请勿重复添加");
      } else {
        showToast("error", `添加失败: ${errorMsg}`);
      }
    }
  };

  const handleDeleteProcessRule = async (id: string) => {
    try {
      await invoke("delete_process_rule", { id });
      await loadSettings();
      showToast("success", "规则已删除");
    } catch (err) {
      console.error("Failed to delete process rule:", err);
      showToast("error", "删除失败");
    }
  };

  const handleToggleProcessRule = async (rule: ProcessRule) => {
    try {
      await invoke("update_process_rule", {
        rule: { ...rule, enabled: !rule.enabled }
      });
      await loadSettings();
    } catch (err) {
      console.error("Failed to toggle process rule:", err);
      showToast("error", "更新失败");
    }
  };

  const handleUpdateProcessRule = async (rule: ProcessRule) => {
    try {
      await invoke("update_process_rule", { rule });
      await loadSettings();
    } catch (err) {
      console.error("Failed to update process rule:", err);
      showToast("error", "更新失败");
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
        className="fixed top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-[110] flex w-[900px] h-[650px] rounded-xl shadow-2xl overflow-hidden animate-[modal-spring-up_0.5s_cubic-bezier(0.2,0.8,0.2,1)_forwards] bg-surface-container/80 backdrop-blur-xl border border-outline-variant/20"
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
                {activeTab === "process" && "进程监听"}
              </h2>
              <p className="font-body-md text-body-md text-on-surface-variant">
                {activeTab === "general" && "管理 Filter Manage 的核心运行行为"}
                {activeTab === "shortcuts" && "为每个颜色方案绑定全局快捷键"}
                {activeTab === "display" && "选择在系统托盘菜单中展示的方案"}
                {activeTab === "process" && "根据运行中的进程自动切换配色方案"}
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
                  <Toggle checked={settings.autostart} onChange={handleToggleAutostart} />
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
                      <div className="ml-auto">
                        <Toggle checked={settings.shortcut_notification} onChange={(v) => saveSettings({ ...settings, shortcut_notification: v })} />
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
                    configs.map((config) => {
                      const configName = config.name;
                      const binding = settings.shortcuts.find(s => s.config_name === configName);
                      return (
                        <div
                          key={configName}
                          className="flex items-center justify-between p-5 bg-surface-container-high/60 rounded-xl border border-outline-variant/20 hover:bg-surface-container-high/80 transition-all duration-300"
                        >
                          <div className="flex items-center gap-3">
                            {renderConfigIcon(config)}
                            <span className="font-title-sm text-title-sm">{configName}</span>
                          </div>
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
                      {configs.map((config) => {
                        const configName = config.name;
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
                            {renderConfigIcon(config)}
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

            {/* ===== 进程监听 ===== */}
            {activeTab === "process" && (
              <div className="space-y-6">
                {/* 总开关 */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <span className="material-symbols-outlined text-primary text-[18px]">monitoring</span>
                    <div className="space-y-0.5">
                      <h4 className="font-title-sm text-title-sm">进程监听</h4>
                      <p className="font-label-sm text-label-sm text-on-surface-variant">
                        当指定进程运行时自动切换配色方案
                      </p>
                    </div>
                  </div>
                  <Toggle checked={settings.process_watcher_enabled} onChange={handleToggleProcessWatcher} />
                </div>

                {/* 通知开关 */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <span className="material-symbols-outlined text-primary text-[18px]">notifications</span>
                    <div className="space-y-0.5">
                      <h4 className="font-title-sm text-title-sm">切换通知</h4>
                      <p className="font-label-sm text-label-sm text-on-surface-variant">
                        自动切换配色方案时弹出 Toast 提示
                      </p>
                    </div>
                  </div>
                  <Toggle checked={settings.process_notification} onChange={(v) => saveSettings({ ...settings, process_notification: v })} />
                </div>

                {/* Divider */}
                <div className="h-px bg-outline-variant/30 w-full" />

                {/* 规则列表区域 — 跟随总开关状态 */}
                <div className={`space-y-3 transition-opacity duration-300 ${settings.process_watcher_enabled ? "opacity-100" : "opacity-40 pointer-events-none"}`}>
                  <div className="flex items-center justify-between">
                    <div className="flex items-center gap-2">
                      <span className="material-symbols-outlined text-primary text-[18px]">rule</span>
                      <h4 className="font-title-sm text-title-sm">监听规则</h4>
                      <div className="group relative flex items-center">
                        <span className="material-symbols-outlined text-[16px] text-on-surface-variant/40 cursor-help hover:text-on-surface-variant/70 transition-colors leading-none relative -top-px">help</span>
                        <div className="absolute left-1/2 -translate-x-1/2 bottom-full mb-2 w-[240px] px-3 py-2.5 rounded-lg bg-surface-container-highest text-on-surface text-[12px] leading-relaxed opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all duration-200 pointer-events-none shadow-lg border border-outline-variant/20 z-50">
                          规则按列表顺序匹配，第一个命中的生效。当指定进程运行时，自动切换到绑定的配色方案。进程退出后可选择是否恢复上一方案。
                          <div className="absolute left-1/2 -translate-x-1/2 top-full w-2 h-2 bg-surface-container-highest rotate-45 -mt-1 border-r border-b border-outline-variant/20" />
                        </div>
                      </div>
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
                      <span className="font-label-md text-label-md">添加规则</span>
                    </button>
                  </div>

                  {settings.process_rules.length === 0 ? (
                    <div className="flex flex-col items-center justify-center py-12 text-on-surface-variant/40">
                      <span className="material-symbols-outlined text-[48px] mb-4">terminal</span>
                      <p className="font-body-md">暂无监听规则</p>
                      <p className="font-body-sm mt-1">添加规则后，当指定进程运行时自动切换配色方案</p>
                    </div>
                  ) : (
                    <div className="space-y-2">
                       {settings.process_rules.map((rule, index) => (
                        <div
                          key={rule.id}
                          className={`group relative rounded-xl border transition-all duration-200 ${
                            rule.enabled
                              ? "bg-surface-container-high/60 border-outline-variant/20 hover:bg-surface-container-high/80"
                              : "bg-surface-container-high/25 border-outline-variant/10"
                          }`}
                        >
                          {/* Row 1: 优先级 + Toggle + 进程名 + 方案标签 + 删除 */}
                          <div className="flex items-center gap-3 px-4 py-3">
                            {/* 优先级徽章 */}
                            <div className={`w-6 h-6 rounded-md flex items-center justify-center text-[11px] font-bold shrink-0 ${
                              index === 0
                                ? "bg-primary/20 text-primary"
                                : "bg-surface-container-highest/60 text-on-surface-variant/60"
                            }`}>
                              {index + 1}
                            </div>

                            <Toggle size="sm" checked={rule.enabled} onChange={() => handleToggleProcessRule(rule)} />

                            {/* 进程名 */}
                            <span className={`font-mono text-[14px] font-medium flex-1 min-w-0 truncate ${rule.enabled ? "text-on-surface" : "text-on-surface-variant/50"}`}>
                              {rule.process_name}
                            </span>

                            {/* 方案标签 */}
                            <span className={`text-[13px] font-semibold px-2.5 py-1 rounded-lg shrink-0 ${
                              rule.enabled
                                ? "bg-primary/10 text-primary"
                                : "bg-surface-container-highest/40 text-on-surface-variant/50"
                            }`}>
                              {rule.config_name}
                            </span>

                            {/* 删除 */}
                            <button
                              onClick={() => handleDeleteProcessRule(rule.id)}
                              className="w-7 h-7 flex items-center justify-center rounded-lg text-on-surface-variant/40 hover:text-error hover:bg-error/10 transition-all duration-200 active:scale-90 shrink-0"
                            >
                              <span className="material-symbols-outlined text-[18px]">delete</span>
                            </button>
                          </div>

                          {/* Row 2: 方案下拉（左）+ 退出时恢复（右） */}
                          <div className="flex items-center justify-between px-4 pb-3 pt-0">
                            {/* 方案切换下拉 */}
                            <div className="flex items-center gap-2">
                              <div className="relative">
                                <select
                                  value={rule.config_name}
                                  onChange={(e) => handleUpdateProcessRule({ ...rule, config_name: e.target.value })}
                                  className="appearance-none bg-surface-container-highest/40 border border-outline-variant/30 text-on-surface-variant text-[13px] font-normal rounded-lg px-2.5 py-1 pr-7 cursor-pointer hover:border-primary/40 hover:text-on-surface focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all duration-200"
                                >
                                  {configs.map((config) => (
                                    <option key={config.name} value={config.name}>
                                      {config.name}
                                    </option>
                                  ))}
                                </select>
                                <span className="material-symbols-outlined absolute right-1 top-1/2 -translate-y-1/2 text-on-surface-variant/50 text-[16px] pointer-events-none">
                                  expand_more
                                </span>
                              </div>
                              <span className="text-[12px] text-on-surface-variant/60">可切换</span>
                            </div>

                            {/* 退出时恢复 */}
                            <div className="flex items-center gap-2">
                              <span className="text-[12px] text-on-surface-variant/60">退出时恢复</span>
                              <TextSwitch
                                checked={rule.restore_on_exit}
                                onChange={(v) => handleUpdateProcessRule({ ...rule, restore_on_exit: v })}
                                checkedLabel="是"
                                uncheckedLabel="否"
                              />
                            </div>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}

                </div>
              </div>
            )}
            </div>
          </div>
        </div>
      </div>

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
              <h3 className="font-headline-sm text-headline-sm font-bold">添加进程规则</h3>
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
                    value={newRuleProcessName}
                    onChange={(e) => setNewRuleProcessName(e.target.value)}
                    placeholder="例如: chrome.exe"
                    className="flex-1 bg-surface-container-highest/60 border border-outline-variant/40 text-on-surface font-body-md text-body-md rounded-lg px-3 py-2 placeholder:text-on-surface-variant/50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all duration-200"
                  />
                </div>
              </div>

              {/* 配置方案选择 */}
              <div className="space-y-2">
                <label className="font-label-md text-label-md text-on-surface">配置方案</label>
                <div className="relative">
                  <select
                    value={newRuleConfigName}
                    onChange={(e) => setNewRuleConfigName(e.target.value)}
                    className="w-full appearance-none bg-surface-container-highest/60 border border-outline-variant/40 text-on-surface font-body-md text-body-md rounded-lg px-3 py-2 pr-8 cursor-pointer hover:border-primary/50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary/30 transition-all duration-200"
                  >
                    <option value="">请选择配置方案</option>
                    {configs.map((config) => (
                      <option key={config.name} value={config.name}>
                        {config.name}
                      </option>
                    ))}
                  </select>
                  <span className="material-symbols-outlined absolute right-1.5 top-1/2 -translate-y-1/2 text-on-surface-variant text-[18px] pointer-events-none">
                    expand_more
                  </span>
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
                <div className="max-h-56 overflow-y-auto space-y-1 bg-surface-container-highest/30 rounded-xl p-1.5 custom-scrollbar">
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
                            onClick={() => setNewRuleProcessName(p.name)}
                            className={`w-full text-left px-3 py-2.5 rounded-xl transition-all duration-200 active:scale-[0.98] flex items-center gap-3 ${
                              newRuleProcessName === p.name
                                ? "bg-primary/15 border border-primary/30 shadow-sm shadow-primary/5"
                                : "border border-transparent hover:bg-surface-variant/40 text-on-surface"
                            }`}
                          >
                            {/* 进程图标 */}
                            <div className={`w-8 h-8 rounded-lg flex items-center justify-center shrink-0 overflow-hidden ${
                              newRuleProcessName === p.name ? "bg-primary/10" : "bg-surface-container-highest/40"
                            }`}>
                              {p.icon ? (
                                <img src={p.icon} alt="" className="w-5 h-5 object-contain" />
                              ) : (
                                <span className={`material-symbols-outlined text-[18px] ${
                                  newRuleProcessName === p.name ? "text-primary" : "text-on-surface-variant/40"
                                }`}>terminal</span>
                              )}
                            </div>

                            {/* 进程信息 */}
                            <div className="flex-1 min-w-0">
                              <span className={`font-mono text-sm block truncate ${
                                newRuleProcessName === p.name ? "text-primary font-medium" : "text-on-surface"
                              }`}>
                                {p.name}
                              </span>
                              <span className="font-mono text-[11px] text-on-surface-variant/40 block">
                                PID {p.pid}
                              </span>
                            </div>

                            {/* 选中标记 */}
                            {newRuleProcessName === p.name && (
                              <span className="material-symbols-outlined text-[18px] text-primary shrink-0">check_circle</span>
                            )}
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
                添加规则
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default SettingsModal;
