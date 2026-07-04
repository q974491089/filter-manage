import { useEffect, useRef, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface Mirror { name: string; url: string }
export interface UpdateInfo {
  hasUpdate: boolean;
  version: string;
  notes: string;
  signature: string;
  mirrors: Mirror[];
}

export type UpdateStatus =
  | "idle" | "checking" | "available"
  | "downloading" | "verifying" | "ready"
  | "done" | "error" | "cancelled";

const SNOOZE_KEY = "update_snooze_until";
const THIRTY_DAYS = 30 * 24 * 60 * 60 * 1000;
const SLOW_AFTER_MS = 120_000;       // 2 分钟
const SLOW_BELOW_BPS = 100 * 1024;   // 100 KB/s

function checkSnooze(): boolean {
  const v = localStorage.getItem(SNOOZE_KEY);
  return v ? Date.now() < parseInt(v, 10) : false;
}

function setSnooze(): void {
  localStorage.setItem(SNOOZE_KEY, String(Date.now() + THIRTY_DAYS));
}

export function useUpdater() {
  const [status, setStatus] = useState<UpdateStatus>("idle");
  const [version, setVersion] = useState("");
  const [body, setBody] = useState("");
  const [progress, setProgress] = useState(0);      // 0~100
  const [speed, setSpeed] = useState(0);            // 字节/秒
  const [mirrors, setMirrors] = useState<Mirror[]>([]);
  const [currentMirror, setCurrentMirror] = useState<string>("");
  const [showMirrorPrompt, setShowMirrorPrompt] = useState(false);
  const [error, setError] = useState("");
  const [snoozed, setSnoozed] = useState(() => checkSnooze());
  const [remainingSeconds, setRemainingSeconds] = useState(0); // 预估剩余秒数

  // 测速采样
  const startRef = useRef(0);
  const lastRef = useRef<{ t: number; bytes: number }>({ t: 0, bytes: 0 });

  // 换源：取消后需要重下的 url（ref 模式避免 setTimeout 竞态）
  const pendingMirrorRef = useRef<string | null>(null);

  // 自动检查（启动时）
  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (checkSnooze()) { setSnoozed(true); return; }
      try {
        setStatus("checking");
        console.log("[Updater] checking for updates...");
        const info = await invoke<UpdateInfo>("check_update");
        if (cancelled) return;
        if (info.hasUpdate) {
          console.log("[Updater] update available:", info.version);
          setVersion(info.version);
          setBody(info.notes || "");
          setMirrors(info.mirrors);
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
    })();
    return () => { cancelled = true; };
  }, []);

  // 事件监听
  useEffect(() => {
    const unlisteners: UnlistenFn[] = [];
    (async () => {
      // 下载进度
      unlisteners.push(await listen<{ downloaded: number; total: number }>(
        "update://progress", (e) => {
          const { downloaded, total } = e.payload;
          if (total > 0) setProgress(Math.round((downloaded / total) * 100));

          const now = Date.now();
          if (startRef.current === 0) {
            startRef.current = now;
            lastRef.current = { t: now, bytes: downloaded };
          }
          const dt = (now - lastRef.current.t) / 1000;
          if (dt >= 1) {
            const bps = (downloaded - lastRef.current.bytes) / dt;
            setSpeed(bps);
            lastRef.current = { t: now, bytes: downloaded };
            // 计算剩余时间
            if (bps > 0 && total > downloaded) {
              const remaining = Math.ceil((total - downloaded) / bps);
              setRemainingSeconds(remaining);
            } else {
              setRemainingSeconds(0);
            }
            const elapsed = now - startRef.current;
            if (elapsed > SLOW_AFTER_MS || bps < SLOW_BELOW_BPS) {
              setShowMirrorPrompt(true);
            }
          }
        }));

      // 下载完成且校验通过 → 自动安装
      unlisteners.push(await listen("update://verified", async () => {
        console.log("[Updater] verified, installing...");
        setStatus("verifying");
        // 短暂展示校验态让用户看到
        await new Promise(r => setTimeout(r, 800));
        setStatus("ready");
        try {
          await invoke("install_update");
          setStatus("done");
        } catch (e) {
          console.error("[Updater] install failed:", e);
          setError(String(e));
          setStatus("error");
        }
      }));

      // 下载/校验失败
      unlisteners.push(await listen<{ message: string }>("update://error", (e) => {
        console.error("[Updater] error:", e.payload.message);
        setError(e.payload.message);
        setStatus("error");
      }));

      // 取消完成
      unlisteners.push(await listen("update://cancelled", () => {
        console.log("[Updater] cancelled");
        setStatus("available");
        setProgress(0);
        setSpeed(0);
        // 如果有待切换的镜像源，取消完成后自动重下
        if (pendingMirrorRef.current) {
          const url = pendingMirrorRef.current;
          pendingMirrorRef.current = null;
          startDownload(url);
        }
      }));
    })();
    return () => { unlisteners.forEach((u) => u()); };
  }, []);

  const startDownload = useCallback(async (url?: string) => {
    const target = url || mirrors[0]?.url;
    if (!target) return;
    console.log("[Updater] starting download:", target);
    setCurrentMirror(target);
    setShowMirrorPrompt(false);
    setProgress(0);
    setSpeed(0);
    setRemainingSeconds(0);
    startRef.current = 0;
    setStatus("downloading");
    try {
      await invoke("download_update", { url: target });
    } catch (e) {
      console.error("[Updater] download failed:", e);
      setError(String(e));
      setStatus("error");
    }
  }, [mirrors]);

  // 换源：取消当前下载，等 cancelled 事件后自动重下
  const switchMirror = useCallback(async (url: string) => {
    console.log("[Updater] switching mirror:", url);
    pendingMirrorRef.current = url;
    try {
      await invoke("cancel_update_download");
    } catch (e) {
      // 取消失败（可能已经结束了），直接重下
      pendingMirrorRef.current = null;
      startDownload(url);
    }
  }, [startDownload]);

  const cancel = useCallback(async () => {
    console.log("[Updater] cancelling download...");
    pendingMirrorRef.current = null;
    try {
      await invoke("cancel_update_download");
    } catch (e) {
      console.error("[Updater] cancel failed:", e);
    }
    setShowMirrorPrompt(false);
  }, []);

  const dismiss = useCallback(() => {
    setStatus("idle");
  }, []);

  const snooze = useCallback(() => {
    setSnooze();
    setSnoozed(true);
    setStatus("idle");
  }, []);

  // 手动检查（关于面板用），返回结果
  const checkForUpdate = useCallback(async (): Promise<"available" | "latest" | "error"> => {
    try {
      setStatus("checking");
      console.log("[Updater] manual check...");
      const info = await invoke<UpdateInfo>("check_update");
      if (info.hasUpdate) {
        console.log("[Updater] manual: update available:", info.version);
        setVersion(info.version);
        setBody(info.notes || "");
        setMirrors(info.mirrors);
        setStatus("available");
        return "available";
      }
      console.log("[Updater] manual: no update available");
      setStatus("idle");
      return "latest";
    } catch (e) {
      console.error("[Updater] manual check failed:", e);
      setError(String(e));
      setStatus("error");
      return "error";
    }
  }, []);

  return {
    status, version, body, progress, speed, mirrors, currentMirror,
    showMirrorPrompt, error, snoozed, remainingSeconds,
    startDownload, switchMirror, cancel, dismiss, snooze, checkForUpdate,
  };
}
