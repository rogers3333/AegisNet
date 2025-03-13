//! 集群同步模块
//!
//! 该模块负责多集群环境下的策略和身份同步，确保跨集群的一致性。
//! 包含集群同步核心逻辑和同步状态监控。

mod cluster_sync;
mod sync_monitor;

pub use cluster_sync::{ClusterSync, ClusterSyncConfig, SyncStatus, create_default_sync};
pub use sync_monitor::{SyncMonitor, create_default_monitor};

use anyhow::Result;
use std::sync::Arc;
use tracing::info;

/// 初始化同步系统
pub async fn init_sync_system() -> Result<Arc<ClusterSync>> {
    info!("初始化集群同步系统");
    
    // 创建集群同步管理器
    let sync = Arc::new(create_default_sync().await?);
    
    // 创建同步监控器
    let monitor = create_default_monitor(sync.clone());
    
    // 启动同步任务
    sync.start_sync_task().await?;
    
    // 启动监控任务
    monitor.start_monitoring().await?;
    
    info!("集群同步系统初始化完成");
    
    Ok(sync)
}