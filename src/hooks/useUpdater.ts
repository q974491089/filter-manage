import { useEffect, useState } from "react";
import { check, Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export type UpdateStatus = "idle" | "checking" | "available" | "downloading" | "done" | "error";

const SNOOZE_KEY = "update_snooze_until";
const THIRTY_DAYS = 30 * 24 * 60 * 60 * 1000;

function checkSnooze(): boolean {
  const snoozeUntil = localStorage.getItem(SNOOZE_KEY);
  if (!snoozeUntil) return false;
  return Date.now() < parseInt(snoozeUntil, 10);
}

function setSnooze(): void {
  localStorage.setItem(SNOOZE_KEY, String(Date.now() + THIRTY_DAYS));
}

export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [version, setVersion] = useState("");
  const [body, setBody] = useState("");
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState("");
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [snoozed, setSnoozed] = useState(() => checkSnooze());

  useEffect(() => {
    let cancelled = false;

    async function checkForUpdate() {
      if (checkSnooze()) {
        setSnoozed(true);
        return;
      }

      try {
        setStatus("checking");
        const update = await check();
        if (cancelled) return;

        if (update) {
          setVersion(update.version);
          setBody(update.body || "");
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

  function snooze() {
    setSnooze();
    setSnoozed(true);
    setStatus("idle");
    setPendingUpdate(null);
  }

  return { status, version, body, progress, error, snoozed, installUpdate, dismiss, snooze };
}
