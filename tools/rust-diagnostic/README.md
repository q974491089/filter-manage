# Filter-Manage Diagnostic

独立 Windows 诊断工具，用于定位“桌面 Gamma 有效，但点击进入游戏/全屏后失效”的问题。

## 用户流程

1. 选择目标显示器和目标进程关键词。
2. 点击「开始监视」。
3. 进入游戏并复现问题。
4. 在进入游戏、切换全屏、问题出现和回到桌面四个时刻分别点击对应的用户标记按钮。
5. 回到桌面后点击「停止」。
6. 点击「导出诊断报告」，把桌面上的 `diagnostic-report-v1.json` 发回。

动态监视每 500ms 采样一次，只在显示模式、Gamma ramp 或目标进程发生变化时写入事件。

## Gamma 探针

「写入临时 Gamma」会保存当前 ramp，再写入 Gamma 2.4。它只在用户明确确认后执行；停止监视、点击「恢复原始 ramp」、窗口关闭和应用退出时都会尝试恢复。恢复失败会保留本地恢复文件，并在下次启动继续尝试。

探针可能改变屏幕亮度，不能在未保存原始状态时强行终止程序。

## Windows 构建

在仓库根目录 Windows PowerShell 中执行：

```powershell
npm run build:diagnostic
Set-Location tools/rust-diagnostic/src-tauri
& "../../../../node_modules/.bin/tauri.cmd" build
```

安装包位于 `tools/rust-diagnostic/src-tauri/target/release/bundle/nsis/`。

## 报告校验

在导出报告的 Windows 机器上运行：

```powershell
.\tools\validate-rust-diagnostic-report.ps1
```

也可以传入报告路径。脚本只读取 JSON，不会修改显示器、Gamma 或进程状态；它会检查 schema、必需字段以及 `MONITOR_STARTED`/`SNAPSHOT_INITIAL` 基础事件。

## Rust/Python 对照

在同一台机器、同一复现步骤分别导出 Rust 和 Python 报告后，可运行：

```powershell
.\tools\compare-diagnostic-reports.ps1 `
  -RustPath "$env:USERPROFILE\Desktop\diagnostic-report-v1-rust.json" `
  -PythonPath "$env:USERPROFILE\Desktop\diagnostic-report-v1-python.json"
```

脚本会严格比较 schema、目标显示器和显示器模式；事件数量、最新快照模式和 Gamma 形状的时间差异以 warning 输出，不会把正常采样时序差异误判为契约失败。
