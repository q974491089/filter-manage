import { useState, useRef, useEffect, useCallback } from "react";
import { invoke, convertFileSrc } from "../lib/ipc";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { Icon } from "./Icon";
import Select from "./Select";

/** 对应后端 icc::DisplayModeInfo */
interface DisplayModeInfo {
  width: number;
  height: number;
  refresh_rate: number;
  is_current: boolean;
}

interface PreviewImageProps {
  showToast: (type: "success" | "error", text: string) => void;
  /** 当前选中显示器的适配器设备名（\\.\DISPLAYn），分辨率切换作用于它 */
  deviceId?: string;
  /** 显示器型号名，用于分辨率控件的悬浮提示 */
  deviceName?: string;
}

function PreviewImage({ showToast, deviceId, deviceName }: PreviewImageProps) {
  const [imageSrc, setImageSrc] = useState<string | null>(null);
  const [imageName, setImageName] = useState<string>("");
  const [dragOver, setDragOver] = useState(false);
  const [loading, setLoading] = useState(false);
  const [modes, setModes] = useState<DisplayModeInfo[]>([]);
  const [applyingRes, setApplyingRes] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);

  const loadModes = useCallback(async () => {
    if (!deviceId) {
      setModes([]);
      return;
    }
    try {
      const list = await invoke<DisplayModeInfo[]>("get_display_modes", { deviceId });
      setModes(list);
    } catch (err) {
      console.error("Failed to load display modes:", err);
      setModes([]);
    }
  }, [deviceId]);

  useEffect(() => {
    loadModes();
  }, [loadModes]);

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setDragOver(true);
      } else if (event.payload.type === "drop") {
        setDragOver(false);
        const paths = event.payload.paths;
        console.log("[PreviewImage] dropped paths:", paths);
        const imagePath = paths.find((p) =>
          /\.(jpg|jpeg|png|webp|bmp|gif)$/i.test(p)
        );
        if (imagePath) {
          handleImagePath(imagePath);
        } else {
          showToast("error", "请拖入图片文件 (JPG/PNG/WEBP/BMP/GIF)");
        }
      } else {
        setDragOver(false);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const handleImagePath = async (path: string) => {
    setLoading(true);
    try {
      const validPath = await invoke<string>("set_preview_image", {
        imagePath: path,
      });
      setImageSrc(convertFileSrc(validPath));
      setImageName(path.split(/[/\\]/).pop() || "");
    } catch (err) {
      showToast("error", `图片加载失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleSelect = async () => {
    try {
      const selected = await open({
        filters: [
          {
            name: "图片",
            extensions: ["jpg", "jpeg", "png", "webp", "bmp", "gif"],
          },
        ],
        multiple: false,
      });
      if (!selected) return;
      setLoading(true);
      const validPath = await invoke<string>("set_preview_image", {
        imagePath: selected as string,
      });
      setImageSrc(convertFileSrc(validPath));
      const name = (selected as string).split(/[/\\]/).pop() || "";
      setImageName(name);
    } catch (err) {
      showToast("error", `图片加载失败: ${err}`);
    } finally {
      setLoading(false);
    }
  };

  const handleClear = () => {
    setImageSrc(null);
    setImageName("");
  };

  const currentMode = modes.find((m) => m.is_current);
  const resolutionOptions = modes.map((m) => ({
    value: `${m.width}x${m.height}`,
    label: `${m.width} × ${m.height}`,
  }));

  const handleResolutionChange = async (value: string) => {
    const [width, height] = value.split("x").map(Number);
    if (!width || !height || !deviceId) return;
    if (currentMode && currentMode.width === width && currentMode.height === height) return;
    setApplyingRes(true);
    try {
      await invoke("set_display_mode", { deviceId, width, height });
      showToast("success", `分辨率已切换为 ${width} × ${height}`);
    } catch (err) {
      showToast("error", `分辨率切换失败: ${err}`);
    } finally {
      setApplyingRes(false);
      loadModes();
    }
  };

  return (
    <div data-components="PreviewImage" className="flex-1 flex flex-col min-h-[140px]">
      <div className="mb-md flex justify-between items-end shrink-0 gap-sm">
        <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface font-medium">效果预览</h3>
        <div className="flex items-center gap-sm shrink-0">
          {resolutionOptions.length > 1 && (
            <div
              className={`w-[140px] ${applyingRes ? "pointer-events-none opacity-50" : ""}`}
              title={`调整${deviceName ? ` ${deviceName}` : "当前显示器"}的屏幕分辨率`}
            >
              <Select
                value={currentMode ? `${currentMode.width}x${currentMode.height}` : ""}
                onChange={handleResolutionChange}
                options={resolutionOptions}
                placeholder="分辨率"
              />
            </div>
          )}
          {imageSrc && (
            <button
              onClick={handleClear}
              className="text-primary text-[12px] font-medium hover:underline"
            >
              清除
            </button>
          )}
        </div>
      </div>

      <div
        data-name="preview-area"
        className={`flex-1 rounded-lg border-2 border-dashed transition-all flex flex-col items-center justify-center relative overflow-hidden group cursor-pointer ${
          dragOver
            ? "border-primary bg-primary/10"
            : "border-outline-variant/40 bg-surface-container hover:border-primary/50 hover:bg-primary/5"
        }`}
        onClick={!imageSrc ? handleSelect : undefined}
      >
        {imageSrc ? (
          <>
            <img
              src={imageSrc}
              alt={imageName}
              className="w-full h-full object-cover rounded-md"
            />
            <div className="absolute bottom-0 left-0 right-0 bg-gradient-to-t from-black/70 to-transparent p-md">
              <p className="text-white text-label-md font-label-md truncate">{imageName}</p>
            </div>
            {/* Click to replace */}
            <div
              className="absolute inset-0 bg-black/0 hover:bg-black/25 transition-colors flex items-center justify-center opacity-0 hover:opacity-100"
              onClick={(e) => {
                e.stopPropagation();
                handleSelect();
              }}
            >
              <div className="bg-primary/90 px-md py-sm rounded-md flex items-center gap-xs shadow-lg">
                <Icon name="edit" className="text-white text-[18px]" />
                <span className="text-white text-label-md font-label-md">更换图片</span>
              </div>
            </div>
          </>
        ) : (
          <div className="z-10 text-center space-y-md">
            <div className="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center mx-auto mb-md group-hover:scale-110 transition-transform border border-primary/20">
              {loading ? (
                <svg className="animate-spin w-8 h-8 text-primary" fill="none" viewBox="0 0 24 24">
                  <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                  <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
                </svg>
              ) : (
                <Icon name="add_photo_alternate" className="text-primary text-[32px]" />
              )}
            </div>
            <p className="font-label-md text-label-md text-on-surface">
              {dragOver ? "释放以加载图片" : "拖拽图片到此处或点击加载"}
            </p>
            <p className="text-[11px] text-on-surface-variant">
              支持 JPG, PNG, WEBP, BMP, GIF 格式
            </p>
          </div>
        )}
        <div className="absolute inset-0 bg-gradient-to-br from-primary/3 to-transparent pointer-events-none"></div>
      </div>

      <input
        ref={inputRef}
        type="file"
        accept=".jpg,.jpeg,.png,.webp,.bmp,.gif"
        className="hidden"
        onChange={(e) => {
          const file = e.target.files?.[0];
          if (file) {
            const path = (file as any).path;
            if (path) handleImagePath(path);
          }
          e.target.value = "";
        }}
      />

      <button
        data-name="upload-button"
        onClick={handleSelect}
        className="mt-md w-full py-md rounded-lg bg-surface-container-high border border-outline-variant/20 font-label-md text-label-md text-on-surface hover:bg-surface-variant transition-all"
      >
        选择参考图片...
      </button>
    </div>
  );
}

export default PreviewImage;
