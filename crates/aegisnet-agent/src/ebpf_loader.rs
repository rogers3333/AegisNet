//! eBPF 程序加载器
//!
//! 该模块负责动态加载 eBPF 字节码，支持 CO-RE（一次编译，多内核运行）技术，
//! 并实现策略热更新，无需重启内核。

use aya::{Bpf, BpfLoader, BpfOptions, include_bytes_aligned, programs::ProgramError};
use aya::programs::{Xdp, SchedClassifier, TracePoint};
use aegisnet_ebpf::{HookManager, MapManager};
use anyhow::{Result, Context, anyhow};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};
use crate::config::ConfigManager;

/// eBPF 程序类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgramType {
    /// XDP 程序
    Xdp,
    /// TC 程序
    Tc,
    /// TracePoint 程序
    TracePoint,
}

/// eBPF 程序信息
pub struct ProgramInfo {
    /// 程序类型
    pub program_type: ProgramType,
    /// 程序名称
    pub name: String,
    /// 挂载点（如网络接口名称）
    pub attach_point: String,
    /// 程序状态
    pub active: bool,
}

/// eBPF 加载器
pub struct EbpfLoader {
    /// 配置管理器
    config: Arc<RwLock<ConfigManager>>,
    /// 钩子管理器
    hook_manager: Option<HookManager>,
    /// Map 管理器
    map_manager: Option<MapManager>,
    /// 已加载的程序信息
    loaded_programs: HashMap<String, ProgramInfo>,
    /// eBPF 字节码路径
    bpf_path: PathBuf,
}

impl EbpfLoader {
    /// 创建新的 eBPF 加载器
    pub fn new(config: Arc<RwLock<ConfigManager>>) -> Result<Self> {
        Ok(Self {
            config,
            hook_manager: None,
            map_manager: None,
            loaded_programs: HashMap::new(),
            bpf_path: PathBuf::from("/opt/aegisnet/bpf"),
        })
    }

    /// 加载 eBPF 程序
    pub async fn load_programs(&mut self) -> Result<()> {
        let config = self.config.read().await;
        let bpf_path = config.get_bpf_path()?;
        self.bpf_path = PathBuf::from(bpf_path);
        
        // 检查 eBPF 字节码文件是否存在
        if !self.bpf_path.exists() {
            return Err(anyhow!("eBPF 字节码路径不存在: {:?}", self.bpf_path));
        }
        
        info!("加载 eBPF 程序: {:?}", self.bpf_path);
        
        // 创建钩子管理器
        let hook_manager = aegisnet_ebpf::create_default_hook_manager(&self.bpf_path)
            .context("创建钩子管理器失败")?;
        
        // 获取 BPF 实例
        let bpf = hook_manager.get_bpf().clone();
        
        // 创建 Map 管理器
        let connection_map = bpf.map_mut("CONNECTION_MAP")
            .context("获取连接 Map 失败")?;
        let policy_map = bpf.map_mut("POLICY_MAP")
            .context("获取策略 Map 失败")?;
        let perf_array = bpf.map_mut("PERF_EVENTS")
            .context("获取性能事件数组失败")?;
        
        let map_manager = MapManager::new(
            connection_map.try_into()?,
            policy_map.try_into()?,
            perf_array.try_into()?,
        );
        
        // 挂载程序到指定接口
        self.attach_programs(&hook_manager, &config).await?;
        
        // 保存管理器实例
        self.hook_manager = Some(hook_manager);
        self.map_manager = Some(map_manager);
        
        info!("eBPF 程序加载完成");
        Ok(())
    }
    
    /// 挂载程序到指定接口
    async fn attach_programs(&mut self, hook_manager: &HookManager, config: &ConfigManager) -> Result<()> {
        let interfaces = config.get_network_interfaces()?;
        let mut hook_manager = hook_manager.get_bpf_mut().clone();
        
        for interface in interfaces {
            // 挂载 XDP 程序
            if let Ok(program) = hook_manager.program_mut("xdp_firewall").try_into::<Xdp>() {
                info!("挂载 XDP 程序到接口: {}", interface);
                program.attach(&interface, aya::programs::XdpFlags::default())
                    .context(format!("挂载 XDP 程序到接口 {} 失败", interface))?;
                
                self.loaded_programs.insert(
                    format!("xdp_{}", interface),
                    ProgramInfo {
                        program_type: ProgramType::Xdp,
                        name: "xdp_firewall".to_string(),
                        attach_point: interface.clone(),
                        active: true,
                    },
                );
            }
            
            // 挂载 TC 程序
            if let Ok(program) = hook_manager.program_mut("tc_policy").try_into::<SchedClassifier>() {
                info!("挂载 TC 程序到接口: {}", interface);
                program.attach(&interface, aya::programs::LinkType::Egress)
                    .context(format!("挂载 TC 程序到接口 {} 失败", interface))?;
                
                self.loaded_programs.insert(
                    format!("tc_{}", interface),
                    ProgramInfo {
                        program_type: ProgramType::Tc,
                        name: "tc_policy".to_string(),
                        attach_point: interface.clone(),
                        active: true,
                    },
                );
            }
        }
        
        // 挂载 TracePoint 程序用于连接跟踪
        if let Ok(program) = hook_manager.program_mut("tp_connect").try_into::<TracePoint>() {
            info!("挂载 TracePoint 程序用于连接跟踪");
            program.attach("sock", "inet_sock_set_state")
                .context("挂载 TracePoint 程序失败")?;
            
            self.loaded_programs.insert(
                "tp_connect".to_string(),
                ProgramInfo {
                    program_type: ProgramType::TracePoint,
                    name: "tp_connect".to_string(),
                    attach_point: "inet_sock_set_state".to_string(),
                    active: true,
                },
            );
        }
        
        Ok(())
    }
    
    /// 卸载 eBPF 程序
    pub async fn unload_programs(&mut self) -> Result<()> {
        if let Some(hook_manager) = self.hook_manager.take() {
            info!("卸载 eBPF 程序");
            hook_manager.detach_all()?;
        }
        
        self.loaded_programs.clear();
        self.map_manager = None;
        
        Ok(())
    }
    
    /// 重新加载 eBPF 程序（热更新）
    pub async fn reload_programs(&mut self) -> Result<()> {
        info!("重新加载 eBPF 程序");
        self.unload_programs().await?;
        self.load_programs().await?;
        Ok(())
    }
    
    /// 更新策略
    pub async fn update_policy(&self, policy_id: &str, value: u32) -> Result<()> {
        if let Some(map_manager) = &self.map_manager {
            debug!("更新策略: {} = {}", policy_id, value);
            map_manager.update_policy(policy_id, value)?;
        } else {
            warn!("Map 管理器未初始化，无法更新策略");
        }
        
        Ok(())
    }
    
    /// 获取已加载的程序信息
    pub fn get_loaded_programs(&self) -> &HashMap<String, ProgramInfo> {
        &self.loaded_programs
    }
    
    /// 获取 Map 管理器
    pub fn get_map_manager(&self) -> Option<&MapManager> {
        self.map_manager.as_ref()
    }
}