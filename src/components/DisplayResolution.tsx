import { useState, useEffect, useCallback } from "react";
import { invoke } from "../lib/ipc";
import Select from "./Select";

/** 对应后端 icc::DisplayModeInfo */
interface DisplayModeInfo {
  width: number;
  height: number;
  refresh_rate: number;
  is_current: boolean;
}

/** 比例分组：宽高比落在基准值 2% 以内即归入该组 */
const RATIO_GROUPS = [
  { key: "16:9", value: 16 / 9 },
  { key: "16:10", value: 16 / 10 },
  { key: "4:3", value: 4 / 3 },
  { key: "5:4", value: 5 / 4 },
  { key: "21:9", value: 21 / 9 },
  { key: "32:9", value: 32 / 9 },
];

function classifyRatio(width: number, height: number): string {
  const r = width / height;
  const hit = RATIO_GROUPS.find((g) => Math.abs(r - g.value) / g.value < 0.02);
  return hit ? hit.key : "其他";
}

interface DisplayResolutionProps {
  showToast: (type: "success" | "error", text: string) => void;
  /** 当前选中显示器的适配器设备名（\\.\DISPLAYn），分辨率切换作用于它 */
  deviceId?: string;
  /** 显示器型号名，用于控件的悬浮提示 */
  deviceName?: string;
  /** 应用成功后向 App 上报当前实际分辨率（保存方案时写入），null=未知 */
  onCurrentResolution: (res: string | null) => void;
  /** 变化时重新拉取模式列表（托盘/快捷键/进程监听应用方案后同步真实状态） */
  refreshKey: number;
}

function DisplayResolution({
  showToast,
  deviceId,
  deviceName,
  onCurrentResolution,
  refreshKey,
}: DisplayResolutionProps) {
  const [modes, setModes] = useState<DisplayModeInfo[]>([]);
  const [applying, setApplying] = useState(false);
  const [ratioKey, setRatioKey] = useState<string>(() => {
    return localStorage.getItem("resolutionRatioFilter") || "all";
  });

  const loadModes = useCallback(async () => {
    if (!deviceId) {
      setModes([]);
      onCurrentResolution(null);
      return;
    }
    try {
      const list = await invoke<DisplayModeInfo[]>("get_display_modes", { deviceId });
      setModes(list);
      const current = list.find((m) => m.is_current);
      onCurrentResolution(current ? `${current.width}x${current.height}` : null);
    } catch (err) {
      console.error("Failed to load display modes:", err);
      setModes([]);
      onCurrentResolution(null);
    }
    // onCurrentResolution 是 App 的 setState，引用稳定
  }, [deviceId, refreshKey]);

  useEffect(() => {
    loadModes();
  }, [loadModes]);

  const currentMode = modes.find((m) => m.is_current);

  // 比例选项 = 「全部」+ 当前列表中实际出现的分组
  const presentRatios = [...new Set(modes.map((m) => classifyRatio(m.width, m.height)))];
  const orderedRatios = [
    ...RATIO_GROUPS.map((g) => g.key).filter((k) => presentRatios.includes(k)),
    ...presentRatios.filter((k) => !RATIO_GROUPS.some((g) => g.key === k)),
  ];
  const ratioOptions = [
    { value: "all", label: "全部比例" },
    ...orderedRatios.map((k) => ({ value: k, label: k })),
  ];

  const filteredModes =
    ratioKey === "all" ? modes : modes.filter((m) => classifyRatio(m.width, m.height) === ratioKey);
  const resolutionOptions = filteredModes.map((m) => ({
    value: `${m.width}x${m.height}`,
    label: `${m.width} × ${m.height}`,
  }));

  const handleRatioChange = (value: string) => {
    setRatioKey(value);
    localStorage.setItem("resolutionRatioFilter", value);
  };

  const handleResolutionChange = async (value: string) => {
    const [width, height] = value.split("x").map(Number);
    if (!width || !height || !deviceId) return;
    if (currentMode && currentMode.width === width && currentMode.height === height) return;
    setApplying(true);
    try {
      // 走方案同款应用路径：登记还原点，之后应用不带分辨率的方案可自动还原
      await invoke("apply_config_resolution", { deviceId, resolution: `${width}x${height}` });
      showToast("success", `分辨率已切换为 ${width} × ${height}`);
    } catch (err) {
      showToast("error", `分辨率切换失败: ${err}`);
    } finally {
      setApplying(false);
      loadModes();
    }
  };

  return (
    <div data-components="DisplayResolution" className="shrink-0">
      <div className="mb-md flex justify-between items-end shrink-0">
        <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface font-medium">分辨率调整</h3>
        {currentMode && (
          <span className="text-[11px] text-on-surface-variant" title={deviceName}>
            当前 {currentMode.width} × {currentMode.height}
          </span>
        )}
      </div>
      <div className="flex gap-sm" title={`调整${deviceName ? ` ${deviceName}` : "当前显示器"}的屏幕分辨率`}>
        <div className="w-[110px] shrink-0">
          <Select value={ratioKey} onChange={handleRatioChange} options={ratioOptions} placeholder="比例" />
        </div>
        <div className={`flex-1 ${applying ? "pointer-events-none opacity-50" : ""}`}>
          <Select
            value={currentMode ? `${currentMode.width}x${currentMode.height}` : ""}
            onChange={handleResolutionChange}
            options={resolutionOptions}
            placeholder="分辨率"
          />
        </div>
      </div>
    </div>
  );
}

export default DisplayResolution;
