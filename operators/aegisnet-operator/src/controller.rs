//! 控制器模块
//!
//! 该模块实现了 AegisNet Operator 的核心控制器，负责协调 reconcile 和 event_handler 的功能，
//! 管理自定义资源的生命周期和状态变化。

use anyhow::{Result, Context};
use futures::StreamExt;
use kube::{
    api::Api,
    client::Client,
    runtime::controller::{Controller as KubeController, Action},
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::crd::ZeroTrustPolicy;
use crate::event_handler::EventHandler;
use crate::reconcile::Reconciler;

/// 控制器结构体
pub struct Controller {
    /// Kubernetes 客户端
    client: Client,
    /// 协调器
    reconciler: Arc<Reconciler>,
    /// 事件处理器
    event_handler: Option<EventHandler>,
    /// 控制器是否正在运行
    running: Arc<RwLock<bool>>,
}

impl Controller {
    /// 创建新的控制器
    pub fn new(client: Client) -> Self {
        let reconciler = Arc::new(Reconciler::new(client.clone()));
        
        Self {
            client,
            reconciler,
            event_handler: None,
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// 启动控制器
    pub async fn start(&mut self) -> Result<()> {
        // 设置运行状态
        {
            let mut running = self.running.write().await;
            *running = true;
        }
        
        // 创建事件处理器
        let event_handler = EventHandler::new(self.client.clone(), self.reconciler.clone());
        
        // 启动事件处理器
        event_handler.start().await?;
        
        // 保存事件处理器实例
        self.event_handler = Some(event_handler);
        
        // 启动 ZeroTrustPolicy 控制器
        self.start_policy_controller().await?;
        
        info!("AegisNet Operator 控制器已启动");
        
        Ok(())
    }
    
    /// 停止控制器
    pub async fn stop(&mut self) -> Result<()> {
        // 设置运行状态为 false
        {
            let mut running = self.running.write().await;
            *running = false;
        }
        
        // 停止事件处理器
        if let Some(event_handler) = &self.event_handler {
            event_handler.stop().await?;
        }
        
        info!("AegisNet Operator 控制器已停止");
        
        Ok(())
    }
    
    /// 启动 ZeroTrustPolicy 控制器
    async fn start_policy_controller(&self) -> Result<()> {
        let client = self.client.clone();
        let reconciler = self.reconciler.clone();
        let running = self.running.clone();
        
        tokio::spawn(async move {
            // 创建 ZeroTrustPolicy API
            let policies: Api<ZeroTrustPolicy> = Api::all(client.clone());
            
            // 创建控制器
            let controller = KubeController::new(policies, Default::default())
                .run(
                    move |policy, _| {
                        let reconciler = reconciler.clone();
                        async move {
                            reconciler.reconcile(policy).await
                        }
                    },
                    move |policy, error, _| {
                        let reconciler = reconciler.clone();
                        async move {
                            reconciler.handle_error(policy, &error)
                        }
                    },
                    running.clone(),
                )
                .for_each(|result| async {
                    match result {
                        Ok(o) => debug!("协调成功: {:?}", o),
                        Err(e) => error!("协调错误: {}", e),
                    }
                });
            
            info!("启动 ZeroTrustPolicy 控制器");
            controller.await;
        });
        
        Ok(())
    }
    
    /// 获取协调器
    pub fn get_reconciler(&self) -> Arc<Reconciler> {
        self.reconciler.clone()
    }
}

/// 创建默认的控制器
pub fn create_default_controller(client: Client) -> Controller {
    Controller::new(client)
}