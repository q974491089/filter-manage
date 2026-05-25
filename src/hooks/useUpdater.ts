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
        console.log("[Updater] checking for updates...");
        const update = await check();
        if (cancelled) return;

        if (update) {
          console.log("[Updater] update available:", update.version);
          setVersion(update.version);
          setBody(update.body || "");
          setPendingUpdate(update);
          setStatus("available");
        } else {
          console.log("[Updater] no update available");
          setStatus("idle");
        }
      } catch (e) {
        if (cancelled) return;
        console.error("[Updater] check failed:", e);
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

  async function checkForUpdate(): Promise<"available" | "latest" | "error"> {
    try {
      setStatus("checking");
      console.log("[Updater] manual check...");
      const update = await check();
      if (update) {
        console.log("[Updater] manual: update available:", update.version);
        setVersion(update.version);
        setBody(update.body || "");
        setPendingUpdate(update);
        setStatus("available");
        return "available";
      } else {
        console.log("[Updater] manual: no update available");
        setStatus("idle");
        return "latest";
      }
    } catch (e) {
      console.error("[Updater] manual check failed:", e);
      setError(String(e));
      setStatus("error");
      return "error";
    }
  }

  return { status, version, body, progress, error, snoozed, installUpdate, dismiss, snooze, checkForUpdate };
}
