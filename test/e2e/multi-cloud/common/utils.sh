#!/bin/bash

# AegisNet 多云测试工具函数

# 设置颜色输出
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m' # No Color

# 日志函数
log_info() {
  echo -e "${GREEN}[INFO]${NC} $1"
}

log_warn() {
  echo -e "${YELLOW}[WARN]${NC} $1"
}

log_error() {
  echo -e "${RED}[ERROR]${NC} $1"
}

# 检查必要工具是否安装
check_prerequisites() {
  local missing_tools=0
  
  # 检查 kubectl
  if ! command -v kubectl &> /dev/null; then
    log_error "kubectl 未安装"
    missing_tools=1
  fi
  
  # 检查 aws cli
  if ! command -v aws &> /dev/null; then
    log_error "AWS CLI 未安装"
    missing_tools=1
  fi
  
  # 检查 gcloud cli
  if ! command -v gcloud &> /dev/null; then
    log_error "Google Cloud SDK 未安装"
    missing_tools=1
  fi
  
  # 检查 jq
  if ! command -v jq &> /dev/null; then
    log_error "jq 未安装"
    missing_tools=1
  fi
  
  if [ $missing_tools -ne 0 ]; then
    log_error "缺少必要工具，请安装后重试"
    exit 1
  fi
  
  log_info "所有必要工具已安装"
}

# 切换 Kubernetes 上下文
switch_context() {
  local context=$1
  log_info "切换到 Kubernetes 上下文: $context"
  kubectl config use-context "$context"
  if [ $? -ne 0 ]; then
    log_error "切换上下文失败"
    exit 1
  fi
}

# 等待资源就绪
wait_for_resource() {
  local resource_type=$1
  local resource_name=$2
  local namespace=$3
  local timeout=${4:-300} # 默认超时时间为300秒
  
  log_info "等待 $resource_type/$resource_name 就绪..."
  
  local start_time=$(date +%s)
  local end_time=$((start_time + timeout))
  
  while [ $(date +%s) -lt $end_time ]; do
    if kubectl get $resource_type $resource_name -n $namespace &> /dev/null; then
      local status=$(kubectl get $resource_type $resource_name -n $namespace -o jsonpath='{.status.phase}' 2>/dev/null)
      if [ "$status" == "Running" ] || [ "$status" == "Active" ] || [ "$status" == "Ready" ]; then
        log_info "$resource_type/$resource_name 已就绪"
        return 0
      fi
    fi
    sleep 5
  done
  
  log_error "等待 $resource_type/$resource_name 超时"
  return 1
}

# 测量延迟
measure_latency() {
  local start_time=$1
  local end_time=$(date +%s.%N)
  local latency=$(echo "$end_time - $start_time" | bc)
  echo $latency
}

# 收集指标
collect_metrics() {
  local namespace=$1
  local pod_selector=$2
  local output_file=$3
  
  log_info "收集 $namespace 命名空间中 $pod_selector 的指标"
  
  # 获取 Pod 列表
  local pods=$(kubectl get pods -n $namespace -l $pod_selector -o jsonpath='{.items[*].metadata.name}')
  
  # 创建输出目录
  mkdir -p $(dirname $output_file)
  
  # 收集 CPU 和内存指标
  for pod in $pods; do
    kubectl top pod $pod -n $namespace >> $output_file
  done
  
  log_info "指标已保存到 $output_file"
}

# 主函数
main() {
  log_info "AegisNet 多云测试工具已加载"
  check_prerequisites
}

# 如果直接执行此脚本，则运行主函数
if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
  main "$@"
fi