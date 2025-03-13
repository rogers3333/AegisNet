//! AegisNet Agent - 用户态守护进程管理 eBPF 程序
//!
//! 该模块实现了 AegisNet 的用户态代理，负责管理 eBPF 程序的生命周期、
//! 收集指标、处理配置和提供健康检查功能。

pub mod ebpf_loader;
pub mod metrics;
pub mod config;
pub mod health_check;

use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Agent 主结构体
pub struct Agent {
    /// eBPF 加载器
    ebpf_loader: Arc<RwLock<ebpf_loader::EbpfLoader>>,
    /// 指标收集器
    metrics: Arc<metrics::MetricsCollector>,
    /// 配置管理器
    config: Arc<RwLock<config::ConfigManager>>,
    /// 健康检查器
    health_checker: Arc<health_check::HealthChecker>,
}

impl Agent {
    /// 创建新的 Agent 实例
    pub async fn new(config_path: &str) -> Result<Self> {
        // 加载配置
        let config = Arc::new(RwLock::new(config::ConfigManager::new(config_path)?));
        
        // 创建 eBPF 加载器
        let ebpf_loader = Arc::new(RwLock::new(ebpf_loader::EbpfLoader::new(
            config.clone(),
        )?));
        
        // 创建指标收集器
        let metrics = Arc::new(metrics::MetricsCollector::new(
            ebpf_loader.clone(),
        ));
        
        // 创建健康检查器
        let health_checker = Arc::new(health_check::HealthChecker::new(
            ebpf_loader.clone(),
            config.clone(),
        ));
        
        Ok(Self {
            ebpf_loader,
            metrics,
            config,
            health_checker,
        })
    }
    
    /// 启动 Agent
    pub async fn start(&self) -> Result<()> {
        // 加载 eBPF 程序
        self.ebpf_loader.write().await.load_programs().await?;
        
        // 启动配置热重载监听
        let config = self.config.clone();
        let ebpf_loader = self.ebpf_loader.clone();
        tokio::spawn(async move {
            config.write().await.start_config_watcher(ebpf_loader).await
                .expect("配置监听器启动失败");
        });
        
        // 启动指标收集
        self.metrics.start_metrics_server().await?;
        
        // 启动健康检查
        self.health_checker.start_health_check().await?;
        
        Ok(())
    }
    
    /// 停止 Agent
    pub async fn stop(&self) -> Result<()> {
        // 卸载 eBPF 程序
        self.ebpf_loader.write().await.unload_programs().await?;
        
        Ok(())
    }
}