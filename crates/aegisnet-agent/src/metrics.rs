//! 指标收集与导出模块
//!
//! 该模块负责收集 eBPF 程序的统计数据，如流量计数、策略命中次数等，
//! 并通过 Prometheus 格式导出监控指标。

use anyhow::Result;
use hyper::{
    service::{make_service_fn, service_fn},
    Body, Request, Response, Server,
};
use prometheus::{Encoder, TextEncoder, Registry, IntCounter, IntGauge, Opts};
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{self, Duration};
use tracing::{info, warn, error, debug};

use crate::ebpf_loader::EbpfLoader;

/// 指标收集器
pub struct MetricsCollector {
    /// eBPF 加载器引用
    ebpf_loader: Arc<RwLock<EbpfLoader>>,
    /// Prometheus 注册表
    registry: Registry,
    /// 连接计数器
    connections_total: IntCounter,
    /// 策略命中计数器
    policy_hits_total: IntCounter,
    /// 丢弃的数据包计数器
    dropped_packets_total: IntCounter,
    /// 当前活跃连接数
    active_connections: IntGauge,
    /// 已加载的 eBPF 程序数
    loaded_programs: IntGauge,
}

impl MetricsCollector {
    /// 创建新的指标收集器
    pub fn new(ebpf_loader: Arc<RwLock<EbpfLoader>>) -> Self {
        // 创建 Prometheus 注册表
        let registry = Registry::new();
        
        // 创建指标
        let connections_total = IntCounter::new(
            "aegisnet_connections_total", 
            "Total number of connections processed"
        ).unwrap();
        
        let policy_hits_total = IntCounter::new(
            "aegisnet_policy_hits_total", 
            "Total number of policy rule hits"
        ).unwrap();
        
        let dropped_packets_total = IntCounter::new(
            "aegisnet_dropped_packets_total", 
            "Total number of dropped packets"
        ).unwrap();
        
        let active_connections = IntGauge::new(
            "aegisnet_active_connections", 
            "Current number of active connections"
        ).unwrap();
        
        let loaded_programs = IntGauge::new(
            "aegisnet_loaded_programs", 
            "Number of loaded eBPF programs"
        ).unwrap();
        
        // 注册指标
        registry.register(Box::new(connections_total.clone())).unwrap();
        registry.register(Box::new(policy_hits_total.clone())).unwrap();
        registry.register(Box::new(dropped_packets_total.clone())).unwrap();
        registry.register(Box::new(active_connections.clone())).unwrap();
        registry.register(Box::new(loaded_programs.clone())).unwrap();
        
        Self {
            ebpf_loader,
            registry,
            connections_total,
            policy_hits_total,
            dropped_packets_total,
            active_connections,
            loaded_programs,
        }
    }
    
    /// 启动指标收集服务器
    pub async fn start_metrics_server(&self) -> Result<()> {
        // 启动指标收集任务
        let ebpf_loader = self.ebpf_loader.clone();
        let connections_total = self.connections_total.clone();
        let policy_hits_total = self.policy_hits_total.clone();
        let dropped_packets_total = self.dropped_packets_total.clone();
        let active_connections = self.active_connections.clone();
        let loaded_programs = self.loaded_programs.clone();
        
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(15));
            
            loop {
                interval.tick().await;
                
                // 收集指标
                if let Ok(loader) = ebpf_loader.read().await {
                    // 更新已加载程序数量
                    loaded_programs.set(loader.get_loaded_programs().len() as i64);
                    
                    // 从 eBPF maps 中收集指标
                    if let Some(map_manager) = loader.get_map_manager() {
                        // 这里需要根据实际情况从 map_manager 中获取指标数据
                        // 例如：遍历连接表，统计活跃连接数
                        // 由于这需要与 eBPF 程序的具体实现相关联，这里只是示例
                        
                        // 模拟更新指标（实际应用中应从 eBPF maps 中读取）
                        connections_total.inc();
                        policy_hits_total.inc_by(5);
                        
                        // 更新活跃连接数（实际应用中应计算真实值）
                        active_connections.set(10);
                    }
                }
            }
        });
        
        // 启动 HTTP 服务器提供 Prometheus 指标端点
        let registry = self.registry.clone();
        let addr = SocketAddr::from(([0, 0, 0, 0], 9090));
        
        let serve_future = async move {
            let make_svc = make_service_fn(move |_| {
                let registry = registry.clone();
                async move {
                    Ok::<_, Infallible>(service_fn(move |_: Request<Body>| {
                        let registry = registry.clone();
                        async move {
                            let encoder = TextEncoder::new();
                            let metric_families = registry.gather();
                            let mut buffer = vec![];
                            encoder.encode(&metric_families, &mut buffer).unwrap();
                            
                            let response = Response::builder()
                                .status(200)
                                .header("Content-Type", encoder.format_type())
                                .body(Body::from(buffer))
                                .unwrap();
                            
                            Ok::<_, Infallible>(response)
                        }
                    }))
                }
            });
            
            info!("指标服务器启动在 {}", addr);
            
            if let Err(e) = Server::bind(&addr).serve(make_svc).await {
                error!("指标服务器错误: {}", e);
            }
        };
        
        tokio::spawn(serve_future);
        
        Ok(())
    }
    
    /// 手动增加连接计数
    pub fn increment_connections(&self, count: u64) {
        self.connections_total.inc_by(count);
    }
    
    /// 手动增加策略命中计数
    pub fn increment_policy_hits(&self, count: u64) {
        self.policy_hits_total.inc_by(count);
    }
    
    /// 手动增加丢弃的数据包计数
    pub fn increment_dropped_packets(&self, count: u64) {
        self.dropped_packets_total.inc_by(count);
    }
    
    /// 设置当前活跃连接数
    pub fn set_active_connections(&self, count: i64) {
        self.active_connections.set(count);
    }
    
    /// 获取 Prometheus 注册表
    pub fn get_registry(&self) -> &Registry {
        &self.registry
    }
}

/// 创建默认的指标收集器
pub fn create_default_metrics_collector(
    ebpf_loader: Arc<RwLock<EbpfLoader>>
) -> MetricsCollector {
    MetricsCollector::new(ebpf_loader)
}