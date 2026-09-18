# R2 发布下载链接 - 服务端交接

## 目标

服务端下发 Windows 安装包时，使用 Cloudflare R2 自定义域，不再使用 GitHub Release 下载地址。

## R2 基础地址

```text
https://filter-manage-download.xyls.us.kg
```

## 安装包 URL 规则

版本号使用纯数字格式，不带 `v` 前缀。

```text
{R2_PUBLIC_URL}/releases/{version}/Filter-Manage_{version}_x64-setup.exe
```

示例：

```text
https://filter-manage-download.xyls.us.kg/releases/0.5.0/Filter-Manage_0.5.0_x64-setup.exe
```

签名文件 URL：

```text
{R2_PUBLIC_URL}/releases/{version}/Filter-Manage_{version}_x64-setup.exe.sig
```

## 服务端返回示例

如果接口只需要返回下载地址：

```json
{
  "version": "0.5.0",
  "downloadUrl": "https://filter-manage-download.xyls.us.kg/releases/0.5.0/Filter-Manage_0.5.0_x64-setup.exe"
}
```

如果接口同时返回签名地址：

```json
{
  "version": "0.5.0",
  "downloadUrl": "https://filter-manage-download.xyls.us.kg/releases/0.5.0/Filter-Manage_0.5.0_x64-setup.exe",
  "signatureUrl": "https://filter-manage-download.xyls.us.kg/releases/0.5.0/Filter-Manage_0.5.0_x64-setup.exe.sig"
}
```

## Tauri 自动更新

Tauri 更新清单固定地址：

```text
https://filter-manage-download.xyls.us.kg/latest.json
```

服务端如果只是提供“检查更新”入口，应返回这个地址，或直接代理该 JSON 内容。不要把 `latest.json` 拼成版本目录路径。

当前 `latest.json` 中的 Windows 平台地址已经指向 R2：

```text
platforms.windows-x86_64.url
platforms.windows-x86_64-nsis.url
```

## 实现要求

1. `version` 统一使用 `0.5.0` 格式，不要生成 `v0.5.0` 路径。
2. 只允许使用服务端保存的版本号拼接 URL，不要直接信任客户端传入的任意路径。
3. 保留旧 GitHub 地址作为失败回退时的备用地址（如果服务端已有回退逻辑）。
4. 不要在服务端保存或读取 R2 Access Key、Secret Access Key；下载是公开自定义域访问。
5. 生产环境使用 HTTPS，并保持完整文件名中的大小写和下划线。

## 验收

```bash
curl -I https://filter-manage-download.xyls.us.kg/latest.json
curl -I https://filter-manage-download.xyls.us.kg/releases/0.5.0/Filter-Manage_0.5.0_x64-setup.exe
```

两个请求都应返回 `HTTP 200`。安装包响应应包含：

```text
Content-Disposition: attachment
```
