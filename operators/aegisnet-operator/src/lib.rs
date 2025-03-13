//! AegisNet Operator - Kubernetes Operator 自动化管理 AegisNet 资源
//!
//! 该模块实现了 AegisNet 的 Kubernetes Operator，负责自动化管理 AegisNet 的自定义资源，
//! 包括 ZeroTrustPolicy、NetworkIdentity 等，确保实际状态与期望状态一致。

pub mod reconcile;
pub mod event_handler;
pub mod crd;
pub mod controller;

use anyhow::Result;
use kube::Client;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Operator 主结构体
pub struct AegisNetOperator {
    /// Kubernetes 客户端
    client: Client,
    /// 控制器
    controller: Arc<RwLock<controller::Controller>>,
}

impl AegisNetOperator {
    /// 创建新的 Operator 实例
    pub async fn new() -> Result<Self> {
        // 创建 Kubernetes 客户端
        let client = Client::try_default().await?;
        
        // 创建控制器
        let controller = Arc::new(RwLock::new(controller::Controller::new(
            client.clone(),
        )));
        
        Ok(Self {
            client,
            controller,
        })
    }
    
    /// 启动 Operator
    pub async fn start(&self) -> Result<()> {
        // 启动控制器
        self.controller.write().await.start().await?;
        
        Ok(())
    }
    
    /// 停止 Operator
    pub async fn stop(&self) -> Result<()> {
        // 停止控制器
        self.controller.write().await.stop().await?;
        
        Ok(())
    }
}