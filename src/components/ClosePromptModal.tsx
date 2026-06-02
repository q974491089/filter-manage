import { useEffect, useState } from "react";

interface ClosePromptModalProps {
  open: boolean;
  initialCloseToTray: boolean | null;
  onSelect: (closeToTray: boolean, remember: boolean) => void;
}

function ClosePromptModal({ open, initialCloseToTray, onSelect }: ClosePromptModalProps) {
  const [remember, setRemember] = useState(false);
  const [selected, setSelected] = useState<"minimize" | "exit">("minimize");

  // 每次弹窗打开时，根据当前设置同步选中项；null 时默认"最小化到托盘"
  useEffect(() => {
    if (open) {
      setSelected(initialCloseToTray === false ? "exit" : "minimize");
      setRemember(false);
    }
  }, [open, initialCloseToTray]);

  if (!open) return null;

  const handleConfirm = () => {
    onSelect(selected === "minimize", remember);
  };

  return (
    <div
      data-components="ClosePromptModal"
      className="fixed inset-0 z-[200] flex items-center justify-center"
    >
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />

      <div
        data-name="close-prompt-modal"
        className="relative bg-surface-container/80 backdrop-blur-xl border border-outline-variant/30 rounded-xl shadow-2xl w-full max-w-md mx-4 overflow-hidden"
      >
        {/* Header */}
        <div className="flex items-center justify-between px-6 pt-6 pb-4">
          <h2 className="font-headline-md text-headline-md text-on-surface flex items-center gap-2">
            <span className="material-symbols-outlined text-primary" style={{ fontVariationSettings: "'FILL' 1" }}>power_settings_new</span>
            关闭行为
          </h2>
        </div>

        {/* Content */}
        <div className="px-6 pb-6 flex flex-col gap-4">
          <p className="font-body-md text-body-md text-on-surface-variant leading-relaxed">
            关闭窗口后，应用将继续在系统托盘运行，全局快捷键保持生效。
          </p>

          {/* Options Group */}
          <div className="flex flex-col gap-3 mt-2">
            {/* Option 1: Minimize to Tray */}
            <label className="relative cursor-pointer group">
              <input
                type="radio"
                name="close_behavior"
                value="minimize"
                checked={selected === "minimize"}
                onChange={() => setSelected("minimize")}
                className="sr-only peer"
              />
              <div className="flex items-center p-4 rounded-lg border border-outline-variant/40 bg-surface-container hover:bg-surface-variant/50 transition-all duration-200 peer-checked:bg-surface-container-high peer-checked:border-primary peer-checked:shadow-[0_0_0_1px_theme('colors.primary'),0_4px_20px_-5px_rgba(173,198,255,0.15)]">
                <div className="flex items-center justify-center w-10 h-10 rounded-full bg-surface-container-high text-primary mr-4 shadow-sm group-hover:scale-105 transition-transform">
                  <span className="material-symbols-outlined">keyboard_arrow_down</span>
                </div>
                <div className="flex-1">
                  <span className="font-title-sm text-title-sm text-on-surface block">最小化到托盘 (推荐)</span>
                  <span className="font-label-sm text-label-sm text-on-surface-variant block mt-0.5">保持后台运行和快捷键响应</span>
                </div>
                <div className={`relative w-5 h-5 rounded-full border-2 flex-shrink-0 transition-colors ml-4 ${
                  selected === "minimize" ? "border-primary bg-primary" : "border-outline-variant"
                }`}>
                  {selected === "minimize" && (
                    <div className="absolute inset-0 flex items-center justify-center">
                      <div className="w-2 h-2 rounded-full bg-surface" />
                    </div>
                  )}
                </div>
              </div>
            </label>

            {/* Option 2: Exit */}
            <label className="relative cursor-pointer group">
              <input
                type="radio"
                name="close_behavior"
                value="exit"
                checked={selected === "exit"}
                onChange={() => setSelected("exit")}
                className="sr-only peer"
              />
              <div className="flex items-center p-4 rounded-lg border border-outline-variant/40 bg-surface-container hover:bg-surface-variant/50 transition-all duration-200 peer-checked:bg-surface-container-high peer-checked:border-primary peer-checked:shadow-[0_0_0_1px_theme('colors.primary'),0_4px_20px_-5px_rgba(173,198,255,0.15)]">
                <div className="flex items-center justify-center w-10 h-10 rounded-full bg-surface-container-high text-error mr-4 shadow-sm group-hover:scale-105 transition-transform">
                  <span className="material-symbols-outlined">logout</span>
                </div>
                <div className="flex-1">
                  <span className="font-title-sm text-title-sm text-on-surface block">直接退出应用</span>
                  <span className="font-label-sm text-label-sm text-on-surface-variant block mt-0.5">完全关闭程序，释放所有资源</span>
                </div>
                <div className={`relative w-5 h-5 rounded-full border-2 flex-shrink-0 transition-colors ml-4 ${
                  selected === "exit" ? "border-primary bg-primary" : "border-outline-variant"
                }`}>
                  {selected === "exit" && (
                    <div className="absolute inset-0 flex items-center justify-center">
                      <div className="w-2 h-2 rounded-full bg-surface" />
                    </div>
                  )}
                </div>
              </div>
            </label>
          </div>
        </div>

        {/* Footer */}
        <div className="bg-surface-container-low/50 border-t border-outline-variant/30 px-6 py-4 flex items-center justify-between">
          <label className="flex items-center cursor-pointer group">
            <div className="relative flex items-center justify-center w-5 h-5 mr-3">
              <input
                type="checkbox"
                checked={remember}
                onChange={() => setRemember(!remember)}
                className="peer appearance-none w-5 h-5 border-2 border-outline-variant rounded bg-transparent checked:bg-primary checked:border-primary focus:outline-none focus:ring-2 focus:ring-primary/40 focus:ring-offset-1 focus:ring-offset-surface-container-low transition-colors cursor-pointer"
              />
              <span className="material-symbols-outlined text-[16px] text-on-primary absolute pointer-events-none opacity-0 peer-checked:opacity-100 transition-opacity" style={{ fontVariationSettings: "'FILL' 1" }}>check</span>
            </div>
            <span className="font-body-md text-body-md text-on-surface-variant group-hover:text-on-surface transition-colors select-none">记住我的选择</span>
          </label>

          <div className="flex gap-3">
            <button
              onClick={() => onSelect(selected === "minimize", false)}
              className="px-5 py-2 rounded font-title-sm text-title-sm text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-all focus:outline-none focus:ring-2 focus:ring-outline-variant active:scale-95 duration-150"
            >
              取消
            </button>
            <button
              onClick={handleConfirm}
              className="px-6 py-2 rounded font-title-sm text-title-sm bg-primary text-on-primary shadow-[0_0_15px_rgba(173,198,255,0.2)] hover:brightness-110 hover:shadow-[0_0_20px_rgba(173,198,255,0.3)] transition-all focus:outline-none focus:ring-2 focus:ring-primary focus:ring-offset-2 focus:ring-offset-surface-container-low active:scale-95 duration-150 flex items-center gap-2"
            >
              确定
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export default ClosePromptModal;
