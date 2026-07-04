#!/bin/bash
# 环境检测工具函数
# 用于检测当前运行环境（WSL/Windows/Linux/Mac）

# 检测当前环境
# 返回值: wsl, windows-msys, windows-ps, unix
detect_environment() {
    # WSL 检测
    if uname -r 2>/dev/null | grep -qi microsoft; then
        echo "wsl"
        return 0
    fi

    # Windows Git Bash/MSYS 检测
    if uname -o 2>/dev/null | grep -qi msys; then
        echo "windows-msys"
        return 0
    fi

    # Windows Cygwin 检测
    if uname -o 2>/dev/null | grep -qi cygwin; then
        echo "windows-cygwin"
        return 0
    fi

    # Windows PowerShell (通过环境变量)
    if [[ "$OS" == "Windows_NT" ]]; then
        echo "windows-ps"
        return 0
    fi

    # macOS 检测
    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "macos"
        return 0
    fi

    # Linux
    if [[ "$OSTYPE" == "linux-gnu"* ]]; then
        echo "linux"
        return 0
    fi

    # 默认返回 unix
    echo "unix"
    return 0
}

# 创建 symlink（自动适配环境）
# 参数: $1=目标路径, $2=链接路径
create_symlink() {
    local target="$1"
    local link="$2"
    local env=$(detect_environment)

    # 验证目标存在
    if [[ ! -e "$target" ]]; then
        echo "❌ 错误：目标路径不存在: $target" >&2
        return 1
    fi

    # 验证链接路径的父目录存在
    local link_dir=$(dirname "$link")
    if [[ ! -d "$link_dir" ]]; then
        echo "❌ 错误：链接路径的父目录不存在: $link_dir" >&2
        return 1
    fi

    # 如果链接已存在，先删除
    if [[ -e "$link" || -L "$link" ]]; then
        rm -rf "$link"
    fi

    case "$env" in
        wsl|linux|macos|unix)
            ln -s "$target" "$link"
            ;;
        windows-msys|windows-cygwin)
            # Git Bash/Cygwin: 使用 cmd.exe mklink
            # 需要转换路径为 Windows 格式
            local win_target=$(wslpath -w "$target" 2>/dev/null || cygpath -w "$target" 2>/dev/null)
            local win_link=$(wslpath -w "$link" 2>/dev/null || cygpath -w "$link" 2>/dev/null)

            if [[ -z "$win_target" || -z "$win_link" ]]; then
                echo "❌ 错误：无法转换路径为 Windows 格式" >&2
                return 1
            fi

            # 使用 mklink /J 创建目录 junction（比 symlink 更兼容）
            if [[ -d "$target" ]]; then
                cmd.exe /c "mklink /J \"$win_link\" \"$win_target\"" 2>/dev/null
            else
                # 文件 symlink
                cmd.exe /c "mklink \"$win_link\" \"$win_target\"" 2>/dev/null
            fi
            ;;
        windows-ps)
            # PowerShell
            if [[ -d "$target" ]]; then
                powershell.exe -Command "New-Item -ItemType Junction -Path '$link' -Target '$target'" 2>/dev/null
            else
                powershell.exe -Command "New-Item -ItemType SymbolicLink -Path '$link' -Target '$target'" 2>/dev/null
            fi
            ;;
        *)
            # 默认使用 ln -s
            ln -s "$target" "$link"
            ;;
    esac

    # 验证 symlink 创建成功
    if [[ -L "$link" || -d "$link" ]]; then
        return 0
    else
        echo "❌ 错误：symlink 创建失败" >&2
        return 1
    fi
}

# 路径转换：WSL → Windows
# 参数: $1=WSL 路径
wsl_to_windows_path() {
    local wsl_path="$1"
    wslpath -w "$wsl_path" 2>/dev/null || echo "$wsl_path"
}

# 路径转换：Windows → WSL
# 参数: $1=Windows 路径
windows_to_wsl_path() {
    local win_path="$1"
    wslpath -u "$win_path" 2>/dev/null || echo "$win_path"
}

# 检查是否有管理员权限（Windows）
# 返回: 0=有权限, 1=无权限
check_admin_privileges() {
    local env=$(detect_environment)

    case "$env" in
        windows-msys|windows-cygwin|windows-ps)
            # Windows: 检查是否以管理员身份运行
            net session >/dev/null 2>&1
            return $?
            ;;
        *)
            # Unix: 检查是否为 root
            [[ "$EUID" -eq 0 ]]
            return $?
            ;;
    esac
}

# 获取环境信息摘要
get_environment_info() {
    local env=$(detect_environment)

    echo "环境: $env"
    echo "操作系统: $(uname -s 2>/dev/null)"
    echo "内核版本: $(uname -r 2>/dev/null)"
    echo "架构: $(uname -m 2>/dev/null)"

    if [[ "$env" == "wsl" ]]; then
        echo "WSL 版本: $(cat /proc/version 2>/dev/null | head -1)"
    fi

    if check_admin_privileges; then
        echo "权限: 管理员/root"
    else
        echo "权限: 普通用户"
    fi
}

# 如果直接运行此脚本，显示环境信息
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "=== 环境检测 ==="
    get_environment_info
    echo ""
    echo "=== Symlink 测试 ==="
    echo "create_symlink 函数已加载，可用于创建 symlink"
fi
