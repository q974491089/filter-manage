# 全屏 Gamma 调试器

`fullscreen_gamma_debug.py` 是独立的 Windows 工具，不读取或修改 Filter-Manage 配置，不注入 VALORANT。

## 用户操作

1. 先让 Filter-Manage 应用平时使用的方案。
2. 启动压缩包内的 `Filter-Manage-Fullscreen-Debug*.exe`，选择出问题的显示器，点击“开始监视”。
3. 启动 VALORANT，进入会黑屏的全屏模式；黑屏结束后点击“标记当前状态”。
4. Alt+Tab 回桌面，再点击一次“标记当前状态”。
5. 点击“停止并保存”，把桌面上的 `diagnostic-report-v1.json` 发回；原有 `fullscreen-gamma-debug-*.txt` 文本日志也会保留。

## 强验证模式

点击“写入临时 Gamma 2.4 并监视”会保存当前 ramp，然后写入明显变暗的测试 ramp。进入游戏并完成一次全屏切换后：

- `GAMMA_RAMP_CHANGED` 变成 `shape=identity`：切换过程重置了 LUT。
- ramp 仍是 `shape=non-identity`，但游戏画面没变化：更像游戏绕过桌面 LUT。
- `DISPLAY_MODE_CHANGED`：全屏改变了分辨率、刷新率或位深。

测试结束必须点“恢复原 ramp”。关闭窗口也会尝试恢复。

## 打包

在 Windows 上双击 `build-fullscreen-gamma-debug.bat`，生成 `dist\Filter-Manage-Fullscreen-Debug.exe`。脚本只依赖 Python 3 和 PyInstaller。
