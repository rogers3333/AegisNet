//! 策略优化器模块
//!
//! 该模块负责从Prometheus获取监控数据，使用GNN模型分析流量模式，
//! 并生成优化后的策略规则，形成完整的策略优化闭环。

use anyhow::{Result, Context};
use std::sync::Arc;
use tokio::time::{Duration, interval};
use tracing::{info, warn, error, debug};
use prometheus::{Registry, Gauge};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ai::gnn_model::{GnnModel, ModelPrediction};
use crate::policy::{ZeroTrustPolicy, PolicyGenerator};

/// Prometheus查询结果
#[derive(Debug, Deserialize)]
struct PrometheusResponse {
    status: String,
    data: PrometheusData,
}

#[derive(Debug, Deserialize)]
struct PrometheusData {
    result_type: String,
    result: Vec<PrometheusResult>,
}

#[derive(Debug, Deserialize)]
struct PrometheusResult {
    metric: std::collections::HashMap<String, String>,
    value: (f64, String),
}

/// 策略优化配置
#[derive(Debug, Clone)]
pub struct PolicyOptimizerConfig {
    /// Prometheus服务器地址
    pub prometheus_url: String,
    /// 优化间隔（秒）
    pub optimization_interval: u64,
    /// 流量阈值（触发优化的最小流量量）
    pub traffic_threshold: f64,
    /// 是否启用自动优化
    pub auto_optimize: bool,
}

impl Default for PolicyOptimizerConfig {
    fn default() -> Self {
        Self {
            prometheus_url: "http://prometheus:9090".to_string(),
            optimization_interval: 3600, // 默认每小时优化一次
            traffic_threshold: 1000.0,    // 默认至少1000个请求才触发优化
            auto_optimize: true,
        }
    }
}

/// 策略优化器
pub struct PolicyOptimizer {
    /// 配置
    config: PolicyOptimizerConfig,
    /// GNN模型
    model: Arc<GnnModel>,
    /// 策略生成器
    policy_generator: Arc<PolicyGenerator>,
    /// HTTP客户端
    http_client: HttpClient,
    /// 优化计数器（用于监控）
    optimization_counter: Gauge,
}

impl PolicyOptimizer {
    /// 创建新的策略优化器
    pub fn new(
        config: PolicyOptimizerConfig,
        model: Arc<GnnModel>,
        policy_generator: Arc<PolicyGenerator>,
        registry: &Registry,
    ) -> Result<Self> {
        // 创建HTTP客户端
        let http_client = HttpClient::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("创建HTTP客户端失败")?;
        
        // 创建监控指标
        let optimization_counter = Gauge::new(
            "aegisnet_policy_optimizations_total",
            "策略优化总次数"
        ).context("创建监控指标失败")?;
        
        registry.register(Box::new(optimization_counter.clone()))
            .context("注册监控指标失败")?;
        
        Ok(Self {
            config,
            model,
            policy_generator,
            http_client,
            optimization_counter,
        })
    }
    
    /// 启动策略优化任务
    pub async fn start_optimization_loop(&self) -> Result<()> {
        if !self.config.auto_optimize {
            info!("自动策略优化已禁用");
            return Ok(());
        }
        
        let optimizer = Arc::new(self.clone());
        let interval_duration = Duration::from_secs(self.config.optimization_interval);
        
        tokio::spawn(async move {
            let mut interval_timer = interval(interval_duration);
            loop {
                interval_timer.tick().await;
                if let Err(e) = optimizer.optimize_policies().await {
                    error!("策略优化失败: {}", e);
                }
            }
        });
        
        info!("策略优化任务已启动，间隔: {}秒", self.config.optimization_interval);
        Ok(())
    }
    
    /// 从Prometheus获取流量数据
    async fn fetch_traffic_data(&self, source: &str, destination: &str) -> Result<Vec<Vec<f32>>> {
        let query = format!(
            "sum(rate(aegisnet_traffic_bytes_total{{source=\"{}\",destination=\"{}\"}}[5m])) by (protocol, port)",
            source, destination
        );
        
        let url = format!("{}/api/v1/query", self.config.prometheus_url);
        
        debug!("查询Prometheus: {}", query);
        
        let response = self.http_client.get(&url)
            .query(&[("query", query)])
            .send()
            .await
            .context("请求Prometheus失败")?;
        
        let prometheus_data: PrometheusResponse = response.json().await
            .context("解析Prometheus响应失败")?;
        
        if prometheus_data.status != "success" {
            return Err(anyhow::anyhow!("Prometheus查询失败: {}", prometheus_data.status));
        }
        
        // 将Prometheus数据转换为模型输入特征
        let mut traffic_samples = Vec::new();
        
        for result in prometheus_data.data.result {
            let mut features = Vec::new();
            
            // 提取协议特征
            if let Some(protocol) = result.metric.get("protocol") {
                match protocol.as_str() {
                    "http" => features.push(1.0),
                    "https" => features.push(2.0),
                    "tcp" => features.push(3.0),
                    "udp" => features.push(4.0),
                    _ => features.push(0.0),
                }
            } else {
                features.push(0.0);
            }
            
            // 提取端口特征
            if let Some(port) = result.metric.get("port") {
                if let Ok(port_num) = port.parse::<f32>() {
                    features.push(port_num / 65535.0); // 归一化端口号
                } else {
                    features.push(0.0);
                }
            } else {
                features.push(0.0);
            }
            
            // 提取流量特征
            if let Ok(traffic) = result.value.1.parse::<f32>() {
                features.push(traffic);
            } else {
                features.push(0.0);
            }
            
            // 添加更多特征...
            // 为了简化示例，这里只使用了几个基本特征
            
            traffic_samples.push(features);
        }
        
        if traffic_samples.is_empty() {
            warn!("未找到 {} -> {} 的流量数据", source, destination);
        } else {
            info!("获取到 {} 个流量样本", traffic_samples.len());
        }
        
        Ok(traffic_samples)
    }
    
    /// 优化所有策略
    async fn optimize_policies(&self) -> Result<()> {
        info!("开始策略优化循环");
        
        // 获取所有策略
        let policies = self.fetch_current_policies().await?;
        
        let mut optimized_count = 0;
        
        for policy in policies {
            let source = &policy.spec.source.service;
            let destination = &policy.spec.destination.service;
            
            // 获取流量数据
            let traffic_samples = self.fetch_traffic_data(source, destination).await?;
            
            if traffic_samples.is_empty() {
                debug!("跳过优化 {} -> {}: 无流量数据", source, destination);
                continue;
            }
            
            // 检查流量是否达到阈值
            let total_traffic: f32 = traffic_samples.iter()
                .flat_map(|sample| sample.iter())
                .sum();
            
            if total_traffic < self.config.traffic_threshold as f32 {
                debug!("跳过优化 {} -> {}: 流量低于阈值", source, destination);
                continue;
            }
            
            // 使用GNN模型生成优化规则
            match self.model.generate_minimal_rules(source, destination, &traffic_samples).await {
                Ok(optimized_rules) => {
                    info!("为 {} -> {} 生成了 {} 条优化规则", source, destination, optimized_rules.len());
                    
                    // 应用优化后的规则
                    if let Err(e) = self.apply_optimized_rules(&policy, optimized_rules).await {
                        error!("应用优化规则失败: {}", e);
                    } else {
                        optimized_count += 1;
                    }
                },
                Err(e) => {
                    error!("生成优化规则失败: {}", e);
                }
            }
        }
        
        // 更新优化计数器
        self.optimization_counter.inc_by(optimized_count as f64);
        
        info!("策略优化完成，优化了 {} 个策略", optimized_count);
        Ok(())
    }
    
    /// 获取当前所有策略
    async fn fetch_current_policies(&self) -> Result<Vec<ZeroTrustPolicy>> {
        // 这里应该实现从Kubernetes API获取策略的逻辑
        // 为了示例，这里返回一个空列表
        warn!("获取当前策略功能尚未完全实现");
        Ok(Vec::new())
    }
    
    /// 应用优化后的规则
    async fn apply_optimized_rules(&self, policy: &ZeroTrustPolicy, optimized_rules: Vec<String>) -> Result<()> {
        // 这里应该实现将优化后的规则应用到策略的逻辑
        // 包括更新Kubernetes CRD和分发策略到Agent
        info!("应用优化规则到策略: {}", policy.name_any());
        
        // 为了示例，这里只是记录日志
        for (i, rule) in optimized_rules.iter().enumerate() {
            debug!("规则 {}: {}", i+1, rule);
        }
        
        Ok(())
    }
}

/// 创建默认的策略优化器
pub fn create_default_optimizer(
    model: Arc<GnnModel>,
    policy_generator: Arc<PolicyGenerator>,
    registry: &Registry,
) -> Result<PolicyOptimizer> {
    PolicyOptimizer::new(
        PolicyOptimizerConfig::default(),
        model,
        policy_generator,
        registry,
    )
}

impl Clone for PolicyOptimizer {
    fn clone(&self) -> Self {
        // 注意：这个克隆实现不会克隆计数器
        // 这是有意的，因为我们希望所有克隆共享同一个计数器
        Self {
            config: self.config.clone(),
            model: self.model.clone(),
            policy_generator: self.policy_generator.clone(),
            http_client: self.http_client.clone(),
            optimization_counter: self.optimization_counter.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::data_preprocessor::create_default_preprocessor;
    use crate::ai::gnn_model::create_default_model;
    use crate::policy::create_default_generator;
    
    #[tokio::test]
    async fn test_policy_optimizer() {
        // 创建测试依赖
        let preprocessor = Arc::new(create_default_preprocessor());
        let model = Arc::new(create_default_model(preprocessor));
        let policy_generator = Arc::new(create_default_generator());
        let registry = Registry::new();
        
        // 创建优化器
        let optimizer = create_default_optimizer(
            model,
            policy_generator,
            &registry,
        ).unwrap();
        
        // 测试优化逻辑
        // 注意：这只是一个基本的测试框架，实际测试需要更多的模拟和断言
        assert!(optimizer.config.auto_optimize);
    }
}