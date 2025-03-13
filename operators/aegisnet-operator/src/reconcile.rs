//! 资源协调模块
//!
//! 该模块负责协调 AegisNet 自定义资源的状态，确保实际状态与期望状态一致。
//! 实现了 Kubernetes Operator 模式中的核心协调逻辑。

use anyhow::{Result, Context};
use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams, Patch, PatchParams},
    client::Client,
    runtime::controller::{Action, Controller},
    Resource, ResourceExt,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

use crate::crd::{ZeroTrustPolicy, ZeroTrustPolicyStatus};

/// 协调器结构体
pub struct Reconciler {
    /// Kubernetes 客户端
    client: Client,
    /// 协调状态
    state: Arc<RwLock<ReconcilerState>>,
}

/// 协调器状态
#[derive(Default, Debug)]
pub struct ReconcilerState {
    /// 已处理的策略数量
    policies_processed: usize,
    /// 上次协调时间
    last_reconcile_time: Option<chrono::DateTime<chrono::Utc>>,
}

impl Reconciler {
    /// 创建新的协调器
    pub fn new(client: Client) -> Self {
        Self {
            client,
            state: Arc::new(RwLock::new(ReconcilerState::default())),
        }
    }

    /// 协调 ZeroTrustPolicy 资源
    pub async fn reconcile(&self, policy: Arc<ZeroTrustPolicy>) -> Result<Action> {
        let name = policy.name_any();
        let namespace = policy.namespace().unwrap_or_else(|| "default".into());
        
        info!("协调 ZeroTrustPolicy {}/{}", namespace, name);
        
        // 更新协调器状态
        {
            let mut state = self.state.write().await;
            state.policies_processed += 1;
            state.last_reconcile_time = Some(chrono::Utc::now());
        }
        
        // 检查策略是否已应用
        if self.is_policy_applied(&policy).await? {
            debug!("策略 {}/{} 已应用，无需更新", namespace, name);
            return Ok(Action::requeue(Duration::from_secs(300))); // 5分钟后重新检查
        }
        
        // 应用策略
        self.apply_policy(&policy).await?;
        
        // 更新策略状态
        self.update_policy_status(&policy).await?;
        
        // 10秒后重新检查
        Ok(Action::requeue(Duration::from_secs(10)))
    }
    
    /// 检查策略是否已应用
    async fn is_policy_applied(&self, policy: &ZeroTrustPolicy) -> Result<bool> {
        // 获取策略状态
        if let Some(status) = &policy.status {
            // 检查状态是否为已应用
            if status.state == "Applied" {
                return Ok(true);
            }
        }
        
        Ok(false)
    }
    
    /// 应用策略
    async fn apply_policy(&self, policy: &ZeroTrustPolicy) -> Result<()> {
        let name = policy.name_any();
        let namespace = policy.namespace().unwrap_or_else(|| "default".into());
        
        info!("应用策略 {}/{}", namespace, name);
        
        // 获取受策略影响的 Pod 列表
        let pods = self.get_affected_pods(policy).await?;
        
        // 对每个 Pod 应用策略
        for pod in pods {
            self.apply_policy_to_pod(policy, &pod).await?;
        }
        
        Ok(())
    }
    
    /// 获取受策略影响的 Pod 列表
    async fn get_affected_pods(&self, policy: &ZeroTrustPolicy) -> Result<Vec<Pod>> {
        let namespace = policy.namespace().unwrap_or_else(|| "default".into());
        let api: Api<Pod> = Api::namespaced(self.client.clone(), &namespace);
        
        // 构建标签选择器
        let label_selector = if let Some(selector) = &policy.spec.selector {
            selector.match_labels.iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(",")
        } else {
            String::new()
        };
        
        // 查询 Pod
        let params = ListParams::default().labels(&label_selector);
        let pods = api.list(&params).await.context("获取 Pod 列表失败")?;
        
        Ok(pods.items)
    }
    
    /// 对单个 Pod 应用策略
    async fn apply_policy_to_pod(&self, policy: &ZeroTrustPolicy, pod: &Pod) -> Result<()> {
        let pod_name = pod.name_any();
        let namespace = pod.namespace().unwrap_or_else(|| "default".into());
        
        debug!("对 Pod {}/{} 应用策略", namespace, pod_name);
        
        // 这里实现具体的策略应用逻辑
        // 例如：通过 sidecar 容器注入、修改 Pod 标签等方式
        
        Ok(())
    }
    
    /// 更新策略状态
    async fn update_policy_status(&self, policy: &ZeroTrustPolicy) -> Result<()> {
        let name = policy.name_any();
        let namespace = policy.namespace().unwrap_or_else(|| "default".into());
        let api: Api<ZeroTrustPolicy> = Api::namespaced(self.client.clone(), &namespace);
        
        // 创建新的状态
        let status = ZeroTrustPolicyStatus {
            state: "Applied".to_string(),
            last_updated: Some(chrono::Utc::now()),
            message: Some("策略已成功应用".to_string()),
        };
        
        // 创建状态补丁
        let patch = serde_json::json!({
            "status": status
        });
        
        // 应用补丁
        let patch_params = PatchParams::default();
        api.patch_status(&name, &patch_params, &Patch::Merge(patch))
            .await
            .context("更新策略状态失败")?;
        
        info!("已更新策略 {}/{} 状态为已应用", namespace, name);
        
        Ok(())
    }
    
    /// 处理错误
    pub fn handle_error(&self, policy: Arc<ZeroTrustPolicy>, error: &anyhow::Error) -> Action {
        let name = policy.name_any();
        let namespace = policy.namespace().unwrap_or_else(|| "default".into());
        
        error!("协调策略 {}/{} 失败: {}", namespace, name, error);
        
        // 短时间后重试
        Action::requeue(Duration::from_secs(60))
    }
    
    /// 获取协调器状态
    pub async fn get_state(&self) -> ReconcilerState {
        self.state.read().await.clone()
    }
}

/// 创建默认的协调器
pub fn create_default_reconciler(client: Client) -> Reconciler {
    Reconciler::new(client)
}