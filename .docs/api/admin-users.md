# 管理端用户列表 — 跨仓库接口契约

**日期**：2026-11-10。**范围**：客户端注册账号的只读管理列表；不是后台管理员账号列表，也不包含仅有匿名 contributor UUID 的投稿/分享身份。

## 接口

`GET /api/internal/icc/users`

使用现有后台 session cookie，服务端逐请求重新校验后台身份。仅 `ADMIN` 可读；匿名/无效会话或只有客户端 Bearer 的请求返回 401，有效 `EDITOR` 返回 403。前端依据真实 session `role` 控制 `/users` 菜单及路由，不沿用旧的“已登录即 admin”假设。

| 参数 | 规则 |
| --- | --- |
| `q` | 昵称、邮箱、ID 的全库不区分大小写字面子串；去首尾空格，最多 100 个 Unicode 字符；`%`、`_`、反斜杠不是通配符 |
| `status` | 可选 `ACTIVE` / `BANNED`，不传表示全部 |
| `provider` | 可选 `email` / `github`，不传表示全部 |
| `limit` | 默认 20，1–100 |
| `offset` | 默认 0，非负整数 |

返回 `{items,total,limit,offset}`。`items` 中每项包含 `id`、`displayName`、`email`、`status`、`createdAt`、`updatedAt`、`providers: string[]`、`lastActiveAt: string | null`。空列表与空登录方式返回 `[]`，无会话活跃记录返回 `null`。按照 `createdAt DESC, id DESC` 稳定排序；多个登录身份不重复计数，`total` 为全部筛选后记录总数。最近活跃为会话 `last_used_at` 最大值，不代表当前在线，也不是单独审计的最近登录时间。

不查询或返回密码哈希、会话/历史令牌及哈希、验证码、OAuth state/ticket/secret。不新增修改、封禁、删除或导出操作。

## 前端行为

- `/users` →「用户列表」，复用现有 admin 设计 token、Ant Design Table 与面板样式。
- 查询、状态/登录方式筛选及每页数量变化时回到第一页。查询通过服务端作用于全库，不只过滤当前页。
- 每页 20 / 50 / 100 条，显示筛选后总数，支持刷新。
- 区分加载、无用户、无匹配、失败/重试；旧请求不得覆盖新查询结果，卸载后不更新状态。
- 展示昵称、邮箱、可复制 ID、登录方式、账号状态、注册时间与最近活跃时间；缺失邮箱/昵称/活跃时间使用真实缺省文案。

## 环境检查

- 数据在 market 服务既有 `user_accounts` / `user_identities` / `user_sessions`；注册账号使用 bigint 自增主键，接口 ID 仍为字符串，迁移行为见本文新增说明。
- 复用 `/api/internal/icc` 路由前缀：本地/生产转发 market :9879，预发转发独立 market :9872。不新增 nginx location，不硬编码环境域名。
- 后台同源 cookie，沿用内部端点 CORS 白名单；不是公开 `/api/market/*` Bearer 端点。
- 预发/线上仍是独立数据库；部署前分别确认 market 与 admin 版本，不能把本地实现宣称为上线。
- 仅内部管理员入口，不新增面向客户端的入口或 feature flag。
- 集成验证仅使用 loopback、`filter_auth_test_` 前缀的临时 PostgreSQL 数据库，绝不访问预发/线上。

## 分工与同步

前端：`filter-manage-admin` 的 `src/pages/users`、`src/services/users.ts`、路由及真实 session role 传递/访问控制。后端：`filter-manage-api/server-go/internal/userauth` 的只读查询、`internal/api` 的受权 handler/路由及测试。主 Agent 负责合同一致性、环境核对与整体验证。

详细服务端契约见相邻 `filter-manage-api` 仓库的 `docs/admin-users.md`。验证结果在本文件的完成记录补充；未提交或部署。
## 本地完成与验证（2026-11-10）

- admin `/users` 页面及 market `GET /api/internal/icc/users` 已实现；仅注册客户端账号、只读、ADMIN 可见/可读。
- 前端 `npm test`：5 个测试文件、21 项测试通过；`npm run tsc`、生产 `npm run build` 与本任务文件 lint 通过。局部类型标注修复验证前已存在的 Umi 默认导出 TS2883，补齐既有 Vitest 引用但缺失的测试 setup；未增加依赖。
- 后端 `go test ./... -count=1` 与 `go vet ./...` 通过，真实 PostgreSQL 16.4 集成测试使用新的 loopback 临时 cluster 与专用测试数据库，最终运行无测试项 skip。新增回归覆盖权限、参数、空页保留总数、多身份去重、字面量搜索、稳定分页与错误脱敏。
- 真实浏览器使用 28 个合成账号验证：第一页之外的邮箱可搜索到；BANNED 总数 4、BANNED+GitHub 总数 1；无匹配显示独立空态；ADMIN 可见列表，EDITOR 菜单隐藏、直接访问显示 403。375px 窄屏筛选栏换行、整页无横向溢出，长表在容器内横向滚动。
- 本地 QA 的代理配置仅经验证进程环境传入；为处理 Umi 代理改写 Origin，临时 market 只允许指定 loopback 来源。未改部署代理或扩大生产 CORS 白名单。
- 未使用真实账号、真实邮箱、预发/线上数据库，未触发邮件/OAuth。只验证本地实现，不宣称已经上线；需分别部署 admin 和 API market。

## 注册用户自增 ID（2026-11-11，已部署预发）

- 注册账号表 `user_accounts.id` 改为 PostgreSQL `bigint GENERATED ALWAYS AS IDENTITY`，从 1 开始递增；身份、会话和登录票据的 `account_id` 外键同步改为 bigint。会话 ID、匿名/账号投稿身份 UUID 和后台管理员账号保持各自原有含义。
- 登录、注册、refresh、ticket、session 及后台用户列表中的 `account.id` / `id` 仍是十进制字符串，例如 `"1"`、`"2"`。前端和客户端不能转成 JavaScript number，以免丢失 bigint 精度；现有客户端字符串协议可兼容。
- market 启动时 `userauth.Repo.EnsureSchema` 在一个事务内识别旧 text 主键并执行一次性测试阶段迁移：删除旧注册账号，通过级联删除其登录身份、会话、轮换历史及账号票据，解除旧账号的 `market_contributors.account_id` 绑定，再转换主键及外键。保留投稿、ICC 数据、投稿 UUID 和后台管理员。
- 首次升级后旧测试账号需要重新注册，新账号从 `"1"` 开始。已经是 bigint 的数据库后续启动不再清理或重置序列；并发启动通过事务级 advisory lock 串行化，迁移失败会完整回滚。
- 预发和线上是独立数据库。2026-11-11 已按用户授权部署预发 `b6b2e2e`（CI run 43024858715），清理预发 2 个旧测试账号及登录记录；数据库主键与相关外键均为 bigint，序列尚未调用，下一次注册从 ID 1 开始。ICC、投稿、contributor 和后台管理员保留。线上数据库与服务镜像未变化。
- 用户列表按注册时间倒序、数字 ID 倒序分页；ID 搜索使用十进制文本的字面量子串，不改变昵称、邮箱搜索及权限。
