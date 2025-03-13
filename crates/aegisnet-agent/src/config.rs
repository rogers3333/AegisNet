//! 配置管理模块
//!
//! 该模块负责加载和管理 Agent 的配置文件，支持 YAML/JSON 格式，
//! 并实现配置热重载功能，无需重启服务。

use anyhow::{Result, Context, anyhow};
use config::{Config, ConfigError, File, FileFormat};
use notify::{Watcher, RecursiveMode, Event, EventKind};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn, error, debug};

/// Agent 配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// eBPF 程序路径
    pub bpf_path: String,
    /// 网络接口列表
    pub interfaces: Vec<String>,
    /// 日志级别
    pub log_level: String,
    /// 指标服务器配置
    pub metrics: MetricsConfig,
    /// 健康检查配置
    pub health_check: HealthCheckConfig,
}

/// 指标服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// 监听地址
    pub listen_address: String,
    /// 监听端口
    pub port: u16,
    /// 收集间隔（秒）
    pub interval_seconds: u64,
}

/// 健康检查配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// 检查间隔（秒）
    pub interval_seconds: u64,
    /// 健康检查端点
    pub endpoint: String,
}

/// 配置管理器
pub struct ConfigManager {
    /// 配置文件路径
    config_path: PathBuf,
    /// 当前配置
    config: AgentConfig,
}

impl ConfigManager {
    /// 创建新的配置管理器
    pub fn new(config_path: &str) -> Result<Self> {
        let config_path = PathBuf::from(config_path);
        
        // 加载配置文件
        let config = Self::load_config(&config_path)
            .context(format!("无法加载配置文件: {:?}", config_path))?;
        
        Ok(Self {
            config_path,
            config,
        })
    }
    
    /// 加载配置文件
    fn load_config(config_path: &Path) -> Result<AgentConfig> {
        let config_file = config_path.to_str().ok_or_else(|| anyhow!("配置路径无效"))?;
        
        // 确定配置文件格式
        let format = match config_path.extension().and_then(|ext| ext.to_str()) {
            Some("yaml") | Some("yml") => FileFormat::Yaml,
            Some("json") => FileFormat::Json,
            _ => return Err(anyhow!("不支持的配置文件格式，仅支持 YAML 或 JSON"))
        };
        
        // 构建配置
        let config = Config::builder()
            .add_source(File::with_name(config_file).format(format))
            .build()
            .context("构建配置失败")?;
        
        // 反序列化为 AgentConfig
        let agent_config = config.try_deserialize::<AgentConfig>()
            .context("配置格式错误")?;
        
        Ok(agent_config)
    }
    
    /// 启动配置文件监听器，实现热重载
    pub async fn start_config_watcher(
        &self,
        ebpf_loader: Arc<RwLock<crate::ebpf_loader::EbpfLoader>>
    ) -> Result<()> {
        let config_path = self.config_path.clone();
        let config_dir = config_path.parent().unwrap_or(Path::new("."));
        
        // 创建文件系统监听器
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // 只处理配置文件的修改事件
                    if let EventKind::Modify(_) = event.kind {
                        if event.paths.iter().any(|p| p == &config_path) {
                            info!("检测到配置文件变更，正在重新加载...");
                            
                            // 尝试重新加载配置
                            match Self::load_config(&config_path) {
                                Ok(new_config) => {
                                    // 更新配置并重新加载 eBPF 程序
                                    let reload_future = async {
                                        let mut loader = ebpf_loader.write().await;
                                        if let Err(e) = loader.reload_programs().await {
                                            error!("重新加载 eBPF 程序失败: {}", e);
                                        } else {
                                            info!("配置热重载成功");
                                        }
                                    };
                                    
                                    // 在 tokio 运行时中执行异步任务
                                    tokio::spawn(reload_future);
                                }
                                Err(e) => {
                                    error!("重新加载配置文件失败: {}", e);
                                }
                            }
                        }
                    }
                }
                Err(e) => error!("监听配置文件错误: {}", e),
            }
        })?;
        
        // 开始监听配置文件目录
        watcher.watch(config_dir, RecursiveMode::NonRecursive)?;
        
        // 保持 watcher 活跃
        tokio::spawn(async move {
            // 这个任务会一直运行，保持 watcher 不被丢弃
            std::future::pending::<()>().await;
        });
        
        Ok(())
    }
    
    /// 获取 eBPF 程序路径
    pub fn get_bpf_path(&self) -> Result<&str> {
        Ok(&self.config.bpf_path)
    }
    
    /// 获取网络接口列表
    pub fn get_network_interfaces(&self) -> Result<Vec<String>> {
        Ok(self.config.interfaces.clone())
    }
    
    /// 获取指标服务器配置
    pub fn get_metrics_config(&self) -> &MetricsConfig {
        &self.config.metrics
    }
    
    /// 获取健康检查配置
    pub fn get_health_check_config(&self) -> &HealthCheckConfig {
        &self.config.health_check
    }
    
    /// 获取完整配置
    pub fn get_config(&self) -> &AgentConfig {
        &self.config
    }
}

/// 创建默认配置
pub fn create_default_config() -> AgentConfig {
    AgentConfig {
        bpf_path: "/opt/aegisnet/bpf".to_string(),
        interfaces: vec!["eth0".to_string()],
        log_level: "info".to_string(),
        metrics: MetricsConfig {
            listen_address: "0.0.0.0".to_string(),
            port: 9090,
            interval_seconds: 15,
        },
        health_check: HealthCheckConfig {
            interval_seconds: 30,
            endpoint: "/health".to_string(),
        },
    }
}