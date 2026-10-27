import { useEffect, useState } from "react";
import { useModalAccessibility } from "../hooks/useModalAccessibility";
import { Icon } from "./Icon";

interface IccEffectPreviewModalProps {
  open: boolean;
  imageBase64: string | null;
  profileName: string;
  savedPath: string | null;
  onOpenFolder: () => void;
  onClose: () => void;
}

export default function IccEffectPreviewModal({
  open,
  imageBase64,
  profileName,
  savedPath,
  onOpenFolder,
  onClose,
}: IccEffectPreviewModalProps) {
  const [zoom, setZoom] = useState(100);
  const modalRef = useModalAccessibility(open, onClose);

  useEffect(() => {
    if (open) setZoom(100);
  }, [open, imageBase64]);

  if (!open || !imageBase64) return null;

  return (
    <div
      data-component="IccEffectPreviewModal"
      data-name="backdrop"
      className="fixed inset-0 z-[340] flex items-center justify-center bg-black/80 p-3 backdrop-blur-sm sm:p-6"
      onClick={onClose}
    >
      <section
        ref={modalRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby="icc-effect-preview-title"
        tabIndex={-1}
        onClick={(event) => event.stopPropagation()}
        className="flex h-[92vh] w-[96vw] max-w-[1500px] flex-col overflow-hidden rounded-lg border border-outline-variant/25 bg-surface-container shadow-2xl"
      >
        <header className="flex shrink-0 items-center gap-md border-b border-outline-variant/20 px-md py-sm sm:px-lg">
          <div className="min-w-0 flex-1">
            <h2 id="icc-effect-preview-title" className="font-title-md text-title-md text-on-surface">
              显示效果模拟图
            </h2>
            <p className="mt-0.5 truncate text-xs text-on-surface-variant">
              {profileName === "Default" ? "默认 ICC" : profileName} · 软件模拟，不代表面板实际输出
            </p>
          </div>
          <div className="flex shrink-0 items-center gap-xs">
            <button
              aria-label="缩小"
              title="缩小"
              onClick={() => setZoom((current) => Math.max(25, current - 25))}
              disabled={zoom <= 25}
              className="grid h-9 w-9 place-items-center rounded-md text-on-surface-variant hover:bg-surface-variant/60 disabled:opacity-40"
            ><Icon name="remove" className="text-[18px]" /></button>
            <button
              aria-label="恢复适配大小"
              title="恢复适配大小"
              onClick={() => setZoom(100)}
              className="min-w-12 rounded-md px-1 py-2 text-xs tabular-nums text-on-surface-variant hover:bg-surface-variant/60"
            >{zoom}%</button>
            <button
              aria-label="放大"
              title="放大"
              onClick={() => setZoom((current) => Math.min(300, current + 25))}
              disabled={zoom >= 300}
              className="grid h-9 w-9 place-items-center rounded-md text-on-surface-variant hover:bg-surface-variant/60 disabled:opacity-40"
            ><Icon name="add" className="text-[18px]" /></button>
            <button
              aria-label="关闭"
              title="关闭"
              onClick={onClose}
              className="ml-xs grid h-9 w-9 place-items-center rounded-md text-on-surface-variant hover:bg-surface-variant/60 hover:text-on-surface"
            ><Icon name="close" className="text-[20px]" /></button>
          </div>
        </header>
        <div data-name="image-viewport" className="flex min-h-0 flex-1 items-center justify-center overflow-auto bg-black/35 p-3 sm:p-5">
          <img
            src={`data:image/png;base64,${imageBase64}`}
            alt={`ICC ${profileName} 显示效果模拟图`}
            draggable={false}
            className="h-auto shrink-0 object-contain"
            style={zoom === 100
              ? { maxWidth: "100%", maxHeight: "100%" }
              : { width: `${zoom}%`, maxWidth: "none" }}
          />
        </div>
        <footer className="shrink-0 border-t border-outline-variant/15 px-md py-sm text-[11px] leading-relaxed text-on-surface-variant sm:px-lg">
          <div className="flex flex-wrap items-center justify-between gap-sm">
            <div className="min-w-0 flex-1">
              <p>ICC VCGT、亮度、对比度、Gamma 与 RGB 增益按软件曲线合成；数字振动为近似模拟。显示器硬件 LUT、驱动及面板色彩管理无法通过普通截图精确复现。</p>
              {savedPath && <p className="mt-1 truncate" title={savedPath}>已保存：{savedPath}</p>}
            </div>
            <button
              onClick={onOpenFolder}
              className="flex min-h-9 shrink-0 items-center gap-xs rounded-md border border-outline-variant/30 px-sm text-label-sm text-on-surface transition-colors hover:border-primary/50 hover:text-primary"
            >
              <Icon name="folder_open" className="text-[16px]" />
              打开文件夹
            </button>
          </div>
        </footer>
      </section>
    </div>
  );
}
