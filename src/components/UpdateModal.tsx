import { useState } from "react";
import Markdown from "react-markdown";
import { openUrl } from "@tauri-apps/plugin-opener";
import type { useUpdater } from "../hooks/useUpdater";
import { Icon } from "./Icon";

const CHANGELOG_URL = "https://filter-manage.6ya.site/changelog.html";

function fmtSpeed(bps: number): string {
  if (bps <= 0) return "—";
  if (bps < 1024 * 1024) return `${(bps / 1024).toFixed(0)} KB/s`;
  return `${(bps / 1024 / 1024).toFixed(1)} MB/s`;
}

function fmtRemaining(seconds: number): string {
  if (seconds <= 0) return "";
  if (seconds < 60) return `约 ${seconds} 秒`;
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return s > 0 ? `约 ${m} 分 ${s} 秒` : `约 ${m} 分钟`;
}

export default function UpdateModal({ updater }: { updater: ReturnType<typeof useUpdater> }) {
  const {
    status, version, body, progress, speed, mirrors, currentMirror,
    showMirrorPrompt, startDownload, switchMirror, cancel, dismiss, snooze,
    remainingSeconds,
  } = updater;
  const [snoozeChecked, setSnoozeChecked] = useState(false);

  if (status === "idle" || status === "checking" || status === "error") return null;

  // 下载 / 校验 / 安装态
  if (status === "downloading" || status === "verifying" || status === "ready" || status === "done") {
    const verifying = status === "verifying" || status === "ready";
    const remaining = verifying ? 0 : remainingSeconds;
    const infoParts: string[] = [];
    if (!verifying && speed > 0) infoParts.push(`下载速度: ${fmtSpeed(speed)}`);
    if (remaining > 0) infoParts.push(`剩余时间: ${fmtRemaining(remaining)}`);
    const infoText = infoParts.join(" | ") || " "; // nbsp when empty

    return (
      <div data-component="UpdateModal" data-name="downloading-overlay"
           className="fixed inset-0 z-[200] flex items-center justify-center bg-black/60 backdrop-blur-sm">
        <div data-component="UpdateModal" data-name="downloading-card"
             className="glass-panel rounded-xl shadow-2xl w-[400px] flex flex-col overflow-hidden">
          {/* Header */}
          <div className="flex items-center justify-between px-6 pt-6 pb-4">
            <h2 className="font-headline-md text-headline-md text-on-surface flex items-center gap-2">
              <Icon name="download" filled className="text-[20px] text-primary" />
              {verifying ? "正在校验并安装" : "正在下载更新"}
            </h2>
            {!verifying && (
              <button data-component="UpdateModal" data-name="close-download"
                      onClick={cancel}
                      className="text-on-surface-variant hover:text-on-surface transition-colors p-1 rounded hover:bg-surface-variant/50">
                <Icon name="close" className="text-[18px]" />
              </button>
            )}
          </div>

          {/* Content */}
          <div className="px-6 pb-6 flex flex-col gap-4">
            {/* Version + percentage */}
            <div className="flex justify-between items-end">
              <span className="font-title-sm text-title-sm text-on-surface">
                {verifying ? "正在校验..." : `正在下载 v${version}...`}
              </span>
              <span className="font-label-sm text-label-sm text-primary">{verifying ? "" : `${progress}%`}</span>
            </div>

            {/* Progress bar — gradient + glow */}
            <div className="w-full h-2 bg-surface-container-highest rounded-full overflow-hidden relative">
              <div className="absolute top-0 left-0 h-full rounded-full transition-all duration-500"
                   style={{
                     width: `${verifying ? 100 : progress}%`,
                     background: 'linear-gradient(90deg, #3b82f6, #60a5fa)',
                     boxShadow: '0 0 10px rgba(59,130,246,0.5)',
                   }} />
            </div>

            {/* Speed + remaining time */}
            <div className="flex justify-between">
              <span className="font-label-sm text-label-sm text-on-surface-variant">{infoText}</span>
            </div>

            {/* 换源提示 */}
            {showMirrorPrompt && !verifying && (
              <div data-component="UpdateModal" data-name="mirror-prompt"
                   className="p-3 rounded-lg bg-surface-variant/30 border border-outline-variant/20">
                <p className="font-label-md text-label-md text-on-surface mb-2">下载过慢，换个源？</p>
                <div className="flex flex-col gap-1">
                  {mirrors.map((m) => (
                    <button key={m.url}
                            data-component="UpdateModal" data-name={`mirror-${m.name}`}
                            onClick={() => switchMirror(m.url)}
                            disabled={m.url === currentMirror}
                            className="flex items-center justify-between px-2 py-1 rounded text-label-sm font-label-sm text-on-surface-variant hover:bg-primary/10 hover:text-primary disabled:opacity-40 disabled:cursor-default transition-colors">
                      <span>{m.name}</span>
                      {m.url === currentMirror && <span className="text-label-sm">当前</span>}
                    </button>
                ))}
              </div>
            </div>
          )}
          </div>

          {/* Footer */}
          <div className="bg-surface-container-low/50 border-t border-outline-variant/30 px-6 py-4 flex items-center justify-end gap-3">
            {!verifying && (
              <>
                <button data-component="UpdateModal" data-name="cancel-download"
                        onClick={cancel}
                        className="px-5 py-2 rounded font-title-sm text-title-sm text-on-surface-variant hover:text-on-surface hover:bg-surface-variant/50 transition-all active:scale-95">
                  取消
                </button>
                <button data-component="UpdateModal" data-name="minimize-download"
                        onClick={cancel}
                        className="px-6 py-2 rounded font-title-sm text-title-sm bg-surface-container-high text-on-surface border border-outline-variant/30 hover:bg-surface-variant/50 transition-all active:scale-95">
                  最小化
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  // available 态（发现新版本）
  const handleDismiss = () => (snoozeChecked ? snooze() : dismiss());

  return (
    <div data-component="UpdateModal" data-name="available-overlay"
         className="fixed inset-0 z-[200] flex items-center justify-center bg-black/40 backdrop-blur-sm">
      <div data-component="UpdateModal" data-name="available-card"
           className="bg-surface-container/80 backdrop-blur-md border border-outline-variant/20 rounded-xl p-xl shadow-2xl shadow-black/40 w-[600px] max-h-[85vh] flex flex-col">
        <div className="flex items-center gap-md mb-lg">
          <Icon name="system_update" className="text-[24px] text-primary" />
          <h2 className="font-headline-sm text-headline-sm text-on-surface">发现新版本</h2>
          <div className="flex-1" />
          <button data-component="UpdateModal" data-name="open-changelog"
                  onClick={() => openUrl(CHANGELOG_URL)}
                  className="flex items-center gap-xs px-sm py-xs rounded text-on-surface-variant hover:text-primary hover:bg-primary/10 transition-colors"
                  title="查看完整更新日志">
            <Icon name="open_in_new" className="text-[16px]" />
            <span className="font-label-sm text-label-sm">完整日志</span>
          </button>
        </div>

        <p className="font-body-md text-on-surface-variant mb-lg">
          新版本 <span className="text-primary font-medium">v{version}</span> 可用
        </p>

        {body && (
          <div className="mb-lg flex-1 min-h-0">
            <h3 className="font-label-md text-label-md text-on-surface-variant mb-sm">更新内容</h3>
            <div className="bg-surface-variant/30 rounded-lg p-md h-full max-h-[240px] overflow-y-auto">
              <div className="prose prose-invert prose-sm max-w-none text-on-surface-variant">
                <Markdown>{body}</Markdown>
              </div>
            </div>
          </div>
        )}

        <div className="flex gap-md mb-lg">
          <button data-component="UpdateModal" data-name="install-now"
                  onClick={() => startDownload()}
                  className="flex-1 px-lg py-sm rounded-lg bg-primary text-on-primary font-label-md text-label-md font-medium hover:opacity-90 active:scale-[0.98] transition-all">
            立即更新
          </button>
          <button data-component="UpdateModal" data-name="later"
                  onClick={handleDismiss}
                  className="flex-1 px-lg py-sm rounded-lg border border-outline-variant/50 text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 active:scale-[0.98] transition-all">
            稍后
          </button>
        </div>

        <label data-component="UpdateModal" data-name="snooze-toggle"
               className="flex items-center gap-sm cursor-pointer group">
          <input type="checkbox" checked={snoozeChecked}
                 onChange={(e) => setSnoozeChecked(e.target.checked)}
                 className="w-4 h-4 rounded border-outline-variant/50 bg-surface-variant/30 text-primary focus:ring-primary/50 focus:ring-offset-0" />
          <span className="font-label-md text-label-md text-on-surface-variant group-hover:text-on-surface transition-colors">
            30天不再提示
          </span>
        </label>
      </div>
    </div>
  );
}
