//! AegisNet 控制平面
//!
//! 该模块实现 AegisNet 的控制平面功能，负责策略管理、身份认证和多集群同步。
//! 使用 kube-rs 框架与 Kubernetes API 交互。

pub mod identity;
pub mod policy;
pub mod sync;
pub mod ai;
pub mod utils;

use anyhow::Result;
use tracing::info;
use std::sync::Arc;
use prometheus::Registry;

/// 控制器初始化函数
pub async fn init() -> Result<()> {
    info!("初始化 AegisNet 控制平面");
    
    // 创建Prometheus注册表
    let registry = Registry::new();
    
    // 初始化身份管理系统
    let identity_client = identity::init_identity_system().await?;
    info!("身份管理系统初始化完成");
    
    // 初始化策略管理系统
    let policy_watcher = policy::init_policy_system().await?;
    let policy_generator = Arc::new(policy::create_default_generator());
    info!("策略管理系统初始化完成");
    
    // 初始化集群同步系统
    let sync_manager = sync::init_sync_system().await?;
    info!("集群同步系统初始化完成");
    
    // 初始化 AI 系统
    let ai_model = ai::init_ai_system().await?;
    info!("AI 系统初始化完成");
    
    // 初始化策略优化系统
    let policy_optimizer = ai::init_policy_optimization_system(
        ai_model,
        policy_generator,
        &registry
    ).await?;
    info!("策略优化系统初始化完成");
    
    info!("AegisNet 控制平面初始化完成，策略优化闭环已启动");
    Ok(())
}