@echo off
:: ===========================================================================
:: GPU gamma-ramp diagnostic (brightness / contrast / gamma / RGB gain)
:: -- double-click to run.  Companion to gpu-color-diag.bat, which covers the
::    digital-vibrance side (NVAPI / ADL / ADLX).  This one covers the GDI
::    SetDeviceGammaRamp side, i.e. the 亮度/对比度/伽马/RGB 增益 sliders.
::
:: Hybrid bat + PowerShell file. cmd runs only this header and exits; the
:: PowerShell half lives after the #PS_START sentinel line and is re-read
:: from this same file as UTF-8.
::
:: Keep this header ASCII-only. cmd parses the file using the OEM codepage
:: before chcp takes effect, and UTF-8 bytes can decode into command
:: separators that split a :: line into bogus commands.
::
:: Save as CRLF. With LF endings cmd may miss the exit /b line boundary and
:: fall through into the C# source below, spraying errors and creating junk
:: files from the > in the code.
:: ===========================================================================
chcp 65001 >nul 2>&1
setlocal enabledelayedexpansion
set "ps1=%temp%\gamma-ramp-diag-%random%.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$c=[IO.File]::ReadAllLines('%~f0',[Text.Encoding]::UTF8);$i=0;while($i -lt $c.Length -and $c[$i] -notlike '#PS_START*'){$i++};if($i -ge $c.Length){exit 1};[IO.File]::WriteAllLines('%ps1%',$c[($i+1)..($c.Length-1)],[Text.Encoding]::UTF8);exit 0"
if errorlevel 1 (echo PowerShell prepare failed & pause & exit /b 1)
powershell -NoProfile -ExecutionPolicy Bypass -File "%ps1%"
set exitcode=%errorlevel%
del /q "%ps1%" 2>nul
echo.
pause
exit /b %exitcode%
#PS_START
# ── gamma ramp（亮度/对比度/伽马/RGB 增益）诊断 —— PowerShell 部分 ──
#
# 上面的 bat 包装靠 #PS_START 这个哨兵行来切分，不依赖行号，
# 所以这里增删行是安全的。但不要动 #PS_START 那一行本身。
#
# 与 gpu-color-diag.bat 的分工：
#   gpu-color-diag.bat  → 显卡/驱动能力：显示器枚举、NVAPI(DVC)、ADL、ADLX
#   本脚本              → GDI gamma ramp 这条链：读回、写入对照、谁在抢
# 亮度/对比度/伽马/RGB 增益四个滑块最终都写同一个调用（SetDeviceGammaRamp），
# 所以"四个都不生效"必然是同一条链的问题；数字振动/饱和度是另一条链，
# 本脚本不测它。

$ErrorActionPreference = 'Continue'
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch {}

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Security.Principal;

public static class GammaDiag {
  // ═══════════ Win32 ═══════════
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
  public struct DISPLAY_DEVICE {
    public int cb;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)]  public string DeviceName;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string DeviceString;
    public int StateFlags;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string DeviceID;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=128)] public string DeviceKey;
  }
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  static extern bool EnumDisplayDevicesW(string dev, uint num, ref DISPLAY_DEVICE dd, uint flags);
  [DllImport("user32.dll")]
  static extern int GetSystemMetrics(int index);
  [DllImport("kernel32.dll")]
  static extern uint WTSGetActiveConsoleSessionId();
  [DllImport("kernel32.dll", SetLastError=true)]
  static extern bool ProcessIdToSessionId(uint pid, out uint sid);

  [DllImport("gdi32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  static extern IntPtr CreateDCW(string pwszDriver, string pwszDevice, string pszPort, IntPtr pdm);
  [DllImport("gdi32.dll", SetLastError=true)]
  static extern bool DeleteDC(IntPtr hdc);
  [DllImport("gdi32.dll", EntryPoint="GetDeviceGammaRamp", SetLastError=true)]
  static extern bool GetRamp(IntPtr hdc, [Out] ushort[] ramp);
  [DllImport("gdi32.dll", EntryPoint="SetDeviceGammaRamp", SetLastError=true)]
  static extern bool SetRamp(IntPtr hdc, [In] ushort[] ramp);
  [DllImport("gdi32.dll", EntryPoint="GetICMProfileW", CharSet=CharSet.Unicode, SetLastError=true)]
  static extern bool GetIcm(IntPtr hdc, ref uint size, StringBuilder name);

  // ═══════════ 显示拓扑 / HDR（只读）═══════════
  // 只查不写：这两个 API 各只声明 Get 变体，Set 变体在本文件里不存在。
  // 拓扑决定「ICC 有没有可能生效」—— 克隆组和高级颜色（HDR）都会让写入静默失效。
  [StructLayout(LayoutKind.Sequential)]
  public struct LUID { public uint LowPart; public int HighPart; }

  [StructLayout(LayoutKind.Sequential)]
  public struct DISPLAYCONFIG_DEVICE_INFO_HEADER {
    public uint type; public uint size; public LUID adapterId; public uint id;
  }
  [StructLayout(LayoutKind.Sequential)]
  public struct DISPLAYCONFIG_PATH_SOURCE_INFO {
    public LUID adapterId; public uint id; public uint modeInfoIdx; public uint statusFlags;
  }
  [StructLayout(LayoutKind.Sequential)]
  public struct DISPLAYCONFIG_RATIONAL { public uint Numerator; public uint Denominator; }
  [StructLayout(LayoutKind.Sequential)]
  public struct DISPLAYCONFIG_PATH_TARGET_INFO {
    public LUID adapterId; public uint id; public uint modeInfoIdx;
    public uint outputTechnology; public uint rotation; public uint scaling;
    public DISPLAYCONFIG_RATIONAL refreshRate; public uint scanLineOrdering;
    public int targetAvailable; public uint statusFlags;
  }
  [StructLayout(LayoutKind.Sequential)]
  public struct DISPLAYCONFIG_PATH_INFO {
    public DISPLAYCONFIG_PATH_SOURCE_INFO sourceInfo;
    public DISPLAYCONFIG_PATH_TARGET_INFO targetInfo;
  }
  [StructLayout(LayoutKind.Sequential)]
  public struct DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO {
    public DISPLAYCONFIG_DEVICE_INFO_HEADER header;
    public uint value; public uint colorEncoding; public uint bitsPerColorChannel;
  }
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
  public struct DISPLAYCONFIG_SOURCE_DEVICE_NAME {
    public DISPLAYCONFIG_DEVICE_INFO_HEADER header;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=32)] public string viewGdiDeviceName;
  }

  [DllImport("user32.dll")]
  static extern int GetDisplayConfigBufferSizes(uint flags, out uint numPath, out uint numMode);
  // modes 用不透明字节缓冲接：只需要它够大，不解释内容（省掉联合体的三个子结构体）
  [DllImport("user32.dll")]
  static extern int QueryDisplayConfig(uint flags, ref uint numPath,
      [Out] DISPLAYCONFIG_PATH_INFO[] paths, ref uint numMode, [Out] byte[] modes, IntPtr topo);
  [DllImport("user32.dll")]
  static extern int DisplayConfigGetDeviceInfo(ref DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO info);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  static extern int DisplayConfigGetDeviceInfo(ref DISPLAYCONFIG_SOURCE_DEVICE_NAME info);

  public static bool LayoutsValid() {
    return Marshal.SizeOf(typeof(DISPLAYCONFIG_DEVICE_INFO_HEADER)) == 20
        && Marshal.SizeOf(typeof(DISPLAYCONFIG_PATH_SOURCE_INFO)) == 20
        && Marshal.SizeOf(typeof(DISPLAYCONFIG_PATH_TARGET_INFO)) == 48
        && Marshal.SizeOf(typeof(DISPLAYCONFIG_PATH_INFO)) == 68
        && Marshal.SizeOf(typeof(DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO)) == 32
        && Marshal.SizeOf(typeof(DISPLAYCONFIG_SOURCE_DEVICE_NAME)) == 84;
  }

  static string OutputTech(uint t) {
    switch (t) {
      case 0: return "VGA";
      case 4: return "DVI";
      case 5: return "HDMI";
      case 6: return "LVDS(笔记本内置屏)";
      case 10: return "DisplayPort 外接";
      case 11: return "DisplayPort 内置(笔记本)";
      case 12: return "UDI 外接";
      case 13: return "UDI 内置";
      case 14: return "SDTV dongle";
      case 15: return "Miracast 无线";
      case 16: return "间接有线(DisplayLink/扩展坞)";
      case 17: return "间接虚拟(串流/虚拟屏)";
      case 0x80000000: return "内置屏";
      default: return "未知(" + t + ")";
    }
  }

  // ═══════════ 活动显示器 ═══════════
  static readonly List<string> _name = new List<string>();
  static readonly List<string> _card = new List<string>();
  static readonly List<bool>   _prim = new List<bool>();
  static readonly List<bool>   _mirr = new List<bool>();
  static readonly List<string> _pnp  = new List<string>();

  static void LoadDisplays() {
    if (_name.Count > 0) return;
    for (uint i = 0; i < 32; i++) {
      var dd = new DISPLAY_DEVICE();
      dd.cb = Marshal.SizeOf(typeof(DISPLAY_DEVICE));
      if (!EnumDisplayDevicesW(null, i, ref dd, 0)) break;
      if ((dd.StateFlags & 0x1) == 0) continue;          // 只要已连接（活动）的
      _name.Add(dd.DeviceName);
      _card.Add(dd.DeviceString);
      _prim.Add((dd.StateFlags & 0x4) != 0);
      _mirr.Add((dd.StateFlags & 0x8) != 0);
      var mon = new DISPLAY_DEVICE();
      mon.cb = Marshal.SizeOf(typeof(DISPLAY_DEVICE));
      _pnp.Add(EnumDisplayDevicesW(dd.DeviceName, 0, ref mon, 0) ? mon.DeviceID : "");
    }
  }

  static string Primary() {
    for (int i = 0; i < _name.Count; i++) if (_prim[i]) return _name[i];
    return _name.Count > 0 ? _name[0] : null;
  }

  // ═══════════ ramp 读写与比较 ═══════════
  //
  // 驱动普遍把 16 位 LUT 量化到 8 位（写进去 0x3333，读回来 0x3300），
  // 所以所有比较和采样都只看高字节，否则会把量化当成"没写进去"。

  static IntPtr OpenDc(string dev) {
    return CreateDCW(dev, null, null, IntPtr.Zero);
  }
  static ushort[] ReadRamp(string dev) {
    IntPtr h = OpenDc(dev);
    if (h == IntPtr.Zero) return null;                   // IntPtr.Zero == 失败
    try {
      var r = new ushort[768];
      if (!GetRamp(h, r)) return null;
      return r;
    } finally { DeleteDC(h); }
  }
  static string WriteRamp(string dev, ushort[] ramp, out int err) {
    err = 0;
    IntPtr h = OpenDc(dev);
    if (h == IntPtr.Zero) { err = Marshal.GetLastWin32Error(); return "CreateDC 失败"; }
    try {
      if (!SetRamp(h, ramp)) { err = Marshal.GetLastWin32Error(); return "False"; }
      return "True";
    } finally { DeleteDC(h); }
  }
  static ushort[] Identity() {
    var r = new ushort[768];
    for (int i = 0; i < 256; i++) {
      ushort v = (ushort)Math.Min(65535, i * 257);
      for (int c = 0; c < 3; c++) r[c * 256 + i] = v;
    }
    return r;
  }
  // 复刻 src-tauri/src/nvidia.rs 的 calculate_lut（UI 标度 -> ramp）
  static ushort[] AppLut(int uiBrightness, int uiContrast, double gammaRaw) {
    double contrast = (Clamp((uiContrast + 82.0) / 164.0, 0, 1) - 0.5) * 2.0;
    double brightness = (Clamp((uiBrightness + 125.0) / 250.0, 0, 1) - 0.5) * 2.0;
    double offset = contrast > 0.0 ? contrast * -25.4 : contrast * -32.0;
    double range = 255.0 + offset * 2.0;
    offset = offset + brightness * (range / 5.0);
    double gamma = Clamp(gammaRaw, 0.4, 2.8);
    var lut = new ushort[256];
    for (int i = 0; i < 256; i++) {
      double linear = Clamp((i + offset) / range, 0, 1);
      double v = Math.Abs(gamma - 1.0) < 1e-6 ? linear : Math.Pow(linear, 1.0 / gamma);
      lut[i] = (ushort)Math.Round(v * 65535.0);
    }
    var full = new ushort[768];
    for (int i = 0; i < 256; i++) for (int c = 0; c < 3; c++) full[c * 256 + i] = lut[i];
    return full;
  }
  static ushort[] GammaCurve(double gamma) {
    var full = new ushort[768];
    for (int i = 0; i < 256; i++) {
      double v = Math.Pow(i / 255.0, 1.0 / gamma);
      ushort u = (ushort)Math.Round(Clamp(v, 0, 1) * 65535.0);
      for (int c = 0; c < 3; c++) full[c * 256 + i] = u;
    }
    return full;
  }
  // 恒等 ramp 上挖一个 2 级深、3 个点的凹陷：肉眼完全看不出来，但真的破坏了单调性。
  // 用来单独探测"驱动到底会不会拒绝非单调 ramp" —— 如果连这个都返回 False，
  // 那 app 里任何算歪的 ramp（比如 vcgt 解析出错生成的递减曲线）都会被拒。
  static ushort[] SmallDip() {
    var r = Identity();
    for (int i = 128; i <= 130; i++)
      for (int c = 0; c < 3; c++) r[c * 256 + i] = (ushort)(r[c * 256 + i] - 514);
    return r;
  }
  static double Clamp(double v, double lo, double hi) {
    return v < lo ? lo : (v > hi ? hi : v);
  }
  static int MaxDiff(ushort[] a, ushort[] b) {
    int m = 0;
    for (int i = 0; i < 768; i++) {
      int d = (a[i] >> 8) - (b[i] >> 8);
      if (d < 0) d = -d;
      if (d > m) m = d;
    }
    return m;
  }
  static int MaxDiffChannel(ushort[] a, ushort[] b, int ch) {
    int m = 0;
    for (int i = 0; i < 256; i++) {
      int d = (a[ch * 256 + i] >> 8) - (b[ch * 256 + i] >> 8);
      if (d < 0) d = -d;
      if (d > m) m = d;
    }
    return m;
  }
  static string Sample(ushort[] r, int ch) {
    int[] pts = { 0, 1, 16, 64, 128, 192, 240, 254, 255 };
    var sb = new StringBuilder();
    for (int k = 0; k < pts.Length; k++) {
      sb.Append(pts[k]);
      sb.Append(":");
      sb.Append(r[ch * 256 + pts[k]] >> 8);
      if (k < pts.Length - 1) sb.Append(" ");
    }
    return sb.ToString();
  }
  static string Shape(ushort[] r) {
    if (MaxDiff(r, Identity()) == 0) return "恒等（没有任何程序在写显卡 LUT）";
    bool allZero = true, allMax = true;
    for (int i = 0; i < 768; i++) {
      if ((r[i] >> 8) != 0) allZero = false;
      if ((r[i] >> 8) != 255) allMax = false;
    }
    if (allZero) return "全 0 —— 异常！像是被写入过一份坏 ramp";
    if (allMax) return "全 255 —— 异常！";
    return "非恒等（有程序写过 LUT：ICC / 校准加载器 / 夜间模式 / 第三方，或本 app 自己写的）";
  }
  static string RampBlock(string dev) {
    var sb = new StringBuilder();
    var r = ReadRamp(dev);
    if (r == null) {
      sb.AppendLine("    GetDeviceGammaRamp 失败 —— 驱动不支持回读，只能靠肉眼判断");
      return sb.ToString();
    }
    sb.AppendLine("    R 采样 = " + Sample(r, 0));
    sb.AppendLine("    G 采样 = " + Sample(r, 1));
    sb.AppendLine("    B 采样 = " + Sample(r, 2));
    sb.AppendLine("    判定：" + Shape(r));
    return sb.ToString();
  }

  // ═══════════ 1. 环境 ═══════════
  public static string EnvInfo() {
    var sb = new StringBuilder();
    sb.AppendLine("---- 1. 环境 ----");
    bool admin = false;
    try { admin = new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator); }
    catch (Exception) { }
    sb.AppendLine("  系统         = " + Environment.OSVersion);
    sb.AppendLine("  64 位进程    = " + Environment.Is64BitProcess + "   管理员 = " + admin);
    sb.AppendLine("  远程桌面会话 = " + (GetSystemMetrics(0x1000) != 0 ? "是（远程会话里 gamma ramp 本来就不生效）" : "否"));
    // 会话检查：GDI 的 gamma 写入只对"物理控制台会话"可靠；如果脚本跑在别的会话里
    // （远程桌面 / 云电脑 / 某些远程控制软件建的会话），SetDeviceGammaRamp 会直接返回 False。
    try {
      uint mySid = 0;
      ProcessIdToSessionId((uint)System.Diagnostics.Process.GetCurrentProcess().Id, out mySid);
      uint console = WTSGetActiveConsoleSessionId();
      sb.AppendLine("  进程会话 ID  = " + mySid + "   物理控制台会话 ID = "
        + (console == 0xFFFFFFFF ? "无" : console.ToString()));
      if (console == 0xFFFFFFFF)
        sb.AppendLine("    ** 取不到物理控制台会话 —— 这台机器可能没有本地登录会话 **");
      else if (mySid != console)
        sb.AppendLine("    ** 脚本不在物理控制台会话里：gamma ramp 写入很可能被系统拒绝（这是重点嫌疑）**");
      else
        sb.AppendLine("    会话一致（本地控制台会话）");
    } catch (Exception ex) {
      sb.AppendLine("  会话检查失败: " + ex.Message);
    }
    return sb.ToString();
  }

  // ═══════════ 2. 活动显示器 ═══════════
  public static string Displays() {
    LoadDisplays();
    var sb = new StringBuilder();
    sb.AppendLine("---- 2. 活动显示器（亮度/对比度/伽马/RGB 只会写到这里的显示器）----");
    if (_name.Count == 0) {
      sb.AppendLine("  没有已连接的显示器 —— 这个状态本身就不正常");
      return sb.ToString();
    }
    for (int i = 0; i < _name.Count; i++) {
      sb.AppendLine("  [" + i + "] " + _name[i] + "  主屏=" + _prim[i] + "  镜像=" + _mirr[i] + "  显卡=" + _card[i]);
      sb.AppendLine("       显示器 PnP = " + (_pnp[i] == null || _pnp[i].Length == 0 ? "(取不到)" : _pnp[i]));
    }
    if (_name.Count > 1) sb.AppendLine("  注意：有多台显示器 —— 请说明症状出现在哪一台（主屏是 " + Primary() + "）");
    return sb.ToString();
  }

  // ═══════════ 3/4. 用户拖动滑条的前后对照 ═══════════
  static readonly Dictionary<string, ushort[]> _base = new Dictionary<string, ushort[]>();

  public static string CaptureBaseline() {
    LoadDisplays();
    var sb = new StringBuilder();
    sb.AppendLine("---- 3. 基线 ramp（拖动滑条之前）----");
    foreach (string dev in _name) {
      sb.AppendLine("  " + dev + (_prim[_name.IndexOf(dev)] ? "  [主屏]" : ""));
      sb.Append(RampBlock(dev));
      var r = ReadRamp(dev);
      if (r != null) _base[dev] = r;
    }
    return sb.ToString();
  }

  public static string CaptureCompare() {
    LoadDisplays();
    var sb = new StringBuilder();
    sb.AppendLine("---- 4. 拖动滑条之后的 ramp ----");
    foreach (string dev in _name) {
      sb.AppendLine("  " + dev);
      sb.Append(RampBlock(dev));
      var now = ReadRamp(dev);
      if (now == null) { sb.AppendLine("    无法比较（回读失败）"); continue; }
      if (!_base.ContainsKey(dev)) { sb.AppendLine("    没有基线可比（第 3 节回读失败）"); continue; }
      ushort[] was = _base[dev];
      sb.AppendLine(string.Format("    与基线比较：最大偏差 R={0} G={1} B={2}（高字节 0-255）",
        MaxDiffChannel(was, now, 0), MaxDiffChannel(was, now, 1), MaxDiffChannel(was, now, 2)));
      int d = MaxDiff(was, now);
      if (d == 0)
        sb.AppendLine("    判定：** 与基线完全相同 → 这次拖动没有把 ramp 写进显卡 **\n"
          + "          （可能是：后端没被调用 / 写入报错 / 写到了别的显示器 / 当前屏被别的程序锁住）");
      else
        sb.AppendLine("    判定：** ramp 确实变了 → app 的写入在 GDI 层是生效的 **\n"
          + "          （那问题就变成“写进去了但画面不变”：HDR/高级颜色、校准加载器回写、看错显示器）");
    }
    return sb.ToString();
  }

  // ═══════════ 5. 本进程写入测试 ═══════════
  static readonly Dictionary<string, ushort[]> _saved = new Dictionary<string, ushort[]>();
  static readonly List<string> _tested = new List<string>();

  public static string ApplyWriteTests() {
    LoadDisplays();
    var sb = new StringBuilder();
    sb.AppendLine("---- 5. 本进程写入测试（测完自动恢复原样）----");
    sb.AppendLine("  说明：下面依次写入 4 条 ramp，每条写完立刻回读；");
    sb.AppendLine("        值都按高字节（0-255）打印。测完会恢复成测试前的样子。");
    _tested.Clear();
    foreach (string dev in _name) {
      sb.AppendLine("  " + dev);
      var orig = ReadRamp(dev);
      if (orig != null) {
        _saved[dev] = orig;
        sb.AppendLine("    原 ramp  R 采样 = " + Sample(orig, 0));
      } else {
        sb.AppendLine("    原 ramp 回读失败（下面写完就没法恢复原样，会写回恒等 ramp）");
        _saved[dev] = Identity();
      }
      RunOne(sb, dev, "A", "恒等 ramp（0..65535 线性）", Identity());
      RunOne(sb, dev, "B", "本 app 亮度+100% 的 LUT", AppLut(125, 0, 1.0));
      RunOne(sb, dev, "C", "轻微非单调 ramp（肉眼不可见）", SmallDip());
      RunOne(sb, dev, "D", "gamma 2.4 曲线（画面应明显变暗）", GammaCurve(2.4));
      _tested.Add(dev);
      sb.AppendLine("    （[D] 会保持约 3 秒让你看屏幕，然后自动恢复原样）");
    }
    return sb.ToString();
  }

  static void RunOne(StringBuilder sb, string dev, string tag, string label, ushort[] ramp) {
    int err;
    string wr = WriteRamp(dev, ramp, out err);
    string line = string.Format("    [{0}] {1,-28} 写入={2}", tag, label, wr);
    if (wr != "True") line += "  错误码=" + err + "  ← ** 驱动/系统拒绝这条 ramp **";
    var back = ReadRamp(dev);
    if (back == null) {
      sb.AppendLine(line);
      sb.AppendLine("         回读失败，无法确认");
      return;
    }
    int d = MaxDiff(ramp, back);
    line += string.Format("  回读偏差={0}", d);
    line += d == 0 ? "  → 写入生效" : "  → ** 写进去了但读回来不是它，驱动没接受（或立刻被改回）**";
    sb.AppendLine(line);
    sb.AppendLine("         回读 R 采样 = " + Sample(back, 0));
  }

  public static string FinishWriteTests() {
    var sb = new StringBuilder();
    sb.AppendLine("---- 5b. 复查与恢复 ----");
    foreach (string dev in _tested) {
      var back = ReadRamp(dev);
      sb.AppendLine("  " + dev);
      if (back == null) {
        sb.AppendLine("    回读失败，无法复查");
      } else {
        int d = MaxDiff(GammaCurve(2.4), back);
        sb.AppendLine("    几秒后回读 [D]：偏差=" + d
          + (d == 0 ? "  → 没有被别的程序改回去" : "  → ** 有程序在抢 ramp（校准加载器 / 夜间模式 / 第三方工具）**"));
        sb.AppendLine("    当前 R 采样 = " + Sample(back, 0));
      }
      int err;
      string wr = WriteRamp(dev, _saved[dev], out err);
      var now = ReadRamp(dev);
      int rd = now == null ? -1 : MaxDiff(_saved[dev], now);
      sb.AppendLine("    恢复原 ramp: 写入=" + wr + (wr == "True" ? "  回读偏差=" + rd
        + (rd == 0 ? "  → 已恢复" : "  → ** 恢复后与保存的不一致 **") : "  错误码=" + err));
    }
    return sb.ToString();
  }

  // ═══════════ 7. ICC 与 vcgt ═══════════
  // 判定基准是 src-tauri/src/icc.rs 的 parse_vcgt —— 要回答的是
  // 「app 拿到这个 ICC 会往 LUT 写什么」，不是「规范上这个标签长什么样」。
  // 两者不一致的地方（单通道、反向曲线、标签长度声明错误）必须都报出来。
  static uint BE32(byte[] d, int o) {
    return ((uint)d[o] << 24) | ((uint)d[o + 1] << 16) | ((uint)d[o + 2] << 8) | d[o + 3];
  }
  static int BE16(byte[] d, int o) { return (d[o] << 8) | d[o + 1]; }

  public sealed class VcgtResult {
    public bool HasTag;
    public bool Valid;
    public string Kind = "absent";      // absent | table | formula | unknown
    public string Error;
    public ushort[] Ramp;               // 768 项：0..255=R, 256..511=G, 512..767=B
    public int Channels, EntryCount, EntrySize;
    public double[] Gamma, Min, Max;    // formula 专用
    public long TagSizeDeclared, TagSizeNeeded;
    public bool TagSizeMismatch;
  }

  public static VcgtResult ParseVcgt(byte[] d) {
    var r = new VcgtResult();
    if (d == null || d.Length < 132) { r.Error = "文件不足 132 字节，连 ICC 头都不完整"; return r; }
    long declared = BE32(d, 0);
    if (declared < 132 || declared > d.Length) {
      r.Error = "头部声明文件长度 " + declared + " 字节，实际 " + d.Length + " 字节"; return r;
    }
    if (BE32(d, 36) != 0x61637370u) { r.Error = "缺少 acsp 签名，不是合法 ICC"; return r; }

    long tagCount = BE32(d, 128);
    if (132L + tagCount * 12L > d.Length) {
      r.Error = "标签表声明 " + tagCount + " 个标签，超出文件长度"; return r;
    }
    long off = -1, size = 0;
    for (long i = 0; i < tagCount; i++) {
      long b = 132 + i * 12;
      if (BE32(d, (int)b) == 0x76636774u) {
        off = BE32(d, (int)b + 4); size = BE32(d, (int)b + 8); break;
      }
    }
    if (off < 0) { r.Kind = "absent"; r.Valid = true; return r; }   // profile 完好，只是没有 vcgt
    r.HasTag = true;
    r.TagSizeDeclared = size;
    if (off < 132 || off + 12 > d.Length) { r.Error = "vcgt 标签偏移 " + off + " 越界"; return r; }

    int type = (int)BE32(d, (int)off + 8);
    if (type == 0) {
      r.Kind = "table";
      // 按标签声明的长度设界（比 app 严：app 不读这个字段，见 AppVerdict 的说明）
      long limit = Math.Min(off + (size > 0 ? size : d.Length - off), d.Length);
      if (limit < off + 18) { r.Error = "table 标签声明 " + size + " 字节，装不下 18 字节表头"; return r; }
      int ch = BE16(d, (int)off + 12), n = BE16(d, (int)off + 14), es = BE16(d, (int)off + 16);
      r.Channels = ch; r.EntryCount = n; r.EntrySize = es;
      if (ch < 1 || n == 0 || es == 0) { r.Error = "table 的 channels/entryCount/entrySize 含 0"; return r; }
      if (es != 1 && es != 2) { r.Error = "table entrySize=" + es + " 不是 1 或 2"; return r; }
      long need = 18L + (long)ch * n * es;
      r.TagSizeNeeded = need;
      if (need > limit - off) {
        r.TagSizeMismatch = true;
        r.Error = "table 需要 " + need + " 字节，标签只声明 " + size
                + " 字节（本 app 不校验该字段，仍会按文件剩余内容解析）";
        return r;
      }
      // 复刻 app 的重采样：src_idx = i*(n-1)/255 向下取整 —— 最近邻，不是插值
      var ramp = new ushort[768];
      for (int c = 0; c < 3; c++) {
        int cc = Math.Min(c, ch - 1);                     // 单通道时灰阶复制到三通道
        for (int i = 0; i < 256; i++) {
          int src = n > 1 ? (int)((long)i * (n - 1) / 255) : 0;
          if (src > n - 1) src = n - 1;
          long o2 = off + 18 + (long)cc * n * es + (long)src * es;
          ramp[c * 256 + i] = es == 2 ? (ushort)BE16(d, (int)o2) : (ushort)(d[o2] << 8);
        }
      }
      r.Ramp = ramp; r.Valid = true; return r;
    }
    if (type == 1) {
      r.Kind = "formula";
      if (off + 48 > d.Length) { r.Error = "formula 标签越界（需要 48 字节）"; return r; }
      if (size > 0 && size < 48) {
        r.TagSizeMismatch = true;
        r.Error = "formula 需要 48 字节，标签只声明 " + size
                + " 字节（本 app 不校验该字段，仍会按文件剩余内容解析）";
        return r;
      }
      // 交错布局：每通道 gamma / min / max 各一个 s15Fixed16
      var g = new double[3]; var mn = new double[3]; var mx = new double[3];
      for (int c = 0; c < 3; c++) {
        g[c] = (int)BE32(d, (int)off + 12 + c * 12) / 65536.0;
        mn[c] = (int)BE32(d, (int)off + 16 + c * 12) / 65536.0;
        mx[c] = (int)BE32(d, (int)off + 20 + c * 12) / 65536.0;
      }
      r.Gamma = g; r.Min = mn; r.Max = mx; r.TagSizeNeeded = 48;
      for (int c = 0; c < 3; c++) {
        if (!(g[c] > 0.0)) {
          r.Error = "formula 第 " + (c + 1) + " 通道 gamma=" + g[c].ToString("F3") + " <= 0（非法参数）";
          return r;
        }
      }
      var ramp = new ushort[768];
      for (int c = 0; c < 3; c++) {
        for (int i = 0; i < 256; i++) {
          double x = i / 255.0;
          double y = mn[c] + (mx[c] - mn[c]) * Math.Pow(x, g[c]);
          if (y < 0) y = 0;
          if (y > 1) y = 1;
          ramp[c * 256 + i] = (ushort)Math.Round(y * 65535.0);
        }
      }
      r.Ramp = ramp; r.Valid = true; return r;
    }
    r.Kind = "unknown";
    r.Error = "未知 vcgt 类型 " + type;
    return r;
  }

  /// app 拿到这个文件到底会不会往 LUT 写东西 —— 逐条复刻 icc.rs parse_vcgt 的取舍。
  public static string AppVerdict(byte[] d, VcgtResult r) {
    if (d == null || d.Length < 132)
      return "会放弃：文件不足 132 字节（parse_vcgt 返回 None，不写 LUT）";
    if (!r.HasTag)
      return "只做 WCS 注册，不写 LUT：文件里没有 vcgt 标签（app 只在有 vcgt 时才动显卡 LUT）";
    long off = -1, size = 0;
    long tagCount = BE32(d, 128);
    for (long i = 0; i < tagCount && 132 + i * 12 + 12 <= d.Length; i++) {
      long b = 132 + i * 12;
      if (BE32(d, (int)b) == 0x76636774u) { off = BE32(d, (int)b + 4); size = BE32(d, (int)b + 8); break; }
    }
    if (off <= 0 || off + 12 > d.Length)
      return "会放弃：vcgt 标签偏移越界（parse_vcgt 返回 None，不写 LUT）";
    long vlen = d.Length - off;      // app 用的是「文件剩余部分」，不看标签声明的长度
    int type = (int)BE32(d, (int)off + 8);
    string note = r.TagSizeMismatch
      ? "（注意：标签声明的长度与实际数据不符；本 app 不校验这个字段，仍会照常解析）" : "";
    if (type == 0) {
      if (vlen < 18) return "会放弃：vcgt 数据不足 18 字节表头";
      int ch = BE16(d, (int)off + 12), n = BE16(d, (int)off + 14), es = BE16(d, (int)off + 16);
      if (ch < 3) return "会放弃：本 app 要求 channels>=3，这个 profile 只有 " + ch
                       + " 个通道（ICC 规范允许单通道灰阶，app 不支持）";
      if (n == 0 || es == 0) return "会放弃：entryCount/entrySize 为 0";
      if (es != 1 && es != 2) return "会放弃：entrySize=" + es + " 不是 1 或 2";
      if (18L + (long)ch * n * es > vlen) return "会放弃：表数据超出文件末尾";
      return "会写入 LUT：table " + ch + " 通道 × " + n + " 项 × " + es + " 字节" + note;
    }
    if (type == 1) {
      if (vlen < 48) return "会放弃：formula 数据不足 48 字节";
      for (int c = 0; c < 3; c++) {
        double g = (int)BE32(d, (int)off + 12 + c * 12) / 65536.0;
        double mn = (int)BE32(d, (int)off + 16 + c * 12) / 65536.0;
        double mx = (int)BE32(d, (int)off + 20 + c * 12) / 65536.0;
        if (!(g > 0.0)) return "会放弃：第 " + (c + 1) + " 通道 gamma<=0（app 判为非法）";
        if (mx <= mn) return "会放弃：第 " + (c + 1) + " 通道 max<=min（反向曲线；ICC 规范允许，app 会拒绝）";
      }
      return "会写入 LUT：formula 三通道" + note;
    }
    return "会放弃：vcgt 类型 " + type + "（app 只认 table/formula）";
  }

  /// 只看高字节：驱动普遍把 16 位 LUT 量化到 8 位，低字节差异是噪声。
  public static int MaxHighByteDiff(ushort[] a, ushort[] b) {
    if (a == null || b == null) return -1;
    if (a.Length < 768 || b.Length < 768) return -1;
    int m = 0;
    for (int i = 0; i < 768; i++) {
      int dv = Math.Abs((a[i] >> 8) - (b[i] >> 8));
      if (dv > m) m = dv;
    }
    return m;
  }

  /// 恒等曲线 = app 会「成功写入」但画面不会有任何可见变化 —— 「点了没反应」的常见真因。
  public static string IdentityVerdict(ushort[] ramp) {
    if (ramp == null || ramp.Length < 768) return "未知（没有可解析的曲线）";
    var id = Identity();
    int dev = MaxHighByteDiff(ramp, id);
    if (dev == 0)
      return "≡ 恒等曲线（直线）：写进 LUT 后画面不会有任何可见变化 —— 这是「点了没反应」的原因，不是 app 没工作";
    int at = 0, best = -1;
    for (int i = 0; i < 768; i++) {
      int d2 = Math.Abs((ramp[i] >> 8) - (id[i] >> 8));
      if (d2 > best) { best = d2; at = i; }
    }
    return "曲线有实际形状：最大偏差 " + dev + "/255（出现在第 " + (at % 256) + " 项，R/G/B 第 "
         + (at / 256) + " 通道）";
  }

  static string VcgtSummary(byte[] d) {
    var r = ParseVcgt(d);
    if (!r.Valid) return "vcgt 解析失败：" + (r.Error ?? "未知原因");
    if (r.Kind == "absent") return null;
    if (r.Kind == "table")
      return "vcgt=table channels=" + r.Channels + " entries=" + r.EntryCount
           + " entrySize=" + r.EntrySize + "  R 采样 " + Sample(r.Ramp, 0);
    if (r.Kind == "formula") {
      var sb = new StringBuilder("vcgt=formula（交错布局 gamma/min/max）");
      for (int c = 0; c < 3; c++)
        sb.AppendFormat("  {0}: gamma={1:F3} min={2:F3} max={3:F3}",
                        "RGB"[c].ToString(), r.Gamma[c], r.Min[c], r.Max[c]);
      return sb.ToString();
    }
    return "vcgt 类型未知";
  }

  // ═══════════ 2d/2e. 拓扑、HDR、app 会列出的显示器 ═══════════
  public static string DisplayTopology() {
    var sb = new StringBuilder();
    sb.AppendLine("---- 2d. 显示拓扑 / HDR（高级颜色）----");
    sb.AppendLine("  （这一节直接决定「ICC 有没有可能生效」：克隆组和 HDR 都会让写入静默失效）");
    if (!LayoutsValid()) {
      sb.AppendLine("  ** 结构体尺寸与 SDK ABI 不符，拓扑检测结果不可信（不猜，直接标未知）**");
      return sb.ToString();
    }
    uint np = 0, nm = 0;
    int err = GetDisplayConfigBufferSizes(2, out np, out nm);   // QDC_ONLY_ACTIVE_PATHS
    if (err != 0) { sb.AppendLine("  GetDisplayConfigBufferSizes 失败 err=" + err + "（拓扑未知）"); return sb.ToString(); }
    var paths = new DISPLAYCONFIG_PATH_INFO[np];
    var modes = new byte[(int)nm * 64];
    err = QueryDisplayConfig(2, ref np, paths, ref nm, modes, IntPtr.Zero);
    if (err != 0) { sb.AppendLine("  QueryDisplayConfig 失败 err=" + err + "（拓扑未知）"); return sb.ToString(); }
    sb.AppendLine("  活动路径数 = " + np + "（每条路径 = 一路实际输出；路径数 > 显示器数 说明有克隆）");

    var gdiSeen = new Dictionary<string, int>();
    for (uint i = 0; i < np; i++) {
      var p = paths[i];
      var sn = new DISPLAYCONFIG_SOURCE_DEVICE_NAME();
      sn.viewGdiDeviceName = "";
      sn.header.type = 1;                                       // GET_SOURCE_NAME
      sn.header.size = (uint)Marshal.SizeOf(typeof(DISPLAYCONFIG_SOURCE_DEVICE_NAME));
      sn.header.adapterId = p.sourceInfo.adapterId; sn.header.id = p.sourceInfo.id;
      string gdi = "(取不到)";
      try { if (DisplayConfigGetDeviceInfo(ref sn) == 0) gdi = sn.viewGdiDeviceName; } catch (Exception) { }

      var ac = new DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO();
      ac.header.type = 9;                                       // GET_ADVANCED_COLOR_INFO
      ac.header.size = (uint)Marshal.SizeOf(typeof(DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO));
      ac.header.adapterId = p.targetInfo.adapterId; ac.header.id = p.targetInfo.id;
      string hdr;
      try {
        int acErr = DisplayConfigGetDeviceInfo(ref ac);
        if (acErr != 0) {
          hdr = "未知（查询返回 " + acErr + "，多见于老系统/老驱动）";
        } else {
          bool supported = (ac.value & 0x1) != 0;
          bool enabled = (ac.value & 0x2) != 0;
          bool wide = (ac.value & 0x4) != 0;
          bool forcedOff = (ac.value & 0x8) != 0;
          hdr = "支持=" + supported + " 已启用=" + enabled + " 宽色域强制=" + wide
              + " 被强制关闭=" + forcedOff + " 编码=" + ac.colorEncoding
              + " 位深=" + ac.bitsPerColorChannel + "bit";
          if (enabled) hdr += "  ** HDR 已开：gamma ramp 写入和 WCS 关联都会被系统忽略 —— 这就是没效果的原因 **";
          else if (supported) hdr += "（支持但未开，不挡写入）";
        }
      } catch (Exception ex) { hdr = "未知（" + ex.GetType().Name + "）"; }

      int cloneGroup = (int)(p.sourceInfo.modeInfoIdx & 0xFFFF);
      sb.AppendLine("  [" + i + "] GDI=" + gdi + "  输出=" + OutputTech(p.targetInfo.outputTechnology)
                  + "  刷新率=" + p.targetInfo.refreshRate.Numerator + "/" + p.targetInfo.refreshRate.Denominator
                  + "  目标可用=" + (p.targetInfo.targetAvailable != 0));
      sb.AppendLine("       HDR/高级颜色: " + hdr);
      if (cloneGroup != 0) sb.AppendLine("       输出组 id = " + cloneGroup
        + "（单条路径时是正常值，无意义；只有多条路径共享同一个 id 才是克隆）");
      if (gdiSeen.ContainsKey(gdi)) gdiSeen[gdi]++;
      else gdiSeen[gdi] = 1;
    }
    int cloned = 0;
    foreach (var kv in gdiSeen) if (kv.Value > 1 && kv.Key != "(取不到)") cloned++;
    if (cloned > 0) {
      sb.AppendLine("  ** 检测到克隆/复制模式：有 " + cloned + " 个 GDI 设备各对应多路输出 ——");
      sb.AppendLine("     同一个 \\\\.\\DISPLAYn 背后挂着多块屏，按设备名选 ICC 会同时命中它们。**");
    } else {
      sb.AppendLine("  未发现多路输出共用一个 GDI 设备（不是克隆模式，或者是扩展模式）");
    }
    return sb.ToString();
  }

  /// 复刻 app 的 get_display_monitors：适配器枚举 + 第二层只取 monitor index 0。
  /// 把第二层全部列出来做对照 —— 差集就是用户在 app 里「选不中」的那块屏。
  public static string AppMonitorList() {
    var sb = new StringBuilder();
    sb.AppendLine("---- 2e. Filter-Manage 会列出的显示器（复刻 get_display_monitors 的枚举方式）----");
    sb.AppendLine("  用户在 app 里能选到的就是下面这几项；和 2d 的活动路径对照，差集就是选不中/选错的部分。");
    int adapters = 0, listed = 0;
    for (uint i = 0; i < 32; i++) {
      var dd = new DISPLAY_DEVICE();
      dd.cb = Marshal.SizeOf(typeof(DISPLAY_DEVICE));
      if (!EnumDisplayDevicesW(null, i, ref dd, 0)) break;
      if ((dd.StateFlags & 0x1) == 0) continue;
      adapters++;
      bool prim = (dd.StateFlags & 0x4) != 0;
      bool mirr = (dd.StateFlags & 0x8) != 0;
      string firstPnp = "";
      var allPnp = new List<string>();
      for (uint m = 0; m < 8; m++) {
        var mon = new DISPLAY_DEVICE();
        mon.cb = Marshal.SizeOf(typeof(DISPLAY_DEVICE));
        if (!EnumDisplayDevicesW(dd.DeviceName, m, ref mon, 0)) break;
        if (m == 0) firstPnp = mon.DeviceID;
        allPnp.Add(mon.DeviceID);
      }
      listed++;
      sb.AppendLine("  [" + listed + "] " + dd.DeviceName + "  主屏=" + prim + "  镜像标志=" + mirr
                  + "  适配器=" + dd.DeviceString);
      sb.AppendLine("       app 的 device_id = " + dd.DeviceName + "；PnP（app 只取 monitor index 0）= "
                  + (firstPnp.Length == 0 ? "(取不到)" : firstPnp));
      if (allPnp.Count > 1) {
        sb.AppendLine("       ** 这个适配器下实际有 " + allPnp.Count + " 个 monitor 节点，app 只取 index 0 ——");
        sb.AppendLine("          其余 " + (allPnp.Count - 1) + " 个在 app 的列表里不存在（选不中）：**");
        for (int m = 1; m < allPnp.Count; m++)
          sb.AppendLine("          index " + m + " = " + (allPnp[m].Length == 0 ? "(空)" : allPnp[m]));
      }
    }
    sb.AppendLine("  小结：Windows 报告活动适配器 " + adapters + " 个，app 会列出 " + listed + " 项。");
    return sb.ToString();
  }
  public static string IccProfiles() {
    LoadDisplays();
    var sb = new StringBuilder();
    sb.AppendLine("---- 7. ICC 配置与 vcgt ----");
    foreach (string dev in _name) {
      IntPtr h = OpenDc(dev);
      if (h == IntPtr.Zero) { sb.AppendLine("  " + dev + " : CreateDC 失败"); continue; }
      try {
        uint sz = 260;
        var buf = new StringBuilder(260);
        string p = GetIcm(h, ref sz, buf) ? buf.ToString() : "(GetICMProfile 失败)";
        sb.AppendLine("  " + dev + " 的关联 ICC = " + p);
      } finally { DeleteDC(h); }
    }
    string dir = System.IO.Path.Combine(Environment.SystemDirectory, "spool\\drivers\\color");
    sb.AppendLine("  颜色目录 = " + dir);
    string[] files = new string[0];
    try {
      var all = new List<string>();
      all.AddRange(System.IO.Directory.GetFiles(dir, "*.icm"));
      all.AddRange(System.IO.Directory.GetFiles(dir, "*.icc"));
      files = all.ToArray();
    } catch (Exception ex) {
      sb.AppendLine("  枚举失败: " + ex.Message);
      return sb.ToString();
    }
    int withVcgt = 0, broken = 0;
    var lines = new List<string>();
    foreach (string f in files) {
      byte[] d = null;
      try { d = System.IO.File.ReadAllBytes(f); } catch (Exception) { continue; }
      string nm = System.IO.Path.GetFileName(f);
      var vr = ParseVcgt(d);
      if (!vr.Valid) {
        if (vr.HasTag) { broken++; lines.Add("    ** " + nm + " : vcgt 有结构问题 —— " + vr.Error); }
        else if (vr.Error != null) lines.Add("    ** " + nm + " : 文件有问题 —— " + vr.Error);
        continue;
      }
      if (vr.Kind == "absent") continue;
      withVcgt++;
      lines.Add("    " + nm + " : " + VcgtSummary(d));
      lines.Add("        → " + AppVerdict(d, vr));
      lines.Add("        → " + IdentityVerdict(vr.Ramp));
    }
    sb.AppendLine("  目录内 " + files.Length + " 个 profile，其中 " + withVcgt + " 个带 vcgt（只有带 vcgt 的才会动显卡 LUT）、"
                + broken + " 个 vcgt 结构有问题：");
    foreach (string l in lines) sb.AppendLine(l);
    if (withVcgt == 0) sb.AppendLine("    （一个都没有 —— 那 ramp 的变化就只能来自本 app 或第三方工具）");
    return sb.ToString();
  }

  // ═══════════ 备用：把基线 ramp 导成一行，便于人工比对 ═══════════
  public static string BaselineDump() {
    LoadDisplays();
    var sb = new StringBuilder();
    foreach (string dev in _name) {
      var r = ReadRamp(dev);
      if (r == null) { sb.AppendLine("  " + dev + " 回读失败"); continue; }
      sb.AppendLine("  " + dev + " R=" + Sample(r, 0));
    }
    return sb.ToString();
  }
}
'@

# ── 主流程 ──
$sb = New-Object System.Text.StringBuilder
function Add-Line([string]$t) { [void]$sb.AppendLine($t) }
function Add-Text([string]$t) { [void]$sb.Append($t) }

try {
    Add-Line "============ 屏幕配色诊断（ICC 应用 / gamma ramp）============"
    Add-Line ("时间: " + (Get-Date -Format "yyyy-MM-dd HH:mm:ss"))
    Add-Line "说明: 本脚本只查 GDI gamma ramp 这条链 —— 亮度、对比度、伽马、RGB 增益四个滑块"
    Add-Line "      最终都写同一个调用（SetDeviceGammaRamp），所以「四个都不生效」必然是这条链的问题。"
    Add-Line "      数字振动 / 饱和度不在这里，那个走 NVAPI/ADLX，用 gpu-color-diag.bat 查。"
    # ── 第 0 节：选排查路径 ──
    Write-Host ""
    Write-Host "请选择要排查的问题：" -ForegroundColor Cyan
    Write-Host "  1) 点「应用 ICC」没反应 / 有些 ICC 应用不了（ICC 专线，流程短）" -ForegroundColor Cyan
    Write-Host "  2) 拖亮度/对比度/伽马/RGB 滑条没反应或报错（完整流程）" -ForegroundColor Cyan
    Write-Host "输入 1 或 2（直接回车 = 2）：" -ForegroundColor Cyan
    $modeIn = Read-Host
    $iccMode = ($modeIn -eq '1')
    Add-Line ("排查路径: " + $(if ($iccMode) { "1 - ICC 专线（点应用 ICC 没反应）" } else { "2 - 完整流程（滑条没反应/报错）" }))
    Add-Line ""
    if ($iccMode) {
        Add-Line "  注意：ICC 专线全程请【保持 Filter-Manage 运行】，不要退出 —— 要点它里面的 ICC。"
        Add-Line "        写入测试（第 5 节）会跳过，因为那一步要求先退出 app。"
        Add-Line ""
    }
    Add-Line ""

    Add-Text ([GammaDiag]::EnvInfo())
    Add-Line ""
    Add-Text ([GammaDiag]::Displays())
    Add-Text ([GammaDiag]::DisplayTopology())
    Add-Line ""
    Add-Text ([GammaDiag]::AppMonitorList())
    Add-Line ""
    Add-Line ""

    # ── 第 2b 节：显示适配器设备状态（幽灵适配器 / 驱动异常）──
    Add-Line "---- 2b. 显示适配器设备状态 ----"
    $devs = $null
    try { $devs = Get-PnpDevice -Class Display -ErrorAction Stop } catch { }
    if ($devs) {
        $i = 0
        foreach ($d in $devs) {
            Add-Line ("  [" + $i + "] 状态=" + $d.Status + "   名称=" + $d.FriendlyName)
            Add-Line ("       实例 = " + $d.InstanceId)
            $i++
        }
        Add-Line ("  显示适配器设备数 = " + $i + "（正常一般是 2 个：核显 + 独显。明显更多 = 驱动装过很多遍、留了一堆失效实例）")
        Add-Line "  （状态=Unknown 的一般是以前装过、现在不存在的显卡实例，也就是「幽灵适配器」）"
    } else {
        Add-Line "  Get-PnpDevice 不可用，改读注册表："
        $cls = "HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}"
        $n = 0
        Get-ChildItem $cls -ErrorAction SilentlyContinue | Where-Object { $_.PSChildName -match '^\d{4}$' } | ForEach-Object {
            $dp = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
            Add-Line ("  [" + $_.PSChildName + "] " + $dp.DriverDesc + "   厂商=" + $dp.ProviderName + "   驱动版本=" + $dp.DriverVersion)
            $n++
        }
        Add-Line ("  显示适配器实例数 = " + $n)
    }
    try {
        $vc = Get-CimInstance Win32_VideoController -ErrorAction Stop
        foreach ($v in $vc) {
            Add-Line ("  显卡: " + $v.Name + "  驱动=" + $v.DriverVersion + "  状态=" + $v.Status +
                      "  当前模式=" + $v.CurrentHorizontalResolution + "x" + $v.CurrentVerticalResolution)
        }
    } catch {
        Add-Line "  Win32_VideoController 查询失败"
    }
    Add-Line ""

    if (-not $iccMode) {
    # ── 第 2c 节：全屏优化 / GameDVR 状态 ──
    # 决定游戏跑的是"真独占全屏"还是"全屏优化(flip-model 无边框)"。
    # 真独占全屏会绕开桌面色彩管线 —— 那桌面 LUT（我们的亮度/伽马）在游戏里就不生效，
    # 写入还可能被直接拒（SetDeviceGammaRamp 返回 FALSE）。这两个开关就是分水岭：
    #   HKCU\System\GameConfigStore\GameDVR_DXGIHonorFSEWindowsCompatible = 1 → 全局真 FSE
    #   逐 exe 的"禁用全屏优化"(DISABLEDXMAXIMIZEDWINDOWEDMODE)          → 只影响那一个游戏
    Add-Line "---- 2c. 全屏优化 / GameDVR 状态（决定游戏是不是「真独占全屏」）----"
    $gcs = "HKCU:\System\GameConfigStore"
    foreach ($nm in @("GameDVR_DXGIHonorFSEWindowsCompatible", "GameDVR_FSEBehaviorMode",
                      "GameDVR_HonorUserFSEBehaviorMode", "GameDVR_Enabled")) {
        try {
            $val = (Get-ItemProperty -Path $gcs -Name $nm -ErrorAction Stop).$nm
            $extra = ""
            if ($nm -eq "GameDVR_DXGIHonorFSEWindowsCompatible" -and $val -eq 1) {
                $extra = "   ← ** 1 = 所有游戏都会变成真独占全屏，桌面色彩管理在游戏里失效（重点嫌疑）**"
            }
            Add-Line ("  " + $nm + " = " + $val + $extra)
        } catch {
            Add-Line ("  " + $nm + " = (没有这个值)")
        }
    }
    foreach ($root in @("HKCU:\Software\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers",
                        "HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion\AppCompatFlags\Layers")) {
        $any = $false
        try {
            $lay = Get-ItemProperty -Path $root -ErrorAction Stop
            foreach ($p in $lay.PSObject.Properties) {
                if ($p.Name -like "PS*") { continue }
                if ($p.Name -notlike "*:\*") { continue }
                $flag = ""
                if (([string]$p.Value) -like "*DISABLEDXMAXIMIZEDWINDOWEDMODE*") {
                    $flag = "   ← ** 这个程序被设了「禁用全屏优化」= 它会跑真独占全屏（重点嫌疑）**"
                }
                Add-Line ("  兼容性标记 [" + $p.Name + "] = " + $p.Value + $flag)
                $any = $true
            }
        } catch { }
        if (-not $any) { Add-Line ("  " + $root + " : 没有逐程序的兼容性标记") }
    }
    foreach ($pol in @(@("HKLM:\SOFTWARE\Microsoft\PolicyManager\default\ApplicationManagement", "AllowGameDVR"),
                       @("HKLM:\SOFTWARE\Policies\Microsoft\Windows\GameDVR", "AllowGameDVR"))) {
        try {
            $pv = (Get-ItemProperty -Path $pol[0] -Name $pol[1] -ErrorAction Stop).($pol[1])
            Add-Line ("  策略 " + $pol[0] + "\" + $pol[1] + " = " + $pv)
        } catch { }
    }
    # 逐程序的 GPU 指派（混合显卡笔记本上关键：游戏被指派到哪块 GPU 渲染）
    # GpuPreference: 1=省电(GPU) 2=高性能(GPU) 0/缺省=让系统决定
    try {
        $gpu = Get-ItemProperty -Path "HKCU:\Software\Microsoft\DirectX\UserGpuPreferences" -ErrorAction Stop
        $n = 0
        foreach ($p in $gpu.PSObject.Properties) {
            if ($p.Name -like "PS*") { continue }
            if ($p.Name -notlike "*:\*") { continue }
            Add-Line ("  GPU 指派 [" + $p.Name + "] = " + $p.Value)
            $n++
        }
        if ($n -eq 0) { Add-Line "  GPU 指派: 没有任何逐程序指派（全部由系统决定）" }
    } catch {
        Add-Line "  GPU 指派: 没有 UserGpuPreferences 项（没有任何逐程序指派）"
    }
    Add-Line ""

    }

    # ── 第 3 节：基线 ──
    Write-Host ""
    Write-Host "【第 1 步】先别动 Filter-Manage。" -ForegroundColor Cyan
    Write-Host "按回车，我记录一次当前屏幕的 gamma ramp 作为基线。" -ForegroundColor Cyan
    [void](Read-Host)
    Add-Text ([GammaDiag]::CaptureBaseline())
    Add-Line ""

    if ($iccMode) {
        # ── 第 4 节（ICC 专线）：点一次「应用 ICC」的前后对照 ──
        Write-Host ""
        Write-Host "【第 2 步 —— 关键】现在回到 Filter-Manage，点一次那个「应用不了」的 ICC。" -ForegroundColor Yellow
        Write-Host "  1) 先选中出问题的那台显示器（如果列表里压根没有它，记住这件事）" -ForegroundColor Yellow
        Write-Host "  2) 点一次那个有问题的 ICC 文件名 —— 只点一次，不要拖任何滑条" -ForegroundColor Yellow
        Write-Host "  3) 记住两件事：【屏幕变没变】+【有没有弹报错，原文是什么】" -ForegroundColor Yellow
        Write-Host "点完之后回到本窗口按回车（输入 s 跳过这一步）：" -ForegroundColor Yellow
        $skipIcc = Read-Host
        if ($skipIcc -ne 's') {
            Add-Text ([GammaDiag]::CaptureCompare())
            Add-Line ""
            Add-Line "---- 4-ICC. 点「应用 ICC」的结果 ----"
            Write-Host ">>> 你点的是哪个 ICC 文件？（文件名，不确定就填「不知道」）" -ForegroundColor Cyan
            $iccName = Read-Host
            Add-Line ("  点击的 ICC: " + $iccName)
            Write-Host ">>> 屏幕有没有变化？（有 / 没有 / 只有一点点）" -ForegroundColor Cyan
            $iccObs = Read-Host
            Add-Line ("  屏幕变化: " + $iccObs)
            Write-Host ">>> 有没有弹报错？原文一字不差抄下来（没有就填「没有」）：" -ForegroundColor Cyan
            $iccErr = Read-Host
            Add-Line ("  报错原文: " + $iccErr)
            Write-Host ">>> 出问题那台显示器，在 app 的显示器列表里能选中吗？（能 / 列表里没有它 / 没注意）" -ForegroundColor Cyan
            $iccPick = Read-Host
            Add-Line ("  能否选中出问题的显示器: " + $iccPick)
            Write-Host ">>> 换一个别的 ICC 点一次，屏幕有变化吗？（有 / 还是没有 / 没试）" -ForegroundColor Cyan
            $iccOther = Read-Host
            Add-Line ("  换别的 ICC: " + $iccOther)
            Write-Host ">>> 点完之后那一项的选中高亮，有没有落到你点的 ICC 上？（有 / 没反应 / 没注意）" -ForegroundColor Cyan
            $iccMark = Read-Host
            Add-Line ("  选中状态是否更新: " + $iccMark)
            Add-Line ""
        } else {
            Add-Line "---- 4-ICC. 用户跳过了点击对照 ----"
            Add-Line ""
        }
    } else {
    # ── 第 4 节：用户拖动滑条后的对照 ──
    $appNow = (Get-Process -Name "Filter-Manage" -ErrorAction SilentlyContinue | Measure-Object).Count
    Write-Host ""
    Write-Host "【第 2 步 —— 关键】请在 Filter-Manage 里挨个试一遍。" -ForegroundColor Yellow
    Write-Host "每一步都记住两件事：【屏幕变没变】+【弹没弹报错】。" -ForegroundColor Yellow
    Write-Host "  1) 打开 Filter-Manage（如果没开），选中有问题的那台显示器" -ForegroundColor Yellow
    Write-Host "  2) 亮度：先拖到最左（-100%），再拖到最右（+100%）" -ForegroundColor Yellow
    Write-Host "  3) 对比度：拖到最左，再拖到最右" -ForegroundColor Yellow
    Write-Host "  4) 伽马：拖到 2.00 附近，再拖到 0.50 附近" -ForegroundColor Yellow
    Write-Host "  5) RGB 三个滑条：各拖几个来回" -ForegroundColor Yellow
    Write-Host "  6) 数字振动：拖到 80 附近（这条走 NVAPI，是另一条链，用来对照）" -ForegroundColor Yellow
    Write-Host "  7) 最后应用一次你平时用的、带 ICC 的那个预设，记住报错（如果有）" -ForegroundColor Yellow
    if ($appNow -eq 0) { Write-Host "  （注意：刚才没检测到 Filter-Manage 在运行）" -ForegroundColor DarkYellow }
    Write-Host "全部试完后回到本窗口按回车（输入 s 跳过这一步）：" -ForegroundColor Yellow
    $skip = Read-Host
    if ($skip -ne 's') {
        Add-Text ([GammaDiag]::CaptureCompare())
        Add-Line ""
        Add-Line "---- 4b. 用户观察与报错（对应第 2 步的 7 小步）----"
        Write-Host ">>> Q1 拖的时候屏幕到底有没有变化？（有 / 没有 / 只有一点点）" -ForegroundColor Cyan
        $observe1 = Read-Host
        Add-Line ("  Q1 屏幕有没有变化: " + $observe1)
        Write-Host ">>> Q2 哪些滑条会弹报错？（例如「亮度对比度伽马都报错」/「只有亮度」/「都不报错」）" -ForegroundColor Cyan
        $whichErr = Read-Host
        Add-Line ("  Q2 会报错的滑条: " + $whichErr)
        Write-Host ">>> Q3 亮度往【负数】方向拖（变暗）也报错吗？（也报错 / 只有往正数拖才报错 / 没注意）" -ForegroundColor Cyan
        $negErr = Read-Host
        Add-Line ("  Q3 往负方向拖: " + $negErr)
        Write-Host ">>> Q4 报错原文，一字不差抄下来（没有就填「没有」）：" -ForegroundColor Cyan
        $errtext = Read-Host
        Add-Line ("  Q4 拖动时报错原文: " + $errtext)
        Write-Host ">>> Q5 应用带 ICC 的预设时，报错原文是什么？（没报错填「没报错」，没试填「没试」）" -ForegroundColor Cyan
        $iccErr = Read-Host
        Add-Line ("  Q5 应用 ICC 预设的报错原文: " + $iccErr)
        Write-Host ">>> Q6 数字振动那条滑条拖起来正常吗？（正常 / 也报错 / 没试）" -ForegroundColor Cyan
        $dvcOk = Read-Host
        Add-Line ("  Q6 数字振动: " + $dvcOk)
        Add-Line ("  拖动前后 Filter-Manage 进程数: 第2步时=" + $appNow + "  现在=" + (Get-Process -Name "Filter-Manage" -ErrorAction SilentlyContinue | Measure-Object).Count)
        Add-Line ""
    }

    # ── 第 4c 节：重启 app 对照（能区分「app 内存里的状态坏了」和「系统层面就是写不进去」）──
    Write-Host ""
    Write-Host "【第 2.5 步 —— 只在有报错时做，但很关键】重启对照实验：" -ForegroundColor Cyan
    Write-Host "  1) 完全退出 Filter-Manage（托盘图标右键 → 退出，别只关窗口）" -ForegroundColor Cyan
    Write-Host "  2) 重新打开它，什么都别设置，直接拖【亮度】" -ForegroundColor Cyan
    Write-Host "  3) 记住还报不报错" -ForegroundColor Cyan
    $doRestart = Read-Host "做完后按回车（不想做就输入 s 跳过）"
    if ($doRestart -ne 's') {
        Write-Host ">>> 重启之后直接拖亮度，结果是？（不报错能调 / 还是报错 / 变正常了）" -ForegroundColor Cyan
        $afterRestart = Read-Host
        Add-Line "---- 4c. 重启 app 后再拖亮度 ----"
        Add-Line ("  重启后的结果: " + $afterRestart)
        Write-Host ">>> 如果重启后正常：再把你平时用的那个预设（带 ICC 的那个）应用一次，然后再拖亮度，结果如何？" -ForegroundColor Cyan
        $afterIcc = Read-Host "（正常 / 又报错 / 没试，然后按回车）"
        Add-Line ("  应用 ICC 预设后再拖亮度: " + $afterIcc)
        Add-Line ""
    }

    # ── 第 5 节：本进程写入测试 ──
    Write-Host ""
    Write-Host "【第 3 步】开始前请把 Filter-Manage 完全退出（托盘图标右键 → 退出，别只关窗口）。" -ForegroundColor Magenta
    Write-Host "这一步测的是「这台机器本身能不能往显卡写 gamma」，要排除 app 的干扰。" -ForegroundColor Magenta
    Write-Host "然后我会写 4 条测试 ramp（恒等 / 亮度+100% / 轻微非单调 / gamma 2.4）。" -ForegroundColor Magenta
    Write-Host "屏幕会变暗约 3 秒，然后自动恢复 —— 这是正常的。" -ForegroundColor Magenta
    Write-Host "注意：这 3 秒内请不要关掉本窗口（真关了也坏不了，注销或重启即可恢复）。" -ForegroundColor DarkYellow
    Write-Host "按回车开始：" -ForegroundColor Magenta
    [void](Read-Host)
    $appDuringWrite = (Get-Process -Name "Filter-Manage" -ErrorAction SilentlyContinue | Measure-Object).Count
    Add-Line ("  （写入测试期间 Filter-Manage 进程数 = " + $appDuringWrite + "，0 才是干净环境）")
    Add-Text ([GammaDiag]::ApplyWriteTests())
    Add-Line ""
    Write-Host ""
    Write-Host ">>> 看屏幕：现在屏幕应该明显变暗了，保持 3 秒……" -ForegroundColor Magenta
    Start-Sleep -Seconds 3
    Write-Host ">>> 正在恢复原样……" -ForegroundColor Magenta
    Add-Text ([GammaDiag]::FinishWriteTests())
    Add-Line ""
    Write-Host ""
    Write-Host ">>> 刚才那 3 秒，屏幕到底变暗了没有？" -ForegroundColor Cyan
    $observe2 = Read-Host "（变暗了 / 完全没变，然后按回车）"
    Add-Line "---- 5a. 用户主观观察（脚本写入 gamma 2.4 的那 3 秒）----"
    Add-Line ("  用户回答: " + $observe2)
    Write-Host ""
    }

    # ── 4d. 共用问答（两种模式都问）──
    Write-Host ""
    Write-Host ">>> 脚本在 2d 节已经检测过每台显示器的 HDR/高级颜色状态。" -ForegroundColor Cyan
    Write-Host "    请核对：有没有哪台屏 HDR 实际开着、但 2d 里显示「已启用=False」？" -ForegroundColor Cyan
    Write-Host "    （没有就填「没有」；有就写「显示器X 开着」）" -ForegroundColor Cyan
    $hdr = Read-Host
    Add-Line ("  HDR 核对: " + $hdr)
    Write-Host ">>> 你现在是在这台机器上直接操作，还是远程连过来的？（本机 / 远程桌面 / 向日葵 / ToDesk / 云电脑 / 虚拟机）" -ForegroundColor Cyan
    $remote = Read-Host
    Add-Line ("  操作方式: " + $remote)
    Write-Host ">>> 症状出现在哪一台显示器上？（只有一台就填「只有一台」）" -ForegroundColor Cyan
    $which = Read-Host
    Add-Line ("  症状所在显示器: " + $which)
    if (-not $iccMode) {
        Write-Host ">>> Q7 出问题的那个游戏，画面设置里选的是「全屏」还是「无边框窗口/全屏窗口」？" -ForegroundColor Cyan
        $gmode = Read-Host
        Add-Line ("  Q7 游戏显示模式: " + $gmode)
        Write-Host ">>> Q8 游戏内的分辨率和刷新率，跟桌面一致吗？（一致 / 不一致，游戏里是 xxx）" -ForegroundColor Cyan
        $gres = Read-Host
        Add-Line ("  Q8 游戏分辨率/刷新率: " + $gres)
        Write-Host ">>> Q9 换成「无边框窗口」再试，亮度能生效吗？（能 / 还是不行 / 没试）" -ForegroundColor Cyan
        $gborderless = Read-Host
        Add-Line ("  Q9 无边框窗口下: " + $gborderless)
    }
    Write-Host ">>> 一句话描述症状（例如「拖亮度没反应」/「点了 ICC 没反应」）：" -ForegroundColor Cyan
    $symptom = Read-Host
    Add-Line ("  症状描述: " + $symptom)
    Add-Line ""

    # ── 第 6 节：谁在抢 ramp ──
    Add-Line "---- 6. 夜间模式 / 可能占用显卡 LUT 的程序 ----"
    try {
        $bl = "HKCU:\Software\Microsoft\Windows\CurrentVersion\CloudStore\Store\DefaultAccount\Current\default`$windows.data.bluelightreduction.bluelightreductionstate"
        $p = Get-ItemProperty -Path $bl -ErrorAction Stop
        if ($null -ne $p.Data) {
            $hex = ($p.Data | ForEach-Object { $_.ToString("X2") }) -join ' '
            $b18 = if ($p.Data.Length -gt 18) { $p.Data[18] } else { -1 }
            Add-Line ("  夜间模式注册表: 有数据  长度=" + $p.Data.Length + "  byte[18]=0x" + ("{0:X2}" -f $b18) + "（0x10=关，0x15=开）")
            Add-Line ("  原始前 32 字节: " + (($hex -split ' ')[0..([Math]::Min(31, ($hex -split ' ').Count - 1))] -join ' '))
        } else {
            Add-Line "  夜间模式注册表: 存在但无 Data 值"
        }
    } catch {
        Add-Line "  夜间模式注册表: 没找到（一般代表没开过夜间模式）"
    }
    $kw = 'gamma|calib|color|flux|eye|redshift|gammy|护眼|蓝光|night'
    $hits = Get-Process -ErrorAction SilentlyContinue | Where-Object {
        ($_.Name -match $kw) -or ($_.Description -match $kw)
    } | Select-Object -First 20
    if ($hits) {
        Add-Line "  名字里带 gamma/color/calib/护眼/蓝光… 的进程（可能就是抢 LUT 的那个）："
        foreach ($h in $hits) {
            $desc = $h.Description; if (-not $desc) { $desc = "" }
            Add-Line ("    " + $h.Name + "  pid=" + $h.Id + "  " + $desc)
        }
    } else {
        Add-Line "  没有名字可疑的常驻进程"
    }
    # 显示器色彩管理相关计划任务（校准加载器就是它把 ICC 的 vcgt 写回 LUT）
    try {
        $task = Get-ScheduledTask -TaskPath "\Microsoft\Windows\WindowsColorSystem\" -ErrorAction Stop
        if ($task) {
            foreach ($t in $task) { Add-Line ("  计划任务: " + $t.TaskPath + $t.TaskName + "  状态=" + $t.State) }
        } else {
            Add-Line "  WindowsColorSystem 下没有计划任务"
        }
    } catch {
        Add-Line "  查不到 WindowsColorSystem 计划任务（校准加载器不存在或不可见）"
    }
    Add-Line ""

    # ── 第 7 节：ICC ──
    Add-Text ([GammaDiag]::IccProfiles())
    Add-Line ""

    # ── 第 8 节：Filter-Manage 自身 ──
    Add-Line "---- 8. Filter-Manage 自身 ----"
    $procs = Get-Process -Name "Filter-Manage" -ErrorAction SilentlyContinue
    if ($procs) {
        foreach ($p in $procs) {
            $ver = ""
            try { $ver = (Get-Item $p.Path).VersionInfo.FileVersion } catch { }
            Add-Line ("  进程: pid=" + $p.Id + "  路径=" + $p.Path + "  版本=" + $ver)
        }
    } else {
        Add-Line "  进程: 没在运行"
    }
    foreach ($root in @("HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall",
                        "HKLM:\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
                        "HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall")) {
        Get-ChildItem $root -ErrorAction SilentlyContinue | ForEach-Object {
            $it = Get-ItemProperty $_.PSPath -ErrorAction SilentlyContinue
            if ($it.DisplayName -like "*Filter*Manage*") {
                # 注册表里的 InstallLocation 常自带一对引号，先剥掉
                $loc = ([string]$it.InstallLocation).Trim('"')
                Add-Line ("  已安装: " + $it.DisplayName + "  版本=" + $it.DisplayVersion + "  位置=" + $loc)
                # 进程没在跑的时候也把 exe 版本读出来（写入测试前用户会被要求退出 app）
                if ($loc.Length -gt 0) {
                    $exe = Join-Path $loc "Filter-Manage.exe"
                    if (Test-Path $exe) {
                        try { Add-Line ("    exe 版本 = " + (Get-Item $exe).VersionInfo.FileVersion) } catch { }
                    }
                }
            }
        }
    }
    $cfg = Join-Path $env:APPDATA "filter-manage\app.json"
    Add-Line ("  配置文件: " + $cfg)
    if (Test-Path $cfg) {
        try {
            $j = Get-Content $cfg -Raw -Encoding UTF8 | ConvertFrom-Json
            Add-Line ("  格式版本=" + $j.version + "  默认预设=" + $j.default_preset)
            foreach ($preset in $j.presets) {
                $icc = $preset.icc_profile; if (-not $icc) { $icc = "(无)" }
                Add-Line ("    预设 [" + $preset.name + "] 亮度=" + $preset.brightness + " 对比度=" + $preset.contrast +
                          " 伽马=" + $preset.gamma + " 数字振动=" + $preset.digital_vibrance +
                          " RGB=" + $preset.rgb_r + "/" + $preset.rgb_g + "/" + $preset.rgb_b + " ICC=" + $icc)
            }
            $rules = $j.settings.process_rules
            if ($rules) {
                Add-Line ("    进程规则: " + (($rules | ForEach-Object { $_.process_name + "→" + $_.config_name + "(enabled=" + $_.enabled + ")" }) -join ", "))
            }
        } catch {
            Add-Line ("  读取 app.json 失败: " + $_)
        }
    } else {
        Add-Line "  app.json 不存在（没跑过或装在别的用户下）"
    }
    Add-Line ""
    Add-Line "============ 结束 ============"

    $result = $sb.ToString()
    Write-Host ""
    Write-Host $result
    $desktop = [Environment]::GetFolderPath("Desktop")
    $ts = Get-Date -Format "yyyyMMdd-HHmmss"
    $outFile = Join-Path $desktop "gamma-ramp-diag-$ts.txt"
    [System.IO.File]::WriteAllText($outFile, $result, [System.Text.Encoding]::UTF8)
    Write-Host ""
    Write-Host "诊断报告已保存到桌面: $outFile" -ForegroundColor Green
    Write-Host "请把桌面上的这个 txt 发回给我。" -ForegroundColor Green
} catch {
    Write-Host "脚本执行出错: $_" -ForegroundColor Red
    Write-Host $_.Exception.StackTrace
    exit 1
}
