//! 策略规则模型
//!
//! 该模块定义了 AegisNet 的网络策略规则，用于控制服务间通信的访问控制。
//! 包括基于身份的访问控制（IBAC）和基于属性的访问控制（ABAC）规则。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

use crate::error::{Error, Result};
use crate::models::spiffe::SpiffeId;

/// 策略动作
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PolicyAction {
    /// 允许通信
    Allow,
    /// 拒绝通信
    Deny,
    /// 记录通信但不阻止
    Log,
    /// 限制通信速率
    RateLimit,
}

impl fmt::Display for PolicyAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PolicyAction::Allow => write!(f, "allow"),
            PolicyAction::Deny => write!(f, "deny"),
            PolicyAction::Log => write!(f, "log"),
            PolicyAction::RateLimit => write!(f, "rate_limit"),
        }
    }
}

impl FromStr for PolicyAction {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "allow" => Ok(PolicyAction::Allow),
            "deny" => Ok(PolicyAction::Deny),
            "log" => Ok(PolicyAction::Log),
            "rate_limit" => Ok(PolicyAction::RateLimit),
            _ => Err(Error::Policy(format!("无效的策略动作: {}", s))),
        }
    }
}

/// 协议类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Protocol {
    /// TCP 协议
    TCP,
    /// UDP 协议
    UDP,
    /// ICMP 协议
    ICMP,
    /// 所有协议
    All,
}

impl fmt::Display for Protocol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Protocol::TCP => write!(f, "tcp"),
            Protocol::UDP => write!(f, "udp"),
            Protocol::ICMP => write!(f, "icmp"),
            Protocol::All => write!(f, "all"),
        }
    }
}

impl FromStr for Protocol {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(Protocol::TCP),
            "udp" => Ok(Protocol::UDP),
            "icmp" => Ok(Protocol::ICMP),
            "all" => Ok(Protocol::All),
            _ => Err(Error::Policy(format!("无效的协议类型: {}", s))),
        }
    }
}

/// 端口范围
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PortRange {
    /// 起始端口
    pub from: u16,
    /// 结束端口
    pub to: u16,
}

impl PortRange {
    /// 创建新的端口范围
    pub fn new(from: u16, to: u16) -> Result<Self> {
        if from > to {
            return Err(Error::Policy(format!(
                "无效的端口范围: {} > {}", from, to
            )));
        }
        
        Ok(Self { from, to })
    }
    
    /// 创建单一端口的范围
    pub fn single(port: u16) -> Self {
        Self { from: port, to: port }
    }
    
    /// 创建包含所有端口的范围
    pub fn all() -> Self {
        Self { from: 0, to: 65535 }
    }
    
    /// 检查端口是否在范围内
    pub fn contains(&self, port: u16) -> bool {
        port >= self.from && port <= self.to
    }
}

impl fmt::Display for PortRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.from == self.to {
            write!(f, "{}", self.from)
        } else if self.from == 0 && self.to == 65535 {
            write!(f, "all")
        } else {
            write!(f, "{}-{}", self.from, self.to)
        }
    }
}

/// 网络策略规则
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// 策略 ID
    pub id: String,
    /// 策略名称
    pub name: String,
    /// 策略描述
    pub description: Option<String>,
    /// 源身份（SPIFFE ID）
    pub source: Option<SpiffeId>,
    /// 目标身份（SPIFFE ID）
    pub destination: Option<SpiffeId>,
    /// 源 IP 地址
    pub source_ip: Option<IpAddr>,
    /// 目标 IP 地址
    pub destination_ip: Option<IpAddr>,
    /// 协议
    pub protocol: Protocol,
    /// 源端口范围
    pub source_ports: Vec<PortRange>,
    /// 目标端口范围
    pub destination_ports: Vec<PortRange>,
    /// 策略动作
    pub action: PolicyAction,
    /// 优先级（数字越小优先级越高）
    pub priority: u32,
    /// 标签（用于分组和筛选）
    pub labels: HashMap<String, String>,
    /// 是否启用
    pub enabled: bool,
}

impl NetworkPolicy {
    /// 创建新的网络策略
    pub fn new(
        id: String,
        name: String,
        protocol: Protocol,
        action: PolicyAction,
        priority: u32,
    ) -> Self {
        Self {
            id,
            name,
            description: None,
            source: None,
            destination: None,
            source_ip: None,
            destination_ip: None,
            protocol,
            source_ports: vec![],
            destination_ports: vec![],
            action,
            priority,
            labels: HashMap::new(),
            enabled: true,
        }
    }
    
    /// 设置策略描述
    pub fn with_description(mut self, description: &str) -> Self {
        self.description = Some(description.to_string());
        self
    }
    
    /// 设置源身份
    pub fn with_source(mut self, source: SpiffeId) -> Self {
        self.source = Some(source);
        self
    }
    
    /// 设置目标身份
    pub fn with_destination(mut self, destination: SpiffeId) -> Self {
        self.destination = Some(destination);
        self
    }
    
    /// 设置源 IP 地址
    pub fn with_source_ip(mut self, ip: IpAddr) -> Self {
        self.source_ip = Some(ip);
        self
    }
    
    /// 设置目标 IP 地址
    pub fn with_destination_ip(mut self, ip: IpAddr) -> Self {
        self.destination_ip = Some(ip);
        self
    }
    
    /// 添加源端口范围
    pub fn add_source_port_range(&mut self, range: PortRange) {
        self.source_ports.push(range);
    }
    
    /// 添加目标端口范围
    pub fn add_destination_port_range(&mut self, range: PortRange) {
        self.destination_ports.push(range);
    }
    
    /// 添加标签
    pub fn add_label(&mut self, key: &str, value: &str) {
        self.labels.insert(key.to_string(), value.to_string());
    }
    
    /// 启用策略
    pub fn enable(&mut self) {
        self.enabled = true;
    }
    
    /// 禁用策略
    pub fn disable(&mut self) {
        self.enabled = false;
    }
    
    /// 检查策略是否匹配给定的连接
    pub fn matches(
        &self,
        source_id: Option<&SpiffeId>,
        destination_id: Option<&SpiffeId>,
        source_ip: IpAddr,
        destination_ip: IpAddr,
        protocol: Protocol,
        source_port: u16,
        destination_port: u16,
    ) -> bool {
        // 如果策略未启用，则不匹配
        if !self.enabled {
            return false;
        }
        
        // 检查协议
        if self.protocol != Protocol::All && self.protocol != protocol {
            return false;
        }
        
        // 检查源身份
        if let Some(ref policy_source) = self.source {
            if let Some(conn_source) = source_id {
                if policy_source != conn_source {
                    return false;
                }
            } else {
                return false;
            }
        }
        
        // 检查目标身份
        if let Some(ref policy_dest) = self.destination {
            if let Some(conn_dest) = destination_id {
                if policy_dest != conn_dest {
                    return false;
                }
            } else {
                return false;
            }
        }
        
        // 检查源 IP
        if let Some(ref policy_source_ip) = self.source_ip {
            if *policy_source_ip != source_ip {
                return false;
            }
        }
        
        // 检查目标 IP
        if let Some(ref policy_dest_ip) = self.destination_ip {
            if *policy_dest_ip != destination_ip {
                return false;
            }
        }
        
        // 检查源端口
        if !self.source_ports.is_empty() {
            let mut port_match = false;
            for range in &self.source_ports {
                if range.contains(source_port) {
                    port_match = true;
                    break;
                }
            }
            if !port_match {
                return false;
            }
        }
        
        // 检查目标端口
        if !self.destination_ports.is_empty() {
            let mut port_match = false;
            for range in &self.destination_ports {
                if range.contains(destination_port) {
                    port_match = true;
                    break;
                }
            }
            if !port_match {
                return false;
            }
        }
        
        // 所有条件都匹配
        true
    }
}

/// 策略集合
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PolicySet {
    /// 策略列表
    policies: Vec<NetworkPolicy>,
}

impl PolicySet {
    /// 创建新的策略集合
    pub fn new() -> Self {
        Self {
            policies: Vec::new(),
        }
    }
    
    /// 添加策略
    pub fn add_policy(&mut self, policy: NetworkPolicy) {
        self.policies.push(policy);
        // 按优先级排序（数字越小优先级越高）
        self.policies.sort_by_key(|p| p.priority);
    }
    
    /// 移除策略
    pub fn remove_policy(&mut self, id: &str) -> Option<NetworkPolicy> {
        if let Some(index) = self.policies.iter().position(|p| p.id == id) {
            Some(self.policies.remove(index))
        } else {
            None
        }
    }
    
    /// 获取策略
    pub fn get_policy(&self, id: &str) -> Option<&NetworkPolicy> {
        self.policies.iter().find(|p| p.id == id)
    }
    
    /// 获取所有策略
    pub fn get_policies(&self) -> &[NetworkPolicy] {
        &self.policies
    }
    
    /// 获取匹配的策略
    pub fn get_matching_policy(
        &self,
        source_id: Option<&SpiffeId>,
        destination_id: Option<&SpiffeId>,
        source_ip: IpAddr,
        destination_ip: IpAddr,
        protocol: Protocol,
        source_port: u16,
        destination_port: u16,
    ) -> Option<&NetworkPolicy> {
        // 按优先级顺序检查每个策略
        for policy in &self.policies {
            if policy.matches(
                source_id,
                destination_id,
                source_ip,
                destination_ip,
                protocol,
                source_port,
                destination_port,
            ) {
                return Some(policy);
            }
        }
        
        None
    }
    
    /// 评估连接是否允许
    pub fn evaluate(
        &self,
        source_id: Option<&SpiffeId>,
        destination_id: Option<&SpiffeId>,
        source_ip: IpAddr,
        destination_ip: IpAddr,
        protocol: Protocol,
        source_port: u16,
        destination_port: u16,
    )