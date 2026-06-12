#!/bin/bash
# Multi-Agent Framework - Skills Sync Script
# 用途：根据角色动态链接 skills

set -e

CLI_NAME=$1
ROLE=$2

# 显示用法
if [ -z "$CLI_NAME" ] || [ -z "$ROLE" ]; then
    echo "用法: ./scripts/sync-skills.sh <cli-name> <role>"
    echo ""
    echo "示例:"
    echo "  ./scripts/sync-skills.sh claude frontend   # Claude Code 切换到前端"
    echo "  ./scripts/sync-skills.sh claude backend    # Claude Code 切换到后端"
    echo "  ./scripts/sync-skills.sh claude universal  # Claude Code 切换到全栈"
    echo "  ./scripts/sync-skills.sh kiro backend      # Kiro CLI 切换到后端"
    echo ""
    echo "可用 CLI: claude, kiro"
    echo "可用角色: frontend, backend, universal, devops"
    exit 1
fi

# 映射 CLI 到目录
case $CLI_NAME in
    claude)
        TARGET_DIR=".claude/skills"
        ;;
    kiro)
        TARGET_DIR=".kiro/skills"
        ;;
    opencode)
        TARGET_DIR=".opencode/skills"
        ;;
    qoder)
        TARGET_DIR=".qoder/skills"
        ;;
    *)
        echo "❌ 未知 CLI: $CLI_NAME"
        echo "可用 CLI: claude, kiro, opencode, qoder"
        exit 1
        ;;
esac

# 验证角色
case $ROLE in
    frontend|backend|universal|devops)
        # 合法角色
        ;;
    *)
        echo "❌ 未知角色: $ROLE"
        echo "可用角色: frontend, backend, universal, devops"
        exit 1
        ;;
esac

# 检查 skills 目录存在
if [ ! -d ".skills/shared" ]; then
    echo "❌ 错误：.skills/shared/ 目录不存在"
    echo "请确保项目已初始化 Multi-Agent Framework"
    exit 1
fi

echo "🔄 开始同步 skills..."
echo "   CLI: $CLI_NAME"
echo "   角色: $ROLE"
echo "   目标: $TARGET_DIR"
echo ""

# 清理旧链接
if [ -d "$TARGET_DIR" ]; then
    echo "🗑️  清理旧 skills..."
    rm -rf "$TARGET_DIR"
fi

# 创建目录
mkdir -p "$TARGET_DIR"

# 链接 shared skills（所有角色通用）
echo "🔗 链接 shared skills..."
for skill in .skills/shared/*.md; do
    if [ -f "$skill" ]; then
        ln -s "../../$skill" "$TARGET_DIR/$(basename $skill)"
    fi
done
# 链接 shared 目录中的 skill 目录
for skill_dir in .skills/shared/*/; do
    if [ -d "$skill_dir" ] && [ -f "${skill_dir}SKILL.md" ]; then
        ln -s "../../$skill_dir" "$TARGET_DIR/$(basename $skill_dir)"
    fi
done

# 根据角色链接特定 skills
case $ROLE in
    frontend)
        if [ -d ".skills/frontend" ]; then
            echo "🔗 链接 frontend skills..."
            for skill in .skills/frontend/*.md; do
                if [ -f "$skill" ]; then
                    ln -s "../../$skill" "$TARGET_DIR/$(basename $skill)"
                fi
            done
            # 链接 frontend 目录中的 skill 目录
            for skill_dir in .skills/frontend/*/; do
                if [ -d "$skill_dir" ] && [ -f "${skill_dir}SKILL.md" ]; then
                    ln -s "../../$skill_dir" "$TARGET_DIR/$(basename $skill_dir)"
                fi
            done
        fi
        ;;
    backend)
        if [ -d ".skills/backend" ]; then
            echo "🔗 链接 backend skills..."
            for skill in .skills/backend/*.md; do
                if [ -f "$skill" ]; then
                    ln -s "../../$skill" "$TARGET_DIR/$(basename $skill)"
                fi
            done
        fi
        ;;
    universal)
        # Universal 角色链接所有 skills
        if [ -d ".skills/frontend" ]; then
            echo "🔗 链接 frontend skills..."
            for skill in .skills/frontend/*.md; do
                if [ -f "$skill" ]; then
                    ln -s "../../$skill" "$TARGET_DIR/$(basename $skill)"
                fi
            done
            # 链接 frontend 目录中的 skill 目录
            for skill_dir in .skills/frontend/*/; do
                if [ -d "$skill_dir" ] && [ -f "${skill_dir}SKILL.md" ]; then
                    ln -s "../../$skill_dir" "$TARGET_DIR/$(basename $skill_dir)"
                fi
            done
        fi
        if [ -d ".skills/backend" ]; then
            echo "🔗 链接 backend skills..."
            for skill in .skills/backend/*.md; do
                if [ -f "$skill" ]; then
                    ln -s "../../$skill" "$TARGET_DIR/$(basename $skill)"
                fi
            done
            # 链接 backend 目录中的 skill 目录
            for skill_dir in .skills/backend/*/; do
                if [ -d "$skill_dir" ] && [ -f "${skill_dir}SKILL.md" ]; then
                    ln -s "../../$skill_dir" "$TARGET_DIR/$(basename $skill_dir)"
                fi
            done
        fi
        ;;
    devops)
        if [ -d ".skills/devops" ]; then
            echo "🔗 链接 devops skills..."
            for skill in .skills/devops/*.md; do
                if [ -f "$skill" ]; then
                    ln -s "../../$skill" "$TARGET_DIR/$(basename $skill)"
                fi
            done
        fi
        ;;
esac

echo ""
echo "✅ Skills 同步完成！"
echo ""
echo "📂 $CLI_NAME 当前 skills:"
ls -la "$TARGET_DIR" | grep -v "^total" | grep -v "^\." | awk '{print "   - " $NF}'
echo ""
echo "🎯 $CLI_NAME 现在是 $ROLE agent"
echo ""
echo "💡 提示："
echo "   1. 重启 $CLI_NAME 使改动生效"
echo "   2. 查看职责文档: .agent/${ROLE}.md"
echo "   3. 查看统一规则: .rules/*.md"
