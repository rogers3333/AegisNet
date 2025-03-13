//! 事件处理模块
//!
//! 该模块负责监听 Kubernetes 集群事件，如 Pod 创建、删除等，
//! 并在事件发生时触发相应的策略更新或资源清理操作。

use anyhow::{Result, Context};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::{Pod, Namespace};
use kube::{
    api::{Api, ListParams, WatchEvent},
    client::Client,
    runtime::watcher,
    Resource, ResourceExt,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, error, info, warn};

use crate::crd::ZeroTrustPolicy;
use crate::reconcile::Reconciler;

/// 事件类型
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    /// 资源添加
    Added,
    /// 资源修改
    Modified,
    /// 资源删除
    Deleted,
    /// 资源重新列举
    Restarted,
    /// 错误
    Error,
}

/// 事件处理器结构体
pub struct EventHandler {
    /// Kubernetes 客户端
    client: Client,
    /// 协调器
    reconciler: Arc<Reconciler>,
    /// 事件通道发送端
    event_tx: mpsc::Sender<Event>,
    /// 事件通道接收端
    event_rx: Arc<RwLock<mpsc::Receiver<Event>>>,
    /// 是否正在运行
    running: Arc<RwLock<bool>>,
}

/// 事件结构体
#[derive(Debug, Clone)]
pub struct Event {
    /// 事件类型
    pub event_type: EventType,
    /// 资源类型
    pub resource_type: String,
    /// 资源名称
    pub resource_name: String,
    /// 资源命名空间
    pub namespace: Option<String>,
}

impl EventHandler {
    /// 创建新的事件处理器
    pub fn new(client: Client, reconciler: Arc<Reconciler>) -> Self {
        let (event_tx, event_rx) = mpsc::channel(100); // 缓冲区大小为 100
        
        Self {
            client,
            reconciler,
            event_tx,
            event_rx: Arc::new(RwLock::new(event_rx)),
            running: Arc::new(RwLock::new(false)),
        }
    }
    
    /// 启动事件处理器
    pub async fn start(&self) -> Result<()> {
        // 设置运行状态
        {
            let mut running = self.running.write().await;
            *running = true;
        }
        
        // 启动 Pod 监听器
        self.start_pod_watcher().await?;
        
        // 启动命名空间监听器
        self.start_namespace_watcher().await?;
        
        // 启动策略监听器
        self.start_policy_watcher().await?;
        
        // 启动事件处理循环
        self.start_event_processor().await?;
        
        Ok(())
    }
    
    /// 停止事件处理器
    pub async fn stop(&self) -> Result<()> {
        // 设置运行状态为 false
        {
            let mut running = self.running.write().await;
            *running = false;
        }
        
        Ok(())
    }
    
    /// 启动 Pod 监听器
    async fn start_pod_watcher(&self) -> Result<()> {
        let client = self.client.clone();
        let event_tx = self.event_tx.clone();
        let running = self.running.clone();
        
        tokio::spawn(async move {
            let api: Api<Pod> = Api::all(client);
            let watcher = watcher(api, ListParams::default().timeout(60));
            
            info!("启动 Pod 监听器");
            
            watcher
                .try_for_each(|event| async {
                    // 检查是否仍在运行
                    if !*running.read().await {
                        return Ok(());
                    }
                    
                    match event {
                        WatchEvent::Added(pod) => {
                            let event = Event {
                                event_type: EventType::Added,
                                resource_type: "Pod".to_string(),
                                resource_name: pod.name_any(),
                                namespace: pod.namespace(),
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送 Pod 添加事件失败: {}", e);
                            }
                        },
                        WatchEvent::Modified(pod) => {
                            let event = Event {
                                event_type: EventType::Modified,
                                resource_type: "Pod".to_string(),
                                resource_name: pod.name_any(),
                                namespace: pod.namespace(),
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送 Pod 修改事件失败: {}", e);
                            }
                        },
                        WatchEvent::Deleted(pod) => {
                            let event = Event {
                                event_type: EventType::Deleted,
                                resource_type: "Pod".to_string(),
                                resource_name: pod.name_any(),
                                namespace: pod.namespace(),
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送 Pod 删除事件失败: {}", e);
                            }
                        },
                        WatchEvent::Bookmark(_) => {
                            // 忽略书签事件
                        },
                        WatchEvent::Error(e) => {
                            error!("Pod 监听器错误: {}", e);
                            
                            let event = Event {
                                event_type: EventType::Error,
                                resource_type: "Pod".to_string(),
                                resource_name: "error".to_string(),
                                namespace: None,
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送 Pod 错误事件失败: {}", e);
                            }
                        },
                    }
                    
                    Ok(())
                })
                .await
                .unwrap_or_else(|e| error!("Pod 监听器错误: {}", e));
            
            // 如果监听器退出，尝试重新启动
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // 检查是否仍在运行
            if *running.read().await {
                info!("重新启动 Pod 监听器");
                let event = Event {
                    event_type: EventType::Restarted,
                    resource_type: "Pod".to_string(),
                    resource_name: "watcher".to_string(),
                    namespace: None,
                };
                
                if let Err(e) = event_tx.send(event).await {
                    error!("发送 Pod 监听器重启事件失败: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    /// 启动命名空间监听器
    async fn start_namespace_watcher(&self) -> Result<()> {
        let client = self.client.clone();
        let event_tx = self.event_tx.clone();
        let running = self.running.clone();
        
        tokio::spawn(async move {
            let api: Api<Namespace> = Api::all(client);
            let watcher = watcher(api, ListParams::default().timeout(60));
            
            info!("启动命名空间监听器");
            
            watcher
                .try_for_each(|event| async {
                    // 检查是否仍在运行
                    if !*running.read().await {
                        return Ok(());
                    }
                    
                    match event {
                        WatchEvent::Added(ns) => {
                            let event = Event {
                                event_type: EventType::Added,
                                resource_type: "Namespace".to_string(),
                                resource_name: ns.name_any(),
                                namespace: None,
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送命名空间添加事件失败: {}", e);
                            }
                        },
                        WatchEvent::Modified(ns) => {
                            let event = Event {
                                event_type: EventType::Modified,
                                resource_type: "Namespace".to_string(),
                                resource_name: ns.name_any(),
                                namespace: None,
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送命名空间修改事件失败: {}", e);
                            }
                        },
                        WatchEvent::Deleted(ns) => {
                            let event = Event {
                                event_type: EventType::Deleted,
                                resource_type: "Namespace".to_string(),
                                resource_name: ns.name_any(),
                                namespace: None,
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送命名空间删除事件失败: {}", e);
                            }
                        },
                        WatchEvent::Bookmark(_) => {
                            // 忽略书签事件
                        },
                        WatchEvent::Error(e) => {
                            error!("命名空间监听器错误: {}", e);
                            
                            let event = Event {
                                event_type: EventType::Error,
                                resource_type: "Namespace".to_string(),
                                resource_name: "error".to_string(),
                                namespace: None,
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送命名空间错误事件失败: {}", e);
                            }
                        },
                    }
                    
                    Ok(())
                })
                .await
                .unwrap_or_else(|e| error!("命名空间监听器错误: {}", e));
            
            // 如果监听器退出，尝试重新启动
            tokio::time::sleep(Duration::from_secs(5)).await;
            
            // 检查是否仍在运行
            if *running.read().await {
                info!("重新启动命名空间监听器");
                let event = Event {
                    event_type: EventType::Restarted,
                    resource_type: "Namespace".to_string(),
                    resource_name: "watcher".to_string(),
                    namespace: None,
                };
                
                if let Err(e) = event_tx.send(event).await {
                    error!("发送命名空间监听器重启事件失败: {}", e);
                }
            }
        });
        
        Ok(())
    }
    
    /// 启动策略监听器
    async fn start_policy_watcher(&self) -> Result<()> {
        let client = self.client.clone();
        let event_tx = self.event_tx.clone();
        let running = self.running.clone();
        
        tokio::spawn(async move {
            let api: Api<ZeroTrustPolicy> = Api::all(client);
            let watcher = watcher(api, ListParams::default().timeout(60));
            
            info!("启动策略监听器");
            
            watcher
                .try_for_each(|event| async {
                    // 检查是否仍在运行
                    if !*running.read().await {
                        return Ok(());
                    }
                    
                    match event {
                        WatchEvent::Added(policy) => {
                            let event = Event {
                                event_type: EventType::Added,
                                resource_type: "ZeroTrustPolicy".to_string(),
                                resource_name: policy.name_any(),
                                namespace: policy.namespace(),
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送策略添加事件失败: {}", e);
                            }
                        },
                        WatchEvent::Modified(policy) => {
                            let event = Event {
                                event_type: EventType::Modified,
                                resource_type: "ZeroTrustPolicy".to_string(),
                                resource_name: policy.name_any(),
                                namespace: policy.namespace(),
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送策略修改事件失败: {}", e);
                            }
                        },
                        WatchEvent::Deleted(policy) => {
                            let event = Event {
                                event_type: EventType::Deleted,
                                resource_type: "ZeroTrustPolicy".to_string(),
                                resource_name: policy.name_any(),
                                namespace: policy.namespace(),
                            };
                            
                            if let Err(e) = event_tx.send(event).await {
                                error!("发送策