//! SPIFFE ID 模型
//!
//! 该模块实现了符合 SPIFFE 标准的身份标识符，用于服务间身份验证和授权。
//! SPIFFE（Secure Production Identity Framework For Everyone）是一个开放标准，
//! 用于在动态和异构环境中安全地识别和验证服务身份。

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use url::Url;
use uuid::Uuid;

use crate::error::{Error, Result};

/// SPIFFE ID 结构体
/// 
/// 符合 SPIFFE 标准的身份标识符，格式为：
/// spiffe://trust-domain/path
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpiffeId {
    /// 信任域（如 example.org）
    pub trust_domain: String,
    /// 路径部分（如 /service/database）
    pub path: String,
}

impl SpiffeId {
    /// 创建新的 SPIFFE ID
    pub fn new(trust_domain: &str, path: &str) -> Result<Self> {
        // 验证信任域
        if trust_domain.is_empty() {
            return Err(Error::Authentication("信任域不能为空".to_string()));
        }
        
        // 验证路径
        if !path.starts_with('/') {
            return Err(Error::Authentication("路径必须以 '/' 开头".to_string()));
        }
        
        Ok(Self {
            trust_domain: trust_domain.to_string(),
            path: path.to_string(),
        })
    }
    
    /// 从 URI 字符串解析 SPIFFE ID
    pub fn from_uri(uri: &str) -> Result<Self> {
        let url = Url::parse(uri)
            .map_err(|e| Error::Authentication(format!("无效的 SPIFFE URI: {}", e)))?;
            
        // 验证 scheme 是否为 spiffe
        if url.scheme() != "spiffe" {
            return Err(Error::Authentication(
                format!("无效的 SPIFFE URI scheme: {}", url.scheme())
            ));
        }
        
        // 获取信任域和路径
        let trust_domain = url.host_str()
            .ok_or_else(|| Error::Authentication("缺少信任域".to_string()))?;
            
        let path = url.path();
        
        Self::new(trust_domain, path)
    }
    
    /// 生成 SPIFFE URI 字符串
    pub fn uri(&self) -> String {
        format!("spiffe://{}{}", self.trust_domain, self.path)
    }
    
    /// 为工作负载生成唯一的 SPIFFE ID
    pub fn for_workload(trust_domain: &str, workload_name: &str, namespace: &str) -> Result<Self> {
        let path = format!("/ns/{}/workload/{}", namespace, workload_name);
        Self::new(trust_domain, &path)
    }
    
    /// 为节点生成唯一的 SPIFFE ID
    pub fn for_node(trust_domain: &str, node_name: &str, cluster: &str) -> Result<Self> {
        let path = format!("/cluster/{}/node/{}", cluster, node_name);
        Self::new(trust_domain, &path)
    }
    
    /// 生成随机的 SPIFFE ID（用于测试）
    pub fn random(trust_domain: &str) -> Result<Self> {
        let uuid = Uuid::new_v4();
        let path = format!("/random/{}", uuid);
        Self::new(trust_domain, &path)
    }
}

impl fmt::Display for SpiffeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}" , self.uri())
    }
}

impl FromStr for SpiffeId {
    type Err = Error;
    
    fn from_str(s: &str) -> Result<Self> {
        Self::from_uri(s)
    }
}

/// SPIFFE 身份证书
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpiffeSvid {
    /// SPIFFE ID
    pub id: SpiffeId,
    /// X.509 证书（PEM 格式）
    pub cert_pem: String,
    /// 私钥（PEM 格式）
    pub key_pem: String,
    /// 证书链（PEM 格式）
    pub chain_pem: Option<String>,
    /// 过期时间（Unix 时间戳）
    pub expires_at: i64,
}

impl SpiffeSvid {
    /// 创建新的 SPIFFE SVID
    pub fn new(
        id: SpiffeId,
        cert_pem: String,
        key_pem: String,
        chain_pem: Option<String>,
        expires_at: i64,
    ) -> Self {
        Self {
            id,
            cert_pem,
            key_pem,
            chain_pem,
            expires_at,
        }
    }
    
    /// 检查 SVID 是否已过期
    pub fn is_expired(&self) -> bool {
        let now = chrono::Utc::now().timestamp();
        self.expires_at <= now
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_spiffe_id_creation() {
        let id = SpiffeId::new("example.org", "/service/db").unwrap();
        assert_eq!(id.trust_domain, "example.org");
        assert_eq!(id.path, "/service/db");
        assert_eq!(id.uri(), "spiffe://example.org/service/db");
    }
    
    #[test]
    fn test_spiffe_id_from_uri() {
        let id = SpiffeId::from_uri("spiffe://example.org/service/web").unwrap();
        assert_eq!(id.trust_domain, "example.org");
        assert_eq!(id.path, "/service/web");
    }
    
    #[test]
    fn test_invalid_spiffe_id() {
        // 无效的 scheme
        assert!(SpiffeId::from_uri("https://example.org/service").is_err());
        
        // 缺少信任域
        assert!(SpiffeId::from_uri("spiffe:///service").is_err());
        
        // 无效的路径
        assert!(SpiffeId::new("example.org", "service").is_err());
    }
    
    #[test]
    fn test_workload_spiffe_id() {
        let id = SpiffeId::for_workload("example.org", "nginx", "default").unwrap();
        assert_eq!(id.uri(), "spiffe://example.org/ns/default/workload/nginx");
    }
}