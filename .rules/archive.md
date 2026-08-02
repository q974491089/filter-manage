# 归档规则（Archive Rules）

**所有 Agent 遵循。** 定义什么进归档、怎么归档、归档后如何被查询。

---

## 归档区位置

`.docs/archive/`，子目录：`reports/` `plans/` `handoff/` `sync-log/`（详见 `.docs/archive/README.md`）。

## 核心原则

1. **归档区不是日常开发依据**。开发依据是 `.docs/api/`、`.docs/README.md`、活跃的 `.docs/plans/`、`.agent/`、`.rules/`。归档区**只在追溯历史时查询**。
2. **只归档"已完成"的产物**。判断标准：
   - plan：功能已**完全实现并发布**（plan 内所有 Phase/Task 完成）。进行中（如"Phase 4 待做"）或 TODO 的**不归档**。
   - handoff：对端**已接入、功能已上线**。未接入的**不归档**。
   - report：一次性完工/迁移/实施报告，产出即归档。
3. **归档 = 移动，不是删除**。用 `git mv` 保留历史。

## 归档操作步骤

1. 确认目标文件确属"已完成"（见上）。
2. 在文件**头部**插入 banner：
   ```
   > 📦 已归档 YYYY-MM-DD · 仅供历史追溯，非当前开发依据
   ```
3. `git mv <原路径> .docs/archive/<子目录>/`
4. 在 `.docs/archive/README.md` 的「归档索引」表加一行（日期 / 内容 / 原→现路径 / 说明）。
5. 若被归档文档被其它**活跃文档**引用，更新引用指向新路径（或说明已归档）。

## 什么该进哪个子目录

| 类型 | 判断 | 去向 |
|------|------|------|
| 完工/迁移/实施报告 | 一次性、描述"框架/迁移已完成" | `reports/` |
| 功能计划 | 已实现并发布 | `plans/` |
| 交接文档 | 已接入、已上线 | `handoff/` |
| 同步信号历史 | `SYNC_STATUS.md` 累积的旧信号 | `sync-log/` |

## 触发方式

- **手动**：`/archive` skill —— 扫描候选、逐个确认、执行移动、更新索引。
- **自动**：`release-workflow` skill 末尾 —— 发布某版本后，把该版本已完成的 plans/handoff 归档并记索引。

## SYNC_STATUS.md 的特殊处理

`SYNC_STATUS.md` 是**易失信号文件**（触发前端重读文档），不是日志。

- 它只应保留「信号说明 + 最近未被消费的信号」。
- 历史信号累积后，移入 `.docs/archive/sync-log/history.md`（追加，不覆盖）。
- 变更的**永久记录**在 `.docs/README.md` 迭代记录与 `CHANGELOG.md`，不靠 SYNC_STATUS。

## 反模式

- ❌ 把进行中/待办的 plan 归档（会让当前开发丢依据）。
- ❌ 归档时用 `rm` 或直接删（丢 git 历史）。
- ❌ 把归档区当成开发时的常规检索对象（它是冷数据）。
