//! eBPF Map 定义与管理
//!
//! 该模块定义了 eBPF 程序使用的各种 Map 结构，用于内核态和用户态之间的数据交换。
//! 包括策略映射、连接跟踪、身份缓存等。

use aya::maps::{HashMap, MapData, MapError, PerfEventArray};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::Arc;

/// 连接标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ConnectionId {
    /// 源 IP 地址
    pub src_ip: [u8; 16],
    /// 目标 IP 地址
    pub dst_ip: [u8; 16],
    /// 源端口
    pub src_port: u16,
    /// 目标端口
    pub dst_port: u16,
    /// 协议类型
    pub protocol: u8,
}

/// 连接状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    /// 新建连接
    New,
    /// 已建立连接
    Established,
    /// 关闭中的连接
    Closing,
    /// 已关闭连接
    Closed,
}

/// 连接信息
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    /// 连接状态
    pub state: ConnectionState,
    /// 连接创建时间戳
    pub created_at: u64,
    /// 最后活动时间戳
    pub last_seen: u64,
    /// 关联的 SPIFFE ID
    pub spiffe_id: Option<String>,
    /// 应用的策略 ID
    pub policy_id: Option<String>,
}

/// eBPF Map 管理器
pub struct MapManager {
    /// 连接跟踪表
    connection_map: Arc<HashMap<MapData, ConnectionId, ConnectionInfo>>,
    /// 策略映射表
    policy_map: Arc<HashMap<MapData, String, u32>>,
    /// 性能事件数组，用于日志和指标收集
    perf_array: Arc<PerfEventArray<MapData>>,
}

impl MapManager {
    /// 创建新的 Map 管理器
    pub fn new(
        connection_map: HashMap<MapData, ConnectionId, ConnectionInfo>,
        policy_map: HashMap<MapData, String, u32>,
        perf_array: PerfEventArray<MapData>,
    ) -> Self {
        Self {
            connection_map: Arc::new(connection_map),
            policy_map: Arc::new(policy_map),
            perf_array: Arc::new(perf_array),
        }
    }

    /// 更新连接信息
    pub fn update_connection(
        &self,
        conn_id: &ConnectionId,
        conn_info: &ConnectionInfo,
    ) -> Result<(), MapError> {
        self.connection_map.insert(conn_id, conn_info, 0)
    }

    /// 获取连接信息
    pub fn get_connection(&self, conn_id: &ConnectionId) -> Result<Option<ConnectionInfo>, MapError> {
        self.connection_map.get(conn_id, 0)
    }

    /// 删除连接信息
    pub fn remove_connection(&self, conn_id: &ConnectionId) -> Result<(), MapError> {
        self.connection_map.remove(conn_id)
    }

    /// 更新策略映射
    pub fn update_policy(&self, policy_id: &str, value: u32) -> Result<(), MapError> {
        self.policy_map.insert(policy_id, &value, 0)
    }

    /// 获取策略值
    pub fn get_policy(&self, policy_id: &str) -> Result<Option<u32>, MapError> {
        self.policy_map.get(policy_id, 0)
    }
}

/// 创建 IPv4 地址的连接 ID
pub fn create_ipv4_connection_id(
    src_ip: Ipv4Addr,
    dst_ip: Ipv4Addr,
    src_port: u16,
    dst_port: u16,
    protocol: u8,
) -> ConnectionId {
    let mut src_ip_bytes = [0u8; 16];
    let mut dst_ip_bytes = [0u8; 16];

    // 将 IPv4 地址转换为 IPv6 映射地址格式
    src_ip_bytes[10..12].copy_from_slice(&[0xFF, 0xFF]);
    src_ip_bytes[12..16].copy_from_slice(&src_ip.octets());

    dst_ip_bytes[10..12].copy_from_slice(&[0xFF, 0xFF]);
    dst_ip_bytes[12..16].copy_from_slice(&dst_ip.octets());

    ConnectionId {
        src_ip: src_ip_bytes,
        dst_ip: dst_ip_bytes,
        src_port,
        dst_port,
        protocol,
    }
}

/// 创建 IPv6 地址的连接 ID
pub fn create_ipv6_connection_id(
    src_ip: Ipv6Addr,
    dst_ip: Ipv6Addr,
    src_port: u16,
    dst_port: u16,
    protocol: u8,
) -> ConnectionId {
    ConnectionId {
        src_ip: src_ip.octets(),
        dst_ip: dst_ip.octets(),
        src_port,
        dst_port,
        protocol,
    }
}