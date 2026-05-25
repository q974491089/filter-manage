import { openUrl } from "@tauri-apps/plugin-opener";

interface AboutModalProps {
  open: boolean;
  onClose: () => void;
}

function AboutModal({ open, onClose }: AboutModalProps) {
  if (!open) return null;

  const handleOpenWebsite = () => {
    openUrl("https://filter-manage.6ya.site/");
  };

  return (
    <div
      data-name="about-overlay"
      className="fixed inset-0 z-[100] flex items-center justify-center bg-black/50 backdrop-blur-sm"
      onClick={onClose}
    >
      <div
        data-name="about-modal"
        onClick={(e) => e.stopPropagation()}
        className="bg-surface-container rounded-2xl shadow-2xl border border-outline-variant/20 w-[420px] max-w-[90vw] overflow-hidden"
      >
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
