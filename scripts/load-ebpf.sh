#!/bin/bash

# load-ebpf.sh - 自动检测内核版本并加载对应的 eBPF 字节码
# 作者: AegisNet Team

set -e

# 日志函数
log_info() {
    echo -e "\033[0;32m[INFO]\033[0m $1"
}

log_warn() {
    echo -e "\033[0;33m[WARN]\033[0m $1"
}

log_error() {
    echo -e "\033[0;31m[ERROR]\033[0m $1"
    exit 1
}

# 检查是否以 root 权限运行
if [ "$(id -u)" -ne 0 ]; then
    log_error "此脚本需要 root 权限运行，请使用 sudo"
fi

# 默认 eBPF 字节码路径
DEFAULT_BPF_PATH="/opt/aegisnet/bpf"
BPF_PATH=${BPF_PATH:-$DEFAULT_BPF_PATH}

# 检查 eBPF 字节码目录是否存在
if [ ! -d "$BPF_PATH" ]; then
    log_error "eBPF 字节码目录不存在: $BPF_PATH"
fi

# 获取内核版本
KERNEL_VERSION=$(uname -r)
KERNEL_MAJOR=$(echo $KERNEL_VERSION | cut -d. -f1)
KERNEL_MINOR=$(echo $KERNEL_VERSION | cut -d. -f2)

log_info "检测到内核版本: $KERNEL_VERSION (主版本: $KERNEL_MAJOR, 次版本: $KERNEL_MINOR)"

# 检查内核版本兼容性
if [ $KERNEL_MAJOR -lt 5 ] || ([ $KERNEL_MAJOR -eq 5 ] && [ $KERNEL_MINOR -lt 4 ]); then
    log_warn "内核版本低于 5.4，部分 eBPF 功能可能不可用"
    COMPAT_MODE="legacy"
elif [ $KERNEL_MAJOR -lt 5 ] || ([ $KERNEL_MAJOR -eq 5 ] && [ $KERNEL_MINOR -lt 10 ]); then
    log_info "内核版本 5.4-5.10，使用标准兼容模式"
    COMPAT_MODE="standard"
else
    log_info "内核版本 >= 5.10，使用完整功能模式"
    COMPAT_MODE="full"
fi

# 根据兼容模式选择对应的 eBPF 字节码
case $COMPAT_MODE in
    "legacy")
        BPF_BYTECODE="$BPF_PATH/aegisnet_legacy.o"
        ;;
    "standard")
        BPF_BYTECODE="$BPF_PATH/aegisnet_standard.o"
        ;;
    "full")
        BPF_BYTECODE="$BPF_PATH/aegisnet.o"
        ;;
    *)
        log_error "未知的兼容模式: $COMPAT_MODE"
        ;;
esac

# 检查选择的字节码文件是否存在
if [ ! -f "$BPF_BYTECODE" ]; then
    log_error "eBPF 字节码文件不存在: $BPF_BYTECODE"
fi

log_info "使用 eBPF 字节码: $BPF_BYTECODE"

# 检查是否已加载 eBPF 程序
if bpftool prog show | grep -q "aegisnet"; then
    log_warn "检测到已加载的 AegisNet eBPF 程序，正在卸载..."
    # 这里可以添加卸载逻辑，但通常重新加载会自动替换
fi

# 加载 eBPF 程序
log_info "正在加载 eBPF 程序..."

# 检查 aegisnet-agent 是否已安装
if command -v aegisnet-agent &> /dev/null; then
    log_info "使用 aegisnet-agent 加载 eBPF 程序"
    
    # 创建临时配置文件
    TMP_CONFIG=$(mktemp)
    cat > $TMP_CONFIG << EOF
bpf_path: "$BPF_BYTECODE"
interfaces: ["eth0"]
log_level: "info"
metrics:
  listen_address: "0.0.0.0"
  port: 9090
  interval_seconds: 15
health_check:
  interval_seconds: 30
  endpoint: "/health"
EOF
    
    # 使用 aegisnet-agent 加载
    aegisnet-agent --config $TMP_CONFIG --load-only
    
    # 清理临时文件
    rm $TMP_CONFIG
else
    log_warn "未找到 aegisnet-agent，尝试使用 bpftool 直接加载"
    
    # 使用 bpftool 直接加载
    # 注意：这种方式只加载程序，不会设置 maps 和挂载点
    if command -v bpftool &> /dev/null; then
        bpftool prog load "$BPF_BYTECODE" /sys/fs/bpf/aegisnet
        log_info "eBPF 程序已加载到 /sys/fs/bpf/aegisnet"
        log_warn "注意：使用 bpftool 直接加载不会配置 maps 和挂载点，建议安装 aegisnet-agent"
    else
        log_error "未找到 bpftool，无法加载 eBPF 程序"
    fi
fi

log_info "eBPF 程序加载完成"

# 验证加载状态
if bpftool prog show | grep -q "aegisnet"; then
    log_info "验证成功：AegisNet eBPF 程序已加载"
else
    log_error "验证失败：AegisNet eBPF 程序未成功加载"
fi

log_info "完成！AegisNet eBPF 程序已成功加载"