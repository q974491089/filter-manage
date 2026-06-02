# 截图 + 色彩注入功能实现计划

**状态**: TODO  
**优先级**: 中  
**创建于**: 2026-06-02

## 背景

市面上所有截图工具均无法捕获 ICC 色彩配置文件的效果，原因是：

- **ICC 变换**发生在显示管线的下游（ICC profile → 显示器），操作系统截图 API（BitBlt 等）捕获的是渲染输出，还没有经过 ICC 变换。
- **NVIDIA DVC（数字振动）**同理，是驱动层的信号处理，截图 API 看不到。

解决方案：截图后在软件侧将 ICC 变换和 NVIDIA 色彩参数**重新应用**到像素上，模拟用户眼睛看到的效果。

## 功能范围

| 功能 | 状态 | 说明 |
|------|------|------|
| 全屏截图 | TODO | 捕获主显示器原始帧 |
| 解析当前 ICC 并应用 | TODO | 读取激活的 ICC 文件，通过 LUT 变换像素 |
| 应用 NVIDIA 亮度/对比度/伽马 | TODO | 逐像素数学变换 |
| 模拟 DVC（数字振动）| TODO | v3.1 加入，用饱和度增强近似 |
| 保存为 PNG | TODO | 无损输出 |

> DVC 精确算法未公开，采用 HSL 饱和度增强近似，效果接近但非完全一致。

## 技术方案

### 依赖

```toml
# src-tauri/Cargo.toml
screenshots = "0.8"       # Windows 截图，基于 GDI
image = "0.25"            # 像素处理、PNG 编码
lcms2 = "6"               # ICC LUT 解析与应用（C 绑定）
```

> `lcms2` 需要系统安装 liblcms2-dev（CI 需要在 workflow 中安装）。  
> 备选纯 Rust 方案：`qcms`（Firefox 同款），无 C 依赖，但功能较简单。

### 处理流程

```
1. 截图
   screenshots::Screen::all() → 取主显示器 → capture() → RgbaImage

2. 读取当前 ICC 路径
   调用已有的 get_active_icc_profile() 获取当前激活的 ICC 文件路径

3. 应用 ICC 变换
   lcms2::Profile::new_file(icc_path) → 建立 sRGB→ICC 的变换
   Transform::new(srgb, TYPE_RGBA_8, icc_profile, TYPE_RGBA_8, Intent::Perceptual)
   逐行应用变换

4. 应用 NVIDIA 色彩参数
   读取当前配置（亮度 b、对比度 c、伽马 g）
   每个像素：
     pixel = clamp((pixel / 255.0 ^ (1/g) * (1 + c/82.0) + b/125.0) * 255.0, 0, 255)

5. 模拟 DVC（v3.1）
   将 RGB 转换为 HSL
   saturation *= (1 + dvc / 100.0)  // dvc=0 无变化，dvc=100 饱和度翻倍
   转回 RGB

6. 编码为 PNG
   image::DynamicImage::save(output_path)

7. 返回保存路径给前端
```

### Tauri 命令签名（草稿）

```rust
#[tauri::command]
pub async fn capture_screen_with_color(
    output_path: Option<String>,  // None 则保存到用户桌面
) -> Result<String, String>       // 返回保存路径
```

前端调用时可传可不传路径，方便直接截图保存到桌面。

## 实现步骤

- [ ] **Step 1**: 添加 `screenshots`、`image` crate，验证基础截图可用
- [ ] **Step 2**: 实现截图 → 保存 PNG（不含色彩变换），验证文件正常
- [ ] **Step 3**: 接入 `lcms2`，实现 ICC 变换逻辑，单元测试验证像素变化
- [ ] **Step 4**: 实现 NVIDIA 亮度/对比度/伽马逐像素变换
- [ ] **Step 5**: 实现 DVC 饱和度近似（v3.1）
- [ ] **Step 6**: 注册 Tauri 命令，更新 `.docs/api/screenshot.md`
- [ ] **Step 7**: 写前端交接文档 `.docs/handoff/screenshot-frontend.md`

## 待确认问题

- [ ] `lcms2` C 绑定在 CI（GitHub Actions Windows）能否正常编译？若不行切换 `qcms`
- [ ] 多显示器场景：截哪个屏？当前方案取主显示器，后续可扩展为选择显示器
- [ ] 输出路径：默认桌面 `filter-manage-screenshot-{timestamp}.png`，是否合适？

## 相关文件

- 后端实现位置：`src-tauri/src/screenshot.rs`（待创建）
- 接口文档：`.docs/api/screenshot.md`（待创建）
- 前端交接：`.docs/handoff/screenshot-frontend.md`（待创建）
