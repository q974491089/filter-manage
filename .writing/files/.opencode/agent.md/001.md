# OpenCode Agent Configuration

**Model**: Xiaomi MiMo  
**Role**: Frontend  
**Responsibility Document**: `.agent/frontend.md`

---

## Quick Reference

### Code Range

**Can Modify**:
- `src/` - All React components, hooks, utils
- `tailwind.config.js` - Tailwind configuration
- `package.json` - Frontend dependencies
- `vite.config.ts` - Vite configuration
- `tsconfig.json` - TypeScript configuration

**Cannot Modify**:
- `src-tauri/` - Backend Rust code (Backend Agent responsibility)
- `Cargo.toml` / `Cargo.lock` - Backend dependencies
- `.github/workflows/` - CI/CD configuration (DevOps Agent responsibility)

### Core Rules

1. **CodeGraph First**: Any code operation must use CodeGraph first
2. **Design System**: Follow `DESIGN.md` specifications
3. **DevTools**: Add `data-component` and `data-name` attributes to all components
4. **WSL Restrictions**: No `npm install` in WSL, use Windows PowerShell

### Skills

Linked from `.skills/` to `.opencode/skills/`:
- `systematic-debugging.md`
- `test-driven-development.md`
- `using-superpowers.md`
- `verification-before-completion.md`
- `writing-plans.md`
- `requesting-code-review.md`
- `stitch-d2c.md`

### Documentation

**Must Read First**:
1. `AGENTS.md` - Agent registry
2. `.agent/frontend.md` - Detailed responsibility document
3. `.rules/tools.md` - Tool usage rules (mandatory)

**API Documentation**:
- `.docs/api/*.md` - API reference
- `SYNC_STATUS.md` - Backend sync signals

### Handoff Protocol

When backend support needed:
1. Create `.docs/handoff/<feature>-backend.md`
2. Backend Agent implements
3. Backend updates `.docs/api/<module>.md`
4. Backend creates `.docs/handoff/<feature>-frontend.md`
5. Backend writes `SYNC_STATUS.md` completion signal

---

## Full Documentation

See `.agent/frontend.md` for complete details.
