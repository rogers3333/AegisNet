//! 控制平面工具函数
//!
//! 该模块封装了控制平面常用的工具函数，提高代码复用性。
//! 包括日志、配置、网络和安全相关的辅助函数。

use anyhow::Result;
use std::path::Path;
use std::time::{Duration, SystemTime};
use tracing::{debug, info, warn};

/// 配置加载器
pub struct ConfigLoader {
    /// 配置文件路径
    config_path: String,
    /// 上次加载时间
    last_loaded: SystemTime,
}

impl ConfigLoader {
    /// 创建新的配置加载器
    pub fn new(config_path: &str) -> Self {
        Self {
            config_path: config_path.to_string(),
            last_loaded: SystemTime::now(),
        }
    }
    
    /// 加载配置文件
    pub fn load<T: serde::de::DeserializeOwned>(&mut self) -> Result<T> {
        let path = Path::new(&self.config_path);
        if !path.exists() {
            return Err(anyhow::anyhow!("配置文件不存在: {}", self.config_path));
        }
        
        let content = std::fs::read_to_string(path)?;
        let config: T = serde_json::from_str(&content)?;
        
        self.last_loaded = SystemTime::now();
        debug!("从 {} 加载配置成功", self.config_path);
        
        Ok(config)
    }
    
    /// 获取上次加载时间
    pub fn last_loaded(&self) -> SystemTime {
        self.last_loaded
    }
}

/// 格式化持续时间为人类可读的字符串
pub fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    
    if seconds < 60 {
        return format!("{} 秒", seconds);
    }
    
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{} 分钟 {} 秒", minutes, seconds % 60);
    }
    
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{} 小时 {} 分钟", hours, minutes % 60);
    }
    
    let days = hours / 24;
    format!("{} 天 {} 小时", days, hours % 24)
}

/// 检查端口是否可用
pub async fn is_port_available(host: &str, port: u16) -> bool {
    match tokio::net::TcpStream::connect(format!("{host}:{port}")).await {
        Ok(_) => false, // 端口已被占用
        Err(_) => true,  // 端口可用
    }
}

/// 生成随机 ID
pub fn generate_random_id(prefix: &str, length: usize) -> String {
    use rand::{Rng, thread_rng};
    use rand::distributions::Alphanumeric;
    
    let random_part: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(length)
        .map(char::from)
        .collect();
    
    format!("{}-{}", prefix, random_part)
}

/// 计算字符串的 SHA-256 哈希
pub fn sha256_hash(input: &str) -> String {
    use sha2::{Sha256, Digest};
    
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();
    
    format!("{:x}", result)
}

/// 重试执行异步函数
pub async fn retry_async<F, Fut, T, E>(f: F, retries: usize, delay: Duration) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = std::result::Result<T, E>>,
    E: std::fmt::Display + std::error::Error + Send + Sync + 'static,
{
    let mut last_error = None;
    
    for attempt in 0..retries {
        match f().await {
            Ok(value) => return Ok(value),
            Err(e) => {
                warn!("尝试 {} 失败: {}", attempt + 1, e);
                last_error = Some(anyhow::anyhow!(e));
                if attempt < retries - 1 {
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("重试失败，但没有错误信息")))
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(30)), "30 秒");
        assert_eq!(format_duration(Duration::from_secs(90)), "1 分钟 30 秒");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1 小时 0 分钟");
        assert_eq!(format_duration(Duration::from_secs(86400)), "1 天 0 小时");
    }
    
    #[test]
    fn test_generate_random_id() {
        let id1 = generate_random_id("test", 8);
        let id2 = generate_random_id("test", 8);
        
        assert!(id1.starts_with("test-"));
        assert_eq!(id1.len(), 13); // "test-" + 8 chars
        assert_ne!(id1, id2); // 随机 ID 应该不同
    }
    
    #[test]
    fn test_sha256_hash() {
        let hash = sha256_hash("hello");
        assert_eq!(hash, "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824");
    }
}