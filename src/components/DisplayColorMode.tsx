import { useEffect, useRef, useState } from "react";
import { invoke } from "../lib/ipc";
import type { DisplayColorBackend, DisplayColorStatus } from "../lib/displayColor";
import { Icon } from "./Icon";
import { useModalAccessibility } from "../hooks/useModalAccessibility";

interface DisplayColorModeProps {
  enabled: boolean;
  deviceId?: string;
  status: DisplayColorStatus | null;
  error: string | null;
  querying: boolean;
  busy: boolean;
  monitorName?: string;
  onSwitch: (backend: DisplayColorBackend) => Promise<boolean>;
  onRefresh: () => Promise<void>;
  showToast: (type: "success" | "error", text: string) => void;
}

export default function DisplayColorMode({
  enabled, deviceId, status, error, querying, busy, monitorName, onSwitch, onRefresh, showToast,
}: DisplayColorModeProps) {
  const [promptFor, setPromptFor] = useState<string | null>(null);
  const [openingDevices, setOpeningDevices] = useState(false);
  const prompted = useRef(new Set<string>());
  const currentKey = status ? `${status.stable_key || status.device_id}:${status.monitor_state}` : null;
  const canTryNvidia = enabled && !!status && status.gpu_vendor === "nvidia" && status.nvidia_support !== "unavailable";
  const promptOpen = enabled && !!promptFor && promptFor === currentKey && status?.monitor_state === "disabled";
  const dialog = useModalAccessibility(promptOpen, () => setPromptFor(null), true, !busy);

  useEffect(() => {
    if (status?.monitor_state === "enabled") {
      prompted.current.delete(`${status.stable_key || status.device_id}:disabled`);
    }
    if (!enabled || !status || status.monitor_state !== "disabled" || !currentKey) {
      setPromptFor(null);
      return;
    }
    if (!prompted.current.has(currentKey)) {
      prompted.current.add(currentKey);
      setPromptFor(currentKey);
    }
  }, [enabled, currentKey, status]);

  if (!enabled && status?.active_backend !== "nvidia") return null;

  const handleSwitch = async (backend: DisplayColorBackend) => {
    if (busy) return;
    try {
      const switched = await onSwitch(backend);
      if (!switched) return;
      setPromptFor(null);
      showToast("success", backend === "nvidia"
        ? "NVIDIA 曲线已写入，请观察实际画面；可随时返回标准模式"
        : "标准模式颜色已写入");
    } catch (err) {
      showToast("error", `调色模式设置失败：${String(err)}`);
    }
  };

  const openDeviceManager = async () => {
    if (openingDevices) return;
    setOpeningDevices(true);
    try {
      await invoke("open_display_device_manager");
    } catch (err) {
      showToast("error", `打开设备管理器失败：${String(err)}`);
    } finally {
      setOpeningDevices(false);
    }
  };

  const buttonStyle = "rounded-md px-sm py-sm text-[12px] font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary disabled:opacity-40 disabled:cursor-not-allowed";

  // 开关关闭后若驱动清理失败，保留唯一的恢复入口；不再展示 B 的启用操作。
  if (!enabled) return (
    <section
      data-component="DisplayColorMode"
      data-name="nvidia-color-emergency-recovery"
      aria-label="恢复标准调色模式"
      aria-busy={busy || querying}
      className="mb-md rounded-lg border border-error/30 bg-error/5 p-md"
    >
      <h4 className="font-label-md text-label-md text-on-surface">需要恢复标准模式</h4>
      <p className="mt-sm text-[12px] leading-relaxed text-on-surface-variant">NVIDIA 专用模式入口已关闭，但当前颜色状态尚未恢复。请返回标准模式后继续使用。</p>
      {status?.reason && <p role="status" className="mt-sm text-[11px] leading-relaxed text-error break-words">{status.reason}</p>}
      {error && <p role="alert" className="mt-sm text-[11px] leading-relaxed text-error break-words">{error}</p>}
      <button
        data-name="emergency-standard-color-mode"
        type="button"
        disabled={busy || !deviceId}
        onClick={() => void handleSwitch("standard")}
        className={`${buttonStyle} mt-md bg-primary text-on-primary hover:brightness-110`}
      >{busy ? "正在恢复…" : "返回标准模式"}</button>
    </section>
  );

  return (
    <section
      data-component="DisplayColorMode"
      data-name="display-color-mode"
      aria-label="当前显示器调色模式"
      aria-busy={busy || querying}
      className="mb-md rounded-lg border border-outline-variant/25 bg-surface-container-high/40 p-md"
    >
      <div className="flex items-center justify-between gap-sm mb-sm">
        <h4 className="font-label-md text-label-md text-on-surface">调色模式</h4>
        <button
          data-name="refresh-color-status"
          type="button"
          onClick={() => void onRefresh()}
          disabled={busy || querying}
          className={`${buttonStyle} text-on-surface-variant hover:bg-surface-variant/50`}
          aria-label="重新检查显示器调色状态"
          title="重新检查显示器状态"
        >
          <Icon name="refresh" className="text-[16px]" />
        </button>
      </div>
      <div role="group" aria-label="选择调色模式" className="grid grid-cols-2 gap-xs rounded-lg bg-surface-container-highest/50 p-xs">
        <button
          data-name="standard-color-mode"
          type="button"
          aria-pressed={status?.active_backend === "standard"}
          disabled={busy || !deviceId}
          onClick={() => void handleSwitch("standard")}
          className={`${buttonStyle} ${status?.active_backend === "standard" ? "bg-primary/15 text-primary" : "text-on-surface-variant hover:bg-surface-variant/50"}`}
        >标准模式</button>
        <button
          data-name="nvidia-color-mode"
          type="button"
          aria-pressed={status?.active_backend === "nvidia"}
          disabled={busy || querying || !canTryNvidia}
          onClick={() => void handleSwitch("nvidia")}
          className={`${buttonStyle} ${status?.active_backend === "nvidia" ? "bg-primary/15 text-primary" : "text-on-surface-variant hover:bg-surface-variant/50"}`}
        >NVIDIA 专用<span className="block mt-xs text-[10px] font-normal">实验性</span></button>
      </div>
      <div role="status" className="mt-sm text-[11px] leading-relaxed text-on-surface-variant break-words">
        {!status && !error && <p>{querying ? "正在检查当前显示器…" : "等待目标显示器就绪"}</p>}
        {status && <>
          <p data-name="monitor-device-state">
            监视器设备：{status.monitor_state === "enabled" ? "已启用" : status.monitor_state === "disabled" ? "已禁用" : "状态待确认"}
            {status.gpu_vendor === "nvidia" && " · NVIDIA 输出"}
          </p>
          <p className="mt-xs">
            {status.active_backend === "nvidia"
              ? "使用 NVIDIA 专用曲线复现已选 ICC 和现有滑块，实际效果请观察画面。"
              : "继续使用当前 ICC 和标准调色流程。"}
          </p>
          {status.nvidia_support === "unverified" && <p className="mt-xs">NVIDIA 模式可尝试，当前驱动的实际效果尚未验证。</p>}
          {status.preferred_backend !== status.active_backend && <p className="mt-xs">已保存的调色模式当前未启用，请查看下方原因。</p>}
          {status.reason && <p data-name="color-mode-reason" className="mt-xs">{status.reason}</p>}
          {status.warnings.map((warning, index) => <p key={`${index}:${warning}`} className="mt-xs">{warning}</p>)}
        </>}
        {error && <p data-name="color-mode-error" className="mt-xs text-error">{error}</p>}
      </div>
      {status && <details className="mt-sm text-[11px] leading-relaxed text-on-surface-variant">
        <summary className="cursor-pointer text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary">参考模式与恢复说明</summary>
        <p className="mt-sm">标准模式建议关闭 NVIDIA 控制面板的“覆盖参考模式”。NVIDIA 专用模式不会自动改这个开关，可分别测试开启和关闭后的画面，再重新应用颜色。</p>
        <p className="mt-xs">参考模式可能使控制面板的颜色滑块置灰。本软件的滑块通过专用曲线工作，是否生效取决于当前驱动。</p>
        <p className="mt-xs">模式只保存到本机这台显示器。重置默认仍使用软件保存的默认方案，没有默认方案时恢复系统 sRGB 与中性调节。</p>
        <button
          data-name="reapply-current-color-mode"
          type="button"
          disabled={busy || querying}
          onClick={() => void handleSwitch(status.active_backend)}
          className={`${buttonStyle} mt-sm border border-outline-variant/30 text-primary hover:bg-primary/10`}
        >重新应用当前颜色</button>
      </details>}

      {promptOpen && <div data-component="DisabledMonitorPrompt" className="fixed inset-0 z-[100] flex items-center justify-center p-md">
        <div className="absolute inset-0 bg-black/60" />
        <div
          ref={dialog}
          role="dialog"
          aria-modal="true"
          aria-labelledby="disabled-monitor-title"
          aria-describedby="disabled-monitor-description"
          tabIndex={-1}
          data-name="disabled-monitor-dialog"
          className="relative z-[110] w-full max-w-lg rounded-xl border border-outline-variant/30 bg-surface-container p-lg shadow-xl"
        >
          <h2 id="disabled-monitor-title" className="font-headline-sm text-headline-sm text-on-surface">当前监视器设备已禁用</h2>
          <p id="disabled-monitor-description" className="mt-md font-body-sm text-body-sm text-on-surface-variant leading-relaxed">
            {monitorName || "当前显示器"} 的监视器设备已被禁用，标准 ICC 调色可能受到影响。可以恢复启用设备后继续标准模式，或尝试 NVIDIA 专用模式复现当前滤镜。
          </p>
          <p className="mt-sm font-body-sm text-body-sm text-on-surface-variant leading-relaxed">在设备管理器中展开“监视器”，确认对应设备后右键选择“启用设备”。软件不会自动启用它。</p>
          {!canTryNvidia && <p className="mt-sm text-[12px] text-error">{status?.reason || "当前目标显示器暂不支持 NVIDIA 专用模式。"}</p>}
          {error && <p role="alert" className="mt-sm text-[12px] text-error break-words">{error}</p>}
          <div className="mt-lg flex flex-wrap justify-end gap-sm">
            <button data-name="view-monitor-recovery" type="button" disabled={openingDevices || busy} onClick={() => void openDeviceManager()} className={`${buttonStyle} border border-outline-variant/40 text-on-surface-variant hover:bg-surface-variant/50`}>{openingDevices ? "正在打开…" : "查看恢复方法"}</button>
            <button data-name="continue-current-color-mode" type="button" disabled={busy} onClick={() => setPromptFor(null)} className={`${buttonStyle} text-on-surface-variant hover:bg-surface-variant/50`}>继续当前模式</button>
            <button data-name="try-nvidia-color-mode" type="button" disabled={busy || !canTryNvidia} onClick={() => void handleSwitch("nvidia")} className={`${buttonStyle} bg-primary text-on-primary hover:brightness-110`}>{busy ? "正在切换…" : "尝试 NVIDIA 模式"}</button>
          </div>
        </div>
      </div>}
    </section>
  );
}
