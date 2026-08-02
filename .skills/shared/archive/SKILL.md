---
name: archive
description: Use when a feature is fully done/released, or when the user asks to archive/clean up completed plans, handoff docs, one-off reports, or accumulated SYNC_STATUS signals. Moves finished history into .docs/archive/.
---

# Skill: 归档历史产物

把**已完成、不再作为开发依据**的文档移入 `.docs/archive/`，保留可追溯历史。规则见 `.rules/archive.md`。

## 触发条件

- 用户说："归档"、"清理已完成的计划/交接"、"archive"、"整理历史文档"
- 发布流程结束后自动调用（见 `release-workflow` skill）

## 执行步骤

### 1. 扫描候选

列出可能可归档的对象，**逐类判断是否"已完成"**：

| 候选 | 完成判据 | 位置 |
|------|---------|------|
| 一次性完工/实施/迁移报告 | 产出即完成 | 根目录、`.agent/` |
| 功能计划 | plan 内所有 Phase/Task 完成且已发布 | `.docs/plans/*.md` |
| 交接文档 | 对端已接入、功能已上线 | `.docs/handoff/*.md` |
| SYNC_STATUS 旧信号 | 已被前端消费 | `SYNC_STATUS.md` |

**判断 plan 是否完成**：读文件头的「状态」字段。`TODO` / `Phase N 待做` / 进行中 → **不归档**。

### 2. 逐个与用户确认

**不要自动批量移动。** 列出每个候选 + 你的完成判断，让用户确认。不确定的默认**不归档**。

### 3. 执行归档（对每个确认项）

1. 在文件头部插入 banner：
   ```
   > 📦 已归档 YYYY-MM-DD · 仅供历史追溯，非当前开发依据
   ```
2. 移动（保留 git 历史）：
   ```bash
   git mv <原路径> .docs/archive/<reports|plans|handoff|sync-log>/
   ```
3. 若有活跃文档引用它 → 更新引用路径。

### 4. 更新归档索引

在 `.docs/archive/README.md` 的「归档索引」表加一行：日期 / 内容 / 原→现路径 / 说明。

### 5. SYNC_STATUS 收敛（如涉及）

- 把 `SYNC_STATUS.md` 的历史信号**追加**到 `.docs/archive/sync-log/history.md`。
- `SYNC_STATUS.md` 只保留：信号格式说明 + 最近尚未被前端消费的信号。

### 6. 汇报

告诉用户：归档了哪些、去了哪、索引已更新。提醒"归档区平时不读，仅追溯历史时查"。

## 注意

- ❌ 不归档进行中/待办的 plan。
- ❌ 不用 `rm`，一律 `git mv`。
- ✅ 拿不准就不动，问用户。
