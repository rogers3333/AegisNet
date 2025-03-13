//! WasmEdge 策略执行引擎
//!
//! 该模块实现基于 WebAssembly 的策略执行引擎，使用 WasmEdge 运行时加载和执行策略。
//! 支持动态加载和热更新策略，无需重启服务。

use anyhow::Result;
use std::collections::HashMap;
use std::path::Path;
use wasmedge_sdk::{params, Vm, Module, Config, Store, Executor, ImportObject};

/// 策略类型
pub enum PolicyType {
    /// 允许通信
    Allow,
    /// 拒绝通信
    Deny,
    /// 限制通信（带条件）
    Restrict,
}

/// 策略执行结果
pub enum PolicyResult {
    /// 允许通信
    Allow,
    /// 拒绝通信
    Deny,
    /// 需要进一步检查
    NeedInspection,
}

/// Wasm 策略执行器
pub struct WasmPolicyEngine {
    /// 策略缓存
    policy_cache: HashMap<String, Module>,
    /// WasmEdge 虚拟机
    vm: Vm,
}

impl WasmPolicyEngine {
    /// 创建新的策略执行器
    pub fn new() -> Result<Self> {
        // 配置 WasmEdge 运行时
        let config = Config::create()?;
        let store = Store::create()?;
        let executor = Executor::create(Some(&config), None)?;
        let vm = Vm::create(Some(executor), Some(store), None)?;

        Ok(Self {
            policy_cache: HashMap::new(),
            vm,
        })
    }

    /// 加载策略
    pub fn load_policy(&mut self, policy_id: &str, wasm_path: &Path) -> Result<()> {
        // 加载 Wasm 模块
        let module = Module::from_file(None, wasm_path)?;
        self.policy_cache.insert(policy_id.to_string(), module);
        Ok(())
    }

    /// 执行策略
    pub fn execute_policy(&self, policy_id: &str, context: &[u8]) -> Result<PolicyResult> {
        if let Some(module) = self.policy_cache.get(policy_id) {
            // 注册模块并执行
            let mut vm = self.vm.clone();
            vm.register_module(None, module)?;

            // 调用策略函数
            let result = vm.run_function("evaluate", params!(context))?;
            
            // 解析结果
            match result.get_i32() {
                0 => Ok(PolicyResult::Allow),
                1 => Ok(PolicyResult::Deny),
                _ => Ok(PolicyResult::NeedInspection),
            }
        } else {
            Err(anyhow::anyhow!("策略不存在: {}", policy_id))
        }
    }

    /// 卸载策略
    pub fn unload_policy(&mut self, policy_id: &str) -> bool {
        self.policy_cache.remove(policy_id).is_some()
    }

    /// 获取已加载策略列表
    pub fn list_policies(&self) -> Vec<String> {
        self.policy_cache.keys().cloned().collect()
    }
}

/// 创建默认的策略执行器
pub fn create_default_policy_engine() -> Result<WasmPolicyEngine> {
    WasmPolicyEngine::new()
}