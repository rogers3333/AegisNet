#!/bin/bash

# setup-kind-clusters.sh - 创建本地 Kubernetes 集群并预配置测试资源
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

# 检查依赖
check_dependencies() {
    log_info "检查依赖..."
    
    # 检查 kind 是否安装
    if ! command -v kind &> /dev/null; then
        log_error "未找到 kind 命令，请先安装 kind: https://kind.sigs.k8s.io/docs/user/quick-start/"
    fi
    
    # 检查 kubectl 是否安装
    if ! command -v kubectl &> /dev/null; then
        log_error "未找到 kubectl 命令，请先安装 kubectl: https://kubernetes.io/docs/tasks/tools/"
    fi
    
    # 检查 docker 是否安装并运行
    if ! command -v docker &> /dev/null; then
        log_error "未找到 docker 命令，请先安装 docker: https://docs.docker.com/get-docker/"
    fi
    
    if ! docker info &> /dev/null; then
        log_error "Docker 服务未运行，请启动 Docker 服务"
    fi
    
    log_info "所有依赖检查通过"
}

# 创建 Kind 集群配置文件
create_kind_config() {
    local config_file="$1"
    
    cat > "$config_file" << EOF
kind: Cluster
apiVersion: kind.x-k8s.io/v1alpha4
nodes:
- role: control-plane
  kubeadmConfigPatches:
  - |
    kind: InitConfiguration
    nodeRegistration:
      kubeletExtraArgs:
        node-labels: "ingress-ready=true"
  extraPortMappings:
  - containerPort: 80
    hostPort: 80
    protocol: TCP
  - containerPort: 443
    hostPort: 443
    protocol: TCP
- role: worker
- role: worker
networking:
  podSubnet: "10.244.0.0/16"
  serviceSubnet: "10.96.0.0/16"
EOF
    
    log_info "Kind 集群配置文件已创建: $config_file"
}

# 创建 Kind 集群
create_kind_cluster() {
    local cluster_name="$1"
    local config_file="$2"
    
    log_info "创建 Kind 集群: $cluster_name"
    
    # 检查集群是否已存在
    if kind get clusters | grep -q "^$cluster_name$"; then
        log_warn "集群 '$cluster_name' 已存在，是否删除并重新创建? [y/N]"
        read -r response
        if [[ "$response" =~ ^([yY][eE][sS]|[yY])$ ]]; then
            log_info "删除现有集群: $cluster_name"
            kind delete cluster --name "$cluster_name"
        else
            log_info "保留现有集群: $cluster_name"
            return 0
        fi
    fi
    
    # 创建集群
    kind create cluster --name "$cluster_name" --config "$config_file"
    
    # 等待集群就绪
    log_info "等待集群就绪..."
    kubectl wait --for=condition=Ready nodes --all --timeout=300s
    
    log_info "Kind 集群 '$cluster_name' 创建成功"
}

# 安装 Nginx Ingress Controller
install_nginx_ingress() {
    log_info "安装 Nginx Ingress Controller"
    
    kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/main/deploy/static/provider/kind/deploy.yaml
    
    # 等待 Ingress 控制器就绪
    log_info "等待 Ingress 控制器就绪..."
    kubectl wait --namespace ingress-nginx \
      --for=condition=ready pod \
      --selector=app.kubernetes.io/component=controller \
      --timeout=300s
      
    log_info "Nginx Ingress Controller 安装完成"
}

# 创建测试命名空间和资源
create_test_resources() {
    log_info "创建测试命名空间和资源"
    
    # 创建测试命名空间
    kubectl create namespace aegisnet-test
    kubectl create namespace aegisnet-system
    
    # 创建 ConfigMap 用于 AegisNet 配置
    cat > /tmp/aegisnet-config.yaml << EOF
apiVersion: v1
kind: ConfigMap
metadata:
  name: aegisnet-config
  namespace: aegisnet-system
data:
  config.yaml: |
    bpf_path: "/opt/aegisnet/bpf/aegisnet.o"
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
    
    kubectl apply -f /tmp/aegisnet-config.yaml
    
    # 创建测试 Pod 和 Service
    cat > /tmp/test-deployment.yaml << EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: nginx-test
  namespace: aegisnet-test
spec:
  replicas: 2
  selector:
    matchLabels:
      app: nginx-test
  template:
    metadata:
      labels:
        app: nginx-test
    spec:
      containers:
      - name: nginx
        image: nginx:latest
        ports:
        - containerPort: 80
---
apiVersion: v1
kind: Service
metadata:
  name: nginx-test
  namespace: aegisnet-test
spec:
  selector:
    app: nginx-test
  ports:
  - port: 80
    targetPort: 80
  type: ClusterIP
EOF
    
    kubectl apply -f /tmp/test-deployment.yaml
    
    # 清理临时文件
    rm /tmp/aegisnet-config.yaml /tmp/test-deployment.yaml
    
    log_info "测试资源创建完成"
}

# 主函数
main() {
    local cluster_name="aegisnet-dev"
    local config_file="/tmp/kind-config.yaml"
    
    # 解析命令行参数
    while [[ $# -gt 0 ]]; do
        case $1 in
            --name)
                cluster_name="$2"
                shift 2
                ;;
            --help)
                echo "用法: $0 [--name CLUSTER_NAME]"
                echo "创建本地 Kubernetes 集群并预配置测试资源"
                echo ""
                echo "选项:"
                echo "  --name CLUSTER_NAME  指定集群名称 (默认: aegisnet-dev)"
                echo "  --help              显示此帮助信息"
                exit 0
                ;;
            *)
                log_error "未知参数: $1"
                ;;
        esac
    done
    
    log_info "开始设置 AegisNet 开发环境"
    
    # 检查依赖
    check_dependencies
    
    # 创建配置文件
    create_kind_config "$config_file"
    
    # 创建集群
    create_kind_cluster "$cluster_name" "$config_file"
    
    # 安装 Nginx Ingress
    install_nginx_ingress
    
    # 创建测试资源
    create_test_resources
    
    # 清理临时文件
    rm -f "$config_file"
    
    log_info "AegisNet 开发环境设置完成！"
    log_info "集群名称: $cluster_name"
    log_info "使用以下命令访问集群:"
    log_info "  kubectl cluster-info --context kind-$cluster_name"
    log_info "测试命名空间: aegisnet-test, aegisnet-system"
}

# 执行主函数
main "$@"