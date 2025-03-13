//! 自定义资源定义模块
//!
//! 该模块定义了 AegisNet 的自定义资源类型，如 ZeroTrustPolicy、NetworkIdentity 等，
//! 这些资源类型将被注册到 Kubernetes 集群中，供用户创建和管理。

use chrono::{DateTime, Utc};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::LabelSelector;
use kube::CustomResource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 零信任策略规范
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug)]
#[kube(group = "aegisnet.io", version = "v1alpha1", kind = "ZeroTrustPolicy", namespaced)]
#[kube(status = "ZeroTrustPolicyStatus")]
#[kube(printcolumn = {name = "状态", type = "string", jsonPath = ".status.state"})]
#[kube(printcolumn = {name = "上次更新", type = "date", jsonPath = ".status.lastUpdated"})]
pub struct ZeroTrustPolicySpec {
    /// 策略选择器，用于选择应用策略的工作负载
    pub selector: Option<LabelSelector>,
    
    /// 策略规则
    pub rules: Vec<PolicyRule>,
    
    /// 策略优先级，数字越小优先级越高
    #[serde(default = "default_priority")]
    pub priority: i32,
    
    /// 策略模式：Enforce（强制执行）或 Audit（仅审计）
    #[serde(default = "default_mode")]
    pub mode: String,
    
    /// 策略标签
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// 策略规则
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PolicyRule {
    /// 规则名称
    pub name: String,
    
    /// 规则类型：Ingress（入站）或 Egress（出站）
    pub rule_type: String,
    
    /// 源端点
    pub from: Option<Endpoint>,
    
    /// 目标端点
    pub to: Option<Endpoint>,
    
    /// 端口列表
    pub ports: Option<Vec<Port>>,
    
    /// 协议列表
    pub protocols: Option<Vec<String>>,
    
    /// 动作：Allow（允许）或 Deny（拒绝）
    pub action: String,
    
    /// 日志级别
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

/// 端点定义
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Endpoint {
    /// 端点类型：Pod、Service、CIDR 等
    pub endpoint_type: String,
    
    /// 端点选择器
    pub selector: Option<LabelSelector>,
    
    /// CIDR 地址范围
    pub cidr: Option<String>,
    
    /// 命名空间列表
    pub namespaces: Option<Vec<String>>,
    
    /// 身份标识
    pub identity: Option<String>,
}

/// 端口定义
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Port {
    /// 端口号
    pub port: u16,
    
    /// 端口范围结束（可选）
    pub end_port: Option<u16>,
    
    /// 端口名称（可选）
    pub name: Option<String>,
}

/// 策略状态
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ZeroTrustPolicyStatus {
    /// 状态：Pending、Applied、Error
    pub state: String,
    
    /// 上次更新时间
    pub last_updated: Option<DateTime<Utc>>,
    
    /// 状态消息
    pub message: Option<String>,
}

/// 默认优先级
fn default_priority() -> i32 {
    1000
}

/// 默认模式
fn default_mode() -> String {
    "Enforce".to_string()
}

/// 默认日志级别
fn default_log_level() -> String {
    "Info".to_string()
}

/// 网络身份自定义资源
#[derive(CustomResource, Serialize, Deserialize, Clone, Debug)]
#[kube(group = "aegisnet.io", version = "v1alpha1", kind = "NetworkIdentity", namespaced)]
#[kube(status = "NetworkIdentityStatus")]
#[kube(printcolumn = {name = "状态", type = "string", jsonPath = ".status.state"})]
#[kube(printcolumn = {name = "上次更新", type = "date", jsonPath = ".status.lastUpdated"})]
pub struct NetworkIdentitySpec {
    /// 身份名称
    pub name: String,
    
    /// 身份选择器
    pub selector: LabelSelector,
    
    /// 身份属性
    pub attributes: HashMap<String, String>,
    
    /// 身份有效期
    pub valid_until: Option<DateTime<Utc>>,
}

/// 网络身份状态
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct NetworkIdentityStatus {
    /// 状态：Pending、Active、Expired、Error
    pub state: String,
    
    /// 上次更新时间
    pub last_updated: Option<DateTime<Utc>>,
    
    /// 状态消息
    pub message: Option<String>,
}