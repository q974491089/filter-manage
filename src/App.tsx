import { useCallback, useEffect, useRef, useState } from "react";
import { invoke } from "./lib/ipc";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { listen } from "@tauri-apps/api/event";
import { exit } from "@tauri-apps/plugin-process";
import ProfileList from "./components/ProfileList";
import ColorAdjuster from "./components/ColorAdjuster";
import PreviewImage from "./components/PreviewImage";
import DisplayResolution from "./components/DisplayResolution";
import ConfigManager from "./components/ConfigManager";
import SaveModal from "./components/SaveModal";
import UpdateModal from "./components/UpdateModal";
import AboutModal from "./components/AboutModal";
import DebugModeModal from "./components/DebugModeModal";
import SettingsModal from "./components/SettingsModal";
import ClosePromptModal from "./components/ClosePromptModal";
import AnnouncementBell from "./components/AnnouncementBell";
import AnnouncementModal from "./components/AnnouncementModal";
import SharePlanModal from "./components/SharePlanModal";
import ImportShareModal from "./components/ImportShareModal";
import IccEffectPreviewModal from "./components/IccEffectPreviewModal";
import AuthModal from "./components/AuthModal";
import AccountMenu from "./components/AccountMenu";
import MyInfoModal from "./components/MyInfoModal";
import { Icon } from "./components/Icon";
import { useUpdater } from "./hooks/useUpdater";
import { useAnnouncements } from "./hooks/useAnnouncements";
import { useFeatureFlags } from "./hooks/useFeatureFlags";
import { FEATURE_PLAN_SHARE } from "./lib/featureFlags";
import { parseShareSlug } from "./lib/share";
import {
  authErrorMessage,
  authLogout,
  bindContributor,
  getAuthSession,
  isUnauthorizedError,
  refreshAuth,
  type AuthSession,
} from "./lib/auth";
import { getContributorId } from "./lib/submissions";
import { ensureEnvReady } from "./lib/serverEnv";
import { debugNote, setDebugRecording } from "./lib/debug";
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
  /** 目标分辨率 "宽x高"，null/undefined = 方案不调节分辨率 */
  resolution?: string | null;
  icc_profile: string | null;
}

interface DisplayMonitor {
  name: string;
  device_id: string;
  pnp_id: string;
  is_primary: boolean;
}

/**
 * `apply_preset_resolution` 返回值（见 .docs/api/icc.md）：判别字段 status。
 * 五种 status 都是 Ok（部分成功）；`skipped_unsupported` / `skipped_invalid`
 * 表示分辨率没动但颜色应继续应用，不作为全局失败处理。
 */
type PresetResolutionResult =
  | { status: "unchanged" }
  | { status: "applied" }
  | { status: "restored" }
  | { status: "skipped_unsupported"; requested: string; reason: string }
  | { status: "skipped_invalid"; requested: string };

/** 数字振动能力探测结果（对应后端 `nvidia::DvcCapability`） */
interface DvcCapability {
  supported: boolean;
  /** 命中的后端："nvidia" | "amd"；不支持时为 null */
  vendor: string | null;
  /** 不支持时的原因，可直接展示给用户 */
  reason: string | null;
  driver_min: number;
  driver_max: number;
  default_ui_value: number;
}

interface AppSettings {
  close_to_tray: boolean | null;  // null=未选择，true=最小化到托盘，false=直接关闭
  close_prompted: boolean;
  autostart: boolean;
  autostart_silent: boolean;
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
  const [iccEffectPreview, setIccEffectPreview] = useState<string | null>(null);
  const [iccEffectPreviewPath, setIccEffectPreviewPath] = useState<string | null>(null);
  const [iccEffectPreviewOpen, setIccEffectPreviewOpen] = useState(false);
  const [capturingIccPreview, setCapturingIccPreview] = useState(false);
  const [showClosePrompt, setShowClosePrompt] = useState(false);
  const [closePromptInitialTray, setClosePromptInitialTray] = useState<boolean | null>(null);
  // 调试模式：左上角图标 15 秒内连点 10 次触发（彩蛋入口），授权后记录操作与系统响应
  const [showDebugModal, setShowDebugModal] = useState(false);
  const [debugActive, setDebugActive] = useState(false);
  const [debugLogPath, setDebugLogPath] = useState<string | null>(null);
  const [lastReportPath, setLastReportPath] = useState<string | null>(null);
  /** 分享：待分享的方案名（null = 关闭）；importShareOpen 控制导入弹窗 */
  const [shareConfigName, setShareConfigName] = useState<string | null>(null);
  const [shareConfig, setShareConfig] = useState<ColorConfig | null>(null);
  const [importShareOpen, setImportShareOpen] = useState(false);
  /** 启动时从剪贴板检测到的分享 slug（null = 手动打开入口） */
  const [importShareSlug, setImportShareSlug] = useState<string | null>(null);
  const importedFromShareRef = useRef(false);
  /** 本会话已处理过（导入或忽略）的分享 slug：防止焦点检测反复打扰 */
  const handledShareSlugsRef = useRef(new Set<string>());
  const importShareOpenRef = useRef(false);
  importShareOpenRef.current = importShareOpen;
  const updater = useUpdater();
  const ann = useAnnouncements();
  const featureFlags = useFeatureFlags();
  const featureFlagsRef = useRef(featureFlags);
  featureFlagsRef.current = featureFlags;

  /** 客户端账号会话：null = 未登录（登录弹窗由顶栏入口触发） */
  const [session, setSession] = useState<AuthSession | null>(null);
  const [showAuthModal, setShowAuthModal] = useState(false);
  const [showMyInfoModal, setShowMyInfoModal] = useState(false);

  /**
   * 启动会话恢复：凭据管理器里有令牌就校验一次（顺带轮换，30 天 TTL）。
   * 401 = 令牌失效/被吊销 → 回未登录态；网络失败时 auth_get_session 已返回
   * 缓存的上次已知会话，这里不重复兜底。
   */
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        await ensureEnvReady();
      } catch {
        /* serverEnv 自带兜底值，继续 */
      }
      try {
        let current = await getAuthSession();
        if (current) {
          try {
            current = await refreshAuth();
          } catch (err) {
            if (isUnauthorizedError(err)) current = null;
            // 其它错误（如网络抖动）保持现有会话
          }
        }
        if (!cancelled) setSession(current);
      } catch (err) {
        debugNote(`startup auth: ${authErrorMessage(err)}`);
        if (!cancelled) setSession(null);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  /** 登录/注册/GitHub 授权成功：更新会话 + 静默绑定投稿身份。 */
  const handleAuthLoggedIn = useCallback((loggedIn: AuthSession) => {
    setSession(loggedIn);
    setShowAuthModal(false);
    void (async () => {
      try {
        const contributorId = await getContributorId();
        await bindContributor(contributorId, loggedIn.account.displayName);
      } catch (err) {
        // 冲突（身份已属其他账号）等错误不打断登录流程，留痕排查。
        debugNote(`bind-contributor: ${authErrorMessage(err)}`);
      }
    })();
  }, []);

  /** 手动打开导入弹窗：不带预填 slug */
  const openImportShare = useCallback(() => {
    importedFromShareRef.current = false;
    setImportShareSlug(null);
    setImportShareOpen(true);
  }, []);

  /**
   * 从一条链接/协议 URL 打开导入弹窗。
   * force = true 时跳过抑制检查（落地页按钮、协议唤起都是显式意图）。
   */
  const openShareFromUrl = useCallback((url: string, force: boolean) => {
    const slug = parseShareSlug(url);
    debugNote(`[share] 收到链接 ${url} (force=${force}, slug=${slug ?? "无"})`);
    if (!slug) return;
    if (!featureFlagsRef.current.isEnabled(FEATURE_PLAN_SHARE)) return;
    if (!force) {
      // 本会话已处理过（导入或忽略）→ 不再打扰
      if (handledShareSlugsRef.current.has(slug)) return;
      let dismissed: string[] = [];
      try {
        dismissed = JSON.parse(localStorage.getItem("dismissedShareSlugs") ?? "[]");
      } catch {
        /* localStorage 不可用就算了 */
      }
      if (Array.isArray(dismissed) && dismissed.includes(slug)) return;
    }
    importedFromShareRef.current = false;
    setImportShareSlug(slug);
    setImportShareOpen(true);
  }, []);

  /** 读取剪贴板，是分享链接就弹导入窗（启动与窗口聚焦共用） */
  const checkClipboardForShare = useCallback(() => {
    if (importShareOpenRef.current) return;
    if (!featureFlagsRef.current.isEnabled(FEATURE_PLAN_SHARE)) return;
    void (async () => {
      try {
        // 先等接口环境就绪，否则会用线上兜底值查询（明明切了预发却请求线上的竞态）
        await ensureEnvReady();
        const text = await invoke<string>("read_clipboard_text");
        openShareFromUrl(text, false);
      } catch {
        // 剪贴板没有文本或放的是文件等其他格式：静默
      }
    })();
  }, [openShareFromUrl]);

  // 启动后 600ms 检测一次剪贴板
  useEffect(() => {
    const timer = window.setTimeout(() => checkClipboardForShare(), 600);
    return () => window.clearTimeout(timer);
  }, [checkClipboardForShare]);

  // 窗口获得焦点时再检测一次：客户端开着的时候复制链接切回来也能弹
  const lastClipboardCheckRef = useRef(0);
  useEffect(() => {
    const onFocus = () => {
      const now = Date.now();
      if (now - lastClipboardCheckRef.current < 2000) return;
      lastClipboardCheckRef.current = now;
      checkClipboardForShare();
    };
    window.addEventListener("focus", onFocus);
    const onVisibility = () => {
      if (!document.hidden) onFocus();
    };
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onVisibility);
    };
  }, [checkClipboardForShare]);

  // 协议唤起（热启动）：单实例回调转发的 filter-manage:// 链接
  useEffect(() => {
    const unlisten = listen<string>("share-deep-link", (event) => {
      openShareFromUrl(event.payload, true);
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [openShareFromUrl]);

  // 协议唤起（冷启动）：setup 阶段暂存的链接，前端挂载后取走
  useEffect(() => {
    void (async () => {
      try {
        const url = await invoke<string | null>("get_pending_share_url");
        if (url) openShareFromUrl(url, true);
      } catch {
        /* 没有待处理的链接 */
      }
    })();
  }, [openShareFromUrl]);

  const [baseline, setBaseline] = useState({
    brightness: 0,
    contrast: 0,
    gamma: 1.0,
    digitalVibrance: 50,
    rgbR: 0,
    rgbG: 0,
    rgbB: 0,
    iccProfile: "Default",
    /** 分辨率基线：最近一次采纳点（启动/加载方案/切显示器）的实际模式 "宽x高@Hz" */
    resolution: null as string | null,
    /** 保存范围基线：上次保存/加载时"是否把分辨率写入方案" */
    includeResolution: false,
  });
  /**
   * 保存范围开关：是否把当前分辨率和刷新率写入方案。
   * 新建方案默认关闭；加载含 resolution 的已有方案时开启（见 handleApply / syncUiFromAppliedConfig）。
   * 关闭时保存不写入目标模式，且不自动改动已有方案的 JSON——只有用户保存/更新时才落盘。
   */
  const [includeResolution, setIncludeResolution] = useState(false);
  /**
   * 保存范围控件是否对用户可见。
   *
   * 分辨率卡片（分辨率功能入口）目前没有独立远程开关，始终渲染 → 控件可见。
   * 但保存路径必须按"控件可见性"收敛：一旦未来入口被远程开关隐藏
   * （用 featureFlags.isEnabled(<key>) 包住 DisplayResolution 即可），
   * 把这里改成对应的 isEnabled 判断——不可见时更新已有方案保留原 resolution
   * 字段、新建方案不携带分辨率，绝不因控件不可见而静默清空用户的方案字段。
   */
  const resolutionSaveControlVisible = true;
  /** 方案区域的部分成功/失败说明（role="status"）；null = 不显示。每次应用前清空，不积累 */
  const [presetStatusNote, setPresetStatusNote] = useState<string | null>(null);
  const [monitors, setMonitors] = useState<DisplayMonitor[]>([]);
  const [dvcCap, setDvcCap] = useState<DvcCapability | null>(null);
  /** 显示器自定义命名：键取稳定标识（pnp_id 优先，空则 device_id），存本机 localStorage */
  const [monitorNames, setMonitorNames] = useState<Record<string, string>>(() => {
    try {
      return JSON.parse(localStorage.getItem("monitorNames") || "{}");
    } catch {
      return {};
    }
  });
  /** 命名/编辑态变化时 +1，让顶部滑动指示器重新测量标签宽度 */
  const [monitorNameRev, setMonitorNameRev] = useState(0);
  /** 正在内联重命名的显示器标签键 */
  const [editingMonitorKey, setEditingMonitorKey] = useState<string | null>(null);

  const setMonitorName = (key: string, name: string) => {
    const trimmed = name.trim();
    setMonitorNames((prev) => {
      const next = { ...prev };
      if (trimmed) next[key] = trimmed;
      else delete next[key];
      localStorage.setItem("monitorNames", JSON.stringify(next));
      return next;
    });
    setMonitorNameRev((r) => r + 1);
  };
  const getMonitorDisplayName = (m: DisplayMonitor) =>
    monitorNames[m.pnp_id || m.device_id] || m.name;
  const activeMonitor = monitors.find((m) => m.device_id === selectedDeviceId);
  /** 当前选中显示器的实际分辨率 "宽x高"，由 DisplayResolution 卡片上报；保存方案时写入 */
  const [currentResolution, setCurrentResolution] = useState<string | null>(null);
  /** 变化时让 DisplayResolution 重拉模式（托盘/快捷键/进程监听应用方案后同步真实状态） */
  const [resRefreshKey, setResRefreshKey] = useState(0);
  /** 分辨率基线采纳开关：置 true 后，卡片上报的下一个实际模式写入 baseline.resolution
   *  （启动、应用方案、切显示器时置位），其余上报只更新当前值参与脏判断 */
  const awaitResBaselineRef = useRef(true);
  const handleCurrentResolutionReport = (res: string | null) => {
    setCurrentResolution(res);
    if (awaitResBaselineRef.current) {
      awaitResBaselineRef.current = false;
      setBaseline((prev) =>
        prev.resolution === res ? prev : { ...prev, resolution: res }
      );
    }
  };
  const monitorRefs = useRef<(HTMLElement | null)[]>([]);
  const logoClicksRef = useRef({ count: 0, first: 0 });
  const [indicator, setIndicator] = useState({ left: 0, width: 0 });

  const hasChanges =
    brightness !== baseline.brightness ||
    contrast !== baseline.contrast ||
    gamma !== baseline.gamma ||
    digitalVibrance !== baseline.digitalVibrance ||
    rgbR !== baseline.rgbR ||
    rgbG !== baseline.rgbG ||
    rgbB !== baseline.rgbB ||
    activeProfile !== baseline.iccProfile ||
    // 保存范围开关变化本身参与脏判断
    includeResolution !== baseline.includeResolution ||
    // 仅在开关开启（保存会写入分辨率）时，实际模式变化才计为分辨率变更
    (includeResolution && currentResolution !== baseline.resolution);

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
  }, [selectedDeviceId, monitors, monitorNameRev, editingMonitorKey]);

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
          // 已设置为直接关闭 → 显式退出应用。
          //
          // 不能靠"不 preventDefault 让它自然关闭"：Tauri 只要检测到前端注册了
          // close-requested 监听器就会自己 prevent_close，再由 JS 包装层调
          // window.destroy()。而 capabilities 里没有 core:window:allow-destroy
          // （只加了 allow-hide），destroy 会被权限系统拒绝，窗口纹丝不动。
          // 走 process 插件的 exit()，和弹窗里选"直接退出"完全一致。
          console.log("[App] close_to_tray=false, exiting");
          event.preventDefault();
          await exit(0);
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
          // 默认方案不携带分辨率：保存范围基线同步归位，避免只改 UI 不改基线
          setIncludeResolution(false);
          setBaseline((prev) => ({
            brightness: def.brightness,
            contrast: def.contrast,
            gamma: def.gamma,
            digitalVibrance: def.digital_vibrance,
            rgbR: rr,
            rgbG: rg,
            rgbB: rb,
            iccProfile: "Default",
            resolution: prev.resolution,
            includeResolution: false,
          }));
        }
        // 后台路径可能改了分辨率：卡片重拉真实状态，并以重拉上报值为分辨率基线
        awaitResBaselineRef.current = true;
        setResRefreshKey((k) => k + 1);
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
      // 后台应用的方案带不带分辨率 → 保存范围状态与基线一起显式更新
      const cfgIncludeResolution = !!cfg.resolution;
      setIncludeResolution(cfgIncludeResolution);
      setBaseline((prev) => ({
        brightness: cfg.brightness,
        contrast: cfg.contrast,
        gamma: cfg.gamma,
        digitalVibrance: cfg.digital_vibrance,
        rgbR: rr,
        rgbG: rg,
        rgbB: rb,
        iccProfile: cfg.icc_profile || "Default",
        resolution: prev.resolution,
        includeResolution: cfgIncludeResolution,
      }));
      // 后台路径可能改了分辨率：卡片重拉真实状态，并以重拉上报值为分辨率基线
      awaitResBaselineRef.current = true;
      setResRefreshKey((k) => k + 1);
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

  // 数字振动只有 NVIDIA 输出的显示器支持。先探测，再决定滑块能不能拖、
  // 应用方案时要不要调它 —— 否则 AMD / Intel 机器上每次操作都会弹一次错误。
  useEffect(() => {
    let cancelled = false;
    // 切显示器后模式列表不同，卡片重拉上报时采纳为新分辨率基线
    awaitResBaselineRef.current = true;
    invoke<DvcCapability>("get_dvc_capability", { deviceId: selectedDeviceId })
      .then((cap) => { if (!cancelled) setDvcCap(cap); })
      .catch((err) => {
        console.error("Failed to probe DVC capability:", err);
        if (!cancelled) setDvcCap(null);
      });
    return () => { cancelled = true; };
  }, [selectedDeviceId]);

  const showToast = useCallback((type: "success" | "error", text: string) => {
    setToast({ type, text });
    // error toast 是「用户可见失败」的统一出口：后端校验失败、前端本地判断失败
    // （如方案重名）最终都会走到这里。调试中一并落盘——比 invoke 漏斗多覆盖
    // 一层不发命令、纯前端判断出来的失败。
    if (type === "error") {
      debugNote(`用户可见错误提示：${text}`, "ui-error");
    }
    setTimeout(() => setToast(null), 3000);
  }, []);

  /** 退出登录：吊销服务端会话 + 清凭据管理器（幂等），界面回未登录态。 */
  const handleAuthLogout = useCallback(async () => {
    try {
      await authLogout();
    } finally {
      setSession(null);
    }
    showToast("success", "已退出登录");
  }, [showToast]);

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
          setBaseline((prev) => ({
            brightness: existing.brightness,
            contrast: existing.contrast,
            gamma: existing.gamma,
            digitalVibrance: existing.digital_vibrance,
            rgbR: rr,
            rgbG: rg,
            rgbB: rb,
            iccProfile: profile,
            resolution: prev.resolution,
            includeResolution: false,
          }));
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

  // 全局 JS 错误兜底：调试中任何未捕获异常 / 未处理 Promise 拒绝都进日志
  useEffect(() => {
    if (!debugActive) return;
    const onError = (e: ErrorEvent) => {
      debugNote(`JS 错误：${e.message} @ ${e.filename}:${e.lineno}:${e.colno}`, "ui-error");
    };
    const onRejection = (e: PromiseRejectionEvent) => {
      debugNote(`未处理的 Promise 拒绝：${String(e.reason)}`, "ui-error");
    };
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    return () => {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    };
  }, [debugActive]);

  // 应用启动时同步一次后端的记录状态（调试中重启 app 后状态条要能恢复）；
  // debugActive 变化同步给行为记录模块（所有组件的 debugNote 靠这个开关门控）
  useEffect(() => {
    invoke<boolean>("debug_status")
      .then((active) => {
        setDebugActive(active);
        setDebugRecording(active);
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    setDebugRecording(debugActive);
  }, [debugActive]);

  // 左上角图标 15 秒内连点 10 次进入调试模式；记录中再点 = 打开控制弹窗
  const handleLogoClick = () => {
    if (debugActive) {
      setShowDebugModal(true);
      return;
    }
    const now = Date.now();
    const st = logoClicksRef.current;
    // 从第一次点击起算 15 秒窗口，超时重新计数
    if (now - st.first > 15000) st.count = 0;
    if (st.count === 0) st.first = now;
    st.count += 1;
    if (st.count >= 5 && st.count < 10) {
      showToast("success", `再点击 ${10 - st.count} 次开启调试模式`);
    } else if (st.count >= 10) {
      st.count = 0;
      setShowDebugModal(true);
    }
  };

  const handleApply = async (config: ColorConfig) => {
    debugNote(`应用方案「${config.name}」`);
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
    // 重新应用即清掉上一轮的部分成功说明，不积累旧状态
    setPresetStatusNote(null);

    // 分辨率先行：模式切换可能重置 gamma ramp，先落分辨率再上色（与后端中央路径一致）。
    // 兼容入口 apply_preset_resolution：不支持的目标模式不报错，返回 skipped_unsupported
    // 暂存为提示，颜色参数继续应用；真实失败仍 toast，且不阻断配色。
    let resolutionSkipNote: string | null = null;
    try {
      const result = await invoke<PresetResolutionResult>("apply_preset_resolution", {
        deviceId: selectedDeviceId,
        resolution: config.resolution ?? null,
      });
      if (result.status === "skipped_unsupported") {
        resolutionSkipNote = `当前显示器不支持方案中的 ${result.requested}，已保留当前分辨率`;
      } else if (result.status === "skipped_invalid") {
        // 配置数据损坏，不做用户提示，静默留痕
        debugNote(`方案分辨率字段非法，已忽略: ${result.requested}`);
      }
    } catch (err) {
      console.error("Failed to apply config resolution:", err);
      showToast("error", `分辨率切换失败: ${err}`);
    }

    setBaseline({
      brightness: config.brightness,
      contrast: config.contrast,
      gamma: config.gamma,
      digitalVibrance: config.digital_vibrance,
      rgbR: rr,
      rgbG: rg,
      rgbB: rb,
      iccProfile: profile,
      // 预填当前实际模式，等下方重拉上报后采纳为应用后的真实基线
      resolution: currentResolution,
      // 保存范围跟随所应用方案：带 resolution 的方案开启，否则关闭
      includeResolution: !!config.resolution,
    });
    setIncludeResolution(!!config.resolution);

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

    let colorError: string | null = null;
    try {
      const calls = [
        invoke("set_nvidia_brightness", { deviceId: selectedDeviceId, value: config.brightness }),
        invoke("set_nvidia_contrast", { deviceId: selectedDeviceId, value: config.contrast }),
        invoke("set_nvidia_gamma", { deviceId: selectedDeviceId, value: config.gamma }),
        invoke("set_nvidia_rgb_gain", { deviceId: selectedDeviceId, r: rr, g: rg, b: rb }),
      ];
      // 探测到明确不支持才跳过；尚未探测完（null）时照常尝试
      if (dvcCap?.supported !== false) {
        calls.push(invoke("set_nvidia_digital_vibrance", { deviceId: selectedDeviceId, value: config.digital_vibrance }));
      }
      await Promise.all(calls);
    } catch (err) {
      console.error("Failed to apply NVIDIA:", err);
      showToast("error", `应用颜色设置失败: ${err}`);
      colorError = String(err);
    }

    // 颜色参数全部处理完后，在方案区域给出部分成功说明：
    // 兼容跳过既不是全局成功也不是全局失败；颜色失败时显示实际错误并保留跳过说明。
    if (resolutionSkipNote) {
      setPresetStatusNote(
        colorError
          ? `颜色设置应用失败：${colorError}；${resolutionSkipNote}`
          : `颜色设置已应用；${resolutionSkipNote}`
      );
    }

    // 本路径可能改了分辨率：卡片重拉真实状态，并以重拉上报值为分辨率基线
    // （skipped_unsupported 时模式未变，重拉同样把实际值采纳为基线）
    awaitResBaselineRef.current = true;
    setResRefreshKey((k) => k + 1);
  };

  const handleRestore = async () => {
    setLoading(true);
    debugNote("点击「重置默认」");
    try {
      const def = await invoke<ColorConfig | null>("load_default_config");
      if (def) {
        const dvcDefault = await invoke<number>("get_dvc_default_ui_value", { deviceId: selectedDeviceId });
        def.digital_vibrance = dvcDefault;
        await handleApply(def);
        setSelectedConfig("");
        showToast("success", "已恢复默认设置");
      } else {
        // 无默认方案：配色无从恢复，但若此前应用过带分辨率的方案，至少把分辨率还回去
        // （兼容入口：resolution=null = 恢复本应用此前改过的分辨率，含刷新率）
        setPresetStatusNote(null);
        try {
          await invoke<PresetResolutionResult>("apply_preset_resolution", {
            deviceId: selectedDeviceId,
            resolution: null,
          });
          awaitResBaselineRef.current = true;
          setResRefreshKey((k) => k + 1);
        } catch {
          /* 无可还原项，忽略 */
        }
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
    debugNote(`切换显示器 → ${deviceId || "主显示器"}`);
    // 旧显示器上的兼容跳过说明对新显示器不再成立，切走即清
    setPresetStatusNote(null);
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
      setBaseline((prev) => ({
        brightness: settings.brightness,
        contrast: settings.contrast,
        gamma: settings.gamma,
        digitalVibrance: settings.digital_vibrance,
        rgbR: rr,
        rgbG: rg,
        rgbB: rb,
        iccProfile: activeProfile,
        resolution: prev.resolution,
        // 切显示器与颜色参数一样重新对齐保存范围基线（实际模式由重拉上报采纳）
        includeResolution,
      }));
    } catch (err) {
      console.error("Failed to load settings for display:", err);
    }
  };

  // 监听显示器拓扑变化（Win+P 投影 / 关显示器 / 热插拔，后端 WM_DISPLAYCHANGE /
  // WM_DEVICECHANGE 事件驱动）：「显示器 1/2」标签动态增减；当前选中的显示器
  // 被移除时自动落到主显示器并加载其参数。
  // 用 ref 持最新处理器，监听器只注册一次、不随状态重建
  const selectedDeviceRef = useRef(selectedDeviceId);
  selectedDeviceRef.current = selectedDeviceId;
  const monitorChangeRef = useRef(handleMonitorChange);
  monitorChangeRef.current = handleMonitorChange;
  useEffect(() => {
    const unlisten = listen<DisplayMonitor[]>("monitors-changed", (event) => {
      const list = event.payload;
      setMonitors(list);
      setResRefreshKey((k) => k + 1);
      const stillThere = list.some((m) => m.device_id === selectedDeviceRef.current);
      if (!stillThere) {
        const fallback = list.find((m) => m.is_primary) || list[0];
        if (fallback) {
          monitorChangeRef.current(fallback.device_id);
        } else {
          setSelectedDeviceId(undefined);
        }
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const buildColorFields = () => ({
    brightness,
    contrast,
    gamma,
    digital_vibrance: digitalVibrance,
    rgb_r: rgbR,
    rgb_g: rgbG,
    rgb_b: rgbB,
    // 保存范围开关决定是否写入目标模式；关闭时写 null，且不自动改动已有方案的 JSON
    // （只在用户保存/更新时落盘）
    resolution: includeResolution ? currentResolution : null,
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
        // 保存范围控件不可见（分辨率功能入口被远程开关隐藏）时，新建方案不携带分辨率
        ...(resolutionSaveControlVisible ? {} : { resolution: null }),
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
        resolution: currentResolution,
        includeResolution,
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
        // 保存范围控件不可见（分辨率功能入口被远程开关隐藏）时，不因控件不可见
        // 而清空原 resolution 字段——只有用户可见地关闭保存范围后才写入 null
        ...(resolutionSaveControlVisible ? {} : { resolution: existing.resolution ?? null }),
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
        resolution: currentResolution,
        includeResolution,
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
    // 基线（含分辨率）统一由 handleApply 设置，这里不再重复覆盖
    await handleApply(config);
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

  /** 分享前把方案完整读出：ConfigManager 的本地类型缺 rgb/resolution 字段 */
  useEffect(() => {
    if (!shareConfigName) {
      setShareConfig(null);
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const cfg = await invoke<ColorConfig>("load_config", { name: shareConfigName });
        if (!cancelled) setShareConfig(cfg);
      } catch (err) {
        if (cancelled) return;
        setShareConfigName(null);
        showToast("error", err instanceof Error ? err.message : String(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [shareConfigName, showToast]);

  /**
   * 按给定参数生成模拟效果图并打上 SIMULATED 标识，返回 base64。
   *
   * 不读写任何 UI 状态：分享弹窗要的是「被分享方案」的画面，而不是当前
   * 屏幕正在用的参数；顶栏相机入口再自己决定保存/打开预览。
   */
  const renderSimulatedPreview = async (params: {
    iccProfile: string | null;
    brightness: number;
    contrast: number;
    gamma: number;
    digitalVibrance: number;
    rgbR: number;
    rgbG: number;
    rgbB: number;
  }): Promise<string> => {
    const pngBase64 = await invoke<string>("capture_icc_effect_preview", {
      profileName:
        !params.iccProfile || params.iccProfile === "Default"
          ? null
          : params.iccProfile,
      settings: {
        brightness: params.brightness,
        contrast: params.contrast,
        gamma: params.gamma,
        digitalVibrance: params.digitalVibrance,
        rgbR: params.rgbR,
        rgbG: params.rgbG,
        rgbB: params.rgbB,
      },
    });

    const image = new Image();
    image.src = `data:image/png;base64,${pngBase64}`;
    await image.decode();
    const canvas = document.createElement("canvas");
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const context = canvas.getContext("2d");
    if (!context) throw new Error("无法生成模拟效果图");
    context.drawImage(image, 0, 0);
    const fontSize = Math.max(14, Math.round(canvas.width / 100));
    const label = "SIMULATED DISPLAY PREVIEW";
    context.font = `600 ${fontSize}px Inter, sans-serif`;
    const padding = Math.round(fontSize * 0.7);
    const labelWidth = context.measureText(label).width;
    context.fillStyle = "rgba(0, 0, 0, 0.72)";
    context.fillRect(16, 16, labelWidth + padding * 2, fontSize + padding * 2);
    context.fillStyle = "#ffffff";
    context.fillText(label, 16 + padding, 16 + padding + fontSize * 0.82);
    const stampedImage = await new Promise<Blob>((resolve, reject) => {
      canvas.toBlob((blob) => blob ? resolve(blob) : reject(new Error("PNG 编码失败")), "image/png");
    });
    const bytes = new Uint8Array(await stampedImage.arrayBuffer());
    let binary = "";
    for (let offset = 0; offset < bytes.length; offset += 0x8000) {
      binary += String.fromCharCode(...bytes.subarray(offset, offset + 0x8000));
    }
    return btoa(binary);
  };

  const handleCaptureIccPreview = async () => {
    if (capturingIccPreview) return;
    setCapturingIccPreview(true);
    try {
      const stampedBase64 = await renderSimulatedPreview({
        iccProfile: activeProfile,
        brightness,
        contrast,
        gamma,
        digitalVibrance,
        rgbR,
        rgbG,
        rgbB,
      });
      const savedPath = await invoke<string>("save_icc_effect_preview", {
        pngBase64: stampedBase64,
      });
      setIccEffectPreviewPath(savedPath);
      setIccEffectPreview(stampedBase64);
      setIccEffectPreviewOpen(true);
    } catch (err) {
      showToast("error", err instanceof Error ? err.message : String(err));
    } finally {
      setCapturingIccPreview(false);
    }
  };

  return (
    <div data-components="App" className="min-h-screen bg-background text-on-surface font-body-md">
      <UpdateModal updater={updater} />
      <AnnouncementModal items={ann.modalItems} onDismiss={ann.dismissModal} />
      <DebugModeModal
        open={showDebugModal}
        active={debugActive}
        logPath={debugLogPath}
        reportPath={lastReportPath}
        onClose={() => {
          setShowDebugModal(false);
          setLastReportPath(null);
        }}
        onStarted={(logPath) => {
          setDebugActive(true);
          setDebugLogPath(logPath);
          setLastReportPath(null);
          showToast("success", "调试记录已开启");
        }}
        onStopped={(reportPath) => {
          setDebugActive(false);
          setDebugLogPath(null);
          // 弹窗停在「报告已生成」完成态：文件已自动复制到剪贴板，可复制文案或确认路径
          setLastReportPath(reportPath);
          // 后端停止时已自动恢复默认配置：把界面滑条拉回与屏幕一致，
          // 避免界面显示调试值、屏幕却是默认值的错位
          (async () => {
            try {
              const def = await invoke<ColorConfig | null>("load_default_config");
              const dvcDefault = await invoke<number>("get_dvc_default_ui_value", {
                deviceId: selectedDeviceId,
              });
              const rr = def?.rgb_r ?? 0;
              const rg = def?.rgb_g ?? 0;
              const rb = def?.rgb_b ?? 0;
              const profile = def?.icc_profile || "Default";
              setBrightness(def?.brightness ?? 0);
              setContrast(def?.contrast ?? 0);
              setGamma(def?.gamma ?? 1.0);
              setDigitalVibrance(dvcDefault);
              setRgbR(rr);
              setRgbG(rg);
              setRgbB(rb);
              setActiveProfile(profile);
              setSelectedConfig("");
              setIncludeResolution(false);
              setBaseline({
                brightness: def?.brightness ?? 0,
                contrast: def?.contrast ?? 0,
                gamma: def?.gamma ?? 1.0,
                digitalVibrance: dvcDefault,
                rgbR: rr,
                rgbG: rg,
                rgbB: rb,
                iccProfile: profile,
                resolution: null,
                includeResolution: false,
              });
              // 后端复位可能改过分辨率：卡片重拉，以上报的实际模式为新基线
              awaitResBaselineRef.current = true;
              setResRefreshKey((k) => k + 1);
            } catch {
              // 恢复界面状态失败不影响报告流程；屏幕本身已被后端重置
            }
          })();
        }}
      />
      {/* Header - Filter Manage style */}
      <header data-name="header" className="bg-surface-container-low border-b border-outline-variant/20 flex items-center px-lg h-16 w-full z-50 fixed top-0">
        {/* Logo & Title */}
        <div className="flex items-center gap-md mr-lg">
          <img
            src="/favicon.png"
            alt="icon"
            onClick={handleLogoClick}
            className="w-8 h-8 rounded-md cursor-default select-none"
          />
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

        {/* 显示器切换 - Segmented Control with sliding indicator，
            标签悬浮出现编辑图标，点击进入内联重命名（Enter/失焦保存，Esc 取消） */}
        {monitors.length > 1 && (
          <div data-name="monitor-tabs" className="relative flex items-center bg-surface-container-highest/60 rounded-lg p-[3px] h-8">
            {monitors.map((monitor, idx) => {
              const isSelected = selectedDeviceId === monitor.device_id;
              const nameKey = monitor.pnp_id || monitor.device_id;
              const isEditing = editingMonitorKey === nameKey;
              return (
                <div
                  key={monitor.device_id}
                  ref={(el) => { monitorRefs.current[idx] = el; }}
                  className="relative z-10 min-w-0 max-w-[240px]"
                >
                  {isEditing ? (
                    <input
                      autoFocus
                      data-name="monitor-rename-input"
                      defaultValue={monitorNames[nameKey] || ""}
                      placeholder={monitor.name}
                      onClick={(e) => e.stopPropagation()}
                      onBlur={(e) => {
                        setMonitorName(nameKey, e.target.value);
                        setEditingMonitorKey(null);
                      }}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          setMonitorName(nameKey, e.currentTarget.value);
                          setEditingMonitorKey(null);
                        } else if (e.key === "Escape") {
                          setEditingMonitorKey(null);
                        }
                      }}
                      className="w-32 h-7 px-2 rounded-md bg-surface-container-high border border-primary/60 font-label-md text-label-md text-on-surface text-center outline-none"
                    />
                  ) : (
                    <button
                      onClick={() => handleMonitorChange(monitor.device_id)}
                      title={getMonitorDisplayName(monitor)}
                      className={`group relative w-full flex items-center justify-center px-md py-2 rounded-md font-label-md text-label-md transition-colors duration-200 whitespace-nowrap ${
                        isSelected
                          ? "text-on-surface"
                          : "text-on-surface-variant hover:text-on-surface"
                      }`}
                    >
                      <span className="truncate">{getMonitorDisplayName(monitor)}</span>
                      {/* 编辑图标悬浮覆盖在标签右缘，不占布局宽度 */}
                      <span
                        role="button"
                        data-name="monitor-rename-trigger"
                        title="重命名显示器"
                        onClick={(e) => {
                          e.stopPropagation();
                          setEditingMonitorKey(nameKey);
                        }}
                        className="absolute right-0.5 top-1/2 -translate-y-1/2 w-5 h-5 rounded-full bg-surface-container shadow-sm flex items-center justify-center text-on-surface-variant opacity-0 group-hover:opacity-100 hover:!opacity-100 hover:!text-on-surface transition-opacity"
                      >
                        <Icon name="edit" className="text-[13px]" />
                      </span>
                    </button>
                  )}
                </div>
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

          <button
            data-name="icc-effect-preview-button"
            onClick={() => void handleCaptureIccPreview()}
            disabled={capturingIccPreview}
            className="w-8 h-8 rounded-full flex items-center justify-center text-on-surface-variant hover:bg-primary/10 hover:text-primary transition-colors disabled:opacity-50"
            title={capturingIccPreview ? "正在生成模拟效果图" : "生成当前显示效果模拟图"}
            aria-label="生成当前显示效果模拟图"
          >
            <Icon name="photo_camera" className="text-[20px]" />
          </button>

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

          <AccountMenu
            session={session}
            onOpenLogin={() => setShowAuthModal(true)}
            onOpenMyInfo={() => setShowMyInfoModal(true)}
            onOpenSettings={() => setShowSettingsModal(true)}
            onLogout={() => void handleAuthLogout()}
          />

        </div>
      </header>

      {/* 调试记录中：常驻状态条（点击打开控制弹窗）。
          交互状态全集：hover 抬升+提亮、active 按压、focus-visible 环、
          脉冲点传达「记录中」状态，reduced-motion 下全部静态化 */}
      {debugActive && (
        <button
          data-name="debug-chip"
          onClick={() => setShowDebugModal(true)}
          title="点击停止记录并生成报告"
          className="fixed bottom-4 right-4 z-[200] flex items-center gap-xs pl-md pr-sm py-sm rounded-full bg-error text-on-error font-label-md text-label-md shadow-lg ring-1 ring-error/40 cursor-pointer select-none transition-all duration-200 ease-out hover:shadow-xl hover:-translate-y-0.5 hover:brightness-110 active:translate-y-0 active:scale-95 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-error focus-visible:ring-offset-2 motion-reduce:transition-none"
        >
          <span className="w-2 h-2 rounded-full bg-white animate-pulse motion-reduce:animate-none" />
          调试记录中
          <Icon name="expand_more" className="text-[16px] rotate-180 opacity-80" />
        </button>
      )}

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
              currentMonitorName={activeMonitor ? getMonitorDisplayName(activeMonitor) : undefined}
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
              showToast={showToast}
              dvcCapability={dvcCap}
            />
          </div>
          <div data-name="preview-config-panel" className="col-span-4 flex flex-col min-h-0 h-full overflow-hidden gap-gutter">
            <DisplayResolution
              showToast={showToast}
              deviceId={selectedDeviceId}
              deviceName={activeMonitor ? getMonitorDisplayName(activeMonitor) : undefined}
              onCurrentResolution={handleCurrentResolutionReport}
              refreshKey={resRefreshKey}
              // 保存范围开关与保存路径按同一可见性收敛：入口被隐藏时开关一并消失，
              // 更新已有方案保留原 resolution 字段、新建方案不携带分辨率
              includeResolution={resolutionSaveControlVisible ? includeResolution : undefined}
              onIncludeResolutionChange={
                resolutionSaveControlVisible ? setIncludeResolution : undefined
              }
            />
            <PreviewImage showToast={showToast} />
            {/* 应用方案的部分成功/失败说明：分辨率被兼容跳过、颜色已应用或失败。
                role="status" 让读屏软件播报变化；持续显示至下一次应用/切换，不积累 */}
            {presetStatusNote && (
              <div
                role="status"
                data-name="preset-status-note"
                className="shrink-0 rounded-lg border border-outline-variant/20 bg-surface-container px-md py-sm"
              >
                <p className="font-body-sm text-body-sm text-on-surface-variant leading-snug">
                  {presetStatusNote}
                </p>
              </div>
            )}
            <ConfigManager
              configs={configs}
              selectedConfig={selectedConfig}
              onConfigLoad={handleConfigLoad}
              onConfigsChange={refreshConfigs}
              showToast={showToast}
              shareEnabled={featureFlags.isEnabled(FEATURE_PLAN_SHARE)}
              onShareConfig={(name) => setShareConfigName(name)}
              onImportShare={() => openImportShare()}
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
        monitors={monitors}
        monitorNames={monitorNames}
        onRenameMonitor={setMonitorName}
        onShowAbout={() => {
          setShowSettingsModal(false);
          setShowAboutModal(true);
        }}
        onApiEnvChanged={() => void featureFlags.refresh()}
      />

      <AuthModal
        open={showAuthModal}
        onClose={() => setShowAuthModal(false)}
        showToast={showToast}
        onLoggedIn={handleAuthLoggedIn}
      />

      {session ? (
        <MyInfoModal
          open={showMyInfoModal}
          onClose={() => setShowMyInfoModal(false)}
          session={session}
          onOpenSettings={() => setShowSettingsModal(true)}
        />
      ) : null}

      <SharePlanModal
        open={shareConfigName !== null}
        config={shareConfig}
        onClose={() => setShareConfigName(null)}
        showToast={showToast}
        onShared={(slug) => handledShareSlugsRef.current.add(slug)}
        onCaptureEffect={async (config) => {
          // 用**被分享方案**的参数渲染，而不是当前屏幕正在用的参数；
          // 分享时不应该顺带改动顶栏预览状态或磁盘上的预览图。
          try {
            return await renderSimulatedPreview({
              iccProfile: config.icc_profile ?? null,
              brightness: config.brightness,
              contrast: config.contrast,
              gamma: config.gamma,
              digitalVibrance: config.digital_vibrance,
              rgbR: config.rgb_r ?? 0,
              rgbG: config.rgb_g ?? 0,
              rgbB: config.rgb_b ?? 0,
            });
          } catch (err) {
            showToast("error", err instanceof Error ? err.message : String(err));
            return null;
          }
        }}
      />

      <ImportShareModal
        open={importShareOpen}
        initialSlug={importShareSlug}
        deviceId={selectedDeviceId}
        onClose={() => {
          if (importShareSlug) {
            // 本会话不再为它弹
            handledShareSlugsRef.current.add(importShareSlug);
            // 没导入就关（取消/×/Esc）→ 持久忽略，重启也不弹；导入过则清除
            try {
              const dismissed = JSON.parse(
                localStorage.getItem("dismissedShareSlugs") ?? "[]",
              );
              const list = Array.isArray(dismissed) ? dismissed : [];
              const next = importedFromShareRef.current
                ? list.filter((s) => s !== importShareSlug)
                : list.includes(importShareSlug)
                  ? list
                  : [...list, importShareSlug].slice(-50);
              localStorage.setItem("dismissedShareSlugs", JSON.stringify(next));
            } catch {
              /* localStorage 不可用就算了 */
            }
          }
          importedFromShareRef.current = false;
          setImportShareOpen(false);
        }}
        showToast={showToast}
        onImported={(name) => {
          importedFromShareRef.current = true;
          handledShareSlugsRef.current.add(importShareSlug ?? "");
          void refreshConfigs();
          setSelectedConfig(name);
        }}
      />

      <ClosePromptModal
        open={showClosePrompt}
        initialCloseToTray={closePromptInitialTray}
        onSelect={handleClosePromptSelect}
      />

      <IccEffectPreviewModal
        open={iccEffectPreviewOpen}
        imageBase64={iccEffectPreview}
        profileName={activeProfile}
        savedPath={iccEffectPreviewPath}
        onOpenFolder={async () => {
          try {
            await invoke("open_icc_effect_preview_directory");
          } catch (err) {
            showToast("error", `打开效果图目录失败：${String(err)}`);
          }
        }}
        onClose={() => setIccEffectPreviewOpen(false)}
      />
    </div>
  );
}

export default App;
