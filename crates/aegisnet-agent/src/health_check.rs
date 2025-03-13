//! 健康检查模块
//!
//! 该模块负责定期检查 eBPF 程序状态与内核兼容性，
//! 并提供健康状态 API 供 Kubernetes 探针调用。

use anyhow::Result;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server, StatusCode,
};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::{RwLock, Mutex};
use tokio::time::{self, Duration};
use tracing::{info, warn, error, debug};

use crate::ebpf_loader::EbpfLoader;
use crate::config::ConfigManager;

/// 健康状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// 健康
    Healthy,
    /// 降级（部分功能可用）
    Degraded,
    /// 不健康
    Unhealthy,
}

/// 健康检查结果
#[derive(Debug, Clone)]
pub struct HealthCheckResult {
    /// 健康状态
    pub status: HealthStatus,
    /// 详细信息
    pub details: String,
    /// 上次检查时间
    pub last_check: chrono::DateTime<chrono::Utc>,
    /// 已加载的 eBPF 程序数
    pub loaded_programs: usize,
    /// 内核版本
    pub kernel_version: String,
}

/// 健康检查器
pub struct HealthChecker {
    /// eBPF 加载器引用
    ebpf_loader: Arc<RwLock<EbpfLoader>>,
    /// 配置管理器引用
    config: Arc<RwLock<ConfigManager>>,
    /// 最新的健康检查结果
    latest_result: Arc<Mutex<HealthCheckResult>>,
}

impl HealthChecker {
    /// 创建新的健康检查器
    pub fn new(
        ebpf_loader: Arc<RwLock<EbpfLoader>>,
        config: Arc<RwLock<ConfigManager>>,
    ) -> Self {
        // 创建初始健康检查结果
        let initial_result = HealthCheckResult {
            status: HealthStatus::Unhealthy,
            details: "健康检查尚未运行".to_string(),
            last_check: chrono::Utc::now(),
            loaded_programs: 0,
            kernel_version: Self::get_kernel_version().unwrap_or_else(|_| "未知".to_string()),
        };
        
        Self {
            ebpf_loader,
            config,
            latest_result: Arc::new(Mutex::new(initial_result)),
        }
    }
    
    /// 启动健康检查
    pub async fn start_health_check(&self) -> Result<()> {
        // 启动定期健康检查任务
        let ebpf_loader = self.ebpf_loader.clone();
        let config = self.config.clone();
        let latest_result = self.latest_result.clone();
        
        tokio::spawn(async move {
            // 读取配置获取检查间隔
            let interval_seconds = match config.read().await.get_health_check_config().interval_seconds {
                0 => 30, // 默认 30 秒
                n => n,
            };
            
            let mut interval = time::interval(Duration::from_secs(interval_seconds));
            
            loop {
                interval.tick().await;
                
                // 执行健康检查
                let check_result = Self::perform_health_check(&ebpf_loader).await;
                
                // 更新最新结果
                if let Ok(mut result) = latest_result.lock().await {
                    *result = check_result;
                }
            }
        });
        
        // 启动 HTTP 服务器提供健康检查 API
        let latest_result = self.latest_result.clone();
        let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
        
        let serve_future = async move {
            let make_svc = make_service_fn(move |_| {
                let latest_result = latest_result.clone();
                async move {
                    Ok::<_, Infallible>(service_fn(move |req: Request<Body>| {
                        let latest_result = latest_result.clone();
                        async move {
                            let response = match req.uri().path() {
                                "/health" => Self::handle_health_check(latest_result).await,
                                "/ready" => Self::handle_readiness_check(latest_result).await,
                                _ => Response::builder()
                                    .status(StatusCode::NOT_FOUND)
                                    .body(Body::from("Not Found"))
                                    .unwrap(),
                            };
                            
                            Ok::<_, Infallible>(response)
                        }
                    }))
                }
            });
            
            info!("健康检查服务器启动在 {}", addr);
            
            if let Err(e) = Server::bind(&addr).serve(make_svc).await {
                error!("健康检查服务器错误: {}", e);
            }
        };
        
        tokio::spawn(serve_future);
        
        Ok(())
    }
    
    /// 执行健康检查
    async fn perform_health_check(ebpf_loader: &Arc<RwLock<EbpfLoader>>) -> HealthCheckResult {
        let now = chrono::Utc::now();
        let kernel_version = Self::get_kernel_version().unwrap_or_else(|_| "未知".to_string());
        
        // 检查 eBPF 程序状态
        if let Ok(loader) = ebpf_loader.read().await {
            let loaded_programs = loader.get_loaded_programs();
            
            if loaded_programs.is_empty() {
                // 没有加载任何程序
                return HealthCheckResult {
                    status: HealthStatus::Unhealthy,
                    details: "未加载任何 eBPF 程序".to_string(),
                    last_check: now,
                    loaded_programs: 0,
                    kernel_version,
                };
            }
            
            // 检查是否有不活跃的程序
            let inactive_programs: Vec<_> = loaded_programs.iter()
                .filter(|(_, info)| !info.active)
                .collect();
            
            if !inactive_programs.is_empty() {
                // 有不活跃的程序，降级状态
                return HealthCheckResult {
                    status: HealthStatus::Degraded,
                    details: format!("部分 eBPF 程序不活跃: {}", 
                        inactive_programs.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(", ")),
                    last_check: now,
                    loaded_programs: loaded_programs.len(),
                    kernel_version,
                };
            }
            
            // 所有程序都正常
            return HealthCheckResult {
                status: HealthStatus::Healthy,
                details: "所有 eBPF 程序正常运行".to_string(),
                last_check: now,
                loaded_programs: loaded_programs.len(),
                kernel_version,
            };
        }
        
        // 无法访问加载器
        HealthCheckResult {
            status: HealthStatus::Unhealthy,
            details: "无法访问 eBPF 加载器".to_string(),
            last_check: now,
            loaded_programs: 0,
            kernel_version,
        }
    }
    
    /// 处理健康检查请求
    async fn handle_health_check(latest_result: Arc<Mutex<HealthCheckResult>>) -> Response<Body> {
        let result = latest_result.lock().await;
        
        let status_code = match result.status {
            HealthStatus::Healthy => StatusCode::OK,
            HealthStatus::Degraded => StatusCode::OK, // 降级状态仍然返回 200 OK
            HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
        };
        
        let body = serde_json::json!({
            "status": format!("{:?}", result.status),
            "details": result.details,
            "last_check": result.last_check.to_rfc3339(),
            "loaded_programs": result.loaded_programs,
            "kernel_version": result.kernel_version,
        });
        
        Response::builder()
            .status(status_code)
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_string(&body).unwrap()))
            .unwrap()
    }
    
    /// 处理就绪检查请求
    async fn handle_readiness_check(latest_result: Arc<Mutex<HealthCheckResult>>) -> Response<Body> {
        let result = latest_result.lock().await;
        
        let status_code = match result.status {
            HealthStatus::Healthy | HealthStatus::Degraded => StatusCode::OK,
            HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
        };
        
        Response::builder()
            .status(status_code)
            .body(Body::empty())
            .unwrap()
    }
    
    /// 获取内核版本
    fn get_kernel_version() -> Result<String> {
        let output = std::process::Command::new("uname")
            .arg("-r")
            .output()?;
        
        let version = String::from_utf8(output.stdout)?
            .trim()
            .to_string();
        
        Ok(version)
    }
    
    /// 获取最新的健康检查结果
    pub async fn get_latest_result(&self) -> HealthCheckResult {
        self.latest_result.lock().await.clone()
    }
}

/// 创建默认的健康检查器
pub fn create_default_health_checker(
    ebpf_loader: Arc<RwLock<EbpfLoader>>,
    config: Arc<RwLock<ConfigManager>>,
) -> HealthChecker {
    HealthChecker::new(ebpf_loader, config)
}