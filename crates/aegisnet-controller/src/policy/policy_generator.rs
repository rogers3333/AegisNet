//! 策略生成器模块
//!
//! 该模块负责将 ZeroTrustPolicy CRD 转换为可执行的策略规则。
//! 支持生成 Wasm 格式的策略模块，用于在数据平面执行。

use anyhow::{Result, Context};
use std::sync::Arc;
use tracing::{info, warn, error};
use crate::policy::crd_watcher::{ZeroTrustPolicy, PolicyAction};
use aegisnet_common::policy::{PolicyRule, PolicyDefinition};

/// 策略生成器
pub struct PolicyGenerator {
    /// 策略版本
    version: String,
}

impl PolicyGenerator {
    /// 创建新的策略生成器
    pub fn new(version: String) -> Self {
        Self { version }
    }

    /// 从 ZeroTrustPolicy CRD 生成策略定义
    pub fn generate_policy_definition(&self, policy: &ZeroTrustPolicy) -> Result<PolicyDefinition> {
        let name = policy.name_any();
        info!("为策略 {} 生成策略定义", name);

        // 创建基本策略定义
        let mut definition = PolicyDefinition {
            name: name.clone(),
            version: self.version.clone(),
            source: policy.spec.source.service.clone(),
            source_namespace: policy.spec.source.namespace.clone(),
            destination: policy.spec.destination.service.clone(),
            destination_namespace: policy.spec.destination.namespace.clone(),
            default_action: match policy.spec.action {
                PolicyAction::Allow => aegisnet_common::policy::Action::Allow,
                PolicyAction::Deny => aegisnet_common::policy::Action::Deny,
                PolicyAction::RequireAuth => aegisnet_common::policy::Action::RequireAuth,
            },
            rules: Vec::new(),
        };

        // 转换策略规则
        for rule in &policy.spec.rules {
            let action = match rule.action {
                PolicyAction::Allow => aegisnet_common::policy::Action::Allow,
                PolicyAction::Deny => aegisnet_common::policy::Action::Deny,
                PolicyAction::RequireAuth => aegisnet_common::policy::Action::RequireAuth,
            };

            definition.rules.push(PolicyRule {
                name: rule.name.clone(),
                condition: rule.condition.clone(),
                action,
            });
        }

        Ok(definition)
    }

    /// 生成 Wasm 策略模块
    pub async fn generate_wasm_module(&self, policy: &ZeroTrustPolicy) -> Result<Vec<u8>> {
        // 生成策略定义
        let definition = self.generate_policy_definition(policy)
            .context("生成策略定义失败")?;
        
        // 使用 aegisnet-common 库生成 Wasm 模块
        let wasm_bytes = aegisnet_common::policy::compile_policy_to_wasm(&definition)
            .context("编译策略为 Wasm 失败")?;
        
        info!("成功为策略 {} 生成 Wasm 模块，大小: {} 字节", 
              policy.name_any(), wasm_bytes.len());
        
        Ok(wasm_bytes)
    }

    /// 验证策略有效性
    pub fn validate_policy(&self, policy: &ZeroTrustPolicy) -> Result<()> {
        // 验证源服务和目标服务
        if policy.spec.source.service.is_empty() || policy.spec.destination.service.is_empty() {
            return Err(anyhow::anyhow!("源服务和目标服务不能为空"));
        }

        // 验证规则条件
        for rule in &policy.spec.rules {
            if rule.condition.is_empty() {
                warn!("策略 {} 中的规则 {} 没有条件", policy.name_any(), rule.name);
            }
        }

        Ok(())
    }
}

/// 创建默认的策略生成器
pub fn create_default_generator() -> PolicyGenerator {
    PolicyGenerator::new("1.0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::crd_watcher::{ZeroTrustPolicySpec, ServiceSelector, PolicyRule as CrdPolicyRule};

    #[test]
    fn test_policy_generation() {
        // 创建测试策略
        let policy = ZeroTrustPolicy {
            spec: ZeroTrustPolicySpec {
                source: ServiceSelector {
                    namespace: "default".to_string(),
                    service: "frontend".to_string(),
                    port: Some(8080),
                },
                destination: ServiceSelector {
                    namespace: "default".to_string(),
                    service: "backend".to_string(),
                    port: Some(9000),
                },
                action: PolicyAction::Allow,
                rules: vec![
                    CrdPolicyRule {
                        name: "test-rule".to_string(),
                        condition: "method == 'GET'".to_string(),
                        action: PolicyAction::Allow,
                    }
                ],
            },
            status: None,
        };

        // 创建策略生成器
        let generator = create_default_generator();

        // 生成策略定义
        let definition = generator.generate_policy_definition(&policy).unwrap();

        // 验证生成的定义
        assert_eq!(definition.source, "frontend");
        assert_eq!(definition.destination, "backend");
        assert_eq!(definition.rules.len(), 1);
        assert_eq!(definition.rules[0].name, "test-rule");
    }
}