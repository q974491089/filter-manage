> 📦 已归档 2026-08-02 · 仅供历史追溯，非当前开发依据

# 公告功能 - 服务端交接（Spring Boot + Docker）

> **状态：✅ 已完成（已上线）** — 2026-08-02 验证：双域名 `GET /api/announcements?type=client` 均返回 200 且数据结构符合契约（含 `category` 字段），已有线上公告数据。桌面客户端契约见 `.docs/api/announcements.md`。以下为原始实现需求，留档备查。

## 背景

桌面客户端（Filter Manage）要加「公告」：不发版即可向用户推送通知（维护预告、活动、重要提醒）。客户端启动时拉取公告列表，顶栏铃铛展示，重要公告弹窗。**公告内容由维护者手动维护**（编辑一个 JSON 文件后 scp 到服务器 `data/` 目录）。

**关键**：过滤（有效期）、排序、已读全部在客户端做，**服务端只做只读透传**，原样返回文件内容即可。

## 本项目现状（已确认）

- 包：`com.filtermanage.update`，控制器 `src/main/java/com/filtermanage/update/controller/UpdateController.java`（`@RequestMapping("/api")`，现有 `/check-update`、`/internal/release`）。
- 数据目录：`docker-compose.yml` 把宿主机 `./data` 挂到容器 `/app/data`。更新清单在 `/app/data/latest.json`（宿主机 `~/filter-manage-api/data/latest.json`）。
- 部署：**Docker**。`Dockerfile` 用 maven 构建 jar 再跑；`docker compose up -d --build` 即可。端口 9877，接入 `1panel-network`。
- 已有全局 `CorsConfig`，新接口自动继承，无需额外处理。

## 你要做的（就一件事：加一个只读接口）

在 `UpdateController` 里加 `GET /api/announcements`，读 `/app/data/announcements.json` 原样返回。**无鉴权**（公告是公开信息，与 `/check-update` 一致）。文件不存在 → 返回**空数组** `[]`（客户端视为「暂无公告」，不是错误）。

---

## 数据契约（`announcements.json` 的结构）

服务端只透传，无需理解字段，但按此结构存储：

```jsonc
// data/announcements.json  →  Announcement[]
[
  {
    "id": "2026-07-10-maintenance",        // 稳定唯一 id（客户端据此记已读，勿复用旧 id）
    "title": "服务器维护通知",
    "body": "## 维护时间\n本周六 02:00–04:00 服务短暂不可用。",  // Markdown 文本
    "level": "normal",                      // "normal"（铃铛）| "important"（启动弹窗）
    "publishedAt": "2026-07-10T08:00:00Z",  // ISO8601 UTC，客户端排序用
    "startAt": "2026-07-10T00:00:00Z",      // 可选，缺省=立即生效
    "endAt": "2026-07-13T00:00:00Z"         // 可选，缺省=永不过期
  }
]
```

字段说明：
- `level`：`"important"` 会在客户端**启动时弹窗**强触达，请克制使用（否则每次启动都弹，扰民）；日常用 `"normal"`。
- `startAt`/`endAt`：客户端按当前时间过滤窗口外的公告（`now < startAt` 或 `now >= endAt` 即隐藏）。适合限时活动公告，到期自动消失。
- 时间统一 **UTC ISO8601**（带 `Z`）。

---

## 实现（在 UpdateController 里加一个方法）

`src/main/java/com/filtermanage/update/controller/UpdateController.java`，新增 import 与方法：

```java
import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import java.nio.file.Files;
import java.nio.file.Path;
// （类内已有 import java.io.IOException; java.util.List; java.util.Map;）

// ... UpdateController 类内新增字段 + 方法：

    private static final Path ANNOUNCEMENTS_FILE = Path.of("/app/data/announcements.json");
    private final ObjectMapper announcementsMapper = new ObjectMapper();

    /** 公开只读：原样返回 announcements.json（不存在则空数组）。过滤/排序全在客户端。 */
    @GetMapping("/announcements")
    public List<Map<String, Object>> announcements() throws IOException {
        if (!Files.exists(ANNOUNCEMENTS_FILE)) {
            return List.of();
        }
        return announcementsMapper.readValue(
            ANNOUNCEMENTS_FILE.toFile(),
            new TypeReference<List<Map<String, Object>>>() {}
        );
    }
```

> 用 `List<Map<String,Object>>` 透传即可，不必为公告建强类型 DTO——服务端不消费字段。
> 路径 `/app/data/announcements.json` 是容器内路径（宿主机 `./data` 已挂载到 `/app/data`），与 `latest.json` 同目录。若想可配置，可仿 `UpdateProperties` 加属性，非必须。

---

## 部署（Docker，在服务器 `~/filter-manage-api` 目录）

改完 `UpdateController.java` 后重建镜像并重启：

```bash
cd ~/filter-manage-api
docker compose up -d --build
```

（`Dockerfile` 会在镜像内用 maven 重新打包 jar，无需本地装 JDK/Maven。）

验证容器起来了：
```bash
docker compose ps
docker compose logs --tail=50 filter-manage-api
```

---

## 公告内容维护（日常发公告，维护者操作）

接口上线后，发一条公告 = 编辑 `announcements.json` → scp 到服务器 `data/` 目录：

```bash
# 客户端仓库根目录，用 SSH 别名 tencent（HostName/User/Key 已在 ~/.ssh/config）
scp announcements.json tencent:~/filter-manage-api/data/announcements.json
```

**无需重建/重启容器**——`data/` 是 volume 挂载，接口每次请求都重新读文件，改完文件即时生效。

### 起步样例 `announcements.json`

```json
[
  {
    "id": "2026-07-10-welcome",
    "title": "公告功能上线",
    "body": "## 👋 欢迎\n现在可以在这里看到最新公告了。",
    "level": "normal",
    "publishedAt": "2026-07-10T08:00:00Z"
  },
  {
    "id": "2026-07-12-maintenance",
    "title": "服务器维护",
    "body": "本周六 **02:00–04:00** 更新服务短暂不可用，不影响本地使用。",
    "level": "important",
    "publishedAt": "2026-07-12T00:00:00Z",
    "startAt": "2026-07-11T00:00:00Z",
    "endAt": "2026-07-13T00:00:00Z"
  }
]
```

---

## 验证

```bash
# 服务器本地直连（文件还没建时应返回 []）
curl http://127.0.0.1:9877/api/announcements

# 公网（客户端实际访问地址，双域名任一）
curl https://filter-manage-api.xyls.us.kg/api/announcements
```

客户端侧：启动应用 → 顶栏铃铛出现未读红点 → 重要公告弹窗；断网启动应静默无公告、不报错。

---

## 安全 / 注意

1. **只读、公开**：`/api/announcements` 无鉴权，只读文件，不接受任何写入。
2. **唯一写路径是 scp**：已由 SSH 密钥保护；**不要**为公告开放任何 HTTP 写接口（本次不做发布后台）。
3. **id 稳定唯一**：客户端用 `id` 记已读，改内容别改 id（否则用户重新看到"未读"）；发新公告用新 id。
4. **谨慎 `important`**：只有真正需要打断用户的才用，否则每次启动弹窗扰民。

---

**关联**：桌面客户端契约 `.docs/api/announcements.md`（客户端仓库）· 同应用的更新 API（现有 `/check-update`、`/internal/release`）。
