//! ZeroTrustPolicy CRD 监听器
//!
//! 该模块实现对 Kubernetes ZeroTrustPolicy 自定义资源的监听和处理。
//! 使用 kube-rs 框架的 Controller 模式实现资源变更的监听和协调。

use anyhow::Result;
use kube::api::{Api, ListParams, ResourceExt};
use kube::runtime::controller::{Action, Controller};
use kube::runtime::watcher;
use kube::Client;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::Duration;
use tracing::{error, info, warn};

/// ZeroTrustPolicy 自定义资源定义
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZeroTrustPolicy {
    /// 策略规则
    pub spec: ZeroTrustPolicySpec,
    /// 策略状态
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<ZeroTrustPolicyStatus>,
}

/// ZeroTrustPolicy 规格定义
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZeroTrustPolicySpec {
    /// 源服务
    pub source: ServiceSelector,
    /// 目标服务
    pub destination: ServiceSelector,
    /// 策略类型（允许/拒绝）
    pub action: PolicyAction,
    /// 策略规则
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub rules: Vec<PolicyRule>,
}

/// 服务选择器
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSelector {
    /// 命名空间
    pub namespace: String,
    /// 服务名称
    pub service: String,
    /// 端口
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

/// 策略动作
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PolicyAction {
    /// 允许通信
    Allow,
    /// 拒绝通信
    Deny,
    /// 需要认证
    RequireAuth,
}

/// 策略规则
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRule {
    /// 规则名称
    pub name: String,
    /// 规则条件
    pub condition: String,
    /// 规则动作
    pub action: PolicyAction,
}

/// ZeroTrustPolicy 状态
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ZeroTrustPolicyStatus {
    /// 策略状态
    pub state: String,
    /// 上次更新时间
    pub last_updated: String,
    /// 错误信息
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// CRD 监听器
pub struct CrdWatcher {
    /// Kubernetes 客户端
    client: Client,
    /// 策略缓存
    policies: Arc<RwLock<Vec<ZeroTrustPolicy>>>,
}

impl CrdWatcher {
    /// 创建新的 CRD 监听器
    pub fn new(client: Client) -> Self {
        Self {
            client,
            policies: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// 启动监听任务
    pub async fn start(&self) -> Result<()> {
        let policies = self.policies.clone();
        let client = self.client.clone();

        // 创建 ZeroTrustPolicy API
        let api: Api<ZeroTrustPolicy> = Api::all(client.clone());

        // 创建控制器
        Controller::new(api, ListParams::default())
            .run(
                move |obj, _| Self::reconcile(obj, policies.clone()),
                move |err, _| Self::error_policy(err),
                client,
            )
            .for_each(|_| async {})
            .await;

        Ok(())
    }

    /// 协调函数，处理 CRD 变更
    async fn reconcile(
        policy: Arc<ZeroTrustPolicy>,
        policies: Arc<RwLock<Vec<ZeroTrustPolicy>>>,
    ) -> Result<Action, anyhow::Error> {
        let name = policy.name_any();
        info!("处理 ZeroTrustPolicy 变更: {}", name);

        // 更新策略缓存
        let mut policies_write = policies.write().await;
        
        // 移除同名策略
        policies_write.retain(|p| p.name_any() != name);
        
        // 添加新策略
        policies_write.push((*policy).clone());
        
        info!("策略 {} 已更新，当前策略数量: {}", name, policies_write.len());

        // 每 5 分钟重新协调一次
        Ok(Action::requeue(Duration::from_secs(300)))
    }

    /// 错误处理策略
    fn error_policy(error: &anyhow::Error) -> Action {
        error!("处理 ZeroTrustPolicy 时出错: {}", error);
        // 出错后 30 秒重试
        Action::requeue(Duration::from_secs(30))
    }

    /// 获取所有策略
    pub async fn get_policies(&self) -> Vec<ZeroTrustPolicy> {
        self.policies.read().await.clone()
    }

    /// 获取特定策略
    pub async fn get_policy(&self, name: &str) -> Option<ZeroTrustPolicy> {
        self.policies.read().await.iter()
            .find(|p| p.name_any() == name)
            .cloned()
    }
}

/// 创建默认的 CRD 监听器
pub async fn create_default_watcher() -> Result<CrdWatcher> {
    let client = Client::try_default().await?;
    Ok(CrdWatcher::new(client))
}