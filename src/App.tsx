import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import ProfileList from "./components/ProfileList";
import ColorAdjuster from "./components/ColorAdjuster";
import PreviewImage from "./components/PreviewImage";
import ConfigManager from "./components/ConfigManager";
import SaveModal from "./components/SaveModal";
import UpdateBanner from "./components/UpdateBanner";
import "./App.css";

interface ColorConfig {
  name: string;
  icon?: string;
  brightness: number;
  contrast: number;
  gamma: number;
  digital_vibrance: number;
  icc_profile: string | null;
}

interface DisplayMonitor {
  name: string;
  device_id: string;
  pnp_id: string;
  is_primary: boolean;
}

function App() {
  const [activeProfile, setActiveProfile] = useState<string>("Default");
  const [brightness, setBrightness] = useState(0);
  const [contrast, setContrast] = useState(0);
  const [gamma, setGamma] = useState(1.0);
  const [digitalVibrance, setDigitalVibrance] = useState(50);
  const [selectedDeviceId, setSelectedDeviceId] = useState<string | undefined>();

  const [configs, setConfigs] = useState<string[]>([]);
  const [selectedConfig, setSelectedConfig] = useState("");
  const [loading, setLoading] = useState(false);
  const [toast, setToast] = useState<{ type: "success" | "error"; text: string } | null>(null);
  const [showSaveModal, setShowSaveModal] = useState(false);
  const [dark, setDark] = useState(() => {
    const saved = localStorage.getItem("theme");
    return saved ? saved === "dark" : true;
  });
  const [baseline, setBaseline] = useState({ brightness: 0, contrast: 0, gamma: 1.0, digitalVibrance: 50, iccProfile: "Default" });
  const [monitors, setMonitors] = useState<DisplayMonitor[]>([]);
  const monitorRefs = useRef<(HTMLButtonElement | null)[]>([]);
  const [indicator, setIndicator] = useState({ left: 0, width: 0 });

  const hasChanges =
    brightness !== baseline.brightness ||
    contrast !== baseline.contrast ||
    gamma !== baseline.gamma ||
    digitalVibrance !== baseline.digitalVibrance ||
    activeProfile !== baseline.iccProfile;

  useEffect(() => {
    document.documentElement.classList.toggle("dark", dark);
    localStorage.setItem("theme", dark ? "dark" : "light");
  }, [dark]);

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

  const showToast = useCallback((type: "success" | "error", text: string) => {
    setToast({ type, text });
    setTimeout(() => setToast(null), 3000);
  }, []);

  const refreshConfigs = useCallback(async () => {
    try {
      const result = await invoke<string[]>("list_configs");
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
          setBrightness(existing.brightness);
          setContrast(existing.contrast);
          setGamma(existing.gamma);
          const profile = existing.icc_profile || "Default";
          setActiveProfile(profile);
          setBaseline({
            brightness: existing.brightness,
            contrast: existing.contrast,
            gamma: existing.gamma,
            digitalVibrance: existing.digital_vibrance,
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
    setBrightness(config.brightness);
    setContrast(config.contrast);
    setGamma(config.gamma);
    setDigitalVibrance(config.digital_vibrance);
    const profile = config.icc_profile || "Default";
    setActiveProfile(profile);
    setBaseline({
      brightness: config.brightness,
      contrast: config.contrast,
      gamma: config.gamma,
      digitalVibrance: config.digital_vibrance,
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
      const settings = await invoke<{ brightness: number; contrast: number; gamma: number; digital_vibrance: number }>(
        "get_nvidia_settings",
        { deviceId: deviceId || undefined }
      );
      setBrightness(settings.brightness);
      setContrast(settings.contrast);
      setGamma(settings.gamma);
      setDigitalVibrance(settings.digital_vibrance);
      setBaseline({
        brightness: settings.brightness,
        contrast: settings.contrast,
        gamma: settings.gamma,
        digitalVibrance: settings.digital_vibrance,
        iccProfile: activeProfile,
      });
    } catch (err) {
      console.error("Failed to load settings for display:", err);
    }
  };

  const handleSaveCurrent = async (name: string, icon?: string) => {
    setLoading(true);
    try {
      const config: ColorConfig = {
        name,
        icon,
        brightness,
        contrast,
        gamma,
        digital_vibrance: digitalVibrance,
        icc_profile: activeProfile !== "Default" ? activeProfile : null,
      };
      await invoke("save_config", { config });
      showToast("success", `「${name}」已保存`);
      await refreshConfigs();
    } catch (err) {
      showToast("error", `保存失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleConfigLoad = async (config: ColorConfig) => {
    await handleApply(config);
  };

  return (
    <div data-components="App" className="min-h-screen bg-background text-on-surface font-body-md">
      <UpdateBanner />
      {/* Header - Lumina Precision style */}
      <header data-name="header" className="bg-surface-container-low/80 backdrop-blur-md border-b border-outline-variant/20 flex items-center px-lg h-16 w-full z-50 fixed top-0">
        {/* Logo & Title */}
        <div className="flex items-center gap-md mr-lg">
          <img src="/favicon.png" alt="icon" className="w-8 h-8 rounded-md" />
          <div className="flex items-baseline gap-sm">
            <h1 className="font-headline-md text-headline-md font-medium text-primary">Filter Manage</h1>
            <span className="font-label-sm text-label-sm text-on-surface-variant/60">v{__APP_VERSION__}</span>
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
            <span className="material-symbols-outlined text-[18px]">settings_backup_restore</span>
            <span className="font-label-md text-label-md">重置默认</span>
          </button>

          <button
            data-name="save-button"
            onClick={() => setShowSaveModal(true)}
            disabled={loading || (!selectedConfig && !hasChanges)}
            className="flex items-center gap-xs px-lg py-sm rounded bg-primary text-on-primary font-bold shadow-lg shadow-primary/20 active:scale-95 duration-150 transition-all disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <span className="material-symbols-outlined text-[18px]">save</span>
            <span className="font-label-md text-label-md">保存方案</span>
          </button>

          <div className="w-[1px] h-6 bg-outline-variant/30 mx-sm"></div>

          <button
            data-name="theme-toggle"
            onClick={() => setDark(!dark)}
            className="w-8 h-8 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-primary/10 transition-colors"
          >
            <span className="material-symbols-outlined text-[20px]">{dark ? "light_mode" : "dark_mode"}</span>
          </button>
        </div>
      </header>

      {/* Toast */}
      {toast && (
        <div data-name="toast" className="fixed top-20 left-1/2 -translate-x-1/2 z-50">
          <div className={`px-md py-sm rounded-md font-label-md text-label-md shadow-lg backdrop-blur-sm ${
            toast.type === "success"
              ? "bg-primary/90 text-on-primary"
              : "bg-error/90 text-on-error"
          }`}>
            {toast.text}
          </div>
        </div>
      )}

      {/* Main Content */}
      <main data-name="main-content" className="pt-16 h-screen overflow-hidden">
        <div className="grid grid-cols-12 h-full gap-md p-md">
          <div data-name="profile-panel" className="col-span-3 flex flex-col h-full">
            <ProfileList
              activeProfile={activeProfile}
              onProfileSelect={handleProfileChange}
              showToast={showToast}
              selectedDeviceId={selectedDeviceId}
            />
          </div>
          <div data-name="adjuster-panel" className="col-span-5 flex flex-col h-full">
            <ColorAdjuster
              brightness={brightness}
              contrast={contrast}
              gamma={gamma}
              digitalVibrance={digitalVibrance}
              selectedDeviceId={selectedDeviceId}
              monitors={monitors}
              currentMonitorName={monitors.find(m => m.device_id === selectedDeviceId)?.name}
              onBrightnessChange={setBrightness}
              onContrastChange={setContrast}
              onGammaChange={setGamma}
              onDigitalVibranceChange={setDigitalVibrance}
              onDeviceChange={setSelectedDeviceId}
            />
          </div>
          <div data-name="preview-config-panel" className="col-span-4 flex flex-col h-full gap-gutter">
            <PreviewImage showToast={showToast} />
            <ConfigManager
              configs={configs}
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
      />
    </div>
  );
}

export default App;
