import { useEffect, useState } from "react";
import { check, Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "done" | "error";

export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [version, setVersion] = useState("");
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState("");
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function checkForUpdate() {
      try {
        setStatus("checking");
        const update = await check();
        if (cancelled) return;

        if (update) {
          setVersion(update.version);
          setPendingUpdate(update);
          setStatus("available");
        } else {
          setStatus("idle");
        }
      } catch (e) {
        if (cancelled) return;
        setError(String(e));
        setStatus("error");
      }
    }

    checkForUpdate();
    return () => { cancelled = true; };
  }, []);

  async function installUpdate() {
    if (!pendingUpdate) return;
    try {
      setStatus("downloading");
      let totalLen = 0;
      let downloaded = 0;

      await pendingUpdate.downloadAndInstall((event) => {
        if (event.event === "Started" && event.data.contentLength) {
          totalLen = event.data.contentLength;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (totalLen > 0) setProgress(Math.round((downloaded / totalLen) * 100));
        } else if (event.event === "Finished") {
          setStatus("done");
        }
      });

      await relaunch();
    } catch (e) {
      setError(String(e));
      setStatus("error");
    }
  }

  function dismiss() {
    setStatus("idle");
    setPendingUpdate(null);
  }

  return { status, version, progress, error, installUpdate, dismiss };
}
