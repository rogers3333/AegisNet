//! 策略管理模块
//!
//! 该模块负责管理 AegisNet 的安全策略，包括策略的监听、生成和分发。
//! 使用 Kubernetes CRD 机制存储和管理策略。

mod crd_watcher;
mod policy_generator;

pub use crd_watcher::{ZeroTrustPolicy, ZeroTrustPolicySpec, ZeroTrustPolicyStatus, PolicyAction, PolicyRule, ServiceSelector, CrdWatcher, create_default_watcher};
pub use policy_generator::{PolicyGenerator, create_default_generator};

use anyhow::Result;
use std::sync::Arc;
use tracing::info;

/// 初始化策略管理系统
pub async fn init_policy_system() -> Result<Arc<CrdWatcher>> {
    info!("初始化策略管理系统");
    
    // 创建 CRD 监听器
    let watcher = Arc::new(create_default_watcher().await?);
    
    // 启动监听任务
    watcher.start().await?;
    
    // 创建策略生成器
    let generator = create_default_generator();
    
    info!("策略管理系统初始化完成");
    
    Ok(watcher)
}