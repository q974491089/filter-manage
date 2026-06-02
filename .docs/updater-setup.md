# Tauri Updater 签名密钥设置

## 生成密钥

在项目根目录运行：

```bash
npx tauri signer generate -w ~/.tauri/filter-manage.key
```

会提示输入密码（可留空），生成两个文件：
- `~/.tauri/filter-manage.key` — 私钥（**不要提交到 Git**）
- `~/.tauri/filter-manage.key.pub` — 公钥

## 配置公钥

将 `.key.pub` 文件内容复制到 `src-tauri/tauri.conf.json` 中：

```json
"plugins": {
  "updater": {
    "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIH..."
  }
}
```

## 配置 GitHub Secrets

在仓库 Settings → Secrets and variables → Actions 中添加：

| Secret 名称 | 值 |
|---|---|
| `TAURI_SIGNING_PRIVATE_KEY` | `.key` 文件的完整内容 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 生成时设置的密码（如果留空则设为空字符串） |

## CI 构建

GitHub Actions 构建时需要设置环境变量：

```yaml
env:
  TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
```

使用 `tauri-apps/tauri-action` 并设置 `includeUpdaterJson: true`，会自动生成 `latest.json` 并上传到 Release。
