import { useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";

interface IccProfile {
  name: string;
  path: string;
  is_active: boolean;
}

interface ProfileListProps {
  activeProfile: string;
  onProfileSelect: (profile: string) => void;
  showToast: (type: "success" | "error", text: string) => void;
  selectedDeviceId?: string;
}

function ProfileList({ activeProfile, onProfileSelect, showToast, selectedDeviceId }: ProfileListProps) {
  const [profiles, setProfiles] = useState<IccProfile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [dragOver, setDragOver] = useState(false);
  const [importing, setImporting] = useState(false);

  const loadData = useCallback(async () => {
    try {
      setLoading(true);
      const profilesResult = await invoke<IccProfile[]>("get_icc_profiles");
      setProfiles(profilesResult);
      setError(null);
    } catch (err) {
      setError("加载失败，请重试");
      console.error(err);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  useEffect(() => {
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "over") {
        setDragOver(true);
      } else if (event.payload.type === "drop") {
        setDragOver(false);
        const paths = event.payload.paths;
        console.log("[ProfileList] dropped paths:", paths);
        const iccPaths = paths.filter(
          (p) => p.endsWith(".icc") || p.endsWith(".icm")
        );
        if (iccPaths.length > 0) {
          importIccFiles(iccPaths);
        }
      } else {
        setDragOver(false);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const importIccFiles = async (paths: string[]) => {
    setImporting(true);
    let successCount = 0;
    for (const filePath of paths) {
      try {
        console.log("[ProfileList] importing:", filePath);
        await invoke("import_icc_profile", { srcPath: filePath });
        successCount++;
      } catch (err) {
        console.error("[ProfileList] Import failed:", err);
      }
    }
    setImporting(false);

    if (successCount > 0) {
      showToast("success", `成功导入 ${successCount} 个配置文件`);
      await loadData();
    } else {
      showToast("error", "导入失败");
    }
  };

  const handleImport = async () => {
    try {
      const selected = await open({
        filters: [{ name: "ICC Profile", extensions: ["icc", "icm"] }],
        multiple: false,
      });
      if (!selected) return;
      setImporting(true);
      await invoke("import_icc_profile", { srcPath: selected as string });
      showToast("success", "ICC 配置文件导入成功");
      await loadData();
    } catch (err) {
      showToast("error", `导入失败: ${err}`);
    } finally {
      setImporting(false);
    }
  };

  const handleProfileSelect = async (profile: IccProfile) => {
    try {
      await invoke("set_icc_profile", {
        profilePath: profile.path,
        deviceId: selectedDeviceId,
      });
      onProfileSelect(profile.name);
    } catch (err) {
      console.error("Failed to set ICC profile:", err);
    }
  };

  const handleRestoreDefault = async () => {
    try {
      await invoke("restore_default_icc_profile", {
        deviceId: selectedDeviceId,
      });
      onProfileSelect("Default");
    } catch (err) {
      console.error("Failed to restore default ICC profile:", err);
    }
  };

  if (loading) {
    return (
      <div data-components="ProfileList" className="flex flex-col min-h-0 overflow-hidden">
        <div className="mb-md flex justify-between items-end">
          <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface">ICC 配置文件</h3>
        </div>
        <div className="flex-1 bg-surface-container rounded-md border border-outline-variant/20 overflow-hidden flex flex-col">
          <div className="flex items-center justify-center py-12 text-on-surface-variant">
            <svg className="animate-spin w-5 h-5 mr-2" fill="none" viewBox="0 0 24 24">
              <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
              <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            加载中...
          </div>
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div data-components="ProfileList" className="flex flex-col min-h-0 overflow-hidden">
        <div className="mb-md flex justify-between items-end">
          <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface">ICC 配置文件</h3>
        </div>
        <div className="flex-1 bg-surface-container rounded-md border border-outline-variant/20 overflow-hidden flex flex-col">
          <div className="text-center py-8">
            <p className="text-body-md text-error mb-3">{error}</p>
            <button
              onClick={loadData}
              className="text-body-md text-primary hover:text-primary-container font-medium transition-colors"
            >
              重新加载
            </button>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div data-components="ProfileList" className="flex flex-col min-h-0 overflow-hidden">
      <div className="mb-md flex justify-between items-center">
        <div className="flex items-baseline gap-sm">
          <h3 data-name="title" className="font-headline-sm text-headline-sm text-on-surface">ICC 配置文件</h3>
          <span data-name="count" className="text-label-sm text-on-surface-variant font-medium">{profiles.length} 个文件</span>
        </div>
        <button
          data-name="refresh-button"
          onClick={loadData}
          className="flex items-center gap-xs px-sm py-xs rounded text-on-surface-variant hover:bg-surface-variant/50 transition-colors font-label-md text-label-md"
        >
          <span className="material-symbols-outlined text-[16px]">refresh</span>
          刷新
        </button>
      </div>

      <div
        className={`flex-1 bg-surface-container rounded-md border overflow-hidden flex flex-col transition-colors ${
          dragOver ? "border-primary border-2" : "border-outline-variant/20"
        }`}
      >
        {/* Action buttons */}
        <div data-name="action-buttons" className="p-sm border-b border-outline-variant/15 bg-surface-container-high/30 flex gap-sm">
          <button
            data-name="restore-button"
            onClick={handleRestoreDefault}
            className="flex-1 py-xs px-sm rounded-md bg-primary/10 text-primary border border-primary/25 font-label-md text-label-md flex flex-col items-center justify-center gap-xs hover:bg-primary/18 hover:border-primary/40 transition-all whitespace-nowrap"
          >
            <span className="material-symbols-outlined text-[18px]">refresh</span>
            恢复默认
          </button>
          <button
            data-name="import-button"
            onClick={handleImport}
            disabled={importing}
            className="flex-1 py-xs px-sm rounded-md bg-surface-variant/50 text-on-surface-variant border border-outline-variant/30 font-label-md text-label-md flex flex-col items-center justify-center gap-xs hover:bg-surface-variant/80 hover:border-outline-variant/50 transition-all disabled:opacity-40 whitespace-nowrap"
          >
            <span className="material-symbols-outlined text-[18px]">upload</span>
            {importing ? "导入中..." : "导入 ICC"}
          </button>
          <button
            data-name="open-dir-button"
            onClick={() => invoke("open_icc_directory")}
            className="flex-1 py-xs px-sm rounded-md bg-surface-variant/50 text-on-surface-variant border border-outline-variant/30 font-label-md text-label-md flex flex-col items-center justify-center gap-xs hover:bg-surface-variant/80 hover:border-outline-variant/50 transition-all whitespace-nowrap"
          >
            <span className="material-symbols-outlined text-[18px]">folder_open</span>
            打开目录
          </button>
        </div>

        {/* Drag overlay */}
        {dragOver && (
          <div data-name="drag-overlay" className="absolute inset-0 bg-primary/10 backdrop-blur-sm z-10 flex items-center justify-center rounded-md border-2 border-dashed border-primary m-sm">
            <div className="text-center">
              <span className="material-symbols-outlined text-primary text-[48px]">upload_file</span>
              <p className="text-primary font-label-md text-label-md mt-sm">释放以导入 ICC 文件</p>
            </div>
          </div>
        )}

        {/* Profile list */}
        <div data-name="profile-list" className="flex-1 overflow-y-auto p-sm space-y-sm relative">
          {profiles.length === 0 ? (
            <div className="text-center py-8 text-on-surface-variant text-body-md">
              未找到 ICC 配置文件
            </div>
          ) : (
            profiles.map((profile) => (
              <div
                key={profile.name}
                data-name="profile-item"
                onClick={() => handleProfileSelect(profile)}
                className={`p-md rounded-lg cursor-pointer group transition-all ${
                  activeProfile === profile.name
                    ? "bg-primary/15 border-2 border-primary shadow-sm"
                    : "bg-primary/5 hover:bg-primary/12 border border-primary/15 hover:border-primary/40"
                }`}
              >
                <div className="flex justify-between items-start mb-xs">
                  <span className="font-label-md text-label-md text-on-surface">{profile.name}</span>
                  {activeProfile === profile.name ? (
                    <span className="material-symbols-outlined text-primary text-[18px]" style={{ fontVariationSettings: "'FILL' 1" }}>check_circle</span>
                  ) : (
                    <span className="material-symbols-outlined text-on-surface-variant text-[18px] opacity-0 group-hover:opacity-100">radio_button_unchecked</span>
                  )}
                </div>
                <code className="text-[10px] text-on-surface-variant block opacity-60 group-hover:opacity-100 truncate">
                  {profile.path}
                </code>
              </div>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

export default ProfileList;
