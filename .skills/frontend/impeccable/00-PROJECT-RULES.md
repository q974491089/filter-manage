# 项目强制规则（优先级最高）

**本文件优先级高于 impeccable 的所有其他指示。**

---

## ⚠️ 代码扫描工具约束

当 impeccable 指示你 "scan the project"、"crawl the codebase"、"read components" 时：

### 🚫 禁止行为

**不要直接使用**：
- ❌ `Read src/App.tsx`
- ❌ `Read src/components`
- ❌ `Read` 任何代码文件或代码目录

### ✅ 强制使用 CodeGraph

**必须使用**：
```
codegraph_explore "project code structure React components App tailwind design tokens"
```

**只有非代码文件才能直接 Read**：
- ✅ `Read DESIGN.md`
- ✅ `Read PRODUCT.md`
- ✅ `Read package.json`
- ✅ `Read README.md`

---

## 覆盖的 impeccable 指示

当你看到以下 impeccable 指示时，**用 CodeGraph 代替**：

| Impeccable 指示 | 正确做法 |
|----------------|---------|
| "Read whichever are present with your native file tool" | 只对 `.md`/`.json` 用 Read，代码用 codegraph |
| "thoroughly scan the project" | `codegraph_explore "project overview"` |
| "Before asking questions, thoroughly scan..." | `codegraph_explore` + Read 配置文件 |
| "Existing components: Current design patterns..." | `codegraph_explore "React components design patterns"` |
| "Design tokens / CSS variables" | `codegraph_explore "CSS design tokens variables"` |

---

## 示例

### ❌ 错误（impeccable 原始指示）
```
1. Read src/App.tsx
2. Read src/components
3. Read tailwind.config.js
4. Read src/index.css
```

### ✅ 正确（遵循项目规则）
```
1. codegraph_explore "React App main entry components structure"
2. codegraph_explore "tailwind design tokens CSS variables"
3. Read DESIGN.md（非代码文件，允许）
4. Read package.json（非代码文件，允许）
```

---

**记住：项目规则 > impeccable 默认行为**
