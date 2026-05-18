import { useUpdater } from "../hooks/useUpdater";

export default function UpdateBanner() {
  const { status, version, progress, installUpdate, dismiss } = useUpdater();

  if (status === "idle" || status === "checking" || status === "error") return null;

  if (status === "downloading" || status === "done") {
    return (
      <div className="fixed top-0 inset-x-0 z-50 bg-blue-600 text-white text-center py-2 text-sm">
        正在下载更新 v{version}... {progress}%
      </div>
    );
  }

  return (
    <div className="fixed top-0 inset-x-0 z-50 bg-green-600 text-white text-center py-2 text-sm flex items-center justify-center gap-4">
      <span>发现新版本 v{version}</span>
      <button
        onClick={installUpdate}
        className="px-3 py-0.5 bg-white text-green-700 rounded text-xs font-medium hover:bg-green-50"
      >
        立即更新
      </button>
      <button
        onClick={dismiss}
        className="px-2 py-0.5 text-green-100 hover:text-white text-xs"
      >
        稍后
      </button>
    </div>
  );
}
