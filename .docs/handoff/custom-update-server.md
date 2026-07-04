# 自定义更新流程 - 服务端交接（Spring Boot）

> **状态：📋 待实现** — 配套 [plan](../plans/custom-update-flow.md)。本文档面向**服务端实现者**（部署到 `.env.local` 中配置的服务器，用 Java Spring Boot 实现）。

## 背景

桌面客户端要做自定义更新（多镜像 + 可取消 + 测速换源），不再用 Tauri 内置 updater 直连 GitHub。需要一个服务端提供「检查更新」接口，**集中下发镜像列表**——这样增删镜像只改服务端配置，无需重新发布客户端。

**关键约束**：服务器在国内，**不要在请求时回源 GitHub**（不稳定）。版本清单由 CI 在发版时主动推送给服务端存下来。

## 服务器信息（沿用现有）

- **服务器**：腾讯云 Ubuntu 22.04，具体地址见 `.env.local` 的 `FILTER_MANAGE_SSH_TARGET`
- **SSH**：`ssh -i "${FILTER_MANAGE_SSH_KEY}" "${FILTER_MANAGE_SSH_TARGET}"`
- **Cloudflare Tunnel**：公网域名见 `.env.local` 或 GitHub Secrets
- 现有服务：AList（`127.0.0.1:5244`）+ Python 上传 webhook（`127.0.0.1:9876`）。本服务与它们并列，**另起端口**（如 `127.0.0.1:9877`）。

---

## 两个接口

### 1. `GET /api/check-update`（公开只读，客户端调用）

**Query**：`current` — 客户端当前版本，如 `0.3.2`

**响应**（`200 application/json`），**字段名固定**（客户端按此解析）：

```json
{
  "hasUpdate": true,
  "version": "0.3.4",
  "notes": "### 修复\n- 修复了 xxx",
  "signature": "dW50cnVzdGVkIGNvbW1lbnQ6...",
  "mirrors": [
    { "name": "GitHub 镜像 1", "url": "https://gh-proxy.org/https://github.com/q974491089/filter-manage/releases/download/v0.3.4/Filter-Manage_0.3.4_x64-setup.exe" },
    { "name": "GitHub 镜像 2", "url": "https://v4.gh-proxy.org/https://github.com/q974491089/filter-manage/releases/download/v0.3.4/Filter-Manage_0.3.4_x64-setup.exe" },
    { "name": "GitHub 镜像 3", "url": "https://cdn.gh-proxy.org/https://github.com/q974491089/filter-manage/releases/download/v0.3.4/Filter-Manage_0.3.4_x64-setup.exe" },
    { "name": "GitHub 原始", "url": "https://github.com/q974491089/filter-manage/releases/download/v0.3.4/Filter-Manage_0.3.4_x64-setup.exe" }
  ]
}
```

逻辑：
- 从存储读最新清单（`version` / `notes` / `signature` / `assetName`）
- `hasUpdate` = semver 比较 `version > current`（无 `current` 时返回最新但 `hasUpdate=false`）
- `mirrors[]` 请求时拼：对配置里每个镜像 `prefix`，`url = prefix + githubBase + "/v" + version + "/" + assetName`（`prefix=""` 即 GitHub 原始）
- `signature` 原样透传（base64 minisign 签名，**服务端不接触私钥**）

> **无需 CORS**：客户端是 Rust 原生请求，不走浏览器。

### 2. `POST /api/internal/release`（CI 推送，需鉴权）

发版时由 GitHub Actions 调用，存下最新版本清单。

**Headers**：`X-Release-Secret: <密钥>`（与 systemd 环境变量一致；非法 → `401`）

**Body**：
```json
{
  "version": "0.3.4",
  "notes": "### 修复\n- ...",
  "signature": "dW50cnVzdGVkIGNvbW1lbnQ6...",
  "assetName": "Filter-Manage_0.3.4_x64-setup.exe"
}
```

**响应**：`200 {"status":"ok","version":"0.3.4"}`

逻辑：校验密钥 → 持久化为「当前最新清单」（覆盖）→ MVP 用单个 JSON 文件即可。

---

## Spring Boot 实现骨架

> Spring Boot 3 / Java 17。仅供参考，可按习惯调整包结构。

### 配置属性

```java
@ConfigurationProperties(prefix = "update")
public record UpdateProperties(
    String releaseSecret,
    String githubBase,       // https://github.com/q974491089/filter-manage/releases/download
    String manifestFile,     // /opt/filter-manage-update/latest.json
    List<MirrorCfg> mirrors
) {
    public record MirrorCfg(String name, String prefix) {}
}
```

### DTO

```java
public record MirrorDto(String name, String url) {}

public record CheckUpdateResponse(
    boolean hasUpdate, String version, String notes,
    String signature, List<MirrorDto> mirrors
) {}

// 存储 & CI 推送共用
public record ReleaseManifest(
    String version, String notes, String signature, String assetName
) {}
```

### 存储服务（JSON 文件，线程安全）

```java
@Service
public class ReleaseStore {
    private final ObjectMapper mapper = new ObjectMapper();
    private final Path file;
    private volatile ReleaseManifest latest;

    public ReleaseStore(UpdateProperties props) throws IOException {
        this.file = Path.of(props.manifestFile());
        if (Files.exists(file)) {
            this.latest = mapper.readValue(file.toFile(), ReleaseManifest.class);
        }
    }

    public synchronized void save(ReleaseManifest m) throws IOException {
        Files.createDirectories(file.getParent());
        mapper.writeValue(file.toFile(), m);
        this.latest = m;
    }

    public ReleaseManifest latest() { return latest; }
}
```

### 控制器

```java
@RestController
@RequestMapping("/api")
public class UpdateController {
    private final ReleaseStore store;
    private final UpdateProperties props;

    public UpdateController(ReleaseStore store, UpdateProperties props) {
        this.store = store; this.props = props;
    }

    @GetMapping("/check-update")
    public CheckUpdateResponse check(@RequestParam(required = false) String current) {
        ReleaseManifest m = store.latest();
        if (m == null) return new CheckUpdateResponse(false, "", "", "", List.of());

        boolean hasUpdate = current != null && SemVer.gt(m.version(), current);
        String githubUrl = props.githubBase() + "/v" + m.version() + "/" + m.assetName();
        List<MirrorDto> mirrors = props.mirrors().stream()
            .map(c -> new MirrorDto(c.name(), c.prefix() + githubUrl))
            .toList();
        return new CheckUpdateResponse(hasUpdate, m.version(), m.notes(), m.signature(), mirrors);
    }

    @PostMapping("/internal/release")
    public ResponseEntity<?> push(
        @RequestHeader(value = "X-Release-Secret", required = false) String secret,
        @RequestBody ReleaseManifest body) throws IOException {
        if (!props.releaseSecret().equals(secret)) {
            return ResponseEntity.status(401).body(Map.of("error", "unauthorized"));
        }
        store.save(body);
        return ResponseEntity.ok(Map.of("status", "ok", "version", body.version()));
    }
}
```

### semver 比较

```java
public final class SemVer {
    /** a > b ? 仅比较 major.minor.patch 数字段 */
    public static boolean gt(String a, String b) {
        int[] x = parse(a), y = parse(b);
        for (int i = 0; i < 3; i++) {
            if (x[i] != y[i]) return x[i] > y[i];
        }
        return false;
    }
    private static int[] parse(String v) {
        String[] p = v.replaceAll("[^0-9.]", "").split("\\.");
        int[] r = new int[3];
        for (int i = 0; i < 3 && i < p.length; i++)
            r[i] = p[i].isEmpty() ? 0 : Integer.parseInt(p[i]);
        return r;
    }
}
```

### `application.yml`

```yaml
server:
  port: 9877
  address: 127.0.0.1   # 仅本地，由 Cloudflare Tunnel 暴露

update:
  release-secret: ${UPDATE_RELEASE_SECRET}
  github-base: https://github.com/q974491089/filter-manage/releases/download
  manifest-file: /opt/filter-manage-update/latest.json
  mirrors:
    - { name: "GitHub 镜像 1", prefix: "https://gh-proxy.org/" }
    - { name: "GitHub 镜像 2", prefix: "https://v4.gh-proxy.org/" }
    - { name: "GitHub 镜像 3", prefix: "https://cdn.gh-proxy.org/" }
    - { name: "GitHub 原始",   prefix: "" }
```

> 增删镜像 = 改这里的 `mirrors` 列表 + 重启服务，**客户端无需变更**。

---

## 部署（fat jar + systemd）

```bash
# 打包（本地或 CI）
./mvnw clean package    # 产出 target/filter-manage-update-*.jar

# 上传到服务器
scp -i "${FILTER_MANAGE_SSH_KEY}" target/filter-manage-update-*.jar \
    "${FILTER_MANAGE_SSH_TARGET}":/opt/filter-manage-update/app.jar
```

`/etc/systemd/system/filter-manage-update.service`：
```ini
[Unit]
Description=Filter Manage Update API
After=network.target

[Service]
Type=simple
User=ubuntu
WorkingDirectory=/opt/filter-manage-update
Environment=UPDATE_RELEASE_SECRET=<随机密钥>
ExecStart=/usr/bin/java -jar /opt/filter-manage-update/app.jar
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

```bash
sudo systemctl daemon-reload
sudo systemctl enable filter-manage-update
sudo systemctl start filter-manage-update
```

需要 JDK 17+：`sudo apt install openjdk-17-jre-headless`

### Cloudflare Tunnel 路由

把 `127.0.0.1:9877` 暴露到公网（让客户端和 GitHub Actions 能访问）。在 Tunnel 配置加一条：
```
<UPDATE_PUBLIC_HOST> → http://127.0.0.1:9877
```
或在现有公网域名上加路径转发。**最终公网 URL 要回填给客户端**（Rust 的 `UPDATE_API_HOSTS` 常量数组，目前配两个：主域名 `filter-manage-api.xyls.us.kg` + 备用 `filter-manage-api.6ya.kdns.fr`，`check_update` 双域名竞速）和 GitHub Secret（`UPDATE_API_URL` 用主域名即可）。

---

## CI 端（由本地 Agent 完成，供你了解会收到什么）

`release.yml` 在 tauri 构建后新增一步：从构建产物取 version/notes/signature/assetName，POST 到 `/api/internal/release`。具体 step 见 `.skills/shared/release-workflow.md` 第 9 步（权威 spec）。

```bash
# version: tag 去掉 v 前缀
VERSION="${GITHUB_REF_NAME#v}"
# notes: 复用 release.yml 里 Extract changelog step 从 CHANGELOG.md 提取的内容
#   （通过 env: NOTES: ${{ steps.changelog.outputs.body }} 传入）
# signature: 从构建产物 .sig 文件读（base64 文本）—— tauri-action 用 TAURI_SIGNING_PRIVATE_KEY 生成
SIG_FILE=$(find src-tauri/target -name "*.exe.sig" | head -1)
SIG=$(jq -Rs @base64 < "$SIG_FILE" | tr -d '\n')
# assetName: setup.exe 文件名
ASSET=$(basename "$(find src-tauri/target -name "*-setup.exe" | head -1)")

jq -n --arg v "$VERSION" --arg n "$NOTES" --arg s "$SIG" --arg a "$ASSET" \
  '{version:$v,notes:$n,signature:$s,assetName:$a}' \
| curl -sf -X POST "$UPDATE_API_URL" \
    -H "Content-Type: application/json" \
    -H "X-Release-Secret: $UPDATE_RELEASE_SECRET" \
    --data @-
```

> ⚠️ signature 必须带上（base64 编码的 .sig 文件内容），客户端 `verify_minisign` 用它校验安装包完整性。v0.3.3 推送时漏带，导致客户端无法校验。

GitHub Secrets：`UPDATE_API_URL`（`https://filter-manage-api.xyls.us.kg/api/internal/release`）、`UPDATE_RELEASE_SECRET`（与 systemd 一致）。

---

## 验证

```bash
# 推送一条假清单
curl -X POST http://127.0.0.1:9877/api/internal/release \
  -H "Content-Type: application/json" \
  -H "X-Release-Secret: <密钥>" \
  -d '{"version":"0.3.4","notes":"test","signature":"abc","assetName":"Filter-Manage_0.3.4_x64-setup.exe"}'

# 检查更新（旧版本 → 应 hasUpdate=true）
curl "http://127.0.0.1:9877/api/check-update?current=0.3.2"
# 检查更新（同版本 → 应 hasUpdate=false）
curl "http://127.0.0.1:9877/api/check-update?current=0.3.4"

# 鉴权失败
curl -X POST http://127.0.0.1:9877/api/internal/release -H "X-Release-Secret: wrong" -d '{}'
# 预期 401
```

---

## 安全 / 注意

1. **私钥不上服务器**：服务端只透传 base64 签名，minisign 私钥仍只在 GitHub Secret。
2. **`/api/internal/release` 必须鉴权**：`X-Release-Secret` 用随机长字符串。
3. **`/api/check-update` 公开只读**：可加简单限流（防滥用）。
4. **镜像可用性**：镜像列表顺序即客户端默认/换源顺序；把最稳的放前面。
5. **降级（可选）**：服务端不可达时，客户端是否回退直连 GitHub `latest.json`？见 plan 待确认项，需要的话本服务也可提供一个无 mirror 的兜底响应。

---

**关联**：[plan](../plans/custom-update-flow.md) · [Rust API](../api/updater.md) · [前端交接](./custom-updater-frontend.md)
