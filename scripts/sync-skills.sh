#!/bin/bash
# Multi-Agent Framework - Skills Sync Script
# 用途：根据角色动态链接 skills
# QoderCLI 需要 目录/SKILL.md 格式（含 YAML frontmatter），其他 CLI 用 symlink

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
    echo "  ./scripts/sync-skills.sh qoder backend     # QoderCLI 切换到后端"
    echo ""
    echo "可用 CLI: claude, kiro, opencode, qoder"
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

# ── 链接函数 ──
# link_skill <source_category> <skill_name_without_ext>
#   source_category: shared, frontend, backend, devops
#   QoderCLI: 创建 目录/SKILL.md 格式（从源 .md 提取 frontmatter）
#   其他 CLI: symlink

link_skill() {
    local category="$1"
    local skill_name="$2"
    local src_dir=".skills/${category}"
    local src_md="${src_dir}/${skill_name}.md"
    local src_skill_dir="${src_dir}/${skill_name}"

    # 优先处理已有 SKILL.md 的目录格式 skill
    if [ -d "$src_skill_dir" ] && [ -f "${src_skill_dir}/SKILL.md" ]; then
        if [ "$CLI_NAME" = "qoder" ]; then
            # QoderCLI: symlink 整个目录（已有 SKILL.md，格式兼容）
            ln -s "../../${src_skill_dir}" "${TARGET_DIR}/${skill_name}"
        else
            ln -s "../../${src_skill_dir}" "${TARGET_DIR}/${skill_name}"
        fi
        return
    fi

    # 处理纯 .md 文件
    if [ -f "$src_md" ]; then
        if [ "$CLI_NAME" = "qoder" ]; then
            # QoderCLI: 生成 SKILL.md 到中间目录 .skills/_build/，再 symlink
            local build_dir=".skills/_build/${category}/${skill_name}"
            mkdir -p "$build_dir"
            # 从源 .md 提取描述：取第一个 # 标题行作为 description
            local desc=$(grep -m1 '^# ' "$src_md" | sed 's/^# //')
            {
                echo "---"
                echo "name: ${skill_name}"
                echo "description: ${desc}"
                echo "---"
                echo ""
                cat "$src_md"
            } > "${build_dir}/SKILL.md"
            # symlink: .qoder/skills/<name> → ../../.skills/_build/<category>/<name>
            ln -s "../../${build_dir}" "${TARGET_DIR}/${skill_name}"
        else
            # 其他 CLI: 直接 symlink .md 文件
            ln -s "../../${src_md}" "${TARGET_DIR}/${skill_name}.md"
        fi
        return
    fi
}

# ── 批量链接函数 ──
# link_category <category>
link_category() {
    local category="$1"
    local src_dir=".skills/${category}"

    if [ ! -d "$src_dir" ]; then
        return
    fi

    echo "🔗 链接 ${category} skills..."

    # 链接纯 .md 文件
    for skill_md in ${src_dir}/*.md; do
        if [ -f "$skill_md" ]; then
            local name=$(basename "$skill_md" .md)
            link_skill "$category" "$name"
        fi
    done

    # 链接已有 SKILL.md 的目录
    for skill_dir in ${src_dir}/*/; do
        if [ -d "$skill_dir" ] && [ -f "${skill_dir}SKILL.md" ]; then
            local name=$(basename "$skill_dir")
            link_skill "$category" "$name"
        fi
    done
}

echo "🔄 开始同步 skills..."
echo "   CLI: $CLI_NAME"
echo "   角色: $ROLE"
echo "   目标: $TARGET_DIR"
echo ""

# 清理旧链接/目录
if [ -d "$TARGET_DIR" ]; then
    echo "🗑️  清理旧 skills..."
    rm -rf "$TARGET_DIR"
fi

# QoderCLI: 清理旧的 _build 中间目录
if [ "$CLI_NAME" = "qoder" ] && [ -d ".skills/_build" ]; then
    echo "🗑️  清理旧 _build..."
    rm -rf ".skills/_build"
fi

# 创建目录
mkdir -p "$TARGET_DIR"

# 链接 shared skills（所有角色通用）
link_category "shared"

# 根据角色链接特定 skills
case $ROLE in
    frontend)
        link_category "frontend"
        ;;
    backend)
        link_category "backend"
        ;;
    universal)
        link_category "frontend"
        link_category "backend"
        ;;
    devops)
        link_category "devops"
        ;;
esac

echo ""
echo "✅ Skills 同步完成！"
echo ""
echo "📂 $CLI_NAME 当前 skills:"
if [ "$CLI_NAME" = "qoder" ]; then
    for item in "$TARGET_DIR"/*/; do
        if [ -d "$item" ]; then
            echo "   - $(basename "$item")"
        fi
    done
else
    ls -la "$TARGET_DIR" | grep -v "^total" | grep -v "^\." | awk '{print "   - " $NF}'
fi
echo ""
echo "🎯 $CLI_NAME 现在是 $ROLE agent"
echo ""
echo "💡 提示："
echo "   1. 重启 $CLI_NAME 使改动生效"
echo "   2. 查看职责文档: .agent/${ROLE}.md"
echo "   3. 查看统一规则: .rules/*.md"
