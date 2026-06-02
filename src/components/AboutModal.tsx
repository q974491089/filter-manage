import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

interface AboutModalProps {
  open: boolean;
  onClose: () => void;
  onCheckUpdate: () => Promise<"available" | "latest" | "error">;
}

function AboutModal({ open, onClose, onCheckUpdate }: AboutModalProps) {
  const [checking, setChecking] = useState(false);
  const [checkError, setCheckError] = useState<string | null>(null);
  const [toastMsg, setToastMsg] = useState<string | null>(null);
  if (!open) return null;

  const handleOpenWebsite = () => {
    openUrl("https://filter-manage.6ya.site/");
  };

  const handleCheckUpdate = async () => {
    setChecking(true);
    setCheckError(null);
    setToastMsg(null);
    const result = await onCheckUpdate();
    if (result === "error") {
      setCheckError("检查失败，请检查网络后重试");
    } else if (result === "latest") {
      setCheckError(null);
      setToastMsg("已是最新版本");
      setTimeout(() => setToastMsg(null), 3000);
    }
    setChecking(false);
  };

  return (
    <div
      data-name="about-overlay"
      className="fixed inset-0 z-[300] flex items-center justify-center bg-black/50 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        data-name="about-modal"
        onClick={(e) => e.stopPropagation()}
        className="bg-surface-container rounded-2xl shadow-2xl border border-outline-variant/20 w-[420px] max-w-[90vw] overflow-hidden relative"
      >
        {/* Toast */}
        {toastMsg && (
          <div className="absolute top-4 left-1/2 -translate-x-1/2 z-10 px-md py-sm rounded-md bg-primary/90 text-on-primary font-label-md text-label-md shadow-lg backdrop-blur-sm">
            {toastMsg}
          </div>
        )}
        {/* Header with icon */}
        <div className="flex flex-col items-center pt-8 pb-4">
          <img src="/favicon.png" alt="icon" className="w-16 h-16 rounded-2xl shadow-lg mb-4" />
          <h2 className="font-headline-lg text-headline-lg font-medium text-on-surface">Filter Manage</h2>
          <span className="font-label-md text-label-md text-on-surface-variant/60 mt-1">v{__APP_VERSION__}</span>
        </div>

        {/* Divider */}
        <div className="mx-6 border-t border-outline-variant/20" />

        {/* Description */}
        <div className="px-6 py-5">
          <p className="font-body-md text-body-md text-on-surface-variant leading-relaxed text-center">
            一站式显示器色彩管理工具。支持 ICC 色彩配置文件切换、NVIDIA 显卡亮度/对比度/伽马/数字振动精细调节，多显示器独立配置，方案保存与快速切换。
          </p>
        </div>

        {/* Actions */}
        <div className="flex flex-col gap-2 px-6 pb-6">
          <button
            data-name="website-button"
            onClick={handleOpenWebsite}
            className="w-full flex items-center justify-center gap-2 px-4 py-3 rounded-xl bg-primary/10 text-primary font-label-md text-label-md hover:bg-primary/20 transition-colors"
          >
            <span className="material-symbols-outlined text-[18px]">language</span>
            访问官网
          </button>
          <button
            data-name="check-update-button"
            onClick={handleCheckUpdate}
            disabled={checking}
            className="w-full flex items-center justify-center gap-2 px-4 py-3 rounded-xl bg-surface-variant/50 text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/80 transition-colors disabled:opacity-50"
          >
            {checking ? (
              <svg className="animate-spin w-4 h-4" fill="none" viewBox="0 0 24 24">
                <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
                <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
            ) : (
              <span className="material-symbols-outlined text-[18px]">system_update</span>
            )}
            {checking ? "检查中..." : "检查更新"}
          </button>
          {checkError && (
            <p className="text-center text-error text-label-sm font-label-sm">{checkError}</p>
          )}
          <button
            data-name="close-about-button"
            onClick={onClose}
            className="w-full px-4 py-3 rounded-xl bg-surface-variant/50 text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/80 transition-colors"
          >
            关闭
          </button>
        </div>
      </div>
    </div>
  );
}

export default AboutModal;
