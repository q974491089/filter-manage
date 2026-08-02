---
name: prd
description: Use at the start of a new feature, before planning or coding, to turn a rough idea into a sharp PRD. Runs a grilling interview first, then writes a structured PRD to .docs/prd/ and hands off to writing-plans.
---

# Skill: 生成 PRD（产品需求文档）

把一个模糊的想法，经过**追问式盘问**打磨成清晰、可落地的 PRD，作为后续 `writing-plans` 生成执行计划的输入。这是 AI 协作开发主线的第一步（见 `WORKFLOW.md`）。

主线位置：
```
【本 skill】grill 追问 → PRD(.docs/prd/) → writing-plans(.docs/plans/) → 分工执行 → 验证 → 发布+归档
```

## 触发条件

- 用户说："写个 PRD"、"我想做个新功能"、"帮我理需求"、"prd"
- 用户描述了一个还比较粗的功能想法，准备开工前

## 执行步骤

### 1. 先盘问（grilling）

调用 `grill-me` skill（`/grilling`）对用户的想法做**无情追问**，逼出模糊点：

- 到底要解决谁的什么问题？不做什么（非目标）？
- 涉及哪些端（前端 / 客户端 Rust / 服务端）？—— 这直接决定后续分工。
- 边界与约束（Windows 平台、WSL 编辑+Windows 构建、现有架构）。
- 成功长什么样？怎么验收？
- 有没有已知的坑 / 依赖 / 风险？

若 `grill-me` 不可用，就用等价的结构化追问顶上，别跳过"把需求问清楚"这一步。

### 2. 写 PRD

盘问收敛后，按 `.docs/prd/README.md` 的模板产出，存到：

```
.docs/prd/YYYY-MM-DD-<feature-slug>.md
```

必含章节：**背景 / 目标 / 范围 / 非目标 / 涉及端 / 验收标准 / 已知风险**。

- 「涉及端」要写明前端/后端/服务端各自要动什么 —— 供 `.rules/subagent-dispatch.md` 判断是否跨端、如何分工。
- 验收标准要可检验，不写"体验更好"这种空话。

### 3. 交接给 writing-plans

PRD 写完后提示用户：

> PRD 已保存到 `.docs/prd/<file>.md`。是否进入 **writing-plans** 生成分步执行计划（落盘 `.docs/plans/`）？

用户确认后调用 `writing-plans` skill，**plan 统一存 `.docs/plans/`**（覆盖该 skill 默认路径）。

## 注意

- PRD 是"做什么/为什么"，plan 是"怎么做" —— 不要在 PRD 里写具体代码步骤。
- PRD 属于活跃开发依据，放 `.docs/prd/`；功能发布后可按 `.rules/archive.md` 归档。
