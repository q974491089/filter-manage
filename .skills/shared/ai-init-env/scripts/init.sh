#!/bin/bash
# AI Init Environment - 主初始化脚本
# 用于将多 Agent 框架从源项目复制到目标项目

set -e

# 脚本所在目录
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(dirname "$SCRIPT_DIR")"

# 加载环境检测
source "$SCRIPT_DIR/env-detect.sh"

# 颜色定义
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# 默认配置
DEFAULT_SOURCE=""  # 将在运行时确定
TARGET_DIR="."

# 组件选择（默认全选）
SELECT_AGENT=true
SELECT_RULES=true
SELECT_SKILLS=true
SELECT_HOOKS=true
SELECT_CONFIG=true
SELECT_CODEGRAPH=false

# CLI 选择
SELECT_CLAUDE=true
SELECT_OPENCODE=true
SELECT_QODER=false
SELECT_CODEX=false

# 覆盖策略
OVERWRITE_ASK=true  # true=询问, false=跳过
OVERWRITE_ALL=false # 用户选择"全部覆盖"

# 显示帮助
show_help() {
    echo "用法: $0 [选项] [源项目路径]"
    echo ""
    echo "选项:"
    echo "  -h, --help          显示此帮助信息"
    echo "  -t, --target DIR    目标项目目录 (默认: 当前目录)"
    echo "  -y, --yes           自动确认，不询问"
    echo "  -f, --force         强制覆盖所有文件"
    echo "  --no-agent          不复制 Agent 定义"
    echo "  --no-rules          不复制 Rules"
    echo "  --no-skills         不复制 Skills"
    echo "  --no-hooks          不复制 Hooks"
    echo "  --no-config         不复制配置模板"
    echo "  --with-codegraph    包含 CodeGraph 配置"
    echo "  --claude            包含 Claude Code 配置"
    echo "  --opencode          包含 OpenCode 配置"
    echo "  --qoder             包含 QoderCLI 配置"
    echo "  --codex             包含 Codex 配置"
    echo ""
    echo "示例:"
    echo "  $0 /path/to/source-project"
    echo "  $0 -t /path/to/target /path/to/source"
    echo "  $0 --no-hooks --qoder /path/to/source"
}

# 解析参数
parse_args() {
    local source_set=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            -h|--help)
                show_help
                exit 0
                ;;
            -t|--target)
                TARGET_DIR="$2"
                shift 2
                ;;
            -y|--yes)
                OVERWRITE_ASK=false
                OVERWRITE_ALL=true
                shift
                ;;
            -f|--force)
                OVERWRITE_ALL=true
                shift
                ;;
            --no-agent)
                SELECT_AGENT=false
                shift
                ;;
            --no-rules)
                SELECT_RULES=false
                shift
                ;;
            --no-skills)
                SELECT_SKILLS=false
                shift
                ;;
            --no-hooks)
                SELECT_HOOKS=false
                shift
                ;;
            --no-config)
                SELECT_CONFIG=false
                shift
                ;;
            --with-codegraph)
                SELECT_CODEGRAPH=true
                shift
                ;;
            --claude)
                SELECT_CLAUDE=true
                shift
                ;;
            --opencode)
                SELECT_OPENCODE=true
                shift
                ;;
            --qoder)
                SELECT_QODER=true
                shift
                ;;
            --codex)
                SELECT_CODEX=true
                shift
                ;;
            -*)
                echo -e "${RED}❌ 未知选项: $1${NC}" >&2
                show_help
                exit 1
                ;;
            *)
                if [[ "$source_set" == false ]]; then
                    DEFAULT_SOURCE="$1"
                    source_set=true
                else
                    echo -e "${RED}❌ 多余的参数: $1${NC}" >&2
                    show_help
                    exit 1
                fi
                shift
                ;;
        esac
    done
}

# 验证源项目
validate_source() {
    local source="$1"

    if [[ ! -d "$source" ]]; then
        echo -e "${RED}❌ 错误：源项目路径不存在: $source${NC}" >&2
        return 1
    fi

    # 检查必要目录
    local missing=()
    [[ ! -d "$source/.agent" ]] && missing+=(".agent")
    [[ ! -d "$source/.rules" ]] && missing+=(".rules")
    [[ ! -d "$source/.skills" ]] && missing+=(".skills")

    if [[ ${#missing[@]} -gt 0 ]]; then
        echo -e "${RED}❌ 错误：源项目缺少必要目录: ${missing[*]}${NC}" >&2
        echo "请确保源项目已初始化多 Agent 框架" >&2
        return 1
    fi

    return 0
}

# 显示交互式菜单
show_interactive_menu() {
    echo -e "${CYAN}🔧 AI Init Environment${NC}"
    echo ""
    echo -e "源项目: ${GREEN}$DEFAULT_SOURCE${NC}"
    echo -e "目标项目: ${GREEN}$(realpath "$TARGET_DIR")${NC}"
    echo -e "环境: ${GREEN}$(detect_environment)${NC}"
    echo ""

    echo -e "${YELLOW}请选择要初始化的组件（输入编号切换，回车确认）：${NC}"
    echo ""
    echo "  1. $(_checkbox $SELECT_AGENT) Agent 定义     - .agent/*.md（角色职责文档）"
    echo "  2. $(_checkbox $SELECT_RULES) Rules          - .rules/*.md（统一规则）"
    echo "  3. $(_checkbox $SELECT_SKILLS) Skills         - .skills/ + sync-skills.sh"
    echo "  4. $(_checkbox $SELECT_HOOKS) Hooks          - 各 CLI 的 hooks 目录"
    echo "  5. $(_checkbox $SELECT_CONFIG) 配置模板       - AGENTS.md, CLAUDE.md 等"
    echo "  6. $(_checkbox $SELECT_CODEGRAPH) CodeGraph     - .codegraph/ 配置（可选）"
    echo ""

    echo -e "${YELLOW}目标 CLI（输入编号切换，回车确认）：${NC}"
    echo ""
    echo "  A. $(_checkbox $SELECT_CLAUDE) Claude Code    - .claude/"
    echo "  B. $(_checkbox $SELECT_OPENCODE) OpenCode       - .opencode/"
    echo "  C. $(_checkbox $SELECT_QODER) QoderCLI       - .qoder/"
    echo "  D. $(_checkbox $SELECT_CODEX) Codex          - .codex/"
    echo ""
}

# 复选框辅助函数
_checkbox() {
    if [[ "$1" == true ]]; then
        echo -e "${GREEN}[x]${NC}"
    else
        echo -e "${RED}[ ]${NC}"
    fi
}

# 处理用户输入
handle_user_input() {
    while true; do
        read -rp "输入编号切换组件（直接回车确认）: " choice

        case "$choice" in
            1) SELECT_AGENT=$(! $SELECT_AGENT && echo true || echo false) ;;
            2) SELECT_RULES=$(! $SELECT_RULES && echo true || echo false) ;;
            3) SELECT_SKILLS=$(! $SELECT_SKILLS && echo true || echo false) ;;
            4) SELECT_HOOKS=$(! $SELECT_HOOKS && echo true || echo false) ;;
            5) SELECT_CONFIG=$(! $SELECT_CONFIG && echo true || echo false) ;;
            6) SELECT_CODEGRAPH=$(! $SELECT_CODEGRAPH && echo true || echo false) ;;
            [Aa]) SELECT_CLAUDE=$(! $SELECT_CLAUDE && echo true || echo false) ;;
            [Bb]) SELECT_OPENCODE=$(! $SELECT_OPENCODE && echo true || echo false) ;;
            [Cc]) SELECT_QODER=$(! $SELECT_QODER && echo true || echo false) ;;
            [Dd]) SELECT_CODEX=$(! $SELECT_CODEX && echo true || echo false) ;;
            "") break ;;
            *) echo -e "${YELLOW}⚠️  无效输入，请重试${NC}" ;;
        esac

        # 重新显示菜单
        clear
        show_interactive_menu
    done
}

# 询问是否覆盖文件
ask_overwrite() {
    local file="$1"

    if [[ "$OVERWRITE_ALL" == true ]]; then
        return 0  # 全部覆盖
    fi

    if [[ "$OVERWRITE_ASK" == false ]]; then
        return 1  # 全部跳过
    fi

    echo -e "${YELLOW}⚠️  文件已存在: $file${NC}"
    echo ""
    echo "选项："
    echo "  [o] 覆盖 - 用源文件替换"
    echo "  [s] 跳过 - 保留现有文件"
    echo "  [b] 备份 - 备份后覆盖"
    echo "  [a] 全部覆盖 - 后续文件不再询问"
    echo ""

    while true; do
        read -rp "请选择 [o/s/b/a]: " choice
        case "$choice" in
            [Oo])
                return 0  # 覆盖
                ;;
            [Ss])
                return 1  # 跳过
                ;;
            [Bb])
                # 备份
                local backup="${file}.backup.$(date +%Y%m%d%H%M%S)"
                mv "$file" "$backup"
                echo -e "${GREEN}✅ 已备份到: $backup${NC}"
                return 0  # 覆盖
                ;;
            [Aa])
                OVERWRITE_ALL=true
                return 0  # 全部覆盖
                ;;
            *)
                echo -e "${YELLOW}⚠️  无效输入，请重试${NC}"
                ;;
        esac
    done
}

# 复制目录（带覆盖询问）
copy_directory() {
    local src="$1"
    local dst="$2"
    local label="$3"

    if [[ ! -d "$src" ]]; then
        echo -e "${YELLOW}⚠️  源目录不存在，跳过: $src${NC}"
        return 0
    fi

    echo -e "${BLUE}📦 复制 $label...${NC}"

    # 创建目标目录
    mkdir -p "$dst"

    # 复制文件
    local count=0
    for item in "$src"/*; do
        if [[ ! -e "$item" ]]; then
            continue
        fi

        local basename=$(basename "$item")
        local dst_item="$dst/$basename"

        if [[ -e "$dst_item" ]]; then
            if ask_overwrite "$dst_item"; then
                rm -rf "$dst_item"
                cp -r "$item" "$dst_item"
                count=$((count + 1))
            fi
        else
            cp -r "$item" "$dst_item"
            count=$((count + 1))
        fi
    done

    echo -e "${GREEN}✅ 已复制 $count 个文件到 $dst${NC}"
}

# 复制单个文件（带覆盖询问）
copy_file() {
    local src="$1"
    local dst="$2"
    local label="$3"

    if [[ ! -f "$src" ]]; then
        echo -e "${YELLOW}⚠️  源文件不存在，跳过: $src${NC}"
        return 0
    fi

    if [[ -f "$dst" ]]; then
        if ! ask_overwrite "$dst"; then
            return 0
        fi
    fi

    # 确保目标目录存在
    mkdir -p "$(dirname "$dst")"

    cp "$src" "$dst"
    echo -e "${GREEN}✅ 已复制: $label${NC}"
}

# 初始化 Agent 定义
init_agent() {
    if [[ "$SELECT_AGENT" != true ]]; then
        return 0
    fi

    copy_directory "$DEFAULT_SOURCE/.agent" "$TARGET_DIR/.agent" "Agent 定义"
}

# 初始化 Rules
init_rules() {
    if [[ "$SELECT_RULES" != true ]]; then
        return 0
    fi

    copy_directory "$DEFAULT_SOURCE/.rules" "$TARGET_DIR/.rules" "Rules"
}

# 初始化 Skills
init_skills() {
    if [[ "$SELECT_SKILLS" != true ]]; then
        return 0
    fi

    # 复制 .skills/ 目录（源文件）
    copy_directory "$DEFAULT_SOURCE/.skills" "$TARGET_DIR/.skills" "Skills 源文件"

    # 复制 sync-skills.sh 脚本
    if [[ -f "$DEFAULT_SOURCE/scripts/sync-skills.sh" ]]; then
        mkdir -p "$TARGET_DIR/scripts"
        copy_file "$DEFAULT_SOURCE/scripts/sync-skills.sh" "$TARGET_DIR/scripts/sync-skills.sh" "sync-skills.sh"
        chmod +x "$TARGET_DIR/scripts/sync-skills.sh"
    fi

    # 运行 sync-skills.sh 为每个 CLI 创建 symlink
    echo -e "${BLUE}🔗 创建 Skills symlink...${NC}"

    local clis=()
    [[ "$SELECT_CLAUDE" == true ]] && clis+=("claude")
    [[ "$SELECT_OPENCODE" == true ]] && clis+=("opencode")
    [[ "$SELECT_QODER" == true ]] && clis+=("qoder")
    [[ "$SELECT_CODEX" == true ]] && clis+=("codex")

    for cli in "${clis[@]}"; do
        echo -e "  链接 $cli..."
        cd "$TARGET_DIR" && ./scripts/sync-skills.sh "$cli" "universal" 2>/dev/null || true
        cd - > /dev/null
    done
}

# 初始化 Hooks
init_hooks() {
    if [[ "$SELECT_HOOKS" != true ]]; then
        return 0
    fi

    echo -e "${BLUE}🪝 复制 Hooks...${NC}"

    # Claude Code hooks
    if [[ "$SELECT_CLAUDE" == true && -d "$DEFAULT_SOURCE/.claude/hooks" ]]; then
        mkdir -p "$TARGET_DIR/.claude/hooks"
        copy_directory "$DEFAULT_SOURCE/.claude/hooks" "$TARGET_DIR/.claude/hooks" "Claude Code hooks"
    fi

    # OpenCode hooks
    if [[ "$SELECT_OPENCODE" == true && -d "$DEFAULT_SOURCE/.opencode/hooks" ]]; then
        mkdir -p "$TARGET_DIR/.opencode/hooks"
        copy_directory "$DEFAULT_SOURCE/.opencode/hooks" "$TARGET_DIR/.opencode/hooks" "OpenCode hooks"
    fi

    # QoderCLI hooks
    if [[ "$SELECT_QODER" == true && -d "$DEFAULT_SOURCE/.qoder/hooks" ]]; then
        mkdir -p "$TARGET_DIR/.qoder/hooks"
        copy_directory "$DEFAULT_SOURCE/.qoder/hooks" "$TARGET_DIR/.qoder/hooks" "QoderCLI hooks"
    fi
}

# 初始化配置模板
init_config() {
    if [[ "$SELECT_CONFIG" != true ]]; then
        return 0
    fi

    echo -e "${BLUE}⚙️  复制配置模板...${NC}"

    # AGENTS.md
    if [[ -f "$DEFAULT_SOURCE/AGENTS.md" ]]; then
        copy_file "$DEFAULT_SOURCE/AGENTS.md" "$TARGET_DIR/AGENTS.md" "AGENTS.md"
    fi

    # CLAUDE.md（如果不存在，从模板生成）
    if [[ ! -f "$TARGET_DIR/CLAUDE.md" && -f "$DEFAULT_SOURCE/CLAUDE.md" ]]; then
        copy_file "$DEFAULT_SOURCE/CLAUDE.md" "$TARGET_DIR/CLAUDE.md" "CLAUDE.md"
    fi

    # 各 CLI 配置文件
    if [[ "$SELECT_CLAUDE" == true && -f "$DEFAULT_SOURCE/.claude/settings.local.json" ]]; then
        mkdir -p "$TARGET_DIR/.claude"
        copy_file "$DEFAULT_SOURCE/.claude/settings.local.json" "$TARGET_DIR/.claude/settings.local.json" "Claude Code settings"
    fi

    if [[ "$SELECT_OPENCODE" == true && -f "$DEFAULT_SOURCE/.opencode/opencode.json" ]]; then
        mkdir -p "$TARGET_DIR/.opencode"
        copy_file "$DEFAULT_SOURCE/.opencode/opencode.json" "$TARGET_DIR/.opencode/opencode.json" "OpenCode config"
    fi

    if [[ "$SELECT_QODER" == true && -f "$DEFAULT_SOURCE/.qoder/settings.json" ]]; then
        mkdir -p "$TARGET_DIR/.qoder"
        copy_file "$DEFAULT_SOURCE/.qoder/settings.json" "$TARGET_DIR/.qoder/settings.json" "QoderCLI settings"
    fi

    if [[ "$SELECT_CODEX" == true && -f "$DEFAULT_SOURCE/.codex/config.toml" ]]; then
        mkdir -p "$TARGET_DIR/.codex"
        copy_file "$DEFAULT_SOURCE/.codex/config.toml" "$TARGET_DIR/.codex/config.toml" "Codex config"
    fi
}

# 初始化 CodeGraph
init_codegraph() {
    if [[ "$SELECT_CODEGRAPH" != true ]]; then
        return 0
    fi

    echo -e "${BLUE}📊 初始化 CodeGraph...${NC}"

    # 复制 .codegraph/ 配置（如果存在）
    if [[ -d "$DEFAULT_SOURCE/.codegraph" ]]; then
        mkdir -p "$TARGET_DIR/.codegraph"
        # 只复制配置文件，不复制运行时数据
        for item in "$DEFAULT_SOURCE/.codegraph"/*; do
            local basename=$(basename "$item")
            # 跳过运行时文件
            if [[ "$basename" == "daemon.pid" || "$basename" == "*.db" || "$basename" == "*.db-journal" ]]; then
                continue
            fi
            cp -r "$item" "$TARGET_DIR/.codegraph/"
        done
    fi

    echo -e "${GREEN}✅ CodeGraph 配置已复制${NC}"
    echo -e "${YELLOW}💡 提示：在目标项目运行 'codegraph init' 初始化索引${NC}"
}

# 验证初始化结果
verify_init() {
    echo ""
    echo -e "${CYAN}🔍 验证初始化结果...${NC}"
    echo ""

    # 运行 check 脚本
    if [[ -f "$SCRIPT_DIR/check.sh" ]]; then
        bash "$SCRIPT_DIR/check.sh" "$TARGET_DIR"
    fi
}

# 显示使用说明
show_usage_guide() {
    echo ""
    echo -e "${GREEN}✅ AI 环境初始化完成！${NC}"
    echo ""
    echo -e "${YELLOW}已初始化组件：${NC}"

    [[ "$SELECT_AGENT" == true ]] && echo "  - .agent/ (角色定义)"
    [[ "$SELECT_RULES" == true ]] && echo "  - .rules/ (统一规则)"
    [[ "$SELECT_SKILLS" == true ]] && echo "  - .skills/ (Skills + symlink)"
    [[ "$SELECT_HOOKS" == true ]] && echo "  - hooks/ (钩子脚本)"
    [[ "$SELECT_CONFIG" == true ]] && echo "  - 配置文件"

    echo ""
    echo -e "${YELLOW}下一步：${NC}"
    echo "  1. 读取 AGENTS.md 了解多 Agent 框架"
    echo "  2. 读取 .agent/universal.md 了解你的角色"
    echo "  3. 读取 .rules/tools.md 了解工具使用规则"
    echo "  4. 运行 'ai-init-env check' 验证环境状态"
    echo ""
    echo -e "${YELLOW}切换角色：${NC}"
    echo "  ./scripts/sync-skills.sh claude frontend"
    echo "  ./scripts/sync-skills.sh claude backend"
    echo "  ./scripts/sync-skills.sh claude universal"
}

# 主函数
main() {
    # 解析参数
    parse_args "$@"

    # 如果未指定源项目，使用当前 skill 所在项目
    if [[ -z "$DEFAULT_SOURCE" ]]; then
        # 尝试从 skill 位置推断源项目
        # skill 通常在 .skills/shared/ai-init-env/
        # 源项目应该是 .skills/ 的父目录
        DEFAULT_SOURCE="$(dirname "$(dirname "$SKILL_DIR")")"

        # 验证推断的源项目
        if [[ ! -d "$DEFAULT_SOURCE/.agent" ]]; then
            echo -e "${YELLOW}⚠️  无法自动检测源项目${NC}"
            read -rp "请输入源项目路径: " DEFAULT_SOURCE
        fi
    fi

    # 验证源项目
    if ! validate_source "$DEFAULT_SOURCE"; then
        exit 1
    fi

    # 转换为绝对路径
    DEFAULT_SOURCE=$(realpath "$DEFAULT_SOURCE")
    TARGET_DIR=$(realpath "$TARGET_DIR")

    # 如果是交互模式（未使用 -y 参数），显示菜单
    if [[ "$OVERWRITE_ASK" == true ]]; then
        show_interactive_menu
        handle_user_input
    fi

    # 确认初始化
    echo ""
    echo -e "${CYAN}📋 初始化配置：${NC}"
    echo "  源项目: $DEFAULT_SOURCE"
    echo "  目标项目: $TARGET_DIR"
    echo "  环境: $(detect_environment)"
    echo ""
    echo -e "${YELLOW}组件：${NC}"
    [[ "$SELECT_AGENT" == true ]] && echo "  ✅ Agent 定义"
    [[ "$SELECT_RULES" == true ]] && echo "  ✅ Rules"
    [[ "$SELECT_SKILLS" == true ]] && echo "  ✅ Skills"
    [[ "$SELECT_HOOKS" == true ]] && echo "  ✅ Hooks"
    [[ "$SELECT_CONFIG" == true ]] && echo "  ✅ 配置模板"
    [[ "$SELECT_CODEGRAPH" == true ]] && echo "  ✅ CodeGraph"
    echo ""
    echo -e "${YELLOW}CLI：${NC}"
    [[ "$SELECT_CLAUDE" == true ]] && echo "  ✅ Claude Code"
    [[ "$SELECT_OPENCODE" == true ]] && echo "  ✅ OpenCode"
    [[ "$SELECT_QODER" == true ]] && echo "  ✅ QoderCLI"
    [[ "$SELECT_CODEX" == true ]] && echo "  ✅ Codex"
    echo ""

    if [[ "$OVERWRITE_ASK" == true ]]; then
        read -rp "确认初始化？[Y/n] " confirm
        if [[ "$confirm" == [Nn] ]]; then
            echo -e "${YELLOW}⚠️  已取消初始化${NC}"
            exit 0
        fi
    fi

    # 执行初始化
    echo ""
    echo -e "${CYAN}🚀 开始初始化...${NC}"
    echo ""

    init_agent
    init_rules
    init_skills
    init_hooks
    init_config
    init_codegraph

    # 验证结果
    verify_init

    # 显示使用说明
    show_usage_guide
}

# 运行主函数
main "$@"
