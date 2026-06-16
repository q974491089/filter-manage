# 服务器端云盘上传 Webhook 服务 — 交接文档

## 背景

GitHub Actions 构建完 Release 后，需要把 `.exe` 安装包上传到 9 个云盘。之前通过 GitHub Actions 直接上传，因为网络问题（海外 → 国内服务器 → Cloudflare Tunnel）经常超时失败。

**新方案**：GitHub Actions 构建完后发 POST 请求触发本服务器上的 webhook 服务，服务器本地下载 exe 并通过 AList localhost 分发到各云盘。

## 服务器信息

- **服务器**：腾讯云 Ubuntu 22.04，IP `118.25.20.249`
- **AList**：Docker 容器 `1Panel-alist-0q8u`，监听 `127.0.0.1:5244`
- **AList 账号**：`admin` / `alist123456`
- **SSH**：`ssh -i ~/.ssh/Qq2282782.pem ubuntu@118.25.20.249`
- **Cloudflare Tunnel**：`https://filter-manage-api.xyls.us.kg` 已指向 AList

## 需要完成的任务

### 1. 创建 Webhook 上传服务

在 `/opt/scripts/upload-server.py` 创建 Python HTTP 服务：

**功能要求**：
- 监听 `127.0.0.1:9876`
- 接收 `POST /upload` 请求
- 请求体：`{"version": "0.3.2"}`
- 请求头：`X-Upload-Secret: <密钥>` 用于鉴权
- 收到请求后异步执行上传（返回 202 Accepted）
- `GET /health` 返回 `{"status":"ok"}`

**上传流程**：
1. 通过国内 GitHub 镜像下载 exe 到 `/tmp/`
2. 登录 AList（`http://127.0.0.1:5244/api/auth/login`）
3. 上传 exe 到 8 个云盘（`quark`、`aliyundrive`、`115`、`baidu`、`lanzou`、`uc`、`yandex`、`doubao`）
4. 打包 zip 上传到 `wukong`（悟空禁止 exe）
5. 清理临时文件

**GitHub 镜像地址**（服务器在国内，无法直连 GitHub）：
```
# 下载 URL 格式：
https://gh-proxy.org/https://github.com/q974491089/filter-manage/releases/download/v{version}/Filter-Manage_{version}_x64-setup.exe

# 备用镜像（如果一个不行换另一个）：
https://v4.gh-proxy.org/https://github.com/...
https://v6.gh-proxy.org/https://github.com/...
https://cdn.gh-proxy.org/https://github.com/...
```

**AList 上传 API**：
```
PUT http://127.0.0.1:5244/api/fs/put
Headers:
  Authorization: <token>
  File-Path: <url-encoded path, e.g. /quark/filter-manage/Filter-Manage_0.3.2_x64-setup.exe>
  Content-Type: application/octet-stream
Body: <文件二进制内容>
```

**云盘存储路径**：
| 云盘 | 路径 |
|------|------|
| quark | `/quark/filter-manage/` |
| aliyundrive | `/aliyundrive/filter-manage/` |
| 115 | `/115/filter-manage/` |
| baidu | `/baidu/filter-manage/` |
| lanzou | `/lanzou/filter-manage/` |
| uc | `/uc/filter-manage/` |
| yandex | `/yandex/filter-manage/` |
| doubao | `/doubao/filter-manage/` |
| wukong | `/wukong/filter-manage/`（需要 zip 格式） |

### 2. 创建 systemd 服务

创建 `/etc/systemd/system/alist-upload.service`：

```ini
[Unit]
Description=AList Cloud Upload Webhook
After=network.target

[Service]
Type=simple
User=ubuntu
WorkingDirectory=/opt/scripts
Environment=UPLOAD_SECRET=<自定义密钥>
ExecStart=/usr/bin/python3 /opt/scripts/upload-server.py
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

启动：
```bash
sudo systemctl daemon-reload
sudo systemctl enable alist-upload
sudo systemctl start alist-upload
```

### 3. 配置 Cloudflare Tunnel

在 Cloudflare Tunnel 配置中添加一条路由，将 webhook 暴露到公网（让 GitHub Actions 能触发）：

```
upload.filter-manage-api.xyls.us.kg → http://127.0.0.1:9876
```

或者在已有的 `filter-manage-api.xyls.us.kg` 域名上添加路径转发（如果 AList 和 webhook 可以共享域名）。

### 4. 验证

```bash
# 本地测试
curl http://127.0.0.1:9876/health
# 预期: {"status":"ok"}

# 触发测试（替换密钥和版本号）
curl -X POST http://127.0.0.1:9876/upload \
  -H "Content-Type: application/json" \
  -H "X-Upload-Secret: <密钥>" \
  -d '{"version":"0.3.2"}'
# 预期: {"status":"uploading","version":"0.3.2"}

# 查看日志
journalctl -u alist-upload -f
```

## GitHub 端配置（由本地 Agent 完成）

GitHub Actions 的 `release.yml` 会修改为：构建完成后 POST 请求触发此 webhook。需要设置 GitHub Secret：
- `UPLOAD_WEBHOOK_URL`：webhook 公网地址
- `UPLOAD_SECRET`：鉴权密钥（与 systemd 中一致）

## 注意事项

1. **密钥安全**：`UPLOAD_SECRET` 不要用简单密码，建议用随机字符串
2. **日志**：通过 `journalctl -u alist-upload` 查看上传日志
3. **镜像失败**：如果一个镜像站不行，自动尝试备用镜像
4. **超时**：每个云盘上传设置 300 秒超时
5. **zip 打包**：悟空网盘禁止 exe，需要 `zip -j` 打包后上传
6. **临时文件**：上传完成后清理 `/tmp/` 下的 exe 和 zip
