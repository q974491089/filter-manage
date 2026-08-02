import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { exit } from "@tauri-apps/plugin-process";
import ProfileList from "./components/ProfileList";
import ColorAdjuster from "./components/ColorAdjuster";
import PreviewImage from "./components/PreviewImage";
import ConfigManager from "./components/ConfigManager";
import SaveModal from "./components/SaveModal";
import UpdateModal from "./components/UpdateModal";
import AboutModal from "./components/AboutModal";
import SettingsModal from "./components/SettingsModal";
import ClosePromptModal from "./components/ClosePromptModal";
import AnnouncementBell from "./components/AnnouncementBell";
import AnnouncementModal from "./components/AnnouncementModal";
import { Icon } from "./components/Icon";
import { useUpdater } from "./hooks/useUpdater";
import { useAnnouncements } from "./hooks/useAnnouncements";
import type { RgbScaleMode } from "./lib/rgbScale";

interface ColorConfig {
  name: string;
  icon?: string;
  brightness: number;
  contrast: number;
  gamma: number;
  digital_vibrance: number;
  rgb_r?: number;
  rgb_g?: number;
  rgb_b?: number;
  icc_profile: string | null;
}

interface DisplayMonitor {
  name: string;
  device_id: string;
  pnp_id: string;
  is_primary: boolean;
}

interface AppSettings {
  close_to_tray: boolean | null;  // null=未选择，true=最小化到托盘，false=直接关闭
  close_prompted: boolean;
  autostart: boolean;
  tray_presets: string[];
  shortcuts: { shortcut: string; config_name: string }[];
}

function App() {
  const [activeProfile, setActiveProfile] = useState<string>("Default");
  const [brightness, setBrightness] = useState(0);
  const [contrast, setContrast] = useState(0);
  const [gamma, setGamma] = useState(1.0);
  const [digitalVibrance, setDigitalVibrance] = useState(50);
  const [rgbR, setRgbR] = useState(0);
  const [rgbG, setRgbG] = useState(0);
  const [rgbB, setRgbB] = useState(0);
  const [rgbScaleMode, setRgbScaleMode] = useState<RgbScaleMode>(() => {
    const saved = localStorage.getItem("rgbScaleMode");
    if (saved === "nvidia" || saved === "zowie" || saved === "aoc") return saved;
    return "nvidia";
  });
  const [selectedDeviceId, setSelectedDeviceId] = useState<string | undefined>();

  const [configs, setConfigs] = useState<ColorConfig[]>([]);
  const [selectedConfig, setSelectedConfig] = useState("");
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [showSaveModal, setShowSaveModal] = useState(false);
  type ThemeMode = "light" | "dark" | "system";
  const [themeMode, setThemeMode] = useState<ThemeMode>(() => {
    return (localStorage.getItem("themeMode") as ThemeMode) || "dark";
  });
  const [systemDark, setSystemDark] = useState(() => {
    return window.matchMedia("(prefers-color-scheme: dark)").matches;
  });
  const dark = themeMode === "system" ? systemDark : themeMode === "dark";
  const [showAboutModal, setShowAboutModal] = useState(false);
  const [showSettingsModal, setShowSettingsModal] = useState(false);
  const [showClosePrompt, setShowClosePrompt] = useState(false);
  const [closePromptInitialTray, setClosePromptInitialTray] = useState<boolean | null>(null);
  const updater = useUpdater();
  const ann = useAnnouncements();
  const [baseline, setBaseline] = useState({
    brightness: 0,
    contrast: 0,
    gamma: 1.0,
    digitalVibrance: 50,
    rgbR: 0,
    rgbG: 0,
    rgbB: 0,
    iccProfile: "Default",
  });
  const [monitors, setMonitors] = useState<DisplayMonitor[]>([]);
  const monitorRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const [indicator, setIndicator] = useState({ left: 0, width: 0 });

  const hasChanges =
    brightness !== baseline.brightness ||
    contrast !== baseline.contrast ||
    gamma !== baseline.gamma ||
    digitalVibrance !== baseline.digitalVibrance ||
    rgbR !== baseline.rgbR ||
    rgbG !== baseline.rgbG ||
    rgbB !== baseline.rgbB ||
    activeProfile !== baseline.iccProfile;

  const handleRgbScaleModeChange = (mode: RgbScaleMode) => {
    setRgbScaleMode(mode);
    localStorage.setItem("rgbScaleMode", mode);
  };

  // 监听系统主题变化
  useEffect(() => {
    const mql = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = (e: MediaQueryListEvent) => setSystemDark(e.matches);
    mql.addEventListener("change", handler);
    return () => mql.removeEventListener("change", handler);
  }, []);

  // 应用主题到 DOM
  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    localStorage.setItem("themeMode", themeMode);
  }, [dark, themeMode]);

  // 更新滑动指示器位置
  useEffect(() => {
    const idx = monitors.findIndex((m) => m.device_id === selectedDeviceId);
    const btn = monitorRefs.current[idx];
    if (btn) {
      setIndicator({ left: btn.offsetLeft, width: btn.offsetWidth });
    }
  }, [selectedDeviceId, monitors]);

  useEffect(() => {
    if (!import.meta.env.DEV) {
      const handler = (e: Event) => e.preventDefault();
      document.addEventListener("contextmenu", handler);
      return () => document.removeEventListener("contextmenu", handler);
    }
  }, []);

  // 监听窗口关闭事件
  useEffect(() => {
    const unlisten = getCurrentWindow().onCloseRequested(async (event) => {
      console.log("[App] CloseRequested event received");
      try {
        const settings = await invoke<AppSettings>("get_app_settings");
        console.log("[App] settings:", settings);

        if (settings.close_to_tray === true) {
          // 已设置为最小化到托盘 → 直接隐藏
          console.log("[App] close_to_tray=true, hiding window");
          event.preventDefault();
          await getCurrentWindow().hide();
          return;
        }
        if (settings.close_to_tray === false) {
          // 已设置为直接关闭 → 直接退出
          console.log("[App] close_to_tray=false, exiting");
          // 不 preventDefault，让窗口正常关闭退出
          return;
        }

        // close_to_tray === null → 未设置，弹窗询问
        console.log("[App] close_to_tray=null, showing prompt");
        event.preventDefault();
        setClosePromptInitialTray(null);
        setShowClosePrompt(true);
      } catch (err) {
        console.error("Failed to get settings:", err);
        event.preventDefault();
        setClosePromptInitialTray(null);
        setShowClosePrompt(true);
      }
    });
    return () => { unlisten.then(fn => fn()); };
  }, []);

  // 将已应用的方案同步到快速方案勾选 / 滑块（托盘、快捷键、进程监听共用）
  const syncUiFromAppliedConfig = useCallback(async (configName: string) => {
    if (configName === "__default__") {
      try {
        const def = await invoke<ColorConfig | null>("load_default_config");
        if (def) {
          const rr = def.rgb_r ?? 0;
          const rg = def.rgb_g ?? 0;
          const rb = def.rgb_b ?? 0;
          setBrightness(def.brightness);
          setContrast(def.contrast);
          setGamma(def.gamma);
          setDigitalVibrance(def.digital_vibrance);
          setRgbR(rr);
          setRgbG(rg);
          setRgbB(rb);
          setActiveProfile("Default");
          setSelectedConfig("");
          setBaseline({
            brightness: def.brightness,
            contrast: def.contrast,
            gamma: def.gamma,
            digitalVibrance: def.digital_vibrance,
            rgbR: rr,
            rgbG: rg,
            rgbB: rb,
            iccProfile: "Default",
          });
        }
      } catch (err) {
        console.error("Failed to sync UI after config-applied:", err);
      }
      return;
    }

    try {
      const cfg = await invoke<ColorConfig>("load_config", { name: configName });
      const rr = cfg.rgb_r ?? 0;
      const rg = cfg.rgb_g ?? 0;
      const rb = cfg.rgb_b ?? 0;
      setBrightness(cfg.brightness);
      setContrast(cfg.contrast);
      setGamma(cfg.gamma);
      setDigitalVibrance(cfg.digital_vibrance);
      setRgbR(rr);
      setRgbG(rg);
      setRgbB(rb);
      setActiveProfile(cfg.icc_profile || "Default");
      setSelectedConfig(configName);
      setBaseline({
        brightness: cfg.brightness,
        contrast: cfg.contrast,
        gamma: cfg.gamma,
        digitalVibrance: cfg.digital_vibrance,
        rgbR: rr,
        rgbG: rg,
        rgbB: rb,
        iccProfile: cfg.icc_profile || "Default",
      });
    } catch (err) {
      console.error("Failed to sync UI after config-applied:", err);
    }
  }, []);

  // 监听快捷键/托盘/进程监听应用方案事件
  useEffect(() => {
    const unlisten = listen<string>("config-applied", async (event) => {
      await syncUiFromAppliedConfig(event.payload);
    });

    return () => { unlisten.then(fn => fn()); };
  }, [syncUiFromAppliedConfig]);

  // 启动时补同步：进程监听 reconcile 可能在前端 listen 就绪前就 emit，事件会丢
  useEffect(() => {
    let cancelled = false;

    const syncFromWatcher = async () => {
      try {
        const status = await invoke<{
          active_config_name: string | null;
        }>("get_watcher_status");
        if (cancelled) return;
        const name = status.active_config_name;
        if (name) {
          await syncUiFromAppliedConfig(name);
        }
      } catch (err) {
        console.error("Failed to sync UI from watcher status:", err);
      }
    };

    void syncFromWatcher();
    // reconcile/apply 可能稍晚于首屏，短延迟再拉一次
    const t = window.setTimeout(() => { void syncFromWatcher(); }, 800);

    return () => {
      cancelled = true;
      window.clearTimeout(t);
    };
  }, [syncUiFromAppliedConfig]);

  const showToast = useCallback((type: "success" | "error", text: string) => {
    setToast({ type, text });
    setTimeout(() => setToast(null), 3000);
  }, []);

  const refreshConfigs = useCallback(async () => {
    try {
      const result = await invoke<ColorConfig[]>("list_configs");
      setConfigs(result);
    } catch (err) {
      console.error("Failed to load configs:", err);
    }
  }, []);

  useEffect(() => {
    const loadMonitors = async () => {
      try {
        const result = await invoke<DisplayMonitor[]>("get_display_monitors");
        setMonitors(result);
        if (result.length > 0 && !selectedDeviceId) {
          const primary = result.find((m) => m.is_primary) || result[0];
          setSelectedDeviceId(primary.device_id);
        }
      } catch (err) {
        console.error("Failed to load monitors:", err);
      }
    };
    loadMonitors();
  }, []);

  useEffect(() => {
    const init = async () => {
      await invoke("install_builtin_icc_profiles");
      await refreshConfigs();
      try {
        const existing = await invoke<ColorConfig | null>("load_default_config");
        if (existing) {
          const rr = existing.rgb_r ?? 0;
          const rg = existing.rgb_g ?? 0;
          const rb = existing.rgb_b ?? 0;
          setBrightness(existing.brightness);
          setContrast(existing.contrast);
          setGamma(existing.gamma);
          setRgbR(rr);
          setRgbG(rg);
          setRgbB(rb);
          const profile = existing.icc_profile || "Default";
          setActiveProfile(profile);
          setBaseline({
            brightness: existing.brightness,
            contrast: existing.contrast,
            gamma: existing.gamma,
            digitalVibrance: existing.digital_vibrance,
            rgbR: rr,
            rgbG: rg,
            rgbB: rb,
            iccProfile: profile,
          });
        } else {
          const dvcDefault = await invoke<number>("get_dvc_default_ui_value", { deviceId: selectedDeviceId });
          await invoke("save_default_config", {
            config: {
              name: "__default__",
              brightness: 0,
              contrast: 0,
              gamma: 1.0,
              digital_vibrance: dvcDefault,
              rgb_r: 0,
              rgb_g: 0,
              rgb_b: 0,
              icc_profile: null,
            },
          });
        }
        const driverDvc = await invoke<number>("sync_dvc_from_driver", { deviceId: selectedDeviceId });
        setDigitalVibrance(driverDvc);
        setBaseline((prev) => ({ ...prev, digitalVibrance: driverDvc }));
      } catch (err) {
        console.error("Failed to init defaults:", err);
      }
    };
    init();
  }, [refreshConfigs]);

  const handleApply = async (config: ColorConfig) => {
    const rr = config.rgb_r ?? 0;
    const rg = config.rgb_g ?? 0;
    const rb = config.rgb_b ?? 0;
    setBrightness(config.brightness);
    setContrast(config.contrast);
    setGamma(config.gamma);
    setDigitalVibrance(config.digital_vibrance);
    setRgbR(rr);
    setRgbG(rg);
    setRgbB(rb);
    const profile = config.icc_profile || "Default";
    setActiveProfile(profile);
    setBaseline({
      brightness: config.brightness,
      contrast: config.contrast,
      gamma: config.gamma,
      digitalVibrance: config.digital_vibrance,
      rgbR: rr,
      rgbG: rg,
      rgbB: rb,
      iccProfile: profile,
    });

    if (config.icc_profile) {
      try {
        const profiles = await invoke<{ name: string; path: string }[]>("get_icc_profiles");
        const match = profiles.find((p) => p.name === config.icc_profile);
        if (match) await invoke("set_icc_profile", { profilePath: match.path, deviceId: selectedDeviceId });
      } catch (err) {
        console.error("Failed to apply ICC:", err);
      }
    } else {
      setActiveProfile("Default");
      try {
        await invoke("restore_default_icc_profile", { deviceId: selectedDeviceId });
      } catch (err) {
        console.error("Failed to restore ICC:", err);
      }
    }

    try {
      await Promise.all([
        invoke("set_nvidia_brightness", { deviceId: selectedDeviceId, value: config.brightness }),
        invoke("set_nvidia_contrast", { deviceId: selectedDeviceId, value: config.contrast }),
        invoke("set_nvidia_gamma", { deviceId: selectedDeviceId, value: config.gamma }),
        invoke("set_nvidia_digital_vibrance", { deviceId: selectedDeviceId, value: config.digital_vibrance }),
        invoke("set_nvidia_rgb_gain", { deviceId: selectedDeviceId, r: rr, g: rg, b: rb }),
      ]);
    } catch (err) {
      console.error("Failed to apply NVIDIA:", err);
    }
  };

  const handleRestore = async () => {
    setLoading(true);
    try {
      const def = await invoke<ColorConfig | null>("load_default_config");
      if (def) {
        const dvcDefault = await invoke<number>("get_dvc_default_ui_value", { deviceId: selectedDeviceId });
        def.digital_vibrance = dvcDefault;
        await handleApply(def);
        setSelectedConfig("");
        showToast("success", "已恢复默认设置");
      } else {
        showToast("error", "未找到默认设置");
      }
    } catch (err) {
      showToast("error", `恢复失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleProfileChange = (profile: string) => {
    setActiveProfile(profile);
  };

  const handleMonitorChange = async (deviceId: string) => {
    setSelectedDeviceId(deviceId || undefined);
    try {
      const settings = await invoke<{
        brightness: number;
        contrast: number;
        gamma: number;
        digital_vibrance: number;
        rgb_r?: number;
        rgb_g?: number;
        rgb_b?: number;
      }>("get_nvidia_settings", { deviceId: deviceId || undefined });
      const rr = settings.rgb_r ?? 0;
      const rg = settings.rgb_g ?? 0;
      const rb = settings.rgb_b ?? 0;
      setBrightness(settings.brightness);
      setContrast(settings.contrast);
      setGamma(settings.gamma);
      setDigitalVibrance(settings.digital_vibrance);
      setRgbR(rr);
      setRgbG(rg);
      setRgbB(rb);
      setBaseline({
        brightness: settings.brightness,
        contrast: settings.contrast,
        gamma: settings.gamma,
        digitalVibrance: settings.digital_vibrance,
        rgbR: rr,
        rgbG: rg,
        rgbB: rb,
        iccProfile: activeProfile,
      });
    } catch (err) {
      console.error("Failed to load settings for display:", err);
    }
  };

  const buildColorFields = () => ({
    brightness,
    contrast,
    gamma,
    digital_vibrance: digitalVibrance,
    rgb_r: rgbR,
    rgb_g: rgbG,
    rgb_b: rgbB,
    icc_profile: activeProfile !== "Default" ? activeProfile : null,
  });

  /** 另存为 / 无当前方案时的新建；禁止重名覆盖 */
  const handleSaveCurrent = async (name: string, icon?: string) => {
    const exists = configs.some((c) => c.name.toLowerCase() === name.toLowerCase());
    if (exists) {
      showToast("error", `方案「${name}」已存在，请换一个名称`);
      throw new Error("duplicate name");
    }

    setLoading(true);
    try {
      const config: ColorConfig = {
        name,
        icon,
        ...buildColorFields(),
      };
      await invoke("save_config", { config });
      setSelectedConfig(name);
      setBaseline({
        brightness,
        contrast,
        gamma,
        digitalVibrance,
        rgbR,
        rgbG,
        rgbB,
        iccProfile: activeProfile,
      });
      showToast("success", `「${name}」已保存`);
      await refreshConfigs();
    } catch (err) {
      if (err instanceof Error && err.message === "duplicate name") throw err;
      showToast("error", `保存失败: ${err}`);
      throw err;
    } finally {
      setLoading(false);
    }
  };

  /** 将当前颜色参数写回当前方案（不改名称/图标） */
  const handleUpdateCurrent = async () => {
    if (!selectedConfig || !hasChanges) return;

    const stillExists = configs.some((c) => c.name === selectedConfig);
    if (!stillExists) {
      showToast("error", `方案「${selectedConfig}」不存在，请另存为新方案`);
      setSelectedConfig("");
      return;
    }

    setLoading(true);
    try {
      const existing = await invoke<ColorConfig>("load_config", { name: selectedConfig });
      const config: ColorConfig = {
        ...existing,
        name: selectedConfig,
        ...buildColorFields(),
        // 保留原 icon（load 的 existing.icon）
        icon: existing.icon,
      };
      await invoke("save_config", { config });
      setBaseline({
        brightness,
        contrast,
        gamma,
        digitalVibrance,
        rgbR,
        rgbG,
        rgbB,
        iccProfile: activeProfile,
      });
      showToast("success", `「${selectedConfig}」已更新`);
      await refreshConfigs();
    } catch (err) {
      const msg = String(err);
      if (/not found/i.test(msg)) {
        showToast("error", `方案「${selectedConfig}」不存在，请另存为新方案`);
        setSelectedConfig("");
      } else {
        showToast("error", `更新失败: ${err}`);
      }
    } finally {
      setLoading(false);
    }
  };

  const handleConfigLoad = async (config: ColorConfig) => {
    setSelectedConfig(config.name);
    await handleApply(config);
    const rr = config.rgb_r ?? 0;
    const rg = config.rgb_g ?? 0;
    const rb = config.rgb_b ?? 0;
    setBaseline({
      brightness: config.brightness,
      contrast: config.contrast,
      gamma: config.gamma,
      digitalVibrance: config.digital_vibrance,
      rgbR: rr,
      rgbG: rg,
      rgbB: rb,
      iccProfile: config.icc_profile || "Default",
    });
  };

  const handleClosePromptSelect = async (closeToTray: boolean, remember: boolean) => {
    console.log("[ClosePrompt] handle select:", { closeToTray, remember });

    // 保存到后端设置
    try {
      const settings = await invoke<AppSettings>("get_app_settings");
      await invoke("save_app_settings", {
        settings: {
          ...settings,
          // 勾选「记住」→ 写入具体值；不勾 → 保持 null（下次再问）
          close_to_tray: remember ? closeToTray : null,
          close_prompted: remember,
        },
      });
      console.log("[ClosePrompt] settings saved");
    } catch (err) {
      console.error("Failed to save settings:", err);
    }

    setShowClosePrompt(false);
    console.log("[ClosePrompt] modal closed");

    if (closeToTray) {
      console.log("[ClosePrompt] hiding window...");
      await getCurrentWindow().hide();   // 隐藏窗口到托盘
      console.log("[ClosePrompt] window hidden");
    } else {
      console.log("[ClosePrompt] exiting...");
      await exit(0);   // 退出应用
    }
  };

  return (
    <div data-components="App" className="min-h-screen bg-background text-on-surface font-body-md">
      <UpdateModal updater={updater} />
      <AnnouncementModal items={ann.modalItems} onDismiss={ann.dismissModal} />
      {/* Header - Filter Manage style */}
      <header data-name="header" className="bg-surface-container-low border-b border-outline-variant/20 flex items-center px-lg h-16 w-full z-50 fixed top-0">
        {/* Logo & Title */}
        <div className="flex items-center gap-md mr-lg">
          <img src="/favicon.png" alt="icon" className="w-8 h-8 rounded-md" />
          <div className="flex items-baseline gap-sm">
            <h1 className="font-headline-md text-headline-md font-medium text-primary">Filter Manage</h1>
            <button
              data-name="version-button"
              onClick={() => setShowAboutModal(true)}
              className="font-label-sm text-label-sm text-on-surface-variant/60 hover:text-primary underline-offset-2 hover:underline transition-colors"
            >
              v{__APP_VERSION__}
            </button>
          </div>
        </div>

        {/* 显示器切换 - Segmented Control with sliding indicator */}
        {monitors.length > 1 && (
          <div data-name="monitor-tabs" className="relative flex items-center bg-surface-container-highest/60 rounded-lg p-[3px] h-8">
            {monitors.map((monitor, idx) => {
              const isSelected = selectedDeviceId === monitor.device_id;
              return (
                <button
                  key={monitor.device_id}
                  ref={(el) => { monitorRefs.current[idx] = el; }}
                  onClick={() => handleMonitorChange(monitor.device_id)}
                  className={`relative z-10 flex-1 px-md py-2 rounded-md font-label-md text-label-md transition-colors duration-200 whitespace-nowrap ${
                    isSelected
                      ? "text-on-surface"
                      : "text-on-surface-variant hover:text-on-surface"
                  }`}
                >
                  {monitor.name}
                </button>
              );
            })}
            {/* Sliding indicator */}
            <span
              className="absolute inset-y-[3px] rounded-md bg-surface-container shadow-sm transition-all duration-300 ease-out"
              style={{ left: `${indicator.left}px`, width: `${indicator.width}px` }}
            />
          </div>
        )}

        {/* Spacer */}
        <div className="flex-1" />

        <div data-name="header-actions" className="flex items-center gap-sm">
          <button
            data-name="restore-button"
            onClick={handleRestore}
            disabled={loading}
            className="flex items-center gap-xs px-md py-sm rounded text-on-surface-variant hover:bg-surface-variant/50 transition-all active:scale-95 duration-150 disabled:opacity-40"
          >
            <Icon name="settings_backup_restore" className="text-[18px]" />
            <span className="font-label-md text-label-md">重置默认</span>
          </button>

          {selectedConfig ? (
            <>
              <button
                data-name="update-button"
                onClick={handleUpdateCurrent}
                disabled={loading || !hasChanges}
                className="flex items-center gap-xs px-lg py-sm rounded bg-primary text-on-primary font-bold shadow-lg shadow-primary/20 active:scale-95 duration-150 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
                title={hasChanges ? `将颜色参数写回「${selectedConfig}」` : "当前参数与方案一致，无需更新"}
              >
                <Icon name="save" className="text-[18px]" />
                <span className="font-label-md text-label-md">更新方案</span>
              </button>
              <button
                data-name="save-as-button"
                onClick={() => setShowSaveModal(true)}
                disabled={loading}
                className="flex items-center gap-xs px-md py-sm rounded border border-outline-variant/50 text-on-surface hover:bg-surface-variant/50 active:scale-95 duration-150 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
                title="将当前颜色参数另存为新方案"
              >
                <Icon name="add" className="text-[18px]" />
                <span className="font-label-md text-label-md">另存为</span>
              </button>
            </>
          ) : (
            <button
              data-name="save-button"
              onClick={() => setShowSaveModal(true)}
              disabled={loading || !hasChanges}
              className="flex items-center gap-xs px-lg py-sm rounded bg-primary text-on-primary font-bold shadow-lg shadow-primary/20 active:scale-95 duration-150 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
            >
              <Icon name="save" className="text-[18px]" />
              <span className="font-label-md text-label-md">保存方案</span>
            </button>
          )}

          <div className="w-[1px] h-6 bg-outline-variant/30 mx-sm"></div>

          <AnnouncementBell
            items={ann.visibleItems}
            unreadCount={ann.unreadCount}
            unreadByCategory={ann.unreadByCategory}
            activeTab={ann.activeTab}
            setActiveTab={ann.setActiveTab}
            isRead={ann.isRead}
            open={ann.panelOpen}
            setOpen={ann.setPanelOpen}
            markAllRead={ann.markAllRead}
            detailItem={ann.detailItem}
            openDetail={ann.openDetail}
            closeDetail={ann.closeDetail}
          />

          <button
            data-name="settings-button"
            onClick={() => setShowSettingsModal(true)}
            className="w-8 h-8 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-primary/10 transition-colors"
            title="设置"
          >
            <Icon name="settings" className="text-[20px]" />
          </button>

        </div>
      </header>

      {/* Toast */}
      {toast && (
        <div data-name="toast" className="fixed top-20 left-1/2 -translate-x-1/2 z-[300]">
          <div className={`px-md py-sm rounded-md font-label-md text-label-md shadow-lg backdrop-blur-sm ${
            toast.type === "success"
              ? "bg-primary/90 text-on-primary"
              : "bg-error/90 text-on-error"
          }`}>
            {toast.text}
          </div>
        </div>
      )}

      {/* Main Content — 三列等高，底部卡片底边对齐 */}
      <main data-name="main-content" className="pt-16 h-screen overflow-hidden">
        <div className="grid grid-cols-12 h-full gap-md p-md items-stretch">
          <div data-name="profile-panel" className="col-span-3 flex flex-col min-h-0 h-full">
            <ProfileList
              activeProfile={activeProfile}
              onProfileSelect={handleProfileChange}
              showToast={showToast}
              selectedDeviceId={selectedDeviceId}
            />
          </div>
          <div data-name="adjuster-panel" className="col-span-5 flex flex-col min-h-0 h-full overflow-hidden">
            <ColorAdjuster
              brightness={brightness}
              contrast={contrast}
              gamma={gamma}
              digitalVibrance={digitalVibrance}
              rgbR={rgbR}
              rgbG={rgbG}
              rgbB={rgbB}
              rgbScaleMode={rgbScaleMode}
              selectedDeviceId={selectedDeviceId}
              monitors={monitors}
              currentMonitorName={monitors.find(m => m.device_id === selectedDeviceId)?.name}
              onBrightnessChange={setBrightness}
              onContrastChange={setContrast}
              onGammaChange={setGamma}
              onDigitalVibranceChange={setDigitalVibrance}
              onRgbChange={(r, g, b) => {
                setRgbR(r);
                setRgbG(g);
                setRgbB(b);
              }}
              onRgbScaleModeChange={handleRgbScaleModeChange}
              onDeviceChange={setSelectedDeviceId}
            />
          </div>
          <div data-name="preview-config-panel" className="col-span-4 flex flex-col min-h-0 h-full overflow-hidden gap-gutter">
            <PreviewImage showToast={showToast} />
            <ConfigManager
              configs={configs}
              selectedConfig={selectedConfig}
              onConfigLoad={handleConfigLoad}
              onConfigsChange={refreshConfigs}
              showToast={showToast}
            />
          </div>
        </div>
      </main>

      <SaveModal
        open={showSaveModal}
        onClose={() => setShowSaveModal(false)}
        onSave={handleSaveCurrent}
        loading={loading}
        existingNames={configs.map((c) => c.name)}
        title={selectedConfig ? "另存为新方案" : "保存配置方案"}
        description={
          selectedConfig
            ? "以当前颜色参数创建新方案（不会修改当前方案）"
            : "为当前的颜色设置命名"
        }
        confirmLabel={selectedConfig ? "另存为" : "保存"}
      />

      <AboutModal
        open={showAboutModal}
        onClose={() => setShowAboutModal(false)}
        onCheckUpdate={async () => {
          const result = await updater.checkForUpdate();
          if (result === "available") setShowAboutModal(false);
          return result;
        }}
      />

      <SettingsModal
        open={showSettingsModal}
        onClose={() => setShowSettingsModal(false)}
        configs={configs}
        showToast={showToast}
        themeMode={themeMode}
        onThemeModeChange={setThemeMode}
        onShowAbout={() => {
          setShowSettingsModal(false);
          setShowAboutModal(true);
        }}
      />

      <ClosePromptModal
        open={showClosePrompt}
        initialCloseToTray={closePromptInitialTray}
        onSelect={handleClosePromptSelect}
      />
    </div>
  );
}

export default App;
