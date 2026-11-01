import { useState, useEffect, useCallback } from "react";
import { invoke } from "../lib/ipc";
import Select from "./Select";

/** 对应后端 icc::DisplayModeList / DisplayModeInfo */
interface DisplayModeInfo {
  width: number;
  height: number;
  /** 该分辨率下全部可用刷新率，降序 */
  refresh_rates: number[];
  is_current: boolean;
}

interface DisplayModeList {
  modes: DisplayModeInfo[];
  current: { width: number; height: number; refresh_rate: number } | null;
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

/** 估算一批选项标签需要的最小宽度：中文按 12px、西文按 7px 计字宽，
 *  加上 Select 的左右内边距/图标/间隙余量。
 *  Select 内部 span 带 truncate，外层无法靠内容自然撑开，只能显式给宽。 */
function estimateSelectWidth(labels: string[]): string {
  const maxTextWidth = Math.max(
    ...labels.map((l) =>
      [...l].reduce((acc, ch) => acc + (ch.charCodeAt(0) > 0x2e80 ? 12 : 7), 0)
    )
  );
  return `${Math.ceil(maxTextWidth) + 64}px`;
}

interface DisplayResolutionProps {
  showToast: (type: "success" | "error", text: string) => void;
  /** 当前选中显示器的适配器设备名（\\.\DISPLAYn），分辨率切换作用于它 */
  deviceId?: string;
  /** 显示器型号名，用于控件的悬浮提示 */
  deviceName?: string;
  /** 上报当前实际模式（保存方案时写入；App 侧在启动/加载方案等采纳点用作分辨率基线），null=未知 */
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
  const [current, setCurrent] = useState<DisplayModeList["current"]>(null);
  const [applying, setApplying] = useState(false);
  const [ratioKey, setRatioKey] = useState<string>(() => {
    return localStorage.getItem("resolutionRatioFilter") || "all";
  });

  const loadModes = useCallback(async () => {
    if (!deviceId) {
      setModes([]);
      setCurrent(null);
      onCurrentResolution(null);
      return;
    }
    try {
      const list = await invoke<DisplayModeList>("get_display_modes", { deviceId });
      setModes(list.modes);
      setCurrent(list.current);
      onCurrentResolution(
        list.current ? `${list.current.width}x${list.current.height}@${list.current.refresh_rate}` : null
      );
    } catch (err) {
      console.error("Failed to load display modes:", err);
      setModes([]);
      setCurrent(null);
      onCurrentResolution(null);
    }
    // onCurrentResolution 的闭包只引用稳定项（ref 与 setState），无需进依赖
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
    { value: "all", label: "全部" },
    ...orderedRatios.map((k) => ({ value: k, label: k })),
  ];

  const filteredModes =
    ratioKey === "all" ? modes : modes.filter((m) => classifyRatio(m.width, m.height) === ratioKey);
  const resolutionOptions = filteredModes.map((m) => ({
    value: `${m.width}x${m.height}`,
    label: `${m.width} × ${m.height}`,
  }));

  // 刷新率选项跟随当前分辨率
  const refreshOptions = (currentMode?.refresh_rates ?? []).map((r) => ({
    value: String(r),
    label: `${r} Hz`,
  }));

  const handleRatioChange = (value: string) => {
    setRatioKey(value);
    localStorage.setItem("resolutionRatioFilter", value);
  };

  /** 切分辨率：不带 @Hz，刷新率由后端择优（优先保持当前值） */
  const handleResolutionChange = async (value: string) => {
    const [width, height] = value.split("x").map(Number);
    if (!width || !height || !deviceId) return;
    if (current && current.width === width && current.height === height) return;
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

  /** 切刷新率：当前分辨率下精确指定 Hz */
  const handleRefreshRateChange = async (value: string) => {
    const hz = Number(value);
    if (!hz || !current) return;
    if (current.refresh_rate === hz) return;
    setApplying(true);
    try {
      await invoke("apply_config_resolution", {
        deviceId,
        resolution: `${current.width}x${current.height}@${hz}`,
      });
      showToast("success", `刷新率已切换为 ${hz} Hz`);
    } catch (err) {
      showToast("error", `刷新率切换失败: ${err}`);
    } finally {
      setApplying(false);
      loadModes();
    }
  };

  return (
    <div data-components="DisplayResolution" className="shrink-0">
      <div className="mb-md flex justify-between items-end shrink-0">
        <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface font-medium">分辨率调整</h3>
        {current && (
          <span className="text-[11px] text-on-surface-variant" title={deviceName}>
            当前 {current.width} × {current.height} @ {current.refresh_rate} Hz
          </span>
        )}
      </div>
      <div className="flex gap-sm" title={`调整${deviceName ? ` ${deviceName}` : "当前显示器"}的屏幕分辨率与刷新率`}>
        {/* 比例/刷新率按最长选项文本给定最小宽度（Select 内部 truncate 撑不开外层），
            空间不足时优先压缩中间的分辨率列 */}
        <div className="shrink-0" style={{ minWidth: estimateSelectWidth(ratioOptions.map((o) => o.label)) }}>
          <Select value={ratioKey} onChange={handleRatioChange} options={ratioOptions} placeholder="比例" />
        </div>
        <div className={`flex-1 min-w-0 ${applying ? "pointer-events-none opacity-50" : ""}`}>
          <Select
            value={current ? `${current.width}x${current.height}` : ""}
            onChange={handleResolutionChange}
            options={resolutionOptions}
            placeholder="分辨率"
          />
        </div>
        <div
          className="shrink-0"
          style={{
            minWidth: estimateSelectWidth([
              ...(currentMode?.refresh_rates ?? []).map((r) => `${r} Hz`),
              "刷新率",
            ]),
          }}
        >
          <Select
            value={current ? String(current.refresh_rate) : ""}
            onChange={handleRefreshRateChange}
            options={refreshOptions}
            placeholder="刷新率"
          />
        </div>
      </div>
    </div>
  );
}

export default DisplayResolution;
