import { useState } from "react";
import Markdown from "react-markdown";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useUpdater } from "../hooks/useUpdater";
import { Icon } from "./Icon";

const CHANGELOG_URL = "https://filter-manage.6ya.site/changelog.html";

export default function UpdateModal() {
  const { status, version, body, progress, installUpdate, dismiss, snooze } = useUpdater();
  const [snoozeChecked, setSnoozeChecked] = useState(false);

  if (status === "idle" || status === "checking" || status === "error") return null;

  if (status === "downloading" || status === "done") {
    return (
      <div className="fixed inset-0 z-[200] flex items-center justify-center bg-black/40 backdrop-blur-sm">
        <div className="bg-surface-container/80 backdrop-blur-md border border-outline-variant/20 rounded-xl p-xl shadow-2xl shadow-black/40 w-[360px]">
          <div className="flex items-center gap-md mb-lg">
            <Icon name="download" className="text-[24px] text-primary" />
            <h2 className="font-headline-sm text-headline-sm text-on-surface">正在更新</h2>
          </div>
          <p className="font-body-md text-on-surface-variant mb-md">
            正在下载 v{version}... {progress}%
          </p>
          <div className="w-full bg-surface-variant/50 rounded-full h-2">
            <div
              className="bg-primary h-2 rounded-full transition-all duration-300"
              style={{ width: `${progress}%` }}
            />
          </div>
        </div>
      </div>
    );
  }

  const handleDismiss = () => {
    if (snoozeChecked) {
      snooze();
    } else {
      dismiss();
    }
  };

  return (
    <div className="fixed inset-0 z-[200] flex items-center justify-center bg-black/40 backdrop-blur-sm">
      <div className="bg-surface-container/80 backdrop-blur-md border border-outline-variant/20 rounded-xl p-xl shadow-2xl shadow-black/40 w-[400px] max-h-[80vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center gap-md mb-lg">
          <Icon name="system_update" className="text-[24px] text-primary" />
          <h2 className="font-headline-sm text-headline-sm text-on-surface">发现新版本</h2>
          <div className="flex-1" />
          <button
            onClick={() => openUrl(CHANGELOG_URL)}
            className="flex items-center gap-xs px-sm py-xs rounded text-on-surface-variant hover:text-primary hover:bg-primary/10 transition-colors"
            title="查看完整更新日志"
          >
            <Icon name="open_in_new" className="text-[16px]" />
            <span className="font-label-sm text-label-sm">完整日志</span>
          </button>
        </div>

        {/* Version */}
        <p className="font-body-md text-on-surface-variant mb-lg">
          新版本 <span className="text-primary font-medium">v{version}</span> 可用
        </p>

        {/* Changelog */}
        {body && (
          <div className="mb-lg flex-1 min-h-0">
            <h3 className="font-label-md text-label-md text-on-surface-variant mb-sm">更新内容</h3>
            <div className="bg-surface-variant/30 rounded-lg p-md h-full max-h-[300px] overflow-y-auto">
              <div className="prose prose-invert prose-sm max-w-none text-on-surface-variant">
                <Markdown>{body}</Markdown>
              </div>
            </div>
          </div>
        )}

        {/* Actions */}
        <div className="flex gap-md mb-lg">
          <button
            onClick={installUpdate}
            className="flex-1 px-lg py-sm rounded-lg bg-primary text-on-primary font-label-md text-label-md font-medium hover:opacity-90 active:scale-[0.98] transition-all"
          >
            立即更新
          </button>
          <button
            onClick={handleDismiss}
            className="flex-1 px-lg py-sm rounded-lg border border-outline-variant/50 text-on-surface-variant font-label-md text-label-md hover:bg-surface-variant/50 active:scale-[0.98] transition-all"
          >
            稍后
          </button>
        </div>

        {/* Snooze Checkbox */}
        <label className="flex items-center gap-sm cursor-pointer group">
          <input
            type="checkbox"
            checked={snoozeChecked}
            onChange={(e) => setSnoozeChecked(e.target.checked)}
            className="w-4 h-4 rounded border-outline-variant/50 bg-surface-variant/30 text-primary focus:ring-primary/50 focus:ring-offset-0"
          />
          <span className="font-label-md text-label-md text-on-surface-variant group-hover:text-on-surface transition-colors">
            30天不再提示
          </span>
        </label>
      </div>
    </div>
  );
}
