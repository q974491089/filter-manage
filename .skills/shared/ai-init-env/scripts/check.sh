#!/bin/bash
# AI Init Environment - 状态检查脚本
# 用于检查当前项目的多 Agent 框架状态

# 脚本所在目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# 加载环境检测
source "$SCRIPT_DIR/env-detect.sh"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 目标项目目录（默认当前目录）
TARGET_DIR="${1:-.}"

# 转换为绝对路径
TARGET_DIR=$(realpath "$TARGET_DIR")

# 状态统计
TOTAL_CHECKS=0
PASSED_CHECKS=0
FAILED_CHECKS=0
WARNINGS=0

# 辅助函数：检查通过
pass() {
    ((TOTAL_CHECKS++))
    ((PASSED_CHECKS++))
    echo -e "  ${GREEN}✅ $1${NC}"
}

# 辅助函数：检查失败
fail() {
    ((TOTAL_CHECKS++))
    ((FAILED_CHECKS++))
    echo -e "  ${RED}❌ $1${NC}"
}

# 辅助函数：警告
warn() {
    ((TOTAL_CHECKS++))
    ((WARNINGS++))
    echo -e "  ${YELLOW}⚠️  $1${NC}"
}

# 辅助函数：信息
info() {
    echo -e "  ${BLUE}ℹ️  $1${NC}"
}

# 检查目录是否存在
check_directory() {
    local dir="$1"
    local label="$2"

    if [[ -d "$TARGET_DIR/$dir" ]]; then
        local count=$(find "$TARGET_DIR/$dir" -type f 2>/dev/null | wc -l)
        pass "$label 目录存在 ($count 个文件)"
        return 0
    else
        fail "$label 目录缺失"
        return 1
    fi
}

# 检查文件是否存在
check_file() {
    local file="$1"
    local label="$2"

    if [[ -f "$TARGET_DIR/$file" ]]; then
        pass "$label 存在"
        return 0
    else
        fail "$label 缺失"
        return 1
    fi
}

# 检查 symlink 有效性
check_symlinks() {
    local dir="$1"
    local label="$2"

    if [[ ! -d "$TARGET_DIR/$dir" ]]; then
        return 1
    fi

    local total=0
    local valid=0
    local broken=0

    while IFS= read -r link; do
        if [[ -L "$link" ]]; then
            ((total++))
            if [[ -e "$link" ]]; then
                ((valid++))
            else
                ((broken++))
                warn "  损坏的 symlink: $link"
            fi
        fi
    done < <(find "$TARGET_DIR/$dir" -type l 2>/dev/null)

    if [[ $total -eq 0 ]]; then
        info "$label: 无 symlink"
        return 0
    fi

    if [[ $broken -eq 0 ]]; then
        pass "$label: $valid/$total symlink 有效"
        return 0
    else
        fail "$label: $broken/$total symlink 损坏"
        return 1
    fi
}

# 检查 Agent 定义
check_agent() {
    echo ""
    echo -e "${CYAN}📋 Agent 定义${NC}"

    check_directory ".agent" "Agent 定义"

    # 检查角色文件
    local roles=("universal" "frontend" "backend")
    for role in "${roles[@]}"; do
        if [[ -f "$TARGET_DIR/.agent/$role.md" ]]; then
            pass "  角色 $role 已定义"
        else
            warn "  角色 $role 未定义"
        fi
    done
}

# 检查 Rules
check_rules() {
    echo ""
    echo -e "${CYAN}📜 Rules${NC}"

    check_directory ".rules" "Rules"

    # 检查核心规则文件
    local rules=("tools" "docs" "handoff" "git")
    for rule in "${rules[@]}"; do
        if [[ -f "$TARGET_DIR/.rules/$rule.md" ]]; then
            pass "  规则 $rule 已定义"
        else
            warn "  规则 $rule 未定义"
        fi
    done
}

# 检查 Skills
check_skills() {
    echo ""
    echo -e "${CYAN}🎯 Skills${NC}"

    check_directory ".skills" "Skills 源文件"

    # 检查各 CLI 的 skills 目录
    local clis=("claude" "opencode" "qoder" "codex")
    for cli in "${clis[@]}"; do
        local skills_dir="."

        case "$cli" in
            claude) skills_dir=".claude/skills" ;;
            opencode) skills_dir=".opencode/skills" ;;
            qoder) skills_dir=".qoder/skills" ;;
            codex) skills_dir=".codex/skills" ;;
        esac

        if [[ -d "$TARGET_DIR/$skills_dir" ]]; then
            local count=$(find "$TARGET_DIR/$skills_dir" -maxdepth 1 -type l 2>/dev/null | wc -l)
            if [[ $count -gt 0 ]]; then
                check_symlinks "$skills_dir" "$cli skills"
            else
                info "$cli skills: 无 symlink（可能未配置）"
            fi
        else
            info "$cli skills: 目录不存在（未配置该 CLI）"
        fi
    done
}

# 检查 Hooks
check_hooks() {
    echo ""
    echo -e "${CYAN}🪝 Hooks${NC}"

    local clis=("claude" "opencode" "qoder" "codex")
    for cli in "${clis[@]}"; do
        local hooks_dir="."

        case "$cli" in
            claude) hooks_dir=".claude/hooks" ;;
            opencode) hooks_dir=".opencode/hooks" ;;
            qoder) hooks_dir=".qoder/hooks" ;;
            codex) hooks_dir=".codex/hooks" ;;
        esac

        if [[ -d "$TARGET_DIR/$hooks_dir" ]]; then
            local count=$(find "$TARGET_DIR/$hooks_dir" -type f 2>/dev/null | wc -l)
            if [[ $count -gt 0 ]]; then
                pass "$cli hooks: $count 个文件"
            else
                info "$cli hooks: 目录存在但无文件"
            fi
        else
            info "$cli hooks: 目录不存在（未配置该 CLI）"
        fi
    done
}

# 检查配置文件
check_config() {
    echo ""
    echo -e "${CYAN}⚙️  配置文件${NC}"

    check_file "AGENTS.md" "AGENTS.md"
    check_file "CLAUDE.md" "CLAUDE.md"

    # 检查各 CLI 配置
    if [[ -d "$TARGET_DIR/.claude" ]]; then
        check_file ".claude/settings.local.json" "Claude Code settings"
    fi

    if [[ -d "$TARGET_DIR/.opencode" ]]; then
        check_file ".opencode/opencode.json" "OpenCode config"
    fi

    if [[ -d "$TARGET_DIR/.qoder" ]]; then
        check_file ".qoder/settings.json" "QoderCLI settings"
    fi

    if [[ -d "$TARGET_DIR/.codex" ]]; then
        check_file ".codex/config.toml" "Codex config"
    fi
}

# 检查 Scripts
check_scripts() {
    echo ""
    echo -e "${CYAN}📜 Scripts${NC}"

    if [[ -f "$TARGET_DIR/scripts/sync-skills.sh" ]]; then
        if [[ -x "$TARGET_DIR/scripts/sync-skills.sh" ]]; then
            pass "sync-skills.sh 存在且可执行"
        else
            warn "sync-skills.sh 存在但不可执行"
        fi
    else
        fail "sync-skills.sh 缺失"
    fi
}

# 检查 CodeGraph
check_codegraph() {
    echo ""
    echo -e "${CYAN}📊 CodeGraph${NC}"

    if [[ -d "$TARGET_DIR/.codegraph" ]]; then
        pass ".codegraph 目录存在"

        # 检查是否有索引文件
        if [[ -f "$TARGET_DIR/.codegraph/codegraph.db" ]]; then
            pass "CodeGraph 索引已初始化"
        else
            warn "CodeGraph 索引未初始化"
            info "运行 'codegraph init' 初始化索引"
        fi
    else
        info ".codegraph 目录不存在（可选组件）"
    fi
}

# 检查已安装的 CLI
check_installed_clis() {
    echo ""
    echo -e "${CYAN}🔧 已安装 CLI${NC}"

    local clis=("claude" "opencode" "qoder" "codex" "codegraph")
    for cli in "${clis[@]}"; do
        if command -v "$cli" &> /dev/null; then
            local version=$($cli --version 2>/dev/null | head -1 || echo "未知版本")
            pass "$cli: $version"
        else
            info "$cli: 未安装"
        fi
    done
}

# 检查 Git 状态
check_git() {
    echo ""
    echo -e "${CYAN}📦 Git 状态${NC}"

    if [[ -d "$TARGET_DIR/.git" ]]; then
        pass "Git 仓库已初始化"

        # 检查 .gitignore
        if [[ -f "$TARGET_DIR/.gitignore" ]]; then
            pass ".gitignore 存在"

            # 检查关键忽略项
            local ignores=(".codegraph" ".skills/_build" ".env.local")
            for ignore in "${ignores[@]}"; do
                if grep -q "$ignore" "$TARGET_DIR/.gitignore" 2>/dev/null; then
                    pass "  $ignore 已忽略"
                else
                    warn "  $ignore 未在 .gitignore 中"
                fi
            done
        else
            warn ".gitignore 缺失"
        fi
    else
        warn "Git 仓库未初始化"
    fi
}

# 显示环境信息
show_environment() {
    echo -e "${CYAN}🌍 环境信息${NC}"
    echo ""

    local env=$(detect_environment)
    echo -e "  操作系统: ${GREEN}$env${NC}"
    echo -e "  内核版本: ${GREEN}$(uname -r 2>/dev/null)${NC}"
    echo -e "  架构: ${GREEN}$(uname -m 2>/dev/null)${NC}"

    if [[ "$env" == "wsl" ]]; then
        echo -e "  WSL 版本: ${GREEN}$(cat /proc/version 2>/dev/null | head -1)${NC}"
    fi

    echo -e "  Shell: ${GREEN}$SHELL${NC}"
    echo -e "  项目路径: ${GREEN}$TARGET_DIR${NC}"
}

# 显示摘要
show_summary() {
    echo ""
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
    echo -e "${CYAN}📊 检查摘要${NC}"
    echo -e "${CYAN}═══════════════════════════════════════════════════════════════${NC}"
    echo ""
    echo -e "  总检查项: ${BLUE}$TOTAL_CHECKS${NC}"
    echo -e "  通过: ${GREEN}$PASSED_CHECKS${NC}"
    echo -e "  失败: ${RED}$FAILED_CHECKS${NC}"
    echo -e "  警告: ${YELLOW}$WARNINGS${NC}"
    echo ""

    if [[ $FAILED_CHECKS -eq 0 && $WARNINGS -eq 0 ]]; then
        echo -e "  ${GREEN}✅ 环境状态完美！${NC}"
    elif [[ $FAILED_CHECKS -eq 0 ]]; then
        echo -e "  ${YELLOW}⚠️  环境基本正常，但有警告项需要关注${NC}"
    else
        echo -e "  ${RED}❌ 环境存在问题，建议运行 'ai-init-env init' 修复${NC}"
    fi
}

# 显示建议
show_recommendations() {
    echo ""
    echo -e "${CYAN}💡 建议${NC}"
    echo ""

    if [[ $FAILED_CHECKS -gt 0 ]]; then
        echo -e "  1. 运行 ${GREEN}ai-init-env init${NC} 初始化缺失组件"
    fi

    if [[ $WARNINGS -gt 0 ]]; then
        echo -e "  2. 检查警告项，决定是否需要处理"
    fi

    # 检查 symlink 是否有损坏的
    local broken_links=$(find "$TARGET_DIR" -type l ! -exec test -e {} \; -print 2>/dev/null | wc -l)
    if [[ $broken_links -gt 0 ]]; then
        echo -e "  3. 运行 ${GREEN}./scripts/sync-skills.sh claude universal${NC} 重建 symlink"
    fi

    echo ""
}

# 主函数
main() {
    echo -e "${CYAN}🔍 AI Environment Status${NC}"
    echo ""
    echo -e "项目: ${GREEN}$TARGET_DIR${NC}"
    echo ""

    # 检查目标目录是否存在
    if [[ ! -d "$TARGET_DIR" ]]; then
        echo -e "${RED}❌ 错误：目标目录不存在: $TARGET_DIR${NC}" >&2
        exit 1
    fi

    # 执行各项检查
    show_environment
    check_agent
    check_rules
    check_skills
    check_hooks
    check_config
    check_scripts
    check_codegraph
    check_installed_clis
    check_git

    # 显示摘要
    show_summary

    # 显示建议
    show_recommendations
}

# 运行主函数
main
