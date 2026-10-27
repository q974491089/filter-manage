"""Windows-only standalone gamma/display-mode diagnostic (stdlib only)."""
from __future__ import annotations

import csv
import ctypes
from ctypes import wintypes
from datetime import datetime
import json
import os
import platform
import subprocess
import time
import tkinter as tk
from tkinter import messagebox, ttk


user32 = ctypes.WinDLL("user32", use_last_error=True)
gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)


class DisplayDevice(ctypes.Structure):
    _fields_ = [
        ("cb", wintypes.DWORD),
        ("DeviceName", wintypes.WCHAR * 32),
        ("DeviceString", wintypes.WCHAR * 128),
        ("StateFlags", wintypes.DWORD),
        ("DeviceID", wintypes.WCHAR * 128),
        ("DeviceKey", wintypes.WCHAR * 128),
    ]


class PointL(ctypes.Structure):
    _fields_ = [("x", wintypes.LONG), ("y", wintypes.LONG)]


class DevModeUnion(ctypes.Structure):
    _fields_ = [("dmPosition", PointL), ("dmDisplayOrientation", wintypes.DWORD), ("dmDisplayFixedOutput", wintypes.DWORD)]


class DevMode(ctypes.Structure):
    _anonymous_ = ("u",)
    _fields_ = [
        ("dmDeviceName", wintypes.WCHAR * 32), ("dmSpecVersion", wintypes.WORD),
        ("dmDriverVersion", wintypes.WORD), ("dmSize", wintypes.WORD),
        ("dmDriverExtra", wintypes.WORD), ("dmFields", wintypes.DWORD),
        ("u", DevModeUnion), ("dmColor", wintypes.WORD), ("dmDuplex", wintypes.WORD),
        ("dmYResolution", wintypes.WORD), ("dmTTOption", wintypes.WORD),
        ("dmCollate", wintypes.WORD), ("dmFormName", wintypes.WCHAR * 32),
        ("dmLogPixels", wintypes.WORD), ("dmBitsPerPel", wintypes.DWORD),
        ("dmPelsWidth", wintypes.DWORD), ("dmPelsHeight", wintypes.DWORD),
        ("dmDisplayFlags", wintypes.DWORD), ("dmDisplayFrequency", wintypes.DWORD),
        ("dmICMMethod", wintypes.DWORD), ("dmICMIntent", wintypes.DWORD),
        ("dmMediaType", wintypes.DWORD), ("dmDitherType", wintypes.DWORD),
        ("dmReserved1", wintypes.DWORD), ("dmReserved2", wintypes.DWORD),
        ("dmPanningWidth", wintypes.DWORD), ("dmPanningHeight", wintypes.DWORD),
    ]


user32.EnumDisplayDevicesW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, ctypes.POINTER(DisplayDevice), wintypes.DWORD]
user32.EnumDisplayDevicesW.restype = wintypes.BOOL
user32.EnumDisplaySettingsW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, ctypes.POINTER(DevMode)]
user32.EnumDisplaySettingsW.restype = wintypes.BOOL
gdi32.CreateDCW.argtypes = [wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.LPVOID]
gdi32.CreateDCW.restype = wintypes.HDC
gdi32.DeleteDC.argtypes = [wintypes.HDC]
gdi32.GetDeviceGammaRamp.argtypes = [wintypes.HDC, ctypes.POINTER(wintypes.WORD)]
gdi32.GetDeviceGammaRamp.restype = wintypes.BOOL
gdi32.SetDeviceGammaRamp.argtypes = [wintypes.HDC, ctypes.POINTER(wintypes.WORD)]
gdi32.SetDeviceGammaRamp.restype = wintypes.BOOL

ENUM_CURRENT_SETTINGS = 0xFFFFFFFF
DISPLAY_DEVICE_ACTIVE = 0x00000001
DISPLAY_DEVICE_PRIMARY = 0x00000004
DEFAULT_PROCESS_TERMS = ("valorant", "riotclient", "filter-manage", "vgc")
SAMPLE_POINTS = (0, 1, 16, 64, 128, 192, 240, 254, 255)


def displays():
    result = []
    for index in range(32):
        item = DisplayDevice()
        item.cb = ctypes.sizeof(DisplayDevice)
        if not user32.EnumDisplayDevicesW(None, index, ctypes.byref(item), 0):
            break
        if item.StateFlags & DISPLAY_DEVICE_ACTIVE:
            monitor = DisplayDevice()
            monitor.cb = ctypes.sizeof(DisplayDevice)
            pnp_id = ""
            if user32.EnumDisplayDevicesW(item.DeviceName, 0, ctypes.byref(monitor), 0):
                pnp_id = monitor.DeviceID
            result.append({
                "id": item.DeviceName,
                "adapter": item.DeviceString,
                "pnp_id": pnp_id,
                "primary": bool(item.StateFlags & DISPLAY_DEVICE_PRIMARY),
                "mode": mode_payload(mode(item.DeviceName)),
            })
    return result


def mode(device_id):
    item = DevMode()
    item.dmSize = ctypes.sizeof(DevMode)
    if not user32.EnumDisplaySettingsW(device_id, ENUM_CURRENT_SETTINGS, ctypes.byref(item)):
        return None
    return (int(item.dmPelsWidth), int(item.dmPelsHeight), int(item.dmDisplayFrequency), int(item.dmBitsPerPel))


def read_ramp(device_id):
    dc = gdi32.CreateDCW(device_id, None, None, None)
    if not dc:
        return None, ctypes.get_last_error()
    try:
        values = (wintypes.WORD * 768)()
        if not gdi32.GetDeviceGammaRamp(dc, values):
            return None, ctypes.get_last_error()
        return tuple(int(value) for value in values), 0
    finally:
        gdi32.DeleteDC(dc)


def write_ramp(device_id, values):
    dc = gdi32.CreateDCW(device_id, None, None, None)
    if not dc:
        return False, ctypes.get_last_error()
    try:
        data = (wintypes.WORD * 768)(*values)
        ok = bool(gdi32.SetDeviceGammaRamp(dc, data))
        return ok, 0 if ok else ctypes.get_last_error()
    finally:
        gdi32.DeleteDC(dc)


def identity_ramp():
    return tuple(index * 257 for _channel in range(3) for index in range(256))


def probe_ramp():
    values = []
    for _channel in range(3):
        for index in range(256):
            # A power above 1.0 lowers mid-tones, making the probe visibly darker.
            values.append(round(pow(index / 255.0, 2.4) * 65535))
    return tuple(values)


def signature(ramp):
    return None if ramp is None else tuple(value >> 8 for value in ramp)


def gamma_state(ramp, error=None):
    if ramp is None:
        return {"available": False, "shape": None, "samples": None, "signature": None, "error": error}
    identity = signature(identity_ramp())
    samples = [
        [ramp[channel * 256 + point] >> 8 for point in SAMPLE_POINTS]
        for channel in range(3)
    ]
    return {
        "available": True,
        "shape": "identity" if signature(ramp) == identity else "non-identity",
        "samples": samples,
        "signature": list(signature(ramp)),
        "error": None,
    }


def summary(ramp):
    if ramp is None:
        return "GetDeviceGammaRamp 失败"
    points = (0, 1, 16, 64, 128, 192, 240, 254, 255)
    channels = []
    for channel in range(3):
        channels.append(" ".join(f"{point}:{ramp[channel * 256 + point] >> 8}" for point in points))
    shape = "identity" if signature(ramp) == signature(identity_ramp()) else "non-identity"
    return f"R=[{channels[0]}] G=[{channels[1]}] B=[{channels[2]}] shape={shape}"


def process_infos():
    try:
        output = subprocess.check_output(["tasklist", "/FO", "CSV", "/NH"], text=True, encoding="mbcs", errors="replace", creationflags=getattr(subprocess, "CREATE_NO_WINDOW", 0))
    except (OSError, subprocess.SubprocessError):
        return []
    processes = []
    for row in csv.reader(output.splitlines()):
        if not row or not any(token in row[0].lower() for token in DEFAULT_PROCESS_TERMS):
            continue
        try:
            pid = int(row[1])
        except (IndexError, ValueError):
            continue
        processes.append({"name": row[0], "pid": pid})
    return sorted(processes, key=lambda item: (item["name"], item["pid"]))


def process_names():
    return sorted({item["name"] for item in process_infos()})


def mode_payload(current_mode):
    if current_mode is None:
        return None
    return {
        "width": current_mode[0],
        "height": current_mode[1],
        "refresh_hz": current_mode[2],
        "bits_per_pixel": current_mode[3],
    }


def snapshot_payload(device, current_mode, ramp, error, processes):
    errors = [] if error is None else [f"GetDeviceGammaRamp failed winerr={error}"]
    return {
        "timestamp_ms": int(time.time() * 1000),
        "display_id": device,
        "mode": mode_payload(current_mode),
        "gamma": gamma_state(ramp, errors[0] if errors else None),
        "processes": processes,
        "errors": errors,
    }


class App:
    def __init__(self, root):
        self.root = root
        self.root.title("Filter-Manage 全屏 Gamma 调试器")
        self.root.geometry("980x680")
        self.root.minsize(780, 520)
        self.items = []
        self.running = False
        self.previous_mode = None
        self.previous_signature = None
        self.saved_ramp = None
        self.events = []
        self.event_records = []
        self.selected = tk.StringVar()
        self.status = tk.StringVar(value="未监视")
        self.build_ui()
        self.refresh()
        self.add("启动 | " + platform.platform() + " | Python " + platform.python_version())

    def build_ui(self):
        frame = ttk.Frame(self.root, padding=12)
        frame.pack(fill="both", expand=True)
        ttk.Label(frame, text="全屏 Gamma / 显示模式诊断", font=("Segoe UI", 15, "bold")).pack(anchor="w")
        ttk.Label(frame, text="默认只读，不注入游戏；先让 Filter-Manage 应用方案，再开始监视。",).pack(anchor="w", pady=(2, 8))
        bar = ttk.Frame(frame)
        bar.pack(fill="x")
        ttk.Label(bar, text="目标显示器:").pack(side="left")
        self.combo = ttk.Combobox(bar, textvariable=self.selected, state="readonly", width=46)
        self.combo.pack(side="left", padx=6)
        ttk.Button(bar, text="刷新", command=self.refresh).pack(side="left")
        ttk.Button(bar, text="开始监视", command=self.start).pack(side="left", padx=(12, 4))
        ttk.Button(bar, text="停止并保存", command=self.stop).pack(side="left")
        ttk.Label(bar, textvariable=self.status).pack(side="right")
        probe = ttk.Frame(frame)
        probe.pack(fill="x", pady=(8, 4))
        ttk.Button(probe, text="写入临时 Gamma 2.4 并监视", command=self.apply_probe).pack(side="left")
        ttk.Button(probe, text="恢复原 ramp", command=self.restore).pack(side="left", padx=6)
        ttk.Button(probe, text="标记当前状态", command=lambda: self.snapshot("用户标记")).pack(side="left")
        text = ("推荐流程：应用 Filter-Manage 方案 → 开始监视 → 启动 VALORANT 进入问题全屏 → 黑屏后点标记 → Alt+Tab 回桌面再点标记。\n"
                "强验证：写入临时 Gamma 2.4 后进入游戏；若日志显示 ramp 变成 identity，说明切换重置了 LUT；若 ramp 仍 non-identity 但画面没变，更像游戏绕过桌面 LUT。\n"
                "测试结束必须点“恢复原 ramp”。")
        ttk.Label(frame, text=text, justify="left").pack(anchor="w", pady=(5, 8))
        self.log = tk.Text(frame, height=24, wrap="none", state="disabled", font=("Consolas", 9))
        self.log.pack(fill="both", expand=True)

    def refresh(self):
        self.items = displays()
        values = [f"{item['id']}  {item['adapter']}" for item in self.items]
        self.combo["values"] = values
        if values:
            index = next((i for i, item in enumerate(self.items) if item["primary"]), 0)
            self.combo.current(index)
            self.add("显示器刷新: " + ", ".join(item["id"] for item in self.items))
        else:
            self.selected.set("")
            self.add("未找到活动显示器")

    def device(self):
        index = self.combo.current()
        return self.items[index]["id"] if 0 <= index < len(self.items) else None

    def add(self, message, kind="INFO", snapshot=None):
        now = datetime.now()
        line = now.strftime("%Y-%m-%d %H:%M:%S.%f")[:-3] + " | " + message
        self.events.append(line)
        self.event_records.append({
            "timestamp_ms": int(now.timestamp() * 1000),
            "kind": kind,
            "message": message,
            "snapshot": snapshot,
        })
        if hasattr(self, "log"):
            self.log.configure(state="normal")
            self.log.insert("end", line + "\n")
            self.log.see("end")
            self.log.configure(state="disabled")

    def snapshot(self, reason):
        device = self.device()
        if not device:
            self.add("无目标显示器")
            return
        current_mode = mode(device)
        ramp, error = read_ramp(device)
        mode_text = "unavailable" if current_mode is None else f"{current_mode[0]}x{current_mode[1]} @{current_mode[2]}Hz {current_mode[3]}bpp"
        ramp_text = summary(ramp) if ramp is not None else f"GetDeviceGammaRamp failed winerr={error}"
        process_info = process_infos()
        processes = ",".join(item["name"] for item in process_info) or "none"
        payload = snapshot_payload(device, current_mode, ramp, error, process_info)
        kind = "SNAPSHOT_INITIAL" if reason == "监视开始" else "USER_MARK"
        self.add(f"{reason} | display={device} mode={mode_text} | {ramp_text} | processes={processes}", kind, payload)
        self.previous_mode = current_mode
        self.previous_signature = signature(ramp)

    def start(self):
        if self.running:
            return
        if not self.device():
            messagebox.showerror("没有显示器", "没有找到活动显示器。")
            return
        self.running = True
        self.status.set("监视中")
        self.add("监视开始", "MONITOR_STARTED")
        self.snapshot("监视开始")
        self.poll()

    def stop(self):
        self.running = False
        self.status.set("已停止")
        self.add("监视停止", "MONITOR_STOPPED")
        self.save_report()

    def poll(self):
        if not self.running:
            return
        device = self.device()
        if device:
            current_mode = mode(device)
            ramp, error = read_ramp(device)
            process_info = process_infos()
            payload = snapshot_payload(device, current_mode, ramp, error, process_info)
            if current_mode != self.previous_mode:
                text = "unavailable" if current_mode is None else f"{current_mode[0]}x{current_mode[1]} @{current_mode[2]}Hz {current_mode[3]}bpp"
                self.add(f"DISPLAY_MODE_CHANGED | display={device} mode={text}", "DISPLAY_MODE_CHANGED", payload)
                self.previous_mode = current_mode
            current_signature = signature(ramp)
            if current_signature != self.previous_signature:
                text = summary(ramp) if ramp is not None else f"read failed winerr={error}"
                self.add(f"GAMMA_RAMP_CHANGED | display={device} {text}", "GAMMA_RAMP_CHANGED", payload)
                self.previous_signature = current_signature
        self.root.after(500, self.poll)

    def apply_probe(self):
        device = self.device()
        if not device:
            return
        if self.saved_ramp is None:
            self.saved_ramp, error = read_ramp(device)
            if self.saved_ramp is None:
                messagebox.showerror("读取失败", f"无法保存原 ramp，错误码 {error}")
                return
        ok, error = write_ramp(device, probe_ramp())
        current_mode = mode(device)
        ramp, read_error = read_ramp(device)
        payload = snapshot_payload(device, current_mode, ramp, read_error, process_infos())
        self.add(f"PROBE_WRITE | display={device} gamma=2.4 ok={ok} winerr={error}", "PROBE_WRITE", payload)
        if not self.running:
            self.start()
        if ok:
            messagebox.showinfo("测试 ramp 已写入", "屏幕应变暗。现在进入 VALORANT 的问题全屏，结束后点击恢复原 ramp。")
        else:
            messagebox.showerror("写入失败", f"SetDeviceGammaRamp 返回失败，错误码 {error}")

    def restore(self):
        device = self.device()
        if not device or self.saved_ramp is None:
            self.add("PROBE_RESTORE_SKIP | 没有保存的原 ramp")
            return
        ok, error = write_ramp(device, self.saved_ramp)
        current_mode = mode(device)
        ramp, read_error = read_ramp(device)
        payload = snapshot_payload(device, current_mode, ramp, read_error, process_infos())
        self.add(f"PROBE_RESTORE | display={device} ok={ok} winerr={error}", "PROBE_RESTORE", payload)
        if ok:
            self.saved_ramp = None
        else:
            messagebox.showerror("恢复失败", f"请重新运行 Filter-Manage 或注销/重启，错误码 {error}")
        self.save_report()

    def save_report(self):
        if not self.events:
            return
        desktop = os.path.join(os.path.expanduser("~"), "Desktop")
        if not os.path.isdir(desktop):
            desktop = os.getcwd()
        json_path = os.path.join(desktop, "diagnostic-report-v1.json")
        report = {
            "schema_version": "diagnostic-report-v1",
            "generated_at_ms": int(time.time() * 1000),
            "tool": "Filter-Manage Python Diagnostic",
            "os": {
                "architecture": platform.machine(),
                "windows_version": platform.platform(),
            },
            "displays": displays(),
            "display_id": self.device(),
            "process_terms": list(DEFAULT_PROCESS_TERMS),
            "events": self.event_records,
            "probe_active": self.saved_ramp is not None,
        }
        with open(json_path, "w", encoding="utf-8") as handle:
            json.dump(report, handle, ensure_ascii=False, indent=2)

        text_path = os.path.join(desktop, f"fullscreen-gamma-debug-{datetime.now():%Y%m%d-%H%M%S}.txt")
        with open(text_path, "w", encoding="utf-8") as handle:
            handle.write("\n".join(self.events) + "\n")
        self.add("报告已保存: " + json_path)


def main():
    if os.name != "nt":
        raise SystemExit("This diagnostic only runs on Windows.")
    root = tk.Tk()
    app = App(root)
    def close():
        app.restore()
        app.stop()
        root.destroy()
    root.protocol("WM_DELETE_WINDOW", close)
    root.mainloop()


if __name__ == "__main__":
    main()
