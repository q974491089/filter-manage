@echo off
:: ===========================================================================
:: GPU color-control diagnostic  (NVIDIA NVAPI / AMD ADL+ADLX)  -- double-click to run
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
set "ps1=%temp%\gpu-color-diag-%random%.ps1"
powershell -NoProfile -ExecutionPolicy Bypass -Command "$c=[IO.File]::ReadAllLines('%~f0',[Text.Encoding]::UTF8);$i=0;while($i -lt $c.Length -and $c[$i] -notlike '#PS_START*'){$i++};if($i -ge $c.Length){exit 1};[IO.File]::WriteAllLines('%ps1%',$c[($i+1)..($c.Length-1)],[Text.Encoding]::UTF8);exit 0"
if errorlevel 1 (echo PowerShell prepare failed & pause & exit /b 1)
powershell -NoProfile -ExecutionPolicy Bypass -File "%ps1%"
set exitcode=%errorlevel%
del /q "%ps1%" 2>nul
if %exitcode% neq 0 pause
exit /b %exitcode%
#PS_START
# ── 显卡颜色控制诊断 —— PowerShell 部分 ──
#
# 上面的 bat 包装靠 #PS_START 这个哨兵行来切分，不依赖行号，
# 所以这里增删行是安全的。但不要动 #PS_START 那一行本身。

$ErrorActionPreference = 'Continue'
try { [Console]::OutputEncoding = [System.Text.Encoding]::UTF8 } catch {}

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;

public static class Diag {
  // ═══════════ Win32 显示适配器 ═══════════
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

  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  static extern IntPtr LoadLibraryW(string name);
  [DllImport("kernel32.dll", SetLastError=true)]
  static extern bool FreeLibrary(IntPtr h);
  [DllImport("kernel32.dll", CharSet=CharSet.Ansi, ExactSpelling=true, SetLastError=true)]
  static extern IntPtr GetProcAddress(IntPtr h, string proc);

  static T Fn<T>(IntPtr lib, string name) where T : class {
    IntPtr p = GetProcAddress(lib, name);
    if (p == IntPtr.Zero) return null;
    return Marshal.GetDelegateForFunctionPointer(p, typeof(T)) as T;
  }

  // ═══════════ NVIDIA / NVAPI ═══════════
  [DllImport("nvapi64.dll", EntryPoint="nvapi_QueryInterface", CallingConvention=CallingConvention.Cdecl)]
  static extern IntPtr NvQ(uint id);

  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int NvInit();
  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int NvEnum(int i, out uint h);
  [UnmanagedFunctionPointer(CallingConvention.Cdecl, CharSet=CharSet.Ansi)] delegate int NvVer(out uint v, StringBuilder b);
  [UnmanagedFunctionPointer(CallingConvention.Cdecl, CharSet=CharSet.Ansi)] delegate int NvAssoc(StringBuilder name, out uint h);

  [StructLayout(LayoutKind.Sequential)]
  public struct NvDvc { public uint version; public int cur; public int min; public int max; public int def; }
  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int NvGetDvc(uint h, uint o, ref NvDvc e);

  const uint NV_INIT=0x0150E828, NV_ENUM=0x9ABDD40D, NV_ASSOC=0x35C29134;
  const uint NV_VER=0x2926AAAD, NV_GET_EX=0x0E45002D;
  const uint NV_SET_EX=0x4A82C2B1, NV_SET_LEG=0x172409B4, NV_GET_LEG=0x4085DE45;

  static T NvFn<T>(uint id) where T : class {
    IntPtr p = NvQ(id);
    if (p == IntPtr.Zero) return null;
    return Marshal.GetDelegateForFunctionPointer(p, typeof(T)) as T;
  }
  static uint NvVerFlag { get { return (uint)(Marshal.SizeOf(typeof(NvDvc)) | (1 << 16)); } }

  // ═══════════ AMD / ADL ═══════════
  // ADL 用 (adapterIndex, displayIndex) 两级索引，和 Windows 的 \\.\DISPLAYn 编号
  // 没有对应关系 —— 靠 AdapterInfo.strDisplayName 字符串匹配才能对上。
  [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
  public delegate IntPtr AdlMalloc(int size);

  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int AdlCreate(AdlMalloc cb, int enumConnected);
  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int AdlDestroy();
  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int AdlNumAdapters(out int n);
  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int AdlAdapterInfo(IntPtr info, int size);
  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int AdlColorCaps(int ai, int di, out int caps, out int valid);
  [UnmanagedFunctionPointer(CallingConvention.Cdecl)]
  delegate int AdlColorGet(int ai, int di, int type, out int cur, out int def, out int min, out int max, out int step);

  // adl_structures.h 的 AdapterInfo，ADL_MAX_PATH = 256
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Ansi)]
  public struct AdlAdapter {
    public int iSize;
    public int iAdapterIndex;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string strUDID;
    public int iBusNumber;
    public int iDeviceNumber;
    public int iFunctionNumber;
    public int iVendorID;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string strAdapterName;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string strDisplayName;
    public int iPresent;
    public int iExist;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string strDriverPath;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string strDriverPathExt;
    [MarshalAs(UnmanagedType.ByValTStr, SizeConst=256)] public string strPNPString;
    public int iOSDisplayIndex;
  }

  // adl_defines.h
  const int ADL_COLOR_BRIGHTNESS = 1 << 0;
  const int ADL_COLOR_CONTRAST   = 1 << 1;
  const int ADL_COLOR_SATURATION = 1 << 2;
  const int ADL_COLOR_HUE        = 1 << 3;

  // 回调必须保持强引用，否则 GC 掉之后 ADL 回调进来就是野指针
  static readonly AdlMalloc _adlAlloc = size => Marshal.AllocCoTaskMem(size);

  static string ColorTypeName(int t) {
    if (t == ADL_COLOR_BRIGHTNESS) return "亮度";
    if (t == ADL_COLOR_CONTRAST)   return "对比度";
    if (t == ADL_COLOR_SATURATION) return "饱和度";
    if (t == ADL_COLOR_HUE)        return "色调";
    return "type" + t;
  }

  // ═══════════ 各节采集 ═══════════

  static string _primary = null;
  static readonly List<string> _vendors = new List<string>();

  static void SectionDisplays(StringBuilder sb) {
    sb.AppendLine("---- 1. Windows 显示适配器（EnumDisplayDevices）----");
    for (uint i = 0; i < 24; i++) {
      var dd = new DISPLAY_DEVICE();
      dd.cb = Marshal.SizeOf(typeof(DISPLAY_DEVICE));
      if (!EnumDisplayDevicesW(null, i, ref dd, 0)) break;
      bool act = (dd.StateFlags & 0x1) != 0;
      bool pri = (dd.StateFlags & 0x4) != 0;
      bool mir = (dd.StateFlags & 0x8) != 0;
      if (pri) _primary = dd.DeviceName;
      if (act && !_vendors.Contains(dd.DeviceString)) _vendors.Add(dd.DeviceString);
      sb.AppendLine(string.Format("  [{0,2}] {1,-16} 已连接={2,-5} 主屏={3,-5} 镜像={4,-5} flags=0x{5:X8}  {6}",
        i, dd.DeviceName, act, pri, mir, dd.StateFlags, dd.DeviceString));
    }
    sb.AppendLine("  主屏 = " + (_primary ?? "(未找到)"));
    sb.AppendLine("  已连接显示器所属显卡: " + (_vendors.Count == 0 ? "(无)" : string.Join(" / ", _vendors.ToArray())));
    sb.AppendLine();
  }

  static void SectionNvidia(StringBuilder sb) {
    sb.AppendLine("---- 2. NVIDIA (NVAPI) ----");
    if (!System.IO.File.Exists(System.IO.Path.Combine(Environment.SystemDirectory, "nvapi64.dll"))) {
      sb.AppendLine("  未找到 nvapi64.dll —— 本机没有 NVIDIA 驱动，数字振动不可用（这是正常的）");
      sb.AppendLine();
      return;
    }
    NvInit init = null;
    try { init = NvFn<NvInit>(NV_INIT); }
    catch (Exception ex) { sb.AppendLine("  加载 nvapi64.dll 失败: " + ex.Message); sb.AppendLine(); return; }
    if (init == null) { sb.AppendLine("  找不到 NvAPI_Initialize 入口"); sb.AppendLine(); return; }

    int st = init();
    sb.AppendLine("  NvAPI_Initialize = " + st);
    if (st != 0) { sb.AppendLine("  初始化失败，后面都不用看了"); sb.AppendLine(); return; }

    var dv = NvFn<NvVer>(NV_VER);
    if (dv != null) {
      uint ver = 0; var br = new StringBuilder(64);
      if (dv(out ver, br) == 0)
        sb.AppendLine(string.Format("  驱动版本 = {0}.{1:00}   分支 = {2}", ver / 100, ver % 100, br));
    }
    sb.AppendLine("  入口 SetDVCLevelEx=" + (NvQ(NV_SET_EX) != IntPtr.Zero)
      + " SetDVCLevel=" + (NvQ(NV_SET_LEG) != IntPtr.Zero)
      + " GetDVCInfoEx=" + (NvQ(NV_GET_EX) != IntPtr.Zero)
      + " GetAssocHandle=" + (NvQ(NV_ASSOC) != IntPtr.Zero));

    var en = NvFn<NvEnum>(NV_ENUM);
    var getEx = NvFn<NvGetDvc>(NV_GET_EX);
    int enumMax = -1;
    sb.AppendLine("  EnumNvidiaDisplayHandle（索引 → 句柄）:");
    for (int i = 0; i < 8 && en != null; i++) {
      uint h = 0;
      int es = en(i, out h);
      if (es != 0) {
        sb.AppendLine("    index " + i + " : status=" + es + (es == -7 ? " (END_ENUMERATION，到此为止)" : ""));
        break;
      }
      enumMax = i;
      sb.AppendLine(string.Format("    index {0} : handle=0x{1:X8}", i, h));
      if (getEx != null) {
        var e = new NvDvc(); e.version = NvVerFlag;
        int gs = getEx(h, 0, ref e);
        sb.AppendLine(gs == 0
          ? string.Format("              DVC: cur={0} min={1} max={2} default={3}", e.cur, e.min, e.max, e.def)
          : "              DVC: GetDVCInfoEx 失败 status=" + gs);
      }
    }

    var assoc = NvFn<NvAssoc>(NV_ASSOC);
    if (assoc != null) {
      sb.AppendLine("  GetAssociatedNvidiaDisplayHandle（设备名 → 句柄）:");
      for (int i = 1; i <= 8; i++) {
        string nm = @"\\.\DISPLAY" + i;
        uint h = 0;
        int s3 = assoc(new StringBuilder(nm, 64), out h);
        sb.AppendLine("    " + nm.PadRight(16) + " : " + (s3 == 0
          ? string.Format("handle=0x{0:X8}", h)
          : "status=" + s3 + (s3 == -6 ? " (未找到/非 NVIDIA 输出)" : "")));
      }
    }

    // 旧版按 \\.\DISPLAYn 编号当索引会不会翻车
    if (_primary != null && en != null) {
      int digits = 0;
      foreach (char c in _primary) if (char.IsDigit(c)) digits = digits * 10 + (c - '0');
      int oldIndex = Math.Max(0, digits - 1);
      uint h2 = 0;
      int es2 = en(oldIndex, out h2);
      sb.AppendLine("  [编号错位检查] 主屏 " + _primary + " → 旧逻辑索引 " + oldIndex
        + (es2 == 0 ? " → Enum 成功（这台机器不会因此失效）"
                    : " → Enum 失败 status=" + es2 + "  ** 旧版会在这里断链 **"));
      sb.AppendLine("  （NVAPI 可用索引只到 " + enumMax + "）");
    }
    sb.AppendLine();
  }

  static void SectionAmd(StringBuilder sb) {
    sb.AppendLine("---- 3. AMD (ADL) ----");
    string dll = System.IO.Path.Combine(Environment.SystemDirectory, "atiadlxx.dll");
    if (!System.IO.File.Exists(dll)) {
      sb.AppendLine("  未找到 atiadlxx.dll —— 本机没有 AMD 驱动（这是正常的）");
      sb.AppendLine();
      return;
    }
    sb.AppendLine("  找到 " + dll);

    IntPtr lib = LoadLibraryW("atiadlxx.dll");
    if (lib == IntPtr.Zero) {
      sb.AppendLine("  LoadLibrary 失败，错误码 " + Marshal.GetLastWin32Error());
      sb.AppendLine();
      return;
    }
    try {
      var create  = Fn<AdlCreate>(lib, "ADL_Main_Control_Create");
      var destroy = Fn<AdlDestroy>(lib, "ADL_Main_Control_Destroy");
      var numFn   = Fn<AdlNumAdapters>(lib, "ADL_Adapter_NumberOfAdapters_Get");
      var infoFn  = Fn<AdlAdapterInfo>(lib, "ADL_Adapter_AdapterInfo_Get");
      var capsFn  = Fn<AdlColorCaps>(lib, "ADL_Display_ColorCaps_Get");
      var colorFn = Fn<AdlColorGet>(lib, "ADL_Display_Color_Get");

      sb.AppendLine("  入口 Create=" + (create != null) + " NumAdapters=" + (numFn != null)
        + " AdapterInfo=" + (infoFn != null) + " ColorCaps=" + (capsFn != null)
        + " ColorGet=" + (colorFn != null)
        + " ColorSet=" + (GetProcAddress(lib, "ADL_Display_Color_Set") != IntPtr.Zero));

      if (create == null) { sb.AppendLine("  没有 ADL_Main_Control_Create，无法继续"); return; }
      // 第二个参数 1 = 只枚举已连接的适配器
      int cst = create(_adlAlloc, 1);
      sb.AppendLine("  ADL_Main_Control_Create = " + cst + (cst == 0 ? "" : "  << 初始化失败"));
      if (cst != 0) return;

      try {
        int n = 0;
        if (numFn != null && numFn(out n) == 0) sb.AppendLine("  适配器数量 = " + n);
        else { sb.AppendLine("  NumberOfAdapters_Get 失败"); return; }
        if (n <= 0) { sb.AppendLine("  没有适配器"); return; }

        int one = Marshal.SizeOf(typeof(AdlAdapter));
        IntPtr buf = Marshal.AllocCoTaskMem(one * n);
        try {
          // ADL 要求调用方把 iSize 填好
          for (int i = 0; i < n; i++)
            Marshal.WriteInt32(new IntPtr(buf.ToInt64() + i * one), 0, one);

          if (infoFn == null || infoFn(buf, one * n) != 0) {
            sb.AppendLine("  AdapterInfo_Get 失败");
            return;
          }
          for (int i = 0; i < n; i++) {
            var a = (AdlAdapter)Marshal.PtrToStructure(new IntPtr(buf.ToInt64() + i * one), typeof(AdlAdapter));
            sb.AppendLine(string.Format("  适配器[{0}] idx={1} vendor=0x{2:X4} present={3} exist={4}",
              i, a.iAdapterIndex, a.iVendorID, a.iPresent, a.iExist));
            sb.AppendLine("      名称       = " + (a.strAdapterName ?? "").Trim());
            sb.AppendLine("      设备名     = " + (a.strDisplayName ?? "").Trim()
              + "   OS显示索引 = " + a.iOSDisplayIndex);

            if (a.iPresent == 0) { sb.AppendLine("      (未激活，跳过颜色查询)"); continue; }

            // displayIndex 靠试：ColorCaps_Get 对无效索引会直接报错，
            // 比走 DisplayInfo_Get 那套 ADL 自己分配数组的流程简单得多
            bool anyDisplay = false;
            for (int di = 0; di < 6; di++) {
              int caps = 0, valid = 0;
              if (capsFn == null || capsFn(a.iAdapterIndex, di, out caps, out valid) != 0) continue;
              anyDisplay = true;
              bool sat = (caps & ADL_COLOR_SATURATION) != 0;
              sb.AppendLine(string.Format("      显示器[{0}] caps=0x{1:X} valid=0x{2:X}  支持饱和度={3}",
                di, caps, valid, sat ? "是" : "否"));
              foreach (int t in new int[] { ADL_COLOR_SATURATION, ADL_COLOR_BRIGHTNESS, ADL_COLOR_CONTRAST, ADL_COLOR_HUE }) {
                if ((caps & t) == 0) continue;
                int cur, def, mn, mx, step;
                if (colorFn != null && colorFn(a.iAdapterIndex, di, t, out cur, out def, out mn, out mx, out step) == 0)
                  sb.AppendLine(string.Format("          {0,-6}: cur={1} default={2} min={3} max={4} step={5}",
                    ColorTypeName(t), cur, def, mn, mx, step));
                else
                  sb.AppendLine("          " + ColorTypeName(t) + ": Color_Get 失败");
              }
            }
            if (!anyDisplay) sb.AppendLine("      没有可查询颜色能力的显示器（displayIndex 0-5 全部失败）");
          }
        } finally { Marshal.FreeCoTaskMem(buf); }
      } finally { if (destroy != null) destroy(); }
    } catch (Exception ex) {
      sb.AppendLine("  ADL 查询异常: " + ex.GetType().Name + ": " + ex.Message);
    } finally { FreeLibrary(lib); }
    sb.AppendLine();
  }

  // ═══════════ AMD / ADLX（新接口）═══════════
  // ADLX = amdadlx64.dll，AMD 推荐新项目使用的接口。C 接口是 COM 风格虚表：
  // 对象第一个成员是 pVtbl，方法按 SDK 头文件顺序排列。下面的槽位取自官方头文件，
  // 已核对 v1.0 与 v1.5 一致：
  //   IADLXSystem          : [3] GetDisplaysServices   （System 没有 Acquire/Release）
  //   IADLXDisplayServices : [1] Release [3] GetNumberOfDisplays [4] GetDisplays [16] GetCustomColor
  //   IADLXDisplayList     : [1] Release [3] Size [11] At_DisplayList
  //   IADLXDisplay         : [1] Release [6] Name [13] UniqueId
  //   IADLXCustomColor     : [1] Release
  //                          [3..5] 色调 Is/Range/Get  [7..9] 饱和度  [11..13] 亮度
  //                          [15..17] 对比度  [19..21] 色温
  // 虚表方法是 __stdcall、入口函数是 __cdecl；x64 下二者 ABI 相同。adlx_bool 是 1 字节。

  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int AdlxInitFn(ulong version, out IntPtr ppSystem);
  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int AdlxQueryVerFn(out ulong fullVersion);
  [UnmanagedFunctionPointer(CallingConvention.Cdecl)] delegate int AdlxTerminateFn();

  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int  VOutPtr(IntPtr pThis, out IntPtr pp);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int  VPtrOutPtr(IntPtr pThis, IntPtr p, out IntPtr pp);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int  VUintOutPtr(IntPtr pThis, uint i, out IntPtr pp);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate uint VRetUint(IntPtr pThis);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int  VRetInt(IntPtr pThis);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int  VOutUint(IntPtr pThis, out uint v);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int  VOutUlong(IntPtr pThis, out ulong v);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int  VRefInt(IntPtr pThis, ref int v);
  [UnmanagedFunctionPointer(CallingConvention.StdCall)] delegate int  VOutRange(IntPtr pThis, out AdlxIntRange r);

  [StructLayout(LayoutKind.Sequential)]
  public struct AdlxIntRange { public int minValue; public int maxValue; public int step; }

  // ADLX_MAKE_FULL_VER(1, 5, 0, 124) —— 本脚本依据的 SDK 版本
  const ulong ADLX_SDK_VERSION = (1UL << 48) | (5UL << 32) | (0UL << 16) | 124UL;

  static readonly string[] AdlxResultNames = {
    "OK","ALREADY_ENABLED","ALREADY_INITIALIZED","FAIL","INVALID_ARGS","BAD_VER","UNKNOWN_INTERFACE",
    "TERMINATED","ADL_INIT_ERROR","NOT_FOUND","INVALID_OBJECT","ORPHAN_OBJECTS","NOT_SUPPORTED",
    "PENDING_OPERATION","GPU_INACTIVE","GPU_IN_USE","TIMEOUT_OPERATION","NOT_ACTIVE","RESET_NEEDED" };
  static string R(int r) {
    return (r >= 0 && r < AdlxResultNames.Length) ? r + " (ADLX_" + AdlxResultNames[r] + ")" : r.ToString();
  }
  static string AdlxVer(ulong v) {
    return string.Format("{0}.{1}.{2}.{3}", (v >> 48) & 0xFFFF, (v >> 32) & 0xFFFF, (v >> 16) & 0xFFFF, v & 0xFFFF);
  }

  // 读对象虚表第 index 个槽并转成委托
  static T Slot<T>(IntPtr pObj, int index) where T : class {
    if (pObj == IntPtr.Zero) return null;
    IntPtr vtbl = Marshal.ReadIntPtr(pObj);
    if (vtbl == IntPtr.Zero) return null;
    IntPtr fn = Marshal.ReadIntPtr(vtbl, index * IntPtr.Size);
    if (fn == IntPtr.Zero) return null;
    return Marshal.GetDelegateForFunctionPointer(fn, typeof(T)) as T;
  }
  static void AdlxRelease(IntPtr pObj) {
    var rel = Slot<VRetInt>(pObj, 1);
    if (rel != null) rel(pObj);
  }

  // 探测 CustomColor 的一项：isIdx = Is*Supported 槽位，其后紧跟 Range 与 Get
  static void ProbeColorItem(StringBuilder sb, IntPtr pCC, string label, int isIdx) {
    var isFn = Slot<VRefInt>(pCC, isIdx);
    if (isFn == null) { sb.AppendLine("          " + label + ": 虚表槽位为空"); return; }
    int sup = 0;                                 // 用 4 字节缓冲接 1 字节的 adlx_bool，只看低字节
    int r1 = isFn(pCC, ref sup);
    bool supported = (sup & 0xFF) != 0;
    string line = string.Format("          {0,-3}: 支持={1}", label,
      r1 == 0 ? (supported ? "是" : "否") : "查询失败 " + R(r1));
    if (r1 == 0 && supported) {
      var rangeFn = Slot<VOutRange>(pCC, isIdx + 1);
      var getFn   = Slot<VRefInt>(pCC, isIdx + 2);
      if (rangeFn != null) {
        AdlxIntRange rg;
        int r2 = rangeFn(pCC, out rg);
        line += r2 == 0
          ? string.Format("  范围=[{0},{1}] step={2}", rg.minValue, rg.maxValue, rg.step)
          : "  Range失败 " + R(r2);
      }
      if (getFn != null) {
        int cur = 0;
        int r3 = getFn(pCC, ref cur);
        line += r3 == 0 ? "  当前=" + cur : "  Get失败 " + R(r3);
      }
    }
    sb.AppendLine(line);
  }

  static void SectionAdlx(StringBuilder sb) {
    sb.AppendLine("---- 4. AMD (ADLX 新接口) ----");
    string dll = System.IO.Path.Combine(Environment.SystemDirectory, "amdadlx64.dll");
    if (!System.IO.File.Exists(dll)) {
      sb.AppendLine("  未找到 amdadlx64.dll —— 驱动太旧（ADLX 需要 Adrenalin 22.x 以上）或非 AMD 显卡");
      sb.AppendLine();
      return;
    }
    sb.AppendLine("  找到 " + dll);

    IntPtr lib = LoadLibraryW("amdadlx64.dll");
    if (lib == IntPtr.Zero) {
      sb.AppendLine("  LoadLibrary 失败，错误码 " + Marshal.GetLastWin32Error());
      sb.AppendLine();
      return;
    }

    IntPtr pSystem = IntPtr.Zero;
    AdlxTerminateFn terminate = null;
    try {
      var queryVer   = Fn<AdlxQueryVerFn>(lib, "ADLXQueryFullVersion");
      var init       = Fn<AdlxInitFn>(lib, "ADLXInitialize");
      var initIncomp = Fn<AdlxInitFn>(lib, "ADLXInitializeWithIncompatibleDriver");
      terminate      = Fn<AdlxTerminateFn>(lib, "ADLXTerminate");
      sb.AppendLine("  入口 QueryFullVersion=" + (queryVer != null) + " Initialize=" + (init != null)
        + " InitWithIncompatibleDriver=" + (initIncomp != null) + " Terminate=" + (terminate != null));
      if (init == null) { sb.AppendLine("  没有 ADLXInitialize，无法继续"); return; }

      ulong runtimeVer = 0;
      if (queryVer != null) {
        int rv = queryVer(out runtimeVer);
        sb.AppendLine("  驱动自带的 ADLX 版本 = " + (rv == 0 ? AdlxVer(runtimeVer) : "查询失败 " + R(rv))
          + "   （脚本按 SDK " + AdlxVer(ADLX_SDK_VERSION) + " 头文件编写）");
      }

      // 版本协商：先按 SDK 版本；不行就退回驱动自己报的版本；再不行走"不兼容驱动"入口
      int ir = init(ADLX_SDK_VERSION, out pSystem);
      sb.AppendLine("  ADLXInitialize(" + AdlxVer(ADLX_SDK_VERSION) + ") = " + R(ir));
      if (ir != 0 && runtimeVer != 0) {
        ir = init(runtimeVer, out pSystem);
        sb.AppendLine("  ADLXInitialize(" + AdlxVer(runtimeVer) + ") = " + R(ir));
      }
      if (ir != 0 && initIncomp != null) {
        ir = initIncomp(runtimeVer != 0 ? runtimeVer : ADLX_SDK_VERSION, out pSystem);
        sb.AppendLine("  ADLXInitializeWithIncompatibleDriver = " + R(ir));
      }
      if (ir != 0 || pSystem == IntPtr.Zero) {
        sb.AppendLine("  ADLX 初始化失败，后面都不用看了");
        pSystem = IntPtr.Zero;
        return;
      }

      var getDS = Slot<VOutPtr>(pSystem, 3);
      if (getDS == null) { sb.AppendLine("  IADLXSystem 虚表异常（GetDisplaysServices 槽位为空）"); return; }
      IntPtr pDS;
      int r = getDS(pSystem, out pDS);
      sb.AppendLine("  GetDisplaysServices = " + R(r));
      if (r != 0 || pDS == IntPtr.Zero) return;

      try {
        var getNum = Slot<VOutUint>(pDS, 3);
        uint n = 0;
        if (getNum != null && getNum(pDS, out n) == 0) sb.AppendLine("  GetNumberOfDisplays = " + n);

        var getDisplays = Slot<VOutPtr>(pDS, 4);
        var getCC       = Slot<VPtrOutPtr>(pDS, 16);
        if (getDisplays == null || getCC == null) { sb.AppendLine("  DisplayServices 虚表异常"); return; }

        IntPtr pList;
        r = getDisplays(pDS, out pList);
        sb.AppendLine("  GetDisplays = " + R(r));
        if (r != 0 || pList == IntPtr.Zero) return;

        try {
          var sizeFn = Slot<VRetUint>(pList, 3);
          var atFn   = Slot<VUintOutPtr>(pList, 11);
          if (sizeFn == null || atFn == null) { sb.AppendLine("  DisplayList 虚表异常"); return; }
          uint size = sizeFn(pList);
          sb.AppendLine("  ADLX 枚举到的显示器数量 = " + size);

          for (uint i = 0; i < size; i++) {
            IntPtr pDisp;
            int ra = atFn(pList, i, out pDisp);
            if (ra != 0 || pDisp == IntPtr.Zero) { sb.AppendLine("  显示器[" + i + "] At 失败 " + R(ra)); continue; }
            try {
              string name = "?";
              IntPtr pName;
              var nameFn = Slot<VOutPtr>(pDisp, 6);
              if (nameFn != null && nameFn(pDisp, out pName) == 0 && pName != IntPtr.Zero)
                name = Marshal.PtrToStringAnsi(pName);
              ulong uid = 0;
              var uidFn = Slot<VOutUlong>(pDisp, 13);
              bool hasUid = uidFn != null && uidFn(pDisp, out uid) == 0;
              sb.AppendLine("  显示器[" + i + "] 名称 = " + name + (hasUid ? "   UniqueId = " + uid : ""));

              IntPtr pCC;
              int rc = getCC(pDS, pDisp, out pCC);
              sb.AppendLine("      GetCustomColor = " + R(rc)
                + (rc == 12 ? "   ← 驱动明确说这台显示器不支持自定义颜色" : ""));
              if (rc != 0 || pCC == IntPtr.Zero) continue;
              try {
                ProbeColorItem(sb, pCC, "饱和度", 7);
                ProbeColorItem(sb, pCC, "色调",   3);
                ProbeColorItem(sb, pCC, "亮度",   11);
                ProbeColorItem(sb, pCC, "对比度", 15);
                ProbeColorItem(sb, pCC, "色温",   19);
              } finally { AdlxRelease(pCC); }
            } finally { AdlxRelease(pDisp); }
          }
        } finally { AdlxRelease(pList); }
      } finally { AdlxRelease(pDS); }
    } catch (Exception ex) {
      sb.AppendLine("  ADLX 查询异常: " + ex.GetType().Name + ": " + ex.Message);
    } finally {
      if (pSystem != IntPtr.Zero && terminate != null) {
        int tr = terminate();
        sb.AppendLine("  ADLXTerminate = " + R(tr));
      }
      FreeLibrary(lib);
    }
    sb.AppendLine();
  }

  public static string Run() {
    var sb = new StringBuilder();
    sb.AppendLine("================ 显卡颜色控制诊断 ================");
    sb.AppendLine("时间: " + DateTime.Now.ToString("yyyy-MM-dd HH:mm:ss"));
    sb.AppendLine("系统: " + Environment.OSVersion + "   64位进程=" + Environment.Is64BitProcess);
    sb.AppendLine();
    SectionDisplays(sb);
    SectionNvidia(sb);
    SectionAmd(sb);
    SectionAdlx(sb);
    sb.AppendLine("================ 结束 ================");
    return sb.ToString();
  }
}
'@

# ── 主流程 ──
try {
    $result = [Diag]::Run()
    Write-Host $result
    $desktop = [Environment]::GetFolderPath("Desktop")
    $ts = Get-Date -Format "yyyyMMdd-HHmmss"
    $outFile = Join-Path $desktop "gpu-color-diag-$ts.txt"
    [System.IO.File]::WriteAllText($outFile, $result, [System.Text.Encoding]::UTF8)
    Write-Host ""
    Write-Host "诊断报告已保存到桌面: $outFile" -ForegroundColor Green
} catch {
    Write-Host "脚本执行出错: $_" -ForegroundColor Red
    Write-Host $_.Exception.StackTrace
    exit 1
}

