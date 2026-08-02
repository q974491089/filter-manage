# 📋 PRD（产品需求文档）

存放**活跃功能**的需求文档。PRD 回答"做什么 / 为什么 / 验收标准"，是 `writing-plans` 生成执行计划的输入。

## 位置与命名

```
.docs/prd/YYYY-MM-DD-<feature-slug>.md
```

## 在主线中的位置

```
grill-me(/grilling 追问) → 【PRD 本目录】 → writing-plans(.docs/plans/) → 分工执行 → 验证 → 发布+归档
```

- 生成方式：`/prd` skill（先盘问，再产出）。详见 `.skills/shared/prd.md`。
- PRD 是活跃开发依据；功能发布后可按 `.rules/archive.md` 归档到 `.docs/archive/`。

## 与其它文档的区别

| 文档 | 回答 | 位置 |
|------|------|------|
| **PRD** | 做什么 / 为什么 / 验收 | `.docs/prd/` |
| **plan** | 怎么做（分步、含代码） | `.docs/plans/` |
| **api/handoff** | 接口契约 | `.docs/api/`、`.docs/handoff/` |

---

## PRD 模板

复制以下模板另存为 `YYYY-MM-DD-<feature-slug>.md`：

```markdown
# [功能名称] PRD

**状态**: 草稿 / 已评审 / 已排期 / 已发布
**创建于**: YYYY-MM-DD
**涉及端**: 前端 / 客户端 Rust / 服务端（勾选实际涉及的）

## 背景

要解决谁的什么问题？为什么现在做？当前是怎么做的、痛点在哪？

## 目标

这个功能要达成什么。可量化优先。

## 范围

本次要做的具体内容（按端拆）：
- 前端：……
- 后端（Rust/Tauri）：……
- 服务端：……

## 非目标

明确本次**不做**什么，避免范围蔓延。

## 涉及端与分工提示

哪些端要改、各改什么。供 `.rules/subagent-dispatch.md` 判断是否跨端、如何分工。

## 验收标准

可检验的完成条件（不写"体验更好"这类空话）：
- [ ] ……
- [ ] ……

## 已知风险 / 依赖 / 坑

技术风险、外部依赖、平台限制（Windows / WSL 构建约束等）、已知的历史坑（可查 `.docs/archive/`）。
```
