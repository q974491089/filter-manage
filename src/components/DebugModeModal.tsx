import { useState } from "react";
import { invoke } from "../lib/ipc";
import { Icon } from "./Icon";

interface DebugModeModalProps {
  open: boolean;
  /** 调试记录是否进行中（记录中形态） */
  active: boolean;
  /** 进行中的日志文件路径 */
  logPath: string | null;
  /** 刚生成的报告路径（非空且未在记录中 → 「报告已生成」完成态） */
  reportPath: string | null;
  onClose: () => void;
  onStarted: (logPath: string) => void;
  onStopped: (reportPath: string, fileCopied: boolean) => void;
}

/**
 * 调试模式弹窗。入口是左上角图标连点 10 次（隐藏彩蛋）。
 * 三种形态：授权开启 → 记录中（控制台）→ 报告已生成（快速分享）。
 * 停止时自动恢复默认设置（调试期间随意的拖拽不该留给用户收拾）；
 * 报告文件默认自动复制到剪贴板，粘贴进 QQ/微信即发送。
 */
function DebugModeModal({
  open,
  active,
  logPath,
  reportPath,
  onClose,
  onStarted,
  onStopped,
}: DebugModeModalProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [fileCopied, setFileCopied] = useState(false);
  const [copyState, setCopyState] = useState<"idle" | "ok" | "fail">("idle");
  if (!open) return null;

  // 视图优先级：记录中 > 报告完成态 > 授权态
  const view = active ? "recording" : reportPath ? "report" : "authorize";

  const handleStart = async () => {
    setBusy(true);
    setError(null);
    try {
      const info = await invoke<{ log_path: string }>("debug_start");
      onStarted(info.log_path);
      onClose();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleStop = async () => {
    setBusy(true);
    setError(null);
    try {
      const info = await invoke<{ report_path: string; file_copied: boolean }>("debug_stop");
      setFileCopied(info.file_copied);
      setCopyState("idle");
      // 不关弹窗：停在「报告已生成」完成态，让用户直接复制文案或确认路径
      onStopped(info.report_path, info.file_copied);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  const handleCopyText = async () => {
    if (!reportPath) return;
    setBusy(true);
    setCopyState("idle");
    try {
      await invoke("debug_copy_report_text", { path: reportPath });
      setCopyState("ok");
    } catch (err) {
      setCopyState("fail");
      setError(String(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div
      data-name="debug-overlay"
      className="fixed inset-0 z-[300] flex items-center justify-center bg-black/50 backdrop-blur-sm"
    >
      {/* 注意：蒙层不响应点击关闭——连点图标进入调试模式后手上的余点击在蒙层上，
          误触关掉弹窗会让人以为入口坏了。关闭只走明确的按钮。 */}
      <div
        data-name="debug-modal"
        className="bg-surface-container rounded-2xl shadow-2xl border border-outline-variant/20 w-[440px] max-w-[90vw] overflow-hidden relative"
      >
        <button
          data-name="debug-close"
          onClick={onClose}
          aria-label="关闭"
          className="absolute top-3 right-3 p-1 rounded-md text-on-surface-variant/60 hover:text-on-surface hover:bg-surface-variant/50 transition-colors"
        >
          <Icon name="close" className="text-[18px]" />
        </button>

        {/* Header：左图右题的横向布局，比居中竖排更紧凑；pr-12 给右上角关闭钮让位 */}
        <div className="flex items-center gap-3 px-6 pt-5 pb-3 pr-12">
          <div
            className={`w-11 h-11 rounded-xl flex items-center justify-center shrink-0 border ${
              view === "report"
                ? "bg-green-500/15 border-green-500/30"
                : "bg-primary/10 border-primary/20"
            }`}
          >
            <Icon
              name={view === "report" ? "check_circle" : "terminal"}
              className={`text-[22px] ${view === "report" ? "text-green-500" : "text-primary"}`}
            />
          </div>
          <div className="min-w-0">
            <h2 className="font-headline-sm text-headline-sm font-medium text-on-surface">调试模式</h2>
            <span className="font-label-sm text-label-sm text-on-surface-variant/60">
              {view === "recording" && "记录进行中"}
              {view === "report" && "报告已生成"}
              {view === "authorize" && "排查屏幕色彩问题"}
            </span>
          </div>
        </div>

        <div className="mx-6 border-t border-outline-variant/15" />

        {/* Body：信息密度优先，分组留白而不是大空隙 */}
        <div className="px-6 py-4 space-y-3">
          {view === "recording" && (
            <>
              <p className="font-body-sm text-body-sm text-on-surface-variant leading-relaxed">
                正在记录。请现在复现问题：点击 ICC、拖动滑条、切换方案——每次操作和系统的真实响应都会写入日志。
              </p>
              <div className="rounded-lg bg-primary/8 border border-primary/15 px-3 py-2">
                <p className="font-label-sm text-label-sm text-on-surface-variant leading-relaxed">
                  停止时会自动<span className="text-on-surface font-medium">恢复默认设置</span>
                  （不怕拖乱屏幕），并生成桌面报告、复制到剪贴板，直接粘贴到 QQ / 微信即可发送。
                </p>
              </div>
              {logPath && (
                <p className="font-label-sm text-label-sm text-on-surface-variant/50 break-all">
                  日志：{logPath}
                </p>
              )}
            </>
          )}

          {view === "report" && (
            <>
              <p className="font-label-md text-label-md text-on-surface-variant">
                报告已保存到桌面
              </p>
              <p className="font-mono text-[11px] leading-relaxed text-on-surface-variant/80 break-all bg-surface-variant/30 border border-outline-variant/15 rounded-lg px-3 py-2">
                {reportPath}
              </p>
              {fileCopied ? (
                <p className="font-body-sm text-body-sm text-on-surface leading-relaxed flex items-start gap-xs">
                  <Icon
                    name="check_circle"
                    className="text-green-600 text-[18px] mt-px shrink-0"
                  />
                  <span>报告文件已复制到剪贴板——直接在 QQ / 微信等输入框粘贴即可发送文件。</span>
                </p>
              ) : (
                <p className="font-body-sm text-body-sm text-on-surface-variant leading-relaxed">
                  自动复制未成功（剪贴板可能被占用），请到桌面手动选择该文件发送。
                </p>
              )}
              <p className="font-label-sm text-label-sm text-on-surface-variant/70 leading-relaxed flex items-start gap-xs">
                <Icon name="settings_backup_restore" className="text-[16px] mt-px shrink-0 opacity-70" />
                <span>调试期间的调整已自动恢复为默认设置。</span>
              </p>
              {copyState === "ok" && (
                <p className="font-label-sm text-label-sm text-green-600">
                  文案已复制到剪贴板，可直接粘贴为文本。
                </p>
              )}
              {copyState === "fail" && error && (
                <p className="text-error font-label-sm text-label-sm">{error}</p>
              )}
            </>
          )}

          {view === "authorize" && (
            <>
              <p className="font-body-sm text-body-sm text-on-surface-variant leading-relaxed">
                开启后收集以下信息，用于排查「应用了没反应」类问题：
              </p>
              <ul className="font-body-sm text-body-sm text-on-surface-variant leading-relaxed list-disc pl-5 space-y-1.5">
                <li>环境快照：系统版本、显卡驱动、显示器、HDR 状态与当前伽马曲线</li>
                <li>操作记录：ICC 应用、滑条调节、方案切换及其参数</li>
                <li>系统响应：API 返回值与写入回读验证结果</li>
              </ul>
              <p className="font-label-sm text-label-sm text-on-surface-variant/70 leading-relaxed">
                只写入本机磁盘、不联网上传。停止时会自动恢复默认设置，并生成桌面报告、复制到剪贴板，是否发送由你决定。
              </p>
            </>
          )}
          {view !== "report" && error && <p className="text-error font-label-sm text-label-sm">{error}</p>}
        </div>

        {/* Actions */}
        <div className="flex gap-2 px-6 pb-6">
          {view === "recording" && (
            <button
              data-name="debug-stop"
              onClick={handleStop}
              disabled={busy}
              className="flex-1 flex items-center justify-center gap-2 px-4 py-3 rounded-xl bg-error/90 text-on-error font-label-md text-label-md hover:bg-error transition-colors disabled:opacity-50"
            >
              <Icon name="power_settings_new" className="text-[18px]" />
              {busy ? "生成中…" : "停止并生成报告"}
            </button>
          )}

          {view === "report" && (
            <>
              <button
                data-name="debug-copy-text"
                onClick={handleCopyText}
                disabled={busy}
                className="flex-1 px-4 py-3 rounded-xl bg-surface-variant/50 text-on-surface-variant border border-outline-variant/30 font-label-md text-label-md hover:bg-surface-variant/80 transition-colors disabled:opacity-50"
              >
                {copyState === "ok" ? "已复制文案" : "复制报告文案"}
              </button>
              <button
                data-name="debug-done"
                onClick={onClose}
                disabled={busy}
                className="flex-1 px-4 py-3 rounded-xl bg-primary text-on-primary font-label-md text-label-md hover:bg-primary-container transition-colors disabled:opacity-50"
              >
                完成
              </button>
            </>
          )}

          {view === "authorize" && (
            <>
              <button
                data-name="debug-cancel"
                onClick={onClose}
                disabled={busy}
                className="flex-1 px-4 py-3 rounded-xl bg-surface-variant/50 text-on-surface-variant border border-outline-variant/30 font-label-md text-label-md hover:bg-surface-variant/80 transition-colors disabled:opacity-50"
              >
                取消
              </button>
              <button
                data-name="debug-start"
                onClick={handleStart}
                disabled={busy}
                className="flex-1 flex items-center justify-center gap-2 px-4 py-3 rounded-xl bg-primary text-on-primary font-label-md text-label-md hover:bg-primary-container transition-colors disabled:opacity-50"
              >
                <Icon name="terminal" className="text-[18px]" />
                {busy ? "开启中…" : "授权并开始记录"}
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

export default DebugModeModal;
