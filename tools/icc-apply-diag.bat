@echo off
rem ============================================================
rem  icc-apply-diag.bat - read-only ICC apply diagnostic
rem
rem  READ-ONLY BY DESIGN:
rem    * never writes a gamma ramp (only GetDeviceGammaRamp)
rem    * never installs or associates an ICC profile
rem    * never touches the registry
rem    * never opens a network connection
rem    * never launches another process
rem  It is safe to run with Filter-Manage still open.
rem
rem  Usage: double-click, or drag the problem .icc/.icm file onto it.
rem
rem  The PowerShell half lives after the #PS_START sentinel line and is
rem  re-read from this same file as UTF-8, so no second file is needed.
rem ============================================================
setlocal enabledelayedexpansion
chcp 65001 >nul
set "ps1=%temp%\icc-apply-diag-%random%.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$c=[IO.File]::ReadAllLines('%~f0',[Text.Encoding]::UTF8);$i=0;while($i -lt $c.Length -and $c[$i] -notlike '#PS_START*'){$i++};if($i -ge $c.Length){exit 1};[IO.File]::WriteAllLines('%ps1%',$c[($i+1)..($c.Length-1)],[Text.Encoding]::UTF8);exit 0"
if errorlevel 1 (echo PowerShell prepare failed & pause & exit /b 1)
powershell -NoProfile -ExecutionPolicy Bypass -File "%ps1%" %*
set "rc=%errorlevel%"
del "%ps1%" >nul 2>&1
if not "%rc%"=="0" pause
exit /b %rc%
#PS_START
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [Text.Encoding]::UTF8

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Security.Principal;

public static class IccDiag {
  // ═══════════ Win32（全部只读）═══════════
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
  [DllImport("gdi32.dll", EntryPoint="GetICMProfileW", CharSet=CharSet.Unicode, SetLastError=true)]
  static extern bool GetIcm(IntPtr hdc, ref uint size, StringBuilder name);

  // ── 显示拓扑 / 高级颜色（只声明 Get 变体）──
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
  [DllImport("user32.dll")]
  static extern int QueryDisplayConfig(uint flags, ref uint numPath,
      [Out] DISPLAYCONFIG_PATH_INFO[] paths, ref uint numMode, [Out] byte[] modes, IntPtr topo);
  [DllImport("user32.dll")]
  static extern int DisplayConfigGetDeviceInfo(ref DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO info);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)]
  static extern int DisplayConfigGetDeviceInfo(ref DISPLAYCONFIG_SOURCE_DEVICE_NAME info);

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

  public static bool LayoutsValid() {
    return Marshal.SizeOf(typeof(DISPLAYCONFIG_DEVICE_INFO_HEADER)) == 20
        && Marshal.SizeOf(typeof(DISPLAYCONFIG_PATH_SOURCE_INFO)) == 20
        && Marshal.SizeOf(typeof(DISPLAYCONFIG_PATH_TARGET_INFO)) == 48
        && Marshal.SizeOf(typeof(DISPLAYCONFIG_PATH_INFO)) == 68
        && Marshal.SizeOf(typeof(DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO)) == 32
        && Marshal.SizeOf(typeof(DISPLAYCONFIG_SOURCE_DEVICE_NAME)) == 84;
  }

  // ═══════════ ICC / vcgt ═══════════
  // 判定基准是 src-tauri/src/icc.rs 的 parse_vcgt：要回答的是
  // 「app 拿到这个 ICC 会往 LUT 写什么」，不是「规范上这个标签长什么样」。
  // 两者不一致处（单通道、反向曲线、标签长度声明错误）都必须报出来 ——
  // 那正是「有些 ICC 应用不了」的候选原因。
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

  // ═══════════ ramp 比较（只读高字节）═══════════
  static ushort[] IdentityRamp() {
    var r = new ushort[768];
    for (int c = 0; c < 3; c++)
      for (int i = 0; i < 256; i++) r[c * 256 + i] = (ushort)(i * 257);
    return r;
  }
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
    var id = IdentityRamp();
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
  static string Sample(ushort[] r, int ch) {
    int[] pts = { 0, 1, 16, 64, 128, 192, 240, 254, 255 };
    var sb = new StringBuilder();
    for (int k = 0; k < pts.Length; k++) {
      sb.Append(pts[k]); sb.Append(":"); sb.Append(r[ch * 256 + pts[k]] >> 8);
      if (k < pts.Length - 1) sb.Append(" ");
    }
    return sb.ToString();
  }
  static string Shape(ushort[] r) {
    int d = MaxHighByteDiff(r, IdentityRamp());
    if (d == 0) return "恒等（当前没有任何程序在写这台显示器的 LUT）";
    return "非恒等（有程序写过 LUT：ICC / 校准加载器 / 夜间模式 / 第三方，或本 app 自己写的）";
  }

  // ═══════════ 显示器 ═══════════
  static readonly List<string> _name = new List<string>();
  static void LoadDisplays() {
    if (_name.Count > 0) return;
    for (uint i = 0; i < 32; i++) {
      var dd = new DISPLAY_DEVICE();
      dd.cb = Marshal.SizeOf(typeof(DISPLAY_DEVICE));
      if (!EnumDisplayDevicesW(null, i, ref dd, 0)) break;
      if ((dd.StateFlags & 0x1) == 0) continue;
      _name.Add(dd.DeviceName);
    }
  }
  static ushort[] ReadRamp(string dev) {
    IntPtr h = CreateDCW(dev, null, null, IntPtr.Zero);
    if (h == IntPtr.Zero) return null;
    try {
      var r = new ushort[768];
      if (!GetRamp(h, r)) return null;
      return r;
    } finally { DeleteDC(h); }
  }
  static string ReadIcc(string dev) {
    IntPtr h = CreateDCW(dev, null, null, IntPtr.Zero);
    if (h == IntPtr.Zero) return "(CreateDC 失败)";
    try {
      uint sz = 260;
      var buf = new StringBuilder(260);
      return GetIcm(h, ref sz, buf) ? buf.ToString() : "(GetICMProfile 失败)";
    } finally { DeleteDC(h); }
  }

  // ═══════════ 各节输出 ═══════════
  public static string Env() {
    var sb = new StringBuilder();
    sb.AppendLine("---- 1. 环境 ----");
    bool admin = false;
    try { admin = new WindowsPrincipal(WindowsIdentity.GetCurrent()).IsInRole(WindowsBuiltInRole.Administrator); }
    catch (Exception) { }
    sb.AppendLine("  系统         = " + Environment.OSVersion);
    sb.AppendLine("  64 位进程    = " + Environment.Is64BitProcess + "   管理员 = " + admin);
    sb.AppendLine("  远程桌面会话 = " + (GetSystemMetrics(0x1000) != 0 ? "是（远程会话里 gamma 写入本来就不生效）" : "否"));
    try {
      uint mySid = 0;
      ProcessIdToSessionId((uint)System.Diagnostics.Process.GetCurrentProcess().Id, out mySid);
      uint console = WTSGetActiveConsoleSessionId();
      sb.AppendLine("  进程会话 ID  = " + mySid + "   物理控制台会话 ID = "
        + (console == 0xFFFFFFFF ? "无" : console.ToString()));
      if (console != 0xFFFFFFFF && mySid != console)
        sb.AppendLine("    ** 脚本不在物理控制台会话里：gamma 写入很可能被系统拒绝（重点嫌疑）**");
      else
        sb.AppendLine("    会话一致（本地控制台会话）");
    } catch (Exception ex) { sb.AppendLine("  会话检查失败: " + ex.Message); }
    return sb.ToString();
  }

  /// 用户拖进来的那个 ICC 解析出的曲线，供 5b / 6 节比对用
  static ushort[] _iccRamp;

  /// 分析用户拖进来的那个 ICC —— 这一节不需要任何交互，直接给结论。
  public static string AnalyzeIcc(string path) {    var sb = new StringBuilder();
    sb.AppendLine("---- 2. 你拖进来的 ICC 文件 ----");
    if (string.IsNullOrEmpty(path)) {
      sb.AppendLine("  没有提供文件路径（可以把这个 .bat 直接拖到 ICC 文件上再试）");
      return sb.ToString();
    }
    sb.AppendLine("  文件 = " + path);
    if (!System.IO.File.Exists(path)) { sb.AppendLine("  ** 文件不存在 **"); return sb.ToString(); }
    var fi = new System.IO.FileInfo(path);
    sb.AppendLine("  大小 = " + fi.Length + " 字节   修改时间 = " + fi.LastWriteTime.ToString("yyyy-MM-dd HH:mm:ss"));
    byte[] d = null;
    try { d = System.IO.File.ReadAllBytes(path); }
    catch (Exception ex) { sb.AppendLine("  ** 读取失败: " + ex.Message + " **"); return sb.ToString(); }

    var r = ParseVcgt(d);
    _iccRamp = r.Valid ? r.Ramp : null;
    if (!r.Valid) {
      if (r.HasTag) {
        sb.AppendLine("  ** vcgt 有结构问题：" + r.Error + " **");
        sb.AppendLine("     → app 判定：" + AppVerdict(d, r));
      } else {
        sb.AppendLine("  ** 文件本身有问题：" + r.Error + " **");
      }
      return sb.ToString();
    }
    if (r.Kind == "absent") {
      sb.AppendLine("  vcgt 标签 = 没有");
      sb.AppendLine("     → app 判定：" + AppVerdict(d, r));
      sb.AppendLine("     ** 这就是「点了没反应」最常见的原因：profile 里没有 vcgt 标签，");
      sb.AppendLine("        app 只会做一次 WCS 注册，完全不会去动显卡 LUT，画面自然不变。 **");
      return sb.ToString();
    }
    if (r.Kind == "table") {
      sb.AppendLine("  vcgt = table  channels=" + r.Channels + " entries=" + r.EntryCount
                  + " entrySize=" + r.EntrySize);
    } else if (r.Kind == "formula") {
      sb.AppendLine("  vcgt = formula（交错布局 gamma/min/max）");
      for (int c = 0; c < 3; c++)
        sb.AppendLine("    " + "RGB"[c] + " : gamma=" + r.Gamma[c].ToString("F4")
                    + "  min=" + r.Min[c].ToString("F4") + "  max=" + r.Max[c].ToString("F4"));
    }
    if (r.TagSizeMismatch)
      sb.AppendLine("  ** 标签声明的长度与实际数据不符：" + r.Error + " **");
    sb.AppendLine("  app 判定：" + AppVerdict(d, r));
    sb.AppendLine("  曲线判定：" + IdentityVerdict(r.Ramp));
    if (r.Ramp != null) {
      sb.AppendLine("  解析出的曲线（高字节 0-255）：");
      sb.AppendLine("    R = " + Sample(r.Ramp, 0));
      sb.AppendLine("    G = " + Sample(r.Ramp, 1));
      sb.AppendLine("    B = " + Sample(r.Ramp, 2));
    }
    return sb.ToString();
  }

  /// 复刻 app 的 get_display_monitors：适配器枚举 + 第二层只取 monitor index 0。
  public static string AppMonitors() {
    var sb = new StringBuilder();
    sb.AppendLine("---- 3. Filter-Manage 的显示器列表（复刻 get_display_monitors 的枚举方式）----");
    sb.AppendLine("  用户在 app 里能选到的就是下面这几项；和 2d 的活动路径对照，差集就是选不中的部分。");
    int adapters = 0, listed = 0;
    for (uint i = 0; i < 32; i++) {
      var dd = new DISPLAY_DEVICE();
      dd.cb = Marshal.SizeOf(typeof(DISPLAY_DEVICE));
      if (!EnumDisplayDevicesW(null, i, ref dd, 0)) break;
      if ((dd.StateFlags & 0x1) == 0) continue;
      adapters++;
      bool prim = (dd.StateFlags & 0x4) != 0;
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
      sb.AppendLine("  [" + listed + "] " + dd.DeviceName + "  主屏=" + prim + "  适配器=" + dd.DeviceString);
      sb.AppendLine("       app 的 device_id = " + dd.DeviceName + "；PnP（app 只取 monitor index 0）= "
                  + (firstPnp.Length == 0 ? "(取不到)" : firstPnp));
      if (allPnp.Count > 1) {
        sb.AppendLine("       ** 这个适配器下实际有 " + allPnp.Count + " 个 monitor 节点，app 只取 index 0 ——");
        sb.AppendLine("          其余 " + (allPnp.Count - 1) + " 个在 app 的列表里根本不存在（选不中）：**");
        for (int m = 1; m < allPnp.Count; m++)
          sb.AppendLine("          index " + m + " = " + (allPnp[m].Length == 0 ? "(空)" : allPnp[m]));
      }
    }
    sb.AppendLine("  小结：Windows 报告活动适配器 " + adapters + " 个，app 会列出 " + listed + " 项。");
    return sb.ToString();
  }

  public static string Topology() {
    var sb = new StringBuilder();
    sb.AppendLine("---- 4. 显示拓扑 / HDR（高级颜色）----");
    sb.AppendLine("  （这一节直接决定「ICC 有没有可能生效」：克隆组和 HDR 都会让写入静默失效）");
    if (!LayoutsValid()) {
      sb.AppendLine("  ** 结构体尺寸与 SDK ABI 不符，本节结果不可信（不猜，标未知）**");
      return sb.ToString();
    }
    uint np = 0, nm = 0;
    int err = GetDisplayConfigBufferSizes(2, out np, out nm);
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
      sn.header.type = 1;
      sn.header.size = (uint)Marshal.SizeOf(typeof(DISPLAYCONFIG_SOURCE_DEVICE_NAME));
      sn.header.adapterId = p.sourceInfo.adapterId; sn.header.id = p.sourceInfo.id;
      string gdi = "(取不到)";
      try { if (DisplayConfigGetDeviceInfo(ref sn) == 0) gdi = sn.viewGdiDeviceName; } catch (Exception) { }

      var ac = new DISPLAYCONFIG_GET_ADVANCED_COLOR_INFO();
      ac.header.type = 9;
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
          hdr = "支持=" + supported + " 已启用=" + enabled
              + " 宽色域强制=" + ((ac.value & 0x4) != 0) + " 被强制关闭=" + ((ac.value & 0x8) != 0)
              + " 位深=" + ac.bitsPerColorChannel + "bit";
          if (enabled) hdr += "  ** HDR 已开：gamma ramp 写入和 WCS 关联都会被系统忽略 —— 这就是没效果的原因 **";
          else if (supported) hdr += "（支持但未开，不挡写入）";
        }
      } catch (Exception ex) { hdr = "未知（" + ex.GetType().Name + "）"; }

      sb.AppendLine("  [" + i + "] GDI=" + gdi + "  输出=" + OutputTech(p.targetInfo.outputTechnology)
                  + "  刷新率=" + p.targetInfo.refreshRate.Numerator + "/" + p.targetInfo.refreshRate.Denominator);
      sb.AppendLine("       HDR/高级颜色: " + hdr);
      if (gdiSeen.ContainsKey(gdi)) gdiSeen[gdi]++;
      else gdiSeen[gdi] = 1;
    }
    int cloned = 0;
    foreach (var kv in gdiSeen) if (kv.Value > 1 && kv.Key != "(取不到)") cloned++;
    if (cloned > 0) {
      sb.AppendLine("  ** 检测到克隆/复制模式：有 " + cloned + " 个 GDI 设备各对应多路输出 ——");
      sb.AppendLine("     同一个 GDI 设备名背后挂着多块屏，按设备名选 ICC 会同时命中它们。**");
    } else {
      sb.AppendLine("  未发现多路输出共用一个 GDI 设备（不是克隆模式，或者是扩展模式）");
    }
    return sb.ToString();
  }

  // ═══════════ 前后快照（只读回读）═══════════
  static readonly Dictionary<string, ushort[]> _baseRamp = new Dictionary<string, ushort[]>();
  static readonly Dictionary<string, string> _baseIcc = new Dictionary<string, string>();

  public static string Snapshot() {
    LoadDisplays();
    var sb = new StringBuilder();
    sb.AppendLine("---- 5. 点击「应用 ICC」之前的快照 ----");
    bool any = false;
    foreach (string dev in _name) {
      any = true;
      string icc = ReadIcc(dev);
      var r = ReadRamp(dev);
      _baseIcc[dev] = icc;
      _baseRamp[dev] = r;
      sb.AppendLine("  " + dev);
      sb.AppendLine("    当前关联 ICC = " + icc);
      if (r == null) sb.AppendLine("    gamma ramp 回读失败（驱动不支持回读，只能靠肉眼判断）");
      else {
        sb.AppendLine("    ramp R = " + Sample(r, 0));
        sb.AppendLine("    判定：" + Shape(r));
      }
    }
    if (!any) sb.AppendLine("  没有已连接的活动显示器 —— 这个状态本身就不正常");
    return sb.ToString();
  }

  public static string Compare() {
    LoadDisplays();
    var sb = new StringBuilder();
    sb.AppendLine("---- 6. 点击「应用 ICC」之后的对比 ----");
    foreach (string dev in _name) {
      var now = ReadRamp(dev);
      string iccNow = ReadIcc(dev);
      string iccBefore = _baseIcc.ContainsKey(dev) ? _baseIcc[dev] : "(基线缺失)";
      var rBefore = _baseRamp.ContainsKey(dev) ? _baseRamp[dev] : null;
      sb.AppendLine("  " + dev);
      if (iccNow == iccBefore) sb.AppendLine("    关联 ICC 没变 = " + iccNow);
      else sb.AppendLine("    ** 关联 ICC 变了：\n       之前 = " + iccBefore + "\n       现在 = " + iccNow + " **");
      if (now == null || rBefore == null) {
        sb.AppendLine("    gamma ramp 无法比较（某一侧回读失败）");
        continue;
      }
      int d = MaxHighByteDiff(now, rBefore);
      if (d == 0) sb.AppendLine("    gamma ramp 没有变化（高字节完全一致）");
      else sb.AppendLine("    ** gamma ramp 变了：最大差 " + d + "/255 **");
      sb.AppendLine("    ramp R = " + Sample(now, 0));
    }
    return sb.ToString();
  }

  /// 画面上现在跑的是不是就是这个 ICC 的曲线 —— 是的话「再点一次没反应」是必然的。
  public static string IccAlreadyActive() {
    LoadDisplays();
    var sb = new StringBuilder();
    sb.AppendLine("---- 5b. 这个 ICC 现在是不是已经生效了 ----");
    if (_iccRamp == null) {
      sb.AppendLine("  这个 ICC 没有可用的 vcgt 曲线（没有标签 / 解析失败），无法比对");
      return sb.ToString();
    }
    foreach (string dev in _name) {
      var live = ReadRamp(dev);
      if (live == null) { sb.AppendLine("  " + dev + " : ramp 回读失败，无法比对"); continue; }
      int d = MaxHighByteDiff(live, _iccRamp);
      if (d == 0)
        sb.AppendLine("  " + dev + " : ** 画面上跑的就是这个 ICC 的曲线（逐点一致）——"
                    + " 它本来就已经生效了，再点一次当然看不出任何变化 **");
      else
        sb.AppendLine("  " + dev + " : 画面当前曲线和这个 ICC 不同（最大差 " + d + "/255）");
    }
    return sb.ToString();
  }

  /// 连续采样：把「没写」和「写了又被抢回去」区分开。
  /// 两次快照做不到这件事 —— 抢占者只要在用户点完、切回终端之前改回去，头尾就都一样。
  public static string Burst(int seconds) {
    LoadDisplays();
    var sb = new StringBuilder();
    sb.AppendLine("---- 6. 点击期间连续采样（每 250ms 一次，共 " + seconds + " 秒）----");
    if (_name.Count == 0) { sb.AppendLine("  没有已连接的活动显示器"); return sb.ToString(); }
    var hdc = new Dictionary<string, IntPtr>();
    foreach (string dev in _name) hdc[dev] = CreateDCW(dev, null, null, IntPtr.Zero);
    try {
      var marks = new Dictionary<string, List<long>>();
      var states = new Dictionary<string, List<ushort[]>>();
      foreach (string dev in _name) { marks[dev] = new List<long>(); states[dev] = new List<ushort[]>(); }
      var sw = System.Diagnostics.Stopwatch.StartNew();
      long total = seconds * 1000L;
      while (sw.ElapsedMilliseconds < total) {
        foreach (string dev in _name) {
          if (hdc[dev] == IntPtr.Zero) continue;
          var r = new ushort[768];
          if (!GetRamp(hdc[dev], r)) continue;
          var list = states[dev];
          if (list.Count == 0 || MaxHighByteDiff(r, list[list.Count - 1]) > 0) {
            list.Add(r);
            marks[dev].Add(sw.ElapsedMilliseconds);
          }
        }
        long left = (total - sw.ElapsedMilliseconds + 999) / 1000;
        try { Console.Write("\r  采样中… 还剩 " + left + " 秒   "); } catch (Exception) { }
        System.Threading.Thread.Sleep(250);
      }
      try { Console.Write("\r                                        \r"); } catch (Exception) { }
      foreach (string dev in _name) {
        sb.AppendLine("  " + dev);
        var list = states[dev];
        if (list.Count == 0) { sb.AppendLine("    ramp 回读失败，本节无数据"); continue; }
        var baseRamp = _baseRamp.ContainsKey(dev) ? _baseRamp[dev] : null;
        bool sawChange = false, endsAtBase = false;
        for (int i = 0; i < list.Count; i++) {
          string rel = "(基线缺失)";
          if (baseRamp != null) {
            int d = MaxHighByteDiff(list[i], baseRamp);
            if (d == 0) rel = "= 基线（和点之前一样）";
            else { rel = "≠ 基线，最大差 " + d + "/255"; sawChange = true; }
          }
          string vsIcc = (_iccRamp != null && MaxHighByteDiff(list[i], _iccRamp) == 0)
                       ? "  ← 正是你拖进来的这个 ICC 的曲线" : "";
          sb.AppendLine("      [" + i + "] t=" + (marks[dev][i] / 1000.0).ToString("F2") + "s  " + rel + vsIcc);
          sb.AppendLine("          R = " + Sample(list[i], 0));
        }
        if (baseRamp != null) endsAtBase = MaxHighByteDiff(list[list.Count - 1], baseRamp) == 0;
        if (!sawChange)
          sb.AppendLine("    ** 结论：整段采样期间 ramp 一次都没变 —— app 没有写入，或写入被同一瞬间挡掉 **");
        else if (endsAtBase && list.Count > 1)
          sb.AppendLine("    ** 结论：ramp 变了又变回原样 —— 有别的程序在抢 LUT 并把它改了回来 **");
        else
          sb.AppendLine("    ** 结论：ramp 变了并保持住了 —— 写入生效；若肉眼仍看不出，看第 2 节的曲线判定 **");
      }
    } finally {
      foreach (var kv in hdc) if (kv.Value != IntPtr.Zero) DeleteDC(kv.Value);
    }
    return sb.ToString();
  }

  public static string Luttakers() {
    var sb = new StringBuilder();
    sb.AppendLine("---- 8. 谁可能在占用显卡 LUT ----");
    try {
      var hits = new List<string>();
      foreach (var p in System.Diagnostics.Process.GetProcesses()) {
        try {
          string nm = p.ProcessName;
          string ds = "";
          try { ds = p.MainModule != null ? (p.MainModule.FileVersionInfo.FileDescription ?? "") : ""; } catch (Exception) { }
          string s = (nm + " " + ds).ToLowerInvariant();
          if (s.Contains("gamma") || s.Contains("calib") || s.Contains("color") ||
              s.Contains("flux") || s.Contains("redshift") || s.Contains("gammy") ||
              s.Contains("night") || s.Contains("bluelight"))
            hits.Add("    " + nm + "  pid=" + p.Id + "  " + ds);
        } catch (Exception) { }
      }
      if (hits.Count > 0) {
        sb.AppendLine("  名字里带 gamma/color/calib/night… 的进程（可能就是抢 LUT 的那个）：");
        foreach (string h in hits) sb.AppendLine(h);
      } else {
        sb.AppendLine("  没有名字可疑的常驻进程");
      }
    } catch (Exception ex) { sb.AppendLine("  进程枚举失败: " + ex.Message); }
    return sb.ToString();
  }
}
'@

function Add-Line([string]$t) { [void]$sb.AppendLine($t) }
function Add-Text([string]$t) { [void]$sb.Append($t) }

$sb = New-Object System.Text.StringBuilder

# ── 机器类型 / 显卡驱动健康 / LUT 抢占者（全部只读：WMI + 进程/服务枚举）──
# 为什么放这里：DisplayCAL 有记录在案的先例 —— SetDeviceGammaRamp 到处失败，
# 根因是显示适配器驱动没装好（设备管理器里显示成 "Generic PnP device"），
# 重装驱动后立刻恢复。所以"驱动健不健康"要机器可读地采出来，不能让用户截图。
function Get-ErrCodeText([int]$code) {
    switch ($code) {
        0  { '工作正常' }
        1  { '设备未正确配置' }
        3  { '驱动损坏或系统内存不足' }
        10 { '设备无法启动' }
        12 { '资源不足' }
        14 { '需要重启才能生效' }
        18 { '需要重新安装驱动' }
        19 { '注册表配置损坏' }
        22 { '设备已被禁用' }
        24 { '设备未安装或未连接（历史遗留记录，通常无害）' }
        28 { '** 驱动未安装 **（设备管理器里会显示成通用/未知设备）' }
        31 { '** Windows 无法加载该设备所需的驱动 **' }
        43 { '驱动报告设备故障，已被系统停止' }
        45 { '设备当前未连接（历史遗留记录，通常无害）' }
        default { "未知错误码 $code" }
    }
}

function Get-MachineProfile {
    $o = New-Object System.Text.StringBuilder
    [void]$o.AppendLine("---- 4b. 机器类型 / 显卡驱动健康 / LUT 抢占者（脚本自动采集）----")

    # ① 机型：判断是不是「混合显卡笔记本」这种高发场景
    try {
        $cs = Get-CimInstance Win32_ComputerSystem -ErrorAction Stop
        $ptMap = @{ 1 = '台式'; 2 = '笔记本'; 3 = '工作站'; 4 = '企业服务器'; 5 = '小型办公服务器'; 6 = '中型服务器'; 7 = '大型服务器'; 8 = '小型/掌上设备' }
        $pt = $ptMap[[int]$cs.PCSystemType]
        if (-not $pt) { $pt = "未知($($cs.PCSystemType))" }
        [void]$o.AppendLine("  机型 = $pt    厂商 = $($cs.Manufacturer)    型号 = $($cs.Model)")
    } catch { [void]$o.AppendLine("  机型读取失败: $_") }
    try {
        $chMap = @{ 3 = '台式'; 4 = '低profile台式'; 5 = 'PizzaBox'; 6 = 'MiniTower'; 7 = 'Tower'; 8 = '便携'; 9 = '笔记本'; 10 = '笔记本'; 11 = '手持'; 12 = '扩展坞'; 13 = '一体机'; 14 = '超便携'; 16 = 'MainSystemChassis'; 17 = 'MainSystemChassis'; 18 = '扩展机箱'; 21 = '外设机箱'; 23 = '密封机箱'; 30 = '平板'; 31 = '可翻转'; 32 = '可拆卸' }
        foreach ($e in (Get-CimInstance Win32_SystemEnclosure -ErrorAction Stop)) {
            $names = ($e.ChassisTypes | ForEach-Object { $k = [int]$_; if ($chMap.ContainsKey($k)) { $chMap[$k] } else { "类型$k" } }) -join '/'
            [void]$o.AppendLine("  机箱类型 = $names")
        }
    } catch { }
    $bat = @(Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue)
    [void]$o.AppendLine("  电池 = " + $(if ($bat.Count -gt 0) { "有 $($bat.Count) 个 → 笔记本/一体机（混合显卡场景高发）" } else { "无 → 台式或一体机" }))
    [void]$o.AppendLine("")

    # ② 显示适配器驱动健康 —— "驱动装坏了"那条线索的直接证据
    # 问题码 45（未连接）/ 24（未安装·未连接）是「以前插过、现在不在」的幽灵记录：每台机器都有
    # （本机实测：1 个旧显卡 + 6 个旧显示器全是 45），属正常现象。把它们报成故障会让报告满屏
    # 假警报、把人引向错误方向 —— 所以只有真正影响使用的错误码才报警。
    [void]$o.AppendLine("  ── 显示适配器 ──")
    $badCodes = @(1, 3, 10, 12, 14, 18, 19, 21, 22, 28, 31, 43)
    $anyBad = $false
    $ghostAdapter = 0

    $pnp = $null
    try { $pnp = Get-PnpDevice -Class Display -ErrorAction Stop } catch { }
    if ($pnp) {
        foreach ($d in $pnp) {
            $pr = 0
            try { $pr = [int]$d.Problem } catch { }
            if ($badCodes -contains $pr) {
                $anyBad = $true
                $desc = $d.ProblemDescription
                if (-not $desc) { $desc = Get-ErrCodeText $pr }
                [void]$o.AppendLine("   · $($d.FriendlyName)")
                [void]$o.AppendLine("       InstanceId = $($d.InstanceId)")
                [void]$o.AppendLine("       ** 驱动异常！问题码 $pr = $desc **")
            } elseif ($d.Status -eq 'OK') {
                [void]$o.AppendLine("   · [正常] $($d.FriendlyName)")
                [void]$o.AppendLine("       InstanceId = $($d.InstanceId)")
            } else {
                $ghostAdapter++
            }
        }
    } else {
        foreach ($d in @(Get-CimInstance Win32_PnPEntity -Filter "PNPClass='Display'" -ErrorAction SilentlyContinue)) {
            $pr = 0
            try { $pr = [int]$d.ConfigManagerErrorCode } catch { }
            if ($badCodes -contains $pr) {
                $anyBad = $true
                [void]$o.AppendLine("   · $($d.Name)")
                [void]$o.AppendLine("       PNPDeviceID = $($d.PNPDeviceID)")
                [void]$o.AppendLine("       ** 驱动异常！问题码 $pr = $(Get-ErrCodeText $pr) **")
            } elseif ($pr -eq 0) {
                [void]$o.AppendLine("   · [正常] $($d.Name)")
            } else {
                $ghostAdapter++
            }
        }
    }
    if ($ghostAdapter -gt 0) {
        [void]$o.AppendLine("   （另有 $ghostAdapter 个历史遗留的旧适配器记录，未连接状态，属正常现象）")
    }
    foreach ($v in @(Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue)) {
        [void]$o.AppendLine("   · 控制器: $($v.Name)   驱动版本 = $($v.DriverVersion)   驱动日期 = $($v.DriverDate)")
    }

    # 显示器（Monitor 类）只详列当前活动的；幽灵记录仅计数
    $mon = $null
    try { $mon = Get-PnpDevice -Class Monitor -ErrorAction Stop } catch { }
    if ($mon) {
        $ghostMon = 0
        foreach ($d in $mon) {
            $pr = 0
            try { $pr = [int]$d.Problem } catch { }
            if ($badCodes -contains $pr) {
                $anyBad = $true
                [void]$o.AppendLine("   · [显示器类] $($d.FriendlyName)   ** 驱动异常！问题码 $pr **")
            } elseif ($d.Status -eq 'OK') {
                [void]$o.AppendLine("   · [显示器类] [活动] $($d.FriendlyName)")
            } else {
                $ghostMon++
            }
        }
        if ($ghostMon -gt 0) {
            [void]$o.AppendLine("   （另有 $ghostMon 个未连接的历史显示器记录，属正常现象）")
        }
    }

    if ($anyBad) {
        [void]$o.AppendLine("  ** 上面标「驱动异常」的设备是真的有问题 —— 建议干净重装显卡驱动后再试 **")
    } else {
        [void]$o.AppendLine("  显卡与显示器的驱动状态均正常（没有影响使用的错误码）")
    }
    [void]$o.AppendLine("")

    # ③ 谁在抢 LUT / 谁在拦写入 —— 把「让用户关掉再试」这件事自动化
    # loud=$true  才是真正的嫌疑对象，会触发「请退出后重跑」的指令
    # loud=$false 只是告知（例如 NVIDIA 自己的容器服务，任何 N 卡机器都有，不该叫人去关它）
    # 分层的原因：一旦对必然常驻的东西也喊「请关掉」，每份报告都会带这句，等于没有信号。
    [void]$o.AppendLine("  ── 可能抢占 LUT 或拦截写入的程序 / 服务 ──")
    $contenders = @(
        @{ n = '火绒安全';              p = 'HipsTray|HipsDaemon|HipsMain|usysdiag|wsctrl|huorong'; loud = $true },
        @{ n = 'MSI Dragon Center';     p = 'Dragon ?Center|MSI ?Dragon|DCv2'; loud = $true },
        @{ n = 'MSI True Color';        p = 'True ?Color|TrueColor'; loud = $true },
        @{ n = '其它杀软/安全软件';      p = '360|QQPCTray|QQPCRTP|kxetray|kav|avp|zhudongfangyu|ZhuDongFangYu|McAfee|Norton|Avast|ESET|Kaspersky|Comodo|Bitdefender'; loud = $true },
        @{ n = 'f.lux / 色温工具';       p = 'flux|redshift|Gammy|lightbulb'; loud = $true },
        @{ n = 'DisplayCAL / Argyll';   p = 'displaycal|argyll|dispwin'; loud = $true },
        @{ n = '远程/串流（会改写 LUT）'; p = 'SunloginClient|ToDesk|oray|AnyDesk|TeamViewer|parsec|mstsc'; loud = $true },
        @{ n = '灯效/同步类（会 hook 显示链路）'; p = 'SignalRGB|OpenRGB|iCUE|Corsair|Aura|LightingService|Synapse|RGB ?Fusion|Mystic ?Light'; loud = $true },
        @{ n = 'NVIDIA 驱动组件（正常存在，无需处理）'; p = 'NVDisplay\.Container|nvcontainer|nvcpl|NVIDIA ?App'; loud = $false }
    )
    $procs = @(Get-Process -ErrorAction SilentlyContinue)
    $svcs = @(Get-Service -ErrorAction SilentlyContinue)
    $loudHit = $false
    $anyHit = $false
    foreach ($c in $contenders) {
        $pm = @($procs | Where-Object { $_.Name -match $c.p -or ($_.Description -and $_.Description -match $c.p) })
        $sm = @($svcs | Where-Object { ($_.Name -match $c.p -or $_.DisplayName -match $c.p) -and $_.Status -eq 'Running' })
        if ($pm.Count -gt 0 -or $sm.Count -gt 0) {
            $anyHit = $true
            if ($c.loud) { $loudHit = $true }
            [void]$o.AppendLine("   ▸ $($c.n)")
            foreach ($p in $pm) { [void]$o.AppendLine("       进程: $($p.Name)  pid=$($p.Id)") }
            foreach ($s in $sm) { [void]$o.AppendLine("       服务: $($s.Name)  [$($s.Status)]  $($s.DisplayName)") }
        }
    }
    if ($loudHit) {
        [void]$o.AppendLine("")
        [void]$o.AppendLine("   ** 上面列出的请全部退出后，重新运行本脚本一次，对比两次报告 **")
        [void]$o.AppendLine("   （能退出就行，不必卸载；**服务型的必须在 services.msc 里停掉才算真退出** ——")
        [void]$o.AppendLine("     只关托盘图标的话，服务会继续把 LUT 改回它自己的值）")
    } elseif ($anyHit) {
        [void]$o.AppendLine("")
        [void]$o.AppendLine("   （以上都是显卡驱动自带组件，属正常常驻，不需要处理）")
    } else {
        [void]$o.AppendLine("   没有检测到已知的 LUT 抢占者 / 安全软件在运行")
    }
    [void]$o.AppendLine("   （注：这条只扫进程名和服务名，扫不到的第三方工具需要人工留意）")

    return $o.ToString()
}

try {
    Add-Line "================================================================"
    Add-Line "  ICC 应用诊断报告（只读：不写 gamma、不应用 ICC、不改注册表、不联网）"
    Add-Line "================================================================"
    Add-Line ("时间: " + (Get-Date -Format "yyyy-MM-dd HH:mm:ss"))
    Add-Line ""

    Add-Text ([IccDiag]::Env())
    Add-Line ""

    # 拖进来的文件路径（双击运行为空）—— 没有这个文件的报告基本没用，所以反复要
    $iccPath = ""
    if ($args.Count -ge 1) { $iccPath = ([string]$args[0]).Trim().Trim('"') }
    while ($true) {
        if ($iccPath -eq 's') { $iccPath = ""; break }
        if ($iccPath -ne "" -and (Test-Path -LiteralPath $iccPath)) { break }
        if ($iccPath -ne "") { Write-Host "找不到这个文件: $iccPath" -ForegroundColor Red }
        Write-Host "请把那个「应用不了」的 ICC 文件拖到本窗口里，然后按回车：" -ForegroundColor Cyan
        Write-Host "（也可以直接把 ICC 文件拖到这个 .bat 上运行；确实拿不到就输入 s 跳过）" -ForegroundColor DarkGray
        $iccPath = ([string](Read-Host)).Trim().Trim('"')
    }
    Add-Text ([IccDiag]::AnalyzeIcc($iccPath))
    Add-Line ""

    Add-Text ([IccDiag]::AppMonitors())
    Add-Line ""
    Add-Text ([IccDiag]::Topology())
    Add-Line ""
    Add-Text (Get-MachineProfile)
    Add-Line ""
    Add-Text ([IccDiag]::Snapshot())
    Add-Line ""
    Add-Text ([IccDiag]::IccAlreadyActive())
    Add-Line ""

    Write-Host ""
    Write-Host "【关键一步】按回车后，脚本会连续采样 20 秒。" -ForegroundColor Yellow
    Write-Host "请在这 20 秒内切到 Filter-Manage：选中出问题的那台显示器，" -ForegroundColor Yellow
    Write-Host "点一次那个有问题的 ICC 文件名 —— 只点一次，不要拖任何滑条。" -ForegroundColor Yellow
    Write-Host "（脚本全程只读，app 照常开着就行，不用退出。）" -ForegroundColor DarkYellow
    Write-Host "准备好就按回车，然后立刻去点：" -ForegroundColor Yellow
    [void](Read-Host)
    Add-Text ([IccDiag]::Burst(20))
    Add-Line ""
    Add-Text ([IccDiag]::Compare())
    Add-Line ""

    Add-Line "---- 7. 你的观察 ----"
    Write-Host ">>> Q1 你点的是哪个 ICC？（直接回车 = 就用你拖进来那个）" -ForegroundColor Cyan
    $q1 = Read-Host
    if ($q1 -eq "") { $q1 = $iccPath }
    Add-Line ("  Q1 点击的 ICC: " + $q1)
    Write-Host ">>> Q2 那 20 秒里屏幕有没有变化？（有 / 没有 / 只有一点点）" -ForegroundColor Cyan
    Add-Line ("  Q2 屏幕变化: " + (Read-Host))
    Write-Host ">>> Q3 有没有弹报错？原文一字不差抄下来（没有就填「没有」）：" -ForegroundColor Cyan
    Add-Line ("  Q3 报错原文: " + (Read-Host))
    Write-Host ">>> Q4 出问题那台显示器，在 app 的显示器列表里能选中吗？（能 / 列表里没有它 / 没注意）" -ForegroundColor Cyan
    Add-Line ("  Q4 能否选中: " + (Read-Host))
    Write-Host ">>> Q5 换一个别的 ICC 点一次，屏幕有变化吗？（有 / 还是没有 / 没试）" -ForegroundColor Cyan
    Add-Line ("  Q5 换别的 ICC: " + (Read-Host))
    Write-Host ">>> Q6 你现在是直接坐在这台机器前，还是远程连过来的？（本机 / 远程桌面 / 向日葵 / ToDesk / 云电脑 / 虚拟机）" -ForegroundColor Cyan
    Add-Line ("  Q6 操作方式: " + (Read-Host))
    Write-Host ">>> Q7 症状出现在哪一台显示器上？（只有一台就填「只有一台」）" -ForegroundColor Cyan
    Add-Line ("  Q7 症状所在显示器: " + (Read-Host))
    Write-Host ">>> Q8 一句话描述症状（例如「点了没反应」/「点了弹报错」）：" -ForegroundColor Cyan
    Add-Line ("  Q8 症状描述: " + (Read-Host))
    Add-Line ""

    Add-Text ([IccDiag]::Luttakers())
    Add-Line ""

    Add-Line "---- 9. Filter-Manage 自身 ----"
    $procs = Get-Process -Name "Filter-Manage" -ErrorAction SilentlyContinue
    if ($procs) {
        foreach ($p in $procs) {
            $ver = ""
            try { $ver = (Get-Item $p.Path).VersionInfo.FileVersion } catch { }
            Add-Line ("  进程: pid=" + $p.Id + "  版本=" + $ver)
        }
    } else {
        Add-Line "  进程: 没在运行（那也是「点了没反应」的一种解释）"
    }
    Add-Line ""
    Add-Line "================================================================"
    Add-Line "  结束 —— 请把这个 txt 发回给我"
    Add-Line "================================================================"

    $result = $sb.ToString()
    Write-Host ""
    Write-Host $result
    $desktop = [Environment]::GetFolderPath("Desktop")
    $ts = Get-Date -Format "yyyyMMdd-HHmmss"
    $outFile = Join-Path $desktop "icc-apply-diag-$ts.txt"
    [System.IO.File]::WriteAllText($outFile, $result, [System.Text.Encoding]::UTF8)
    Write-Host ""
    Write-Host "诊断报告已保存到桌面: $outFile" -ForegroundColor Green
    Write-Host "请把桌面上的这个 txt 发回给我。" -ForegroundColor Green
    exit 0
} catch {
    Write-Host "脚本执行出错: $_" -ForegroundColor Red
    Write-Host $_.Exception.StackTrace
    exit 1
}
